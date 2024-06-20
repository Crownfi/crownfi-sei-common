import { isValidEvmAddress } from "../address.js";

// If I ever see this in the end result, should be a cause for concern.
const GURAD_BYTES = Buffer.from("fefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefe", "hex");
import { keccak256 } from "keccak-wasm";
import { EVMABIFunctionDefinition, EVMABITupleComponent, NULL_BYTES, ONE_BYTE, UINT256_MAX, UINT256_SIZE, encodedArrayType, functionSignatureToABIDefinition, normalizeTupleComponent, tupleComponentsTypeSignature } from "./common.js";

// Encoding functions
const encodeBoolean = function(bool: any): Buffer {
	if(bool){
		// We can't count on the return value not being modified, so we'll do a copy.
		return Buffer.from(ONE_BYTE); // Truly a waste.
	}
	return Buffer.from(NULL_BYTES);
};

const encodeInt = function(num: any): Buffer {
	// Note: We don't actually check the bit component of the int type, we always encode as u256 or i256.
	// Assuming the value is within range, the resulting encoding is fully compliant with the ABI spec.
	if (typeof num == "number") {
		num = BigInt(Math.trunc(num || 0));
	}
	if (typeof num == "string") {
		const decimalIndex = num.indexOf(".");
		if (decimalIndex >= 0) {
			num = BigInt(num.substring(0, decimalIndex));
		} else {
			num = BigInt(num);
		}
	}
	if (typeof num != "bigint") {
		num = BigInt(Math.trunc(Number(num) || 0))
	}
	if(num < 0){
		num = UINT256_MAX + 1n + num;
		if(num < 0){
			throw new RangeError("Cannot have a negative integer less than -115,792,089,237,316,195,423,570,985,008,687,907,853,269,984,665,640,564,039,457,584,007,913,129,639,935.");
		}
	}
	// This is actually the fastest way to encode a bigint to a buffer
	const str = num.toString(16);
	if(str.length > 64){
		throw new RangeError("Cannot have a integer larger than 115,792,089,237,316,195,423,570,985,008,687,907,853,269,984,665,640,564,039,457,584,007,913,129,639,935.");
	}
	return Buffer.from("0".repeat(64 - str.length) + str, "hex");
};

const encodeDecimal = function(num: any, decimalExponent: number) {
	// Note: We don't actually check the bit component the fixed type. 256 bits is always assumed.
	if (typeof num == "bigint") {
		return encodeInt(num * (10n ** BigInt(decimalExponent)));
	}
	if (typeof num == "number") {
		num = String(num);
	}
	if (typeof num != "string") {
		num = String(Number(num));
	}
	const numString = (num as string).trim(); // Not sure why ts couldn't inferr that I've guaranteed the type, but w/e
	if (!numString || numString == "NaN") {
		return Buffer.from(NULL_BYTES);
	}
	const sciNotation = numString.match(/^([-+]?)[0-9]+e([-+]?)([0-9]+)$/);
	if (sciNotation != null) {
		let valueDecimalExponent = (BigInt(sciNotation[4]) * (sciNotation[3] == "-" ? -1n : 1n));
		const mantissaString = sciNotation[2];
		const mantissaPoint = mantissaString.indexOf(".");
		let valueAsInt = 0n;
		if (mantissaPoint) {
			valueAsInt = BigInt(mantissaString.substring(0, mantissaPoint) + mantissaString.substring(mantissaPoint + 1));
			valueDecimalExponent -= BigInt(mantissaString.length) - BigInt(mantissaPoint) - 1n;
		} else {
			valueAsInt = BigInt(mantissaString);
		}
		valueDecimalExponent += BigInt(decimalExponent);
		if (valueDecimalExponent < 0) {
			valueAsInt /= 10n ** (valueDecimalExponent * -1n);
		} else {
			valueAsInt *= 10n ** valueDecimalExponent;
		}
		if (sciNotation[1] == "-") {
			valueAsInt *= -1n;
		}
		return encodeInt(valueAsInt);
	}
	const decimalParts = numString.match(/^([0-9]+)\.?([0-9]*)$/);
	if (decimalParts == null) {
		throw new TypeError("Couldn't encode \"" + num + "\" as a decimal number");
	}
	let [_, sign, intPart, fractionPart] = decimalParts;
	if (fractionPart.length > decimalExponent) {
		fractionPart = fractionPart.substring(0, decimalExponent);
	} else if (fractionPart.length < decimalExponent) {
		fractionPart += "0".repeat(decimalExponent - fractionPart.length);
	}
	let valueAsInt = BigInt(intPart) * (10n ** BigInt(decimalExponent));
	valueAsInt += BigInt(fractionPart);
	if (sign == "-") {
		valueAsInt *= -1n;
	}
	return encodeInt(valueAsInt);
}

const encodeAddress = function(maybeAddress: any): Buffer {
	if (maybeAddress instanceof Uint8Array) {
		if (maybeAddress.length != 20) {
			throw new TypeError("Can only pass buffers directly into the address param if they're 20 bytes long");
		}
		return Buffer.concat([Buffer.alloc(12), maybeAddress], 32);
	}
	const address = maybeAddress + "";
	if (!isValidEvmAddress(address, true)) {
		throw new TypeError("\"" + address + "\" is not a valid EVM address.");
	}
	const result = Buffer.alloc(32);
	result.write(address.substring(2), 12, 20, "hex");
	return result;
};

function validateContractFunc(contractFunc: any): asserts contractFunc is {address: string, func: EVMABIFunctionDefinition | string} {
	if (
		typeof contractFunc != "object" ||
		contractFunc == null ||
		typeof contractFunc.address != "string" ||
		typeof contractFunc.func != "string" ||
		contractFunc.func == null ||
		typeof contractFunc.func.name != "string" ||
		!Array.isArray(contractFunc.func.inputs)
	) {
		throw new TypeError("encodeFunction: parameter must be a {address: string, func: EVMABIFunctionDefinition | string}");
	}
}
const encodeFunction = function(contractFunc: any): Buffer {
	validateContractFunc(contractFunc);
	if (!isValidEvmAddress(contractFunc.address, true)) {
		throw new TypeError("\"" + contractFunc.address + "\" is not a valid EVM address.");
	}
	const result = Buffer.alloc(32);
	result.write(contractFunc.address.substring(2), "hex");

	const funcDefinition = typeof contractFunc.func == "string" ?
		functionSignatureToABIDefinition(contractFunc.func) :
		contractFunc.func;
	
	if (funcDefinition.type != "fallback") {
		result.fill(
			keccak256(
				Buffer.from(
					funcDefinition.name +
					tupleComponentsTypeSignature(funcDefinition.inputs)
				)
			),
			20,
			24
		);
	}
	return result;
};

const encodeFixedBuffer = function(maybeBuf: any, expectedLength: number): Buffer {
	if (typeof maybeBuf == "string" && maybeBuf.startsWith("0x")) {
		maybeBuf = Buffer.from(maybeBuf.substring(2), "hex");
	}
	if (!Buffer.isBuffer(maybeBuf)) {
		try {
			maybeBuf = Buffer.from(maybeBuf);
		}catch(ex: any) {
			maybeBuf = Buffer.from(maybeBuf + "");
		}
	}
	const buff = maybeBuf as Buffer;
	if (buff.length > 32) {
		throw new RangeError("Fixed-length buffers cannot be longer than 32 bytes in size");
	}
	if (expectedLength != buff.length) {
		console.trace(
			"You passed a buffer of length " + buff.length + " to an EVM function which takes a bytes" + expectedLength + ".",
			"Nothing really wrong with this as it will get truncated anyway, but you should feel bad.",
		);
	}
	if (buff.length == 32) {
		return buff;
	}
	return Buffer.concat([buff, new Uint8Array(32 - buff.length)], 32);
};

const encodeBuffer = function(maybeBuf: any): {requiresPointer: Buffer} {
	if (typeof maybeBuf == "string" && maybeBuf.startsWith("0x")) {
		maybeBuf = Buffer.from(maybeBuf.substring(2), "hex");
	}
	if (!Buffer.isBuffer(maybeBuf)) {
		try {
			maybeBuf = Buffer.from(maybeBuf);
		}catch(ex: any) {
			maybeBuf = Buffer.from(maybeBuf + "");
		}
	}
	const buff = maybeBuf as Buffer;
	if(buff.length == 0){
		return {requiresPointer: Buffer.alloc(0)}
	}else{
		return {requiresPointer: Buffer.concat([encodeInt(buff.length), buff, Buffer.alloc(buff.length % 32)])}
	}
};

const encodeString = function(str: any): {requiresPointer: Buffer} {
	return encodeBuffer(Buffer.from(str + ""));
};
type EncodeFunction = (val: any) => Buffer | {requiresPointer: Buffer};

const encodeFunctions: {[key: string]: EncodeFunction} = {
	int: encodeInt,
	uint: encodeInt,
	address: encodeAddress,
	bool: encodeBoolean,
	bytes: encodeBuffer,
	string: encodeString,
	func: encodeFunction,
	function: encodeFunction
};
// > Actually copy/pasting
for (let i = 8; i <= 256; i += 8) {
	encodeFunctions["uint" + i] = encodeInt;
	encodeFunctions["int" + i] = encodeInt;
	for (let ii = 1; ii <= 80; ii += 1) {
		encodeFunctions["fixed" + i + "x" + ii] = function(value) {
			return encodeDecimal(value, ii);
		};
		encodeFunctions["int" + i + "x" + ii] = function(value) {
			return encodeDecimal(value, ii);
		};
	}
}
for (let i = 1; i <= 32; i += 1) {
	const ii = i;
	encodeFunctions["bytes" + ii] = function(value) {
		return encodeFixedBuffer(value, ii);
	};
}
encodeFunctions.byte = encodeFunctions.bytes1;
encodeFunctions.fixed = encodeFunctions.fixed128x18;
encodeFunctions.ufixed = encodeFunctions.ufixed128x18;




class EvmPayloadEncoder {
	prepend: Uint8Array
	currentLength: number
	payloadSegments: Uint8Array[]
	appendedHeapSegments: {segmentIndex: number, data: Buffer}[]
	appendedDynamicArray: {segmentIndex: number, arrayLength: number, encoder: EvmPayloadEncoder}[]
	constructor(prepend: Uint8Array = new Uint8Array(0)) {
		this.currentLength = 0;
		this.payloadSegments = [];
		this.appendedHeapSegments = [];
		this.appendedDynamicArray = [];
		this.prepend = prepend;
		this.__zeroLengthPtrValue = null;
	}
	appendData(data: Buffer) {
		this.payloadSegments.push(data);
		this.currentLength += data.length;
	}
	appendDynamicData(data: Buffer) {
		this.appendedHeapSegments.push({
			data,
			segmentIndex: this.payloadSegments.length
		});
		this.payloadSegments.push(GURAD_BYTES);
		this.currentLength += GURAD_BYTES.length;
	}
	encodeDynamicArray(value: any[], abiDef: EVMABITupleComponent) {
		const newEncoder = new EvmPayloadEncoder();
		for (let i = 0; i < value.length; i += 1) {
			newEncoder.encodeTupleComponent(value[i], abiDef);
		}
		this.appendedDynamicArray.push({
			encoder: newEncoder,
			arrayLength: value.length,
			segmentIndex: this.payloadSegments.length
		});
		this.payloadSegments.push(GURAD_BYTES);
		this.currentLength += GURAD_BYTES.length;
	}

	encodeTupleComponent(value: any, abiDefs: EVMABITupleComponent) {
		normalizeTupleComponent(abiDefs);
		const [isArray, arrayLength, innerType] = encodedArrayType(abiDefs.type);
		if (isArray) {
			if (!Array.isArray(value)) {
				throw new Error("Attempted to encode an " + abiDefs.type + " without the given value being an array.");
			}
			const innerAbiDef = {
				name: abiDefs.name,
				type: innerType,
				components: abiDefs.components // Might be an array of tuple!!
			};
			if (arrayLength == null) {
				this.encodeDynamicArray(value, innerAbiDef);
			} else {

				for (let i = 0; i < arrayLength; i += 1) {
					this.encodeTupleComponent(value[i], innerAbiDef);
				}
			}
			return;
		}
		if (innerType == "tuple") {
			const tupleComponents = abiDefs.components || [];
			if (!Array.isArray(value)) {
				if (value == null) {
					this.encodeTuple([], tupleComponents);
					return;
				}
				const valueAsArray = [];
				for (let i = 0; i < tupleComponents.length; i += 1) {
					// This is how structs are encoded so... we can try this.
					valueAsArray.push(value[tupleComponents[i].name]);
				}
				this.encodeTuple(valueAsArray, tupleComponents);
				return;
			} 
			this.encodeTuple(value, tupleComponents);
			return;
		}
		if (innerType in encodeFunctions) {
			const result = encodeFunctions[innerType](value);
			if ("requiresPointer" in result) {
				this.appendDynamicData(result.requiresPointer);
			} else {
				this.appendData(result);
			}
		} else {
			throw new TypeError("Invalid or unsupported EVM type " + innerType);
		}
	}
	encodeTuple(values: any[], abiDefs: EVMABITupleComponent[]) {
		for (let i = 0; i < values.length; i += 1) {
			this.encodeTupleComponent(values[i], abiDefs[i]);
		}
	}
	__zeroLengthPtrValue: number | null;
	_zeroLengthPtrValue() {
		if (this.__zeroLengthPtrValue == null) {
			this.__zeroLengthPtrValue = this.currentLength;
			this.payloadSegments.push(NULL_BYTES);
			this.currentLength += NULL_BYTES.length;
		}
		return this.__zeroLengthPtrValue;
	}
	payload(): Buffer {
		while (this.appendedHeapSegments.length) {
			const {segmentIndex, data} = this.appendedHeapSegments.pop()!;
			if (data.length == 0) {
				this.payloadSegments[segmentIndex] = encodeInt(this._zeroLengthPtrValue());
			} else {
				// Encoded variable length strings and buffers are already length-prefixed.
				this.payloadSegments[segmentIndex] = encodeInt(this.currentLength);
				this.payloadSegments.push(data);
				this.currentLength += data.length;
			}
		}
		while (this.appendedDynamicArray.length) {
			// Note: apparently this also works with nested dynamic arrays as the offset is counted from the beginning
			// of the array data.
			const {segmentIndex, encoder, arrayLength} = this.appendedDynamicArray.pop()!;
			if (arrayLength == 0) {
				this.payloadSegments[segmentIndex] = encodeInt(this._zeroLengthPtrValue());
			} else {
				this.payloadSegments[segmentIndex] = encodeInt(this.currentLength);
				// Push dyn array length
				this.payloadSegments.push(encodeInt(arrayLength));
				this.currentLength += UINT256_SIZE;
				// Push dyn array values
				const innerEncoded = encoder.payload();
				this.payloadSegments.push(innerEncoded);
				this.currentLength += innerEncoded.length;
			}
		}
		if (this.prepend.length) {
			this.payloadSegments.unshift(this.prepend);
			this.currentLength += this.prepend.length;
		}
		const result = Buffer.concat(this.payloadSegments, this.currentLength);
		// Might as well reset in case this is re-used
		this.payloadSegments.length = 0;
		this.currentLength = 0;
		this.__zeroLengthPtrValue = null;
		return result;
	}
	
}

export function encodeEvmType(value: any, evmType: string | EVMABITupleComponent): Buffer {
	if (typeof evmType == "string") {
		evmType = normalizeTupleComponent({
			name: "",
			type: evmType
		});
	}
	const encoder = new EvmPayloadEncoder();
	encoder.encodeTupleComponent(value, evmType);
	return encoder.payload();
}

export function encodeEvmFuncCall(funcDefinition: EVMABIFunctionDefinition | string, parameters: any[]): Buffer {
	if (typeof funcDefinition == "string") {
		funcDefinition = functionSignatureToABIDefinition(funcDefinition);
	}
	if (funcDefinition.type == "receive") {
		return Buffer.alloc(0);
	}
	const encoder = new EvmPayloadEncoder(
		funcDefinition.type == "fallback" ?
		Buffer.alloc(4, 0xff) :
		keccak256(
			// The type signature is re-encoded even if the funcDefinition is a string as it could include param names
			Buffer.from(funcDefinition.name + tupleComponentsTypeSignature(funcDefinition.inputs))
		).subarray(0, 4)
	);
	encoder.encodeTuple(parameters, funcDefinition.inputs);
	return encoder.payload();
}
