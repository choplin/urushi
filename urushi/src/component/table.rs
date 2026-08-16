//! Renderer-neutral tables with an independent public model.

use crate::text::{PrintableLines, PrintableText, wrap_text};
use crate::{
    Align, BlockStyle, BlockStylePropertyKey, Border, TableRole, TextStyle, VerticalAlign, View,
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
/// A cell is a block, so the hook returns a [`BlockStyle`]. Only its colors,
/// text modifiers, and alignment are used; the geometry — padding, dimensions,
/// border, and margin — is supplied by the table, which owns layout.
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
    padding: usize,
    width: Option<usize>,
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
        self.padding = padding as usize;
        self
    }

    /// Constrains the total composed width, in terminal cells.
    ///
    /// Columns are widened evenly when the natural table is narrower, and the
    /// widest column is narrowed — wrapping its content — when it is wider. A
    /// column never shrinks below one content cell plus its padding, so a
    /// constraint narrower than that minimum still overflows.
    ///
    /// Content that still does not fit its column is truncated to the column's
    /// cells, so every composed line stays the same width. A column one cell
    /// wide therefore drops a two-cell character entirely rather than letting
    /// it break the frame.
    ///
    /// Surplus width levels the columns toward equal width rather than scaling
    /// them in proportion to their content, so a constraint much wider than the
    /// content can leave a short column wider than a long one.
    ///
    /// The width is a `u16` for the same reason [`BlockStyle::width`] is: a
    /// terminal dimension, not an arbitrary count.
    #[must_use]
    pub const fn width(mut self, width: u16) -> Self {
        self.width = Some(width as usize);
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
    /// A cell style contributes its colors, text modifiers, and alignment. Its
    /// geometry — border, padding, width, and height — is replaced by the
    /// table's, because the table owns layout.
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
        let widths = self.column_widths(table, columns, rows);
        self.compose(table, &widths, rows, has_headers)
    }

    /// Composes the visible rows into a view.
    ///
    /// Row selection happens before this point, so a future vertical viewport
    /// only has to narrow `rows` without changing composition.
    fn compose(
        &self,
        table: &Table,
        widths: &[usize],
        rows: &[Vec<String>],
        has_headers: bool,
    ) -> View {
        let mut lines = Vec::new();

        if self.border_top {
            lines.push(self.rule(
                widths,
                self.border.top_left,
                self.border.top,
                self.border.middle_top,
                self.border.top_right,
            ));
        }

        if has_headers {
            lines.push(self.content_row(&table.headers, widths, TableRow::Header));
            if self.border_header && !rows.is_empty() {
                lines.push(self.separator(widths));
            }
        }

        for (index, row) in rows.iter().enumerate() {
            lines.push(self.content_row(row, widths, TableRow::Body(index)));
            if self.border_row && index + 1 < rows.len() {
                lines.push(self.separator(widths));
            }
        }

        if self.border_bottom {
            lines.push(self.rule(
                widths,
                self.border.bottom_left,
                self.border.bottom,
                self.border.middle_bottom,
                self.border.bottom_right,
            ));
        }

        View::column(Align::Left, lines)
    }

    /// Builds one horizontal rule from its corner, fill, and junction glyphs.
    fn rule(&self, widths: &[usize], left: char, fill: char, junction: char, right: char) -> View {
        let mut text = String::new();
        if self.border_left {
            text.push(left);
        }
        for (index, width) in widths.iter().enumerate() {
            for _ in 0..*width {
                text.push(fill);
            }
            if self.border_column && index + 1 < widths.len() {
                text.push(junction);
            }
        }
        if self.border_right {
            text.push(right);
        }
        View::text(text, self.border_style.clone())
    }

    /// Builds the rule drawn between two rows.
    fn separator(&self, widths: &[usize]) -> View {
        self.rule(
            widths,
            self.border.middle_left,
            self.border.middle_horizontal,
            self.border.middle,
            self.border.middle_right,
        )
    }

    /// Lays one row of cells out as a row of blocks.
    ///
    /// Every cell is a block: the table supplies the column width, the padding,
    /// and the row height, and the cell style supplies the alignment applied
    /// inside them. The table implements no alignment of its own.
    fn content_row(&self, cells: &[String], widths: &[usize], row: TableRow) -> View {
        let blocks: Vec<(BlockStyle, Vec<String>)> = widths
            .iter()
            .enumerate()
            .map(|(column, width)| {
                let text = cells.get(column).map_or("", String::as_str);
                let style = self.cell_style_at(TableCell { text, row, column });
                (style, self.cell_lines(text, *width))
            })
            .collect();
        let height = blocks
            .iter()
            .map(|(_, lines)| lines.len())
            .max()
            .unwrap_or(1);

        let mut children = Vec::with_capacity(blocks.len() * 2 + 2);
        if self.border_left {
            children.push(self.vertical_rule(self.border.left, height));
        }
        for (column, ((style, lines), width)) in blocks.into_iter().zip(widths).enumerate() {
            children.push(self.cell_block(style, lines, *width, height));
            if self.border_column && column + 1 < widths.len() {
                children.push(self.vertical_rule(self.border.left, height));
            }
        }
        if self.border_right {
            children.push(self.vertical_rule(self.border.right, height));
        }

        View::row(VerticalAlign::Top, children)
    }

    /// Builds one column rule, repeated over the height of its row.
    fn vertical_rule(&self, glyph: char, height: usize) -> View {
        let text = std::iter::repeat_n(glyph.to_string(), height)
            .collect::<Vec<_>>()
            .join("\n");
        View::text(text, self.border_style.clone())
    }

    /// Fits one cell's text to its column, wrapping and then clamping.
    ///
    /// `wrap_text` still emits a grapheme wider than the requested width when
    /// that grapheme starts the line, so a wide character in a narrow column
    /// would otherwise push the row past the frame. Clamping here keeps every
    /// composed line the same width; placing the fitted text is the block's job.
    fn cell_lines(&self, text: &str, width: usize) -> Vec<String> {
        let inner = self.content_width(width);
        wrap_text(PrintableLines::new(text), inner)
            .iter()
            .map(|line| PrintableText::new(line).truncate(inner).as_str().to_owned())
            .collect()
    }

    /// Wraps one cell's fitted lines in the block that positions them.
    fn cell_block(
        &self,
        style: BlockStyle,
        lines: Vec<String>,
        width: usize,
        height: usize,
    ) -> View {
        let mut block = style
            .remove(BlockStylePropertyKey::Border)
            .remove(BlockStylePropertyKey::Margin)
            .remove(BlockStylePropertyKey::MaxWidth)
            .remove(BlockStylePropertyKey::MaxHeight)
            .padding((0, self.padding as u16))
            .height(height as u16);
        if self.content_width(width) > 0 {
            block = block.width(width as u16);
        }
        let text = View::text(lines.join("\n"), block.text().clone());
        View::block(block, text)
    }

    /// Resolves one cell's style from the hook, falling back to its role.
    fn cell_style_at(&self, cell: TableCell<'_>) -> BlockStyle {
        (self.style_func)(cell).unwrap_or_else(|| match cell.row {
            TableRow::Header => self.header.clone(),
            _ => self.cell.clone(),
        })
    }

    /// Returns the content cells available inside one column.
    const fn content_width(&self, width: usize) -> usize {
        width.saturating_sub(self.padding * 2)
    }

    /// Returns the narrowest a column may become.
    const fn minimum_width(&self) -> usize {
        self.padding * 2 + 1
    }

    /// Computes each column's total width, honoring the width constraint.
    fn column_widths(&self, table: &Table, columns: usize, rows: &[Vec<String>]) -> Vec<usize> {
        let mut widths: Vec<usize> = (0..columns)
            .map(|column| {
                let header = table.headers.get(column);
                let natural = rows
                    .iter()
                    .filter_map(|row| row.get(column))
                    .chain(header)
                    .map(|cell| natural_width(cell))
                    .max()
                    .unwrap_or_default();
                natural + self.padding * 2
            })
            .collect();

        let Some(target) = self.width else {
            return widths;
        };

        let frame = usize::from(self.border_left)
            + usize::from(self.border_right)
            + if self.border_column { columns - 1 } else { 0 };
        let mut total = widths.iter().sum::<usize>() + frame;

        while total < target {
            let Some(index) = index_of_min(&widths) else {
                break;
            };
            widths[index] += 1;
            total += 1;
        }
        while total > target {
            let Some(index) = index_of_max(&widths, self.minimum_width()) else {
                break;
            };
            widths[index] -= 1;
            total -= 1;
        }

        widths
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

/// Returns the widest line of a possibly multi-line cell.
fn natural_width(cell: &str) -> usize {
    cell.lines()
        .map(|line| PrintableText::new(line).width())
        .max()
        .unwrap_or_default()
}

/// Returns the index of the narrowest column, preferring the leftmost.
fn index_of_min(widths: &[usize]) -> Option<usize> {
    widths
        .iter()
        .enumerate()
        .min_by_key(|(index, width)| (**width, *index))
        .map(|(index, _)| index)
}

/// Returns the index of the widest column that may still shrink.
fn index_of_max(widths: &[usize], minimum: usize) -> Option<usize> {
    widths
        .iter()
        .enumerate()
        .filter(|(_, width)| **width > minimum)
        .max_by_key(|(index, width)| (**width, std::cmp::Reverse(*index)))
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{plain_exact as plain, plain_rows, style_at};
    use crate::{Color, ComponentStyles, SemanticTokens, measure};

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
    fn a_width_constraint_keeps_cjk_rows_rectangular() {
        // A column narrower than one wide grapheme is the case `wrap_text`
        // alone does not handle: it emits the grapheme anyway rather than
        // splitting it. The composed row must still match the frame.
        let table = Table::new().headers(["A", "B"]).row(["日本語", "x"]);
        let narrow = styles().table().clone().width(9);
        let view = narrow.view(&table);

        for row in plain_rows(&view) {
            assert_eq!(PrintableText::new(&row).width(), 9);
        }
        assert_eq!(
            plain_rows(&view)[3],
            "│   │ x │",
            "a cell too narrow for its content drops it rather than overflowing"
        );

        let padded = styles()
            .table()
            .clone()
            .padding(0)
            .width(5)
            .view(&Table::new().row(["日本", "ab"]));
        for row in plain_rows(&padded) {
            assert_eq!(PrintableText::new(&row).width(), 5);
        }
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
    fn a_hook_style_box_model_does_not_change_layout() {
        fn boxed(_: TableCell<'_>) -> Option<BlockStyle> {
            Some(
                BlockStyle::new()
                    .padding((0, 3))
                    .width(30)
                    .height(4)
                    .border(Border::DOUBLE),
            )
        }

        let plain_default = plain(&styles().table().view(&sample()));
        let table_style = styles().table().clone().style_func(boxed);

        assert_eq!(
            plain(&table_style.view(&sample())),
            plain_default,
            "the table owns the box model; a cell style's box properties are ignored"
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
