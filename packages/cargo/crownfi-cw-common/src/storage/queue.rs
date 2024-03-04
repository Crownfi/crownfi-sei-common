use bytemuck::{Pod, Zeroable};
use cosmwasm_std::{StdError, StdResult};

use super::{map::StoredMap, vec::IndexedStoredItemIter, MaybeMutableStorage, SerializableItem};

#[derive(Debug, Default, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct QueueEnds {
	pub front: u32,
	pub back: u32,
}

pub struct StoredVecDeque<'exec, V: SerializableItem> {
	namespace: &'static [u8],
	storage: MaybeMutableStorage<'exec>,
	map: StoredMap<'exec, u32, V>,
	ends: QueueEnds,
}
impl<'exec, V: SerializableItem> StoredVecDeque<'exec, V> {
	pub fn new(namespace: &'static [u8], storage: MaybeMutableStorage<'exec>) -> Self {
		let ends = storage
			.get(&namespace)
			.map(|data| {
				if data.len() == 4 {
					// Vec that has been "upgraded" to a queue
					return QueueEnds {
						front: 0,
						back: u32::from_le_bytes(data.try_into().unwrap()),
					};
				}
				if data.len() < std::mem::size_of::<QueueEnds>() {
					return QueueEnds::default();
				}
				// Doing this way because I don't trust the alignment of the data returned from storage
				QueueEnds {
					front: u32::from_le_bytes(data[0..4].try_into().unwrap()),
					back: u32::from_le_bytes(data[4..].try_into().unwrap()),
				}
			})
			.unwrap_or_default();
		Self {
			namespace,
			storage: storage.clone(),
			map: StoredMap::new(namespace, storage),
			ends,
		}
	}
	#[inline]
	fn set_ends(&mut self, value: QueueEnds) {
		self.ends = value;
		#[cfg(target_endian = "big")]
		let value = QueueEnds {
			front: value.front.swap_bytes(),
			back: value.back.swap_bytes(),
		};
		self.storage.set(self.namespace, &bytemuck::bytes_of(&value))
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
			u32::MAX - (ends.front - ends.back)
		}
	}
	pub fn get(&self, index: u32) -> StdResult<Option<V>> {
		if index >= self.len() {
			return Err(StdError::not_found("StoredVecDeque out of bounds"));
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
	pub fn iter(&self) -> IndexedStoredItemIter<'exec, V> {
		let ends = self.ends();
		IndexedStoredItemIter::new(self.namespace, self.storage.clone(), ends.front, ends.back)
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
	pub fn get_back(&self) -> StdResult<Option<V>> {
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
	pub fn pop_back(&mut self) -> StdResult<Option<V>> {
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
	pub fn get_front(&self) -> StdResult<Option<V>> {
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
	pub fn pop_front(&mut self) -> StdResult<Option<V>> {
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

impl<'exec, V: SerializableItem> IntoIterator for StoredVecDeque<'exec, V> {
	type Item = Result<V, StdError>;
	type IntoIter = IndexedStoredItemIter<'exec, V>;
	fn into_iter(self) -> Self::IntoIter {
		let ends = self.ends();
		IndexedStoredItemIter::new(self.namespace, self.storage, ends.front, ends.back)
	}
}
impl<'exec, V: SerializableItem> IntoIterator for &StoredVecDeque<'exec, V> {
	type Item = Result<V, StdError>;
	type IntoIter = IndexedStoredItemIter<'exec, V>;
	fn into_iter(self) -> Self::IntoIter {
		let ends = self.ends();
		IndexedStoredItemIter::new(self.namespace, self.storage.clone(), ends.front, ends.back)
	}
}
