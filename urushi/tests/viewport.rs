//! One-shot projection through the renderer-neutral Viewport node.

use urushi::{
    Align, Available, Axis, BlockStyle, Canvas, CanvasContext, CanvasItem, GridStyle,
    LayoutErrorKind, Length, Position, Projection, ProjectionBoundary, ResolvedView, Size,
    TextStyle, VerticalAlign, View, Viewport, resolve, try_measure,
};

fn text(content: &str) -> View {
    View::text(content, TextStyle::new())
}

fn resolve_ok(view: &View, available: Available) -> ResolvedView {
    resolve(view, available).unwrap()
}

fn rows(view: &View, available: Available) -> Vec<String> {
    resolve_ok(view, available)
        .rows()
        .iter()
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect()
}

fn preserve(origin: i64) -> Projection {
    Projection::new(origin, ProjectionBoundary::Preserve)
}

fn clamp(origin: i64) -> Projection {
    Projection::new(origin, ProjectionBoundary::Clamp)
}

#[test]
fn public_values_keep_the_selected_axes_origin_and_boundary() {
    let horizontal = preserve(-4);
    let vertical = clamp(7);
    let viewport = Viewport::both(horizontal, vertical);

    assert_eq!(horizontal.origin(), -4);
    assert_eq!(horizontal.boundary(), ProjectionBoundary::Preserve);
    assert_eq!(viewport.horizontal_projection(), Some(horizontal));
    assert_eq!(viewport.vertical_projection(), Some(vertical));
}

#[test]
fn preserve_uses_positive_negative_and_past_end_origins_exactly() {
    let projected = |origin| View::viewport(Viewport::horizontal(preserve(origin)), text("abcdef"));

    assert_eq!(rows(&projected(2), Available::columns(3)), ["cde"]);
    assert_eq!(rows(&projected(-2), Available::columns(4)), ["  ab"]);
    assert_eq!(rows(&projected(20), Available::columns(3)), ["   "]);
}

#[test]
fn clamp_limits_every_requested_origin_to_the_fillable_range() {
    for (origin, expected) in [
        (-9, "abcd"),
        (0, "abcd"),
        (1, "bcde"),
        (2, "cdef"),
        (20, "cdef"),
    ] {
        let view = View::viewport(Viewport::horizontal(clamp(origin)), text("abcdef"));
        assert_eq!(rows(&view, Available::columns(4)), [expected]);
    }

    let short = View::viewport(Viewport::horizontal(clamp(20)), text("ab"));
    assert_eq!(rows(&short, Available::columns(4)), ["ab  "]);
}

#[test]
fn vertical_projection_keeps_wrapping_at_the_finite_unprojected_width() {
    let view = View::viewport(Viewport::vertical(preserve(1)), text("abcdefgh"));

    assert_eq!(rows(&view, Available::size(4, 1)), ["efgh"]);
}

#[test]
fn horizontal_projection_selects_from_unwrapped_content() {
    let view = View::viewport(Viewport::horizontal(preserve(2)), text("abcdefgh"));

    assert_eq!(
        resolve_ok(&view, Available::size(4, 2)).size(),
        Size::new(4, 1)
    );
    assert_eq!(rows(&view, Available::size(4, 2)), ["cdef"]);
}

#[test]
fn both_axes_project_the_same_settled_child_rectangle() {
    let view = View::viewport(
        Viewport::both(preserve(1), preserve(1)),
        text("abcd\nefgh\nijkl"),
    );

    assert_eq!(rows(&view, Available::size(2, 2)), ["fg", "jk"]);
}

#[test]
fn projected_axes_require_a_finite_reference_after_allocation() {
    let horizontal = View::viewport(Viewport::horizontal(preserve(0)), text("abc"));
    let error = try_measure(&horizontal).unwrap_err();
    assert_eq!(error.kind(), LayoutErrorKind::ViewportExtent);
    assert_eq!(error.axis(), Axis::Width);

    let intrinsic_grid = View::grid(GridStyle::new(), [[horizontal.clone()]]);
    let error = try_measure(&intrinsic_grid).unwrap_err();
    assert_eq!(error.kind(), LayoutErrorKind::ViewportExtent);
    assert_eq!(error.axis(), Axis::Width);

    let supplied = View::block(
        BlockStyle::new().width(Length::Cells(2)),
        View::viewport(Viewport::horizontal(preserve(1)), text("abc")),
    );
    assert_eq!(try_measure(&supplied).unwrap(), Size::new(2, 1));
}

#[test]
fn auto_ancestors_and_grid_tracks_propagate_projection_dependencies() {
    let nested = View::block(
        BlockStyle::new(),
        View::column(
            Align::Left,
            [View::viewport(
                Viewport::horizontal(preserve(1)),
                text("abcd"),
            )],
        ),
    );
    assert_eq!(rows(&nested, Available::columns(2)), ["bc"]);

    let grid = View::grid(
        GridStyle::new(),
        [[View::viewport(
            Viewport::horizontal(preserve(1)),
            text("abcd"),
        )]],
    );
    assert_eq!(rows(&grid, Available::columns(3)), ["bcd"]);

    let fixed = View::grid(
        GridStyle::new().columns([Some(Length::Cells(2))]),
        [[View::viewport(
            Viewport::horizontal(preserve(1)),
            text("abcd"),
        )]],
    );
    assert_eq!(rows(&fixed, Available::columns(5)), ["bc"]);
}

#[test]
fn a_grid_row_with_vertical_projection_takes_a_fill_share() {
    let grid = View::grid(
        GridStyle::new(),
        [[View::viewport(Viewport::vertical(preserve(-1)), text("x"))]],
    );

    assert_eq!(
        resolve_ok(&grid, Available::size(1, 3)).size(),
        Size::new(1, 3)
    );
    assert_eq!(rows(&grid, Available::size(1, 3)), [" ", "x", " "]);
}

#[test]
fn row_column_and_explicit_grid_fill_shares_supply_viewport_references() {
    let row = View::row(
        VerticalAlign::Top,
        [
            View::viewport(Viewport::horizontal(preserve(1)), text("abcdef")),
            text("!"),
        ],
    );
    assert_eq!(rows(&row, Available::columns(4)), ["bcd!"]);

    let column = View::column(
        Align::Left,
        [
            View::viewport(Viewport::vertical(preserve(1)), text("a\nb\nc")),
            text("!"),
        ],
    );
    assert_eq!(rows(&column, Available::size(1, 3)), ["b", "c", "!"]);

    let grid = View::grid(
        GridStyle::new().columns([Some(Length::fill(1)), Some(Length::fill(2))]),
        [[
            View::viewport(Viewport::horizontal(preserve(1)), text("abcd")),
            text("x"),
        ]],
    );
    assert_eq!(rows(&grid, Available::columns(6)), ["bcx   "]);
}

#[derive(Debug, Clone, PartialEq)]
struct FullSurface;

impl CanvasItem for FullSurface {
    fn draw(&self, context: &mut CanvasContext) {
        assert_eq!(context.size(), Size::new(5, 1));
        context.text(Position::default(), "abcde", TextStyle::new());
    }
}

#[derive(Debug, Clone, PartialEq)]
struct AllocatedViewport;

impl CanvasItem for AllocatedViewport {
    fn draw(&self, context: &mut CanvasContext) {
        context.view(
            Position::default(),
            View::viewport(Viewport::horizontal(preserve(1)), text("abc")),
            Some(2),
            Some(1),
        );
    }
}

#[test]
fn canvas_keeps_full_surface_rasterization_and_can_allocate_a_viewport_command() {
    let projected = View::viewport(
        Viewport::horizontal(preserve(2)),
        View::block(
            BlockStyle::new()
                .width(Length::Cells(5))
                .height(Length::Cells(1)),
            View::canvas(Canvas::new().item(FullSurface)),
        ),
    );
    assert_eq!(rows(&projected, Available::columns(2)), ["cd"]);

    let canvas = View::canvas(
        Canvas::new()
            .extent(Size::new(2, 1))
            .item(AllocatedViewport),
    );
    assert_eq!(rows(&canvas, Available::NONE), ["bc"]);
}

#[test]
fn wide_graphemes_crossing_either_horizontal_edge_are_dropped_whole() {
    let left = View::viewport(Viewport::horizontal(preserve(2)), text("a界b"));
    let right = View::viewport(Viewport::horizontal(preserve(0)), text("a界b"));

    assert_eq!(rows(&left, Available::columns(2)), [" b"]);
    assert_eq!(rows(&right, Available::columns(2)), ["a "]);
}

#[test]
fn anchors_keep_logical_geometry_and_accumulated_visible_intersections() {
    let partial = View::viewport(
        Viewport::horizontal(preserve(2)),
        View::anchor_block(
            "region",
            BlockStyle::new()
                .width(Length::Cells(5))
                .height(Length::Cells(1)),
            View::empty(),
        ),
    );
    let resolved = resolve_ok(&partial, Available::columns(3));
    let anchor = resolved.anchor("region").unwrap();
    assert_eq!(
        (anchor.x(), anchor.y(), anchor.width(), anchor.height()),
        (-2, 0, 5, 1)
    );
    let visible = anchor.visible().unwrap();
    assert_eq!(
        (visible.x(), visible.y(), visible.width(), visible.height()),
        (0, 0, 3, 1)
    );
    assert!(!anchor.is_within_resolved_view());

    let cannot_resurrect = View::viewport(
        Viewport::horizontal(preserve(-2)),
        View::viewport(
            Viewport::horizontal(preserve(2)),
            View::anchor_block(
                "region",
                BlockStyle::new()
                    .width(Length::Cells(1))
                    .height(Length::Cells(1)),
                View::empty(),
            ),
        ),
    );
    let resolved = resolve_ok(&cannot_resurrect, Available::columns(1));
    let anchor = resolved.anchor("region").unwrap();
    assert_eq!(
        anchor.x(),
        0,
        "ancestor translation restores the logical coordinate"
    );
    assert_eq!(
        anchor.visible(),
        None,
        "an earlier local clip remains authoritative"
    );
}

#[test]
fn block_and_root_safety_clips_narrow_anchor_visibility_monotonically() {
    let clipped_by_block = View::block(
        BlockStyle::new().height(Length::Cells(1)),
        View::anchor_block(
            "region",
            BlockStyle::new().min_width(1).min_height(2),
            View::empty(),
        ),
    );
    let resolved = resolve_ok(&clipped_by_block, Available::NONE);
    let anchor = resolved.anchor("region").unwrap();
    assert_eq!((anchor.y(), anchor.height()), (0, 2));
    assert_eq!(anchor.visible().unwrap().height(), 1);

    let clipped_at_root = View::anchor_block(
        "region",
        BlockStyle::new().min_width(4).min_height(1),
        View::empty(),
    );
    let resolved = resolve_ok(&clipped_at_root, Available::columns(1));
    let anchor = resolved.anchor("region").unwrap();
    assert_eq!((anchor.x(), anchor.width()), (0, 4));
    assert_eq!(anchor.visible().unwrap().width(), 1);
}

#[test]
fn zero_sized_cursor_points_use_half_open_viewport_bounds() {
    let inside = View::viewport(
        Viewport::horizontal(preserve(0)),
        View::row(VerticalAlign::Top, [View::anchor("cursor"), text("x")]),
    );
    assert!(
        resolve_ok(&inside, Available::columns(1))
            .anchor("cursor")
            .unwrap()
            .visible()
            .is_some()
    );

    let edge = View::viewport(
        Viewport::horizontal(preserve(0)),
        View::row(VerticalAlign::Top, [text("x"), View::anchor("cursor")]),
    );
    assert_eq!(
        resolve_ok(&edge, Available::columns(1))
            .anchor("cursor")
            .unwrap()
            .visible(),
        None
    );
}
