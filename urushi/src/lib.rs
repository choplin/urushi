//! Composable styling, layout, and rendering for terminal applications.
//!
//! `urushi` separates logical [`View`] construction, layout resolution, ANSI
//! serialization, and output. [`resolve`] turns a view into a [`ResolvedView`]
//! under an [`Available`] area. [`render`] then serializes that rectangle using
//! explicit [`RenderSettings`]. [`render_text`] bypasses layout for a
//! [`StyledText`], preserving source tabs and line boundaries. [`print()`],
//! [`println()`], [`eprint()`], and [`eprintln()`] are the convenient text path
//! for the process standard streams; their `*_view` peers resolve layout first.
//!
//! # Example
//!
//! ```
//! use urushi::{Available, BlockStyle, Border, RenderSettings, TextStyle, View, render, resolve};
//!
//! let style = BlockStyle::new().border(Border::ROUNDED).padding((0, 1));
//! let view = View::block(style, View::text("こんにちは, urushi!", TextStyle::new()));
//! let resolved = resolve(&view, Available::NONE).unwrap();
//!
//! assert_eq!(render(&resolved, &RenderSettings::default()).lines().count(), 3);
//! ```
//!
//! Width calculations are aware of East Asian wide characters. Terminal
//! contracts and inspection live in the workspace-independent
//! `urushi-terminal` crate; arbitrary writers combine its detection with
//! [`resolve`], [`render`], and [`std::io::Write`].

mod component;
mod key;
mod output;
mod render;
mod style;
#[cfg(test)]
mod test_support;
mod text;
mod theme;
mod view;

pub use component::{
    List, ListEnumerator, ListItem, ListItemPresentation, ListPosition, ListPresentation, Table,
    TableBorder, TableCell, TablePresentation, TableRow, TableRowCells, TableRowPosition,
    TableRowPresentation, TextTable, TextTableRow, Tree, TreeNode, TreeNodePresentation,
    TreePosition, TreePresentation, alphabet_enumerator, arabic_enumerator, asterisk_enumerator,
    bullet_enumerator, dash_enumerator, roman_enumerator,
};
pub use key::Key;
pub use output::{
    eprint, eprint_view, eprintln, eprintln_view, print, print_view, println, println_view,
};
pub use render::{RenderSettings, render, render_text};
pub use style::{
    Align, BlockStyle, Border, Color, GridStyle, Hyperlink, InvalidFillWeight, Length, Modifier,
    Overflow, Sides, TextStyle, Underline, UnderlineStyle, UnderlineStyleSet, VerticalAlign,
};
pub use text::{
    Grapheme, InvalidTabMarker, PrintableLines, PrintableText, StyledText, StyledTextError,
    TabPolicy, TextSpan,
};
pub use theme::{
    BlockThemeRole, ColorScheme, ComponentRole, ComponentTheme, ListRole, PanelRole,
    SemanticTokens, TableRole, TextThemeRole, Theme, ThemeSet, TreeRole,
};
pub use urushi_derive::TableRow;
pub use urushi_terminal::ColorLevel;
pub use view::{
    AnchoredRect, Available, Axis, BlockTitle, Canvas, CanvasCell, CanvasContext, CanvasItem,
    CanvasSizing, CellContribution, Composition, LayoutError, LayoutErrorKind, LineContinuations,
    LineGlyphs, LineNetwork, Position, PositionedCell, ResolvedView, Size, StyledGrapheme, View,
    measure, resolve, try_measure,
};
