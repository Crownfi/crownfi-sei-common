import { Coin, GasPrice } from "@cosmjs/stargate";
import { ClientEnv, SeiChainId, getDefaultNetworkConfig, setDefaultNetwork } from "@crownfi/sei-utils";
import { spawn } from "promisify-child-process";

/**
 * Sets all the default `@crownfi/sei-utils` `ClientEnv` parameters to that which was specified by environment
 * variables.
 *
 * * `GAS_PRICE`: The gas price
 * * `MNEMONIC`: The wallet seed phrase
 * * `MNEMONIC_INDEX`: The seed phrase index
 * * `CHAIN_ID`: The chain ID, this also determines the RPC endpoint
 */
export async function applyEnvVarsToDefaultClientEnv() {
	if (process.env.CHAIN_ID) {
		setDefaultNetwork(process.env.CHAIN_ID as SeiChainId);
	} else {
		console.warn("Env var CHAIN_ID not set! Defaulting to:", getDefaultNetworkConfig().chainId);
	}
	if (process.env.GAS_PRICE) {
		ClientEnv.setDefaultGasPrice(GasPrice.fromString(process.env.GAS_PRICE));
	} else {
		console.warn("Env var GAS_PRICE not set! Defaulting to:", ClientEnv.getDefaultGasPrice().toString());
	}
	if (!process.env.MNEMONIC) {
		process.env.MNEMONIC =
			"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
		console.warn("Env var MNEMONIC not set! Defaulting to:", process.env.MNEMONIC);
	}
	await ClientEnv.setDefaultProvider({
		seed: process.env.MNEMONIC,
		index: Number(process.env.MNEMONIC_INDEX) || 0,
	});
}

type SeidKeysListEntry = {
	name: string;
	type: string;
	address: string;
	pubkey: string;
};
type SeidTxResult = {
	height: string;
	code: number;
	txhash: string;
	events: any[];
	raw_log: string;
	data: string;
	logs: any[];
	gas_used: string;
	gas_wanted: string;
};

export async function fundFromLocalKeychain(fromName: string, toClient: ClientEnv, amount: Coin | string) {
	const { stdout: listCmdOutput } = await spawn("seid", ["keys", "list", "--output", "json"], { encoding: "utf8" });
	const parsedListOutput = JSON.parse(listCmdOutput?.toString() + "") as SeidKeysListEntry[];
	const adminAddress = parsedListOutput.find((v) => v.name == fromName && v.type == "local")?.address;
	if (adminAddress == undefined) {
		throw new Error("Couldn't find \"" + fromName + '" in seid keychain');
	}
	const amountAsString = typeof amount == "string" ? amount : amount.amount + amount.denom;
	const { stdout: txCmdOutput } = await spawn(
		"seid",
		[
			"tx",
			"bank",
			"send",
			adminAddress,
			toClient.getAccount().seiAddress,
			amountAsString,
			"--yes",
			"--gas-prices",
			"1usei",
			"--output",
			"json",
		],
		{ encoding: "utf8" }
	);
	const { txhash } = JSON.parse(txCmdOutput?.toString() + "") as SeidTxResult;
	await toClient.waitForTxConfirm(txhash);
}
