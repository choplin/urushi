//! How the layout pass scales.
//!
//! The point of these benchmarks is not an absolute number; it is the *shape*
//! of the curve. Resolution walks a tree, and the way it walks decides whether
//! a deep view costs linear or quadratic time — a distinction that does not
//! show up in a correctness test and that a refactor can silently lose. Each
//! case is therefore run at sizes that differ by a factor of four, so a
//! regression from linear reads directly off the ratio between the rows.
//!
//! The tree shapes are the ones the phases treat differently:
//!
//! - `blocks` nests one box in the next. This is where asking a subtree for its
//!   extent used to cost a walk of its own, making the width phase quadratic in
//!   depth.
//! - `columns` nests stacks, each with a sibling, under a height too small for
//!   them. This is the one place the height phase asks a subtree a second time,
//!   because a height demand is what fitting the content produced.
//! - `rows` nests the same structure on the other axis, where the width is
//!   divided instead.
//! - `siblings` widens a single row rather than deepening it, which is the
//!   distribution rule's own cost.
//! - `dashboard` is a view of the shape an application actually builds.
//! - `table` exercises intrinsic Canvas sizing and a dense presentation item,
//!   both as a retained View and when composed anew.
//!
//! `measure` is the sizing phases alone; `resolve` adds assembly, whose cost is
//! the area of the rectangle it produces rather than the size of the tree.

use divan::{Bencher, black_box};
use urushi::{
    Align, Available, BlockStyle, Border, Length, Overflow, Table, TablePresentation, TextStyle,
    VerticalAlign, View, measure, resolve,
};

fn main() {
    divan::main();
}

/// The sizes each shape is built at. Four apart, so the ratio between two rows
/// is the growth rate: linear reads as 4, quadratic as 16.
const SIZES: &[usize] = &[8, 32, 128];

/// The area `resolve` runs under: a terminal that cannot hold the deep cases,
/// so the deficit paths are exercised rather than skipped.
const AREA: Available = Available::new(Some(60), Some(30));

fn leaf() -> View {
    View::text("core text that wraps a little", TextStyle::new())
}

/// Boxes inside boxes, alternating a border and padding.
fn blocks(depth: usize) -> View {
    (0..depth).fold(leaf(), |inner, index| {
        View::block(
            if index % 2 == 0 {
                BlockStyle::new().border(Border::NORMAL)
            } else {
                BlockStyle::new().padding((0, 1))
            },
            inner,
        )
    })
}

/// Stacks inside stacks. The sibling is what gives the column something to
/// divide its height between.
fn columns(depth: usize) -> View {
    (0..depth).fold(leaf(), |inner, _| {
        View::column(Align::Left, [inner, leaf()])
    })
}

fn rows(depth: usize) -> View {
    (0..depth).fold(leaf(), |inner, _| {
        View::row(VerticalAlign::Top, [inner, leaf()])
    })
}

/// One row of many `Fill` children: width to divide, no depth.
fn siblings(count: usize) -> View {
    View::row(
        VerticalAlign::Top,
        (0..count)
            .map(|_| View::block(BlockStyle::new().width(Length::fill(1)), leaf()))
            .collect::<Vec<_>>(),
    )
}

/// A sidebar beside a body, capped and centred — the idiom the model documents.
fn dashboard(rows_of_body: usize) -> View {
    View::block(
        BlockStyle::new()
            .width(Length::fill(1))
            .align(Align::Center),
        View::block(
            BlockStyle::new().max_width(80),
            View::row(
                VerticalAlign::Top,
                [
                    View::block(
                        BlockStyle::new().width(20).border(Border::ROUNDED),
                        View::column(
                            Align::Left,
                            (0..rows_of_body)
                                .map(|index| View::text(format!("item {index}"), TextStyle::new()))
                                .collect::<Vec<_>>(),
                        ),
                    ),
                    View::block(
                        BlockStyle::new()
                            .width(Length::fill(1))
                            .border(Border::NORMAL)
                            .padding((0, 1))
                            .overflow(Overflow::ellipsis()),
                        View::column(
                            Align::Left,
                            (0..rows_of_body)
                                .map(|index| {
                                    View::text(
                                        format!(
                                            "a body line {index} long enough to wrap somewhere"
                                        ),
                                        TextStyle::new(),
                                    )
                                })
                                .collect::<Vec<_>>(),
                        ),
                    ),
                ],
            ),
        ),
    )
}

/// A wide-enough table to exercise per-cell wrapping and Canvas assembly.
fn table(rows: usize) -> View {
    let table = (0..rows).fold(Table::new(), |table, row| {
        table.row(
            (0..10)
                .map(|column| format!("cell {row}:{column} with content that wraps"))
                .collect::<Vec<_>>(),
        )
    });
    TablePresentation::new(BlockStyle::new(), BlockStyle::new(), TextStyle::new())
        .border(Border::NORMAL)
        .compose(&table)
}

/// The sizing phases alone: no rectangle is allocated.
#[divan::bench_group]
mod measuring {
    use super::{Bencher, SIZES, View, black_box, measure};

    fn run(bencher: Bencher, size: usize, build: fn(usize) -> View) {
        bencher
            .with_inputs(|| build(size))
            .bench_refs(|view| black_box(measure(view)));
    }

    #[divan::bench(args = SIZES)]
    fn blocks(bencher: Bencher, size: usize) {
        run(bencher, size, super::blocks);
    }

    #[divan::bench(args = SIZES)]
    fn columns(bencher: Bencher, size: usize) {
        run(bencher, size, super::columns);
    }

    #[divan::bench(args = SIZES)]
    fn rows(bencher: Bencher, size: usize) {
        run(bencher, size, super::rows);
    }

    #[divan::bench(args = SIZES)]
    fn siblings(bencher: Bencher, size: usize) {
        run(bencher, size, super::siblings);
    }

    #[divan::bench(args = SIZES)]
    fn dashboard(bencher: Bencher, size: usize) {
        run(bencher, size, super::dashboard);
    }
}

/// The whole pass, including the rectangle it builds.
#[divan::bench_group]
mod resolving {
    use super::{AREA, Bencher, SIZES, View, black_box, resolve};

    fn run(bencher: Bencher, size: usize, build: fn(usize) -> View) {
        bencher
            .with_inputs(|| build(size))
            .bench_refs(|view| black_box(resolve(view, AREA).size()));
    }

    #[divan::bench(args = SIZES)]
    fn blocks(bencher: Bencher, size: usize) {
        run(bencher, size, super::blocks);
    }

    #[divan::bench(args = SIZES)]
    fn columns(bencher: Bencher, size: usize) {
        run(bencher, size, super::columns);
    }

    #[divan::bench(args = SIZES)]
    fn rows(bencher: Bencher, size: usize) {
        run(bencher, size, super::rows);
    }

    #[divan::bench(args = SIZES)]
    fn siblings(bencher: Bencher, size: usize) {
        run(bencher, size, super::siblings);
    }

    #[divan::bench(args = SIZES)]
    fn dashboard(bencher: Bencher, size: usize) {
        run(bencher, size, super::dashboard);
    }

    #[divan::bench(args = SIZES)]
    fn table_warm(bencher: Bencher, size: usize) {
        run(bencher, size, super::table);
    }

    #[divan::bench(args = SIZES)]
    fn table_cold(bencher: Bencher, size: usize) {
        bencher.bench(|| {
            let view = super::table(black_box(size));
            black_box(resolve(&view, AREA).size())
        });
    }
}
