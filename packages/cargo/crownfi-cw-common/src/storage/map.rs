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
			key_type: PhantomData {},
			value_type: PhantomData {}
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
}
