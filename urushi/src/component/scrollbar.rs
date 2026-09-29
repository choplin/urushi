//! A finite-viewport position and its selectable terminal presentation.

use crate::{
    BlockStyle, Canvas, CanvasContext, CanvasItem, CellContribution, Composition, Grapheme, Length,
    Position, PositionedCell, PrintableText, ScrollbarRole, TextStyle, View,
};

/// The axis along which a [`Scrollbar`] describes a viewport.
///
/// Orientation does not choose an edge. A parent `View` places a vertical
/// scrollbar on the left or right, or a horizontal scrollbar above or below
/// its content.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ScrollbarOrientation {
    /// The viewport moves through rows.
    #[default]
    Vertical,
    /// The viewport moves through columns.
    Horizontal,
}

/// How a [`ScrollbarPresentation`] represents the visible viewport on its track.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ScrollbarThumbSizing {
    /// Size the thumb in proportion to the visible share of the content.
    #[default]
    Proportional,
    /// Draw one cell that represents position without representing viewport size.
    Marker,
}

/// Immutable semantic input for one finite-viewport position indicator.
///
/// Lengths and positions use caller-defined content units. They may be rows,
/// columns, items, or another uniform unit, provided all three values use the
/// same unit. Rendering clamps `position` to the last origin at which the
/// viewport remains within the content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Scrollbar {
    orientation: ScrollbarOrientation,
    content_length: usize,
    viewport_length: usize,
    position: usize,
}

impl Scrollbar {
    /// Creates a scrollbar at the beginning of its content.
    pub const fn new(
        orientation: ScrollbarOrientation,
        content_length: usize,
        viewport_length: usize,
    ) -> Self {
        Self {
            orientation,
            content_length,
            viewport_length,
            position: 0,
        }
    }

    /// Replaces the requested viewport origin.
    ///
    /// The presentation clamps an out-of-range value after accounting for the
    /// current content and viewport lengths.
    #[must_use]
    pub const fn position(mut self, position: usize) -> Self {
        self.position = position;
        self
    }

    pub const fn orientation(&self) -> ScrollbarOrientation {
        self.orientation
    }

    pub const fn content_length(&self) -> usize {
        self.content_length
    }

    pub const fn viewport_length(&self) -> usize {
        self.viewport_length
    }

    pub const fn get_position(&self) -> usize {
        self.position
    }
}

/// The four glyphs used along one scrollbar orientation.
///
/// The thumb is required. Track and endpoint glyphs are optional, allowing a
/// presentation to range from a conventional arrowed rail to a thumb-only
/// indicator. Every present glyph must be exactly one printable, one-cell
/// grapheme.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScrollbarGlyphs {
    thumb: String,
    track: Option<String>,
    begin: Option<String>,
    end: Option<String>,
}

impl ScrollbarGlyphs {
    /// Creates a thumb-only glyph set.
    ///
    /// # Panics
    ///
    /// Panics unless `thumb` is exactly one printable, one-cell grapheme.
    pub fn new(thumb: impl Into<String>) -> Self {
        Self {
            thumb: checked_glyph(thumb),
            track: None,
            begin: None,
            end: None,
        }
    }

    /// Replaces or removes the track glyph.
    ///
    /// # Panics
    ///
    /// Panics unless a present glyph is exactly one printable, one-cell
    /// grapheme.
    #[must_use]
    pub fn track(mut self, track: Option<&str>) -> Self {
        self.track = track.map(checked_glyph);
        self
    }

    /// Replaces or removes the beginning glyph.
    ///
    /// # Panics
    ///
    /// Panics unless a present glyph is exactly one printable, one-cell
    /// grapheme.
    #[must_use]
    pub fn begin(mut self, begin: Option<&str>) -> Self {
        self.begin = begin.map(checked_glyph);
        self
    }

    /// Replaces or removes the ending glyph.
    ///
    /// # Panics
    ///
    /// Panics unless a present glyph is exactly one printable, one-cell
    /// grapheme.
    #[must_use]
    pub fn end(mut self, end: Option<&str>) -> Self {
        self.end = end.map(checked_glyph);
        self
    }

    pub fn get_thumb(&self) -> &str {
        &self.thumb
    }

    pub fn get_track(&self) -> Option<&str> {
        self.track.as_deref()
    }

    pub fn get_begin(&self) -> Option<&str> {
        self.begin.as_deref()
    }

    pub fn get_end(&self) -> Option<&str> {
        self.end.as_deref()
    }
}

fn checked_glyph(glyph: impl Into<String>) -> String {
    let glyph = glyph.into();
    assert!(
        !glyph.chars().any(char::is_control),
        "a scrollbar glyph must not contain control characters: {glyph:?}"
    );
    let mut graphemes = PrintableText::new(&glyph).graphemes();
    let first = graphemes.next();
    assert!(
        first.is_some() && graphemes.next().is_none(),
        "a scrollbar glyph must be exactly one grapheme: {glyph:?}"
    );
    assert_eq!(
        first.map(Grapheme::width),
        Some(1),
        "a scrollbar glyph must occupy exactly one terminal cell: {glyph:?}"
    );
    drop(graphemes);
    glyph
}

/// Selectable visual and geometric policy for a [`Scrollbar`].
///
/// The presentation stores separate glyph sets for both orientations so the
/// same value can compose vertical and horizontal semantic scrollbars. A
/// caller can clone a Theme-owned presentation and replace either set or any
/// logical part style without changing the semantic value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollbarPresentation {
    vertical_glyphs: ScrollbarGlyphs,
    horizontal_glyphs: ScrollbarGlyphs,
    styles: [TextStyle; 4],
    thumb_sizing: ScrollbarThumbSizing,
}

impl ScrollbarPresentation {
    /// Creates a proportional presentation with conventional thin rails.
    pub fn new(thumb_style: TextStyle, track_style: TextStyle) -> Self {
        Self {
            vertical_glyphs: ScrollbarGlyphs::new("█")
                .track(Some("│"))
                .begin(Some("↑"))
                .end(Some("↓")),
            horizontal_glyphs: ScrollbarGlyphs::new("█")
                .track(Some("─"))
                .begin(Some("←"))
                .end(Some("→")),
            styles: [
                thumb_style,
                track_style.clone(),
                track_style.clone(),
                track_style,
            ],
            thumb_sizing: ScrollbarThumbSizing::Proportional,
        }
    }

    pub fn get_glyphs(&self, orientation: ScrollbarOrientation) -> &ScrollbarGlyphs {
        match orientation {
            ScrollbarOrientation::Vertical => &self.vertical_glyphs,
            ScrollbarOrientation::Horizontal => &self.horizontal_glyphs,
        }
    }

    /// Replaces the glyph set for one orientation.
    #[must_use]
    pub fn glyphs(mut self, orientation: ScrollbarOrientation, glyphs: ScrollbarGlyphs) -> Self {
        match orientation {
            ScrollbarOrientation::Vertical => self.vertical_glyphs = glyphs,
            ScrollbarOrientation::Horizontal => self.horizontal_glyphs = glyphs,
        }
        self
    }

    pub const fn get_thumb_sizing(&self) -> ScrollbarThumbSizing {
        self.thumb_sizing
    }

    /// Replaces the policy that maps viewport state onto the track.
    #[must_use]
    pub const fn thumb_sizing(mut self, thumb_sizing: ScrollbarThumbSizing) -> Self {
        self.thumb_sizing = thumb_sizing;
        self
    }

    pub fn get_style(&self, role: ScrollbarRole) -> &TextStyle {
        &self.styles[role.index()]
    }

    /// Replaces one complete logical part style.
    #[must_use]
    pub fn style(mut self, role: ScrollbarRole, style: TextStyle) -> Self {
        self.styles[role.index()] = style;
        self
    }

    #[must_use]
    pub fn thumb_style(self, style: TextStyle) -> Self {
        self.style(ScrollbarRole::Thumb, style)
    }

    #[must_use]
    pub fn track_style(self, style: TextStyle) -> Self {
        self.style(ScrollbarRole::Track, style)
    }

    #[must_use]
    pub fn begin_style(self, style: TextStyle) -> Self {
        self.style(ScrollbarRole::Begin, style)
    }

    #[must_use]
    pub fn end_style(self, style: TextStyle) -> Self {
        self.style(ScrollbarRole::End, style)
    }

    /// Composes the semantic viewport state into a one-cell-cross-axis View.
    ///
    /// The main axis remains area-dependent. Resolving a vertical scrollbar
    /// therefore requires a finite height, and resolving a horizontal one
    /// requires a finite width.
    pub fn compose(&self, scrollbar: &Scrollbar) -> View {
        let item = ScrollbarItem {
            scrollbar: *scrollbar,
            glyphs: self.get_glyphs(scrollbar.orientation).clone(),
            styles: self.styles.clone(),
            thumb_sizing: self.thumb_sizing,
        };
        let canvas = View::canvas(Canvas::new().item(item));
        let style = match scrollbar.orientation {
            ScrollbarOrientation::Vertical => BlockStyle::new()
                .width(Length::Cells(1))
                .height(Length::fill(1)),
            ScrollbarOrientation::Horizontal => BlockStyle::new()
                .width(Length::fill(1))
                .height(Length::Cells(1)),
        };
        View::block(style, canvas)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScrollbarItem {
    scrollbar: Scrollbar,
    glyphs: ScrollbarGlyphs,
    styles: [TextStyle; 4],
    thumb_sizing: ScrollbarThumbSizing,
}

impl CanvasItem for ScrollbarItem {
    fn draw(&self, context: &mut CanvasContext) {
        if self.scrollbar.content_length == 0 {
            return;
        }
        let extent = match self.scrollbar.orientation {
            ScrollbarOrientation::Vertical => context.size().height(),
            ScrollbarOrientation::Horizontal => context.size().width(),
        };
        if extent == 0 {
            return;
        }

        let endpoint_count = usize::from(self.glyphs.begin.is_some())
            .saturating_add(usize::from(self.glyphs.end.is_some()));
        let show_endpoints = extent >= endpoint_count.saturating_add(1);
        let begin = show_endpoints
            .then_some(self.glyphs.begin.as_deref())
            .flatten();
        let end = show_endpoints
            .then_some(self.glyphs.end.as_deref())
            .flatten();
        let track_origin = usize::from(begin.is_some());
        let track_length = extent
            .saturating_sub(track_origin)
            .saturating_sub(usize::from(end.is_some()));
        if track_length == 0 {
            return;
        }

        if let Some(begin) = begin {
            self.draw_part(context, 0, begin, ScrollbarRole::Begin);
        }
        if let Some(track) = self.glyphs.track.as_deref() {
            let cells = (0..track_length).map(|offset| {
                self.cell(
                    track_origin.saturating_add(offset),
                    track,
                    ScrollbarRole::Track,
                )
            });
            context.cells_with(cells, Composition::Replace);
        }
        if let Some(end) = end {
            self.draw_part(
                context,
                track_origin.saturating_add(track_length),
                end,
                ScrollbarRole::End,
            );
        }

        let (thumb_start, thumb_length) = self.thumb_geometry(track_length);
        let cells = (0..thumb_length).map(|offset| {
            self.cell(
                track_origin
                    .saturating_add(thumb_start)
                    .saturating_add(offset),
                &self.glyphs.thumb,
                ScrollbarRole::Thumb,
            )
        });
        context.cells_with(cells, Composition::Replace);
    }
}

impl ScrollbarItem {
    fn thumb_geometry(&self, track_length: usize) -> (usize, usize) {
        let content = self.scrollbar.content_length;
        let viewport = self.scrollbar.viewport_length.min(content);
        let maximum = content.saturating_sub(viewport);
        let position = self.scrollbar.position.min(maximum);

        match self.thumb_sizing {
            ScrollbarThumbSizing::Proportional => {
                if maximum == 0 {
                    return (0, track_length);
                }
                let length = scale_round(viewport, track_length, content).clamp(1, track_length);
                let travel = track_length.saturating_sub(length);
                (scale_round(position, travel, maximum), length)
            }
            ScrollbarThumbSizing::Marker => {
                let travel = track_length.saturating_sub(1);
                let start = if maximum == 0 {
                    0
                } else {
                    scale_round(position, travel, maximum)
                };
                (start, 1)
            }
        }
    }

    fn draw_part(
        &self,
        context: &mut CanvasContext,
        offset: usize,
        glyph: &str,
        role: ScrollbarRole,
    ) {
        context.cells_with([self.cell(offset, glyph, role)], Composition::Replace);
    }

    fn cell(&self, offset: usize, glyph: &str, role: ScrollbarRole) -> PositionedCell {
        let position = match self.scrollbar.orientation {
            ScrollbarOrientation::Vertical => Position::new(0, coordinate(offset)),
            ScrollbarOrientation::Horizontal => Position::new(coordinate(offset), 0),
        };
        PositionedCell::new(
            position,
            CellContribution::new()
                .symbol(Grapheme::new(glyph))
                .style(self.styles[role.index()].clone()),
        )
    }
}

fn scale_round(value: usize, scale: usize, denominator: usize) -> usize {
    debug_assert!(denominator > 0);
    let numerator = (value as u128).saturating_mul(scale as u128);
    let rounded = numerator.saturating_add((denominator / 2) as u128) / denominator as u128;
    usize::try_from(rounded).unwrap_or(usize::MAX)
}

fn coordinate(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
