use std::{rc::Rc, cell::RefCell};
use std::ops::{Deref, DerefMut};
use cosmwasm_std::{Storage, StdError};
use super::SerializableItem;


//pub fn load_cast_from_key

pub trait StoredItem: SerializableItem {
	fn namespace() -> &'static [u8];
	fn load_from_key(storage: & dyn Storage, key: &[u8]) -> Result<Option<Self>, StdError> where Self: Sized {
		let Some(data) = storage.get(key) else {
			return Ok(None);
		};
		Ok(
			Some(
				Self::deserialize(&data)?
			)
		)
	}
	fn save_to_key(&self, storage: &mut dyn Storage, key: &[u8]) -> Result<(), StdError> {
		storage.set(key, self.serialize()?.as_ref());
		Ok(())
	}
	#[inline]
	fn load(storage: & dyn Storage) -> Result<Option<Self>, StdError> where Self: Sized {
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
		storage: &Rc<RefCell<&'a mut dyn Storage>>
	) -> Result<Option<AutosavingStoredItem<'a, Self>>, StdError> where Self: Sized {
		AutosavingStoredItem::new(storage)
	}
	fn load_with_autosave_or_default<'a>(
		storage: &Rc<RefCell<&'a mut dyn Storage>>
	) -> Result<AutosavingStoredItem<'a, Self>, StdError> where Self: Default {
		AutosavingStoredItem::new_or_default(storage)
	}
}

pub struct AutosavingStoredItem<'a, T: StoredItem> {
	value: T,
	storage: Rc<RefCell<&'a mut dyn Storage>>
}
impl<'a, T: StoredItem> AutosavingStoredItem<'a, T> {
	pub fn new(storage: &Rc<RefCell<&'a mut dyn Storage>>) -> Result<Option<Self>, StdError> {
		let Some(value) = T::load(*storage.borrow())? else {
			return Ok(None);
		};
		Ok(
			Some(
				Self {
					value,
					storage: storage.clone()
				}
			)
		)
	}
}
impl<'a, T: StoredItem + Default> AutosavingStoredItem<'a, T> {
	pub fn new_or_default(storage: &Rc<RefCell<&'a mut dyn Storage>>) -> Result<Self, StdError> {
		let Some(value) = T::load(*storage.borrow())? else {
			return Ok(
				Self {
					value: T::default(),
					storage: storage.clone()
				}
			);
		};
		Ok(
			Self {
				value,
				storage: storage.clone()
			}
		)
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
impl<'a, T> Drop for AutosavingStoredItem<'a, T> where T: StoredItem {
	fn drop(&mut self) {
		self.value.save(*self.storage.borrow_mut())
			.expect("serialization error on autosave")
	}
}

pub struct AutosavingSerializableItem<'a, T: SerializableItem> {
	value: T,
	namespace: Vec<u8>,
	storage: Rc<RefCell<&'a mut dyn Storage>>
}
impl<'a, T: SerializableItem> AutosavingSerializableItem<'a, T> {
	pub fn new(storage: &Rc<RefCell<&'a mut dyn Storage>>, namespace: Vec<u8>) -> Result<Option<Self>, StdError> {
		let Some(data) = storage.borrow().get(&namespace) else {
			return Ok(None)
		};
		Ok(
			Some(
				Self {
					value: T::deserialize(&data)?,
					namespace,
					storage: storage.clone()
				}
			)
		)
		
	}
}
impl<'a, T: SerializableItem + Default> AutosavingSerializableItem<'a, T> {
	pub fn new_or_default(storage: &Rc<RefCell<&'a mut dyn Storage>>, namespace: Vec<u8>) -> Result<Self, StdError> {
		let Some(data) = storage.borrow().get(&namespace) else {
			return Ok(
				Self {
					value: T::default(),
					namespace,
					storage: storage.clone()
				}
			);
		};
		Ok(
			Self {
				value: T::deserialize(&data)?,
				namespace,
				storage: storage.clone()
			}
		)
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
impl<'a, T> Drop for AutosavingSerializableItem<'a, T> where T: SerializableItem {
	fn drop(&mut self) {
		self.storage.borrow_mut().set(
			&self.namespace,
			self.value.serialize().expect("autosave serialize should never fail").as_ref()
		);
	}
}
