use urushi::{
    Align, BlockStyle, Border, Color, RenderedBlock, VerticalAlign, join_horizontal, join_vertical,
};

/// The cells one rendered row occupies.
///
/// Rendered output is measured through the crate's one ANSI-aware entry point;
/// there is no free function that takes a string and guesses at its domain.
fn row_width(line: &str) -> usize {
    RenderedBlock::from_ansi(line).size().width()
}

fn adopted(text: &str) -> RenderedBlock {
    RenderedBlock::from_ansi(text)
}

#[test]
fn horizontal_join_places_short_blocks_at_each_vertical_alignment() {
    let blocks = [adopted("L"), adopted("a\nb\nc")];

    assert_eq!(
        join_horizontal(VerticalAlign::Top, &blocks).as_str(),
        "La\n b\n c"
    );
    assert_eq!(
        join_horizontal(VerticalAlign::Center, &blocks).as_str(),
        " a\nLb\n c"
    );
    assert_eq!(
        join_horizontal(VerticalAlign::Bottom, &blocks).as_str(),
        " a\n b\nLc"
    );
}

#[test]
fn horizontal_join_keeps_cjk_borders_aligned() {
    let japanese = BlockStyle::new().border(Border::NORMAL).render("日本語");
    let tall = BlockStyle::new().border(Border::NORMAL).render("A\nB");

    let joined = join_horizontal(VerticalAlign::Top, &[japanese, tall]);
    assert_eq!(
        joined.as_str(),
        concat!(
            "┌──────┐┌─┐\n",
            "│日本語││A│\n",
            "└──────┘│B│\n",
            "        └─┘"
        )
    );
    assert_eq!(joined.size().width(), 11);
    assert!(joined.as_str().lines().all(|line| row_width(line) == 11));
}

#[test]
fn horizontal_join_preserves_ansi_and_visible_alignment() {
    let red = BlockStyle::new().foreground(Color::RED).render("赤\nx");
    let joined = join_horizontal(VerticalAlign::Top, &[red, adopted("A\nBB")]);

    assert_eq!(joined.as_str(), "\x1b[31m赤\x1b[0mA \n\x1b[31mx \x1b[0mBB");
    assert!(joined.as_str().lines().all(|line| row_width(line) == 4));
}

#[test]
fn vertical_join_places_blocks_at_each_horizontal_alignment() {
    let blocks = [adopted("甲\nx"), adopted("long")];

    assert_eq!(
        join_vertical(Align::Left, &blocks).as_str(),
        "甲  \nx   \nlong"
    );
    assert_eq!(
        join_vertical(Align::Center, &blocks).as_str(),
        " 甲 \n x  \nlong"
    );
    // A block is placed as a rectangle, so its own rows keep the shape they
    // were measured with rather than being realigned one by one.
    assert_eq!(
        join_vertical(Align::Right, &blocks).as_str(),
        "  甲\n  x \nlong"
    );
}

#[test]
fn vertical_join_preserves_ansi_and_visible_alignment() {
    let red = BlockStyle::new().foreground(Color::RED).render("赤");
    let joined = join_vertical(Align::Right, &[red, adopted("x")]);

    assert_eq!(joined.as_str(), "\x1b[31m赤\x1b[0m\n x");
    assert!(joined.as_str().lines().all(|line| row_width(line) == 2));
}

#[test]
fn an_adopted_block_is_measured_once_and_kept_rectangular() {
    let adopted = RenderedBlock::from_ansi("\x1b[31m赤\x1b[0m\nx");

    assert_eq!(adopted.size().width(), 2);
    assert_eq!(adopted.size().height(), 2);
    assert_eq!(adopted.as_str(), "\x1b[31m赤\x1b[0m\nx ");
}
