import { isProbablyTxError, makeTxExecErrLessFugly } from "@crownfi/sei-utils";
import { addErrorMsgFormatter } from "@crownfi/css-gothic-fantasy";

addErrorMsgFormatter((err: any) => {
	if (!isProbablyTxError(err)) {
		return null;
	}
	// Note, this regex was only tested on atlantic-2
	const errParts = makeTxExecErrLessFugly(err.message);
	if (errParts) {
		const {messageIndex, errorSource, errorDetail} = errParts;
		return {
			title: "Transaction failed to execute",
			message: "Message #" + messageIndex + " failed.\n" + errorSource + ": " + errorDetail,
			dialogIcon: "warning",
			dialogClass: "warning"
		}
	}
	return {
		title: "Error returned from Sei network",
		message: err.message,
		dialogIcon: "warning",
		dialogClass: "warning"
	}
});
