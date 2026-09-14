//! Text modifiers that can be added to or removed from a logical style.

use std::ops::BitOr;

/// A set of terminal text modifiers.
///
/// Modifiers are stored as a bitset so several flags can be passed to
/// [`TextStyle::add`](crate::TextStyle::add) or [`TextStyle::remove`](crate::TextStyle::remove)
/// at once.
///
/// The underline is not among them: it carries a shape and a color, so it is
/// the [`Underline`](crate::Underline) value on the style. A flag beside that
/// value would give a single underline two spellings, so it does not exist:
///
/// ```compile_fail
/// use urushi::Modifier;
///
/// let _ = Modifier::UNDERLINED;
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifier(u16);

impl Modifier {
    pub const BOLD: Self = Self(1 << 0);
    pub const DIM: Self = Self(1 << 1);
    pub const ITALIC: Self = Self(1 << 2);
    pub const SLOW_BLINK: Self = Self(1 << 3);
    pub const REVERSED: Self = Self(1 << 4);
    pub const HIDDEN: Self = Self(1 << 5);
    pub const CROSSED_OUT: Self = Self(1 << 6);

    const ALL_BITS: u16 = Self::BOLD.0
        | Self::DIM.0
        | Self::ITALIC.0
        | Self::SLOW_BLINK.0
        | Self::REVERSED.0
        | Self::HIDDEN.0
        | Self::CROSSED_OUT.0;

    /// Returns a set containing no modifiers.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Returns a set containing every modifier supported by Urushi.
    pub const fn all() -> Self {
        Self(Self::ALL_BITS)
    }

    /// Returns the union of two modifier sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns the modifiers in `self` that are absent from `other`.
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & (Self::ALL_BITS ^ other.0))
    }

    /// Returns the modifiers present in both sets.
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Returns whether every modifier in `other` is present in this set.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns whether this set contains no modifiers.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for Modifier {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_operations_compose_supported_flags() {
        let emphasis = Modifier::BOLD | Modifier::ITALIC;

        assert!(emphasis.contains(Modifier::BOLD));
        assert!(emphasis.contains(Modifier::ITALIC));
        assert!(!emphasis.contains(Modifier::DIM));
        assert_eq!(emphasis.difference(Modifier::ITALIC), Modifier::BOLD);
        assert!(Modifier::empty().is_empty());
        assert!(Modifier::all().contains(emphasis));
    }
}
