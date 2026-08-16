use urushi::{
    Align, AnsiPolicy, AnsiRenderer, BlockStyle, Border, Color, ColorProfile, List, ListItem,
    SemanticTokens, Table, TerminalProfile, TextStyle, Theme, Tree, TreeNode, VerticalAlign, View,
    arabic_enumerator, measure,
};

/// Blank space of a fixed width, used to separate samples.
fn gap(width: usize) -> View {
    View::text(" ".repeat(width), TextStyle::new())
}

/// Wraps plain text in a block filled with that block's own style.
fn boxed(style: BlockStyle, text: &str) -> View {
    let content = View::text(text, style.text().clone());
    View::block(style, content)
}

fn row(label: &str, sample: View) -> View {
    row_with_alignment(label, sample, VerticalAlign::Center)
}

fn row_with_alignment(label: &str, sample: View, alignment: VerticalAlign) -> View {
    let label = View::block(
        BlockStyle::new()
            .foreground(Color::BRIGHT_CYAN)
            .bold()
            .width(20)
            .align(Align::Right),
        View::text(
            label,
            TextStyle::new().foreground(Color::BRIGHT_CYAN).bold(),
        ),
    );
    let divider = View::text(" | ", TextStyle::new().foreground(Color::BRIGHT_BLACK));

    View::row(alignment, [label, divider, sample])
}

fn vertical_alignment_sample(top: &str, center: &str, bottom: &str) -> View {
    let cell = BlockStyle::new()
        .background(Color::BRIGHT_BLACK)
        .width(6)
        .height(5)
        .align(Align::Center);

    View::row(
        VerticalAlign::Top,
        [
            boxed(cell.clone().align_vertical(VerticalAlign::Top), top),
            gap(1),
            boxed(cell.clone().align_vertical(VerticalAlign::Center), center),
            gap(1),
            boxed(cell.align_vertical(VerticalAlign::Bottom), bottom),
        ],
    )
}

fn horizontal_alignment_sample(left: &str, center: &str, right: &str) -> View {
    let cell = BlockStyle::new().background(Color::BRIGHT_BLACK).width(20);

    View::column(
        Align::Left,
        [
            boxed(cell.clone().align(Align::Left), left),
            gap(20),
            boxed(cell.clone().align(Align::Center), center),
            gap(20),
            boxed(cell.align(Align::Right), right),
        ],
    )
}

fn junction_grid(border: Border) -> String {
    let top = border.top.to_string().repeat(3);
    let middle = border.middle_horizontal.to_string().repeat(3);
    let bottom = border.bottom.to_string().repeat(3);

    format!(
        "{}{}{}{}{}\n{}甲 {}乙 {}\n{}{}{}{}{}\n{}丙 {}丁 {}\n{}{}{}{}{}",
        border.top_left,
        top,
        border.middle_top,
        top,
        border.top_right,
        border.left,
        border.left,
        border.right,
        border.middle_left,
        middle,
        border.middle,
        middle,
        border.middle_right,
        border.left,
        border.left,
        border.right,
        border.bottom_left,
        bottom,
        border.middle_bottom,
        bottom,
        border.bottom_right,
    )
}

fn border_card(label: &str, border: Border, color: Color) -> View {
    let label = View::block(
        BlockStyle::new()
            .foreground(color)
            .bold()
            .width(9)
            .align(Align::Center),
        View::text(label, TextStyle::new().foreground(color).bold()),
    );
    let mut grid_style = TextStyle::new().foreground(color);
    if border == Border::HIDDEN {
        grid_style = grid_style.background(Color::BRIGHT_BLACK);
    }
    let grid = View::text(junction_grid(border), grid_style);

    View::column(Align::Left, [label, grid])
}

fn border_preset_sample() -> View {
    let pair = |left, right| View::row(VerticalAlign::Top, [left, gap(1), right]);
    View::column(
        Align::Left,
        [
            pair(
                border_card("標準", Border::NORMAL, Color::BRIGHT_GREEN),
                border_card("角丸", Border::ROUNDED, Color::BRIGHT_CYAN),
            ),
            gap(19),
            pair(
                border_card("太線", Border::THICK, Color::BRIGHT_MAGENTA),
                border_card("二重線", Border::DOUBLE, Color::BRIGHT_BLUE),
            ),
            gap(19),
            pair(
                border_card("ASCII", Border::ASCII, Color::BRIGHT_YELLOW),
                border_card("Markdown", Border::MARKDOWN, Color::BRIGHT_RED),
            ),
            gap(19),
            pair(
                border_card("三線表", Border::BOOKTABS, Color::BRIGHT_CYAN),
                border_card("非表示", Border::HIDDEN, Color::BRIGHT_BLACK),
            ),
        ],
    )
}

fn side_card(label: &str, sample: View, blank_above: bool, blank_below: bool) -> View {
    let label = View::block(
        BlockStyle::new()
            .foreground(Color::BRIGHT_CYAN)
            .bold()
            .width(20)
            .align(Align::Center),
        View::text(
            label,
            TextStyle::new().foreground(Color::BRIGHT_CYAN).bold(),
        ),
    );
    let sample = View::block(BlockStyle::new().width(20).align(Align::Center), sample);
    let mut rows = vec![label];
    if blank_above {
        rows.push(gap(20));
    }
    rows.push(sample);
    if blank_below {
        rows.push(gap(20));
    }
    View::column(Align::Left, rows)
}

fn border_side_grid() -> View {
    let border = |style: BlockStyle| {
        boxed(
            style
                .border(Border::NORMAL)
                .border_foreground(Color::BRIGHT_MAGENTA),
            "内容",
        )
    };
    let top = side_card(
        "上辺のみ",
        border(
            BlockStyle::new()
                .border_right(false)
                .border_bottom(false)
                .border_left(false),
        ),
        false,
        true,
    );
    let right = side_card(
        "右辺のみ",
        border(
            BlockStyle::new()
                .border_top(false)
                .border_bottom(false)
                .border_left(false),
        ),
        true,
        true,
    );
    let bottom = side_card(
        "下辺のみ",
        border(
            BlockStyle::new()
                .border_top(false)
                .border_right(false)
                .border_left(false),
        ),
        true,
        false,
    );
    let left = side_card(
        "左辺のみ",
        border(
            BlockStyle::new()
                .border_top(false)
                .border_right(false)
                .border_bottom(false),
        ),
        true,
        true,
    );
    let centered = |view| View::block(BlockStyle::new().width(43).align(Align::Center), view);
    let middle = View::row(VerticalAlign::Top, [left, gap(2), right]);
    let heading = View::block(
        BlockStyle::new()
            .foreground(Color::BRIGHT_YELLOW)
            .bold()
            .width(43)
            .align(Align::Center),
        View::text(
            "辺ごとの罫線",
            TextStyle::new().foreground(Color::BRIGHT_YELLOW).bold(),
        ),
    );

    View::column(
        Align::Left,
        [
            heading,
            centered(top),
            gap(43),
            middle,
            gap(43),
            centered(bottom),
        ],
    )
}

fn component_theme() -> Theme {
    Theme::from_tokens(SemanticTokens {
        text: Color::WHITE,
        text_muted: Color::BRIGHT_BLACK,
        background: Color::BLACK,
        surface: Color::BLACK,
        accent: Color::CYAN,
        accent_text: Color::BLACK,
        success: Color::GREEN,
        warning: Color::YELLOW,
        error: Color::RED,
        border: Color::BRIGHT_BLACK,
    })
}

fn tree_sample() -> View {
    let tree = Tree::new()
        .root("うるし")
        .child(TreeNode::new("ソース").child("部品").child("描画"))
        .child("設定");

    component_theme().components().tree().view(&tree)
}

fn list_sample() -> View {
    let list = List::new()
        .item("設計する")
        .item(ListItem::new("実装する").items(["モデル", "ビュー"]))
        .item("検証する");
    let theme = component_theme();
    let list_style = theme
        .components()
        .list()
        .clone()
        .enumerator(arabic_enumerator);

    list_style.view(&list)
}

fn table_sample() -> View {
    let table = Table::new()
        .headers(["項目", "値"])
        .row(["表示", "有効"])
        .row(["色数", "16"]);

    component_theme().components().table().view(&table)
}

fn panel(title: &str, rows: Vec<View>) -> View {
    let body = View::column(Align::Left, rows);
    let width = measure(&body).width() as u16;
    let title = View::block(
        BlockStyle::new()
            .foreground(Color::BRIGHT_YELLOW)
            .bold()
            .width(width)
            .align(Align::Center),
        View::text(
            title,
            TextStyle::new().foreground(Color::BRIGHT_YELLOW).bold(),
        ),
    );
    let catalog = View::column(Align::Left, [title, body]);

    View::block(
        BlockStyle::new()
            .padding((0, 1))
            .border(Border::ROUNDED)
            .border_foreground(Color::BRIGHT_BLACK)
            .margin(1),
        catalog,
    )
}

/// Builds a readable Japanese catalog that exercises CJK display widths.
pub fn cjk_showcase_view() -> View {
    let card = |label| {
        boxed(
            BlockStyle::new()
                .padding((0, 1))
                .border(Border::ROUNDED)
                .border_foreground(Color::BRIGHT_GREEN),
            label,
        )
    };
    let rows = vec![
        row(
            "前景色",
            View::text(
                "水色の文字",
                TextStyle::new().foreground(Color::BRIGHT_CYAN),
            ),
        ),
        row(
            "背景色",
            View::text("青い領域", TextStyle::new().background(Color::BLUE)),
        ),
        row("太字", View::text("太い文字", TextStyle::new().bold())),
        row(
            "下線",
            View::text("下線付き文字", TextStyle::new().underline()),
        ),
        // 描画済みの文字列を入れ子にするのではなく、テキストを並べて合成する。
        row(
            "行内スタイル",
            View::row(
                VerticalAlign::Top,
                [
                    View::text("外側 ", TextStyle::new().foreground(Color::BRIGHT_GREEN)),
                    View::text("内側", TextStyle::new().foreground(Color::BRIGHT_RED)),
                    View::text(" 外側", TextStyle::new().foreground(Color::BRIGHT_GREEN)),
                ],
            ),
        ),
        // 行の中の block: 行全体が block の高さを持つ1つの矩形になる。
        row(
            "行内ブロック",
            View::row(
                VerticalAlign::Center,
                [
                    View::text("状態 ", TextStyle::new()),
                    card("完了"),
                    View::text(" です", TextStyle::new()),
                ],
            ),
        ),
        gap(43),
        row(
            "パディング",
            boxed(
                BlockStyle::new()
                    .background(Color::BRIGHT_BLACK)
                    .padding((1, 2)),
                "内容",
            ),
        ),
        row(
            "四辺の罫線",
            boxed(
                BlockStyle::new().padding((0, 1)).border(Border::ROUNDED),
                "内容",
            ),
        ),
        gap(43),
        row(
            "固定幅",
            boxed(
                BlockStyle::new().background(Color::BRIGHT_BLACK).width(20),
                "20列分を確保",
            ),
        ),
        gap(43),
        row_with_alignment(
            "固定高さ",
            boxed(
                BlockStyle::new().background(Color::BRIGHT_BLACK).height(3),
                "3行分を確保",
            ),
            VerticalAlign::Top,
        ),
        gap(43),
        row("垂直揃え", vertical_alignment_sample("上", "中央", "下")),
        gap(43),
        row("水平揃え", horizontal_alignment_sample("左", "中央", "右")),
        gap(43),
        row(
            "横並び",
            View::row(VerticalAlign::Top, [card("甲"), card("乙")]),
        ),
        row(
            "縦積み",
            View::column(Align::Left, [card("甲"), card("乙")]),
        ),
        gap(43),
        row_with_alignment("木構造", tree_sample(), VerticalAlign::Top),
        gap(43),
        row_with_alignment("箇条書き", list_sample(), VerticalAlign::Top),
        gap(43),
        row_with_alignment("表", table_sample(), VerticalAlign::Top),
        gap(43),
        row("罫線の種類", border_preset_sample()),
        border_side_grid(),
    ];

    panel("URUSHI スタイル・ショーケース", rows)
}

/// Renders the Japanese catalog for a 16-color terminal.
pub fn render_cjk_showcase() -> String {
    AnsiRenderer::new(TerminalProfile::new(
        ColorProfile::Ansi16,
        AnsiPolicy::Enabled,
    ))
    .render(&cjk_showcase_view())
    .into_string()
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    println!("{}", render_cjk_showcase());
}
