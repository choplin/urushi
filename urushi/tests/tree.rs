use std::{cell::Cell, fmt, rc::Rc};

use urushi::{
    Available, Color, SemanticTokens, StyledGrapheme, TextStyle, Theme, Tree, TreeNode,
    TreeNodePresentation, TreePosition, resolve,
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
struct Entry {
    id: usize,
    label: &'static str,
    selected: bool,
}

impl fmt::Display for Entry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label)
    }
}

#[test]
fn construction_is_uniform_outside_the_crate() {
    let empty = Tree::<String>::new();
    let node = TreeNode::new("nested").child("child");
    let tree = Tree::new().root("root").children(["first", "second"]);
    let owned = Tree::new()
        .root(String::from("root"))
        .child(String::from("child"));

    assert!(empty.child_nodes().is_empty());
    assert_eq!(*node.value(), "nested");
    assert_eq!(node.child_nodes().len(), 1);
    assert_eq!(plain(&theme().tree(&tree)), "root\n├── first\n└── second");
    assert_eq!(plain(&theme().tree(&owned)), "root\n└── child");
}

#[test]
fn typed_tree_and_node_presentation_are_usable_through_crate_root_exports() {
    let tree = Tree::new()
        .root(Entry {
            id: 1,
            label: "root",
            selected: true,
        })
        .child(Entry {
            id: 6,
            label: "skip",
            selected: false,
        })
        .child(
            TreeNode::new(Entry {
                id: 7,
                label: "parent",
                selected: true,
            })
            .children([
                TreeNode::new(Entry {
                    id: 8,
                    label: "hidden",
                    selected: false,
                })
                .hidden(true),
                TreeNode::new(Entry {
                    id: 9,
                    label: "child",
                    selected: false,
                }),
            ]),
        )
        .child(Entry {
            id: 10,
            label: "drop",
            selected: false,
        })
        .child_offset(1, 1);
    let selected = TextStyle::new().foreground(Color::GREEN);
    let selected_style = selected.clone();
    let nodes = TreeNodePresentation::new(|entry: &Entry, position| match position {
        TreePosition::Root => format!("{}:{}:root", entry.id, entry.label),
        TreePosition::Child { index, len, depth } => {
            format!("{}:{}:{index}/{len}@{depth}", entry.id, entry.label)
        }
    })
    .per_node_style(move |entry, _| entry.selected.then(|| selected_style.clone()));
    let theme = theme();
    let style_only = TreeNodePresentation::<Entry>::display()
        .per_node_style(|entry, _| entry.selected.then(TextStyle::new));
    assert_eq!(
        plain(&theme.components().tree().compose_with(&tree, &style_only)),
        "root\n└── parent\n    └── child"
    );
    let view = theme.components().tree().compose_with(&tree, &nodes);
    let resolved = resolve(&view, Available::NONE).unwrap();

    assert_eq!(tree.root_value().map(|entry| entry.id), Some(1));
    assert_eq!(tree.child_nodes()[1].value().id, 7);
    assert_eq!(
        plain(&view),
        "1:root:root\n└── 7:parent:0/1@0\n    └── 9:child:0/1@1"
    );
    assert_eq!(resolved.rows()[0][0].style(), &selected);
    assert_eq!(resolved.rows()[1][4].style(), &selected);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpaqueEntry<'a> {
    label: &'a str,
}

#[test]
fn custom_policy_borrows_non_display_state_and_is_snapshotted_during_composition() {
    let root_label = String::from("root");
    let child_label = String::from("child");
    let tree = Tree::new()
        .root(OpaqueEntry { label: &root_label })
        .child(OpaqueEntry {
            label: &child_label,
        });
    let prefix_text = String::from("entry");
    let prefix = Rc::new(prefix_text.as_str());
    let format_calls = Rc::new(Cell::new(0));
    let style_calls = Rc::new(Cell::new(0));
    let counted_formats = Rc::clone(&format_calls);
    let counted_styles = Rc::clone(&style_calls);
    let nodes = TreeNodePresentation::new(move |entry: &OpaqueEntry<'_>, position| {
        counted_formats.set(counted_formats.get() + 1);
        let suffix = match position {
            TreePosition::Root => "root".to_owned(),
            TreePosition::Child { index, .. } => format!("child-{index}"),
        };
        format!("{}:{}:{suffix}", prefix.as_ref(), entry.label)
    })
    .per_node_style(move |_, _| {
        counted_styles.set(counted_styles.get() + 1);
        None
    });

    let view = theme().components().tree().compose_with(&tree, &nodes);
    assert_eq!(format_calls.get(), 2);
    assert_eq!(style_calls.get(), 2);

    let _ = resolve(&view, Available::columns(20)).unwrap();
    let _ = resolve(&view, Available::columns(8)).unwrap();

    assert_eq!(format_calls.get(), 2);
    assert_eq!(style_calls.get(), 2);
    assert_eq!(plain(&view), "entry:root:root\n└── entry:child:child-0");
}
