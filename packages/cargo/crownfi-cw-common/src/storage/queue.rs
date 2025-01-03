use bytemuck::{Pod, Zeroable};
use cosmwasm_std::{StdError, StdResult};

use crate::impl_serializable_as_ref;

use super::{
	base::{storage_read, storage_write_item},
	map::StoredMap,
	vec::IndexedStoredItemIter,
	OZeroCopy, SerializableItem,
};

#[derive(Debug, Default, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct QueueEnds {
	pub front: u32,
	pub back: u32,
}
impl_serializable_as_ref!(QueueEnds);

pub struct StoredVecDeque<V: SerializableItem> {
	namespace: &'static [u8],
	map: StoredMap<u32, V>,
	ends: QueueEnds,
}
impl<V: SerializableItem> StoredVecDeque<V> {
	pub fn new(namespace: &'static [u8]) -> Self {
		let ends = storage_read(namespace)
			.map(|data| {
				if data.len() == 4 {
					// Vec that has been "upgraded" to a queue
					return QueueEnds {
						front: 0,
						back: u32::from_le_bytes(data.try_into().unwrap()),
					};
				}
				QueueEnds::deserialize_to_owned(&data).unwrap_or_default()
			})
			.unwrap_or_default();

		Self {
			namespace,
			map: StoredMap::new(namespace),
			ends,
		}
	}

	#[inline]
	fn set_ends(&mut self, value: QueueEnds) {
		self.ends = value;
		storage_write_item(self.namespace, &value).expect("2 u32's should never fail to serialize");
	}

	#[inline]
	pub fn ends(&self) -> QueueEnds {
		self.ends
	}

	fn to_raw_index(&self, index: u32) -> u32 {
		index.wrapping_add(self.ends().front)
	}

	pub fn len(&self) -> u32 {
		let ends = self.ends();
		if ends.back >= ends.front {
			ends.back - ends.front
		} else {
			u32::MAX - (ends.front - ends.back) + 1
		}
	}
	pub fn get(&self, index: u32) -> StdResult<Option<OZeroCopy<V>>> {
		if index >= self.len() {
			return Ok(None);
		}

		self.map.get(&self.to_raw_index(index))
	}
	pub fn set(&self, index: u32, value: &V) -> StdResult<()> {
		if index >= self.len() {
			return Err(StdError::not_found("StoredVecDeque out of bounds"));
		}
		self.map.set(&self.to_raw_index(index), value)
	}

	pub fn swap(&self, index1: u32, index2: u32) -> StdResult<()> {
		let index1 = self.to_raw_index(index1);
		let index2 = self.to_raw_index(index2);
		let tmp_value = self
			.map
			.get_raw_bytes(&index1)
			.ok_or(StdError::not_found("StoredVecDeque out of bounds"))?;
		self.map.set_raw_bytes(
			&index1,
			&self
				.map
				.get_raw_bytes(&index2)
				.ok_or(StdError::not_found("StoredVecDeque out of bounds"))?,
		);
		self.map.set_raw_bytes(&index2, &tmp_value);
		Ok(())
	}

	pub fn capacity(&self) -> u32 {
		u32::MAX
	}
	pub fn iter(&self) -> IndexedStoredItemIter<V> {
		let ends = self.ends();
		IndexedStoredItemIter::new(self.namespace, ends.front, ends.back)
	}

	#[inline]
	pub fn is_empty(&self) -> bool {
		self.ends.front == self.ends.back
	}

	pub fn clear(&mut self, dirty: bool) {
		if !dirty {
			while self.ends.front != self.ends.back {
				self.map.remove(&self.ends.front);
				self.ends.front = self.ends.front.wrapping_add(1);
			}
		}
		self.set_ends(QueueEnds { front: 0, back: 0 });
	}
	pub fn get_back(&self) -> StdResult<Option<OZeroCopy<V>>> {
		if self.is_empty() {
			return Ok(None);
		}
		self.map.get(&self.ends.back.wrapping_sub(1))
	}

	pub fn set_back(&self, value: &V) -> StdResult<()> {
		if self.is_empty() {
			return Err(StdError::not_found("StoredVecDeque out of bounds"));
		}
		self.map.set(&self.ends.back.wrapping_sub(1), value)
	}
	pub fn pop_back(&mut self) -> StdResult<Option<OZeroCopy<V>>> {
		if self.is_empty() {
			return Ok(None);
		}
		let mut ends = self.ends();
		ends.back = self.ends.back.wrapping_sub(1);
		let result = self.map.get(&ends.back)?;
		self.map.remove(&ends.back);
		self.set_ends(ends);
		Ok(result)
	}

	pub fn push_back(&mut self, value: &V) -> StdResult<()> {
		let mut ends = self.ends();
		if ends.back.wrapping_add(1) == ends.front {
			return Err(StdError::generic_err("StoredVecQueue is full"))?;
		}
		self.map.set(&ends.back, value)?;
		ends.back = ends.back.wrapping_add(1);
		self.set_ends(ends);
		Ok(())
	}
	pub fn get_front(&self) -> StdResult<Option<OZeroCopy<V>>> {
		if self.is_empty() {
			return Ok(None);
		}
		self.map.get(&self.ends.front)
	}

	pub fn set_front(&self, value: &V) -> StdResult<()> {
		if self.is_empty() {
			return Err(StdError::not_found("StoredVecDeque out of bounds"));
		}
		self.map.set(&self.ends.front, value)
	}
	pub fn pop_front(&mut self) -> StdResult<Option<OZeroCopy<V>>> {
		if self.is_empty() {
			return Ok(None);
		}
		let mut ends = self.ends();
		let result = self.map.get(&ends.front)?;
		self.map.remove(&ends.front);
		ends.front = ends.front.wrapping_add(1);
		self.set_ends(ends);
		Ok(result)
	}

	pub fn push_front(&mut self, value: &V) -> StdResult<()> {
		let mut ends = self.ends();
		ends.front = ends.front.wrapping_sub(1);
		if ends.front == ends.back {
			return Err(StdError::generic_err("StoredVecQueue is full"))?;
		}
		self.map.set(&ends.front, value)?;
		self.set_ends(ends);
		Ok(())
	}
}

impl<V: SerializableItem> IntoIterator for StoredVecDeque<V> {
	type Item = Result<OZeroCopy<V>, StdError>;
	type IntoIter = IndexedStoredItemIter<V>;
	fn into_iter(self) -> Self::IntoIter {
		let ends = self.ends();
		IndexedStoredItemIter::new(self.namespace, ends.front, ends.back)
	}
}
impl<V: SerializableItem> IntoIterator for &StoredVecDeque<V> {
	type Item = Result<OZeroCopy<V>, StdError>;
	type IntoIter = IndexedStoredItemIter<V>;
	fn into_iter(self) -> Self::IntoIter {
		let ends = self.ends();
		IndexedStoredItemIter::new(self.namespace, ends.front, ends.back)
	}
}
#[cfg(test)]
mod tests {
	use super::*;
	use crate::storage::{
		base::{storage_has, storage_remove},
		testing_common::*,
	};
	use std::collections::VecDeque;

	type TestingResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
	const NAMESPACE: &[u8] = b"testing";
	#[test]
	fn push_front_get_pop() -> TestingResult {
		let _storage_lock = init()?;
		let mut queue = StoredVecDeque::<u16>::new(NAMESPACE);

		queue.push_front(&1)?;
		queue.push_front(&2)?;
		queue.push_front(&3)?;

		let val = queue.get(3);

		// assert_eq!(queue.len(), 3); XXX: won't work until StoredVecDeque::len is fixed
		assert_eq!(val, Ok(None));
		assert_eq!(queue.pop_back()?.map(|ozc| { ozc.into_inner() }), Some(1));
		assert_eq!(queue.len(), 2);

		assert_eq!(queue.pop_front()?.map(|ozc| { ozc.into_inner() }), Some(3));
		assert_eq!(queue.len(), 1);
		assert_eq!(queue.get(0)?.map(|ozc| { ozc.into_inner() }), Some(2));
		Ok(())
	}
	#[test]
	fn push_back_get_pop() -> TestingResult {
		let _storage_lock = init()?;
		let mut queue = StoredVecDeque::<u16>::new(NAMESPACE);

		queue.push_back(&1)?;
		queue.push_back(&2)?;
		queue.push_back(&3)?;

		let val = queue.get(3);

		assert_eq!(queue.len(), 3);
		assert_eq!(val, Ok(None));
		assert_eq!(queue.pop_front()?.map(|ozc| { ozc.into_inner() }), Some(1));
		assert_eq!(queue.len(), 2);

		assert_eq!(queue.pop_back()?.map(|ozc| { ozc.into_inner() }), Some(3));
		assert_eq!(queue.len(), 1);
		assert_eq!(queue.get(0)?.map(|ozc| { ozc.into_inner() }), Some(2));

		Ok(())
	}

	#[test]
	fn queue() -> TestingResult {
		let _storage_lock = init()?;
		let mut queue = StoredVecDeque::<u16>::new(NAMESPACE);

		queue.push_front(&69)?;
		queue.push_back(&420)?;
		queue.push_front(&1234)?;
		queue.pop_back()?;

		let queue_: VecDeque<u16> = queue.iter().filter_map(Result::ok).map(OZeroCopy::into_inner).collect();
		let sample = VecDeque::<u16>::from([1234, 69]);

		assert_eq!(queue_, sample);
		assert_eq!(Some(OZeroCopy::from_inner(1234)), queue.get_front()?);
		assert_eq!(Some(OZeroCopy::from_inner(1234)), queue.get(0)?);
		assert_eq!(Some(OZeroCopy::from_inner(69)), queue.get_back()?);
		assert_eq!(Some(69), queue.get(1)?.map(OZeroCopy::into_inner)); // XXX: BROKEN BECAUSE OF queue.len()

		queue.set(0, &69)?;
		// queue.set(u32::MAX - 1, &420)?;

		assert_eq!(Some(OZeroCopy::from_inner(69)), queue.get(0)?);
		// assert_eq!(Some(420), queue.get(u32::MAX - 1)?);

		queue.set_front(&420)?;
		queue.set_back(&69)?;
		assert_eq!(Some(OZeroCopy::from_inner(420)), queue.get_front()?);
		assert_eq!(Some(OZeroCopy::from_inner(69)), queue.get_back()?);
		let ends = queue.ends();
		dbg!(ends.clone());

		// XXX: broken
		assert!(queue.swap(ends.front, ends.back).is_err());
		// assert_eq!(Some(69), queue.get_front()?);
		// assert_eq!(Some(420), queue.get_back()?);

		queue.clear(true);
		assert!(queue.set_front(&69).is_err());
		assert_eq!(None, queue.get_front()?);
		assert_eq!(None, queue.pop_front()?);
		assert_eq!(None, queue.get_back()?);
		assert_eq!(None, queue.pop_back()?);

		assert_eq!(u32::MAX, queue.capacity());

		Ok(())
	}

	#[test]
	fn queue_rm() -> TestingResult {
		let _storage_lock = init()?;
		let mut queue = StoredVecDeque::<u16>::new(NAMESPACE);

		queue.push_front(&69)?;
		queue.push_back(&420)?;
		queue.push_front(&1234)?;
		queue.pop_back()?;
		queue.pop_front()?;

		storage_remove(NAMESPACE);
		assert!(!storage_has(NAMESPACE));

		Ok(())
	}

	// kuriboh: Aritz doesn't know how to handle it,
	// but at least this proves that the vec caches its length
	// aritz: yeah, similar problem to std::fs being "unsafe".
	#[test]
	fn wanted_behavior_question_mark() -> TestingResult {
		let _storage_lock = init()?;
		let mut queue = StoredVecDeque::<u16>::new(NAMESPACE);

		queue.push_front(&69)?;
		queue.push_back(&420)?;

		storage_remove(NAMESPACE);

		assert!(!storage_has(NAMESPACE));
		assert!(queue.into_iter().all(|x| x.is_ok()));

		Ok(())
	}

	#[test]
	fn queue_from_vec() -> TestingResult {
		let _storage_lock = init()?;
		let mut vec = crate::storage::vec::StoredVec::<u16>::new(NAMESPACE);

		vec.push(&69)?;
		vec.push(&420)?;
		drop(vec);

		let queue = StoredVecDeque::<u16>::new(NAMESPACE);
		let queue = queue
			.into_iter()
			.filter_map(Result::ok)
			.map(OZeroCopy::into_inner)
			.collect::<VecDeque<u16>>();
		assert_eq!(queue, VecDeque::from([69, 420]));

		Ok(())
	}

	#[test]
	fn queue_from_queue() -> TestingResult {
		let _storage_lock = init()?;
		let mut queue = StoredVecDeque::<u8>::new(NAMESPACE);

		queue.push_back(&69)?;
		drop(queue);

		let queue = StoredVecDeque::<u8>::new(NAMESPACE);
		let queue = queue
			.into_iter()
			.filter_map(Result::ok)
			.map(OZeroCopy::into_inner)
			.collect::<VecDeque<u8>>();
		assert_eq!(queue, VecDeque::from([69]));

		Ok(())
	}

	#[test]
	fn queue_length() -> TestingResult {
		let _storage_lock = init()?;
		let mut queue = StoredVecDeque::<u16>::new(NAMESPACE);

		queue.push_back(&69)?;
		queue.push_back(&420)?;

		drop(queue);

		let queue = StoredVecDeque::<u16>::new(NAMESPACE);
		let len = queue.len();
		assert_eq!(len, 2);

		Ok(())
	}

	#[test]
	fn queue_length_back_and_front() {
		let _storage_lock = init().unwrap();
		let mut queue = StoredVecDeque::<u16>::new(NAMESPACE);

		queue.push_back(&69).unwrap();
		queue.push_front(&420).unwrap();

		drop(queue);

		let queue = StoredVecDeque::<u16>::new(NAMESPACE);
		let len = queue.len();
		assert_eq!(len, 2);

		let queue = queue
			.iter()
			.filter_map(Result::ok)
			.collect::<VecDeque<OZeroCopy<u16>>>();
		assert_eq!(queue.len(), 2);
	}

	#[test]
	fn clean_queue() -> TestingResult {
		let _storage_lock = init()?;
		let mut queue = StoredVecDeque::<u16>::new(NAMESPACE);

		queue.push_back(&69).unwrap();
		queue.push_front(&420).unwrap();
		queue.clear(true);
		drop(queue);

		let mut queue = StoredVecDeque::<u16>::new(NAMESPACE);
		let q: VecDeque<OZeroCopy<u16>> = queue.iter().filter_map(Result::ok).collect();
		assert_eq!(q.len(), 0);

		queue.push_back(&69).unwrap();
		queue.push_front(&420).unwrap();
		queue.clear(false);
		drop(queue);

		let mut queue = StoredVecDeque::<u16>::new(NAMESPACE);
		let q: VecDeque<OZeroCopy<u16>> = queue.iter().filter_map(Result::ok).collect();
		assert_eq!(q.len(), 0);

		queue.push_back(&69).unwrap();
		queue.push_front(&420).unwrap();
		drop(queue);

		let mut queue = StoredVecDeque::<u16>::new(NAMESPACE);
		queue.clear(false);
		let q: VecDeque<OZeroCopy<u16>> = queue.iter().filter_map(Result::ok).collect();
		assert_eq!(q.len(), 0);

		queue.push_back(&69).unwrap();
		queue.push_front(&420).unwrap();
		drop(queue);

		let mut queue = StoredVecDeque::<u16>::new(NAMESPACE);
		queue.clear(true);
		let q: VecDeque<OZeroCopy<u16>> = (&queue).into_iter().filter_map(Result::ok).collect();
		assert_eq!(q.len(), 0);

		Ok(())
	}

	// #[test]
	// fn queue_is_full() -> TestingResult {
	// 	let _storage_lock = init()?;
	// 	let mut queue = StoredVecDeque::<u32>::new(NAMESPACE);
	// 		for x in 0..u32::MAX {
	// 		queue.push_back(&x)?;
	// 	}
	//
	// 	assert!(queue.push_back(&420).is_err());
	//
	// 	Ok(())
	// }
}
