//! Composition of rendered blocks.
//!
//! These compose output this crate did not lay out — text adopted with
//! [`RenderedBlock::from_ansi`] — or output destined straight for a writer.
//! Content that participates in layout is expressed as a [`View`](crate::View)
//! and resolved instead.

use crate::{Align, VerticalAlign};

use super::layout::Size;
use super::rendered::RenderedBlock;

/// Joins rendered blocks side by side.
///
/// Shorter blocks are placed at the top, center, or bottom of the tallest
/// block. Each block carries the size it was measured at, so no escape
/// sequence is measured again here.
pub fn join_horizontal(align: VerticalAlign, blocks: &[RenderedBlock]) -> RenderedBlock {
    match blocks {
        [] => return RenderedBlock::empty(),
        [block] => return block.clone(),
        _ => {}
    }

    let height = blocks
        .iter()
        .map(|block| block.size().height())
        .max()
        .unwrap_or(0);
    let width = blocks.iter().map(|block| block.size().width()).sum();

    let mut rows = vec![String::new(); height];
    for block in blocks {
        let block_rows = block.rows();
        let blank = " ".repeat(block.size().width());
        let offset = vertical_offset(align, height - block_rows.len());
        for (index, row) in rows.iter_mut().enumerate() {
            match index
                .checked_sub(offset)
                .and_then(|source| block_rows.get(source))
            {
                Some(source) => row.push_str(source),
                None => row.push_str(&blank),
            }
        }
    }

    RenderedBlock::measured(rows.join("\n"), Size::new(width, height))
}

/// Stacks rendered blocks vertically.
///
/// Every row is padded to the widest block using the requested horizontal
/// alignment. Each block carries the size it was measured at, so no escape
/// sequence is measured again here.
pub fn join_vertical(align: Align, blocks: &[RenderedBlock]) -> RenderedBlock {
    match blocks {
        [] => return RenderedBlock::empty(),
        [block] => return block.clone(),
        _ => {}
    }

    let width = blocks
        .iter()
        .map(|block| block.size().width())
        .max()
        .unwrap_or(0);

    let mut rows = Vec::new();
    for block in blocks {
        let gap = width - block.size().width();
        let (left, right) = match align {
            Align::Left => (0, gap),
            Align::Center => (gap / 2, gap - gap / 2),
            Align::Right => (gap, 0),
        };
        for row in block.rows() {
            rows.push(format!("{}{row}{}", " ".repeat(left), " ".repeat(right)));
        }
    }

    let height = rows.len();
    RenderedBlock::measured(rows.join("\n"), Size::new(width, height))
}

const fn vertical_offset(align: VerticalAlign, gap: usize) -> usize {
    match align {
        VerticalAlign::Top => 0,
        // This matches lipgloss's center position: an odd extra row appears
        // above the shorter block.
        VerticalAlign::Center => gap.div_ceil(2),
        VerticalAlign::Bottom => gap,
    }
}
