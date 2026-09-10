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

    const fn from_step(x: i64, y: i64) -> Self {
        match (x, y) {
            (0, -1) => Self::UP,
            (1, 0) => Self::RIGHT,
            (0, 1) => Self::DOWN,
            (-1, 0) => Self::LEFT,
            _ => Self(0),
        }
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
}

/// One connected drawing whose horizontal and vertical segments form junctions.
///
/// Unlike [`crate::CanvasContext::line`], a line network does not accept
/// diagonal segments or an arbitrary marker. Intersections are rendered as
/// corners, tees, and crossings selected from its [`LineGlyphs`]. Only
/// segments recorded in the same value contribute to those junctions;
/// separate Canvas commands combine through their recorded composition.
///
/// ```
/// use urushi::{CanvasContext, LineGlyphs, LineNetwork, TextStyle};
///
/// fn draw(context: &mut CanvasContext) {
///     let mut network = LineNetwork::new(LineGlyphs::NORMAL, TextStyle::new());
///     network.horizontal(1, 0..=4).vertical(2, 0..=2);
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
        if columns.is_empty() {
            return self;
        }
        self.segments.push(Segment {
            from: Position::new(*columns.start(), y),
            to: Position::new(*columns.end(), y),
        });
        self
    }

    /// Adds a vertical segment at `x`, including both endpoints.
    ///
    /// An empty range adds nothing.
    pub fn vertical(&mut self, x: i64, rows: RangeInclusive<i64>) -> &mut Self {
        if rows.is_empty() {
            return self;
        }
        self.segments.push(Segment {
            from: Position::new(x, *rows.start()),
            to: Position::new(x, *rows.end()),
        });
        self
    }

    pub(super) fn rasterize(&self, size: Size) -> Vec<NetworkCell> {
        let mut cells = Vec::<NetworkCell>::new();
        let mut indexes = HashMap::<Position, usize>::new();
        for segment in &self.segments {
            if let Some((from, to)) = clip_line(segment.from, segment.to, size) {
                for position in line_points(from, to) {
                    let connections = segment_connections(position, segment.from, segment.to);
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

fn segment_connections(
    position: Position,
    original_from: Position,
    original_to: Position,
) -> LineConnections {
    if original_from == original_to {
        return LineConnections::default();
    }
    let step = Position::new(
        match original_to.x.cmp(&original_from.x) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        },
        match original_to.y.cmp(&original_from.y) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        },
    );
    let mut connections = LineConnections::default();
    if position != original_from {
        connections = connections.union(LineConnections::from_step(-step.x, -step.y));
    }
    if position != original_to {
        connections = connections.union(LineConnections::from_step(step.x, step.y));
    }
    connections
}
