use urushi::{
    Align, Border, Color, Style, VerticalAlign, join_horizontal, join_vertical, visible_width,
};

#[test]
fn horizontal_join_places_short_blocks_at_each_vertical_alignment() {
    let blocks = ["L", "a\nb\nc"];

    assert_eq!(join_horizontal(VerticalAlign::Top, &blocks), "La\n b\n c");
    assert_eq!(
        join_horizontal(VerticalAlign::Center, &blocks),
        " a\nLb\n c"
    );
    assert_eq!(
        join_horizontal(VerticalAlign::Bottom, &blocks),
        " a\n b\nLc"
    );
}

#[test]
fn horizontal_join_keeps_cjk_borders_aligned() {
    let japanese = Style::new().border(Border::NORMAL).render("日本語");
    let tall = Style::new().border(Border::NORMAL).render("A\nB");

    let joined = join_horizontal(VerticalAlign::Top, &[japanese, tall]);
    assert_eq!(
        joined,
        concat!(
            "┌──────┐┌─┐\n",
            "│日本語││A│\n",
            "└──────┘│B│\n",
            "        └─┘"
        )
    );
    assert!(joined.lines().all(|line| visible_width(line) == 11));
}

#[test]
fn horizontal_join_preserves_ansi_and_visible_alignment() {
    let red = Style::new().foreground(Color::RED).render("赤\nx");
    let joined = join_horizontal(VerticalAlign::Top, &[red, "A\nBB".to_string()]);

    assert_eq!(joined, "\x1b[31m赤\x1b[0mA \n\x1b[31mx \x1b[0mBB");
    assert!(joined.lines().all(|line| visible_width(line) == 4));
}

#[test]
fn vertical_join_places_blocks_at_each_horizontal_alignment() {
    let blocks = ["甲\nx", "long"];

    assert_eq!(join_vertical(Align::Left, &blocks), "甲  \nx   \nlong");
    assert_eq!(join_vertical(Align::Center, &blocks), " 甲 \n x  \nlong");
    assert_eq!(join_vertical(Align::Right, &blocks), "  甲\n   x\nlong");
}

#[test]
fn vertical_join_preserves_ansi_and_visible_alignment() {
    let red = Style::new().foreground(Color::RED).render("赤");
    let joined = join_vertical(Align::Right, &[red, "x".to_string()]);

    assert_eq!(joined, "\x1b[31m赤\x1b[0m\n x");
    assert!(joined.lines().all(|line| visible_width(line) == 2));
}
