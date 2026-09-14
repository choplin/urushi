use std::collections::HashMap;
use std::ops::RangeInclusive;

use crate::TextStyle;

use super::Position;
use super::cell::validate_cell_glyph;
use super::cell_primitives::{clip_line, line_points};
use crate::view::geometry::Size;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct LineConnections(u8);

impl LineConnections {
    const UP: Self = Self(1);
    const RIGHT: Self = Self(2);
    const DOWN: Self = Self(4);
    const LEFT: Self = Self(8);

    pub(super) const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Incidence that continues beyond a line segment's inclusive range.
///
/// `START` and `END` refer to the ascending endpoints of the supplied range.
/// For a horizontal segment they continue left and right respectively; for a
/// vertical segment they continue up and down. Continuation affects the glyph
/// selected at the endpoint without drawing or occupying the outside cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LineContinuations(u8);

impl LineContinuations {
    /// No endpoint continues beyond the inclusive range.
    pub const NONE: Self = Self(0);
    /// Only the start endpoint continues beyond the inclusive range.
    pub const START: Self = Self(1);
    /// Only the end endpoint continues beyond the inclusive range.
    pub const END: Self = Self(2);
    /// Both endpoints continue beyond the inclusive range.
    pub const BOTH: Self = Self(Self::START.0 | Self::END.0);

    const fn includes_start(self) -> bool {
        self.0 & Self::START.0 != 0
    }

    const fn includes_end(self) -> bool {
        self.0 & Self::END.0 != 0
    }
}

/// Glyphs selected by the incident directions of one line-network cell.
///
/// Every field must be exactly one printable, one-cell-wide character. Public
/// fields allow application-specific repertoires as well as the built-in ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineGlyphs {
    /// A point with no incident segment.
    pub isolated: char,
    /// A one-ended segment continuing upward.
    pub end_up: char,
    /// A one-ended segment continuing rightward.
    pub end_right: char,
    /// A one-ended segment continuing downward.
    pub end_down: char,
    /// A one-ended segment continuing leftward.
    pub end_left: char,
    /// A segment continuing upward and downward.
    pub vertical: char,
    /// A segment continuing leftward and rightward.
    pub horizontal: char,
    /// A corner continuing downward and rightward.
    pub corner_down_right: char,
    /// A corner continuing downward and leftward.
    pub corner_down_left: char,
    /// A corner continuing upward and rightward.
    pub corner_up_right: char,
    /// A corner continuing upward and leftward.
    pub corner_up_left: char,
    /// A tee continuing upward, downward, and rightward.
    pub tee_right: char,
    /// A tee continuing leftward, rightward, and downward.
    pub tee_down: char,
    /// A tee continuing upward, downward, and leftward.
    pub tee_left: char,
    /// A tee continuing leftward, rightward, and upward.
    pub tee_up: char,
    /// A four-way crossing.
    pub cross: char,
}

impl LineGlyphs {
    /// Standard square box-drawing characters.
    pub const NORMAL: Self = Self {
        isolated: '•',
        end_up: '│',
        end_right: '─',
        end_down: '│',
        end_left: '─',
        vertical: '│',
        horizontal: '─',
        corner_down_right: '┌',
        corner_down_left: '┐',
        corner_up_right: '└',
        corner_up_left: '┘',
        tee_right: '├',
        tee_down: '┬',
        tee_left: '┤',
        tee_up: '┴',
        cross: '┼',
    };

    /// Standard box drawing with rounded corners.
    pub const ROUNDED: Self = Self {
        corner_down_right: '╭',
        corner_down_left: '╮',
        corner_up_right: '╰',
        corner_up_left: '╯',
        ..Self::NORMAL
    };

    /// ASCII-only line drawing.
    pub const ASCII: Self = Self {
        isolated: '*',
        end_up: '|',
        end_right: '-',
        end_down: '|',
        end_left: '-',
        vertical: '|',
        horizontal: '-',
        corner_down_right: '+',
        corner_down_left: '+',
        corner_up_right: '+',
        corner_up_left: '+',
        tee_right: '+',
        tee_down: '+',
        tee_left: '+',
        tee_up: '+',
        cross: '+',
    };

    /// Heavy box-drawing characters.
    pub const THICK: Self = Self {
        isolated: '•',
        end_up: '┃',
        end_right: '━',
        end_down: '┃',
        end_left: '━',
        vertical: '┃',
        horizontal: '━',
        corner_down_right: '┏',
        corner_down_left: '┓',
        corner_up_right: '┗',
        corner_up_left: '┛',
        tee_right: '┣',
        tee_down: '┳',
        tee_left: '┫',
        tee_up: '┻',
        cross: '╋',
    };

    /// Double box-drawing characters.
    pub const DOUBLE: Self = Self {
        isolated: '•',
        end_up: '║',
        end_right: '═',
        end_down: '║',
        end_left: '═',
        vertical: '║',
        horizontal: '═',
        corner_down_right: '╔',
        corner_down_left: '╗',
        corner_up_right: '╚',
        corner_up_left: '╝',
        tee_right: '╠',
        tee_down: '╦',
        tee_left: '╣',
        tee_up: '╩',
        cross: '╬',
    };

    /// Invisible line drawing that still occupies its cells.
    pub const HIDDEN: Self = Self {
        isolated: ' ',
        end_up: ' ',
        end_right: ' ',
        end_down: ' ',
        end_left: ' ',
        vertical: ' ',
        horizontal: ' ',
        corner_down_right: ' ',
        corner_down_left: ' ',
        corner_up_right: ' ',
        corner_up_left: ' ',
        tee_right: ' ',
        tee_down: ' ',
        tee_left: ' ',
        tee_up: ' ',
        cross: ' ',
    };

    pub(super) fn glyph(self, connections: LineConnections) -> char {
        match connections.0 {
            0 => self.isolated,
            1 => self.end_up,
            2 => self.end_right,
            4 => self.end_down,
            8 => self.end_left,
            5 => self.vertical,
            10 => self.horizontal,
            6 => self.corner_down_right,
            12 => self.corner_down_left,
            3 => self.corner_up_right,
            9 => self.corner_up_left,
            7 => self.tee_right,
            14 => self.tee_down,
            13 => self.tee_left,
            11 => self.tee_up,
            _ => self.cross,
        }
    }

    fn validate(self) {
        for glyph in [
            self.isolated,
            self.end_up,
            self.end_right,
            self.end_down,
            self.end_left,
            self.vertical,
            self.horizontal,
            self.corner_down_right,
            self.corner_down_left,
            self.corner_up_right,
            self.corner_up_left,
            self.tee_right,
            self.tee_down,
            self.tee_left,
            self.tee_up,
            self.cross,
        ] {
            let mut encoded = [0; 4];
            let glyph = glyph.encode_utf8(&mut encoded);
            validate_cell_glyph(glyph);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Segment {
    from: Position,
    to: Position,
    axis: SegmentAxis,
    continuations: LineContinuations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentAxis {
    Horizontal,
    Vertical,
}

/// One connected drawing whose horizontal and vertical segments form junctions.
///
/// Unlike [`crate::CanvasContext::line`], a line network does not accept
/// diagonal segments or an arbitrary marker. Intersections are rendered as
/// corners, tees, and crossings selected from its [`LineGlyphs`]. Only
/// segments recorded in the same value contribute to those junctions;
/// separate Canvas commands combine through their recorded composition.
/// [`LineContinuations`] can add outward incidence at either endpoint without
/// adding a cell outside the segment's inclusive range.
///
/// ```
/// use urushi::{
///     CanvasContext, LineContinuations, LineGlyphs, LineNetwork, TextStyle,
/// };
///
/// fn draw(context: &mut CanvasContext) {
///     let mut network = LineNetwork::new(LineGlyphs::NORMAL, TextStyle::new());
///     network
///         .horizontal(1, 0..=4)
///         .vertical_with(2, 0..=2, LineContinuations::START);
///     context.line_network(network);
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineNetwork {
    segments: Vec<Segment>,
    glyphs: LineGlyphs,
    style: TextStyle,
}

impl LineNetwork {
    /// Creates an empty line network with one glyph repertoire and style.
    ///
    /// # Panics
    ///
    /// Panics if a repertoire character is not printable or does not occupy
    /// exactly one terminal cell.
    pub fn new(glyphs: LineGlyphs, style: TextStyle) -> Self {
        glyphs.validate();
        Self {
            segments: Vec::new(),
            glyphs,
            style,
        }
    }

    /// Adds a horizontal segment at `y`, including both endpoints.
    ///
    /// An empty range adds nothing.
    pub fn horizontal(&mut self, y: i64, columns: RangeInclusive<i64>) -> &mut Self {
        self.horizontal_with(y, columns, LineContinuations::NONE)
    }

    /// Adds a horizontal segment with endpoint continuation at `y`.
    ///
    /// `START` continues left from the first column and `END` continues right
    /// from the last column. Continuation changes endpoint incidence without
    /// drawing outside the inclusive range. An empty range adds nothing.
    pub fn horizontal_with(
        &mut self,
        y: i64,
        columns: RangeInclusive<i64>,
        continuations: LineContinuations,
    ) -> &mut Self {
        if columns.is_empty() {
            return self;
        }
        self.segments.push(Segment {
            from: Position::new(*columns.start(), y),
            to: Position::new(*columns.end(), y),
            axis: SegmentAxis::Horizontal,
            continuations,
        });
        self
    }

    /// Adds a vertical segment at `x`, including both endpoints.
    ///
    /// An empty range adds nothing.
    pub fn vertical(&mut self, x: i64, rows: RangeInclusive<i64>) -> &mut Self {
        self.vertical_with(x, rows, LineContinuations::NONE)
    }

    /// Adds a vertical segment with endpoint continuation at `x`.
    ///
    /// `START` continues up from the first row and `END` continues down from
    /// the last row. Continuation changes endpoint incidence without drawing
    /// outside the inclusive range. An empty range adds nothing.
    pub fn vertical_with(
        &mut self,
        x: i64,
        rows: RangeInclusive<i64>,
        continuations: LineContinuations,
    ) -> &mut Self {
        if rows.is_empty() {
            return self;
        }
        self.segments.push(Segment {
            from: Position::new(x, *rows.start()),
            to: Position::new(x, *rows.end()),
            axis: SegmentAxis::Vertical,
            continuations,
        });
        self
    }

    pub(super) fn rasterize(&self, size: Size) -> Vec<NetworkCell> {
        let mut cells = Vec::<NetworkCell>::new();
        let mut indexes = HashMap::<Position, usize>::new();
        for segment in &self.segments {
            if let Some((from, to)) = clip_line(segment.from, segment.to, size) {
                for position in line_points(from, to) {
                    let connections = segment_connections(position, *segment);
                    if let Some(index) = indexes.get(&position).copied() {
                        cells[index].connections = cells[index].connections.union(connections);
                    } else {
                        indexes.insert(position, cells.len());
                        cells.push(NetworkCell {
                            position,
                            connections,
                        });
                    }
                }
            }
        }
        cells
    }

    pub(super) const fn glyphs(&self) -> LineGlyphs {
        self.glyphs
    }

    pub(super) const fn style(&self) -> &TextStyle {
        &self.style
    }
}

pub(super) struct NetworkCell {
    pub(super) position: Position,
    pub(super) connections: LineConnections,
}

fn segment_connections(position: Position, segment: Segment) -> LineConnections {
    let (toward_start, toward_end) = match segment.axis {
        SegmentAxis::Horizontal => (LineConnections::LEFT, LineConnections::RIGHT),
        SegmentAxis::Vertical => (LineConnections::UP, LineConnections::DOWN),
    };
    let mut connections = LineConnections::default();
    if position != segment.from || segment.continuations.includes_start() {
        connections = connections.union(toward_start);
    }
    if position != segment.to || segment.continuations.includes_end() {
        connections = connections.union(toward_end);
    }
    connections
}
