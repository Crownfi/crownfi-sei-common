
export class ClientAccountMissingError extends Error {
	name!: "ClientAccountMissingError";
}
ClientAccountMissingError.prototype.name == "ClientAccountMissingError";

export class ClientNotSignableError extends Error {
	name!: "ClientNotSignableError";
}
ClientNotSignableError.prototype.name == "ClientNotSignableError";

export class ClientPubkeyUnknownError extends Error {
	name!: "ClientPubkeyUnknownError";
}
ClientPubkeyUnknownError.prototype.name == "ClientPubkeyUnknownError";

class EVMABIParseError extends Error {
	name!: "EVMABIParseError";
}
EVMABIParseError.prototype.name = "EVMABIParseError";

class EVMContractRevertError extends Error {
	name!: "EVMContractRevertError";
}
EVMContractRevertError.prototype.name = "EVMContractRevertError";
