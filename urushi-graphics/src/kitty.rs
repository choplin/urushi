//! Stateless and retained Kitty graphics presentation.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use urushi::{Key, Position, ResolvedView, Size, View};
use urushi_terminal::{
    Command, CommandWriter, ControlString, CursorMove, Position as TerminalPosition, TerminalOutput,
};

use crate::{GraphicPlacement, Image, RgbaRaster};

const RAW_CHUNK_BYTES: usize = 3_072;
const MAX_UPLOADED_IMAGES: usize = 64;
const MAX_UPLOADED_BYTES: usize = 64 * 1024 * 1024;

/// Renderer-owned Kitty protocol state for reconciling successive image frames.
///
/// A lifecycle retains terminal upload and placement identities, but owns no
/// terminal connection, session, frame scheduling, or application state. The
/// renderer keeps this value across frames for one terminal presentation and
/// calls [`Self::clear`] before resize teardown or exit.
///
/// [`Self::present`] commits its candidate state only after every command and
/// the final flush succeed. After a partial failure, the next presentation
/// first deletes every image that may have reached the terminal, then rebuilds
/// the desired frame with fresh protocol identifiers.
#[derive(Debug)]
pub struct KittyLifecycle {
    committed: KittyState,
    possible_image_ids: HashSet<u32>,
    needs_reset: bool,
    cursor_restore_pending: bool,
}

impl Default for KittyLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl KittyLifecycle {
    #[must_use]
    pub fn new() -> Self {
        Self {
            committed: KittyState::new(),
            possible_image_ids: HashSet::new(),
            needs_reset: false,
            cursor_restore_pending: false,
        }
    }

    /// Reconciles and presents one immutable desired image frame.
    ///
    /// `resolved` must be the result of resolving `view`; the lifecycle finds
    /// the Image snapshots embedded in that View and matches their keys to the
    /// resolved anchors without requiring a parallel image list.
    ///
    /// Images without a complete, non-empty resolved placement are absent from
    /// the desired frame. Their previous Kitty placements are deleted while
    /// their uploads may remain cached for later reuse.
    pub fn present(
        &mut self,
        view: &View,
        resolved: &ResolvedView,
        terminal: &mut (impl CommandWriter + ?Sized),
    ) -> io::Result<()> {
        self.restore_cursor(terminal)?;
        let desired = DesiredFrame::new(resolved, crate::image::collect(view));
        let mut operations = Vec::new();
        let mut candidate = if self.needs_reset {
            let mut image_ids = self.possible_image_ids();
            image_ids.sort_unstable();
            image_ids.dedup();
            operations.extend(
                image_ids
                    .into_iter()
                    .map(|image_id| KittyOperation::DeleteImage { image_id }),
            );
            KittyState::after(&self.committed)
        } else {
            self.committed.clone()
        };
        operations.extend(candidate.reconcile(&desired)?);

        for operation in operations {
            if let Err(error) = write_operation(
                operation,
                &desired,
                terminal,
                &mut self.cursor_restore_pending,
            ) {
                self.retain_possible_state(&candidate);
                return Err(error);
            }
        }
        if let Err(error) = TerminalOutput::flush(terminal) {
            self.retain_possible_state(&candidate);
            return Err(error);
        }

        self.committed = candidate;
        self.possible_image_ids.clear();
        self.needs_reset = false;
        self.cursor_restore_pending = false;
        Ok(())
    }

    /// Deletes every Kitty image that may belong to this lifecycle.
    ///
    /// Call this when a resize invalidates placement geometry and when the
    /// renderer releases the terminal presentation. A failed cleanup remains
    /// pending so a later `clear` or [`Self::present`] retries conservatively.
    pub fn clear(&mut self, terminal: &mut (impl CommandWriter + ?Sized)) -> io::Result<()> {
        self.restore_cursor(terminal)?;
        let mut image_ids = self.possible_image_ids();
        image_ids.sort_unstable();
        image_ids.dedup();
        for image_id in image_ids {
            if let Err(error) = write_delete_image(image_id, terminal) {
                self.needs_reset = true;
                return Err(error);
            }
        }
        if let Err(error) = TerminalOutput::flush(terminal) {
            self.needs_reset = true;
            return Err(error);
        }

        self.committed.clear();
        self.possible_image_ids.clear();
        self.needs_reset = false;
        Ok(())
    }

    fn restore_cursor(&mut self, terminal: &mut (impl CommandWriter + ?Sized)) -> io::Result<()> {
        if !self.cursor_restore_pending {
            return Ok(());
        }
        terminal.write_command(Command::RestoreCursorPosition)?;
        TerminalOutput::flush(terminal)?;
        self.cursor_restore_pending = false;
        Ok(())
    }

    fn possible_image_ids(&self) -> Vec<u32> {
        self.committed
            .uploaded
            .values()
            .map(|uploaded| uploaded.image_id)
            .chain(self.possible_image_ids.iter().copied())
            .collect()
    }

    fn retain_possible_state(&mut self, candidate: &KittyState) {
        self.possible_image_ids.extend(
            self.committed
                .uploaded
                .values()
                .map(|uploaded| uploaded.image_id),
        );
        self.possible_image_ids.extend(
            candidate
                .uploaded
                .values()
                .map(|uploaded| uploaded.image_id),
        );
        self.committed.next_image_id = self.committed.next_image_id.max(candidate.next_image_id);
        self.committed.next_placement_id = self
            .committed
            .next_placement_id
            .max(candidate.next_placement_id);
        self.committed.generation = self.committed.generation.max(candidate.generation);
        self.needs_reset = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Uploaded {
    image_id: u32,
    last_used: u64,
    byte_len: usize,
    retire_when_unused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisiblePlacement {
    placement_id: u32,
    image_id: u32,
    placement: DesiredPlacement,
}

#[derive(Debug, Clone, Default)]
struct KittyState {
    uploaded: HashMap<Key, Uploaded>,
    visible: HashMap<Key, VisiblePlacement>,
    next_image_id: u32,
    next_placement_id: u32,
    generation: u64,
}

impl KittyState {
    fn new() -> Self {
        Self {
            next_image_id: 1,
            next_placement_id: 1,
            ..Self::default()
        }
    }

    fn after(previous: &Self) -> Self {
        Self {
            next_image_id: previous.next_image_id,
            next_placement_id: previous.next_placement_id,
            generation: previous.generation,
            ..Self::default()
        }
    }

    fn clear(&mut self) {
        self.uploaded.clear();
        self.visible.clear();
    }

    fn reconcile(&mut self, frame: &DesiredFrame<'_>) -> io::Result<Vec<KittyOperation>> {
        self.generation = self.generation.saturating_add(1);
        let mut operations = Vec::new();
        let mut retired_images = HashSet::new();
        let mut adopted_images = HashSet::new();
        let desired_placements = frame
            .placements
            .iter()
            .map(|placement| placement.key)
            .collect::<HashSet<_>>();
        let mut stale = self
            .visible
            .iter()
            .filter(|(key, _)| !desired_placements.contains(key))
            .map(|(key, placement)| (*key, placement.placement_id))
            .collect::<Vec<_>>();
        stale.sort_by_key(|(_, placement_id)| *placement_id);
        for (key, _) in stale {
            if let Some(placement) = self.visible.remove(&key) {
                operations.push(KittyOperation::DeletePlacement {
                    image_id: placement.image_id,
                    placement_id: placement.placement_id,
                });
            }
        }

        for desired in &frame.placements {
            let uploaded = if let Some(uploaded) = self.uploaded.get_mut(&desired.asset_key) {
                uploaded.last_used = self.generation;
                *uploaded
            } else {
                let image_id = self.allocate_image_id()?;
                let uploaded = Uploaded {
                    image_id,
                    last_used: self.generation,
                    byte_len: frame.assets[&desired.asset_key].bytes().len(),
                    retire_when_unused: false,
                };
                self.uploaded.insert(desired.asset_key, uploaded);
                operations.push(KittyOperation::Upload {
                    image_id,
                    asset_key: desired.asset_key,
                });
                uploaded
            };

            match self.visible.get(&desired.key).copied() {
                Some(current)
                    if current.image_id == uploaded.image_id && current.placement == *desired => {}
                Some(current) => {
                    if current.image_id != uploaded.image_id {
                        retired_images.insert(current.image_id);
                        adopted_images.insert(uploaded.image_id);
                        operations.push(KittyOperation::DeletePlacement {
                            image_id: current.image_id,
                            placement_id: current.placement_id,
                        });
                    }
                    let placement_id = current.placement_id;
                    operations.push(KittyOperation::Place {
                        image_id: uploaded.image_id,
                        placement_id,
                        placement: *desired,
                    });
                    self.visible.insert(
                        desired.key,
                        VisiblePlacement {
                            placement_id,
                            image_id: uploaded.image_id,
                            placement: *desired,
                        },
                    );
                }
                None => {
                    adopted_images.insert(uploaded.image_id);
                    let placement_id = self.allocate_placement_id()?;
                    operations.push(KittyOperation::Place {
                        image_id: uploaded.image_id,
                        placement_id,
                        placement: *desired,
                    });
                    self.visible.insert(
                        desired.key,
                        VisiblePlacement {
                            placement_id,
                            image_id: uploaded.image_id,
                            placement: *desired,
                        },
                    );
                }
            }
        }

        for uploaded in self.uploaded.values_mut() {
            if adopted_images.contains(&uploaded.image_id) {
                uploaded.retire_when_unused = false;
            } else if retired_images.contains(&uploaded.image_id) {
                uploaded.retire_when_unused = true;
            }
        }

        let referenced = self
            .visible
            .values()
            .map(|placement| placement.image_id)
            .collect::<HashSet<_>>();
        let mut unused = self
            .uploaded
            .iter()
            .filter(|(_, uploaded)| !referenced.contains(&uploaded.image_id))
            .map(|(key, uploaded)| (*key, *uploaded))
            .collect::<Vec<_>>();
        unused.sort_by_key(|(_, uploaded)| (uploaded.last_used, uploaded.image_id));
        let mut retained_count = self.uploaded.len();
        let mut retained_bytes = self.uploaded.values().fold(0usize, |total, uploaded| {
            total.saturating_add(uploaded.byte_len)
        });
        let mut evicted = HashSet::new();
        for (_, uploaded) in &unused {
            if uploaded.retire_when_unused {
                retained_count -= 1;
                retained_bytes = retained_bytes.saturating_sub(uploaded.byte_len);
                evicted.insert(uploaded.image_id);
            }
        }
        for (_, uploaded) in &unused {
            if retained_count <= MAX_UPLOADED_IMAGES && retained_bytes <= MAX_UPLOADED_BYTES {
                break;
            }
            if evicted.contains(&uploaded.image_id) {
                continue;
            }
            retained_count -= 1;
            retained_bytes = retained_bytes.saturating_sub(uploaded.byte_len);
            evicted.insert(uploaded.image_id);
        }
        for (key, uploaded) in unused {
            if !evicted.contains(&uploaded.image_id) {
                continue;
            }
            self.uploaded.remove(&key);
            operations.push(KittyOperation::DeleteImage {
                image_id: uploaded.image_id,
            });
        }
        Ok(operations)
    }

    fn allocate_image_id(&mut self) -> io::Result<u32> {
        let image_id = self.next_image_id;
        self.next_image_id = image_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("the Kitty image identifier space is exhausted"))?;
        Ok(image_id)
    }

    fn allocate_placement_id(&mut self) -> io::Result<u32> {
        let placement_id = self.next_placement_id;
        self.next_placement_id = placement_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("the Kitty placement identifier space is exhausted"))?;
        Ok(placement_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DesiredPlacement {
    key: Key,
    asset_key: Key,
    origin: Position,
    size: Size,
}

struct DesiredFrame<'a> {
    assets: HashMap<Key, &'a RgbaRaster>,
    placements: Vec<DesiredPlacement>,
}

impl<'a> DesiredFrame<'a> {
    fn new(view: &ResolvedView, images: impl IntoIterator<Item = &'a Image>) -> Self {
        let mut assets = HashMap::new();
        let mut placements = Vec::new();
        for image in images {
            let Some(placement) = image.placement(view).filter(|placement| {
                placement.is_within_resolved_view() && !placement.size().is_empty()
            }) else {
                continue;
            };
            assets
                .entry(placement.raster().key())
                .or_insert(placement.raster());
            placements.push(DesiredPlacement {
                key: placement.key(),
                asset_key: placement.raster().key(),
                origin: placement.origin(),
                size: placement.size(),
            });
        }
        Self { assets, placements }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KittyOperation {
    Upload {
        image_id: u32,
        asset_key: Key,
    },
    DeletePlacement {
        image_id: u32,
        placement_id: u32,
    },
    Place {
        image_id: u32,
        placement_id: u32,
        placement: DesiredPlacement,
    },
    DeleteImage {
        image_id: u32,
    },
}

/// Queues every fully visible image through a terminal command connection.
///
/// This is the stateless first-frame path: each call transmits the complete
/// RGBA asset with the Kitty graphics protocol and displays it in the anchor
/// resolved with the fallback cells. The terminal backend owns APC framing and
/// physical output.
pub fn render_kitty<'a>(
    view: &ResolvedView,
    images: impl IntoIterator<Item = &'a Image>,
    terminal: &mut (impl CommandWriter + ?Sized),
) -> io::Result<()> {
    for placement in images
        .into_iter()
        .filter_map(|image| image.placement(view))
        .filter(|placement| placement.is_within_resolved_view() && !placement.size().is_empty())
    {
        write_placement(placement, terminal)?;
    }
    TerminalOutput::flush(terminal)
}

fn write_placement(
    placement: GraphicPlacement<'_>,
    terminal: &mut (impl CommandWriter + ?Sized),
) -> io::Result<()> {
    let raster = placement.raster();
    let mut chunks = raster.bytes().chunks(RAW_CHUNK_BYTES).peekable();
    let mut payload = String::with_capacity(RAW_CHUNK_BYTES / 3 * 4 + 128);
    let column = usize::try_from(placement.origin().x).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a Kitty image column cannot be negative",
        )
    })?;
    let row = usize::try_from(placement.origin().y).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a Kitty image row cannot be negative",
        )
    })?;

    terminal.write_command(Command::SaveCursorPosition)?;
    terminal.write_command(Command::MoveCursor(CursorMove::To(TerminalPosition::new(
        column, row,
    ))))?;
    let mut first = true;
    while let Some(chunk) = chunks.next() {
        payload.clear();
        let more = u8::from(chunks.peek().is_some());
        if first {
            write!(
                payload,
                "Ga=T,f=32,s={},v={},c={},r={},C=1,q=2,m={more};",
                raster.size().width(),
                raster.size().height(),
                placement.size().width(),
                placement.size().height(),
            )
            .map_err(io::Error::other)?;
        } else {
            write!(payload, "Gm={more};").map_err(io::Error::other)?;
        }
        STANDARD.encode_string(chunk, &mut payload);
        let payload = ControlString::try_from(payload.as_str())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        terminal.write_command(Command::ApplicationProgram(payload))?;
        first = false;
    }
    terminal.write_command(Command::RestoreCursorPosition)
}

fn write_operation(
    operation: KittyOperation,
    frame: &DesiredFrame<'_>,
    terminal: &mut (impl CommandWriter + ?Sized),
    cursor_restore_pending: &mut bool,
) -> io::Result<()> {
    match operation {
        KittyOperation::Upload {
            image_id,
            asset_key,
        } => {
            let raster = frame.assets.get(&asset_key).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "a desired Kitty raster asset is missing",
                )
            })?;
            write_upload(image_id, raster, terminal)
        }
        KittyOperation::DeletePlacement {
            image_id,
            placement_id,
        } => write_application_program(
            &format!("Ga=d,d=i,i={image_id},p={placement_id},q=2"),
            terminal,
        ),
        KittyOperation::Place {
            image_id,
            placement_id,
            placement,
        } => write_retained_placement(
            image_id,
            placement_id,
            placement,
            terminal,
            cursor_restore_pending,
        ),
        KittyOperation::DeleteImage { image_id } => write_delete_image(image_id, terminal),
    }
}

fn write_upload(
    image_id: u32,
    raster: &RgbaRaster,
    terminal: &mut (impl CommandWriter + ?Sized),
) -> io::Result<()> {
    let mut chunks = raster.bytes().chunks(RAW_CHUNK_BYTES).peekable();
    let mut payload = String::with_capacity(RAW_CHUNK_BYTES / 3 * 4 + 128);
    let mut first = true;
    while let Some(chunk) = chunks.next() {
        payload.clear();
        let more = u8::from(chunks.peek().is_some());
        if first {
            write!(
                payload,
                "Ga=t,f=32,s={},v={},i={image_id},q=2,m={more};",
                raster.size().width(),
                raster.size().height(),
            )
            .map_err(io::Error::other)?;
        } else {
            write!(payload, "Gm={more};").map_err(io::Error::other)?;
        }
        STANDARD.encode_string(chunk, &mut payload);
        write_application_program(&payload, terminal)?;
        first = false;
    }
    Ok(())
}

fn write_retained_placement(
    image_id: u32,
    placement_id: u32,
    placement: DesiredPlacement,
    terminal: &mut (impl CommandWriter + ?Sized),
    cursor_restore_pending: &mut bool,
) -> io::Result<()> {
    let column = usize::try_from(placement.origin.x).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a Kitty image column cannot be negative",
        )
    })?;
    let row = usize::try_from(placement.origin.y).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a Kitty image row cannot be negative",
        )
    })?;

    terminal.write_command(Command::SaveCursorPosition)?;
    *cursor_restore_pending = true;
    let result = terminal
        .write_command(Command::MoveCursor(CursorMove::To(TerminalPosition::new(
            column, row,
        ))))
        .and_then(|()| {
            write_application_program(
                &format!(
                    "Ga=p,i={image_id},p={placement_id},c={},r={},C=1,q=2",
                    placement.size.width(),
                    placement.size.height(),
                ),
                terminal,
            )
        });
    match result {
        Ok(()) => terminal.write_command(Command::RestoreCursorPosition),
        Err(error) => {
            let _ = terminal.write_command(Command::RestoreCursorPosition);
            Err(error)
        }
    }
}

fn write_delete_image(
    image_id: u32,
    terminal: &mut (impl CommandWriter + ?Sized),
) -> io::Result<()> {
    write_application_program(&format!("Ga=d,d=I,i={image_id},q=2"), terminal)
}

fn write_application_program(
    payload: &str,
    terminal: &mut (impl CommandWriter + ?Sized),
) -> io::Result<()> {
    let payload = ControlString::try_from(payload)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    terminal.write_command(Command::ApplicationProgram(payload))
}

#[cfg(test)]
mod tests {
    use urushi::{
        Available, Canvas, CanvasContext, CanvasItem, Position, Size, TextStyle, View, resolve,
    };
    use urushi_terminal::{CommandWriter, TerminalOutput, backend::ansi::AnsiWriter};

    use crate::{CellSize, ImagePresentation, PixelSize};

    use super::*;

    #[derive(Default)]
    struct RecordingTerminal {
        commands: Vec<String>,
        command_count: usize,
        flushes: usize,
        fail_command: Option<usize>,
        fail_flush: bool,
    }

    impl RecordingTerminal {
        fn fail_command(index: usize) -> Self {
            Self {
                fail_command: Some(index),
                ..Self::default()
            }
        }

        fn application_programs(&self) -> impl Iterator<Item = &str> {
            self.commands
                .iter()
                .filter_map(|command| command.strip_prefix("APC "))
        }
    }

    impl TerminalOutput for RecordingTerminal {
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            if self.fail_flush {
                Err(io::Error::other("injected flush failure"))
            } else {
                Ok(())
            }
        }
    }

    impl CommandWriter for RecordingTerminal {
        fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
            let index = self.command_count;
            self.command_count += 1;
            if self.fail_command == Some(index) {
                return Err(io::Error::other("injected command failure"));
            }
            self.commands.push(match command {
                Command::ApplicationProgram(payload) => format!("APC {}", payload.as_str()),
                Command::SaveCursorPosition => "save".to_owned(),
                Command::RestoreCursorPosition => "restore".to_owned(),
                Command::MoveCursor(CursorMove::To(position)) => {
                    format!("move {},{}", position.column(), position.row())
                }
                other => format!("{other:?}"),
            });
            Ok(())
        }
    }

    fn image_view(image: &Image, column: usize) -> (View, ResolvedView) {
        let image = ImagePresentation::new().compose(image, CellSize::new(2, 1));
        let view = if column == 0 {
            image
        } else {
            View::row(
                urushi::VerticalAlign::Top,
                [View::text(" ".repeat(column), TextStyle::new()), image],
            )
        };
        let resolved = resolve(&view, Available::NONE).unwrap();
        (view, resolved)
    }

    fn empty_view() -> (View, ResolvedView) {
        let view = View::empty();
        let resolved = resolve(&view, Available::NONE).unwrap();
        (view, resolved)
    }

    fn present_image(
        lifecycle: &mut KittyLifecycle,
        image: &Image,
        column: usize,
        terminal: &mut (impl CommandWriter + ?Sized),
    ) -> io::Result<()> {
        let (view, resolved) = image_view(image, column);
        lifecycle.present(&view, &resolved, terminal)
    }

    fn present_empty(
        lifecycle: &mut KittyLifecycle,
        terminal: &mut (impl CommandWriter + ?Sized),
    ) -> io::Result<()> {
        let (view, resolved) = empty_view();
        lifecycle.present(&view, &resolved, terminal)
    }

    #[test]
    fn writes_one_rgba_image_at_the_resolved_anchor() {
        let image = Image::rgba(
            "placement",
            "asset",
            PixelSize::new(2, 1),
            [255, 0, 0, 255, 0, 255, 0, 255],
        )
        .unwrap()
        .fallback("alt");
        let view = ImagePresentation::new().compose(&image, CellSize::new(3, 2));
        let resolved = resolve(&view, Available::NONE).unwrap();
        let mut output = AnsiWriter::new(Vec::new());

        render_kitty(&resolved, [&image], &mut output).unwrap();

        let output = String::from_utf8(output.into_inner()).unwrap();
        assert!(output.starts_with("\x1b7\x1b[1;1H\x1b_G"));
        assert!(output.contains("a=T,f=32,s=2,v=1,c=3,r=2,C=1,q=2,m=0;"));
        assert!(output.ends_with("\x1b\\\x1b8"));
        assert!(output.contains(&STANDARD.encode(image.raster().bytes())));
    }

    #[test]
    fn omits_an_image_whose_anchor_is_outside_the_resolved_view() {
        #[derive(Debug, Clone, PartialEq)]
        struct PartlyVisibleImage(View);

        impl CanvasItem for PartlyVisibleImage {
            fn draw(&self, context: &mut CanvasContext) {
                context.view(Position::new(-1, 0), self.0.clone(), Some(2), Some(2));
            }
        }

        let image = Image::rgba(
            "placement",
            "asset",
            PixelSize::new(1, 1),
            [255, 255, 255, 255],
        )
        .unwrap();
        let image_view = ImagePresentation::new().compose(&image, CellSize::new(2, 2));
        let view = View::canvas(
            Canvas::new()
                .extent(Size::new(1, 2))
                .item(PartlyVisibleImage(image_view)),
        );
        let resolved = resolve(&view, Available::size(1, 1)).unwrap();
        let mut output = AnsiWriter::new(Vec::new());

        render_kitty(&resolved, [&image], &mut output).unwrap();

        assert!(output.into_inner().is_empty());
    }

    #[test]
    fn omits_an_image_clipped_by_a_nested_canvas_after_its_anchor_is_shifted() {
        #[derive(Debug, Clone, PartialEq)]
        struct PartlyVisibleImage(View);

        impl CanvasItem for PartlyVisibleImage {
            fn draw(&self, context: &mut CanvasContext) {
                context.view(Position::new(-1, 0), self.0.clone(), Some(2), Some(2));
            }
        }

        let image = Image::rgba(
            "placement",
            "asset",
            PixelSize::new(1, 1),
            [255, 255, 255, 255],
        )
        .unwrap();
        let image_view = ImagePresentation::new().compose(&image, CellSize::new(2, 2));
        let clipped = View::canvas(
            Canvas::new()
                .extent(Size::new(1, 2))
                .item(PartlyVisibleImage(image_view)),
        );
        let view = View::row(
            urushi::VerticalAlign::Top,
            [View::text("x", TextStyle::new()), clipped],
        );
        let resolved = resolve(&view, Available::NONE).unwrap();
        let placement = image.placement(&resolved).unwrap();
        let mut output = AnsiWriter::new(Vec::new());

        assert_eq!(placement.origin(), Position::new(0, 0));
        assert_eq!(placement.size(), Size::new(2, 2));
        assert!(!placement.is_within_resolved_view());
        render_kitty(&resolved, [&image], &mut output).unwrap();

        assert!(output.into_inner().is_empty());
    }

    #[test]
    fn omits_an_empty_cell_placement() {
        let image = Image::rgba(
            "placement",
            "asset",
            PixelSize::new(1, 1),
            [255, 255, 255, 255],
        )
        .unwrap();
        let view = ImagePresentation::new().compose(&image, CellSize::ZERO);
        let resolved = resolve(&view, Available::NONE).unwrap();
        let mut output = AnsiWriter::new(Vec::new());

        render_kitty(&resolved, [&image], &mut output).unwrap();

        assert!(output.into_inner().is_empty());
    }

    #[test]
    fn chunks_payloads_without_splitting_base64_groups() {
        let rgba = vec![7; RAW_CHUNK_BYTES + 4];
        let image = Image::rgba("placement", "asset", PixelSize::new(769, 1), rgba).unwrap();
        let view = ImagePresentation::new().compose(&image, CellSize::new(1, 1));
        let resolved = resolve(&view, Available::NONE).unwrap();
        let mut output = AnsiWriter::new(Vec::new());

        render_kitty(&resolved, [&image], &mut output).unwrap();

        let output = String::from_utf8(output.into_inner()).unwrap();
        assert!(output.contains("m=1;"));
        assert!(output.contains("\x1b\\\x1b_Gm=0;"));
    }

    #[test]
    fn lifecycle_uploads_once_and_reconciles_move_hide_and_reappearance() {
        let image =
            Image::rgba("placement", "asset", PixelSize::new(1, 1), [255, 0, 0, 255]).unwrap();
        let mut lifecycle = KittyLifecycle::new();
        let mut first = RecordingTerminal::default();

        present_image(&mut lifecycle, &image, 0, &mut first).unwrap();

        let first = first.application_programs().collect::<Vec<_>>();
        assert_eq!(
            first
                .iter()
                .filter(|payload| payload.contains("a=t"))
                .count(),
            1
        );
        assert_eq!(
            first
                .iter()
                .filter(|payload| payload.contains("a=p"))
                .count(),
            1
        );

        let mut unchanged = RecordingTerminal::default();
        present_image(&mut lifecycle, &image, 0, &mut unchanged).unwrap();
        assert_eq!(unchanged.application_programs().count(), 0);
        assert_eq!(unchanged.flushes, 1);

        let mut moved = RecordingTerminal::default();
        present_image(&mut lifecycle, &image, 3, &mut moved).unwrap();
        let moved = moved.application_programs().collect::<Vec<_>>();
        assert_eq!(
            moved
                .iter()
                .filter(|payload| payload.contains("a=t"))
                .count(),
            0
        );
        assert_eq!(
            moved
                .iter()
                .filter(|payload| payload.contains("a=p,i=1,p=1"))
                .count(),
            1
        );

        let mut hidden = RecordingTerminal::default();
        present_empty(&mut lifecycle, &mut hidden).unwrap();
        assert!(
            hidden
                .application_programs()
                .any(|payload| { payload.contains("a=d,d=i,i=1,p=1") })
        );

        let mut shown = RecordingTerminal::default();
        present_image(&mut lifecycle, &image, 0, &mut shown).unwrap();
        let shown = shown.application_programs().collect::<Vec<_>>();
        assert_eq!(
            shown
                .iter()
                .filter(|payload| payload.contains("a=t"))
                .count(),
            0
        );
        assert_eq!(
            shown
                .iter()
                .filter(|payload| payload.contains("a=p,i=1,p=2"))
                .count(),
            1
        );
    }

    #[test]
    fn lifecycle_replaces_an_asset_and_deletes_the_superseded_upload() {
        let first = Image::rgba(
            "placement",
            "first-asset",
            PixelSize::new(1, 1),
            [255, 0, 0, 255],
        )
        .unwrap();
        let second = Image::rgba(
            "placement",
            "second-asset",
            PixelSize::new(1, 1),
            [0, 255, 0, 255],
        )
        .unwrap();
        let mut lifecycle = KittyLifecycle::new();
        present_image(&mut lifecycle, &first, 0, &mut RecordingTerminal::default()).unwrap();
        let mut replacement = RecordingTerminal::default();

        present_image(&mut lifecycle, &second, 0, &mut replacement).unwrap();

        let payloads = replacement.application_programs().collect::<Vec<_>>();
        assert!(
            payloads
                .iter()
                .any(|payload| payload.contains("a=t") && payload.contains("i=2"))
        );
        assert!(
            payloads
                .iter()
                .any(|payload| payload.contains("a=d,d=i,i=1,p=1"))
        );
        assert!(
            payloads
                .iter()
                .any(|payload| payload.contains("a=p,i=2,p=1"))
        );
        assert!(
            payloads
                .iter()
                .any(|payload| payload.contains("a=d,d=I,i=1"))
        );
    }

    #[test]
    fn lifecycle_shares_one_upload_between_independent_placements() {
        let first = Image::rgba(
            "first-placement",
            "shared-asset",
            PixelSize::new(1, 1),
            [255, 0, 0, 255],
        )
        .unwrap();
        let second = Image::rgba(
            "second-placement",
            "shared-asset",
            PixelSize::new(1, 1),
            [255, 0, 0, 255],
        )
        .unwrap();
        let presentation = ImagePresentation::new();
        let view = View::row(
            urushi::VerticalAlign::Top,
            [
                presentation.compose(&first, CellSize::new(1, 1)),
                presentation.compose(&second, CellSize::new(1, 1)),
            ],
        );
        let resolved = resolve(&view, Available::NONE).unwrap();
        let mut lifecycle = KittyLifecycle::new();
        let mut terminal = RecordingTerminal::default();

        lifecycle.present(&view, &resolved, &mut terminal).unwrap();

        let payloads = terminal.application_programs().collect::<Vec<_>>();
        assert_eq!(
            payloads
                .iter()
                .filter(|payload| payload.contains("a=t"))
                .count(),
            1
        );
        assert_eq!(
            payloads
                .iter()
                .filter(|payload| payload.contains("a=p,i=1"))
                .count(),
            2
        );
    }

    #[test]
    fn partial_failure_resets_possible_uploads_before_the_next_frame() {
        let first = Image::rgba(
            "placement",
            "first-asset",
            PixelSize::new(1, 1),
            [255, 0, 0, 255],
        )
        .unwrap();
        let second = Image::rgba(
            "placement",
            "second-asset",
            PixelSize::new(1, 1),
            [0, 255, 0, 255],
        )
        .unwrap();
        let mut lifecycle = KittyLifecycle::new();
        present_image(&mut lifecycle, &first, 0, &mut RecordingTerminal::default()).unwrap();

        present_image(
            &mut lifecycle,
            &second,
            0,
            &mut RecordingTerminal::fail_command(1),
        )
        .expect_err("the injected command failure should abort the candidate frame");

        let mut recovered = RecordingTerminal::default();
        present_image(&mut lifecycle, &second, 0, &mut recovered).unwrap();
        let payloads = recovered.application_programs().collect::<Vec<_>>();
        assert_eq!(&payloads[..2], ["Ga=d,d=I,i=1,q=2", "Ga=d,d=I,i=2,q=2"]);
        assert!(
            payloads
                .iter()
                .any(|payload| payload.contains("a=t") && payload.contains("i=3"))
        );
        assert!(
            payloads
                .iter()
                .any(|payload| payload.contains("a=p,i=3,p=2"))
        );
    }

    #[test]
    fn failed_cursor_restore_is_retried_before_reconciliation() {
        let image =
            Image::rgba("placement", "asset", PixelSize::new(1, 1), [255, 0, 0, 255]).unwrap();
        let mut lifecycle = KittyLifecycle::new();

        present_image(
            &mut lifecycle,
            &image,
            0,
            &mut RecordingTerminal::fail_command(4),
        )
        .expect_err("the injected cursor restore failure should abort the frame");

        let mut recovered = RecordingTerminal::default();
        present_image(&mut lifecycle, &image, 0, &mut recovered).unwrap();

        assert_eq!(
            recovered.commands.first().map(String::as_str),
            Some("restore")
        );
        assert!(
            recovered
                .commands
                .iter()
                .skip(1)
                .any(|command| command == "save")
        );
        assert_eq!(recovered.flushes, 2);
    }

    #[test]
    fn failed_flush_also_forces_a_complete_resynchronization() {
        let image =
            Image::rgba("placement", "asset", PixelSize::new(1, 1), [255, 0, 0, 255]).unwrap();
        let mut lifecycle = KittyLifecycle::new();
        let mut failed = RecordingTerminal {
            fail_flush: true,
            ..RecordingTerminal::default()
        };

        present_image(&mut lifecycle, &image, 0, &mut failed)
            .expect_err("the injected flush failure should abort the candidate frame");

        let mut recovered = RecordingTerminal::default();
        present_image(&mut lifecycle, &image, 0, &mut recovered).unwrap();
        let payloads = recovered.application_programs().collect::<Vec<_>>();
        assert_eq!(payloads[0], "Ga=d,d=I,i=1,q=2");
        assert!(
            payloads
                .iter()
                .any(|payload| payload.contains("a=t") && payload.contains("i=2"))
        );
    }

    #[test]
    fn clear_cleans_committed_or_possible_state_and_forces_a_new_upload() {
        let image =
            Image::rgba("placement", "asset", PixelSize::new(1, 1), [255, 0, 0, 255]).unwrap();
        let mut lifecycle = KittyLifecycle::new();
        present_image(&mut lifecycle, &image, 0, &mut RecordingTerminal::default()).unwrap();
        let mut cleanup = RecordingTerminal::default();

        lifecycle.clear(&mut cleanup).unwrap();

        assert_eq!(
            cleanup.application_programs().collect::<Vec<_>>(),
            ["Ga=d,d=I,i=1,q=2"]
        );
        let mut redrawn = RecordingTerminal::default();
        present_image(&mut lifecycle, &image, 0, &mut redrawn).unwrap();
        assert!(
            redrawn
                .application_programs()
                .any(|payload| { payload.contains("a=t") && payload.contains("i=2") })
        );
    }

    #[test]
    fn failed_clear_remains_pending_for_a_later_cleanup() {
        let image =
            Image::rgba("placement", "asset", PixelSize::new(1, 1), [255, 0, 0, 255]).unwrap();
        let mut lifecycle = KittyLifecycle::new();
        present_image(&mut lifecycle, &image, 0, &mut RecordingTerminal::default()).unwrap();

        lifecycle
            .clear(&mut RecordingTerminal::fail_command(0))
            .expect_err("the injected cleanup failure should remain pending");
        let mut retried = RecordingTerminal::default();
        lifecycle.clear(&mut retried).unwrap();

        assert_eq!(
            retried.application_programs().collect::<Vec<_>>(),
            ["Ga=d,d=I,i=1,q=2"]
        );
    }

    #[test]
    fn lifecycle_evicts_the_oldest_unused_upload_after_the_cache_limit() {
        let mut lifecycle = KittyLifecycle::new();
        let mut final_frame = RecordingTerminal::default();
        for index in 0..=MAX_UPLOADED_IMAGES {
            let image = Image::rgba(
                Key::of(&("placement", index)),
                Key::of(&("asset", index)),
                PixelSize::new(1, 1),
                [u8::try_from(index).unwrap(), 0, 0, 255],
            )
            .unwrap();
            let terminal = if index == MAX_UPLOADED_IMAGES {
                &mut final_frame
            } else {
                &mut RecordingTerminal::default()
            };
            present_image(&mut lifecycle, &image, 0, terminal).unwrap();
            present_empty(&mut lifecycle, &mut RecordingTerminal::default()).unwrap();
        }

        assert!(
            final_frame
                .application_programs()
                .any(|payload| { payload.contains("a=d,d=I,i=1") })
        );
    }

    #[test]
    fn lifecycle_deletes_a_superseded_shared_upload_after_its_last_reference() {
        let first = Image::rgba(
            "first-placement",
            "shared-asset",
            PixelSize::new(1, 1),
            [255, 0, 0, 255],
        )
        .unwrap();
        let second = Image::rgba(
            "second-placement",
            "shared-asset",
            PixelSize::new(1, 1),
            [255, 0, 0, 255],
        )
        .unwrap();
        let replacement = Image::rgba(
            "first-placement",
            "replacement-asset",
            PixelSize::new(1, 1),
            [0, 255, 0, 255],
        )
        .unwrap();
        let presentation = ImagePresentation::new();
        let initial = View::row(
            urushi::VerticalAlign::Top,
            [
                presentation.compose(&first, CellSize::new(1, 1)),
                presentation.compose(&second, CellSize::new(1, 1)),
            ],
        );
        let initial_resolved = resolve(&initial, Available::NONE).unwrap();
        let replaced = View::row(
            urushi::VerticalAlign::Top,
            [
                presentation.compose(&replacement, CellSize::new(1, 1)),
                presentation.compose(&second, CellSize::new(1, 1)),
            ],
        );
        let replaced_resolved = resolve(&replaced, Available::NONE).unwrap();
        let mut lifecycle = KittyLifecycle::new();

        lifecycle
            .present(
                &initial,
                &initial_resolved,
                &mut RecordingTerminal::default(),
            )
            .unwrap();
        let mut replacement_frame = RecordingTerminal::default();
        lifecycle
            .present(&replaced, &replaced_resolved, &mut replacement_frame)
            .unwrap();
        assert!(
            !replacement_frame
                .application_programs()
                .any(|payload| payload.contains("a=d,d=I,i=1"))
        );

        let mut hidden = RecordingTerminal::default();
        present_empty(&mut lifecycle, &mut hidden).unwrap();

        assert!(
            hidden
                .application_programs()
                .any(|payload| payload.contains("a=d,d=I,i=1"))
        );
    }

    #[test]
    fn lifecycle_does_not_cache_an_unused_upload_beyond_the_byte_budget() {
        let asset_key = Key::of(&"oversized-asset");
        let mut state = KittyState::new();
        state.uploaded.insert(
            asset_key,
            Uploaded {
                image_id: 1,
                last_used: 0,
                byte_len: MAX_UPLOADED_BYTES + 1,
                retire_when_unused: false,
            },
        );
        let frame = DesiredFrame {
            assets: HashMap::new(),
            placements: Vec::new(),
        };

        let operations = state.reconcile(&frame).unwrap();

        assert_eq!(operations, [KittyOperation::DeleteImage { image_id: 1 }]);
        assert!(state.uploaded.is_empty());
    }

    #[test]
    fn a_readopted_upload_becomes_cacheable_again() {
        let placement_key = Key::of(&"first-placement");
        let shared_placement_key = Key::of(&"second-placement");
        let first_asset_key = Key::of(&"first-asset");
        let second_asset_key = Key::of(&"second-asset");
        let placement = |key, asset_key, x| DesiredPlacement {
            key,
            asset_key,
            origin: Position::new(x, 0),
            size: Size::new(1, 1),
        };
        let mut state = KittyState::new();
        state.uploaded.insert(
            first_asset_key,
            Uploaded {
                image_id: 1,
                last_used: 0,
                byte_len: 4,
                retire_when_unused: false,
            },
        );
        state.uploaded.insert(
            second_asset_key,
            Uploaded {
                image_id: 2,
                last_used: 0,
                byte_len: 4,
                retire_when_unused: false,
            },
        );
        state.visible.insert(
            placement_key,
            VisiblePlacement {
                placement_id: 1,
                image_id: 1,
                placement: placement(placement_key, first_asset_key, 0),
            },
        );
        state.visible.insert(
            shared_placement_key,
            VisiblePlacement {
                placement_id: 2,
                image_id: 1,
                placement: placement(shared_placement_key, first_asset_key, 1),
            },
        );

        state
            .reconcile(&DesiredFrame {
                assets: HashMap::new(),
                placements: vec![
                    placement(placement_key, second_asset_key, 0),
                    placement(shared_placement_key, first_asset_key, 1),
                ],
            })
            .unwrap();
        assert!(state.uploaded[&first_asset_key].retire_when_unused);

        state
            .reconcile(&DesiredFrame {
                assets: HashMap::new(),
                placements: vec![
                    placement(placement_key, first_asset_key, 0),
                    placement(shared_placement_key, first_asset_key, 1),
                ],
            })
            .unwrap();
        assert!(!state.uploaded[&first_asset_key].retire_when_unused);

        state
            .reconcile(&DesiredFrame {
                assets: HashMap::new(),
                placements: Vec::new(),
            })
            .unwrap();
        let shown_again = state
            .reconcile(&DesiredFrame {
                assets: HashMap::new(),
                placements: vec![placement(placement_key, first_asset_key, 0)],
            })
            .unwrap();

        assert!(state.uploaded.contains_key(&first_asset_key));
        assert!(
            !shown_again
                .iter()
                .any(|operation| matches!(operation, KittyOperation::Upload { .. }))
        );
    }

    #[test]
    fn swapping_two_assets_keeps_both_uploads_cacheable() {
        let first_placement_key = Key::of(&"first-placement");
        let second_placement_key = Key::of(&"second-placement");
        let first_asset_key = Key::of(&"first-asset");
        let second_asset_key = Key::of(&"second-asset");
        let placement = |key, asset_key, x| DesiredPlacement {
            key,
            asset_key,
            origin: Position::new(x, 0),
            size: Size::new(1, 1),
        };
        let mut state = KittyState::new();
        for (asset_key, image_id) in [(first_asset_key, 1), (second_asset_key, 2)] {
            state.uploaded.insert(
                asset_key,
                Uploaded {
                    image_id,
                    last_used: 0,
                    byte_len: 4,
                    retire_when_unused: false,
                },
            );
        }
        for (key, asset_key, image_id, placement_id, x) in [
            (first_placement_key, first_asset_key, 1, 1, 0),
            (second_placement_key, second_asset_key, 2, 2, 1),
        ] {
            state.visible.insert(
                key,
                VisiblePlacement {
                    placement_id,
                    image_id,
                    placement: placement(key, asset_key, x),
                },
            );
        }

        state
            .reconcile(&DesiredFrame {
                assets: HashMap::new(),
                placements: vec![
                    placement(first_placement_key, second_asset_key, 0),
                    placement(second_placement_key, first_asset_key, 1),
                ],
            })
            .unwrap();
        state
            .reconcile(&DesiredFrame {
                assets: HashMap::new(),
                placements: Vec::new(),
            })
            .unwrap();

        assert!(state.uploaded.contains_key(&first_asset_key));
        assert!(state.uploaded.contains_key(&second_asset_key));
        assert!(!state.uploaded[&first_asset_key].retire_when_unused);
        assert!(!state.uploaded[&second_asset_key].retire_when_unused);
    }

    #[test]
    fn mandatory_retirement_is_counted_before_lru_eviction() {
        let cached_key = Key::of(&"cached");
        let retired_key = Key::of(&"retired");
        let visible_key = Key::of(&"visible");
        let placement_key = Key::of(&"placement");
        let visible_placement = DesiredPlacement {
            key: placement_key,
            asset_key: visible_key,
            origin: Position::new(0, 0),
            size: Size::new(1, 1),
        };
        let mebibytes = 1024 * 1024;
        let mut state = KittyState::new();
        for (key, image_id, byte_len, retire_when_unused) in [
            (cached_key, 1, 20 * mebibytes, false),
            (retired_key, 2, 20 * mebibytes, true),
            (visible_key, 3, 40 * mebibytes, false),
        ] {
            state.uploaded.insert(
                key,
                Uploaded {
                    image_id,
                    last_used: image_id.into(),
                    byte_len,
                    retire_when_unused,
                },
            );
        }
        state.visible.insert(
            placement_key,
            VisiblePlacement {
                placement_id: 1,
                image_id: 3,
                placement: visible_placement,
            },
        );

        let operations = state
            .reconcile(&DesiredFrame {
                assets: HashMap::new(),
                placements: vec![visible_placement],
            })
            .unwrap();

        assert_eq!(operations, [KittyOperation::DeleteImage { image_id: 2 }]);
        assert!(state.uploaded.contains_key(&cached_key));
        assert!(state.uploaded.contains_key(&visible_key));
        assert!(!state.uploaded.contains_key(&retired_key));
    }
}
