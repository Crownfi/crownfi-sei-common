import { CosmWasmClient, ExecuteInstruction, SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate"
import { Addr, ContractVersionInfo } from "./common_sei_types.js";
import { Coin } from "@cosmjs/amino";
import semverSatisfies from "semver/functions/satisfies.js";

const CONTRACT_INFO_KEY = Buffer.from("contract_info");

export class ContractVersionNotSatisfiedError extends Error {
	name!: "ContractVersionNotSatisfiedError"
	contractAddress: Addr
	expectedVersions: {[name: string]: string}
	actualVersionInfo: ContractVersionInfo | null
	constructor(
		contractAddress: Addr,
		expectedVersions: {[name: string]: string},
		actualVersionInfo: ContractVersionInfo | null
	){
		if (actualVersionInfo) {
			super(
				"Expected contract " + contractAddress + " to be compatible with " + 
				Object.keys(expectedVersions).map((name) => {
					name + "@" + expectedVersions[name]
				}).join(", ") + "; " +
				"but the actual contract version information was " +
				actualVersionInfo.name + "@" + actualVersionInfo.version
			);
		}else{
			super("The contract at " + contractAddress + " has no version information; perhaps it doesn't exist")
		}
		
		this.contractAddress = contractAddress;
		this.expectedVersions = expectedVersions;
		this.actualVersionInfo = actualVersionInfo;
	}
}
ContractVersionNotSatisfiedError.prototype.name = "ContractVersionNotSatisfiedError";

/**
 * A class which is usually extended upon to generate a contract API
 */
export class ContractBase {
	address: Addr;
	endpoint: CosmWasmClient;
	/**
	 * @param endpoint The cosmwasm client
	 * @param address Contract address
	 */
	constructor(endpoint: CosmWasmClient, address: Addr) {
		this.endpoint = endpoint;
		this.address = address;
	}
	/**
	 * Reads contract state at key "contract_info" and returnes the parsed state if it exists.
	 */
	async getVersion(): Promise<ContractVersionInfo | null> {
		const storedData = await this.endpoint.queryContractRaw(this.address, CONTRACT_INFO_KEY);
		if (storedData == null) {
			return null;
		}
		return JSON.parse(Buffer.from(storedData.buffer, storedData.byteOffset, storedData.byteLength).toString());
	}
	/**
	 * Resolves the promise if the contract name matches and if the contract version satisfies the semver range
	 * specified. Otherwise, the promise is rejected with a `ContractVersionNotSatisfiedError`.
	 * @param versions A map of name => version. e.g. `{"my-awesome-contract": "^1.0.0"}`
	 */
	async checkVersion(versions: {[name: string]: string}): Promise<void> {
		const versionInfo = await this.getVersion();
		if (versionInfo == null) {
			throw new ContractVersionNotSatisfiedError(
				this.address,
				versions,
				versionInfo
			);
		}
		const expectedVersion = versions[versionInfo.name];
		if (!expectedVersion || !semverSatisfies(versionInfo.version, expectedVersion)) {
			throw new ContractVersionNotSatisfiedError(
				this.address,
				versions,
				versionInfo
			);
		}
	}
	/**
	 * Executes the contracts `query` function with the specified payload encoded as JSON
	 * @param msg 
	 * @returns 
	 */
	query(msg: any): Promise<any> {
		return this.endpoint.queryContractSmart(this.address, msg);
	}
	executeIx(msg: any, funds?: Coin[]): ExecuteInstruction {
		const result: ExecuteInstruction = {
			contractAddress: this.address,
			msg
		};
		if (funds) {
			result.funds = funds;
		}
		return result;
	}
	executeIxCw20(msg: any, tokenContractOrUnifiedDenom: string, amount: string | bigint | number): ExecuteInstruction {
		if (tokenContractOrUnifiedDenom.startsWith("cw20/")) {
			tokenContractOrUnifiedDenom = tokenContractOrUnifiedDenom.substring(5); // "cw20/".length
		}
		return {
			contractAddress: tokenContractOrUnifiedDenom,
			msg: {
				send: {
					amount: amount.toString(),
					contract: this.address,
					// I can't believe no one took a look at base64-encoded-json and thought:
					// "How is this supposed to be fast?"
					msg:(Buffer.isBuffer(msg) ? msg : Buffer.from(
						JSON.stringify(
							msg
						)
					)).toString("base64")
				}
			}
		};
	}
}
