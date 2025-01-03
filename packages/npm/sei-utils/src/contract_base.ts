import { ExecuteInstruction as WasmExecuteInstruction, WasmExtension } from "@cosmjs/cosmwasm-stargate";
import { Addr, ContractVersionInfo } from "./common_sei_types.js";
import { Coin } from "@cosmjs/amino";
import semverSatisfies from "semver/functions/satisfies.js";
import { QueryClient as StargateQueryClient } from "@cosmjs/stargate";
import { EVMABIFunctionDefinition, toChecksumAddressEvm } from "./evm-interop-utils/index.js";
import { stringToCanonicalAddr } from "@crownfi/sei-js-core";

const CONTRACT_INFO_KEY = Buffer.from("contract_info");

export interface EvmExecuteInstruction {
	contractAddress: string;
	evmMsg: {
		function: string | EVMABIFunctionDefinition
		params: any[],
		funds?: bigint
	};
}

export type EvmOrWasmExecuteInstruction = EvmExecuteInstruction | WasmExecuteInstruction;

/**
 * Can be thrown by {@link ContractBase.checkVersion}
 */
export class ContractVersionNotSatisfiedError extends Error {
	name!: "ContractVersionNotSatisfiedError";
	contractAddress: Addr;
	expectedVersions: { [name: string]: string };
	actualVersionInfo: ContractVersionInfo | null;
	constructor(
		contractAddress: Addr,
		expectedVersions: { [name: string]: string },
		actualVersionInfo: ContractVersionInfo | null
	) {
		if (actualVersionInfo) {
			super(
				"Expected contract " +
					contractAddress +
					" to be compatible with " +
					Object.keys(expectedVersions)
						.map((name) => {
							name + "@" + expectedVersions[name];
						})
						.join(", ") +
					"; " +
					"but the actual contract version information was " +
					actualVersionInfo.name +
					"@" +
					actualVersionInfo.version
			);
		} else {
			super("The contract at " + contractAddress + " has no version information; perhaps it doesn't exist");
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
export class ContractBase<Q extends StargateQueryClient & WasmExtension> {
	// These properties should be assigned in the constructor as it calls the address setter
	#evmAddress!: string;
	#address!: Addr;
	get address(): Addr {
		return this.#address;
	}
	set address(address: Addr) {
		this.#address = address;
		const canonAddr = Buffer.from(stringToCanonicalAddr(address));
		this.#evmAddress = toChecksumAddressEvm(
			"0x" + canonAddr.subarray(canonAddr.length - 20).toString("hex")
		);
	}
	get evmAddress(): string {
		return this.#evmAddress;
	}
	endpoint: Q;
	/**
	 * @param endpoint The cosmwasm client
	 * @param address Contract address
	 */
	constructor(endpoint: Q, address: Addr) {
		this.endpoint = endpoint;
		this.address = address;
	}
	
	/**
	 * Reads contract state at key "contract_info" and returnes the parsed state if it exists.
	 */
	async getVersion(): Promise<ContractVersionInfo | null> {
		const storedData = (await this.endpoint.wasm.queryContractRaw(this.address, CONTRACT_INFO_KEY)).data;
		if (storedData == null) {
			return null;
		}
		return JSON.parse(Buffer.from(storedData.buffer, storedData.byteOffset, storedData.byteLength).toString());
	}
	/**
	 * Resolves the promise if the contract name matches and if the contract version satisfies the semver range
	 * specified. Otherwise, the promise is rejected with a {@link ContractVersionNotSatisfiedError}.
	 * @param versions A map of name => version. e.g. `{"my-awesome-contract": "^1.0.0"}`
	 */
	async checkVersion(versions: { [name: string]: string }): Promise<void> {
		const versionInfo = await this.getVersion();
		if (versionInfo == null) {
			throw new ContractVersionNotSatisfiedError(this.address, versions, versionInfo);
		}
		const expectedVersion = versions[versionInfo.name];
		if (!expectedVersion || !semverSatisfies(versionInfo.version, expectedVersion)) {
			throw new ContractVersionNotSatisfiedError(this.address, versions, versionInfo);
		}
	}
	/**
	 * Executes the contracts `query` function with the specified payload encoded as JSON
	 * @param msg
	 * @returns
	 */
	async query(msg: any): Promise<any> {
		return this.endpoint.wasm.queryContractSmart(this.address, msg);
	}
	executeIx(msg: any, funds?: Coin[]): WasmExecuteInstruction {
		const result: WasmExecuteInstruction = {
			contractAddress: this.address,
			msg,
		};
		if (funds) {
			result.funds = funds;
		}
		return result;
	}
	executeIxCw20(msg: any, tokenContractOrUnifiedDenom: string, amount: string | bigint | number): WasmExecuteInstruction {
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
					msg: (Buffer.isBuffer(msg) ? msg : Buffer.from(JSON.stringify(msg))).toString("base64"),
				},
			},
		};
	}
}
