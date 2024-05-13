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

#[derive(Debug, Clone, PartialEq, Eq)]
enum OZeroCopyType<T: Sized + SerializableItem> {
	Copy(T),
	ZeroCopy(Vec<u8>),
}
impl<T> Default for OZeroCopyType<T>
where
	T: Default + Sized + SerializableItem,
{
	fn default() -> Self {
		OZeroCopyType::Copy(T::default())
	}
}

/// Opportunistically zero-copy-deserialized object.
///
/// This allows for a SerializableItem to be "parsed" with near-zero gas costs.
///
/// This object exists because while ideally we would convert a Vec<u8> into a Box<T>, the issue is that one of Rust's
/// guarantees is that the alignment of a block of data allocated on the heap does not change, or rather, calls to
/// `alloc` and `dealloc` will be provided the same size and layout.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OZeroCopy<T: Sized + SerializableItem>(OZeroCopyType<T>);
impl<T: Sized + SerializableItem> OZeroCopy<T> {
	pub fn new(bytes: Vec<u8>) -> Result<Self, StdError> {
		if T::deserialize_as_ref(&bytes).is_some() {
			Ok(OZeroCopy(OZeroCopyType::ZeroCopy(bytes)))
		} else {
			Ok(OZeroCopy(OZeroCopyType::Copy(T::deserialize_to_owned(&bytes)?)))
		}
	}
	pub fn from_inner(value: T) -> Self {
		Self(OZeroCopyType::Copy(value))
	}
	pub fn into_inner(self) -> T {
		match self.0 {
			OZeroCopyType::Copy(val) => val,
			OZeroCopyType::ZeroCopy(bytes) => T::deserialize_to_owned(&bytes)
				.expect("deserialize_to_owned should succeed if deserialize_as_ref did before"),
		}
	}
	pub fn try_into_bytes(self) -> Result<Vec<u8>, StdError> {
		Ok(match self.0 {
			OZeroCopyType::Copy(val) => val.serialize_to_owned()?,
			OZeroCopyType::ZeroCopy(bytes) => bytes,
		})
	}
}
impl<T: Sized + SerializableItem> AsRef<T> for OZeroCopy<T> {
	fn as_ref(&self) -> &T {
		match &self.0 {
			OZeroCopyType::Copy(val) => val,
			OZeroCopyType::ZeroCopy(bytes) => T::deserialize_as_ref(bytes).unwrap(),
		}
	}
}
impl<T: Sized + SerializableItem> AsMut<T> for OZeroCopy<T> {
	fn as_mut(&mut self) -> &mut T {
		match &mut self.0 {
			OZeroCopyType::Copy(val) => val,
			OZeroCopyType::ZeroCopy(bytes) => T::deserialize_as_ref_mut(bytes).unwrap(),
		}
	}
}
impl<T: Sized + SerializableItem> Deref for OZeroCopy<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		self.as_ref()
	}
}
impl<T: Sized + SerializableItem> DerefMut for OZeroCopy<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.as_mut()
	}
}

pub trait SerializableItem {
	fn serialize_to_owned(&self) -> Result<Vec<u8>, StdError>;
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
	fn deserialize_as_ref(data: &[u8]) -> Option<&Self>
	where
		Self: Sized,
	{
		None
	}
	#[allow(unused)]
	fn deserialize_as_ref_mut(data: &mut [u8]) -> Option<&mut Self>
	where
		Self: Sized,
	{
		None
	}
}

#[macro_export]
macro_rules! impl_serializable_as_ref {
	( $data_type:ident ) => {
		impl SerializableItem for $data_type {
			fn serialize_to_owned(&self) -> Result<Vec<u8>, StdError> {
				// black_box is used to be sure that the optimizer won't throw away changes to the struct
				Ok(bytemuck::bytes_of(std::hint::black_box(self)).into())
			}
			fn serialize_as_ref(&self) -> Option<&[u8]> {
				// ditto use of black_box as above
				Some(bytemuck::bytes_of(std::hint::black_box(self)))
			}
			fn deserialize_to_owned(data: &[u8]) -> Result<Self, StdError> {
				// If we're gonna clone anyway might as well use read_unaligned
				// I don't trust the storage api to give me bytes which don't align to 8 bytes anyway
				bytemuck::try_pod_read_unaligned(std::hint::black_box(data))
					.map_err(|err| StdError::parse_err(stringify!($data_type), err))
			}
			fn deserialize_as_ref(data: &[u8]) -> Option<&Self> {
				bytemuck::try_from_bytes(data).ok()
			}
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
		if let Some(descending_key) = &self.descending_key {
			if *descending_key >= data_key {
				return None;
			}
		}
		self.ascending_key = Some(data_key.clone());
		Some((data_key, data_value))
	}
	fn next_key(&mut self) -> Option<Rc<[u8]>> {
		let ascending_id = self.ascending_id();
		let data_key: Rc<[u8]> = storage_iter_next_key(ascending_id)?.into();
		if let Some(descending_key) = &self.descending_key {
			if *descending_key >= data_key {
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
		if let Some(ascending_key) = &self.ascending_key {
			if *ascending_key < data_key {
				return None;
			}
			// The interator had both next() and next_back() called.
			if self.ascending_id.is_some() && *ascending_key == data_key {
				return None;
			}
		}
		self.ascending_key = Some(data_key.clone());
		Some((data_key, data_value))
	}
	fn next_key_back(&mut self) -> Option<Rc<[u8]>> {
		let descending_id = self.descending_id();
		let data_key: Rc<[u8]> = storage_iter_next_key(descending_id)?.into();
		if let Some(ascending_key) = &self.ascending_key {
			if *ascending_key < data_key {
				return None;
			}
			// The interator had both next() and next_back() called.
			if *ascending_key == data_key && self.ascending_id.is_some() {
				return None;
			}
		}
		self.ascending_key = Some(data_key.clone());
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

pub(crate) fn lexicographic_next(bytes: &[u8]) -> Vec<u8> {
	let mut result = Vec::from(bytes);
	let mut add = true;
	for val in result.iter_mut().rev() {
		(*val, add) = val.overflowing_add(1);
		if !add {
			break;
		}
	}
	if add {
		// Turns out all the array values where u8::MAX, the only lexicographically next value is a larger array.
		// This also allows for an empty array.
		result.fill(u8::MAX);
		result.push(0);
	}
	result
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
