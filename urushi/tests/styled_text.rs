//! Public styled-text construction and layout contracts.

use urushi::{
    Available, BlockStyle, Color, Overflow, RenderSettings, ResolvedView, StyledGrapheme,
    StyledText, TabPolicy, TextSpan, TextStyle, View, render, render_text, resolve,
};

fn resolve_ok(view: &View, available: Available) -> ResolvedView {
    resolve(view, available).unwrap()
}

fn ansi_settings() -> RenderSettings {
    RenderSettings::all()
}

fn row_text(row: &[StyledGrapheme]) -> String {
    row.iter().map(StyledGrapheme::symbol).collect()
}

#[test]
fn strings_become_default_styled_segments() {
    let owned: TextSpan = String::from("owned").into();
    let borrowed: TextSpan = " borrowed".into();

    assert_eq!(owned.text(), "owned");
    assert_eq!(owned.style(), &TextStyle::new());
    assert_eq!(borrowed.style(), &TextStyle::new());

    let text = StyledText::try_from_spans([owned, borrowed]).expect("whole grapheme boundaries");
    assert_eq!(text.as_str(), "owned borrowed");
    assert_eq!(
        text.spans().collect::<Vec<_>>(),
        [("owned borrowed", &TextStyle::new())]
    );

    let literals = StyledText::try_from_spans(["plain", " text"])
        .expect("string literals become default-style spans");
    let strings = StyledText::try_from_spans([String::from("owned text")])
        .expect("owned strings become default-style spans");
    assert_eq!(literals.as_str(), "plain text");
    assert_eq!(
        strings.spans().collect::<Vec<_>>(),
        [("owned text", &TextStyle::new())]
    );
}

#[test]
fn raw_ansi_cannot_enter_direct_text_rendering() {
    let attempted = std::panic::catch_unwind(|| {
        let text = StyledText::new("\x1b[31mred\x1b[0m", TextStyle::new());
        render_text(&text, &RenderSettings::all())
    });

    assert!(attempted.is_err());
}

#[test]
fn a_segment_boundary_must_be_a_whole_text_grapheme_boundary() {
    let error = StyledText::try_from_spans([
        TextSpan::from("e"),
        TextSpan::new("\u{301}", TextStyle::new().bold()),
    ])
    .expect_err("the two segments form one grapheme");

    assert_eq!(error.span(), 0);
    assert_eq!(error.byte_offset(), 1);
}

#[test]
fn wrapping_and_clipping_cross_style_boundaries() {
    let red = TextStyle::new().foreground(Color::RED);
    let blue = TextStyle::new().foreground(Color::BLUE);
    let text = StyledText::try_from_spans([
        TextSpan::new("ab", red.clone()),
        TextSpan::new("cdef", blue.clone()),
    ])
    .expect("whole grapheme boundaries");

    let wrapped = resolve_ok(
        &View::block(
            BlockStyle::new().max_width(4),
            View::styled_text(text.clone()),
        ),
        Available::NONE,
    );
    assert_eq!(
        wrapped
            .rows()
            .iter()
            .map(|row| row_text(row))
            .collect::<Vec<_>>(),
        ["abcd", "ef  "]
    );
    assert_eq!(wrapped.rows()[0][1].style(), &red);
    assert_eq!(wrapped.rows()[0][2].style(), &blue);

    let clipped = resolve_ok(
        &View::block(
            BlockStyle::new()
                .max_width(4)
                .overflow(Overflow::ellipsis()),
            View::styled_text(text),
        ),
        Available::NONE,
    );
    assert_eq!(row_text(&clipped.rows()[0]), "abc…");
    assert_eq!(clipped.rows()[0][1].style(), &red);
    assert_eq!(clipped.rows()[0][2].style(), &blue);
    assert_eq!(
        clipped.rows()[0][3].style(),
        &blue,
        "the marker represents and inherits the first omitted grapheme"
    );
}

#[test]
fn multiline_cjk_and_emoji_keep_segment_styles() {
    let first = TextStyle::new().foreground(Color::GREEN);
    let second = TextStyle::new().bold();
    let text = StyledText::try_from_spans([
        TextSpan::new("日本\n", first.clone()),
        TextSpan::new("👩‍💻x", second.clone()),
    ])
    .expect("whole grapheme boundaries");

    let resolved = resolve_ok(&View::styled_text(text), Available::NONE);
    assert_eq!(resolved.size(), urushi::Size::new(4, 2));
    assert_eq!(row_text(&resolved.rows()[0]), "日本");
    assert_eq!(row_text(&resolved.rows()[1]), "👩‍💻x ");
    assert_eq!(resolved.rows()[0][0].width(), 2);
    assert_eq!(resolved.rows()[0][0].style(), &first);
    assert_eq!(resolved.rows()[1][0].width(), 2);
    assert_eq!(resolved.rows()[1][0].style(), &second);
}

#[test]
fn tabs_expand_before_measurement_wrapping_and_rendering() {
    let style = TextStyle::new().foreground(Color::GREEN);
    let text = StyledText::new("a\t日\tb", style.clone()).tab_policy(TabPolicy::spaces(2));
    let view = View::block(BlockStyle::new().max_width(5), View::styled_text(text));
    let resolved = resolve_ok(&view, Available::NONE);

    assert_eq!(resolved.size(), urushi::Size::new(5, 2));
    assert_eq!(
        resolved
            .rows()
            .iter()
            .map(|row| row_text(row))
            .collect::<Vec<_>>(),
        ["a  日", "b    "]
    );
    assert!(
        resolved
            .rows()
            .iter()
            .flatten()
            .all(|cell| cell.symbol() != "\t")
    );
    assert!(
        resolved
            .rows()
            .iter()
            .flatten()
            .filter(|cell| cell.symbol() != " ")
            .all(|cell| cell.style() == &style)
    );
    assert!(!render(&resolved, &ansi_settings()).contains('\t'));

    let fixed = View::block(
        BlockStyle::new().width(8).align(urushi::Align::Right),
        View::styled_text(
            StyledText::new("a\tb", TextStyle::new()).tab_policy(TabPolicy::spaces(2)),
        ),
    );
    assert_eq!(
        row_text(&resolve_ok(&fixed, Available::NONE).rows()[0]),
        "    a  b"
    );
}

#[test]
fn visible_tab_marker_inherits_style_and_pads_to_the_fixed_width() {
    let tab_style = TextStyle::new().bold();
    let text = StyledText::try_from_spans([
        TextSpan::new("左", TextStyle::new()),
        TextSpan::new("\t", tab_style.clone()),
        TextSpan::new("right", TextStyle::new()),
    ])
    .unwrap()
    .tab_policy(TabPolicy::with_marker(4, "→").unwrap());
    let resolved = resolve_ok(&View::styled_text(text), Available::NONE);

    assert_eq!(row_text(&resolved.rows()[0]), "左→   right");
    assert_eq!(resolved.rows()[0][1].style(), &tab_style);
    assert_eq!(resolved.rows()[0][2].style(), &tab_style);
    assert_eq!(resolved.rows()[0][3].style(), &tab_style);
    assert_eq!(resolved.rows()[0][4].style(), &tab_style);
}

#[test]
fn zero_width_removes_tabs_and_removing_the_property_restores_default_spaces() {
    let removed = StyledText::new("a\tb", TextStyle::new()).tab_policy(TabPolicy::spaces(0));
    assert_eq!(
        row_text(&resolve_ok(&View::styled_text(removed), Available::NONE).rows()[0]),
        "ab"
    );

    let defaulted = StyledText::new("a\tb", TextStyle::new())
        .tab_policy(TabPolicy::spaces(1))
        .reset_tab_policy();
    assert_eq!(defaulted.get_tab_policy(), None);
    assert_eq!(
        row_text(&resolve_ok(&View::styled_text(defaulted), Available::NONE).rows()[0]),
        "a    b"
    );
}

#[test]
fn direct_text_rendering_preserves_literal_tabs_without_applying_layout_policy() {
    let text = StyledText::new("name\t値", TextStyle::new().bold())
        .tab_policy(TabPolicy::with_marker(4, "[T]").unwrap());

    assert_eq!(
        render_text(&text, &ansi_settings()),
        "\x1b[1mname\t値\x1b[0m"
    );
}
