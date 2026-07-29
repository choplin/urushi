use urushi::{Align, Border, Color, Style, VerticalAlign, join_horizontal, join_vertical};

/// Renders the complete MVP composition shown by this example.
pub fn render_showcase() -> String {
    let title_content = Style::new()
        .foreground(Color::BRIGHT_YELLOW)
        .underline()
        .render("urushi\n日本語も、きれいに。");
    let title = Style::new()
        .background(Color::BLUE)
        .bold()
        .padding((0, 1))
        .width(42)
        .align(Align::Center)
        .border(Border::THICK)
        .border_foreground(Color::BRIGHT_CYAN)
        .render(&title_content);

    let lacquer = Style::new()
        .foreground(Color::BRIGHT_WHITE)
        .background(Color::MAGENTA)
        .padding(1)
        .width(20)
        .align(Align::Center)
        .border(Border::ROUNDED)
        .border_foreground(Color::BRIGHT_MAGENTA)
        .render("漆\nCJK 幅: 正確");
    let layout = Style::new()
        .foreground(Color::BRIGHT_WHITE)
        .background(Color::GREEN)
        .padding((0, 1))
        .width(20)
        .align(Align::Center)
        .border(Border::DOUBLE)
        .border_foreground(Color::BRIGHT_GREEN)
        .render("横に join\n縦にも join");
    let cards = join_horizontal(VerticalAlign::Center, &[lacquer, layout]);

    let footer = Style::new()
        .foreground(Color::BRIGHT_BLACK)
        .padding((0, 1))
        .width(30)
        .align(Align::Center)
        .border(Border::NORMAL)
        .border_foreground(Color::BRIGHT_BLACK)
        .render("ANSI を保ったまま組み立てる");

    join_vertical(Align::Center, &[title, cards, footer])
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    println!("{}", render_showcase());
}
