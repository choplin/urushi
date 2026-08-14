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
            .render("content")
    };
    let top = side_card(
        "BORDER TOP ONLY",
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
        "BORDER RIGHT ONLY",
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
        "BORDER BOTTOM ONLY",
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
        "BORDER LEFT ONLY",
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
        .render("PER-SIDE BORDER");

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

/// Renders a readable catalog with one primary feature per row.
pub fn render_showcase() -> String {
    let inner = Style::new().foreground(Color::BRIGHT_RED).render("inner");
    let rows = [
        row(
            "FOREGROUND COLOR",
            Style::new()
                .foreground(Color::BRIGHT_CYAN)
                .render("cyan text"),
        ),
        row(
            "BACKGROUND COLOR",
            Style::new().background(Color::BLUE).render("blue field"),
        ),
        row("BOLD", Style::new().bold().render("bold text")),
        row(
            "UNDERLINE",
            Style::new().underline().render("underlined text"),
        ),
        row(
            "NESTED ANSI",
            Style::new()
                .foreground(Color::BRIGHT_GREEN)
                .render(&format!("outer {inner} outer")),
        ),
        " ".repeat(43),
        row(
            "PADDING",
            Style::new()
                .background(Color::BRIGHT_BLACK)
                .padding((1, 2))
                .render("content"),
        ),
        row(
            "FULL BORDER",
            Style::new()
                .padding((0, 1))
                .border(Border::ROUNDED)
                .render("content"),
        ),
        " ".repeat(43),
        row(
            "FIXED WIDTH",
            Style::new()
                .background(Color::BRIGHT_BLACK)
                .width(20)
                .render("reserves 20 cols"),
        ),
        " ".repeat(43),
        row_with_alignment(
            "FIXED HEIGHT",
            Style::new()
                .background(Color::BRIGHT_BLACK)
                .height(3)
                .render("reserves 3 rows"),
            VerticalAlign::Top,
        ),
        " ".repeat(43),
        row(
            "VERTICAL ALIGNMENT",
            vertical_alignment_sample("TOP", "CENTER", "BOTTOM"),
        ),
        " ".repeat(43),
        row(
            "HORIZONTAL ALIGNMENT",
            horizontal_alignment_sample("LEFT", "CENTER", "RIGHT"),
        ),
        " ".repeat(43),
        row(
            "JOIN HORIZONTAL",
            join_horizontal(
                VerticalAlign::Top,
                &[
                    Style::new()
                        .padding((0, 1))
                        .border(Border::ROUNDED)
                        .border_foreground(Color::BRIGHT_GREEN)
                        .render("A"),
                    Style::new()
                        .padding((0, 1))
                        .border(Border::ROUNDED)
                        .border_foreground(Color::BRIGHT_GREEN)
                        .render("B"),
                ],
            ),
        ),
        row(
            "JOIN VERTICAL",
            join_vertical(
                Align::Left,
                &[
                    Style::new()
                        .padding((0, 1))
                        .border(Border::ROUNDED)
                        .border_foreground(Color::BRIGHT_GREEN)
                        .render("A"),
                    Style::new()
                        .padding((0, 1))
                        .border(Border::ROUNDED)
                        .border_foreground(Color::BRIGHT_GREEN)
                        .render("B"),
                ],
            ),
        ),
        border_side_grid(),
    ];

    panel("URUSHI STYLE SHOWCASE", &rows)
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    println!("{}", render_showcase());
}
