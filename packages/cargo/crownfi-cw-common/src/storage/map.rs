use cosmwasm_std::StdResult;
use std::{marker::PhantomData, num::NonZeroUsize};

use crate::utils::lexicographic_next;

use super::{
	base::{storage_has, storage_read, storage_read_item, storage_remove, storage_write, storage_write_item},
	concat_byte_array_pairs,
	item::AutosavingSerializableItem,
	OZeroCopy, SerializableItem, StorageKeyIterator, StoragePairIterator,
};
pub struct StoredMap<K: SerializableItem, V: SerializableItem> {
	namespace: &'static [u8],
	key_type: PhantomData<K>,
	value_type: PhantomData<V>,
}

impl<'exec, K: SerializableItem, V: SerializableItem> StoredMap<K, V> {
	pub fn new(namespace: &'static [u8]) -> Self {
		Self {
			namespace,
			key_type: PhantomData,
			value_type: PhantomData,
		}
	}

	#[inline]
	pub fn key(&self, key: &K) -> Vec<u8> {
		if let Some(key_bytes) = key.serialize_as_ref() {
			concat_byte_array_pairs(self.namespace, key_bytes)
		} else {
			concat_byte_array_pairs(
				self.namespace,
				&key.serialize_as_ref().unwrap_or(
					key.serialize_to_owned()
						.expect("key serialization should never fail")
						.as_ref(),
				),
			)
		}
	}

	#[inline]
	pub fn get_raw_bytes(&self, key: &K) -> Option<Vec<u8>> {
		storage_read(&self.key(key))
	}

	#[inline]
	pub(crate) fn set_raw_bytes(&self, key: &K, bytes: &[u8]) {
		storage_write(&self.key(key), bytes)
	}

	pub fn get(&self, key: &K) -> StdResult<Option<OZeroCopy<V>>> {
		storage_read_item(&self.key(key))
	}

	pub fn get_autosaving(&self, key: &K) -> StdResult<Option<AutosavingSerializableItem<V>>> {
		AutosavingSerializableItem::new(self.key(key))
	}

	pub fn get_or_default_autosaving(&self, key: &K) -> StdResult<AutosavingSerializableItem<V>>
	where
		V: Default,
	{
		AutosavingSerializableItem::new_or_default(self.key(key))
	}

	/// At the time of writing, the cosmwasm API cannot actually facilitate this, you should probably match on get()
	pub fn has(&self, key: &K) -> bool {
		storage_has(&self.key(key))
	}

	pub fn set(&self, key: &K, value: &V) -> StdResult<()> {
		storage_write_item(&self.key(key), value)
	}

	pub fn remove(&self, key: &K) {
		storage_remove(&self.key(key))
	}

	/// Returns an iterator which iterates over all key/value pairs of the map
	///
	/// By default it iterates in an ascending order. Though is a double-ended iterator, so you can use the `.rev()`
	/// method to switch to descending order.
	pub fn iter(&self) -> StdResult<StoredMapIter<K, V>> {
		StoredMapIter::new(self.namespace, (), None, None)
	}

	/// Returns an iterator over a range of keys.
	///
	/// You can use `after` to skip items while in ascending order. Or `before` along with the `.rev()` method to skip
	/// items while iterating in a descending order.
	pub fn iter_range(&self, after: Option<K>, before: Option<K>) -> StdResult<StoredMapIter<K, V>> {
		StoredMapIter::new(self.namespace, (), after, before)
	}

	/// Returns an iterator which iterates over all keys of the map
	///
	/// By default it iterates in an ascending order. Though is a double-ended iterator, so you can use the `.rev()`
	/// method to switch to descending order.
	pub fn iter_keys(&self) -> StdResult<StoredMapKeyIter<K>> {
		StoredMapKeyIter::new(self.namespace, (), None, None)
	}

	/// Returns an iterator over a range of keys.
	///
	/// You can use `after` to skip items while in ascending order. Or `before` along with the `.rev()` method to skip
	/// items while iterating in a descending order.
	pub fn iter_range_keys(&self, after: Option<K>, before: Option<K>) -> StdResult<StoredMapKeyIter<K>> {
		StoredMapKeyIter::new(self.namespace, (), after, before)
	}
}

/// Allows you to iterate over a stored map.
///
/// If your key type for your stored map is a tuple, i.e. `(T1, T2, T3)`, you can set `K` to `(T2, T3)` while providing
/// `T1` as the `partial_key` in the `new()` function.
///
/// If you don't care about the keys or values and don't want to parse them, set it to the unit type `()`.
pub struct StoredMapIter<K: SerializableItem, V: SerializableItem> {
	inner_iter: StoragePairIterator,
	key_slicing: usize,
	key_type: PhantomData<K>,
	value_type: PhantomData<V>,
}

impl<'a, K: SerializableItem, V: SerializableItem> StoredMapIter<K, V> {
	/// Note that start_key and end_key are both exclusive, i.e. this key, if it exists, will be skipped
	pub fn new<P>(namespace: &[u8], key_prefix: P, start_key: Option<K>, end_key: Option<K>) -> StdResult<Self>
	where
		P: SerializableItem,
	{
		let (start_key, end_key, full_prefix_bytes_len) =
			prefixed_key_range_to_byte_prefixes(namespace, key_prefix, start_key, end_key)?;
		Ok(Self {
			inner_iter: StoragePairIterator::new(Some(&start_key), Some(&end_key)),
			key_slicing: full_prefix_bytes_len,
			key_type: PhantomData,
			value_type: PhantomData,
		})
	}
	fn advance_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
		self.inner_iter.0.advance_by(n)
	}
	fn advance_back_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
		self.inner_iter.0.advance_back_by(n)
	}
}
impl<'a, K: SerializableItem, V: SerializableItem> Iterator for StoredMapIter<K, V> {
	type Item = (K, OZeroCopy<V>);
	fn next(&mut self) -> Option<Self::Item> {
		self.inner_iter.next().and_then(|(key_bytes, value_bytes)| {
			Some((
				K::deserialize_to_owned(&key_bytes[self.key_slicing..]).ok()?,
				OZeroCopy::new(value_bytes).ok()?,
			))
		})
	}
	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.advance_by(n).ok()?;
		self.next()
	}
	// TODO: impl advance_by when stable
}
impl<'a, K: SerializableItem, V: SerializableItem> DoubleEndedIterator for StoredMapIter<K, V> {
	fn next_back(&mut self) -> Option<Self::Item> {
		self.inner_iter.next_back().and_then(|(key_bytes, value_bytes)| {
			Some((
				K::deserialize_to_owned(&key_bytes[self.key_slicing..]).ok()?,
				OZeroCopy::new(value_bytes).ok()?,
			))
		})
	}
	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		self.advance_back_by(n).ok()?;
		self.next()
	}
	// TODO: impl advance_by when stable
}

/// Allows you to iterate the keys over a stored map.
///
/// If your key type for your stored map is a tuple, i.e. `(T1, T2, T3)`, you can set `K` to `(T2, T3)` while providing
/// `T1` as the `partial_key` in the `new()` function.
pub struct StoredMapKeyIter<K: SerializableItem> {
	inner_iter: StorageKeyIterator,
	key_slicing: usize,
	key_type: PhantomData<K>,
}

impl<'a, K: SerializableItem> StoredMapKeyIter<K> {
	/// Note that start_key and end_key are both exclusive, i.e. this key, if it exists, will be skipped
	pub fn new<P>(namespace: &[u8], key_prefix: P, start_key: Option<K>, end_key: Option<K>) -> StdResult<Self>
	where
		P: SerializableItem,
	{
		let (start_key, end_key, full_prefix_bytes_len) =
			prefixed_key_range_to_byte_prefixes(namespace, key_prefix, start_key, end_key)?;
		Ok(Self {
			inner_iter: StorageKeyIterator::new(Some(&start_key), Some(&end_key)),
			key_slicing: full_prefix_bytes_len,
			key_type: PhantomData,
		})
	}
	fn advance_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
		self.inner_iter.0.advance_by(n)
	}
	fn advance_back_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
		self.inner_iter.0.advance_back_by(n)
	}
}
impl<'a, K: SerializableItem> Iterator for StoredMapKeyIter<K> {
	type Item = K;
	fn next(&mut self) -> Option<Self::Item> {
		self.inner_iter
			.next()
			.and_then(|key_bytes| Some(K::deserialize_to_owned(&key_bytes[self.key_slicing..]).ok()?))
	}
	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.advance_by(n).ok()?;
		self.next()
	}
	// TODO: impl advance_by when stable
}
impl<'a, K: SerializableItem> DoubleEndedIterator for StoredMapKeyIter<K> {
	fn next_back(&mut self) -> Option<Self::Item> {
		self.inner_iter
			.next_back()
			.and_then(|key_bytes| Some(K::deserialize_to_owned(&key_bytes[self.key_slicing..]).ok()?))
	}
	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		self.advance_back_by(n).ok()?;
		self.next()
	}
	// TODO: impl advance_by when stable
}

fn prefixed_key_range_to_byte_prefixes<P, K>(
	namespace: &[u8],
	key_prefix: P,
	start_key: Option<K>,
	end_key: Option<K>,
) -> StdResult<(Vec<u8>, Vec<u8>, usize)>
where
	K: SerializableItem,
	P: SerializableItem,
{
	let prefix_bytes = key_prefix.serialize_to_owned()?;
	let start_bytes = start_key.map_or(Ok(Vec::new()), |k| {
		k.serialize_to_owned().map(|maybe_vec| maybe_vec.into())
	})?;
	let end_bytes = end_key.map_or(Ok(Vec::new()), |k| {
		k.serialize_to_owned().map(|maybe_vec| maybe_vec.into())
	})?;

	let mut start_key = Vec::with_capacity(namespace.len() + prefix_bytes.len() + start_bytes.len());
	start_key.extend_from_slice(namespace);
	start_key.extend_from_slice(prefix_bytes.as_ref());
	start_key.extend_from_slice(start_bytes.as_ref());

	let end_key = if end_bytes.len() == 0 {
		lexicographic_next(&concat_byte_array_pairs(&namespace, &prefix_bytes))
	} else {
		let mut end_key = Vec::with_capacity(namespace.len() + prefix_bytes.len() + end_bytes.len());
		end_key.extend_from_slice(namespace);
		end_key.extend_from_slice(prefix_bytes.as_ref());
		end_key.extend_from_slice(end_bytes.as_ref());
		end_key
	};
	Ok((start_key, end_key, namespace.len() + prefix_bytes.len()))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::storage::testing_common::*;

	#[test]
	fn stored_empty_map_iter() {
		let _storage_lock = init().unwrap();
		let stored_map = StoredMap::<String, String>::new(b"namespace");
		let mut stored_map_iter = stored_map.iter().unwrap();
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			None
		);
		storage_write(b"unrelated", b"ayy lmao");

		stored_map_iter = stored_map.iter().unwrap();
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			None
		);
	}

	#[test]
	fn stored_map_iter() {
		let _storage_lock = init().unwrap();

		let stored_map = StoredMap::<String, String>::new(b"namespace");
		stored_map.set(&"key1".to_string(), &"val1".to_string()).unwrap();
		assert_eq!(
			stored_map
				.get(&"key1".to_string())
				.map(|result| { result.map(|thing| { thing.into_inner() }) }),
			Ok(Some("val1".into()))
		);
		assert_eq!(
			stored_map
				.iter()
				.unwrap()
				.next()
				.map(|(key, value)| { (key, value.into_inner()) }),
			Some(("key1".into(), "val1".into()))
		);
		stored_map.set(&"key2".to_string(), &"val2".to_string()).unwrap();

		let mut stored_map_iter = stored_map.iter().unwrap();
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			Some(("key1".into(), "val1".into()))
		);
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			Some(("key2".into(), "val2".into()))
		);
		assert!(stored_map_iter.next().is_none());

		stored_map.set(&"key3".to_string(), &"val3".to_string()).unwrap();

		let mut stored_map_iter = stored_map.iter().unwrap().rev();
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			Some(("key3".into(), "val3".into()))
		);
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			Some(("key2".into(), "val2".into()))
		);
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			Some(("key1".into(), "val1".into()))
		);
		assert_eq!(stored_map_iter.next(), None);

		let mut stored_map_iter = stored_map.iter_range(Some("key".into()), Some("key3".into())).unwrap();
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			Some(("key1".into(), "val1".into()))
		);
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			Some(("key2".into(), "val2".into()))
		);
		assert_eq!(stored_map_iter.next(), None);

		// Note: when it comes to iter_range, the "start" position is inclusive, while the "end" is exclusive
		let mut stored_map_iter = stored_map.iter_range(Some("key2".into()), None).unwrap();
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			Some(("key2".into(), "val2".into()))
		);
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			Some(("key3".into(), "val3".into()))
		);
		assert_eq!(stored_map_iter.next(), None);

		// Note: when it comes to iter_range, the "start" position is inclusive, while the "end" is exclusive
		let mut stored_map_iter = stored_map
			.iter_range(Some("key1".into()), Some("key3".into()))
			.unwrap()
			.rev();
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			Some(("key2".into(), "val2".into()))
		);
		assert_eq!(
			stored_map_iter.next().map(|(key, value)| { (key, value.into_inner()) }),
			Some(("key1".into(), "val1".into()))
		);
		assert_eq!(stored_map_iter.next(), None);
	}

	#[test]
	fn stored_map_key_iter() {
		let _storage_lock = init().unwrap();

		let stored_map = StoredMap::<String, String>::new(b"namespace");
		stored_map.set(&"key1".to_string(), &"val1".to_string()).unwrap();
		assert_eq!(
			stored_map
				.get(&"key1".to_string())
				.map(|result| { result.map(|thing| { thing.into_inner() }) }),
			Ok(Some("val1".into()))
		);
		assert_eq!(stored_map.iter_keys().unwrap().next(), Some("key1".into()));
		stored_map.set(&"key2".to_string(), &"val2".to_string()).unwrap();

		let mut stored_map_iter = stored_map.iter_keys().unwrap();
		assert_eq!(stored_map_iter.next(), Some("key1".into()));
		assert_eq!(stored_map_iter.next(), Some("key2".into()));
		assert!(stored_map_iter.next().is_none());

		stored_map.set(&"key3".to_string(), &"val3".to_string()).unwrap();

		let mut stored_map_iter = stored_map.iter_keys().unwrap().rev();
		assert_eq!(stored_map_iter.next(), Some("key3".into()));
		assert_eq!(stored_map_iter.next(), Some("key2".into()));
		assert_eq!(stored_map_iter.next(), Some("key1".into()));
		assert_eq!(stored_map_iter.next(), None);

		let mut stored_map_iter = stored_map
			.iter_range_keys(Some("key".into()), Some("key3".into()))
			.unwrap();
		assert_eq!(stored_map_iter.next(), Some("key1".into()));
		assert_eq!(stored_map_iter.next(), Some("key2".into()));
		assert_eq!(stored_map_iter.next(), None);

		// Note: when it comes to iter_range_keys, the "start" position is inclusive, while the "end" is exclusive
		let mut stored_map_iter = stored_map.iter_range_keys(Some("key2".into()), None).unwrap();
		assert_eq!(stored_map_iter.next(), Some("key2".into()));
		assert_eq!(stored_map_iter.next(), Some("key3".into()));
		assert_eq!(stored_map_iter.next(), None);

		// Note: when it comes to iter_range_keys, the "start" position is inclusive, while the "end" is exclusive
		let mut stored_map_iter = stored_map
			.iter_range_keys(Some("key1".into()), Some("key3".into()))
			.unwrap()
			.rev();
		assert_eq!(stored_map_iter.next(), Some("key2".into()));
		assert_eq!(stored_map_iter.next(), Some("key1".into()));
		assert_eq!(stored_map_iter.next(), None);
	}

	#[test]
	fn basic() -> TestingResult {
		let _storage_lock = init()?;
		let stored_map = StoredMap::<String, String>::new(NAMESPACE);

		let key = String::from("key1");
		let value = String::from("val1");

		stored_map.set(&key, &value).unwrap();
		assert!(stored_map.has(&key));
		assert_eq!(stored_map.get(&"banana".to_string()).unwrap(), None);
		assert_eq!(stored_map.get(&key).unwrap(), Some(OZeroCopy::from_inner(value)));

		Ok(())
	}

	#[test]
	fn raw() -> TestingResult {
		let _storage_lock = init()?;
		let stored_map = StoredMap::<[u8; 4], [u8; 4]>::new(NAMESPACE);

		let key = b"key1";
		let value = b"val1";

		stored_map.set_raw_bytes(key, value);
		assert_eq!(
			stored_map.get(key).unwrap(),
			Some(OZeroCopy::from_inner(value.to_owned()))
		);
		assert_eq!(stored_map.get_raw_bytes(key), Some(b"val1".to_vec()));

		Ok(())
	}

	#[test]
	fn autosaving() -> TestingResult {
		let _storage_lock = init()?;
		let stored_map = StoredMap::<String, String>::new(NAMESPACE);

		let key = String::from("key1");
		let fake_key = String::from("banana");
		let value = String::from("val1");

		stored_map.set(&key, &value).unwrap();
		let mut v1 = stored_map.get_autosaving(&key).unwrap().unwrap();
		let v2 = stored_map.get_or_default_autosaving(&fake_key).unwrap();

		assert_eq!(*v1, value);
		assert_eq!(*v2, "");

		*v1 = String::from("banana2");

		drop(v1);
		drop(v2);

		assert!(storage_has(&stored_map.key(&key)));
		// XXX: expected behavior?
		assert!(storage_has(&stored_map.key(&fake_key)));

		let v1 = stored_map.get(&key).unwrap().unwrap();
		assert_eq!(*v1, String::from("banana2"));

		Ok(())
	}
}
