use std::fmt;
use std::sync::Arc;

use urushi::{
    BlockStyle, Canvas, CanvasContext, CanvasItem, Key, Length, Position, ResolvedView, Size,
    TextStyle, Theme, View,
};
use urushi_terminal::PixelSize as CellPixelSize;

/// Pixel dimensions of a prepared raster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PixelSize {
    width: u32,
    height: u32,
}

/// Pixel coordinates within a prepared raster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PixelPosition {
    x: u32,
    y: u32,
}

impl PixelPosition {
    pub const fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    pub const fn x(self) -> u32 {
        self.x
    }

    pub const fn y(self) -> u32 {
        self.y
    }
}

/// Terminal-cell dimensions requested for one image placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellSize {
    width: u16,
    height: u16,
}

impl CellSize {
    pub const ZERO: Self = Self::new(0, 0);

    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    pub const fn width(self) -> u16 {
        self.width
    }

    pub const fn height(self) -> u16 {
        self.height
    }
}

impl PixelSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }
}

/// Why RGBA bytes cannot form a prepared raster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidRgbaRaster {
    /// At least one pixel dimension is zero.
    Empty,
    /// The byte length does not equal `width * height * 4`.
    ByteLength { expected: usize, actual: usize },
    /// The expected byte length does not fit in `usize`.
    ByteLengthOverflow,
}

impl fmt::Display for InvalidRgbaRaster {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("an RGBA raster must have non-zero dimensions"),
            Self::ByteLength { expected, actual } => write!(
                formatter,
                "an RGBA raster of this size needs {expected} bytes, but received {actual}"
            ),
            Self::ByteLengthOverflow => {
                formatter.write_str("the RGBA raster byte length does not fit in memory")
            }
        }
    }
}

impl std::error::Error for InvalidRgbaRaster {}

/// Immutable, prepared RGBA pixels identified independently from a placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaRaster {
    key: Key,
    size: PixelSize,
    bytes: Arc<[u8]>,
}

impl RgbaRaster {
    fn new(
        key: impl Into<Key>,
        size: PixelSize,
        bytes: impl Into<Arc<[u8]>>,
    ) -> Result<Self, InvalidRgbaRaster> {
        if size.width == 0 || size.height == 0 {
            return Err(InvalidRgbaRaster::Empty);
        }
        let expected = usize::try_from(size.width)
            .ok()
            .and_then(|width| {
                usize::try_from(size.height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(InvalidRgbaRaster::ByteLengthOverflow)?;
        let bytes = bytes.into();
        if bytes.len() != expected {
            return Err(InvalidRgbaRaster::ByteLength {
                expected,
                actual: bytes.len(),
            });
        }
        Ok(Self {
            key: key.into(),
            size,
            bytes,
        })
    }

    /// Returns the stable identity of the prepared pixel content.
    pub const fn key(&self) -> Key {
        self.key
    }

    pub const fn size(&self) -> PixelSize {
        self.size
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Prepared image content and the identities of its asset and placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    key: Key,
    raster: RgbaRaster,
    fallback: String,
}

impl Image {
    /// Creates an image from checked, prepared RGBA pixels.
    pub fn rgba(
        key: impl Into<Key>,
        asset_key: impl Into<Key>,
        pixels: PixelSize,
        rgba: impl Into<Arc<[u8]>>,
    ) -> Result<Self, InvalidRgbaRaster> {
        Ok(Self {
            key: key.into(),
            raster: RgbaRaster::new(asset_key, pixels, rgba)?,
            fallback: "[image]".to_owned(),
        })
    }

    /// Replaces the text shown when no graphics adapter is used.
    #[must_use]
    pub fn fallback(mut self, fallback: impl Into<String>) -> Self {
        self.fallback = fallback.into();
        self
    }

    pub const fn key(&self) -> Key {
        self.key
    }

    pub const fn raster(&self) -> &RgbaRaster {
        &self.raster
    }

    pub fn get_fallback(&self) -> &str {
        &self.fallback
    }

    /// Locates this image in an already resolved Urushi view.
    pub fn placement<'a>(
        &'a self,
        view: &ResolvedView,
        cell_pixels: Option<CellPixelSize>,
    ) -> Option<GraphicPlacement<'a>> {
        let region = view.anchor(self.key)?;
        let visible = region.visible();
        Some(GraphicPlacement {
            key: self.key,
            raster: &self.raster,
            logical_origin: Position::new(region.x(), region.y()),
            logical_size: Size::new(region.width(), region.height()),
            visible_origin: visible.map(|visible| Position::new(visible.x(), visible.y())),
            visible_size: visible.map(|visible| Size::new(visible.width(), visible.height())),
            cell_pixels,
        })
    }
}

/// Canonical policy for composing an [`Image`] into a fixed cell rectangle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePresentation {
    fallback_style: TextStyle,
}

impl Default for ImagePresentation {
    fn default() -> Self {
        Self::new()
    }
}

impl ImagePresentation {
    /// Creates an image presentation with the default fallback text style.
    pub fn new() -> Self {
        Self {
            fallback_style: TextStyle::new(),
        }
    }

    /// Derives the canonical image fallback style from an Urushi theme.
    pub fn from_theme(theme: &Theme) -> Self {
        Self::new().fallback_style(
            TextStyle::new()
                .foreground(theme.tokens().text_muted)
                .italic(),
        )
    }

    pub const fn get_fallback_style(&self) -> &TextStyle {
        &self.fallback_style
    }

    #[must_use]
    pub fn fallback_style(mut self, style: TextStyle) -> Self {
        self.fallback_style = style;
        self
    }

    /// Composes `image` as a generic anchored region with fallback cells.
    pub fn compose(&self, image: &Image, cells: CellSize) -> View {
        let size = Size::new(usize::from(cells.width()), usize::from(cells.height()));
        let canvas = Canvas::new().extent(size).item(ImageCanvasItem {
            image: image.clone(),
            cells,
            fallback_style: self.fallback_style.clone(),
        });
        View::block(
            BlockStyle::new()
                .width(Length::Cells(cells.width()))
                .height(Length::Cells(cells.height())),
            View::canvas(canvas),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ImageCanvasItem {
    image: Image,
    cells: CellSize,
    fallback_style: TextStyle,
}

impl CanvasItem for ImageCanvasItem {
    fn draw(&self, context: &mut CanvasContext) {
        let width = usize::from(self.cells.width());
        let height = usize::from(self.cells.height());
        context.view(
            Position::default(),
            View::anchor_block(
                self.image.key,
                BlockStyle::new()
                    .width(Length::Cells(self.cells.width()))
                    .height(Length::Cells(self.cells.height())),
                View::empty(),
            ),
            Some(width),
            Some(height),
        );
        context.text(
            Position::default(),
            self.image.fallback.clone(),
            self.fallback_style.clone(),
        );
    }
}

pub(crate) fn collect(view: &View) -> Vec<&Image> {
    let mut images = Vec::new();
    collect_from(view, &mut images);
    images
}

fn collect_from<'a>(view: &'a View, images: &mut Vec<&'a Image>) {
    match view {
        View::Text(_) => {}
        View::Block(_, _, child) | View::Viewport(_, child) | View::AnchorBlock(_, _, _, child) => {
            collect_from(child, images);
        }
        View::Row(_, children) | View::Column(_, children) => {
            for child in children {
                collect_from(child, images);
            }
        }
        View::Grid(_, rows) => {
            for child in rows.iter().flatten() {
                collect_from(child, images);
            }
        }
        View::Canvas(canvas) => {
            images.extend(canvas.items::<ImageCanvasItem>().map(|item| &item.image));
        }
    }
}

/// One renderer-neutral image placement derived from an Urushi anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicPlacement<'a> {
    key: Key,
    raster: &'a RgbaRaster,
    logical_origin: Position,
    logical_size: Size,
    visible_origin: Option<Position>,
    visible_size: Option<Size>,
    cell_pixels: Option<CellPixelSize>,
}

impl<'a> GraphicPlacement<'a> {
    pub const fn key(&self) -> Key {
        self.key
    }

    pub const fn raster(&self) -> &'a RgbaRaster {
        self.raster
    }

    /// Returns the unclipped rectangle's origin in resolved-view cells.
    pub const fn logical_origin(&self) -> Position {
        self.logical_origin
    }

    /// Returns the unclipped rectangle's size in cells.
    pub const fn logical_size(&self) -> Size {
        self.logical_size
    }

    /// Returns the origin of the rectangle that survived every enclosing clip.
    pub const fn visible_origin(&self) -> Option<Position> {
        self.visible_origin
    }

    /// Returns the size of the rectangle that survived every enclosing clip.
    pub const fn visible_size(&self) -> Option<Size> {
        self.visible_size
    }

    /// Returns the observed dimensions of one terminal cell, when available.
    pub const fn cell_pixels(&self) -> Option<CellPixelSize> {
        self.cell_pixels
    }

    /// Returns the visible rectangle's offset within the source raster.
    pub fn source_offset(&self) -> Option<PixelPosition> {
        let (x, y, _, _) = self.source_bounds()?;
        Some(PixelPosition::new(x, y))
    }

    /// Returns the visible rectangle's extent within the source raster.
    pub fn source_size(&self) -> Option<PixelSize> {
        let (left, top, right, bottom) = self.source_bounds()?;
        Some(PixelSize::new(right - left, bottom - top))
    }

    /// Returns the visible output extent in terminal pixels.
    pub fn visible_pixels(&self) -> Option<CellPixelSize> {
        let visible = self.visible_size?;
        let cell = self.cell_pixels?;
        Some(CellPixelSize::new(
            visible.width().checked_mul(cell.width())?,
            visible.height().checked_mul(cell.height())?,
        ))
    }

    fn source_bounds(&self) -> Option<(u32, u32, u32, u32)> {
        let visible_origin = self.visible_origin?;
        let visible_size = self.visible_size?;
        if self.logical_size.is_empty() || visible_size.is_empty() {
            return None;
        }
        let offset_x = u128::try_from(visible_origin.x.checked_sub(self.logical_origin.x)?).ok()?;
        let offset_y = u128::try_from(visible_origin.y.checked_sub(self.logical_origin.y)?).ok()?;
        let logical_width = self.logical_size.width() as u128;
        let logical_height = self.logical_size.height() as u128;
        let raster_width = u128::from(self.raster.size().width());
        let raster_height = u128::from(self.raster.size().height());
        let visible_right = offset_x.checked_add(visible_size.width() as u128)?;
        let visible_bottom = offset_y.checked_add(visible_size.height() as u128)?;
        let left = raster_width.checked_mul(offset_x)? / logical_width;
        let top = raster_height.checked_mul(offset_y)? / logical_height;
        let right = raster_width
            .checked_mul(visible_right)?
            .div_ceil(logical_width)
            .min(raster_width);
        let bottom = raster_height
            .checked_mul(visible_bottom)?
            .div_ceil(logical_height)
            .min(raster_height);
        Some((
            left.try_into().ok()?,
            top.try_into().ok()?,
            right.try_into().ok()?,
            bottom.try_into().ok()?,
        ))
    }
}
