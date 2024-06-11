import { StdFee } from "@cosmjs/amino";
import { EncodeObject } from "@cosmjs/proto-signing";
import { DeliverTxResponse } from "@cosmjs/stargate";
import { FullscreenLoadingTask, alert } from "@crownfi/css-gothic-fantasy";
import { ReceiptInformation as EvmReceiptInformation, Transaction as EvmTransaction } from "@crownfi/ethereum-rpc-types";
import { ClientEnv, ClientNotSignableError, EvmOrWasmExecuteInstruction, SeiChainId, TransactionFinality, transactionSucceeded } from "@crownfi/sei-utils";
const DEFAULT_TX_TIMEOUT_MS = 60000;


export async function txConfirmMsgBox(tx: EvmReceiptInformation | DeliverTxResponse, chainId: SeiChainId) {
	const txLink = document.createElement("p");
	txLink.innerHTML = `<a href="https://seitrace.com/tx/${
			tx.transactionHash
		}?chain=${
			chainId
		}" target="_blank">View details on seitrace</a><span class="cicon cicon-link-external"></span>`;
	if (transactionSucceeded(tx)) {
		const message = document.createElement("p");
		message.innerText = "Your transaction has been successfully processed.";
		await alert(
			"Transaction confirmed",
			[message, txLink],
			"check-circle-o",
			"success"
		);
	} else {
		const message = document.createElement("p");
		message.innerText = "Your transaction was processed, but resulted in an error. \
			This may be due to a conflicting tranasction being confirmed before yours.";
		await alert(
			"Transaction confirmed with error",
			[message, txLink],
			"warning",
			"warning"
		);
	}
}
export async function txsConfirmMsgBox(
	txs: (EvmReceiptInformation | DeliverTxResponse)[],
	chainId: SeiChainId,
	expectedLength?: number
) {
	if (!txs.length) {
		return;
	}
	if (txs.length == 1) {
		return txConfirmMsgBox(txs[0], chainId);
	}
	let hasFailedTransaction = false;
	const message: HTMLElement[] = [];
	message.push((() => {
		const list = document.createElement("ol");
		for (let i = 0; i < txs.length; i += 1) {
			const listItem = document.createElement("li");
			listItem.innerHTML = `<a href="https://seitrace.com/tx/${
					txs[i].transactionHash
				}?chain=${
					chainId
				}" target="_blank">${
					txs[i].transactionHash
				}</a><span class="cicon cicon-link-external"></span>`;
			list.style.wordBreak = "break-word";
			if (!transactionSucceeded(txs[i])) {
				listItem.innerHTML = "⚠️ " + listItem.innerHTML;
				hasFailedTransaction = true;
			}
			list.appendChild(listItem);
		}
		return list;
	})());
	if (expectedLength && expectedLength > txs.length) {
		const skippedMsg = document.createElement("p");
		skippedMsg.innerText = (expectedLength - txs.length) +
			" transaction(s) where cancelled due to previous failure(s)";
		message.push(skippedMsg);
	}
	if (hasFailedTransaction) {
		message.unshift((() => {
			const msg = document.createElement("p");
			msg.innerText = "Some transactions in the sequence have failed. Details can be viewed below. \
				This may be due to a conflicting tranasction being confirmed before yours.";
			return msg;
		})());
		return alert(
			"Transactions confirmed with error",
			message,
			"warning",
			"warning"
		);
	}else{
		message.unshift((() => {
			const msg = document.createElement("p");
			msg.innerText = "All transactions have been been successfully processed. Details can be viewed below.";
			return msg;
		})());
		await alert(
			"Transactions confirmed",
			message,
			"check-circle-o",
			"success"
		);
	}
}

export class WebClientEnv extends ClientEnv {
	signAndSend(msgs: EncodeObject[]): Promise<DeliverTxResponse>;
	signAndSend(msgs: EncodeObject[], memo?: string): Promise<DeliverTxResponse>;
	signAndSend(msgs: EncodeObject[], memo?: string, fee?: "auto" | StdFee): Promise<DeliverTxResponse>;
	signAndSend(
		msgs: EncodeObject[],
		memo: string | undefined,
		fee: "auto" | StdFee | undefined,
		finality: "broadcasted",
		spinnerTextSuffix?: string,
		noConfirmMsgBox?: boolean
	): Promise<string>;
	signAndSend(
		msgs: EncodeObject[],
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: { confirmed: { timeoutMs?: number } },
		spinnerTextSuffix?: string,
		noConfirmMsgBox?: boolean
	): Promise<DeliverTxResponse>;
	signAndSend(
		msgs: EncodeObject[],
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: TransactionFinality,
		spinnerTextSuffix?: string,
		noConfirmMsgBox?: boolean
	): Promise<DeliverTxResponse | string>;
	async signAndSend(
		msgs: EncodeObject[],
		memo: string = "",
		fee: "auto" | StdFee = "auto",
		finality: TransactionFinality = { confirmed: {} },
		spinnerTextSuffix: string = "",
		noConfirmMsgBox?: boolean
	): Promise<DeliverTxResponse | string> {
		const task = new FullscreenLoadingTask();
		// Re-implementing everything cuz super emits events (which we should remove)
		try {
			if (!this.isSignable()) {
				throw new ClientNotSignableError("Cannot execute transactions - " + this.readonlyReason);
			}
			task.text = "Awaiting transaction approval... " + spinnerTextSuffix;
			task.show();
			const transactionHash = await this.stargateClient.signAndBroadcastSync(
				this.account.seiAddress,
				msgs,
				fee,
				memo
			);
			if (finality == "broadcasted") {
				return transactionHash;
			}
			task.text = `Awaiting transaction confirmation for ${transactionHash} ... ${spinnerTextSuffix}`;
			const {
				confirmed: { timeoutMs = DEFAULT_TX_TIMEOUT_MS },
			} = finality;
			const result = await this.waitForTxConfirm(transactionHash, timeoutMs, true);
			if (!noConfirmMsgBox) {
				await txConfirmMsgBox(result, this.chainId);
			}
			return result;
		} finally {
			task.hide();
		}
	}
	evmSignAndSend(
		msg: EvmTransaction
	): Promise<EvmReceiptInformation>;
	evmSignAndSend(
		msg: EvmTransaction,
		finality: "broadcasted",
		spinnerTextSuffix?: string,
		noConfirmMsgBox?: boolean
	): Promise<string>;
	evmSignAndSend(
		msg: EvmTransaction,
		finality?: { confirmed: { timeoutMs?: number } },
		spinnerTextSuffix?: string,
		noConfirmMsgBox?: boolean
	): Promise<EvmReceiptInformation>;
	evmSignAndSend(
		msg: EvmTransaction,
		finality?: TransactionFinality,
		spinnerTextSuffix?: string,
		noConfirmMsgBox?: boolean
	): Promise<EvmReceiptInformation | string>;
	async evmSignAndSend(
		msg: EvmTransaction,
		finality: TransactionFinality = { confirmed: {} },
		spinnerTextSuffix: string = "",
		noConfirmMsgBox?: boolean
	): Promise<EvmReceiptInformation | string> {
		const task = new FullscreenLoadingTask();
		try {
			task.text = "Awaiting transaction approval... " + spinnerTextSuffix;
			task.show();
			// This is fine as the super impl doesn't emit any events (why did I think that was a good idea?)
			const transactionHash = await super.evmSignAndSend(msg, "broadcasted");
			if (finality == "broadcasted") {
				return transactionHash;
			}
			task.text = `Waiting for ${transactionHash} confirmation... ${spinnerTextSuffix}`;
			const {
				confirmed: { timeoutMs = DEFAULT_TX_TIMEOUT_MS },
			} = finality;
			const result = await this.waitForEvmTxConfirm(transactionHash, timeoutMs, true);
			if (!noConfirmMsgBox) {
				await txConfirmMsgBox(result, this.chainId);
			}
			return result;
		} finally {
			task.hide();
		}
	}
	async processHackyTransactionSequence(
		sequence: ({evmMsg: EvmTransaction} | {cosmMsg: EncodeObject[]})[],
		ignoreFailures?: boolean,
		noConfirmMsgBox?: boolean
	): Promise<(EvmReceiptInformation | DeliverTxResponse)[]> {
		if (sequence.length == 1) {
			if ("evmMsg" in sequence[0]) {
				return [await this.evmSignAndSend(
					sequence[0].evmMsg,
					{ confirmed: {} },
					"",
					noConfirmMsgBox
				)];
			} else {
				return [await this.signAndSend(
					sequence[0].cosmMsg,
					"",
					"auto",
					{ confirmed: {} },
					"",
					noConfirmMsgBox
				)];
			}
		}
		const result: (EvmReceiptInformation | DeliverTxResponse)[] = [];
		for (let i = 0; i < sequence.length; i += 1) {
			const msg = sequence[i];
			if ("evmMsg" in msg) {
				const txResult = await this.evmSignAndSend(
					msg.evmMsg,
					{ confirmed: {} },
					"(" + (i + 1) + "/" + sequence.length + ")",
					true
				);
				result.push(txResult);
				// "0x0" means failure, don't continue
				if (!Number(txResult.status) && !ignoreFailures) {
					break;
				}
			} else {
				const txResult = await this.signAndSend(
					msg.cosmMsg,
					"",
					"auto",
					{ confirmed: {} },
					"(" + (i + 1) + "/" + sequence.length + ")",
					true
				);
				result.push(txResult);
				// non-zero status means failure, don't continue
				if (txResult.code && !ignoreFailures) {
					break;
				}
			}
		}
		if (!noConfirmMsgBox) {
			await txsConfirmMsgBox(result, this.chainId, sequence.length);
		}
		return result;
	}
	/**
	 * Temporary measures to safely handle mix of EVM and WASM instructions, as currently top-level EVM invokes require
	 * EVM signatures.
	 * 
	 * @param instructions 
	 * @returns 
	 */
	async executeContractHackySequence(
		instructions: EvmOrWasmExecuteInstruction[]
	): Promise<(EvmReceiptInformation | DeliverTxResponse)[]> {
		const task = new FullscreenLoadingTask();
		try {
			task.text = "Encoding transaction...";
			task.show();
			const sequence = await this.makeHackyTransactionSequenceAsNeeded(
				this.execIxsToCosmosMsgs(instructions)
			);
			return this.processHackyTransactionSequence(sequence);
		} finally {
			task.text = "";
			task.hide();
		}
	}
}
