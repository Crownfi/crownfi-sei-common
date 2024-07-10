import { QueryClient } from "@cosmjs/stargate";
import { SeiEvmExtension } from "@crownfi/sei-js-core";
import { EVMABIFunctionDefinition, decodeEvmOutputAsArray, decodeEvmOutputAsStruct, encodeEvmFuncCall, functionSignatureToABIDefinition } from "./abi/index.js";

export async function queryEvmContract(
	queryClient: QueryClient & SeiEvmExtension,
	contractAddress: string,
	functionDefinition: EVMABIFunctionDefinition | string,
	params: any[]
): Promise<any[]> {
	if (typeof functionDefinition == "string") {
		functionDefinition = functionSignatureToABIDefinition(functionDefinition);
	}
	const result = await queryClient.evm.staticCall({
		data: encodeEvmFuncCall(functionDefinition, params),
		to: contractAddress
	});
	return decodeEvmOutputAsArray(Buffer.from(result.data), functionDefinition.outputs);
}

export async function queryEvmContractForObject(
	queryClient: QueryClient & SeiEvmExtension,
	contractAddress: string,
	functionDefinition: EVMABIFunctionDefinition | string,
	params: any[]
): Promise<any> {
	if (typeof functionDefinition == "string") {
		functionDefinition = functionSignatureToABIDefinition(functionDefinition);
	}
	const result = await queryClient.evm.staticCall({
		data: encodeEvmFuncCall(functionDefinition, params),
		to: contractAddress
	});
	return decodeEvmOutputAsStruct(Buffer.from(result.data), functionDefinition.outputs);
}
