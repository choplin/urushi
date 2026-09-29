use std::io;

use urushi::{Align, Available, Size, TextStyle, View, resolve};
use urushi_graphics::kitty::{KittyLifecycle, render_kitty};
use urushi_graphics::{CellSize, Image, ImagePresentation, InvalidRgbaRaster, PixelSize};
use urushi_terminal::{Command, CommandWriter, CursorMove, Position, TerminalOutput};

#[derive(Debug, PartialEq, Eq)]
enum RecordedCommand {
    SaveCursor,
    Move(Position),
    ApplicationProgram(String),
    RestoreCursor,
}

#[derive(Default)]
struct RecordingTerminal {
    commands: Vec<RecordedCommand>,
    flushes: usize,
}

impl TerminalOutput for RecordingTerminal {
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}

impl CommandWriter for RecordingTerminal {
    fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
        self.commands.push(match command {
            Command::SaveCursorPosition => RecordedCommand::SaveCursor,
            Command::MoveCursor(CursorMove::To(position)) => RecordedCommand::Move(position),
            Command::ApplicationProgram(payload) => {
                RecordedCommand::ApplicationProgram(payload.as_str().to_owned())
            }
            Command::RestoreCursorPosition => RecordedCommand::RestoreCursor,
            _ => panic!("unexpected Kitty terminal command: {command:?}"),
        });
        Ok(())
    }
}

fn symbols(view: &urushi::ResolvedView) -> Vec<String> {
    view.rows()
        .iter()
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect()
}

#[test]
fn image_composes_fallback_cells_and_placement_from_one_resolution() {
    let image = Image::rgba(
        "cover-placement",
        "cover-asset",
        PixelSize::new(2, 1),
        [255, 0, 0, 255, 0, 255, 0, 255],
    )
    .unwrap()
    .fallback("cover");
    let image_view = ImagePresentation::new()
        .fallback_style(TextStyle::new().dim())
        .compose(&image, CellSize::new(5, 2));
    let view = View::column(
        Align::Left,
        [View::text("title", TextStyle::new()), image_view],
    );

    let resolved = resolve(&view, Available::NONE).unwrap();
    let placement = image.placement(&resolved).unwrap();

    assert_eq!(resolved.size(), Size::new(5, 3));
    assert_eq!(symbols(&resolved), ["title", "cover", "     "]);
    assert_eq!(placement.origin(), urushi::Position::new(0, 1));
    assert_eq!(placement.size(), Size::new(5, 2));
    assert_eq!(placement.raster().key(), urushi::Key::from("cover-asset"));
    assert!(placement.is_within_resolved_view());
}

#[test]
fn placement_and_asset_identities_are_independent() {
    let first = Image::rgba(
        "first-placement",
        "shared-asset",
        PixelSize::new(1, 1),
        [0, 0, 0, 0],
    )
    .unwrap();
    let second = Image::rgba(
        "second-placement",
        "shared-asset",
        PixelSize::new(1, 1),
        [0, 0, 0, 0],
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
    let first = first.placement(&resolved).unwrap();
    let second = second.placement(&resolved).unwrap();

    assert_ne!(first.key(), second.key());
    assert_eq!(first.raster().key(), second.raster().key());
}

#[test]
fn prepared_rgba_rejects_empty_or_mismatched_pixels() {
    assert_eq!(
        Image::rgba("placement", "asset", PixelSize::new(0, 1), Vec::<u8>::new()).unwrap_err(),
        InvalidRgbaRaster::Empty
    );
    assert_eq!(
        Image::rgba("placement", "asset", PixelSize::new(2, 1), [0, 0, 0, 0]).unwrap_err(),
        InvalidRgbaRaster::ByteLength {
            expected: 8,
            actual: 4,
        }
    );
}

#[test]
fn fallback_content_never_expands_the_requested_cell_rectangle() {
    let zero_width = Image::rgba(
        "zero-placement",
        "zero-asset",
        PixelSize::new(1, 1),
        [0, 0, 0, 0],
    )
    .unwrap();
    let wide_fallback = Image::rgba(
        "wide-placement",
        "wide-asset",
        PixelSize::new(1, 1),
        [0, 0, 0, 0],
    )
    .unwrap()
    .fallback("界");
    let presentation = ImagePresentation::new();

    let zero = presentation.compose(&zero_width, CellSize::new(0, 1));
    let zero = resolve(&zero, Available::NONE).unwrap();
    assert_eq!(zero.size(), Size::new(0, 1));
    assert_eq!(zero_width.placement(&zero).unwrap().size(), Size::new(0, 1));

    let narrow = presentation.compose(&wide_fallback, CellSize::new(1, 1));
    let narrow = resolve(&narrow, Available::NONE).unwrap();
    assert_eq!(narrow.size(), Size::new(1, 1));
    assert_eq!(
        wide_fallback.placement(&narrow).unwrap().size(),
        Size::new(1, 1)
    );
}

#[test]
fn kitty_uses_the_public_terminal_command_contract() {
    let image = Image::rgba("placement", "asset", PixelSize::new(1, 1), [255, 0, 0, 255]).unwrap();
    let view = ImagePresentation::new().compose(&image, CellSize::new(2, 1));
    let resolved = resolve(&view, Available::NONE).unwrap();
    let mut terminal = RecordingTerminal::default();

    fn render_through_trait_object(
        terminal: &mut dyn CommandWriter,
        resolved: &urushi::ResolvedView,
        image: &Image,
    ) -> io::Result<()> {
        render_kitty(resolved, [image], terminal)
    }

    render_through_trait_object(&mut terminal, &resolved, &image).unwrap();

    assert_eq!(
        terminal.commands,
        [
            RecordedCommand::SaveCursor,
            RecordedCommand::Move(Position::new(0, 0)),
            RecordedCommand::ApplicationProgram(
                "Ga=T,f=32,s=1,v=1,c=2,r=1,C=1,q=2,m=0;/wAA/w==".to_owned(),
            ),
            RecordedCommand::RestoreCursor,
        ]
    );
    assert_eq!(terminal.flushes, 1);
}

#[test]
fn zero_width_image_keeps_its_fallback_and_emits_no_kitty_command() {
    let image = Image::rgba("placement", "asset", PixelSize::new(1, 1), [255, 0, 0, 255]).unwrap();
    let view = ImagePresentation::new().compose(&image, CellSize::new(0, 1));
    let resolved = resolve(&view, Available::NONE).unwrap();
    let mut terminal = RecordingTerminal::default();

    render_kitty(&resolved, [&image], &mut terminal).unwrap();

    assert_eq!(resolved.size(), Size::new(0, 1));
    assert!(terminal.commands.is_empty());
    assert_eq!(terminal.flushes, 1);
}

#[test]
fn renderer_can_retain_and_cleanup_kitty_lifecycle_state() {
    let image = Image::rgba("placement", "asset", PixelSize::new(1, 1), [255, 0, 0, 255]).unwrap();
    let view = ImagePresentation::new().compose(&image, CellSize::new(2, 1));
    let resolved = resolve(&view, Available::NONE).unwrap();
    let mut lifecycle = KittyLifecycle::new();
    let mut terminal = RecordingTerminal::default();

    lifecycle.present(&view, &resolved, &mut terminal).unwrap();
    terminal.commands.clear();
    lifecycle.present(&view, &resolved, &mut terminal).unwrap();

    assert!(
        terminal.commands.is_empty(),
        "an unchanged frame emits no Kitty commands"
    );
    lifecycle.clear(&mut terminal).unwrap();
    assert!(matches!(
        terminal.commands.as_slice(),
        [RecordedCommand::ApplicationProgram(payload)] if payload == "Ga=d,d=I,i=1,q=2"
    ));
    assert_eq!(terminal.flushes, 3);
}
