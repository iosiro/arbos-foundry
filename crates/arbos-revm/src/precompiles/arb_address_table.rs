use alloy_sol_types::{sol, SolCall, SolError};
use revm::{interpreter::{Gas, InstructionResult, InterpreterResult}, precompile::PrecompileId, primitives::{address, Address, Bytes, U256}};

use crate::{precompiles::extension::ExtendedPrecompile, state::ArbStateGetter, ArbitrumContextTr};
use crate::state::ArbState;


sol!{
/**
 * @title Allows registering / retrieving addresses at uint indices, saving calldata.
 * @notice Precompiled contract that exists in every Arbitrum chain at 0x0000000000000000000000000000000000000066.
 */
interface ArbAddressTable {
    /**
     * @notice Check whether an address exists in the address table
     * @param addr address to check for presence in table
     * @return true if address is in table
     */
    function addressExists(
        address addr
    ) external view returns (bool);

    /**
     * @notice compress an address and return the result
     * @param addr address to compress
     * @return compressed address bytes
     */
    function compress(
        address addr
    ) external returns (bytes memory);

    /**
     * @notice read a compressed address from a bytes buffer
     * @param buf bytes buffer containing an address
     * @param offset offset of target address
     * @return resulting address and updated offset into the buffer (revert if buffer is too short)
     */
    function decompress(
        bytes calldata buf,
        uint256 offset
    ) external view returns (address, uint256);

    /**
     * @param addr address to lookup
     * @return index of an address in the address table (revert if address isn't in the table)
     */
    function lookup(
        address addr
    ) external view returns (uint256);

    /**
     * @param index index to lookup address
     * @return address at a given index in address table (revert if index is beyond end of table)
     */
    function lookupIndex(
        uint256 index
    ) external view returns (address);

    /**
     * @notice Register an address in the address table
     * @param addr address to register
     * @return index of the address (existing index, or newly created index if not already registered)
     */
    function register(
        address addr
    ) external returns (uint256);

    /**
     * @return size of address table (= first unused index)
     */
    function size() external view returns (uint256);
}

}

pub fn arb_address_table_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbAddressTable")),
        address!("0x0000000000000000000000000000000000000066"),
        arb_address_table_run::<CTX>,
    )
}
/// Run the precompile with the given context and input data.
/// Run the arb_address_table precompile with the given context and input data.
fn arb_address_table_run<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    input: &[u8],
    _target_address: &Address,
    _caller_address: Address,
    _call_value: U256,
    _is_static: bool,
    gas_limit: u64,
) -> Result<Option<InterpreterResult>, String> {
    
    // decode selector
    if input.len() < 4 {
        return Ok(Some(InterpreterResult {
            result: InstructionResult::Revert,
            gas: Gas::new(gas_limit),
            output: Bytes::from("Input too short"),
        }));
    }

    // decode selector
    let selector: [u8; 4] = input[0..4].try_into().unwrap();

    match selector {
        ArbAddressTable::addressExistsCall::SELECTOR => {
            let call = ArbAddressTable::addressExistsCall::abi_decode(&input).unwrap();

            let exists = context.arb_state().address_table().address_exists(call.addr);

            let output = ArbAddressTable::addressExistsCall::abi_encode_returns(&exists);

            return Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }));
        },
        ArbAddressTable::compressCall::SELECTOR => {
            let call = ArbAddressTable::compressCall::abi_decode(&input).unwrap();

            let compressed = context.arb_state().address_table().compress(call.addr);

            let output = ArbAddressTable::compressCall::abi_encode_returns(&compressed);

            return Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }));
        },
        ArbAddressTable::decompressCall::SELECTOR => {
            let call = ArbAddressTable::decompressCall::abi_decode(&input).unwrap();

            let (decompressed, new_offset) = context.arb_state().address_table().decompress(&call.buf, call.offset)?;
            let output = ArbAddressTable::decompressCall::abi_encode_returns(&(decompressed, new_offset));
            return Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }));
        },
        ArbAddressTable::lookupCall::SELECTOR => {
            let call = ArbAddressTable::lookupCall::abi_decode(&input).unwrap();
            let index = context.arb_state().address_table().lookup(call.addr)?;
            let output = ArbAddressTable::lookupCall::abi_encode_returns(&index);
            return Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }));
        },
        ArbAddressTable::lookupIndexCall::SELECTOR => {
            let call = ArbAddressTable::lookupIndexCall::abi_decode(&input).unwrap();
            let addr = context.arb_state().address_table().lookup_index(call.index)?;
            let output = ArbAddressTable::lookupIndexCall::abi_encode_returns(&addr);
            return Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }));
        },
        ArbAddressTable::registerCall::SELECTOR => {
            let call = ArbAddressTable::registerCall::abi_decode(&input).unwrap();
            let index = context.arb_state().address_table().register(call.addr);
            let output = ArbAddressTable::registerCall::abi_encode_returns(&index);
            return Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }));
        },
        ArbAddressTable::sizeCall::SELECTOR => {
            let _ = ArbAddressTable::sizeCall::abi_decode(&input).unwrap();
            let size = context.arb_state().address_table().size();
            let output = ArbAddressTable::sizeCall::abi_encode_returns(&size);
            return Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }));
        },
        _ => {
            return Ok(Some(InterpreterResult {
                result: InstructionResult::Revert,
                gas: Gas::new(gas_limit),
                output: Bytes::from("Unknown function selector"),
            }));
        }
    }
}
