import { toChecksumAddressEvm } from "../address.js";
import { EVMABITupleComponent, NULL_BYTES, UINT256_SIZE, eVMTypeToComponent, encodedArrayType, normalizeTupleComponent } from "./common.js";

type PayloadDecodeType = "struct" | "array";
class EvmPayloadDecoder {
	decodeBoolean(): boolean {
		return !this.data.subarray(this.index, this.index += UINT256_SIZE).equals(NULL_BYTES);
	}
	// uint8 to uint48
	decodeUInt(byteLength: number): number {
		return this.data.readUIntBE((this.index += UINT256_SIZE) - byteLength, byteLength);
	}
	// int8 to int48
	decodeInt(byteLength: number): number {
		return this.data.readIntBE((this.index += UINT256_SIZE) - byteLength, byteLength);
	}
	decodeFixedByteArray(byteLength: number, leftPadded: boolean = false): Buffer {
		if (byteLength > 32) {
			byteLength = 32;
		}
		if (leftPadded) {
			// For ints, etc.
			return this.data.subarray((this.index += UINT256_SIZE) - byteLength, this.index);
		} else {
			const result = this.data.subarray(this.index, this.index + byteLength);
			this.index += UINT256_SIZE;
			return result;
		}
	}
	decodeBigUInt(byteLength: number): bigint {
		return BigInt("0x" + this.decodeFixedByteArray(byteLength, true))
	}
	decodeBigInt(byteLength: number): bigint {
		const maxValue = 2n ** BigInt(byteLength * 8 - 1) - 1n;
		let result = this.decodeBigUInt(byteLength);
		if (result > maxValue) {
			result -= 2n ** BigInt(byteLength * 8);
		}
		return result;
	}
	decodeUDecimal(byteLength: number, decimalExponent: number): number {
		let value = this.decodeUInt(byteLength);
		value /= 10 ** decimalExponent;
		return value;
	}
	decodeDecimal(byteLength: number, decimalExponent: number): number {
		let value = this.decodeInt(byteLength);
		value /= 10 ** decimalExponent;
		return value;
	}
	decodeBigUDecimal(byteLength: number, decimalExponent: number): string {
		const result = this.decodeBigUInt(byteLength).toString();
		const decimalPos = result.length - decimalExponent;
		return result.substring(0, decimalPos) + "." + result.substring(decimalPos);
	}
	decodeBigDecimal(byteLength: number, decimalExponent: number): string {
		const result = this.decodeBigInt(byteLength).toString();
		const decimalPos = result.length - decimalExponent;
		return result.substring(0, decimalPos) + "." + result.substring(decimalPos);
	}
	decodeAddress() {
		return toChecksumAddressEvm("0x" + this.decodeFixedByteArray(20, true));
	}
	decodeBuffer() {
		let bufferStartOffset = this.decodeUInt(6) + this.arrayPointerOffset; 
		if (bufferStartOffset >= this.data.length) {
			throw new RangeError("Malformed EVM data? A bytes or string referenced data outside the buffer!");
		}
		bufferStartOffset += UINT256_SIZE;
		const bufferLength = this.data.readInt32LE(bufferStartOffset - 4);
		const bufferEndOffset = bufferStartOffset + bufferLength;
		if (bufferEndOffset > this.data.length) {
			throw new RangeError("Malformed EVM data? A bytes or string referenced data outside the buffer!");
		}
		return this.data.subarray(bufferStartOffset, bufferEndOffset);
	}
	decodeString() {
		return this.decodeBuffer().toString();
	}
	decodeTupleComponent(abiDef: EVMABITupleComponent) {
		normalizeTupleComponent(abiDef);
		const [isArray, arrayLength, innerType] = encodedArrayType(abiDef.type);
		let result: any;
		if (isArray) {
			const innerAbiDef = {
				name: abiDef.name,
				type: innerType,
				components: abiDef.components // Might be an array of tuple!!
			};
			if (arrayLength != null) {
				let arrayStartOffset = this.decodeUInt(6) + this.arrayPointerOffset; 
				if (arrayStartOffset >= this.data.length) {
					throw new RangeError("Malformed EVM data? A bytes or string referenced data outside the buffer!");
				}
				arrayStartOffset += UINT256_SIZE;
				const arrayLength = this.data.readInt32LE(arrayStartOffset - 4);
				const newArray = ((new Array(arrayLength) as unknown[]) as EVMABITupleComponent[]).fill(innerAbiDef);
				const innerDecoder = new EvmPayloadDecoder(
					this.data,
					newArray,
					"array",
					arrayStartOffset
				);
				result = innerDecoder.resultArray;
			} else {
				const newArray = ((new Array(arrayLength) as unknown[]) as EVMABITupleComponent[]).fill(innerAbiDef);
				const innerDecoder = new EvmPayloadDecoder(
					this.data,
					newArray,
					"array",
					this.index,
					this.arrayPointerOffset
				);
				this.index = innerDecoder.index;
				result = innerDecoder.resultArray;
			}
		}
		switch (abiDef.type) {
			case "tuple": {
				const innerComponents = abiDef.components || [];
				const innerDecoder = new EvmPayloadDecoder(
					this.data,
					innerComponents,
					innerComponents.findIndex(v => v.name.length > 0) == -1 ? "array" : "struct",
					this.index,
					this.arrayPointerOffset
				);
				this.index = innerDecoder.index;
				const innerValue = innerDecoder.decodeType == "array" ?
					innerDecoder.resultArray :
					innerDecoder.resultObject;
				if (this.decodeType == "struct" && abiDef.name) {
					this.resultObject[abiDef.name] = innerValue;
				} else {
					this.resultArray.push(innerValue);
				}
				return;
			}
			case "address":
				result = this.decodeAddress();
				break;
			case "bool":
				result = this.decodeBoolean();
				break;
			case "uint":
				result = this.decodeBigUInt(32);
				break;
			case "int":
				result = this.decodeBigInt(32);
				break;
			case "fixed":
				result = this.decodeBigDecimal(16, 18);
				break;
			case "ufixed":
				result = this.decodeBigUDecimal(16, 18);
				break;
			case "func":
			case "function":
				result = this.decodeFixedByteArray(24);
				break;
			case "byte":
				this.decodeFixedByteArray(1);
				break;
			case "bytes":
				result = this.decodeBuffer();
				break;
			case "string":
				result = this.decodeString();
				break;
			default:
				if (isArray) {
					break;
				}
				if (innerType.startsWith("uint")) {
					const byteLength = Number(innerType.substring("uint".length)) / 8;
					if (byteLength > 6) {
						result = this.decodeBigUInt(byteLength);
					} else {
						result = this.decodeUInt(byteLength);
					}
				} else if (innerType.startsWith("int")) {
					const byteLength = Number(innerType.substring("int".length)) / 8;
					if (byteLength > 6) {
						result = this.decodeBigInt(byteLength);
					} else {
						result = this.decodeInt(byteLength);
					}
				} else if (innerType.startsWith("bytes")) {
					result = this.decodeFixedByteArray(Number(innerType.substring("bytes".length)));
				} else if (innerType.startsWith("fixed")) {
					const sep = innerType.indexOf("x");
					const byteLength = Number(innerType.substring("fixed".length, sep)) / 8;
					const decimalExponent = Number(innerType.substring(sep + 1));
					if (byteLength > 6) {
						result = this.decodeBigDecimal(byteLength, decimalExponent);
					} else {
						result = this.decodeDecimal(byteLength, decimalExponent);
					}
				} else if (innerType.startsWith("ufixed")) {
					const sep = innerType.indexOf("x");
					const byteLength = Number(innerType.substring("ufixed".length, sep)) / 8;
					const decimalExponent = Number(innerType.substring(sep + 1));
					if (byteLength > 6) {
						result = this.decodeBigUDecimal(byteLength, decimalExponent);
					} else {
						result = this.decodeUDecimal(byteLength, decimalExponent);
					}
				}
		}
		if (this.decodeType == "struct" && abiDef.name) {
			this.resultObject[abiDef.name] = result;
		} else {
			this.resultArray.push(result);
		}
	}
	
	index: number;
	arrayPointerOffset: number;
	data: Buffer
	decodeType: PayloadDecodeType;
	resultArray: any[];
	resultObject: any;
	constructor(
		data: Buffer,
		abiDefs: EVMABITupleComponent[],
		decodeType: PayloadDecodeType,
		initialIndex: number = 0,
		arrayPointerOffset: number = initialIndex
	) {
		this.resultArray = [];
		this.resultObject = Object.create(null);
		this.data = data;
		this.index = initialIndex;
		this.arrayPointerOffset = arrayPointerOffset;
		this.decodeType = decodeType;
		for (let i = 0; i < abiDefs.length; i += 1) {
			this.decodeTupleComponent(abiDefs[i]);
		}
	}
}

export function decodeEvmType(data: Buffer, evmType: string | EVMABITupleComponent): any {
	if (typeof evmType == "string") {
		evmType = eVMTypeToComponent(evmType);
	}
	if (evmType.type == "tuple") {
		const unnamed = (evmType.components || []).findIndex(v => v.name.length > 0) == -1;
		const decoder = new EvmPayloadDecoder(data, [evmType], unnamed ? "array" : "struct");
		return unnamed ? decoder.resultArray : decoder.resultObject;
	}else{
		const decoder = new EvmPayloadDecoder(data, [evmType], "array");
		return decoder.resultArray[0];
	}
}

export function decodeEvmOutputAsArray(data: Buffer, output: EVMABITupleComponent[]): any {
	const decoder = new EvmPayloadDecoder(data, output, "array");
	return decoder.resultArray;
}
export function decodeEvmOutputAsStruct(buffer: Buffer, output: EVMABITupleComponent[]): any {
	const decoder = new EvmPayloadDecoder(buffer, output, "struct");
	return decoder.resultObject;
}
