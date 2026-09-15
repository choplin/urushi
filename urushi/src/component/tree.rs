//! Renderer-neutral trees with reusable owned nodes.

use std::{fmt, sync::Arc};

use crate::text::{PrintableLines, PrintableText, wrap_text};
use crate::view::{CanvasMeasure, CanvasRequirements};
use crate::{
    Canvas, CanvasContext, CanvasItem, CanvasSizing, Composition, Grapheme, LineContinuations,
    LineGlyphs, LineNetwork, Position, TextStyle, TreeRole, View,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ChildOffset {
    start: usize,
    end: usize,
}

/// A visible value's position in a Tree.
///
/// The optional root has no sibling group. Child positions are assigned after
/// offsets and hidden nodes are removed, and top-level children have depth
/// zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreePosition {
    /// The Tree's optional root value.
    Root,
    /// One node among its visible siblings.
    Child {
        /// The zero-based visible sibling index.
        index: usize,
        /// The number of visible siblings in this group.
        len: usize,
        /// The zero-based nesting depth used by the canonical connectors.
        depth: usize,
    },
}

/// One owned value and its recursive child nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode<T> {
    value: T,
    children: Vec<Self>,
    hidden: bool,
    child_offset: ChildOffset,
}

impl<T> TreeNode<T> {
    /// Creates a visible leaf node from a typed value.
    pub fn new(value: T) -> Self {
        Self {
            value,
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

    /// Returns this node's value.
    pub const fn value(&self) -> &T {
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

impl<T> From<T> for TreeNode<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

type NodeFormatter<'a, T> = dyn Fn(&T, TreePosition) -> String + 'a;
type NodeStyler<'a, T> = dyn Fn(&T, TreePosition) -> Option<TextStyle> + 'a;

/// Typed formatting and optional text-style overrides for Tree values.
///
/// This policy covers both the optional root and child nodes. It is separate
/// from [`TreePresentation`], which remains the type-independent Tree-wide
/// presentation stored by a Theme. Composition snapshots callback results;
/// measurement and drawing do not evaluate application policy.
#[derive(Clone)]
pub struct TreeNodePresentation<'a, T> {
    format: Arc<NodeFormatter<'a, T>>,
    style: Arc<NodeStyler<'a, T>>,
}

impl<T> fmt::Debug for TreeNodePresentation<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TreeNodePresentation { .. }")
    }
}

impl<'a, T: 'a> TreeNodePresentation<'a, T> {
    /// Creates a node presentation with a custom formatter and no style overrides.
    pub fn new<F>(format: F) -> Self
    where
        F: Fn(&T, TreePosition) -> String + 'a,
    {
        Self {
            format: Arc::new(format),
            style: Arc::new(no_node_style::<T>),
        }
    }

    /// Replaces the per-node text-style policy.
    ///
    /// `None` keeps the [`TreePresentation`] root or item default.
    /// `Some(style)` replaces that complete style rather than layering over it.
    #[must_use]
    pub fn node_style<S>(mut self, style: S) -> Self
    where
        S: Fn(&T, TreePosition) -> Option<TextStyle> + 'a,
    {
        self.style = Arc::new(style);
        self
    }
}

impl<'a, T> TreeNodePresentation<'a, T>
where
    T: fmt::Display + 'a,
{
    /// Uses the value's canonical [`fmt::Display`] representation.
    pub fn display() -> Self {
        Self::new(display_node::<T>)
    }
}

fn display_node<T>(value: &T, _: TreePosition) -> String
where
    T: fmt::Display,
{
    value.to_string()
}

fn no_node_style<T>(_: &T, _: TreePosition) -> Option<TextStyle> {
    None
}

/// Presentation policy used to compose a [`Tree`] into a [`View`].
#[derive(Debug, Clone, PartialEq)]
pub struct TreePresentation {
    root: TextStyle,
    item: TextStyle,
    connector: TextStyle,
    line_glyphs: LineGlyphs,
    indent_width: usize,
}

impl TreePresentation {
    /// Creates the canonical tree presentation with square line glyphs.
    pub fn new(root: TextStyle, item: TextStyle, connector: TextStyle) -> Self {
        Self {
            root,
            item,
            connector,
            line_glyphs: LineGlyphs::NORMAL,
            indent_width: 4,
        }
    }

    /// Returns the style assigned to one logical tree role.
    pub fn style(&self, role: TreeRole) -> &TextStyle {
        match role {
            TreeRole::Root => &self.root,
            TreeRole::Item => &self.item,
            TreeRole::Connector => &self.connector,
        }
    }

    /// Replaces the style assigned to one logical tree role.
    #[must_use]
    pub fn with_style(mut self, role: TreeRole, style: TextStyle) -> Self {
        match role {
            TreeRole::Root => self.root = style,
            TreeRole::Item => self.item = style,
            TreeRole::Connector => self.connector = style,
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

    /// Replaces the connector style.
    #[must_use]
    pub fn connector_style(self, style: TextStyle) -> Self {
        self.with_style(TreeRole::Connector, style)
    }

    /// Replaces the complete one-cell connector glyph repertoire.
    #[must_use]
    pub const fn line_glyphs(mut self, line_glyphs: LineGlyphs) -> Self {
        self.line_glyphs = line_glyphs;
        self
    }

    /// Sets the cell distance from one connector column to the next.
    ///
    /// The width includes the junction, at least one horizontal continuation
    /// cell, and one gap before the node body.
    ///
    /// # Panics
    ///
    /// Panics when `indent_width` is less than three cells.
    #[must_use]
    pub const fn indent_width(mut self, indent_width: usize) -> Self {
        assert!(
            indent_width >= 3,
            "tree indentation must be at least three cells"
        );
        self.indent_width = indent_width;
        self
    }

    /// Composes displayable tree data into an intrinsically sized Canvas.
    pub fn compose<T>(&self, tree: &Tree<T>) -> View
    where
        T: fmt::Display,
    {
        self.compose_using(tree, &display_node::<T>, &no_node_style::<T>)
    }

    /// Composes tree data with typed node formatting and text-style overrides.
    pub fn compose_with<T>(&self, tree: &Tree<T>, nodes: &TreeNodePresentation<'_, T>) -> View {
        self.compose_using(tree, &*nodes.format, &*nodes.style)
    }

    fn compose_using<T, F, S>(&self, tree: &Tree<T>, format: &F, style: &S) -> View
    where
        F: Fn(&T, TreePosition) -> String + ?Sized,
        S: Fn(&T, TreePosition) -> Option<TextStyle> + ?Sized,
    {
        if tree.hidden {
            return View::empty();
        }

        let children = visible_children(&tree.children, tree.child_offset);
        if tree.root.is_none() && children.is_empty() {
            return View::empty();
        }

        let mut nodes = Vec::new();
        let mut groups = Vec::new();
        let root = tree.root.as_ref().map(|value| {
            let position = TreePosition::Root;
            BoundTreeRoot {
                value: format(value, position),
                style: style(value, position).unwrap_or_else(|| self.root.clone()),
            }
        });
        bind_group(&mut nodes, &mut groups, &children, 0, self, format, style);
        let item = TreeCanvasItem(Arc::new(TreeFrame {
            root,
            nodes,
            groups,
            connector_style: self.connector.clone(),
            line_glyphs: self.line_glyphs,
            indent_width: self.indent_width,
        }));
        View::canvas(Canvas::new().sizing(item.sizing()).item(item))
    }
}

#[derive(Debug, Clone, PartialEq)]
struct BoundTreeRoot {
    value: String,
    style: TextStyle,
}

#[derive(Debug, Clone, PartialEq)]
struct BoundTreeNode {
    value: String,
    style: TextStyle,
    connector_x: usize,
    content_x: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundSiblingGroup {
    connector_x: usize,
    nodes: Vec<usize>,
}

fn bind_group<T, F, S>(
    bound: &mut Vec<BoundTreeNode>,
    groups: &mut Vec<BoundSiblingGroup>,
    nodes: &[&TreeNode<T>],
    depth: usize,
    presentation: &TreePresentation,
    format: &F,
    style: &S,
) where
    F: Fn(&T, TreePosition) -> String + ?Sized,
    S: Fn(&T, TreePosition) -> Option<TextStyle> + ?Sized,
{
    if nodes.is_empty() {
        return;
    }

    let connector_x = depth.saturating_mul(presentation.indent_width);
    let content_x = connector_x.saturating_add(presentation.indent_width);
    let group_index = groups.len();
    groups.push(BoundSiblingGroup {
        connector_x,
        nodes: Vec::with_capacity(nodes.len()),
    });

    for (index, node) in nodes.iter().enumerate() {
        let position = TreePosition::Child {
            index,
            len: nodes.len(),
            depth,
        };
        let node_index = bound.len();
        bound.push(BoundTreeNode {
            value: format(&node.value, position),
            style: style(&node.value, position).unwrap_or_else(|| presentation.item.clone()),
            connector_x,
            content_x,
        });
        groups[group_index].nodes.push(node_index);

        let children = node.visible_children();
        bind_group(
            bound,
            groups,
            &children,
            depth.saturating_add(1),
            presentation,
            format,
            style,
        );
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TreeFrame {
    root: Option<BoundTreeRoot>,
    nodes: Vec<BoundTreeNode>,
    groups: Vec<BoundSiblingGroup>,
    connector_style: TextStyle,
    line_glyphs: LineGlyphs,
    indent_width: usize,
}

impl TreeFrame {
    fn width_requirements(&self) -> CanvasRequirements {
        let mut demand = 0;
        let mut floor = 0;
        if let Some(root) = &self.root {
            let widths = text_widths(&root.value);
            demand = widths.0;
            floor = widths.1;
        }
        for node in &self.nodes {
            let (content_demand, content_floor) = text_widths(&node.value);
            demand = demand.max(node.content_x.saturating_add(content_demand));
            floor = floor.max(node.content_x.saturating_add(content_floor));
        }
        CanvasRequirements::new(demand, floor)
    }

    fn rows(&self, width: usize) -> TreeRows {
        let mut rows = Vec::new();
        if let Some(root) = &self.root {
            rows.extend(
                wrapped_rows(&root.value, width)
                    .into_iter()
                    .map(TreeRow::Root),
            );
        }

        let mut node_y = Vec::with_capacity(self.nodes.len());
        for (node_index, node) in self.nodes.iter().enumerate() {
            node_y.push(rows.len());
            let content_width = width.saturating_sub(node.content_x);
            rows.extend(
                wrapped_rows(&node.value, content_width)
                    .into_iter()
                    .map(|text| TreeRow::Node { node_index, text }),
            );
        }
        TreeRows { rows, node_y }
    }

    fn draw(&self, context: &mut CanvasContext) {
        let plan = self.rows(context.size().width());
        let mut network = LineNetwork::new(self.line_glyphs, self.connector_style.clone());
        let branch_length = self.indent_width - 2;
        for group in &self.groups {
            let first_y = plan.node_y[group.nodes[0]];
            let last_y = plan.node_y[*group.nodes.last().expect("a bound group is non-empty")];
            network.vertical_with(
                position(group.connector_x),
                position(first_y)..=position(last_y),
                LineContinuations::START,
            );
            for &node_index in &group.nodes {
                let node = &self.nodes[node_index];
                let y = plan.node_y[node_index];
                network.horizontal(
                    position(y),
                    position(node.connector_x)
                        ..=position(node.connector_x.saturating_add(branch_length)),
                );
            }
        }
        context.line_network(network);

        for (y, row) in plan.rows.into_iter().enumerate() {
            let (x, text, style) = match row {
                TreeRow::Root(text) => (
                    0,
                    text,
                    self.root
                        .as_ref()
                        .expect("a root row has a bound root")
                        .style
                        .clone(),
                ),
                TreeRow::Node { node_index, text } => (
                    self.nodes[node_index].content_x,
                    text,
                    self.nodes[node_index].style.clone(),
                ),
            };
            context.text_with(
                Position::new(position(x), position(y)),
                text,
                style,
                Composition::Replace,
            );
        }
    }
}

fn wrapped_rows(text: &str, width: usize) -> Vec<String> {
    text.split('\n')
        .flat_map(|line| wrap_text(PrintableLines::new(line), width))
        .collect()
}

fn text_widths(text: &str) -> (usize, usize) {
    text.split('\n')
        .map(PrintableText::new)
        .fold((0, 0), |(demand, floor), line| {
            (
                demand.max(line.width()),
                floor.max(line.graphemes().map(Grapheme::width).max().unwrap_or(0)),
            )
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TreeRows {
    rows: Vec<TreeRow>,
    node_y: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TreeRow {
    Root(String),
    Node { node_index: usize, text: String },
}

#[derive(Debug, Clone, PartialEq)]
struct TreeCanvasItem(Arc<TreeFrame>);

impl TreeCanvasItem {
    fn sizing(&self) -> CanvasSizing {
        CanvasSizing::intrinsic(self.clone())
    }
}

impl CanvasMeasure for TreeCanvasItem {
    fn width_requirements(&self) -> CanvasRequirements {
        self.0.width_requirements()
    }

    fn height_requirements(&self, width: usize) -> CanvasRequirements {
        CanvasRequirements::new(self.0.rows(width).rows.len(), 0)
    }
}

impl CanvasItem for TreeCanvasItem {
    fn draw(&self, context: &mut CanvasContext) {
        self.0.draw(context);
    }
}

fn position(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Owned tree data independent of presentation policy.
///
/// Nodes remain data-only so the model can be reused independently of one
/// component's presentation policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree<T> {
    root: Option<T>,
    children: Vec<TreeNode<T>>,
    hidden: bool,
    child_offset: ChildOffset,
}

impl<T> Default for Tree<T> {
    fn default() -> Self {
        Self {
            root: None,
            children: Vec::new(),
            hidden: false,
            child_offset: ChildOffset::default(),
        }
    }
}

impl<T> Tree<T> {
    /// Creates an empty, rootless tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the optional typed root value.
    #[must_use]
    pub fn root(mut self, root: T) -> Self {
        self.root = Some(root);
        self
    }

    /// Appends one top-level node.
    #[must_use]
    pub fn child(mut self, child: impl Into<TreeNode<T>>) -> Self {
        self.children.push(child.into());
        self
    }

    /// Appends top-level nodes in iteration order.
    #[must_use]
    pub fn children<I, N>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = N>,
        N: Into<TreeNode<T>>,
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

    /// Returns the optional root value.
    pub const fn root_value(&self) -> Option<&T> {
        self.root.as_ref()
    }

    /// Returns all owned top-level nodes before visibility and offset are applied.
    pub fn child_nodes(&self) -> &[TreeNode<T>] {
        &self.children
    }
}

fn visible_children<T>(children: &[TreeNode<T>], offset: ChildOffset) -> Vec<&TreeNode<T>> {
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
    use crate::{Color, ComponentTheme, SemanticTokens, measure};

    fn styles() -> ComponentTheme {
        ComponentTheme::from_tokens(&SemanticTokens {
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
        assert!(measure(&styles().tree().compose(&Tree::<String>::new())).is_empty());
        assert_eq!(
            plain(&styles().tree().compose(&Tree::new().root("root"))),
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
            plain(&styles().tree().compose(&tree)),
            "├── alpha\n├── beta\n│   └── nested\n└── omega"
        );
    }

    #[test]
    fn keeps_descendants_connected_when_the_last_node_is_a_subtree() {
        let tree = Tree::new()
            .child("first")
            .child(TreeNode::new("last").children(["one", "two"]));

        assert_eq!(
            plain(&styles().tree().compose(&tree)),
            "├── first\n└── last\n    ├── one\n    └── two"
        );
    }

    #[test]
    fn excludes_hidden_nodes_before_assigning_branch_markers() {
        let tree = Tree::new()
            .child("visible")
            .child(TreeNode::new("hidden").hidden(true));

        assert_eq!(plain(&styles().tree().compose(&tree)), "└── visible");
        assert!(measure(&styles().tree().compose(&tree.hidden(true))).is_empty());
    }

    #[test]
    fn offsets_children_from_both_ends() {
        let tree = Tree::new()
            .children(["skip", "one", "two", "drop"])
            .child_offset(1, 1);
        let nested = Tree::<&str>::new().child(
            TreeNode::new("parent")
                .children(["skip", "kept", "drop"])
                .child_offset(1, 1),
        );

        assert_eq!(plain(&styles().tree().compose(&tree)), "├── one\n└── two");
        assert_eq!(
            plain(&styles().tree().compose(&nested)),
            "└── parent\n    └── kept"
        );
    }

    #[test]
    fn aligns_multiline_values_at_the_node_body() {
        let tree = Tree::<&str>::new().child(
            TreeNode::new("親")
                .child("日本語\nsecond")
                .child("終端\n続き"),
        );

        assert_eq!(
            plain(&styles().tree().compose(&tree)),
            "└── 親\n    ├── 日本語\n    │   second\n    └── 終端\n        続き"
        );
        let rows = plain_rows(&styles().tree().compose(&tree));
        assert!(
            rows[2].starts_with("    │   "),
            "a continuation aligns under its node body: {:?}",
            rows[2]
        );
    }

    fn plain_at(view: &View, width: usize) -> String {
        crate::resolve(view, crate::Available::columns(width))
            .unwrap()
            .rows()
            .iter()
            .map(|row| {
                row.iter()
                    .map(crate::StyledGrapheme::symbol)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn selected_width_reflows_content_without_recomposing() {
        let view = styles().tree().compose(&Tree::new().child("alpha beta"));

        assert_eq!(plain_at(&view, 14), "└── alpha beta");
        assert_eq!(plain_at(&view, 9), "└── alpha\n    beta");
    }

    #[test]
    fn protects_deep_prefixes_and_wraps_cjk_and_emoji_content() {
        let tree = Tree::<&str>::new().child(
            TreeNode::new("親").child(TreeNode::new("branch").children(["日本語", "👩‍💻 end"])),
        );
        let view = styles().tree().compose(&tree);

        assert_eq!(
            plain_at(&view, 15),
            "└── 親\n    └── branch\n        ├── 日\n        │   本\n        │   語\n        └── 👩‍💻\n            end"
        );
    }

    #[test]
    fn preserves_explicit_and_wrapped_continuations() {
        let tree = Tree::new().children(["ab cd\n日\n", "last"]);
        let view = styles().tree().compose(&tree);

        assert_eq!(plain_at(&view, 8), "├── ab\n│   cd\n│   日\n│\n└── last");
    }

    #[test]
    fn supports_rounded_ascii_and_custom_connector_glyphs() {
        let tree = Tree::new().children(["one", "two"]);
        let rounded = styles()
            .tree()
            .clone()
            .line_glyphs(LineGlyphs::ROUNDED)
            .compose(&tree);
        let ascii = styles()
            .tree()
            .clone()
            .line_glyphs(LineGlyphs::ASCII)
            .compose(&tree);
        let custom = styles()
            .tree()
            .clone()
            .line_glyphs(LineGlyphs {
                tee_right: 'T',
                corner_up_right: 'L',
                horizontal: '=',
                end_left: '=',
                ..LineGlyphs::ASCII
            })
            .compose(&tree);

        assert_eq!(plain(&rounded), "├── one\n╰── two");
        assert_eq!(plain(&ascii), "+-- one\n+-- two");
        assert_eq!(plain(&custom), "T== one\nL== two");
    }

    #[test]
    fn applies_root_item_and_connector_styles() {
        let root = TextStyle::new().foreground(Color::RED);
        let item = TextStyle::new().foreground(Color::GREEN);
        let connector = TextStyle::new().foreground(Color::BLUE);
        let tree = Tree::new()
            .root("root")
            .child(TreeNode::new("parent").children(["one", "two"]))
            .child("last");
        let view = styles()
            .tree()
            .clone()
            .root_style(root.clone())
            .item_style(item.clone())
            .connector_style(connector.clone())
            .compose(&tree);

        assert_eq!(style_at(&view, 0, 0), root);
        assert_eq!(style_at(&view, 1, 0), connector);
        assert_eq!(style_at(&view, 1, 4), item);
        assert_eq!(style_at(&view, 2, 0), connector);
        assert_eq!(style_at(&view, 2, 4), connector);
        assert_eq!(style_at(&view, 2, 8), item);
    }

    #[test]
    fn presentation_and_composed_view_equality_include_policy() {
        let base = styles().tree().clone();
        let tree = Tree::new().child("item");

        assert_eq!(base, base.clone());
        assert_eq!(base.compose(&tree), base.clone().compose(&tree));
        assert_ne!(base, base.clone().line_glyphs(LineGlyphs::ASCII));
        assert_ne!(base, base.clone().indent_width(3));
        assert_ne!(base, base.clone().connector_style(TextStyle::new().bold()));
    }

    #[test]
    fn width_requirements_include_content_prefix_and_widest_grapheme() {
        let tree = Tree::new()
            .root("root")
            .child(TreeNode::new("a").child(TreeNode::new("b").child("日本語")));
        let children = visible_children(&tree.children, tree.child_offset);
        let mut nodes = Vec::new();
        let mut groups = Vec::new();
        let presentation =
            TreePresentation::new(TextStyle::new(), TextStyle::new(), TextStyle::new());
        bind_group(
            &mut nodes,
            &mut groups,
            &children,
            0,
            &presentation,
            &display_node::<&str>,
            &no_node_style::<&str>,
        );
        let requirements = TreeFrame {
            root: tree.root.map(|value| BoundTreeRoot {
                value: value.to_owned(),
                style: TextStyle::new(),
            }),
            nodes,
            groups,
            connector_style: TextStyle::new(),
            line_glyphs: LineGlyphs::NORMAL,
            indent_width: 4,
        }
        .width_requirements();

        assert_eq!(requirements.demand(), 18);
        assert_eq!(requirements.floor(), 14);
    }

    #[test]
    #[should_panic(expected = "tree indentation must be at least three cells")]
    fn rejects_indentation_without_junction_continuation_and_gap_cells() {
        let _ = styles().tree().clone().indent_width(2);
    }

    #[test]
    fn tree_nodes_remain_independent_semantic_values() {
        let node = TreeNode::new("parent").children(["one", "two"]);
        let tree = Tree::new().root("root").child(node.clone());

        assert_eq!(*node.value(), "parent");
        assert_eq!(node.child_nodes().len(), 2);
        assert_eq!(tree.root_value().copied(), Some("root"));
        assert_eq!(tree.child_nodes(), &[node]);
    }
}
