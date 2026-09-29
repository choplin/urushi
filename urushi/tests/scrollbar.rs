//! Public Scrollbar composition and layout behavior.

use urushi::{
    Available, Axis, Color, LayoutErrorKind, Scrollbar, ScrollbarGlyphs, ScrollbarOrientation,
    ScrollbarPresentation, ScrollbarRole, ScrollbarThumbSizing, SemanticTokens, Size, TextStyle,
    Theme, resolve,
};

fn presentation() -> ScrollbarPresentation {
    ScrollbarPresentation::new(TextStyle::new(), TextStyle::new())
}

fn symbols(
    presentation: &ScrollbarPresentation,
    scrollbar: &Scrollbar,
    width: usize,
    height: usize,
) -> Vec<String> {
    resolve(
        &presentation.compose(scrollbar),
        Available::size(width, height),
    )
    .unwrap()
    .rows()
    .iter()
    .map(|row| row.iter().map(|cell| cell.symbol()).collect())
    .collect()
}

#[test]
fn semantic_data_keeps_orientation_lengths_and_requested_position() {
    let scrollbar = Scrollbar::new(ScrollbarOrientation::Horizontal, 120, 30).position(47);

    assert_eq!(scrollbar.orientation(), ScrollbarOrientation::Horizontal);
    assert_eq!(scrollbar.content_length(), 120);
    assert_eq!(scrollbar.viewport_length(), 30);
    assert_eq!(scrollbar.get_position(), 47);
}

#[test]
fn proportional_thumb_tracks_the_start_middle_and_end() {
    let scrollbar =
        |position| Scrollbar::new(ScrollbarOrientation::Horizontal, 10, 4).position(position);

    assert_eq!(
        symbols(&presentation(), &scrollbar(0), 10, 4),
        ["←███─────→"]
    );
    assert_eq!(
        symbols(&presentation(), &scrollbar(3), 10, 4),
        ["←───███──→"]
    );
    assert_eq!(
        symbols(&presentation(), &scrollbar(6), 10, 4),
        ["←─────███→"]
    );
    assert_eq!(
        symbols(&presentation(), &scrollbar(usize::MAX), 10, 4),
        ["←─────███→"]
    );
}

#[test]
fn marker_uses_one_cell_and_the_same_clamped_scroll_range() {
    let marker = presentation()
        .thumb_sizing(ScrollbarThumbSizing::Marker)
        .glyphs(
            ScrollbarOrientation::Horizontal,
            ScrollbarGlyphs::new("#").track(Some("-")),
        );
    let scrollbar =
        |position| Scrollbar::new(ScrollbarOrientation::Horizontal, 10, 2).position(position);

    assert_eq!(symbols(&marker, &scrollbar(0), 5, 2), ["#----"]);
    assert_eq!(symbols(&marker, &scrollbar(4), 5, 2), ["--#--"]);
    assert_eq!(symbols(&marker, &scrollbar(8), 5, 2), ["----#"]);
    assert_eq!(symbols(&marker, &scrollbar(99), 5, 2), ["----#"]);
}

#[test]
fn orientation_uses_one_cross_axis_cell_and_parent_selected_placement() {
    let vertical = presentation().compose(&Scrollbar::new(ScrollbarOrientation::Vertical, 10, 2));
    let horizontal =
        presentation().compose(&Scrollbar::new(ScrollbarOrientation::Horizontal, 10, 2));

    assert_eq!(
        resolve(&vertical, Available::size(9, 6)).unwrap().size(),
        Size::new(1, 6)
    );
    assert_eq!(
        resolve(&horizontal, Available::size(6, 9)).unwrap().size(),
        Size::new(6, 1)
    );
}

#[test]
fn the_main_axis_requires_finite_allocation() {
    let vertical = presentation().compose(&Scrollbar::new(ScrollbarOrientation::Vertical, 10, 2));
    let horizontal =
        presentation().compose(&Scrollbar::new(ScrollbarOrientation::Horizontal, 10, 2));

    let vertical_error = resolve(&vertical, Available::columns(1)).unwrap_err();
    assert_eq!(vertical_error.kind(), LayoutErrorKind::CanvasExtent);
    assert_eq!(vertical_error.axis(), Axis::Height);

    let horizontal_error = resolve(&horizontal, Available::new(None, Some(1))).unwrap_err();
    assert_eq!(horizontal_error.kind(), LayoutErrorKind::CanvasExtent);
    assert_eq!(horizontal_error.axis(), Axis::Width);
}

#[test]
fn empty_complete_zero_viewport_and_zero_extent_have_defined_results() {
    let bare = presentation().glyphs(
        ScrollbarOrientation::Horizontal,
        ScrollbarGlyphs::new("#").track(Some("-")),
    );

    assert_eq!(
        symbols(
            &bare,
            &Scrollbar::new(ScrollbarOrientation::Horizontal, 0, 0),
            5,
            1,
        ),
        ["     "]
    );
    assert_eq!(
        symbols(
            &bare,
            &Scrollbar::new(ScrollbarOrientation::Horizontal, 4, 8).position(99),
            5,
            1,
        ),
        ["#####"]
    );
    assert_eq!(
        symbols(
            &bare,
            &Scrollbar::new(ScrollbarOrientation::Horizontal, 10, 0).position(10),
            5,
            1,
        ),
        ["----#"]
    );
    assert_eq!(
        resolve(
            &bare.compose(&Scrollbar::new(ScrollbarOrientation::Horizontal, 10, 2)),
            Available::size(0, 1),
        )
        .unwrap()
        .size(),
        Size::new(0, 1)
    );
}

#[test]
fn narrow_tracks_drop_endpoints_before_the_position_indicator() {
    let complete = Scrollbar::new(ScrollbarOrientation::Horizontal, 1, 1);
    let marker = Scrollbar::new(ScrollbarOrientation::Horizontal, 10, 1).position(9);

    assert_eq!(symbols(&presentation(), &complete, 1, 1), ["█"]);
    assert_eq!(symbols(&presentation(), &complete, 2, 1), ["██"]);
    assert_eq!(
        symbols(
            &presentation().thumb_sizing(ScrollbarThumbSizing::Marker),
            &marker,
            1,
            1,
        ),
        ["█"]
    );
}

#[test]
fn extreme_lengths_do_not_overflow_and_reach_the_track_end() {
    let bare = presentation().glyphs(
        ScrollbarOrientation::Horizontal,
        ScrollbarGlyphs::new("#").track(Some("-")),
    );
    let scrollbar =
        Scrollbar::new(ScrollbarOrientation::Horizontal, usize::MAX, 1).position(usize::MAX);

    assert_eq!(symbols(&bare, &scrollbar, 4, 1), ["---#"]);
}

#[test]
fn glyph_sets_and_part_styles_select_distinct_terminal_presentations() {
    let thumb = TextStyle::new().foreground(Color::RED);
    let track = TextStyle::new().foreground(Color::BLUE);
    let begin = TextStyle::new().foreground(Color::GREEN);
    let end = TextStyle::new().foreground(Color::YELLOW);
    let configured = presentation()
        .glyphs(
            ScrollbarOrientation::Horizontal,
            ScrollbarGlyphs::new("#")
                .track(Some("-"))
                .begin(Some("<"))
                .end(Some(">")),
        )
        .thumb_style(thumb.clone())
        .track_style(track.clone())
        .begin_style(begin.clone())
        .end_style(end.clone());
    let resolved = resolve(
        &configured.compose(&Scrollbar::new(ScrollbarOrientation::Horizontal, 4, 2)),
        Available::size(6, 1),
    )
    .unwrap();
    let row = &resolved.rows()[0];

    assert_eq!(
        row.iter().map(|cell| cell.symbol()).collect::<String>(),
        "<##-->"
    );
    assert_eq!(row[0].style(), &begin);
    assert_eq!(row[1].style(), &thumb);
    assert_eq!(row[3].style(), &track);
    assert_eq!(row[5].style(), &end);
    assert_eq!(configured.get_style(ScrollbarRole::Thumb), &thumb);
}

#[test]
fn thumb_only_and_background_bar_are_local_variations_of_the_same_data() {
    let scrollbar = Scrollbar::new(ScrollbarOrientation::Vertical, 10, 2).position(4);
    let thumb_only =
        presentation().glyphs(ScrollbarOrientation::Vertical, ScrollbarGlyphs::new("▐"));
    let background = presentation()
        .glyphs(
            ScrollbarOrientation::Vertical,
            ScrollbarGlyphs::new(" ").track(Some(" ")),
        )
        .thumb_style(TextStyle::new().background(Color::BLUE))
        .track_style(TextStyle::new().background(Color::BRIGHT_BLACK));

    assert_eq!(
        symbols(&thumb_only, &scrollbar, 1, 5),
        [" ", " ", "▐", " ", " "]
    );

    let resolved = resolve(&background.compose(&scrollbar), Available::size(1, 5)).unwrap();
    assert!(resolved.rows().iter().all(|row| row[0].symbol() == " "));
    assert_eq!(
        resolved.rows()[2][0].style().get_background(),
        Some(Color::BLUE)
    );
    assert_eq!(
        resolved.rows()[0][0].style().get_background(),
        Some(Color::BRIGHT_BLACK)
    );
}

#[test]
fn theme_shortcut_delegates_to_the_canonical_presentation() {
    let theme = Theme::from_tokens(SemanticTokens {
        text: Color::WHITE,
        text_muted: Color::BRIGHT_BLACK,
        background: Color::BLACK,
        surface: Color::BLACK,
        accent: Color::BLUE,
        accent_text: Color::WHITE,
        success: Color::GREEN,
        warning: Color::YELLOW,
        error: Color::RED,
        border: Color::BRIGHT_BLACK,
    });
    let scrollbar = Scrollbar::new(ScrollbarOrientation::Vertical, 20, 5).position(3);

    assert_eq!(
        theme.scrollbar(&scrollbar),
        theme.components().scrollbar().compose(&scrollbar)
    );
}

#[test]
#[should_panic(expected = "exactly one grapheme")]
fn several_graphemes_are_not_valid_scrollbar_glyphs() {
    let _ = ScrollbarGlyphs::new("ab");
}

#[test]
#[should_panic(expected = "control characters")]
fn control_characters_are_not_valid_scrollbar_glyphs() {
    let _ = ScrollbarGlyphs::new("\n");
}

#[test]
#[should_panic(expected = "exactly one terminal cell")]
fn wide_graphemes_are_not_valid_scrollbar_glyphs() {
    let _ = ScrollbarGlyphs::new("界");
}

#[test]
#[should_panic(expected = "exactly one terminal cell")]
fn zero_width_graphemes_are_not_valid_scrollbar_glyphs() {
    let _ = ScrollbarGlyphs::new("\u{0301}");
}
