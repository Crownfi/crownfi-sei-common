import { addErrorMsgFormatter } from "@crownfi/css-gothic-fantasy";
import { ClientAccountMissingError, ClientNotSignableError, ClientPubkeyUnknownError, isProbablyTxError, makeTxExecErrLessFugly } from "@crownfi/sei-utils";


addErrorMsgFormatter((err: any) => {
	if (!(err instanceof ClientAccountMissingError)) {
		return null;
	}
	return {
		title: "No account",
		message: err.message,
		dialogIcon: "info",
		hideDetails: true
	}
});
addErrorMsgFormatter((err: any) => {
	if (!(err instanceof ClientNotSignableError)) {
		return null;
	}
	return {
		title: "Account cannot sign",
		message: err.message,
		dialogIcon: "key",
		hideDetails: true
	}
});
addErrorMsgFormatter((err: any) => {
	if (!(err instanceof ClientPubkeyUnknownError)) {
		return null;
	}
	return {
		title: "Public key not found",
		message: err.message + "\nA public key can only be known if a wallet is connected or if the address has had a recent transaction.",
		dialogIcon: "key",
		hideDetails: true
	}
});
addErrorMsgFormatter((err: any) => {
	if (!isProbablyTxError(err)) {
		return null;
	}
	const errorParts = makeTxExecErrLessFugly(err.message);
	if (errorParts) {
		const {messageIndex, errorSource, errorDetail} = errorParts;
		return {
			title: "Transaction Execution Error",
			message: "Message #" + messageIndex + " failed.\n" + errorSource + ": " + errorDetail,
			dialogIcon: "warning",
			dialogClass: "warning"
		};
	}
	return {
		title: "RPC Error",
		message: err.message,
		dialogIcon: "warning",
		dialogClass: "warning"
	}
});
