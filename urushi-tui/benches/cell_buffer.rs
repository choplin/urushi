//! Compares the native cell buffer with the Ratatui 0.30 buffer it replaces.
//!
//! `diff` measures already-built committed and working frames. `frame` measures
//! the reusable-buffer path Screen will take: reset, write one complete frame,
//! then consume its diff. The cases cover ordinary and large terminal areas,
//! sparse and complete changes, and moved wide graphemes.

use std::fmt;

use divan::{AllocProfiler, Bencher, black_box};
use ratatui::{buffer::Buffer as RatatuiBuffer, layout::Rect};
use urushi::{Available, StyledGrapheme, TextStyle, View, resolve};
use urushi_terminal::TerminalSize;

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

fn main() {
    validate_change_counts();
    divan::main();
}

// The production module yields these cells to its terminal writer. Defining
// the boundary locally lets this benchmark compile the private implementation
// itself without making Buffer part of urushi-tui's public API.
mod terminal {
    use urushi_terminal::TerminalStyle;

    pub struct Cell<'a> {
        pub symbol: &'a str,
        pub style: TerminalStyle,
    }
}

#[expect(
    unused_imports,
    reason = "the included production module's unit tests are not registered by the benchmark harness"
)]
#[path = "../src/cell.rs"]
mod cell;

use cell::Buffer;

const SIZES: &[ScreenSize] = &[ScreenSize::Standard, ScreenSize::Large];
const CASES: &[DiffCase] = &[
    DiffCase::new(ScreenSize::Standard, Change::Unchanged),
    DiffCase::new(ScreenSize::Standard, Change::Sparse),
    DiffCase::new(ScreenSize::Standard, Change::Full),
    DiffCase::new(ScreenSize::Standard, Change::WideMove),
    DiffCase::new(ScreenSize::Large, Change::Unchanged),
    DiffCase::new(ScreenSize::Large, Change::Sparse),
    DiffCase::new(ScreenSize::Large, Change::Full),
    DiffCase::new(ScreenSize::Large, Change::WideMove),
];

#[derive(Clone, Copy)]
enum ScreenSize {
    Standard,
    Large,
}

impl ScreenSize {
    const fn dimensions(self) -> (usize, usize) {
        match self {
            Self::Standard => (80, 24),
            Self::Large => (200, 60),
        }
    }
}

impl fmt::Display for ScreenSize {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (columns, rows) = self.dimensions();
        write!(formatter, "{columns}x{rows}")
    }
}

#[derive(Clone, Copy)]
enum Change {
    Unchanged,
    Sparse,
    Full,
    WideMove,
}

impl fmt::Display for Change {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unchanged => "unchanged",
            Self::Sparse => "sparse-1-percent",
            Self::Full => "full",
            Self::WideMove => "wide-move",
        })
    }
}

#[derive(Clone, Copy)]
struct DiffCase {
    size: ScreenSize,
    change: Change,
}

impl DiffCase {
    const fn new(size: ScreenSize, change: Change) -> Self {
        Self { size, change }
    }
}

impl fmt::Display for DiffCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.size, self.change)
    }
}

struct Glyphs {
    a: StyledGrapheme,
    b: StyledGrapheme,
    wide: StyledGrapheme,
}

impl Glyphs {
    fn new() -> Self {
        Self {
            a: glyph("a"),
            b: glyph("b"),
            wide: glyph("界"),
        }
    }
}

fn glyph(symbol: &str) -> StyledGrapheme {
    let view = View::text(symbol, TextStyle::new());
    let resolved = resolve(&view, Available::size(2, 1)).expect("benchmark glyph resolves");
    resolved.rows()[0][0].clone()
}

#[divan::bench(args = CASES)]
fn native_diff(bencher: Bencher, case: DiffCase) {
    bencher
        .with_inputs(|| native_pair(case))
        .bench_refs(|(committed, working)| {
            black_box(
                committed
                    .diff(working)
                    .expect("benchmark bounds match")
                    .count(),
            )
        });
}

#[divan::bench(args = CASES)]
fn ratatui_diff(bencher: Bencher, case: DiffCase) {
    bencher
        .with_inputs(|| ratatui_pair(case))
        .bench_refs(|(committed, working)| black_box(committed.diff(working).len()));
}

#[divan::bench(args = SIZES)]
fn native_frame(bencher: Bencher, size: ScreenSize) {
    bencher
        .with_inputs(|| {
            let glyphs = Glyphs::new();
            let (columns, rows) = size.dimensions();
            let committed = native_filled(columns, rows, &glyphs.a);
            let working = Buffer::new(TerminalSize::new(columns, rows)).unwrap();
            (committed, working, glyphs)
        })
        .bench_refs(|(committed, working, glyphs)| {
            working.reset();
            fill_native(working, &glyphs.b);
            black_box(committed.diff(working).unwrap().count())
        });
}

#[divan::bench(args = SIZES)]
fn native_create(size: ScreenSize) -> Buffer {
    let (columns, rows) = size.dimensions();
    Buffer::new(TerminalSize::new(columns, rows)).unwrap()
}

#[divan::bench(args = SIZES)]
fn ratatui_frame(bencher: Bencher, size: ScreenSize) {
    bencher
        .with_inputs(|| {
            let (columns, rows) = size.dimensions();
            let committed = ratatui_filled(columns, rows, "a");
            let working = ratatui_empty(columns, rows);
            (committed, working)
        })
        .bench_refs(|(committed, working)| {
            working.reset();
            fill_ratatui(working, "b");
            black_box(committed.diff(working).len())
        });
}

#[divan::bench(args = SIZES)]
fn ratatui_create(size: ScreenSize) -> RatatuiBuffer {
    let (columns, rows) = size.dimensions();
    ratatui_empty(columns, rows)
}

fn validate_change_counts() {
    for case in CASES {
        let (native_committed, native_working) = native_pair(*case);
        let native_count = native_committed.diff(&native_working).unwrap().count();
        let (ratatui_committed, ratatui_working) = ratatui_pair(*case);
        let ratatui_count = ratatui_committed.diff(&ratatui_working).len();
        assert_eq!(
            native_count, ratatui_count,
            "{} must compare the same number of output cells",
            case
        );
    }
}

fn native_pair(case: DiffCase) -> (Buffer, Buffer) {
    let glyphs = Glyphs::new();
    let (columns, rows) = case.size.dimensions();
    match case.change {
        Change::Unchanged => {
            let committed = native_filled(columns, rows, &glyphs.a);
            let working = committed.clone();
            (committed, working)
        }
        Change::Sparse => {
            let committed = native_filled(columns, rows, &glyphs.a);
            let mut working = committed.clone();
            for index in (0..columns * rows).step_by(100) {
                working
                    .write(index % columns, index / columns, &glyphs.b)
                    .unwrap();
            }
            (committed, working)
        }
        Change::Full => (
            native_filled(columns, rows, &glyphs.a),
            native_filled(columns, rows, &glyphs.b),
        ),
        Change::WideMove => {
            let mut committed = Buffer::new(TerminalSize::new(columns, rows)).unwrap();
            let mut working = committed.clone();
            for row in 0..rows {
                for column in (0..columns.saturating_sub(2)).step_by(4) {
                    committed.write(column, row, &glyphs.wide).unwrap();
                    working.write(column + 1, row, &glyphs.wide).unwrap();
                }
            }
            (committed, working)
        }
    }
}

fn ratatui_pair(case: DiffCase) -> (RatatuiBuffer, RatatuiBuffer) {
    let (columns, rows) = case.size.dimensions();
    match case.change {
        Change::Unchanged => {
            let committed = ratatui_filled(columns, rows, "a");
            let working = committed.clone();
            (committed, working)
        }
        Change::Sparse => {
            let committed = ratatui_filled(columns, rows, "a");
            let mut working = committed.clone();
            for index in (0..columns * rows).step_by(100) {
                working[(index as u16 % columns as u16, index as u16 / columns as u16)]
                    .set_symbol("b");
            }
            (committed, working)
        }
        Change::Full => (
            ratatui_filled(columns, rows, "a"),
            ratatui_filled(columns, rows, "b"),
        ),
        Change::WideMove => {
            let mut committed = ratatui_empty(columns, rows);
            let mut working = committed.clone();
            for row in 0..rows as u16 {
                for column in (0..columns.saturating_sub(2)).step_by(4) {
                    committed[(column as u16, row)].set_symbol("界");
                    working[(column as u16 + 1, row)].set_symbol("界");
                }
            }
            (committed, working)
        }
    }
}

fn native_filled(columns: usize, rows: usize, glyph: &StyledGrapheme) -> Buffer {
    let mut buffer = Buffer::new(TerminalSize::new(columns, rows)).unwrap();
    for row in 0..rows {
        for column in 0..columns {
            buffer.write(column, row, glyph).unwrap();
        }
    }
    buffer
}

fn fill_native(buffer: &mut Buffer, glyph: &StyledGrapheme) {
    let size = buffer.size();
    for row in 0..size.rows() {
        for column in 0..size.columns() {
            buffer.write(column, row, glyph).unwrap();
        }
    }
}

fn ratatui_empty(columns: usize, rows: usize) -> RatatuiBuffer {
    RatatuiBuffer::empty(Rect::new(0, 0, columns as u16, rows as u16))
}

fn ratatui_filled(columns: usize, rows: usize, symbol: &str) -> RatatuiBuffer {
    let mut buffer = ratatui_empty(columns, rows);
    fill_ratatui(&mut buffer, symbol);
    buffer
}

fn fill_ratatui(buffer: &mut RatatuiBuffer, symbol: &str) {
    let area = buffer.area;
    for row in area.top()..area.bottom() {
        for column in area.left()..area.right() {
            buffer[(column, row)].set_symbol(symbol);
        }
    }
}
