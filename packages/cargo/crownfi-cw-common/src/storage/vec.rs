use std::{marker::PhantomData, num::NonZeroUsize};

use cosmwasm_std::{OverflowError, StdError};

use super::{
	base::{storage_read, storage_read_item, storage_write},
	concat_byte_array_pairs,
	map::StoredMap,
	OZeroCopy, SerializableItem,
};

pub struct StoredVec<V: SerializableItem> {
	namespace: &'static [u8],
	map: StoredMap<u32, V>,
	len: u32,
}

impl<'exec, V: SerializableItem> StoredVec<V> {
	pub fn new(namespace: &'static [u8]) -> Self {
		let len = storage_read(namespace)
			.map(|data| u32::from_le_bytes(data.try_into().unwrap_or_default()))
			.unwrap_or_default();

		Self {
			namespace,
			map: StoredMap::new(namespace),
			len,
		}
	}

	#[inline]
	fn set_len(&mut self, value: u32) {
		self.len = value;
		storage_write(self.namespace, &value.to_le_bytes());
	}

	pub fn len(&self) -> u32 {
		return self.len;
	}
	pub fn get(&self, index: u32) -> Result<Option<OZeroCopy<V>>, StdError> {
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
	pub fn iter(&self) -> IndexedStoredItemIter<V> {
		let len = self.len();
		IndexedStoredItemIter::new(self.namespace, 0, len)
	}
	pub fn pop(&mut self) -> Result<Option<OZeroCopy<V>>, StdError> {
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
	pub fn remove(&mut self, index: u32) -> Result<OZeroCopy<V>, StdError> {
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
	pub fn swap_remove(&mut self, index: u32) -> Result<OZeroCopy<V>, StdError> {
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

impl<V: SerializableItem> IntoIterator for StoredVec<V> {
	type Item = Result<OZeroCopy<V>, StdError>;
	type IntoIter = IndexedStoredItemIter<V>;
	fn into_iter(self) -> Self::IntoIter {
		let len = self.len();
		IndexedStoredItemIter::new(self.namespace, 0, len)
	}
}
impl<V: SerializableItem> IntoIterator for &StoredVec<V> {
	type Item = Result<OZeroCopy<V>, StdError>;
	type IntoIter = IndexedStoredItemIter<V>;
	fn into_iter(self) -> Self::IntoIter {
		let len = self.len();
		IndexedStoredItemIter::new(self.namespace, 0, len)
	}
}

/// Iterator for StoredVec and StoredVecDeque
pub struct IndexedStoredItemIter<V: SerializableItem> {
	namespace: &'static [u8],
	start: u32,
	end: u32,
	value_type: PhantomData<V>,
}
impl<'exec, V: SerializableItem> IndexedStoredItemIter<V> {
	pub fn new(namespace: &'static [u8], start: u32, end: u32) -> Self {
		Self {
			namespace,
			start,
			end,
			value_type: PhantomData,
		}
	}
	// TODO: move to respective trait when https://github.com/rust-lang/rust/issues/77404 is closed.
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
	// TODO: move to respective trait when https://github.com/rust-lang/rust/issues/77404 is closed.
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

impl<'exec, V: SerializableItem> Iterator for IndexedStoredItemIter<V> {
	type Item = Result<OZeroCopy<V>, StdError>;
	fn next(&mut self) -> Option<Self::Item> {
		if self.start == self.end {
			return None;
		}
		let result = storage_read_item(&concat_byte_array_pairs(self.namespace, &self.start.to_le_bytes())).transpose();
		self.start = self.start.wrapping_add(1);
		result
	}

	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.advance_by(n).ok()?;
		self.next()
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let result;
		if self.start > self.end {
			result = u32::MAX - (self.start - self.end) + 1;
		} else {
			result = self.end - self.start;
		}
		(result as usize, Some(result as usize))
	}
}
impl<'exec, V: SerializableItem> DoubleEndedIterator for IndexedStoredItemIter<V> {
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.start == self.end {
			return None;
		}
		self.end = self.end.wrapping_sub(1);
		storage_read_item(&concat_byte_array_pairs(self.namespace, &self.end.to_le_bytes())).transpose()
	}

	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		self.advance_back_by(n).ok()?;
		self.next_back()
	}
}
impl<'exec, V: SerializableItem> ExactSizeIterator for IndexedStoredItemIter<V> {
	// relies on size_hint to return 2 exact numbers
}

#[cfg(test)]
mod tests {
	use cosmwasm_std::MemoryStorage;

	use crate::storage::base::set_global_storage;

	use super::*;
	use crate::storage::testing_common::*;

	#[test]
	fn get_after_dirty_clear() -> TestingResult {
		set_global_storage(Box::new(MemoryStorage::new()));
		let _storage_lock = init()?;
		let mut vec = StoredVec::<u16>::new(NAMESPACE);

		vec.extend([1, 2, 3].into_iter())?;
		vec.clear(true);

		let val = vec.get(0);

		assert_eq!(val, Ok(None));

		Ok(())
	}

	#[test]
	fn stored_vec() -> TestingResult {
		let _storage_lock = init()?;
		let mut vec = StoredVec::<u16>::new(NAMESPACE);

		vec.push(&69)?;
		vec.push(&420)?;

		let vec: Vec<u16> = vec
			.into_iter()
			.filter_map(Result::ok)
			.map(OZeroCopy::into_inner)
			.collect();
		assert_eq!(vec, vec![69, 420]);

		let vec = StoredVec::<u16>::new(NAMESPACE);
		assert_eq!(2, vec.len());
		assert_eq!(Some(OZeroCopy::from_inner(69)), vec.get(0)?);
		assert_eq!(Some(OZeroCopy::from_inner(420)), vec.get(1)?);

		vec.set(0, &123)?;
		assert!(vec.set(vec.len() + 1, &123).is_err());
		assert_eq!(Some(OZeroCopy::from_inner(123)), vec.get(0)?);

		assert_eq!(vec.capacity(), u32::MAX); // unnecessary, but i wanted to see all the function tested on the HTML file

		Ok(())
	}

	#[test]
	fn extend() -> TestingResult {
		let _storage_lock = init()?;
		let mut vec = StoredVec::<u16>::new(NAMESPACE);

		vec.push(&69)?;
		vec.push(&420)?;
		vec.extend([1, 2, 3].into_iter())?;
		vec.extend_ref([Box::new(4)].into_iter())?;

		let vec: Vec<u16> = vec
			.into_iter()
			.filter_map(Result::ok)
			.map(OZeroCopy::into_inner)
			.collect();
		assert_eq!(vec, vec![69, 420, 1, 2, 3, 4]);

		Ok(())
	}

	#[test]
	fn insert_and_remove() -> TestingResult {
		let _storage_lock = init()?;
		let mut vec = StoredVec::<u16>::new(NAMESPACE);

		vec.push(&69)?;
		vec.push(&420)?;

		vec.insert(1, &1)?;
		let v: Vec<_> = vec.iter().filter_map(Result::ok).map(OZeroCopy::into_inner).collect();
		assert_eq!(v, vec![69, 1]);

		vec.remove(1)?;
		let v: Vec<_> = vec.iter().filter_map(Result::ok).map(OZeroCopy::into_inner).collect();
		assert_eq!(v, vec![69]);

		vec.extend([1, 2, 3].into_iter())?;
		vec.pop()?;

		let v: Vec<_> = vec.iter().filter_map(Result::ok).map(OZeroCopy::into_inner).collect();
		assert_eq!(v, vec![69, 1, 2]);

		vec.remove(1)?;
		let v: Vec<_> = vec.iter().filter_map(Result::ok).map(OZeroCopy::into_inner).collect();
		assert_eq!(v, vec![69, 2]);

		vec.clear(true);
		assert!(vec.is_empty());
		assert!(vec.pop()?.is_none());

		Ok(())
	}

	#[test]
	fn extra_ops() -> TestingResult {
		let _storage_lock = init()?;
		let mut vec = StoredVec::<u16>::new(NAMESPACE);

		vec.push(&69)?;
		vec.push(&420)?;

		vec.swap(0, 1)?;

		let v: Vec<_> = vec.iter().filter_map(Result::ok).map(OZeroCopy::into_inner).collect();
		assert_eq!(v, vec![420, 69]);

		vec.extend([1, 2, 3].into_iter())?;
		vec.swap_remove(1)?;

		let v: Vec<_> = vec.iter().filter_map(Result::ok).map(OZeroCopy::into_inner).collect();
		assert_eq!(v, vec![420, 3, 1, 2]);

		vec.truncate(3, true);
		let v: Vec<_> = vec.iter().filter_map(Result::ok).map(OZeroCopy::into_inner).collect();
		assert_eq!(v, vec![420, 3, 1]);

		let mut iterator = vec.iter();
		iterator.advance_by(1).unwrap();
		let v: Vec<_> = iterator.filter_map(Result::ok).map(OZeroCopy::into_inner).collect();
		assert_eq!(v, vec![3, 1]);

		let mut iterator = vec.iter();
		iterator.advance_back_by(1).unwrap();
		let v: Vec<_> = iterator.filter_map(Result::ok).map(OZeroCopy::into_inner).collect();
		assert_eq!(v, vec![420, 3]);

		let mut iterator = vec.iter();
		let x = iterator.next_back().unwrap()?;
		assert_eq!(*x, 1);
		let x = iterator.nth_back(0).unwrap()?;
		assert_eq!(*x, 3);
		let x = iterator.nth(0).unwrap()?;
		assert_eq!(*x, 420);

		Ok(())
	}

	#[test]
	fn after_drop() -> TestingResult {
		let _storage_lock = init()?;
		let mut vec = StoredVec::<u16>::new(NAMESPACE);

		vec.push(&69)?;
		vec.push(&420)?;

		drop(vec);

		let vec: Vec<u16> = StoredVec::<u16>::new(NAMESPACE)
			.into_iter()
			.filter_map(Result::ok)
			.map(OZeroCopy::into_inner)
			.collect();
		assert_eq!(vec, vec![69, 420]);

		Ok(())
	}

	#[test]
	fn clean() -> TestingResult {
		let _storage_lock = init()?;
		let mut vec = StoredVec::<u16>::new(NAMESPACE);

		let push_values = |vec: &mut StoredVec<u16>| -> TestingResult {
			vec.push(&69)?;
			vec.push(&420)?;
			Ok(())
		};

		let check_values = |vec: &StoredVec<u16>| -> TestingResult {
			let vec: Vec<u16> = vec.iter().filter_map(Result::ok).map(OZeroCopy::into_inner).collect();
			assert_eq!(vec, vec![69, 420]);
			Ok(())
		};

		push_values(&mut vec)?;
		check_values(&vec)?;
		vec.clear(true);
		drop(vec);

		let mut vec = StoredVec::<u16>::new(NAMESPACE);
		let q: Vec<_> = vec.iter().filter_map(Result::ok).collect();
		assert_eq!(q.len(), 0);

		push_values(&mut vec)?;
		check_values(&vec)?;
		vec.clear(false);
		drop(vec);

		let mut vec = StoredVec::<u16>::new(NAMESPACE);
		let q: Vec<_> = vec.iter().filter_map(Result::ok).collect();
		assert_eq!(q.len(), 0);

		push_values(&mut vec)?;
		check_values(&vec)?;
		drop(vec);

		let mut vec = StoredVec::<u16>::new(NAMESPACE);
		vec.clear(false);
		let q: Vec<_> = vec.iter().filter_map(Result::ok).collect();
		assert_eq!(q.len(), 0);

		push_values(&mut vec)?;
		check_values(&vec)?;
		drop(vec);

		let mut vec = StoredVec::<u16>::new(NAMESPACE);
		vec.clear(true);
		let q: Vec<_> = (&vec).into_iter().filter_map(Result::ok).collect();
		assert_eq!(q.len(), 0);

		Ok(())
	}
}
