//! The [`GridStyle`] builder: shared column lengths and default cell padding.

use crate::{Length, Sides};

/// A grid's geometry: an optional [`Length`] per column and the padding its
/// cells take.
///
/// A `GridStyle` is an immutable value, like [`BlockStyle`](crate::BlockStyle).
/// It holds no box geometry or line presentation: a grid that needs a border,
/// margin, or stated size is placed inside a
/// [`View::Block`](crate::View::Block), while a presentation that needs an
/// internal line network owns it in a [`Canvas`](crate::Canvas).
///
/// ```
/// use urushi::{GridStyle, Length, TextStyle, View, measure};
///
/// let style = GridStyle::new()
///     .cell_padding((0, 1))
///     .columns([None, Some(Length::fill(1))]);
/// let grid = View::grid(
///     style,
///     [
///         [View::text("id", TextStyle::new()), View::text("name", TextStyle::new())],
///         [View::text("1", TextStyle::new()), View::text("urushi", TextStyle::new())],
///     ],
/// );
///
/// assert_eq!(measure(&grid).height(), 2);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GridStyle {
    columns: Vec<Option<Length>>,
    cell_padding: Sides,
}

impl GridStyle {
    pub fn new() -> Self {
        Self::default()
    }

    /// States the [`Length`] each column claims, left to right.
    ///
    /// A column with no stated length — an absent entry, or one past the end
    /// of this list — is auto: it claims the intrinsic width of the cells
    /// beneath it. A stated length supplies only the *kind* of the claim; the
    /// demand and the floor still come from those cells.
    pub fn columns(mut self, columns: impl IntoIterator<Item = Option<Length>>) -> Self {
        self.columns = columns.into_iter().collect();
        self
    }

    /// Sets the padding every cell that states none of its own takes.
    ///
    /// A cell that states its own padding replaces this value rather than
    /// adding to it. Columns stay aligned either way: a column is as wide as
    /// its widest cell, whatever padding that cell carries.
    pub fn cell_padding(mut self, sides: impl Into<Sides>) -> Self {
        self.cell_padding = sides.into();
        self
    }

    /// Returns the stated length of column `index`, if it states one.
    pub fn column_length(&self, index: usize) -> Option<Length> {
        self.columns.get(index).copied().flatten()
    }

    /// Returns the padding a cell that states none of its own takes.
    pub const fn cell_padding_sides(&self) -> Sides {
        self.cell_padding
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_column_past_the_stated_list_is_auto() {
        let style = GridStyle::new().columns([Some(Length::Cells(4)), None]);

        assert_eq!(style.column_length(0), Some(Length::Cells(4)));
        assert_eq!(style.column_length(1), None, "stated as absent");
        assert_eq!(style.column_length(9), None, "past the end");
    }

    #[test]
    fn builders_can_restore_grid_defaults_explicitly() {
        let stated = GridStyle::new()
            .columns([Some(Length::Cells(4))])
            .cell_padding((0, 1));

        assert_eq!(
            stated.clone().columns([]).cell_padding(Sides::default()),
            GridStyle::new()
        );
        assert_ne!(stated, GridStyle::new(), "the removals did the work");
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
