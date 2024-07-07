use convert_case::{Case, Casing};
use cosmwasm_schema::QueryResponses;
use itertools::Itertools;
use lazy_regex::regex;
use schemars::{
	schema::{InstanceType, RootSchema, Schema, SchemaObject, SingleOrVec},
	schema_for, JsonSchema,
};
use std::{
	collections::{BTreeMap, BTreeSet, HashMap},
	fs,
	io::Write,
	path::PathBuf,
	process::{Command, Stdio},
	rc::Rc,
	sync::{Arc, OnceLock},
};

#[cfg(not(target_family = "wasm"))]
use which::which;

#[cfg(target_family = "wasm")]
fn which(_: &str) -> Result<PathBuf, ()> {
	Err(())
}

use crate::{
	error::SdkMakerError,
	strings_for_code::{make_type_name, schema_type_string, MethodArgType, MethodGenType},
	struct_extentions::{SchemaStructExtentions, SingleOrVecStructExtentions},
};

const TYPESCRIPT_OUTPUT_DISCLAIMER_COMMENT: &'static str = "/* eslint-disable */
/**
 * This file was automatically generated by crownfi-sei-sdk-autogen.
 * DO NOT MODIFY IT BY HAND.
 * The Rust definition of the associated structs is the source of truth!!
 */
";

fn type_to_module() -> &'static HashMap<Arc<str>, Arc<str>> {
	static VALUE: OnceLock<HashMap<Arc<str>, Arc<str>>> = OnceLock::new();
	VALUE.get_or_init(|| {
		let mut m = HashMap::new();
		m.insert("ContractBase".into(), "@crownfi/sei-utils".into());
		m.insert("ExecuteInstruction".into(), "@cosmjs/cosmwasm-stargate".into());
		m.insert("Coin".into(), "@cosmjs/amino".into());
		// put Addr and shit here
		m
	})
}
fn default_module() -> &'static Arc<str> {
	static VALUE: OnceLock<Arc<str>> = OnceLock::new();
	VALUE.get_or_init(|| Arc::from("./types.js"))
}

#[derive(Debug)]
pub struct CrownfiSdkMaker {
	root_schema: RootSchema,
	contracts: BTreeMap<Rc<str>, ContractSdkContractDefinition>,
}

#[derive(Debug, Clone)]
pub struct ContractSdkContractDefinition {
	pub instantiate_type: Option<Rc<str>>,
	pub execute_type: Option<Rc<str>>,
	pub query_type: Option<Rc<str>>,
	pub query_enum_varient_to_return_type: BTreeMap<Arc<str>, Arc<str>>,
	pub migrate_type: Option<Rc<str>>,
	pub sudo_type: Option<Rc<str>>,
	pub cw20_hook_type: Option<Rc<str>>,
}
impl ContractSdkContractDefinition {
	pub fn new(dummy_schema: &RootSchema) -> Self {
		let schema_property_to_type_name = |schema: &_| {
			match schema {
				Schema::Bool(_) => None,
				Schema::Object(schema_object) => {
					schema_object.reference.as_ref().map(|string| {
						assert!(string.starts_with("#/definitions/"));
						let sub_str = &string[14..];
						Rc::<str>::from(
							sub_str, // "#/definitions/".len()
						)
					})
				}
			}
		};
		ContractSdkContractDefinition {
			instantiate_type: dummy_schema.schema.object.as_ref().and_then(|obj| {
				obj.properties
					.get("instantiate")
					.and_then(&schema_property_to_type_name)
			}),
			execute_type: dummy_schema
				.schema
				.object
				.as_ref()
				.and_then(|obj| obj.properties.get("execute").and_then(&schema_property_to_type_name)),
			query_type: dummy_schema
				.schema
				.object
				.as_ref()
				.and_then(|obj| obj.properties.get("query").and_then(&schema_property_to_type_name)),
			query_enum_varient_to_return_type: BTreeMap::new(),
			migrate_type: dummy_schema
				.schema
				.object
				.as_ref()
				.and_then(|obj| obj.properties.get("migrate").and_then(&schema_property_to_type_name)),
			sudo_type: dummy_schema
				.schema
				.object
				.as_ref()
				.and_then(|obj| obj.properties.get("sudo").and_then(&schema_property_to_type_name)),
			cw20_hook_type: dummy_schema
				.schema
				.object
				.as_ref()
				.and_then(|obj| obj.properties.get("cw20_hook").and_then(&schema_property_to_type_name)),
		}
	}
}

#[derive(JsonSchema)]
pub struct ContractDummySchema<
	InstantiateType: JsonSchema,
	ExecuteType: JsonSchema,
	QueryType: JsonSchema + QueryResponses,
	MigrateType: JsonSchema,
	SudoType: JsonSchema,
	Cw20HookType: JsonSchema,
> {
	pub instantiate: InstantiateType,
	pub execute: ExecuteType,
	pub query: QueryType,
	pub migrate: MigrateType,
	pub sudo: SudoType,
	pub cw20_hook: Cw20HookType,
}

impl CrownfiSdkMaker {
	pub fn new() -> Self {
		let mut seyulf = Self {
			root_schema: RootSchema::default(),
			contracts: BTreeMap::new(),
		};

		// Assemble the bare minimum schema
		seyulf.root_schema.schema.metadata().title = Some("CrownfiSdkMakerAutogen".to_string());
		seyulf.root_schema.schema.instance_type = Some(SingleOrVec::Single(Box::new(InstanceType::Object)));
		seyulf
	}
	/// Adds your contract message types to the schema.
	/// It's important to note that it is expected that your message types have a unique name.
	/// Which means, if you have multiple contracts, their query messages cannot just be called `QueryMsg`
	pub fn add_contract<
		InstantiateType: JsonSchema,
		ExecuteType: JsonSchema,
		QueryType: JsonSchema + QueryResponses,
		MigrateType: JsonSchema,
		SudoType: JsonSchema,
		Cw20HookType: JsonSchema,
	>(
		&mut self,
		name: &str,
	) -> Result<&mut Self, SdkMakerError> {
		if !name.is_case(Case::Snake) {
			return Err(SdkMakerError::ContractNameNotSnakeCase);
		}

		let mut dummy_schema = schema_for!(
			ContractDummySchema::<InstantiateType, ExecuteType, QueryType, MigrateType, SudoType, Cw20HookType>
		);

		// Not sure if these 2 loops are needed.
		// But this should account for any unused definitions being pruned

		/*
		for (property_name, property_schema) in dummy_schema.schema.object().properties.iter() {
			self.root_schema.schema.object().properties.insert(
				[name, "_", property_name].join("_"),
				property_schema.clone()
			);
		}
		for property_name in dummy_schema.schema.object().required.iter() {
			self.root_schema.schema.object().required.insert(
				[name, "_", property_name].join("_")
			);
		}
		*/

		self.root_schema.definitions.append(&mut dummy_schema.definitions);
		let mut new_contract_def = ContractSdkContractDefinition::new(&dummy_schema);

		for (query_enum_varient, response_schema) in QueryType::response_schemas().unwrap().into_iter() {
			self.root_schema.definitions.extend(response_schema.definitions);
			let mut new_definition = response_schema.schema;
			let new_definition_key = new_definition
				.metadata
				.as_mut()
				.expect("root schema should have metadata")
				.title
				.take() // Definitions don't have titles, so set it to None
				.expect("root schema should have title");
			// println!("query_enum_varient: {query_enum_varient}");
			// println!("new_definition_key: {new_definition_key}");

			self.root_schema.definitions.insert(
				new_definition_key.clone(),
				schemars::schema::Schema::Object(new_definition),
			);
			new_contract_def
				.query_enum_varient_to_return_type
				.insert(query_enum_varient.into(), new_definition_key.into());
		}
		self.contracts.insert(Rc::from(name), new_contract_def);
		Ok(self)
	}

	fn codegen_types(&self, output_path: &mut PathBuf, files_list: &mut Vec<String>) -> Result<(), SdkMakerError> {
		let json2ts_bin_path = which("json2ts").map_err(|err| SdkMakerError::Json2TsNotFound(err))?;
		files_list.push("types.ts".into());
		output_path.push("types.ts");
		let mut child = Command::new(json2ts_bin_path)
			.arg("--output")
			.arg(&output_path)
			.arg("--bannerComment")
			.arg(TYPESCRIPT_OUTPUT_DISCLAIMER_COMMENT)
			.arg("--unreachableDefinitions")
			.arg("true")
			.arg("--additionalProperties")
			.arg("false")
			.stdin(Stdio::piped())
			.spawn()?;
		output_path.pop();

		serde_json::to_writer(
			child
				.stdin
				.as_mut()
				.expect("setting child's stdin to piped should have worked"),
			&self.root_schema,
		)?;
		child.wait()?;
		Ok(())
	}

	fn codegen_contract_method(
		&self,
		output: &mut impl Write,
		required_types: &mut BTreeSet<Arc<str>>,
		msg_type_name: &str,
		msg_enum_variant: &str,
		msg_enum_varient_fields: MethodArgType,
		kind: MethodGenType,
		description: &str,
	) -> Result<(), SdkMakerError> {
		if description.len() > 0 {
			writeln!(output, "\t/** {0} */", regex!(r"\*/").replace_all(description, "* /"))?;
		}

		write!(output, "\t{}(", kind.generate_method_name(msg_enum_variant))?;
		if kind.prepend_extra_args() {
			output.write_all(kind.extra_func_args().as_bytes())?;
		}
		match msg_enum_varient_fields {
			MethodArgType::None => {}
			MethodArgType::Object(msg_enum_varient_fields) if msg_enum_varient_fields.properties.len() == 0 => {}
			MethodArgType::Object(msg_enum_varient_fields) => {
				if kind.prepend_extra_args() {
					write!(output, ", ")?;
				}
				write!(output, "args: {{\n")?;

				let mut fields_iter = msg_enum_varient_fields.properties.iter().peekable();
				while let Some((key, value)) = fields_iter.next() {
					if let Some(value_description) = value
						.as_object()
						.and_then(|schema| Some(schema.metadata.as_ref()?.as_ref().description.as_deref()?))
					{
						write!(output, "\t\t/** {0} */\n", value_description)?;
					}
					write!(
						output,
						"\t\t\"{}\"{}: {}",
						key.escape_default(),
						if msg_enum_varient_fields.required.contains(key) {
							""
						} else {
							"?"
						},
						schema_type_string(value, msg_type_name, msg_enum_variant, key, required_types)?
					)?;

					//match value.as
					if fields_iter.peek().is_some() {
						write!(output, ",\n")?;
					} else {
						write!(output, "\n")?;
					}
				}
				write!(output, "\t}}")?;
				if msg_enum_varient_fields.required.len() == 0 {
					write!(output, " = {{}}")?;
				}

				if !kind.prepend_extra_args() && kind.extra_func_args().len() > 0 {
					write!(output, ", ")?;
				}
			}
			MethodArgType::TypeRef(type_ref) => {
				if kind.prepend_extra_args() {
					write!(output, ", ")?;
				}
				write!(output, "args: {}", type_ref)?;
				if !kind.prepend_extra_args() && kind.extra_func_args().len() > 0 {
					write!(output, ", ")?;
				}
				let type_name = make_type_name(type_ref);
				required_types.insert(type_name.into());
			}
		}
		if !kind.prepend_extra_args() {
			output.write_all(kind.extra_func_args().as_bytes())?;
		}
		let return_type = kind.return_type(msg_enum_variant);
		let typescript_return_type = make_type_name(&return_type);

		if kind.is_query() {
			writeln!(output, "): Promise<{}> {{", typescript_return_type)?;
		} else {
			writeln!(output, "): {} {{", typescript_return_type)?;
		}

		required_types.insert(typescript_return_type.into());

		write!(output, "\t\tconst msg = ")?;
		if msg_enum_varient_fields.is_empty_object() {
			write!(output, "{{\"{}\": {{}}}}", msg_enum_variant.escape_default())?;
		} else if msg_enum_varient_fields.is_some() {
			write!(output, "{{\"{}\": args}}", msg_enum_variant.escape_default())?;
		} else {
			write!(output, "\"{}\"", msg_enum_variant.escape_default())?;
		}
		writeln!(output, " satisfies {};", msg_type_name)?;
		writeln!(output, "\t\treturn {};", kind.parent_func_call())?;
		writeln!(output, "\t}}")?;
		Ok(())
	}

	fn codegen_contract_methods(
		&self,
		output: &mut impl Write,
		required_types: &mut BTreeSet<Arc<str>>,
		msg_type_name: &str,
		msg_type_def: &SchemaObject,
		kind: MethodGenType,
	) -> Result<(), SdkMakerError> {
		required_types.insert(make_type_name(msg_type_name).into());

		let Some(enum_varients_def) = msg_type_def
			.subschemas
			.as_ref()
			.and_then(|subschemas| subschemas.as_ref().one_of.as_ref())
		else {
			return Err(SdkMakerError::MsgTypeNotEnum(msg_type_name.to_string()));
		};
		for enum_varient_def in enum_varients_def.iter() {
			let Some(enum_varient_def) = enum_varient_def.as_object() else {
				// Just ignore it, shouldn't happen anyway
				continue;
			};

			let Some(instance_type) = enum_varient_def
				.instance_type
				.as_ref()
				.and_then(|instance_type| instance_type.as_single())
			else {
				return Err(SdkMakerError::MalformedEnumVariant(
					msg_type_name.to_string(),
					"instance_type is not a single".to_string(),
				));
			};
			match instance_type {
				InstanceType::String => {
					let Some(enum_values) = enum_varient_def
						.enum_values
						.as_ref()
						.filter(|enum_values| enum_values.len() > 0)
					else {
						return Err(SdkMakerError::MalformedEnumVariant(
							msg_type_name.to_string(),
							"empty enum_values for String enum variant".to_string(),
						));
					};
					for enum_variant in enum_values.iter() {
						let Some(enum_variant) = enum_variant.as_str() else {
							return Err(SdkMakerError::MalformedEnumVariant(
								msg_type_name.to_string(),
								"string enum variant is specified with a non-string value".to_string(),
							));
						};
						let description = enum_varient_def
							.metadata
							.as_ref()
							.and_then(|val| val.as_ref().description.as_deref())
							.unwrap_or_default();
						self.codegen_contract_method(
							output,
							required_types,
							msg_type_name,
							enum_variant,
							MethodArgType::None,
							kind,
							description,
						)?;
					}
				}
				InstanceType::Object => {
					let Some(object) = enum_varient_def
						.object
						.as_ref()
						.filter(|object| object.required.len() == 1 && object.properties.len() == 1)
					else {
						return Err(SdkMakerError::MalformedEnumVariant(
							msg_type_name.to_string(),
							"object has more than one property".to_string(),
						));
					};
					let (enum_variant, enum_variant_schema) = object
						.properties
						.iter()
						.next()
						.expect("object.properties.len() == 1 should mean at least 1 item is returned");

					let description = enum_varient_def
						.metadata
						.as_ref()
						.and_then(|val| val.as_ref().description.as_deref())
						.unwrap_or_default();

					// Quick hack, allow enum varients with references to single types
					if let Some(type_reference) = enum_variant_schema
						.as_object()
						.and_then(|schema| schema.reference.as_ref())
						.and_then(|ref_string| {
							if ref_string.starts_with("#/definitions/") {
								Some(&ref_string[14..])
							} else {
								None
							}
						}) {
						self.codegen_contract_method(
							output,
							required_types,
							msg_type_name,
							enum_variant,
							MethodArgType::TypeRef(type_reference),
							kind,
							description,
						)?;
						continue;
					}

					if !enum_variant_schema.as_object().is_some_and(|enum_variant_schema| {
						enum_variant_schema.instance_type == Some(SingleOrVec::Single(Box::new(InstanceType::Object)))
					}) {
						eprintln!("enum_varient_def: {:#?}", enum_varient_def);
						return Err(SdkMakerError::EnumNamedFieldsExpected(
							msg_type_name.to_string(),
							enum_variant.clone(),
						));
					}
					let Some((enum_variant_schema, other_description)) =
						enum_variant_schema.as_object().and_then(|enum_variant_schema| {
							Some((
								enum_variant_schema.object.as_ref()?.as_ref(),
								enum_variant_schema
									.metadata
									.as_ref()
									.and_then(|metadata| metadata.description.as_deref())
									.unwrap_or_default(),
							))
						})
					else {
						return Err(SdkMakerError::EnumNamedFieldsExpected(
							msg_type_name.to_string(),
							enum_variant.clone(),
						));
					};
					self.codegen_contract_method(
						output,
						required_types,
						msg_type_name,
						enum_variant,
						MethodArgType::Object(enum_variant_schema),
						kind,
						if other_description.len() > 0 {
							other_description
						} else {
							description
						},
					)?;
				}
				_ => {
					return Err(SdkMakerError::MalformedEnumVariant(
						msg_type_name.to_string(),
						"instance_type neither string nor object".to_string(),
					));
				}
			}
		}
		Ok(())
	}
	fn codegen_contracts(&self, output_path: &mut PathBuf, files_list: &mut Vec<String>) -> Result<(), SdkMakerError> {
		let mut types_required = BTreeSet::<Arc<str>>::new();
		// Creating a temp buffer as we must import the types first and we only know that as we go through the contract
		let mut contract_body = Vec::<u8>::new();
		for (contract_name, contract_def) in self.contracts.iter() {
			let contract_class_name = contract_name.as_ref().to_case(Case::Pascal);
			types_required.insert("ContractBase".into());
			types_required.insert("Coin".into());

			writeln!(
				contract_body,
				"export class {}Contract extends ContractBase {{",
				contract_class_name
			)?;

			if let Some(query_type) = &contract_def.query_type {
				let query_def = self
					.root_schema
					.definitions
					.get(query_type.as_ref())
					.and_then(|s| s.as_object())
					.expect("types referenced by contract_def should exist in root_schema.definitions");
				self.codegen_contract_methods(
					&mut contract_body,
					&mut types_required,
					query_type.as_ref(),
					query_def,
					MethodGenType::Query(&contract_def.query_enum_varient_to_return_type),
				)?;
			}
			if let Some(execute_type) = &contract_def.execute_type {
				let query_def = self
					.root_schema
					.definitions
					.get(execute_type.as_ref())
					.and_then(|s| s.as_object())
					.expect("types referenced by contract_def should exist in root_schema.definitions");
				self.codegen_contract_methods(
					&mut contract_body,
					&mut types_required,
					execute_type.as_ref(),
					query_def,
					MethodGenType::Execute,
				)?;
			}
			if let Some(cw20_hook_type) = &contract_def.cw20_hook_type {
				let query_def = self
					.root_schema
					.definitions
					.get(cw20_hook_type.as_ref())
					.and_then(|s| s.as_object())
					.expect("types referenced by contract_def should exist in root_schema.definitions");
				self.codegen_contract_methods(
					&mut contract_body,
					&mut types_required,
					cw20_hook_type.as_ref(),
					query_def,
					MethodGenType::Cw20Hook,
				)?;
			}

			writeln!(contract_body, "}}")?;
			files_list.push([contract_name, ".ts"].join(""));
			output_path.push(files_list.last().expect("literally just pushed this"));
			let modules_to_types = {
				let mut modules_to_types = BTreeMap::<Arc<str>, BTreeSet<Arc<str>>>::new();
				for type_required in types_required.iter().cloned() {
					let module = type_to_module()
						.get(&type_required)
						.unwrap_or(&default_module())
						.clone();

					modules_to_types
						.entry(module)
						.or_insert(BTreeSet::new())
						.insert(type_required);
				}

				modules_to_types
			};

			let mut out_file = fs::File::create(&output_path)?;
			output_path.pop();

			out_file.write_all(TYPESCRIPT_OUTPUT_DISCLAIMER_COMMENT.as_bytes())?;
			for (module, imported_types) in modules_to_types.iter() {
				writeln!(
					out_file,
					"import {{{}}} from \"{}\";",
					imported_types.iter().format(", "),
					module
				)?;
			}
			out_file.write_all(&contract_body)?;
			out_file.sync_all()?;
			types_required.clear();
			contract_body.clear();
		}
		Ok(())
	}

	pub fn generate_code<P: Into<PathBuf>>(&self, out_dir: P) -> Result<(), SdkMakerError> {
		let mut output_path: PathBuf = out_dir.into();
		fs::create_dir_all(&output_path)?;
		let mut files_list = Vec::new();
		self.codegen_types(&mut output_path, &mut files_list)?;
		self.codegen_contracts(&mut output_path, &mut files_list)?;

		output_path.push("index.ts");
		let mut out_file = fs::File::create(&output_path)?;
		output_path.pop();
		out_file.write_all(TYPESCRIPT_OUTPUT_DISCLAIMER_COMMENT.as_bytes())?;
		for mut file_name in files_list.into_iter() {
			if file_name.ends_with(".ts") {
				file_name.truncate(file_name.len() - 2);
				file_name.push_str("js");
			}
			writeln!(out_file, "export * from \"./{}\";", file_name.escape_default())?;
		}
		out_file.sync_all()?;

		Ok(())
	}
}
