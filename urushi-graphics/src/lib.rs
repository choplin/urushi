//! Image components and terminal graphics adapters for Urushi.
//!
//! Core `urushi` reserves and resolves image regions through its generic anchor
//! contract. This crate owns the image data, presentation, placement lookup,
//! and terminal-protocol adapters layered on those resolved regions. One-shot
//! rendering stays stateless; an interactive renderer may retain
//! [`kitty::KittyLifecycle`] for Kitty upload and placement reuse or
//! [`sixel::SixelLifecycle`] for encoded Sixel band reuse.

mod image;
pub mod kitty;
pub mod sixel;

use std::fmt;
use std::io;

use urushi::{
    Available, RenderSettings, ResolvedView, TerminalTextStyle, TextStyle, View, resolve,
};
use urushi_terminal::{
    Command, CommandWriter, CursorMove, HyperlinkParameter, PixelSize as CellPixelSize,
    Position as TerminalPosition, TerminalCapabilities, TerminalGraphicsProtocol,
    TerminalHyperlink, TerminalOutput, TerminalQuery, TerminalText,
};

pub use image::{
    CellSize, GraphicPlacement, Image, ImagePresentation, InvalidRgbaRaster, PixelPosition,
    PixelSize, RgbaRaster,
};

/// Caller-selected policy for terminal image output.
///
/// Automatic selection prefers Kitty, then Sixel when character-cell pixel
/// geometry is available, and finally the Image component's text fallback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GraphicsPreference {
    /// Choose the strongest positively confirmed usable protocol.
    #[default]
    Auto,
    /// Require Kitty graphics.
    Kitty,
    /// Require Sixel graphics and character-cell pixel geometry.
    Sixel,
    /// Always use the Image component's text fallback.
    Text,
}

/// The graphics path selected for one terminal presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GraphicsSelection {
    Kitty,
    Sixel,
    Text,
}

/// Why an explicitly requested graphics path cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphicsUnavailable {
    UnsupportedProtocol(TerminalGraphicsProtocol),
    MissingCellPixelGeometry,
}

impl fmt::Display for GraphicsUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProtocol(TerminalGraphicsProtocol::Kitty) => {
                formatter.write_str("Kitty graphics were requested, but the terminal did not confirm support")
            }
            Self::UnsupportedProtocol(TerminalGraphicsProtocol::Sixel) => {
                formatter.write_str("Sixel graphics were requested, but the terminal did not confirm support")
            }
            Self::MissingCellPixelGeometry => formatter.write_str(
                "Sixel graphics were requested, but the terminal did not report uniform character-cell pixel geometry",
            ),
        }
    }
}

impl std::error::Error for GraphicsUnavailable {}

/// Selects the graphics path from caller policy and positive terminal evidence.
pub fn select_graphics(
    preference: GraphicsPreference,
    capabilities: TerminalCapabilities,
    cell_pixels: Option<CellPixelSize>,
) -> Result<GraphicsSelection, GraphicsUnavailable> {
    let kitty = capabilities.supports_graphics(TerminalGraphicsProtocol::Kitty);
    let sixel = capabilities.supports_graphics(TerminalGraphicsProtocol::Sixel);
    match preference {
        GraphicsPreference::Auto if kitty => Ok(GraphicsSelection::Kitty),
        GraphicsPreference::Auto if sixel && cell_pixels.is_some() => Ok(GraphicsSelection::Sixel),
        GraphicsPreference::Auto | GraphicsPreference::Text => Ok(GraphicsSelection::Text),
        GraphicsPreference::Kitty if kitty => Ok(GraphicsSelection::Kitty),
        GraphicsPreference::Kitty => Err(GraphicsUnavailable::UnsupportedProtocol(
            TerminalGraphicsProtocol::Kitty,
        )),
        GraphicsPreference::Sixel if !sixel => Err(GraphicsUnavailable::UnsupportedProtocol(
            TerminalGraphicsProtocol::Sixel,
        )),
        GraphicsPreference::Sixel if cell_pixels.is_none() => {
            Err(GraphicsUnavailable::MissingCellPixelGeometry)
        }
        GraphicsPreference::Sixel => Ok(GraphicsSelection::Sixel),
    }
}

/// Resolves and renders one complete View at the terminal's top-left cell.
///
/// Image components embedded in the View are discovered internally. The
/// terminal is queried for current geometry and positively confirmed
/// capabilities; output prefers Kitty, then Sixel when cell-pixel geometry is
/// available, and otherwise leaves the View's text fallbacks visible.
///
/// This operation retains no graphics state after it returns. Interactive
/// renderers that reconcile multiple frames own their caches and lifecycle state.
pub fn render_view(
    terminal: &mut (impl CommandWriter + TerminalQuery + ?Sized),
    view: &View,
) -> io::Result<Option<TerminalGraphicsProtocol>> {
    let window = terminal.window_size()?;
    let capabilities = terminal.terminal_capabilities()?;
    let resolved = resolve(
        view,
        Available::size(window.cells().columns(), window.cells().rows()),
    )
    .map_err(io::Error::other)?;
    write_cells(terminal, &resolved, &RenderSettings::from(capabilities))?;
    let images = image::collect(view);
    if images.is_empty() {
        TerminalOutput::flush(terminal)?;
        return Ok(None);
    }
    let protocol = render_resolved_images(
        &resolved,
        images,
        capabilities,
        window.cell_pixels(),
        terminal,
    )?;
    if protocol.is_none() {
        TerminalOutput::flush(terminal)?;
    }
    Ok(protocol)
}

/// Renders caller-supplied images over an already resolved View.
///
/// This is the low-level path for a caller that owns resolution, capability
/// observation, and cell output. Ordinary one-shot callers use [`render_view`].
pub fn render_resolved_images<'a>(
    view: &ResolvedView,
    images: impl IntoIterator<Item = &'a Image>,
    capabilities: TerminalCapabilities,
    cell_pixels: Option<CellPixelSize>,
    terminal: &mut (impl CommandWriter + ?Sized),
) -> io::Result<Option<TerminalGraphicsProtocol>> {
    match select_graphics(GraphicsPreference::Auto, capabilities, cell_pixels)
        .expect("automatic graphics selection is always available")
    {
        GraphicsSelection::Kitty => {
            kitty::render_kitty(view, images, cell_pixels, terminal)?;
            Ok(Some(TerminalGraphicsProtocol::Kitty))
        }
        GraphicsSelection::Sixel => {
            sixel::render_sixel(
                view,
                images,
                cell_pixels.expect("Sixel selection requires cell-pixel geometry"),
                terminal,
            )?;
            Ok(Some(TerminalGraphicsProtocol::Sixel))
        }
        GraphicsSelection::Text => Ok(None),
    }
}

fn write_cells(
    terminal: &mut (impl CommandWriter + ?Sized),
    view: &ResolvedView,
    settings: &RenderSettings,
) -> io::Result<()> {
    for (row, graphemes) in view.rows().iter().enumerate() {
        terminal.write_command(Command::MoveCursor(CursorMove::To(TerminalPosition::new(
            0, row,
        ))))?;
        let mut index = 0;
        while index < graphemes.len() {
            let style = settings.resolve_text_style(graphemes[index].style());
            let mut text = String::new();
            while index < graphemes.len()
                && settings.resolve_text_style(graphemes[index].style()) == style
            {
                text.push_str(graphemes[index].symbol());
                index += 1;
            }
            write_run(terminal, &text, &style)?;
        }
    }
    Ok(())
}

fn write_run(
    terminal: &mut (impl CommandWriter + ?Sized),
    text: &str,
    style: &TextStyle,
) -> io::Result<()> {
    let text = TerminalText::try_from(text)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let style = TerminalTextStyle::from(style);
    terminal.write_command(Command::SetStyle(style.style()))?;
    if let Some(link) = style.hyperlink() {
        let parameters = link
            .parameters()
            .iter()
            .map(|(key, value)| HyperlinkParameter { key, value })
            .collect::<Vec<_>>();
        terminal.write_command(Command::SetHyperlink(Some(TerminalHyperlink {
            uri: link.uri(),
            parameters: &parameters,
        })))?;
        terminal.write_command(Command::Print(text))?;
        terminal.write_command(Command::SetHyperlink(None))?;
    } else {
        terminal.write_command(Command::Print(text))?;
    }
    terminal.write_command(Command::ResetStyle)
}

#[cfg(test)]
mod tests {
    use urushi::{Available, resolve};
    use urushi_terminal::{
        Position, TerminalGraphicsProtocols, TerminalSize, WindowSize, backend::ansi::AnsiWriter,
    };

    use super::*;

    fn fixture() -> (Image, ResolvedView) {
        let image =
            Image::rgba("placement", "asset", PixelSize::new(1, 1), [255, 0, 0, 255]).unwrap();
        let view = ImagePresentation::new().compose(&image, CellSize::new(1, 1));
        let resolved = resolve(&view, Available::NONE).unwrap();
        (image, resolved)
    }

    #[test]
    fn automatic_rendering_prefers_kitty_then_sixel_then_fallback() {
        let (image, resolved) = fixture();
        let both = TerminalCapabilities::none().with_graphics_protocols(
            TerminalGraphicsProtocols::KITTY | TerminalGraphicsProtocols::SIXEL,
        );
        let mut kitty_output = AnsiWriter::new(Vec::new());
        assert_eq!(
            render_resolved_images(
                &resolved,
                [&image],
                both,
                Some(CellPixelSize::new(8, 16)),
                &mut kitty_output,
            )
            .unwrap(),
            Some(TerminalGraphicsProtocol::Kitty)
        );
        assert!(
            kitty_output
                .into_inner()
                .windows(3)
                .any(|bytes| bytes == b"\x1b_G")
        );

        let sixel =
            TerminalCapabilities::none().with_graphics_protocols(TerminalGraphicsProtocols::SIXEL);
        let mut sixel_output = AnsiWriter::new(Vec::new());
        assert_eq!(
            render_resolved_images(
                &resolved,
                [&image],
                sixel,
                Some(CellPixelSize::new(2, 3)),
                &mut sixel_output,
            )
            .unwrap(),
            Some(TerminalGraphicsProtocol::Sixel)
        );
        assert!(
            sixel_output
                .into_inner()
                .windows(2)
                .any(|bytes| bytes == b"\x1bP")
        );

        let mut fallback_output = AnsiWriter::new(Vec::new());
        assert_eq!(
            render_resolved_images(
                &resolved,
                [&image],
                TerminalCapabilities::none(),
                None,
                &mut fallback_output,
            )
            .unwrap(),
            None
        );
        assert!(fallback_output.into_inner().is_empty());
    }

    #[test]
    fn explicit_selection_reports_missing_protocol_or_geometry() {
        let both = TerminalCapabilities::none().with_graphics_protocols(
            TerminalGraphicsProtocols::KITTY | TerminalGraphicsProtocols::SIXEL,
        );
        assert_eq!(
            select_graphics(
                GraphicsPreference::Sixel,
                both,
                Some(CellPixelSize::new(8, 16)),
            ),
            Ok(GraphicsSelection::Sixel)
        );
        assert_eq!(
            select_graphics(GraphicsPreference::Sixel, both, None),
            Err(GraphicsUnavailable::MissingCellPixelGeometry)
        );
        assert_eq!(
            select_graphics(
                GraphicsPreference::Kitty,
                TerminalCapabilities::none(),
                None,
            ),
            Err(GraphicsUnavailable::UnsupportedProtocol(
                TerminalGraphicsProtocol::Kitty
            ))
        );
    }

    struct QueryingTerminal {
        writer: AnsiWriter<Vec<u8>>,
        window: WindowSize,
        capabilities: TerminalCapabilities,
    }

    impl QueryingTerminal {
        fn new(window: WindowSize, capabilities: TerminalCapabilities) -> Self {
            Self {
                writer: AnsiWriter::new(Vec::new()),
                window,
                capabilities,
            }
        }
    }

    impl TerminalOutput for QueryingTerminal {
        fn flush(&mut self) -> io::Result<()> {
            self.writer.flush()
        }
    }

    impl CommandWriter for QueryingTerminal {
        fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
            self.writer.write_command(command)
        }
    }

    impl TerminalQuery for QueryingTerminal {
        fn terminal_size(&mut self) -> io::Result<TerminalSize> {
            Ok(self.window.cells())
        }

        fn cursor_position(&mut self) -> io::Result<Position> {
            Ok(Position::new(0, 0))
        }

        fn window_size(&mut self) -> io::Result<WindowSize> {
            Ok(self.window)
        }

        fn raw_mode_enabled(&mut self) -> io::Result<bool> {
            Ok(false)
        }

        fn terminal_capabilities(&mut self) -> io::Result<TerminalCapabilities> {
            Ok(self.capabilities)
        }
    }

    fn owned_image_view() -> View {
        let image = Image::rgba("placement", "asset", PixelSize::new(1, 1), [255, 0, 0, 255])
            .unwrap()
            .fallback("fallback");
        ImagePresentation::new().compose(&image, CellSize::new(8, 1))
    }

    #[test]
    fn high_level_rendering_owns_the_image_and_prefers_kitty() {
        let view = owned_image_view();
        let capabilities = TerminalCapabilities::none().with_graphics_protocols(
            TerminalGraphicsProtocols::KITTY | TerminalGraphicsProtocols::SIXEL,
        );
        let window = WindowSize::new(TerminalSize::new(8, 1), Some(CellPixelSize::new(64, 16)));
        let mut terminal = QueryingTerminal::new(window, capabilities);

        assert_eq!(
            render_view(&mut terminal, &view).unwrap(),
            Some(TerminalGraphicsProtocol::Kitty)
        );

        let output = terminal.writer.into_inner();
        assert!(
            output
                .windows(b"fallback".len())
                .any(|bytes| bytes == b"fallback")
        );
        assert!(output.windows(3).any(|bytes| bytes == b"\x1b_G"));
        assert!(!output.windows(2).any(|bytes| bytes == b"\x1bP"));
    }
}
