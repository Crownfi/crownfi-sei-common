use std::{
	borrow::Cow,
	collections::{BTreeMap, BTreeSet},
	sync::Arc,
};

use convert_case::{Case, Casing};
use deunicode::deunicode_with_tofu_cow;
use lazy_regex::{
	Captures, Regex,
	{regex, regex::Replacer},
};
use schemars::schema::{InstanceType, ObjectValidation, Schema};

use crate::{
	error::SdkMakerError,
	struct_extentions::{SchemaStructExtentions, SingleOrVecStructExtentions},
};

/// makes the first letter uppercase
pub(crate) fn upper_first_ascii<'a>(input: &mut Cow<'a, str>) {
	let first_char = input.chars().next();
	if first_char.is_none() || first_char.is_some_and(|char| !char.is_ascii_lowercase()) {
		return;
	}
	let mut result = String::from(input.as_ref());
	// SAFTY: We confiremd that first_char.is_ascii_lowercase();
	unsafe {
		result.as_mut_vec()[0] &= 0b01011111;
	}
	*input = Cow::Owned(result);
}

fn mut_cow_str_replace_all<'a>(cow_str: &'_ mut Cow<'a, str>, regex: &Regex, replacer: impl Replacer) {
	let maybe_string = match regex.replace_all(cow_str, replacer) {
		Cow::Borrowed(_) => None,
		Cow::Owned(owned) => Some(owned),
	};
	if let Some(string) = maybe_string {
		*cow_str = Cow::Owned(string);
	}
}

/// a re-implementation of json2ts's function so _HOPEFULLY_ things work right.
/// https://github.com/bcherny/json-schema-to-typescript/blob/3bb5d2b6c48c9d9ffc091577ea53ddeafb8c00fb/src/utils.ts#L161
/// I don't agree with its implementation, but as long as we're using on it to make the initial TS types, so be it.
pub(crate) fn make_type_name<'a>(txt: &'a str) -> Cow<'a, str> {
	// remove accents, umlauts, ... by their basic latin letters
	let mut txt = deunicode_with_tofu_cow(txt, "\u{FFFD}");
	// replace chars which are not valid for typescript identifiers with whitespace
	mut_cow_str_replace_all(&mut txt, regex!(r"(?:^\s*[^a-zA-Z_$])|(?:[^a-zA-Z_$\d])"), " ");
	// uppercase leading underscores followed by lowercase
	mut_cow_str_replace_all(&mut txt, regex!(r"^_[a-z]"), |m: &Captures<'_>| {
		m[0].to_ascii_uppercase()
	});
	// remove non-leading underscores followed by lowercase (convert snake_case)
	mut_cow_str_replace_all(&mut txt, regex!(r"_[a-z]"), |m: &Captures<'_>| {
		m[0][1..].to_ascii_uppercase()
	});
	// uppercase letters after digits, dollars
	mut_cow_str_replace_all(&mut txt, regex!(r"(?:[\d$]+[a-zA-Z])"), |m: &Captures<'_>| {
		m[0].to_ascii_uppercase()
	});
	// uppercase first letter after whitespace
	mut_cow_str_replace_all(&mut txt, regex!(r"\s+([a-zA-Z])"), |m: &Captures<'_>| {
		m[0].trim().to_ascii_uppercase()
	});
	// remove remaining whitespace
	mut_cow_str_replace_all(&mut txt, regex!(r"\s"), "");
	// upper first
	upper_first_ascii(&mut txt);
	return txt;
}

pub(crate) fn schema_type_string(
	schema: &Schema,
	msg_type_name: &str,
	msg_enum_variant: &str,
	msg_enum_variant_field: &str,
	required_types: &mut BTreeSet<Arc<str>>,
) -> Result<String, SdkMakerError> {
	if let Some(schema_object) = schema.as_object() {
		if let Some(schema_object_array) = schema_object.array.as_ref() {
			let Some(sub_type) = schema_object_array
				.items
				.as_ref()
				.and_then(|array_items| array_items.as_single())
			else {
				return Err(SdkMakerError::EnumVariantFieldHasMultiTypedArray(
					msg_type_name.to_string(),
					msg_enum_variant.to_string(),
					msg_enum_variant_field.to_string(),
				));
			};
			let mut sub_type = schema_type_string(
				sub_type,
				msg_type_name,
				msg_enum_variant,
				msg_enum_variant_field,
				required_types,
			)?;

			if let Some(array_length) = schema_object_array
				.max_items
				.filter(|max_items| *max_items == schema_object_array.min_items.unwrap_or_default())
			{
				let mut result = String::from("[");
				result.push_str(&sub_type);
				for _ in 1..array_length {
					result.push_str(", ");
					result.push_str(&sub_type);
				}
				result.push_str("]");
				return Ok(result);
			} else {
				sub_type.push_str("[]");
				return Ok(sub_type);
			}
		} else if let Some(value_instance_types) = schema_object.instance_type.as_ref() {
			// The vec must be handled for nullable types

			let mut result = String::new();
			let mut value_instance_types = value_instance_types.iter().peekable();
			while let Some(value_instance_type) = value_instance_types.next() {
				match value_instance_type {
					InstanceType::Null => {
						result.push_str("null");
					},
					InstanceType::Boolean => {
						result.push_str("boolean");
					},
					// Inline defined types are not supported
					InstanceType::Object |
					// If this was a valid Array, schema_object.array should exist
					InstanceType::Array => {
						eprintln!("There's some schrodinger's bullshit going on with this object");
						return Err(
							SdkMakerError::UnknownEnumVariantField(
								msg_type_name.to_string(),
								msg_enum_variant.to_string(),
								msg_enum_variant_field.to_string()
							)
						);
					},
					InstanceType::Number => {
						result.push_str("number");
					},
					InstanceType::String => {
						result.push_str("string");
					},
					InstanceType::Integer => {
						result.push_str("number");
					},
				}
				if value_instance_types.peek().is_some() {
					result.push_str(" | ");
				}
			}
			return Ok(result);
		// Not-nullable type references
		} else if let Some(schema_object_reference) = schema_object.reference.as_ref().and_then(|ref_string| {
			if ref_string.starts_with("#/definitions/") {
				Some(&ref_string[14..])
			} else {
				None
			}
		}) {
			let schema_object_type_name = make_type_name(schema_object_reference);
			required_types.insert(schema_object_type_name.clone().into());
			return Ok(schema_object_type_name.to_string());
		// Nullable type references but represented as an any_of with a length of 1
		} else if let Some(schema_object_reference) = schema_object
			.subschemas
			.as_ref()
			.and_then(|subschemas| subschemas.all_of.as_ref())
			.and_then(|subschemas_all_of| {
				if subschemas_all_of.len() == 1 {
					subschemas_all_of[0].as_object()?.reference.as_ref()
				} else {
					None
				}
			})
			.and_then(|ref_string| {
				if ref_string.starts_with("#/definitions/") {
					Some(&ref_string[14..])
				} else {
					None
				}
			}) {
			let schema_object_type_name = make_type_name(schema_object_reference);
			required_types.insert(schema_object_type_name.clone().into());
			return Ok(schema_object_type_name.to_string());
		// Nullable type references
		} else if let Some(schema_object_reference) = schema_object
			.subschemas
			.as_ref()
			.and_then(|subschema| subschema.any_of.as_ref())
			.and_then(|multi_type| {
				let [actual_type, nullable_type] = multi_type.as_slice() else {
					return None;
				};
				if !nullable_type
					.as_object()
					.and_then(|v| v.instance_type.as_ref())
					.and_then(|instance_type| instance_type.as_single())
					.is_some_and(|instance| *instance == InstanceType::Null)
				{
					return None;
				}
				return actual_type.as_object().and_then(|actual_type| {
					actual_type.reference.as_ref().and_then(|ref_string| {
						if ref_string.starts_with("#/definitions/") {
							Some(&ref_string[14..])
						} else {
							None
						}
					})
				});
			}) {
			let schema_object_type_name = make_type_name(schema_object_reference);
			required_types.insert(schema_object_type_name.clone().into());
			return Ok([&schema_object_type_name, " | ", "null"].join(""));
		} else {
			eprintln!("invalid schema: {:#?}", schema);
			return Err(SdkMakerError::UnknownEnumVariantField(
				msg_type_name.to_string(),
				msg_enum_variant.to_string(),
				msg_enum_variant_field.to_string(),
			));
		}
	} else {
		return Ok("any".to_string());
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MethodGenType<'a> {
	Instantiate,
	Execute,
	Query(&'a BTreeMap<Arc<str>, Arc<str>>),
	Migrate,
	Sudo,
	Cw20Hook,
}

impl MethodGenType<'_> {
	#[inline]
	pub(crate) fn is_query(&self) -> bool {
		match self {
			MethodGenType::Query(_) => true,
			_ => false,
		}
	}
	pub(crate) fn generate_method_name(&self, enum_variant: &str) -> String {
		match self {
			MethodGenType::Instantiate => "instantiateIx".to_string(),
			MethodGenType::Execute => ["build", &enum_variant.to_case(Case::Pascal), "Ix"].join(""),
			MethodGenType::Query(_) => ["query", &enum_variant.to_case(Case::Pascal)].join(""),
			MethodGenType::Migrate => "migrateIx".to_string(),
			MethodGenType::Sudo => ["sudoExec", &enum_variant.to_case(Case::Pascal), "Ix"].join(""),
			MethodGenType::Cw20Hook => ["build", &enum_variant.to_case(Case::Pascal), "Cw20Ix"].join(""),
		}
	}
	pub(crate) fn prepend_extra_args(&self) -> bool {
		match self {
			MethodGenType::Cw20Hook => true,
			_ => false,
		}
	}
	pub(crate) fn extra_func_args(&self) -> &'static str {
		match self {
			MethodGenType::Instantiate | MethodGenType::Execute | MethodGenType::Sudo => "funds?: Coin[]",
			MethodGenType::Query(_) | MethodGenType::Migrate => "",
			MethodGenType::Cw20Hook => "tokenContractOrUnifiedDenom: string, amount: string | bigint | number",
		}
	}
	pub(crate) fn parent_func_call(&self) -> &'static str {
		match self {
			MethodGenType::Instantiate | MethodGenType::Migrate | MethodGenType::Sudo => {
				todo!("Unknown parent function for {:?}", self)
			}
			MethodGenType::Execute => "this.executeIx(msg, funds)",
			MethodGenType::Query(_) => "this.query(msg)",
			MethodGenType::Cw20Hook => "this.executeIxCw20(msg, tokenContractOrUnifiedDenom, amount)",
		}
	}
	pub(crate) fn return_type(&self, enum_variant: &str) -> Arc<str> {
		match self {
			MethodGenType::Instantiate | MethodGenType::Migrate | MethodGenType::Sudo => {
				todo!("Unknown parent function for {:?}", self)
			}
			MethodGenType::Execute => "ExecuteInstruction".into(),
			MethodGenType::Query(return_type_map) => {
				return_type_map.get(enum_variant).cloned().unwrap_or("unknown".into())
			}
			MethodGenType::Cw20Hook => "ExecuteInstruction".into(),
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum MethodArgType<'a> {
	None,
	Object(&'a ObjectValidation),
	TypeRef(&'a str),
}
impl MethodArgType<'_> {
	#[inline]
	pub(crate) fn is_none(&self) -> bool {
		match self {
			MethodArgType::None => true,
			_ => false,
		}
	}
	#[inline]
	pub(crate) fn is_some(&self) -> bool {
		match self {
			MethodArgType::None => false,
			_ => true,
		}
	}
	#[inline]
	pub(crate) fn is_empty_object(&self) -> bool {
		match self {
			MethodArgType::Object(obj_validation) => obj_validation.properties.len() == 0,
			_ => false,
		}
	}
}

#[cfg(test)]
mod tests {}
