import { StdFee } from "@cosmjs/amino";
import { EncodeObject } from "@cosmjs/proto-signing";
import { DeliverTxResponse } from "@cosmjs/stargate";
import { FullscreenLoadingTask } from "@crownfi/css-gothic-fantasy";
import { ReceiptInformation, Transaction as EvmTransaction } from "@crownfi/ethereum-rpc-types";
import { ClientEnv, ClientNotSignableError, EvmOrWasmExecuteInstruction, TransactionFinality } from "@crownfi/sei-utils";
const DEFAULT_TX_TIMEOUT_MS = 60000;
export class WebClientEnv extends ClientEnv {
	signAndSend(msgs: EncodeObject[]): Promise<DeliverTxResponse>;
	signAndSend(msgs: EncodeObject[], memo?: string): Promise<DeliverTxResponse>;
	signAndSend(msgs: EncodeObject[], memo?: string, fee?: "auto" | StdFee): Promise<DeliverTxResponse>;
	signAndSend(
		msgs: EncodeObject[],
		memo: string | undefined,
		fee: "auto" | StdFee | undefined,
		finality: "broadcasted",
		spinnerTextSuffix?: string
	): Promise<string>;
	signAndSend(
		msgs: EncodeObject[],
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: { confirmed: { timeoutMs?: number } },
		spinnerTextSuffix?: string
	): Promise<DeliverTxResponse>;
	signAndSend(
		msgs: EncodeObject[],
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: TransactionFinality,
		spinnerTextSuffix?: string
	): Promise<DeliverTxResponse | string>;
	async signAndSend(
		msgs: EncodeObject[],
		memo: string = "",
		fee: "auto" | StdFee = "auto",
		finality: TransactionFinality = { confirmed: {} },
		spinnerTextSuffix: string = ""
	): Promise<DeliverTxResponse | string> {
		const task = new FullscreenLoadingTask();
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
			task.text = `Waiting for ${transactionHash} confirmation... ${spinnerTextSuffix}`;
			const {
				confirmed: { timeoutMs = DEFAULT_TX_TIMEOUT_MS },
			} = finality;
			return await this.waitForTxConfirm(transactionHash, timeoutMs, true);
		} finally {
			task.hide();
		}
	}
	evmSignAndSend(
		msg: EvmTransaction
	): Promise<ReceiptInformation>;
	evmSignAndSend(
		msg: EvmTransaction,
		finality: "broadcasted",
		spinnerTextSuffix?: string
	): Promise<string>;
	evmSignAndSend(
		msg: EvmTransaction,
		finality?: { confirmed: { timeoutMs?: number } },
		spinnerTextSuffix?: string
	): Promise<ReceiptInformation>;
	evmSignAndSend(
		msg: EvmTransaction,
		finality?: TransactionFinality,
		spinnerTextSuffix?: string
	): Promise<ReceiptInformation | string>;
	async evmSignAndSend(
		msg: EvmTransaction,
		finality: TransactionFinality = { confirmed: {} },
		spinnerTextSuffix: string = ""
	): Promise<ReceiptInformation | string> {
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
			return await this.waitForEvmTxConfirm(transactionHash, timeoutMs, true);
		} finally {
			task.hide();
		}
	}
	async processHackyTransactionSequence(
		sequence: ({evmMsg: EvmTransaction} | {cosmMsg: EncodeObject[]})[]
	): Promise<(ReceiptInformation | DeliverTxResponse)[]> {
		if (sequence.length == 1) {
			if ("evmMsg" in sequence[0]) {
				return [await this.evmSignAndSend(
					sequence[0].evmMsg
				)];
			}else{
				return [await this.signAndSend(
					sequence[0].cosmMsg,
				)];
			}
		}
		const result: (ReceiptInformation | DeliverTxResponse)[] = [];
		for (let i = 0; i < sequence.length; i += 1) {
			const msg = sequence[i];
			if ("evmMsg" in msg) {
				result.push(
					await this.evmSignAndSend(
						msg.evmMsg,
						{ confirmed: {} },
						(i + 1) + "/" + sequence.length
					)
				);
			}else{
				result.push(
					await this.signAndSend(
						msg.cosmMsg,
						"",
						"auto",
						{ confirmed: {} },
						(i + 1) + "/" + sequence.length
					)
				);
			}
		}
		return result;
	}
	async executeContractHackySequence(
		instructions: EvmOrWasmExecuteInstruction[]
	): Promise<(ReceiptInformation | DeliverTxResponse)[]> {
		const task = new FullscreenLoadingTask();
		task.text = "Encoding transaction...";
		task.show();
		const sequence = await this.makeHackyTransactionSequenceAsNeeded(
			this.execIxsToCosmosMsgs(instructions)
		);
		task.hide();
		return this.processHackyTransactionSequence(sequence);
	}
}
