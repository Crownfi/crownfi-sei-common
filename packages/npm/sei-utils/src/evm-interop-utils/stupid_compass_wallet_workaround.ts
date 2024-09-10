import { EthereumProvider, EthereumProviderEventMap, EthereumRpcMethodMap } from "@crownfi/ethereum-rpc-types";

/**
 * At the time of writing, compass wallet shits itself when you have multiple of the same request happening at the same
 * time.
 */
export class StupidCompassWalletWorkaround implements EthereumProvider {
	#requestCache: Map<string, Promise<any>> = new Map();
	constructor(
		public readonly inner: EthereumProvider
	) {}
	request<M extends keyof EthereumRpcMethodMap>(request: { method: M; params: EthereumRpcMethodMap[M]["params"]; }): Promise<EthereumRpcMethodMap[M]["result"]> {
		const requestKey = JSON.stringify(request);
		const maybeCachedRequest = this.#requestCache.get(requestKey);
		if (maybeCachedRequest == undefined) {
			const newCachedRequest = (async () => {
				try {
					let result = await this.inner.request(request);
					let waitTime = 1;
					while (result === undefined) {
						// Using the timeout cuz the request _synchronously_ returns undefined.
						// So it seems they had the idea to combine duplicate requests, but fucked up about it.
						await new Promise(resolve => setTimeout(resolve, waitTime))
						result = await this.inner.request(request);
						waitTime *= 2;
					}
					return result;
				} finally {
					this.#requestCache.delete(requestKey);
				}
			})();
			this.#requestCache.set(requestKey, newCachedRequest);
			return newCachedRequest;
		} else {
			return maybeCachedRequest;
		}
	}
	on<T extends keyof EthereumProviderEventMap>(eventName: T, listener: (eventData: EthereumProviderEventMap[T]) => void): typeof this {
		this.inner.on(eventName, listener);
		return this;
	}
	off<T extends keyof EthereumProviderEventMap>(eventName: T, listener: (eventData: EthereumProviderEventMap[T]) => void): typeof this {
		this.inner.off(eventName, listener);
		return this;
	}
	addListener<T extends keyof EthereumProviderEventMap>(eventName: T, listener: (eventData: EthereumProviderEventMap[T]) => void): typeof this {
		this.inner.addListener(eventName, listener);
		return this;
	}
	removeListener<T extends keyof EthereumProviderEventMap>(eventName: T, listener: (eventData: EthereumProviderEventMap[T]) => void): typeof this {
		this.inner.removeListener(eventName, listener);
		return this;
	}
}
