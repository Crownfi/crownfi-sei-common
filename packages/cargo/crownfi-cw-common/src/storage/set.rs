use cosmwasm_std::StdResult;

use super::{
	map::{StoredMap, StoredMapKeyIter},
	SerializableItem,
};

/// Represents a set a values.
///
/// At the time of writing, the cosmwasm storage backend can't consistently differentiate
/// between empty and non-existant values, so this is actually a StoredMap<V, u8> under the hood.
#[repr(transparent)]
pub struct StoredSet<V: SerializableItem> {
	inner_map: StoredMap<V, u8>,
}

impl<'exec, V: SerializableItem> StoredSet<V> {
	#[inline]
	pub fn new(namespace: &'static [u8]) -> Self {
		Self {
			inner_map: StoredMap::new(namespace),
		}
	}
	#[inline]
	pub fn has(&self, value: &V) -> bool {
		self.inner_map.has(value)
	}
	#[inline]
	pub fn add(&self, value: &V) -> StdResult<()> {
		self.inner_map.set(value, &254) // A completely arbitrary choice by Snow
	}

	pub fn remove(&self, value: &V) {
		self.inner_map.remove(value)
	}

	/// Returns an iterator which iterates over all set values
	///
	/// By default it iterates in an ascending order. Though is a double-ended iterator, so you can use the `.rev()`
	/// method to switch to descending order.
	pub fn iter(&self) -> StdResult<StoredMapKeyIter<V>> {
		self.inner_map.iter_keys()
	}

	/// Returns an iterator which iterates over all set values over a specified range
	///
	/// You can use `after` to skip items while in ascending order. Or `before` along with the `.rev()` method to skip
	/// items while iterating in a descending order.
	pub fn iter_range(&self, after: Option<V>, before: Option<V>) -> StdResult<StoredMapKeyIter<V>> {
		self.inner_map.iter_range_keys(after, before)
	}
}

// Depends on set which is proven to work
