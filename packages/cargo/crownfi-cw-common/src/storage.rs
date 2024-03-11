use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::Pod;
use cosmwasm_std::{StdError, Storage};
use std::{
	cell::{Ref, RefCell},
	rc::Rc,
};

pub mod item;
pub mod map;
pub mod queue;
pub mod vec;

pub fn concat_byte_array_pairs(a: &[u8], b: &[u8]) -> Vec<u8> {
	let mut result = Vec::with_capacity(a.len() + b.len());
	result.extend_from_slice(a);
	result.extend_from_slice(b);
	result
}

pub trait SerializableItem {
	fn serialize_to_owned(&self) -> Result<Vec<u8>, StdError>;
	fn serialize_as_ref(&self) -> Option<&[u8]>;
	fn deserialize(data: &[u8]) -> Result<Self, StdError>
	where
		Self: Sized;
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

			fn deserialize(data: &[u8]) -> Result<Self, StdError>
			where
				Self: Sized,
			{
				// If we're gonna clone anyway might as well use read_unaligned
				// I don't trust the storage api to give me bytes which don't align to 8 bytes anyway
				bytemuck::try_pod_read_unaligned(std::hint::black_box(data))
					.map_err(|err| StdError::parse_err(stringify!($data_type), err))
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

			fn serialize_as_ref(&self) -> Option<&[u8]> {
				None
			}

			fn deserialize(data: &[u8]) -> Result<Self, StdError> where Self: Sized {
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

			fn serialize_as_ref(&self) -> Option<&[u8]> {
				None
			}

			fn deserialize(data: &[u8]) -> Result<Self, StdError> where Self: Sized {
				Self::try_from_slice(data).map_err(|err| {
					StdError::parse_err(stringify!($data_type), err)
				})
			}
		}
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
	fn deserialize(data: &[u8]) -> Result<Self, StdError>
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

	fn deserialize(data: &[u8]) -> Result<Self, StdError>
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

#[derive(Clone)]
pub enum MaybeMutableStorage<'exec> {
	Immutable(&'exec dyn Storage),
	MutableShared(Rc<RefCell<&'exec mut dyn Storage>>),
}

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

	/// Returns None when key does not exist.
	/// Returns Some(Vec<u8>) when key exists.
	///
	/// Note: Support for differentiating between a non-existent key and a key with empty value
	/// is not great yet and might not be possible in all backends. But we're trying to get there.
	pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
		match self {
			MaybeMutableStorage::Immutable(storage) => storage.get(key),
			MaybeMutableStorage::MutableShared(storage) => storage.borrow().get(key),
		}
	}

	pub fn set(&self, key: &[u8], value: &[u8]) {
		match self {
			MaybeMutableStorage::Immutable(_) => {
				panic!("MaybeMutableStorage.set called on immutable storage")
			}
			MaybeMutableStorage::MutableShared(storage) => storage.borrow_mut().set(key, value),
		}
	}

	/// Removes a database entry at `key`.
	///
	/// The current interface does not allow to differentiate between a key that existed
	/// before and one that didn't exist. See https://github.com/CosmWasm/cosmwasm/issues/290
	pub fn remove(&self, key: &[u8]) {
		match self {
			MaybeMutableStorage::Immutable(_) => {
				panic!("MaybeMutableStorage.remove called on immutable storage")
			}
			MaybeMutableStorage::MutableShared(storage) => storage.borrow_mut().remove(key),
		}
	}

	/// Returns the lexicographically next key/value pair after the specified key.
	/// Used for implementing double-ended iterators and to allow multiple mutable iterators to exist at once.
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
				storage
					.borrow()
					.range(Some(&next_key), before, cosmwasm_std::Order::Ascending)
					.next()
			}
		}
	}

	/// Returns the lexicographically next key/value pair after the specified key.
	/// Used for implementing double-ended iterators and to allow multiple mutable iterators to exist at once.
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
