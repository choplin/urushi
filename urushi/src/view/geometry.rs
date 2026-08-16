//! The two numbers resolution maps between: the area a view is given, and the
//! size it resolved to.

/// The size of a resolved rectangle, in terminal cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Size {
    width: usize,
    height: usize,
}

impl Size {
    /// The empty rectangle.
    pub const ZERO: Self = Self {
        width: 0,
        height: 0,
    };

    pub const fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }

    /// Returns whether the rectangle occupies no cells.
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// The area a view may occupy: an input to layout, not an afterthought.
///
/// A terminal width or a Ratatui `Rect` becomes an `Available`. It flows down
/// the tree and each node resolves its own size under it, so a frame closes at
/// whatever size the area forces. It is not a clip applied to a finished
/// rectangle; the only crop left is the degenerate-case safety net in
/// [`resolve`](super::resolve).
///
/// An absent bound is not zero: it means the axis is unbounded, which is what
/// `measure` resolves under and what makes an intrinsic size the same
/// computation as a bounded one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Available {
    width: Option<usize>,
    height: Option<usize>,
}

impl Available {
    /// Imposes no bound on either axis.
    pub const NONE: Self = Self {
        width: None,
        height: None,
    };

    pub const fn new(width: Option<usize>, height: Option<usize>) -> Self {
        Self { width, height }
    }

    /// Bounds the width only — a terminal of a known width and no known
    /// height.
    pub const fn columns(width: usize) -> Self {
        Self::new(Some(width), None)
    }

    /// Bounds both axes.
    pub const fn size(width: usize, height: usize) -> Self {
        Self::new(Some(width), Some(height))
    }

    pub const fn width(&self) -> Option<usize> {
        self.width
    }

    pub const fn height(&self) -> Option<usize> {
        self.height
    }
}
