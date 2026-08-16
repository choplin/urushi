//! Renderer-neutral lists with an independent public model.

use crate::{ListRole, TextStyle, View};

use super::traversable::{Traversable, TraversalStyles, render as render_traversable};

/// A list item's position among its visible siblings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListPosition {
    index: usize,
    len: usize,
}

impl ListPosition {
    /// Creates a list sibling position.
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

/// Produces the single-line marker drawn before one visible list item.
pub type ListEnumerator = fn(ListPosition) -> String;

/// Produces the single-line continuation drawn beneath one visible list item.
pub type ListIndenter = fn(ListPosition) -> String;

/// Draws the default bullet marker.
pub fn bullet_enumerator(_: ListPosition) -> String {
    "• ".to_owned()
}

/// Draws a dash marker.
pub fn dash_enumerator(_: ListPosition) -> String {
    "- ".to_owned()
}

/// Draws an asterisk marker.
pub fn asterisk_enumerator(_: ListPosition) -> String {
    "* ".to_owned()
}

/// Draws a one-based Arabic numeral marker.
pub fn arabic_enumerator(position: ListPosition) -> String {
    format!("{}. ", position.index().saturating_add(1))
}

/// Draws an uppercase alphabetic marker (`A` through `Z`, then `AA`, ...).
pub fn alphabet_enumerator(position: ListPosition) -> String {
    let mut index = position.index();
    let mut letters = Vec::new();

    loop {
        letters.push((b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }

    letters.reverse();
    format!("{}. ", letters.into_iter().collect::<String>())
}

/// Draws an uppercase Roman numeral marker through 3999.
///
/// Larger positions fall back to Arabic numerals because conventional Roman
/// notation has no single portable representation above `MMMCMXCIX`.
pub fn roman_enumerator(position: ListPosition) -> String {
    const NUMERALS: &[(usize, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];

    let mut value = position.index().saturating_add(1);
    if value > 3999 {
        return format!("{value}. ");
    }
    let mut numeral = String::new();
    for &(arabic, roman) in NUMERALS {
        while value >= arabic {
            value -= arabic;
            numeral.push_str(roman);
        }
    }
    format!("{numeral}. ")
}

/// Draws the blank continuation used by the default list layout.
pub fn default_list_indenter(_: ListPosition) -> String {
    "  ".to_owned()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ItemOffset {
    start: usize,
    end: usize,
}

/// One owned value and its recursive nested list items.
///
/// `ListItem` is deliberately independent of the Tree component's public node
/// model. The components share only a private traversal contract, so either
/// public API can evolve without changing the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    value: String,
    items: Vec<Self>,
    hidden: bool,
    offset: ItemOffset,
}

impl ListItem {
    /// Creates a visible leaf item.
    /// `value` is plain text: escape sequences and cursor movement in it break
    /// that contract, and debug builds panic on them.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            items: Vec::new(),
            hidden: false,
            offset: ItemOffset::default(),
        }
    }

    /// Appends one nested item.
    #[must_use]
    pub fn item(mut self, item: impl Into<Self>) -> Self {
        self.items.push(item.into());
        self
    }

    /// Appends nested items in iteration order.
    #[must_use]
    pub fn items<I, N>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = N>,
        N: Into<Self>,
    {
        self.items.extend(items.into_iter().map(Into::into));
        self
    }

    /// Includes or excludes this item and all of its descendants.
    #[must_use]
    pub const fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Omits `start` nested items from the front and `end` from the back.
    #[must_use]
    pub const fn offset(mut self, start: usize, end: usize) -> Self {
        self.offset = ItemOffset { start, end };
        self
    }

    /// Returns this item's text.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns all nested items before visibility and offset are applied.
    pub fn item_nodes(&self) -> &[Self] {
        &self.items
    }

    /// Returns whether this item and its descendants are excluded.
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }

    fn visible_items(&self) -> Vec<&Self> {
        visible_items(&self.items, self.offset)
    }
}

impl From<String> for ListItem {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for ListItem {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl Traversable for ListItem {
    fn value(&self) -> &str {
        &self.value
    }

    fn visible_children(&self) -> Vec<&Self> {
        self.visible_items()
    }
}

/// Presentation policy used to compose a [`List`] into a [`View`].
#[derive(Debug, Clone)]
pub struct ListStyle {
    item: TextStyle,
    enumerator_style: TextStyle,
    indenter_style: TextStyle,
    enumerator: ListEnumerator,
    indenter: ListIndenter,
}

impl PartialEq for ListStyle {
    fn eq(&self, other: &Self) -> bool {
        self.item == other.item
            && self.enumerator_style == other.enumerator_style
            && self.indenter_style == other.indenter_style
            && std::ptr::fn_addr_eq(self.enumerator, other.enumerator)
            && std::ptr::fn_addr_eq(self.indenter, other.indenter)
    }
}

impl ListStyle {
    /// Creates a list style with the default bullet and continuation policies.
    pub fn new(item: TextStyle, enumerator: TextStyle, indenter: TextStyle) -> Self {
        Self {
            item,
            enumerator_style: enumerator,
            indenter_style: indenter,
            enumerator: bullet_enumerator,
            indenter: default_list_indenter,
        }
    }

    /// Returns the style assigned to one logical list role.
    pub fn style(&self, role: ListRole) -> &TextStyle {
        match role {
            ListRole::Item => &self.item,
            ListRole::Enumerator => &self.enumerator_style,
            ListRole::Indenter => &self.indenter_style,
        }
    }

    /// Replaces the style assigned to one logical list role.
    #[must_use]
    pub fn with_style(mut self, role: ListRole, style: TextStyle) -> Self {
        match role {
            ListRole::Item => self.item = style,
            ListRole::Enumerator => self.enumerator_style = style,
            ListRole::Indenter => self.indenter_style = style,
        }
        self
    }

    /// Replaces the item style.
    #[must_use]
    pub fn item_style(self, style: TextStyle) -> Self {
        self.with_style(ListRole::Item, style)
    }

    /// Replaces the marker style.
    #[must_use]
    pub fn enumerator_style(self, style: TextStyle) -> Self {
        self.with_style(ListRole::Enumerator, style)
    }

    /// Replaces the continuation style.
    #[must_use]
    pub fn indenter_style(self, style: TextStyle) -> Self {
        self.with_style(ListRole::Indenter, style)
    }

    /// Replaces the item-marker policy.
    #[must_use]
    pub const fn enumerator(mut self, enumerator: ListEnumerator) -> Self {
        self.enumerator = enumerator;
        self
    }

    /// Replaces the nested-continuation policy.
    #[must_use]
    pub const fn indenter(mut self, indenter: ListIndenter) -> Self {
        self.indenter = indenter;
        self
    }

    /// Composes list data into a renderer-neutral view.
    pub fn view(&self, list: &List) -> View {
        if list.hidden {
            return View::empty();
        }

        let traversal_styles = TraversalStyles {
            item: self.item.clone(),
            enumerator: self.enumerator_style.clone(),
            indenter: self.indenter_style.clone(),
        };
        let items = visible_items(&list.items, list.offset);
        render_traversable(
            Vec::new(),
            &items,
            &traversal_styles,
            ListPosition::new,
            self.enumerator,
            self.indenter,
        )
    }
}

/// Owned list data independent of presentation policy.
///
/// List owns its public model independently from Tree. Both components implement
/// a private traversal contract that shares recursive layout without coupling
/// either public data API to the other component.
///
/// Offsets are supported with [`List::offset`] and [`ListItem::offset`]. A
/// filter callback is intentionally not part of this API: omit items before
/// construction or mark individual [`ListItem`] values hidden instead.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct List {
    items: Vec<ListItem>,
    hidden: bool,
    offset: ItemOffset,
}

impl List {
    /// Creates an empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one top-level item.
    #[must_use]
    pub fn item(mut self, item: impl Into<ListItem>) -> Self {
        self.items.push(item.into());
        self
    }

    /// Appends top-level items in iteration order.
    #[must_use]
    pub fn items<I, N>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = N>,
        N: Into<ListItem>,
    {
        self.items.extend(items.into_iter().map(Into::into));
        self
    }

    /// Includes or excludes the complete list.
    #[must_use]
    pub const fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Omits `start` top-level items from the front and `end` from the back.
    #[must_use]
    pub const fn offset(mut self, start: usize, end: usize) -> Self {
        self.offset = ItemOffset { start, end };
        self
    }

    /// Returns all owned top-level items before visibility and offset are applied.
    pub fn item_nodes(&self) -> &[ListItem] {
        &self.items
    }
}

fn visible_items(items: &[ListItem], offset: ItemOffset) -> Vec<&ListItem> {
    let end = items.len().saturating_sub(offset.end);
    if offset.start >= end {
        return Vec::new();
    }
    items[offset.start..end]
        .iter()
        .filter(|item| !item.hidden)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{plain, plain_rows, style_at};
    use crate::{Color, ComponentStyles, SemanticTokens, measure};

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
    fn renders_empty_flat_and_nested_lists() {
        assert!(measure(&styles().list().view(&List::new())).is_empty());

        let list = List::new()
            .item("alpha")
            .item(ListItem::new("beta").items(["nested", "last"]))
            .item("omega");

        assert_eq!(
            plain(&styles().list().view(&list)),
            "• alpha\n• beta\n  • nested\n  • last\n• omega"
        );
    }

    #[test]
    fn list_items_are_independent_from_tree_nodes() {
        fn accepts_list_item(_: ListItem) {}
        accepts_list_item(ListItem::new("list").item("nested"));

        assert_eq!(ListItem::new("item").value(), "item");
        assert_eq!(
            ListItem::new("item")
                .items(["one", "two"])
                .item_nodes()
                .len(),
            2
        );
    }

    #[test]
    fn supports_all_builtin_enumerators() {
        let items = ["one", "two", "three"];
        let cases = [
            (bullet_enumerator as ListEnumerator, "• one\n• two\n• three"),
            (dash_enumerator, "- one\n- two\n- three"),
            (asterisk_enumerator, "* one\n* two\n* three"),
            (arabic_enumerator, "1. one\n2. two\n3. three"),
            (alphabet_enumerator, "A. one\nB. two\nC. three"),
            (roman_enumerator, "  I. one\n II. two\nIII. three"),
        ];

        for (enumerator, expected) in cases {
            let list = List::new().items(items);
            let list_style = styles().list().clone().enumerator(enumerator);
            let view = list_style.view(&list);
            assert_eq!(plain(&view), expected);
        }
    }

    #[test]
    fn alphabet_enumerator_extends_beyond_one_letter() {
        let positions = [
            (25, "Z. "),
            (26, "AA. "),
            (51, "AZ. "),
            (701, "ZZ. "),
            (702, "AAA. "),
        ];
        for (index, expected) in positions {
            assert_eq!(alphabet_enumerator(ListPosition::new(index, 703)), expected);
        }
    }

    #[test]
    fn numeric_enumerators_are_bounded_at_extreme_public_positions() {
        assert_eq!(
            roman_enumerator(ListPosition::new(3998, 3999)),
            "MMMCMXCIX. "
        );
        assert_eq!(roman_enumerator(ListPosition::new(3999, 4000)), "4000. ");
        assert_eq!(
            roman_enumerator(ListPosition::new(usize::MAX, usize::MAX)),
            format!("{}. ", usize::MAX)
        );
        assert_eq!(
            arabic_enumerator(ListPosition::new(usize::MAX, usize::MAX)),
            format!("{}. ", usize::MAX)
        );
    }

    fn custom_enumerator(position: ListPosition) -> String {
        format!("[{}] ", position.index())
    }

    fn custom_indenter(_: ListPosition) -> String {
        "→ ".to_owned()
    }

    #[test]
    fn supports_custom_markers_styles_visibility_and_offsets() {
        let item = TextStyle::new().foreground(Color::GREEN);
        let enumerator = TextStyle::new().foreground(Color::BLUE);
        let indenter = TextStyle::new().foreground(Color::YELLOW);
        let list = List::new()
            .items([
                ListItem::new("skip"),
                ListItem::new("parent")
                    .items([ListItem::new("hidden").hidden(true), ListItem::new("child")]),
                ListItem::new("drop"),
            ])
            .offset(1, 1);
        let list_style = styles()
            .list()
            .clone()
            .enumerator(custom_enumerator)
            .indenter(custom_indenter)
            .item_style(item.clone())
            .enumerator_style(enumerator.clone())
            .indenter_style(indenter.clone());
        let view = list_style.view(&list);

        assert_eq!(plain(&view), "[0] parent\n→   [0] child");
        assert_eq!(style_at(&view, 0, 0), enumerator);
        assert_eq!(style_at(&view, 0, 4), item);
        assert_eq!(style_at(&view, 1, 0), indenter);
        assert_eq!(style_at(&view, 1, 4), enumerator);
    }

    #[test]
    fn nested_offsets_apply_before_hidden_items() {
        let list = List::new().item(
            ListItem::new("parent")
                .items(["skip", "hidden", "kept", "drop"])
                .offset(1, 1),
        );
        let hidden_list = List::new().item(ListItem::new("parent").items([
            ListItem::new("visible"),
            ListItem::new("hidden").hidden(true),
        ]));
        let component_styles = styles();
        let view = component_styles.list().view(&list);
        let hidden = component_styles.list().view(&hidden_list);

        assert_eq!(plain(&view), "• parent\n  • hidden\n  • kept");
        assert_eq!(plain(&hidden), "• parent\n  • visible");
    }

    #[test]
    fn aligns_multiline_cjk_items_by_terminal_cell_width() {
        let list = List::new().items(["日本語\nsecond", "終端\n続き"]);
        let list_style = styles().list().clone().enumerator(arabic_enumerator);
        let view = list_style.view(&list);

        assert_eq!(plain(&view), "1. 日本語\n   second\n2. 終端\n   続き");
        let rows = plain_rows(&view);
        assert!(
            rows.iter().all(|row| row.starts_with("1. ")
                || row.starts_with("2. ")
                || row.starts_with("   ")),
            "a continuation aligns under its marker: {rows:?}"
        );
    }

    #[test]
    fn hidden_list_returns_an_empty_view() {
        let list = List::new().item("item").hidden(true);
        assert!(measure(&styles().list().view(&list)).is_empty());
    }
}
