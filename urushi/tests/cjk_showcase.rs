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
        "固定高さ",
        "垂直揃え",
        "水平揃え",
        "横結合",
        "縦結合",
        "木構造",
        "箇条書き",
        "罫線の種類",
        "上辺のみ",
        "右辺のみ",
        "下辺のみ",
        "左辺のみ",
    ] {
        assert!(output.contains(label), "機能ラベルがありません: {label}");
    }
    assert!(output.contains("20列分を確保"));
    assert!(output.contains("3行分を確保"));
    for cell in ["項目", "有効", "色数"] {
        assert!(output.contains(cell), "表のセルがありません: {cell}");
    }
    for node in ["うるし", "ソース", "部品", "描画", "設定"] {
        assert!(output.contains(node), "木構造のノードがありません: {node}");
    }
    for item in ["設計する", "実装する", "モデル", "ビュー", "検証する"] {
        assert!(output.contains(item), "箇条書きの項目がありません: {item}");
    }
    assert!(output.contains("┌───┬───┐"));
    assert!(output.contains("├───┼───┤"));
    assert!(output.contains("└───┴───┘"));
    assert!(output.contains("+---+---+"));
    assert!(output.contains("|---|---|"));
    assert!(output.contains("━━━━━━━━━"));
    assert!(output.contains("─────────"));
    for preset in [
        "標準",
        "角丸",
        "太線",
        "二重線",
        "ASCII",
        "Markdown",
        "三線表",
        "非表示",
    ] {
        assert!(output.contains(preset), "罫線presetがありません: {preset}");
    }
    for position in ["上", "中央", "下", "左", "右"] {
        assert!(
            output.contains(position),
            "揃え位置がありません: {position}"
        );
    }

    let lines: Vec<_> = output.lines().collect();
    let fixed_width = lines
        .iter()
        .position(|line| line.contains("固定幅"))
        .expect("固定幅の行");
    let fixed_height = lines
        .iter()
        .position(|line| line.contains("固定高さ"))
        .expect("固定高さの行");
    let horizontal_alignment = lines
        .iter()
        .position(|line| line.contains("水平揃え"))
        .expect("水平揃えの見本");
    let vertical_alignment = lines
        .iter()
        .position(|line| line.contains("垂直揃え"))
        .expect("垂直揃えの見本");
    assert_eq!(fixed_height - fixed_width, 2);
    assert_eq!(vertical_alignment - fixed_height, 6);
    assert_eq!(horizontal_alignment - vertical_alignment, 6);
}
