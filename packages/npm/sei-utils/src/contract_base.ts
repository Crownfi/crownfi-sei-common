import { CosmWasmClient, ExecuteInstruction, SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate"
import { Addr, ContractVersionInfo } from "./common_sei_types.js";
import { Coin } from "@cosmjs/amino";

const CONTRACT_INFO_KEY = Buffer.from("contract_info");

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
		return this.executeIx(
			{
				send: {
					amount: amount.toString(),
					contract: tokenContractOrUnifiedDenom,
					// I can't believe no one took a look at base64-encoded-json and thought:
					// "How is this supposed to be fast?"
					msg: Buffer.from(
						JSON.stringify(
							msg
						)
					).toString("base64")
				}
			}
		)
	}
}
