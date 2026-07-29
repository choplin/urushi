//! ANSI- and CJK-aware helpers for composing rendered text blocks.

use crate::{Align, visible_width};

/// Vertical alignment of blocks joined side by side.
///
/// This is separate from [`Align`] because horizontal joins align block
/// heights, whereas [`Align`] aligns widths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VerticalAlign {
    #[default]
    Top,
    Center,
    Bottom,
}

/// Joins rendered blocks side by side.
///
/// Each line is padded to the maximum visible width of its own block. Shorter
/// blocks are placed at the top, center, or bottom of the tallest block.
/// ANSI escape sequences and East Asian wide characters are measured with
/// [`visible_width`].
pub fn join_horizontal<T: AsRef<str>>(align: VerticalAlign, blocks: &[T]) -> String {
    match blocks {
        [] => return String::new(),
        [block] => return block.as_ref().to_string(),
        _ => {}
    }

    let blocks: Vec<Vec<&str>> = blocks
        .iter()
        .map(|block| block.as_ref().split('\n').collect())
        .collect();
    let widths: Vec<usize> = blocks
        .iter()
        .map(|block| {
            block
                .iter()
                .map(|line| visible_width(line))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let max_height = blocks.iter().map(Vec::len).max().unwrap_or(0);

    let mut rows = Vec::with_capacity(max_height);
    for row in 0..max_height {
        let mut joined = String::new();
        for (block, width) in blocks.iter().zip(&widths) {
            let offset = vertical_offset(align, max_height - block.len());
            if let Some(line) = row.checked_sub(offset).and_then(|index| block.get(index)) {
                joined.push_str(line);
                joined.push_str(&" ".repeat(width.saturating_sub(visible_width(line))));
            } else {
                joined.push_str(&" ".repeat(*width));
            }
        }
        rows.push(joined);
    }
    rows.join("\n")
}

/// Stacks rendered blocks vertically.
///
/// Every line is padded to the maximum visible width of all blocks, using the
/// requested horizontal alignment. ANSI escape sequences and East Asian wide
/// characters are measured with [`visible_width`].
pub fn join_vertical<T: AsRef<str>>(align: Align, blocks: &[T]) -> String {
    match blocks {
        [] => return String::new(),
        [block] => return block.as_ref().to_string(),
        _ => {}
    }

    let blocks: Vec<Vec<&str>> = blocks
        .iter()
        .map(|block| block.as_ref().split('\n').collect())
        .collect();
    let width = blocks
        .iter()
        .flatten()
        .map(|line| visible_width(line))
        .max()
        .unwrap_or(0);

    let mut rows = Vec::new();
    for block in blocks {
        for line in block {
            let gap = width.saturating_sub(visible_width(line));
            let (left, right) = match align {
                Align::Left => (0, gap),
                Align::Center => (gap / 2, gap - gap / 2),
                Align::Right => (gap, 0),
            };
            rows.push(format!("{}{}{}", " ".repeat(left), line, " ".repeat(right)));
        }
    }
    rows.join("\n")
}

fn vertical_offset(align: VerticalAlign, gap: usize) -> usize {
    match align {
        VerticalAlign::Top => 0,
        // This matches lipgloss's center position: an odd extra row appears
        // above the shorter block.
        VerticalAlign::Center => gap.div_ceil(2),
        VerticalAlign::Bottom => gap,
    }
}
