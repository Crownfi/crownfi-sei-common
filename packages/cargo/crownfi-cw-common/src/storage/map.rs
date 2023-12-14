use std::marker::PhantomData;
use cosmwasm_std::StdResult;

use super::{SerializableItem, MaybeMutableStorage, concat_byte_array_pairs, item::AutosavingSerializableItem};
pub struct StoredMap<'exec, K: SerializableItem, V: SerializableItem> {
	namespace: &'static [u8],
	storage: MaybeMutableStorage<'exec>,
	key_type: PhantomData<K>,
	value_type: PhantomData<V>
}

impl<'exec, K: SerializableItem, V: SerializableItem> StoredMap<'exec, K, V> {
	pub fn new(namespace: &'static [u8], storage: MaybeMutableStorage<'exec>) -> Self {
		Self {
			namespace,
			storage,
			key_type: PhantomData,
			value_type: PhantomData
		}
	}
	#[inline]
	pub fn key(&self, key: &K) -> Vec<u8> {
		concat_byte_array_pairs(
			self.namespace, key.serialize().expect("key serialization should never fail").as_ref()
		)
	}
	#[inline]
	pub fn get_raw_bytes(&self, key: &K) -> Option<Vec<u8>> {
		self.storage.get(&self.key(key))
	}
	#[inline]
	pub(crate) fn set_raw_bytes(&self, key: &K, bytes: &[u8]) {
		self.storage.set(&self.key(key), bytes)
	}
	pub fn get(&self, key: &K) -> StdResult<Option<V>> {
		let Some(data) = self.get_raw_bytes(key) else {
			return Ok(None);
		};
		Ok(
			Some(
				V::deserialize(&data)?
			)
		)
	}
	pub fn get_autosaving(&self, key: &K) -> StdResult<Option<AutosavingSerializableItem<'exec, V>>> {
		AutosavingSerializableItem::new(
			&self.storage.get_mutable_shared().expect("get_autosaving should only be used in a mutable context"),
			self.key(key)
		)
	}
	pub fn get_or_default_autosaving(
		&self,
		key: &K
	) -> StdResult<AutosavingSerializableItem<'exec, V>> where V: Default {
		AutosavingSerializableItem::new_or_default(
			&self.storage.get_mutable_shared().expect("get_autosaving should only be used in a mutable context"),
			self.key(key)
		)
	}
	pub fn has(&self, key: &K) -> bool {
		self.storage.get(&self.key(key)).is_some()
	}
	pub fn set(&self, key: &K, value: &V) -> StdResult<()> {
		self.storage.set(
			&self.key(key),
			&value.serialize()?.as_ref()
		);
		Ok(())
	}
	pub fn remove(&self, key: &K) {
		self.storage.remove(&self.key(key))
	}
	pub fn iter(&self) -> StdResult<StoredMapIter<'exec, K, V>> {
		StoredMapIter::new(
			self.storage.clone(),
			self.namespace,
			(),
			None,
			None
		)
	}
}


/// Allows you to iterate over a stored map.
/// 
/// If your key type for your stored map is a tuple, i.e. `(T1, T2, T3)`, you can set `K` to `(T2, T3)` while providing
/// `T1` as the `partial_key` in the `new()` function.
/// 
/// If you don't care about the keys or values and don't want to parse them, set it to the unit type `()`.
pub struct StoredMapIter<'a, K: SerializableItem, V: SerializableItem> {
	storage: MaybeMutableStorage<'a>,
	last_forward_key: Vec<u8>,
	last_backward_key: Vec<u8>,
	key_type: PhantomData<K>,
	value_type: PhantomData<V>,
	key_slicing: usize
}

impl<'a, K: SerializableItem, V: SerializableItem> StoredMapIter<'a, K, V> {
	/// Note that start_key and end_key are both exclusive, i.e. this key, if it exists, will be skipped
	pub fn new<P>(
		storage: MaybeMutableStorage<'a>,
		namespace: &[u8],
		key_prefix: P,
		start_key: Option<K>,
		end_key: Option<K>,
	) -> StdResult<Self> where P: SerializableItem  {
		let prefix_bytes = key_prefix.serialize()?;
		let start_bytes = start_key.map_or(Ok(Vec::new()), |k| {
			k.serialize().map(|maybe_vec| {maybe_vec.into()})
		})?;
		let end_bytes = end_key.map_or(Ok(Vec::new()), |k| {
			k.serialize().map(|maybe_vec| {maybe_vec.into()})
		})?;

		let mut start_key = Vec::with_capacity(
			namespace.len() +
			prefix_bytes.as_ref().len() +
			start_bytes.len()
		);
		start_key.extend_from_slice(namespace);
		start_key.extend_from_slice(prefix_bytes.as_ref());
		start_key.extend_from_slice(start_bytes.as_ref());

		let mut end_key = Vec::with_capacity(
			namespace.len() +
			prefix_bytes.as_ref().len() +
			end_bytes.len()
		);
		end_key.extend_from_slice(namespace);
		end_key.extend_from_slice(prefix_bytes.as_ref());
		end_key.extend_from_slice(end_bytes.as_ref());

		Ok(
			Self {
				storage,
				last_forward_key: start_key,
				last_backward_key: end_key,
				key_type: PhantomData,
				value_type: PhantomData,
				key_slicing: namespace.len() + prefix_bytes.as_ref().len()
			}
		)
	}
}
impl<'a, K: SerializableItem, V: SerializableItem> Iterator for StoredMapIter<'a, K, V> {
	type Item = (K, V);
	fn next(&mut self) -> Option<Self::Item> {
		let Some((key_bytes, value_bytes)) = self.storage.next_record(
			&self.last_forward_key,
			Some(&self.last_backward_key)
		) else {
			return None;
		};
		if key_bytes == self.last_backward_key {
			return None;
		}
		let deserialized_key = K::deserialize(&key_bytes[self.key_slicing..]).ok()?;
		self.last_forward_key = key_bytes;
		Some((
			deserialized_key,
			V::deserialize(&value_bytes).ok()?
		))
	}
}
impl<'a, K: SerializableItem, V: SerializableItem> DoubleEndedIterator for StoredMapIter<'a, K, V> {
	fn next_back(&mut self) -> Option<Self::Item> {
		let Some((key_bytes, value_bytes)) = self.storage.prev_record(
			&self.last_backward_key,
			Some(&self.last_forward_key)
		) else {
			return None;
		};
		if key_bytes == self.last_forward_key {
			return None;
		}
		let deserialized_key = K::deserialize(&key_bytes[self.key_slicing..]).ok()?;
		self.last_backward_key = key_bytes;
		Some((
			deserialized_key,
			V::deserialize(&value_bytes).ok()?
		))
	}
}
