use std::marker::PhantomData;

use cosmwasm_std::StdResult;

use super::{SerializableItem, MaybeMutableStorage, concat_byte_array_pairs};

pub struct StoredMap<'exec, V: SerializableItem> {
	namespace: &'static [u8],
	storage: MaybeMutableStorage<'exec>,
	value_type: PhantomData<V>
}

impl<'exec, V: SerializableItem> StoredMap<'exec, V> {
	pub fn new(namespace: &'static [u8], storage: MaybeMutableStorage<'exec>) -> Self {
		Self {
			namespace,
			storage,
			value_type: PhantomData {}
		}
	}
	/*
	pub fn get(&self, key: &K) -> StdResult<Option<V>> {
		let Some(data) = self.storage.get(&self.key(key)) else {
			return Ok(None);
		};
		Ok(
			Some(
				V::deserialize(&data)?
			)
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
	*/
}
