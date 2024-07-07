use cosmwasm_std::StdError;
use hex::{FromHex, ToHex};
use tiny_keccak::Hasher;

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
	if !addr_str.starts_with("0x") {
		return Err(StdError::parse_err(
			"[u8; 20]",
			"parse_ethereum_address: address does not start with 0x",
		));
	}
	Ok(<[u8; 20]>::from_hex(addr_str.split_at(2).1)
		.map_err(|err| StdError::parse_err("[u8; 20]", format!("parse_ethereum_address: hex parsing failed: {err}")))?)
}

pub fn bytes_to_ethereum_address(addr_bytes: &[u8]) -> Result<String, StdError> {
	let Some(addr_bytes_fixed): Option<&[u8; 20]> = addr_bytes.last_chunk() else {
		return Err(StdError::serialize_err(
			"EthereumAddress",
			"address was less than 20 bytes in length",
		));
	};
	// hex crate does not expose the iter, and that makes me sed
	let unprefixed_result: String = addr_bytes_fixed.encode_hex();
	let mut result = String::with_capacity(42);
	result.push_str("0x");
	result.push_str(&unprefixed_result);
	Ok(result)
}

/// Turns the specified all-lowercase ethereum address into a checksum-case addres
///
/// **This performs a keccak hash** and might use a lot of gas
pub fn checksumify_ethereum_address(addr_str: &mut str) -> Result<(), StdError> {
	let (addr_prefix, addr_hex) = addr_str.split_at_mut(2);
	if addr_prefix != "0x" {
		return Err(StdError::generic_err(
			"checksumify_ethereum_address: address does not start with 0x",
		));
	}
	if addr_hex.len() != 40 {
		return Err(StdError::generic_err(
			"checksumify_ethereum_address: address is not 42 bytes long",
		));
	}
	// Technically too permissive, but don't have the time to write something as efficient as this impl
	if !addr_hex.is_ascii() {
		return Err(StdError::generic_err(
			"checksumify_ethereum_address: address contains invalid characters",
		));
	}
	// We're not actually gonna check if it's all hex characters for now
	addr_hex.make_ascii_lowercase();
	let mut hash = [0u8; 32];
	let mut hasher = tiny_keccak::Keccak::v256();
	hasher.update(addr_hex.as_bytes());
	hasher.finalize(&mut hash);

	// If of hex hash is > "7" then print in upper case
	for hash_index in 0..20usize {
		let mut addr_index = hash_index * 2;
		let mut hash_byte = hash[hash_index];
		if hash_byte > 0x70 {
			// SAFTY: We asserted that addr_hex.is_ascii() and is 40 bytes long
			unsafe {
				let addr_char_as_uppercase = addr_hex.as_bytes().get_unchecked(addr_index).to_ascii_uppercase();
				*addr_hex.as_bytes_mut().get_unchecked_mut(addr_index) = addr_char_as_uppercase;
			}
		}
		hash_byte &= 0x0f;
		addr_index += 1;
		if hash_byte > 0x07 {
			// SAFTY: We asserted that addr_hex.is_ascii() and is 40 bytes long
			unsafe {
				let addr_char_as_uppercase = addr_hex.as_bytes().get_unchecked(addr_index).to_ascii_uppercase();
				*addr_hex.as_bytes_mut().get_unchecked_mut(addr_index) = addr_char_as_uppercase;
			}
		}
	}
	Ok(())
}
