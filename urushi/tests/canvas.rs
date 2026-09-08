use urushi::{
    Available, Axis, BlockStyle, Canvas, CanvasCell, CanvasContext, CanvasItem, CanvasSizing,
    CellContribution, Composition, Grapheme, LayoutErrorKind, Length, Path, Position,
    PositionedCell, Size, TextStyle, View, resolve, try_resolve,
};

#[derive(Debug, Clone, PartialEq)]
struct Scene {
    label: &'static str,
}

impl CanvasItem for Scene {
    fn draw(&self, context: &mut CanvasContext) {
        assert_eq!(context.bounds(), (Position::new(0, 0), Size::new(8, 4)));
        context.path(Path::line(
            Position::new(-2, 1),
            Position::new(6, 1),
            Grapheme::new("─"),
            TextStyle::new(),
        ));
        context.text(Position::new(1, 1), self.label, TextStyle::new());
        context.cells([PositionedCell::new(
            Position::new(7, 3),
            CellContribution::new().symbol(Grapheme::new("x")),
        )]);
    }
}

fn lines(view: &View, available: Available) -> Vec<String> {
    resolve(view, available)
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

#[test]
#[should_panic]
fn canvas_symbol_boundaries_reject_terminal_control_sequences() {
    let symbol = Grapheme::new("\u{1b}]8;;https://example.invalid\u{7}");
    let _ = CellContribution::new().symbol(symbol);
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
    let resolved = resolve(
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
                BlockStyle::new().width(Length::Fill(1)),
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
                    BlockStyle::new().width(Length::Fill(1)),
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
                BlockStyle::new().width(Length::Fill(1)).max_width(4),
                View::text("fill", TextStyle::new()),
            ),
            None,
            Some(1),
        );
    }
}

#[test]
fn unbounded_canvas_and_fill_view_commands_report_the_missing_axis() {
    let extent = try_resolve(&View::canvas(Canvas::new()), Available::NONE).unwrap_err();
    assert_eq!(extent.kind(), LayoutErrorKind::CanvasExtent);
    assert_eq!(extent.axis(), Axis::Width);

    let allocation = try_resolve(
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

    let nested = try_resolve(
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

    let capped = try_resolve(
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
            .width(Length::Fill(1))
            .height(Length::Fill(1)),
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
    let resolved = resolve(
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
        try_resolve(&view, Available::NONE).unwrap_err().axis(),
        Axis::Width
    );
}

#[test]
fn finite_column_and_grid_shares_are_canvas_allocations() {
    let column = View::column(urushi::Align::Left, [View::canvas(Canvas::new().width(2))]);
    assert_eq!(
        resolve(&column, Available::size(2, 3)).size(),
        Size::new(2, 3)
    );

    let grid = View::grid(
        urushi::GridStyle::new(),
        [[View::canvas(Canvas::new().width(2))]],
    );
    assert_eq!(
        resolve(&grid, Available::size(2, 3)).size(),
        Size::new(2, 0),
        "the auto-height grid assigns its intrinsic zero-height row a finite zero-cell share"
    );
}

#[derive(Debug, Clone, PartialEq)]
struct ExtremePath;

impl CanvasItem for ExtremePath {
    fn draw(&self, context: &mut CanvasContext) {
        context.path(Path::line(
            Position::new(i64::MIN, 0),
            Position::new(i64::MAX, 0),
            Grapheme::new("─"),
            TextStyle::new(),
        ));
    }
}

#[test]
fn path_rasterization_is_bounded_by_the_canvas_before_expansion() {
    let view = View::canvas(Canvas::new().extent(Size::new(3, 1)).item(ExtremePath));
    assert_eq!(lines(&view, Available::NONE), ["───"]);
}

#[derive(Debug, Clone, PartialEq)]
struct ExtremeDiagonal;

impl CanvasItem for ExtremeDiagonal {
    fn draw(&self, context: &mut CanvasContext) {
        context.path(Path::line(
            Position::new(i64::MIN, i64::MIN),
            Position::new(i64::MAX, i64::MAX),
            Grapheme::new("x"),
            TextStyle::new(),
        ));
    }
}

#[test]
fn path_clipping_avoids_intermediate_overflow_on_both_axes() {
    let view = View::canvas(Canvas::new().extent(Size::new(3, 3)).item(ExtremeDiagonal));
    assert_eq!(lines(&view, Available::NONE), ["x  ", " x ", "  x"]);
}

#[derive(Debug, Clone, PartialEq)]
struct RepresentativeGraph;

fn mark(existing: &CanvasCell, _: &CellContribution) -> CanvasCell {
    existing.clone().with_symbol(Grapheme::new("#"))
}

impl CanvasItem for RepresentativeGraph {
    fn draw(&self, context: &mut CanvasContext) {
        context.path(Path::line(
            Position::new(-2, 1),
            Position::new(7, 1),
            Grapheme::new("-"),
            TextStyle::new(),
        ));
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
                    .width(Length::Fill(1))
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
    let resolved = resolve(
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
