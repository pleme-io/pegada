//! The set algebra every footprint stands on: a list that cannot be empty, the
//! [`Member`] contract, and set inclusion written once.

use std::collections::BTreeSet;
use std::fmt::Display;

/// A thing a footprint can own.
///
/// `Ord` is identity: two members that compare equal are one member. `Display`
/// must be injective (two distinct members never display alike), because the
/// [`crate::Reaped`] content address is computed over the displayed form.
pub trait Member: Ord + Clone + Display {}

/// A list with at least one element. There is no empty value to construct.
///
/// ```compile_fail
/// let _ = pegada::NonEmpty::<u8>(Vec::new());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonEmpty<T>(Vec<T>);

impl<T> NonEmpty<T> {
    /// One head and any tail.
    #[must_use]
    pub fn new(head: T, tail: impl IntoIterator<Item = T>) -> Self {
        let mut items = vec![head];
        items.extend(tail);
        Self(items)
    }

    /// `None` when `items` is empty.
    #[must_use]
    pub fn from_vec(items: Vec<T>) -> Option<Self> {
        if items.is_empty() {
            None
        } else {
            Some(Self(items))
        }
    }

    /// The first element, which always exists.
    #[must_use]
    pub fn first(&self) -> &T {
        &self.0[0]
    }

    /// The elements, in order.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// The number of elements, never zero.
    #[must_use]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// The elements, in order.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }

    /// The elements, in order.
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}

impl<'a, T> IntoIterator for &'a NonEmpty<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Every element of `expected` that `actual` lacks, in `expected`'s order.
///
/// The one set inclusion every gate uses: forward to find what a capture
/// missed, backward to find what a restore or a reap left standing.
#[must_use]
pub fn missing_from<'a, M: Ord>(expected: &'a [M], actual: &[M]) -> Vec<&'a M> {
    let present: BTreeSet<&M> = actual.iter().collect();
    expected.iter().filter(|m| !present.contains(m)).collect()
}
