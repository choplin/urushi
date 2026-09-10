use crate::{Grapheme, TextStyle};

use super::cell::validate_cell_glyph;
use super::{CellContribution, Position, PositionedCell};
use crate::view::geometry::Size;

#[derive(Debug, Clone)]
pub(super) struct CellPath {
    vertices: Vec<Position>,
    symbol: String,
    style: TextStyle,
}

impl CellPath {
    pub(super) fn line(from: Position, to: Position, symbol: &Grapheme, style: TextStyle) -> Self {
        validate_cell_glyph(symbol.as_str());
        Self {
            vertices: vec![from, to],
            symbol: symbol.as_str().to_owned(),
            style,
        }
    }

    pub(super) fn polyline(
        points: impl IntoIterator<Item = Position>,
        symbol: &Grapheme,
        style: TextStyle,
    ) -> Self {
        validate_cell_glyph(symbol.as_str());
        Self {
            vertices: points.into_iter().collect(),
            symbol: symbol.as_str().to_owned(),
            style,
        }
    }

    pub(super) fn rectangle(
        origin: Position,
        width: usize,
        height: usize,
        symbol: &Grapheme,
        style: TextStyle,
    ) -> Self {
        validate_cell_glyph(symbol.as_str());
        if width == 0 || height == 0 {
            return Self {
                vertices: Vec::new(),
                symbol: symbol.as_str().to_owned(),
                style,
            };
        }
        let right = origin
            .x
            .saturating_add(i64::try_from(width.saturating_sub(1)).unwrap_or(i64::MAX));
        let bottom = origin
            .y
            .saturating_add(i64::try_from(height.saturating_sub(1)).unwrap_or(i64::MAX));
        Self::polyline(
            [
                origin,
                Position::new(right, origin.y),
                Position::new(right, bottom),
                Position::new(origin.x, bottom),
                origin,
            ],
            symbol,
            style,
        )
    }

    pub(super) fn cells(&self, size: Size) -> Vec<PositionedCell> {
        let mut cells = Vec::new();
        let symbol = Grapheme::new(&self.symbol);
        for pair in self.vertices.windows(2) {
            if let Some((from, to)) = clip_line(pair[0], pair[1], size) {
                for position in line_points(from, to) {
                    if cells
                        .last()
                        .is_some_and(|cell: &PositionedCell| cell.position == position)
                    {
                        continue;
                    }
                    cells.push(PositionedCell::new(
                        position,
                        CellContribution::new()
                            .symbol(symbol)
                            .style(self.style.clone()),
                    ));
                }
            }
        }
        if self.vertices.len() == 1 && inside(self.vertices[0], size) {
            cells.push(PositionedCell::new(
                self.vertices[0],
                CellContribution::new()
                    .symbol(symbol)
                    .style(self.style.clone()),
            ));
        }
        cells
    }
}

pub(super) fn line_points(from: Position, to: Position) -> LinePoints {
    LinePoints {
        current: from,
        to,
        dx: (to.x - from.x).abs(),
        sx: if from.x < to.x { 1 } else { -1 },
        dy: -(to.y - from.y).abs(),
        sy: if from.y < to.y { 1 } else { -1 },
        error: (to.x - from.x).abs() - (to.y - from.y).abs(),
        finished: false,
    }
}

pub(super) struct LinePoints {
    current: Position,
    to: Position,
    dx: i64,
    sx: i64,
    dy: i64,
    sy: i64,
    error: i64,
    finished: bool,
}

impl Iterator for LinePoints {
    type Item = Position;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let point = self.current;
        if point == self.to {
            self.finished = true;
            return Some(point);
        }
        let twice = self.error.saturating_mul(2);
        if twice >= self.dy {
            self.error += self.dy;
            self.current.x += self.sx;
        }
        if twice <= self.dx {
            self.error += self.dx;
            self.current.y += self.sy;
        }
        Some(point)
    }
}

pub(super) fn inside(position: Position, size: Size) -> bool {
    position.x >= 0
        && position.y >= 0
        && usize::try_from(position.x).is_ok_and(|x| x < size.width())
        && usize::try_from(position.y).is_ok_and(|y| y < size.height())
}

pub(super) fn clip_line(
    mut from: Position,
    mut to: Position,
    size: Size,
) -> Option<(Position, Position)> {
    if size.is_empty() {
        return None;
    }
    let right = i64::try_from(size.width().saturating_sub(1)).unwrap_or(i64::MAX);
    let bottom = i64::try_from(size.height().saturating_sub(1)).unwrap_or(i64::MAX);
    loop {
        let a = out_code(from, right, bottom);
        let b = out_code(to, right, bottom);
        if a | b == 0 {
            return Some((from, to));
        }
        if a & b != 0 {
            return None;
        }
        let code = if a != 0 { a } else { b };
        let dx = i128::from(to.x) - i128::from(from.x);
        let dy = i128::from(to.y) - i128::from(from.y);
        let (x, y) = if code & 8 != 0 {
            (
                i128::from(from.x) + mul_div(dx, i128::from(bottom) - i128::from(from.y), dy)?,
                i128::from(bottom),
            )
        } else if code & 4 != 0 {
            (
                i128::from(from.x) + mul_div(dx, -i128::from(from.y), dy)?,
                0,
            )
        } else if code & 2 != 0 {
            (
                i128::from(right),
                i128::from(from.y) + mul_div(dy, i128::from(right) - i128::from(from.x), dx)?,
            )
        } else {
            (
                0,
                i128::from(from.y) + mul_div(dy, -i128::from(from.x), dx)?,
            )
        };
        let clipped = Position::new(i64::try_from(x).ok()?, i64::try_from(y).ok()?);
        if code == a {
            from = clipped;
        } else {
            to = clipped;
        }
    }
}

fn mul_div(left: i128, right: i128, divisor: i128) -> Option<i128> {
    if divisor == 0 {
        return None;
    }
    let negative = (left < 0) ^ (right < 0) ^ (divisor < 0);
    let magnitude = left.unsigned_abs().checked_mul(right.unsigned_abs())? / divisor.unsigned_abs();
    let magnitude = i128::try_from(magnitude).ok()?;
    Some(if negative { -magnitude } else { magnitude })
}

const fn out_code(position: Position, right: i64, bottom: i64) -> u8 {
    (if position.x < 0 {
        1
    } else if position.x > right {
        2
    } else {
        0
    }) | (if position.y < 0 {
        4
    } else if position.y > bottom {
        8
    } else {
        0
    })
}
