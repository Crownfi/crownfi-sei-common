import { Coin } from "@cosmjs/amino";
import { SeiChainId, getDefaultNetworkConfig } from "./chain_config.js";

/**
 * Used for `Array.prototype.sort` when dealing with `Coin[]`'s
 */
export function nativeDenomSortCompare(a: Coin, b: Coin) {
	if (a.denom < b.denom) {
		return -1;
	} else if (a.denom > b.denom) {
		return 1;
	}
	return 0;
}

// Temporary hard-coded info until our API is up and running
const userTokenInfo: { [denom: string]: UserTokenInfo } = {
	usei: {
		name: "Sei",
		symbol: "SEI",
		decimals: 6,
		icon: "https://app.crownfi.io/assets/coins/sei.svg",
	},
	uusdc: {
		name: "USD Coin",
		symbol: "USDC",
		decimals: 6,
		icon: "https://app.crownfi.io/assets/coins/usdc.svg",
	},
	"factory/sei1ug2zf426lyucgwr7nuneqr0cymc0fxx2qjkhd8/test-ln4z7ryp": {
		name: "CrownFi Native Test Token 1",
		symbol: "TESTN1",
		decimals: 6,
		icon: "https://app.crownfi.io/assets/placeholder.svg",
	},
	"cw20/sei17k6s089jcg3d02ny2h3a3z675307a9j8dvrslsrku6rkawe5q73q9sygav": {
		name: "CrownFi CW20 Test Token 1",
		symbol: "TESTC1",
		decimals: 6,
		icon: "https://app.crownfi.io/assets/placeholder.svg",
	},
};

export type UserTokenInfo = {
	name: string;
	symbol: string;
	decimals: number;
	icon: string;
};

/**
 * Returns the user token info for the given denom. If there is none, fake data is returned.
 */
export function getUserTokenInfo(unifiedDenom: string): UserTokenInfo {
	return (
		userTokenInfo[unifiedDenom] ?? {
			name: "Unknown token (" + unifiedDenom + ")",
			symbol: "(" + unifiedDenom + ")",
			decimals: 0,
			icon: "https://app.crownfi.io/assets/placeholder.svg",
		}
	);
}

/**
 * Updates the data returned by getUserTokenInfo
 * @param network The sei network to use, defaults to `getDefaultNetworkConfig().chainId`
 * @param apiEndpoint API Endpoint to get the coin data from, defaults to crownfi
 * @returns
 */
export function updateUserTokenInfo(
	network: SeiChainId = getDefaultNetworkConfig().chainId,
	apiEndpoint?: string
): Promise<void> {
	// Noop until our API is up and running
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
 * @param unifiedDenom the denom of the token, or "cw20/{address}" for a cw20 token
 * @param trimTrailingZeros If true, the result won't have trailing zeros. e.g. "0.100000" becomes "0.1"
 * @returns A user friendly string, e.g. "1.234567 SEI"
 */
export function UIAmount(
	amount: bigint | string | number,
	unifiedDenom: string,
	trimTrailingZeros: boolean = false
): string {
	const tokenUserInfo = getUserTokenInfo(unifiedDenom);
	if (typeof amount == "string") {
		amount = BigInt(amount);
	}
	return bigIntToStringDecimal(amount, tokenUserInfo.decimals, trimTrailingZeros) + " " + tokenUserInfo.symbol;
}
