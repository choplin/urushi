use urushi::{
    Align, AnsiPolicy, AnsiRenderer, BlockStyle, Border, Color, ColorProfile, List, ListItem,
    SemanticTokens, Table, TableBorder, TableCell, TableCellStyler, TablePresentation,
    TerminalProfile, TextStyle, Theme, Tree, TreeNode, VerticalAlign, View, arabic_enumerator,
    measure,
};

/// Blank space of a fixed width, used to separate samples.
fn gap(width: usize) -> View {
    View::text(" ".repeat(width), TextStyle::new())
}

/// Fits one sample into the catalog's content column.
fn fit(width: u16, view: View) -> View {
    View::block(BlockStyle::new().width(width), view)
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

    fit(43, View::row(alignment, [label, divider, sample]))
}

fn vertical_alignment_sample(top: &str, center: &str, bottom: &str) -> View {
    let cell = BlockStyle::new()
        .background(Color::BRIGHT_BLACK)
        .width(6)
        .height(5)
        .align(Align::Center);
    let filled = |style: BlockStyle, text: &str| {
        let content = View::text(text, style.text().clone());
        View::block(style, content)
    };

    View::row(
        VerticalAlign::Top,
        [
            filled(cell.clone().align_vertical(VerticalAlign::Top), top),
            gap(1),
            filled(cell.clone().align_vertical(VerticalAlign::Center), center),
            gap(1),
            filled(cell.align_vertical(VerticalAlign::Bottom), bottom),
        ],
    )
}

fn horizontal_alignment_sample(left: &str, center: &str, right: &str) -> View {
    let cell = BlockStyle::new().background(Color::BRIGHT_BLACK).width(20);
    let filled = |style: BlockStyle, text: &str| {
        let content = View::text(text, style.text().clone());
        View::block(style, content)
    };

    View::column(
        Align::Left,
        [
            filled(cell.clone().align(Align::Left), left),
            gap(20),
            filled(cell.clone().align(Align::Center), center),
            gap(20),
            filled(cell.align(Align::Right), right),
        ],
    )
}

fn maximum_width_sample() -> View {
    let heading = |label: &str| {
        View::block(
            BlockStyle::new().width(20).align(Align::Center),
            View::text(label, TextStyle::new()),
        )
    };
    let cell = BlockStyle::new().background(Color::BRIGHT_BLACK).width(20);
    let content = || View::text("abcdefghijklmnopqrst", TextStyle::new());

    View::column(
        Align::Left,
        [
            heading("BEFORE"),
            View::block(cell.clone(), content()),
            heading("AFTER"),
            View::block(cell.max_width(12), content()),
        ],
    )
}

fn maximum_height_sample() -> View {
    let heading = |label: &str| {
        View::block(
            BlockStyle::new().width(9).align(Align::Center),
            View::text(label, TextStyle::new()),
        )
    };
    let cell = BlockStyle::new()
        .background(Color::BRIGHT_BLACK)
        .width(9)
        .height(6)
        .align(Align::Center);
    let content = || View::text("a\nb\nc\nd\ne\nf", TextStyle::new());
    let before = View::column(
        Align::Left,
        [heading("BEFORE"), View::block(cell.clone(), content())],
    );
    let after = View::column(
        Align::Left,
        [heading("AFTER"), View::block(cell.max_height(3), content())],
    );

    View::row(VerticalAlign::Top, [before, gap(2), after])
}

fn border_card(label: &str, border: TableBorder, color: Color) -> View {
    let label = View::block(
        BlockStyle::new()
            .foreground(color)
            .bold()
            .width(9)
            .align(Align::Center),
        View::text(label, TextStyle::new().foreground(color).bold()),
    );
    let mut cell_style = BlockStyle::new();
    let mut border_style = TextStyle::new().foreground(color);
    if border == TableBorder::HIDDEN {
        cell_style = cell_style.background(Color::BRIGHT_BLACK);
        border_style = border_style.background(Color::BRIGHT_BLACK);
    }
    let grid = TablePresentation::new(cell_style.clone(), cell_style, border_style)
        .border(border)
        .width(9)
        .compose(&Table::new().headers(["A", "B"]).row(["C", "D"]));

    View::column(Align::Left, [label, grid])
}

fn border_preset_sample() -> View {
    let pair = |left, right| View::row(VerticalAlign::Top, [left, gap(1), right]);
    View::column(
        Align::Left,
        [
            pair(
                border_card("NORMAL", TableBorder::NORMAL, Color::BRIGHT_GREEN),
                border_card("ROUNDED", TableBorder::ROUNDED, Color::BRIGHT_CYAN),
            ),
            gap(19),
            pair(
                border_card("THICK", TableBorder::THICK, Color::BRIGHT_MAGENTA),
                border_card("DOUBLE", TableBorder::DOUBLE, Color::BRIGHT_BLUE),
            ),
            gap(19),
            pair(
                border_card("ASCII", TableBorder::ASCII, Color::BRIGHT_YELLOW),
                border_card("MARKDOWN", TableBorder::MARKDOWN, Color::BRIGHT_RED),
            ),
            gap(19),
            pair(
                border_card("BOOKTABS", TableBorder::BOOKTABS, Color::BRIGHT_CYAN),
                border_card("HIDDEN", TableBorder::HIDDEN, Color::BRIGHT_BLACK),
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
        View::block(
            style
                .border(Border::NORMAL)
                .border_foreground(Color::BRIGHT_MAGENTA),
            View::text("content", TextStyle::new()),
        )
    };
    let top = side_card(
        "BORDER TOP ONLY",
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
        "BORDER RIGHT ONLY",
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
        "BORDER BOTTOM ONLY",
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
        "BORDER LEFT ONLY",
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
            "PER-SIDE BORDER",
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

fn sample_theme() -> Theme {
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
        .root("urushi")
        .child(
            TreeNode::new("src")
                .child("component")
                .child("render")
                .child("theme"),
        )
        .child("Cargo.toml");

    sample_theme().components().tree().compose(&tree)
}

fn list_sample() -> View {
    let list = List::new()
        .item("Define the API")
        .item(ListItem::new("Implement").items(["model", "view"]))
        .item("Verify behavior");
    let theme = sample_theme();
    let presentation = theme
        .components()
        .list()
        .clone()
        .enumerator(arabic_enumerator);

    presentation.compose(&list)
}

fn table_sample() -> View {
    let table = Table::new()
        .headers(["Key", "Value"])
        .row(["mode", "plain"])
        .row(["theme", "dark"]);

    sample_theme().table(&table)
}

/// Styles every cell from its own coordinates: one checkerboard covering the
/// header and the body, with right-aligned numeric columns.
///
/// A style the strategy returns replaces the role default rather than layering
/// over it, so it restates the foreground it wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Checkerboard;

impl TableCellStyler for Checkerboard {
    fn style(&self, cell: TableCell<'_>) -> Option<BlockStyle> {
        // The header sits one row above the first body row, so it continues the
        // same board instead of starting a new one.
        let parity = match cell.row() {
            Some(row) => row + cell.column(),
            None => cell.column() + 1,
        };
        // Both shades must differ from the surrounding backdrop, or the board reads
        // as detached blocks instead of alternating squares.
        let shade = if parity % 2 == 0 {
            Color::BRIGHT_BLACK
        } else {
            Color::BLUE
        };
        let align = if cell.column() == 0 {
            Align::Left
        } else {
            Align::Right
        };

        let square = BlockStyle::new()
            .foreground(Color::WHITE)
            .background(shade)
            .align(align);
        Some(if cell.is_header() {
            square.bold()
        } else {
            square
        })
    }
}

/// Shows the presentation policy a caller can vary: every border edge off, a
/// total width, and a per-cell strategy driven by row and column.
fn table_style_sample() -> View {
    let table = Table::new()
        .headers(["Cmd", "Ok", "Err"])
        .row(["build", "12", "0"])
        .row(["test", "340", "2"])
        .row(["lint", "97", "1"]);
    let theme = sample_theme();
    let table_style = theme
        .components()
        .table()
        .clone()
        .border_top(false)
        .border_bottom(false)
        .border_left(false)
        .border_right(false)
        .border_header(false)
        .border_column(false)
        .width(20)
        .cell_styler(Checkerboard);

    table_style.compose(&table)
}

fn section(title: &str, rows: Vec<View>) -> View {
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
            .margin((0, 1)),
        catalog,
    )
}

/// Builds a readable catalog with one primary feature per row.
pub fn showcase_view() -> View {
    let heading = View::block(
        BlockStyle::new()
            .foreground(Color::BRIGHT_YELLOW)
            .bold()
            .width(49)
            .align(Align::Center),
        View::text(
            "URUSHI STYLE SHOWCASE",
            TextStyle::new().foreground(Color::BRIGHT_YELLOW).bold(),
        ),
    );
    let boxed = |style: BlockStyle, text: &str| {
        let content = View::text(text, style.text().clone());
        View::block(style, content)
    };
    let sections = [
        section(
            "COLOR & TEXT",
            vec![
                row(
                    "FOREGROUND COLOR",
                    View::text("cyan text", TextStyle::new().foreground(Color::BRIGHT_CYAN)),
                ),
                row(
                    "BACKGROUND COLOR",
                    View::text("blue field", TextStyle::new().background(Color::BLUE)),
                ),
                row("BOLD", View::text("bold text", TextStyle::new().bold())),
                row(
                    "UNDERLINE",
                    View::text("underlined text", TextStyle::new().underline()),
                ),
                row(
                    "HYPERLINK",
                    View::text(
                        "open docs.rs",
                        TextStyle::new().hyperlink("https://docs.rs/urushi"),
                    ),
                ),
                // Styles compose by placing text beside text, not by nesting
                // rendered output inside a style.
                row(
                    "INLINE STYLES",
                    View::row(
                        VerticalAlign::Top,
                        [
                            View::text("outer ", TextStyle::new().foreground(Color::BRIGHT_GREEN)),
                            View::text("inner", TextStyle::new().foreground(Color::BRIGHT_RED)),
                            View::text(" outer", TextStyle::new().foreground(Color::BRIGHT_GREEN)),
                        ],
                    ),
                ),
                // A bordered block inside a row: the row is one rectangle as
                // tall as the block, which a line of spans could not express.
                row(
                    "INLINE BLOCK",
                    View::row(
                        VerticalAlign::Center,
                        [
                            View::text("status ", TextStyle::new()),
                            boxed(
                                BlockStyle::new()
                                    .padding((0, 1))
                                    .border(Border::ROUNDED)
                                    .border_foreground(Color::BRIGHT_GREEN)
                                    .foreground(Color::BRIGHT_GREEN),
                                "ok",
                            ),
                            View::text(" done", TextStyle::new()),
                        ],
                    ),
                ),
            ],
        ),
        section(
            "BOX MODEL",
            vec![
                row(
                    "PADDING",
                    boxed(
                        BlockStyle::new()
                            .background(Color::BRIGHT_BLACK)
                            .padding((1, 2)),
                        "content",
                    ),
                ),
                row(
                    "FULL BORDER",
                    boxed(
                        BlockStyle::new().padding((0, 1)).border(Border::ROUNDED),
                        "content",
                    ),
                ),
                row(
                    "FIXED WIDTH",
                    boxed(
                        BlockStyle::new().background(Color::BRIGHT_BLACK).width(20),
                        "reserves 20 cols",
                    ),
                ),
                gap(43),
                row_with_alignment(
                    "FIXED HEIGHT",
                    boxed(
                        BlockStyle::new().background(Color::BRIGHT_BLACK).height(3),
                        "reserves 3 rows",
                    ),
                    VerticalAlign::Top,
                ),
                gap(43),
                row_with_alignment("MAX WIDTH", maximum_width_sample(), VerticalAlign::Top),
                gap(43),
                row_with_alignment("MAX HEIGHT", maximum_height_sample(), VerticalAlign::Top),
                gap(43),
                row_with_alignment("BORDER PRESETS", border_preset_sample(), VerticalAlign::Top),
                gap(43),
                border_side_grid(),
            ],
        ),
        section(
            "ALIGNMENT",
            vec![
                row(
                    "VERTICAL ALIGNMENT",
                    vertical_alignment_sample("TOP", "CENTER", "BOTTOM"),
                ),
                gap(43),
                row(
                    "HORIZONTAL ALIGNMENT",
                    horizontal_alignment_sample("LEFT", "CENTER", "RIGHT"),
                ),
            ],
        ),
        section(
            "COMPOSITION",
            vec![
                row("ROW", {
                    let card = |label| {
                        boxed(
                            BlockStyle::new()
                                .padding((0, 1))
                                .border(Border::ROUNDED)
                                .border_foreground(Color::BRIGHT_GREEN),
                            label,
                        )
                    };
                    View::row(VerticalAlign::Top, [card("A"), card("B")])
                }),
                row("COLUMN", {
                    let card = |label| {
                        boxed(
                            BlockStyle::new()
                                .padding((0, 1))
                                .border(Border::ROUNDED)
                                .border_foreground(Color::BRIGHT_GREEN),
                            label,
                        )
                    };
                    View::column(Align::Left, [card("A"), card("B")])
                }),
            ],
        ),
        section(
            "COMPONENTS",
            vec![
                row_with_alignment("TREE", tree_sample(), VerticalAlign::Top),
                row_with_alignment("LIST", list_sample(), VerticalAlign::Top),
                row_with_alignment("TABLE", table_sample(), VerticalAlign::Top),
                gap(43),
                row_with_alignment("TABLE STYLE", table_style_sample(), VerticalAlign::Top),
            ],
        ),
    ];

    let mut blocks = vec![heading, gap(49)];
    for (index, section) in sections.into_iter().enumerate() {
        if index > 0 {
            blocks.push(gap(49));
        }
        blocks.push(section);
    }

    View::column(Align::Left, blocks)
}

/// Renders the catalog for a terminal that keeps every color as written.
pub fn render_showcase() -> String {
    AnsiRenderer::new(TerminalProfile::new(
        ColorProfile::TrueColor,
        AnsiPolicy::Enabled,
    ))
    .render(&showcase_view())
    .into_string()
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    println!("{}", render_showcase());
}
