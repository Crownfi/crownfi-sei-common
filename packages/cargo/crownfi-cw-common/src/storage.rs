use std::{rc::Rc, cell::{RefCell, Ref}};



use cosmwasm_std::{StdError, Storage};
use serde::{de::DeserializeOwned as SerdeDeserializeOwned, Serialize as SerdeSerialize};

pub mod item;
pub mod map;
pub mod vec;
pub mod queue;

pub fn concat_byte_array_pairs(a: &[u8], b: &[u8]) -> Vec<u8> {
	let mut result = Vec::with_capacity(a.len() + b.len());
	result.extend_from_slice(a);
	result.extend_from_slice(b);
	result
}



#[derive(Debug, Clone)]
pub enum SerializationResult<'a> {
	Ref(&'a [u8]),
	Owned(Rc<[u8]>),
	OwnedMut(Vec<u8>)
}
impl Default for SerializationResult<'_> {
	fn default() -> Self {
		Self::Ref(b"")
	}
}
impl AsRef<[u8]> for SerializationResult<'_> {
	fn as_ref(&self) -> &[u8] {
		match self {
			SerializationResult::Ref(bytes) => bytes,
			SerializationResult::Owned(bytes) => bytes.as_ref(),
			SerializationResult::OwnedMut(bytes) => bytes.as_ref(),
		}
	}
}
impl Into<Vec<u8>> for SerializationResult<'_> {
	fn into(self) -> Vec<u8> {
		match self {
			SerializationResult::Ref(bytes) => bytes.to_vec(),
			SerializationResult::Owned(bytes) => bytes.to_vec(),
			SerializationResult::OwnedMut(bytes) => bytes,
		}
	}
}
impl Into<Rc<[u8]>> for SerializationResult<'_> {
	fn into(self) -> Rc<[u8]> {
		match self {
			SerializationResult::Ref(bytes) => bytes.into(),
			SerializationResult::Owned(bytes) => bytes,
			SerializationResult::OwnedMut(bytes) => bytes.into(),
		}
	}
}

pub trait SerializableItem {
	fn serialize(&self) -> Result<SerializationResult, StdError>;
	fn deserialize(data: &[u8]) -> Result<Self, StdError> where Self: Sized;
}

impl<T> SerializableItem for T where T: SerdeDeserializeOwned + SerdeSerialize {
	fn serialize(&self) -> Result<SerializationResult, StdError> {
		Ok(
			SerializationResult::OwnedMut(bincode::serialize(self).map_err(|ser_error| {
				StdError::SerializeErr {
					source_type: "bincode::serialize".into(),
					msg: ser_error.to_string()
				}
			})?)
		)
	}
	fn deserialize(data: &[u8]) -> Result<Self, StdError> where Self: Sized {
		bincode::deserialize(data).map_err(move |parse_error| {
			StdError::ParseErr {
				target_type: "bincode::deserialize".into(),
				msg: parse_error.to_string()
			}
		})
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
	MutableShared(Rc<RefCell<&'exec mut dyn Storage>>)
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
			},
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
			},
			MaybeMutableStorage::MutableShared(storage) => storage.borrow_mut().remove(key),
		}
	}
	/// Returns the lexicographically next key/value pair after the specified key.
	/// Used for implementing double-ended iterators and to allow multiple mutable iterators to exist at once.
	pub fn next_record(&self, key: &[u8], before: Option<&[u8]>) -> Option<(Vec<u8>, Vec<u8>)> {
		let next_key = lexicographic_next(key);
		match self {
			MaybeMutableStorage::Immutable(storage) => {
				// I have no idea why this behaviour isn't just already exposed.
				storage.range(
					Some(&next_key),
					before,
					// It seems like this only exists because the cosmos team didn't know `DoubleEndedIterator` existed
					cosmwasm_std::Order::Ascending
				).next()
			},
			MaybeMutableStorage::MutableShared(storage) => {
				// Implementing iterators on top of this is far from ideal, but the core issue is that unlike solana,
				// where I'm given a byte array which I can parition however I want, I have to juggle around a single
				// mutable reference. Which means there's a lot more BS to go through in order to, for example,
				// immutabily iterate over one mapping while mutabily iterating over another.
				// I miss being able to RefMut::map_split(account_info.data.borrow_mut(), ...)
				storage.borrow().range(
					Some(&next_key),
					before,
					cosmwasm_std::Order::Ascending
				).next()
			},
		}
	}

	/// Returns the lexicographically next key/value pair after the specified key.
	/// Used for implementing double-ended iterators and to allow multiple mutable iterators to exist at once.
	pub fn prev_record(&self, key: &[u8], after: Option<&[u8]>) -> Option<(Vec<u8>, Vec<u8>)> {
		// ditto statements as above
		match self {
			MaybeMutableStorage::Immutable(storage) => {
				storage.range(
					after,
					Some(key),
					cosmwasm_std::Order::Ascending
				).next()
			},
			MaybeMutableStorage::MutableShared(storage) => {
				storage.borrow().range(
					after,
					Some(key),
					cosmwasm_std::Order::Descending
				).next()
			},
		}
	}
}


pub enum MaybeMutableStorageRef<'a> {
	Immutable(&'a dyn Storage),
	MutableShared(Ref<'a, &'a mut dyn Storage>)
}

// Originally I wanted to do either bytemuck and/or borsh and have the option to switch between the 2 with borsh being
// the preferred, but 2 major things got in my way.
// 1. None of the cosmwasm_std types impl the Borsh traits (no surprise there, they seemingly don't care about perf)
// 2. Contextually switching to casting vs "actual" serialization is infeasible without being able to define multiple
//    blanket implementations. (Trait specialization might solve this) 

/*
pub fn serialize_borsh<T: BorshSerialize + Sized>(data: &T) -> Result<Vec<u8>, StdError> {
	let mut vec: Vec<u8> = Vec::with_capacity(size_of::<T>());
	data.serialize(&mut vec).map_err(|ser_error| {
		StdError::SerializeErr {
			source_type: "BorshSerialize".into(),
			msg: ser_error.to_string()
		}
	})?;
	Ok(vec)
}
pub fn deserialize_borsh<T: BorshDeserialize>(data: &[u8]) -> Result<T, StdError> {
	T::try_from_slice(&data).map_err(|parse_error| {
		StdError::ParseErr {
			target_type: "BorshDeserialize".into(),
			msg: parse_error.to_string()
		}
	})
}
pub fn serialize_cast<T: Pod>(data: &T) -> Result<&[u8], StdError> {
	Ok(bytes_of(data))
}
pub fn deserialize_cast<T: Pod>(data: &[u8]) -> Result<T, StdError> {
	Ok(
		*try_from_bytes(&data).map_err(|parse_error| {
			StdError::ParseErr {
				target_type: "Pod".into(),
				msg: parse_error.to_string()
			}
		})?
	)
}

#[macro_export]
macro_rules! impl_serializable_cast {
	( $data_type:ident ) => {
		impl SerializableItem for $data_type {
			fn serialize(&self) -> Result<SerializationResult, StdError> {
				Ok(SerializationResult::Ref(serialize_cast(self)?))
			}
			fn deserialize(data: &[u8]) -> Result<Self, StdError> where Self: Sized {
				deserialize_cast(data)
			}
		}
	}
}

#[macro_export]
macro_rules! impl_serializable_borsh {
	( $data_type:ident ) => {
		impl SerializableItem for $data_type {
			fn serialize(&self) -> Result<SerializationResult, StdError> {
				Ok(SerializationResult::OwnedMut(serialize_borsh(self)?))
			}
			fn deserialize(data: &[u8]) -> Result<Self, StdError> where Self: Sized {
				deserialize_borsh(data)
			}
		}
	}
}
impl_serializable_cast!(u8);
impl_serializable_cast!(i8);
impl_serializable_cast!(u16);
impl_serializable_cast!(i16);
impl_serializable_cast!(u32);
impl_serializable_cast!(i32);
impl_serializable_cast!(u64);
impl_serializable_cast!(i64);
impl_serializable_cast!(usize);
impl_serializable_cast!(isize);
impl_serializable_cast!(u128);
impl_serializable_cast!(i128);
impl_serializable_cast!(f32);
impl_serializable_cast!(f64);

impl_serializable_borsh!(String);

impl SerializableItem for Addr {
	fn serialize(&self) -> Result<SerializationResult, StdError> {
		Ok(SerializationResult::OwnedMut(serialize_borsh(&self.as_str())?))
	}
	fn deserialize(data: &[u8]) -> Result<Self, StdError> where Self: Sized {
		Ok(Addr::unchecked(deserialize_borsh::<String>(data)?))
	}
}

impl<T> SerializableItem for Vec<T> where T: BorshDeserialize + BorshSerialize {
	fn serialize(&self) -> Result<SerializationResult, StdError> {
		Ok(SerializationResult::OwnedMut(serialize_borsh(&self)?))
	}
	fn deserialize(data: &[u8]) -> Result<Self, StdError> where Self: Sized {
		deserialize_borsh(data)
	}
}
*/
