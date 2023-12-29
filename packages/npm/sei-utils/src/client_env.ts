import { AccountData, Coin, encodeSecp256k1Pubkey } from "@cosmjs/amino"
import { CosmWasmClient, ExecuteInstruction, InstantiateResult, MigrateResult, MsgExecuteContractEncodeObject, SigningCosmWasmClient, UploadResult } from "@cosmjs/cosmwasm-stargate"
import { KNOWN_SEI_PROVIDER_INFO, KnownSeiProviders, SeiWallet, getCosmWasmClient, getQueryClient, getSigningCosmWasmClient } from "@crownfi/sei-js-core"
import { seiUtilEventEmitter } from "./events.js";
import { EncodeObject, OfflineSigner } from "@cosmjs/proto-signing";
import { SeiChainNetConfig, getDefaultNetworkConfig } from "./chain_config.js";
import { GasPrice, calculateFee, isDeliverTxFailure } from "@cosmjs/stargate";
import { nativeDenomSortCompare } from "./funds_util.js";
import { MsgExecuteContract } from "cosmjs-types/cosmwasm/wasm/v1/tx.js";
import { Addr } from "./common_sei_types.js";

export type MaybeSelectedProviderString = KnownSeiProviders | "seed-wallet" | null;
export type MaybeSelectedProvider = KnownSeiProviders | {seed: string} | null;

export class TransactionError extends Error {
	name!: "TransactionError"
	public constructor(
		public code: string | number,
		public txhash: string | undefined,
		public rawLog: string,
	) {
		super("Transaction confirmed with an error")
	}
}
TransactionError.prototype.name == "TransactionError";

function maybeProviderToMaybeString(provider: MaybeSelectedProvider): MaybeSelectedProviderString {
	if (typeof provider == "object" && provider != null) {
		return "seed-wallet";
	}
	return provider;
}
export type SimulateResponse = Awaited<ReturnType<ReturnType<CosmWasmClient["forceGetQueryClient"]>["tx"]["simulate"]>>;
interface ClientEnvConstruct {
	account: AccountData | null
	chainId: string
	wasmClient: SigningCosmWasmClient | CosmWasmClient
	queryClient: Awaited<ReturnType<typeof getQueryClient>>
	readonlyReason: string
}
let defaultProvider: MaybeSelectedProvider = null;
export class ClientEnv {
	account: AccountData | null
	chainId: string
	wasmClient: SigningCosmWasmClient | CosmWasmClient
	queryClient: Awaited<ReturnType<typeof getQueryClient>>
	readonlyReason: string

	/**
	 * Sets the default provider to `null` synchronously
	 */
	static nullifyDefaultProvider() {
		if (defaultProvider != null) {
			defaultProvider = null;
			seiUtilEventEmitter.emit(
				"defaultProviderChangeRequest",
				{
					status: "success",
					provider: null
				}
			);
			seiUtilEventEmitter.emit(
				"defaultProviderChanged",
				{
					chainId: getDefaultNetworkConfig().chainId,
					provider: null,
					account: null
				}
			);
		}
	}
	static async setDefaultProvider(provider: MaybeSelectedProvider, dontThrowOnFail: boolean = false) {
		const oldProvider = defaultProvider;
		defaultProvider = provider;
		if (oldProvider == defaultProvider) {
			return;
		}
		if (typeof defaultProvider == "object" && typeof oldProvider == "object") {
			if (defaultProvider != null && oldProvider != null && defaultProvider.seed == oldProvider.seed) {
				return;
			}
		}
		const newProviderString = maybeProviderToMaybeString(defaultProvider);
		if (defaultProvider == null) {
			seiUtilEventEmitter.emit(
				"defaultProviderChangeRequest",
				{
					status: "success",
					provider: newProviderString
				}
			);
			seiUtilEventEmitter.emit(
				"defaultProviderChanged",
				{
					chainId: getDefaultNetworkConfig().chainId,
					provider: newProviderString,
					account: null
				}
			);
		} else {
			try{
				seiUtilEventEmitter.emit(
					"defaultProviderChangeRequest",
					{
						status: "requesting",
						provider: newProviderString
					}
				);
				const clientEnv = await ClientEnv.get();
				const clientAccount = clientEnv.getAccount();
				seiUtilEventEmitter.emit(
					"defaultProviderChangeRequest",
					{
						status: "success",
						provider: newProviderString
					}
				);
				seiUtilEventEmitter.emit(
					"defaultProviderChanged",
					{
						chainId: getDefaultNetworkConfig().chainId,
						provider: newProviderString,
						account: clientAccount.address
					}
				);
			}catch(ex) {
				defaultProvider = oldProvider;
				seiUtilEventEmitter.emit(
					"defaultProviderChangeRequest",
					{
						status: "failure",
						provider: newProviderString,
						failureException: ex
					}
				);
				if (!dontThrowOnFail) {
					throw ex;
				}
			}
		}
	}
	private static async getSigner(
		provider: MaybeSelectedProvider,
		networkConfig: SeiChainNetConfig
	): Promise<OfflineSigner | string> {
		if (provider == null) {
			return "No wallet selected";
		}
		if (typeof provider == "object") {
			// async imports allow us to load the signing stuff only if needed. (hopefully)
			const {restoreWallet} = await import("@crownfi/sei-js-core");
			// Just support the NULL seed for now.
			return await restoreWallet(provider.seed)
		}
		const signer = await (new SeiWallet(provider)).getOfflineSigner(networkConfig.chainId);
		if (signer == undefined) {
			return KNOWN_SEI_PROVIDER_INFO[provider].name +
				" did not provide a signer. (Is the wallet unlocked and are we authorized?)";
		}
		return signer;
	}

	static async get<T extends typeof ClientEnv>(
		this: T,
		provider: MaybeSelectedProvider = defaultProvider,
		networkConfig: SeiChainNetConfig = getDefaultNetworkConfig()
	): Promise<InstanceType<T>> {
		const queryClient = await getQueryClient(networkConfig.restUrl);

		const [wasmClient, account, readonlyReason] = await (async () => {
			const maybeSigner = await ClientEnv.getSigner(provider, networkConfig);
			if (typeof maybeSigner == "string") {
				return [await getCosmWasmClient(networkConfig.rpcUrl), null, maybeSigner];
			}
			const accounts = await maybeSigner.getAccounts();
			if (accounts.length !== 1) {
				return [
					await getCosmWasmClient(networkConfig.rpcUrl),
					null,
					"Expected wallet to expose exactly 1 account but got " + accounts.length + " accounts"
				];
			}
			return [
				await getSigningCosmWasmClient(
					networkConfig.rpcUrl,
					maybeSigner,
					{
						gasPrice: GasPrice.fromString("0.1usei")
					}
				),
				accounts[0],
				""
			]
		})();
		if (account != null) {
			console.info("Found user address:", account.address);
		}
		return new this({ account, chainId: networkConfig.chainId, wasmClient, queryClient, readonlyReason }) as InstanceType<T>;
	}
	/**
	 * use of the constructor is discouraged and isn't guaranteed to be stable. Use the get() function instead.
	 */
	constructor({account, chainId, wasmClient, queryClient, readonlyReason}: ClientEnvConstruct) {
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
	 * @returns true if tranasction signing is available
	 */
	isSignable(): this is { wasmClient: SigningCosmWasmClient, account: AccountData } {
		return (this.wasmClient instanceof SigningCosmWasmClient) && this.account != null;
	}

	async signAndSend(msgs: EncodeObject[]) {
		if (!this.isSignable()) {
			throw new Error("Cannot execute transactions - " + this.readonlyReason);
		}
		const result = await this.wasmClient.signAndBroadcast(this.account.address, msgs, "auto")
		if (isDeliverTxFailure(result)) {
			throw new TransactionError(result.code, result.transactionHash, result.rawLog + "")
		}
		seiUtilEventEmitter.emit("transactionConfirmed", {
			chainId: this.chainId,
			sender: this.account.address,
			result
		})
		return result
	}
	async executeContract(contractAddress: string, msg: object, funds?: Coin[]) {
		if (!this.isSignable()) {
			throw new Error("Cannot execute transactions - " + this.readonlyReason);
		}
		if (funds) {
			// 🙄🙄🙄🙄
			funds.sort(nativeDenomSortCompare);
		}
		const result = await this.wasmClient.execute(this.account.address, contractAddress, msg, "auto", undefined, funds)
		seiUtilEventEmitter.emit("transactionConfirmed", {
			chainId: this.chainId,
			sender: this.account.address,
			result
		});
		return result;
	}
	async executeContractMulti(instructions: ExecuteInstruction[]) {
		if (!this.isSignable()) {
			throw new Error("Cannot execute transactions - " + this.readonlyReason);
		}
		for(let i = 0; i < instructions.length; i += 1){
			if (instructions[i].funds) {
				// 🙄🙄🙄🙄🙄🙄🙄🙄🙄🙄🙄🙄🙄🙄🙄🙄
				instructions[i].funds = (instructions[i].funds as Coin[]).sort(nativeDenomSortCompare);
			}
		}
		const result = await this.wasmClient.executeMultiple(this.account.address, instructions, "auto");
		seiUtilEventEmitter.emit("transactionConfirmed", {
			chainId: this.chainId,
			sender: this.account.address,
			result
		});
		return result;
	}
	async simulateTransactionButWithActuallyUsefulInformation(messages: readonly EncodeObject[]): Promise<SimulateResponse> {
		// Because cosmjs says: "Why would anyone want any information other than estimated gas from a simulation?"
		if (!this.isSignable()) {
			throw new Error("Cannot execute transactions - " + this.readonlyReason);
		}
		const { sequence } = await this.wasmClient.getSequence(this.account.address);
		// Using [] notation bypasses the "protected" rule.
		return this.wasmClient["forceGetQueryClient"]().tx.simulate(
			messages.map((m) => this.wasmClient.registry.encodeAsAny(m)),
			undefined,
			encodeSecp256k1Pubkey(this.account.pubkey),
			sequence
		)
	}
	async simulateContractMulti(
		instructions: ExecuteInstruction[]
	): Promise<SimulateResponse> {
		if (!this.isSignable()) {
			throw new Error("Cannot execute transactions - " + this.readonlyReason);
		}
		const msgs: MsgExecuteContractEncodeObject[] = instructions.map((i) => {
			if (i.funds) {
				// 🙄🙄🙄🙄
				i.funds = (i.funds as Coin[]).sort(nativeDenomSortCompare);
			}
			return {
				typeUrl: "/cosmwasm.wasm.v1.MsgExecuteContract",
					value: MsgExecuteContract.fromPartial({
					sender: this.account.address,
					contract: i.contractAddress,
					msg: Buffer.from(JSON.stringify(i.msg)),
					funds: [...(i.funds || [])],
				}),
			}
		});
		return this.simulateTransactionButWithActuallyUsefulInformation(msgs);
	}
	async simulateContract(
		contractAddress: string, msg: object, funds?: Coin[]
	): Promise<SimulateResponse> {
		if (!this.isSignable()) {
			throw new Error("Cannot execute transactions - " + this.readonlyReason);
		}
		const instruction: ExecuteInstruction = {
			contractAddress: contractAddress,
			msg: msg,
			funds: funds,
		};
		return this.simulateContractMulti([instruction]);
	  }
	async queryContract(contractAddress: string, query: object): Promise<any> {
		return await this.wasmClient.queryContractSmart(contractAddress, query)
	}
	async getBalance(unifiedDenom: string, accountAddress: string = this.getAccount().address): Promise<bigint> {
		if (unifiedDenom.startsWith("cw20/")) {
			// TODO: Add return types for cw20
			const {balance} = await this.queryContract(
				unifiedDenom.substring("cw20/".length),
				{
					balance: {address: accountAddress}
				}/* satisfies Cw20QueryMsg */
			) /* as Cw20BalanceResponse*/;
			return BigInt(balance);
		}else{
			const result = await this.queryClient.cosmos.bank.v1beta1.balance({
				address: accountAddress,
				denom: unifiedDenom
			});
			return BigInt(result.balance!.amount);
		}
	}
	async getSupplyOf(unifiedDenom: string): Promise<bigint> {
		if (unifiedDenom.startsWith("cw20/")) {
			// TODO: Add return types for cw20
			const {total_supply} = await this.queryContract(
				unifiedDenom.substring("cw20/".length),
				{
					token_info: {}
				}/* satisfies Cw20QueryMsg */
			) /* as Cw20TokenInfoResponse*/;
			return BigInt(total_supply);
		}else{
			const result = await this.queryClient.cosmos.bank.v1beta1.supplyOf({
				denom: unifiedDenom
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
			allowFactories ? {
				"address": "",
				"addresses": [],
				"permission": 3 // ACCESS_TYPE_EVERYBODY
			} : {
				// This property is apparently deprecrated but Sei can't understand anything else anyway
				"address": this.account.address,
				"addresses": [],
				"permission": 2 // ACCESS_TYPE_ONLY_ADDRESS
			}
		)
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
				admin: upgradeAdmin || undefined
			}
		)
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
		const {codeId} = await this.uploadContract(wasmCode, allowFactories);
		return this.instantiateContract(codeId, instantiateMsg, label, funds, upgradeAdmin);
	}
	async migrateContract(
		contract: Addr,
		newCodeId: number,
		migrateMsg: object
	): Promise<MigrateResult> {
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
		const {codeId} = await this.uploadContract(wasmCode, allowFactories);
		return this.migrateContract(
			contract,
			codeId,
			migrateMsg
		);
	}
}
