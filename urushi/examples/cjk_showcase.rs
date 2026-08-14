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

fn vertical_alignment_sample(top: &str, center: &str, bottom: &str) -> String {
    let cell = Style::new()
        .background(Color::BRIGHT_BLACK)
        .width(6)
        .height(5)
        .align(Align::Center);
    let top = cell.clone().align_vertical(VerticalAlign::Top).render(top);
    let center = cell
        .clone()
        .align_vertical(VerticalAlign::Center)
        .render(center);
    let bottom = cell.align_vertical(VerticalAlign::Bottom).render(bottom);

    join_horizontal(
        VerticalAlign::Top,
        &[top, " ".to_string(), center, " ".to_string(), bottom],
    )
}

fn horizontal_alignment_sample(left: &str, center: &str, right: &str) -> String {
    let cell = Style::new().background(Color::BRIGHT_BLACK).width(20);
    let left = cell.clone().align(Align::Left).render(left);
    let center = cell.clone().align(Align::Center).render(center);
    let right = cell.align(Align::Right).render(right);

    let gap = " ".repeat(20);

    join_vertical(Align::Left, &[left, gap.clone(), center, gap, right])
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

fn border_card(label: &str, border: Border, color: Color) -> String {
    let label = Style::new()
        .foreground(color)
        .bold()
        .width(9)
        .align(Align::Center)
        .render(label);
    let mut grid_style = Style::new().foreground(color);
    if border == Border::HIDDEN {
        grid_style = grid_style.background(Color::BRIGHT_BLACK);
    }
    let grid = grid_style.render(&junction_grid(border));

    join_vertical(Align::Left, &[label, grid])
}

fn border_preset_sample() -> String {
    let pair = |left, right| join_horizontal(VerticalAlign::Top, &[left, " ".to_string(), right]);
    join_vertical(
        Align::Left,
        &[
            pair(
                border_card("標準", Border::NORMAL, Color::BRIGHT_GREEN),
                border_card("角丸", Border::ROUNDED, Color::BRIGHT_CYAN),
            ),
            " ".repeat(19),
            pair(
                border_card("太線", Border::THICK, Color::BRIGHT_MAGENTA),
                border_card("二重線", Border::DOUBLE, Color::BRIGHT_BLUE),
            ),
            " ".repeat(19),
            pair(
                border_card("ASCII", Border::ASCII, Color::BRIGHT_YELLOW),
                border_card("Markdown", Border::MARKDOWN, Color::BRIGHT_RED),
            ),
            " ".repeat(19),
            pair(
                border_card("三線表", Border::BOOKTABS, Color::BRIGHT_CYAN),
                border_card("非表示", Border::HIDDEN, Color::BRIGHT_BLACK),
            ),
        ],
    )
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
        row("垂直揃え", vertical_alignment_sample("上", "中央", "下")),
        " ".repeat(43),
        row("水平揃え", horizontal_alignment_sample("左", "中央", "右")),
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
        " ".repeat(43),
        row("罫線の種類", border_preset_sample()),
        border_side_grid(),
    ];

    panel("URUSHI スタイル・ショーケース", &rows)
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    println!("{}", render_cjk_showcase());
}
