use std::io;

use icy_sixel::{EncodeOptions, QuantizeMethod};
use urushi::ResolvedView;
use urushi_terminal::{
    Command, CommandWriter, ControlString, CursorMove, PixelSize as CellPixelSize,
    Position as TerminalPosition, TerminalOutput,
};

use crate::{GraphicPlacement, Image};

const MAX_COLORS: u16 = 256;

/// Queues every fully visible image through the Sixel protocol.
///
/// Sixel has no cell-based scaling parameter, so the prepared RGBA raster is
/// resampled to the placement's pixel extent before encoding. The caller must
/// supply character-cell pixel dimensions obtained from terminal geometry.
pub fn render_sixel<'a>(
    view: &ResolvedView,
    images: impl IntoIterator<Item = &'a Image>,
    cell_pixels: CellPixelSize,
    terminal: &mut (impl CommandWriter + ?Sized),
) -> io::Result<()> {
    if cell_pixels.width() == 0 || cell_pixels.height() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Sixel rendering requires non-zero character-cell pixel dimensions",
        ));
    }
    for placement in images
        .into_iter()
        .filter_map(|image| image.placement(view))
        .filter(|placement| placement.is_within_resolved_view() && !placement.size().is_empty())
    {
        write_placement(placement, cell_pixels, terminal)?;
    }
    TerminalOutput::flush(terminal)
}

fn write_placement(
    placement: GraphicPlacement<'_>,
    cell_pixels: CellPixelSize,
    terminal: &mut (impl CommandWriter + ?Sized),
) -> io::Result<()> {
    let column = usize::try_from(placement.origin().x).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a Sixel image column cannot be negative",
        )
    })?;
    let row = usize::try_from(placement.origin().y).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a Sixel image row cannot be negative",
        )
    })?;
    let width = placement
        .size()
        .width()
        .checked_mul(cell_pixels.width())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Sixel width overflow"))?;
    let height = placement
        .size()
        .height()
        .checked_mul(cell_pixels.height())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Sixel height overflow"))?;
    let rgba = scale_rgba(placement, width, height)?;
    let options = EncodeOptions {
        max_colors: MAX_COLORS,
        diffusion: 0.0,
        quantize_method: QuantizeMethod::Wu,
    };
    #[allow(deprecated)]
    let encoded = icy_sixel::sixel_encode(&rgba, width, height, &options)
        .map_err(|error| io::Error::other(format!("could not encode Sixel: {error}")))?;
    let payload = encoded
        .strip_prefix("\x1bP")
        .and_then(|encoded| encoded.strip_suffix("\x1b\\"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid Sixel framing"))?;
    let payload = ControlString::try_from(payload)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    terminal.write_command(Command::SaveCursorPosition)?;
    terminal.write_command(Command::MoveCursor(CursorMove::To(TerminalPosition::new(
        column, row,
    ))))?;
    terminal.write_command(Command::DeviceControl(payload))?;
    terminal.write_command(Command::RestoreCursorPosition)
}

fn scale_rgba(
    placement: GraphicPlacement<'_>,
    target_width: usize,
    target_height: usize,
) -> io::Result<Vec<u8>> {
    let raster = placement.raster();
    let source_width = usize::try_from(raster.size().width())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "source width overflow"))?;
    let source_height = usize::try_from(raster.size().height())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "source height overflow"))?;
    let length = target_width
        .checked_mul(target_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "scaled raster overflow"))?;
    let mut scaled = vec![0; length];
    for target_y in 0..target_height {
        let source_y: usize = ((target_y as u128 * source_height as u128) / target_height as u128)
            .try_into()
            .expect("scaled source row remains within the source height");
        for target_x in 0..target_width {
            let source_x: usize = ((target_x as u128 * source_width as u128)
                / target_width as u128)
                .try_into()
                .expect("scaled source column remains within the source width");
            let source = (source_y * source_width + source_x) * 4;
            let target = (target_y * target_width + target_x) * 4;
            scaled[target..target + 4].copy_from_slice(&raster.bytes()[source..source + 4]);
        }
    }
    Ok(scaled)
}

#[cfg(test)]
mod tests {
    use urushi::{Available, resolve};
    use urushi_terminal::backend::ansi::AnsiWriter;

    use crate::{CellSize, ImagePresentation, PixelSize};

    use super::*;

    #[test]
    fn scales_and_writes_one_image_with_backend_owned_dcs_framing() {
        let image =
            Image::rgba("placement", "asset", PixelSize::new(1, 1), [255, 0, 0, 255]).unwrap();
        let view = ImagePresentation::new().compose(&image, CellSize::new(2, 1));
        let resolved = resolve(&view, Available::NONE).unwrap();
        let mut output = AnsiWriter::new(Vec::new());

        render_sixel(&resolved, [&image], CellPixelSize::new(2, 3), &mut output).unwrap();

        let output = String::from_utf8(output.into_inner()).unwrap();
        assert!(output.starts_with("\x1b7\x1b[1;1H\x1bP"));
        assert!(output.contains("\"1;1;4;3"));
        assert!(output.ends_with("\x1b\\\x1b8"));
    }

    #[test]
    fn rejects_missing_cell_pixel_geometry() {
        let image =
            Image::rgba("placement", "asset", PixelSize::new(1, 1), [255, 0, 0, 255]).unwrap();
        let resolved = resolve(
            &ImagePresentation::new().compose(&image, CellSize::new(1, 1)),
            Available::NONE,
        )
        .unwrap();
        let mut output = AnsiWriter::new(Vec::new());

        let error = render_sixel(&resolved, [&image], CellPixelSize::default(), &mut output)
            .expect_err("zero geometry is rejected");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(output.into_inner().is_empty());
    }
}
