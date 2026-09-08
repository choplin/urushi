use crate::text::Grapheme;
use crate::{Available, Axis, Length, TextStyle, View};

use super::{Canvas, CanvasCell, CanvasCommand, CellContribution, Composition, Position};
use crate::view::assemble::Rect;
use crate::view::geometry::Size;
use crate::view::grid;
use crate::view::resolve::{LayoutError, ResolvedView, StyledGrapheme, try_resolve};

#[derive(Clone)]
enum Slot {
    Start(StyledGrapheme),
    Continuation(usize),
}

pub(in crate::view) fn canvas_rect(canvas: &Canvas, size: Size) -> Result<Rect, LayoutError> {
    let blank = StyledGrapheme::space(TextStyle::new());
    let mut slots = vec![vec![Slot::Start(blank.clone()); size.width()]; size.height()];
    let mut anchors = Vec::new();
    for command in canvas.draw(size).into_commands() {
        match command {
            CanvasCommand::View {
                origin,
                view,
                allocation,
                composition,
            } => {
                if allocation.0.is_none() && requires_allocation(&view, Axis::Width) {
                    return Err(LayoutError::missing_allocation(Axis::Width));
                }
                if allocation.1.is_none() && requires_allocation(&view, Axis::Height) {
                    return Err(LayoutError::missing_allocation(Axis::Height));
                }
                let resolved = try_resolve(&view, Available::new(allocation.0, allocation.1))?;
                for anchor in resolved.anchors() {
                    anchors.push(anchor.offset(origin.x, origin.y));
                }
                paint_resolved(&mut slots, origin, &resolved, composition);
            }
            CanvasCommand::Text {
                origin,
                text,
                style,
                composition,
            } => {
                let resolved = try_resolve(&View::text(text, style), Available::NONE)?;
                paint_resolved(&mut slots, origin, &resolved, composition);
            }
            CanvasCommand::Path { path, composition } => {
                for cell in path.cells(size) {
                    paint(&mut slots, cell.position, &cell.contribution, composition);
                }
            }
            CanvasCommand::Cells { cells, composition } => {
                for cell in cells {
                    paint(&mut slots, cell.position, &cell.contribution, composition);
                }
            }
        }
    }
    let rows = slots
        .into_iter()
        .map(|row| {
            row.into_iter()
                .filter_map(|slot| match slot {
                    Slot::Start(cell) => Some(cell),
                    Slot::Continuation(_) => None,
                })
                .collect()
        })
        .collect();
    Ok(Rect {
        width: size.width(),
        rows,
        anchors,
    })
}

fn requires_allocation(view: &View, axis: Axis) -> bool {
    match view {
        View::Text(..) => false,
        View::Canvas(canvas) => match axis {
            Axis::Width => canvas.explicit_width().is_none(),
            Axis::Height => canvas.explicit_height().is_none(),
        },
        View::Block(style, child) | View::AnchorBlock(_, style, child) => {
            let (length, maximum) = match axis {
                Axis::Width => (style.width_length(), style.maximum_width()),
                Axis::Height => (style.height_length(), style.maximum_height()),
            };
            match length {
                Some(Length::Fill(_)) => true,
                Some(Length::Cells(_)) => false,
                None if maximum.is_some() => false,
                None => requires_allocation(child, axis),
            }
        }
        View::Row(_, children) | View::Column(_, children) => children
            .iter()
            .any(|child| requires_allocation(child, axis)),
        View::Grid(style, rows) => match axis {
            Axis::Height => rows
                .iter()
                .flatten()
                .any(|child| requires_allocation(child, axis)),
            Axis::Width => {
                (0..grid::columns(rows)).any(|column| match style.column_length(column) {
                    Some(Length::Fill(_)) => true,
                    Some(Length::Cells(_)) => false,
                    None => (0..rows.len())
                        .map(|row| grid::cell(rows, row, column))
                        .any(|child| requires_allocation(child, axis)),
                })
            }
        },
    }
}

fn paint_resolved(
    slots: &mut [Vec<Slot>],
    origin: Position,
    resolved: &ResolvedView,
    composition: Composition,
) {
    for (y, row) in resolved.rows().iter().enumerate() {
        let mut x = 0usize;
        for grapheme in row {
            paint(
                slots,
                Position::new(
                    origin.x.saturating_add(x as i64),
                    origin.y.saturating_add(y as i64),
                ),
                &CellContribution::new()
                    .symbol(Grapheme::new(grapheme.symbol()))
                    .style(grapheme.style().clone()),
                composition,
            );
            x += grapheme.width();
        }
    }
}

fn paint(
    slots: &mut [Vec<Slot>],
    position: Position,
    contribution: &CellContribution,
    composition: Composition,
) {
    let (Ok(y), Ok(x)) = (usize::try_from(position.y), usize::try_from(position.x)) else {
        return;
    };
    if y >= slots.len() || x >= slots[y].len() {
        return;
    }
    let owner = match slots[y][x] {
        Slot::Start(_) => x,
        Slot::Continuation(owner) => owner,
    };
    let existing = match &slots[y][owner] {
        Slot::Start(cell) => CanvasCell::new(Grapheme::new(cell.symbol()), cell.style().clone()),
        Slot::Continuation(_) => unreachable!(),
    };
    let next = match composition {
        Composition::Replace => CanvasCell::new(
            Grapheme::new(contribution.symbol_value().unwrap_or(" ")),
            contribution.style_value().cloned().unwrap_or_default(),
        ),
        Composition::Overlay => {
            let symbol = contribution.symbol_value().unwrap_or(existing.symbol());
            let style = contribution
                .style_value()
                .map_or(existing.style().clone(), |style| {
                    overlay_style(existing.style().clone(), style)
                });
            CanvasCell::new(Grapheme::new(symbol), style)
        }
        Composition::Custom(compose) => compose(&existing, contribution),
    };
    let write_x = if contribution.symbol_value().is_none() {
        owner
    } else {
        x
    };
    let grapheme = Grapheme::new(next.symbol());
    let width = grapheme.width();
    if width == 0
        || write_x
            .checked_add(width)
            .is_none_or(|right| right > slots[y].len())
    {
        return;
    }
    clear_owner(&mut slots[y], owner);
    for target in write_x..write_x + width {
        let target_owner = match slots[y][target] {
            Slot::Start(_) => target,
            Slot::Continuation(owner) => owner,
        };
        clear_owner(&mut slots[y], target_owner);
    }
    slots[y][write_x] = Slot::Start(StyledGrapheme::new(grapheme, next.style().clone()));
    for slot in slots[y].iter_mut().skip(write_x + 1).take(width - 1) {
        *slot = Slot::Continuation(write_x);
    }
}

fn overlay_style(mut existing: TextStyle, contribution: &TextStyle) -> TextStyle {
    if let Some(color) = contribution.foreground_color() {
        existing = existing.foreground(color);
    }
    if let Some(color) = contribution.background_color() {
        existing = existing.background(color);
    }
    if let Some(underline) = contribution.underline_value() {
        existing = existing.add(underline);
    }
    existing.add(contribution.modifiers())
}

fn clear_owner(row: &mut [Slot], owner: usize) {
    let width = match &row[owner] {
        Slot::Start(cell) => cell.width(),
        Slot::Continuation(_) => return,
    };
    for slot in row.iter_mut().skip(owner).take(width) {
        *slot = Slot::Start(StyledGrapheme::space(TextStyle::new()));
    }
}
