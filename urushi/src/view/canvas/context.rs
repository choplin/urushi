use crate::{Grapheme, TextStyle, View};

use super::cell_primitives::CellPath;
use super::command::{CellsCommand, RecordedCommand, TextCommand, ViewCommand};
use super::line_network::LineNetwork;
use super::{Composition, Position, PositionedCell};
use crate::view::geometry::Size;

/// Frame-scoped command recorder passed to [`super::CanvasItem::draw`].
#[derive(Debug)]
pub struct CanvasContext {
    size: Size,
    commands: Vec<RecordedCommand>,
}

impl CanvasContext {
    pub(super) fn new(size: Size) -> Self {
        Self {
            size,
            commands: Vec::new(),
        }
    }

    pub const fn size(&self) -> Size {
        self.size
    }

    pub const fn bounds(&self) -> (Position, Size) {
        (Position::new(0, 0), self.size)
    }

    /// Records a View using [`Composition::Replace`].
    ///
    /// Supply a finite allocation on every axis whose result depends on
    /// [`crate::Length::Fill`]. Canvas bounds are not an implicit allocation.
    pub fn view(
        &mut self,
        origin: Position,
        view: View,
        width: Option<usize>,
        height: Option<usize>,
    ) {
        self.view_with(origin, view, width, height, Composition::Replace);
    }

    /// Records a View with an explicit composition rule.
    ///
    /// Allocation requirements are the same as for [`Self::view`].
    pub fn view_with(
        &mut self,
        origin: Position,
        view: View,
        width: Option<usize>,
        height: Option<usize>,
        composition: Composition,
    ) {
        self.record(
            ViewCommand {
                origin,
                view,
                allocation: (width, height),
            },
            composition,
        );
    }

    /// Records plain text using [`Composition::Overlay`].
    ///
    /// As with [`View::text`], `text` must not contain terminal controls.
    pub fn text(&mut self, origin: Position, text: impl Into<String>, style: TextStyle) {
        self.text_with(origin, text, style, Composition::Overlay);
    }

    /// Records plain text with an explicit composition rule.
    ///
    /// The plain-text contract is the same as for [`Self::text`].
    pub fn text_with(
        &mut self,
        origin: Position,
        text: impl Into<String>,
        style: TextStyle,
        composition: Composition,
    ) {
        self.record(
            TextCommand {
                origin,
                text: text.into(),
                style,
            },
            composition,
        );
    }

    /// Draws a marker along one cell-space segment using [`Composition::Overlay`].
    ///
    /// The segment may be horizontal, vertical, or diagonal. Intersecting
    /// lines remain ordinary marker cells; use [`LineNetwork`] when crossings
    /// must be derived from connectivity.
    ///
    /// # Panics
    ///
    /// Panics if `marker` is not exactly one printable, one-cell grapheme.
    pub fn line(&mut self, from: Position, to: Position, marker: &Grapheme, style: TextStyle) {
        self.line_with(from, to, marker, style, Composition::Overlay);
    }

    /// Draws a marker along one cell-space segment with explicit composition.
    ///
    /// # Panics
    ///
    /// Panics if `marker` is not exactly one printable, one-cell grapheme.
    pub fn line_with(
        &mut self,
        from: Position,
        to: Position,
        marker: &Grapheme,
        style: TextStyle,
        composition: Composition,
    ) {
        self.record(CellPath::line(from, to, marker, style), composition);
    }

    /// Draws connected marker segments using [`Composition::Overlay`].
    ///
    /// # Panics
    ///
    /// Panics if `marker` is not exactly one printable, one-cell grapheme.
    pub fn polyline(
        &mut self,
        points: impl IntoIterator<Item = Position>,
        marker: &Grapheme,
        style: TextStyle,
    ) {
        self.polyline_with(points, marker, style, Composition::Overlay);
    }

    /// Draws connected marker segments with explicit composition.
    ///
    /// # Panics
    ///
    /// Panics if `marker` is not exactly one printable, one-cell grapheme.
    pub fn polyline_with(
        &mut self,
        points: impl IntoIterator<Item = Position>,
        marker: &Grapheme,
        style: TextStyle,
        composition: Composition,
    ) {
        self.record(CellPath::polyline(points, marker, style), composition);
    }

    /// Draws a cell-aligned marker rectangle using [`Composition::Overlay`].
    ///
    /// # Panics
    ///
    /// Panics if `marker` is not exactly one printable, one-cell grapheme.
    pub fn rectangle(
        &mut self,
        origin: Position,
        width: usize,
        height: usize,
        marker: &Grapheme,
        style: TextStyle,
    ) {
        self.rectangle_with(origin, width, height, marker, style, Composition::Overlay);
    }

    /// Draws a cell-aligned marker rectangle with explicit composition.
    ///
    /// # Panics
    ///
    /// Panics if `marker` is not exactly one printable, one-cell grapheme.
    pub fn rectangle_with(
        &mut self,
        origin: Position,
        width: usize,
        height: usize,
        marker: &Grapheme,
        style: TextStyle,
        composition: Composition,
    ) {
        self.record(
            CellPath::rectangle(origin, width, height, marker, style),
            composition,
        );
    }

    /// Draws horizontal and vertical segments as one connected network.
    ///
    /// Intersections between segments in this network are rasterized as
    /// corners, tees, and crossings. The resulting cells use
    /// [`Composition::Overlay`], like other cell-producing commands.
    pub fn line_network(&mut self, network: LineNetwork) {
        self.line_network_with(network, Composition::Overlay);
    }

    /// Draws one connected line network with an explicit composition rule.
    ///
    /// The rule combines this network's rasterized glyphs with earlier Canvas
    /// cells. It does not merge this value's connections with another command.
    pub fn line_network_with(&mut self, network: LineNetwork, composition: Composition) {
        self.record(network, composition);
    }

    /// Records sparse cell contributions using [`Composition::Overlay`].
    pub fn cells(&mut self, cells: impl IntoIterator<Item = PositionedCell>) {
        self.cells_with(cells, Composition::Overlay);
    }

    /// Records sparse cell contributions with an explicit composition rule.
    pub fn cells_with(
        &mut self,
        cells: impl IntoIterator<Item = PositionedCell>,
        composition: Composition,
    ) {
        self.record(CellsCommand(cells.into_iter().collect()), composition);
    }

    fn record(
        &mut self,
        command: impl super::command::CanvasCommand + 'static,
        composition: Composition,
    ) {
        self.commands
            .push(RecordedCommand::new(command, composition));
    }

    pub(super) fn into_commands(self) -> Vec<RecordedCommand> {
        self.commands
    }
}
