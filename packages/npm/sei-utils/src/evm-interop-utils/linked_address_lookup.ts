import { SeiQueryClient, getAddressStringFromPubKey, isValidSeiAddress, stringToCanonicalAddr } from "@crownfi/sei-js-core";
import { BaseAccount } from "cosmjs-types/cosmos/auth/v1beta1/auth.js";
import { PubKey as Secp256k1PubkeyType } from "cosmjs-types/cosmos/crypto/secp256k1/keys.js";
import { SeiClientAccountData } from "../client_env.js";
import { getEvmAddressFromPubkey, toChecksumAddressEvm } from "./address.js";
import { AssociatedEvmAddressNotFoundError } from "../error.js";

// Yes, these never get free'd. Too bad! (LRU might be good here.)
const accountDataCache: {[address: string]: SeiClientAccountData} = {};
export function addSeiClientAccountDataToCache(accountData: SeiClientAccountData) {
	accountDataCache[accountData.evmAddress] = accountData;
	accountDataCache[accountData.seiAddress] = accountData;
	if (typeof localStorage !== "undefined") {
		const accountDataJson = JSON.stringify(
			{
				seiAddress: accountData.seiAddress,
				evmAddress: accountData.evmAddress,
				pubkey: accountData.pubkey == undefined ? undefined : Buffer.from(
					accountData.pubkey.buffer,
					accountData.pubkey.byteOffset,
					accountData.pubkey.byteLength
				).toString("base64")
			}
		);
		localStorage.setItem("account_data_" + accountData.evmAddress, accountDataJson);
		localStorage.setItem("account_data_" + accountData.seiAddress, accountDataJson);
	}
}
export function getSeiClientAccountDataFromCache(seiOrEvmAddress: string): SeiClientAccountData | null {
	if (seiOrEvmAddress in accountDataCache) {
		return accountDataCache[seiOrEvmAddress];
	}
	if (typeof localStorage !== "undefined") {
		const storedData = localStorage.getItem("account_data_" + seiOrEvmAddress);
		try {
			const accountDataJsonable = JSON.parse(storedData + "");
			if (
				!isValidSeiAddress(accountDataJsonable.seiAddress) ||
				!isValidSeiAddress(accountDataJsonable.evmAddress)
			) {
				throw null;
			}
			const result: SeiClientAccountData = {
				seiAddress: accountDataJsonable.seiAddress,
				evmAddress: accountDataJsonable.evmAddress
			};
			if (accountDataJsonable.pubkey) {
				(result.pubkey as Uint8Array) = Buffer.from(accountDataJsonable.pubkey, "base64");
			}
			accountDataCache[result.evmAddress] = result;
			accountDataCache[result.seiAddress] = result;
			return result;
		} catch(ex: any) {
			// we don't care about parsing errors
		}
	}
	// Couldn't find anything in cache
	return null;
}

export async function normalizeToEvmAddress(
	queryClient: SeiQueryClient,
	seiOrEvmAddress: string,
	internalLookupTable?: {[addr: string]: string}
): Promise<string> {
	if (seiOrEvmAddress.startsWith("0x")) {
		return seiOrEvmAddress;
	}
	// The optional lookup table exists because down the line there's a eVMAddressBySeiAddress
	// I am unaware if the bank module is updated if a user _only_ made EVM transactions.
	if (internalLookupTable && internalLookupTable[seiOrEvmAddress]) {
		return internalLookupTable[seiOrEvmAddress];
	}
	const evmAddress = (await getSeiClientAccountDataFromNetwork(queryClient, seiOrEvmAddress))?.evmAddress;
	if (!evmAddress) {
		throw new AssociatedEvmAddressNotFoundError(seiOrEvmAddress);
	}
	if (internalLookupTable) {
		internalLookupTable[seiOrEvmAddress] = evmAddress;
	}
	return evmAddress;
}
export async function getSeiClientAccountDataFromNetwork(
	queryClient: SeiQueryClient,
	seiOrEvmAddress: string
): Promise<SeiClientAccountData | null> {
	const cachedResult = getSeiClientAccountDataFromCache(seiOrEvmAddress);
	if (cachedResult) {
		return cachedResult;
	}
	let evmAddress = "";
	let seiAddress = "";
	if (seiOrEvmAddress.startsWith("0x")) {
		evmAddress = seiOrEvmAddress;
		seiAddress = (await queryClient.evm.seiAddressByEVMAddress({evmAddress: seiOrEvmAddress})).seiAddress;
		if (seiAddress == "") {
			return null;
		}
	} else {
		if(!isValidSeiAddress(seiOrEvmAddress)) {
			return null;
		}
		seiAddress = seiOrEvmAddress;
	}
	const canonicalSeiAddr = stringToCanonicalAddr(seiAddress);
	if (canonicalSeiAddr.length == 32) {
		// Contract address, through light experimentations, this appears valid.
		const result = {
			seiAddress,
			evmAddress: toChecksumAddressEvm(
				"0x" + Buffer.from(
					canonicalSeiAddr.buffer,
					canonicalSeiAddr.byteOffset + 12,
					20
				).toString("hex"),
				false
			)
		};
		Object.freeze(result);
		addSeiClientAccountDataToCache(result);
		return result;
	}
	const pubkey = await (async () => {
		try{
			const baseAccountAsAny = await queryClient.auth.account(seiAddress);
			if (!baseAccountAsAny) {
				return null;
			}
			if (baseAccountAsAny.typeUrl != BaseAccount.typeUrl) {
				return null;
			}
			const baseAccount = BaseAccount.decode(baseAccountAsAny.value);
			if (baseAccount.pubKey == null || baseAccount.pubKey.typeUrl != Secp256k1PubkeyType.typeUrl) {
				return null;
			}
			return Secp256k1PubkeyType.decode(baseAccount.pubKey.value).key;
		}catch(ex: any) {
			if (typeof ex.message == "string" && ex.message.includes("key not found")) {
				return null;
			}
		}
	})();
	if (pubkey == null) {
		if (!evmAddress) {
			evmAddress = (await queryClient.evm.eVMAddressBySeiAddress({seiAddress})).evmAddress;
		}
		if (!evmAddress) {
			return null;
		}
		return {
			seiAddress,
			evmAddress
		};
	}
	if (!evmAddress) {
		evmAddress = getEvmAddressFromPubkey(pubkey);
	}
	const result = {
		seiAddress,
		evmAddress,
		pubkey
	};
	Object.freeze(result);
	addSeiClientAccountDataToCache(result);
	return result;
}
