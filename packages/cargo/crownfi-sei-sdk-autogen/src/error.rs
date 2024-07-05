use thiserror::Error;

#[derive(Error, Debug)]
pub enum SdkMakerError {
	#[error("IO Error: {0}")]
	IO(#[from] std::io::Error),
	#[error("JSON serialization error: {0}")]
	SerdeJson(#[from] serde_json::Error),
	#[error("Contract name must be snake_case")]
	ContractNameNotSnakeCase,
	#[error("The contract's auto-generated root JSON schema seemingly has no properties")]
	DummyRootSchmaNoObject,
	#[error("Expected the contract's auto-generated JSON schema's message types to be refrences to the actual type")]
	DummyRootSchemaInvalidProperty,
	#[cfg(not(target_family = "wasm"))]
	#[error("\"json2ts\" wasn't found: {0} (Try \"npm install -g json-schema-to-typescript\")")]
	Json2TsNotFound(which::Error),
	#[cfg(target_family = "wasm")]
	#[error(
		"The \"which\" crate this depends upon doesn't know how to search for executable programs in a wasm space"
	)]
	Json2TsNotFound(()),
	#[error("{0} is not an enum. (Must be made up of subschemas using one_of)")]
	MsgTypeNotEnum(String),
	#[error("{0} has a malformed enum variant: {1}")]
	MalformedEnumVariant(String, String),
	#[error("{0}::{1} is expected to have named fields")]
	EnumNamedFieldsExpected(String, String),
	#[error("{0}::{1}.{2} is not represented by a referenced type or non-object primitive")]
	UnknownEnumVariantField(String, String, String),
	#[error("{0}::{1}.{2} is defined as an array/tuple of multiple types, which this tool currently cannot handle.")]
	EnumVariantFieldHasMultiTypedArray(String, String, String),
}
