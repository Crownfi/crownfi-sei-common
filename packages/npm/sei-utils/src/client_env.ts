import { Coin, StdFee, encodeSecp256k1Pubkey } from "@cosmjs/amino";
import {
	InstantiateResult,
	MigrateResult,
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
	getStargateClient
} from "@crownfi/sei-js-core";
import { seiUtilEventEmitter } from "./events.js";
import { EncodeObject, OfflineSigner, Registry } from "@cosmjs/proto-signing";
import { SeiChainId, getCometClient, getDefaultNetworkConfig, getNetworkConfig } from "./chain_config.js";
import { DeliverTxResponse, GasPrice, IndexedTx, SigningStargateClient, StargateClient, TimeoutError, calculateFee } from "@cosmjs/stargate";
import { CometClient } from "@cosmjs/tendermint-rpc";
import { EthereumProvider, ReceiptInformation, Transaction as EvmTransaction } from "@crownfi/ethereum-rpc-types";
import { Secp256k1, ExtendedSecp256k1Signature } from '@cosmjs/crypto'; // Heavy import but it's already imported elsewhere

import { nativeDenomSortCompare, UIAmount, UnifiedDenom } from "./funds_util.js";
import { Addr } from "./common_sei_types.js";
import { getEvmAddressFromPubkey, toChecksumAddressEvm } from "./evm-interop-utils/address.js";
import { ClientAccountMissingError, ClientNotSignableError, ClientPubkeyUnknownError, EvmAddressValidationMismatchError } from "./error.js";
import { EVMABIFunctionDefinition } from "./evm-interop-utils/abi/common.js";
import { addSeiClientAccountDataToCache, cosmosMessagesToEvmMessages, encodeEvmFuncCall, getSeiClientAccountDataFromNetwork, queryEvmContract, queryEvmContractForObject } from "./evm-interop-utils/index.js";
import { ERC20_FUNC_BALANCE_OF, ERC20_FUNC_TOTAL_SUPPLY } from "./evm-interop-utils/erc20.js";
import { EvmOrWasmExecuteInstruction } from "./contract_base.js";
import { keccak256 } from "keccak-wasm";
import { StupidCompassWalletWorkaround } from "./evm-interop-utils/stupid_compass_wallet_workaround.js";

/**
 * The account data used by the `ClientEnv` with a wallet.
 */
export interface SeiClientAccountData {
	/**
	 * The user's `sei1...` address.
	 */
	readonly seiAddress: string;
	/**
	 * The user's `0x...` address.
	 */
	readonly evmAddress: string;
	/**
	 * The user's public key used to derive both addresses. May be undefined if this is a read-only account.
	 */
	readonly pubkey?: Uint8Array;
}

/**
 * The string representation of the selected wallet provider
 */
export type MaybeSelectedProviderString = KnownSeiProviders |
	"ethereum" |
	"seed-wallet" |
	"read-only-address" |
	null;

/**
 * Which wallet provider to use.
 * * `ethereum`: use window-ethereum
 * * `{seed: "...", ...}`: recover a wallet with a mnemonic seed
 * * `{address: "..."}`: a read-only account
 */
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
/**
 * The result of a simulated cosmos transaction
 */
export type SimulateResponse = Awaited<ReturnType<SeiQueryClient["tx"]["simulate"]>>;
interface ClientEnvConstruct {
	account: SeiClientAccountData | null;
	chainId: SeiChainId;
	cometClient: CometClient,
	signer: OfflineSigner | null;
	stargateClient: SigningStargateClient | StargateClient;
	queryClient: SeiQueryClient;
	ethereumClient: EthereumProvider | null;
	readonlyReason: string;
	cosmRegistry: Registry;
}
let defaultProvider: MaybeSelectedProvider = null;
let defaultGasPrice = GasPrice.fromString("0.1usei");
/**
 * The main class used to interact with the Sei network, optionally associated with a wallet.
 * 
 * This provides various abstractions for interacting with different assets usable on Sei, along with the ability to
 * interact with both EVM and WASM contracts.
 */
export class ClientEnv {
	protected signer: OfflineSigner | null;
	protected cometClient: CometClient;
	/** User's account data. Will be `null` if this isn't associated with an account */
	readonly account: SeiClientAccountData | null;
	/** The Sei network ID this client is connected to */
	readonly chainId: SeiChainId;
	/** If this is a read-only account, this says why. */
	readonly readonlyReason: string;
	/**
	 * A `SigningStargateClient` if this `ClientEnv` is used with an account which can sign transactions, or a
	 * `StargateClient` otherwise. Used for general network information and optionally signing things.
	 */
	readonly stargateClient: SigningStargateClient | StargateClient;
	/**
	 * The underlying query client, used to query the specific modules on Sei.
	 */
	readonly queryClient: SeiQueryClient;
	/**
	 * The `stargateClient` needs this to encode things properly.
	 */
	readonly cosmRegistry: Registry;
	/**
	 * If an ethereum wallet is connected, it will be exposed here.
	 */
	readonly ethereumClient: EthereumProvider | null;

	/**
	 * Returns the current default provider
	 * @returns The default provider
	 */
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
					// Don't care for now
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
	/**
	 * Sets the specified provider as the default one when {@link ClientEnv.get} is called.
	 * @param provider the provider to use.
	 * @param dontThrowOnFail if false, this will throw an error on failure
	 */
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
					// Don't care for now
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
	private static async connectEthProvider(
		provider: EthereumProvider,
		chainId: SeiChainId,
		expectedAddress?: string
	): Promise<string> {
		const chainConfig =  getNetworkConfig(chainId)!;
		const evmChainIdString = "0x" + chainConfig.evmChainId.toString(16)
		try {
			if (await provider.request({
				method: "eth_chainId",
				params: []
			}) != evmChainIdString) {
				await provider.request(
					{method: "wallet_switchEthereumChain", params: [{chainId: evmChainIdString}]}
				);
			}
		}catch(ex: any) {
			if (ex.code != 4902) {
				throw ex;
			}
			await provider.request({
				method: "wallet_addEthereumChain",
				params: [{
					chainId: evmChainIdString,
					chainName: "Sei (" + chainConfig.chainId + ")",
					rpcUrls: [chainConfig.evmUrl],
					iconUrls: [
						"https://www.crownfi.io/assets/coins/sei.svg"
					],
					nativeCurrency: {
						name: "Sei",
						symbol: "SEI",
						decimals: 18
					},
					blockExplorerUrls: ["https://seitrace.com/"]
				}]
			})
			await provider.request({
				method: "wallet_switchEthereumChain",
				params: [{chainId: evmChainIdString}]
			});
		}
		// Metamask tells you all the accounts connected, but puts the current user-selected one on the top of the list.
		const userEvmAddress = toChecksumAddressEvm(
			(await provider.request({
				method: "eth_requestAccounts",
				params: []
			}))[0]
		);
		if (expectedAddress && userEvmAddress != expectedAddress) {
			throw new EvmAddressValidationMismatchError(expectedAddress, userEvmAddress);
		}
		return userEvmAddress;
	}
	private static async getSigner(
		queryClient: SeiQueryClient,
		provider: MaybeSelectedProvider,
		chainId: SeiChainId
	): Promise<
		{
			cosmosSigner: OfflineSigner
			ethereumProvider: EthereumProvider | null
		} |
		{ ethereumOnlyProvider: {
			provider: EthereumProvider,
			accountData: SeiClientAccountData
		} } |
		{ failure: string } |
		{ address: string }
	> {
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
					cosmosSigner: await restoreWallet(provider.seed, provider.index, provider.cointype),
					ethereumProvider: null
				};
			}
			return {
				address: provider.address,
			};
		}
		if (provider == "ethereum") {
			if (window.ethereum == undefined) {
				return {
					failure: "No ethereum provider found"
				};
			}
			const evmAddress = await this.connectEthProvider(window.ethereum, chainId);
			let accountData = await getSeiClientAccountDataFromNetwork(queryClient, evmAddress);
			if (accountData == null || accountData.pubkey == null) {
				const msgToSign = Buffer.from("A signature is required to get your Sei-native address");
				const sig = Buffer.from(
					(await window.ethereum.request(
						{
							method: "personal_sign",
							params: [
								"0x" + msgToSign.toString("hex"),
								evmAddress
							]
						}
					)).substring(2),
					"hex"
				);
				// Metamask still seems to use this offset for message signing, and yet it's discouraged for txs?
				// Idk, but if pubkey recovery fails, this is probably why.
				sig[64] -= 27;
				const pubkey = Secp256k1.recoverPubkey(
					ExtendedSecp256k1Signature.fromFixedLength(sig),
					keccak256(
						Buffer.concat([
							Buffer.from("\x19Ethereum Signed Message:\n" + msgToSign.length, "ascii"),
							msgToSign
						])
					)
				);
				const derivedEvmPubkey = getEvmAddressFromPubkey(pubkey);
				if (getEvmAddressFromPubkey(pubkey) != evmAddress) {
					throw new EvmAddressValidationMismatchError(evmAddress, derivedEvmPubkey);
				}
				accountData = {
					evmAddress,
					seiAddress: getAddressStringFromPubKey(pubkey),
					pubkey
				};
				addSeiClientAccountDataToCache(accountData);
			}
			return {
				ethereumOnlyProvider: {
					provider: window.ethereum,
					accountData
				}
			}
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
			let ethereumProvider = KNOWN_SEI_PROVIDER_INFO[provider].providesEvm ? (
				window.ethereum ?? null
			) : null;
			if (ethereumProvider != null && provider == "compass") {
				ethereumProvider = new StupidCompassWalletWorkaround(ethereumProvider);
			}
			return {
				cosmosSigner: signer,
				ethereumProvider
			};
		} catch (ex: any) {
			return {
				failure: KNOWN_SEI_PROVIDER_INFO[provider].name + ' says "' + ex.name + ": " + ex.message + '"',
			};
		}
	}

	/**
	 * Don't be afraid of the type definition, just use `ClientEnv.get()`
	 * 
	 * This gets a `ClientEnv` from the provider specified.
	 * 
	 * @param provider The provider to use
	 * @param chainId The network ID to connect to
	 * @param gasPrice gas price
	 * @returns 
	 */
	static async get<T extends typeof ClientEnv>(
		this: T,
		provider: MaybeSelectedProvider = defaultProvider,
		chainId: SeiChainId = getDefaultNetworkConfig().chainId,
		gasPrice: GasPrice = defaultGasPrice
	): Promise<InstanceType<T>> {
		const cometClient = await getCometClient(chainId);
		const queryClient = await getRpcQueryClient(cometClient);
		const [
			stargateClient,
			ethereumClient,
			signer,
			account,
			readonlyReason
		] = await (async () => {
			const maybeSigner = await ClientEnv.getSigner(queryClient, provider, chainId);
			if ("failure" in maybeSigner) {
				return [await getStargateClient(cometClient), null, null, null, maybeSigner.failure];
			} else if ("address" in maybeSigner) {
				const accountData = await getSeiClientAccountDataFromNetwork(queryClient, maybeSigner.address);
				if (accountData == null) {
					return [
						await getStargateClient(cometClient),
						null,
						null,
						null,
						maybeSigner.address + " is an invalid address or doesn't have transaction history"
					];
				} else {
					return [
						await getStargateClient(cometClient),
						null,
						null,
						accountData,
						""
					];
				}
			} else if ("ethereumOnlyProvider" in maybeSigner) {
				const { ethereumOnlyProvider } = maybeSigner;
				return [
					await getStargateClient(cometClient),
					ethereumOnlyProvider.provider,
					null,
					ethereumOnlyProvider.accountData,
					"This operation requires a Sei-native wallet"
				];
			}
			const { cosmosSigner, ethereumProvider } = maybeSigner;
			const accounts = await cosmosSigner.getAccounts();
			if (accounts.length !== 1) {
				return [
					await getStargateClient(cometClient),
					ethereumProvider,
					null,
					null,
					"Expected wallet to expose exactly 1 account but got " + accounts.length + " accounts",
				];
			}
			if (accounts[0].algo != "secp256k1") {
				// The "real" reason is that we can only derive EVM compatible addresses from secp256k1 public keys
				return [
					await getStargateClient(cometClient),
					ethereumProvider,
					null,
					null,
					"An account was received which does not use the \"secp256k1\" signing algorithm. " +
						"This effectively makes the account incompatible with the Sei network."
				];
			}
			const evmAddress = getEvmAddressFromPubkey(accounts[0].pubkey);
			if (ethereumProvider != null) {
				await ClientEnv.connectEthProvider(ethereumProvider, chainId, evmAddress);
			}
			return [
				await getSigningClient(
					cometClient,
					cosmosSigner,
					{
						gasPrice
					}
				),
				ethereumProvider,
				cosmosSigner,
				{
					seiAddress: accounts[0].address,
					pubkey: accounts[0].pubkey,
					evmAddress
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
			ethereumClient,
			readonlyReason,
			// In order to facilitate the simulation of read-only wallets, we need the registry to encode the simulated transaction
			cosmRegistry: createSeiRegistry()
		}) as InstanceType<T>;
	}
	/**
	 * use of the constructor is discouraged and isn't guaranteed to be stable. Use the get() function instead.
	 */
	constructor({
		account,
		chainId,
		cometClient,
		signer,
		stargateClient,
		queryClient,
		ethereumClient,
		readonlyReason,
		cosmRegistry
	}: ClientEnvConstruct) {
		this.account = account;
		this.chainId = chainId;
		this.cometClient = cometClient;
		this.signer = signer;
		this.stargateClient = stargateClient;
		this.queryClient = queryClient;
		this.ethereumClient = ethereumClient;
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
	 * @returns true if cosmos tranasction signing is available and the wallet is known
	 */
	isSignable(): this is {
		signer: OfflineSigner,
		stargateClient: SigningStargateClient;
		account: SeiClientAccountData
	} {
		return this.signer != null && this.stargateClient instanceof SigningStargateClient && this.account != null;
	}

	/**
	 * @returns true if cosmos tranasction signing and ethereum transaction signing is available
	 */
	isSignableAndEthereum(): this is {
		ethereumClient: EthereumProvider,
		signer: OfflineSigner,
		stargateClient: SigningStargateClient;
		account: SeiClientAccountData
	} {
		return this.ethereumClient != null && this.isSignable();	
	}

	/**
	 * @returns true if ethereum transaction signing is available
	 */
	hasEthereum(): this is {ethereumClient: EthereumProvider} {
		return this.ethereumClient != null;
	}

	/**
	 * @returns true if ethereum transaction signing is available, and cosmos tranasction signing is **not** available
	 */
	isEthereumOnly(): this is {
		ethereumClient: EthereumProvider,
		signer: null,
		stargateClient: StargateClient;
	} {
		return this.ethereumClient != null && !(this.stargateClient instanceof SigningStargateClient); 
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
	 * Waits for the specified EVM transaction hash to confirm
	 * @param tx the transcation hash
	 * @param timeoutMs how long to wait until timing out. Defaults to 60 seconds
	 * @param throwOnTimeout whether or not to throw an error if the timeout time has elapsed instead of returning null
	 * @returns the confirmed transaction, or null if we waited too long and `throwOnTimeout` is falsy
	 */
	async waitForEvmTxConfirm(tx: string, timeoutMs?: number, throwOnTimeout?: boolean): Promise<ReceiptInformation | null>;
	/**
	 * Waits for the specified EVM transaction hash to confirm
	 * @param tx the transcation hash
	 * @param timeoutMs how long to wait until timing out. Defaults to 60 seconds if undefined
	 * @param throwOnTimeout you explicitly set this to `true`, so prepare for error throwing
	 * @returns the confirmed transaction
	 */
	async waitForEvmTxConfirm(tx: string, timeoutMs: number | undefined, throwOnTimeout: true): Promise<ReceiptInformation>;
	async waitForEvmTxConfirm(
		tx: string,
		timeoutMs: number = DEFAULT_TX_TIMEOUT_MS,
		throwOnTimeout?: boolean
	): Promise<ReceiptInformation | null> {
		if (!this.hasEthereum()) {
			throw new Error(
				"Cannot await the confirmation of an EVM transaction while an Ethereum provider isn't available"
			);
		}
		// More stuff that cosmjs implements internally that doesn't get exposed to us
		let result: ReceiptInformation | null = null;
		const startTime = Date.now();

		while (result == null && Date.now() - startTime < timeoutMs) {
			await new Promise((resolve) => {
				setTimeout(resolve, 200 + Math.random() * 300);
			});
			result = await this.ethereumClient.request({method: "eth_getTransactionReceipt", params: [tx]});
		}
		if (result == null && throwOnTimeout) {
			throw new TimeoutError(
				"Transaction " +
					tx +
					" wasn't confirmed within " +
					timeoutMs / 1000 +
					" seconds. " +
					"You may want to check this again later.",
				tx
			);
		}
		return result;
	}

	/**
	 * Waits for the specified cosmos transaction to confirm
	 * @param tx the transcation hash
	 * @param timeoutMs how long to wait until timing out. Defaults to 60 seconds
	 * @param throwOnTimeout whether or not to throw an error if the timeout time has elapsed instead of returning null
	 * @returns the confirmed transaction, or null if we waited too long and `throwOnTimeout` is falsy
	 */
	async waitForTxConfirm(tx: string, timeoutMs?: number, throwOnTimeout?: boolean): Promise<DeliverTxResponse | null>;
	/**
	 * Waits for the specified cosmos transaction to confirm
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
					"You may want to check this again later.",
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
	evmSignAndSend(
		msg: EvmTransaction
	): Promise<ReceiptInformation>;
	evmSignAndSend(
		msg: EvmTransaction,
		finality: "broadcasted"
	): Promise<string>;
	evmSignAndSend(
		msg: EvmTransaction,
		finality?: { confirmed: { timeoutMs?: number } }
	): Promise<ReceiptInformation>;
	evmSignAndSend(
		msg: EvmTransaction,
		finality?: TransactionFinality
	): Promise<ReceiptInformation | string>;
	async evmSignAndSend(
		msg: EvmTransaction,
		finality: TransactionFinality = { confirmed: {} }
	): Promise<ReceiptInformation | string> {
		if (!this.hasAccount()) {
			throw new ClientNotSignableError("Cannot execute transactions - " + this.readonlyReason);
		}
		if (!this.hasEthereum()) {
			throw new ClientNotSignableError("No ethereum-capable wallet connected for EVM transaction");
		}
		if (!msg.gas) {
			msg.gas = await this.ethereumClient.request({method: "eth_estimateGas", params: [msg, "latest"]});
		}
		// The sei network needs this
		if (!msg.gasPrice) {
			msg.gasPrice = await this.ethereumClient.request({method: "eth_gasPrice", params: []});
		}
		const transactionHash = await this.ethereumClient.request({method: "eth_sendTransaction", params: [msg]});
		// TODO: events?
		if (finality == "broadcasted") {
			return transactionHash;
		}
		const {
			confirmed: { timeoutMs = DEFAULT_TX_TIMEOUT_MS },
		} = finality;
		await this.waitForEvmTxConfirm(transactionHash, timeoutMs, true);
		return transactionHash;
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
		try {
			const result = await this.waitForTxConfirm(transactionHash, timeoutMs, true);
			seiUtilEventEmitter.emit("transactionConfirmed", {
				chainId: this.chainId,
				sender: this.account.seiAddress,
				result,
			});
			return result;
		}catch(ex: any) {
			if (ex instanceof TimeoutError) {
				seiUtilEventEmitter.emit("transactionTimeout", {
					chainId: this.chainId,
					sender: this.account.seiAddress,
					transactionHash,
				});
			}
			throw ex;
		}
	}

	/**
	 * Executes a single contract instrcution
	 * 
	 * ⚠️ This currently cannot be used to execute EVM contracts, as the transaction will be signed with a cosmos
	 * signature, and Sei currently requires all top-level EVM invocations to have EVM signatures.
	 * 
	 * @param instruction The contract instruction to execute.
	 * @param memo Optional transaction memo
	 * @param fee The fee to pay, defaults to "auto"
	 * @param finality if `"broadcasted"`, this function will resolve whenever the transaction is sent to the chain.
	 * If `{confirmed: {}}`, then the function will resolve when the transaction is fully processed. This defaults to
	 * `{confirmed: { timeoutMs: 60000 }}`
	 * @returns the transaction hash or its receipt, depending on how `finality` was specified
	 */
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

	/**
	 * Executes the specified contract instructions
	 * 
	 * ⚠️ This currently cannot be used to execute EVM contracts, as the transaction will be signed with a cosmos
	 * signature, and Sei currently requires all top-level EVM invocations to have EVM signatures.
	 * 
	 * @param instruction The contract instruction to execute.
	 * @param memo Optional transaction memo
	 * @param fee The fee to pay, defaults to "auto"
	 * @param finality if `"broadcasted"`, this function will resolve whenever the transaction is sent to the chain.
	 * If `{confirmed: {}}`, then the function will resolve when the transaction is fully processed. This defaults to
	 * `{confirmed: { timeoutMs: 60000 }}`
	 * @returns the transaction hash or its receipt, depending on how `finality` was specified
	 */
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
	/**
	 * Makes a read-only call to a solidity contract
	 * @param contractAddress The `0x...` contract address
	 * @param functionDefinition Function to call with its return types
	 * @param params Function arguments
	 * @returns the returned values as an array
	 */
	queryEvmContract(
		contractAddress: string,
		functionDefinition: EVMABIFunctionDefinition | string,
		params: any[]
	): Promise<any[]> {
		return queryEvmContract(this.queryClient, contractAddress, functionDefinition, params);
	}
	/**
	 * Makes a read-only call to a solidity contract
	 * @param contractAddress The `0x...` contract address
	 * @param functionDefinition Function to call with its return types
	 * @param params Function arguments
	 * @returns the returned values as a struct
	 */
	async queryEvmContractForObject(
		contractAddress: string,
		functionDefinition: EVMABIFunctionDefinition | string,
		params: any[]
	): Promise<any> {
		return queryEvmContractForObject(this.queryClient, contractAddress, functionDefinition, params);
	}
	/**
	 * Gets the balance of the specified fungible asset
	 * 
	 * @param unifiedDenom the fungible to get the balance of
	 * @param accountAddress account address, can be a `0x` or `sei1` address (if they're associated) defaults to this
	 * user's `0x` or `sei1` address depending on whether which asset is specified.
	 * @returns the raw value. If you wish to make it look pretty, use it with {@link UIAmount}.
	 */
	async getBalance(unifiedDenom: UnifiedDenom, accountAddress?: string): Promise<bigint> {
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
			// Quick hack to deal with user accounts not associated yet.
			if (
				result.denom == "usei" &&
				accountAddress == this.account?.seiAddress &&
				this.hasEthereum() &&
				!(await this.isAssociated())
			) {
				return BigInt(result.amount) + (
					BigInt(await this.ethereumClient.request(
						{method: "eth_getBalance", params: [this.account.evmAddress, "latest"]}
					)) / 1000000000000n
				);
			}
			return BigInt(result.amount);
		}
	}
	/**
	 * Encodes the specified contract instructions into an array of cosmos messages
	 * 
	 * ⚠️ This uses a placeholder `typeUrl` for EVM invocations, as at the time of writing, Sei does not support
	 * calling the EVM in a top-level cosmos message without an EVM signature. Though the resulting `EncodeObject[]`
	 * may still be used with {@link cosmosMessagesToEvmMessages} and {@link makeHackyTransactionSequenceAsNeeded}
	 * 
	 * @param instructions contract instructions to encode
	 * @returns 
	 */
	execIxsToCosmosMsgs(instructions: EvmOrWasmExecuteInstruction[]): EncodeObject[] {
		if (!this.hasAccount()) {
			throw new ClientAccountMissingError("Can't get user wallet address - " + this.readonlyReason);
		}
		return instructions.map((i) => {
			if ("evmMsg" in i) {
				return {
					typeUrl: "/seiprotocol.seichain.evm.MsgInternalEVMCall",
					value: {
						// string, sei* address
						sender: this.account.seiAddress,
						// string, 0x* address
						to: i.contractAddress,
						// uint8array
						data: encodeEvmFuncCall(i.evmMsg.function, i.evmMsg.params),
						// string (base 10 bigint in usei, i.e. 1e-6)
						funds: (i.evmMsg.funds || 0n).toString()
					}
				};
			}
			if (i.funds) {
				i.funds = (i.funds as Coin[]).sort(nativeDenomSortCompare);
			}
			return {
				typeUrl: "/cosmwasm.wasm.v1.MsgExecuteContract",
				value: {
					sender: this.account.seiAddress,
					contract: i.contractAddress,
					msg: i.msg instanceof Uint8Array ? i.msg : Buffer.from(JSON.stringify(i.msg)),
					funds: [...(i.funds || [])]
				},
			};
		});
	}
	/**
	 * A key compontent in compensating for Sei's idiosyncracies in its CosmWasm <> EVM interop.
	 * 
	 * Idiosyncracies being:
	 * - Sei requires top-level EVM invoke instructions to be ...
	 *     - the _one and only_ thing that occupies the transaction.
	 *     - signed by an Ethereum wallet. There's no way to invoke the EVM on the top level using a cosmos signature.
	 * 
	 * Additionally, ethereum wallets cannot bundle multiple instructions per transaction.
	 * 
	 * To help with this, this function takes the `EncodeObject[]` provided, which may include an internal `typeUrl`
	 * which reprensts EVM invocations, and seperates them into EVM messages or Cosmos messages as needed.
	 * 
	 * - If this represents an Ethereum-only client
	 *   - All messages will be converted to EVM messages using {@link cosmosMessagesToEvmMessages}
	 * - If this represents a cosmos client
	 *   - Cosmos messages will be returned as-is, with any interspersed EVM messages returned as EVM messages
	 */
	async makeHackyTransactionSequenceAsNeeded(
		msgs: EncodeObject[]
	): Promise<({evmMsg: EvmTransaction} | {cosmMsg: EncodeObject[]})[]> {
		if (this.isEthereumOnly()) {
			return (
				await cosmosMessagesToEvmMessages(this.queryClient, msgs, this.account?.evmAddress)
			).map(evmMsg => {return {evmMsg}})
		}
		const result: ({evmMsg: EvmTransaction} | {cosmMsg: EncodeObject[]})[] = [];
		// Ideally there would be a top-level CosmEvm call so we wouldn't have to do this, but...
		while (true) {
			const evmCallIndex = msgs.findIndex(v => v.typeUrl == "/seiprotocol.seichain.evm.MsgInternalEVMCall");
			if (evmCallIndex == -1) {
				result.push({cosmMsg: msgs});
				break;
			}
			const cosmEvmMsg = msgs[evmCallIndex];
			result.push({evmMsg: (await cosmosMessagesToEvmMessages(this.queryClient, [cosmEvmMsg], this.account?.evmAddress))[0]});
			msgs = msgs.slice(evmCallIndex + 1);
		}
		return result;
	}
	/**
	 * Gets the total supply of the specified fungible asset.
	 * @param unifiedDenom the asset to look up
	 * @returns the raw value. If you wish to make it look pretty, use it with {@link UIAmount}.
	 */
	async getSupplyOf(unifiedDenom: UnifiedDenom): Promise<bigint> {
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
	/**
	 * Checks whether or not this account is associated. (The node is aware of the relationship between the 0x and sei1
	 * accounts)
	 * 
	 * Always returns `false` if this `ClientEnv` isn't associated with an account.
	 */
	async isAssociated(): Promise<boolean> {
		if (this.#knownToBeAssociated) {
			return true;
		}
		if (this.account == null) {
			return false;
		}
		return (await this.queryClient.evm.eVMAddressBySeiAddress({seiAddress: this.account.seiAddress})).associated;
	}
	#knownToBeAssociated = false;
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
