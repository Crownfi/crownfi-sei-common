import { ClientEnv } from "./client_env";
import { seiUtilEventEmitter } from "./events";

export declare const KNOWN_SEI_NETWORKS: readonly ["sei-chain", "atlantic-2", "pacific-1"];
export type KnownSeiChainId = typeof KNOWN_SEI_NETWORKS[number];
export type SeiChainId = string | KnownSeiChainId;

export type SeiChainNetConfig<C extends string = SeiChainId> = {
	chainId: C,
	rpcUrl: string,
	restUrl: string
};

const seiNetConfigs: {[chainId: SeiChainId]: SeiChainNetConfig<SeiChainId>} = {
	"sei-chain": {
		chainId: "sei-chain",
		rpcUrl: "http://127.0.0.1:26657",
		restUrl: "http://127.0.0.1:1317",
	},
	"atlantic-2": {
		chainId: "atlantic-2",
		rpcUrl: "https://rpc.atlantic-2.seinetwork.io/",
		restUrl: "https://rest.atlantic-2.seinetwork.io/"
	},
	// This is temporary until we set up our own
	"pacific-1": {
		chainId: "pacific-1",
		rpcUrl: "https://sei-rpc.polkachu.com/",
		restUrl: "https://sei-api.polkachu.com/"
	}
}

/**
 * Adds the specified network config so that it can be selected with `setDefaultNetwork`.
 * This can also be used to change the endpoints for a public chain.
 * @param configs 
 */
export function setNetworkConfig(...configs: SeiChainNetConfig[]) {
	for (let i = 0; i < configs.length; i += 1) {
		seiNetConfigs[configs[i].chainId] = configs[i]
	}
}

let defaultNetwork = "pacific-1";
/**
 * Sets the default network.
 * Note that changing the default network will change the default sei provider to null.
 * @param network the default network to set to
 */
export function setDefaultNetwork(network: SeiChainId) {
	if (seiNetConfigs[network] == null) {
		throw new Error("Cannot set the default network to that which has no endpoint configuration");
	}
	const oldNetwork = defaultNetwork;
	defaultNetwork = network;
	if (defaultNetwork != oldNetwork) {
		seiUtilEventEmitter.emit("defaultNetworkChanged", getDefaultNetworkConfig());
		ClientEnv.nullifyDefaultProvider();
	}
}

export function getNetworkConfig<C extends SeiChainId>(network: C) : SeiChainNetConfig<C> | null {
	const result = seiNetConfigs[network];
	if (result == null) {
		return null;
	}
	return result as SeiChainNetConfig<C>;
}

export function getDefaultNetworkConfig(): SeiChainNetConfig {
	// This should never be undefined since setDefaultNetwork checks the default network
	return seiNetConfigs[defaultNetwork];
}
