use std::{fmt, rc::Rc};

use urushi::{
    Available, Color, List, ListItem, ListItemPresentation, ListRole, SemanticTokens,
    StyledGrapheme, TextStyle, Theme, resolve,
};

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

fn plain(view: &urushi::View) -> String {
    resolve(view, Available::NONE)
        .unwrap()
        .rows()
        .iter()
        .map(|row| {
            row.iter()
                .map(StyledGrapheme::symbol)
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Task {
    id: usize,
    label: &'static str,
    selected: bool,
}

impl fmt::Display for Task {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label)
    }
}

#[test]
fn construction_is_uniform_outside_the_crate() {
    let empty = List::<String>::new();
    let item = ListItem::new("nested").item("child");
    let list = List::new().items(["first", "second"]);
    let owned = List::new().item(String::from("owned"));

    assert!(empty.item_nodes().is_empty());
    assert_eq!(*item.value(), "nested");
    assert_eq!(plain(&theme().list(&list)), "• first\n• second");
    assert_eq!(plain(&theme().list(&owned)), "• owned");
}

#[test]
fn typed_list_and_item_presentation_are_usable_through_crate_root_exports() {
    let list = List::<Task>::new()
        .item(Task {
            id: 1,
            label: "parent",
            selected: true,
        })
        .item(
            ListItem::new(Task {
                id: 2,
                label: "nested",
                selected: false,
            })
            .item(ListItem::new(Task {
                id: 3,
                label: "child",
                selected: false,
            })),
        );
    let theme = theme();

    assert_eq!(plain(&theme.list(&list)), "• parent\n• nested\n  • child");

    let selected_item = TextStyle::new().foreground(Color::GREEN);
    let selected_enumerator = TextStyle::new().foreground(Color::BLUE);
    let item_style = selected_item.clone();
    let enumerator_style = selected_enumerator.clone();
    let items = ListItemPresentation::<Task>::display().per_item_style(move |task, _, role| {
        task.selected.then(|| match role {
            ListRole::Item => item_style.clone(),
            ListRole::Enumerator => enumerator_style.clone(),
        })
    });
    let view = theme.components().list().compose_with(&list, &items);
    let resolved = resolve(&view, Available::NONE).unwrap();

    assert_eq!(plain(&view), "• parent\n• nested\n  • child");
    assert_eq!(resolved.rows()[0][0].style(), &selected_enumerator);
    assert_eq!(resolved.rows()[0][2].style(), &selected_item);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpaqueTask<'a> {
    id: usize,
    label: &'a str,
}

#[test]
fn custom_formatter_accepts_borrowed_non_display_values() {
    let first = String::from("first");
    let second = String::from("second");
    let list = List::new().items([
        OpaqueTask {
            id: 1,
            label: &first,
        },
        OpaqueTask {
            id: 2,
            label: &second,
        },
    ]);
    let prefix_text = String::from("task");
    let prefix = Rc::new(prefix_text.as_str());
    let items = ListItemPresentation::new(move |task: &OpaqueTask, position| {
        format!(
            "{}-{}:{}@{}",
            prefix.as_ref(),
            task.id,
            task.label,
            position.index()
        )
    });

    let view = theme().components().list().compose_with(&list, &items);

    assert_eq!(plain(&view), "• task-1:first@0\n• task-2:second@1");
}
