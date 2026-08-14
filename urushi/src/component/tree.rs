//! Renderer-neutral trees with reusable owned nodes.

use crate::{ComponentStyles, Line, Style, TreeRole, View, visible_width};

/// An item's position among its visible siblings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiblingPosition {
    index: usize,
    len: usize,
}

impl SiblingPosition {
    /// Creates a position that component renderers can share.
    pub const fn new(index: usize, len: usize) -> Self {
        Self { index, len }
    }

    /// Returns the zero-based visible sibling index.
    pub const fn index(self) -> usize {
        self.index
    }

    /// Returns the number of visible siblings.
    pub const fn len(self) -> usize {
        self.len
    }

    /// Returns whether there are no visible siblings.
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Returns whether this is the final visible sibling.
    pub const fn is_last(self) -> bool {
        self.len > 0 && self.index == self.len - 1
    }
}

/// Produces the single-line marker drawn before one visible tree node.
///
/// Line breaks in returned markers are normalized to spaces so a marker cannot
/// violate the one-horizontal-row contract of [`Line`].
pub type TreeEnumerator = fn(SiblingPosition) -> String;

/// Produces the single-line continuation drawn beneath one visible tree node.
///
/// Line breaks in returned markers are normalized to spaces so a marker cannot
/// violate the one-horizontal-row contract of [`Line`].
pub type TreeIndenter = fn(SiblingPosition) -> String;

/// Draws the standard square tree branch for one node.
pub fn default_tree_enumerator(position: SiblingPosition) -> String {
    if position.is_last() {
        "└── ".to_owned()
    } else {
        "├── ".to_owned()
    }
}

/// Draws a vertical continuation while more siblings follow.
pub fn default_tree_indenter(position: SiblingPosition) -> String {
    if position.is_last() {
        "    ".to_owned()
    } else {
        "│   ".to_owned()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ChildOffset {
    start: usize,
    end: usize,
}

/// One owned value and its recursive child nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    value: String,
    children: Vec<Self>,
    hidden: bool,
    child_offset: ChildOffset,
}

impl TreeNode {
    /// Creates a visible leaf node.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            children: Vec::new(),
            hidden: false,
            child_offset: ChildOffset::default(),
        }
    }

    /// Appends one child node.
    #[must_use]
    pub fn child(mut self, child: impl Into<Self>) -> Self {
        self.children.push(child.into());
        self
    }

    /// Appends child nodes in iteration order.
    #[must_use]
    pub fn children<I, N>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = N>,
        N: Into<Self>,
    {
        self.children.extend(children.into_iter().map(Into::into));
        self
    }

    /// Includes or excludes this node and all of its descendants.
    #[must_use]
    pub const fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Omits `start` children from the front and `end` from the back.
    #[must_use]
    pub const fn child_offset(mut self, start: usize, end: usize) -> Self {
        self.child_offset = ChildOffset { start, end };
        self
    }

    /// Returns this node's text.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns all owned children before visibility and offset are applied.
    pub fn child_nodes(&self) -> &[Self] {
        &self.children
    }

    /// Returns whether this node and its descendants are excluded.
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }

    fn visible_children(&self) -> Vec<&Self> {
        visible_children(&self.children, self.child_offset)
    }
}

impl From<String> for TreeNode {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for TreeNode {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Default)]
struct TreeStyleOverrides {
    root: Option<Style>,
    item: Option<Style>,
    enumerator: Option<Style>,
    indenter: Option<Style>,
}

/// An owned tree that composes into a renderer-neutral [`View`].
///
/// Styles and marker policies belong to this outer renderer and are inherited
/// by every nested [`TreeNode`]. Nodes intentionally remain data-only so the
/// model can be reused independently of one component's presentation policy.
#[derive(Debug, Clone)]
pub struct Tree {
    root: Option<String>,
    children: Vec<TreeNode>,
    hidden: bool,
    child_offset: ChildOffset,
    styles: TreeStyleOverrides,
    enumerator: TreeEnumerator,
    indenter: TreeIndenter,
}

impl Default for Tree {
    fn default() -> Self {
        Self {
            root: None,
            children: Vec::new(),
            hidden: false,
            child_offset: ChildOffset::default(),
            styles: TreeStyleOverrides::default(),
            enumerator: default_tree_enumerator,
            indenter: default_tree_indenter,
        }
    }
}

impl Tree {
    /// Creates an empty, rootless tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the optional root text.
    #[must_use]
    pub fn root(mut self, root: impl Into<String>) -> Self {
        self.root = Some(root.into());
        self
    }

    /// Appends one top-level node.
    #[must_use]
    pub fn child(mut self, child: impl Into<TreeNode>) -> Self {
        self.children.push(child.into());
        self
    }

    /// Appends top-level nodes in iteration order.
    #[must_use]
    pub fn children<I, N>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = N>,
        N: Into<TreeNode>,
    {
        self.children.extend(children.into_iter().map(Into::into));
        self
    }

    /// Includes or excludes the complete tree.
    #[must_use]
    pub const fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Omits `start` top-level nodes from the front and `end` from the back.
    #[must_use]
    pub const fn child_offset(mut self, start: usize, end: usize) -> Self {
        self.child_offset = ChildOffset { start, end };
        self
    }

    /// Replaces the root style supplied by the component theme.
    #[must_use]
    pub fn root_style(mut self, style: Style) -> Self {
        self.styles.root = Some(style);
        self
    }

    /// Replaces the item style supplied by the component theme.
    #[must_use]
    pub fn item_style(mut self, style: Style) -> Self {
        self.styles.item = Some(style);
        self
    }

    /// Replaces the branch-marker style supplied by the component theme.
    #[must_use]
    pub fn enumerator_style(mut self, style: Style) -> Self {
        self.styles.enumerator = Some(style);
        self
    }

    /// Replaces the continuation style supplied by the component theme.
    #[must_use]
    pub fn indenter_style(mut self, style: Style) -> Self {
        self.styles.indenter = Some(style);
        self
    }

    /// Replaces the branch-marker policy.
    #[must_use]
    pub const fn enumerator(mut self, enumerator: TreeEnumerator) -> Self {
        self.enumerator = enumerator;
        self
    }

    /// Replaces the nested-continuation policy.
    #[must_use]
    pub const fn indenter(mut self, indenter: TreeIndenter) -> Self {
        self.indenter = indenter;
        self
    }

    /// Returns the optional root text.
    pub fn root_value(&self) -> Option<&str> {
        self.root.as_deref()
    }

    /// Returns all owned top-level nodes before visibility and offset are applied.
    pub fn child_nodes(&self) -> &[TreeNode] {
        &self.children
    }

    /// Composes this tree with logical component styles.
    pub fn view(&self, styles: &ComponentStyles) -> View {
        if self.hidden {
            return View::new();
        }

        let resolved = ResolvedTreeStyles {
            root: self
                .styles
                .root
                .clone()
                .unwrap_or_else(|| styles.tree_style(TreeRole::Root).clone()),
            item: self
                .styles
                .item
                .clone()
                .unwrap_or_else(|| styles.tree_style(TreeRole::Item).clone()),
            enumerator: self
                .styles
                .enumerator
                .clone()
                .unwrap_or_else(|| styles.tree_style(TreeRole::Enumerator).clone()),
            indenter: self
                .styles
                .indenter
                .clone()
                .unwrap_or_else(|| styles.tree_style(TreeRole::Indenter).clone()),
        };
        let mut view = View::new();
        if let Some(root) = &self.root {
            for line in root.split('\n') {
                view = view.push(Line::styled(line, resolved.root.clone()));
            }
        }

        let children = visible_children(&self.children, self.child_offset);
        render_children(
            view,
            &children,
            &[],
            &resolved,
            self.enumerator,
            self.indenter,
        )
    }
}

#[derive(Debug, Clone)]
struct ResolvedTreeStyles {
    root: Style,
    item: Style,
    enumerator: Style,
    indenter: Style,
}

#[derive(Debug, Clone)]
struct PrefixPart {
    text: String,
    style: Style,
}

fn visible_children(children: &[TreeNode], offset: ChildOffset) -> Vec<&TreeNode> {
    let end = children.len().saturating_sub(offset.end);
    if offset.start >= end {
        return Vec::new();
    }
    children[offset.start..end]
        .iter()
        .filter(|child| !child.hidden)
        .collect()
}

fn render_children(
    mut view: View,
    children: &[&TreeNode],
    prefix: &[PrefixPart],
    styles: &ResolvedTreeStyles,
    enumerator: TreeEnumerator,
    indenter: TreeIndenter,
) -> View {
    let markers = (0..children.len())
        .map(|index| {
            let position = SiblingPosition::new(index, children.len());
            (
                normalize_marker(enumerator(position)),
                normalize_marker(indenter(position)),
            )
        })
        .collect::<Vec<_>>();
    let segment_width = markers
        .iter()
        .flat_map(|(enumerator, indenter)| [visible_width(enumerator), visible_width(indenter)])
        .max()
        .unwrap_or(0);

    for (index, child) in children.iter().enumerate() {
        let (enum_marker, indent_marker) = &markers[index];
        let enum_text = align_right(enum_marker.clone(), segment_width);
        let indent_text = align_left(indent_marker.clone(), segment_width);
        let mut value_lines = child.value.split('\n');
        let first = value_lines.next().unwrap_or_default();
        let mut line = append_prefix(Line::new(), prefix)
            .span(enum_text, styles.enumerator.clone())
            .span(first, styles.item.clone());
        view = view.push(line);

        for continuation in value_lines {
            line = append_prefix(Line::new(), prefix)
                .span(indent_text.clone(), styles.indenter.clone())
                .span(continuation, styles.item.clone());
            view = view.push(line);
        }

        let nested = child.visible_children();
        if !nested.is_empty() {
            let mut nested_prefix = prefix.to_vec();
            nested_prefix.push(PrefixPart {
                text: indent_text,
                style: styles.indenter.clone(),
            });
            view = render_children(view, &nested, &nested_prefix, styles, enumerator, indenter);
        }
    }
    view
}

fn append_prefix(mut line: Line, prefix: &[PrefixPart]) -> Line {
    for part in prefix {
        line = line.span(part.text.clone(), part.style.clone());
    }
    line
}

fn align_right(text: String, width: usize) -> String {
    let padding = width.saturating_sub(visible_width(&text));
    format!("{}{text}", " ".repeat(padding))
}

fn align_left(mut text: String, width: usize) -> String {
    text.push_str(&" ".repeat(width.saturating_sub(visible_width(&text))));
    text
}

fn normalize_marker(marker: String) -> String {
    let mut output = String::with_capacity(marker.len());
    let mut characters = marker.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\r' => {
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
                output.push(' ');
            }
            '\n' | '\u{2028}' | '\u{2029}' => output.push(' '),
            other => output.push(other),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, SemanticTokens};

    fn styles() -> ComponentStyles {
        ComponentStyles::from_tokens(&SemanticTokens {
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

    fn plain(view: &View) -> String {
        view.lines()
            .iter()
            .map(|line| {
                line.spans()
                    .iter()
                    .map(|span| span.text())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_empty_and_root_only_trees() {
        assert!(Tree::new().view(&styles()).is_empty());
        assert_eq!(plain(&Tree::new().root("root").view(&styles())), "root");
    }

    #[test]
    fn renders_flat_and_nested_default_branches() {
        let tree = Tree::new()
            .child("alpha")
            .child(TreeNode::new("beta").child("nested"))
            .child("omega");

        assert_eq!(
            plain(&tree.view(&styles())),
            "├── alpha\n├── beta\n│   └── nested\n└── omega"
        );
    }

    #[test]
    fn keeps_descendants_connected_when_the_last_node_is_a_subtree() {
        let tree = Tree::new()
            .child("first")
            .child(TreeNode::new("last").children(["one", "two"]));

        assert_eq!(
            plain(&tree.view(&styles())),
            "├── first\n└── last\n    ├── one\n    └── two"
        );
    }

    #[test]
    fn excludes_hidden_nodes_before_assigning_branch_markers() {
        let tree = Tree::new()
            .child("visible")
            .child(TreeNode::new("hidden").hidden(true));

        assert_eq!(plain(&tree.view(&styles())), "└── visible");
        assert!(tree.hidden(true).view(&styles()).is_empty());
    }

    #[test]
    fn offsets_children_from_both_ends() {
        let tree = Tree::new()
            .children(["skip", "one", "two", "drop"])
            .child_offset(1, 1);
        let nested = Tree::new().child(
            TreeNode::new("parent")
                .children(["skip", "kept", "drop"])
                .child_offset(1, 1),
        );

        assert_eq!(plain(&tree.view(&styles())), "├── one\n└── two");
        assert_eq!(plain(&nested.view(&styles())), "└── parent\n    └── kept");
    }

    #[test]
    fn aligns_multiline_values_at_the_node_body() {
        let tree = Tree::new().child(
            TreeNode::new("親")
                .child("日本語\nsecond")
                .child("終端\n続き"),
        );

        assert_eq!(
            plain(&tree.view(&styles())),
            "└── 親\n    ├── 日本語\n    │   second\n    └── 終端\n        続き"
        );
        let view = tree.view(&styles());
        assert_eq!(
            visible_width(view.lines()[1].spans()[0].text())
                + visible_width(view.lines()[1].spans()[1].text()),
            visible_width(view.lines()[2].spans()[0].text())
                + visible_width(view.lines()[2].spans()[1].text())
        );
    }

    fn custom_enumerator(position: SiblingPosition) -> String {
        format!("{}: ", position.index() + 1)
    }

    fn custom_indenter(_: SiblingPosition) -> String {
        "→ ".to_owned()
    }

    #[test]
    fn supports_custom_enumerator_and_indenter() {
        let tree = Tree::new()
            .enumerator(custom_enumerator)
            .indenter(custom_indenter)
            .child(TreeNode::new("parent").child("child"));

        assert_eq!(plain(&tree.view(&styles())), "1: parent\n→  1: child");
    }

    #[test]
    fn applies_each_style_hook_to_its_semantic_span() {
        let root = Style::new().foreground(Color::RED);
        let item = Style::new().foreground(Color::GREEN);
        let enumerator = Style::new().foreground(Color::BLUE);
        let indenter = Style::new().foreground(Color::YELLOW);
        let view = Tree::new()
            .root("root")
            .child(TreeNode::new("parent").child("child"))
            .root_style(root.clone())
            .item_style(item.clone())
            .enumerator_style(enumerator.clone())
            .indenter_style(indenter.clone())
            .view(&styles());

        assert_eq!(view.lines()[0].spans()[0].style(), &root);
        assert_eq!(view.lines()[1].spans()[0].style(), &enumerator);
        assert_eq!(view.lines()[1].spans()[1].style(), &item);
        assert_eq!(view.lines()[2].spans()[0].style(), &indenter);
        assert_eq!(view.lines()[2].spans()[1].style(), &enumerator);
        assert_eq!(view.lines()[2].spans()[2].style(), &item);
    }

    fn mixed_width_enumerator(position: SiblingPosition) -> String {
        if position.index() == 0 {
            "界".to_owned()
        } else {
            ".".to_owned()
        }
    }

    #[test]
    fn aligns_mixed_enumerator_lengths_by_terminal_cell_width() {
        let view = Tree::new()
            .children(["one", "two"])
            .enumerator(mixed_width_enumerator)
            .view(&styles());

        assert_eq!(plain(&view), "  界one\n   .two");
        assert_eq!(
            visible_width(view.lines()[0].spans()[0].text()),
            visible_width(view.lines()[1].spans()[0].text())
        );
    }

    fn multiline_marker(_: SiblingPosition) -> String {
        "a\r\nb\u{2028}c\u{2029}d".to_owned()
    }

    #[test]
    fn normalizes_custom_markers_to_semantic_lines() {
        let view = Tree::new()
            .child("item")
            .enumerator(multiline_marker)
            .indenter(multiline_marker)
            .view(&styles());

        assert_eq!(view.lines().len(), 1);
        assert_eq!(plain(&view), "a b c ditem");
    }

    #[test]
    fn nested_nodes_inherit_the_outer_render_policy() {
        let view = Tree::new()
            .child(TreeNode::new("parent").child("child"))
            .enumerator(custom_enumerator)
            .indenter(custom_indenter)
            .view(&styles());

        assert_eq!(plain(&view), "1: parent\n→  1: child");
    }
}
