use std::fmt;

use crate::{Available, Axis, Grapheme, Length, TextStyle, View};

use super::cell_primitives::CellPath;
use super::line_network::LineNetwork;
use super::{CellContribution, Composition, Position, PositionedCell};
use crate::view::geometry::Size;
use crate::view::grid;
use crate::view::resolve::{AnchoredRect, LayoutError, ResolvedView, resolve};

/// A command's complete output before it is composed into the Canvas surface.
///
/// Command implementations cannot inspect or mutate the surface. This output
/// is the only route from command-specific rasterization into Canvas
/// composition.
#[derive(Debug)]
pub(super) struct RasterizedCommand {
    pub(super) cells: Vec<PositionedCell>,
    pub(super) anchors: Vec<AnchoredRect>,
    pub(super) composition: Composition,
}

impl CommandOutput {
    fn cells(cells: Vec<PositionedCell>) -> CommandOutput {
        CommandOutput {
            cells,
            anchors: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(super) struct CommandOutput {
    cells: Vec<PositionedCell>,
    anchors: Vec<AnchoredRect>,
}

/// The common contract implemented by every Canvas command.
///
/// Rasterization depends only on the command value and the final Canvas size.
/// Applying the command to earlier cells belongs to the separate composition
/// phase and is deliberately unavailable here.
pub(super) trait CanvasCommand: fmt::Debug {
    fn rasterize(self: Box<Self>, size: Size) -> Result<CommandOutput, LayoutError>;
}

#[derive(Debug)]
pub(super) struct RecordedCommand {
    command: Box<dyn CanvasCommand>,
    composition: Composition,
}

impl RecordedCommand {
    pub(super) fn new(command: impl CanvasCommand + 'static, composition: Composition) -> Self {
        Self {
            command: Box::new(command),
            composition,
        }
    }

    pub(super) fn rasterize(self, size: Size) -> Result<RasterizedCommand, LayoutError> {
        let output = self.command.rasterize(size)?;
        Ok(RasterizedCommand {
            cells: output.cells,
            anchors: output.anchors,
            composition: self.composition,
        })
    }
}

#[derive(Debug)]
pub(super) struct ViewCommand {
    pub(super) origin: Position,
    pub(super) view: View,
    pub(super) allocation: (Option<usize>, Option<usize>),
}

impl CanvasCommand for ViewCommand {
    fn rasterize(self: Box<Self>, _: Size) -> Result<CommandOutput, LayoutError> {
        if self.allocation.0.is_none() && requires_allocation(&self.view, Axis::Width) {
            return Err(LayoutError::missing_allocation(Axis::Width));
        }
        if self.allocation.1.is_none() && requires_allocation(&self.view, Axis::Height) {
            return Err(LayoutError::missing_allocation(Axis::Height));
        }
        let resolved = resolve(
            &self.view,
            Available::new(self.allocation.0, self.allocation.1),
        )?;
        Ok(resolved_output(self.origin, &resolved))
    }
}

#[derive(Debug)]
pub(super) struct TextCommand {
    pub(super) origin: Position,
    pub(super) text: String,
    pub(super) style: TextStyle,
}

impl CanvasCommand for TextCommand {
    fn rasterize(self: Box<Self>, _: Size) -> Result<CommandOutput, LayoutError> {
        let resolved = resolve(&View::text(self.text, self.style), Available::NONE)?;
        Ok(resolved_output(self.origin, &resolved))
    }
}

#[derive(Debug)]
pub(super) struct CellsCommand(pub(super) Vec<PositionedCell>);

impl CanvasCommand for CellsCommand {
    fn rasterize(self: Box<Self>, _: Size) -> Result<CommandOutput, LayoutError> {
        Ok(CommandOutput::cells(self.0))
    }
}

impl CanvasCommand for CellPath {
    fn rasterize(self: Box<Self>, size: Size) -> Result<CommandOutput, LayoutError> {
        Ok(CommandOutput::cells(self.cells(size)))
    }
}

impl CanvasCommand for LineNetwork {
    fn rasterize(self: Box<Self>, size: Size) -> Result<CommandOutput, LayoutError> {
        let glyphs = self.glyphs();
        let style = self.style().clone();
        let cells = LineNetwork::rasterize(&self, size)
            .into_iter()
            .map(|cell| {
                let mut encoded = [0; 4];
                let symbol = glyphs.glyph(cell.connections).encode_utf8(&mut encoded);
                PositionedCell::new(
                    cell.position,
                    CellContribution::new()
                        .symbol(Grapheme::new(symbol))
                        .style(style.clone()),
                )
            })
            .collect();
        Ok(CommandOutput::cells(cells))
    }
}

fn resolved_output(origin: Position, resolved: &ResolvedView) -> CommandOutput {
    let mut cells = Vec::new();
    for (y, row) in resolved.rows().iter().enumerate() {
        let mut x = 0usize;
        for grapheme in row {
            cells.push(PositionedCell::new(
                Position::new(
                    origin.x.saturating_add(x as i64),
                    origin.y.saturating_add(y as i64),
                ),
                CellContribution::new()
                    .symbol(Grapheme::new(grapheme.symbol()))
                    .style(grapheme.style().clone()),
            ));
            x += grapheme.width();
        }
    }
    CommandOutput {
        cells,
        anchors: resolved
            .anchors()
            .iter()
            .map(|anchor| anchor.offset(origin.x, origin.y))
            .collect(),
    }
}

fn requires_allocation(view: &View, axis: Axis) -> bool {
    match view {
        View::Text(..) => false,
        View::Canvas(canvas) => match axis {
            Axis::Width => canvas.explicit_width().is_none(),
            Axis::Height => canvas.explicit_height().is_none(),
        },
        View::Block(style, _, child) | View::AnchorBlock(_, style, _, child) => {
            let (length, maximum) = match axis {
                Axis::Width => (style.get_width(), style.get_max_width()),
                Axis::Height => (style.get_height(), style.get_max_height()),
            };
            match length {
                Some(Length::Fill(_)) => true,
                Some(Length::Cells(_)) => false,
                None if maximum.is_some() => false,
                None => requires_allocation(child, axis),
            }
        }
        View::Row(_, children) | View::Column(_, children) => children
            .iter()
            .any(|child| requires_allocation(child, axis)),
        View::Grid(style, rows) => match axis {
            Axis::Height => rows
                .iter()
                .flatten()
                .any(|child| requires_allocation(child, axis)),
            Axis::Width => (0..grid::columns(rows)).any(|column| match style.get_column(column) {
                Some(Length::Fill(_)) => true,
                Some(Length::Cells(_)) => false,
                None => (0..rows.len())
                    .map(|row| grid::cell(rows, row, column))
                    .any(|child| requires_allocation(child, axis)),
            }),
        },
    }
}
