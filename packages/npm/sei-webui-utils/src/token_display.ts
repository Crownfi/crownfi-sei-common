import { applyCustomElementsWorkaround } from "@aritz-cracker/browser-utils";
import { getRpcQueryClient } from "@crownfi/sei-js-core";
import { addUserTokenInfo, getUserTokenInfo, bigIntToStringDecimal, hasUserTokenInfo, getDefaultNetworkConfig, getCometClient } from "@crownfi/sei-utils";
await applyCustomElementsWorkaround();

function tryParseBigInt(input: string): bigint | null {
	try {
		return BigInt(input);
	}catch(ex: any) {
		return null;
	}
}

/**
 * An element which shows a token amount, represented as `<span is="token-display">` in the document.
 * 
 * This element has the following custom HTML attributes
 * 
 * * `denom`: See the {@link TokenDisplayElement.denom | `denom` property}
 * * `amount`: See the {@link TokenDisplayElement.amount | `amount` property}
 * * `full-name`: See the {@link TokenDisplayElement.fullName | `fullName` property}
 * * `trailing-zeros`: See the {@link TokenDisplayElement.trailingZeros | `trailingZeros` property}
 * * `no-logo`: See the {@link TokenDisplayElement.noLogo | `noLogo` property}
 */
export class TokenDisplayElement extends HTMLSpanElement {
	constructor() {
		super();
		this.#elemAmount = document.createElement("span");
		this.#elemAmount.classList.add("token-display-amount");
		this.#elemLogo = document.createElement("img");
		this.#elemLogo.classList.add("loading-spinner");
		this.#elemLogo.style.height = "1em";
		this.#elemLogo.addEventListener("load", () => {
			this.#elemLogo.classList.remove("loading-spinner");
		});
		this.#elemLogo.addEventListener("error", () => {
			this.#elemLogo.src = "https://www.crownfi.io/assets/lazy_load_crown.svg#danger";
		});
		this.#elemName = document.createElement("span");
		this.#elemName.classList.add("token-display-name");


	}
	static get observedAttributes() {
		return ["denom", "amount", "full-name", "trailing-zeros", "no-logo"];
	}
	/**
	 * Reflects the value of the `denom` attribute.
	 * 
	 * This value is passed to {@link addUserTokenInfo} and {@link getUserTokenInfo} in order to determine how to
	 * display the asset depending on the value of the {@link TokenDisplayElement.fullName | `fullName` property}
	 * property.
	 */
	get denom() {
		return this.getAttribute("denom");
	}
	set denom(value: string | null) {
		if (value == null) {
			this.removeAttribute("denom");
		} else {
			this.setAttribute("denom", value);
		}
	}
	/**
	 * Reflects the value of the `amount` attribute.
	 * 
	 * * If `null` or `""`, it is not used.
	 * * If this value cannot be parsed into an integer, then the value will be shown as-is. ("0x" and "0o" prefixed
	 *   numbers count as valid integers)
	 * * If the value is an integer, then {@link getUserTokenInfo} is called with the
	 *   {@link TokenDisplayElement.denom | `denom` property} and the current default network, and the resulting
	 *   `decimals` value is used with {@link bigIntToStringDecimal} to display the value.
	 */
	get amount() {
		return this.getAttribute("amount");
	}
	set amount(value: string | null) {
		if (value == null) {
			this.removeAttribute("amount");
		} else {
			this.setAttribute("amount", value);
		}
	}
	#fullName: boolean = false;
	/**
	 * Is `true` if the `full-name` attribute exists, including if it's set to an empty string. `false` otherwise.
	 * 
	 * If `true`, then the asset name will show as its full name, e.g. Bitcoin. If `false`, then the asset name will
	 * show as its symbol, e.g. BTC.
	 */
	get fullName() {
		return this.#fullName;
	}
	set fullName(value: boolean) {
		if (value) {
			this.setAttribute("full-name", "");
		} else {
			this.removeAttribute("full-name");
		}
	}
	#trailingZeros: boolean = false;
	/**
	 * Is `true` if the `trailing-zeros` attribute exists, including if it's set to an empty string. `false` otherwise.
	 * 
	 * If `true`, then the asset will always show the amount of decimals possible, e.g. 1.500000000000000000 ETH. If
	 * `false`, the trailing zeros aren't displayed, e.g. 1.5 ETH.
	 */
	get trailingZeros() {
		return this.#trailingZeros;
	}
	set trailingZeros(value: boolean) {
		if (value) {
			this.setAttribute("trailing-zeros", "");
		} else {
			this.removeAttribute("trailing-zeros");
		}
	}
	#noLogo: boolean = false;
	/**
	 * Is `true` if the `no-logo` attribute exists, including if it's set to an empty string. `false` otherwise.
	 * 
	 * If `false`, then the token's logo will be shown. If `true`, then the token's logo will be hidden.
	 */
	get noLogo() {
		return this.#noLogo;
	}
	set noLogo(value: boolean) {
		if (value) {
			this.setAttribute("no-logo", "");
		} else {
			this.removeAttribute("no-logo");
		}
	}
	
	#elemAmount: HTMLSpanElement;
	#elemLogo: HTMLImageElement;
	#elemName: HTMLSpanElement;
	#triedUserTokenInfo: string = "";
	#refreshDisplay() {
		if (
			this.childElementCount != 3 ||
			this.children[0] != this.#elemAmount ||
			this.children[1] != this.#elemLogo ||
			this.children[2] != this.#elemName
		) {
			this.innerHTML = "";
			this.appendChild(this.#elemAmount);
			this.appendChild(this.#elemLogo);
			this.appendChild(this.#elemName);
		}

		const denom = this.denom;
		const chainId = getDefaultNetworkConfig().chainId;
		if (denom) {
			if (hasUserTokenInfo(denom, chainId) || this.#triedUserTokenInfo == denom) {
				const tokenInfo = getUserTokenInfo(denom, chainId);
				this.#elemLogo.hidden = this.noLogo;
				if (!this.noLogo) {
					this.#elemLogo.src = tokenInfo.icon;
				}
				if (!this.amount) {
					this.#elemAmount.innerText = "";
					this.#elemAmount.hidden = true;
				} else {
					const amountAsInt = tryParseBigInt(this.amount);
					if (amountAsInt == null) {
						this.#elemAmount.innerText = this.amount + "\xa0";
					} else {
						this.#elemAmount.innerText = bigIntToStringDecimal(
							amountAsInt,
							tokenInfo.decimals,
							!this.trailingZeros
						) + "\xa0";
					}
				}
				this.#elemAmount.hidden = false;
				if (this.fullName) {
					this.#elemName.innerText = tokenInfo.name;
				} else {
					this.#elemName.innerText = tokenInfo.symbol;
				}
				if (!this.#elemLogo.hidden) {
					this.#elemName.innerHTML = "&nbsp;" + this.#elemName.innerHTML;
				}
				this.#elemName.hidden = false;
			} else {
				(async () => {
					try {
						this.#elemAmount.classList.add("loading-spinner-inline");
						this.#elemAmount.innerText = this.amount || "";
						this.#elemAmount.hidden = false;
						this.#elemLogo.hidden = true;
						this.#elemName.hidden = true;
						await addUserTokenInfo(
							await getRpcQueryClient(await getCometClient(chainId)),
							chainId,
							denom
						);
					} catch(ex: any) {
						console.warn("TokenDisplayElement: Could not get token info for " + denom + ":", ex);
					} finally {
						this.#triedUserTokenInfo = denom;
						this.#elemAmount.classList.remove("loading-spinner-inline");
						this.#refreshDisplay();
					}
				})();
				
			}
		} else {
			this.#elemAmount.innerText = this.amount + "";
			this.#elemAmount.hidden = false;
			this.#elemLogo.hidden = true;
			this.#elemName.hidden = true;
		}
		//this.childNodes.
	}
	/**
	 * @internal
	 */
	attributeChangedCallback(name: string, _: string | null, newValue: string | null) {
		switch(name) {
			case "full-name":
				this.#fullName = newValue != null;
				break;
			case "trailing-zeros":
				this.#trailingZeros = newValue != null;
				break;
			case "no-logo":
				this.#noLogo = newValue != null;
				break;
			default:
				// no default
		}
		this.#refreshDisplay();
	}
}
customElements.define("token-display", TokenDisplayElement, {extends: "span"});
