use std::{marker::PhantomData, num::NonZeroUsize};

use cosmwasm_std::{OverflowError, StdError};

use super::{concat_byte_array_pairs, map::StoredMap, MaybeMutableStorage, SerializableItem};

pub struct StoredVec<'exec, V: SerializableItem> {
	namespace: &'static [u8],
	storage: MaybeMutableStorage<'exec>,
	map: StoredMap<'exec, u32, V>,
	len: u32,
}

impl<'exec, V: SerializableItem> StoredVec<'exec, V> {
	pub fn new(namespace: &'static [u8], storage: MaybeMutableStorage<'exec>) -> Self {
		let len = storage
			.get(namespace)
			.map(|data| u32::from_le_bytes(data.try_into().unwrap_or_default()))
			.unwrap_or_default();
		Self {
			namespace,
			storage: storage.clone(),
			map: StoredMap::new(namespace, storage),
			len,
		}
	}
	#[inline]
	fn set_len(&mut self, value: u32) {
		self.len = value;
		self.storage.set(self.namespace, &value.to_le_bytes())
	}
	pub fn len(&self) -> u32 {
		return self.len;
	}
	pub fn get(&self, index: u32) -> Result<Option<V>, StdError> {
		if index < self.len {
			return self.map.get(&index);
		}
		Ok(None)
	}
	pub fn set(&self, index: u32, value: &V) -> Result<(), StdError> {
		if index >= self.len() {
			return Err(StdError::not_found("StoredVec out of bounds"));
		}
		self.map.set(&index, value)?;
		Ok(())
	}
	#[inline]
	pub fn capacity(&self) -> u32 {
		u32::MAX
	}
	pub fn clear(&mut self, dirty: bool) {
		if !dirty {
			let len = self.len();
			for i in 0..len {
				self.map.remove(&i);
			}
		}
		self.set_len(0);
	}
	pub fn extend<I: Iterator<Item = V>>(&mut self, iter: I) -> Result<(), StdError> {
		let mut len = self.len();
		for item in iter {
			self.map.set(&len, &item)?;
			len = len
				.checked_add(1)
				.ok_or(OverflowError::new(cosmwasm_std::OverflowOperation::Add, len, 1))?;
		}
		self.set_len(len);
		Ok(())
	}
	pub fn extend_ref<R: AsRef<V>, I: Iterator<Item = R>>(&mut self, iter: I) -> Result<(), StdError> {
		let mut len = self.len();
		for item in iter {
			self.map.set(&len, item.as_ref())?;
			len = len
				.checked_add(1)
				.ok_or(OverflowError::new(cosmwasm_std::OverflowOperation::Add, len, 1))?;
		}
		self.set_len(len);
		Ok(())
	}
	pub fn insert(&mut self, index: u32, element: &V) -> Result<(), StdError> {
		let len = self.len();
		if index > len {
			return Err(StdError::not_found("StoredVec out of bounds"));
		}
		for i in (index..len).rev() {
			self.map.set_raw_bytes(&(i + 1), &self.map.get_raw_bytes(&i).unwrap());
		}
		self.map.set(&index, element)
	}
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
	pub fn iter(&self) -> IndexedStoredItemIter<'exec, V> {
		let len = self.len();
		IndexedStoredItemIter::new(self.namespace, self.storage.clone(), 0, len)
	}
	pub fn pop(&mut self) -> Result<Option<V>, StdError> {
		let mut len = self.len();
		if len == 0 {
			return Ok(None);
		}
		len -= 1;
		let result = self.map.get(&len)?;
		self.map.remove(&len);
		self.set_len(len);
		Ok(result)
	}
	pub fn push(&mut self, element: &V) -> Result<(), StdError> {
		let mut len = self.len();
		self.map.set(&len, element)?;
		len = len
			.checked_add(1)
			.ok_or(OverflowError::new(cosmwasm_std::OverflowOperation::Add, len, 1))?;
		self.set_len(len);
		Ok(())
	}
	pub fn remove(&mut self, index: u32) -> Result<V, StdError> {
		let new_len = self
			.len()
			.checked_sub(1)
			.ok_or(StdError::not_found("StoredVec out of bounds"))?;
		let result = self
			.map
			.get(&index)?
			.ok_or(StdError::not_found("StoredVec out of bounds"))?;
		for i in index..new_len {
			self.map.set_raw_bytes(&i, &self.map.get_raw_bytes(&(i + 1)).unwrap());
		}
		self.map.remove(&new_len);
		self.set_len(new_len);
		Ok(result)
	}
	pub fn swap(&self, index1: u32, index2: u32) -> Result<(), StdError> {
		let tmp_value = self
			.map
			.get_raw_bytes(&index1)
			.ok_or(StdError::not_found("StoredVec out of bounds"))?;
		self.map.set_raw_bytes(
			&index1,
			&self
				.map
				.get_raw_bytes(&index2)
				.ok_or(StdError::not_found("StoredVec out of bounds"))?,
		);
		self.map.set_raw_bytes(&index2, &tmp_value);
		Ok(())
	}
	pub fn swap_remove(&mut self, index: u32) -> Result<V, StdError> {
		let new_len = self
			.len()
			.checked_sub(1)
			.ok_or(StdError::not_found("StoredVec out of bounds"))?;
		let result = self
			.map
			.get(&index)?
			.ok_or(StdError::not_found("StoredVec out of bounds"))?;
		self.map
			.set_raw_bytes(&index, &self.map.get_raw_bytes(&new_len).unwrap());
		self.map.remove(&new_len);
		self.set_len(new_len);
		Ok(result)
	}
	pub fn truncate(&mut self, len: u32, dirty: bool) {
		let cur_len = self.len();
		if cur_len <= len {
			return;
		}
		if !dirty {
			for i in len..cur_len {
				self.map.remove(&i);
			}
		}
		self.set_len(len);
	}
}

impl<'exec, V: SerializableItem> IntoIterator for StoredVec<'exec, V> {
	type Item = Result<V, StdError>;
	type IntoIter = IndexedStoredItemIter<'exec, V>;
	fn into_iter(self) -> Self::IntoIter {
		let len = self.len();
		IndexedStoredItemIter::new(self.namespace, self.storage, 0, len)
	}
}
impl<'exec, V: SerializableItem> IntoIterator for &StoredVec<'exec, V> {
	type Item = Result<V, StdError>;
	type IntoIter = IndexedStoredItemIter<'exec, V>;
	fn into_iter(self) -> Self::IntoIter {
		let len = self.len();
		IndexedStoredItemIter::new(self.namespace, self.storage.clone(), 0, len)
	}
}

/// Iterator for StoredVec and StoredVecDeque
pub struct IndexedStoredItemIter<'exec, V: SerializableItem> {
	namespace: &'static [u8],
	storage: MaybeMutableStorage<'exec>,
	start: u32,
	end: u32,
	value_type: PhantomData<V>,
}
impl<'exec, V: SerializableItem> IndexedStoredItemIter<'exec, V> {
	pub fn new(namespace: &'static [u8], storage: MaybeMutableStorage<'exec>, start: u32, end: u32) -> Self {
		Self {
			namespace,
			storage,
			start,
			end,
			value_type: PhantomData,
		}
	}
	// TODO: move to respective traits when https://github.com/rust-lang/rust/issues/77404 is closed.
	// don't needlessly de-serialize things when calling .skip()
	pub fn advance_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
		if self.start == self.end {
			if n == 0 {
				return Ok(());
			} else {
				// SAFTY: the n == 0 check literally just failed
				return Err(unsafe { NonZeroUsize::new_unchecked(n) });
			}
		}
		let result;
		let new_start = self.start.wrapping_add(n as u32);
		if self.start > self.end {
			if new_start < self.start && new_start > self.end {
				result = Err(unsafe { NonZeroUsize::new_unchecked((new_start - self.end) as usize) });
			} else {
				result = Ok(());
			}
		} else {
			if new_start < self.start {
				result = Err(unsafe { NonZeroUsize::new_unchecked((new_start + (u32::MAX - self.end)) as usize) });
			} else if new_start > self.end {
				result = Err(unsafe { NonZeroUsize::new_unchecked((new_start - self.end) as usize) });
			} else {
				result = Ok(());
			}
		}
		if result.is_ok() {
			self.start = new_start;
		} else {
			// Make all future nexts return none
			self.start = self.end;
		}
		result
	}
	fn advance_back_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
		if self.start == self.end {
			if n == 0 {
				return Ok(());
			} else {
				// SAFTY: the n == 0 check literally just failed
				return Err(unsafe { NonZeroUsize::new_unchecked(n) });
			}
		}
		let result;
		let new_end = self.end.wrapping_sub(n as u32);
		if self.start > self.end {
			if new_end > self.end && new_end < self.start {
				result = Err(unsafe { NonZeroUsize::new_unchecked((self.start - new_end) as usize) });
			} else {
				result = Ok(())
			}
		} else {
			if new_end < self.start {
				result = Err(unsafe { NonZeroUsize::new_unchecked((self.start - new_end) as usize) });
			} else if new_end > self.end {
				result = Err(unsafe { NonZeroUsize::new_unchecked((self.start + (u32::MAX - new_end)) as usize) });
			} else {
				result = Ok(())
			}
		}
		if result.is_ok() {
			self.end = new_end;
		} else {
			// Make all future nexts return none
			self.end = self.start;
		}
		result
	}
}

impl<'exec, V: SerializableItem> Iterator for IndexedStoredItemIter<'exec, V> {
	type Item = Result<V, StdError>;
	fn next(&mut self) -> Option<Self::Item> {
		if self.start == self.end {
			return None;
		}
		let Some(data) = self
			.storage
			.get(&concat_byte_array_pairs(self.namespace, &self.start.to_le_bytes()))
		else {
			return None;
		};
		self.start = self.start.wrapping_add(1);
		Some(V::deserialize(&data))
	}
	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.advance_by(n).ok()?;
		self.next()
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let result;
		if self.start > self.end {
			result = u32::MAX - (self.start - self.end);
		} else {
			result = self.end - self.start;
		}
		(result as usize, Some(result as usize))
	}
}
impl<'exec, V: SerializableItem> DoubleEndedIterator for IndexedStoredItemIter<'exec, V> {
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.start == self.end {
			return None;
		}
		self.end = self.end.wrapping_sub(1);
		let Some(data) = self
			.storage
			.get(&concat_byte_array_pairs(self.namespace, &self.end.to_le_bytes()))
		else {
			return None;
		};
		Some(V::deserialize(&data))
	}
	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		self.advance_back_by(n).ok()?;
		self.next_back()
	}
}
impl<'exec, V: SerializableItem> ExactSizeIterator for IndexedStoredItemIter<'exec, V> {
	// relies on size_hint to return 2 exact numbers
}

#[cfg(test)]
mod tests {
	use std::{cell::RefCell, rc::Rc};

	use cosmwasm_std::{testing::MockStorage, Storage};

	use super::*;

	const NAMESPACE: &[u8] = b"testing";

	type TestingResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

	#[test]
	fn get_after_dirty_clear() -> TestingResult {
		let mut storage_ = MockStorage::new();
		let storage = Rc::new(RefCell::new(&mut storage_ as &mut dyn Storage));
		let storage = MaybeMutableStorage::new_mutable_shared(storage);
		let mut vec = StoredVec::<u16>::new(NAMESPACE, storage.clone());

		vec.extend([1, 2, 3].into_iter())?;
		vec.clear(true);

		let val = vec.get(0);

		assert_eq!(val, Ok(None));

		Ok(())
	}
}
