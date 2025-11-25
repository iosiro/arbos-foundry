use alloy_sol_types::{SolCall, sol};
use revm::{
    interpreter::{Gas, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, Bytes, U256, address},
};

use crate::{
    ArbitrumContextTr,
    precompiles::{
        ExtendedPrecompile,
        macros::{return_revert, return_success, try_state},
    },
    state::{ArbState, ArbStateGetter},
};

sol! {
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
    let mut gas = Gas::new(gas_limit);

    // decode selector
    if input.len() < 4 {
        return_revert!(gas, Bytes::from("Input too short"));
    }

    // decode selector
    let selector: [u8; 4] = input[0..4].try_into().unwrap();

    match selector {
        ArbAddressTable::addressExistsCall::SELECTOR => {
            let call = ArbAddressTable::addressExistsCall::abi_decode(input).unwrap();

            let exists = try_state!(
                gas,
                context.arb_state(Some(&mut gas)).address_table().address_exists(call.addr)
            );

            let output = ArbAddressTable::addressExistsCall::abi_encode_returns(&exists);

            return_success!(gas, Bytes::from(output));
        }
        ArbAddressTable::compressCall::SELECTOR => {
            let call = ArbAddressTable::compressCall::abi_decode(input).unwrap();

            let compressed = try_state!(
                gas,
                context.arb_state(Some(&mut gas)).address_table().compress(call.addr)
            );

            return_success!(
                gas,
                Bytes::from(ArbAddressTable::compressCall::abi_encode_returns(&compressed))
            );
        }
        ArbAddressTable::decompressCall::SELECTOR => {
            let call = ArbAddressTable::decompressCall::abi_decode(input).unwrap();

            let offset: u64 = if let Ok(offset) = call.offset.try_into() {
                offset
            } else {
                return Err("invalid offset in ArbAddressTable.Decompress".to_string());
            };

            if offset > call.buf.len() as u64 {
                return Err("invalid offset in ArbAddressTable.Decompress".to_string());
            }

            let data = &call.buf[offset as usize..];

            let (decompressed, new_offset) =
                try_state!(gas, context.arb_state(Some(&mut gas)).address_table().decompress(data));

            let output = ArbAddressTable::decompressCall::abi_encode_returns(
                &ArbAddressTable::decompressReturn::from((decompressed, U256::from(new_offset))),
            );
            return_success!(gas, Bytes::from(output));
        }
        ArbAddressTable::lookupCall::SELECTOR => {
            let call = ArbAddressTable::lookupCall::abi_decode(input).unwrap();
            let index = if let Some(index) =
                try_state!(gas, context.arb_state(Some(&mut gas)).address_table().lookup(call.addr))
            {
                index
            } else {
                return_revert!(gas, Bytes::from("address does not exist in AddressTable"));
            };

            let output = ArbAddressTable::lookupCall::abi_encode_returns(&U256::from(index));
            return_success!(gas, Bytes::from(output));
        }
        ArbAddressTable::lookupIndexCall::SELECTOR => {
            let call = ArbAddressTable::lookupIndexCall::abi_decode(input).unwrap();

            let index = if let Ok(index) = call.index.try_into() {
                index
            } else {
                return_revert!(gas, Bytes::from("invalid index in ArbAddressTable.LookupIndex"));
            };
            let addr = if let Some(addr) = try_state!(
                gas,
                context.arb_state(Some(&mut gas)).address_table().lookup_index(index)
            ) {
                addr
            } else {
                return_revert!(gas, Bytes::from("index does not exist in AddressTable"));
            };

            let output = ArbAddressTable::lookupIndexCall::abi_encode_returns(&addr);
            return_success!(gas, Bytes::from(output));
        }
        ArbAddressTable::registerCall::SELECTOR => {
            let call = ArbAddressTable::registerCall::abi_decode(input).unwrap();
            let index = try_state!(
                gas,
                context.arb_state(Some(&mut gas)).address_table().register(call.addr)
            );
            let output = ArbAddressTable::registerCall::abi_encode_returns(&U256::from(index));
            return_success!(gas, Bytes::from(output));
        }
        ArbAddressTable::sizeCall::SELECTOR => {
            let _ = ArbAddressTable::sizeCall::abi_decode(input).unwrap();
            let size = try_state!(gas, context.arb_state(Some(&mut gas)).address_table().size());
            let output = ArbAddressTable::sizeCall::abi_encode_returns(&U256::from(size));
            return_success!(gas, Bytes::from(output));
        }
        _ => return_revert!(gas, Bytes::from("Unknown function selector")),
    }
}
