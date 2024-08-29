import { Coin } from "@cosmjs/amino";
import { SeiChainId, getDefaultNetworkConfig } from "./chain_config.js";
import { BankExtension, QueryClient } from "@cosmjs/stargate";
import { WasmExtension } from "@cosmjs/cosmwasm-stargate";
import { SeiEvmExtension } from "@crownfi/sei-js-core";
import { queryEvmContract } from "./evm-interop-utils/query_contract.js";
import { ERC20_FUNC_DECIMALS, ERC20_FUNC_NAME, ERC20_FUNC_SYMBOL } from "./evm-interop-utils/erc20.js";

/**
 * This represents either a native token, or a contract token.
 * * CW20 tokens are represented with `"cw20/{contractAddress}"`
 * * ERC20 tokens are represented with `"erc20/{contractAddress}"`
 * * Everything else is assumed to be a native token
 */
export type UnifiedDenom = string;

/**
 * Like cosmjs's `Coin`, but with `amount` being a `bigint` instead of a `string`.
 * 
 * If you want a helper function for constructing 
 */
export interface IBigIntCoin {
	amount: bigint,
	denom: string
}

/**
 * @deprecated use {@link IBigIntCoin} instead
 */
export interface BigIntCoinObj extends IBigIntCoin {}

/**
 * Like cosmjs's `Coin`, but with `amount` being a `bigint` instead of a `string`.
 * 
 * This implements {@link IBigIntCoin}, only difference being is that this class has helper methods.
 * 
 */
export class BigIntCoin implements IBigIntCoin {
	amount: bigint;
	denom: string;
	constructor(object: IBigIntCoin | Coin)
	constructor(amount: string | bigint | number, denom: string)
	constructor(amountOrObject: string | bigint | number | IBigIntCoin | Coin, denom?: string) {
		if (typeof amountOrObject == "object") {
			this.amount = BigInt(amountOrObject.amount);
			this.denom = amountOrObject.denom;
		} else {
			this.amount = BigInt(amountOrObject);
			this.denom = denom + "";
		}
	}
	/**
	 * @returns an object with the `amount` as a string and `denom` unchanged.
	 */
	intoCosmCoin(): Coin {
		return {
			denom: this.denom,
			amount: this.amount + ""
		};
	}
	/**
	 * Returns a string which displays this object's amount and denom in a user-friendly way.
	 * 
     * @param trimTrailingZeros If true, the result won't have trailing zeros. e.g. "0.100000" becomes "0.1"
     * @returns A user friendly string, e.g. "1.234567 SEI"
	 */
	UIAmount(trimTrailingZeros: boolean = false): string {
		return UIAmount(this.amount, this.denom, trimTrailingZeros);
	}
}

export interface ExternalAssetListItem {
	name: string,
	description: string,
	symbol: string,
	base: string,
	display: string,
	denom_units: {denom: string, exponent: number}[],
	images: {
		svg?: string | undefined,
		png?: string | undefined,
		[extension: string]: string | undefined
	},
	type_asset: "sdk.coin" | "erc20" | "cw20" | "ics20",
	pointer_contract: {
        address: string,
        type_asset: "erc20" | "cw20"
	}
}

/**
 * Used for `Array.prototype.sort` when dealing with `Coin[]`'s
 */
export function nativeDenomSortCompare<T extends {denom: string}>(a: T, b: T) {
	if (a.denom < b.denom) {
		return -1;
	} else if (a.denom > b.denom) {
		return 1;
	}
	return 0;
}

// Temporary hard-coded info until our API is up and running
/*
const userTokenInfo: { [denom: string]: UserTokenInfo } = {
	usei: {
		name: "Sei",
		symbol: "SEI",
		decimals: 6,
		icon: "https://www.crownfi.io/assets/coins/sei.svg",
		base: "usei"
	},
};
*/

const userTokenInfoAliases: { [network:string]: { [denom: string]: string } } = {};
const userTokenInfo: { [network:string]: { [denom: string]: UserTokenInfo } } = {};

export type UserTokenInfo = {
	name: string;
	symbol: string;
	decimals: number;
	icon: string;
	base: string;
};
export type PartialUserTokenInfo = {
	name?: string;
	symbol?: string;
	decimals?: number;
	icon?: string;
	base: string;
};

export function matchTokenType<T>(
	unifiedDenom: UnifiedDenom,
	ifCW20Callback: (contractAddress: string) => T,
	ifERC20Callback: (contractAddress: string) => T,
	ifNativeCallback: (denom: string) => T
): T {
	if (unifiedDenom.startsWith("cw20/")) {
		return ifCW20Callback(unifiedDenom.substring(5)); // "cw20/".length
	} else if (unifiedDenom.startsWith("erc20/")) {
		return ifERC20Callback(unifiedDenom.substring(6)); // "erc20/".length
	} else {
		return ifNativeCallback(unifiedDenom);
	}
}

let centralAssetListInfoPromise: Promise<void> | null = null;

/**
 * Sets the specified token info which can later be retrieved via `getUserTokenInfo`
 * 
 * If any fields are unspecified, they will be searched for from the following in order of priority:
 * * The "Sei-Public-Goods" centralized asset list
 * * Metadata returned from the contract (if it's a contract token)
 * * the `denoms_metadata` Sei query (if it's a native token)
 * 
 * @param queryClient 
 * @param baseOrPartialTokenInfo 
 */
export async function addUserTokenInfo(
	queryClient: QueryClient & WasmExtension & SeiEvmExtension & BankExtension,
	network: string,
	baseOrPartialTokenInfo: string | PartialUserTokenInfo
) {
	if (centralAssetListInfoPromise == null) {
		centralAssetListInfoPromise = (async () => {
			try {
				const assetList = await (
					await fetch("https://www.crownfi.io/external/asset_list.json")
				).json() as {[network: string]: ExternalAssetListItem[]};
				for (const network in assetList) {
					for (let i = 0; i < assetList[network].length; i += 1) {
						const assetListItem = assetList[network][i];
						const base = (() => {
							switch (assetListItem.type_asset) {
								case "sdk.coin":
								case "ics20":
									return assetListItem.base;
								case "cw20":
									return "cw20/" + assetListItem.base;
								case "erc20":
									return "erc20/" + assetListItem.base;
								default:
									console.warn(
										"@crownfi/sei-utils: Central asset list contains unknown type_asset:",
										assetListItem.type_asset
									);
									return "";
							}
						})();
						if (!base) {
							continue;
						}
						userTokenInfo[network] = userTokenInfo[network] || {};
						userTokenInfo[network][base] = {
							base,
							name: assetListItem.name,
							symbol: assetListItem.symbol,
							decimals: assetListItem.denom_units.find(
								unit => unit.denom == assetListItem.display
							)?.exponent || 0,
							icon: assetListItem.images.svg ? assetListItem.images.svg : (
								assetListItem.images.png ? assetListItem.images.png : (
									Object.values(assetListItem.images)[0] ||
										"https://www.crownfi.io/assets/placeholder.svg"
								)
							)
						};
					}
				}
			}catch(ex: any) {
				// Allow for retries later
				console.error("Failed to get central asset list:", ex);
				centralAssetListInfoPromise = null;
				throw ex;
			}
		})();
	}
	await centralAssetListInfoPromise;
	const unifiedDenom = typeof baseOrPartialTokenInfo == "object" ?
		baseOrPartialTokenInfo.base : baseOrPartialTokenInfo;
	const providedInfo: PartialUserTokenInfo = typeof baseOrPartialTokenInfo == "object" ?
		baseOrPartialTokenInfo : {base: baseOrPartialTokenInfo};
	
	userTokenInfo[network] = userTokenInfo[network] || {};
	if (userTokenInfo[network][unifiedDenom]) {
		if (providedInfo.decimals) {
			userTokenInfo[network][unifiedDenom].decimals = providedInfo.decimals;
		}
		if (providedInfo.icon) {
			userTokenInfo[network][unifiedDenom].icon = providedInfo.icon;
		}
		if (providedInfo.name) {
			userTokenInfo[network][unifiedDenom].name = providedInfo.name;
		}
		if (providedInfo.symbol) {
			userTokenInfo[network][unifiedDenom].symbol = providedInfo.symbol;
		}
		return;
	}
	
	await matchTokenType(
		unifiedDenom,
		async (wasmAddress) => {
			if (!providedInfo.name || !providedInfo.symbol || !providedInfo.decimals) {
				const {
					name,
					symbol,
					decimals
				} = await queryClient.wasm.queryContractSmart(
					wasmAddress,
					{token_info: {}} /* satisfies Cw20QueryMsg */
				); /* as Cw20TokenInfoResponse*/
				if (!providedInfo.name) {
					providedInfo.name = name + "";
				}
				if (!providedInfo.symbol) {
					providedInfo.symbol = symbol + "";
				}
				if (!providedInfo.decimals) {
					providedInfo.decimals = Number(decimals) || 0;
				}
			}
			if (!providedInfo.icon) {
				const {
					logo
				} = await queryClient.wasm.queryContractSmart(
					wasmAddress,
					{marketing_info: {}} /* satisfies Cw20QueryMsg */
				); /* as Cw20MarketingInfoResponse*/
				if (logo && "url" in logo) {
					providedInfo.icon = logo.url;
				} else {
					try {
						const {
							mime_type,
							data: img_data
						} = await queryClient.wasm.queryContractSmart(
							wasmAddress,
							{download_logo: {}} /* satisfies Cw20QueryMsg */
						); /* as Cw20DownloadLogoResponse*/
						providedInfo.icon = "data:" + mime_type + ";base64," + img_data;
					} catch (ex: any) {
						console.warn("Could not download icon for " + unifiedDenom + ":", ex);
					}
				}
			}
		},
		async (evmAddress) => {
			const [
				[name],
				[symbol],
				[decimals]
			] = await Promise.all([
				queryEvmContract(queryClient, evmAddress, ERC20_FUNC_NAME, []),
				queryEvmContract(queryClient, evmAddress, ERC20_FUNC_SYMBOL, []),
				queryEvmContract(queryClient, evmAddress, ERC20_FUNC_DECIMALS, [])
			]);
			if (!providedInfo.name) {
				providedInfo.name = name + "";
			}
			if (!providedInfo.symbol) {
				providedInfo.symbol = symbol + "";
			}
			if (!providedInfo.decimals) {
				providedInfo.decimals = Number(decimals) || 0;
			}
		},
		async (denom) => {
			const metadata = await queryClient.bank.denomMetadata(denom);
			providedInfo.base = metadata.base;
			if (!providedInfo.name) {
				providedInfo.name = metadata.name;
			}
			if (!providedInfo.symbol) {
				providedInfo.symbol = metadata.symbol;
			}
			if (!providedInfo.decimals) {
				providedInfo.decimals = metadata.denomUnits.find(
					unit => unit.denom == metadata.display
				)?.exponent || 0;
			}
			if (!providedInfo.icon) {
				let iconMatch = metadata.description.match(/\[logo_uri\]\((.+?)\)/);
				if (iconMatch != null) {
					providedInfo.icon = iconMatch[1];
				} else {
					iconMatch = metadata.description.match(/\[ipfs_cid\]\(([a-zA-Z0-9]+?)\)/);
					if (iconMatch != null) {
						providedInfo.icon = "https://ipfs.io/ipfs/" + iconMatch[1];
					}
				}
			}
		}
	)
	if (!providedInfo.icon) {
		if (/(\/|%2f)/i.test(providedInfo.symbol!)) {
			providedInfo.icon = "https://www.crownfi.io/assets/placeholder.svg";
		} else {
			const fallbackIcon = "https://www.crownfi.io/assets/coins/" + providedInfo.symbol!.toLowerCase() + ".svg";
			const contentType = (await fetch(fallbackIcon, {method: "HEAD"})).headers.get("content-type") + "";
			if (contentType.startsWith("image/")) {
				providedInfo.icon = fallbackIcon;
			} else {
				providedInfo.icon = "https://www.crownfi.io/assets/placeholder.svg";
			}
		}
	}
	userTokenInfo[network][providedInfo.base] = providedInfo as UserTokenInfo;
}

/**
 * Returns the user token info for the given denom. If there is none, fake data is returned.
 */
export function getUserTokenInfo(
	unifiedDenom: string,
	network: SeiChainId = getDefaultNetworkConfig().chainId
): UserTokenInfo {
	if (userTokenInfoAliases[network] && userTokenInfoAliases[network][unifiedDenom]) {
		unifiedDenom = userTokenInfoAliases[network][unifiedDenom];
	}
	return (
		(userTokenInfo[network] ?? {})[unifiedDenom] ?? {
			name: "Unknown token (" + unifiedDenom + ")",
			symbol: "(" + unifiedDenom + ")",
			decimals: 0,
			icon: "https://www.crownfi.io/assets/placeholder.svg",
			base: unifiedDenom
		}
	);
}

export function addUserTokenInfoAliases(
	aliasAsset: string,
	realAsset: string,
	network: SeiChainId = getDefaultNetworkConfig().chainId
) {
	userTokenInfoAliases[network] = userTokenInfoAliases[network] || {};
	userTokenInfoAliases[network][aliasAsset] = realAsset;
}

export function hasUserTokenInfo(
	unifiedDenom: string,
	network: SeiChainId = getDefaultNetworkConfig().chainId
): boolean {
	return userTokenInfo[network] != null && userTokenInfo[network][unifiedDenom] != null;
}

/**
 * @deprecated Does nothing
 */
export function updateUserTokenInfo(
	network: SeiChainId = getDefaultNetworkConfig().chainId,
	apiEndpoint?: string
): Promise<void> {
	return Promise.resolve();
}

/**
 * Math.abs but for bigints
 */
export function bigIntAbs(val: bigint): bigint {
	if (val < 0n) {
		return val * -1n;
	}
	return val;
}

/**
 * Multiplies `rawAmount` by `10 ** -decimals` and returns the result as a string without losing precision
 * @param rawAmount The integer value
 * @param decimals The amount of places to move the decimal point to the left by
 * @param trimTrailingZeros If true, the result won't have trailing zeros. e.g. "0.100000" becomes "0.1"
 * @returns The result represented as a string
 */
export function bigIntToStringDecimal(
	rawAmount: bigint | number,
	decimals: number,
	trimTrailingZeros: boolean = false
): string {
	let result: string;
	if (typeof rawAmount === "number") {
		if (decimals == 0) {
			return Math.floor(rawAmount).toString();
		}
		// Can't use .toString() as that switches to scientific notations with values smaller than 0.000001
		result = (rawAmount / 10 ** decimals).toFixed(decimals);
	} else {
		if (decimals == 0) {
			return rawAmount.toString();
		}
		const divisor = 10n ** BigInt(Math.trunc(decimals));
		result =
			(rawAmount / divisor).toString() +
			"." +
			(bigIntAbs(rawAmount) % divisor).toString().padStart(decimals, "0");
	}
	if (trimTrailingZeros) {
		return result.replace(/\.(\d*?)0+$/, (_, g1) => {
			if (!g1) {
				return "";
			}
			return "." + g1;
		});
	}
	return result;
}
/**
 * Multiplies `str` by `10 ** decimals` without losing precision while converting to a bigint. Truncating the result.
 * @param str The decimal value represented as a string
 * @param decimals The amount of places to move the decimal point to the right by
 * @returns The result as a bigint, or null if the input wasn't a valid number.
 */
export function stringDecimalToBigInt(str: string | number, decimals: number): bigint | null {
	str = String(str);
	const matches = str.match(/(-?)(\d*)(?:\.(\d+)|\.)?/);
	if (matches == null) {
		return null;
	}
	const [_, sign, intPart, decimalPart] = matches;
	if (!intPart && !decimalPart) {
		return null;
	}
	const multiplier = 10n ** BigInt(Math.trunc(decimals));
	const result =
		BigInt(intPart || "") * multiplier +
		BigInt(((decimalPart || "").substring(0, decimals) || "").padEnd(decimals, "0"));
	if (sign) {
		return result * -1n;
	}
	return result;
}

/**
 * Returns a string which displays the specified amount and denom in a user-friendly way
 * @param amount the raw token amount as an integer
 * @param unifiedDenom the denom of the token
 * @param trimTrailingZeros If true, the result won't have trailing zeros. e.g. "0.100000" becomes "0.1"
 * @param showSymbol If true, token symbol will be appended to the final string
 * @returns A user friendly string, e.g. "1.234567 SEI"
 */
export function UIAmount(
	amount: bigint | string | number,
	unifiedDenom: UnifiedDenom,
	trimTrailingZeros: boolean = false,
	showSymbol: boolean = true,
): string {
	const tokenUserInfo = getUserTokenInfo(unifiedDenom);
	if (typeof amount == "string") {
		amount = BigInt(amount);
	}
	const symbol = showSymbol ? " " + tokenUserInfo.symbol : "";
	return bigIntToStringDecimal(amount, tokenUserInfo.decimals, trimTrailingZeros) + symbol;
}
