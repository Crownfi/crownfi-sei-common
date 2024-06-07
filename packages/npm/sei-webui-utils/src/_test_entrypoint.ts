import { registerUnhandeledExceptionReporter } from "@crownfi/css-gothic-fantasy";
registerUnhandeledExceptionReporter();
await import("./index.js");
document.addEventListener("initialSeiConnection", (_) => {
	console.info("initialSeiConnection event emitted!");
});
