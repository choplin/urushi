//! Identity for the things an application names.

use std::any::TypeId;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};

/// The identity of something an application names, built from any hashable
/// value.
///
/// A key carries the value's type as well as its hash, so keys built from
/// equal values of the same type are equal and keys of different types never
/// are. An application's own enums are therefore its natural keys, and one
/// name cannot collide with another concern's.
///
/// ```
/// use urushi::Key;
///
/// #[derive(Hash)]
/// enum Pane {
///     Preview,
///     Chart,
/// }
///
/// assert_eq!(Key::of(&Pane::Preview), Key::of(&Pane::Preview));
/// assert_ne!(Key::of(&Pane::Preview), Key::of(&Pane::Chart));
/// assert_eq!(Key::from("preview"), Key::of(&"preview"));
/// assert_eq!(format!("{:?}", Key::from("preview")), "Key(preview)");
/// ```
///
/// The value itself is not kept: a key is a type and a hash, which is what
/// lets it name anything without owning it. Two distinct values of one type
/// whose hashes collide are therefore one key, as they are in any hash map.
///
/// What it does keep is a label to print, and only one that costs nothing to
/// hold: the literal for a key built from one, and the type's name otherwise.
/// A key stays `Copy`, and a log line says which key rather than which hash.
#[derive(Clone, Copy, Eq)]
pub struct Key {
    type_id: TypeId,
    hash: u64,
    /// What to call this key in a log. Not part of its identity: two keys of
    /// one value are one key however each was built.
    label: &'static str,
}

impl Key {
    /// Builds the key of `value`.
    pub fn of<T: Hash + 'static>(value: &T) -> Self {
        Self::new(TypeId::of::<T>(), value, std::any::type_name::<T>())
    }

    fn new<T: Hash + ?Sized>(type_id: TypeId, value: &T, label: &'static str) -> Self {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        Self {
            type_id,
            hash: hasher.finish(),
            label,
        }
    }
}

impl From<&'static str> for Key {
    fn from(name: &'static str) -> Self {
        Self::new(TypeId::of::<&'static str>(), &name, name)
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id && self.hash == other.hash
    }
}

impl Hash for Key {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.type_id.hash(hasher);
        self.hash.hash(hasher);
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Key({})", self.label)
    }
}
