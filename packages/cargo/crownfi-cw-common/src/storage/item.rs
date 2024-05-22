use super::base::{storage_read_item, storage_remove, storage_write, storage_write_item};
use super::{OZeroCopy, SerializableItem};
use cosmwasm_std::{StdError, Storage};
use std::ops::{Deref, DerefMut};

pub trait StoredItem: SerializableItem + Sized {
	fn namespace() -> &'static [u8];

	#[deprecated(note = "please use `storage_read_item` instead")]
	fn load_from_key(storage: &dyn Storage, key: &[u8]) -> Result<Option<Self>, StdError>
	where
		Self: Sized,
	{
		// storage.get(key).as_deref().map(Self::deserialize).transpose()
		let Some(data) = storage.get(key) else {
			return Ok(None);
		};
		Ok(Some(Self::deserialize_to_owned(&data)?))
	}
	#[deprecated(note = "please use `storage_write_item` instead")]
	fn save_to_key(&self, storage: &mut dyn Storage, key: &[u8]) -> Result<(), StdError> {
		if let Some(bytes) = self.serialize_as_ref() {
			storage.set(key, bytes);
		} else {
			storage.set(key, &self.serialize_to_owned()?)
		}
		Ok(())
	}

	#[inline]
	fn load() -> Result<Option<OZeroCopy<Self>>, StdError>
	where
		Self: Sized,
	{
		storage_read_item(Self::namespace())
	}

	#[inline]
	fn save(&self) -> Result<(), StdError> {
		storage_write_item(Self::namespace(), self)
	}

	fn remove() {
		storage_remove(Self::namespace())
	}

	fn load_with_autosave() -> Result<Option<AutosavingStoredItem<Self>>, StdError> {
		AutosavingStoredItem::new()
	}

	fn load_with_autosave_or_default() -> Result<AutosavingStoredItem<Self>, StdError>
	where
		Self: Default,
	{
		AutosavingStoredItem::new_or_default()
	}
}

pub struct AutosavingStoredItem<T: StoredItem> {
	value: OZeroCopy<T>,
}
impl<'a, T: StoredItem> AutosavingStoredItem<T> {
	pub fn new() -> Result<Option<Self>, StdError> {
		let Some(value) = T::load()? else {
			return Ok(None);
		};
		Ok(Some(Self { value }))
	}
}
impl<'a, T: StoredItem + Default> AutosavingStoredItem<T> {
	pub fn new_or_default() -> Result<Self, StdError> {
		let Some(value) = T::load()? else {
			return Ok(Self {
				value: OZeroCopy::from_inner(T::default()),
			});
		};
		Ok(Self { value })
	}
}
impl<T: StoredItem> Deref for AutosavingStoredItem<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.value
	}
}
impl<T: StoredItem> DerefMut for AutosavingStoredItem<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.value
	}
}
impl<T> Drop for AutosavingStoredItem<T>
where
	T: StoredItem,
{
	fn drop(&mut self) {
		match &self.value.0 {
			super::OZeroCopyType::Copy(val) => {
				storage_write_item(T::namespace(), val).expect("serialization error on autosave")
			}
			super::OZeroCopyType::ZeroCopy(bytes) => storage_write(T::namespace(), bytes),
		}
	}
}

pub struct AutosavingSerializableItem<T: SerializableItem> {
	value: OZeroCopy<T>,
	namespace: Vec<u8>,
}
impl<T: SerializableItem> AutosavingSerializableItem<T> {
	pub fn new(namespace: Vec<u8>) -> Result<Option<Self>, StdError> {
		let Some(value) = storage_read_item(&namespace)? else { return Ok(None) };
		Ok(Some(Self { value, namespace }))
	}
}
impl<'a, T: SerializableItem + Default> AutosavingSerializableItem<T> {
	pub fn new_or_default(namespace: Vec<u8>) -> Result<Self, StdError> {
		if let Some(value) = storage_read_item(&namespace)? {
			Ok(Self { value, namespace })
		} else {
			Ok(Self {
				value: OZeroCopy::from_inner(T::default()),
				namespace,
			})
		}
	}
}
impl<T: SerializableItem> Deref for AutosavingSerializableItem<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.value
	}
}
impl<T: SerializableItem> DerefMut for AutosavingSerializableItem<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.value
	}
}
impl<'a, T> Drop for AutosavingSerializableItem<T>
where
	T: SerializableItem,
{
	fn drop(&mut self) {
		match &self.value.0 {
			super::OZeroCopyType::Copy(val) => {
				storage_write_item(&self.namespace, val).expect("serialization error on autosave")
			}
			super::OZeroCopyType::ZeroCopy(bytes) => storage_write(&self.namespace, bytes),
		}
	}
}


#[cfg(test)]
mod tests {
	use crate::storage::{base::{storage_read, storage_remove}, testing_common::*};
	use super::*;

	impl StoredItem for u8 {
		fn namespace() -> &'static [u8] {
			b"testing"
		}
	}

	impl StoredItem for (u16, u16) {
		fn namespace() -> &'static [u8] {
			b"testing2"
		}
	}

	#[test]
	fn autosaving_stored_item() -> TestingResult {
		let _storage_lock = init()?;
		let mut item = u8::load_with_autosave_or_default()?;

		*item = 69;
		drop(item);

		let mut item = u8::load_with_autosave()?.unwrap();
		assert_eq!(69, *item);

		*item *= 2;
		assert_eq!(Some(69), storage_read_item::<u8>(u8::namespace())?.map(|x| x.into_inner()));
		drop(item);
		assert_eq!(Some(69 * 2), storage_read_item::<u8>(u8::namespace())?.map(|x| x.into_inner()));

		Ok(())
	}

	#[test]
	fn autosaving_stored_item_rm() -> TestingResult {
		let _storage_lock = init()?;
		let mut item = u8::load_with_autosave_or_default()?;

		*item = 69;
		drop(item);

		storage_remove(u8::namespace());
		assert!(storage_read(u8::namespace()).is_none());

		Ok(())
	}

	// testing borsh serialize/deserialize
	#[test]
	fn autosaving_tuple_items() -> TestingResult {
		let _storage_lock = init()?;
		let mut item = <(u16, u16)>::load_with_autosave_or_default()?;

		*item = (69, 420);
		drop(item);

		assert_eq!(Some((69, 420)), storage_read_item(<(u16, u16)>::namespace())?.map(OZeroCopy::into_inner));

		storage_remove(<(u16, u16)>::namespace());
		assert_eq!(None, storage_read(<(u16, u16)>::namespace()));

		Ok(())
	}
}
