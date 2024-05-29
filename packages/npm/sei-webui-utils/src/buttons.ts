import { qa } from "@aritz-cracker/browser-utils";
import { seiUtilEventEmitter } from "@crownfi/sei-utils";
import { NetworkSelectionType } from "./modals.js";
// pacific-1

export class WalletOptionButtonElement extends HTMLButtonElement {
	static get observedAttributes() {
		return ["network-selection", "default-network"];
	}
	#networkSelection: NetworkSelectionType = "url";
	get networkSelection(): string | null {
		return this.#networkSelection;
	}
	set networkSelection(v: string | null) {
		if (v == null) {
			this.removeAttribute("network-selection");
		}else{
			this.setAttribute("network-selection", v);
		}
	}
	#hasAppliedDefaultNetwork = false;
	attributeChangedCallback(name: string, oldValue: string | null, newValue: string | null) {
		switch(name) {
			case "network-selection":
				if (newValue == "dropdown") {
					this.#networkSelection = "dropdown";
				} else {
					this.#networkSelection = "url";
				}
				break;
			case "default-network": {

			}
			default:
				// Shouldn't happen
		}
	}
	constructor() {
		super();
		
	}
	connectedCallback() {
		
	}
	disconnectedCallback() {
		
	}
	adoptedCallback() {
		
	}
}
customElements.define("wallet-options", WalletOptionButtonElement, { extends: "button"});

seiUtilEventEmitter.on("defaultProviderChanged", (ev) => {
	/*
	(qa(`button[is="wallet-options"]`) as NodeListOf<WalletOptionButtonElement>).forEach(elem => {
		elem.walletAddress = ev.account?.address ?? null;
	});
	*/
});
seiUtilEventEmitter.on("defaultNetworkChanged", (ev) => {

});
