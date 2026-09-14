//! Anchors: the regions layout places and this crate does not fill.
//!
//! An anchor is a block plus a key, so these tests are about the one thing the
//! key adds — the reported rectangle — and about the promise that adding a key
//! moves nothing.

use urushi::{
    Align, AnchoredRect, Available, BlockStyle, Border, Key, Length, RenderSettings, ResolvedView,
    Size, TextStyle, VerticalAlign, View, render, resolve,
};

fn resolve_ok(view: &View, available: Available) -> ResolvedView {
    resolve(view, available).unwrap()
}

fn text(content: &str) -> View {
    View::text(content, TextStyle::new())
}

/// A view's cells, without styles, one string per row.
fn rows(view: &View, available: Available) -> Vec<String> {
    resolve_ok(view, available)
        .rows()
        .iter()
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect()
}

/// The one region `view` reports, as `(x, y, width, height)`.
fn region(view: &View, available: Available) -> (i64, i64, usize, usize) {
    let resolved = resolve_ok(view, available);
    assert_eq!(resolved.anchors().len(), 1, "exactly one anchor");
    let region = &resolved.anchors()[0];
    (region.x(), region.y(), region.width(), region.height())
}

#[test]
fn a_tree_without_an_anchor_reports_nothing() {
    let view = View::column(
        Align::Left,
        [
            text("hello"),
            View::block(BlockStyle::new().border(Border::NORMAL), text("world")),
        ],
    );

    assert!(resolve_ok(&view, Available::NONE).anchors().is_empty());
    assert!(
        resolve_ok(&view, Available::size(3, 2))
            .anchors()
            .is_empty(),
        "nor after a crop"
    );
}

#[test]
fn an_anchor_reports_the_cell_that_follows_the_text_it_sits_after() {
    let prompt = View::row(
        VerticalAlign::Top,
        [text("> "), View::anchor("cursor"), text("!")],
    );

    let (x, y, width, height) = region(&prompt, Available::NONE);
    assert_eq!((x, y), (2, 0), "the cell after the prompt");
    assert_eq!((width, height), (0, 0), "a cursor covers no cells");
    assert!(
        resolve_ok(&prompt, Available::NONE)
            .anchor("cursor")
            .expect("the anchor resolved")
            .is_within_resolved_view(),
        "an empty region one cell past the content is still within the view"
    );
}

#[test]
fn adding_a_key_moves_nothing() {
    let with = View::row(VerticalAlign::Top, [text("> "), View::anchor("cursor")]);
    let without = View::row(VerticalAlign::Top, [text("> ")]);

    for available in [
        Available::NONE,
        Available::columns(80),
        Available::size(80, 24),
    ] {
        assert_eq!(
            rows(&with, available),
            rows(&without, available),
            "an empty anchor is not a cell and not a row"
        );
    }
}

#[test]
fn a_sized_anchor_reports_the_rectangle_inside_its_frame() {
    let bare = View::anchor_block(
        "chart",
        BlockStyle::new()
            .width(Length::Cells(6))
            .height(Length::Cells(3)),
        View::empty(),
    );
    assert_eq!(region(&bare, Available::NONE), (0, 0, 6, 3));

    let framed = View::anchor_block(
        "chart",
        BlockStyle::new()
            .width(Length::Cells(6))
            .height(Length::Cells(3))
            .border(Border::NORMAL)
            .padding((0, 1))
            .margin((1, 2)),
        View::empty(),
    );
    assert_eq!(
        region(&framed, Available::NONE),
        (4, 2, 2, 1),
        "past the margin, inside the border and the padding"
    );
}

#[test]
fn an_anchor_stretches_with_the_box_that_carries_it() {
    let filling = View::anchor_block(
        "chart",
        BlockStyle::new()
            .width(Length::fill(1))
            .height(Length::fill(1))
            .border(Border::NORMAL),
        View::empty(),
    );
    let screen = View::column(Align::Left, [text("title"), filling]);

    assert_eq!(
        region(&screen, Available::size(20, 10)),
        (1, 2, 18, 7),
        "the fill takes what the title left, and the border takes one cell of it"
    );
}

#[test]
fn a_row_and_a_column_place_the_anchors_inside_them() {
    let boxed = |key: &'static str| {
        View::anchor_block(
            key,
            BlockStyle::new()
                .width(Length::Cells(2))
                .height(Length::Cells(1)),
            View::empty(),
        )
    };

    let row = View::row(VerticalAlign::Bottom, [text("a\nb\nc"), boxed("k")]);
    assert_eq!(
        region(&row, Available::NONE),
        (1, 2, 2, 1),
        "after the text, and pushed to the bottom of the tallest child"
    );

    let column = View::column(Align::Right, [text("hello"), boxed("k")]);
    assert_eq!(
        region(&column, Available::NONE),
        (3, 1, 2, 1),
        "below the text, and pushed to the right of the widest child"
    );
}

#[test]
fn a_box_is_reported_before_what_it_encloses() {
    let cell = |key: &'static str| {
        View::anchor_block(
            key,
            BlockStyle::new()
                .width(Length::Cells(1))
                .height(Length::Cells(1)),
            View::empty(),
        )
    };
    let view = View::anchor_block(
        "outer",
        BlockStyle::new().border(Border::NORMAL),
        View::row(VerticalAlign::Top, [cell("left"), cell("right")]),
    );

    let resolved = resolve_ok(&view, Available::NONE);
    let reported: Vec<(Key, i64)> = resolved
        .anchors()
        .iter()
        .map(|region| (region.key(), region.x()))
        .collect();

    assert_eq!(
        reported,
        vec![
            (Key::from("outer"), 1),
            (Key::from("left"), 1),
            (Key::from("right"), 2)
        ]
    );
    assert_eq!(resolved.anchor("right").map(AnchoredRect::x), Some(2));
    assert!(resolved.anchor("absent").is_none());
}

#[test]
#[should_panic(expected = "a key names one region")]
fn one_key_names_one_region() {
    let view = View::row(
        VerticalAlign::Top,
        [View::anchor("cursor"), View::anchor("cursor")],
    );

    resolve_ok(&view, Available::NONE);
}

#[test]
fn a_bounded_area_reshapes_the_box_and_the_region_with_it() {
    let view = View::anchor_block(
        "chart",
        BlockStyle::new()
            .width(Length::Cells(20))
            .height(Length::Cells(8)),
        View::empty(),
    );

    let resolved = resolve_ok(&view, Available::size(5, 2));
    let region = &resolved.anchors()[0];

    assert_eq!(resolved.size(), Size::new(5, 2));
    assert_eq!(
        (region.x(), region.y(), region.width(), region.height()),
        (0, 0, 5, 2),
        "the area reshaped the box, so the region is what the box became"
    );
    assert!(region.is_within_resolved_view());
}

#[test]
fn a_cursor_below_the_view_is_reported_below_it_and_says_so() {
    let view = View::column(
        Align::Left,
        [
            View::text("a\nb\nc", TextStyle::new()),
            View::anchor("cursor"),
        ],
    );

    let resolved = resolve_ok(&view, Available::size(1, 2));
    let cursor = resolved.anchor("cursor").expect("the anchor resolved");

    assert_eq!(resolved.size().height(), 2, "the area bounded the rows");
    assert_eq!(
        cursor.y(),
        3,
        "the cursor sits below them, which the caller needs to know rather than \
         have rounded into the view"
    );
    assert!(!cursor.is_within_resolved_view());
}

#[test]
fn an_anchor_resolves_to_the_blanks_a_backend_without_one_draws() {
    let view = View::anchor_block(
        "chart",
        BlockStyle::new()
            .width(Length::Cells(4))
            .height(Length::Cells(2))
            .border(Border::NORMAL),
        View::empty(),
    );

    assert_eq!(
        render(
            &resolve_ok(&view, Available::NONE),
            &RenderSettings::default()
        ),
        "┌──┐\n└──┘",
        "the region is blank, and the frame around it is not"
    );
    assert_eq!(
        render(
            &resolve_ok(&View::anchor("cursor"), Available::NONE),
            &RenderSettings::default()
        ),
        "",
        "an empty anchor draws nothing at all"
    );
}
