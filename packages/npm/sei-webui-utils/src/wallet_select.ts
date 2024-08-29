import { applyCustomElementsWorkaround, q, qa } from "@aritz-cracker/browser-utils";
import { FullscreenLoadingTask, alert, msgBoxIfThrow, prompt } from "@crownfi/css-gothic-fantasy";
import { KNOWN_SEI_PROVIDERS, KNOWN_SEI_PROVIDER_INFO, KnownSeiProviders, SeiWallet } from "@crownfi/sei-js-core";
import { ClientEnv, MaybeSelectedProviderString, SeiChainId, SeiClientAccountData, getCometClient, getDefaultNetworkConfig, seiUtilEventEmitter, setDefaultNetwork } from "@crownfi/sei-utils";
import { SeedPhraseModalAutogen, WalletChoiceAutogen, WalletModalAutogen, WalletOptionsAutogen } from "./_autogen/wallet_select.js";
await applyCustomElementsWorkaround();
import "dropdown-menu-element"; // The element has to exist plus we need the event types
// Should this be here?
declare global {
	interface GlobalEventHandlersEventMap {
		"initialSeiConnection": CustomEvent
	}
}
let emittedFirstEvent = false;
function setNetworkFromUrlHash() {
	try {
		if (!window.location.hash.startsWith("#?")) {
			return;
		}
		const chainId = (new URLSearchParams(window.location.hash.substring(2))).get("network");
		if (chainId) {
			msgBoxIfThrow(setDefaultNetwork.bind(undefined, (chainId as SeiChainId)));
		}
	} finally {
		msgBoxIfThrow(async () => {
			const loadingScreen = new FullscreenLoadingTask();
			try {
				loadingScreen.text = "Connecting to Sei network...";
				loadingScreen.show();
				const _ = await getCometClient(getDefaultNetworkConfig().chainId);
				// We don't need to do anything with the client, getting it does a version check.
				await WalletModalElement.handleChoice(localStorage.getItem("sei_provider"), loadingScreen);
			} finally {
				loadingScreen.hide();
				if (!emittedFirstEvent) {
					emittedFirstEvent = true
				}
				document.dispatchEvent(new CustomEvent("initialSeiConnection", {bubbles: true, cancelable: false}));
			}
		});
	}
}
function getExperimentalWalletOptions(): Set<string> {
	if (!window.location.hash.startsWith("#?")) {
		return new Set();
	}
	const options = (new URLSearchParams(window.location.hash.substring(2))).get("experimental_wallets");
	if (!options) {
		return new Set();
	}
	return new Set(options.split(","));
}
function getSeedWalletOptions() {
	if (!window.location.hash.startsWith("#?"))
		return null;
	const url = new URLSearchParams(window.location.hash.substring(2));
	const seed = url.get("seed_mnemonic");
	if (!seed)
		return null;
	return {
		seed,
		index: +(url.get("seed_index") || 0),
		coinType: +(url.get("seed_coin_type") || 118),
	}
}
window.addEventListener("hashchange", (_) => {
	setNetworkFromUrlHash();
});
let currentClientAccountData: SeiClientAccountData | null = null;

/**
 * A wallet options button, represented as `<button is="wallet-options">` in the document.
 * 
 * This button sets {@link ClientEnv} and {@link WebClientEnv}'s default provider to that which the user specified.
 * 
 * May have a `"default-network"`, which is referenced on initial page load. However, if the URL specifies the network,
 * that will be used instead.
 */
export class WalletOptionsButtonElement extends WalletOptionsAutogen {
	static get observedAttributes() {
		return ["default-network"];
	}
	attributeChangedCallback(name: string, _: string | null, newValue: string | null) {
		switch(name) {
			case "default-network": {
				if (window.location.hash.startsWith("#?")) {
					// URL always takes priority, connectedCallback will handle this case.
					break;
				}
				msgBoxIfThrow(() => {
					setDefaultNetwork(newValue as SeiChainId);
				});
				break;
			}
			default:
				// Shouldn't happen
		}
	}
	#notConnected: boolean = true;
	constructor() {
		super();
		this.addEventListener("dropdownOpen", (ev) => {
			if (this.#notConnected) {
				ev.preventDefault();
				WalletModalElement.showModal();
			}
		});
		this.addEventListener("dropdownSelect", (ev) => {
			switch (ev.detail.selectedValue) {
				case "sei-address-seitrace": {
					const seiAddress = this.refs.seiAddressText.innerText;
					const seiChain = getDefaultNetworkConfig().chainId;
					window.open("https://seitrace.com/address/" + seiAddress + "?chain=" + seiChain, "_blank");
					break;
				}
				case "sei-address-seican": {
					const seiAddress = this.refs.seiAddressText.innerText;
					const seiChain = getDefaultNetworkConfig().chainId;
					window.open("https://www.seiscan.app/" + seiChain + "/accounts/" + seiAddress, "_blank");
					break;
				}
				case "sei-address-copy": {
					navigator.clipboard.writeText(this.refs.seiAddressText.innerText);
					break;
				}
				case "evm-address-seitrace": {
					const evmAddress = this.refs.evmAddressText.innerText;
					const seiChain = getDefaultNetworkConfig().chainId;
					window.open("https://seitrace.com/address/" + evmAddress + "?chain=" + seiChain, "_blank");
					break;
				}
				case "evm-address-copy": {
					navigator.clipboard.writeText(this.refs.evmAddressText.innerText);
					break;
				}
				case "switch-wallet": {
					WalletModalElement.showModal();
					break;
				}
				case "disconnect": {
					ClientEnv.nullifyDefaultProvider();
					break;
				}
				default:
					throw new Error("Unhandled dropdown menu option: " + ev.detail.selectedValue);
			}
		});
	}
	connectedCallback() {
		setNetworkFromUrlHash();
		this.updateProvider(ClientEnv.getDefaultProvider(), currentClientAccountData);
	}
	/**
	 * @internal 
	 */
	updateProvider(provider: MaybeSelectedProviderString, accountData: SeiClientAccountData | null) {
		this.#notConnected = accountData == null;
		if (accountData == null) {
			this.refs.evmAddressText.innerText = "null";
			this.refs.seiAddressText.innerText = "null";
		} else {
			this.refs.evmAddressText.innerText = accountData.evmAddress;
			this.refs.seiAddressText.innerText = accountData.seiAddress;
		}
		switch (provider) {
			case null:
				this.refs.text.innerText = "Connect wallet";
				break;
			case "ethereum":
				this.refs.text.innerText = "Ethereum wallet";
				break;
			case "read-only-address":
				this.refs.text.innerText = "Read-only account";
				break;
			case "seed-wallet":
				this.refs.text.innerText = "Built-in wallet";
				break;
			default: {
				const providerInfo = KNOWN_SEI_PROVIDER_INFO[provider];
				this.refs.text.innerText = providerInfo.name + " wallet";
			}
		}
	}
}
WalletOptionsButtonElement.registerElement();

/**
 * A wallet options button, represented as `<button is="wallet-disconnect">` in the document.
 * 
 * Clicking the button calls {@link ClientEnv.nullifyDefaultProvider}. That's it.
 */
export class WalletDisconnectButtonElement extends HTMLButtonElement {
	constructor() {
		super();
		this.addEventListener("click", (_) => {
			ClientEnv.nullifyDefaultProvider();
		})
	}
	connectedCallback() {
		this.updateProvider(ClientEnv.getDefaultProvider());
	}
	updateProvider(provider: MaybeSelectedProviderString) {
		this.style.display = provider ? "" : "none";
	}
}
customElements.define("wallet-disconnect", WalletDisconnectButtonElement, {extends: "button"});

seiUtilEventEmitter.on("defaultNetworkChanged", (ev) => {
	if (ev.chainId != "pacific-1") {
		alert(
			"Development network selected",
			"This app is connected to Sei's \"" + ev.chainId + "\" network, this is a blockchain intended for testing " +
				"and development. Nothing done on this network has any \"real\" value.\n" +
				"All functionality shown here is for for demonstration purposes only.",
			"warning",
			"warning"
		);
	}
});

seiUtilEventEmitter.on("defaultProviderChanged", (ev) => {
	currentClientAccountData = ev.account;
	if (ev.provider == null) {
		localStorage.removeItem("sei_provider");
	} else {
		localStorage.setItem("sei_provider", ev.provider);
	}
	(qa(`button[is="wallet-options"]`) as NodeListOf<WalletOptionsButtonElement>).forEach(elem => {
		elem.updateProvider(ev.provider, ev.account);
	});
	(qa(`button[is="wallet-disconnect"]`) as NodeListOf<WalletDisconnectButtonElement>).forEach(elem => {
		elem.updateProvider(ev.provider);
	});
});

/**
 * Wallet option, automatically created by the {@link WalletModalElement}
 */
export class WalletChoiceElement extends WalletChoiceAutogen {
	constructor(){
		super();
		this.refs.img.classList.add("loading-spinner");
		this.refs.img.addEventListener("load", (_) => {
			this.refs.img.classList.remove("loading-spinner");
		});
	}
	protected onIconChanged(_: string | null, newValue: string | null) {
		this.refs.img.src = newValue || "https://www.crownfi.io/assets/placeholder.svg";
	}
	protected onTextChanged(_: string | null, newValue: string | null) {
		this.refs.text.innerText = newValue + "";
		this.ariaLabel = newValue;
	}
}
WalletChoiceElement.registerElement();

/**
 * Wallet chooser, represented as `<dialog is="wallet-modal">` in the document. Usually this is created by the
 * {@link WalletOptionsButtonElement}
 */
export class WalletModalElement extends WalletModalAutogen {
	static showModal() {
		const dialog = q("dialog[is=wallet-modal]") as WalletModalElement | null;
		if (dialog == null) {
			const newDialog = new WalletModalElement();
			document.body.append(newDialog);
			newDialog.showModal();
		} else {
			dialog.showModal();
		}
	}
	static async handleChoice(
		choice?: string | null,
		loadingTask: FullscreenLoadingTask = new FullscreenLoadingTask()
	) {
		if (!choice) {
			return;
		}
		if (choice in KNOWN_SEI_PROVIDER_INFO) {
			loadingTask.text = "Connecting to " + KNOWN_SEI_PROVIDER_INFO[choice as KnownSeiProviders].name + "...";
			loadingTask.show();
			try {
				await ClientEnv.setDefaultProvider(choice as KnownSeiProviders);
			} finally {
				loadingTask.hide();
			}
			return;
		}
		if (choice == "ethereum") {
			loadingTask.text = "Connecting to Ethereum wallet...";
			loadingTask.show();
			try {
				await ClientEnv.setDefaultProvider("ethereum");
			} finally {
				loadingTask.hide();
			}
			return;
		}
		if (choice == "read-only-address") {
			const address = await prompt(
				"Enter address",
				"Please enter an address beginning with \"sei1\" or \"0x\"",
				""
			);
			if (!address) {
				return;
			}
			loadingTask.text = "Verifying address...";
			loadingTask.show();
			try {
				await ClientEnv.setDefaultProvider({address});
			} finally {
				loadingTask.hide();
			}
			return;
		}
		if (choice == "seed-wallet") {
			const seedOptions = getSeedWalletOptions();
			const walletStuffs = seedOptions ? seedOptions : await SeedPhraseModalElement.showModalAndGetValues();
			if (walletStuffs == null) {
				return;
			}
			loadingTask.text = "Recovering wallet...";
			loadingTask.show();
			try {
				await ClientEnv.setDefaultProvider(walletStuffs);
			} finally {
				loadingTask.hide();
			}
			return;
		}
		alert(
			"Invalid wallet selected",
			choice + " is not a known wallet provider",
			"warning",
			"warning"
		);
		loadingTask.hide();
	}
	constructor() {
		super();
		this.addEventListener("close", _ => {
			msgBoxIfThrow(() => {
				return WalletModalElement.handleChoice(this.returnValue);
			});
		});
	}
	refreshOptions() {
		this.returnValue = "";
		this.refs.choices.innerHTML = "";
		const experimentalWalletOptions = getExperimentalWalletOptions();
		
		const availableWallets: WalletChoiceElement[] = [];
		const unavailableWallets: WalletChoiceElement[] = [];
		const foundProviers = SeiWallet.discoveredWallets();

		let discoveredNativeSeiWallet = false;
		for(const providerId of KNOWN_SEI_PROVIDERS) {
			const providerInfo = KNOWN_SEI_PROVIDER_INFO[providerId];
			const choiceElem = new WalletChoiceElement();
			choiceElem.text = providerInfo.name;
			choiceElem.icon = providerInfo.icon;
			choiceElem.value = providerId;
			choiceElem.classList.add("primary");
			if (foundProviers[providerId]) {
				availableWallets.push(choiceElem);
				discoveredNativeSeiWallet = discoveredNativeSeiWallet || providerInfo.providesEvm;
			}else{
				choiceElem.text += "\n(Not found)";
				choiceElem.disabled = true;
				unavailableWallets.push(choiceElem);
			}
		}
		this.refs.plug.hidden = discoveredNativeSeiWallet;
		(() => {
			const choiceElem = new WalletChoiceElement();
			choiceElem.text = "Ethereum-based wallet";
			choiceElem.icon = "https://www.crownfi.io/assets/wallets/ethereum.svg";
			choiceElem.value = "ethereum";
			if (discoveredNativeSeiWallet) {
				choiceElem.text += "\nSei-native wallet found";
				choiceElem.disabled = true;
				unavailableWallets.push(choiceElem);
			} else if (window.ethereum == undefined) {
				choiceElem.text += "\n(Not found)";
				choiceElem.disabled = true;
				unavailableWallets.push(choiceElem);
			} else {
				availableWallets.push(choiceElem);
				window.ethereum.request({method: "web3_clientVersion", params: []})
					.then(name => {
						choiceElem.text += "\n" + name;
					})
					.catch(_ => {});
			}
		})();
		if (experimentalWalletOptions.has("read_only")) {
			const choiceElem = new WalletChoiceElement();
			choiceElem.text = "Enter address\nRead-only account";
			choiceElem.icon = "https://www.crownfi.io/assets/placeholder.svg";
			choiceElem.value = "read-only-address";
			availableWallets.push(choiceElem);
		}
		if (experimentalWalletOptions.has("seed_wallet")) {
			const choiceElem = new WalletChoiceElement();
			choiceElem.text = "Enter mnemonic seed\nThis is dangerous! You probably shouldn't do this!";
			choiceElem.icon = "https://www.crownfi.io/assets/placeholder.svg";
			choiceElem.value = "seed-wallet";
			availableWallets.push(choiceElem);
		}
		this.refs.choices.append(...availableWallets);
		this.refs.choices.append(...unavailableWallets);
	}
	showModal() {
		this.refreshOptions();
		super.showModal();
	}
	show() {
		this.refreshOptions();
		super.show();
	}
}
WalletModalElement.registerElement();

/**
 * Allows the user to enter a mnemonic seed, represented as `<dialog is="seed-phrase-modal">` in the document.
 * Usually this is created by the {@link WalletOptionsButtonElement}
 */
class SeedPhraseModalElement extends SeedPhraseModalAutogen {
	static showModalAndGetValues(): Promise<{ seed: string; index: number, cointype: number } | null> {
		const dialog = q("dialog[is=seed-phrase-modal]") as SeedPhraseModalElement | null;
		if (dialog == null) {
			const newDialog = new SeedPhraseModalElement();
			document.body.append(newDialog);
			return newDialog.showModalAndGetValues();
		} else {
			return dialog.showModalAndGetValues();
		}
	}
	constructor() {
		super();
		this.refs.cancelBtn.addEventListener("click", (ev) => {
			ev.preventDefault();
			this.close();
		})
	}
	showModalAndGetValues(): Promise<{ seed: string; index: number, cointype: number } | null> {
		this.showModal();
		return new Promise(resolve => {
			this.addEventListener("submit", (_) => {
				resolve(this.refs.form.values());	
			}, {once: true, passive: true});
			this.addEventListener("close", (_) => {
				resolve(null);	
			});
		});
	}
}
SeedPhraseModalElement.registerElement();
