use super::memory::{ConstRegion, OwnedRegion};
use crate::{
	storage::{IteratorDirection, StorageIterId},
	wasm_api::memory::split_off_length_suffixed_bytes,
};
use std::ptr;

// This is heavily inspired from https://github.com/CosmWasm/cosmwasm/blob/336afd2e62f83ea632bb4b2f94488b228ca2e28a/packages/std/src/imports.rs
// Safty consideration references:
//   * https://github.com/CosmWasm/cosmwasm/blob/336afd2e62f83ea632bb4b2f94488b228ca2e28a/packages/vm/src/imports.rs
//   * https://github.com/CosmWasm/cosmwasm/blob/336afd2e62f83ea632bb4b2f94488b228ca2e28a/packages/vm/src/instance.rs

extern "C" {
	#[link_name = "db_read"]
	fn wasmvm_db_read(key: usize) -> *mut OwnedRegion;
	#[link_name = "db_write"]
	fn wasmvm_db_write(key: usize, value: usize);
	#[link_name = "db_remove"]
	fn wasmvm_db_remove(key: usize);

	// scan creates an iterator, which can be read by consecutive next() calls
	#[link_name = "db_scan"]
	fn wasmvm_db_scan(start_ptr: usize, end_ptr: usize, order: IteratorDirection) -> StorageIterId;
	#[link_name = "db_next"]
	fn wasmvm_db_next(iterator_id: StorageIterId) -> *mut OwnedRegion;
	#[cfg(feature = "cosmwasm_1_4")]
	#[link_name = "db_next_key"]
	fn wasmvm_db_next_key(iterator_id: StorageIterId) -> *mut OwnedRegion;
	#[cfg(feature = "cosmwasm_1_4")]
	#[link_name = "db_next_value"]
	fn wasmvm_db_next_value(iterator_id: StorageIterId) -> *mut OwnedRegion;
}
#[inline]
pub fn storage_read(key: &[u8]) -> Option<Vec<u8>> {
	let key_as_region = ConstRegion::new(key);
	// SAFTY:
	// * It is assumed that the key_as_region passed to wasmvm_db_read will not be edited or used beyond this call.
	// * It is assumed that a newly allocated valid region is passed on success.
	// The referenced sources for the cosmwasm VM confirm this.
	unsafe { OwnedRegion::from_ptr(wasmvm_db_read(ptr::from_ref(&key_as_region) as usize)) }.map(|region| region.into())
}
#[inline]
pub fn storage_write(key: &[u8], value: &[u8]) {
	if value.is_empty() {
		panic!("The storage backend cannot properly differentiate between empty values and non-existant values, use storage_remove instead.");
	}
	let key_as_region = ConstRegion::new(key);
	let value_as_region = ConstRegion::new(key);
	// SAFTY:
	// * It is assumed that the key_as_region passed to wasmvm_db_write will not be edited or used beyond this call.
	// * It is assumed that the value_as_region passed to wasmvm_db_write will not be edited or used beyond this call.
	// * It assumed that this will panic on error. e.g., too much data or writing in a read-only environment.
	unsafe {
		wasmvm_db_write(
			ptr::from_ref(&key_as_region) as usize,
			ptr::from_ref(&value_as_region) as usize,
		)
	};
}
#[inline]
pub fn storage_remove(key: &[u8]) {
	let key_as_region = ConstRegion::new(key);
	// SAFTY:
	// * It is assumed that the key_as_region passed to wasmvm_db_remove will not be edited or used beyond this call.
	// * It assumed that this will panic on error. e.g., too much data or writing in a read-only environment.
	unsafe { wasmvm_db_remove(ptr::from_ref(&key_as_region) as usize) };
}

#[inline]
pub fn storage_iter_new(start: Option<&[u8]>, end: Option<&[u8]>, direction: IteratorDirection) -> StorageIterId {
	let start_as_region = start.as_ref().map(|k| ConstRegion::new(k));
	let end_as_region = end.as_ref().map(|k| ConstRegion::new(k));
	// SAFTY:
	// * It is assumed that the passed regions will not be edited or used beyond this call.
	unsafe {
		wasmvm_db_scan(
			start_as_region
				.map(|region| ptr::from_ref(&region) as usize)
				.unwrap_or_default(),
			end_as_region
				.map(|region| ptr::from_ref(&region) as usize)
				.unwrap_or_default(),
			direction,
		)
	}
}

#[inline]
pub fn storage_iter_next_pair(iter: StorageIterId) -> Option<(Vec<u8>, Vec<u8>)> {
	let mut data_pair_bytes = Vec::from(
		// SAFTY:
		// * It is assumed that the runtime passes a newly allocated region for us to handle freely
		unsafe { OwnedRegion::from_ptr(wasmvm_db_next(iter))? },
	);
	let data_value = split_off_length_suffixed_bytes(&mut data_pair_bytes);
	let data_key = split_off_length_suffixed_bytes(&mut data_pair_bytes);
	// Throw away the rest of the data cuz that's what the default implementation effectively does.
	Some((data_key, data_value))
}

#[cfg(feature = "cosmwasm_1_4")]
#[inline]
pub fn storage_iter_next_key(iter: StorageIterId) -> Option<Vec<u8>> {
	// SAFTY:
	// * It is assumed that the runtime passes a newly allocated region for us to handle freely
	unsafe { OwnedRegion::from_ptr(wasmvm_db_next_key(iter)) }.map(|region| region.into())
}

#[cfg(not(feature = "cosmwasm_1_4"))]
#[inline]
pub fn storage_iter_next_key(iter: StorageIterId) -> Option<Vec<u8>> {
	storage_iter_next_pair(iter).map(|pair| pair.0)
}

#[cfg(feature = "cosmwasm_1_4")]
#[inline]
pub fn storage_iter_next_value(iter: StorageIterId) -> Option<Vec<u8>> {
	// SAFTY:
	// * It is assumed that the runtime passes a newly allocated region for us to handle freely
	unsafe { OwnedRegion::from_ptr(wasmvm_db_next_value(iter)) }.map(|region| region.into())
}

#[cfg(not(feature = "cosmwasm_1_4"))]
#[inline]
pub fn storage_iter_next_value(iter: StorageIterId) -> Option<Vec<u8>> {
	storage_iter_next_pair(iter).map(|pair| pair.1)
}
