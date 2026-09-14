//! Composable styling, layout, and rendering for terminal applications.
//!
//! `urushi` separates logical [`View`] construction, layout resolution, ANSI
//! serialization, and output. [`resolve`] turns a view into a [`ResolvedView`]
//! under an [`Available`] area. [`render`] then serializes that rectangle using
//! explicit [`RenderSettings`]. [`print()`], [`println()`], [`eprint()`], and
//! [`eprintln()`] are the convenient path for the process standard streams.
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
//! inspection lives in the lower-level `urushi-terminal` crate; arbitrary
//! writers combine it with [`resolve`], [`render`], and [`std::io::Write`].

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
    List, ListEnumerator, ListItem, ListPosition, ListPresentation, Summary, SummaryField,
    SummaryPresentation, Table, TableBorder, TableCell, TableCellStyler, TablePresentation, Tree,
    TreeNode, TreePresentation, Warning, alphabet_enumerator, arabic_enumerator,
    asterisk_enumerator, bullet_enumerator, dash_enumerator, roman_enumerator,
};
pub use key::Key;
pub use output::{eprint, eprintln, print, println};
pub use render::{RenderSettings, render};
pub use style::{
    Align, BlockStyle, BlockStyleProperty, BlockStylePropertyKey, Border, Color, GridStyle,
    GridStyleProperty, GridStylePropertyKey, Hyperlink, InvalidFillWeight, Length, Modifier,
    Overflow, Sides, TextStyle, TextStyleProperty, TextStylePropertyKey, Underline, UnderlineStyle,
    UnderlineStyleSet, VerticalAlign,
};
pub use text::{Grapheme, PrintableLines, PrintableText, StyledText, StyledTextError, TextSpan};
pub use theme::{
    BlockThemeRole, ColorScheme, ComponentRole, ComponentTheme, ListRole, PanelRole,
    SemanticTokens, TableRole, TextThemeRole, Theme, ThemeSet, TreeRole,
};
pub use urushi_terminal::ColorLevel;
pub use view::{
    AnchoredRect, Available, Axis, BlockTitle, Canvas, CanvasCell, CanvasContext, CanvasItem,
    CanvasSizing, CellContribution, Composition, LayoutError, LayoutErrorKind, LineContinuations,
    LineGlyphs, LineNetwork, Position, PositionedCell, ResolvedView, Size, StyledGrapheme, View,
    measure, resolve, try_measure,
};
