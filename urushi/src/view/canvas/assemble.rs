use crate::TextStyle;
use crate::text::Grapheme;

use super::{Canvas, CanvasCell, CellContribution, Composition, Position};
use crate::view::assemble::Rect;
use crate::view::geometry::Size;
use crate::view::resolve::{LayoutError, StyledGrapheme};

#[derive(Clone)]
enum Slot {
    Start(StyledGrapheme),
    Continuation(usize),
}

/// Rasterizes and composes commands into the Canvas's settled rectangle.
pub(in crate::view) fn compose_canvas(canvas: &Canvas, size: Size) -> Result<Rect, LayoutError> {
    let blank = StyledGrapheme::space(TextStyle::new());
    let mut slots = vec![vec![Slot::Start(blank.clone()); size.width()]; size.height()];
    let mut anchors = Vec::new();

    for command in canvas.draw(size).into_commands() {
        let command = command.rasterize(size)?;
        anchors.extend(
            command
                .anchors
                .into_iter()
                .map(|anchor| anchor.locate(size)),
        );
        for cell in command.cells {
            compose_cell(
                &mut slots,
                cell.position,
                &cell.contribution,
                command.composition,
            );
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

fn compose_cell(
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
    let owner = match &slots[y][x] {
        Slot::Start(_) => x,
        Slot::Continuation(owner) => *owner,
    };
    let existing = match &slots[y][owner] {
        Slot::Start(cell) => CanvasCell::new(Grapheme::new(cell.symbol()), cell.style().clone()),
        Slot::Continuation(_) => unreachable!("an owner always starts a grapheme"),
    };
    let next = match composition {
        Composition::Replace => replace_cell(contribution),
        Composition::Overlay => overlay_cell(existing, contribution),
        Composition::Custom(compose) => compose(&existing, contribution),
    };
    let write_x = if contribution.get_symbol().is_none() {
        owner
    } else {
        x
    };
    write_cell(slots, y, owner, write_x, next);
}

fn replace_cell(contribution: &CellContribution) -> CanvasCell {
    CanvasCell::new(
        Grapheme::new(contribution.get_symbol().unwrap_or(" ")),
        contribution.get_style().cloned().unwrap_or_default(),
    )
}

fn overlay_cell(existing: CanvasCell, contribution: &CellContribution) -> CanvasCell {
    let style = contribution
        .get_style()
        .map_or(existing.get_style().clone(), |style| {
            existing.get_style().clone().overlay(style)
        });
    let Some(symbol) = contribution.get_symbol() else {
        return existing.style(style);
    };
    CanvasCell::new(Grapheme::new(symbol), style)
}

fn write_cell(slots: &mut [Vec<Slot>], y: usize, owner: usize, write_x: usize, cell: CanvasCell) {
    let styled = StyledGrapheme::new(Grapheme::new(cell.get_symbol()), cell.get_style().clone());
    let width = styled.width();
    if width == 0
        || write_x
            .checked_add(width)
            .is_none_or(|right| right > slots[y].len())
    {
        return;
    }

    clear_owner(&mut slots[y], owner);
    for target in write_x..write_x + width {
        let target_owner = match &slots[y][target] {
            Slot::Start(_) => target,
            Slot::Continuation(owner) => *owner,
        };
        clear_owner(&mut slots[y], target_owner);
    }
    slots[y][write_x] = Slot::Start(styled);
    for slot in slots[y].iter_mut().skip(write_x + 1).take(width - 1) {
        *slot = Slot::Continuation(write_x);
    }
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
