//! Renderer-neutral trees with reusable owned nodes.

use crate::{TextStyle, TreeRole, View};

use super::traversable::{Traversable, TraversalStyles, render as render_traversable};

/// A tree node's position among its visible siblings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiblingPosition {
    index: usize,
    len: usize,
}

impl SiblingPosition {
    /// Creates a tree sibling position.
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
/// violate the one-horizontal-row contract of a row cell.
pub type TreeEnumerator = fn(SiblingPosition) -> String;

/// Produces the single-line continuation drawn beneath one visible tree node.
///
/// Line breaks in returned markers are normalized to spaces so a marker cannot
/// violate the one-horizontal-row contract of a row cell.
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

impl Traversable for TreeNode {
    fn value(&self) -> &str {
        &self.value
    }

    fn visible_children(&self) -> Vec<&Self> {
        self.visible_children()
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

/// Presentation policy used to compose a [`Tree`] into a [`View`].
#[derive(Debug, Clone)]
pub struct TreeStyle {
    root: TextStyle,
    item: TextStyle,
    enumerator_style: TextStyle,
    indenter_style: TextStyle,
    enumerator: TreeEnumerator,
    indenter: TreeIndenter,
}

impl PartialEq for TreeStyle {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
            && self.item == other.item
            && self.enumerator_style == other.enumerator_style
            && self.indenter_style == other.indenter_style
            && std::ptr::fn_addr_eq(self.enumerator, other.enumerator)
            && std::ptr::fn_addr_eq(self.indenter, other.indenter)
    }
}

impl TreeStyle {
    /// Creates a tree style with the default branch and continuation policies.
    pub fn new(
        root: TextStyle,
        item: TextStyle,
        enumerator: TextStyle,
        indenter: TextStyle,
    ) -> Self {
        Self {
            root,
            item,
            enumerator_style: enumerator,
            indenter_style: indenter,
            enumerator: default_tree_enumerator,
            indenter: default_tree_indenter,
        }
    }

    /// Returns the style assigned to one logical tree role.
    pub fn style(&self, role: TreeRole) -> &TextStyle {
        match role {
            TreeRole::Root => &self.root,
            TreeRole::Item => &self.item,
            TreeRole::Enumerator => &self.enumerator_style,
            TreeRole::Indenter => &self.indenter_style,
        }
    }

    /// Replaces the style assigned to one logical tree role.
    #[must_use]
    pub fn with_style(mut self, role: TreeRole, style: TextStyle) -> Self {
        match role {
            TreeRole::Root => self.root = style,
            TreeRole::Item => self.item = style,
            TreeRole::Enumerator => self.enumerator_style = style,
            TreeRole::Indenter => self.indenter_style = style,
        }
        self
    }

    /// Replaces the root style.
    #[must_use]
    pub fn root_style(self, style: TextStyle) -> Self {
        self.with_style(TreeRole::Root, style)
    }

    /// Replaces the item style.
    #[must_use]
    pub fn item_style(self, style: TextStyle) -> Self {
        self.with_style(TreeRole::Item, style)
    }

    /// Replaces the branch-marker style.
    #[must_use]
    pub fn enumerator_style(self, style: TextStyle) -> Self {
        self.with_style(TreeRole::Enumerator, style)
    }

    /// Replaces the continuation style.
    #[must_use]
    pub fn indenter_style(self, style: TextStyle) -> Self {
        self.with_style(TreeRole::Indenter, style)
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

    /// Composes tree data into a renderer-neutral view.
    pub fn view(&self, tree: &Tree) -> View {
        if tree.hidden {
            return View::empty();
        }

        let traversal_styles = TraversalStyles {
            item: self.item.clone(),
            enumerator: self.enumerator_style.clone(),
            indenter: self.indenter_style.clone(),
        };
        let mut rows = Vec::new();
        if let Some(root) = &tree.root {
            for line in root.split('\n') {
                rows.push(View::text(line, self.root.clone()));
            }
        }

        let children = visible_children(&tree.children, tree.child_offset);
        render_traversable(
            rows,
            &children,
            &traversal_styles,
            SiblingPosition::new,
            self.enumerator,
            self.indenter,
        )
    }
}

/// Owned tree data independent of presentation policy.
///
/// Nodes remain data-only so the model can be reused independently of one
/// component's presentation policy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tree {
    root: Option<String>,
    children: Vec<TreeNode>,
    hidden: bool,
    child_offset: ChildOffset,
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

    /// Returns the optional root text.
    pub fn root_value(&self) -> Option<&str> {
        self.root.as_deref()
    }

    /// Returns all owned top-level nodes before visibility and offset are applied.
    pub fn child_nodes(&self) -> &[TreeNode] {
        &self.children
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{plain, plain_rows, style_at};
    use crate::{Color, ComponentStyles, SemanticTokens, measure, visible_width};

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

    #[test]
    fn renders_empty_and_root_only_trees() {
        assert!(measure(&styles().tree().view(&Tree::new())).is_empty());
        assert_eq!(
            plain(&styles().tree().view(&Tree::new().root("root"))),
            "root"
        );
    }

    #[test]
    fn renders_flat_and_nested_default_branches() {
        let tree = Tree::new()
            .child("alpha")
            .child(TreeNode::new("beta").child("nested"))
            .child("omega");

        assert_eq!(
            plain(&styles().tree().view(&tree)),
            "├── alpha\n├── beta\n│   └── nested\n└── omega"
        );
    }

    #[test]
    fn keeps_descendants_connected_when_the_last_node_is_a_subtree() {
        let tree = Tree::new()
            .child("first")
            .child(TreeNode::new("last").children(["one", "two"]));

        assert_eq!(
            plain(&styles().tree().view(&tree)),
            "├── first\n└── last\n    ├── one\n    └── two"
        );
    }

    #[test]
    fn excludes_hidden_nodes_before_assigning_branch_markers() {
        let tree = Tree::new()
            .child("visible")
            .child(TreeNode::new("hidden").hidden(true));

        assert_eq!(plain(&styles().tree().view(&tree)), "└── visible");
        assert!(measure(&styles().tree().view(&tree.hidden(true))).is_empty());
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

        assert_eq!(plain(&styles().tree().view(&tree)), "├── one\n└── two");
        assert_eq!(
            plain(&styles().tree().view(&nested)),
            "└── parent\n    └── kept"
        );
    }

    #[test]
    fn aligns_multiline_values_at_the_node_body() {
        let tree = Tree::new().child(
            TreeNode::new("親")
                .child("日本語\nsecond")
                .child("終端\n続き"),
        );

        assert_eq!(
            plain(&styles().tree().view(&tree)),
            "└── 親\n    ├── 日本語\n    │   second\n    └── 終端\n        続き"
        );
        let rows = plain_rows(&styles().tree().view(&tree));
        assert!(
            rows[2].starts_with("    │   "),
            "a continuation aligns under its node body: {:?}",
            rows[2]
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
        let tree = Tree::new().child(TreeNode::new("parent").child("child"));
        let tree_style = styles()
            .tree()
            .clone()
            .enumerator(custom_enumerator)
            .indenter(custom_indenter);

        assert_eq!(plain(&tree_style.view(&tree)), "1: parent\n→  1: child");
    }

    #[test]
    fn applies_each_style_hook_to_its_semantic_span() {
        let root = TextStyle::new().foreground(Color::RED);
        let item = TextStyle::new().foreground(Color::GREEN);
        let enumerator = TextStyle::new().foreground(Color::BLUE);
        let indenter = TextStyle::new().foreground(Color::YELLOW);
        let tree = Tree::new()
            .root("root")
            .child(TreeNode::new("parent").child("child"));
        let tree_style = styles()
            .tree()
            .clone()
            .root_style(root.clone())
            .item_style(item.clone())
            .enumerator_style(enumerator.clone())
            .indenter_style(indenter.clone());
        let view = tree_style.view(&tree);

        assert_eq!(style_at(&view, 0, 0), root);
        assert_eq!(style_at(&view, 1, 0), enumerator);
        assert_eq!(style_at(&view, 1, 4), item);
        assert_eq!(style_at(&view, 2, 0), indenter);
        assert_eq!(style_at(&view, 2, 4), enumerator);
        assert_eq!(style_at(&view, 2, 8), item);
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
        let tree = Tree::new().children(["one", "two"]);
        let tree_style = styles().tree().clone().enumerator(mixed_width_enumerator);
        let view = tree_style.view(&tree);

        assert_eq!(plain(&view), "  界one\n   .two");
        let rows = plain_rows(&view);
        assert_eq!(
            visible_width(&rows[0][..rows[0].find("one").unwrap()]),
            visible_width(&rows[1][..rows[1].find("two").unwrap()]),
            "markers of different cell widths still start the values in one column"
        );
    }

    fn multiline_marker(_: SiblingPosition) -> String {
        "a\r\nb\u{2028}c\u{2029}d".to_owned()
    }

    #[test]
    fn normalizes_custom_markers_to_semantic_lines() {
        let tree = Tree::new().child("item");
        let tree_style = styles()
            .tree()
            .clone()
            .enumerator(multiline_marker)
            .indenter(multiline_marker);
        let view = tree_style.view(&tree);

        assert_eq!(measure(&view).height(), 1);
        assert_eq!(plain(&view), "a b c ditem");
    }

    #[test]
    fn nested_nodes_inherit_the_outer_render_policy() {
        let tree = Tree::new().child(TreeNode::new("parent").child("child"));
        let tree_style = styles()
            .tree()
            .clone()
            .enumerator(custom_enumerator)
            .indenter(custom_indenter);
        let view = tree_style.view(&tree);

        assert_eq!(plain(&view), "1: parent\n→  1: child");
    }
}
