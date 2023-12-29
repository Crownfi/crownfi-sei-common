import { Addr, Uint128 } from "./common_sei_types.js";
import { UIAmount } from "./funds_util.js";

export type AstroportAssetInfo = {
	token: {
		contract_addr: Addr;
	};
} | {
	native_token: {
		denom: string;
	};
};
export interface AstroportAsset {
	/**
	 * A token amount
	 */
	amount: Uint128;
	/**
	 * Information about an asset stored in a [`AssetInfo`] struct
	 */
	info: AstroportAssetInfo;
}

export function astroportAssetToAmountWithDenom(asset: AstroportAsset): [bigint, string] {
	if ("token" in asset.info) {
		return [BigInt(asset.amount), "cw20/" + asset.info.token.contract_addr]
	}else{
		return [BigInt(asset.amount), asset.info.native_token.denom]
	}
}

export function amountWithDenomToAstroportAsset(amount: bigint | string | number, unifiedDenom: string): AstroportAsset {
	return {
		amount: amount.toString(),
		info: denomToAstroportAssetInfo(unifiedDenom)
	}
}
export function denomToAstroportAssetInfo(unifiedDenom: string): AstroportAssetInfo {
	if (unifiedDenom.startsWith("cw20/")) {
		return {
			token: {contract_addr: unifiedDenom.substring("cw20/".length)}
		}
	}else{
		return {
			native_token: {denom: unifiedDenom}
		}
	}
}

export function UIAstroportAsset(asset: AstroportAsset, trimTrailingZeros: boolean = false): string {
	return UIAmount(...astroportAssetToAmountWithDenom(asset), trimTrailingZeros);
}
