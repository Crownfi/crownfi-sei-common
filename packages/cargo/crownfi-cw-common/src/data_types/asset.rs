use std::fmt;
use hex::FromHex;
use borsh::{BorshDeserialize, BorshSerialize};
use cosmwasm_schema::{
	cw_serde,
	schemars::{gen::SchemaGenerator, schema::Schema, JsonSchema},
};
use cosmwasm_std::{
	to_json_binary, Addr, BankMsg, Binary, Coin, ConversionOverflowError, CosmosMsg, QuerierWrapper, StdError, Uint128, Uint256, WasmMsg
};
use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20Coin, Cw20CoinVerified, Cw20ExecuteMsg, Cw20QueryMsg};
use sei_cosmwasm::{SeiMsg, SeiQuerier, SeiQueryWrapper};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::canonical_addr::SeiCanonicalAddr;
use crate::{impl_serializable_borsh, storage::SerializableItem, utils::{bytes_to_ethereum_address, parse_ethereum_address}};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, BorshDeserialize, BorshSerialize)]
pub enum FungibleAssetKind {
	Native(String),
	CW20(SeiCanonicalAddr),
	ERC20([u8; 20])
}
impl_serializable_borsh!(FungibleAssetKind);

impl FungibleAssetKind {
	pub fn is_native(&self) -> bool {
		match self {
			FungibleAssetKind::Native(_) => true,
			_ => false
		}
	}
	pub fn is_cw20(&self) -> bool {
		match self {
			FungibleAssetKind::CW20(_) => true,
			_ => false
		}
	}
	pub fn is_erc20(&self) -> bool {
		match self {
			FungibleAssetKind::ERC20(_) => true,
			_ => false
		}
	}
}

impl TryFrom<FungibleAssetKindString> for FungibleAssetKind {
	type Error = StdError;
	fn try_from(value: FungibleAssetKindString) -> Result<Self, Self::Error> {
		match value {
			FungibleAssetKindString::Native(denom) => Ok(FungibleAssetKind::Native(denom)),
			FungibleAssetKindString::CW20(addr) => Ok(FungibleAssetKind::CW20(Addr::unchecked(addr).try_into()?)),
			FungibleAssetKindString::ERC20(addr) => {
				if !addr.starts_with("0x") {
					return Err(StdError::parse_err("FungibleAssetKindString::ERC20", "Contract address doesn't start with 0x"));
				}
				Ok(
					FungibleAssetKind::ERC20(<[u8; 20]>::from_hex(
						addr.split_at(2).1
					).map_err(|err| {
						StdError::parse_err(
							"FungibleAssetKindString::ERC20",
							format!("Contract address is not valid: {err}")
						)
					})?)
				)
			},
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, BorshDeserialize, BorshSerialize)]
pub enum FungibleAssetKindString {
	Native(String),
	CW20(String),
	ERC20(String),
}

impl FungibleAssetKindString {
	pub fn is_native(&self) -> bool {
		match self {
			FungibleAssetKindString::Native(_) => true,
			_ => false
		}
	}
	pub fn is_cw20(&self) -> bool {
		match self {
			FungibleAssetKindString::CW20(_) => true,
			_ => false
		}
	}
	pub fn is_erc20(&self) -> bool {
		match self {
			FungibleAssetKindString::ERC20(_) => true,
			_ => false
		}
	}
	pub fn into_asset<A: Into<Uint128>>(self, amount: A) -> FungibleAsset {
		match self {
			FungibleAssetKindString::Native(denom) => FungibleAsset::Native(Coin {
				denom,
				amount: amount.into(),
			}),
			FungibleAssetKindString::CW20(address) => FungibleAsset::CW20(Cw20Coin {
				address,
				amount: amount.into(),
			}),
			FungibleAssetKindString::ERC20(address) => FungibleAsset::ERC20(Cw20Coin {
				address,
				amount: amount.into(),
			}),
		}
	}
	/// Queries the balance of the specified holder.
	/// 
	/// Note that in the case of ERC20 assets, a 0x\* addremss may be provided, and sei1* addresses will be attempted to
	/// be converted to 0x\* addresses. If the conversion attempt fails, this will return 0.
	pub fn query_balance(
		&self,
		querier: &QuerierWrapper<SeiQueryWrapper>,
		holder: &Addr,
	) -> Result<Uint128, StdError> {
		match self {
			FungibleAssetKindString::Native(denom) => Ok(querier.query_balance(holder, denom)?.amount),
			FungibleAssetKindString::CW20(address) => Ok(querier
				.query_wasm_smart::<Cw20BalanceResponse>(address, &Cw20QueryMsg::Balance { address: holder.into() })?
				.balance),
			FungibleAssetKindString::ERC20(address) => {
				let querier = SeiQuerier::new(querier);
				let mut evm_payload = Vec::<u8>::with_capacity(36);
				evm_payload.extend_from_slice(&[0x70, 0xa0, 0x82, 0x31]); // balanceOf(address) signature
				evm_payload.extend_from_slice(&[0u8; 12]);
				if holder.as_str().starts_with("0x") {
					evm_payload.extend_from_slice(&parse_ethereum_address(holder.as_str())?);
				} else {
					let holder_canonical = SeiCanonicalAddr::try_from(holder)?;
					if holder_canonical.is_externally_owned_address() {
						let Some(evm_address) = querier.get_evm_address(holder.clone().into_string())
							.ok()
							.map(|result| {result.evm_address})
							.filter(|evm_address| {evm_address.len() > 0})
						else {
							return Ok(Uint128::zero());
						};
						evm_payload.extend_from_slice(&parse_ethereum_address(evm_address.as_str())?);
					} else {
						evm_payload.extend_from_slice(&holder_canonical.as_slice()[12..]);
					}
				}
				let evm_result = Binary::from_base64(
					&querier.static_call(
						// We don't know who the caller is, but who cares?
						"sei1llllllllllllllllllllllllllllllllllllllllllllllllllls09qcrc".into(),
						address.clone(),
						Binary::from(evm_payload).to_base64()
					)?.encoded_data
				)?;
				if evm_result.len() != 32 {
					return Err(StdError::parse_err(
						"Uint256",
						"balanceOf(address) EVM call did not return a 32 byte long result"
					));
				}
				if evm_result[0..16] != [0; 16] {
					return Err(ConversionOverflowError::new(
						"Uint256",
						"Uint128",
						Uint256::from_be_bytes(evm_result.0.try_into().unwrap())
					).into());
				}
				Ok(Uint128::from(<u128>::from_be_bytes(evm_result.0[16..].try_into().unwrap())))
			},
		}
	}
}
impl TryFrom<FungibleAssetKind> for FungibleAssetKindString {
	type Error = StdError;
	fn try_from(value: FungibleAssetKind) -> Result<Self, Self::Error> {
		match value {
			FungibleAssetKind::Native(denom) => Ok(FungibleAssetKindString::Native(denom)),
			FungibleAssetKind::CW20(addr) => Ok(FungibleAssetKindString::CW20(Addr::try_from(addr)?.into_string())),
			FungibleAssetKind::ERC20(addr) => Ok(
				FungibleAssetKindString::ERC20(bytes_to_ethereum_address(&addr)?)
			),
		}
	}
}

impl fmt::Display for FungibleAssetKindString {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			FungibleAssetKindString::Native(string) => f.write_str(string),
			FungibleAssetKindString::CW20(string) => {
				f.write_str("cw20/")?;
				f.write_str(string)?;
				Ok(())
			}
			FungibleAssetKindString::ERC20(string) => {
				f.write_str("erc20/")?;
				f.write_str(string)?;
				Ok(())
			},
		}
	}
}
impl From<&str> for FungibleAssetKindString {
	fn from(value: &str) -> Self {
		if value.starts_with("cw20/") {
			return Self::CW20(value["cw20/".len()..].into());
		}
		if value.starts_with("erc20/") {
			return Self::ERC20(value["erc20/".len()..].into());
		}
		Self::Native(value.into())
	}
}
impl From<String> for FungibleAssetKindString {
	fn from(value: String) -> Self {
		if value.starts_with("cw20/") {
			return Self::CW20(value["cw20/".len()..].into());
		}
		if value.starts_with("erc20/") {
			return Self::ERC20(value["erc20/".len()..].into());
		}
		Self::Native(value)
	}
}
impl Serialize for FungibleAssetKindString {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		match self {
			FungibleAssetKindString::Native(string) => serializer.serialize_str(string),
			FungibleAssetKindString::CW20(string) => {
				let mut prefixed_sring = String::with_capacity("cw20/".len() + string.len());
				prefixed_sring.push_str("cw20/");
				prefixed_sring.push_str(string);
				serializer.serialize_str(&prefixed_sring)
			}
			FungibleAssetKindString::ERC20(string) => {
				let mut prefixed_sring = String::with_capacity("erc20/".len() + string.len());
				prefixed_sring.push_str("erc20/");
				prefixed_sring.push_str(string);
				serializer.serialize_str(&prefixed_sring)
			}
		}
	}
}
impl<'de> Deserialize<'de> for FungibleAssetKindString {
	fn deserialize<D>(deserializer: D) -> Result<FungibleAssetKindString, D::Error>
	where
		D: Deserializer<'de>,
	{
		let string = <String as Deserialize>::deserialize(deserializer)?;
		if string.starts_with("cw20/") {
			return Ok(Self::CW20(string["cw20/".len()..].to_string()));
		}
		if string.starts_with("erc20/") {
			return Ok(Self::ERC20(string["erc20/".len()..].to_string()));
		}
		Ok(Self::Native(string))
	}
}
impl JsonSchema for FungibleAssetKindString {
	fn schema_name() -> String {
		String::from("FungibleAssetKindString")
	}
	fn json_schema(gen: &mut SchemaGenerator) -> Schema {
		String::json_schema(gen)
	}
}

/// Represents a token balance of "any" token! (Currently either native, cw20, or erc20)
#[cw_serde]
pub enum FungibleAsset {
	Native(Coin),
	CW20(Cw20Coin),
	ERC20(Cw20Coin)
}

impl FungibleAsset {
	pub fn into_asset_kind_string_and_amount(self) -> (FungibleAssetKindString, u128) {
		match self {
			FungibleAsset::Native(coin) => (FungibleAssetKindString::Native(coin.denom), coin.amount.u128()),
			FungibleAsset::CW20(cw20_coin) => (
				FungibleAssetKindString::CW20(cw20_coin.address),
				cw20_coin.amount.u128(),
			),
			FungibleAsset::ERC20(erc20_coin) => (
				FungibleAssetKindString::ERC20(erc20_coin.address),
				erc20_coin.amount.u128(),
			),
		}
	}
	pub fn amount(&self) -> u128 {
		match self {
			FungibleAsset::Native(coin) => coin.amount.u128(),
			FungibleAsset::CW20(coin) => coin.amount.u128(),
			FungibleAsset::ERC20(coin) => coin.amount.u128(),
		}
	}
	/// If this is a native coin, it returns the denomination verbatim.
	/// If this is a CW20 coin, it returns "cw20/{address}"
	pub fn identifier(&self) -> String {
		match self {
			FungibleAsset::Native(coin) => coin.denom.clone(),
			FungibleAsset::CW20(coin) => {
				format!("cw20/{}", coin.address)
			}
			FungibleAsset::ERC20(coin) => {
				format!("erc20/{}", coin.address)
			},
		}
	}

	pub fn denom_matches(&self, other: &Self) -> bool {
		match self {
			FungibleAsset::Native(coin) => {
				let FungibleAsset::Native(other_coin) = other else {
					return false;
				};
				return coin.denom == other_coin.denom;
			}
			FungibleAsset::CW20(coin) => {
				let FungibleAsset::CW20(other_coin) = other else {
					return false;
				};
				return coin.address == other_coin.address;
			}
			FungibleAsset::ERC20(coin) => {
				let FungibleAsset::ERC20(other_coin) = other else {
					return false;
				};
				return coin.address == other_coin.address;
			},
		}
	}
	/// Generates a transfer message for this asset
	/// 
	/// Note that in the case of ERC20, you should provide a 0x\* address, as this function encodes sei1\* addresses
	/// for users wrongly. 
	/// 
	/// This function may panic if Addr is invalid
	/// 
	/// **FIXME:** Replace with falliable varient which can also take the querier to do proper sei1\* <> 0x\* address
	/// conversion.
	pub fn transfer_to_msg(&self, to: &Addr) -> CosmosMsg<SeiMsg> {
		match self {
			FungibleAsset::Native(coin) => BankMsg::Send {
				to_address: to.to_string(),
				amount: vec![coin.clone()],
			}.into(),
			FungibleAsset::CW20(coin) => WasmMsg::Execute {
				contract_addr: coin.address.clone(),
				msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
					recipient: to.to_string(),
					amount: coin.amount,
				})
				.expect("serialization shouldn't fail"),
				funds: vec![],
			}.into(),
			FungibleAsset::ERC20(coin) => SeiMsg::CallEvm {
				value: Uint128::zero(),
				to: coin.address.clone(),
				data: {
					let mut buff = Vec::with_capacity(68);
					buff.extend_from_slice(&[0x23, 0xb8, 0x72, 0xdd]); // ERC20 transfer sig
					buff.extend_from_slice(&[0; 12]);
					if to.as_str().starts_with("0x") {
						buff.extend_from_slice(
							<[u8; 20]>::from_hex(to.as_str().split_at(2).1)
								.expect("FungibleAsset::transfer_to_msg: to address isn't a valid 0x* address")
								.as_slice()
						)
					} else {
						let canon_addr = SeiCanonicalAddr::try_from(to)
							.expect("FungibleAsset::transfer_to_msg: to address isn't a valid sei1* address");
						if canon_addr.is_externally_owned_address() {
							// 20 bytes (this is a wrong way to do this)
							buff.extend_from_slice(canon_addr.as_slice());
						} else {
							// 32 bytes
							buff.extend_from_slice(&canon_addr.as_slice()[12..]);
						}
					}
					buff.extend_from_slice(&[0; 16]);
					buff.extend_from_slice(&coin.amount.to_be_bytes());
					Binary::from(buff).to_base64()
				}
			}.into(),
		}
	}

	pub fn as_native_coin(&self) -> Option<&Coin> {
		match self {
			FungibleAsset::Native(coin) => Some(coin),
			FungibleAsset::CW20(_) => None,
			FungibleAsset::ERC20(_) => None,
		}
	}
	pub fn as_cw20_coin(&self) -> Option<&Cw20Coin> {
		match self {
			FungibleAsset::Native(_) => None,
			FungibleAsset::CW20(coin) => Some(coin),
			FungibleAsset::ERC20(_) => None,
		}
	}
	pub fn as_erc20_coin(&self) -> Option<&Cw20Coin> {
		match self {
			FungibleAsset::Native(_) => None,
			FungibleAsset::CW20(_) => None,
			FungibleAsset::ERC20(coin) => Some(coin),
		}
	}
}
impl From<Coin> for FungibleAsset {
	fn from(value: Coin) -> Self {
		FungibleAsset::Native(value)
	}
}
impl From<Cw20Coin> for FungibleAsset {
	fn from(value: Cw20Coin) -> Self {
		FungibleAsset::CW20(value)
	}
}
impl From<Cw20CoinVerified> for FungibleAsset {
	fn from(value: Cw20CoinVerified) -> Self {
		FungibleAsset::CW20(Cw20Coin {
			address: value.address.to_string(),
			amount: value.amount,
		})
	}
}
impl fmt::Display for FungibleAsset {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			FungibleAsset::Native(coin) => {
				// TODO: Check to see if coin.denom.starts_with("factory/") || coin.denom.starts_with("ibc/") saves gas
				if coin.denom.contains('/') {
					// TokenFactory coins, etc.
					write!(f, "{}({})", coin.amount, coin.denom)
				} else {
					write!(f, "{}{}", coin.amount, coin.denom)
				}
			}
			FungibleAsset::CW20(coin) => {
				write!(f, "{}({})", coin.amount, coin.address)
			}
			FungibleAsset::ERC20(coin) => {
				write!(f, "{}({})", coin.amount, coin.address)
			}
		}
	}
}
