import { EncodeObject } from "@cosmjs/proto-signing";
import { EVMABIFunctionDefinition, functionSignatureToABIDefinition } from "./abi/common.js";
import { SeiQueryClient } from "@crownfi/sei-js-core";
import { normalizeToEvmAddress } from "./linked_address_lookup.js";
import { encodeEvmFuncCall } from "./abi/encode.js";

// "actual" message types
import { MsgExecuteContract } from "cosmjs-types/cosmwasm/wasm/v1/tx.js";
import { MsgSend } from "cosmjs-types/cosmos/bank/v1beta1/tx.js";
import { MsgInternalEVMCall } from "@crownfi/sei-js-proto/dist/codegen/evm/tx.js";

export const EVM_COSMWASM_ADDRESS = "0x0000000000000000000000000000000000001004";
export const EVM_COSMWASM_EXECUTE = functionSignatureToABIDefinition("execute(string, bytes, bytes)");
export const EVM_COSMWASM_EXECUTE_BATCH: EVMABIFunctionDefinition = { inputs: [ { components: [ { name: "contractAddress", type: "string" }, { name: "msg", type: "bytes" }, { name: "coins", type: "bytes" } ], name: "executeMsgs", type: "tuple[]" } ], name: "execute_batch", outputs: [ { name: "responses", type: "bytes[]" } ], stateMutability: "payable", type: "function" };

export const EVM_BANK_ADDRESS = "0x0000000000000000000000000000000000001001";
export const EVM_BANK_SEND = functionSignatureToABIDefinition("send(address, address, string, uint256)");
export const EVM_BANK_SEND_SEI = functionSignatureToABIDefinition("sendNative(string)");

// Submit a PR if you want more types, this is all we need for now.

export async function cosmosMessagesToEvmMessages(
	queryClient: SeiQueryClient,
	msgs: EncodeObject[],
	senderEvmAddress?: string
): Promise<{
	from: string,
	to: string,
	value: string,
	data?: string
}[]>
{
	const addrConvertCache: {[addr: string]: string} = {};
	const result = [];
	for (let i = 0; i < msgs.length; i += 1) {
		switch (msgs[i].typeUrl) {
			case "/cosmwasm.wasm.v1.MsgExecuteContract": {
				const msg = msgs[i].value as MsgExecuteContract;
				const funds = msg.funds;
				const seiIndex = funds.findIndex(fund => fund.denom == "usei");
				let value = "0x0";
				if (seiIndex != -1) {
					const seiFund = funds.splice(seiIndex, 1)[0]
					value = "0x" + (BigInt(seiFund.amount) * 10n ** 12n).toString(16);
				}
				result.push({
					from: await normalizeToEvmAddress(queryClient, msg.sender, addrConvertCache),
					to: EVM_COSMWASM_ADDRESS,
					value,
					data: "0x" + encodeEvmFuncCall(
						EVM_COSMWASM_EXECUTE,
						[msg.contract, msg.msg, JSON.stringify(funds)]
					).toString("hex")
				});
				break;
			}
			case "/seiprotocol.seichain.evm.MsgInternalEVMCall": {
				const msg = msgs[i].value as MsgInternalEVMCall;
				result.push({
					from: await normalizeToEvmAddress(queryClient, msg.sender, addrConvertCache),
					to: msg.to,
					value: "0x" + BigInt(msg.value).toString(16),
					data: "0x" + Buffer.from(
						msg.data.buffer,
						msg.data.byteOffset,
						msg.data.byteLength
					).toString("hex")
				});
				break;
			}
			case "/cosmos.bank.v1beta1.MsgSend": {
				const msg = msgs[i].value as MsgSend;
				const from = await normalizeToEvmAddress(queryClient, msg.fromAddress, addrConvertCache);
				if (from == senderEvmAddress && msg.amount.length == 1 && msg.amount[0].denom == "usei") {
					const value = "0x" + (BigInt(msg.amount[0].amount) * 10n ** 12n).toString(16);
					try {
						const to = await normalizeToEvmAddress(queryClient, msg.toAddress, addrConvertCache);
						result.push({
							from,
							to,
							value
						});
					}catch(ex: any) {
						// Couldn't find EVM address for whatever reason, maybe the node will have better luck?
						result.push({
							from,
							to: EVM_BANK_ADDRESS,
							value,
							data: "0x" + encodeEvmFuncCall(
								EVM_BANK_SEND_SEI,
								[msg.toAddress]
							).toString("hex")
						});
					}
				} else {
					const toAddress = await normalizeToEvmAddress(
						queryClient,
						msg.toAddress,
						addrConvertCache
					);
					for (let ii = 0; ii < msg.amount.length; ii += 1) {
						result.push({
							from: senderEvmAddress || from,
							to: EVM_BANK_ADDRESS,
							value: "0x0",
							data: "0x" + encodeEvmFuncCall(
								EVM_BANK_SEND,
								[from, toAddress, msg.amount[ii].denom, BigInt(msg.amount[ii].amount)]
							).toString("hex")
						})
					}
				}
				break;
			}
		}
	}
	return result;
}
