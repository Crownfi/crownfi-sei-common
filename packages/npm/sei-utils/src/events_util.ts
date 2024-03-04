import { Event } from "@cosmjs/stargate";
import { Addr } from "./common_sei_types.js";

export function eventAttributeMatches(event: Event, index: number, key: string, value?: string): boolean {
	const attribute = event.attributes[index];
	if (attribute === undefined) {
		return false;
	}
	if (attribute.key != key) {
		return false;
	}
	if (typeof value == "string" && attribute.value != value) {
		return false;
	}
	return typeof value != "string" || attribute.value == value;
}

export type Cw20TransferEventAttributes = [
	{ readonly key: "_contract_address"; readonly value: Addr },
	{ readonly key: "action"; readonly value: "transfer" },
	{ readonly key: "from"; readonly value: Addr },
	{ readonly key: "to"; readonly value: Addr },
	{ readonly key: "amount"; readonly value: string },
];

export function isCw20TransferEvent(
	event: Event
): event is { readonly type: "wasm"; readonly attributes: Cw20TransferEventAttributes } {
	return (
		event.type == "wasm" &&
		event.attributes.length == 5 &&
		eventAttributeMatches(event, 0, "_contract_address") &&
		eventAttributeMatches(event, 1, "action", "transfer") &&
		eventAttributeMatches(event, 2, "from") &&
		eventAttributeMatches(event, 3, "to") &&
		eventAttributeMatches(event, 4, "amount")
	);
}

export function parseStringifiedCoin(strCoin: string): [bigint, string] {
	for (let i = 0; i < strCoin.length; i += 1) {
		const char = strCoin[i];
		if (char < "0" || char > "9") {
			return [BigInt(strCoin.substring(0, i)), strCoin.substring(i)];
		}
	}
	return [BigInt(strCoin), ""];
}

export type BalanceChangeResult = { [denom: string]: bigint };
export function getBalanceChangesFor(
	address: Addr,
	events: Event[],
	includeCw20Transfers: boolean = false
): BalanceChangeResult {
	const result: BalanceChangeResult = {};
	for (const event of events) {
		if (event.type == "coin_spent") {
			if (eventAttributeMatches(event, 0, "spender", address)) {
				const [amount, denom] = parseStringifiedCoin(event.attributes[1].value);
				if (result[denom] == null) {
					result[denom] = 0n;
				}
				result[denom] -= amount;
			}
		} else if (event.type == "coin_received") {
			if (eventAttributeMatches(event, 0, "receiver", address)) {
				const [amount, denom] = parseStringifiedCoin(event.attributes[1].value);
				if (result[denom] == null) {
					result[denom] = 0n;
				}
				result[denom] += amount;
			}
		} else if (includeCw20Transfers && isCw20TransferEvent(event)) {
			const denom = "cw20/" + event.attributes[0].value;
			const amount = BigInt(event.attributes[4].value);
			if (event.attributes[2].value == address) {
				if (result[denom] == null) {
					result[denom] = 0n;
				}
				result[denom] -= amount;
			} else if (event.attributes[3].value == address) {
				if (result[denom] == null) {
					result[denom] = 0n;
				}
				result[denom] += amount;
			}
		}
	}
	return result;
}
