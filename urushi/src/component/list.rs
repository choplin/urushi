//! Renderer-neutral lists with an independent public model.

use std::{fmt, sync::Arc};

use crate::text::{PrintableLines, PrintableText, wrap_text};
use crate::view::{CanvasMeasure, CanvasRequirements};
use crate::{
    Canvas, CanvasContext, CanvasItem, CanvasSizing, Composition, Grapheme, ListRole, Position,
    TextStyle, View,
};

use super::traversable::normalize_marker;

/// A list item's position among its visible siblings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListPosition {
    index: usize,
    len: usize,
    depth: usize,
}

impl ListPosition {
    /// Creates a list sibling position.
    pub const fn new(index: usize, len: usize, depth: usize) -> Self {
        Self { index, len, depth }
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

    /// Returns the zero-based nesting depth.
    pub const fn depth(self) -> usize {
        self.depth
    }
}

/// Produces the single-line marker drawn before one visible list item.
pub type ListEnumerator = fn(ListPosition) -> String;

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ItemOffset {
    start: usize,
    end: usize,
}

/// One owned typed value and its recursive nested list items.
///
/// `ListItem` is deliberately independent of the Tree component's public node
/// model, so either public API can evolve without changing the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem<T> {
    value: T,
    items: Vec<Self>,
    hidden: bool,
    offset: ItemOffset,
}

impl<T> ListItem<T> {
    /// Creates a visible leaf item from a typed value.
    pub fn new(value: T) -> Self {
        Self {
            value,
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

    /// Returns this item's value.
    pub const fn value(&self) -> &T {
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

impl<T> From<T> for ListItem<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

type ItemFormatter<'a, T> = dyn Fn(&T, ListPosition) -> String + 'a;
type ItemStyler<'a, T> = dyn Fn(&T, ListPosition, ListRole) -> Option<TextStyle> + 'a;

/// Typed formatting and optional role-style overrides for List items.
///
/// This policy is separate from [`ListPresentation`], which remains the
/// type-independent List-wide presentation stored by a Theme. During
/// composition, the formatter is evaluated once per visible item and the style
/// callback once for each List role. The composed frame retains only their text
/// and style results.
#[derive(Clone)]
pub struct ListItemPresentation<'a, T> {
    format: Arc<ItemFormatter<'a, T>>,
    style: Arc<ItemStyler<'a, T>>,
}

impl<T> fmt::Debug for ListItemPresentation<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ListItemPresentation { .. }")
    }
}

impl<'a, T: 'a> ListItemPresentation<'a, T> {
    /// Creates an item presentation with a custom formatter and no style overrides.
    pub fn new<F>(format: F) -> Self
    where
        F: Fn(&T, ListPosition) -> String + 'a,
    {
        Self {
            format: Arc::new(format),
            style: Arc::new(no_item_style::<T>),
        }
    }

    /// Replaces the per-item role-style policy.
    ///
    /// `None` keeps the [`ListPresentation`] role default. `Some(style)`
    /// replaces that complete style rather than layering over it.
    #[must_use]
    pub fn per_item_style<S>(mut self, style: S) -> Self
    where
        S: Fn(&T, ListPosition, ListRole) -> Option<TextStyle> + 'a,
    {
        self.style = Arc::new(style);
        self
    }
}

impl<'a, T> ListItemPresentation<'a, T>
where
    T: fmt::Display + 'a,
{
    /// Uses the value's canonical [`fmt::Display`] representation.
    pub fn display() -> Self {
        Self::new(display_item::<T>)
    }
}

fn display_item<T>(value: &T, _: ListPosition) -> String
where
    T: fmt::Display,
{
    value.to_string()
}

fn no_item_style<T>(_: &T, _: ListPosition, _: ListRole) -> Option<TextStyle> {
    None
}

/// Presentation policy used to compose a [`List`] into a [`View`].
///
/// [`ListPresentation::compose`] formats values with [`fmt::Display`]. Use
/// [`ListPresentation::compose_with`] with a [`ListItemPresentation<T>`] when
/// values need List-specific formatting or per-item role-style overrides.
#[derive(Debug, Clone)]
pub struct ListPresentation {
    item: TextStyle,
    enumerator_style: TextStyle,
    enumerator: ListEnumerator,
    nesting_indent: usize,
}

impl PartialEq for ListPresentation {
    fn eq(&self, other: &Self) -> bool {
        self.item == other.item
            && self.enumerator_style == other.enumerator_style
            && std::ptr::fn_addr_eq(self.enumerator, other.enumerator)
            && self.nesting_indent == other.nesting_indent
    }
}

impl ListPresentation {
    /// Creates the canonical list presentation with bullet markers.
    pub fn new(item: TextStyle, enumerator: TextStyle) -> Self {
        Self {
            item,
            enumerator_style: enumerator,
            enumerator: bullet_enumerator,
            nesting_indent: 2,
        }
    }
}

impl ListPresentation {
    /// Returns the style assigned to one logical list role.
    pub fn style(&self, role: ListRole) -> &TextStyle {
        match role {
            ListRole::Item => &self.item,
            ListRole::Enumerator => &self.enumerator_style,
        }
    }

    /// Replaces the style assigned to one logical list role.
    #[must_use]
    pub fn with_style(mut self, role: ListRole, style: TextStyle) -> Self {
        match role {
            ListRole::Item => self.item = style,
            ListRole::Enumerator => self.enumerator_style = style,
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

    /// Replaces the item-marker policy.
    #[must_use]
    pub const fn enumerator(mut self, enumerator: ListEnumerator) -> Self {
        self.enumerator = enumerator;
        self
    }

    /// Sets the fixed horizontal step between nesting levels, in terminal cells.
    #[must_use]
    pub const fn nesting_indent(mut self, nesting_indent: usize) -> Self {
        self.nesting_indent = nesting_indent;
        self
    }

    /// Composes displayable list data into an intrinsically sized Canvas.
    pub fn compose<T>(&self, list: &List<T>) -> View
    where
        T: fmt::Display,
    {
        self.compose_using(list, &display_item::<T>, &no_item_style::<T>)
    }

    /// Composes list data with typed item formatting and style overrides.
    pub fn compose_with<T>(&self, list: &List<T>, items: &ListItemPresentation<'_, T>) -> View {
        self.compose_using(list, &*items.format, &*items.style)
    }

    fn compose_using<T, F, S>(&self, list: &List<T>, format: &F, style: &S) -> View
    where
        F: Fn(&T, ListPosition) -> String + ?Sized,
        S: Fn(&T, ListPosition, ListRole) -> Option<TextStyle> + ?Sized,
    {
        if list.hidden {
            return View::empty();
        }

        let items = visible_items(&list.items, list.offset);
        if items.is_empty() {
            return View::empty();
        }

        let mut bound = Vec::new();
        bind_items(&mut bound, &items, 0, self, format, style);
        let item = ListCanvasItem(Arc::new(ListFrame { items: bound }));
        View::canvas(Canvas::new().sizing(item.sizing()).item(item))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundListItem {
    value: String,
    marker: String,
    item_style: TextStyle,
    enumerator_style: TextStyle,
    track_x: usize,
    marker_x: usize,
    content_x: usize,
    track_width: usize,
}

fn bind_items<T, F, S>(
    bound: &mut Vec<BoundListItem>,
    items: &[&ListItem<T>],
    depth: usize,
    presentation: &ListPresentation,
    format: &F,
    style: &S,
) where
    F: Fn(&T, ListPosition) -> String + ?Sized,
    S: Fn(&T, ListPosition, ListRole) -> Option<TextStyle> + ?Sized,
{
    let markers = (0..items.len())
        .map(|index| {
            normalize_marker((presentation.enumerator)(ListPosition::new(
                index,
                items.len(),
                depth,
            )))
        })
        .collect::<Vec<_>>();
    let track_width = markers
        .iter()
        .map(|marker| PrintableText::new(marker).width())
        .max()
        .unwrap_or(0);
    let track_x = depth.saturating_mul(presentation.nesting_indent);
    let content_x = track_x.saturating_add(track_width);

    for (index, (item, marker)) in items.iter().zip(markers).enumerate() {
        let position = ListPosition::new(index, items.len(), depth);
        let marker_width = PrintableText::new(&marker).width();
        bound.push(BoundListItem {
            value: format(&item.value, position),
            marker,
            item_style: style(&item.value, position, ListRole::Item)
                .unwrap_or_else(|| presentation.item.clone()),
            enumerator_style: style(&item.value, position, ListRole::Enumerator)
                .unwrap_or_else(|| presentation.enumerator_style.clone()),
            track_x,
            marker_x: track_x.saturating_add(track_width - marker_width),
            content_x,
            track_width,
        });
        let children = item.visible_items();
        if !children.is_empty() {
            bind_items(
                bound,
                &children,
                depth.saturating_add(1),
                presentation,
                format,
                style,
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ListFrame {
    items: Vec<BoundListItem>,
}

impl ListFrame {
    fn width_requirements(&self) -> CanvasRequirements {
        let (demand, floor) = self.items.iter().fold((0, 0), |requirements, item| {
            let lines = item.value.split('\n').map(PrintableText::new);
            let (content_demand, content_floor) = lines.fold((0, 0), |widths, line| {
                (
                    widths.0.max(line.width()),
                    widths
                        .1
                        .max(line.graphemes().map(Grapheme::width).max().unwrap_or(0)),
                )
            });
            (
                requirements
                    .0
                    .max(item.content_x.saturating_add(content_demand)),
                requirements
                    .1
                    .max(item.content_x.saturating_add(content_floor)),
            )
        });
        CanvasRequirements::new(demand, floor)
    }

    fn rows(&self, width: usize) -> Vec<ListRow> {
        let mut rows = Vec::new();
        for (item_index, item) in self.items.iter().enumerate() {
            let content_width = width.saturating_sub(item.content_x);
            let mut first = true;
            for line in item.value.split('\n') {
                for text in wrap_text(PrintableLines::new(line), content_width) {
                    rows.push(ListRow {
                        item_index,
                        text,
                        first,
                    });
                    first = false;
                }
            }
        }
        rows
    }

    fn draw(&self, context: &mut CanvasContext) {
        let width = context.size().width();
        for (y, row) in self.rows(width).into_iter().enumerate() {
            let item = &self.items[row.item_index];
            if row.first {
                let visible_track = width.saturating_sub(item.track_x).min(item.track_width);
                if visible_track > 0 {
                    context.text_with(
                        Position::new(position(item.track_x), position(y)),
                        " ".repeat(visible_track),
                        item.enumerator_style.clone(),
                        Composition::Replace,
                    );
                }
                context.text_with(
                    Position::new(position(item.marker_x), position(y)),
                    item.marker.clone(),
                    item.enumerator_style.clone(),
                    Composition::Replace,
                );
            }
            context.text_with(
                Position::new(position(item.content_x), position(y)),
                row.text,
                item.item_style.clone(),
                Composition::Replace,
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ListRow {
    item_index: usize,
    text: String,
    first: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct ListCanvasItem(Arc<ListFrame>);

impl ListCanvasItem {
    fn sizing(&self) -> CanvasSizing {
        CanvasSizing::intrinsic(self.clone())
    }
}

impl CanvasMeasure for ListCanvasItem {
    fn width_requirements(&self) -> CanvasRequirements {
        self.0.width_requirements()
    }

    fn height_requirements(&self, width: usize) -> CanvasRequirements {
        CanvasRequirements::new(self.0.rows(width).len(), 0)
    }
}

impl CanvasItem for ListCanvasItem {
    fn draw(&self, context: &mut CanvasContext) {
        self.0.draw(context);
    }
}

fn position(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Owned typed list data independent of presentation policy.
///
/// List owns its public model independently from Tree. Its presentation binds
/// this recursive data without coupling either public data API to the other
/// component.
///
/// Offsets are supported with [`List::offset`] and [`ListItem::offset`]. A
/// filter callback is intentionally not part of this API: omit items before
/// construction or mark individual [`ListItem`] values hidden instead.
///
/// [`List::new`] creates the same empty container for every value type. Calls
/// that append an item infer `T` from that value; an empty List states its type
/// explicitly or through its surrounding context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List<T> {
    items: Vec<ListItem<T>>,
    hidden: bool,
    offset: ItemOffset,
}

impl<T> Default for List<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            hidden: false,
            offset: ItemOffset::default(),
        }
    }
}

impl<T> List<T> {
    /// Creates an empty typed list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one top-level item.
    #[must_use]
    pub fn item(mut self, item: impl Into<ListItem<T>>) -> Self {
        self.items.push(item.into());
        self
    }

    /// Appends top-level items in iteration order.
    #[must_use]
    pub fn items<I, N>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = N>,
        N: Into<ListItem<T>>,
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
    pub fn item_nodes(&self) -> &[ListItem<T>] {
        &self.items
    }
}

fn visible_items<T>(items: &[ListItem<T>], offset: ItemOffset) -> Vec<&ListItem<T>> {
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::test_support::{plain, style_at};
    use crate::{
        Available, Color, ComponentTheme, SemanticTokens, StyledGrapheme, measure, resolve,
    };

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

    fn plain_at(view: &View, width: usize) -> String {
        resolve(view, Available::columns(width))
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

    #[test]
    fn renders_empty_flat_and_nested_lists() {
        assert!(measure(&styles().list().compose(&List::<String>::new())).is_empty());

        let list = List::<&str>::new()
            .item("alpha")
            .item(ListItem::new("beta").items(["nested", "last"]))
            .item("omega");

        assert_eq!(
            plain(&styles().list().compose(&list)),
            "• alpha\n• beta\n  • nested\n  • last\n• omega"
        );
    }

    #[test]
    fn list_items_are_independent_from_tree_nodes() {
        fn accepts_list_item(_: ListItem<&str>) {}
        accepts_list_item(ListItem::new("list").item("nested"));

        assert_eq!(*ListItem::new("item").value(), "item");
        assert_eq!(
            ListItem::new("item")
                .items(["one", "two"])
                .item_nodes()
                .len(),
            2
        );
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
    fn typed_values_reach_item_presentation_after_visibility_and_offsets() {
        let selected_item = TextStyle::new().foreground(Color::GREEN);
        let selected_enumerator = TextStyle::new().foreground(Color::BLUE);
        let list = List::<Task>::new()
            .item(Task {
                id: 0,
                label: "skip",
                selected: false,
            })
            .item(
                ListItem::new(Task {
                    id: 7,
                    label: "parent",
                    selected: true,
                })
                .items([
                    ListItem::new(Task {
                        id: 8,
                        label: "hidden",
                        selected: false,
                    })
                    .hidden(true),
                    ListItem::new(Task {
                        id: 9,
                        label: "child",
                        selected: false,
                    }),
                ]),
            )
            .item(Task {
                id: 10,
                label: "drop",
                selected: false,
            })
            .offset(1, 1);
        let item_style = selected_item.clone();
        let enumerator_style = selected_enumerator.clone();
        let items = ListItemPresentation::new(|task: &Task, position| {
            format!(
                "{}:{}:{}/{}@{}",
                task.id,
                task.label,
                position.index(),
                position.len(),
                position.depth()
            )
        })
        .per_item_style(move |task, _, role| {
            task.selected.then(|| match role {
                ListRole::Item => item_style.clone(),
                ListRole::Enumerator => enumerator_style.clone(),
            })
        });
        let component_styles = styles();
        let presentation = component_styles.list();
        let view = presentation.compose_with(&list, &items);

        assert_eq!(list.item_nodes()[1].value().id, 7);
        assert_eq!(plain(&view), "• 7:parent:0/1@0\n  • 9:child:0/1@1");
        assert_eq!(style_at(&view, 0, 0), selected_enumerator);
        assert_eq!(style_at(&view, 0, 2), selected_item);
        assert_eq!(view, presentation.compose_with(&list, &items.clone()));
    }

    #[test]
    fn default_presentation_formats_typed_display_values() {
        let list = List::new().items([
            Task {
                id: 1,
                label: "first",
                selected: false,
            },
            Task {
                id: 2,
                label: "second",
                selected: true,
            },
        ]);

        assert_eq!(plain(&styles().list().compose(&list)), "• first\n• second");
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
            let presentation = styles().list().clone().enumerator(enumerator);
            let view = presentation.compose(&list);
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
            assert_eq!(
                alphabet_enumerator(ListPosition::new(index, 703, 0)),
                expected
            );
        }
    }

    #[test]
    fn numeric_enumerators_are_bounded_at_extreme_public_positions() {
        assert_eq!(
            roman_enumerator(ListPosition::new(3998, 3999, usize::MAX)),
            "MMMCMXCIX. "
        );
        assert_eq!(
            roman_enumerator(ListPosition::new(3999, 4000, usize::MAX)),
            "4000. "
        );
        assert_eq!(
            roman_enumerator(ListPosition::new(usize::MAX, usize::MAX, usize::MAX)),
            format!("{}. ", usize::MAX)
        );
        assert_eq!(
            arabic_enumerator(ListPosition::new(usize::MAX, usize::MAX, usize::MAX)),
            format!("{}. ", usize::MAX)
        );
    }

    fn custom_enumerator(position: ListPosition) -> String {
        if position.depth() == 0 {
            format!("[{}] ", position.index())
        } else {
            "• ".to_owned()
        }
    }

    #[test]
    fn supports_custom_markers_styles_visibility_and_offsets() {
        let item = TextStyle::new().foreground(Color::GREEN);
        let enumerator = TextStyle::new().foreground(Color::BLUE);
        let list = List::<&str>::new()
            .items([
                ListItem::new("skip"),
                ListItem::new("parent")
                    .items([ListItem::new("hidden").hidden(true), ListItem::new("child")]),
                ListItem::new("drop"),
            ])
            .offset(1, 1);
        let presentation = styles()
            .list()
            .clone()
            .enumerator(custom_enumerator)
            .item_style(item.clone())
            .enumerator_style(enumerator.clone());
        let view = presentation.compose(&list);

        assert_eq!(plain(&view), "[0] parent\n  • child");
        assert_eq!(style_at(&view, 0, 0), enumerator);
        assert_eq!(style_at(&view, 0, 4), item);
        assert_eq!(style_at(&view, 1, 2), enumerator);
        assert_eq!(style_at(&view, 1, 4), item);
    }

    #[test]
    fn nested_offsets_apply_before_hidden_items() {
        let list = List::<&str>::new().item(
            ListItem::new("parent")
                .items(["skip", "hidden", "kept", "drop"])
                .offset(1, 1),
        );
        let hidden_list = List::<&str>::new().item(ListItem::new("parent").items([
            ListItem::new("visible"),
            ListItem::new("hidden").hidden(true),
        ]));
        let component_styles = styles();
        let view = component_styles.list().compose(&list);
        let hidden = component_styles.list().compose(&hidden_list);

        assert_eq!(plain(&view), "• parent\n  • hidden\n  • kept");
        assert_eq!(plain(&hidden), "• parent\n  • visible");
    }

    #[test]
    fn aligns_multiline_cjk_items_by_terminal_cell_width() {
        let list = List::new().items(["日本語\nsecond", "終端\n続き"]);
        let presentation = styles().list().clone().enumerator(arabic_enumerator);
        let view = presentation.compose(&list);

        assert_eq!(plain(&view), "1. 日本語\n   second\n2. 終端\n   続き");
    }

    #[test]
    fn selected_width_reflows_without_recomposing() {
        let view = styles().list().compose(&List::new().item("alpha beta"));

        assert_eq!(plain_at(&view, 12), "• alpha beta");
        assert_eq!(plain_at(&view, 8), "• alpha\n  beta");
    }

    fn hanging_enumerator(position: ListPosition) -> String {
        if position.depth() > 0 {
            "• ".to_owned()
        } else if position.index() == 0 {
            "1. ".to_owned()
        } else {
            "1000. ".to_owned()
        }
    }

    #[test]
    fn aligns_markers_in_local_sibling_tracks() {
        let list = List::new()
            .item(ListItem::new("parent").item("child"))
            .item("last");
        let view = styles()
            .list()
            .clone()
            .enumerator(hanging_enumerator)
            .compose(&list);

        assert_eq!(plain(&view), "   1. parent\n  • child\n1000. last");
    }

    #[test]
    fn marker_style_covers_geometric_alignment_cells() {
        let marker = TextStyle::new().background(Color::BLUE);
        let view = styles()
            .list()
            .clone()
            .enumerator(hanging_enumerator)
            .enumerator_style(marker.clone())
            .compose(&List::new().items(["first", "second"]));

        assert_eq!(style_at(&view, 0, 0), marker);
    }

    #[test]
    fn preserves_explicit_and_wrapped_continuations_including_trailing_empty_line() {
        let view = styles().list().compose(&List::new().item("ab cd\n日\n"));

        assert_eq!(plain_at(&view, 5), "• ab\n  cd\n  日\n");
    }

    fn normalized_wide_enumerator(position: ListPosition) -> String {
        if position.index() == 0 {
            "👩‍💻\r\n".to_owned()
        } else {
            "• ".to_owned()
        }
    }

    #[test]
    fn normalizes_and_aligns_cjk_and_emoji_markers() {
        let view = styles()
            .list()
            .clone()
            .enumerator(normalized_wide_enumerator)
            .compose(&List::new().items(["one", "two"]));

        assert_eq!(plain(&view), "👩‍💻 one\n • two");
    }

    #[test]
    fn width_requirements_include_prefix_and_widest_grapheme() {
        let list = List::new().item("a日本語");
        let items = visible_items(&list.items, list.offset);
        let presentation = styles().list().clone();
        let mut bound = Vec::new();
        bind_items(
            &mut bound,
            &items,
            0,
            &presentation,
            &display_item::<&str>,
            &no_item_style::<&str>,
        );
        let requirements = ListFrame { items: bound }.width_requirements();

        assert_eq!(requirements.demand(), 9);
        assert_eq!(requirements.floor(), 4);
    }

    static ENUMERATOR_CALLS: AtomicUsize = AtomicUsize::new(0);
    static TEXT_CALLS: AtomicUsize = AtomicUsize::new(0);
    static ITEM_STYLE_CALLS: AtomicUsize = AtomicUsize::new(0);
    static ENUMERATOR_STYLE_CALLS: AtomicUsize = AtomicUsize::new(0);

    fn counted_enumerator(_: ListPosition) -> String {
        ENUMERATOR_CALLS.fetch_add(1, Ordering::Relaxed);
        "• ".to_owned()
    }

    fn counted_text(value: &&str, _: ListPosition) -> String {
        TEXT_CALLS.fetch_add(1, Ordering::Relaxed);
        (*value).to_owned()
    }

    fn counted_style(_: &&str, _: ListPosition, role: ListRole) -> Option<TextStyle> {
        match role {
            ListRole::Item => &ITEM_STYLE_CALLS,
            ListRole::Enumerator => &ENUMERATOR_STYLE_CALLS,
        }
        .fetch_add(1, Ordering::Relaxed);
        None
    }

    #[test]
    fn evaluates_enumerators_only_while_composing() {
        ENUMERATOR_CALLS.store(0, Ordering::Relaxed);
        TEXT_CALLS.store(0, Ordering::Relaxed);
        ITEM_STYLE_CALLS.store(0, Ordering::Relaxed);
        ENUMERATOR_STYLE_CALLS.store(0, Ordering::Relaxed);
        let presentation = styles().list().clone().enumerator(counted_enumerator);
        let items = ListItemPresentation::new(counted_text).per_item_style(counted_style);
        let view = presentation.compose_with(&List::new().items(["one", "two"]), &items);
        assert_eq!(ENUMERATOR_CALLS.load(Ordering::Relaxed), 2);
        assert_eq!(TEXT_CALLS.load(Ordering::Relaxed), 2);
        assert_eq!(ITEM_STYLE_CALLS.load(Ordering::Relaxed), 2);
        assert_eq!(ENUMERATOR_STYLE_CALLS.load(Ordering::Relaxed), 2);

        let _ = resolve(&view, Available::columns(8));
        let _ = resolve(&view, Available::columns(4));
        assert_eq!(ENUMERATOR_CALLS.load(Ordering::Relaxed), 2);
        assert_eq!(TEXT_CALLS.load(Ordering::Relaxed), 2);
        assert_eq!(ITEM_STYLE_CALLS.load(Ordering::Relaxed), 2);
        assert_eq!(ENUMERATOR_STYLE_CALLS.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn extreme_nesting_arithmetic_saturates() {
        let list = List::<&str>::new().item(ListItem::new("parent").item("child"));
        let items = visible_items(&list.items, list.offset);
        let presentation = styles().list().clone().nesting_indent(usize::MAX);
        let mut bound = Vec::new();

        bind_items(
            &mut bound,
            &items,
            0,
            &presentation,
            &display_item::<&str>,
            &no_item_style::<&str>,
        );

        assert_eq!(bound[1].track_x, usize::MAX);
        assert_eq!(bound[1].content_x, usize::MAX);
    }

    #[test]
    fn hidden_list_returns_an_empty_view() {
        let list = List::new().item("item").hidden(true);
        assert!(measure(&styles().list().compose(&list)).is_empty());
    }
}
