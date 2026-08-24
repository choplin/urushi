//! The [`GridStyle`] builder: the lines a grid draws and the width its columns
//! claim.

use crate::{Border, Color, GridStyleProperty, GridStylePropertyKey, Length, Sides, TextStyle};

/// A grid's own style: a glyph set, the edges and separators it draws, an
/// optional [`Length`] per column, and the padding its cells take.
///
/// A `GridStyle` is an immutable value, like [`BlockStyle`](crate::BlockStyle).
/// It holds no box geometry: a grid that needs a border of its own, a margin,
/// or a stated size is placed inside a [`View::Block`](crate::View::Block).
/// What it does hold is what only the container can decide — one width per
/// column, and the glyph at every intersection of the lines it draws.
///
/// Like a block, a new style enables every edge but draws nothing until a
/// [`Border`] is set.
///
/// ```
/// use urushi::{Border, GridStyle, Length, TextStyle, View, measure};
///
/// let style = GridStyle::new()
///     .border(Border::NORMAL)
///     .cell_padding((0, 1))
///     .columns([None, Some(Length::Fill(1))]);
/// let grid = View::grid(
///     style,
///     [
///         [View::text("id", TextStyle::new()), View::text("name", TextStyle::new())],
///         [View::text("1", TextStyle::new()), View::text("urushi", TextStyle::new())],
///     ],
/// );
///
/// assert_eq!(measure(&grid).height(), 5, "two rows, two outer edges, one rule");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridStyle {
    border: Option<Border>,
    border_top: bool,
    border_right: bool,
    border_bottom: bool,
    border_left: bool,
    border_column: bool,
    border_row: bool,
    border_header: Option<bool>,
    border_fg: Option<Color>,
    border_bg: Option<Color>,
    columns: Vec<Option<Length>>,
    cell_padding: Sides,
}

impl Default for GridStyle {
    fn default() -> Self {
        Self {
            border: None,
            border_top: true,
            border_right: true,
            border_bottom: true,
            border_left: true,
            border_column: true,
            border_row: true,
            border_header: None,
            border_fg: None,
            border_bg: None,
            columns: Vec::new(),
            cell_padding: Sides::default(),
        }
    }
}

impl GridStyle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces a property in this style.
    // This is the collection operation paired with `remove`, not arithmetic.
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, property: impl Into<GridStyleProperty>) -> Self {
        match property.into() {
            GridStyleProperty::Border(border) => self.border = Some(border),
            GridStyleProperty::BorderTop(enabled) => self.border_top = enabled,
            GridStyleProperty::BorderRight(enabled) => self.border_right = enabled,
            GridStyleProperty::BorderBottom(enabled) => self.border_bottom = enabled,
            GridStyleProperty::BorderLeft(enabled) => self.border_left = enabled,
            GridStyleProperty::BorderColumn(enabled) => self.border_column = enabled,
            GridStyleProperty::BorderRow(enabled) => self.border_row = enabled,
            GridStyleProperty::BorderHeader(enabled) => self.border_header = Some(enabled),
            GridStyleProperty::BorderForeground(color) => self.border_fg = Some(color),
            GridStyleProperty::BorderBackground(color) => self.border_bg = Some(color),
            GridStyleProperty::Columns(columns) => self.columns = columns,
            GridStyleProperty::CellPadding(sides) => self.cell_padding = sides,
        }
        self
    }

    /// Removes a property from this style, restoring its default value.
    pub fn remove(mut self, property: impl Into<GridStylePropertyKey>) -> Self {
        match property.into() {
            GridStylePropertyKey::Border => self.border = None,
            GridStylePropertyKey::BorderTop => self.border_top = true,
            GridStylePropertyKey::BorderRight => self.border_right = true,
            GridStylePropertyKey::BorderBottom => self.border_bottom = true,
            GridStylePropertyKey::BorderLeft => self.border_left = true,
            GridStylePropertyKey::BorderColumn => self.border_column = true,
            GridStylePropertyKey::BorderRow => self.border_row = true,
            GridStylePropertyKey::BorderHeader => self.border_header = None,
            GridStylePropertyKey::BorderForeground => self.border_fg = None,
            GridStylePropertyKey::BorderBackground => self.border_bg = None,
            GridStylePropertyKey::Columns => self.columns = Vec::new(),
            GridStylePropertyKey::CellPadding => self.cell_padding = Sides::default(),
        }
        self
    }

    /// Sets the glyph set every line of the grid is drawn from.
    ///
    /// A new style enables all six switches. Use the `border_*` builders to
    /// configure which lines are drawn.
    pub fn border(self, border: Border) -> Self {
        self.add(GridStyleProperty::Border(border))
    }

    /// Enables or disables the outer top edge.
    pub fn border_top(self, enabled: bool) -> Self {
        self.add(GridStyleProperty::BorderTop(enabled))
    }

    /// Enables or disables the outer right edge.
    pub fn border_right(self, enabled: bool) -> Self {
        self.add(GridStyleProperty::BorderRight(enabled))
    }

    /// Enables or disables the outer bottom edge.
    pub fn border_bottom(self, enabled: bool) -> Self {
        self.add(GridStyleProperty::BorderBottom(enabled))
    }

    /// Enables or disables the outer left edge.
    pub fn border_left(self, enabled: bool) -> Self {
        self.add(GridStyleProperty::BorderLeft(enabled))
    }

    /// Enables or disables the line between two neighbouring columns.
    ///
    /// It contributes one column between neighbours and never at the ends,
    /// where the outer edges apply instead.
    pub fn border_column(self, enabled: bool) -> Self {
        self.add(GridStyleProperty::BorderColumn(enabled))
    }

    /// Enables or disables the line between two neighbouring rows.
    ///
    /// It contributes one row between neighbours and never at the ends, where
    /// the outer edges apply instead.
    pub fn border_row(self, enabled: bool) -> Self {
        self.add(GridStyleProperty::BorderRow(enabled))
    }

    /// States that the first row is a header, and whether the line below it is
    /// drawn.
    ///
    /// A grid that states nothing has no header row: every gap between two
    /// rows follows [`border_row`](Self::border_row). Stating this marks the
    /// first row as a header and takes that one gap out of `border_row`'s
    /// hands, in either direction — a header rule above rows that carry none
    /// between them, or rows that carry one between them below a header that
    /// does not.
    pub fn border_header(self, enabled: bool) -> Self {
        self.add(GridStyleProperty::BorderHeader(enabled))
    }

    /// Sets the foreground color every line is drawn in.
    pub fn border_foreground(self, color: impl Into<Color>) -> Self {
        self.add(GridStyleProperty::BorderForeground(color.into()))
    }

    /// Sets the background color every line is drawn in.
    pub fn border_background(self, color: impl Into<Color>) -> Self {
        self.add(GridStyleProperty::BorderBackground(color.into()))
    }

    /// States the [`Length`] each column claims, left to right.
    ///
    /// A column with no stated length — an absent entry, or one past the end
    /// of this list — is auto: it claims the intrinsic width of the cells
    /// beneath it. A stated length supplies only the *kind* of the claim; the
    /// demand and the floor still come from those cells.
    pub fn columns(self, columns: impl IntoIterator<Item = Option<Length>>) -> Self {
        self.add(GridStyleProperty::Columns(columns.into_iter().collect()))
    }

    /// Sets the padding every cell that states none of its own takes.
    ///
    /// A cell that states its own padding replaces this value rather than
    /// adding to it. Columns stay aligned either way: a column is as wide as
    /// its widest cell, whatever padding that cell carries.
    pub fn cell_padding(self, sides: impl Into<Sides>) -> Self {
        self.add(GridStyleProperty::CellPadding(sides.into()))
    }

    /// Returns the glyph set the lines are drawn from, if the grid draws any.
    pub const fn border_kind(&self) -> Option<Border> {
        self.border
    }

    /// Returns whether the outer top edge is enabled.
    pub const fn is_border_top_enabled(&self) -> bool {
        self.border_top
    }

    /// Returns whether the outer right edge is enabled.
    pub const fn is_border_right_enabled(&self) -> bool {
        self.border_right
    }

    /// Returns whether the outer bottom edge is enabled.
    pub const fn is_border_bottom_enabled(&self) -> bool {
        self.border_bottom
    }

    /// Returns whether the outer left edge is enabled.
    pub const fn is_border_left_enabled(&self) -> bool {
        self.border_left
    }

    /// Returns whether the line between neighbouring columns is enabled.
    pub const fn is_border_column_enabled(&self) -> bool {
        self.border_column
    }

    /// Returns whether the line between neighbouring rows is enabled.
    pub const fn is_border_row_enabled(&self) -> bool {
        self.border_row
    }

    /// Returns whether the first row is a header and carries a line below it.
    ///
    /// `None` is a grid with no header row, whose first gap follows
    /// [`is_border_row_enabled`](Self::is_border_row_enabled) like every
    /// other.
    pub const fn border_header_enabled(&self) -> Option<bool> {
        self.border_header
    }

    /// Returns the border foreground color instruction.
    pub const fn border_foreground_color(&self) -> Option<Color> {
        self.border_fg
    }

    /// Returns the border background color instruction.
    pub const fn border_background_color(&self) -> Option<Color> {
        self.border_bg
    }

    /// Returns the stated length of column `index`, if it states one.
    pub fn column_length(&self, index: usize) -> Option<Length> {
        self.columns.get(index).copied().flatten()
    }

    /// Returns the padding a cell that states none of its own takes.
    pub const fn cell_padding_sides(&self) -> Sides {
        self.cell_padding
    }

    /// The style drawn on this grid's line glyphs.
    pub(crate) fn border_style(&self) -> TextStyle {
        let mut style = TextStyle::new();
        if let Some(color) = self.border_fg {
            style = style.foreground(color);
        }
        if let Some(color) = self.border_bg {
            style = style.background(color);
        }
        style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_style_enables_every_line_and_draws_none() {
        let style = GridStyle::new();

        assert_eq!(style.border_kind(), None);
        assert!(style.is_border_top_enabled());
        assert!(style.is_border_right_enabled());
        assert!(style.is_border_bottom_enabled());
        assert!(style.is_border_left_enabled());
        assert!(style.is_border_column_enabled());
        assert!(style.is_border_row_enabled());
    }

    #[test]
    fn a_grid_states_nothing_about_a_header_until_it_has_one() {
        assert_eq!(GridStyle::new().border_header_enabled(), None);
        assert_eq!(
            GridStyle::new()
                .border_header(false)
                .border_header_enabled(),
            Some(false),
            "stating no rule below the header is not the same as having no header"
        );
        assert_eq!(
            GridStyle::new().border_header(true).border_header_enabled(),
            Some(true)
        );
    }

    #[test]
    fn a_column_past_the_stated_list_is_auto() {
        let style = GridStyle::new().columns([Some(Length::Cells(4)), None]);

        assert_eq!(style.column_length(0), Some(Length::Cells(4)));
        assert_eq!(style.column_length(1), None, "stated as absent");
        assert_eq!(style.column_length(9), None, "past the end");
    }

    #[test]
    fn removing_a_property_restores_its_default() {
        let stated = GridStyle::new()
            .border(Border::NORMAL)
            .border_column(false)
            .border_header(true)
            .border_foreground(Color::RED)
            .columns([Some(Length::Cells(4))])
            .cell_padding((0, 1));

        assert_eq!(
            stated
                .clone()
                .remove(GridStylePropertyKey::Border)
                .remove(GridStylePropertyKey::BorderColumn)
                .remove(GridStylePropertyKey::BorderHeader)
                .remove(GridStylePropertyKey::BorderForeground)
                .remove(GridStylePropertyKey::Columns)
                .remove(GridStylePropertyKey::CellPadding),
            GridStyle::new()
        );
        assert_ne!(stated, GridStyle::new(), "the removals did the work");
    }

    #[test]
    fn a_named_builder_is_the_generic_add_under_another_name() {
        assert_eq!(
            GridStyle::new().border_row(false),
            GridStyle::new().add(GridStyleProperty::BorderRow(false))
        );
        assert_eq!(
            GridStyle::new().cell_padding((0, 1)),
            GridStyle::new().add(GridStyleProperty::CellPadding(Sides::from((0, 1))))
        );
    }

    #[test]
    fn cell_padding_accepts_the_same_shorthands_a_block_takes() {
        assert_eq!(
            GridStyle::new().cell_padding((0, 1)).cell_padding_sides(),
            Sides {
                top: 0,
                right: 1,
                bottom: 0,
                left: 1,
            }
        );
    }
}
