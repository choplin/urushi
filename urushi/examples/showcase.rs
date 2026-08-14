use urushi::{
    Align, AnsiPolicy, AnsiRenderer, Border, Color, ColorProfile, SemanticTokens, Style,
    TerminalProfile, Theme, Tree, TreeNode, VerticalAlign, join_horizontal, join_vertical,
    visible_width,
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
    let content = join_horizontal(alignment, &[label, divider, sample]);

    Style::new().width(43).render(&content)
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

fn maximum_width_sample() -> String {
    let heading = |label| Style::new().width(20).align(Align::Center).render(label);
    let before = Style::new()
        .background(Color::BRIGHT_BLACK)
        .width(20)
        .render("abcdefghijklmnopqrst");
    let after = Style::new()
        .background(Color::BRIGHT_BLACK)
        .width(20)
        .max_width(12)
        .render("abcdefghijklmnopqrst");

    join_vertical(
        Align::Left,
        &[heading("BEFORE"), before, heading("AFTER"), after],
    )
}

fn maximum_height_sample() -> String {
    let heading = |label| Style::new().width(9).align(Align::Center).render(label);
    let cell = Style::new()
        .background(Color::BRIGHT_BLACK)
        .width(9)
        .height(6)
        .align(Align::Center);
    let before = join_vertical(
        Align::Left,
        &[heading("BEFORE"), cell.clone().render("a\nb\nc\nd\ne\nf")],
    );
    let after = join_vertical(
        Align::Left,
        &[
            heading("AFTER"),
            cell.max_height(3).render("a\nb\nc\nd\ne\nf"),
        ],
    );

    join_horizontal(VerticalAlign::Top, &[before, "  ".to_string(), after])
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

fn tree_sample() -> String {
    let theme = Theme::from_tokens(SemanticTokens {
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
    });
    let tree = Tree::new()
        .root("urushi")
        .child(
            TreeNode::new("src")
                .child("component")
                .child("render")
                .child("theme"),
        )
        .child("Cargo.toml");
    let renderer = AnsiRenderer::new(TerminalProfile::new(
        ColorProfile::Ansi16,
        AnsiPolicy::Enabled,
    ));

    renderer.render(&tree.view(theme.components()))
}

fn section(title: &str, rows: &[String]) -> String {
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
        .margin((0, 1))
        .render(&catalog)
}

/// Renders a readable catalog with one primary feature per row.
pub fn render_showcase() -> String {
    let inner = Style::new().foreground(Color::BRIGHT_RED).render("inner");
    let heading = Style::new()
        .foreground(Color::BRIGHT_YELLOW)
        .bold()
        .width(49)
        .align(Align::Center)
        .render("URUSHI STYLE SHOWCASE");
    let sections = [
        section(
            "COLOR & TEXT",
            &[
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
            ],
        ),
        section(
            "BOX MODEL",
            &[
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
                row_with_alignment("MAX WIDTH", maximum_width_sample(), VerticalAlign::Top),
                " ".repeat(43),
                row_with_alignment("MAX HEIGHT", maximum_height_sample(), VerticalAlign::Top),
                " ".repeat(43),
                border_side_grid(),
            ],
        ),
        section(
            "ALIGNMENT",
            &[
                row(
                    "VERTICAL ALIGNMENT",
                    vertical_alignment_sample("TOP", "CENTER", "BOTTOM"),
                ),
                " ".repeat(43),
                row(
                    "HORIZONTAL ALIGNMENT",
                    horizontal_alignment_sample("LEFT", "CENTER", "RIGHT"),
                ),
            ],
        ),
        section(
            "COMPOSITION",
            &[
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
            ],
        ),
        section(
            "COMPONENTS",
            &[row_with_alignment(
                "TREE",
                tree_sample(),
                VerticalAlign::Top,
            )],
        ),
    ];
    let gap = " ".repeat(49);
    let mut blocks = vec![heading, gap.clone()];
    for (index, section) in sections.into_iter().enumerate() {
        if index > 0 {
            blocks.push(gap.clone());
        }
        blocks.push(section);
    }

    join_vertical(Align::Left, &blocks)
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    println!("{}", render_showcase());
}
