import { ABIDefType } from "./evm_type_map.js";

export const NULL_BYTES = Buffer.alloc(32);
export const ONE_BYTE = (() => {const buf = Buffer.alloc(32); buf[31] = 1; return buf;})();
export const UINT256_SIZE = 32;
export const UINT256_MAX = 2n ** 256n - 1n;

export type EVMABIFunctionType = "function" | "constructor" | "receive" | "fallback";
export type EVMABIFunctionStateAccess = "pure" | "view" | "nonpayable" | "payable";
/**
 * Represents a function defined in accordance with the solidity ABI specification.
 */
export interface EVMABIFunctionDefinition {
	name: string,
	// Note, "receive" has no payload by definition
	type: EVMABIFunctionType,
	inputs: EVMABITupleComponent[],
	outputs: EVMABITupleComponent[],
	stateMutability: EVMABIFunctionStateAccess
}
/**
 * A struct field or function parametar
 */
export interface EVMABITupleComponent<T extends ABIDefType = ABIDefType> {
	name: string,
	type: T,
	/**
	 * This should only be defiend if `type` is `tuple` or `tuple[]`
	 */
	components?: EVMABITupleComponent[]
}

/**
 * 
 * @param abiDef 
 * @returns 
 */
export function tupleComponentTypeSignature(abiDef: EVMABITupleComponent): string {
	const abiDefType = abiDef.type.trim();
	if (abiDefType == "tuple" || abiDefType == "tuple[]") {
		return tupleComponentsTypeSignature(abiDef.components || []) + (abiDefType.endsWith("[]") ? "[]" : "");
	}
	return abiDefType;
}
export function tupleComponentsTypeSignature(abiDefs: EVMABITupleComponent[]): string {
	return "(" + abiDefs.map(tupleComponentTypeSignature).join(",") + ")";
}

function fullTupleSubstring(str: string): [string, string, string] {
	let bracketCount = 0;
	let index = str.indexOf("(");
	if (index == -1) {
		return ["", "", str];
	}
	const start = str.substring(0, index);
	while (index < str.length) {
		const char = str[index];
		switch(char) {
			case "(":
				bracketCount += 1;
				break;
			case ")":
				bracketCount -= 1;
				break;
			default:
				// no default
		}
		index += 1;
		if (bracketCount == 0) {
			return [start.trim(), str.substring(start.length, index).trim(), str.substring(index).trim()];
		}
	}
	return [start.trim(), str.substring(start.length).trim(), ""];
}

/**
 * Converts an EVM function signature (with an optional outputs component) into an EVMABIFunctionDefinition
 * @param sig Something like "myFunc(uint256, uint256)" or "balanceOf(address owner) view returns (uint256 balance)"
 */
export function functionSignatureToABIDefinition(sig: string): EVMABIFunctionDefinition {
	const [name, inputs, modifiersAndOutput] = fullTupleSubstring(sig);
	const [modifiers, outputs, rest] = fullTupleSubstring(modifiersAndOutput);

	const inputAsComponent = normalizeTupleComponent({name: "arg", type: inputs as ABIDefType});
	const outputAsComponent = !modifiers.endsWith("returns") ?
			{
				name: "",
				type: "tuple" as ABIDefType,
				components: []
			} : (
			outputs ?
				normalizeTupleComponent({name: "arg", type: outputs as ABIDefType}) :
				typeAndNameToComponent(rest)
		);
	return {
		name,
		type: name == "" ? (inputAsComponent.components ? "fallback" : "receive") : "function",
		stateMutability: modifiers.includes("pure") ? "pure" : (
			modifiers.includes("view") ? "view" : (
				modifiers.includes("payable") ? "payable" : "nonpayable"
			)
		),
		inputs: inputAsComponent.components || [],
		outputs: outputAsComponent.type == "tuple" ? (outputAsComponent.components || []) : [outputAsComponent]
	}
}

/**
 * Turns a type string with an optional name into an EVMABITupleComponent
 * 
 * Note that if your typeString has a space, anything after the last space will be used for the resulting name.
 * Otherwise the name will default to ""
 * 
 * @param typeString the type string
 * @returns The resulting EVMABITupleComponent
 */
function typeAndNameToComponent(typeString: string): EVMABITupleComponent {
	const lastSpace = typeString.lastIndexOf(" ");
	// Tuple names or something, don't use the space.
	if (typeString.indexOf(")", lastSpace) !== -1) {
		return {
			name: "",
			type: typeString.trim() as ABIDefType
		};
	}
	if (lastSpace == -1) {
        return {
            name: "",
            type: typeString as ABIDefType
        }
    }
	return {
		name: typeString.substring(lastSpace).trim() || "",
		type: typeString.substring(0, lastSpace).trim() as ABIDefType
	};
}

/**
 * Parses a single type. This can be a primitive, array, or a tuple. For example `uint256` or `(uint32 x, bool y)`
 * @param typeString the type to parse
 * @returns 
 */
export function eVMTypeToComponent(typeString: string): EVMABITupleComponent {
	return normalizeTupleComponent(
		typeAndNameToComponent(typeString)
	);
}

/**
 * If the "type" property is a tuple defined by "(T1, T2, ...)", then normalize it be "tuple"
 * 
 * Note that this will edit the object passed.
 */
export function normalizeTupleComponent(abiDef: EVMABITupleComponent): EVMABITupleComponent {
	const isArray = abiDef.type.endsWith(")[]");
	if (
		!abiDef.type.startsWith("(") ||
		(
			!abiDef.type.endsWith(")") &&
			!abiDef
		)
	) {
		// Not a valid tuple, do nothing
		return abiDef;
	}
	let innerTuplesLevel = 0;
	let curType = "";
	const oldType = abiDef.type;
	abiDef.type = isArray ? "tuple[]" : "tuple";
	abiDef.components = [];
	for (let i = 1; i < oldType.length - (isArray ? 3 : 1); i += 1) {
		const char = oldType[i];
		if (innerTuplesLevel > 0) {
			curType += char;
			if (char == ")") {
				innerTuplesLevel -= 1;
			}
			continue;
		}
		if (char == "(") {
			curType += char;
			innerTuplesLevel += 1;
			continue;
		}
		if (char != ",") {
			curType += char;
			continue;
		}
		abiDef.components.push(
			normalizeTupleComponent(typeAndNameToComponent(curType.trim()))
		);
		curType = "";
	}
	abiDef.components.push(
		normalizeTupleComponent(typeAndNameToComponent(curType.trim()))
	);
	return abiDef;
}

/**
 * @internal
 * internal stuff donut use
 */
export const encodedArrayType = function(type: string): [boolean, number | null, ABIDefType] {
	// Note: In solidity, the behaviour is uint[][5] -> (uint[])[5]
	//   Described as "an array of 5 dynamic arrays"

	const i = type.lastIndexOf("[");
	if(i === -1){
		return [false, null, type as ABIDefType];
	}
	const arrayLength = type.substring(i + 1, type.length - 1); // I'm assuming the last char is "]"
	type = type.substring(0, i);
	if(arrayLength === ""){
		return [true, null, type as ABIDefType];
	}
	return [true, Number(arrayLength) || 0, type as ABIDefType];
};
