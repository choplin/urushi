use std::any::Any;
use std::fmt;

/// The sizing policy of one Canvas.
///
/// [`CanvasSizing::viewport`] is the ordinary policy: a Canvas consumes a
/// finite parent allocation and needs an explicit extent on an unbounded axis.
/// Built-in presentations may internally construct an intrinsic policy whose
/// requirements participate in normal View layout without inspecting Canvas
/// items.
///
/// ```
/// use urushi::{CanvasSizing, Size};
///
/// let first = CanvasSizing::viewport().width(12).height(4);
/// let same = CanvasSizing::viewport().extent(Size::new(12, 4));
/// assert_eq!(first, same);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasSizing(CanvasSizingRepr);

impl CanvasSizing {
    /// Creates viewport sizing without fallback extents.
    pub const fn viewport() -> Self {
        Self(CanvasSizingRepr::Viewport(ViewportSizing {
            width: None,
            height: None,
        }))
    }

    /// States the extent viewport sizing uses when the width axis is unbounded.
    ///
    /// # Panics
    ///
    /// Panics when applied to the crate-private intrinsic variant. Replace the
    /// complete policy with [`Canvas::sizing`](super::Canvas::sizing) instead.
    #[must_use]
    pub const fn width(mut self, width: usize) -> Self {
        self.set_viewport_width(width);
        self
    }

    /// States the extent viewport sizing uses when the height axis is unbounded.
    ///
    /// # Panics
    ///
    /// Panics when applied to the crate-private intrinsic variant. Replace the
    /// complete policy with [`Canvas::sizing`](super::Canvas::sizing) instead.
    #[must_use]
    pub const fn height(mut self, height: usize) -> Self {
        self.set_viewport_height(height);
        self
    }

    /// States both fallback extents used by viewport sizing.
    ///
    /// # Panics
    ///
    /// Panics when applied to the crate-private intrinsic variant. Replace the
    /// complete policy with [`Canvas::sizing`](super::Canvas::sizing) instead.
    #[must_use]
    pub const fn extent(mut self, size: super::Size) -> Self {
        self.set_viewport_extent(size);
        self
    }

    pub(super) const fn set_viewport_width(&mut self, width: usize) {
        match &mut self.0 {
            CanvasSizingRepr::Viewport(viewport) => viewport.width = Some(width),
            CanvasSizingRepr::Intrinsic(_) => {
                panic!("viewport extents cannot modify intrinsic Canvas sizing")
            }
        }
    }

    pub(super) const fn set_viewport_height(&mut self, height: usize) {
        match &mut self.0 {
            CanvasSizingRepr::Viewport(viewport) => viewport.height = Some(height),
            CanvasSizingRepr::Intrinsic(_) => {
                panic!("viewport extents cannot modify intrinsic Canvas sizing")
            }
        }
    }

    pub(super) const fn set_viewport_extent(&mut self, size: super::Size) {
        match &mut self.0 {
            CanvasSizingRepr::Viewport(viewport) => {
                viewport.width = Some(size.width());
                viewport.height = Some(size.height());
            }
            CanvasSizingRepr::Intrinsic(_) => {
                panic!("viewport extents cannot modify intrinsic Canvas sizing")
            }
        }
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the next built-in Canvas presentation consumes this private constructor"
        )
    )]
    pub(crate) fn intrinsic<T>(measure: T) -> Self
    where
        T: CanvasMeasure + Clone + PartialEq,
    {
        Self(CanvasSizingRepr::Intrinsic(Measure(Box::new(measure))))
    }

    pub(super) fn width_requirements(&self) -> Requirements {
        match &self.0 {
            CanvasSizingRepr::Viewport(viewport) => {
                Requirements::new(viewport.width.unwrap_or(0), 0)
            }
            CanvasSizingRepr::Intrinsic(measure) => measure.0.width_requirements(),
        }
    }

    pub(super) fn height_requirements(&self, width: usize) -> Requirements {
        match &self.0 {
            CanvasSizingRepr::Viewport(viewport) => {
                Requirements::new(viewport.height.unwrap_or(0), 0)
            }
            CanvasSizingRepr::Intrinsic(measure) => measure.0.height_requirements(width),
        }
    }

    pub(super) const fn is_viewport(&self) -> bool {
        matches!(self.0, CanvasSizingRepr::Viewport(_))
    }

    pub(super) const fn explicit_width(&self) -> Option<usize> {
        match &self.0 {
            CanvasSizingRepr::Viewport(viewport) => viewport.width,
            CanvasSizingRepr::Intrinsic(_) => None,
        }
    }

    pub(super) const fn explicit_height(&self) -> Option<usize> {
        match &self.0 {
            CanvasSizingRepr::Viewport(viewport) => viewport.height,
            CanvasSizingRepr::Intrinsic(_) => None,
        }
    }
}

impl Default for CanvasSizing {
    fn default() -> Self {
        Self::viewport()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ViewportSizing {
    width: Option<usize>,
    height: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
enum CanvasSizingRepr {
    Viewport(ViewportSizing),
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the next built-in Canvas presentation constructs this private variant"
        )
    )]
    Intrinsic(Measure),
}

/// One checked demand and the smallest allocation that may satisfy it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CanvasRequirements {
    demand: usize,
    floor: usize,
}

impl CanvasRequirements {
    /// Creates a requirement, rejecting a floor greater than its demand.
    pub(crate) const fn new(demand: usize, floor: usize) -> Self {
        assert!(
            floor <= demand,
            "a Canvas sizing floor cannot exceed its demand"
        );
        Self { demand, floor }
    }

    pub(crate) const fn demand(self) -> usize {
        self.demand
    }

    pub(crate) const fn floor(self) -> usize {
        self.floor
    }
}

pub(super) type Requirements = CanvasRequirements;

/// Pure, value-comparable intrinsic measurement for a built-in presentation.
///
/// Width requirements are read before height requirements. The latter receives
/// the width selected by the parent. Implementations must not inspect or draw
/// Canvas items, and equal values must return equal requirements.
pub(crate) trait CanvasMeasure: fmt::Debug + Send + Sync + 'static {
    fn width_requirements(&self) -> CanvasRequirements;
    fn height_requirements(&self, width: usize) -> CanvasRequirements;
}

trait ErasedMeasure: fmt::Debug + Send + Sync {
    fn width_requirements(&self) -> CanvasRequirements;
    fn height_requirements(&self, width: usize) -> CanvasRequirements;
    fn clone_box(&self) -> Box<dyn ErasedMeasure>;
    fn equals(&self, other: &dyn ErasedMeasure) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T> ErasedMeasure for T
where
    T: CanvasMeasure + Clone + PartialEq,
{
    fn width_requirements(&self) -> CanvasRequirements {
        CanvasMeasure::width_requirements(self)
    }

    fn height_requirements(&self, width: usize) -> CanvasRequirements {
        CanvasMeasure::height_requirements(self, width)
    }

    fn clone_box(&self) -> Box<dyn ErasedMeasure> {
        Box::new(self.clone())
    }

    fn equals(&self, other: &dyn ErasedMeasure) -> bool {
        other.as_any().downcast_ref::<T>() == Some(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct Measure(Box<dyn ErasedMeasure>);

impl fmt::Debug for Measure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Clone for Measure {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl PartialEq for Measure {
    fn eq(&self, other: &Self) -> bool {
        self.0.equals(&*other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Fixed(usize);

    impl CanvasMeasure for Fixed {
        fn width_requirements(&self) -> CanvasRequirements {
            CanvasRequirements::new(self.0, self.0 / 2)
        }

        fn height_requirements(&self, width: usize) -> CanvasRequirements {
            CanvasRequirements::new(width, 0)
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct OtherFixed(usize);

    impl CanvasMeasure for OtherFixed {
        fn width_requirements(&self) -> CanvasRequirements {
            CanvasRequirements::new(self.0, 0)
        }

        fn height_requirements(&self, width: usize) -> CanvasRequirements {
            CanvasRequirements::new(width, 0)
        }
    }

    #[test]
    fn sizing_equality_uses_variant_concrete_type_and_value() {
        assert_eq!(
            CanvasSizing::viewport().width(4).height(2),
            CanvasSizing::viewport().extent(crate::Size::new(4, 2))
        );
        assert_eq!(
            CanvasSizing::intrinsic(Fixed(8)),
            CanvasSizing::intrinsic(Fixed(8))
        );
        assert_ne!(
            CanvasSizing::intrinsic(Fixed(8)),
            CanvasSizing::intrinsic(Fixed(9))
        );
        assert_ne!(
            CanvasSizing::intrinsic(Fixed(8)),
            CanvasSizing::intrinsic(OtherFixed(8))
        );
        assert_ne!(CanvasSizing::viewport(), CanvasSizing::intrinsic(Fixed(0)));
    }

    #[test]
    fn zero_requirements_are_valid() {
        assert_eq!(
            CanvasRequirements::new(0, 0),
            CanvasRequirements {
                demand: 0,
                floor: 0
            }
        );
    }

    #[test]
    #[should_panic(expected = "floor cannot exceed")]
    fn an_invalid_requirement_is_rejected_at_construction() {
        let _ = CanvasRequirements::new(1, 2);
    }
}
