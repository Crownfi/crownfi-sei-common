import { CosmWasmClient, ExecuteInstruction, SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate"
import { Addr } from "./common_sei_types.js";
import { Coin } from "@cosmjs/amino";

export class ContractBase {
	address: Addr;
	endpoint: CosmWasmClient;
	constructor(endpoint: CosmWasmClient, address: Addr) {
		this.endpoint = endpoint;
		this.address = address;
	}
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
