import { TimeoutError } from "@cosmjs/stargate";
import { addErrorMsgFormatter } from "@crownfi/css-gothic-fantasy";
import { ClientAccountMissingError, ClientNotSignableError, ClientPubkeyUnknownError, EvmAddressValidationMismatchError, NetworkEndpointNotConfiguredError, getDefaultNetworkConfig, isProbablyTxError, makeTxExecErrLessFugly } from "@crownfi/sei-utils";

addErrorMsgFormatter((err: any) => {
	if (!isProbablyTxError(err)) {
		return null;
	}
	if (err.message.includes("sei does not support EVM->CW->EVM call pattern")) {
		const message = "Sei's Solidity to CosmWasm interopability is incomplete. As a consequence, Sei currently " +
			"doesn't allow you to make trades which result in ERC20 tokens as output on our platform. This is a known " +
			"issue and we're currently awaitng for the Sei team to fix this on their end.\n" +
			"This error message will stop happening as soon as the network is upgraded to fix this.";
		return {
			title: "The Sei team makes CrownFi's CTO cry...",
			message,
			dialogIcon: "cry",
			dialogClass: "error"
		}
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
		title: "Sei RPC Error",
		message: "An error was returned by the Sei-native node:\n" + err.message,
		dialogIcon: "warning",
		dialogClass: "warning"
	}
});

addErrorMsgFormatter((err: any) => {
	if (typeof err.code !== "number" || typeof err.message !== "string") {
		return null;
	}
	if (err.code == 4001) {
		return {
			title: "Cancelled",
			message: "You've successfully asserted your right to say \"no\".",
			dialogIcon: "check-circle-o",
			dialogClass: "success"
		}
	}
	if (err.code == 4100) {
		return {
			title: "Permission denied",
			message: "Your wallet denied the request. If it wasn't your intent to do so, please try again or try re-connecting.",
			dialogIcon: "info"
		}
	}
	return {
		title: "Sei-EVM RPC Error",
		message: "An error was returned by your wallet or the Sei-EVM node:\n" +
			err.code + " - " + err.message,
		dialogIcon: "warning",
		dialogClass: "warning"
	}
});


addErrorMsgFormatter((err: any) => {
	if (!(err instanceof ClientAccountMissingError)) {
		return null;
	}
	return {
		title: "No account provided",
		message: err.message,
		dialogIcon: "warning",
		dialogClass: "warning",
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
		dialogClass: "warning",
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
		dialogIcon: "warning",
		dialogClass: "warning",
		hideDetails: true
	}
});
addErrorMsgFormatter((err: any) => {
	if (!(err instanceof NetworkEndpointNotConfiguredError)) {
		return null;
	}
	return {
		title: "Invalid Sei network",
		message: "Couldn't connect to the \"" + err.attemptedChainId +
			"\" network as there's isn't an endpoint configured for it. The \"" + err.fallbackChainId +
			"\" network will be used instead.",
		dialogIcon: "warning",
		dialogClass: "warning",
		hideDetails: true
	}
});
addErrorMsgFormatter((err: any) => {
	if (!(err instanceof EvmAddressValidationMismatchError)) {
		return null;
	}
	return {
		title: "Address validation error",
		message: "Address \"" + err.expected + "\" was expected, but \"" + err.actual + "\" was given.",
		dialogIcon: "warning",
		dialogClass: "warning",
		hideDetails: true
	}
})
addErrorMsgFormatter((err: any) => {
	if (!(err instanceof TimeoutError)) {
		return null;
	}
	const message1 = document.createElement("p");
	message1.innerText = err.message;
	const message2 = document.createElement("p");
	message1.innerHTML = `<a href="https://seitrace.com/tx/${err.txId}?chain=${getDefaultNetworkConfig().chainId}" target="_blank">View on seitrace</a><span class="cicon cicon-link-external"></span>`;
	return {
		title: "Transaction timed out",
		message: [message1, message2],
		dialogIcon: "warning",
		dialogClass: "warning",
		hideDetails: true
	}
});
