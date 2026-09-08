use crate::{TextStyle, View};

use super::{Composition, Path, Position, PositionedCell};
use crate::view::geometry::Size;

#[derive(Debug, Clone)]
pub(super) enum CanvasCommand {
    View {
        origin: Position,
        view: View,
        allocation: (Option<usize>, Option<usize>),
        composition: Composition,
    },
    Text {
        origin: Position,
        text: String,
        style: TextStyle,
        composition: Composition,
    },
    Path {
        path: Path,
        composition: Composition,
    },
    Cells {
        cells: Vec<PositionedCell>,
        composition: Composition,
    },
}

/// Frame-scoped command recorder passed to [`super::CanvasItem::draw`].
#[derive(Debug)]
pub struct CanvasContext {
    size: Size,
    commands: Vec<CanvasCommand>,
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
        self.commands.push(CanvasCommand::View {
            origin,
            view,
            allocation: (width, height),
            composition,
        });
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
        self.commands.push(CanvasCommand::Text {
            origin,
            text: text.into(),
            style,
            composition,
        });
    }

    /// Records connected geometry using [`Composition::Overlay`].
    pub fn path(&mut self, path: Path) {
        self.path_with(path, Composition::Overlay);
    }

    /// Records connected geometry with an explicit composition rule.
    pub fn path_with(&mut self, path: Path, composition: Composition) {
        self.commands
            .push(CanvasCommand::Path { path, composition });
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
        self.commands.push(CanvasCommand::Cells {
            cells: cells.into_iter().collect(),
            composition,
        });
    }

    pub(super) fn into_commands(self) -> Vec<CanvasCommand> {
        self.commands
    }
}
