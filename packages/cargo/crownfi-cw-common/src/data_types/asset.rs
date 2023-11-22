
use std::fmt;

use cosmwasm_std::{Addr, Coin, CosmosMsg, BankMsg, WasmMsg, to_json_binary};
use cosmwasm_schema::{cw_serde, schemars::{schema::Schema, gen::SchemaGenerator, JsonSchema}};
use cw20::{Cw20Coin, Cw20ExecuteMsg, Cw20CoinVerified};
use sei_cosmwasm::SeiMsg;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FungibleAssetKind {
	Native(String),
	CW20(String)
}
impl FungibleAssetKind {
	pub fn into_asset(self, amount: u128) -> FungibleAsset {
		match self {
			FungibleAssetKind::Native(denom) => {
				FungibleAsset::Native(
					Coin { denom, amount: amount.into() }
				)
			},
			FungibleAssetKind::CW20(address) => {
				FungibleAsset::CW20(
					Cw20Coin { address, amount: amount.into() }
				)
			},
		}
	}
}
impl fmt::Display for FungibleAssetKind {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			FungibleAssetKind::Native(string) => {
				f.serialize_str(string)
			},
			FungibleAssetKind::CW20(string) => {
				let mut prefixed_sring = String::with_capacity(
					"cw20/".len() + string.len()
				);
				prefixed_sring.push_str("cw20/");
				prefixed_sring.push_str(string);
				f.serialize_str(&prefixed_sring)
			},
		}
	}
}
impl Serialize for FungibleAssetKind {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		match self {
			FungibleAssetKind::Native(string) => {
				serializer.serialize_str(string)
			},
			FungibleAssetKind::CW20(string) => {
				let mut prefixed_sring = String::with_capacity(
					"cw20/".len() + string.len()
				);
				prefixed_sring.push_str("cw20/");
				prefixed_sring.push_str(string);
				serializer.serialize_str(&prefixed_sring)
			},
		}
	}
}
impl<'de> Deserialize<'de> for FungibleAssetKind {
	fn deserialize<D>(deserializer: D) -> Result<FungibleAssetKind, D::Error>
	where
		D: Deserializer<'de>,
	{
		let string = String::deserialize(deserializer)?;
		if string.starts_with("cw20/") {
			return Ok(Self::CW20(string["cw20/".len()..].to_string()))
		}
		Ok(Self::Native(string))
	}
}
impl JsonSchema for FungibleAssetKind {
	fn schema_name() -> String {
		String::from("FungibleAssetKind")
	}
	fn json_schema(gen: &mut SchemaGenerator) -> Schema {
		String::json_schema(gen)
	}
}

/// Represents a token balance of "any" token! (Currently either native or cw20)
#[cw_serde]
pub enum FungibleAsset {
	Native(Coin),
	CW20(Cw20Coin)
}

impl FungibleAsset {
	pub fn into_asset_kind_and_amount(self) -> (FungibleAssetKind, u128) {
		match self {
			FungibleAsset::Native(coin) => {
				(FungibleAssetKind::Native(coin.denom), coin.amount.u128())
			},
			FungibleAsset::CW20(cw20_coin) => {
				(FungibleAssetKind::CW20(cw20_coin.address), cw20_coin.amount.u128())
			},
		}
	}
	pub fn amount(&self) -> u128 {
		match self {
			FungibleAsset::Native(coin) => {
				coin.amount.u128()
			},
			FungibleAsset::CW20(coin) => {
				coin.amount.u128()
			}
		}
	}
	/// If this is a native coin, it returns the denomination verbatim.
	/// If this is a CW20 coin, it returns "cw20/{address}"
	pub fn identifier(&self) -> String {
		match self {
			FungibleAsset::Native(coin) => {
				coin.denom.clone()
			},
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
			},
			FungibleAsset::CW20(coin) => {
				let FungibleAsset::CW20(other_coin) = other else {
					return false;
				};
				return coin.address == other_coin.address;
			},
		}
	}
	pub fn transfer_to_msg(&self, to: Addr) -> CosmosMsg<SeiMsg> {
		match self {
			FungibleAsset::Native(coin) => {
				BankMsg::Send {
					to_address: to.to_string(),
					amount: vec![coin.clone()]
				}.into()
			},
			FungibleAsset::CW20(coin) => {
				WasmMsg::Execute {
					contract_addr: coin.address.to_string(),
					msg: to_json_binary(
						&Cw20ExecuteMsg::Transfer {
							recipient: to.to_string(),
							amount: coin.amount
						}
					).expect("serialization shouldn't fail"),
					funds: vec![]
				}.into()
			}
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
		FungibleAsset::CW20(
			Cw20Coin { address: value.address.to_string(), amount: value.amount }
		)
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
				}else{
					write!(f, "{}{}", coin.amount, coin.denom)
				}
				
			},
			FungibleAsset::CW20(coin) => {
				write!(f, "{}({})", coin.amount, coin.address)
			},
		}
	}
}
