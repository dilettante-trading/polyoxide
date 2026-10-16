//! Facts only one venue has, carried on a shared record without a field of
//! their own.

use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{Class, Classify};

/// The object-safe face of an extension value.
trait Extension: Any + Send + Sync {
    fn clone_box(&self) -> Box<dyn Extension>;
    fn eq_dyn(&self, other: &dyn Extension) -> bool;
    fn fmt_dyn(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<T> Extension for T
where
    T: Clone + fmt::Debug + PartialEq + Send + Sync + 'static,
{
    fn clone_box(&self) -> Box<dyn Extension> {
        Box::new(self.clone())
    }

    fn eq_dyn(&self, other: &dyn Extension) -> bool {
        other
            .as_any()
            .downcast_ref::<T>()
            .is_some_and(|other| self == other)
    }

    fn fmt_dyn(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

/// Facts only one venue has, carried on a shared record: at most one value of
/// each type, read back by its type.
///
/// Each product defines one `<Record>Ext` type per record it fills, such as a
/// quote's update id, and inserts one value of it. A value's type is
/// `Clone + Debug + PartialEq + Send + Sync + 'static`, so a record that
/// carries extensions stays `Clone`, `Debug` and `PartialEq`, and two records
/// are equal only when their extensions hold equal values of the same types.
/// There is no serde support: a value's type is known only to the crate that
/// defines it.
///
/// ```
/// use polyoxide_venue::Extensions;
///
/// #[derive(Debug, Clone, PartialEq)]
/// struct QuoteExt {
///     update_id: u64,
/// }
///
/// let mut ext = Extensions::new();
/// ext.insert(QuoteExt { update_id: 7 })?;
/// assert_eq!(ext.get::<QuoteExt>(), Some(&QuoteExt { update_id: 7 }));
/// assert!(ext.insert(QuoteExt { update_id: 8 }).is_err(), "one value per type");
/// assert_ne!(ext, Extensions::of(QuoteExt { update_id: 8 }));
/// # Ok::<(), polyoxide_venue::DuplicateExtension>(())
/// ```
///
/// A value whose type cannot be compared is refused when the program is
/// built:
///
/// ```compile_fail
/// use polyoxide_venue::Extensions;
///
/// #[derive(Debug, Clone)]
/// struct Uncomparable;
///
/// let mut ext = Extensions::new();
/// ext.insert(Uncomparable).unwrap();
/// ```
#[derive(Default)]
pub struct Extensions {
    values: BTreeMap<TypeId, Box<dyn Extension>>,
}

impl Extensions {
    /// No extensions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Extensions holding `value` alone.
    pub fn of<T>(value: T) -> Self
    where
        T: Clone + fmt::Debug + PartialEq + Send + Sync + 'static,
    {
        let mut ext = Self::new();
        ext.values.insert(TypeId::of::<T>(), Box::new(value));
        ext
    }

    /// Adds `value`.
    ///
    /// # Errors
    ///
    /// [`DuplicateExtension`] when a value of `T` is already held, which is
    /// kept unchanged.
    pub fn insert<T>(&mut self, value: T) -> Result<(), DuplicateExtension>
    where
        T: Clone + fmt::Debug + PartialEq + Send + Sync + 'static,
    {
        let id = TypeId::of::<T>();
        if self.values.contains_key(&id) {
            return Err(DuplicateExtension {
                type_name: std::any::type_name::<T>(),
            });
        }
        self.values.insert(id, Box::new(value));
        Ok(())
    }

    /// The value of `T`, when one is held.
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|value| value.as_ref().as_any().downcast_ref())
    }

    /// The value of `T`, to change in place, when one is held.
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.values
            .get_mut(&TypeId::of::<T>())
            .and_then(|value| value.as_mut().as_any_mut().downcast_mut())
    }

    /// Takes the value of `T` out, when one is held.
    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.values
            .remove(&TypeId::of::<T>())
            .and_then(|value| value.into_any().downcast().ok())
            .map(|value: Box<T>| *value)
    }

    /// Whether a value of `T` is held.
    pub fn contains<T: 'static>(&self) -> bool {
        self.values.contains_key(&TypeId::of::<T>())
    }

    /// The types of the values held, one each.
    pub fn type_ids(&self) -> impl Iterator<Item = TypeId> + '_ {
        self.values.keys().copied()
    }

    /// How many values are held.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether no value is held.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl Clone for Extensions {
    fn clone(&self) -> Self {
        Self {
            values: self
                .values
                .iter()
                .map(|(id, value)| (*id, value.as_ref().clone_box()))
                .collect(),
        }
    }
}

impl PartialEq for Extensions {
    fn eq(&self, other: &Self) -> bool {
        self.values.len() == other.values.len()
            && self.values.iter().all(|(id, value)| {
                other
                    .values
                    .get(id)
                    .is_some_and(|theirs| value.as_ref().eq_dyn(theirs.as_ref()))
            })
    }
}

impl fmt::Debug for Extensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Value<'a>(&'a dyn Extension);

        impl fmt::Debug for Value<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt_dyn(f)
            }
        }

        f.debug_set()
            .entries(self.values.values().map(|value| Value(value.as_ref())))
            .finish()
    }
}

/// A value was inserted into [`Extensions`] that already held one of its type.
/// An [`InvalidRequest`](Class::InvalidRequest): the caller's misuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateExtension {
    /// The type both values have.
    pub type_name: &'static str,
}

impl fmt::Display for DuplicateExtension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the record already holds an extension of type {}",
            self.type_name
        )
    }
}

impl Error for DuplicateExtension {}

impl Classify for DuplicateExtension {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct QuoteExt {
        update_id: u64,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct BookExt {
        sequence: u64,
    }

    #[test]
    fn extensions_are_clone_debug_default_partial_eq_send_and_sync() {
        fn holds<T: Clone + fmt::Debug + Default + PartialEq + Send + Sync>() {}
        holds::<Extensions>();
    }

    #[test]
    fn a_value_is_read_back_by_its_type() {
        let mut ext = Extensions::new();
        assert!(ext.is_empty());
        ext.insert(QuoteExt { update_id: 7 }).unwrap();
        ext.insert(BookExt { sequence: 9 }).unwrap();
        assert_eq!(ext.len(), 2);
        assert_eq!(ext.get::<QuoteExt>(), Some(&QuoteExt { update_id: 7 }));
        assert_eq!(ext.get::<BookExt>(), Some(&BookExt { sequence: 9 }));
        assert_eq!(ext.get::<u64>(), None);
        assert!(ext.contains::<QuoteExt>());
        assert!(!ext.contains::<String>());
    }

    #[test]
    fn inserting_a_type_already_held_is_an_error_and_keeps_the_first() {
        let mut ext = Extensions::of(QuoteExt { update_id: 7 });
        let err = ext.insert(QuoteExt { update_id: 8 }).unwrap_err();
        assert!(err.type_name.ends_with("QuoteExt"), "{err}");
        assert_eq!(err.class(), Class::InvalidRequest);
        assert_eq!(ext.get::<QuoteExt>(), Some(&QuoteExt { update_id: 7 }));
        assert_eq!(ext.len(), 1);
    }

    #[test]
    fn equality_compares_the_values_not_just_the_types() {
        let a = Extensions::of(QuoteExt { update_id: 7 });
        assert_eq!(a, Extensions::of(QuoteExt { update_id: 7 }));
        assert_ne!(a, Extensions::of(QuoteExt { update_id: 8 }));
        assert_ne!(a, Extensions::of(BookExt { sequence: 7 }));
        assert_ne!(a, Extensions::new());
        assert_ne!(Extensions::new(), a);
        let mut both = a.clone();
        both.insert(BookExt { sequence: 1 }).unwrap();
        assert_ne!(a, both);
        assert_ne!(both, a);
        assert_eq!(Extensions::new(), Extensions::default());
    }

    #[test]
    fn a_clone_holds_its_own_copies() {
        let original = Extensions::of(QuoteExt { update_id: 7 });
        let mut copy = original.clone();
        assert_eq!(copy, original);
        copy.get_mut::<QuoteExt>().unwrap().update_id = 8;
        assert_eq!(original.get::<QuoteExt>(), Some(&QuoteExt { update_id: 7 }));
        assert_ne!(copy, original);
    }

    #[test]
    fn the_type_ids_held_are_exposed() {
        let mut ext = Extensions::of(QuoteExt { update_id: 7 });
        ext.insert(BookExt { sequence: 9 }).unwrap();
        let mut ids: Vec<TypeId> = ext.type_ids().collect();
        let mut expected = vec![TypeId::of::<QuoteExt>(), TypeId::of::<BookExt>()];
        ids.sort();
        expected.sort();
        assert_eq!(ids, expected);
    }

    #[test]
    fn a_removed_value_comes_back_whole() {
        let mut ext = Extensions::of(QuoteExt { update_id: 7 });
        assert_eq!(ext.remove::<QuoteExt>(), Some(QuoteExt { update_id: 7 }));
        assert_eq!(ext.remove::<QuoteExt>(), None);
        assert!(ext.is_empty());
        ext.insert(QuoteExt { update_id: 8 }).unwrap();
        assert_eq!(ext.get::<QuoteExt>(), Some(&QuoteExt { update_id: 8 }));
    }

    #[test]
    fn debug_shows_each_value() {
        let ext = Extensions::of(QuoteExt { update_id: 7 });
        assert_eq!(format!("{ext:?}"), "{QuoteExt { update_id: 7 }}");
        assert_eq!(format!("{:?}", Extensions::new()), "{}");
    }
}
