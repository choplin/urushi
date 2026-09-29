//! Stateless and cached immediate-mode Sixel presentation.

use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::Arc;

use icy_sixel::{EncodeOptions, QuantizeMethod};
use urushi::{Key, ResolvedView, View};
use urushi_terminal::{
    ClearRegion, Command, CommandWriter, ControlString, CursorMove, PixelSize as CellPixelSize,
    Position as TerminalPosition, TerminalOutput,
};

use crate::{GraphicPlacement, Image, RgbaRaster};

const MAX_COLORS: u16 = 256;
const MAX_ENCODED_BANDS: usize = 64;
const MAX_ENCODED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BandKey {
    asset_key: Key,
    logical_width: usize,
    logical_height: usize,
    target_x: usize,
    target_y: usize,
    width: usize,
    height: usize,
}

#[derive(Debug, Clone)]
struct EncodedBand {
    payload: Arc<str>,
    last_used: u64,
}

#[derive(Debug, Clone, Copy)]
struct DesiredBand<'a> {
    key: BandKey,
    raster: &'a RgbaRaster,
    column: usize,
    row: usize,
}

/// Renderer-owned cache for immediate-mode Sixel presentation.
///
/// Sixel placements are never treated as persistent terminal state. Every
/// successful call to [`Self::present`] emits the complete visible image scene,
/// while encoded cell-row bands are reused across frames. A host that changes
/// the scene first calls [`Self::clear`], redraws its cell frame, and then calls
/// `present`; the combined transaction belongs to that host.
#[derive(Debug, Default)]
pub struct SixelLifecycle {
    encoded: HashMap<BandKey, EncodedBand>,
    generation: u64,
    cursor_restore_pending: bool,
}

impl SixelLifecycle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Emits every visible image in one immutable desired scene.
    ///
    /// Encoding cache updates are safe to retain after output failure because
    /// they describe only immutable raster transformations. A later call still
    /// emits every visible band and therefore converges after the host redraws
    /// the cell frame.
    pub fn present(
        &mut self,
        view: &View,
        resolved: &ResolvedView,
        cell_pixels: CellPixelSize,
        terminal: &mut (impl CommandWriter + ?Sized),
    ) -> io::Result<()> {
        self.present_images(resolved, crate::image::collect(view), cell_pixels, terminal)
    }

    /// Clears terminal cells without discarding reusable encoded raster bands.
    ///
    /// The caller must redraw the complete cell frame before presenting the
    /// complete visible Sixel scene again.
    pub fn clear(&mut self, terminal: &mut (impl CommandWriter + ?Sized)) -> io::Result<()> {
        self.restore_cursor(terminal)?;
        terminal.write_command(Command::Clear(ClearRegion::Screen))?;
        terminal.write_command(Command::MoveCursor(CursorMove::To(TerminalPosition::new(
            0, 0,
        ))))?;
        TerminalOutput::flush(terminal)
    }

    fn present_images<'a>(
        &mut self,
        resolved: &ResolvedView,
        images: impl IntoIterator<Item = &'a Image>,
        cell_pixels: CellPixelSize,
        terminal: &mut (impl CommandWriter + ?Sized),
    ) -> io::Result<()> {
        validate_cell_pixels(cell_pixels)?;
        self.restore_cursor(terminal)?;
        self.generation = self.generation.saturating_add(1);
        let bands = desired_bands(resolved, images, cell_pixels)?;
        let live = bands.iter().map(|band| band.key).collect::<HashSet<_>>();

        let result = (|| {
            for band in bands {
                let payload = self.encoded_band(band)?;
                write_band(
                    band.column,
                    band.row,
                    &payload,
                    terminal,
                    &mut self.cursor_restore_pending,
                )?;
            }
            TerminalOutput::flush(terminal)
        })();
        if result.is_ok() {
            self.cursor_restore_pending = false;
        }
        self.enforce_cache_limits(&live, MAX_ENCODED_BANDS, MAX_ENCODED_BYTES);
        result
    }

    fn encoded_band(&mut self, band: DesiredBand<'_>) -> io::Result<Arc<str>> {
        if let Some(encoded) = self.encoded.get_mut(&band.key) {
            encoded.last_used = self.generation;
            return Ok(Arc::clone(&encoded.payload));
        }

        let rgba = scale_band(band)?;
        let options = EncodeOptions {
            max_colors: MAX_COLORS,
            diffusion: 0.0,
            quantize_method: QuantizeMethod::Wu,
        };
        #[allow(deprecated)]
        let encoded = icy_sixel::sixel_encode(&rgba, band.key.width, band.key.height, &options)
            .map_err(|error| io::Error::other(format!("could not encode Sixel: {error}")))?;
        let payload = encoded
            .strip_prefix("\x1bP")
            .and_then(|encoded| encoded.strip_suffix("\x1b\\"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid Sixel framing"))?;
        ControlString::try_from(payload)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let payload = Arc::<str>::from(payload);
        self.encoded.insert(
            band.key,
            EncodedBand {
                payload: Arc::clone(&payload),
                last_used: self.generation,
            },
        );
        Ok(payload)
    }

    fn enforce_cache_limits(
        &mut self,
        live: &HashSet<BandKey>,
        max_entries: usize,
        max_bytes: usize,
    ) {
        loop {
            let bytes = self.encoded.values().fold(0usize, |total, band| {
                total.saturating_add(band.payload.len())
            });
            if self.encoded.len() <= max_entries && bytes <= max_bytes {
                break;
            }
            let Some(key) = self
                .encoded
                .iter()
                .min_by_key(|(key, band)| {
                    (
                        live.contains(key),
                        band.last_used,
                        key.target_y,
                        key.target_x,
                    )
                })
                .map(|(key, _)| *key)
            else {
                break;
            };
            self.encoded.remove(&key);
        }
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
}

/// Queues every visible image through a stateless Sixel path.
pub fn render_sixel<'a>(
    view: &ResolvedView,
    images: impl IntoIterator<Item = &'a Image>,
    cell_pixels: CellPixelSize,
    terminal: &mut (impl CommandWriter + ?Sized),
) -> io::Result<()> {
    SixelLifecycle::new().present_images(view, images, cell_pixels, terminal)
}

fn validate_cell_pixels(cell_pixels: CellPixelSize) -> io::Result<()> {
    if cell_pixels.width() == 0 || cell_pixels.height() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Sixel rendering requires non-zero character-cell pixel dimensions",
        ));
    }
    Ok(())
}

fn desired_bands<'a>(
    view: &ResolvedView,
    images: impl IntoIterator<Item = &'a Image>,
    cell_pixels: CellPixelSize,
) -> io::Result<Vec<DesiredBand<'a>>> {
    let mut bands = Vec::new();
    for placement in images
        .into_iter()
        .filter_map(|image| image.placement(view, Some(cell_pixels)))
    {
        append_bands(placement, cell_pixels, &mut bands)?;
    }
    Ok(bands)
}

fn append_bands<'a>(
    placement: GraphicPlacement<'a>,
    cell_pixels: CellPixelSize,
    bands: &mut Vec<DesiredBand<'a>>,
) -> io::Result<()> {
    let Some(origin) = placement.visible_origin() else {
        return Ok(());
    };
    let Some(size) = placement.visible_size() else {
        return Ok(());
    };
    if size.is_empty() {
        return Ok(());
    }
    let column = usize::try_from(origin.x).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a Sixel image column cannot be negative",
        )
    })?;
    let row = usize::try_from(origin.y).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a Sixel image row cannot be negative",
        )
    })?;
    let logical = placement.logical_size();
    let logical_width = logical
        .width()
        .checked_mul(cell_pixels.width())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Sixel width overflow"))?;
    let logical_height = logical
        .height()
        .checked_mul(cell_pixels.height())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Sixel height overflow"))?;
    let offset_x = usize::try_from(
        origin
            .x
            .checked_sub(placement.logical_origin().x)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Sixel clip precedes its logical image",
                )
            })?,
    )
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Sixel clip offset overflow"))?;
    let offset_y = usize::try_from(
        origin
            .y
            .checked_sub(placement.logical_origin().y)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Sixel clip precedes its logical image",
                )
            })?,
    )
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Sixel clip offset overflow"))?;
    let target_x = offset_x
        .checked_mul(cell_pixels.width())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Sixel offset overflow"))?;
    let width = size
        .width()
        .checked_mul(cell_pixels.width())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Sixel width overflow"))?;

    for band in 0..size.height() {
        let target_y = offset_y
            .checked_add(band)
            .and_then(|row| row.checked_mul(cell_pixels.height()))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Sixel offset overflow"))?;
        bands.push(DesiredBand {
            key: BandKey {
                asset_key: placement.raster().key(),
                logical_width,
                logical_height,
                target_x,
                target_y,
                width,
                height: cell_pixels.height(),
            },
            raster: placement.raster(),
            column,
            row: row + band,
        });
    }
    Ok(())
}

fn scale_band(band: DesiredBand<'_>) -> io::Result<Vec<u8>> {
    let source_width = usize::try_from(band.raster.size().width())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "source width overflow"))?;
    let source_height = usize::try_from(band.raster.size().height())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "source height overflow"))?;
    let length = band
        .key
        .width
        .checked_mul(band.key.height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "scaled raster overflow"))?;
    let mut scaled = vec![0; length];
    for target_y in 0..band.key.height {
        let full_y =
            band.key.target_y.checked_add(target_y).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "scaled row overflow")
            })?;
        let raster_y = full_y as u128 * source_height as u128 / band.key.logical_height as u128;
        let raster_y = usize::try_from(raster_y).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "scaled source row overflow")
        })?;
        for target_x in 0..band.key.width {
            let full_x = band.key.target_x.checked_add(target_x).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "scaled column overflow")
            })?;
            let raster_x = full_x as u128 * source_width as u128 / band.key.logical_width as u128;
            let raster_x = usize::try_from(raster_x).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "scaled source column overflow")
            })?;
            let source = raster_y
                .checked_mul(source_width)
                .and_then(|offset| offset.checked_add(raster_x))
                .and_then(|offset| offset.checked_mul(4))
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "source raster offset overflow")
                })?;
            let target = (target_y * band.key.width + target_x) * 4;
            let pixel = band.raster.bytes().get(source..source + 4).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "source raster pixel is missing")
            })?;
            scaled[target..target + 4].copy_from_slice(pixel);
        }
    }
    Ok(scaled)
}

fn write_band(
    column: usize,
    row: usize,
    encoded: &str,
    terminal: &mut (impl CommandWriter + ?Sized),
    cursor_restore_pending: &mut bool,
) -> io::Result<()> {
    let payload = ControlString::try_from(encoded)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    terminal.write_command(Command::SaveCursorPosition)?;
    *cursor_restore_pending = true;
    let result = terminal
        .write_command(Command::MoveCursor(CursorMove::To(TerminalPosition::new(
            column, row,
        ))))
        .and_then(|()| terminal.write_command(Command::DeviceControl(payload)))
        .and_then(|()| terminal.write_command(Command::RestoreCursorPosition));
    if result.is_ok() {
        *cursor_restore_pending = false;
    }
    result
}

#[cfg(test)]
mod tests {
    use urushi::{Available, Canvas, CanvasContext, CanvasItem, Position, Size, View, resolve};
    use urushi_terminal::backend::ansi::AnsiWriter;

    use crate::{CellSize, ImagePresentation, PixelSize};

    use super::*;

    #[derive(Default)]
    struct RecordingTerminal {
        commands: Vec<String>,
        command_count: usize,
        fail_command: Option<usize>,
        flushes: usize,
    }

    impl CommandWriter for RecordingTerminal {
        fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
            let index = self.command_count;
            self.command_count += 1;
            if self.fail_command == Some(index) {
                return Err(io::Error::other("injected command failure"));
            }
            self.commands.push(match command {
                Command::SaveCursorPosition => "save".to_owned(),
                Command::RestoreCursorPosition => "restore".to_owned(),
                Command::MoveCursor(CursorMove::To(position)) => {
                    format!("move {},{}", position.column(), position.row())
                }
                Command::DeviceControl(payload) => format!("DCS {}", payload.as_str()),
                Command::Clear(ClearRegion::Screen) => "clear".to_owned(),
                other => format!("{other:?}"),
            });
            Ok(())
        }
    }

    impl TerminalOutput for RecordingTerminal {
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            Ok(())
        }
    }

    fn image() -> Image {
        Image::rgba(
            "placement",
            "asset",
            PixelSize::new(2, 12),
            vec![255; 2 * 12 * 4],
        )
        .unwrap()
    }

    #[derive(Debug, Clone, PartialEq)]
    struct PlacedImage {
        view: View,
        y: i64,
    }

    impl CanvasItem for PlacedImage {
        fn draw(&self, context: &mut CanvasContext) {
            context.view(
                Position::new(0, self.y),
                self.view.clone(),
                Some(1),
                Some(4),
            );
        }
    }

    fn viewport(image: &Image, origin: i64) -> (View, ResolvedView) {
        let image = ImagePresentation::new().compose(image, CellSize::new(1, 4));
        let view = View::canvas(Canvas::new().extent(Size::new(1, 3)).item(PlacedImage {
            view: image,
            y: -origin,
        }));
        let resolved = resolve(&view, Available::NONE).unwrap();
        (view, resolved)
    }

    #[test]
    fn scales_and_writes_one_image_as_cell_row_bands() {
        let image = image();
        let view = ImagePresentation::new().compose(&image, CellSize::new(1, 4));
        let resolved = resolve(&view, Available::NONE).unwrap();
        let mut output = AnsiWriter::new(Vec::new());

        render_sixel(&resolved, [&image], CellPixelSize::new(2, 3), &mut output).unwrap();

        let output = String::from_utf8(output.into_inner()).unwrap();
        assert_eq!(output.matches("\x1bP").count(), 4);
        assert!(output.starts_with("\x1b7\x1b[1;1H\x1bP"));
        assert!(output.contains("\"1;1;2;3"));
        assert!(output.ends_with("\x1b\\\x1b8"));
    }

    #[test]
    fn scrolling_reuses_overlapping_encoded_bands() {
        let image = image();
        let mut lifecycle = SixelLifecycle::new();
        let (_, first_resolved) = viewport(&image, 0);
        lifecycle
            .present_images(
                &first_resolved,
                [&image],
                CellPixelSize::new(2, 3),
                &mut AnsiWriter::new(Vec::new()),
            )
            .unwrap();
        assert_eq!(lifecycle.encoded.len(), 3);

        let (_, second_resolved) = viewport(&image, 1);
        lifecycle
            .present_images(
                &second_resolved,
                [&image],
                CellPixelSize::new(2, 3),
                &mut AnsiWriter::new(Vec::new()),
            )
            .unwrap();

        assert_eq!(lifecycle.encoded.len(), 4);
        assert_eq!(
            lifecycle
                .encoded
                .keys()
                .filter(|key| matches!(key.target_y, 3 | 6))
                .count(),
            2
        );
    }

    #[test]
    fn horizontal_clip_selects_the_matching_source_pixels() {
        #[derive(Debug, Clone, PartialEq)]
        struct ShiftedImage(View);

        impl CanvasItem for ShiftedImage {
            fn draw(&self, context: &mut CanvasContext) {
                context.view(Position::new(-1, 0), self.0.clone(), Some(4), Some(1));
            }
        }

        let image = Image::rgba(
            "placement",
            "asset",
            PixelSize::new(4, 1),
            [
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ],
        )
        .unwrap();
        let image_view = ImagePresentation::new().compose(&image, CellSize::new(4, 1));
        let view = View::canvas(
            Canvas::new()
                .extent(Size::new(2, 1))
                .item(ShiftedImage(image_view)),
        );
        let resolved = resolve(&view, Available::NONE).unwrap();

        let bands = desired_bands(&resolved, [&image], CellPixelSize::new(1, 1)).unwrap();
        assert_eq!(bands.len(), 1);
        assert_eq!(bands[0].key.target_x, 1);
        assert_eq!(bands[0].key.width, 2);
        assert_eq!(
            scale_band(bands[0]).unwrap(),
            [0, 255, 0, 255, 0, 0, 255, 255]
        );
    }

    #[test]
    fn clear_preserves_encoded_cache_for_the_full_redraw() {
        let image = image();
        let (_, resolved) = viewport(&image, 0);
        let mut lifecycle = SixelLifecycle::new();
        lifecycle
            .present_images(
                &resolved,
                [&image],
                CellPixelSize::new(2, 3),
                &mut AnsiWriter::new(Vec::new()),
            )
            .unwrap();
        let cached = lifecycle.encoded.len();
        let mut output = AnsiWriter::new(Vec::new());

        lifecycle.clear(&mut output).unwrap();

        assert_eq!(output.into_inner(), b"\x1b[2J\x1b[1;1H");
        assert_eq!(lifecycle.encoded.len(), cached);
    }

    #[test]
    fn failed_output_restores_the_cursor_then_repaints_the_complete_scene() {
        let image = image();
        let (_, resolved) = viewport(&image, 0);
        let mut lifecycle = SixelLifecycle::new();
        let mut failed = RecordingTerminal {
            fail_command: Some(2),
            ..RecordingTerminal::default()
        };

        lifecycle
            .present_images(&resolved, [&image], CellPixelSize::new(2, 3), &mut failed)
            .expect_err("the injected DCS failure should abort the frame");
        let mut recovered = RecordingTerminal::default();
        lifecycle
            .present_images(
                &resolved,
                [&image],
                CellPixelSize::new(2, 3),
                &mut recovered,
            )
            .unwrap();

        assert_eq!(
            recovered.commands.first().map(String::as_str),
            Some("restore")
        );
        assert_eq!(
            recovered
                .commands
                .iter()
                .filter(|command| command.starts_with("DCS "))
                .count(),
            3
        );
        assert_eq!(recovered.flushes, 2);
    }

    #[test]
    fn encoded_cache_evicts_the_oldest_unused_bands() {
        let image = Image::rgba(
            "placement",
            "asset",
            PixelSize::new(1, 80),
            vec![255; 80 * 4],
        )
        .unwrap();
        let mut lifecycle = SixelLifecycle::new();

        for target_y in 0..70 {
            lifecycle.generation = target_y + 1;
            let key = BandKey {
                asset_key: image.raster().key(),
                logical_width: 1,
                logical_height: 80,
                target_x: 0,
                target_y: usize::try_from(target_y).unwrap(),
                width: 1,
                height: 1,
            };
            lifecycle
                .encoded_band(DesiredBand {
                    key,
                    raster: image.raster(),
                    column: 0,
                    row: 0,
                })
                .unwrap();
            lifecycle.enforce_cache_limits(
                &HashSet::from([key]),
                MAX_ENCODED_BANDS,
                MAX_ENCODED_BYTES,
            );
        }

        assert_eq!(lifecycle.encoded.len(), MAX_ENCODED_BANDS);
        assert!(lifecycle.encoded.keys().all(|key| key.target_y >= 6));
    }

    #[test]
    fn encoded_cache_limits_apply_when_every_entry_is_in_the_visible_scene() {
        let mut lifecycle = SixelLifecycle::new();
        let mut live = HashSet::new();
        for target_y in 0..3 {
            let key = BandKey {
                asset_key: Key::from("asset"),
                logical_width: 1,
                logical_height: 3,
                target_x: 0,
                target_y,
                width: 1,
                height: 1,
            };
            live.insert(key);
            lifecycle.encoded.insert(
                key,
                EncodedBand {
                    payload: Arc::from("1234"),
                    last_used: u64::try_from(target_y).unwrap(),
                },
            );
        }

        lifecycle.enforce_cache_limits(&live, 2, 6);

        assert_eq!(lifecycle.encoded.len(), 1);
        assert_eq!(
            lifecycle
                .encoded
                .values()
                .map(|band| band.payload.len())
                .sum::<usize>(),
            4
        );
        assert!(lifecycle.encoded.keys().all(|key| key.target_y == 2));
    }

    #[test]
    fn rejects_missing_cell_pixel_geometry() {
        let image = image();
        let view = ImagePresentation::new().compose(&image, CellSize::new(1, 1));
        let resolved = resolve(&view, Available::NONE).unwrap();
        let mut output = AnsiWriter::new(Vec::new());

        let error = render_sixel(&resolved, [&image], CellPixelSize::default(), &mut output)
            .expect_err("zero geometry is rejected");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(output.into_inner().is_empty());
    }
}
