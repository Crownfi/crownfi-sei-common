import { TinyEmitter } from "tiny-emitter";
import { DeliverTxResponse, ExecuteResult } from "@cosmjs/cosmwasm-stargate"
import { Addr } from "./common_sei_types";
import { SeiChainId, SeiChainNetConfig } from "./chain_config";
import { MaybeSelectedProviderString } from "./client_env";

// Events with generics inspired from https://github.com/scottcorgan/tiny-emitter/pull/42
type Arguments<T> = [T] extends [(...args: infer U) => any] ? U : [T] extends [void] ? [] : [T];
export interface TypedTinyEmitter<T extends any = any> extends Omit<TinyEmitter, "on" | "once" | "emit" | "off"> {
	on<E extends keyof T>(event: E, callback: T[E], ctx?: any): this;
	once<E extends keyof T>(event: E, callback: T[E], ctx?: any): this;
	emit<E extends keyof T>(event: E, ...args: Arguments<T[E]>): this;
	off<E extends keyof T>(event: E, callback?: T[E]): this;
}

interface SeiUtilEvents {
	transactionConfirmed: (ev: {chainId: SeiChainId, sender: Addr, result: ExecuteResult | DeliverTxResponse}) => void
	defaultNetworkChanged: (ev: SeiChainNetConfig) => void
	defaultProviderChangeRequest: (ev: {
		provider: MaybeSelectedProviderString,
		status: "requesting" | "failure" | "success",
		failureException?: any
	}) => void
	defaultProviderChanged: (ev: {
		chainId: SeiChainId,
		account: Addr | null,
		provider: MaybeSelectedProviderString
	}) => void
}

export const seiUtilEventEmitter = new TinyEmitter() as TypedTinyEmitter<SeiUtilEvents>;
