// This is heavily inspired from https://github.com/CosmWasm/cosmwasm/blob/336afd2e62f83ea632bb4b2f94488b228ca2e28a/packages/std/src/memory.rs

use std::marker::PhantomData;

use static_assertions::assert_eq_size;

// The cosmwasm api assumes that pointers are u32. Which should always be true for wasm32
assert_eq_size!(usize, u32);

/// Structures a Vec<u8> in a manner which the cosmwasm API expects.
#[repr(C)]
pub struct OwnedRegion {
	/// The beginning of the region expressed as bytes from the beginning of the linear memory
	offset: *mut u8,
	/// The number of bytes available in this region
	capacity: usize,
	/// The number of bytes used in this region
	length: usize
}
assert_eq_size!(OwnedRegion, (usize, usize, usize));

impl OwnedRegion {
	/// Creates an OwnedRegion from a ptr. Returns none if the ptr or the offset it references is null.
	/// 
	/// # Safety
	/// You may ***only*** use this on regions that were created by the runtime itself, and are expected to be later
	/// freed by user code. (i.e. areas where `consume_region`  was used in the original implementation) otherwise
	/// a double-free will happen and that's bad.
	pub unsafe fn from_ptr(ptr: *mut OwnedRegion) -> Option<OwnedRegion> {
		if ptr.is_null() {
			return None;
		}
		let region = Box::from_raw(ptr);
		if region.offset.is_null() {
			return None;
		}
		Some(
			OwnedRegion {
				..*region
			}
		)
	}
}
impl From<Vec<u8>> for OwnedRegion {
	fn from(mut value: Vec<u8>) -> Self {
		let offset = value.as_mut_ptr();
		let capacity = value.capacity();
		let length = value.len();
		std::mem::forget(value);
		Self {
			offset,
			capacity,
			length
		}
	}
}
impl From<OwnedRegion> for Vec<u8> {
	fn from(value: OwnedRegion) -> Self {
		// SAFTY: It is assumed that the only way to create an OwnedRegion is via the From<Vec<u8>> trait.
		unsafe {
			Vec::from_raw_parts(value.offset, value.length, value.capacity)
		}
	}
}
impl Drop for OwnedRegion {
	fn drop(&mut self) {
		// Run Vec's destructor
		drop(
			// SAFTY: It is assumed that the only way to create an OwnedRegion is via the From<Vec<u8>> trait.
			unsafe {
				Vec::from_raw_parts(self.offset, self.length, self.capacity)
			}
		)
	}
}

/// Allows you to pass a good ol' `&[u8]` to the cosmwasm API.
#[repr(C)]
pub struct ConstReigon<'a> {
	_lifetime: PhantomData<&'a ()>, // Representing the lifetime of the &[u8] this was constructed from
	offset: *const u8,
	capacity: usize,
	length: usize
}
assert_eq_size!(ConstReigon, (usize, usize, usize));

impl<'a> ConstReigon<'a> {
	pub fn new(bytes: &'a [u8]) -> Self {
		Self {
			_lifetime: PhantomData,
			offset: bytes.as_ptr(),
			capacity: bytes.len(),
			length: bytes.len()
		}
	}
}