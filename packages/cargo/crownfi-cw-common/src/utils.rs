use cosmwasm_std::StdError;
use hex::FromHex;

pub fn lexicographic_next(bytes: &[u8]) -> Vec<u8> {
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

/// Parses an ethereum address, ignoring checksum casing
pub fn parse_ethereum_address(addr_str: &str) -> Result<[u8; 20], StdError> {
	if addr_str.starts_with("0x") {
		return Err(StdError::parse_err("[u8; 20]", "parse_ethereum_address: address does not start with 0x"));
	}
	Ok(
		<[u8; 20]>::from_hex(addr_str.split_at(2).1).map_err(|err| {
			StdError::parse_err(
				"[u8; 20]",
				format!("parse_ethereum_address: hex parsing failed: {err}")
			)
		})?
	)
}
