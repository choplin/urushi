#[path = "../examples/cjk_showcase.rs"]
mod cjk_showcase;

use urushi::visible_width;

#[test]
fn cjk_showcase_preserves_width_and_japanese_content() {
    let output = cjk_showcase::render_cjk_showcase();
    let widths: Vec<usize> = output.lines().map(visible_width).collect();

    assert!(
        widths.iter().all(|width| *width == 49),
        "CJK showcase lines should have the same visible width\n{output}"
    );
    for label in [
        "前景色",
        "背景色",
        "太字",
        "下線",
        "入れ子 ANSI",
        "パディング",
        "四辺の罫線",
        "固定幅",
        "左揃え",
        "中央揃え",
        "右揃え",
        "横結合",
        "縦結合",
        "上辺のみ",
        "右辺のみ",
        "下辺のみ",
        "左辺のみ",
    ] {
        assert!(output.contains(label), "機能ラベルがありません: {label}");
    }
}
