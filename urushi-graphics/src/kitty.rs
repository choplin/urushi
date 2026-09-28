use std::fmt::Write as _;
use std::io;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use urushi::ResolvedView;
use urushi_terminal::{
    Command, CommandWriter, ControlString, CursorMove, Position as TerminalPosition, TerminalOutput,
};

use crate::{GraphicPlacement, Image};

const RAW_CHUNK_BYTES: usize = 3_072;

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

#[cfg(test)]
mod tests {
    use urushi::{
        Available, Canvas, CanvasContext, CanvasItem, Position, Size, TextStyle, View, resolve,
    };
    use urushi_terminal::backend::ansi::AnsiWriter;

    use crate::{CellSize, ImagePresentation, PixelSize};

    use super::*;

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
}
