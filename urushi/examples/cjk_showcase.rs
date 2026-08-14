use urushi::{
    Align, Border, Color, Style, VerticalAlign, join_horizontal, join_vertical, visible_width,
};

fn row(label: &str, sample: String) -> String {
    row_with_alignment(label, sample, VerticalAlign::Center)
}

fn row_with_alignment(label: &str, sample: String, alignment: VerticalAlign) -> String {
    let label = Style::new()
        .foreground(Color::BRIGHT_CYAN)
        .bold()
        .width(20)
        .align(Align::Right)
        .render(label);
    let divider = Style::new().foreground(Color::BRIGHT_BLACK).render(" | ");
    join_horizontal(alignment, &[label, divider, sample])
}

fn side_card(label: &str, sample: String, blank_above: bool, blank_below: bool) -> String {
    let label = Style::new()
        .foreground(Color::BRIGHT_CYAN)
        .bold()
        .width(20)
        .align(Align::Center)
        .render(label);
    let sample = Style::new().width(20).align(Align::Center).render(&sample);
    let blank = " ".repeat(20);
    let mut lines = vec![label];
    if blank_above {
        lines.push(blank.clone());
    }
    lines.push(sample);
    if blank_below {
        lines.push(blank);
    }
    join_vertical(Align::Left, &lines)
}

fn border_side_grid() -> String {
    let border = |style: Style| {
        style
            .border(Border::NORMAL)
            .border_foreground(Color::BRIGHT_MAGENTA)
            .render("内容")
    };
    let top = side_card(
        "上辺のみ",
        border(
            Style::new()
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
            Style::new()
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
            Style::new()
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
            Style::new()
                .border_top(false)
                .border_right(false)
                .border_bottom(false),
        ),
        true,
        true,
    );
    let top = Style::new().width(43).align(Align::Center).render(&top);
    let middle = join_horizontal(VerticalAlign::Top, &[left, "  ".to_string(), right]);
    let bottom = Style::new().width(43).align(Align::Center).render(&bottom);
    let heading = Style::new()
        .foreground(Color::BRIGHT_YELLOW)
        .bold()
        .width(43)
        .align(Align::Center)
        .render("辺ごとの罫線");

    join_vertical(
        Align::Left,
        &[heading, top, " ".repeat(43), middle, " ".repeat(43), bottom],
    )
}

fn panel(title: &str, rows: &[String]) -> String {
    let body = join_vertical(Align::Left, rows);
    let width = body.lines().map(visible_width).max().unwrap_or_default();
    let title = Style::new()
        .foreground(Color::BRIGHT_YELLOW)
        .bold()
        .width(width as u16)
        .align(Align::Center)
        .render(title);
    let catalog = join_vertical(Align::Left, &[title, body]);

    Style::new()
        .padding((0, 1))
        .border(Border::ROUNDED)
        .border_foreground(Color::BRIGHT_BLACK)
        .margin(1)
        .render(&catalog)
}

/// Renders a readable Japanese catalog that exercises CJK display widths.
pub fn render_cjk_showcase() -> String {
    let inner = Style::new().foreground(Color::BRIGHT_RED).render("内側");
    let rows = [
        row(
            "前景色",
            Style::new()
                .foreground(Color::BRIGHT_CYAN)
                .render("水色の文字"),
        ),
        row(
            "背景色",
            Style::new().background(Color::BLUE).render("青い領域"),
        ),
        row("太字", Style::new().bold().render("太い文字")),
        row("下線", Style::new().underline().render("下線付き文字")),
        row(
            "入れ子 ANSI",
            Style::new()
                .foreground(Color::BRIGHT_GREEN)
                .render(&format!("外側 {inner} 外側")),
        ),
        " ".repeat(43),
        row(
            "パディング",
            Style::new()
                .background(Color::BRIGHT_BLACK)
                .padding((1, 2))
                .render("内容"),
        ),
        row(
            "四辺の罫線",
            Style::new()
                .padding((0, 1))
                .border(Border::ROUNDED)
                .render("内容"),
        ),
        " ".repeat(43),
        row(
            "固定幅",
            Style::new()
                .background(Color::BRIGHT_BLACK)
                .width(20)
                .render("20列分を確保"),
        ),
        " ".repeat(43),
        row_with_alignment(
            "固定高さ",
            Style::new()
                .background(Color::BRIGHT_BLACK)
                .height(3)
                .render("3行分を確保"),
            VerticalAlign::Top,
        ),
        " ".repeat(43),
        row(
            "左揃え",
            Style::new()
                .background(Color::BRIGHT_BLACK)
                .width(20)
                .align(Align::Left)
                .render("左"),
        ),
        row(
            "中央揃え",
            Style::new()
                .background(Color::BRIGHT_BLACK)
                .width(20)
                .align(Align::Center)
                .render("中央"),
        ),
        row(
            "右揃え",
            Style::new()
                .background(Color::BRIGHT_BLACK)
                .width(20)
                .align(Align::Right)
                .render("右"),
        ),
        " ".repeat(43),
        row(
            "横結合",
            join_horizontal(
                VerticalAlign::Top,
                &[
                    Style::new()
                        .padding((0, 1))
                        .border(Border::ROUNDED)
                        .border_foreground(Color::BRIGHT_GREEN)
                        .render("甲"),
                    Style::new()
                        .padding((0, 1))
                        .border(Border::ROUNDED)
                        .border_foreground(Color::BRIGHT_GREEN)
                        .render("乙"),
                ],
            ),
        ),
        row(
            "縦結合",
            join_vertical(
                Align::Left,
                &[
                    Style::new()
                        .padding((0, 1))
                        .border(Border::ROUNDED)
                        .border_foreground(Color::BRIGHT_GREEN)
                        .render("甲"),
                    Style::new()
                        .padding((0, 1))
                        .border(Border::ROUNDED)
                        .border_foreground(Color::BRIGHT_GREEN)
                        .render("乙"),
                ],
            ),
        ),
        border_side_grid(),
    ];

    panel("URUSHI スタイル・ショーケース", &rows)
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    println!("{}", render_cjk_showcase());
}
