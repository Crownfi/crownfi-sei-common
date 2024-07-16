import { TimeoutError } from "@cosmjs/stargate";
import { addErrorMsgFormatter } from "@crownfi/css-gothic-fantasy";
import { ClientAccountMissingError, ClientNotSignableError, ClientPubkeyUnknownError, ContractVersionNotSatisfiedError, EvmAddressValidationMismatchError, NetworkEndpointNotConfiguredError, getDefaultNetworkConfig, makeQueryErrLessFugly, makeTxExecErrLessFugly } from "@crownfi/sei-utils";

addErrorMsgFormatter((err: any) => {
	if (!err || typeof err.message != "string") {
		return null;
	}
	const errorParts = makeQueryErrLessFugly(err.message);
	if (errorParts == null) {
		return null;
	}
	const {errorSource, errorDetail} = errorParts;
	return {
		title: "Sei network: " + errorSource,
		message: errorDetail,
		dialogIcon: "warning",
		dialogClass: "warning"
	};
});

addErrorMsgFormatter((err: any) => {
	if (!err || typeof err.message != "string") {
		return null;
	}
	const errorParts = makeTxExecErrLessFugly(err.message);
	if (errorParts == null) {
		return null;
	}
	const {messageIndex, errorSource, errorDetail} = errorParts;
	return {
		title: "Transaction Execution Error",
		message: "Message #" + messageIndex + " failed.\n" + errorSource + ": " + errorDetail,
		dialogIcon: "warning",
		dialogClass: "warning"
	};
});

addErrorMsgFormatter((err: any) => {
	if (!err || typeof err.message !== "string" || !err.message.includes("does not support EVM->CW->EVM call pattern")) {
		return null;
	}
	return {
		title: "Sei node needs updating",
		message: "Trades using Ethereum-based wallets which result in ERC20 tokens as output are dependent on \
			versions of Sei released after June 17th 2024.\n\
			Seems like the connected network hasn't upgraded yet.",
		dialogIcon: "cry",
		dialogClass: "error"
	}
});

addErrorMsgFormatter((err: any) => {
	if (
		!err ||
		typeof err.code !== "number" ||
		typeof err.message !== "string" ||
		err.codespace != undefined ||
		err.log != undefined
	) {
		return null;
	}
	if (err instanceof DOMException) {
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
addErrorMsgFormatter((err: any) => {
	if (!(err instanceof ContractVersionNotSatisfiedError)) {
		return null;
	}
	const msg1 = document.createElement("p");
	msg1.innerText = `The contract located at ${
		err.contractAddress
	} was expected to have${
		Object.keys(err.expectedVersions).length > 1 ? " one of" : ""
	} the following version information:`;
	const msg2 = document.createElement("ul");
	for (const contractName in err.expectedVersions) {
		const li = document.createElement("li");
		li.innerText = `${contractName}@${err.expectedVersions[contractName]}`;
		msg2.appendChild(li);
	}
	const msg3 = document.createElement("p");
	if (err.actualVersionInfo) {
		msg3.innerText = "However, the following incompatible version information was returned: " +
			`${err.actualVersionInfo.name}@${err.actualVersionInfo.version}`;
	} else {
		msg3.innerText = "However, no version information was found. Perhaps the contract doesn't exist.";
	}
	return {
		title: "Incompatible contract",
		message: [msg1, msg2, msg3],
		dialogIcon: "warning",
		dialogClass: "warning",
		hideDetails: true
	}
});
