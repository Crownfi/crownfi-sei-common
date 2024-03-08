use std::ops::{Deref, DerefMut};
use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{StdError, Storage};

use super::SerializableItem;

pub trait StoredItem: SerializableItem {
	fn namespace() -> &'static [u8];

	fn load_from_key(storage: &dyn Storage, key: &[u8]) -> Result<Option<Self>, StdError>
	where
		Self: Sized,
	{
		// storage.get(key).as_deref().map(Self::deserialize).transpose()
		let Some(data) = storage.get(key) else {
			return Ok(None);
		};
		Ok(Some(Self::deserialize(&data)?))
	}

	fn save_to_key(&self, storage: &mut dyn Storage, key: &[u8]) -> Result<(), StdError> {
		if let Some(bytes) = self.serialize_as_ref() {
			storage.set(key, bytes);
		} else {
			storage.set(key, &self.serialize_to_owned()?)
		}
		Ok(())
	}

	#[inline]
	fn load(storage: &dyn Storage) -> Result<Option<Self>, StdError>
	where
		Self: Sized,
	{
		Self::load_from_key(storage, Self::namespace())
	}

	#[inline]
	fn save(&self, storage: &mut dyn Storage) -> Result<(), StdError> {
		self.save_to_key(storage, Self::namespace())
	}

	fn remove(storage: &mut dyn Storage) {
		storage.remove(Self::namespace())
	}

	fn load_with_autosave<'a>(
		storage: &Rc<RefCell<&'a mut dyn Storage>>,
	) -> Result<Option<AutosavingStoredItem<'a, Self>>, StdError>
	where
		Self: Sized,
	{
		AutosavingStoredItem::new(storage)
	}

	fn load_with_autosave_or_default<'a>(
		storage: &Rc<RefCell<&'a mut dyn Storage>>,
	) -> Result<AutosavingStoredItem<'a, Self>, StdError>
	where
		Self: Default,
	{
		AutosavingStoredItem::new_or_default(storage)
	}
}

pub struct AutosavingStoredItem<'a, T: StoredItem> {
	value: T,
	storage: Rc<RefCell<&'a mut dyn Storage>>,
}

impl<'a, T: StoredItem> AutosavingStoredItem<'a, T> {
	pub fn new(storage: &Rc<RefCell<&'a mut dyn Storage>>) -> Result<Option<Self>, StdError> {
		let Some(value) = T::load(*storage.borrow())? else {
			return Ok(None);
		};
		Ok(Some(Self {
			value,
			storage: storage.clone(),
		}))
	}
}

impl<'a, T: StoredItem + Default> AutosavingStoredItem<'a, T> {
	pub fn new_or_default(storage: &Rc<RefCell<&'a mut dyn Storage>>) -> Result<Self, StdError> {
		let Some(value) = T::load(*storage.borrow())? else {
			return Ok(Self {
				value: T::default(),
				storage: storage.clone(),
			});
		};
		Ok(Self {
			value,
			storage: storage.clone(),
		})
	}
}

impl<T: StoredItem> Deref for AutosavingStoredItem<'_, T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.value
	}
}

impl<T: StoredItem> DerefMut for AutosavingStoredItem<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.value
	}
}

impl<'a, T> Drop for AutosavingStoredItem<'a, T>
where
	T: StoredItem,
{
	fn drop(&mut self) {
		self.value
			.save(*self.storage.borrow_mut())
			.expect("serialization error on autosave")
	}
}

pub struct AutosavingSerializableItem<'a, T: SerializableItem> {
	value: T,
	namespace: Vec<u8>,
	storage: Rc<RefCell<&'a mut dyn Storage>>,
}

impl<'a, T: SerializableItem> AutosavingSerializableItem<'a, T> {
	pub fn new(storage: &Rc<RefCell<&'a mut dyn Storage>>, namespace: Vec<u8>) -> Result<Option<Self>, StdError> {
		let Some(data) = storage.borrow().get(&namespace) else { return Ok(None) };
		Ok(Some(Self {
			value: T::deserialize(&data)?,
			namespace,
			storage: storage.clone(),
		}))
	}
}

impl<'a, T: SerializableItem + Default> AutosavingSerializableItem<'a, T> {
	pub fn new_or_default(storage: &Rc<RefCell<&'a mut dyn Storage>>, namespace: Vec<u8>) -> Result<Self, StdError> {
		let Some(data) = storage.borrow().get(&namespace) else {
			return Ok(Self {
				value: T::default(),
				namespace,
				storage: storage.clone(),
			});
		};
		Ok(Self {
			value: T::deserialize(&data)?,
			namespace,
			storage: storage.clone(),
		})
	}
}

impl<T: SerializableItem> Deref for AutosavingSerializableItem<'_, T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.value
	}
}

impl<T: SerializableItem> DerefMut for AutosavingSerializableItem<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.value
	}
}

impl<'a, T> Drop for AutosavingSerializableItem<'a, T>
where
	T: SerializableItem,
{
	fn drop(&mut self) {
		let mut storage = self.storage.borrow_mut();
		if let Some(bytes) = self.value.serialize_as_ref() {
			storage.set(&self.namespace, bytes);
		} else {
			storage.set(
				&self.namespace,
				&self
					.value
					.serialize_to_owned()
					.expect("autosave serialize should never fail"),
			)
		}
	}
}

#[cfg(test)]
mod tests {
	use std::rc::Rc;

	use cosmwasm_std::testing::*;

	use super::*;

	impl StoredItem for u8 {
		fn namespace() -> &'static [u8] {
			b"testing"
		}
	}

	type TestingError<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

	#[test]
	fn autosaving_stored_item() -> TestingError {
		let mut storage_ = MockStorage::new();
		let storage = Rc::new(RefCell::new(&mut storage_ as &mut dyn Storage));
		let mut item = u8::load_with_autosave_or_default(&storage)?;
		*item = 69;

		drop(item);

		assert_eq!(69, u8::load(&mut storage_)?.unwrap());

		Ok(())
	}

	#[test]
	fn autosaving_stored_item_rm() -> TestingError {
		let mut storage_ = MockStorage::new();
		let storage = Rc::new(RefCell::new(&mut storage_ as &mut dyn Storage));
		let mut item = u8::load_with_autosave_or_default(&storage)?;
		*item = 69;
		drop(item);

		u8::remove(&mut storage_);
		assert_eq!(None, u8::load(&mut storage_)?);

		Ok(())
	}
}
