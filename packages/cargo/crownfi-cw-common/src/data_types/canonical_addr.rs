#[cfg(not(target_arch = "wasm32"))]
use bech32::{FromBase32, ToBase32};
use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};
use cosmwasm_std::{Addr, Api, CanonicalAddr, StdError};
use std::fmt::Display;

use crate::{impl_serializable_as_ref, storage::SerializableItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshDeserialize, BorshSerialize, Zeroable, Pod)]
#[repr(C)]
pub struct SeiCanonicalAddr {
	bytes: [u8; 32],
}
impl SeiCanonicalAddr {
	/// basically, is this (probably) an address associated with a pubkey
	#[inline]
	pub fn is_externally_owned_address(&self) -> bool {
		self.bytes[0..12] == [0u8; 12]
	}
	#[inline]
	pub fn as_slice(&self) -> &[u8] {
		if self.is_externally_owned_address() {
			&self.bytes[12..]
		} else {
			&self.bytes
		}
	}
	/// Checks if this is equal to the given addr using the Api
	pub fn is_eq_addr(&self, addr: &Addr, api: &dyn Api) -> Result<bool, StdError> {
		Ok(self.as_slice() == api.addr_canonicalize(addr.as_str())?.as_slice())
	}
}
impl_serializable_as_ref!(SeiCanonicalAddr);
impl From<[u8; 32]> for SeiCanonicalAddr {
	#[inline]
	fn from(bytes: [u8; 32]) -> Self {
		SeiCanonicalAddr { bytes }
	}
}
impl From<[u8; 20]> for SeiCanonicalAddr {
	fn from(value: [u8; 20]) -> Self {
		let mut bytes = [0u8; 32];
		bytes[12..].copy_from_slice(&value);
		SeiCanonicalAddr { bytes }
	}
}
impl TryFrom<&[u8]> for SeiCanonicalAddr {
	type Error = StdError;
	fn try_from(canon_addr: &[u8]) -> Result<Self, StdError> {
		if canon_addr.len() > 32 {
			return Err(StdError::generic_err(
				"expected canonical addresses to be 32 bytes long or smaller",
			));
		}
		let mut bytes = [0u8; 32];
		// prepend with 0's, this is just a guess. Hopefully this is canon with EVM interop
		bytes[(32 - canon_addr.len())..].copy_from_slice(canon_addr);
		return Ok(SeiCanonicalAddr { bytes });
	}
}
impl TryFrom<&CanonicalAddr> for SeiCanonicalAddr {
	type Error = StdError;
	fn try_from(canon_addr: &CanonicalAddr) -> Result<Self, StdError> {
		Self::try_from(canon_addr.as_slice())
	}
}
impl TryFrom<CanonicalAddr> for SeiCanonicalAddr {
	type Error = StdError;
	#[inline]
	fn try_from(canon_addr: CanonicalAddr) -> Result<Self, StdError> {
		Self::try_from(&canon_addr)
	}
}
impl From<&SeiCanonicalAddr> for CanonicalAddr {
	#[inline]
	fn from(value: &SeiCanonicalAddr) -> Self {
		if value.is_externally_owned_address() {
			CanonicalAddr::from(&value.bytes[12..])
		} else {
			CanonicalAddr::from(&value.bytes)
		}
	}
}
impl From<SeiCanonicalAddr> for CanonicalAddr {
	#[inline]
	fn from(value: SeiCanonicalAddr) -> Self {
		Self::from(&value)
	}
}

#[cfg(not(target_arch = "wasm32"))]
impl TryFrom<&str> for SeiCanonicalAddr {
	type Error = StdError;
	fn try_from(value: &str) -> Result<Self, Self::Error> {
		let (prefix, words, _) = bech32::decode(&value)
			.map_err(|err| StdError::parse_err("SeiCanonicalAddr", format!("bech32::decode error: {err}")))?;
		if prefix.as_str() != "sei" {
			return Err(StdError::parse_err(
				"SeiCanonicalAddr",
				format!("\"{value}\" wasn't prefixed with \"sei\""),
			));
		}
		let bytes = Vec::<u8>::from_base32(&words)
			.map_err(|err| StdError::parse_err("SeiCanonicalAddr", format!("base32 decode error error: {err}")))?;
		Self::try_from(bytes.as_slice())
	}
}

#[cfg(target_arch = "wasm32")]
impl TryFrom<&str> for SeiCanonicalAddr {
	type Error = StdError;
	fn try_from(value: &str) -> Result<Self, Self::Error> {
		Self::try_from(crate::wasm_api::addr::addr_canonicalize(value)?.as_slice())
	}
}

impl TryFrom<Addr> for SeiCanonicalAddr {
	type Error = StdError;
	fn try_from(value: Addr) -> Result<Self, Self::Error> {
		Self::try_from(value.as_str())
	}
}

impl TryFrom<&Addr> for SeiCanonicalAddr {
	type Error = StdError;
	fn try_from(value: &Addr) -> Result<Self, Self::Error> {
		Self::try_from(value.as_str())
	}
}

#[cfg(not(target_arch = "wasm32"))]
impl Display for SeiCanonicalAddr {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&bech32::encode("sei", self.as_slice().to_base32(), bech32::Variant::Bech32).unwrap())
	}
}
#[cfg(not(target_arch = "wasm32"))]
impl TryFrom<SeiCanonicalAddr> for Addr {
	type Error = StdError;
	fn try_from(value: SeiCanonicalAddr) -> Result<Self, Self::Error> {
		Ok(Addr::unchecked(value.to_string()))
	}
}
#[cfg(not(target_arch = "wasm32"))]
impl TryFrom<&SeiCanonicalAddr> for Addr {
	type Error = StdError;
	fn try_from(value: &SeiCanonicalAddr) -> Result<Self, Self::Error> {
		Ok(Addr::unchecked(value.to_string()))
	}
}
#[cfg(target_arch = "wasm32")]
impl Display for SeiCanonicalAddr {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(crate::wasm_api::addr::addr_humanize(self.as_slice()).unwrap().as_str())
	}
}
#[cfg(target_arch = "wasm32")]
impl TryFrom<SeiCanonicalAddr> for Addr {
	type Error = StdError;
	fn try_from(value: SeiCanonicalAddr) -> Result<Self, Self::Error> {
		crate::wasm_api::addr::addr_humanize(value.as_slice())
	}
}
#[cfg(target_arch = "wasm32")]
impl TryFrom<&SeiCanonicalAddr> for Addr {
	type Error = StdError;
	fn try_from(value: &SeiCanonicalAddr) -> Result<Self, Self::Error> {
		crate::wasm_api::addr::addr_humanize(value.as_slice())
	}
}

#[cfg(test)]
mod test {
	use super::SeiCanonicalAddr;

	// sei19rl4cm2hmr8afy4kldpxz3fka4jguq0a3vute5 <-> [40, 255, 92, 109, 87, 216, 207, 212, 146, 182, 251, 66, 97, 69, 54, 237, 100, 142, 1, 253]
	#[test]
	fn convert_from_human_readable() {
		let canon_addr = SeiCanonicalAddr::from([
			40, 255, 92, 109, 87, 216, 207, 212, 146, 182, 251, 66, 97, 69, 54, 237, 100, 142, 1, 253,
		]);
		assert!(SeiCanonicalAddr::try_from("sei19rl4cm2hmr8afy4kldpxz3fka4jguq0a3vute5") == Ok(canon_addr));
	}
	#[test]
	fn convert_to_human_readable() {
		let canon_addr = SeiCanonicalAddr::from([
			40, 255, 92, 109, 87, 216, 207, 212, 146, 182, 251, 66, 97, 69, 54, 237, 100, 142, 1, 253,
		]);
		assert!(canon_addr.to_string().as_str() == "sei19rl4cm2hmr8afy4kldpxz3fka4jguq0a3vute5");
	}
}
