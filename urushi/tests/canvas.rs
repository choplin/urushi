use urushi::{
    Available, Axis, BlockStyle, Canvas, CanvasCell, CanvasContext, CanvasItem, CanvasSizing,
    CellContribution, Composition, Grapheme, LayoutErrorKind, Length, LineContinuations,
    LineGlyphs, LineNetwork, Position, PositionedCell, ResolvedView, Size, TextStyle, View,
    resolve,
};

fn resolve_ok(view: &View, available: Available) -> ResolvedView {
    resolve(view, available).unwrap()
}

#[derive(Debug, Clone, PartialEq)]
struct Scene {
    label: &'static str,
}

impl CanvasItem for Scene {
    fn draw(&self, context: &mut CanvasContext) {
        assert_eq!(context.bounds(), (Position::new(0, 0), Size::new(8, 4)));
        context.line(
            Position::new(-2, 1),
            Position::new(6, 1),
            Grapheme::new("─"),
            TextStyle::new(),
        );
        context.text(Position::new(1, 1), self.label, TextStyle::new());
        context.cells([PositionedCell::new(
            Position::new(7, 3),
            CellContribution::new().symbol(Grapheme::new("x")),
        )]);
    }
}

fn lines(view: &View, available: Available) -> Vec<String> {
    resolve_ok(view, available)
        .rows()
        .iter()
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect()
}

#[test]
fn items_are_owned_comparable_values_and_draw_after_size_is_final() {
    let first = View::canvas(Canvas::new().item(Scene { label: "node" }));
    let same = first.clone();
    let different = View::canvas(Canvas::new().item(Scene { label: "other" }));

    assert_eq!(first, same);
    assert_ne!(first, different);
    assert_eq!(
        lines(&first, Available::size(8, 4)),
        ["        ", "─node── ", "        ", "       x"]
    );
}

#[test]
fn viewport_sizing_is_an_explicit_comparable_canvas_value() {
    let implicit = Canvas::new().extent(Size::new(4, 2));
    let explicit = Canvas::new()
        .sizing(CanvasSizing::viewport())
        .extent(Size::new(4, 2));

    assert_eq!(implicit, explicit);
    assert_ne!(explicit, Canvas::new().extent(Size::new(5, 2)));
}

#[derive(Debug, Clone, PartialEq)]
struct Layered;

fn union(existing: &CanvasCell, contribution: &CellContribution) -> CanvasCell {
    if existing.symbol() == "a" && contribution.symbol_value() == Some("b") {
        existing.clone().with_symbol(Grapheme::new("c"))
    } else if let Some(symbol) = contribution.symbol_value() {
        existing.clone().with_symbol(Grapheme::new(symbol))
    } else {
        existing.clone()
    }
}

impl CanvasItem for Layered {
    fn draw(&self, context: &mut CanvasContext) {
        context.cells([PositionedCell::new(
            Position::new(0, 0),
            CellContribution::new().symbol(Grapheme::new("a")),
        )]);
        context.cells_with(
            [PositionedCell::new(
                Position::new(0, 0),
                CellContribution::new().symbol(Grapheme::new("b")),
            )],
            Composition::Custom(union),
        );
        context.view(
            Position::new(2, 0),
            View::text("界", TextStyle::new()),
            None,
            None,
        );
        context.text(Position::new(3, 0), "z", TextStyle::new());
    }
}

#[test]
fn custom_composition_is_ordered_and_wide_graphemes_are_never_split() {
    let view = View::canvas(Canvas::new().extent(Size::new(5, 1)).item(Layered));
    assert_eq!(lines(&view, Available::NONE), ["c  z "]);
}

#[derive(Debug, Clone, PartialEq)]
struct FullRowReplace;

impl CanvasItem for FullRowReplace {
    fn draw(&self, context: &mut CanvasContext) {
        context.view(
            Position::new(0, 0),
            View::text("界x", TextStyle::new().bold()),
            None,
            None,
        );
        context.text_with(
            Position::new(0, 0),
            "abcd",
            TextStyle::new().foreground(urushi::Color::RED),
            Composition::Replace,
        );
    }
}

#[test]
fn a_full_row_text_replace_clears_prior_wide_content() {
    let resolved = resolve_ok(
        &View::canvas(Canvas::new().extent(Size::new(4, 1)).item(FullRowReplace)),
        Available::NONE,
    );

    assert_eq!(
        resolved.rows()[0]
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>(),
        "abcd"
    );
    assert!(resolved.rows()[0].iter().all(|cell| {
        cell.style().foreground_color() == Some(urushi::Color::RED)
            && cell.style().modifiers().is_empty()
    }));
}

#[test]
#[should_panic]
fn canvas_symbol_boundaries_reject_terminal_control_sequences() {
    let symbol = Grapheme::new("\u{1b}]8;;https://example.invalid\u{7}");
    let _ = CellContribution::new().symbol(symbol);
}

#[derive(Debug, Clone, PartialEq)]
struct InvalidMarker(&'static str);

impl CanvasItem for InvalidMarker {
    fn draw(&self, context: &mut CanvasContext) {
        context.line(
            Position::new(0, 0),
            Position::new(1, 0),
            Grapheme::new(self.0),
            TextStyle::new(),
        );
    }
}

#[test]
#[should_panic(expected = "exactly one terminal cell")]
fn cell_lines_reject_zero_width_markers() {
    let view = View::canvas(
        Canvas::new()
            .extent(Size::new(2, 1))
            .item(InvalidMarker("\u{301}")),
    );
    let _ = resolve_ok(&view, Available::NONE);
}

#[test]
#[should_panic(expected = "exactly one terminal cell")]
fn cell_lines_reject_wide_markers() {
    let view = View::canvas(
        Canvas::new()
            .extent(Size::new(2, 1))
            .item(InvalidMarker("界")),
    );
    let _ = resolve_ok(&view, Available::NONE);
}

#[test]
#[should_panic(expected = "exactly one terminal cell")]
fn line_network_rejects_zero_width_glyphs() {
    let _ = LineNetwork::new(
        LineGlyphs {
            isolated: '\u{301}',
            ..LineGlyphs::NORMAL
        },
        TextStyle::new(),
    );
}

#[test]
#[should_panic(expected = "exactly one terminal cell")]
fn line_network_rejects_wide_glyphs() {
    let _ = LineNetwork::new(
        LineGlyphs {
            isolated: '界',
            ..LineGlyphs::NORMAL
        },
        TextStyle::new(),
    );
}

#[derive(Debug, Clone, PartialEq)]
struct ZeroWidthFullRowAfterNetwork;

impl CanvasItem for ZeroWidthFullRowAfterNetwork {
    fn draw(&self, context: &mut CanvasContext) {
        let mut network = LineNetwork::new(LineGlyphs::NORMAL, TextStyle::new());
        network.horizontal(0, 0..=0);
        context.line_network(network);
        context.text_with(
            Position::new(0, 0),
            "\u{301}a",
            TextStyle::new(),
            Composition::Replace,
        );
        context.text(Position::new(1, 0), "x", TextStyle::new());
    }
}

#[test]
fn zero_width_full_row_text_stays_within_canvas_bounds() {
    let view = View::canvas(
        Canvas::new()
            .extent(Size::new(1, 1))
            .item(ZeroWidthFullRowAfterNetwork),
    );

    assert_eq!(lines(&view, Available::NONE), ["a"]);
}

#[derive(Debug, Clone, PartialEq)]
struct Anchored;

impl CanvasItem for Anchored {
    fn draw(&self, context: &mut CanvasContext) {
        context.view(
            Position::new(-2, 1),
            View::anchor_block(
                "node",
                BlockStyle::new()
                    .width(Length::Cells(4))
                    .height(Length::Cells(2)),
                View::text("ok", TextStyle::new()),
            ),
            None,
            None,
        );
    }
}

#[test]
fn clipping_keeps_signed_anchor_geometry() {
    let resolved = resolve_ok(
        &View::canvas(Canvas::new().extent(Size::new(4, 3)).item(Anchored)),
        Available::NONE,
    );
    let anchor = resolved.anchor("node").unwrap();
    assert_eq!(
        (anchor.x(), anchor.y(), anchor.width(), anchor.height()),
        (-2, 1, 4, 2)
    );
    assert!(!anchor.is_within_resolved_view());
}

#[derive(Debug, Clone, PartialEq)]
struct MissingAllocation;

impl CanvasItem for MissingAllocation {
    fn draw(&self, context: &mut CanvasContext) {
        context.view(
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

#[derive(Debug, Clone, PartialEq)]
struct NestedMissingAllocation;

impl CanvasItem for NestedMissingAllocation {
    fn draw(&self, context: &mut CanvasContext) {
        context.view(
            Position::default(),
            View::row(
                urushi::VerticalAlign::Top,
                [View::block(
                    BlockStyle::new().width(Length::fill(1)),
                    View::text("fill", TextStyle::new()),
                )],
            ),
            None,
            Some(1),
        );
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CappedMissingAllocation;

impl CanvasItem for CappedMissingAllocation {
    fn draw(&self, context: &mut CanvasContext) {
        context.view(
            Position::default(),
            View::block(
                BlockStyle::new().width(Length::fill(1)).max_width(4),
                View::text("fill", TextStyle::new()),
            ),
            None,
            Some(1),
        );
    }
}

#[test]
fn unbounded_canvas_and_fill_view_commands_report_the_missing_axis() {
    let extent = resolve(&View::canvas(Canvas::new()), Available::NONE).unwrap_err();
    assert_eq!(extent.kind(), LayoutErrorKind::CanvasExtent);
    assert_eq!(extent.axis(), Axis::Width);

    let allocation = resolve(
        &View::canvas(
            Canvas::new()
                .extent(Size::new(8, 2))
                .item(MissingAllocation),
        ),
        Available::NONE,
    )
    .unwrap_err();
    assert_eq!(allocation.kind(), LayoutErrorKind::ViewAllocation);
    assert_eq!(allocation.axis(), Axis::Width);

    let nested = resolve(
        &View::canvas(
            Canvas::new()
                .extent(Size::new(8, 2))
                .item(NestedMissingAllocation),
        ),
        Available::NONE,
    )
    .unwrap_err();
    assert_eq!(nested.kind(), LayoutErrorKind::ViewAllocation);
    assert_eq!(nested.axis(), Axis::Width);

    let capped = resolve(
        &View::canvas(
            Canvas::new()
                .extent(Size::new(8, 2))
                .item(CappedMissingAllocation),
        ),
        Available::NONE,
    )
    .unwrap_err();
    assert_eq!(capped.kind(), LayoutErrorKind::ViewAllocation);
    assert_eq!(capped.axis(), Axis::Width);
}

#[derive(Debug, Clone, PartialEq)]
struct Dot;

impl CanvasItem for Dot {
    fn draw(&self, context: &mut CanvasContext) {
        context.cells([
            PositionedCell::new(
                Position::new(-1, 0),
                CellContribution::new().symbol(Grapheme::new("l")),
            ),
            PositionedCell::new(
                Position::new(0, -1),
                CellContribution::new().symbol(Grapheme::new("t")),
            ),
            PositionedCell::new(
                Position::new(3, 0),
                CellContribution::new().symbol(Grapheme::new("r")),
            ),
            PositionedCell::new(
                Position::new(0, 2),
                CellContribution::new().symbol(Grapheme::new("b")),
            ),
            PositionedCell::new(
                Position::new(1, 1),
                CellContribution::new().symbol(Grapheme::new("x")),
            ),
        ]);
    }
}

#[test]
fn nested_layout_allocates_canvas_and_clips_all_four_edges() {
    let canvas = View::block(
        BlockStyle::new()
            .width(Length::fill(1))
            .height(Length::fill(1)),
        View::canvas(Canvas::new().extent(Size::new(3, 2)).item(Dot)),
    );
    let nested = View::block(
        BlockStyle::new()
            .width(Length::Cells(3))
            .height(Length::Cells(2)),
        View::column(
            urushi::Align::Left,
            [View::row(urushi::VerticalAlign::Top, [canvas])],
        ),
    );
    assert_eq!(lines(&nested, Available::NONE), ["   ", " x "]);

    let grid = View::grid(
        urushi::GridStyle::new(),
        [[View::canvas(
            Canvas::new().extent(Size::new(3, 2)).item(Dot),
        )]],
    );
    assert_eq!(lines(&grid, Available::size(3, 2)), ["   ", " x "]);
}

#[derive(Debug, Clone, PartialEq)]
struct StyleContinuation;

impl CanvasItem for StyleContinuation {
    fn draw(&self, context: &mut CanvasContext) {
        context.cells([PositionedCell::new(
            Position::new(0, 0),
            CellContribution::new().symbol(Grapheme::new("界")),
        )]);
        context.cells([PositionedCell::new(
            Position::new(1, 0),
            CellContribution::new().style(TextStyle::new().bold()),
        )]);
    }
}

#[test]
fn style_only_overlay_on_a_continuation_updates_the_wide_grapheme_in_place() {
    let resolved = resolve_ok(
        &View::canvas(
            Canvas::new()
                .extent(Size::new(2, 1))
                .item(StyleContinuation),
        ),
        Available::NONE,
    );
    assert_eq!(resolved.rows()[0][0].symbol(), "界");
    assert_eq!(resolved.rows()[0][0].width(), 2);
    assert!(
        resolved.rows()[0][0]
            .style()
            .modifiers()
            .contains(urushi::Modifier::BOLD)
    );
}

#[test]
fn intrinsic_grid_share_does_not_masquerade_as_a_finite_parent_allocation() {
    let view = View::grid(
        urushi::GridStyle::new(),
        [[View::canvas(Canvas::new().height(1))]],
    );
    assert_eq!(
        resolve(&view, Available::NONE).unwrap_err().axis(),
        Axis::Width
    );
}

#[test]
fn finite_column_and_grid_shares_are_canvas_allocations() {
    let column = View::column(urushi::Align::Left, [View::canvas(Canvas::new().width(2))]);
    assert_eq!(
        resolve_ok(&column, Available::size(2, 3)).size(),
        Size::new(2, 3)
    );

    let grid = View::grid(
        urushi::GridStyle::new(),
        [[View::canvas(Canvas::new().width(2))]],
    );
    assert_eq!(
        resolve_ok(&grid, Available::size(2, 3)).size(),
        Size::new(2, 0),
        "the auto-height grid assigns its intrinsic zero-height row a finite zero-cell share"
    );
}

#[derive(Debug, Clone, PartialEq)]
struct ExtremePath;

impl CanvasItem for ExtremePath {
    fn draw(&self, context: &mut CanvasContext) {
        context.line(
            Position::new(i64::MIN, 0),
            Position::new(i64::MAX, 0),
            Grapheme::new("─"),
            TextStyle::new(),
        );
    }
}

#[test]
fn cell_line_rasterization_is_bounded_by_the_canvas_before_expansion() {
    let view = View::canvas(Canvas::new().extent(Size::new(3, 1)).item(ExtremePath));
    assert_eq!(lines(&view, Available::NONE), ["───"]);
}

#[derive(Debug, Clone, PartialEq)]
struct ExtremeDiagonal;

impl CanvasItem for ExtremeDiagonal {
    fn draw(&self, context: &mut CanvasContext) {
        context.line(
            Position::new(i64::MIN, i64::MIN),
            Position::new(i64::MAX, i64::MAX),
            Grapheme::new("x"),
            TextStyle::new(),
        );
    }
}

#[test]
fn cell_line_clipping_avoids_intermediate_overflow_on_both_axes() {
    let view = View::canvas(Canvas::new().extent(Size::new(3, 3)).item(ExtremeDiagonal));
    assert_eq!(lines(&view, Available::NONE), ["x  ", " x ", "  x"]);
}

#[derive(Debug, Clone, PartialEq)]
struct MarkerCrossing;

impl CanvasItem for MarkerCrossing {
    fn draw(&self, context: &mut CanvasContext) {
        context.line(
            Position::new(0, 1),
            Position::new(2, 1),
            Grapheme::new("-"),
            TextStyle::new(),
        );
        context.line(
            Position::new(1, 0),
            Position::new(1, 2),
            Grapheme::new("|"),
            TextStyle::new(),
        );
    }
}

#[test]
fn cell_lines_cross_as_ordinary_marker_cells() {
    let view = View::canvas(Canvas::new().extent(Size::new(3, 3)).item(MarkerCrossing));

    assert_eq!(lines(&view, Available::NONE), [" | ", "-|-", " | "]);
}

#[derive(Debug, Clone, PartialEq)]
struct CellPrimitiveShapes;

impl CanvasItem for CellPrimitiveShapes {
    fn draw(&self, context: &mut CanvasContext) {
        context.polyline(
            [
                Position::new(0, 0),
                Position::new(2, 0),
                Position::new(2, 2),
            ],
            Grapheme::new("p"),
            TextStyle::new(),
        );
        context.rectangle(
            Position::new(3, 0),
            2,
            3,
            Grapheme::new("r"),
            TextStyle::new(),
        );
    }
}

#[test]
fn cell_polyline_and_rectangle_are_direct_canvas_primitives() {
    let view = View::canvas(
        Canvas::new()
            .extent(Size::new(5, 3))
            .item(CellPrimitiveShapes),
    );

    assert_eq!(lines(&view, Available::NONE), ["ppprr", "  prr", "  prr"]);
}

#[derive(Debug, Clone, PartialEq)]
struct NetworkJunctions {
    reverse: bool,
}

impl CanvasItem for NetworkJunctions {
    fn draw(&self, context: &mut CanvasContext) {
        let mut segments = vec![
            (true, 2, 0..=4),
            (false, 2, 0..=4),
            (false, 0, 1..=3),
            (false, 4, 1..=3),
            (true, 0, 1..=3),
            (true, 4, 1..=3),
        ];
        if self.reverse {
            segments.reverse();
        }
        let mut network = LineNetwork::new(LineGlyphs::NORMAL, TextStyle::new());
        for (horizontal, fixed, range) in segments {
            if horizontal {
                network.horizontal(fixed, range);
            } else {
                network.vertical(fixed, range);
            }
        }
        context.line_network(network);
    }
}

#[test]
fn line_network_junctions_do_not_depend_on_network_recording_order() {
    let render = |reverse| {
        lines(
            &View::canvas(
                Canvas::new()
                    .extent(Size::new(5, 5))
                    .item(NetworkJunctions { reverse }),
            ),
            Available::NONE,
        )
    };
    let expected = [" ─┬─ ", "│ │ │", "├─┼─┤", "│ │ │", " ─┴─ "];

    assert_eq!(render(false), expected);
    assert_eq!(render(true), expected);
}

#[derive(Debug, Clone, PartialEq)]
struct NetworkShapes;

impl CanvasItem for NetworkShapes {
    fn draw(&self, context: &mut CanvasContext) {
        let mut rectangle = LineNetwork::new(LineGlyphs::ROUNDED, TextStyle::new());
        rectangle
            .horizontal(0, 0..=3)
            .horizontal(2, 0..=3)
            .vertical(0, 0..=2)
            .vertical(3, 0..=2);
        context.line_network(rectangle);

        let mut polyline = LineNetwork::new(LineGlyphs::ASCII, TextStyle::new());
        polyline.horizontal(1, -4..=1).vertical(1, 1..=5);
        context.line_network(polyline);
    }
}

#[test]
fn separate_network_commands_use_ordinary_canvas_overlay() {
    let view = View::canvas(Canvas::new().extent(Size::new(4, 3)).item(NetworkShapes));

    assert_eq!(lines(&view, Available::NONE), ["╭──╮", "-+ │", "╰|─╯"]);
}

#[derive(Debug, Clone, PartialEq)]
struct NetworkCustomComposition;

fn keep_existing(existing: &CanvasCell, _: &CellContribution) -> CanvasCell {
    existing.clone()
}

impl CanvasItem for NetworkCustomComposition {
    fn draw(&self, context: &mut CanvasContext) {
        context.text(Position::new(0, 0), "x", TextStyle::new());
        let mut network = LineNetwork::new(LineGlyphs::NORMAL, TextStyle::new());
        network.horizontal(0, 0..=0);
        context.line_network_with(network, Composition::Custom(keep_existing));
    }
}

#[test]
fn line_network_cells_use_the_recorded_composition() {
    let view = View::canvas(
        Canvas::new()
            .extent(Size::new(1, 1))
            .item(NetworkCustomComposition),
    );

    assert_eq!(lines(&view, Available::NONE), ["x"]);
}

#[derive(Debug, Clone, PartialEq)]
struct CustomLineGlyphs;

impl CanvasItem for CustomLineGlyphs {
    fn draw(&self, context: &mut CanvasContext) {
        let glyphs = LineGlyphs {
            isolated: 'i',
            end_up: 'u',
            end_right: 'r',
            end_down: 'd',
            end_left: 'l',
            vertical: 'v',
            horizontal: 'h',
            corner_down_right: '1',
            corner_down_left: '2',
            corner_up_right: '3',
            corner_up_left: '4',
            tee_right: '5',
            tee_down: '6',
            tee_left: '7',
            tee_up: '8',
            cross: 'x',
        };
        let mut network = LineNetwork::new(glyphs, TextStyle::new());
        network
            .horizontal(1, i64::MIN..=i64::MAX)
            .vertical(1, i64::MIN..=i64::MAX);
        context.line_network(network);
    }
}

#[test]
fn caller_owned_glyphs_and_extreme_cardinal_coordinates_are_supported() {
    let view = View::canvas(Canvas::new().extent(Size::new(3, 3)).item(CustomLineGlyphs));

    assert_eq!(lines(&view, Available::NONE), [" v ", "hxh", " v "]);
}

#[derive(Debug, Clone, PartialEq)]
struct NetworkOverlap;

impl CanvasItem for NetworkOverlap {
    fn draw(&self, context: &mut CanvasContext) {
        let glyphs = LineGlyphs {
            isolated: 'i',
            end_up: 'u',
            end_right: 'r',
            end_down: 'd',
            end_left: 'l',
            vertical: 'v',
            horizontal: 'h',
            corner_down_right: '1',
            corner_down_left: '2',
            corner_up_right: '3',
            corner_up_left: '4',
            tee_right: '5',
            tee_down: '6',
            tee_left: '7',
            tee_up: '8',
            cross: 'x',
        };
        let mut network = LineNetwork::new(glyphs, TextStyle::new());
        network
            .horizontal(0, 0..=3)
            .horizontal(0, 2..=4)
            .horizontal(0, 5..=5);
        context.line_network(network);
    }
}

#[test]
fn network_endpoints_collinear_overlap_and_isolated_points_keep_connections() {
    let view = View::canvas(Canvas::new().extent(Size::new(6, 1)).item(NetworkOverlap));

    assert_eq!(lines(&view, Available::NONE), ["rhhhli"]);
}

const CONNECTION_GLYPHS: LineGlyphs = LineGlyphs {
    isolated: 'i',
    end_up: 'u',
    end_right: 'r',
    end_down: 'd',
    end_left: 'l',
    vertical: 'v',
    horizontal: 'h',
    corner_down_right: '1',
    corner_down_left: '2',
    corner_up_right: '3',
    corner_up_left: '4',
    tee_right: '5',
    tee_down: '6',
    tee_left: '7',
    tee_up: '8',
    cross: 'x',
};

#[derive(Debug, Clone, PartialEq)]
struct SingleCellContinuations;

impl CanvasItem for SingleCellContinuations {
    fn draw(&self, context: &mut CanvasContext) {
        for (x, continuations) in [
            LineContinuations::NONE,
            LineContinuations::START,
            LineContinuations::END,
            LineContinuations::BOTH,
        ]
        .into_iter()
        .enumerate()
        {
            let mut horizontal = LineNetwork::new(CONNECTION_GLYPHS, TextStyle::new());
            horizontal.horizontal_with(0, x as i64..=x as i64, continuations);
            context.line_network(horizontal);

            let mut vertical = LineNetwork::new(CONNECTION_GLYPHS, TextStyle::new());
            vertical.vertical_with(x as i64, 1..=1, continuations);
            context.line_network(vertical);
        }
    }
}

#[test]
fn line_continuations_select_single_cell_axis_endpoints() {
    let view = View::canvas(
        Canvas::new()
            .extent(Size::new(4, 2))
            .item(SingleCellContinuations),
    );

    assert_eq!(lines(&view, Available::NONE), ["ilrh", "iudv"]);
}

#[derive(Debug, Clone, PartialEq)]
struct ContinuedJunction;

impl CanvasItem for ContinuedJunction {
    fn draw(&self, context: &mut CanvasContext) {
        let mut network = LineNetwork::new(CONNECTION_GLYPHS, TextStyle::new());
        network
            .vertical_with(0, 0..=0, LineContinuations::START)
            .horizontal(0, 0..=1);
        context.line_network(network);
    }
}

#[test]
fn endpoint_continuation_unions_with_intersecting_segments() {
    let view = View::canvas(
        Canvas::new()
            .extent(Size::new(2, 1))
            .item(ContinuedJunction),
    );

    assert_eq!(lines(&view, Available::NONE), ["3l"]);
}

#[derive(Debug, Clone, PartialEq)]
enum ClippedNetwork {
    Horizontal(std::ops::RangeInclusive<i64>),
    Vertical(std::ops::RangeInclusive<i64>),
    Outside,
}

impl CanvasItem for ClippedNetwork {
    fn draw(&self, context: &mut CanvasContext) {
        let mut network = LineNetwork::new(CONNECTION_GLYPHS, TextStyle::new());
        match self {
            Self::Horizontal(columns) => {
                network.horizontal_with(0, columns.clone(), LineContinuations::BOTH);
            }
            Self::Vertical(rows) => {
                network.vertical_with(0, rows.clone(), LineContinuations::BOTH);
            }
            Self::Outside => {
                network.horizontal_with(0, -3..=-1, LineContinuations::BOTH);
            }
        }
        context.line_network(network);
    }
}

#[test]
fn continued_segments_preserve_topology_through_all_canvas_edges() {
    let render = |item| {
        lines(
            &View::canvas(Canvas::new().extent(Size::new(2, 2)).item(item)),
            Available::NONE,
        )
    };

    assert_eq!(render(ClippedNetwork::Horizontal(-2..=1)), ["hh", "  "]);
    assert_eq!(render(ClippedNetwork::Horizontal(0..=3)), ["hh", "  "]);
    assert_eq!(render(ClippedNetwork::Vertical(-2..=1)), ["v ", "v "]);
    assert_eq!(render(ClippedNetwork::Vertical(0..=3)), ["v ", "v "]);
    assert_eq!(render(ClippedNetwork::Outside), ["  ", "  "]);
}

#[test]
fn line_network_equality_includes_axis_and_continuation_but_not_empty_ranges() {
    let empty = LineNetwork::new(LineGlyphs::NORMAL, TextStyle::new());
    let mut still_empty = empty.clone();
    let empty_start = 1;
    let empty_end = 0;
    still_empty.horizontal_with(0, empty_start..=empty_end, LineContinuations::BOTH);
    assert_eq!(empty, still_empty);

    let mut horizontal = empty.clone();
    horizontal.horizontal(0, 0..=0);
    let mut vertical = empty.clone();
    vertical.vertical(0, 0..=0);
    let mut continued = empty;
    continued.horizontal_with(0, 0..=0, LineContinuations::START);

    assert_ne!(horizontal, vertical);
    assert_ne!(horizontal, continued);
}

#[derive(Debug, Clone, PartialEq)]
struct OverwrittenNetwork;

impl CanvasItem for OverwrittenNetwork {
    fn draw(&self, context: &mut CanvasContext) {
        let mut horizontal = LineNetwork::new(LineGlyphs::NORMAL, TextStyle::new());
        horizontal.horizontal(1, 0..=2);
        context.line_network(horizontal);
        context.text(Position::new(1, 1), "x", TextStyle::new());
        let mut vertical = LineNetwork::new(LineGlyphs::NORMAL, TextStyle::new());
        vertical.vertical(1, 0..=2);
        context.line_network(vertical);
    }
}

#[test]
fn separate_network_commands_do_not_resurrect_overwritten_connections() {
    let view = View::canvas(
        Canvas::new()
            .extent(Size::new(3, 3))
            .item(OverwrittenNetwork),
    );

    assert_eq!(lines(&view, Available::NONE), [" │ ", "─│─", " │ "]);
}

#[derive(Debug, Clone, PartialEq)]
struct StyledNetwork;

impl CanvasItem for StyledNetwork {
    fn draw(&self, context: &mut CanvasContext) {
        context.text(
            Position::new(1, 1),
            "x",
            TextStyle::new().background(urushi::Color::BLUE),
        );
        let mut network = LineNetwork::new(
            LineGlyphs::NORMAL,
            TextStyle::new()
                .hyperlink("https://example.com/path")
                .bold(),
        );
        network.horizontal(1, 0..=2).vertical(1, 0..=2);
        context.line_network(network);
    }
}

#[test]
fn line_network_overlays_every_text_style_property() {
    let resolved = resolve_ok(
        &View::canvas(Canvas::new().extent(Size::new(3, 3)).item(StyledNetwork)),
        Available::NONE,
    );
    let crossing = &resolved.rows()[1][1];

    assert_eq!(crossing.symbol(), "┼");
    assert_eq!(
        crossing.style().background_color(),
        Some(urushi::Color::BLUE)
    );
    assert_eq!(
        crossing.style().hyperlink_value().unwrap().uri(),
        "https://example.com/path"
    );
    assert!(
        crossing
            .style()
            .modifiers()
            .contains(urushi::Modifier::BOLD)
    );
}

#[derive(Debug, Clone, PartialEq)]
struct RepresentativeGraph;

fn mark(existing: &CanvasCell, _: &CellContribution) -> CanvasCell {
    existing.clone().with_symbol(Grapheme::new("#"))
}

impl CanvasItem for RepresentativeGraph {
    fn draw(&self, context: &mut CanvasContext) {
        context.line(
            Position::new(-2, 1),
            Position::new(7, 1),
            Grapheme::new("-"),
            TextStyle::new(),
        );
        context.view(
            Position::new(1, 1),
            View::anchor_block(
                "node",
                BlockStyle::new()
                    .width(Length::Cells(2))
                    .height(Length::Cells(1)),
                View::text("N", TextStyle::new()),
            ),
            None,
            None,
        );
        context.view(
            Position::new(4, 1),
            View::block(
                BlockStyle::new()
                    .width(Length::fill(1))
                    .height(Length::Cells(1)),
                View::text("F", TextStyle::new()),
            ),
            Some(2),
            None,
        );
        context.text(Position::new(0, 0), "graph", TextStyle::new());
        context.cells([
            PositionedCell::new(
                Position::new(-1, 2),
                CellContribution::new().symbol(Grapheme::new("!")),
            ),
            PositionedCell::new(
                Position::new(0, 2),
                CellContribution::new().symbol(Grapheme::new(".")),
            ),
        ]);
        context.cells_with(
            [PositionedCell::new(
                Position::new(0, 2),
                CellContribution::new().symbol(Grapheme::new("+")),
            )],
            Composition::Custom(mark),
        );
        context.view(
            Position::new(2, 0),
            View::block(
                BlockStyle::new()
                    .width(Length::Cells(3))
                    .height(Length::Cells(1)),
                View::text("P", TextStyle::new()),
            ),
            None,
            None,
        );
    }
}

#[test]
fn representative_graph_composes_every_command_in_one_resolve() {
    let resolved = resolve_ok(
        &View::canvas(
            Canvas::new()
                .extent(Size::new(8, 4))
                .item(RepresentativeGraph),
        ),
        Available::NONE,
    );

    assert_eq!(
        resolved
            .rows()
            .iter()
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>(),
        ["grP     ", "-N -F --", "#       ", "        "]
    );
    let anchor = resolved.anchor("node").unwrap();
    assert_eq!(
        (anchor.x(), anchor.y(), anchor.width(), anchor.height()),
        (1, 1, 2, 1)
    );
}
