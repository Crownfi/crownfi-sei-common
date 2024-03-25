use std::{ptr, str};
use cosmwasm_std::{Addr, StdError, StdResult};
use super::memory::{ConstReigon, OwnedRegion};

// This is heavily inspired from https://github.com/CosmWasm/cosmwasm/blob/336afd2e62f83ea632bb4b2f94488b228ca2e28a/packages/std/src/imports.rs
// Safty consideration references:
//   * https://github.com/CosmWasm/cosmwasm/blob/336afd2e62f83ea632bb4b2f94488b228ca2e28a/packages/vm/src/imports.rs
//   * https://github.com/CosmWasm/cosmwasm/blob/336afd2e62f83ea632bb4b2f94488b228ca2e28a/packages/vm/src/instance.rs

/// The size of our SeiCanonicalAddr
const CANONICAL_ADDRESS_BUFFER_LENGTH: usize = 32;
/// Length of a bech32 encoded contract address on Sei
const HUMAN_ADDRESS_BUFFER_LENGTH: usize = 62;


extern "C" {
	#[link_name = "addr_validate"]
	fn wasmvm_addr_validate(source_ptr: usize) -> usize;
	#[link_name = "addr_canonicalize"]
    fn wasmvm_addr_canonicalize(source_ptr: usize, destination_ptr: usize) -> usize;
	#[link_name = "addr_humanize"]
    fn wasmvm_addr_humanize(source_ptr: usize, destination_ptr: usize) -> usize;
}

pub fn addr_validate(input: &str) -> Result<(), StdError> {
	let input_bytes = input.as_bytes();
	if input_bytes.len() > 256 {
		// See MAX_LENGTH_HUMAN_ADDRESS in the VM.
		// In this case, the VM will refuse to read the input from the contract.
		// Stop here to allow handling the error in the contract.
		return Err(StdError::generic_err("input too long for addr_validate"));
	}
	let input_reigon = ConstReigon::new(input_bytes);
	// SAFTY:
	// * It is assumed that the input_reigon passed to wasmvm_addr_validate will not be edited.
	// * It is assumed that a newly allocated valid region is passed on error.
	// The referenced sources for the cosmwasm VM confirm this.
	if let Some(error_response) = unsafe {
		OwnedRegion::from_ptr(wasmvm_addr_validate(ptr::from_ref(&input_reigon) as usize) as *mut OwnedRegion)
	} {
		return Err(StdError::generic_err(format!(
			"addr_validate errored: {}",
			// SAFTY: It is assumed that the runtime always passes a valid UTF8 error message.
			unsafe {
				str::from_utf8_unchecked(Vec::from(error_response).as_ref())
			}
		)));
	}
	Ok(())
}

pub fn addr_canonicalize(input: &str) -> StdResult<Vec<u8>> {
	let input_bytes = input.as_bytes();
	if input_bytes.len() > 256 {
		// See MAX_LENGTH_HUMAN_ADDRESS in the VM.
		// In this case, the VM will refuse to read the input from the contract.
		// Stop here to allow handling the error in the contract.
		return Err(StdError::generic_err("input too long for addr_validate"));
	}
	let input_reigon = ConstReigon::new(input_bytes);
	let mut result_reigon = OwnedRegion::from(Vec::with_capacity(CANONICAL_ADDRESS_BUFFER_LENGTH));
	// SAFTY:
	// * It is assumed that the input_reigon passed to wasmvm_addr_validate will not be edited.
	// * It is assumed that a newly allocated valid region is passed on error.
	// * It is assumed that the VM won't invalidate the result_reigon
	// The referenced sources for the cosmwasm VM confirm this.
	if let Some(error_response) = unsafe {
		OwnedRegion::from_ptr(wasmvm_addr_canonicalize(
			ptr::from_ref(&input_reigon) as usize,
			ptr::from_mut(&mut result_reigon) as usize
		) as *mut OwnedRegion)
	} {
		return Err(StdError::generic_err(format!(
			"addr_validate errored: {}",
			// SAFTY: It is assumed that the runtime always passes a valid UTF8 error message.
			unsafe {
				str::from_utf8_unchecked(Vec::from(error_response).as_ref())
			}
		)));
	}
	Ok(result_reigon.into())
}

pub fn addr_humanize(input_bytes: &[u8]) -> StdResult<Addr> {
	let input_reigon = ConstReigon::new(input_bytes);
	let mut result_reigon = OwnedRegion::from(Vec::with_capacity(HUMAN_ADDRESS_BUFFER_LENGTH));
	// SAFTY:
	// * It is assumed that the input_reigon passed to wasmvm_addr_validate will not be edited.
	// * It is assumed that a newly allocated valid region is passed on error.
	// * It is assumed that the VM won't invalidate the result_reigon
	// The referenced sources for the cosmwasm VM confirm this.
	if let Some(error_response) = unsafe {
		OwnedRegion::from_ptr(wasmvm_addr_humanize(
			ptr::from_ref(&input_reigon) as usize,
			ptr::from_mut(&mut result_reigon) as usize
		) as *mut OwnedRegion)
	} {
		return Err(StdError::generic_err(format!(
			"addr_validate errored: {}",
			// SAFTY: It is assumed that the runtime always passes a valid UTF8 error message.
			unsafe {
				str::from_utf8_unchecked(Vec::from(error_response).as_ref())
			}
		)));
	}
	Ok(Addr::unchecked(
		// SAFTY: It is assumed that human-readable addresses are valid UTF8
		unsafe {
			String::from_utf8_unchecked(result_reigon.into())
		}
	))
}
