import { DeliverTxResponse } from "@cosmjs/stargate";
import { ReceiptInformation as EvmReceiptInformation } from "@crownfi/ethereum-rpc-types";

/**
 * Checks to see if the error is a failed transaction error
 * @param e exception to check if it might be a transaction error
 * @returns whether or not it might be
 */
export function isProbablyTxError(e: any): e is {name: string, message: string} {
	return e != null &&
		typeof e.name == "string" &&
		typeof e.message == "string" &&
		e.message.match(/\.go\:\d+\]/m);
}

/**
 * Extracts information that's actually useful to the user from a transaction error
 * `messageIndex` is which message in the transaction threw the error
 * `errorSource` is the type of error
 * `errorDetail` is the error message
 * @param message
 * @returns data described above
 */
export function makeTxExecErrLessFugly(
	message: string
): { messageIndex: string; errorSource: string; errorDetail: string } | null {
	let betterErrorFormat =
		/failed to execute message; message index\:\s*?(\d+)\:\s+?(?:dispatch: submessages: )*(.*)\:\s+?(.*?)\s*?\[/.exec(
			message
		);
	if (betterErrorFormat) {
		const messageIndex = betterErrorFormat[1];
		let errorSource = betterErrorFormat[3];
		if (errorSource == "execute wasm contract failed") {
			errorSource = "Contract returned an error";
		}
		const errorDetail = betterErrorFormat[2];
		return {
			messageIndex,
			errorSource,
			errorDetail,
		};
	} else {
		return null;
	}
}

export function makeQueryErrLessFugly(message: string): { errorSource: string; errorDetail: string } | null {
	let betterErrorFormat = /^Query failed with \((\d*?)\)\: (?:.*\n)*\s*(.*)\:\s*(.*)\:\s*(.*)$/.exec(message);
	if (betterErrorFormat) {
		// const errorCode = betterErrorFormat[1];
		if (betterErrorFormat[4] == "invalid request") {
			if (betterErrorFormat[3] == "query wasm contract failed") {
				return {
					errorSource: "Contract returned an error",
					errorDetail: betterErrorFormat[2],
				};
			} else {
				return {
					errorSource: "Invalid query",
					errorDetail: betterErrorFormat[2] + ": " + betterErrorFormat[3],
				};
			}
		} else {
			return {
				errorSource: betterErrorFormat[4],
				errorDetail: betterErrorFormat[2] + ": " + betterErrorFormat[3],
			};
		}
	} else {
		return null;
	}
}

export function isEvmReceipt(tx: EvmReceiptInformation | DeliverTxResponse): tx is EvmReceiptInformation {
	return tx.transactionHash.startsWith("0x");
}

export function transactionSucceeded(tx: EvmReceiptInformation | DeliverTxResponse): boolean {
	// 1 ("0x1") means success in Ethereum-land, 0 means success in Cosmos-land.
	return isEvmReceipt(tx) ? Boolean(Number(tx.status)) : !tx.code;
}
