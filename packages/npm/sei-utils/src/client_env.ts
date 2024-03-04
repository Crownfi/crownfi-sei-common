import { AccountData, Coin, StdFee, encodeSecp256k1Pubkey } from "@cosmjs/amino";
import {
	CosmWasmClient,
	ExecuteInstruction,
	InstantiateResult,
	MigrateResult,
	MsgExecuteContractEncodeObject,
	SigningCosmWasmClient,
	UploadResult,
} from "@cosmjs/cosmwasm-stargate";
import {
	KNOWN_SEI_PROVIDER_INFO,
	KnownSeiProviders,
	SeiWallet,
	getCosmWasmClient,
	getQueryClient,
	getSigningCosmWasmClient,
} from "@crownfi/sei-js-core";
import { seiUtilEventEmitter } from "./events.js";
import { EncodeObject, OfflineSigner } from "@cosmjs/proto-signing";
import { SeiChainNetConfig, getDefaultNetworkConfig } from "./chain_config.js";
import { DeliverTxResponse, GasPrice, IndexedTx, TimeoutError, calculateFee } from "@cosmjs/stargate";
import { nativeDenomSortCompare } from "./funds_util.js";
import { MsgExecuteContract } from "cosmjs-types/cosmwasm/wasm/v1/tx.js";
import { Addr } from "./common_sei_types.js";

const DEFAULT_TX_TIMEOUT_MS = 60000;

export type MaybeSelectedProviderString = KnownSeiProviders | "seed-wallet" | "read-only-address" | null;
export type MaybeSelectedProvider = KnownSeiProviders | { seed: string; index?: number } | { address: string } | null;

export class TransactionError extends Error {
	name!: "TransactionError";
	public constructor(
		public code: string | number,
		public txhash: string | undefined,
		public rawLog: string
	) {
		super("Transaction confirmed with an error");
	}
}
TransactionError.prototype.name == "TransactionError";

function maybeProviderToMaybeString(provider: MaybeSelectedProvider): MaybeSelectedProviderString {
	if (typeof provider == "object" && provider != null) {
		if ("seed" in provider) {
			return "seed-wallet";
		}
		return "read-only-address";
	}
	return provider;
}
/**
 * This type is usually used with functions which send transactions to the blockchain.
 * A value of `"broadcasted"` means to wait until a transaction is sent. While `{confirmed: {...}}` means waiting until
 * the transaction has been sent and processed. With an optional timeout time, which usually defaults to 60 seconds.
 */
export type TransactionFinality = "broadcasted" | { confirmed: { timeoutMs?: number } };
export type SimulateResponse = Awaited<ReturnType<ReturnType<CosmWasmClient["forceGetQueryClient"]>["tx"]["simulate"]>>;
interface ClientEnvConstruct {
	account: AccountData | null;
	chainId: string;
	wasmClient: SigningCosmWasmClient | CosmWasmClient;
	queryClient: Awaited<ReturnType<typeof getQueryClient>>;
	readonlyReason: string;
}
let defaultProvider: MaybeSelectedProvider = null;
let defaultGasPrice = GasPrice.fromString("0.1usei");
export class ClientEnv {
	account: AccountData | null;
	chainId: string;
	wasmClient: SigningCosmWasmClient | CosmWasmClient;
	queryClient: Awaited<ReturnType<typeof getQueryClient>>;
	readonlyReason: string;

	static getDefaultProvider(): MaybeSelectedProviderString {
		return maybeProviderToMaybeString(defaultProvider);
	}
	/**
	 * Sets the default provider to `null` synchronously
	 */
	static nullifyDefaultProvider() {
		if (defaultProvider != null) {
			defaultProvider = null;
			seiUtilEventEmitter.emit("defaultProviderChangeRequest", {
				status: "success",
				provider: null,
			});
			seiUtilEventEmitter.emit("defaultProviderChanged", {
				chainId: getDefaultNetworkConfig().chainId,
				provider: null,
				account: null,
			});
		}
	}
	static setDefaultGasPrice(gasPrice: GasPrice) {
		defaultGasPrice = gasPrice;
	}
	static getDefaultGasPrice(): GasPrice {
		return defaultGasPrice;
	}
	static async setDefaultProvider(provider: MaybeSelectedProvider, dontThrowOnFail: boolean = false) {
		const oldProvider = defaultProvider;
		defaultProvider = provider;
		if (oldProvider == defaultProvider) {
			return;
		}
		if (typeof defaultProvider == "object" && typeof oldProvider == "object") {
			if (
				defaultProvider != null &&
				oldProvider != null &&
				(defaultProvider as any).address == (oldProvider as any).address &&
				(defaultProvider as any).seed == (oldProvider as any).seed &&
				(defaultProvider as any).index == (oldProvider as any).index
			) {
				return;
			}
		}
		const newProviderString = maybeProviderToMaybeString(defaultProvider);
		if (defaultProvider == null) {
			seiUtilEventEmitter.emit("defaultProviderChangeRequest", {
				status: "success",
				provider: newProviderString,
			});
			seiUtilEventEmitter.emit("defaultProviderChanged", {
				chainId: getDefaultNetworkConfig().chainId,
				provider: newProviderString,
				account: null,
			});
		} else {
			try {
				seiUtilEventEmitter.emit("defaultProviderChangeRequest", {
					status: "requesting",
					provider: newProviderString,
				});
				const clientEnv = await ClientEnv.get();
				const clientAccount = clientEnv.getAccount();
				seiUtilEventEmitter.emit("defaultProviderChangeRequest", {
					status: "success",
					provider: newProviderString,
				});
				seiUtilEventEmitter.emit("defaultProviderChanged", {
					chainId: getDefaultNetworkConfig().chainId,
					provider: newProviderString,
					account: clientAccount,
				});
			} catch (ex) {
				defaultProvider = oldProvider;
				seiUtilEventEmitter.emit("defaultProviderChangeRequest", {
					status: "failure",
					provider: newProviderString,
					failureException: ex,
				});
				if (!dontThrowOnFail) {
					throw ex;
				}
			}
		}
	}
	private static async getSigner(
		provider: MaybeSelectedProvider,
		networkConfig: SeiChainNetConfig
	): Promise<{ signer: OfflineSigner } | { failure: string } | { address: string }> {
		if (provider == null) {
			return {
				failure: "No wallet selected",
			};
		}
		if (typeof provider == "object") {
			if ("seed" in provider) {
				// async imports allow us to load the signing stuff only if needed. (hopefully)
				const { restoreWallet } = await import("@crownfi/sei-js-core");
				return {
					signer: await restoreWallet(provider.seed, provider.index),
				};
			}
			return {
				address: provider.address,
			};
		}
		try {
			const signer = await new SeiWallet(provider).getOfflineSigner(networkConfig.chainId);
			if (signer == undefined) {
				return {
					failure:
						KNOWN_SEI_PROVIDER_INFO[provider].name +
						" did not provide a signer. (Is the wallet unlocked and are we authorized?)",
				};
			}
			return { signer };
		} catch (ex: any) {
			return {
				failure: KNOWN_SEI_PROVIDER_INFO[provider].name + ' says "' + ex.name + ": " + ex.message + '"',
			};
		}
	}

	static async get<T extends typeof ClientEnv>(
		this: T,
		provider: MaybeSelectedProvider = defaultProvider,
		networkConfig: SeiChainNetConfig = getDefaultNetworkConfig(),
		gasPrice: GasPrice = defaultGasPrice
	): Promise<InstanceType<T>> {
		const queryClient = await getQueryClient(networkConfig.restUrl);

		const [wasmClient, account, readonlyReason] = await (async () => {
			const maybeSigner = await ClientEnv.getSigner(provider, networkConfig);
			if ("failure" in maybeSigner) {
				return [await getCosmWasmClient(networkConfig.rpcUrl), null, maybeSigner.failure];
			} else if ("address" in maybeSigner) {
				return [
					await getCosmWasmClient(networkConfig.rpcUrl),
					{
						address: maybeSigner.address,
						// fake data etc.
						algo: "secp256k1",
						pubkey: new Uint8Array(0),
					} satisfies AccountData,
					"Address was provided without a seed nor wallet",
				];
			}
			const { signer } = maybeSigner;
			const accounts = await signer.getAccounts();
			if (accounts.length !== 1) {
				return [
					await getCosmWasmClient(networkConfig.rpcUrl),
					null,
					"Expected wallet to expose exactly 1 account but got " + accounts.length + " accounts",
				];
			}
			return [
				await getSigningCosmWasmClient(networkConfig.rpcUrl, signer, {
					gasPrice,
				}),
				accounts[0],
				"",
			];
		})();
		return new this({
			account,
			chainId: networkConfig.chainId,
			wasmClient,
			queryClient,
			readonlyReason,
		}) as InstanceType<T>;
	}
	/**
	 * use of the constructor is discouraged and isn't guaranteed to be stable. Use the get() function instead.
	 */
	constructor({ account, chainId, wasmClient, queryClient, readonlyReason }: ClientEnvConstruct) {
		this.account = account;
		this.chainId = chainId;
		this.wasmClient = wasmClient;
		this.queryClient = queryClient;
		this.readonlyReason = readonlyReason;
	}
	/**
	 * Conveniently throws an error with the underlying reason if the account property is null
	 */
	getAccount() {
		if (this.account == null) {
			throw new Error("Can't get user wallet address - " + this.readonlyReason);
		}
		return this.account;
	}
	/**
	 * @returns true if tranasction signing is available and the wallet is known
	 */
	isSignable(): this is { wasmClient: SigningCosmWasmClient; account: AccountData } {
		return this.wasmClient instanceof SigningCosmWasmClient && this.account != null;
	}

	/**
	 * If you want to actually check if transactions can be sent, use the `isSignable` method
	 *
	 * @returns true if the wallet is known
	 */
	hasAccount(): this is { account: AccountData } {
		return this.account != null;
	}

	/**
	 *
	 * @param tx the transcation hash
	 * @param timeoutMs how long to wait until timing out. Defaults to 60 seconds
	 * @param throwOnTimeout whether or not to throw an error if the timeout time has elapsed instead of returning null
	 * @returns the confirmed transaction, or null if we waited too long and `throwOnTimeout` is falsy
	 */
	async waitForTxConfirm(tx: string, timeoutMs?: number, throwOnTimeout?: boolean): Promise<DeliverTxResponse | null>;
	/**
	 *
	 * @param tx the transcation hash
	 * @param timeoutMs how long to wait until timing out. Defaults to 60 seconds if undefined
	 * @param throwOnTimeout you explicitly set this to `true`, so prepare for error throwing
	 * @returns the confirmed transaction
	 */
	async waitForTxConfirm(tx: string, timeoutMs: number | undefined, throwOnTimeout: true): Promise<DeliverTxResponse>;
	async waitForTxConfirm(
		tx: string,
		timeoutMs: number = DEFAULT_TX_TIMEOUT_MS,
		throwOnTimeout?: boolean
	): Promise<DeliverTxResponse | null> {
		// More stuff that cosmjs implements internally that doesn't get exposed to us
		let result: IndexedTx | null = null;
		const startTime = Date.now();

		while (result == null && Date.now() - startTime < timeoutMs) {
			await new Promise((resolve) => {
				setTimeout(resolve, 200 + Math.random() * 300);
			});
			result = await this.wasmClient.getTx(tx);
		}
		if (result == null && throwOnTimeout) {
			throw new TimeoutError(
				"Transaction " +
					tx +
					" wasn't confirmed within " +
					timeoutMs / 1000 +
					" seconds. " +
					"You may want to check this again later",
				tx
			);
		}
		return result == null
			? null
			: {
					transactionHash: tx,
					...result,
				};
	}

	signAndSend(msgs: EncodeObject[]): Promise<DeliverTxResponse>;
	signAndSend(msgs: EncodeObject[], memo?: string): Promise<DeliverTxResponse>;
	signAndSend(msgs: EncodeObject[], memo?: string, fee?: "auto" | StdFee): Promise<DeliverTxResponse>;
	signAndSend(
		msgs: EncodeObject[],
		memo: string | undefined,
		fee: "auto" | StdFee | undefined,
		finality: "broadcasted"
	): Promise<string>;
	signAndSend(
		msgs: EncodeObject[],
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: { confirmed: { timeoutMs?: number } }
	): Promise<DeliverTxResponse>;
	signAndSend(
		msgs: EncodeObject[],
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: TransactionFinality
	): Promise<DeliverTxResponse | string>;
	async signAndSend(
		msgs: EncodeObject[],
		memo: string = "",
		fee: "auto" | StdFee = "auto",
		finality: TransactionFinality = { confirmed: {} }
	): Promise<DeliverTxResponse | string> {
		if (!this.isSignable()) {
			throw new Error("Cannot execute transactions - " + this.readonlyReason);
		}
		const transactionHash = await this.wasmClient.signAndBroadcastSync(this.account.address, msgs, fee, memo);
		if (finality == "broadcasted") {
			seiUtilEventEmitter.emit("transactionBroadcasted", {
				chainId: this.chainId,
				sender: this.account.address,
				transactionHash,
				awaiting: false,
			});
			return transactionHash;
		}
		seiUtilEventEmitter.emit("transactionBroadcasted", {
			chainId: this.chainId,
			sender: this.account.address,
			transactionHash,
			awaiting: true,
		});
		const {
			confirmed: { timeoutMs = DEFAULT_TX_TIMEOUT_MS },
		} = finality;
		const result = await this.waitForTxConfirm(transactionHash, timeoutMs);
		if (result == null) {
			seiUtilEventEmitter.emit("transactionTimeout", {
				chainId: this.chainId,
				sender: this.account.address,
				transactionHash,
			});
			throw new TimeoutError(
				"Transaction " +
					transactionHash +
					" wasn't confirmed within " +
					timeoutMs / 1000 +
					" seconds. " +
					"You may want to check this again later",
				transactionHash
			);
		}
		seiUtilEventEmitter.emit("transactionConfirmed", {
			chainId: this.chainId,
			sender: this.account.address,
			result,
		});
		return result;
	}

	executeContract(instruction: ExecuteInstruction): Promise<DeliverTxResponse>;
	executeContract(instruction: ExecuteInstruction, memo?: string): Promise<DeliverTxResponse>;
	executeContract(instruction: ExecuteInstruction, memo?: string, fee?: "auto" | StdFee): Promise<DeliverTxResponse>;
	executeContract(
		instruction: ExecuteInstruction,
		memo: string | undefined,
		fee: "auto" | StdFee | undefined,
		finality: "broadcasted"
	): Promise<string>;
	executeContract(
		instruction: ExecuteInstruction,
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: { confirmed: { timeoutMs?: number } }
	): Promise<DeliverTxResponse>;
	executeContract(
		instruction: ExecuteInstruction,
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: TransactionFinality
	): Promise<DeliverTxResponse | string>;
	executeContract(
		instruction: ExecuteInstruction,
		memo: string = "",
		fee: "auto" | StdFee = "auto",
		finality: TransactionFinality = { confirmed: {} }
	): Promise<DeliverTxResponse | string> {
		return this.executeContractMulti([instruction], memo, fee, finality);
	}

	executeContractMulti(instructions: ExecuteInstruction[]): Promise<DeliverTxResponse>;
	executeContractMulti(instructions: ExecuteInstruction[], memo?: string): Promise<DeliverTxResponse>;
	executeContractMulti(
		instructions: ExecuteInstruction[],
		memo?: string,
		fee?: "auto" | StdFee
	): Promise<DeliverTxResponse>;
	executeContractMulti(
		instructions: ExecuteInstruction[],
		memo: string | undefined,
		fee: "auto" | StdFee | undefined,
		finality: "broadcasted"
	): Promise<string>;
	executeContractMulti(
		instructions: ExecuteInstruction[],
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: { confirmed: { timeoutMs?: number } }
	): Promise<DeliverTxResponse>;
	executeContractMulti(
		instructions: ExecuteInstruction[],
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: TransactionFinality
	): Promise<DeliverTxResponse | string>;
	executeContractMulti(
		instructions: ExecuteInstruction[],
		memo: string = "",
		fee: "auto" | StdFee = "auto",
		finality: TransactionFinality = { confirmed: {} }
	): Promise<DeliverTxResponse | string> {
		return this.signAndSend(this.execIxsToCosmosMsgs(instructions), memo, fee, finality);
	}
	/**
	 * Simulates the transaction and provides actually useful information. Like the events emitted.
	 *
	 * Because cosmjs says: "Why would anyone want any information other than estimated gas from a simulation?"
	 */
	async simulateTransaction(messages: readonly EncodeObject[]): Promise<SimulateResponse> {
		if (!this.isSignable()) {
			throw new Error("Cannot simulate transactions - " + this.readonlyReason);
		}
		const { sequence } = await this.wasmClient.getSequence(this.account.address);
		// Using [] notation bypasses the "protected" rule.
		return this.wasmClient["forceGetQueryClient"]().tx.simulate(
			messages.map((m) => this.wasmClient.registry.encodeAsAny(m)),
			undefined,
			encodeSecp256k1Pubkey(this.account.pubkey),
			sequence
		);
	}
	/**
	 * convenience function for simulating transactions containing cosmwasm messages
	 *
	 * @param instructions cosmwasm instructions to execute
	 * @returns the simulation result
	 */
	async simulateContractMulti(instructions: ExecuteInstruction[]): Promise<SimulateResponse> {
		return this.simulateTransaction(this.execIxsToCosmosMsgs(instructions));
	}
	/**
	 * convenience function for simulating transactions containing a single cosmwasm message
	 *
	 * @param instruction cosmwasm instructions to execute
	 * @returns the simulation result
	 */
	async simulateContract(instruction: ExecuteInstruction): Promise<SimulateResponse> {
		if (!this.isSignable()) {
			throw new Error("Cannot simulate transactions - " + this.readonlyReason);
		}
		return this.simulateContractMulti([instruction]);
	}
	async queryContract(contractAddress: string, query: object): Promise<any> {
		return await this.wasmClient.queryContractSmart(contractAddress, query);
	}
	async getBalance(unifiedDenom: string, accountAddress: string = this.getAccount().address): Promise<bigint> {
		if (unifiedDenom.startsWith("cw20/")) {
			// TODO: Add return types for cw20
			const { balance } = await this.queryContract(
				unifiedDenom.substring("cw20/".length),
				{
					balance: { address: accountAddress },
				} /* satisfies Cw20QueryMsg */
			); /* as Cw20BalanceResponse*/
			return BigInt(balance);
		} else {
			const result = await this.queryClient.cosmos.bank.v1beta1.balance({
				address: accountAddress,
				denom: unifiedDenom,
			});
			return BigInt(result.balance!.amount);
		}
	}
	execIxsToCosmosMsgs(instructions: ExecuteInstruction[]): MsgExecuteContractEncodeObject[] {
		if (!this.hasAccount()) {
			throw new Error("Can't get user wallet address - " + this.readonlyReason);
		}
		return instructions.map((i) => {
			if (i.funds) {
				// 🙄🙄🙄🙄
				i.funds = (i.funds as Coin[]).sort(nativeDenomSortCompare);
			}
			return {
				typeUrl: "/cosmwasm.wasm.v1.MsgExecuteContract",
				value: MsgExecuteContract.fromPartial({
					sender: this.account.address,
					contract: i.contractAddress,
					msg: i.msg instanceof Uint8Array ? i.msg : Buffer.from(JSON.stringify(i.msg)),
					funds: [...(i.funds || [])],
				}),
			};
		});
	}
	async getSupplyOf(unifiedDenom: string): Promise<bigint> {
		if (unifiedDenom.startsWith("cw20/")) {
			// TODO: Add return types for cw20
			const { total_supply } = await this.queryContract(
				unifiedDenom.substring("cw20/".length),
				{
					token_info: {},
				} /* satisfies Cw20QueryMsg */
			); /* as Cw20TokenInfoResponse*/
			return BigInt(total_supply);
		} else {
			const result = await this.queryClient.cosmos.bank.v1beta1.supplyOf({
				denom: unifiedDenom,
			});
			return BigInt(result.amount!.amount);
		}
	}
}

export class ContractDeployingClientEnv extends ClientEnv {
	static gasLimit = 4000000;
	async uploadContract(wasmCode: Uint8Array, allowFactories: boolean): Promise<UploadResult> {
		if (!this.isSignable()) {
			throw new Error("Cannot execute transactions - " + this.readonlyReason);
		}
		const result = await this.wasmClient.upload(
			this.account.address,
			wasmCode,
			calculateFee(ContractDeployingClientEnv.gasLimit, this.wasmClient["gasPrice"]),
			undefined,
			allowFactories
				? {
						address: "",
						addresses: [],
						permission: 3, // ACCESS_TYPE_EVERYBODY
					}
				: {
						// This property is apparently deprecrated but Sei can't understand anything else anyway
						address: this.account.address,
						addresses: [],
						permission: 2, // ACCESS_TYPE_ONLY_ADDRESS
					}
		);
		return result;
	}
	async instantiateContract(
		codeId: number,
		instantiateMsg: object,
		label: string,
		funds?: Coin[],
		upgradeAdmin: Addr | null = null
	): Promise<InstantiateResult> {
		if (!this.isSignable()) {
			throw new Error("Cannot execute transactions - " + this.readonlyReason);
		}
		if (funds && funds.length) {
			funds.sort(nativeDenomSortCompare);
		}
		const result = await this.wasmClient.instantiate(
			this.account.address,
			codeId,
			instantiateMsg,
			label,
			calculateFee(ContractDeployingClientEnv.gasLimit, this.wasmClient["gasPrice"]),
			{
				funds,
				admin: upgradeAdmin || undefined,
			}
		);
		return result;
	}
	async deployContract(
		wasmCode: Uint8Array,
		allowFactories: boolean,
		instantiateMsg: object,
		label: string,
		funds?: Coin[],
		upgradeAdmin: Addr | null = null
	): Promise<InstantiateResult> {
		const { codeId } = await this.uploadContract(wasmCode, allowFactories);
		return this.instantiateContract(codeId, instantiateMsg, label, funds, upgradeAdmin);
	}
	async migrateContract(contract: Addr, newCodeId: number, migrateMsg: object): Promise<MigrateResult> {
		if (!this.isSignable()) {
			throw new Error("Cannot execute transactions - " + this.readonlyReason);
		}
		return this.wasmClient.migrate(
			this.account.address,
			contract,
			newCodeId,
			migrateMsg,
			calculateFee(ContractDeployingClientEnv.gasLimit, this.wasmClient["gasPrice"]),
			undefined
		);
	}
	async upgradeContract(
		contract: Addr,
		wasmCode: Uint8Array,
		allowFactories: boolean,
		migrateMsg: object
	): Promise<MigrateResult> {
		const { codeId } = await this.uploadContract(wasmCode, allowFactories);
		return this.migrateContract(contract, codeId, migrateMsg);
	}
}
