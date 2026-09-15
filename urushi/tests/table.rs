use std::cell::Cell;

use urushi::{
    Align, Available, BlockStyle, Color, SemanticTokens, Table, TablePresentation,
    TableRowPresentation, TextStyle, Theme, resolve,
};

#[derive(urushi::TableRow)]
struct Process<'a> {
    name: &'a str,
    pid: u32,
    #[table(skip)]
    selected: bool,
}

fn plain(view: &urushi::View) -> String {
    resolve(view, Available::NONE)
        .unwrap()
        .rows()
        .iter()
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn theme() -> Theme {
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

#[test]
fn derived_rows_format_display_fields_and_skip_application_state() {
    let table = Table::new().headers(["Name", "PID"]).row(Process {
        name: "server",
        pid: 42,
        selected: true,
    });
    let presentation =
        TablePresentation::new(BlockStyle::new(), BlockStyle::new(), TextStyle::new());

    assert_eq!(
        plain(&presentation.compose(&table)),
        "┌────────┬─────┐\n│ Name   │ PID │\n├────────┼─────┤\n│ server │ 42  │\n└────────┴─────┘"
    );
}

#[test]
fn typed_cell_styles_receive_the_column_or_table_fallback() {
    let table = Table::new().row(Process {
        name: "server",
        pid: 42,
        selected: true,
    });
    let base = BlockStyle::new().foreground(Color::WHITE);
    let numeric = base.clone().foreground(Color::CYAN).align(Align::Right);
    let presentation = TablePresentation::new(BlockStyle::new(), base, TextStyle::new())
        .column_styles([None, Some(numeric)]);
    let calls = Cell::new(0);
    let rows = TableRowPresentation::<Process<'_>>::display().cell_style(
        |process, cell, fallback_style| {
            calls.set(calls.get() + 1);
            (process.selected && cell.column() == 1)
                .then(|| fallback_style.clone().foreground(Color::YELLOW).bold())
        },
    );

    let resolved = resolve(&presentation.compose_with(&table, &rows), Available::NONE).unwrap();
    assert_eq!(calls.get(), 2, "policy is snapshotted once per body cell");
    assert_eq!(
        resolved.rows()[1][2].style().foreground_color(),
        Some(Color::WHITE)
    );
    assert_eq!(
        resolved.rows()[1][11].style().foreground_color(),
        Some(Color::YELLOW)
    );
    assert!(
        resolved.rows()[1][11]
            .style()
            .modifiers()
            .contains(urushi::Modifier::BOLD)
    );
}

#[test]
fn positional_and_cell_styles_are_complete_replacements_in_precedence_order() {
    let table = Table::new().headers(["Value"]).row(Process {
        name: "server",
        pid: 42,
        selected: true,
    });
    let presentation = TablePresentation::new(
        BlockStyle::new().foreground(Color::WHITE).bold(),
        BlockStyle::new().foreground(Color::WHITE).bold(),
        TextStyle::new(),
    )
    .header_styles([Some(BlockStyle::new().foreground(Color::CYAN))])
    .column_styles([Some(BlockStyle::new().foreground(Color::GREEN).bold())]);
    let rows = TableRowPresentation::<Process<'_>>::display().cell_style(|_, cell, _| {
        (cell.column() == 0).then(|| BlockStyle::new().foreground(Color::YELLOW))
    });

    let resolved = resolve(&presentation.compose_with(&table, &rows), Available::NONE).unwrap();
    let header = resolved.rows()[1][2].style();
    let body = resolved.rows()[3][2].style();
    assert_eq!(header.foreground_color(), Some(Color::CYAN));
    assert!(!header.modifiers().contains(urushi::Modifier::BOLD));
    assert_eq!(body.foreground_color(), Some(Color::YELLOW));
    assert!(!body.modifiers().contains(urushi::Modifier::BOLD));
}

#[test]
fn direct_text_rows_share_the_canonical_table_path_and_stay_ragged() {
    let table = Table::text()
        .headers(["A"])
        .row(["one", "two"])
        .row(["three"]);
    let presentation =
        TablePresentation::new(BlockStyle::new(), BlockStyle::new(), TextStyle::new());

    assert_eq!(table.column_count(), 2);
    assert!(plain(&presentation.compose(&table)).contains("│ three │     │"));
}

#[test]
fn custom_formatters_are_borrowed_and_run_only_during_composition() {
    struct Metric {
        value: u32,
    }

    let table = Table::new()
        .row(Metric { value: 1 })
        .row(Metric { value: 42 })
        .row(Metric { value: 99 })
        .offset(1, 1);
    let calls = Cell::new(0);
    let rows = TableRowPresentation::new(|metric: &Metric, position, cells| {
        calls.set(calls.get() + 1);
        assert_eq!(position.index(), 0);
        assert_eq!(position.len(), 1);
        cells.text(format!("{}%", metric.value));
    });
    let presentation =
        TablePresentation::new(BlockStyle::new(), BlockStyle::new(), TextStyle::new());

    let view = presentation.compose_with(&table, &rows);
    assert_eq!(calls.get(), 1);
    let _ = resolve(&view, Available::NONE).unwrap();
    let _ = resolve(&view, Available::NONE).unwrap();
    assert_eq!(
        calls.get(),
        1,
        "formatter output is snapshotted in the view"
    );
    assert!(plain(&view).contains("42%"));
}

#[test]
fn theme_shortcut_is_canonical_and_custom_rows_use_the_presentation() {
    let table = Table::new().row(Process {
        name: "server",
        pid: 42,
        selected: true,
    });
    let theme = theme();
    let rows = TableRowPresentation::new(|process: &Process<'_>, _, cells| {
        cells.text(process.name.to_uppercase());
    });

    assert_eq!(
        theme.table(&table),
        theme.components().table().compose(&table)
    );
    assert!(plain(&theme.components().table().compose_with(&table, &rows)).contains("SERVER"));
}
