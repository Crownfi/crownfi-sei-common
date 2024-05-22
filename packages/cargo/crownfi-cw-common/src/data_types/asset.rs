use std::fmt;

use borsh::{BorshDeserialize, BorshSerialize};
use cosmwasm_schema::{
	cw_serde,
	schemars::{gen::SchemaGenerator, schema::Schema, JsonSchema},
};
use cosmwasm_std::{
	to_json_binary, Addr, BankMsg, Coin, CosmosMsg, CustomQuery, QuerierWrapper, StdError, Uint128, WasmMsg,
};
use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20Coin, Cw20CoinVerified, Cw20ExecuteMsg, Cw20QueryMsg};
use sei_cosmwasm::SeiMsg;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::canonical_addr::SeiCanonicalAddr;
use crate::{impl_serializable_borsh, storage::SerializableItem};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, BorshDeserialize, BorshSerialize)]
pub enum FungibleAssetKind {
	Native(String),
	CW20(SeiCanonicalAddr),
}
impl_serializable_borsh!(FungibleAssetKind);

impl TryFrom<FungibleAssetKindString> for FungibleAssetKind {
	type Error = StdError;
	fn try_from(value: FungibleAssetKindString) -> Result<Self, Self::Error> {
		match value {
			FungibleAssetKindString::Native(denom) => Ok(FungibleAssetKind::Native(denom)),
			FungibleAssetKindString::CW20(addr) => Ok(FungibleAssetKind::CW20(Addr::unchecked(addr).try_into()?)),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, BorshDeserialize, BorshSerialize)]
pub enum FungibleAssetKindString {
	Native(String),
	CW20(String),
}

impl FungibleAssetKindString {
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
		}
	}
	pub fn query_balance<Q: CustomQuery>(
		&self,
		querier: &QuerierWrapper<Q>,
		holder: &Addr,
	) -> Result<Uint128, StdError> {
		match self {
			FungibleAssetKindString::Native(denom) => Ok(querier.query_balance(holder, denom)?.amount),
			FungibleAssetKindString::CW20(address) => Ok(querier
				.query_wasm_smart::<Cw20BalanceResponse>(address, &Cw20QueryMsg::Balance { address: holder.into() })?
				.balance),
		}
	}
}
impl TryFrom<FungibleAssetKind> for FungibleAssetKindString {
	type Error = StdError;
	fn try_from(value: FungibleAssetKind) -> Result<Self, Self::Error> {
		match value {
			FungibleAssetKind::Native(denom) => Ok(FungibleAssetKindString::Native(denom)),
			FungibleAssetKind::CW20(addr) => Ok(FungibleAssetKindString::CW20(Addr::try_from(addr)?.into_string())),
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
		}
	}
}
impl From<&str> for FungibleAssetKindString {
	fn from(value: &str) -> Self {
		if value.starts_with("cw20/") {
			return Self::CW20(value["cw20/".len()..].into());
		}
		Self::Native(value.into())
	}
}
impl From<String> for FungibleAssetKindString {
	fn from(value: String) -> Self {
		if value.starts_with("cw20/") {
			return Self::CW20(value["cw20/".len()..].into());
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

/// Represents a token balance of "any" token! (Currently either native or cw20)
#[cw_serde]
pub enum FungibleAsset {
	Native(Coin),
	CW20(Cw20Coin),
}

impl FungibleAsset {
	pub fn into_asset_kind_string_and_amount(self) -> (FungibleAssetKindString, u128) {
		match self {
			FungibleAsset::Native(coin) => (FungibleAssetKindString::Native(coin.denom), coin.amount.u128()),
			FungibleAsset::CW20(cw20_coin) => (
				FungibleAssetKindString::CW20(cw20_coin.address),
				cw20_coin.amount.u128(),
			),
		}
	}
	pub fn amount(&self) -> u128 {
		match self {
			FungibleAsset::Native(coin) => coin.amount.u128(),
			FungibleAsset::CW20(coin) => coin.amount.u128(),
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
		}
	}
	pub fn transfer_to_msg(&self, to: &Addr) -> CosmosMsg<SeiMsg> {
		match self {
			FungibleAsset::Native(coin) => BankMsg::Send {
				to_address: to.to_string(),
				amount: vec![coin.clone()],
			}
			.into(),
			FungibleAsset::CW20(coin) => WasmMsg::Execute {
				contract_addr: coin.address.clone(),
				msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
					recipient: to.to_string(),
					amount: coin.amount,
				})
				.expect("serialization shouldn't fail"),
				funds: vec![],
			}
			.into(),
		}
	}

	pub fn as_native_coin(&self) -> Option<&Coin> {
		match self {
			FungibleAsset::Native(coin) => Some(coin),
			FungibleAsset::CW20(_) => None,
		}
	}
	pub fn as_cw20_coin(&self) -> Option<&Cw20Coin> {
		match self {
			FungibleAsset::Native(_) => None,
			FungibleAsset::CW20(coin) => Some(coin),
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
				// TODO: Check to see if coin.denom.starts_with("factory/") saves on gas
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
		}
	}
}
