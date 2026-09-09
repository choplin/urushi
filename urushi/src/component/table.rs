//! Renderer-neutral tables with an independent public model.

use std::{any::Any, fmt, sync::Arc};

use crate::text::wrapped_line_count;
use crate::view::{CanvasMeasure, CanvasRequirements, Claim, Kind, distribute, fit_text_lines};
use crate::{
    Align, BlockStyle, BlockStylePropertyKey, Border, Canvas, CanvasContext, CanvasItem,
    CanvasSizing, CellContribution, Composition, Grapheme, Length, Overflow, Position,
    PositionedCell, PrintableLines, PrintableText, TableRole, TextStyle, VerticalAlign, View,
};

/// Selects the glyph implied by the four incident line directions.
const fn junction(
    border: &Border,
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    horizontal: char,
    vertical: char,
) -> char {
    match (up, down, left, right) {
        (true, true, true, true) => border.middle,
        (false, true, true, true) => border.middle_top,
        (true, false, true, true) => border.middle_bottom,
        (true, true, false, true) => border.middle_left,
        (true, true, true, false) => border.middle_right,
        (false, true, false, true) => border.top_left,
        (false, true, true, false) => border.top_right,
        (true, false, false, true) => border.bottom_left,
        (true, false, true, false) => border.bottom_right,
        (true, _, false, false) | (false, true, false, false) => vertical,
        (false, false, true, _) | (false, false, false, true) => horizontal,
        (false, false, false, false) => ' ',
    }
}

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

/// Identifies one cell while a [`TableCellStyler`] resolves its style.
///
/// Lip Gloss passes a bare `(row, column)` pair and encodes the header row as
/// the sentinel index `-1`. urushi passes this opaque value instead, matching
/// the [`ListPosition`](crate::ListPosition) and
/// [`SiblingPosition`](crate::SiblingPosition) precedent: further context can
/// be exposed as new accessors without changing the styler signature.
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
/// `None` keeps the [`TablePresentation`] role default for that cell, so a
/// styler only has to name the cells it overrides. `Some(style)` **replaces**
/// the role default outright rather than layering over it, so a styler that
/// means to keep the theme's colors must set them itself:
///
/// ```
/// use urushi::{
///     Align, BlockStyle, Color, TableCell, TableCellStyler, TablePresentation,
///     TextStyle,
/// };
///
/// #[derive(Debug, Clone, PartialEq)]
/// struct NumbersRight;
///
/// impl TableCellStyler for NumbersRight {
///     fn style(&self, cell: TableCell<'_>) -> Option<BlockStyle> {
///         if cell.is_header() || cell.column() == 0 {
///             return None;
///         }
///         // Restates the foreground, because Some replaces the role default.
///         Some(BlockStyle::new().foreground(Color::CYAN).align(Align::Right))
///     }
/// }
///
/// let cells = BlockStyle::new().foreground(Color::CYAN);
/// let presentation = TablePresentation::new(BlockStyle::new(), cells, TextStyle::new())
///     .cell_styler(NumbersRight);
/// ```
///
/// A cell is a block, so the styler returns a [`BlockStyle`]. Its colors, text
/// modifiers, alignment, and padding are used; a cell that states its own
/// padding replaces the table's for that cell, and its column is as wide as it
/// needs. The rest of the geometry — border, margin, and stated sizes — is
/// dropped, because the table's bound Canvas frame owns layout.
///
/// A styler is retained as an owned value. Its concrete type and complete
/// value participate in presentation equality, so implementations must include
/// every field that can affect the returned style in their `PartialEq`.
pub trait TableCellStyler: fmt::Debug + Send + Sync + 'static {
    /// Returns this cell's complete replacement style, or its role default.
    fn style(&self, cell: TableCell<'_>) -> Option<BlockStyle>;
}

trait ErasedTableCellStyler: fmt::Debug + Send + Sync {
    fn style(&self, cell: TableCell<'_>) -> Option<BlockStyle>;
    fn clone_box(&self) -> Box<dyn ErasedTableCellStyler>;
    fn equals(&self, other: &dyn ErasedTableCellStyler) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T> ErasedTableCellStyler for T
where
    T: TableCellStyler + Clone + PartialEq,
{
    fn style(&self, cell: TableCell<'_>) -> Option<BlockStyle> {
        TableCellStyler::style(self, cell)
    }

    fn clone_box(&self) -> Box<dyn ErasedTableCellStyler> {
        Box::new(self.clone())
    }

    fn equals(&self, other: &dyn ErasedTableCellStyler) -> bool {
        other.as_any().downcast_ref::<T>() == Some(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
struct CellStyler(Box<dyn ErasedTableCellStyler>);

impl CellStyler {
    fn new<T>(styler: T) -> Self
    where
        T: TableCellStyler + Clone + PartialEq,
    {
        Self(Box::new(styler))
    }

    fn style(&self, cell: TableCell<'_>) -> Option<BlockStyle> {
        self.0.style(cell)
    }
}

impl Clone for CellStyler {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl PartialEq for CellStyler {
    fn eq(&self, other: &Self) -> bool {
        self.0.equals(&*other.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DefaultCellStyler;

impl TableCellStyler for DefaultCellStyler {
    fn style(&self, _: TableCell<'_>) -> Option<BlockStyle> {
        None
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
/// cells when it is composed.
///
/// ```
/// use urushi::{BlockStyle, Table, TablePresentation, TextStyle, measure};
///
/// let table = Table::new()
///     .headers(["Name", "Location"])
///     .row(["Kini", "New York"])
///     .row(["Iris", "Paris"]);
///
/// let presentation =
///     TablePresentation::new(BlockStyle::new(), BlockStyle::new(), TextStyle::new());
/// assert_eq!(measure(&presentation.compose(&table)).height(), 6);
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
/// constraint, and the per-cell styling strategy. It borrows the data and
/// performs no terminal output.
#[derive(Debug, Clone)]
pub struct TablePresentation {
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
    cell_styler: CellStyler,
}

impl PartialEq for TablePresentation {
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
            && self.cell_styler == other.cell_styler
    }
}

impl TablePresentation {
    /// Creates a table presentation with a single-line border and one cell of padding.
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
            cell_styler: CellStyler::new(DefaultCellStyler),
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

    /// Replaces the owned per-cell styling strategy.
    #[must_use]
    pub fn cell_styler<T>(mut self, styler: T) -> Self
    where
        T: TableCellStyler + Clone + PartialEq,
    {
        self.cell_styler = CellStyler::new(styler);
        self
    }

    /// Composes table data into an intrinsically sized, renderer-neutral Canvas.
    ///
    /// Composition binds an owned snapshot without receiving an available
    /// area. During resolution, the bound item first supplies width and
    /// width-dependent height requirements, then records cell and rule
    /// commands after the final Canvas size is known.
    pub fn compose(&self, table: &Table) -> View {
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

        let item = TableItem(Arc::new(TableFrame::new(table, self, rows, columns)));
        let canvas = Canvas::new().sizing(item.sizing()).item(item);
        let frame = self.width.map_or_else(BlockStyle::new, |width| {
            BlockStyle::new().width(Length::Cells(width))
        });
        View::block(frame, View::canvas(canvas))
    }

    /// Resolves one cell's style from the strategy, falling back to its role.
    fn cell_style_at(&self, cell: TableCell<'_>) -> BlockStyle {
        self.cell_styler
            .style(cell)
            .unwrap_or_else(|| match cell.row {
                TableRow::Header => self.header.clone(),
                _ => self.cell.clone(),
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
    fn new(
        table: &Table,
        presentation: &TablePresentation,
        visible_rows: &[Vec<String>],
        columns: usize,
    ) -> Self {
        let mut rows =
            Vec::with_capacity(visible_rows.len() + usize::from(!table.headers.is_empty()));
        if !table.headers.is_empty() {
            rows.push(TableFrameRow::new(
                &table.headers,
                columns,
                TableRow::Header,
                presentation,
            ));
        }
        for (index, row) in visible_rows.iter().enumerate() {
            rows.push(TableFrameRow::new(
                row,
                columns,
                TableRow::Body(index),
                presentation,
            ));
        }
        Self {
            presentation: presentation.clone(),
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
            Some(TableRow::Header) => self.presentation.border_header,
            _ => self.presentation.border_row,
        }
    }

    fn column_requirements(&self) -> Vec<(usize, usize)> {
        (0..self.columns)
            .map(|column| {
                self.rows
                    .iter()
                    .map(|row| row.cells[column].width_requirements())
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
                    .zip(columns)
                    .map(|(cell, width)| cell.height_at(*width))
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
        let segments = self.line_segments(&columns);
        let mut line_cells = Vec::new();
        let mut y = 0;

        self.record_cell_rows(context, &columns, &heights);

        if self.presentation.border_top {
            self.push_rule(
                &mut line_cells,
                &segments,
                y,
                false,
                true,
                self.presentation.border.top,
            );
            y += 1;
        }

        for (row_index, (_row, height)) in self.rows.iter().zip(&heights).enumerate() {
            for offset in 0..*height {
                self.push_verticals(&mut line_cells, &segments, y + offset);
            }
            y = y.saturating_add(*height);

            if row_index + 1 < self.rows.len() && self.draws_row_rule(row_index) {
                self.push_rule(
                    &mut line_cells,
                    &segments,
                    y,
                    true,
                    true,
                    self.presentation.border.middle_horizontal,
                );
                y += 1;
            }
        }

        if self.presentation.border_bottom {
            self.push_rule(
                &mut line_cells,
                &segments,
                y,
                true,
                false,
                self.presentation.border.bottom,
            );
        }
        context.cells(line_cells);
    }

    fn record_cell_rows(&self, context: &mut CanvasContext, columns: &[usize], heights: &[usize]) {
        let mut y = usize::from(self.presentation.border_top);
        for (row_index, (row, height)) in self.rows.iter().zip(heights).enumerate() {
            let rendered = row
                .cells
                .iter()
                .zip(columns)
                .map(|(cell, width)| cell.rendered_rows(*width, *height))
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
                    push_run(&mut runs, x, &rows[line], cell.style.text().clone());
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

    fn line_segments(&self, columns: &[usize]) -> Vec<LineSegment> {
        let mut segments = Vec::with_capacity(columns.len() * 2 + 2);
        if self.presentation.border_left {
            segments.push(LineSegment::Rule(self.presentation.border.left));
        }
        for (index, width) in columns.iter().enumerate() {
            if index > 0 && self.presentation.border_column {
                segments.push(LineSegment::Rule(self.presentation.border.left));
            }
            segments.push(LineSegment::Span(*width));
        }
        if self.presentation.border_right {
            segments.push(LineSegment::Rule(self.presentation.border.right));
        }
        segments
    }

    fn push_verticals(&self, output: &mut Vec<PositionedCell>, segments: &[LineSegment], y: usize) {
        let mut x = 0;
        for segment in segments {
            match segment {
                LineSegment::Rule(glyph) => {
                    output.push(self.line_cell(x, y, *glyph));
                    x += 1;
                }
                LineSegment::Span(width) => x = x.saturating_add(*width),
            }
        }
    }

    fn push_rule(
        &self,
        output: &mut Vec<PositionedCell>,
        segments: &[LineSegment],
        y: usize,
        up: bool,
        down: bool,
        horizontal: char,
    ) {
        let mut x = 0;
        for (index, segment) in segments.iter().enumerate() {
            match segment {
                LineSegment::Rule(vertical) => {
                    let glyph = junction(
                        &self.presentation.border,
                        up,
                        down,
                        index > 0,
                        index + 1 < segments.len(),
                        horizontal,
                        *vertical,
                    );
                    output.push(self.line_cell(x, y, glyph));
                    x += 1;
                }
                LineSegment::Span(width) => {
                    for offset in 0..*width {
                        output.push(self.line_cell(x + offset, y, horizontal));
                    }
                    x = x.saturating_add(*width);
                }
            }
        }
    }

    fn line_cell(&self, x: usize, y: usize, glyph: char) -> PositionedCell {
        let glyph = glyph.to_string();
        let mut style = TextStyle::new();
        if let Some(color) = self.presentation.border_style.foreground_color() {
            style = style.foreground(color);
        }
        if let Some(color) = self.presentation.border_style.background_color() {
            style = style.background(color);
        }
        PositionedCell::new(
            Position::new(position(x), position(y)),
            CellContribution::new()
                .symbol(Grapheme::new(&glyph))
                .style(style),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TableFrameRow {
    kind: TableRow,
    cells: Vec<TableFrameCell>,
}

impl TableFrameRow {
    fn new(
        values: &[String],
        columns: usize,
        kind: TableRow,
        presentation: &TablePresentation,
    ) -> Self {
        let cells = (0..columns)
            .map(|column| {
                let text = values.get(column).map_or("", String::as_str);
                let cell = TableCell {
                    text,
                    row: kind,
                    column,
                };
                let mut style = presentation
                    .cell_style_at(cell)
                    .remove(BlockStylePropertyKey::Border)
                    .remove(BlockStylePropertyKey::Margin)
                    .remove(BlockStylePropertyKey::Width)
                    .remove(BlockStylePropertyKey::Height)
                    .remove(BlockStylePropertyKey::MinWidth)
                    .remove(BlockStylePropertyKey::MinHeight)
                    .remove(BlockStylePropertyKey::MaxWidth)
                    .remove(BlockStylePropertyKey::MaxHeight);
                if style.padding_sides() == Default::default() {
                    style = style.padding((0, presentation.padding));
                }
                TableFrameCell {
                    text: text.to_owned(),
                    style,
                }
            })
            .collect();
        Self { kind, cells }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TableFrameCell {
    text: String,
    style: BlockStyle,
}

impl TableFrameCell {
    fn width_requirements(&self) -> (usize, usize) {
        let padding = self.style.padding_sides();
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

    fn height_at(&self, width: usize) -> usize {
        let padding = self.style.padding_sides();
        let content_width = width
            .saturating_sub(usize::from(padding.left).saturating_add(usize::from(padding.right)));
        let content_height = match self.style.overflow_policy() {
            Overflow::Wrap => wrapped_line_count(PrintableLines::new(&self.text), content_width),
            Overflow::Clip(_) => PrintableLines::new(&self.text).lines().len(),
        };
        content_height
            .saturating_add(usize::from(padding.top))
            .saturating_add(usize::from(padding.bottom))
    }

    fn rendered_rows(&self, width: usize, height: usize) -> Vec<String> {
        let padding = self.style.padding_sides();
        let content_width = width
            .saturating_sub(usize::from(padding.left).saturating_add(usize::from(padding.right)));
        let content_height = height
            .saturating_sub(usize::from(padding.top).saturating_add(usize::from(padding.bottom)));
        let mut lines = fit_text_lines(
            &self.text,
            Some(content_width),
            self.style.overflow_policy(),
        );
        lines.truncate(content_height);
        let vertical_gap = content_height.saturating_sub(lines.len());
        let above = match self.style.vertical_alignment() {
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
            let before = match self.style.horizontal_alignment() {
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
    fn view(&self) -> View {
        let style = self
            .style
            .clone()
            .width(Length::fill(1))
            .height(Length::fill(1));
        View::block(
            style,
            View::text(self.text.clone(), self.style.text().clone()),
        )
    }
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

#[derive(Debug, Clone, Copy)]
enum LineSegment {
    Rule(char),
    Span(usize),
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
        Align, Available, Color, ComponentStyles, Overflow, PrintableText, SemanticTokens,
        StyledGrapheme, VerticalAlign, measure, resolve,
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
        assert!(measure(&table_style.compose(&Table::new())).is_empty());
        assert!(measure(&table_style.compose(&sample().hidden(true))).is_empty());

        let headers_only = Table::new().headers(["Name", "Location"]);
        assert_eq!(
            plain(&table_style.compose(&headers_only)),
            "\
┌──────┬──────────┐
│ Name │ Location │
└──────┴──────────┘"
        );

        let rows_only = Table::new().row(["Kini", "New York"]);
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
        let table = Table::new()
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
        let table = Table::new()
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
        let table = Table::new()
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
        let table = Table::new().headers(["A", "B"]).row(["日本語", "x"]);
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
            .compose(&Table::new().row(["日本", "ab"]));
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
        let table = Table::new().headers(["A", "B"]).row(["1", "2"]);
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
        let table = Table::new().row(["only"]);

        assert_eq!(
            plain(&styles().table().compose(&table)),
            "\
┌──────┐
│ only │
└──────┘"
        );
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct AccentHeader;

    impl TableCellStyler for AccentHeader {
        fn style(&self, cell: TableCell<'_>) -> Option<BlockStyle> {
            if cell.is_header() {
                return Some(BlockStyle::new().foreground(Color::CYAN).bold());
            }
            match (cell.row(), cell.column()) {
                (Some(1), _) => Some(BlockStyle::new().foreground(Color::GREEN)),
                (_, 1) => Some(BlockStyle::new().align(Align::Right)),
                _ => None,
            }
        }
    }

    #[test]
    fn the_cell_styler_overrides_cells_rows_and_columns() {
        let component_styles = styles();
        let table_style = component_styles.table().clone().cell_styler(AccentHeader);
        let view = table_style.compose(&sample());

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
        let table_style = styles().table().clone().cell_styler(AccentHeader);
        let view = table_style.compose(&table);

        assert_eq!(
            plain_rows(&view)[5],
            "│ Eli  │   London │",
            "the (_, 1) arm right-aligns column 1 where the cell has slack"
        );
    }

    #[test]
    fn a_styler_keeps_its_padding_and_loses_the_rest_of_the_box() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Boxed;
        impl TableCellStyler for Boxed {
            fn style(&self, _: TableCell<'_>) -> Option<BlockStyle> {
                Some(
                    BlockStyle::new()
                        .padding((0, 3))
                        .width(30)
                        .height(4)
                        .border(Border::DOUBLE),
                )
            }
        }

        let table_style = styles().table().clone().cell_styler(Boxed);

        // A cell that states its own padding replaces the presentation's
        // rather than adding to it, and its column is as wide as it needs.
        // The bound frame owns column geometry, so the cell's stated sizes,
        // border, and margin do not survive.
        assert_eq!(
            plain(&table_style.compose(&sample())),
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
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Right;
        impl TableCellStyler for Right {
            fn style(&self, _: TableCell<'_>) -> Option<BlockStyle> {
                Some(BlockStyle::new().align(Align::Right))
            }
        }

        let table_style = styles().table().clone().cell_styler(Right);

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
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Bottom;
        impl TableCellStyler for Bottom {
            fn style(&self, _: TableCell<'_>) -> Option<BlockStyle> {
                Some(BlockStyle::new().align_vertical(VerticalAlign::Bottom))
            }
        }

        let table = Table::new().row(["a", "one\ntwo"]);
        let table_style = styles().table().clone().cell_styler(Bottom);

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

        let ragged = Table::new().headers(["A"]).row(["1", "2", "3"]);
        assert_eq!(ragged.column_count(), 3);
    }

    #[test]
    fn the_styler_sees_each_cell_its_row_and_its_text() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Shout;
        impl TableCellStyler for Shout {
            fn style(&self, cell: TableCell<'_>) -> Option<BlockStyle> {
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
        }

        let table = Table::new()
            .headers(["Name", "Location"])
            .row(["Kini", "New York"]);
        let _ = styles().table().clone().cell_styler(Shout).compose(&table);
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
        let view = presentation.compose(&Table::new().row(["cell"]));

        assert_eq!(
            style_at(&view, 0, 0),
            TextStyle::new()
                .foreground(Color::RED)
                .background(Color::BLUE)
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
        let view = styles().table().compose(&wide());
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
        let view = styles().table().compose(&wide());

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
    fn table_presentations_compare_by_policy_and_styler() {
        let table_style = styles().table().clone();
        assert_eq!(table_style, styles().table().clone());
        assert_ne!(
            table_style.clone(),
            table_style.clone().cell_styler(AccentHeader)
        );
        assert_ne!(table_style.clone(), table_style.border(Border::ASCII));
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct ColumnStyler(usize);

    impl TableCellStyler for ColumnStyler {
        fn style(&self, cell: TableCell<'_>) -> Option<BlockStyle> {
            (cell.column() == self.0).then(|| BlockStyle::new().bold())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct OtherColumnStyler(usize);

    impl TableCellStyler for OtherColumnStyler {
        fn style(&self, cell: TableCell<'_>) -> Option<BlockStyle> {
            (cell.column() == self.0).then(|| BlockStyle::new().bold())
        }
    }

    #[test]
    fn cell_stylers_compare_by_concrete_type_and_value() {
        let base = styles().table().clone();
        assert_eq!(
            base.clone().cell_styler(ColumnStyler(1)),
            base.clone().cell_styler(ColumnStyler(1))
        );
        assert_ne!(
            base.clone().cell_styler(ColumnStyler(1)),
            base.clone().cell_styler(ColumnStyler(0))
        );
        assert_ne!(
            base.clone().cell_styler(ColumnStyler(1)),
            base.cell_styler(OtherColumnStyler(1)),
            "equal output from another concrete strategy is not value equality"
        );
    }

    #[test]
    fn canonical_composition_binds_an_intrinsic_canvas_item() {
        let view = styles().table().compose(&sample());
        let View::Block(style, child) = view else {
            panic!("a Table is boxed only to state its outer sizing")
        };
        assert_eq!(style.border_kind(), None);
        assert!(matches!(*child, View::Canvas(_)));
    }

    #[test]
    fn narrow_height_measurement_matches_drawing_and_shorter_height_crops() {
        let view = styles().table().compose(&wide());
        let complete = resolve(&view, Available::columns(18));
        let exact = resolve(&view, Available::size(18, complete.size().height()));
        assert_eq!(
            exact, complete,
            "measurement and drawing use one bound frame"
        );

        let cropped = resolve(&view, Available::size(18, 3));
        assert_eq!(cropped.size().height(), 3);
        assert_eq!(cropped.rows(), &complete.rows()[..3]);
    }

    #[test]
    fn table_cell_height_matches_the_shared_block_text_model() {
        let cases = [
            TableFrameCell {
                text: "words that wrap\n日本語".to_owned(),
                style: BlockStyle::new().padding((2, 1)),
            },
            TableFrameCell {
                text: "words that clip\n日本語".to_owned(),
                style: BlockStyle::new()
                    .padding((1, 2))
                    .overflow(Overflow::ellipsis()),
            },
        ];

        for cell in cases {
            let floor = cell.width_requirements().1;
            for width in [floor, floor.saturating_add(3), floor.saturating_add(12)] {
                assert_eq!(
                    cell.height_at(width),
                    resolve(&cell.view(), Available::columns(width))
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
        assert_eq!(resolve(&fixed, Available::NONE).size().width(), 30);

        let fill = View::block(BlockStyle::new().width(Length::fill(1)), table);
        assert_eq!(resolve(&fill, Available::columns(32)).size().width(), 32);
    }

    #[test]
    fn every_border_preset_keeps_its_observable_glyphs() {
        let table = Table::new().headers(["A", "B"]).row(["1", "2"]);
        let cases = [
            (Border::NORMAL, '┌', '┬', '┼', '└'),
            (Border::ROUNDED, '╭', '┬', '┼', '╰'),
            (Border::THICK, '┏', '┳', '╋', '┗'),
            (Border::DOUBLE, '╔', '╦', '╬', '╚'),
            (Border::ASCII, '+', '+', '+', '+'),
            (Border::MARKDOWN, '|', '|', '|', '|'),
            (Border::BOOKTABS, '━', '━', '─', '━'),
            (Border::HIDDEN, ' ', ' ', ' ', ' '),
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
    fn a_custom_border_derives_corners_tees_and_crosses_from_incident_rules() {
        let border = Border {
            top: 't',
            bottom: 'b',
            left: 'l',
            right: 'r',
            top_left: 'A',
            top_right: 'B',
            bottom_left: 'C',
            bottom_right: 'D',
            middle_left: 'E',
            middle_right: 'F',
            middle: 'G',
            middle_horizontal: 'm',
            middle_top: 'H',
            middle_bottom: 'I',
        };
        let table = Table::new()
            .headers(["a", "b"])
            .row(["1", "2"])
            .row(["3", "4"]);
        let view = styles()
            .table()
            .clone()
            .border(border)
            .border_row(true)
            .compose(&table);

        assert_eq!(
            plain(&view),
            [
                "AtttHtttB",
                "l a l b r",
                "EmmmGmmmF",
                "l 1 l 2 r",
                "EmmmGmmmF",
                "l 3 l 4 r",
                "CbbbIbbbD",
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
