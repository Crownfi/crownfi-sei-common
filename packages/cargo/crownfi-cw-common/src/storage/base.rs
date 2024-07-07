use super::{IteratorDirection, OZeroCopy, SerializableItem, StorageIterId};
use cosmwasm_std::{StdError, Storage};

#[cfg(not(target_arch = "wasm32"))]
use cosmwasm_std::MemoryStorage;
#[cfg(not(target_arch = "wasm32"))]
use std::{
	collections::BTreeMap,
	sync::{atomic::AtomicU32, OnceLock, RwLock},
};

#[cfg(target_arch = "wasm32")]
use super::super::wasm_api;

pub fn storage_read_item<T: SerializableItem + Sized>(key: &[u8]) -> Result<Option<OZeroCopy<T>>, StdError> {
	if let Some(bytes) = storage_read(key) {
		Ok(Some(OZeroCopy::new(bytes)?))
	} else {
		Ok(None)
	}
}
pub fn storage_write_item<T: SerializableItem>(key: &[u8], value: &T) -> Result<(), StdError> {
	if let Some(bytes) = value.serialize_as_ref() {
		storage_write(key, bytes);
	} else {
		storage_write(key, &value.serialize_to_owned()?);
	}
	Ok(())
}
/// Currently the cosmwasm API doesn't actually have this, match on `storage_read` instead.
pub fn storage_has(key: &[u8]) -> bool {
	// The wasm_api doesn't have have anything like this at the time of writing
	storage_read(key).is_some()
}

#[cfg(target_arch = "wasm32")]
#[inline]
pub fn storage_read(key: &[u8]) -> Option<Vec<u8>> {
	wasm_api::storage::storage_read(key)
}
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn storage_write(key: &[u8], value: &[u8]) {
	wasm_api::storage::storage_write(key, value)
}
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn storage_remove(key: &[u8]) {
	wasm_api::storage::storage_remove(key)
}

#[cfg(target_arch = "wasm32")]
#[inline]
pub fn storage_iter_new(start: Option<&[u8]>, end: Option<&[u8]>, direction: IteratorDirection) -> StorageIterId {
	wasm_api::storage::storage_iter_new(start, end, direction)
}
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn storage_iter_next_pair(iter: StorageIterId) -> Option<(Vec<u8>, Vec<u8>)> {
	wasm_api::storage::storage_iter_next_pair(iter)
}
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn storage_iter_next_key(iter: StorageIterId) -> Option<Vec<u8>> {
	wasm_api::storage::storage_iter_next_key(iter)
}
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn storage_iter_next_value(iter: StorageIterId) -> Option<Vec<u8>> {
	wasm_api::storage::storage_iter_next_value(iter)
}

pub trait ThreadSafeStorage: Storage + Sync + Send {}
impl<T> ThreadSafeStorage for T where T: Storage + Sync + Send {}

#[cfg(target_arch = "wasm32")]
/// In a non-wasm32 environment, this sets the global `dyn Storage` for testing. By default this is the MemoryStorage.
/// When set, it returns the previously used global storage.
///
/// In a wasm32 environment, this function is the same as std::convert::identity and does nothing.
pub fn set_global_storage(storage: Box<dyn ThreadSafeStorage>) -> Box<dyn ThreadSafeStorage> {
	storage
}
#[cfg(not(target_arch = "wasm32"))]
static STORAGE_SEQ: AtomicU32 = AtomicU32::new(0);

#[cfg(not(target_arch = "wasm32"))]
fn global_storage() -> &'static RwLock<Box<dyn ThreadSafeStorage>> {
	static STORAGE: OnceLock<RwLock<Box<dyn ThreadSafeStorage>>> = OnceLock::new();
	STORAGE.get_or_init(|| RwLock::new(Box::new(MemoryStorage::new())))
}
#[cfg(not(target_arch = "wasm32"))]
/// In a non-wasm32 environment, this sets the global `dyn Storage` for testing. By default this is the MemoryStorage.
/// When set, it returns the previously used global storage.
///
/// In a wasm32 environment, this function is the same as std::convert::identity and does nothing.
pub fn set_global_storage(storage: Box<dyn ThreadSafeStorage>) -> Box<dyn ThreadSafeStorage> {
	use std::sync::atomic::Ordering;
	STORAGE_SEQ.fetch_add(1, Ordering::SeqCst);
	let mut writable_ref = global_storage().write().unwrap();
	std::mem::replace(&mut *writable_ref, storage)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn storage_read(key: &[u8]) -> Option<Vec<u8>> {
	global_storage().read().unwrap().get(key)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn storage_write(key: &[u8], value: &[u8]) {
	global_storage().write().unwrap().set(key, value)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn storage_remove(key: &[u8]) {
	global_storage().write().unwrap().remove(key)
}

#[cfg(not(target_arch = "wasm32"))]
static ITER_SEQ: AtomicU32 = AtomicU32::new(0);
#[cfg(not(target_arch = "wasm32"))]
struct IterState {
	next_record: (Vec<u8>, Vec<u8>),
	end: Option<Vec<u8>>,
	direction: IteratorDirection,
	storage_nonce: u32,
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_iter_states() -> &'static RwLock<BTreeMap<StorageIterId, IterState>> {
	static STORAGE: OnceLock<RwLock<BTreeMap<StorageIterId, IterState>>> = OnceLock::new();
	STORAGE.get_or_init(|| RwLock::new(BTreeMap::new()))
}
#[cfg(not(target_arch = "wasm32"))]
pub fn storage_iter_new(start: Option<&[u8]>, end: Option<&[u8]>, direction: IteratorDirection) -> StorageIterId {
	use std::sync::atomic::Ordering;

	let iter_id = StorageIterId(ITER_SEQ.fetch_add(1, Ordering::SeqCst));
	let storage: std::sync::RwLockReadGuard<Box<dyn ThreadSafeStorage>> = global_storage().read().unwrap();
	let first_record = storage.range(start, end, direction.into()).next();
	if let Some(next_record) = first_record {
		let mut iter_states = storage_iter_states().write().unwrap();
		iter_states.insert(
			iter_id,
			IterState {
				next_record,
				end: match direction {
					IteratorDirection::Ascending => end,
					IteratorDirection::Descending => start,
				}
				.map(|bytes| Vec::from(bytes)),
				direction,
				storage_nonce: STORAGE_SEQ.load(Ordering::SeqCst),
			},
		);
		iter_id
	} else {
		// It will always return None
		iter_id
	}
}
#[cfg(not(target_arch = "wasm32"))]
pub fn storage_iter_next_pair(iter: StorageIterId) -> Option<(Vec<u8>, Vec<u8>)> {
	use std::sync::atomic::Ordering;

	let mut iter_states = storage_iter_states().write().unwrap();
	let Some(iter_state) = iter_states.get_mut(&iter) else {
		return None;
	};
	if iter_state.storage_nonce < STORAGE_SEQ.load(Ordering::SeqCst) {
		iter_states.remove(&iter);
		return None;
	}

	let storage: std::sync::RwLockReadGuard<Box<dyn ThreadSafeStorage>> = global_storage().read().unwrap();
	let next_record = match iter_state.direction {
		IteratorDirection::Ascending => {
			let mut next_key = Vec::with_capacity(iter_state.next_record.0.len() + 1);
			next_key.extend_from_slice(&iter_state.next_record.0);
			next_key.push(0);

			storage
				.range(Some(&next_key), iter_state.end.as_deref(), iter_state.direction.into())
				.next()
		}
		IteratorDirection::Descending => storage
			.range(
				iter_state.end.as_deref(),
				Some(&iter_state.next_record.0),
				iter_state.direction.into(),
			)
			.next(),
	};
	if let Some(next_record) = next_record {
		Some(std::mem::replace(&mut iter_state.next_record, next_record))
	} else {
		Some(iter_states.remove(&iter).unwrap().next_record)
	}
}
#[cfg(not(target_arch = "wasm32"))]
pub fn storage_iter_next_key(iter: StorageIterId) -> Option<Vec<u8>> {
	storage_iter_next_pair(iter).map(|pair| pair.0)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn storage_iter_next_value(iter: StorageIterId) -> Option<Vec<u8>> {
	storage_iter_next_pair(iter).map(|pair| pair.1)
}

struct GlobalStoragePairIter {
	id: StorageIterId,
}
impl GlobalStoragePairIter {
	pub fn new(start: Option<&[u8]>, end: Option<&[u8]>, order: cosmwasm_std::Order) -> Self {
		Self {
			id: storage_iter_new(start, end, order.into()),
		}
	}
}
impl Iterator for GlobalStoragePairIter {
	type Item = (Vec<u8>, Vec<u8>);
	fn next(&mut self) -> Option<Self::Item> {
		storage_iter_next_pair(self.id)
	}
}

struct GlobalStorageIter {
	id: StorageIterId,
	value: bool,
}
impl GlobalStorageIter {
	pub fn new(start: Option<&[u8]>, end: Option<&[u8]>, order: cosmwasm_std::Order, value: bool) -> Self {
		Self {
			id: storage_iter_new(start, end, order.into()),
			value,
		}
	}
}
impl Iterator for GlobalStorageIter {
	type Item = Vec<u8>;
	fn next(&mut self) -> Option<Self::Item> {
		if self.value {
			storage_iter_next_value(self.id)
		} else {
			storage_iter_next_key(self.id)
		}
	}
}

/// A 0 size struct which implements cosmwasm_std::Storage, intended for use with testing.
pub struct GlobalStorage {}
impl Storage for GlobalStorage {
	fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
		storage_read(key)
	}
	fn range<'a>(
		&'a self,
		start: Option<&[u8]>,
		end: Option<&[u8]>,
		order: cosmwasm_std::Order,
	) -> Box<dyn Iterator<Item = cosmwasm_std::Record> + 'a> {
		Box::new(GlobalStoragePairIter::new(start, end, order))
	}
	fn range_keys<'a>(
		&'a self,
		start: Option<&[u8]>,
		end: Option<&[u8]>,
		order: cosmwasm_std::Order,
	) -> Box<dyn Iterator<Item = Vec<u8>> + 'a> {
		Box::new(GlobalStorageIter::new(start, end, order, false))
	}
	fn range_values<'a>(
		&'a self,
		start: Option<&[u8]>,
		end: Option<&[u8]>,
		order: cosmwasm_std::Order,
	) -> Box<dyn Iterator<Item = Vec<u8>> + 'a> {
		Box::new(GlobalStorageIter::new(start, end, order, true))
	}
	fn set(&mut self, key: &[u8], value: &[u8]) {
		storage_write(key, value)
	}
	fn remove(&mut self, key: &[u8]) {
		storage_remove(key)
	}
}
