import { CometClient, connectComet } from "@cosmjs/tendermint-rpc";
import { ClientEnv } from "./client_env.js";
import { seiUtilEventEmitter } from "./events.js";
import { NetworkEndpointNotConfiguredError } from "./error.js";

export declare const KNOWN_SEI_NETWORKS: readonly ["sei-chain", "arctic-1", "atlantic-2", "pacific-1"];
export type SeiChainId = (typeof KNOWN_SEI_NETWORKS)[number];

export type SeiChainNetConfig<C extends string = SeiChainId> = {
	chainId: C;
	evmChainId: number,
	rpcUrl: string;
	restUrl: string;
	evmUrl: string;
};

const seiNetConfigs = {
	"sei-chain": {
		chainId: "sei-chain" as const,
		rpcUrl: "http://127.0.0.1:26657",
		restUrl: "http://127.0.0.1:1317",
		evmChainId: 0xae3f3,
		evmUrl: "http://127.0.0.1:8545"
	},
	"arctic-1": {
		chainId: "arctic-1" as const,
		rpcUrl: "https://rpc-arctic-1.sei-apis.com/",
		restUrl: "https://rest-arctic-1.sei-apis.com/",
		evmChainId: 0xae3f3,
		evmUrl: "https://evm-rpc-arctic-1.sei-apis.com"
	},
	"atlantic-2": {
		chainId: "atlantic-2" as const,
		rpcUrl: "https://rpc.atlantic-2.seinetwork.io/",
		restUrl: "https://rest.atlantic-2.seinetwork.io/",
		evmChainId: 0x530,
		evmUrl: "https://evm-rpc-testnet.sei-apis.com/"
	},
	// This is temporary until we set up our own
	"pacific-1": {
		chainId: "pacific-1" as const,
		rpcUrl: "https://rpc.sei-apis.com",
		restUrl: "https://rest.sei-apis.com",
		evmChainId: 0x531,
		evmUrl: "https://evm-rpc.sei-apis.com/"
	},
};

const cachedCometClients: Record<SeiChainId, Promise<CometClient> | null> = {
	"arctic-1": null,
	"atlantic-2": null,
	"pacific-1": null,
	"sei-chain": null
};

/**
 * Adds the specified network config so that it can be selected with `setDefaultNetwork`.
 * This can also be used to change the endpoints for a public chain.
 * @param configs
 */
export function setNetworkConfig<C extends SeiChainId>(...configs: SeiChainNetConfig<C>[]) {
	for (let i = 0; i < configs.length; i += 1) {
		const config = configs[i];
		const chainId = config.chainId;
		seiNetConfigs[chainId] = config as any;
		cachedCometClients[chainId] = null;
	}
}

let defaultNetwork: SeiChainId = "pacific-1";

/**
 * Sets the default network.
 * Note that changing the default network will change the default sei provider to null.
 * @param network the default network to set to
 */
export function setDefaultNetwork(network: SeiChainId) {
	if (seiNetConfigs[network] == null) {
		throw new NetworkEndpointNotConfiguredError(network, defaultNetwork);
	}
	if (network != defaultNetwork) {
		ClientEnv.nullifyDefaultProvider();
		defaultNetwork = network;
		seiUtilEventEmitter.emit("defaultNetworkChanged", Object.freeze(getDefaultNetworkConfig()));
		
	}
}

export function getNetworkConfig<C extends SeiChainId>(network: C): SeiChainNetConfig<C> | null {
	const result = seiNetConfigs[network];
	if (result == null) {
		return null;
	}
	return result as SeiChainNetConfig<C>;
}

export function getDefaultNetworkConfig(): SeiChainNetConfig {
	// This should never be undefined since setDefaultNetwork checks the default network
	return {...seiNetConfigs[defaultNetwork]};
}

/**
 * Returns a cached comet client associated with the network
 * 
 * @param network 
 * @returns A comet client
 */
export async function getCometClient(network: SeiChainId): Promise<CometClient> {
	if (cachedCometClients[network] == null) {
		if (seiNetConfigs[network] == null) {
			throw new NetworkEndpointNotConfiguredError(network, defaultNetwork);
		}
		cachedCometClients[network] = connectComet(seiNetConfigs[network].rpcUrl);
	}
	return cachedCometClients[network]!;
}
