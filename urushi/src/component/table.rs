//! Renderer-neutral tables with an independent public model.

use std::{fmt, sync::Arc};

use crate::text::wrapped_line_count;
use crate::view::{CanvasMeasure, CanvasRequirements, Claim, Kind, distribute, fit_text_lines};
use crate::{
    Align, BlockStyle, Canvas, CanvasContext, CanvasItem, CanvasSizing, Composition, Grapheme,
    Length, LineGlyphs, LineNetwork, Overflow, Position, PrintableLines, PrintableText, Sides,
    TableRole, TextStyle, VerticalAlign, View,
};

/// Which row of a bound table frame a cell belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableRowKind {
    Header,
    Body(usize),
}

/// Identifies one cell while a [`TableRowPresentation`] resolves its style.
///
/// Header styles are resolved by [`TablePresentation::header_styles`], so this
/// context always identifies a body cell. The row number counts visible body
/// rows after offsets are applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCell<'a> {
    text: &'a str,
    row: usize,
    column: usize,
}

impl<'a> TableCell<'a> {
    /// Returns the zero-based visible body-row number.
    pub const fn row(&self) -> usize {
        self.row
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

/// The visible position of one typed body row during composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRowPosition {
    index: usize,
    len: usize,
}

impl TableRowPosition {
    /// Returns this row's zero-based index after offsets are applied.
    pub const fn index(self) -> usize {
        self.index
    }

    /// Returns the number of visible body rows.
    pub const fn len(self) -> usize {
        self.len
    }

    /// Returns whether there are no visible body rows.
    ///
    /// A formatter is only called for an existing row, so this is always
    /// `false` for positions supplied by [`TableRowPresentation`].
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// A short-lived destination for the formatted cells of one row.
#[derive(Debug, Default)]
pub struct TableRowCells {
    values: Vec<String>,
}

impl TableRowCells {
    /// Appends one value using its canonical [`fmt::Display`] representation.
    pub fn display(&mut self, value: impl fmt::Display) {
        self.values.push(value.to_string());
    }

    /// Appends already formatted plain text.
    pub fn text(&mut self, text: impl Into<String>) {
        self.values.push(text.into());
    }
}

/// Canonically formats one typed value as a table row.
///
/// Derive this trait with the [`TableRow` derive macro](macro@crate::TableRow)
/// for the ordinary case.
/// Non-skipped fields are formatted through [`fmt::Display`] in declaration
/// order. Headers remain explicit Table data.
pub trait TableRow {
    /// Writes this row's cells in column order.
    fn write_cells(&self, cells: &mut TableRowCells);
}

/// One normalized row used by [`Table::text`] for direct plain-text input.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextTableRow(Vec<String>);

impl<I, S> From<I> for TextTableRow
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    fn from(values: I) -> Self {
        Self(values.into_iter().map(Into::into).collect())
    }
}

impl TableRow for TextTableRow {
    fn write_cells(&self, cells: &mut TableRowCells) {
        cells.values.extend(self.0.iter().cloned());
    }
}

type RowFormatter<'a, T> = dyn Fn(&T, TableRowPosition, &mut TableRowCells) + 'a;
type RowCellStyler<'a, T> = dyn Fn(&T, TableCell<'_>, &BlockStyle) -> Option<BlockStyle> + 'a;

/// Typed row formatting and optional complete cell-style replacements.
#[derive(Clone)]
pub struct TableRowPresentation<'a, T> {
    format: Arc<RowFormatter<'a, T>>,
    style: Option<Arc<RowCellStyler<'a, T>>>,
}

impl<T> fmt::Debug for TableRowPresentation<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TableRowPresentation { .. }")
    }
}

impl<'a, T: 'a> TableRowPresentation<'a, T> {
    /// Creates a custom typed row formatter with no cell overrides.
    pub fn new<F>(format: F) -> Self
    where
        F: Fn(&T, TableRowPosition, &mut TableRowCells) + 'a,
    {
        Self {
            format: Arc::new(format),
            style: None,
        }
    }

    /// Replaces the optional typed cell-style policy.
    ///
    /// The callback receives the complete column or table fallback style.
    /// `None` keeps that style; `Some` is a complete replacement.
    #[must_use]
    pub fn cell_style<S>(mut self, style: S) -> Self
    where
        S: Fn(&T, TableCell<'_>, &BlockStyle) -> Option<BlockStyle> + 'a,
    {
        self.style = Some(Arc::new(style));
        self
    }
}

impl<'a, T> TableRowPresentation<'a, T>
where
    T: TableRow + 'a,
{
    /// Uses the row's canonical [`TableRow`] formatting implementation.
    pub fn display() -> Self {
        Self::new(|row: &T, _, cells| row.write_cells(cells))
    }
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
/// [`TablePresentation`].
///
/// Rows may be ragged: a row shorter than the widest row is padded with empty
/// cells when its [`TableRow`] formatting is composed. Use [`Table::text`] for
/// rows that are already plain text.
///
/// ```
/// use urushi::{BlockStyle, Table, TablePresentation, TextStyle, measure};
///
/// let table = Table::text()
///     .headers(["Name", "Location"])
///     .row(["Kini", "New York"])
///     .row(["Iris", "Paris"]);
///
/// let presentation =
///     TablePresentation::new(BlockStyle::new(), BlockStyle::new(), TextStyle::new());
/// assert_eq!(measure(&presentation.compose(&table)).height(), 6);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table<Row = TextTableRow> {
    headers: Vec<String>,
    rows: Vec<Row>,
    hidden: bool,
    offset: RowOffset,
}

impl<Row> Default for Table<Row> {
    fn default() -> Self {
        Self {
            headers: Vec::new(),
            rows: Vec::new(),
            hidden: false,
            offset: RowOffset::default(),
        }
    }
}

impl<Row> Table<Row> {
    /// Creates an empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the header cells, each of which is plain text.
    ///
    /// A table without headers composes its body rows only. Escape sequences
    /// and cursor movement in a header break the plain-text contract:
    /// debug builds panic, and release builds measure them as ordinary
    /// characters and may split them when wrapping or truncating. Raw ANSI is not accepted as component text.
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
    pub fn row(mut self, row: Row) -> Self {
        self.rows.push(row);
        self
    }

    /// Appends typed body rows in iteration order.
    #[must_use]
    pub fn rows<I>(mut self, rows: I) -> Self
    where
        I: IntoIterator<Item = Row>,
    {
        self.rows.extend(rows);
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

    fn visible_rows(&self) -> &[Row] {
        let end = self.rows.len().saturating_sub(self.offset.end);
        if self.offset.start >= end {
            return &[];
        }
        &self.rows[self.offset.start..end]
    }
}

impl<Row> Table<Row>
where
    Row: TableRow,
{
    /// Returns the column count from the widest canonical header or body row.
    ///
    /// This formats every owned row, including rows omitted by the offset.
    /// Composition formats only visible rows, so an omitted wide row does not
    /// add columns to the composed view.
    pub fn column_count(&self) -> usize {
        self.rows
            .iter()
            .map(|row| {
                let mut cells = TableRowCells::default();
                row.write_cells(&mut cells);
                cells.values.len()
            })
            .chain(std::iter::once(self.headers.len()))
            .max()
            .unwrap_or_default()
    }
}

impl Table<TextTableRow> {
    /// Creates an empty table that accepts rows of direct plain text.
    pub fn text() -> TextTable {
        TextTable { table: Self::new() }
    }
}

/// A direct-text Table builder that normalizes every row into owned strings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextTable {
    table: Table<TextTableRow>,
}

impl TextTable {
    /// Replaces the plain-text header cells.
    #[must_use]
    pub fn headers<I, S>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.table = self.table.headers(headers);
        self
    }

    /// Appends one direct-text body row.
    #[must_use]
    pub fn row<I, S>(mut self, row: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.table.rows.push(TextTableRow::from(row));
        self
    }

    /// Appends direct-text body rows in iteration order.
    #[must_use]
    pub fn rows<I, R, S>(mut self, rows: I) -> Self
    where
        I: IntoIterator<Item = R>,
        R: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.table
            .rows
            .extend(rows.into_iter().map(TextTableRow::from));
        self
    }

    /// Includes or excludes the complete table.
    #[must_use]
    pub fn hidden(mut self, hidden: bool) -> Self {
        self.table = self.table.hidden(hidden);
        self
    }

    /// Omits body rows from the front and back.
    #[must_use]
    pub fn offset(mut self, start: usize, end: usize) -> Self {
        self.table = self.table.offset(start, end);
        self
    }

    /// Returns the widest owned row, including headers.
    pub fn column_count(&self) -> usize {
        self.table.column_count()
    }
}

impl std::ops::Deref for TextTable {
    type Target = Table<TextTableRow>;

    fn deref(&self) -> &Self::Target {
        &self.table
    }
}

/// Line model used by a [`TablePresentation`].
///
/// Connected grids use a [`LineGlyphs`] repertoire so [`LineNetwork`] can
/// derive every corner, tee, and crossing from the recorded geometry.
/// Markdown and booktabs tables contain only independent straight rules and
/// therefore use Canvas cell lines instead.
///
/// ```
/// use urushi::{
///     BlockStyle, LineGlyphs, Table, TableBorder, TablePresentation, TextStyle,
/// };
///
/// let table = Table::text().headers(["Name"]).row(["Iris"]);
/// let presentation = TablePresentation::new(
///     BlockStyle::new(),
///     BlockStyle::new(),
///     TextStyle::new(),
/// )
/// .border(TableBorder::network(LineGlyphs::ROUNDED));
/// let _view = presentation.compose(&table);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableBorder(TableBorderKind);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableBorderKind {
    Network(LineGlyphs),
    Markdown,
    Booktabs,
}

impl TableBorder {
    /// A square single-line grid.
    pub const NORMAL: Self = Self::network(LineGlyphs::NORMAL);
    /// A single-line grid with rounded outer corners.
    pub const ROUNDED: Self = Self::network(LineGlyphs::ROUNDED);
    /// A heavy-line grid.
    pub const THICK: Self = Self::network(LineGlyphs::THICK);
    /// A double-line grid.
    pub const DOUBLE: Self = Self::network(LineGlyphs::DOUBLE);
    /// An ASCII-only grid.
    pub const ASCII: Self = Self::network(LineGlyphs::ASCII);
    /// A Markdown table made from `-` and `|` cell lines.
    pub const MARKDOWN: Self = Self(TableBorderKind::Markdown);
    /// A booktabs table with heavy outer rules and light inner rules.
    pub const BOOKTABS: Self = Self(TableBorderKind::Booktabs);
    /// An invisible grid that still occupies its rule rows and columns.
    pub const HIDDEN: Self = Self::network(LineGlyphs::HIDDEN);

    /// Creates a connected grid with a caller-owned glyph repertoire.
    pub const fn network(glyphs: LineGlyphs) -> Self {
        Self(TableBorderKind::Network(glyphs))
    }
}

/// Presentation policy used to compose a [`Table`] into a [`View`].
///
/// The policy owns the border, the edge toggles, cell padding, the total width
/// constraint, and the per-cell styling strategy. It borrows the data and
/// performs no terminal output.
#[derive(Debug, Clone)]
pub struct TablePresentation {
    header: BlockStyle,
    cell: BlockStyle,
    header_styles: Vec<Option<BlockStyle>>,
    column_styles: Vec<Option<BlockStyle>>,
    border_style: TextStyle,
    border: TableBorder,
    border_top: bool,
    border_bottom: bool,
    border_left: bool,
    border_right: bool,
    border_header: bool,
    border_column: bool,
    border_row: bool,
    padding: u16,
    width: Option<u16>,
}

impl PartialEq for TablePresentation {
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header
            && self.cell == other.cell
            && self.header_styles == other.header_styles
            && self.column_styles == other.column_styles
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
    }
}

impl TablePresentation {
    /// Creates a table presentation with a single-line border and one cell of padding.
    pub fn new(header: BlockStyle, cell: BlockStyle, border: TextStyle) -> Self {
        Self {
            header,
            cell,
            header_styles: Vec::new(),
            column_styles: Vec::new(),
            border_style: border,
            border: TableBorder::NORMAL,
            border_top: true,
            border_bottom: true,
            border_left: true,
            border_right: true,
            border_header: true,
            border_column: true,
            border_row: false,
            padding: 1,
            width: None,
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

    /// Replaces positional complete header-cell styles.
    ///
    /// A missing or `None` entry keeps the table header style. Extra entries
    /// do not create columns.
    #[must_use]
    pub fn header_styles<I>(mut self, styles: I) -> Self
    where
        I: IntoIterator<Item = Option<BlockStyle>>,
    {
        self.header_styles = styles.into_iter().collect();
        self
    }

    /// Replaces positional complete body-column styles.
    ///
    /// A missing or `None` entry keeps the table cell style. Extra entries do
    /// not create columns.
    #[must_use]
    pub fn column_styles<I>(mut self, styles: I) -> Self
    where
        I: IntoIterator<Item = Option<BlockStyle>>,
    {
        self.column_styles = styles.into_iter().collect();
        self
    }

    /// Replaces the style drawn on the table's rules.
    #[must_use]
    pub fn border_style(mut self, style: TextStyle) -> Self {
        self.border_style = style;
        self
    }

    /// Replaces the table's line model and glyph repertoire.
    #[must_use]
    pub const fn border(mut self, border: TableBorder) -> Self {
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
    /// A connected table uses its [`LineGlyphs::vertical`] glyph for outer and
    /// inner vertical rules. A straight-rule preset supplies its own vertical
    /// marker.
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
    /// [`compose`](Self::compose) puts its Canvas in a borderless block of
    /// exactly this width. The bound table frame then gives every column an
    /// equal share, which is why a
    /// width much wider than the content can leave a short column wider than a
    /// long one, and a width narrower than the content narrows every column
    /// rather than only the widest.
    ///
    /// Nothing is measured while composing. The bound frame decides columns
    /// and the lines between them together when the view resolves, so they cannot
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

    /// Composes table data into an intrinsically sized, renderer-neutral Canvas.
    ///
    /// Composition binds an owned snapshot without receiving an available
    /// area. During resolution, the bound item first supplies width and
    /// width-dependent height requirements, then records cell and rule
    /// commands after the final Canvas size is known.
    pub fn compose<T>(&self, table: &Table<T>) -> View
    where
        T: TableRow,
    {
        self.compose_with(table, &TableRowPresentation::display())
    }

    /// Composes typed table data with custom row formatting and cell styles.
    pub fn compose_with<T>(&self, table: &Table<T>, rows: &TableRowPresentation<'_, T>) -> View {
        if table.is_hidden() {
            return View::empty();
        }

        let visible_rows = table.visible_rows();
        let mut formatted = Vec::with_capacity(visible_rows.len());
        for (index, row) in visible_rows.iter().enumerate() {
            let mut cells = TableRowCells::default();
            (rows.format)(
                row,
                TableRowPosition {
                    index,
                    len: visible_rows.len(),
                },
                &mut cells,
            );
            formatted.push(cells.values);
        }
        // Only the visible rows may widen the composed table; a wide row the
        // offset excluded must not leave empty columns behind.
        let columns = column_count(&formatted, table.headers.len());
        if columns == 0 {
            return View::empty();
        }

        let item = TableItem(Arc::new(TableFrame::new(
            table,
            self,
            visible_rows,
            &formatted,
            rows.style.as_deref(),
            columns,
        )));
        let canvas = Canvas::new().sizing(item.sizing()).item(item);
        let frame = self.width.map_or_else(BlockStyle::new, |width| {
            BlockStyle::new().width(Length::Cells(width))
        });
        View::block(frame, View::canvas(canvas))
    }

    fn fallback_style(&self, kind: TableRowKind, column: usize) -> &BlockStyle {
        let styles = match kind {
            TableRowKind::Header => &self.header_styles,
            TableRowKind::Body(_) => &self.column_styles,
        };
        styles
            .get(column)
            .and_then(Option::as_ref)
            .unwrap_or(match kind {
                TableRowKind::Header => &self.header,
                TableRowKind::Body(_) => &self.cell,
            })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TableFrame {
    presentation: TablePresentation,
    rows: Vec<TableFrameRow>,
    columns: usize,
}

impl TableFrame {
    fn new<T>(
        table: &Table<T>,
        presentation: &TablePresentation,
        visible_rows: &[T],
        formatted_rows: &[Vec<String>],
        cell_style: Option<&RowCellStyler<'_, T>>,
        columns: usize,
    ) -> Self {
        let mut rows =
            Vec::with_capacity(visible_rows.len() + usize::from(!table.headers.is_empty()));
        if !table.headers.is_empty() {
            rows.push(TableFrameRow::new(
                &table.headers,
                columns,
                TableRowKind::Header,
                presentation,
                |_, _| None,
            ));
        }
        for (index, (row, formatted)) in visible_rows.iter().zip(formatted_rows).enumerate() {
            rows.push(TableFrameRow::new(
                formatted,
                columns,
                TableRowKind::Body(index),
                presentation,
                |cell, fallback| cell_style.and_then(|style| style(row, cell, fallback)),
            ));
        }
        let mut presentation = presentation.clone();
        presentation.header = frame_cell_style(presentation.header, presentation.padding);
        presentation.cell = frame_cell_style(presentation.cell, presentation.padding);
        for style in presentation
            .header_styles
            .iter_mut()
            .chain(&mut presentation.column_styles)
            .flatten()
        {
            *style = frame_cell_style(style.clone(), presentation.padding);
        }
        Self {
            presentation,
            rows,
            columns,
        }
    }

    fn line_width(&self) -> usize {
        usize::from(self.presentation.border_left)
            + usize::from(self.presentation.border_right)
            + usize::from(self.presentation.border_column) * self.columns.saturating_sub(1)
    }

    fn line_height(&self) -> usize {
        usize::from(self.presentation.border_top)
            + usize::from(self.presentation.border_bottom)
            + (0..self.rows.len().saturating_sub(1))
                .filter(|gap| self.draws_row_rule(*gap))
                .count()
    }

    fn draws_row_rule(&self, gap: usize) -> bool {
        match self.rows.get(gap).map(|row| row.kind) {
            Some(TableRowKind::Header) => self.presentation.border_header,
            _ => self.presentation.border_row,
        }
    }

    fn column_requirements(&self) -> Vec<(usize, usize)> {
        (0..self.columns)
            .map(|column| {
                self.rows
                    .iter()
                    .map(|row| {
                        let cell = &row.cells[column];
                        cell.width_requirements(cell.style(&self.presentation, row.kind, column))
                    })
                    .fold((0, 0), |(demand, floor), next| {
                        (demand.max(next.0), floor.max(next.1))
                    })
            })
            .collect()
    }

    fn column_widths(&self, width: usize) -> Vec<usize> {
        let requirements = self.column_requirements();
        let kind = if self.presentation.width.is_some() {
            Kind::fill(1)
        } else {
            Kind::Auto
        };
        let claims: Vec<Claim> = requirements
            .iter()
            .map(|(demand, floor)| Claim {
                kind,
                demand: *demand,
                floor: *floor,
            })
            .collect();
        distribute(width.saturating_sub(self.line_width()), &claims)
    }

    fn row_heights(&self, columns: &[usize]) -> Vec<usize> {
        self.rows
            .iter()
            .map(|row| {
                row.cells
                    .iter()
                    .enumerate()
                    .zip(columns)
                    .map(|((column, cell), width)| {
                        cell.height_at(*width, cell.style(&self.presentation, row.kind, column))
                    })
                    .max()
                    .unwrap_or(0)
            })
            .collect()
    }

    fn height_at(&self, width: usize) -> usize {
        self.row_heights(&self.column_widths(width))
            .iter()
            .sum::<usize>()
            .saturating_add(self.line_height())
    }

    fn draw(&self, context: &mut CanvasContext) {
        let columns = self.column_widths(context.size().width());
        let heights = self.row_heights(&columns);

        self.record_cell_rows(context, &columns, &heights);
        match self.presentation.border.0 {
            TableBorderKind::Network(glyphs) => {
                self.record_line_network(context, &columns, &heights, glyphs);
            }
            TableBorderKind::Markdown => {
                self.record_straight_rules(context, &columns, &heights, '|', '-', '-', '-', true);
            }
            TableBorderKind::Booktabs => {
                self.record_straight_rules(context, &columns, &heights, ' ', '━', '─', '━', false);
            }
        }
    }

    fn record_cell_rows(&self, context: &mut CanvasContext, columns: &[usize], heights: &[usize]) {
        let mut y = usize::from(self.presentation.border_top);
        for (row_index, (row, height)) in self.rows.iter().zip(heights).enumerate() {
            let rendered = row
                .cells
                .iter()
                .zip(columns)
                .enumerate()
                .map(|(column, (cell, width))| {
                    cell.rendered_rows(
                        *width,
                        *height,
                        cell.style(&self.presentation, row.kind, column),
                    )
                })
                .collect::<Vec<_>>();
            for line in 0..*height {
                let mut runs = Vec::<TextRun>::new();
                let mut x = 0;
                if self.presentation.border_left {
                    push_run(&mut runs, x, " ", TextStyle::new());
                    x += 1;
                }
                for (column, ((cell, rows), width)) in
                    row.cells.iter().zip(&rendered).zip(columns).enumerate()
                {
                    let style = cell.style(&self.presentation, row.kind, column);
                    push_run(&mut runs, x, &rows[line], style.text().clone());
                    x = x.saturating_add(*width);
                    if column + 1 < columns.len() && self.presentation.border_column {
                        push_run(&mut runs, x, " ", TextStyle::new());
                        x += 1;
                    }
                }
                if self.presentation.border_right {
                    push_run(&mut runs, x, " ", TextStyle::new());
                }
                for run in runs {
                    context.text_with(
                        Position::new(position(run.x), position(y + line)),
                        run.text,
                        run.style,
                        Composition::Replace,
                    );
                }
            }
            y = y.saturating_add(*height);
            if row_index + 1 < self.rows.len() && self.draws_row_rule(row_index) {
                y += 1;
            }
        }
    }

    fn rule_columns(&self, columns: &[usize]) -> Vec<usize> {
        let mut rules = Vec::with_capacity(columns.len() + 1);
        let mut x = 0usize;
        if self.presentation.border_left {
            rules.push(x);
            x += 1;
        }
        for (index, width) in columns.iter().enumerate() {
            if index > 0 && self.presentation.border_column {
                rules.push(x);
                x += 1;
            }
            x = x.saturating_add(*width);
        }
        if self.presentation.border_right {
            rules.push(x);
        }
        rules
    }

    fn rule_rows(&self, heights: &[usize]) -> Vec<usize> {
        let mut rules = Vec::with_capacity(self.rows.len() + 1);
        let mut y = 0usize;
        if self.presentation.border_top {
            rules.push(y);
            y += 1;
        }
        for (row_index, height) in heights.iter().enumerate() {
            y = y.saturating_add(*height);
            if row_index + 1 < heights.len() && self.draws_row_rule(row_index) {
                rules.push(y);
                y += 1;
            }
        }
        if self.presentation.border_bottom {
            rules.push(y);
        }
        rules
    }

    fn record_line_network(
        &self,
        context: &mut CanvasContext,
        columns: &[usize],
        heights: &[usize],
        glyphs: LineGlyphs,
    ) {
        let width = columns
            .iter()
            .sum::<usize>()
            .saturating_add(self.line_width());
        let height = heights
            .iter()
            .sum::<usize>()
            .saturating_add(self.line_height());
        if width == 0 || height == 0 {
            return;
        }

        let rows = self.rule_rows(heights);
        let columns = self.rule_columns(columns);
        if rows.is_empty() && columns.is_empty() {
            return;
        }

        let mut network = LineNetwork::new(glyphs, self.rule_style());
        // A disabled outer edge removes its rule cell, not the logical
        // continuation of a perpendicular rule. Keep that incidence just
        // outside the Canvas so clipping preserves straight lines and the
        // corners next to zero-width cells.
        let first_x = if self.presentation.border_left { 0 } else { -1 };
        let last_x = if self.presentation.border_right {
            position(width - 1)
        } else {
            position(width)
        };
        let first_y = if self.presentation.border_top { 0 } else { -1 };
        let last_y = if self.presentation.border_bottom {
            position(height - 1)
        } else {
            position(height)
        };
        for y in rows {
            network.horizontal(position(y), first_x..=last_x);
        }
        for x in columns {
            network.vertical(position(x), first_y..=last_y);
        }
        context.line_network(network);
    }

    #[allow(clippy::too_many_arguments)]
    fn record_straight_rules(
        &self,
        context: &mut CanvasContext,
        columns: &[usize],
        heights: &[usize],
        vertical: char,
        top: char,
        middle: char,
        bottom: char,
        vertical_last: bool,
    ) {
        let width = columns
            .iter()
            .sum::<usize>()
            .saturating_add(self.line_width());
        let height = heights
            .iter()
            .sum::<usize>()
            .saturating_add(self.line_height());
        if width == 0 || height == 0 {
            return;
        }

        let record_verticals = |context: &mut CanvasContext| {
            for x in self.rule_columns(columns) {
                self.record_cell_line(context, x, 0, x, height - 1, vertical);
            }
        };
        let record_horizontals = |context: &mut CanvasContext| {
            let rows = self.rule_rows(heights);
            for (index, y) in rows.iter().copied().enumerate() {
                let glyph = if self.presentation.border_top && index == 0 {
                    top
                } else if self.presentation.border_bottom && index + 1 == rows.len() {
                    bottom
                } else {
                    middle
                };
                self.record_cell_line(context, 0, y, width - 1, y, glyph);
            }
        };

        if vertical_last {
            record_horizontals(context);
            record_verticals(context);
        } else {
            record_verticals(context);
            record_horizontals(context);
        }
    }

    fn record_cell_line(
        &self,
        context: &mut CanvasContext,
        from_x: usize,
        from_y: usize,
        to_x: usize,
        to_y: usize,
        glyph: char,
    ) {
        let glyph = glyph.to_string();
        context.line(
            Position::new(position(from_x), position(from_y)),
            Position::new(position(to_x), position(to_y)),
            Grapheme::new(&glyph),
            self.rule_style(),
        );
    }

    fn rule_style(&self) -> TextStyle {
        let mut style = TextStyle::new();
        if let Some(color) = self.presentation.border_style.foreground_color() {
            style = style.foreground(color);
        }
        if let Some(color) = self.presentation.border_style.background_color() {
            style = style.background(color);
        }
        style
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TableFrameRow {
    kind: TableRowKind,
    cells: Vec<TableFrameCell>,
}

impl TableFrameRow {
    fn new<F>(
        values: &[String],
        columns: usize,
        kind: TableRowKind,
        presentation: &TablePresentation,
        mut override_style: F,
    ) -> Self
    where
        F: FnMut(TableCell<'_>, &BlockStyle) -> Option<BlockStyle>,
    {
        let cells = (0..columns)
            .map(|column| {
                let text = values.get(column).map_or("", String::as_str);
                let style_override = match kind {
                    TableRowKind::Header => None,
                    TableRowKind::Body(row) => {
                        let cell = TableCell { text, row, column };
                        override_style(cell, presentation.fallback_style(kind, column))
                    }
                };
                TableFrameCell {
                    text: text.to_owned(),
                    style_override: style_override
                        .map(|style| frame_cell_style(style, presentation.padding)),
                }
            })
            .collect();
        Self { kind, cells }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TableFrameCell {
    text: String,
    style_override: Option<BlockStyle>,
}

impl TableFrameCell {
    fn style<'a>(
        &'a self,
        presentation: &'a TablePresentation,
        kind: TableRowKind,
        column: usize,
    ) -> &'a BlockStyle {
        self.style_override
            .as_ref()
            .unwrap_or_else(|| presentation.fallback_style(kind, column))
    }

    fn width_requirements(&self, style: &BlockStyle) -> (usize, usize) {
        let padding = style.padding_sides();
        let frame = usize::from(padding.left) + usize::from(padding.right);
        let lines = PrintableLines::new(&self.text).lines();
        let demand = lines.iter().map(|line| line.width()).max().unwrap_or(0);
        let floor = lines
            .iter()
            .flat_map(|line| line.graphemes().map(Grapheme::width))
            .max()
            .unwrap_or(0);
        (demand.saturating_add(frame), floor.saturating_add(frame))
    }

    fn height_at(&self, width: usize, style: &BlockStyle) -> usize {
        let padding = style.padding_sides();
        let content_width = width
            .saturating_sub(usize::from(padding.left).saturating_add(usize::from(padding.right)));
        let content_height = match style.overflow_policy() {
            Overflow::Wrap => wrapped_line_count(PrintableLines::new(&self.text), content_width),
            Overflow::Clip(_) => PrintableLines::new(&self.text).lines().len(),
        };
        content_height
            .saturating_add(usize::from(padding.top))
            .saturating_add(usize::from(padding.bottom))
    }

    fn rendered_rows(&self, width: usize, height: usize, style: &BlockStyle) -> Vec<String> {
        let padding = style.padding_sides();
        let content_width = width
            .saturating_sub(usize::from(padding.left).saturating_add(usize::from(padding.right)));
        let content_height = height
            .saturating_sub(usize::from(padding.top).saturating_add(usize::from(padding.bottom)));
        let mut lines = fit_text_lines(&self.text, Some(content_width), style.overflow_policy());
        lines.truncate(content_height);
        let vertical_gap = content_height.saturating_sub(lines.len());
        let above = match style.vertical_alignment() {
            VerticalAlign::Top => 0,
            VerticalAlign::Center => vertical_gap / 2,
            VerticalAlign::Bottom => vertical_gap,
        };
        let blank = " ".repeat(width);
        let mut rows = Vec::with_capacity(height);
        rows.resize(
            usize::from(padding.top).saturating_add(above),
            blank.clone(),
        );
        for line in lines {
            let line_width = PrintableText::new(&line).width();
            let gap = content_width.saturating_sub(line_width);
            let before = match style.horizontal_alignment() {
                Align::Left => 0,
                Align::Center => gap / 2,
                Align::Right => gap,
            };
            let after = gap.saturating_sub(before);
            rows.push(format!(
                "{}{}{}",
                " ".repeat(usize::from(padding.left).saturating_add(before)),
                line,
                " ".repeat(after.saturating_add(usize::from(padding.right)))
            ));
        }
        rows.resize(height, blank);
        rows
    }

    #[cfg(test)]
    fn view(&self, style: &BlockStyle) -> View {
        let style = style.clone().width(Length::fill(1)).height(Length::fill(1));
        let text_style = style.text().clone();
        View::block(style, View::text(self.text.clone(), text_style))
    }
}

fn frame_cell_style(mut style: BlockStyle, padding: u16) -> BlockStyle {
    style = style
        .without_border()
        .margin(Sides::default())
        .auto_width()
        .auto_height()
        .without_min_width()
        .without_min_height()
        .without_max_width()
        .without_max_height();
    if style.padding_sides() == Default::default() {
        style = style.padding((0, padding));
    }
    style
}

struct TextRun {
    x: usize,
    text: String,
    style: TextStyle,
}

fn push_run(runs: &mut Vec<TextRun>, x: usize, text: &str, style: TextStyle) {
    if let Some(run) = runs.last_mut().filter(|run| run.style == style) {
        run.text.push_str(text);
    } else {
        runs.push(TextRun {
            x,
            text: text.to_owned(),
            style,
        });
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TableItem(Arc<TableFrame>);

impl TableItem {
    fn sizing(&self) -> CanvasSizing {
        CanvasSizing::intrinsic(self.clone())
    }
}

impl CanvasMeasure for TableItem {
    fn width_requirements(&self) -> CanvasRequirements {
        let requirements = self.0.column_requirements();
        CanvasRequirements::new(
            requirements
                .iter()
                .map(|requirement| requirement.0)
                .sum::<usize>()
                .saturating_add(self.0.line_width()),
            requirements
                .iter()
                .map(|requirement| requirement.1)
                .sum::<usize>()
                .saturating_add(self.0.line_width()),
        )
    }

    fn height_requirements(&self, width: usize) -> CanvasRequirements {
        CanvasRequirements::new(self.0.height_at(width), 0)
    }
}

impl CanvasItem for TableItem {
    fn draw(&self, context: &mut CanvasContext) {
        self.0.draw(context);
    }
}

fn position(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
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
        Align, Available, Border, Color, ComponentTheme, Overflow, PrintableText, SemanticTokens,
        StyledGrapheme, VerticalAlign, measure, resolve,
    };

    fn styles() -> ComponentTheme {
        ComponentTheme::from_tokens(&SemanticTokens {
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

    fn sample() -> TextTable {
        Table::text()
            .headers(["Name", "Location"])
            .row(["Kini", "New York"])
            .row(["Iris", "Paris"])
    }

    #[test]
    fn composes_headers_rows_and_borders() {
        assert_eq!(
            plain(&styles().table().compose(&sample())),
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
        assert!(measure(&table_style.compose(&Table::text())).is_empty());
        assert!(measure(&table_style.compose(&sample().hidden(true))).is_empty());

        let headers_only = Table::text().headers(["Name", "Location"]);
        assert_eq!(
            plain(&table_style.compose(&headers_only)),
            "\
┌──────┬──────────┐
│ Name │ Location │
└──────┴──────────┘"
        );

        let rows_only = Table::text().row(["Kini", "New York"]);
        assert_eq!(
            plain(&table_style.compose(&rows_only)),
            "\
┌──────┬──────────┐
│ Kini │ New York │
└──────┴──────────┘"
        );
    }

    #[test]
    fn ragged_rows_pad_missing_cells() {
        let table = Table::text()
            .headers(["A", "B", "C"])
            .row(["1"])
            .row(["1", "2", "3"]);

        assert_eq!(
            plain(&styles().table().compose(&table)),
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
            plain(&styles().table().compose(&table)),
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
        let table = Table::text()
            .headers(["名前", "所在地"])
            .row(["日本語", "東京"])
            .row(["ab", "Paris"]);
        let view = styles().table().compose(&table);

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
        let table = Table::text()
            .headers(["Key", "Value"])
            .row(["a", "one\ntwo"]);

        assert_eq!(
            plain(&styles().table().compose(&table)),
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
        let view = wide.compose(&sample());
        for row in plain_rows(&view) {
            assert_eq!(PrintableText::new(&row).width(), 28);
        }

        let narrow = table_style.width(16);
        assert_eq!(
            plain(&narrow.compose(&sample())),
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
        // bound frame widens past the constraint rather than dropping the
        // content, and every row still matches it because column and line
        // geometry are selected together.
        let table = Table::text().headers(["A", "B"]).row(["日本語", "x"]);
        let view = styles().table().clone().width(9).compose(&table);

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
            .compose(&Table::text().row(["日本", "ab"]));
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
        let view = narrow.compose(&sample());
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
                TableBorder::MARKDOWN,
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
                TableBorder::BOOKTABS,
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
                TableBorder::HIDDEN,
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
                plain(&table_style.clone().border(border).compose(&sample())),
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
            plain(&table_style.compose(&sample())),
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
            plain(&table_style.compose(&sample())),
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

    #[test]
    fn a_disabled_header_rule_leaves_the_first_gap_to_the_header_policy() {
        let table_style = styles()
            .table()
            .clone()
            .border_header(false)
            .border_row(true);

        assert_eq!(
            plain(&table_style.compose(&sample())),
            "\
┌──────┬──────────┐
│ Name │ Location │
│ Kini │ New York │
├──────┼──────────┤
│ Iris │ Paris    │
└──────┴──────────┘",
            "the header owns its gap even when body rows carry rules"
        );
    }

    #[test]
    fn a_disabled_column_rule_occupies_no_cell() {
        let table_style = styles().table().clone().border_column(false);

        assert_eq!(
            plain(&table_style.compose(&sample())),
            "\
┌────────────────┐
│ Name  Location │
├────────────────┤
│ Kini  New York │
│ Iris  Paris    │
└────────────────┘",
            "the columns touch while the outer edges and row rules remain"
        );
    }

    #[test]
    fn each_disabled_outer_edge_removes_its_own_cells() {
        let table = Table::text().headers(["A", "B"]).row(["1", "2"]);
        let table_style = styles().table().clone();
        let cases = [
            (
                table_style.clone().border_top(false),
                ["│ A │ B │", "├───┼───┤", "│ 1 │ 2 │", "└───┴───┘"].join("\n"),
            ),
            (
                table_style.clone().border_bottom(false),
                ["┌───┬───┐", "│ A │ B │", "├───┼───┤", "│ 1 │ 2 │"].join("\n"),
            ),
            (
                table_style.clone().border_left(false),
                ["───┬───┐", " A │ B │", "───┼───┤", " 1 │ 2 │", "───┴───┘"].join("\n"),
            ),
            (
                table_style.border_right(false),
                ["┌───┬───", "│ A │ B ", "├───┼───", "│ 1 │ 2 ", "└───┴───"].join("\n"),
            ),
        ];

        for (presentation, expected) in cases {
            assert_eq!(plain(&presentation.compose(&table)), expected);
        }
    }

    #[test]
    fn a_single_cell_has_no_internal_separator() {
        let table = Table::text().row(["only"]);

        assert_eq!(
            plain(&styles().table().compose(&table)),
            "\
┌──────┐
│ only │
└──────┘"
        );
    }

    #[test]
    fn a_standalone_network_rule_uses_its_straight_glyph() {
        let glyphs = LineGlyphs {
            isolated: 'o',
            end_up: 'u',
            end_right: 'r',
            end_down: 'd',
            end_left: 'l',
            vertical: 'v',
            horizontal: 'h',
            ..LineGlyphs::NORMAL
        };
        let one_column = Table::text().row([""]);
        let vertical = styles()
            .table()
            .clone()
            .border(TableBorder::network(glyphs))
            .padding(0)
            .border_top(false)
            .border_bottom(false)
            .border_right(false)
            .border_column(false)
            .compose(&one_column);
        assert_eq!(plain(&vertical), "v");

        let one_cell = Table::text().row(["x"]);
        let horizontal = styles()
            .table()
            .clone()
            .border(TableBorder::network(glyphs))
            .padding(0)
            .border_bottom(false)
            .border_left(false)
            .border_right(false)
            .border_column(false)
            .compose(&one_cell);
        assert_eq!(plain(&horizontal), "h\nx");
    }

    #[test]
    fn zero_width_cells_preserve_corners_next_to_disabled_edges() {
        let table = Table::text().row([""]);
        let left = styles()
            .table()
            .clone()
            .padding(0)
            .border_bottom(false)
            .border_right(false)
            .border_column(false)
            .compose(&table);
        assert_eq!(plain(&left), "┌\n│");

        let right = styles()
            .table()
            .clone()
            .padding(0)
            .border_bottom(false)
            .border_left(false)
            .border_column(false)
            .compose(&table);
        assert_eq!(plain(&right), "┐\n│");
    }

    fn accent_rows() -> TableRowPresentation<'static, TextTableRow> {
        TableRowPresentation::display().cell_style(|_, cell, _| match cell.row() {
            1 => Some(BlockStyle::new().foreground(Color::GREEN)),
            _ => None,
        })
    }

    #[test]
    fn cell_replacements_override_column_and_table_styles() {
        let component_styles = styles();
        let table_style = component_styles
            .table()
            .clone()
            .header_styles([
                Some(BlockStyle::new().foreground(Color::CYAN).bold()),
                Some(BlockStyle::new().foreground(Color::CYAN).bold()),
            ])
            .column_styles([None, Some(BlockStyle::new().align(Align::Right))]);
        let view = table_style.compose_with(&sample(), &accent_rows());

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
    fn a_styler_realigns_a_column_that_has_slack() {
        // Body row 2 escapes the (Body(1), _) arm, so the (_, 1) column arm
        // applies where the cell is narrower than its column.
        let table = sample().row(["Eli", "London"]);
        let table_style = styles()
            .table()
            .clone()
            .column_styles([None, Some(BlockStyle::new().align(Align::Right))]);
        let view = table_style.compose_with(&table, &accent_rows());

        assert_eq!(
            plain_rows(&view)[5],
            "│ Eli  │   London │",
            "the (_, 1) arm right-aligns column 1 where the cell has slack"
        );
    }

    #[test]
    fn a_typed_cell_override_keeps_padding_and_loses_the_rest_of_the_box() {
        let rows = TableRowPresentation::<TextTableRow>::display().cell_style(|_, _, _| {
            Some(
                BlockStyle::new()
                    .padding((0, 3))
                    .margin(2)
                    .width(30)
                    .height(4)
                    .min_width(24)
                    .min_height(3)
                    .max_width(28)
                    .max_height(3)
                    .border(Border::DOUBLE),
            )
        });

        let table_style = styles().table().clone();

        // A cell that states its own padding replaces the presentation's
        // rather than adding to it, and its column is as wide as it needs.
        // The bound frame owns column geometry, so the cell's stated sizes,
        // border, and margin do not survive.
        assert_eq!(
            plain(&table_style.compose_with(&sample(), &rows)),
            "\
┌──────────┬──────────────┐
│ Name     │ Location     │
├──────────┼──────────────┤
│   Kini   │   New York   │
│   Iris   │   Paris      │
└──────────┴──────────────┘"
        );
    }

    #[test]
    fn an_offset_excluded_row_does_not_widen_the_table() {
        let table = Table::text()
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
            plain(&styles().table().compose(&table)),
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
        let table_style = styles().table().clone().column_styles([
            Some(BlockStyle::new().align(Align::Right)),
            Some(BlockStyle::new().align(Align::Right)),
        ]);

        assert_eq!(
            plain(&table_style.compose(&sample())),
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
        let table = Table::text().row(["a", "one\ntwo"]);
        let table_style = styles().table().clone().column_styles([
            Some(BlockStyle::new().align_vertical(VerticalAlign::Bottom)),
            Some(BlockStyle::new().align_vertical(VerticalAlign::Bottom)),
        ]);

        assert_eq!(
            plain(&table_style.compose(&table)),
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
            plain(&table_style.compose(&sample())),
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

        let ragged = Table::text().headers(["A"]).row(["1", "2", "3"]);
        assert_eq!(ragged.column_count(), 3);
    }

    #[test]
    fn the_typed_styler_sees_each_body_cell_its_row_and_its_text() {
        let rows = TableRowPresentation::<TextTableRow>::display().cell_style(|_, cell, _| {
            let expected = match cell.column() {
                0 => "Kini",
                _ => "New York",
            };
            assert_eq!(cell.text(), expected, "column {}", cell.column());
            assert_eq!(cell.row(), 0);
            None
        });

        let table = Table::text()
            .headers(["Name", "Location"])
            .row(["Kini", "New York"]);
        let _ = styles().table().compose_with(&table, &rows);
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
    fn border_drawing_preserves_only_the_legacy_color_properties() {
        let presentation = styles().table().clone().border_style(
            TextStyle::new()
                .foreground(Color::RED)
                .background(Color::BLUE)
                .bold()
                .underline(),
        );
        let view = presentation.compose(&Table::text().row(["cell"]));

        assert_eq!(
            style_at(&view, 0, 0),
            TextStyle::new()
                .foreground(Color::RED)
                .background(Color::BLUE)
        );
    }

    /// A table whose natural width is 41 cells: the case #22 reproduced.
    fn wide() -> TextTable {
        Table::text()
            .headers(["N", "Description"])
            .row(["a", "the quick brown fox jumps again!!"])
            .row(["b", "over the lazy dog"])
    }

    #[test]
    fn an_area_narrower_than_the_table_shrinks_its_columns_and_its_lines() {
        let view = styles().table().compose(&wide());
        assert_eq!(measure(&view).width(), 41, "the natural width");

        let rows: Vec<String> = resolve(&view, Available::columns(18))
            .unwrap()
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
        let view = styles().table().compose(&wide());

        let rows: Vec<String> = resolve(&view, Available::columns(200))
            .unwrap()
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
    fn table_presentations_compare_by_complete_style_lists() {
        let table_style = styles().table().clone();
        assert_eq!(table_style, styles().table().clone());
        assert_ne!(
            table_style.clone(),
            table_style
                .clone()
                .column_styles([None, Some(BlockStyle::new().bold())])
        );
        assert_ne!(table_style.clone(), table_style.border(TableBorder::ASCII));
    }

    #[test]
    fn canonical_composition_binds_an_intrinsic_canvas_item() {
        let view = styles().table().compose(&sample());
        let View::Block(style, _, child) = view else {
            panic!("a Table is boxed only to state its outer sizing")
        };
        assert_eq!(style.border_kind(), None);
        assert!(matches!(*child, View::Canvas(_)));
    }

    #[test]
    fn narrow_height_measurement_matches_drawing_and_shorter_height_crops() {
        let view = styles().table().compose(&wide());
        let complete = resolve(&view, Available::columns(18)).unwrap();
        let exact = resolve(&view, Available::size(18, complete.size().height()));
        assert_eq!(
            exact.unwrap(),
            complete,
            "measurement and drawing use one bound frame"
        );

        let cropped = resolve(&view, Available::size(18, 3)).unwrap();
        assert_eq!(cropped.size().height(), 3);
        assert_eq!(cropped.rows(), &complete.rows()[..3]);
    }

    #[test]
    fn table_cell_height_matches_the_shared_block_text_model() {
        let cases = [
            (
                TableFrameCell {
                    text: "words that wrap\n日本語".to_owned(),
                    style_override: None,
                },
                BlockStyle::new().padding((2, 1)),
            ),
            (
                TableFrameCell {
                    text: "words that clip\n日本語".to_owned(),
                    style_override: None,
                },
                BlockStyle::new()
                    .padding((1, 2))
                    .overflow(Overflow::ellipsis()),
            ),
        ];

        for (cell, style) in cases {
            let floor = cell.width_requirements(&style).1;
            for width in [floor, floor.saturating_add(3), floor.saturating_add(12)] {
                assert_eq!(
                    cell.height_at(width, &style),
                    resolve(&cell.view(&style), Available::columns(width))
                        .unwrap()
                        .size()
                        .height(),
                    "{cell:?} at width {width}"
                );
            }
        }
    }

    #[test]
    fn an_outer_borderless_block_can_state_fixed_or_fill_table_size() {
        let table = styles().table().compose(&sample());
        let fixed = View::block(BlockStyle::new().width(30), table.clone());
        assert_eq!(resolve(&fixed, Available::NONE).unwrap().size().width(), 30);

        let fill = View::block(BlockStyle::new().width(Length::fill(1)), table);
        assert_eq!(
            resolve(&fill, Available::columns(32))
                .unwrap()
                .size()
                .width(),
            32
        );
    }

    #[test]
    fn every_border_preset_keeps_its_observable_glyphs() {
        let table = Table::text().headers(["A", "B"]).row(["1", "2"]);
        let cases = [
            (TableBorder::NORMAL, '┌', '┬', '┼', '└'),
            (TableBorder::ROUNDED, '╭', '┬', '┼', '╰'),
            (TableBorder::THICK, '┏', '┳', '╋', '┗'),
            (TableBorder::DOUBLE, '╔', '╦', '╬', '╚'),
            (TableBorder::ASCII, '+', '+', '+', '+'),
            (TableBorder::MARKDOWN, '|', '|', '|', '|'),
            (TableBorder::BOOKTABS, '━', '━', '─', '━'),
            (TableBorder::HIDDEN, ' ', ' ', ' ', ' '),
        ];

        for (border, top_left, top_middle, middle, bottom_left) in cases {
            let rows = plain_rows(&styles().table().clone().border(border).compose(&table));
            assert_eq!(rows[0].chars().next(), Some(top_left));
            assert!(rows[0].contains(top_middle));
            assert!(rows[2].contains(middle));
            assert_eq!(rows[4].chars().next(), Some(bottom_left));
        }
    }

    #[test]
    fn caller_owned_line_glyphs_derive_every_table_junction() {
        let glyphs = LineGlyphs {
            isolated: 'o',
            end_up: 'u',
            end_right: 'r',
            end_down: 'd',
            end_left: 'l',
            vertical: 'v',
            horizontal: 'h',
            corner_down_right: 'A',
            corner_down_left: 'B',
            corner_up_right: 'C',
            corner_up_left: 'D',
            tee_right: 'E',
            tee_down: 'H',
            tee_left: 'F',
            tee_up: 'I',
            cross: 'G',
        };
        let table = Table::text()
            .headers(["a", "b"])
            .row(["1", "2"])
            .row(["3", "4"]);
        let view = styles()
            .table()
            .clone()
            .border(TableBorder::network(glyphs))
            .border_row(true)
            .compose(&table);

        assert_eq!(
            plain(&view),
            [
                "AhhhHhhhB",
                "v a v b v",
                "EhhhGhhhF",
                "v 1 v 2 v",
                "EhhhGhhhF",
                "v 3 v 4 v",
                "ChhhIhhhD",
            ]
            .join("\n")
        );
    }

    #[test]
    fn theme_table_is_the_canonical_presentation_shortcut() {
        let tokens = SemanticTokens {
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
        };
        let theme = crate::Theme::from_tokens(tokens);
        assert_eq!(
            theme.table(&sample()),
            theme.components().table().compose(&sample())
        );
    }
}
