
export class ClientAccountMissingError extends Error {
	name!: "ClientAccountMissingError";
}
ClientAccountMissingError.prototype.name = "ClientAccountMissingError";

export class ClientNotSignableError extends Error {
	name!: "ClientNotSignableError";
}
ClientNotSignableError.prototype.name = "ClientNotSignableError";

export class ClientPubkeyUnknownError extends Error {
	name!: "ClientPubkeyUnknownError";
}
ClientPubkeyUnknownError.prototype.name = "ClientPubkeyUnknownError";

export class NetworkEndpointNotConfiguredError extends Error {
	name!: "NetworkEndpointNotConfiguredError";
	attemptedChainId: string;
	fallbackChainId: string;
	constructor(attemptedChainId: string, fallbackChainId: string) {
		// Fallback chain not in the message as that's only relevant if you catch the error.
		super("No endpoint configured for Sei network \"" + attemptedChainId + "\"");
		this.attemptedChainId = attemptedChainId;
		this.fallbackChainId = fallbackChainId;
	}
}
NetworkEndpointNotConfiguredError.prototype.name = "NetworkEndpointNotConfiguredError";

class EVMABIParseError extends Error {
	name!: "EVMABIParseError";
}
EVMABIParseError.prototype.name = "EVMABIParseError";

class EVMContractRevertError extends Error {
	name!: "EVMContractRevertError";
}
EVMContractRevertError.prototype.name = "EVMContractRevertError";
