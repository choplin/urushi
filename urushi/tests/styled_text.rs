//! Public styled-text construction and layout contracts.

use urushi::{
    Available, BlockStyle, Color, Overflow, StyledGrapheme, StyledText, TextSpan, TextStyle, View,
    resolve,
};

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

    let wrapped = resolve(
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

    let clipped = resolve(
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

    let resolved = resolve(&View::styled_text(text), Available::NONE);
    assert_eq!(resolved.size(), urushi::Size::new(4, 2));
    assert_eq!(row_text(&resolved.rows()[0]), "日本");
    assert_eq!(row_text(&resolved.rows()[1]), "👩‍💻x ");
    assert_eq!(resolved.rows()[0][0].width(), 2);
    assert_eq!(resolved.rows()[0][0].style(), &first);
    assert_eq!(resolved.rows()[1][0].width(), 2);
    assert_eq!(resolved.rows()[1][0].style(), &second);
}
