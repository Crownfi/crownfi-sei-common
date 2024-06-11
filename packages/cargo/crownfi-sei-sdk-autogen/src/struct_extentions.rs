use schemars::schema::{Schema, SchemaObject, SingleOrVec};

pub(crate) trait SchemaStructExtentions {
	fn as_object(&self) -> Option<&SchemaObject>;
}

impl SchemaStructExtentions for Schema {
	fn as_object(&self) -> Option<&SchemaObject> {
		match self {
			Schema::Bool(_) => None,
			Schema::Object(o) => Some(o),
		}
	}
}

#[allow(dead_code)]
pub(crate) trait SingleOrVecStructExtentions<T> {
	/// Checks if this is a Single
	fn is_single(&self) -> bool;
	/// Checks if this is a Vec
	fn is_vec(&self) -> bool;
	/// Returns the object if this is a single, or array[0] if it isn't
	fn get_first(&self) -> Option<&T>;
	/// Returns the object _only_ if it's a single. Returns None if this is a vec whether or not it contains elements.
	fn as_single(&self) -> Option<&T>;
	/// Creates an iterator that only returns once (if it's a single) or the entire vec
	fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T>
	where
		T: 'a;
}

impl<T> SingleOrVecStructExtentions<T> for SingleOrVec<T> {
	#[inline]
	fn is_single(&self) -> bool {
		match self {
			SingleOrVec::Single(_) => true,
			SingleOrVec::Vec(_) => false,
		}
	}
	#[inline]
	fn is_vec(&self) -> bool {
		!self.is_single()
	}
	#[inline]
	fn get_first(&self) -> Option<&T> {
		match self {
			SingleOrVec::Single(val) => Some(val.as_ref()),
			SingleOrVec::Vec(val) => val.get(0),
		}
	}
	#[inline]
	fn as_single(&self) -> Option<&T> {
		match self {
			SingleOrVec::Single(val) => Some(val.as_ref()),
			SingleOrVec::Vec(_) => None,
		}
	}

	fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a T> + 'a>
	where
		T: 'a,
	{
		match self {
			SingleOrVec::Single(val) => Box::new(std::iter::once(&**val)),
			SingleOrVec::Vec(val) => Box::new(val.iter()),
		}
	}
}
