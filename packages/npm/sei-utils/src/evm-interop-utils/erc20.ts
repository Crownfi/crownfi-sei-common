import { functionSignatureToABIDefinition } from "./abi/common.js";

export const ERC20_FUNC_TOTAL_SUPPLY = functionSignatureToABIDefinition("totalSupply() view returns (uint256)");
export const ERC20_FUNC_BALANCE_OF = functionSignatureToABIDefinition("balanceOf(address) view returns (uint256 balance)");
export const ERC20_FUNC_APPROVE = functionSignatureToABIDefinition("approve(address, uint256) returns (bool success)");
export const ERC20_FUNC_TRANSFER = functionSignatureToABIDefinition("transfer(address, uint256) returns (bool success)");
export const ERC20_FUNC_TRANSFER_FROM = functionSignatureToABIDefinition("transferFrom(address, address, uint256) returns (bool success)")
// If you want more, call functionSignatureToABIDefinition yourself!
