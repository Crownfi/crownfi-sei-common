import { Coin, StdFee, encodeSecp256k1Pubkey } from "@cosmjs/amino";
import {
	ExecuteInstruction as WasmExecuteInstruction,
	InstantiateResult,
	MigrateResult,
	MsgExecuteContractEncodeObject,
	UploadResult,
} from "@cosmjs/cosmwasm-stargate";
import {
	KNOWN_SEI_PROVIDER_INFO,
	KnownSeiProviders,
	SeiQueryClient,
	SeiWallet,
	createSeiRegistry,
	getAddressStringFromPubKey,
	getRpcQueryClient,
	getSigningClient,
	getStargateClient,
	isValidSeiAddress
} from "@crownfi/sei-js-core";
import { seiUtilEventEmitter } from "./events.js";
import { EncodeObject, OfflineSigner, Registry, decodePubkey } from "@cosmjs/proto-signing";
import { SeiChainId, getCometClient, getDefaultNetworkConfig } from "./chain_config.js";
import { DeliverTxResponse, GasPrice, IndexedTx, SigningStargateClient, StargateClient, TimeoutError, calculateFee } from "@cosmjs/stargate";
import { nativeDenomSortCompare } from "./funds_util.js";
import { MsgExecuteContract } from "cosmjs-types/cosmwasm/wasm/v1/tx.js";
import { Addr } from "./common_sei_types.js";
import { getEvmAddressFromPubkey } from "./evm-interop-utils/address.js";
import { CometClient } from "@cosmjs/tendermint-rpc";
import { ClientAccountMissingError, ClientNotSignableError, ClientPubkeyUnknownError } from "./error.js";
import { EVMABIFunctionDefinition, functionSignatureToABIDefinition } from "./evm-interop-utils/abi/common.js";
import { encodeEvmFuncCall } from "./evm-interop-utils/index.js";
import { decodeEvmOutputAsArray, decodeEvmOutputAsStruct } from "./evm-interop-utils/abi/decode.js";
import { ERC20_FUNC_BALANCE_OF, ERC20_FUNC_TOTAL_SUPPLY } from "./evm-interop-utils/erc20.js";

// type SeiQueryClient = Awaited<ReturnType<typeof getQueryClient>>;

/*

const {getRpcQueryClient} = await import("@crownfi/sei-js-core");
const queryClient = await getRpcQueryClient("https://rpc.atlantic-2.seinetwork.io/");

txThing = await queryClient.txs.getTxsEvent({
events: ["message.sender='sei19e6kd2juw63wjcklgsgqf8lpm0g8460g9v49et'"],
orderBy: 0,
pagination: {
key: new Uint8Array([]),
offset: 0n,
countTotal: false,
reverse: false,
limit: 1n
}
})
*/

export interface EvmExecuteInstruction {
	contractAddress: string;
	evmMsg: {
		function: string | EVMABIFunctionDefinition
		params: any[],
		funds?: Coin
	};
}

export type EvmOrWasmExecuteInstruction = EvmExecuteInstruction | WasmExecuteInstruction;

export interface SeiClientAccountData {
	readonly seiAddress: string;
	readonly evmAddress: string;
    readonly pubkey?: Uint8Array;
}

// Yes, these never get free'd. Too bad!
const pubkeyCache: {[address: string]: SeiClientAccountData} = {};

async function tryGetAccountDataFromAddress(queryClient: SeiQueryClient, seiOrEvmAddress: string): Promise<SeiClientAccountData | null> {
	if (pubkeyCache[seiOrEvmAddress]) {
		return pubkeyCache[seiOrEvmAddress];
	}
	let evmAddress = "";
	let seiAddress = "";
	if (seiOrEvmAddress.startsWith("0x")) {
		evmAddress = seiOrEvmAddress;
		seiAddress = (await queryClient.evm.seiAddressByEVMAddress({evmAddress: seiOrEvmAddress})).seiAddress;
		if (seiAddress == "") {
			return null;
		}
	} else {
		if(!isValidSeiAddress(seiOrEvmAddress)) {
			return null;
		}
		seiAddress = seiOrEvmAddress;
	}
	const {txs} = await queryClient.txs.getTxsEvent({
		events: ["message.sender='" + seiAddress + "'"],
		orderBy: 0,
		pagination: {
			key: new Uint8Array([0x13, 0x37]),
			offset: 0n,
			countTotal: true,
			reverse: false,
			limit: 1n
		}
	});
	const pubkey = (() => {
		if (!txs.length || txs[0].authInfo == null) {
			return null;
		}
		for (let i = 0; i < txs[0].authInfo.signerInfos.length; i += 1) {
			const signerInfo = txs[0].authInfo.signerInfos[i];
			if (signerInfo.publicKey == null || signerInfo.publicKey.typeUrl != "/cosmos.crypto.secp256k1.PubKey") {
				continue;
			}
			const pubkey = Buffer.from(decodePubkey(signerInfo.publicKey).value, "base64");
			if (getAddressStringFromPubKey(pubkey) == seiAddress) {
				return pubkey;
			}
		}
		return null;
	})();
	if (pubkey == null) {
		if (!evmAddress) {
			evmAddress = (await queryClient.evm.eVMAddressBySeiAddress({seiAddress})).evmAddress;
		}
		if (!evmAddress) {
			return null;
		}
		return {
			seiAddress,
			evmAddress
		};
	}
	if (!evmAddress) {
		evmAddress = getEvmAddressFromPubkey(pubkey);
	}
	const result = {
		seiAddress,
		evmAddress,
		pubkey
	};
	Object.freeze(result);
	pubkeyCache[seiAddress] = result;
	pubkeyCache[evmAddress] = result;
	return result;
}



export type MaybeSelectedProviderString = KnownSeiProviders |
	"ethereum" |
	"seed-wallet" |
	"read-only-address" |
	null;
export type MaybeSelectedProvider = KnownSeiProviders |
	"ethereum" |
	{ seed: string; index?: number, cointype?: number } |
	{ address: string } |
	null;


const DEFAULT_TX_TIMEOUT_MS = 60000;
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
export type SimulateResponse = Awaited<ReturnType<SeiQueryClient["tx"]["simulate"]>>;
interface ClientEnvConstruct {
	account: SeiClientAccountData | null;
	chainId: SeiChainId;
	cometClient: CometClient,
	signer: OfflineSigner | null;
	stargateClient: SigningStargateClient | StargateClient;
	queryClient: SeiQueryClient;
	readonlyReason: string;
	cosmRegistry: Registry;
}
let defaultProvider: MaybeSelectedProvider = null;
let defaultGasPrice = GasPrice.fromString("0.1usei");
export class ClientEnv {
	protected signer: OfflineSigner | null;
	protected cometClient: CometClient;
	readonly account: SeiClientAccountData | null;
	readonly chainId: SeiChainId;
	readonly readonlyReason: string;

	readonly stargateClient: SigningStargateClient | StargateClient;
	readonly queryClient: SeiQueryClient;
	readonly cosmRegistry: Registry;

	static getDefaultProvider(): MaybeSelectedProviderString {
		return maybeProviderToMaybeString(defaultProvider);
	}
	/**
	 * Sets the default provider to `null` synchronously
	 */
	static nullifyDefaultProvider() {
		if (defaultProvider != null) {
			const chainId = getDefaultNetworkConfig().chainId;
			if (typeof defaultProvider == "string") {
				if (defaultProvider == "ethereum") {
					throw new Error("TODO: Ethereum client handling");
				} else {
					try {
						new SeiWallet(defaultProvider).disconnect(chainId).catch((_) => {});
					}catch(ex: any) {
						// We're informing the wallet as a courtesy, we don't care what they have to say about it.
					}
				}
			}
			defaultProvider = null;
			seiUtilEventEmitter.emit("defaultProviderChangeRequest", {
				status: "success",
				provider: null,
			});
			seiUtilEventEmitter.emit("defaultProviderChanged", {
				chainId: chainId,
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
		const chainId = getDefaultNetworkConfig().chainId;
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
				(defaultProvider as any).index == (oldProvider as any).index &&
				(defaultProvider as any).cointype == (oldProvider as any).cointype
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
				chainId,
				provider: newProviderString,
				account: null,
			});
			if (typeof oldProvider == "string") {
				if (oldProvider == "ethereum") {
					throw new Error("TODO: Ethereum client handling");
				} else {
					try {
						new SeiWallet(oldProvider).disconnect(chainId).catch((_) => {});
					}catch(ex: any) {
						// We're informing the wallet as a courtesy, we don't care what they have to say about it.
					}
				}
			}
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
					chainId,
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
		chainId: SeiChainId
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
					signer: await restoreWallet(provider.seed, provider.index, provider.cointype),
				};
			}
			return {
				address: provider.address,
			};
		}
		if (provider == "ethereum") {
			throw new Error("TODO: Check if local storage exists and check it for matching pubkey, else, ask the user to sign something so we can get the pubkey")
		}
		try {
			const signer = await new SeiWallet(provider).getOfflineSigner(chainId);
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
		chainId: SeiChainId = getDefaultNetworkConfig().chainId,
		gasPrice: GasPrice = defaultGasPrice
	): Promise<InstanceType<T>> {
		const cometClient = await getCometClient(chainId);
		const queryClient = await getRpcQueryClient(cometClient);
		const [stargateClient, signer, account, readonlyReason] = await (async () => {
			const maybeSigner = await ClientEnv.getSigner(provider, chainId);
			if ("failure" in maybeSigner) {
				return [await getStargateClient(cometClient), null, null, maybeSigner.failure];
			} else if ("address" in maybeSigner) {
				const accountData = await tryGetAccountDataFromAddress(queryClient, maybeSigner.address);
				if (accountData == null) {
					return [
						await getStargateClient(cometClient),
						null,
						null,
						maybeSigner.address + " is an invalid address or doesn't have transaction history"
					];
				} else {
					return [
						await getStargateClient(cometClient),
						null,
						accountData,
						""
					];
				}
			}
			const { signer } = maybeSigner;
			const accounts = await signer.getAccounts();
			if (accounts.length !== 1) {
				return [
					await getStargateClient(cometClient),
					null,
					null,
					"Expected wallet to expose exactly 1 account but got " + accounts.length + " accounts",
				];
			}
			if (accounts[0].algo != "secp256k1") {
				// The "real" reason is that we can only derive EVM compatible addresses from secp256k1 public keys
				return [
					await getStargateClient(cometClient),
					null,
					null,
					"An account was received which does not use the \"secp256k1\" signing algorithm. " +
						"This effectively makes the account incompatible with the Sei network."
				];
			}
			return [
				await getSigningClient(
					cometClient,
					signer,
					{
						gasPrice
					}
				),
				signer,
				{
					seiAddress: accounts[0].address,
					pubkey: accounts[0].pubkey,
					evmAddress: getEvmAddressFromPubkey(accounts[0].pubkey)
				},
				"",
			];
		})();
		return new this({
			account,
			cometClient,
			chainId,
			signer,
			stargateClient,
			queryClient,
			readonlyReason,
			// In order to facilitate the simulation of read-only wallets, we need the registry to encode the simulated transaction
			cosmRegistry: createSeiRegistry()
		}) as InstanceType<T>;
	}
	/**
	 * use of the constructor is discouraged and isn't guaranteed to be stable. Use the get() function instead.
	 */
	constructor({ account, chainId, cometClient, signer, stargateClient, queryClient, readonlyReason, cosmRegistry }: ClientEnvConstruct) {
		this.account = account;
		this.chainId = chainId;
		this.cometClient = cometClient;
		this.signer = signer;
		this.stargateClient = stargateClient;
		this.queryClient = queryClient;
		this.readonlyReason = readonlyReason;
		this.cosmRegistry = cosmRegistry;
	}
	/**
	 * Conveniently throws an error with the underlying reason if the account property is null
	 */
	getAccount() {
		if (this.account == null) {
			throw new ClientAccountMissingError("Can't get user wallet address - " + this.readonlyReason);
		}
		return this.account;
	}
	/**
	 * @returns true if tranasction signing is available and the wallet is known
	 */
	isSignable(): this is { signer: OfflineSigner, stargateClient: SigningStargateClient; account: SeiClientAccountData } {
		return this.stargateClient instanceof SigningStargateClient && this.account != null;
	}

	/**
	 * If you want to actually check if transactions can be sent, use the `isSignable` method
	 *
	 * @returns true if the wallet is known
	 */
	hasAccount(): this is { account: SeiClientAccountData } {
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
			result = await this.stargateClient.getTx(tx);
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
			throw new ClientNotSignableError("Cannot execute transactions - " + this.readonlyReason);
		}
		const transactionHash = await this.stargateClient.signAndBroadcastSync(this.account.seiAddress, msgs, fee, memo);
		if (finality == "broadcasted") {
			seiUtilEventEmitter.emit("transactionBroadcasted", {
				chainId: this.chainId,
				sender: this.account.seiAddress,
				transactionHash,
				awaiting: false,
			});
			return transactionHash;
		}
		seiUtilEventEmitter.emit("transactionBroadcasted", {
			chainId: this.chainId,
			sender: this.account.seiAddress,
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
				sender: this.account.seiAddress,
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
			sender: this.account.seiAddress,
			result,
		});
		return result;
	}

	executeContract(instruction: EvmOrWasmExecuteInstruction): Promise<DeliverTxResponse>;
	executeContract(instruction: EvmOrWasmExecuteInstruction, memo?: string): Promise<DeliverTxResponse>;
	executeContract(instruction: EvmOrWasmExecuteInstruction, memo?: string, fee?: "auto" | StdFee): Promise<DeliverTxResponse>;
	executeContract(
		instruction: EvmOrWasmExecuteInstruction,
		memo: string | undefined,
		fee: "auto" | StdFee | undefined,
		finality: "broadcasted"
	): Promise<string>;
	executeContract(
		instruction: EvmOrWasmExecuteInstruction,
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: { confirmed: { timeoutMs?: number } }
	): Promise<DeliverTxResponse>;
	executeContract(
		instruction: EvmOrWasmExecuteInstruction,
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: TransactionFinality
	): Promise<DeliverTxResponse | string>;
	executeContract(
		instruction: EvmOrWasmExecuteInstruction,
		memo: string = "",
		fee: "auto" | StdFee = "auto",
		finality: TransactionFinality = { confirmed: {} }
	): Promise<DeliverTxResponse | string> {
		return this.executeContractMulti([instruction], memo, fee, finality);
	}

	executeContractMulti(instructions: EvmOrWasmExecuteInstruction[]): Promise<DeliverTxResponse>;
	executeContractMulti(instructions: EvmOrWasmExecuteInstruction[], memo?: string): Promise<DeliverTxResponse>;
	executeContractMulti(
		instructions: EvmOrWasmExecuteInstruction[],
		memo?: string,
		fee?: "auto" | StdFee
	): Promise<DeliverTxResponse>;
	executeContractMulti(
		instructions: EvmOrWasmExecuteInstruction[],
		memo: string | undefined,
		fee: "auto" | StdFee | undefined,
		finality: "broadcasted"
	): Promise<string>;
	executeContractMulti(
		instructions: EvmOrWasmExecuteInstruction[],
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: { confirmed: { timeoutMs?: number } }
	): Promise<DeliverTxResponse>;
	executeContractMulti(
		instructions: EvmOrWasmExecuteInstruction[],
		memo?: string,
		fee?: "auto" | StdFee,
		finality?: TransactionFinality
	): Promise<DeliverTxResponse | string>;
	executeContractMulti(
		instructions: EvmOrWasmExecuteInstruction[],
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
		if (!this.hasAccount()) {
			throw new ClientAccountMissingError("Cannot simulate transactions - " + this.readonlyReason);
		}
		if (this.account.pubkey == null) {
			throw new ClientPubkeyUnknownError("Public key is required to simulate transactions");
		}
		const { sequence } = await this.stargateClient.getSequence(this.account.seiAddress);
		
		return this.queryClient.tx.simulate(
			messages.map((m) => this.cosmRegistry.encodeAsAny(m)),
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
	async simulateContractMulti(instructions: EvmOrWasmExecuteInstruction[]): Promise<SimulateResponse> {
		return this.simulateTransaction(this.execIxsToCosmosMsgs(instructions));
	}
	/**
	 * convenience function for simulating transactions containing a single cosmwasm message
	 *
	 * @param instruction cosmwasm instructions to execute
	 * @returns the simulation result
	 */
	async simulateContract(instruction: EvmOrWasmExecuteInstruction): Promise<SimulateResponse> {
		return this.simulateContractMulti([instruction]);
	}
	async queryContract(contractAddress: string, query: object): Promise<any> {
		return await this.queryClient.wasm.queryContractSmart(contractAddress, query);
	}
	async queryEvmContract(
		contractAddress: string,
		functionDefinition: EVMABIFunctionDefinition | string,
		params: any[]
	): Promise<any[]> {
		if (typeof functionDefinition == "string") {
			functionDefinition = functionSignatureToABIDefinition(functionDefinition);
		}
		const result = await this.queryClient.evm.staticCall({
			data: encodeEvmFuncCall(functionDefinition, params),
			to: contractAddress
		});
		return decodeEvmOutputAsArray(Buffer.from(result.data), functionDefinition.outputs);
	}
	async queryEvmContractForObject(
		contractAddress: string,
		functionDefinition: EVMABIFunctionDefinition | string,
		params: any[]
	): Promise<any> {
		if (typeof functionDefinition == "string") {
			functionDefinition = functionSignatureToABIDefinition(functionDefinition);
		}
		const result = await this.queryClient.evm.staticCall({
			data: encodeEvmFuncCall(functionDefinition, params),
			to: contractAddress
		});
		return decodeEvmOutputAsStruct(Buffer.from(result.data), functionDefinition.outputs);
	}
	async getBalance(unifiedDenom: string, accountAddress?: string): Promise<bigint> {
		if (unifiedDenom.startsWith("erc20/")) {
			if (!accountAddress) {
				accountAddress = this.getAccount().evmAddress;
			} else if (!accountAddress.startsWith("0x")) {
				accountAddress = (
					await this.queryClient.evm.eVMAddressBySeiAddress({seiAddress: accountAddress})
				).evmAddress;
				if (!accountAddress) {
					// Not found, return 0.
					return 0n;
				}
			}
		} else {
			if (!accountAddress) {
				accountAddress = this.getAccount().seiAddress;
			} else if (accountAddress.startsWith("0x")) {
				accountAddress = (
					await this.queryClient.evm.seiAddressByEVMAddress({evmAddress: accountAddress})
				).seiAddress;
				if (!accountAddress) {
					// Not found, return 0.
					return 0n;
				}
			}
		}
		if (unifiedDenom.startsWith("cw20/")) {
			// TODO: Add return types for cw20
			const { balance } = await this.queryContract(
				unifiedDenom.substring("cw20/".length),
				{
					balance: { address: accountAddress },
				} /* satisfies Cw20QueryMsg */
			); /* as Cw20BalanceResponse*/
			return BigInt(balance);
		} else if (unifiedDenom.startsWith("erc20/")) {
			// u256 gets encoded as bigint
			return (
				await this.queryEvmContract(unifiedDenom.substring("erc20/".length), ERC20_FUNC_BALANCE_OF, [accountAddress])
			)[0];
		} else {
			const result = await this.queryClient.bank.balance(accountAddress, unifiedDenom);
			return BigInt(result.amount);
		}
	}
	execIxsToCosmosMsgs(instructions: EvmOrWasmExecuteInstruction[]): EncodeObject[] {
		if (!this.hasAccount()) {
			throw new ClientAccountMissingError("Can't get user wallet address - " + this.readonlyReason);
		}
		return instructions.map((i) => {
			if ("evmMsg" in i) {
				return {
					typeUrl: "/seiprotocol.seichain.eth.callEvmPleaseLetMeDoThis",
					value: {
						sender: this.account.seiAddress,
						to: i.contractAddress,
						data: encodeEvmFuncCall(i.evmMsg.function, i.evmMsg.params),
						funds: i.evmMsg.funds
					}
				};
			}
			if (i.funds) {
				i.funds = (i.funds as Coin[]).sort(nativeDenomSortCompare);
			}
			return {
				typeUrl: "/cosmwasm.wasm.v1.MsgExecuteContract",
				value: MsgExecuteContract.fromPartial({
					sender: this.account.seiAddress,
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
		} else if (unifiedDenom.startsWith("erc20/")) {
			// uint256 gets decoded to bigint
			return (await this.queryEvmContract(
				unifiedDenom.substring("cw20/".length),
				ERC20_FUNC_TOTAL_SUPPLY,
				[]
			))[0];
		} else {
			const result = await this.queryClient.bank.supplyOf(unifiedDenom);
			return BigInt(result.amount);
		}
	}
}

/**
 * An extended `ClientEnv` with more methods for contract deployments. Implemented as a seperate class so tree-shaking
 * can actually happen.
 */
export class ContractDeployingClientEnv extends ClientEnv {
	static gasLimit = 4000000;
	async #wasmClient() {
		if (!this.isSignable()) {
			throw new ClientNotSignableError("Cannot execute transactions - " + this.readonlyReason);
		}
		return (await import ("@cosmjs/cosmwasm-stargate")).SigningCosmWasmClient.createWithSigner(this.cometClient, this.signer)
	}
	async uploadContract(wasmCode: Uint8Array, allowFactories: boolean): Promise<UploadResult> {
		if (!this.isSignable()) {
			throw new ClientNotSignableError("Cannot execute transactions - " + this.readonlyReason);
		}
		const result = await (await this.#wasmClient()).upload(
			this.account.seiAddress,
			wasmCode,
			calculateFee(ContractDeployingClientEnv.gasLimit, this.stargateClient["gasPrice"]),
			undefined,
			allowFactories
				? {
						address: "",
						addresses: [],
						permission: 3, // ACCESS_TYPE_EVERYBODY
				  }
				: {
						// This property is apparently deprecrated but Sei can't understand anything else anyway
						address: this.account.seiAddress,
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
			throw new ClientNotSignableError("Cannot execute transactions - " + this.readonlyReason);
		}
		if (funds && funds.length) {
			funds.sort(nativeDenomSortCompare);
		}
		const result = await (await this.#wasmClient()).instantiate(
			this.account.seiAddress,
			codeId,
			instantiateMsg,
			label,
			calculateFee(ContractDeployingClientEnv.gasLimit, this.stargateClient["gasPrice"]),
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
			throw new ClientNotSignableError("Cannot execute transactions - " + this.readonlyReason);
		}
		return (await this.#wasmClient()).migrate(
			this.account.seiAddress,
			contract,
			newCodeId,
			migrateMsg,
			calculateFee(ContractDeployingClientEnv.gasLimit, this.stargateClient["gasPrice"]),
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
