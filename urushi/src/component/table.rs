//! Renderer-neutral tables with an independent public model.

use crate::{
    BlockStyle, BlockStylePropertyKey, Border, GridStyle, Length, TableRole, TextStyle, View,
};

/// Which row of a table a cell belongs to.
///
/// Deliberately private. [`TableCell`] exposes the distinction through
/// [`TableCell::is_header`] and [`TableCell::row`], both of which stay
/// truthful if a further row kind is ever added, so growing this costs no
/// public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableRow {
    Header,
    Body(usize),
}

/// Identifies one cell while a [`TableStyleFunc`] resolves its style.
///
/// Lip Gloss passes a bare `(row, column)` pair and encodes the header row as
/// the sentinel index `-1`. urushi passes this opaque value instead, matching
/// the [`ListPosition`](crate::ListPosition) and
/// [`SiblingPosition`](crate::SiblingPosition) precedent: further context can
/// be exposed as new accessors without changing the hook signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCell<'a> {
    text: &'a str,
    row: TableRow,
    column: usize,
}

impl<'a> TableCell<'a> {
    /// Returns whether this cell belongs to the header row.
    pub const fn is_header(&self) -> bool {
        matches!(self.row, TableRow::Header)
    }

    /// Returns the cell's body-row number, or `None` when the cell is not a
    /// body cell.
    ///
    /// Body rows are numbered from zero, counting the rows the table's offset
    /// leaves visible rather than the rows it owns.
    pub const fn row(&self) -> Option<usize> {
        match self.row {
            TableRow::Body(index) => Some(index),
            _ => None,
        }
    }

    /// Returns the zero-based column index.
    pub const fn column(&self) -> usize {
        self.column
    }

    /// Returns the cell's text, before wrapping and padding.
    pub const fn text(&self) -> &'a str {
        self.text
    }
}

/// Chooses the style of one table cell.
///
/// `None` keeps the [`TableStyle`] role default for that cell, so a hook only
/// has to name the cells it overrides. `Some(style)` **replaces** the role
/// default outright rather than layering over it, so a hook that means to keep
/// the theme's colors must set them itself:
///
/// ```
/// use urushi::{Align, BlockStyle, Color, TextStyle, TableCell, TableStyle};
///
/// fn numbers_right(cell: TableCell<'_>) -> Option<BlockStyle> {
///     if cell.is_header() || cell.column() == 0 {
///         return None;
///     }
///     // Restates the foreground, because Some replaces the role default.
///     Some(BlockStyle::new().foreground(Color::CYAN).align(Align::Right))
/// }
///
/// let cells = BlockStyle::new().foreground(Color::CYAN);
/// let table_style = TableStyle::new(BlockStyle::new(), cells, TextStyle::new())
///     .style_func(numbers_right);
/// ```
///
/// A cell is a block, so the hook returns a [`BlockStyle`]. Its colors, text
/// modifiers, alignment, and padding are used; a cell that states its own
/// padding replaces the table's for that cell, and its column is as wide as it
/// needs. The rest of the geometry — border, margin, and stated sizes — is
/// dropped, because the grid the table composes owns layout.
///
/// The hook is a function pointer rather than a boxed closure, which keeps
/// [`TableStyle`] `Clone` and comparable.
pub type TableStyleFunc = fn(cell: TableCell<'_>) -> Option<BlockStyle>;

/// Keeps every cell on its role default.
pub fn default_table_style_func(_: TableCell<'_>) -> Option<BlockStyle> {
    None
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RowOffset {
    start: usize,
    end: usize,
}

/// Owned table data independent of presentation policy.
///
/// `Table` owns semantic content, structural visibility, and offsets only. Every
/// presentation concern — borders, padding, width, and cell styles — belongs to
/// [`TableStyle`].
///
/// Rows may be ragged: a row shorter than the widest row is padded with empty
/// cells when it is composed.
///
/// ```
/// use urushi::{BlockStyle, TextStyle, Table, TableStyle, measure};
///
/// let table = Table::new()
///     .headers(["Name", "Location"])
///     .row(["Kini", "New York"])
///     .row(["Iris", "Paris"]);
///
/// let table_style = TableStyle::new(BlockStyle::new(), BlockStyle::new(), TextStyle::new());
/// assert_eq!(measure(&table_style.view(&table)).height(), 6);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    hidden: bool,
    offset: RowOffset,
}

impl Table {
    /// Creates an empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the header cells, each of which is plain text.
    ///
    /// A table without headers composes its body rows only. Each cell is plain
    /// text. Escape sequences and cursor movement in it break that contract:
    /// debug builds panic, and release builds measure them as ordinary
    /// characters and may split them when wrapping or truncating. Adopt
    /// already-rendered output with
    /// [`RenderedBlock::from_ansi`](crate::RenderedBlock::from_ansi) instead.
    #[must_use]
    pub fn headers<I, S>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.headers = headers.into_iter().map(Into::into).collect();
        self
    }

    /// Appends one body row.
    #[must_use]
    pub fn row<I, S>(mut self, cells: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.rows.push(cells.into_iter().map(Into::into).collect());
        self
    }

    /// Appends body rows in iteration order, each cell being plain text.
    ///
    /// Escape sequences and cursor movement in it break that contract:
    /// debug builds panic, and release builds measure them as ordinary
    /// characters and may split them when wrapping or truncating. Adopt
    /// already-rendered output with
    /// [`RenderedBlock::from_ansi`](crate::RenderedBlock::from_ansi) instead.
    #[must_use]
    pub fn rows<I, R, S>(mut self, rows: I) -> Self
    where
        I: IntoIterator<Item = R>,
        R: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for row in rows {
            self.rows.push(row.into_iter().map(Into::into).collect());
        }
        self
    }

    /// Includes or excludes the complete table.
    #[must_use]
    pub const fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Omits `start` body rows from the front and `end` from the back.
    ///
    /// Headers are never omitted; the offset selects among body rows only.
    #[must_use]
    pub const fn offset(mut self, start: usize, end: usize) -> Self {
        self.offset = RowOffset { start, end };
        self
    }

    /// Returns whether the table is excluded.
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Returns the column count, taken from the widest of headers and rows.
    ///
    /// This counts every owned row. Composition instead uses only the rows the
    /// offset leaves visible, so a wide row excluded by [`Table::offset`] does
    /// not add columns to the composed view.
    pub fn column_count(&self) -> usize {
        column_count(&self.rows, self.headers.len())
    }

    fn visible_rows(&self) -> &[Vec<String>] {
        let end = self.rows.len().saturating_sub(self.offset.end);
        if self.offset.start >= end {
            return &[];
        }
        &self.rows[self.offset.start..end]
    }
}

/// Presentation policy used to compose a [`Table`] into a [`View`].
///
/// The policy owns the border, the edge toggles, cell padding, the total width
/// constraint, and the per-cell style hook. It borrows the data and performs no
/// terminal output.
#[derive(Debug, Clone)]
pub struct TableStyle {
    header: BlockStyle,
    cell: BlockStyle,
    border_style: TextStyle,
    border: Border,
    border_top: bool,
    border_bottom: bool,
    border_left: bool,
    border_right: bool,
    border_header: bool,
    border_column: bool,
    border_row: bool,
    padding: u16,
    width: Option<u16>,
    style_func: TableStyleFunc,
}

impl PartialEq for TableStyle {
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header
            && self.cell == other.cell
            && self.border_style == other.border_style
            && self.border == other.border
            && self.border_top == other.border_top
            && self.border_bottom == other.border_bottom
            && self.border_left == other.border_left
            && self.border_right == other.border_right
            && self.border_header == other.border_header
            && self.border_column == other.border_column
            && self.border_row == other.border_row
            && self.padding == other.padding
            && self.width == other.width
            && std::ptr::fn_addr_eq(self.style_func, other.style_func)
    }
}

impl TableStyle {
    /// Creates a table style with a single-line border and one cell of padding.
    pub fn new(header: BlockStyle, cell: BlockStyle, border: TextStyle) -> Self {
        Self {
            header,
            cell,
            border_style: border,
            border: Border::NORMAL,
            border_top: true,
            border_bottom: true,
            border_left: true,
            border_right: true,
            border_header: true,
            border_column: true,
            border_row: false,
            padding: 1,
            width: None,
            style_func: default_table_style_func,
        }
    }

    /// Returns the block style assigned to one logical table cell role.
    pub const fn style(&self, role: TableRole) -> &BlockStyle {
        match role {
            TableRole::Header => &self.header,
            TableRole::Cell => &self.cell,
        }
    }

    /// Returns the style drawn on the table's rules.
    pub const fn border_glyph_style(&self) -> &TextStyle {
        &self.border_style
    }

    /// Replaces the block style assigned to one logical table cell role.
    #[must_use]
    pub fn with_style(mut self, role: TableRole, style: BlockStyle) -> Self {
        match role {
            TableRole::Header => self.header = style,
            TableRole::Cell => self.cell = style,
        }
        self
    }

    /// Replaces the header-cell style.
    #[must_use]
    pub fn header_style(self, style: BlockStyle) -> Self {
        self.with_style(TableRole::Header, style)
    }

    /// Replaces the body-cell style.
    #[must_use]
    pub fn cell_style(self, style: BlockStyle) -> Self {
        self.with_style(TableRole::Cell, style)
    }

    /// Replaces the style drawn on the table's rules.
    #[must_use]
    pub fn border_style(mut self, style: TextStyle) -> Self {
        self.border_style = style;
        self
    }

    /// Replaces the border character set.
    #[must_use]
    pub const fn border(mut self, border: Border) -> Self {
        self.border = border;
        self
    }

    /// Draws or omits the top edge.
    #[must_use]
    pub const fn border_top(mut self, enabled: bool) -> Self {
        self.border_top = enabled;
        self
    }

    /// Draws or omits the bottom edge.
    #[must_use]
    pub const fn border_bottom(mut self, enabled: bool) -> Self {
        self.border_bottom = enabled;
        self
    }

    /// Draws or omits the left edge.
    #[must_use]
    pub const fn border_left(mut self, enabled: bool) -> Self {
        self.border_left = enabled;
        self
    }

    /// Draws or omits the right edge.
    #[must_use]
    pub const fn border_right(mut self, enabled: bool) -> Self {
        self.border_right = enabled;
        self
    }

    /// Draws or omits the rule between the header and the first body row.
    #[must_use]
    pub const fn border_header(mut self, enabled: bool) -> Self {
        self.border_header = enabled;
        self
    }

    /// Draws or omits the rules between columns.
    ///
    /// A column rule uses the border's `left` glyph, the same one that draws
    /// the left edge; [`Border`] has no separate vertical-separator glyph.
    #[must_use]
    pub const fn border_column(mut self, enabled: bool) -> Self {
        self.border_column = enabled;
        self
    }

    /// Draws or omits the rules between body rows.
    #[must_use]
    pub const fn border_row(mut self, enabled: bool) -> Self {
        self.border_row = enabled;
        self
    }

    /// Sets the horizontal padding applied to both sides of every cell.
    #[must_use]
    pub const fn padding(mut self, padding: u16) -> Self {
        self.padding = padding;
        self
    }

    /// States the total width the table takes, in terminal cells.
    ///
    /// A total width is a statement about the box the table sits in, so
    /// [`view`](Self::view) puts the grid in one: a block of exactly this
    /// width. What the grid then does with it is a statement about columns,
    /// and the vocabulary for "take a share of what is there" is
    /// [`Length::Fill`]. Every column takes an equal share, which is why a
    /// width much wider than the content can leave a short column wider than a
    /// long one, and a width narrower than the content narrows every column
    /// rather than only the widest.
    ///
    /// Nothing is measured while composing. The columns and the lines between
    /// them are decided together when the view resolves, so they cannot
    /// disagree, and a column floors at the narrowest grapheme it must show:
    /// a table given less than its floor is as wide as that floor, wrapping
    /// its content rather than dropping it.
    ///
    /// Without a stated width the table is as wide as its content, and an area
    /// narrower than that shrinks it by the same rule. This states the width
    /// for a caller who wants one regardless of the area.
    ///
    /// The width is a `u16` for the same reason [`BlockStyle::width`] is: a
    /// terminal dimension, not an arbitrary count.
    #[must_use]
    pub const fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Replaces the per-cell style hook.
    #[must_use]
    pub const fn style_func(mut self, style_func: TableStyleFunc) -> Self {
        self.style_func = style_func;
        self
    }

    /// Composes table data into a renderer-neutral view.
    ///
    /// The result is one [`View::Grid`]: the table states which cells share a
    /// column and which lines are drawn, and
    /// [`resolve`](crate::resolve) decides every width, every wrap, and the
    /// glyph at every intersection. Nothing here is measured, so a table
    /// re-fitted to a narrower area shrinks its columns and its lines
    /// together.
    ///
    /// A cell style contributes its colors, text modifiers, alignment, and
    /// padding. The rest of its geometry — border, margin, and stated sizes —
    /// is dropped, because the grid owns layout.
    pub fn view(&self, table: &Table) -> View {
        if table.is_hidden() {
            return View::empty();
        }

        let rows = table.visible_rows();
        // Only the visible rows may widen the composed table; a wide row the
        // offset excluded must not leave empty columns behind.
        let columns = column_count(rows, table.headers.len());
        if columns == 0 {
            return View::empty();
        }

        let has_headers = !table.headers.is_empty();
        let mut cells = Vec::with_capacity(rows.len() + usize::from(has_headers));
        if has_headers {
            cells.push(self.cell_row(&table.headers, columns, TableRow::Header));
        }
        for (index, row) in rows.iter().enumerate() {
            cells.push(self.cell_row(row, columns, TableRow::Body(index)));
        }

        let grid = View::grid(self.grid_style(columns, has_headers), cells);
        match self.width {
            // A total width is a statement about the box the table is in, not
            // about any one column; see `width`.
            Some(width) => View::block(BlockStyle::new().width(Length::Cells(width)), grid),
            None => grid,
        }
    }

    /// The grid style this table's policy describes.
    ///
    /// `border_header` is stated only where there is a header row to state it
    /// about: without one, the first gap is an ordinary gap between two body
    /// rows and follows `border_row` like every other.
    fn grid_style(&self, columns: usize, has_headers: bool) -> GridStyle {
        let mut style = GridStyle::new()
            .border(self.border)
            .border_top(self.border_top)
            .border_bottom(self.border_bottom)
            .border_left(self.border_left)
            .border_right(self.border_right)
            .border_column(self.border_column)
            .border_row(self.border_row)
            .cell_padding((0, self.padding));
        if has_headers {
            style = style.border_header(self.border_header);
        }
        if let Some(color) = self.border_style.foreground_color() {
            style = style.border_foreground(color);
        }
        if let Some(color) = self.border_style.background_color() {
            style = style.border_background(color);
        }
        // A stated total width is spent on the columns, which is the only
        // place a grid has to put it; see `width`.
        if self.width.is_some() {
            style = style.columns(std::iter::repeat_n(Some(Length::Fill(1)), columns));
        }
        style
    }

    /// Builds one row of cells, padding a short row out to the column count.
    ///
    /// A grid is a rectangle and supplies no cell of its own, so the empty
    /// cell of a ragged row is composed here, where the table's own cell style
    /// is known.
    fn cell_row(&self, cells: &[String], columns: usize, row: TableRow) -> Vec<View> {
        (0..columns)
            .map(|column| {
                let text = cells.get(column).map_or("", String::as_str);
                self.cell(TableCell { text, row, column })
            })
            .collect()
    }

    /// Wraps one cell's text in the block that carries its style.
    fn cell(&self, cell: TableCell<'_>) -> View {
        let style = self
            .cell_style_at(cell)
            .remove(BlockStylePropertyKey::Border)
            .remove(BlockStylePropertyKey::Margin)
            .remove(BlockStylePropertyKey::Width)
            .remove(BlockStylePropertyKey::Height)
            .remove(BlockStylePropertyKey::MinWidth)
            .remove(BlockStylePropertyKey::MinHeight)
            .remove(BlockStylePropertyKey::MaxWidth)
            .remove(BlockStylePropertyKey::MaxHeight);
        let text = View::text(cell.text.to_owned(), style.text().clone());
        View::block(style, text)
    }

    /// Resolves one cell's style from the hook, falling back to its role.
    fn cell_style_at(&self, cell: TableCell<'_>) -> BlockStyle {
        (self.style_func)(cell).unwrap_or_else(|| match cell.row {
            TableRow::Header => self.header.clone(),
            _ => self.cell.clone(),
        })
    }
}

/// Returns the column count spanned by `rows` and a header of `headers` cells.
fn column_count(rows: &[Vec<String>], headers: usize) -> usize {
    rows.iter()
        .map(Vec::len)
        .chain(std::iter::once(headers))
        .max()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{plain_exact as plain, plain_rows, style_at};
    use crate::{
        Align, Available, Color, ComponentStyles, PrintableText, SemanticTokens, StyledGrapheme,
        VerticalAlign, measure, resolve,
    };

    fn styles() -> ComponentStyles {
        ComponentStyles::from_tokens(&SemanticTokens {
            text: Color::WHITE,
            text_muted: Color::BRIGHT_BLACK,
            background: Color::BLACK,
            surface: Color::BLACK,
            accent: Color::CYAN,
            accent_text: Color::BLACK,
            success: Color::GREEN,
            warning: Color::YELLOW,
            error: Color::RED,
            border: Color::BRIGHT_BLACK,
        })
    }

    fn sample() -> Table {
        Table::new()
            .headers(["Name", "Location"])
            .row(["Kini", "New York"])
            .row(["Iris", "Paris"])
    }

    #[test]
    fn composes_headers_rows_and_borders() {
        assert_eq!(
            plain(&styles().table().view(&sample())),
            "\
┌──────┬──────────┐
│ Name │ Location │
├──────┼──────────┤
│ Kini │ New York │
│ Iris │ Paris    │
└──────┴──────────┘"
        );
    }

    #[test]
    fn empty_tables_compose_nothing() {
        let table_style = styles().table().clone();
        assert!(measure(&table_style.view(&Table::new())).is_empty());
        assert!(measure(&table_style.view(&sample().hidden(true))).is_empty());

        let headers_only = Table::new().headers(["Name", "Location"]);
        assert_eq!(
            plain(&table_style.view(&headers_only)),
            "\
┌──────┬──────────┐
│ Name │ Location │
└──────┴──────────┘"
        );

        let rows_only = Table::new().row(["Kini", "New York"]);
        assert_eq!(
            plain(&table_style.view(&rows_only)),
            "\
┌──────┬──────────┐
│ Kini │ New York │
└──────┴──────────┘"
        );
    }

    #[test]
    fn ragged_rows_pad_missing_cells() {
        let table = Table::new()
            .headers(["A", "B", "C"])
            .row(["1"])
            .row(["1", "2", "3"]);

        assert_eq!(
            plain(&styles().table().view(&table)),
            "\
┌───┬───┬───┐
│ A │ B │ C │
├───┼───┼───┤
│ 1 │   │   │
│ 1 │ 2 │ 3 │
└───┴───┴───┘"
        );
    }

    #[test]
    fn offsets_select_body_rows_and_keep_headers() {
        let table = sample().row(["Eli", "London"]).offset(1, 1);

        assert_eq!(
            plain(&styles().table().view(&table)),
            "\
┌──────┬──────────┐
│ Name │ Location │
├──────┼──────────┤
│ Iris │ Paris    │
└──────┴──────────┘",
            "an offset selects body rows; headers still set the column width"
        );
    }

    #[test]
    fn aligns_cjk_content_by_terminal_cell_width() {
        let table = Table::new()
            .headers(["名前", "所在地"])
            .row(["日本語", "東京"])
            .row(["ab", "Paris"]);
        let view = styles().table().view(&table);

        assert_eq!(
            plain(&view),
            "\
┌────────┬────────┐
│ 名前   │ 所在地 │
├────────┼────────┤
│ 日本語 │ 東京   │
│ ab     │ Paris  │
└────────┴────────┘"
        );
        let widths: Vec<usize> = plain_rows(&view)
            .iter()
            .map(|row| PrintableText::new(row).width())
            .collect();
        assert!(widths.iter().all(|width| *width == widths[0]));
    }

    #[test]
    fn multiline_cells_extend_the_row() {
        let table = Table::new()
            .headers(["Key", "Value"])
            .row(["a", "one\ntwo"]);

        assert_eq!(
            plain(&styles().table().view(&table)),
            "\
┌─────┬───────┐
│ Key │ Value │
├─────┼───────┤
│ a   │ one   │
│     │ two   │
└─────┴───────┘"
        );
    }

    #[test]
    fn width_expands_and_shrinks_columns() {
        let table_style = styles().table().clone();
        let wide = table_style.clone().width(28);
        let view = wide.view(&sample());
        for row in plain_rows(&view) {
            assert_eq!(PrintableText::new(&row).width(), 28);
        }

        let narrow = table_style.width(16);
        assert_eq!(
            plain(&narrow.view(&sample())),
            "\
┌──────┬───────┐
│ Name │ Locat │
│      │ ion   │
├──────┼───────┤
│ Kini │ New   │
│      │ York  │
│ Iris │ Paris │
└──────┴───────┘"
        );
    }

    #[test]
    fn a_width_constraint_narrower_than_a_grapheme_keeps_rows_rectangular() {
        // A column narrower than one wide grapheme is where a constraint runs
        // out: the grapheme cannot be split, so the column stops at it. The
        // grid widens past the constraint rather than dropping the content,
        // and every row still matches the frame — the table composes no width
        // of its own, so its lines and its cells cannot disagree.
        let table = Table::new().headers(["A", "B"]).row(["日本語", "x"]);
        let view = styles().table().clone().width(9).view(&table);

        assert_eq!(
            plain(&view),
            "\
┌────┬───┐
│ A  │ B │
├────┼───┤
│ 日 │ x │
│ 本 │   │
│ 語 │   │
└────┴───┘",
            "the column floors at one wide grapheme and wraps the rest"
        );

        let padded = styles()
            .table()
            .clone()
            .padding(0)
            .width(5)
            .view(&Table::new().row(["日本", "ab"]));
        let widths: Vec<usize> = plain_rows(&padded)
            .iter()
            .map(|row| PrintableText::new(row).width())
            .collect();
        assert!(
            widths.iter().all(|width| *width == widths[0]),
            "rectangular at its floor: {widths:?}"
        );
    }

    #[test]
    fn width_below_the_column_minimum_overflows() {
        let narrow = styles().table().clone().width(4);
        let view = narrow.view(&sample());
        let width = PrintableText::new(&plain_rows(&view)[0]).width();

        assert!(
            width > 4,
            "a table cannot shrink below its minimum: {width}"
        );
        assert_eq!(width, 9);
    }

    #[test]
    fn border_presets_change_every_glyph() {
        let table_style = styles().table().clone();
        let cases = [
            (
                Border::MARKDOWN,
                [
                    "|------|----------|",
                    "| Name | Location |",
                    "|------|----------|",
                    "| Kini | New York |",
                    "| Iris | Paris    |",
                    "|------|----------|",
                ]
                .join("\n"),
            ),
            (
                // The heavy outer rules and the light header rule come from
                // separate glyphs; the blank left and right edges still occupy
                // one cell each.
                Border::BOOKTABS,
                [
                    "━━━━━━━━━━━━━━━━━━━",
                    "  Name   Location  ",
                    "───────────────────",
                    "  Kini   New York  ",
                    "  Iris   Paris     ",
                    "━━━━━━━━━━━━━━━━━━━",
                ]
                .join("\n"),
            ),
            (
                Border::HIDDEN,
                [
                    "                   ",
                    "  Name   Location  ",
                    "                   ",
                    "  Kini   New York  ",
                    "  Iris   Paris     ",
                    "                   ",
                ]
                .join("\n"),
            ),
        ];

        for (border, expected) in cases {
            assert_eq!(
                plain(&table_style.clone().border(border).view(&sample())),
                expected
            );
        }
    }

    #[test]
    fn edge_toggles_omit_their_glyphs() {
        let table_style = styles()
            .table()
            .clone()
            .border_top(false)
            .border_bottom(false)
            .border_left(false)
            .border_right(false)
            .border_column(false);

        assert_eq!(
            plain(&table_style.view(&sample())),
            [
                " Name  Location ",
                "────────────────",
                " Kini  New York ",
                " Iris  Paris    ",
            ]
            .join("\n")
        );
    }

    #[test]
    fn row_rules_separate_every_body_row() {
        let table_style = styles().table().clone().border_row(true);

        assert_eq!(
            plain(&table_style.view(&sample())),
            "\
┌──────┬──────────┐
│ Name │ Location │
├──────┼──────────┤
│ Kini │ New York │
├──────┼──────────┤
│ Iris │ Paris    │
└──────┴──────────┘"
        );
    }

    fn accent_header(cell: TableCell<'_>) -> Option<BlockStyle> {
        if cell.is_header() {
            return Some(BlockStyle::new().foreground(Color::CYAN).bold());
        }
        match (cell.row(), cell.column()) {
            (Some(1), _) => Some(BlockStyle::new().foreground(Color::GREEN)),
            (_, 1) => Some(BlockStyle::new().align(Align::Right)),
            _ => None,
        }
    }

    #[test]
    fn the_style_hook_overrides_cells_rows_and_columns() {
        let component_styles = styles();
        let table_style = component_styles.table().clone().style_func(accent_header);
        let view = table_style.view(&sample());

        // Header row: matched by the (Header, _) arm.
        assert_eq!(
            style_at(&view, 1, 2),
            TextStyle::new().foreground(Color::CYAN).bold()
        );
        // Whole body row 1: matched by the (Body(1), _) arm, both columns.
        for column in [2, 9] {
            assert_eq!(
                style_at(&view, 4, column),
                TextStyle::new().foreground(Color::GREEN)
            );
        }
        // Column 0 of body row 0 falls through to the role default.
        assert_eq!(
            style_at(&view, 3, 2),
            component_styles
                .table()
                .style(TableRole::Cell)
                .text()
                .clone()
        );
        // Column 1 of body row 0: matched by the (_, 1) arm, which right-aligns
        // and — because Some replaces rather than layers — drops the role
        // default's foreground.
        assert_eq!(style_at(&view, 3, 9), TextStyle::new());
        assert_eq!(
            plain_rows(&view)[3],
            "│ Kini │ New York │",
            "the widest cell of a column has no slack to realign"
        );
    }

    #[test]
    fn a_hook_style_realigns_a_column_that_has_slack() {
        // Body row 2 escapes the (Body(1), _) arm, so the (_, 1) column arm
        // applies where the cell is narrower than its column.
        let table = sample().row(["Eli", "London"]);
        let table_style = styles().table().clone().style_func(accent_header);
        let view = table_style.view(&table);

        assert_eq!(
            plain_rows(&view)[5],
            "│ Eli  │   London │",
            "the (_, 1) arm right-aligns column 1 where the cell has slack"
        );
    }

    #[test]
    fn a_hook_style_keeps_its_padding_and_loses_the_rest_of_the_box() {
        fn boxed(_: TableCell<'_>) -> Option<BlockStyle> {
            Some(
                BlockStyle::new()
                    .padding((0, 3))
                    .width(30)
                    .height(4)
                    .border(Border::DOUBLE),
            )
        }

        let table_style = styles().table().clone().style_func(boxed);

        // A cell that states its own padding replaces the grid's rather than
        // adding to it, and its column is as wide as it needs — the rule
        // `docs/design/grid.md` states, reached here through the hook. The
        // frame, the stated sizes, and the margin are still the grid's to
        // decide, so none of them survive.
        assert_eq!(
            plain(&table_style.view(&sample())),
            "\
┌──────────┬──────────────┐
│   Name   │   Location   │
├──────────┼──────────────┤
│   Kini   │   New York   │
│   Iris   │   Paris      │
└──────────┴──────────────┘"
        );
    }

    #[test]
    fn an_offset_excluded_row_does_not_widen_the_table() {
        let table = Table::new()
            .headers(["A", "B"])
            .row(["1", "2"])
            .row(["w", "x", "y", "z"])
            .offset(0, 1);

        assert_eq!(
            table.column_count(),
            4,
            "the data-level count still spans every owned row"
        );
        assert_eq!(
            plain(&styles().table().view(&table)),
            "\
┌───┬───┐
│ A │ B │
├───┼───┤
│ 1 │ 2 │
└───┴───┘",
            "composition spans only the visible rows"
        );
    }

    #[test]
    fn cell_alignment_comes_from_the_cell_style() {
        fn right(_: TableCell<'_>) -> Option<BlockStyle> {
            Some(BlockStyle::new().align(Align::Right))
        }

        let table_style = styles().table().clone().style_func(right);

        assert_eq!(
            plain(&table_style.view(&sample())),
            "\
┌──────┬──────────┐
│ Name │ Location │
├──────┼──────────┤
│ Kini │ New York │
│ Iris │    Paris │
└──────┴──────────┘"
        );
    }

    #[test]
    fn vertical_alignment_places_short_cells_within_a_tall_row() {
        fn bottom(_: TableCell<'_>) -> Option<BlockStyle> {
            Some(BlockStyle::new().align_vertical(VerticalAlign::Bottom))
        }

        let table = Table::new().row(["a", "one\ntwo"]);
        let table_style = styles().table().clone().style_func(bottom);

        assert_eq!(
            plain(&table_style.view(&table)),
            "\
┌───┬─────┐
│   │ one │
│ a │ two │
└───┴─────┘"
        );
    }

    #[test]
    fn padding_widens_every_cell() {
        let table_style = styles().table().clone().padding(0);

        assert_eq!(
            plain(&table_style.view(&sample())),
            "\
┌────┬────────┐
│Name│Location│
├────┼────────┤
│Kini│New York│
│Iris│Paris   │
└────┴────────┘"
        );
    }

    #[test]
    fn table_data_is_inspectable_without_a_style() {
        let table = sample().hidden(true).offset(1, 0);

        assert_eq!(table.column_count(), 2);
        assert!(table.is_hidden());

        let ragged = Table::new().headers(["A"]).row(["1", "2", "3"]);
        assert_eq!(ragged.column_count(), 3);
    }

    #[test]
    fn the_hook_sees_each_cell_its_row_and_its_text() {
        fn shout(cell: TableCell<'_>) -> Option<BlockStyle> {
            let expected = match (cell.is_header(), cell.column()) {
                (true, 0) => "Name",
                (true, 1) => "Location",
                (false, 0) => "Kini",
                _ => "New York",
            };
            assert_eq!(cell.text(), expected, "column {}", cell.column());
            assert_eq!(
                cell.is_header(),
                cell.row().is_none(),
                "a header cell has no body-row number"
            );
            None
        }

        let table = Table::new()
            .headers(["Name", "Location"])
            .row(["Kini", "New York"]);
        let _ = styles().table().clone().style_func(shout).view(&table);
    }

    #[test]
    fn role_setters_replace_the_styles_they_name() {
        let underline = BlockStyle::new().underline();
        let table_style = styles()
            .table()
            .clone()
            .header_style(underline.clone())
            .cell_style(underline.clone())
            .border_style(TextStyle::new().underline());

        for role in [TableRole::Header, TableRole::Cell] {
            assert_eq!(table_style.style(role), &underline);
        }
        assert_eq!(
            table_style.border_glyph_style(),
            &TextStyle::new().underline()
        );
    }

    /// A table whose natural width is 41 cells: the case #22 reproduced.
    fn wide() -> Table {
        Table::new()
            .headers(["N", "Description"])
            .row(["a", "the quick brown fox jumps again!!"])
            .row(["b", "over the lazy dog"])
    }

    #[test]
    fn an_area_narrower_than_the_table_shrinks_its_columns_and_its_lines() {
        let view = styles().table().view(&wide());
        assert_eq!(measure(&view).width(), 41, "the natural width");

        let rows: Vec<String> = resolve(&view, Available::columns(18))
            .rows()
            .iter()
            .map(|row| row.iter().map(StyledGrapheme::symbol).collect())
            .collect();

        for row in &rows {
            assert_eq!(
                PrintableText::new(row).width(),
                18,
                "every row fits the area: {row}"
            );
        }
        // The lines and the cells were decided together, so the verticals of
        // a rule land on the column boundaries of the rows around it.
        let boundaries = |row: &str| -> Vec<usize> {
            PrintableText::new(row)
                .as_str()
                .chars()
                .enumerate()
                .filter(|(_, glyph)| "│┼┬┴├┤┌┐└┘".contains(*glyph))
                .map(|(index, _)| index)
                .collect()
        };
        let expected = boundaries(&rows[0]);
        for row in &rows {
            assert_eq!(boundaries(row), expected, "columns line up: {row}");
        }
    }

    #[test]
    fn an_area_wider_than_the_table_does_not_widen_it() {
        let view = styles().table().view(&wide());

        let rows: Vec<String> = resolve(&view, Available::columns(200))
            .rows()
            .iter()
            .map(|row| row.iter().map(StyledGrapheme::symbol).collect())
            .collect();

        for row in &rows {
            assert_eq!(
                PrintableText::new(row).width(),
                41,
                "an area only caps a table; it never pads one out: {row}"
            );
        }
    }

    #[test]
    fn table_styles_compare_by_policy_and_hook() {
        let table_style = styles().table().clone();
        assert_eq!(table_style, styles().table().clone());
        assert_ne!(
            table_style.clone(),
            table_style.clone().style_func(accent_header)
        );
        assert_ne!(table_style.clone(), table_style.border(Border::ASCII));
    }
}
