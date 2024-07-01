use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};
use cosmwasm_std::{StdError, Storage};
use std::{
	cell::{Ref, RefCell},
	num::NonZeroUsize,
	ops::{Deref, DerefMut},
	rc::Rc,
};

use self::base::{storage_iter_new, storage_iter_next_key, storage_iter_next_pair};

pub mod base;
pub mod item;
pub mod map;
pub mod set;
pub mod queue;
pub mod vec;

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u32)]
pub enum IteratorDirection {
	Ascending = 1,
	Descending = 2,
}
impl From<cosmwasm_std::Order> for IteratorDirection {
	fn from(value: cosmwasm_std::Order) -> Self {
		match value {
			cosmwasm_std::Order::Ascending => Self::Ascending,
			cosmwasm_std::Order::Descending => Self::Descending,
		}
	}
}
impl From<IteratorDirection> for cosmwasm_std::Order {
	fn from(value: IteratorDirection) -> Self {
		match value {
			IteratorDirection::Ascending => Self::Ascending,
			IteratorDirection::Descending => Self::Descending,
		}
	}
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Zeroable, Pod)]
#[repr(C)]
pub struct StorageIterId(u32);

pub fn concat_byte_array_pairs(a: &[u8], b: &[u8]) -> Vec<u8> {
	let mut result = Vec::with_capacity(a.len() + b.len());
	result.extend_from_slice(a);
	result.extend_from_slice(b);
	result
}

/// Opportunistically zero-copy-deserialized object.
///
/// This allows for a SerializableItem to be "parsed" with near-zero gas costs.
///
/// This object exists because while ideally we would convert a Vec<u8> into a Box<T>, the issue is that one of Rust's
/// guarantees is that the alignment of a block of data allocated on the heap does not change, or rather, calls to
/// `alloc` and `dealloc` will be provided the same size and layout.
#[derive(Debug, Default)]
//pub struct OZeroCopy<T: Sized + SerializableItem>(OZeroCopyType<T>);
 
pub struct OZeroCopy<T: Sized + SerializableItem>{
	inner_box: Box<T>,
	original_vec_capacity: usize
}
impl<T: Sized + SerializableItem> OZeroCopy<T> {
	pub fn new(bytes: Vec<u8>) -> Result<Self, StdError> {
		// SAFTY:
		// * It is assumed that the `deserialize_vec_into_box` implementation is valid
		// * The Box is converted back into a Vec when self is dropped.
		unsafe {
			match T::deserialize_vec_into_box(bytes) {
				Err(bytes) => {
					Ok(
						Self {
							inner_box: Box::new(T::deserialize_to_owned(bytes.as_ref())?),
							original_vec_capacity: 0
						}
					)
				}
				Ok((inner_box, original_vec_capacity)) => {
					Ok(
						Self { inner_box, original_vec_capacity }
					)
				}
			}
		}
	}
	pub fn from_inner(value: T) -> Self {
		Self {
			inner_box: Box::new(value),
			original_vec_capacity: 0
		}
	}
	pub fn into_inner(self) -> T where T: Clone {
		let inner_value = *self.inner_box.clone();
		inner_value
		// self should still be dropped as normal
	}
	pub fn try_into_bytes(self) -> Result<Vec<u8>, StdError> {
		if self.original_vec_capacity == 0 && std::mem::size_of::<T>() != 0 {
			let ptr = Box::into_raw(self.inner_box) as *mut u8;
			let length = std::mem::size_of::<T>();
			let capacity = self.original_vec_capacity;
			// SAFTY: original_vec_capacity should only be non-zero if this was originally created with a Vec<u8>
			Ok(unsafe {
				Vec::from_raw_parts(ptr, length, capacity)
			})
		} else {
			self.inner_box.serialize_to_owned()
		}
	}
}
impl<T: Sized + SerializableItem> AsRef<T> for OZeroCopy<T> {
	#[inline]
	fn as_ref(&self) -> &T {
		self.inner_box.as_ref()
	}
}
impl<T: Sized + SerializableItem> AsMut<T> for OZeroCopy<T> {
	#[inline]
	fn as_mut(&mut self) -> &mut T {
		self.inner_box.as_mut()
	}
}
impl<T: Sized + SerializableItem> Deref for OZeroCopy<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		self.inner_box.deref()
	}
}
impl<T: Sized + SerializableItem> DerefMut for OZeroCopy<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.inner_box.deref_mut()
	}
}
impl<T: SerializableItem + PartialEq> PartialEq for OZeroCopy<T> {
	fn eq(&self, other: &Self) -> bool {
		self.deref() == other.deref()
	}
}
impl<T: SerializableItem + PartialEq + Eq> Eq for OZeroCopy<T> {}
impl<T: SerializableItem + Clone> Clone for OZeroCopy<T> {
	fn clone(&self) -> Self {
		Self { inner_box: self.inner_box.clone(), original_vec_capacity: 0 }
	}
}
impl<T: SerializableItem> Drop for OZeroCopy<T> {
	fn drop(&mut self) {
		if self.original_vec_capacity == 0 && std::mem::size_of::<T>() != 0 {
			let ptr = Box::into_raw(std::mem::take(&mut self.inner_box)) as *mut u8;
			let length = std::mem::size_of::<T>();
			let capacity = self.original_vec_capacity;
			// SAFTY: original_vec_capacity should only be non-zero if this was originally created with a Vec<u8>
			drop(
				unsafe {
					Vec::from_raw_parts(ptr, length, capacity)
				}
			);
		}
	}
}

pub trait SerializableItem {
	fn serialize_to_owned(&self) -> Result<Vec<u8>, StdError>;
	#[inline]
	fn serialize_as_ref(&self) -> Option<&[u8]> {
		None
	}
	#[deprecated(note = "please use `deserialize_to_owned` instead")]
	fn deserialize(data: &[u8]) -> Result<Self, StdError>
	where
		Self: Sized,
	{
		Self::deserialize_to_owned(data)
	}
	fn deserialize_to_owned(data: &[u8]) -> Result<Self, StdError>
	where
		Self: Sized;
	#[allow(unused)]
	#[inline]
	fn deserialize_as_ref(data: &[u8]) -> Option<&Self>
	where
		Self: Sized,
	{
		None
	}
	#[allow(unused)]
	#[inline]
	fn deserialize_as_ref_mut(data: &mut [u8]) -> Option<&mut Self>
	where
		Self: Sized,
	{
		None
	}

	/// # SAFTY
	/// * `deserialize_as_ref_mut` implementation must only be `Some` if:
	///   * The slice of `data.as_mut()` is aligned for `Self`
	///   * The length of `data` is equal to the size of `Self`
	/// * You must convert the `Box<Self>` it back into a `Vec<u8>` before deallocating
	unsafe fn deserialize_vec_into_box(mut data: Vec<u8>) -> Result<(Box<Self>, usize), Vec<u8>>
	where
		Self: Sized
	{
		// Check if a cast is valid
		if Self::deserialize_as_ref_mut(data.as_mut()).is_none() {
			return Err(data);
		}
		let ptr = data.as_mut_ptr() as *mut Self;
		let capacity = data.capacity();
		std::mem::forget(data);
		// SAFTY: Relies on the `deserialize_as_ref_mut` implementation fulfilling the requirements noted above
		unsafe {
			Ok((Box::from_raw(ptr), capacity))
		}
	}
}

#[macro_export]
macro_rules! impl_serializable_as_ref {
	( $data_type:ident ) => {
		impl SerializableItem for $data_type {
			#[inline]
			fn serialize_to_owned(&self) -> Result<Vec<u8>, StdError> {
				// black_box is used to be sure that the optimizer won't throw away changes to the struct
				Ok(bytemuck::bytes_of(std::hint::black_box(self)).into())
			}
			#[inline]
			fn serialize_as_ref(&self) -> Option<&[u8]> {
				// ditto use of black_box as above
				Some(bytemuck::bytes_of(std::hint::black_box(self)))
			}
			#[inline]
			fn deserialize_to_owned(data: &[u8]) -> Result<Self, StdError> {
				bytemuck::try_pod_read_unaligned(std::hint::black_box(data))
					.map_err(|err| StdError::parse_err(stringify!($data_type), err))
			}
			#[inline]
			fn deserialize_as_ref(data: &[u8]) -> Option<&Self> {
				bytemuck::try_from_bytes(data).ok()
			}
			#[inline]
			fn deserialize_as_ref_mut(data: &mut [u8]) -> Option<&mut Self> {
				bytemuck::try_from_bytes_mut(data).ok()
			}
		}
	};
}
#[macro_export]
macro_rules! impl_serializable_borsh {
	( $data_type:ty ) => {
		impl SerializableItem for $data_type {
			fn serialize_to_owned(&self) -> Result<Vec<u8>, StdError> {
				let mut result = Vec::new();
				self.serialize(&mut result).map_err(|err| {
					StdError::serialize_err(stringify!($data_type), err)
				})?;
				Ok(result)
			}
			fn deserialize_to_owned(data: &[u8]) -> Result<Self, StdError> where Self: Sized {
				Self::try_from_slice(data).map_err(|err| {
					StdError::parse_err(stringify!($data_type), err)
				})
			}
		}
	};
	( $data_type:ty, $($generic:ident),+ ) => {
		impl<$($generic),*> SerializableItem for $data_type where $($generic: BorshDeserialize + BorshSerialize),* {
			fn serialize_to_owned(&self) -> Result<Vec<u8>, StdError> {
				let mut result = Vec::new();
				self.serialize(&mut result).map_err(|err| {
					StdError::serialize_err(stringify!($data_type), err)
				})?;
				Ok(result)
			}
			fn deserialize_to_owned(data: &[u8]) -> Result<Self, StdError> where Self: Sized {
				Self::try_from_slice(data).map_err(|err| {
					StdError::parse_err(stringify!($data_type), err)
				})
			}
		}
	}
}

// I'd love it if double-ended iterators where just exposed...
struct StorageIteratorCommon {
	ascending_id: Option<StorageIterId>,
	ascending_key: Option<Rc<[u8]>>,
	descending_id: Option<StorageIterId>,
	descending_key: Option<Rc<[u8]>>,
}
impl StorageIteratorCommon {
	fn new(start: Option<&[u8]>, end: Option<&[u8]>) -> Self {
		Self {
			ascending_id: None,
			ascending_key: start.map(|bytes| bytes.into()),
			descending_id: None,
			descending_key: end.map(|bytes| bytes.into()),
		}
	}
	fn ascending_id(&mut self) -> StorageIterId {
		*self.ascending_id.get_or_insert_with(|| {
			storage_iter_new(
				self.ascending_key.as_deref(),
				self.descending_key.as_deref(),
				IteratorDirection::Ascending,
			)
		})
	}
	fn descending_id(&mut self) -> StorageIterId {
		*self.descending_id.get_or_insert_with(|| {
			storage_iter_new(
				self.ascending_key.as_deref(),
				self.descending_key.as_deref(),
				IteratorDirection::Descending,
			)
		})
	}
	// Forward implementation
	fn next_pair(&mut self) -> Option<(Rc<[u8]>, Vec<u8>)> {
		let ascending_id = self.ascending_id();
		let (data_key, data_value) = storage_iter_next_pair(ascending_id)?;
		let data_key: Rc<[u8]> = data_key.into();
		if self.descending_id.is_some() {
			if data_key
				>= *self
					.descending_key
					.as_ref()
					.expect("descending_key should be defined if descending_id is")
			{
				return None;
			}
		}
		self.ascending_key = Some(data_key.clone());
		Some((data_key, data_value))
	}
	fn next_key(&mut self) -> Option<Rc<[u8]>> {
		let ascending_id = self.ascending_id();
		let data_key: Rc<[u8]> = storage_iter_next_key(ascending_id)?.into();
		if self.descending_id.is_some() {
			if data_key
				>= *self
					.descending_key
					.as_ref()
					.expect("descending_key should be defined if descending_id is")
			{
				return None;
			}
		}
		self.ascending_key = Some(data_key.clone());
		Some(data_key)
	}
	fn next_value(&mut self) -> Option<Vec<u8>> {
		self.next_pair().map(|pair| pair.1)
	}
	fn advance_by(&mut self, mut n: usize) -> Result<(), NonZeroUsize> {
		let ascending_id = self.ascending_id();
		while n > 0 {
			let next_ascending_key = storage_iter_next_key(ascending_id).map(|bytes| bytes.into());
			if next_ascending_key.is_none()
				|| next_ascending_key
					.as_deref()
					.zip(self.descending_key.as_deref())
					.is_some_and(|(nak, dk)| nak >= dk)
			{
				return Err(
					// SAFTY: This is only reachable if n > 0
					unsafe { NonZeroUsize::new_unchecked(n) },
				);
			}
			self.ascending_key = next_ascending_key;
			n -= 1;
		}
		Ok(())
	}
	// Backward implementation
	fn next_pair_back(&mut self) -> Option<(Rc<[u8]>, Vec<u8>)> {
		let descending_id = self.descending_id();
		let (data_key, data_value) = storage_iter_next_pair(descending_id)?;
		let data_key: Rc<[u8]> = data_key.into();
		if self.ascending_id.is_some() {
			if data_key
				<= *self
					.ascending_key
					.as_ref()
					.expect("ascending_key should be defined if ascending_id is")
			{
				return None;
			}
		}
		self.descending_key = Some(data_key.clone());
		Some((data_key, data_value))
	}
	fn next_key_back(&mut self) -> Option<Rc<[u8]>> {
		let descending_id = self.descending_id();
		let data_key: Rc<[u8]> = storage_iter_next_key(descending_id)?.into();
		if self.ascending_id.is_some() {
			if data_key
				<= *self
					.ascending_key
					.as_ref()
					.expect("ascending_key should be defined if ascending_id is")
			{
				return None;
			}
		}
		self.descending_key = Some(data_key.clone());
		Some(data_key)
	}
	fn next_value_back(&mut self) -> Option<Vec<u8>> {
		self.next_pair_back().map(|pair| pair.1)
	}
	fn advance_back_by(&mut self, mut n: usize) -> Result<(), NonZeroUsize> {
		let descending_id = self.descending_id();
		while n > 0 {
			let next_descending_key = storage_iter_next_key(descending_id).map(|bytes| bytes.into());
			if next_descending_key.is_none()
				|| next_descending_key
					.as_deref()
					.zip(self.ascending_key.as_deref())
					.is_some_and(|(ndk, ak)| ndk < ak || (self.ascending_id.is_some() && ndk == ak))
			{
				return Err(
					// SAFTY: This is only reachable if n > 0
					unsafe { NonZeroUsize::new_unchecked(n) },
				);
			}
			self.descending_key = next_descending_key;
			n -= 1;
		}
		Ok(())
	}
}

pub struct StoragePairIterator(StorageIteratorCommon);
impl StoragePairIterator {
	pub fn new(start: Option<&[u8]>, end: Option<&[u8]>) -> Self {
		Self(StorageIteratorCommon::new(start, end))
	}
}
impl Iterator for StoragePairIterator {
	type Item = (Rc<[u8]>, Vec<u8>);
	fn next(&mut self) -> Option<Self::Item> {
		self.0.next_pair()
	}
	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.0.advance_by(n).ok()?;
		self.next()
	}
	// TODO: Impl advance_by when stable
}
impl DoubleEndedIterator for StoragePairIterator {
	fn next_back(&mut self) -> Option<Self::Item> {
		self.0.next_pair_back()
	}
	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		self.0.advance_back_by(n).ok()?;
		self.next()
	}
	// TODO: Impl advance_back_by when stable
}
impl From<StorageKeyIterator> for StoragePairIterator {
	fn from(value: StorageKeyIterator) -> Self {
		Self(value.0)
	}
}
impl From<StorageValueIterator> for StoragePairIterator {
	fn from(value: StorageValueIterator) -> Self {
		Self(value.0)
	}
}

pub struct StorageKeyIterator(StorageIteratorCommon);
impl StorageKeyIterator {
	pub fn new(start: Option<&[u8]>, end: Option<&[u8]>) -> Self {
		Self(StorageIteratorCommon::new(start, end))
	}
}
impl Iterator for StorageKeyIterator {
	type Item = Rc<[u8]>;
	fn next(&mut self) -> Option<Self::Item> {
		self.0.next_key()
	}
	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.0.advance_by(n).ok()?;
		self.next()
	}
	// TODO: Impl advance_by when stable
}
impl DoubleEndedIterator for StorageKeyIterator {
	fn next_back(&mut self) -> Option<Self::Item> {
		self.0.next_key_back()
	}
	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		self.0.advance_back_by(n).ok()?;
		self.next()
	}
	// TODO: Impl advance_back_by when stable
}
impl From<StoragePairIterator> for StorageKeyIterator {
	fn from(value: StoragePairIterator) -> Self {
		Self(value.0)
	}
}
impl From<StorageValueIterator> for StorageKeyIterator {
	fn from(value: StorageValueIterator) -> Self {
		Self(value.0)
	}
}

pub struct StorageValueIterator(StorageIteratorCommon);
impl StorageValueIterator {
	pub fn new(start: Option<&[u8]>, end: Option<&[u8]>) -> Self {
		Self(StorageIteratorCommon::new(start, end))
	}
}
impl Iterator for StorageValueIterator {
	type Item = Vec<u8>;
	fn next(&mut self) -> Option<Self::Item> {
		self.0.next_value()
	}
	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.0.advance_by(n).ok()?;
		self.next()
	}
	// TODO: Impl advance_by when stable
}
impl DoubleEndedIterator for StorageValueIterator {
	fn next_back(&mut self) -> Option<Self::Item> {
		self.0.next_value_back()
	}
	fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
		self.0.advance_back_by(n).ok()?;
		self.next()
	}
	// TODO: Impl advance_back_by when stable
}
impl From<StoragePairIterator> for StorageValueIterator {
	fn from(value: StoragePairIterator) -> Self {
		Self(value.0)
	}
}
impl From<StorageKeyIterator> for StorageValueIterator {
	fn from(value: StorageKeyIterator) -> Self {
		Self(value.0)
	}
}

impl_serializable_as_ref!(u8);
impl_serializable_as_ref!(i8);
impl_serializable_as_ref!(u16);
impl_serializable_as_ref!(i16);
impl_serializable_as_ref!(u32);
impl_serializable_as_ref!(i32);
impl_serializable_as_ref!(u64);
impl_serializable_as_ref!(i64);
impl_serializable_as_ref!(usize);
impl_serializable_as_ref!(isize);
impl_serializable_as_ref!(u128);
impl_serializable_as_ref!(i128);
impl_serializable_as_ref!(f32);
impl_serializable_as_ref!(f64);
impl_serializable_borsh!(bool);
impl_serializable_borsh!(String);
impl_serializable_borsh!(Vec<T>, T);

// Bytemuck doesn't have blanket impls for tuples, but borsh does! Which allows us to be lazy when defining map keys
impl_serializable_borsh!((T0, T1), T0, T1);
impl_serializable_borsh!((T0, T1, T2, T3), T0, T1, T2, T3);
impl_serializable_borsh!((T0, T1, T2, T3, T4), T0, T1, T2, T3, T4);
impl_serializable_borsh!((T0, T1, T2, T3, T4, T5), T0, T1, T2, T3, T4, T5);
impl_serializable_borsh!((T0, T1, T2, T3, T4, T5, T6), T0, T1, T2, T3, T4, T5, T6);
impl_serializable_borsh!((T0, T1, T2, T3, T4, T5, T6, T7), T0, T1, T2, T3, T4, T5, T6, T7);
impl_serializable_borsh!((T0, T1, T2, T3, T4, T5, T6, T7, T8), T0, T1, T2, T3, T4, T5, T6, T7, T8);
impl_serializable_borsh!(
	(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9),
	T0,
	T1,
	T2,
	T3,
	T4,
	T5,
	T6,
	T7,
	T8,
	T9
);
impl_serializable_borsh!(
	(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10),
	T0,
	T1,
	T2,
	T3,
	T4,
	T5,
	T6,
	T7,
	T8,
	T9,
	T10
);
impl_serializable_borsh!(
	(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11),
	T0,
	T1,
	T2,
	T3,
	T4,
	T5,
	T6,
	T7,
	T8,
	T9,
	T10,
	T11
);
impl_serializable_borsh!(
	(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12),
	T0,
	T1,
	T2,
	T3,
	T4,
	T5,
	T6,
	T7,
	T8,
	T9,
	T10,
	T11,
	T12
);
impl_serializable_borsh!(
	(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13),
	T0,
	T1,
	T2,
	T3,
	T4,
	T5,
	T6,
	T7,
	T8,
	T9,
	T10,
	T11,
	T12,
	T13
);
impl_serializable_borsh!(
	(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14),
	T0,
	T1,
	T2,
	T3,
	T4,
	T5,
	T6,
	T7,
	T8,
	T9,
	T10,
	T11,
	T12,
	T13,
	T14
);
impl_serializable_borsh!(
	(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15),
	T0,
	T1,
	T2,
	T3,
	T4,
	T5,
	T6,
	T7,
	T8,
	T9,
	T10,
	T11,
	T12,
	T13,
	T14,
	T15
);
impl_serializable_borsh!(
	(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16),
	T0,
	T1,
	T2,
	T3,
	T4,
	T5,
	T6,
	T7,
	T8,
	T9,
	T10,
	T11,
	T12,
	T13,
	T14,
	T15,
	T16
);
impl_serializable_borsh!(
	(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17),
	T0,
	T1,
	T2,
	T3,
	T4,
	T5,
	T6,
	T7,
	T8,
	T9,
	T10,
	T11,
	T12,
	T13,
	T14,
	T15,
	T16,
	T17
);
impl_serializable_borsh!(
	(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18),
	T0,
	T1,
	T2,
	T3,
	T4,
	T5,
	T6,
	T7,
	T8,
	T9,
	T10,
	T11,
	T12,
	T13,
	T14,
	T15,
	T16,
	T17,
	T18
);
impl_serializable_borsh!(
	(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19),
	T0,
	T1,
	T2,
	T3,
	T4,
	T5,
	T6,
	T7,
	T8,
	T9,
	T10,
	T11,
	T12,
	T13,
	T14,
	T15,
	T16,
	T17,
	T18,
	T19
);

impl SerializableItem for () {
	fn serialize_to_owned(&self) -> Result<Vec<u8>, StdError> {
		Ok(Vec::new())
	}
	fn serialize_as_ref(&self) -> Option<&[u8]> {
		Some(b"")
	}
	fn deserialize_to_owned(data: &[u8]) -> Result<Self, StdError>
	where
		Self: Sized,
	{
		if data.len() != 0 {
			return Err(StdError::parse_err("()", "data was not empty"));
		}
		Ok(())
	}
}

impl<T, const N: usize> SerializableItem for [T; N]
where
	T: Pod,
{
	fn serialize_to_owned(&self) -> Result<Vec<u8>, StdError> {
		Ok(bytemuck::bytes_of(self).into())
	}

	fn serialize_as_ref(&self) -> Option<&[u8]> {
		Some(bytemuck::bytes_of(self))
	}

	fn deserialize_to_owned(data: &[u8]) -> Result<Self, StdError>
	where
		Self: Sized,
	{
		// If we're gonna clone anyway might as well use read_unaligned
		// I don't trust the storage api to give me bytes which don't align to 8 bytes anyway
		bytemuck::try_pod_read_unaligned(data).map_err(|err| StdError::parse_err("[T; N]", err))
	}
}

#[deprecated(note = "Juggling around dyn pointers to nothing is useless.")]
#[derive(Clone)]
pub enum MaybeMutableStorage<'exec> {
	Immutable(&'exec dyn Storage),
	MutableShared(Rc<RefCell<&'exec mut dyn Storage>>),
}

#[allow(deprecated)]
impl<'exec> MaybeMutableStorage<'exec> {
	pub fn new_immutable(storage: &'exec dyn Storage) -> Self {
		Self::Immutable(storage)
	}

	pub fn new_mutable_shared(storage: Rc<RefCell<&'exec mut dyn Storage>>) -> Self {
		Self::MutableShared(storage)
	}

	pub fn get_immutable(&self) -> Option<&'exec dyn Storage> {
		match self {
			MaybeMutableStorage::Immutable(storage) => Some(*storage),
			MaybeMutableStorage::MutableShared(_) => None,
		}
	}

	pub fn get_mutable_shared(&self) -> Option<Rc<RefCell<&'exec mut dyn Storage>>> {
		match self {
			MaybeMutableStorage::Immutable(_) => None,
			MaybeMutableStorage::MutableShared(storage) => Some(storage.clone()),
		}
	}

	#[deprecated(note = "Use base::storage_read instead")]
	pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
		match self {
			MaybeMutableStorage::Immutable(storage) => storage.get(key),
			MaybeMutableStorage::MutableShared(storage) => storage.borrow().get(key),
		}
	}

	#[deprecated(note = "Use base::storage_write instead")]
	pub fn set(&self, key: &[u8], value: &[u8]) {
		match self {
			MaybeMutableStorage::Immutable(_) => {
				panic!("MaybeMutableStorage.set called on immutable storage")
			}
			MaybeMutableStorage::MutableShared(storage) => storage.borrow_mut().set(key, value),
		}
	}

	#[deprecated(note = "Use base::storage_remove instead")]
	pub fn remove(&self, key: &[u8]) {
		match self {
			MaybeMutableStorage::Immutable(_) => {
				panic!("MaybeMutableStorage.remove called on immutable storage")
			}
			MaybeMutableStorage::MutableShared(storage) => storage.borrow_mut().remove(key),
		}
	}

	#[deprecated(note = "Use one of the iterators provided instead")]
	pub fn next_record(&self, key: &[u8], before: Option<&[u8]>) -> Option<(Vec<u8>, Vec<u8>)> {
		let mut next_key = Vec::with_capacity(key.len() + 1);
		next_key.extend_from_slice(key);
		next_key.push(0);
		match self {
			MaybeMutableStorage::Immutable(storage) => {
				// I have no idea why this behaviour isn't just already exposed.
				storage
					.range(
						Some(&next_key),
						before,
						// It seems like this only exists because the cosmos team didn't know `DoubleEndedIterator` existed
						cosmwasm_std::Order::Ascending,
					)
					.next()
			}
			MaybeMutableStorage::MutableShared(storage) => {
				// Implementing iterators on top of this is far from ideal, but the core issue is that unlike solana,
				// where I'm given a byte array which I can parition however I want, I have to juggle around a single
				// mutable reference. Which means there's a lot more BS to go through in order to, for example,
				// immutabily iterate over one mapping while mutabily iterating over another.
				// I miss being able to RefMut::map_split(account_info.data.borrow_mut(), ...)
				// --UPDATE--
				// Turns out I don't _have_ to juggle around a single mutable reference, so this can be improved upon
				// in the future.
				storage
					.borrow()
					.range(Some(&next_key), before, cosmwasm_std::Order::Ascending)
					.next()
			}
		}
	}

	#[deprecated(note = "Use one of the iterators provided instead")]
	pub fn prev_record(&self, key: &[u8], after: Option<&[u8]>) -> Option<(Vec<u8>, Vec<u8>)> {
		// ditto statements as above
		match self {
			MaybeMutableStorage::Immutable(storage) => {
				storage.range(after, Some(key), cosmwasm_std::Order::Descending).next()
			}
			MaybeMutableStorage::MutableShared(storage) => storage
				.borrow()
				.range(after, Some(key), cosmwasm_std::Order::Descending)
				.next(),
		}
	}
}

pub enum MaybeMutableStorageRef<'a> {
	Immutable(&'a dyn Storage),
	MutableShared(Ref<'a, &'a mut dyn Storage>),
}

#[cfg(test)]
pub mod testing_common {
	use cosmwasm_std::MemoryStorage;

	use super::base::set_global_storage;

	pub type TestingResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
	pub const NAMESPACE: &[u8] = b"testing";

	static MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

	pub fn init<'a>() -> TestingResult<std::sync::MutexGuard<'a, ()>> {
		let lock = MUTEX.lock()?;
		set_global_storage(Box::new(MemoryStorage::new()));

		Ok(lock)
	}
}
