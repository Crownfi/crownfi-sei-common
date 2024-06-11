
import { normalizePubkey } from "@crownfi/sei-js-core/dist/lib/utils/address.js"; // FIXME: Don't reference the file itself
import { keccak256ToHex } from "keccak-wasm";

export function getEvmAddressFromPubkey(pubkey: Uint8Array): string {
	return toChecksumAddressEvm("0x" + keccak256ToHex(normalizePubkey(pubkey, true).subarray(1)).substring(24, 64));
}

/**
 * Converts a non-checksum ethereum address (all uppercase or lowercase) to a checksum address.
 * @param address The address to normalize
 * @param checkAddress whether or not to throw an error if the address in invalid. Defaults to true. Be careful when
 * setting this to false, as the resulting value will be meaningless.
 * @returns The address in checksum case
 */
export function toChecksumAddressEvm(address: string, checkAddress: boolean = true) {
	if(checkAddress){
		if(!/^(0x)?[0-9a-f]{40}$/i.test(address)){
			throw new Error("Invalid Ethereum address");
		}
		address = address.toLowerCase().substring(2);
	}
	const addressHash = keccak256ToHex(Buffer.from(address, "ascii"));
	let checksumAddress = "0x";

	for(let i = 0; i < address.length; i += 1){
		// If character > "7" then print in upper case
		if(addressHash.charCodeAt(i) > 55){
			checksumAddress += address[i].toUpperCase();
		}else{
			checksumAddress += address[i];
		}
	}
	return checksumAddress;
};

function isValidEvmAddressChecksum(address: string): boolean {
	if(address.length !== 42 || !address.startsWith("0x")){
		return false;
	}
	address = address.substring(2);
	const addressHash = keccak256ToHex(Buffer.from(address.toLowerCase()));
	for(let i = 0; i < 40; i += 1){
		if(addressHash.charCodeAt(i) > 55){
			if(address.charCodeAt(i) > 96){ // Is lower case
				return false;
			}
		}else if(address.charCodeAt(i) > 64 && address.charCodeAt(i) < 91){ // Is upper case
			return false;
		}
	}
	return true;
};

/**
 * Checks whether or not the given address is a valid EVM address.
 * @param address The address to check. Should be "0x" followed by 40 characters ranging from [0-9] or [a-f]. Either
 * all uppercase, all lowercase, or checksum-cased.
 * @param lenient defaults to false, which means only checksum-cased is considered valid.
 * @returns 
 */
export function isValidEvmAddress(address: string, lenient: boolean = false){
	if(typeof address !== "string"){
		return false;
	}
	if(lenient){
		return (
			/^(0x|0X)?[0-9a-f]{40}$/.test(address) ||
			/^(0x|0X)?[0-9A-F]{40}$/.test(address) ||
			isValidEvmAddressChecksum(address)
		);
	}
	return isValidEvmAddressChecksum(address);
};
