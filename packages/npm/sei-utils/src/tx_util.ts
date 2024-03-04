/**
 * Checks to see if the error is a failed transaction error
 * @param e exception to check if it might be a transaction error
 * @returns whether or not it might be
 */
export function isProbablyTxError(e: any): boolean {
	return typeof e.message == "string" && e.message.match(/\.go\:\d+\]/m);
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
