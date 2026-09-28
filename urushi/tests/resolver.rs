//! Retained evaluation through the public Resolver boundary.

use std::sync::{Arc, Mutex};

use urushi::{
    Align, Available, BlockStyle, Canvas, CanvasContext, CanvasItem, GridStyle, Length, Position,
    Projection, ProjectionBoundary, Resolver, Size, TextStyle, View, Viewport, resolve,
};

#[derive(Debug, Clone)]
struct CountingPaint {
    content: &'static str,
    paints: Arc<Mutex<Vec<Size>>>,
}

impl PartialEq for CountingPaint {
    fn eq(&self, other: &Self) -> bool {
        self.content == other.content
    }
}

impl CanvasItem for CountingPaint {
    fn draw(&self, canvas: &mut CanvasContext) {
        self.paints.lock().unwrap().push(canvas.size());
        canvas.text(Position::new(0, 0), self.content, TextStyle::new());
    }
}

#[derive(Debug, Clone, PartialEq)]
struct MissingAllocation;

impl CanvasItem for MissingAllocation {
    fn draw(&self, canvas: &mut CanvasContext) {
        canvas.view(
            Position::default(),
            View::block(
                BlockStyle::new().width(Length::fill(1)),
                View::text("fill", TextStyle::new()),
            ),
            None,
            Some(1),
        );
    }
}

fn preserve(origin: i64) -> Projection {
    Projection::new(origin, ProjectionBoundary::Preserve)
}

fn symbols(view: &urushi::ResolvedView) -> Vec<String> {
    view.rows()
        .iter()
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect()
}

fn projected_canvas(origin: i64, paint: CountingPaint) -> View {
    let content = View::block(
        BlockStyle::new().width(6).height(1),
        View::canvas(Canvas::new().item(paint)),
    );
    View::viewport(Viewport::horizontal(preserve(origin)), content)
}

#[test]
fn viewport_reprojection_reuses_materialized_content_and_matches_one_shot() {
    let paints = Arc::new(Mutex::new(Vec::new()));
    let paint = CountingPaint {
        content: "abcdef",
        paints: Arc::clone(&paints),
    };
    let mut resolver = Resolver::new();

    for (origin, expected) in [(0, "abc"), (3, "def"), (0, "abc")] {
        let view = projected_canvas(origin, paint.clone());
        let retained = resolver.resolve(&view, Available::columns(3)).unwrap();
        let one_shot = resolve(&view, Available::columns(3)).unwrap();

        assert_eq!(retained, one_shot);
        assert_eq!(symbols(&retained), [expected]);
    }

    assert_eq!(
        *paints.lock().unwrap(),
        [
            Size::new(6, 1),
            Size::new(6, 1),
            Size::new(6, 1),
            Size::new(6, 1)
        ],
        "the retained path paints once; each one-shot oracle paints independently"
    );
}

#[test]
fn a_changed_sibling_keeps_an_independent_subtree_materialized() {
    let paints = Arc::new(Mutex::new(Vec::new()));
    let paint = CountingPaint {
        content: "abc",
        paints: Arc::clone(&paints),
    };
    let frame = |status| {
        View::row(
            urushi::VerticalAlign::Top,
            [
                View::canvas(Canvas::new().extent(Size::new(3, 1)).item(paint.clone())),
                View::text(status, TextStyle::new()),
            ],
        )
    };
    let mut resolver = Resolver::new();

    assert_eq!(
        symbols(&resolver.resolve(&frame("1"), Available::NONE).unwrap()),
        ["abc1"]
    );
    assert_eq!(
        symbols(&resolver.resolve(&frame("2"), Available::NONE).unwrap()),
        ["abc2"]
    );
    assert_eq!(*paints.lock().unwrap(), [Size::new(3, 1)]);
}

#[test]
fn column_and_grid_keep_unchanged_siblings_at_their_original_occurrence() {
    for layout in ["column", "grid"] {
        let paints = Arc::new(Mutex::new(Vec::new()));
        let paint = CountingPaint {
            content: "x",
            paints: Arc::clone(&paints),
        };
        let frame = |status| {
            let children = [
                View::canvas(Canvas::new().extent(Size::new(1, 1)).item(paint.clone())),
                View::text(status, TextStyle::new()),
            ];
            match layout {
                "column" => View::column(Align::Left, children),
                "grid" => View::grid(GridStyle::new(), [children]),
                _ => unreachable!(),
            }
        };
        let mut resolver = Resolver::new();

        let first = resolver.resolve(&frame("1"), Available::NONE).unwrap();
        let second = resolver.resolve(&frame("2"), Available::NONE).unwrap();

        assert_eq!(first, resolve(&frame("1"), Available::NONE).unwrap());
        assert_eq!(second, resolve(&frame("2"), Available::NONE).unwrap());
        assert_eq!(
            paints.lock().unwrap().len(),
            3,
            "the retained {layout} path paints once and each oracle paints once"
        );
    }
}

#[test]
fn changed_layout_inputs_invalidate_the_affected_surface() {
    let paints = Arc::new(Mutex::new(Vec::new()));
    let view = View::canvas(Canvas::new().height(1).item(CountingPaint {
        content: "abcdef",
        paints: Arc::clone(&paints),
    }));
    let mut resolver = Resolver::new();

    assert_eq!(
        symbols(&resolver.resolve(&view, Available::columns(3)).unwrap()),
        ["abc"]
    );
    assert_eq!(
        symbols(&resolver.resolve(&view, Available::columns(4)).unwrap()),
        ["abcd"]
    );
    assert_eq!(
        symbols(&resolver.resolve(&view, Available::columns(3)).unwrap()),
        ["abc"]
    );
    assert_eq!(
        *paints.lock().unwrap(),
        [Size::new(3, 1), Size::new(4, 1), Size::new(3, 1)]
    );
}

#[test]
fn clear_drops_retained_artifacts() {
    let paints = Arc::new(Mutex::new(Vec::new()));
    let view = View::canvas(Canvas::new().extent(Size::new(1, 1)).item(CountingPaint {
        content: "x",
        paints: Arc::clone(&paints),
    }));
    let mut resolver = Resolver::new();

    resolver.resolve(&view, Available::NONE).unwrap();
    resolver.resolve(&view, Available::NONE).unwrap();
    resolver.clear();
    resolver.resolve(&view, Available::NONE).unwrap();

    assert_eq!(*paints.lock().unwrap(), [Size::new(1, 1), Size::new(1, 1)]);
}

#[test]
fn assembly_error_preserves_the_last_successful_frame() {
    let paints = Arc::new(Mutex::new(Vec::new()));
    let paint = CountingPaint {
        content: "x",
        paints: Arc::clone(&paints),
    };
    let frame = |status| {
        View::row(
            urushi::VerticalAlign::Top,
            [
                View::canvas(Canvas::new().extent(Size::new(1, 1)).item(paint.clone())),
                View::text(status, TextStyle::new()),
            ],
        )
    };
    let failing = View::canvas(
        Canvas::new()
            .extent(Size::new(1, 1))
            .item(MissingAllocation),
    );
    let mut resolver = Resolver::new();

    resolver.resolve(&frame("1"), Available::NONE).unwrap();
    assert!(resolver.resolve(&failing, Available::NONE).is_err());
    assert_eq!(
        symbols(&resolver.resolve(&frame("2"), Available::NONE).unwrap()),
        ["x2"]
    );
    assert_eq!(*paints.lock().unwrap(), [Size::new(1, 1)]);
}

#[test]
fn duplicate_equal_subtrees_preserve_composition_order() {
    let paints = Arc::new(Mutex::new(Vec::new()));
    let canvas = Canvas::new().extent(Size::new(1, 1)).item(CountingPaint {
        content: "x",
        paints: Arc::clone(&paints),
    });
    let view = View::row(
        urushi::VerticalAlign::Top,
        [View::canvas(canvas.clone()), View::canvas(canvas)],
    );

    let retained = Resolver::new().resolve(&view, Available::NONE).unwrap();

    assert_eq!(symbols(&retained), ["xx"]);
    assert_eq!(retained, resolve(&view, Available::NONE).unwrap());
    assert_eq!(
        paints.lock().unwrap().len(),
        4,
        "both paths preserve the two distinct occurrences in tree order"
    );
}

#[test]
fn reordered_subtrees_do_not_reuse_a_different_occurrence() {
    let paints = Arc::new(Mutex::new(Vec::new()));
    let canvas = |content| {
        View::canvas(Canvas::new().extent(Size::new(1, 1)).item(CountingPaint {
            content,
            paints: Arc::clone(&paints),
        }))
    };
    let frame = |left, right| View::row(urushi::VerticalAlign::Top, [canvas(left), canvas(right)]);
    let mut resolver = Resolver::new();

    assert_eq!(
        symbols(&resolver.resolve(&frame("a", "b"), Available::NONE).unwrap()),
        ["ab"]
    );
    assert_eq!(
        symbols(&resolver.resolve(&frame("b", "a"), Available::NONE).unwrap()),
        ["ba"]
    );
}

#[test]
fn nested_viewports_keep_cells_and_anchor_visibility_equivalent() {
    let mut resolver = Resolver::new();
    for horizontal in [
        Projection::new(1, ProjectionBoundary::Preserve),
        Projection::new(20, ProjectionBoundary::Clamp),
    ] {
        let content = View::anchor_block(
            "content",
            BlockStyle::new().width(6).height(2),
            View::text("abcdef\nghijkl", TextStyle::new()),
        );
        let view = View::viewport(
            Viewport::vertical(preserve(1)),
            View::viewport(Viewport::horizontal(horizontal), content),
        );
        let available = Available::size(3, 1);

        assert_eq!(
            resolver.resolve(&view, available).unwrap(),
            resolve(&view, available).unwrap()
        );
    }
}

#[test]
fn reused_parent_keeps_descendant_artifacts_for_a_later_partial_change() {
    let paints = Arc::new(Mutex::new(Vec::new()));
    let paint = CountingPaint {
        content: "abc",
        paints: Arc::clone(&paints),
    };
    let frame = |origin, status| {
        let content = View::block(
            BlockStyle::new().width(4).height(1),
            View::row(
                urushi::VerticalAlign::Top,
                [
                    View::canvas(Canvas::new().extent(Size::new(3, 1)).item(paint.clone())),
                    View::text(status, TextStyle::new()),
                ],
            ),
        );
        View::viewport(Viewport::horizontal(preserve(origin)), content)
    };
    let mut resolver = Resolver::new();

    resolver
        .resolve(&frame(0, "1"), Available::columns(3))
        .unwrap();
    resolver
        .resolve(&frame(1, "1"), Available::columns(3))
        .unwrap();
    let changed = resolver
        .resolve(&frame(1, "2"), Available::columns(3))
        .unwrap();

    assert_eq!(symbols(&changed), ["bc2"]);
    assert_eq!(*paints.lock().unwrap(), [Size::new(3, 1)]);
}
