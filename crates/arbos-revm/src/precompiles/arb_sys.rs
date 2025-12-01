use alloy_sol_types::{SolCall, SolError, sol};
use revm::{
    interpreter::{Gas, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, B256, Bytes, FixedBytes, U256, address, fixed_bytes},
};

use crate::{
    ArbitrumContextTr,
    config::ArbitrumConfigTr,
    generate_state_mut_table,
    macros::{interpreter_return, interpreter_revert},
    precompile_impl,
    precompiles::{
        ArbPrecompileLogic, ExtendedPrecompile, StateMutability, decode_call, selector_or_revert,
    },
};

sol! {
///
/// @title System level functionality
/// @notice For use by contracts to interact with core L2-specific functionality.
/// Precompiled contract that exists in every Arbitrum chain at address(100), 0x0000000000000000000000000000000000000064.
///
interface ArbSys {
    ///
    /// @notice Get Arbitrum block number (distinct from L1 block number; Arbitrum genesis block has block number 0)
    /// @return block number as int
    ///
    function arbBlockNumber() external view returns (uint256);

    ///
    /// @notice Get Arbitrum block hash (reverts unless currentBlockNum-256 <= arbBlockNum < currentBlockNum)
    /// @return block hash
    ///
    function arbBlockHash(
        uint256 arbBlockNum
    ) external view returns (bytes32);

    ///
    /// @notice Gets the rollup's unique chain identifier
    /// @return Chain identifier as int
    ///
    function arbChainID() external view returns (uint256);

    ///
    /// @notice Get internal version number identifying an ArbOS build, this is `55 + nitroArbOS version number`
    /// e.g. on ArbOS 31 this would return 86. This is the only function that have the 55 offset.
    /// @return version number as int
    ///
    function arbOSVersion() external view returns (uint256);

    ///
    /// @notice Returns 0 since Nitro has no concept of storage gas
    /// @return uint 0
    ///
    function getStorageGasAvailable() external view returns (uint256);

    ///
    /// @notice (deprecated) check if current call is top level (meaning it was triggered by an EoA or a L1 contract)
    /// @dev this call has been deprecated and may be removed in a future release
    /// @return true if current execution frame is not a call by another L2 contract
    ///
    function isTopLevelCall() external view returns (bool);

    ///
    /// @notice map L1 sender contract address to its L2 alias
    /// @param sender sender address
    /// @param unused argument no longer used
    /// @return aliased sender address
    ///
    function mapL1SenderContractAddressToL2Alias(
        address sender,
        address unused
    ) external pure returns (address);

    ///
    /// @notice check if the caller (of this caller of this) is an aliased L1 contract address
    /// @return true iff the caller's address is an alias for an L1 contract address
    ///
    function wasMyCallersAddressAliased() external view returns (bool);

    ///
    /// @notice return the address of the caller (of this caller of this), without applying L1 contract address aliasing
    /// @return address of the caller's caller, without applying L1 contract address aliasing
    ///
    function myCallersAddressWithoutAliasing() external view returns (address);

    ///
    /// @notice Send given amount of Eth to dest from sender.
    /// This is a convenience function, which is equivalent to calling sendTxToL1 with empty data.
    /// @param destination recipient address on L1
    /// @return unique identifier for this L2-to-L1 transaction.
    ///
    function withdrawEth(
        address destination
    ) external payable returns (uint256);

    ///
    /// @notice Send a transaction to L1
    /// @dev it is not possible to execute on the L1 any L2-to-L1 transaction which contains data
    /// to a contract address without any code (as enforced by the Bridge contract).
    /// @param destination recipient address on L1
    /// @param data (optional) calldata for L1 contract call
    /// @return a unique identifier for this L2-to-L1 transaction.
    ///
    function sendTxToL1(
        address destination,
        bytes calldata data
    ) external payable returns (uint256);

    ///
    /// @notice Get send Merkle tree state
    /// @return size number of sends in the history
    /// @return root root hash of the send history
    /// @return partials hashes of partial subtrees in the send history tree
    ///
    function sendMerkleTreeState()
        external
        view
        returns (uint256 size, bytes32 root, bytes32[] memory partials);

    ///
    /// @notice creates a send txn from L2 to L1
    /// @param position = (level << 192) + leaf = (0 << 192) + leaf = leaf
    ///
    event L2ToL1Tx(
        address caller,
        address indexed destination,
        uint256 indexed hash,
        uint256 indexed position,
        uint256 arbBlockNum,
        uint256 ethBlockNum,
        uint256 timestamp,
        uint256 callvalue,
        bytes data
    );

    /// @dev DEPRECATED in favour of the new L2ToL1Tx event above after the nitro upgrade
    event L2ToL1Transaction(
        address caller,
        address indexed destination,
        uint256 indexed uniqueId,
        uint256 indexed batchNumber,
        uint256 indexInBatch,
        uint256 arbBlockNum,
        uint256 ethBlockNum,
        uint256 timestamp,
        uint256 callvalue,
        bytes data
    );

    ///
    /// @notice logs a merkle branch for proof synthesis
    /// @param reserved an index meant only to align the 4th index with L2ToL1Transaction's 4th event
    /// @param hash the merkle hash
    /// @param position = (level << 192) + leaf
    ///
    event SendMerkleUpdate(
        uint256 indexed reserved, bytes32 indexed hash, uint256 indexed position
    );

    error InvalidBlockNumber(uint256 requested, uint256 current);
}
}

pub fn arb_sys_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbSys")),
        address!("0x0000000000000000000000000000000000000064"),
        precompile_impl!(ArbSysPrecompile),
    )
}
struct ArbSysPrecompile;

impl<CTX: ArbitrumContextTr> ArbPrecompileLogic<CTX> for ArbSysPrecompile {
    const STATE_MUT_TABLE: &'static [([u8; 4], StateMutability)] = generate_state_mut_table! {
        ArbSys => {
            arbBlockNumberCall(View),
            arbBlockHashCall(View),
            arbChainIDCall(View),
            arbOSVersionCall(View),
            getStorageGasAvailableCall(View),
            isTopLevelCallCall(View),
            mapL1SenderContractAddressToL2AliasCall(Pure),
            wasMyCallersAddressAliasedCall(View),
            myCallersAddressWithoutAliasingCall(View),
            withdrawEthCall(Payable),
            sendTxToL1Call(Payable),
            sendMerkleTreeStateCall(View),
        }
    };

    fn inner(
        context: &mut CTX,
        input: &[u8],
        _target_address: &Address,
        _caller_address: Address,
        _call_value: U256,
        _is_static: bool,
        gas_limit: u64,
    ) -> Option<InterpreterResult> {
        let mut gas = Gas::new(gas_limit);

        let selector = selector_or_revert!(gas, input);

        match selector {
            ArbSys::arbBlockNumberCall::SELECTOR => {
                let output =
                    ArbSys::arbBlockNumberCall::abi_encode_returns(&context.block_number());

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbSys::arbChainIDCall::SELECTOR => {
                let output = ArbSys::arbChainIDCall::abi_encode_returns(&context.chain_id());

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbSys::arbOSVersionCall::SELECTOR => {
                let output = ArbSys::arbOSVersionCall::abi_encode_returns(&U256::from(
                    context.cfg().arbos_version() + 55,
                ));

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbSys::arbBlockHashCall::SELECTOR => {
                let call = decode_call!(gas, ArbSys::arbBlockHashCall, input);

                let current_block = context.block_number().saturating_to::<u64>();
                let requested_block: u64 = call.arbBlockNum.saturating_to();

                if requested_block >= current_block || requested_block + 256 < current_block {
                    if context.cfg().arbos_version() >= 33 {
                        interpreter_revert!(
                            gas,
                            ArbSys::InvalidBlockNumber {
                                requested: call.arbBlockNum,
                                current: U256::from(current_block),
                            }
                            .abi_encode()
                        );
                    }

                    interpreter_return!(gas, Bytes::from("invalid block number for ArbBlockHAsh"));
                }

                let hash = context.block_hash(requested_block).unwrap_or_default();

                let output = ArbSys::arbBlockHashCall::abi_encode_returns(&hash);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbSys::getStorageGasAvailableCall::SELECTOR => {
                let output = ArbSys::getStorageGasAvailableCall::abi_encode_returns(&U256::ZERO);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbSys::isTopLevelCallCall::SELECTOR => {
                let output = ArbSys::isTopLevelCallCall::abi_encode_returns(&false);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbSys::mapL1SenderContractAddressToL2AliasCall::SELECTOR => {
                let call =
                    decode_call!(gas, ArbSys::mapL1SenderContractAddressToL2AliasCall, input);

                let aliased_address = remap_l1_address(&call.sender);

                let output = ArbSys::mapL1SenderContractAddressToL2AliasCall::abi_encode_returns(
                    &aliased_address,
                );

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbSys::wasMyCallersAddressAliasedCall::SELECTOR => {
                let output = ArbSys::wasMyCallersAddressAliasedCall::abi_encode_returns(&false);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbSys::myCallersAddressWithoutAliasingCall::SELECTOR => {
                let address = Address::ZERO;
                let output =
                    ArbSys::myCallersAddressWithoutAliasingCall::abi_encode_returns(&address);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbSys::sendTxToL1Call::SELECTOR => {
                let output = ArbSys::sendTxToL1Call::abi_encode_returns(&U256::ONE);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbSys::withdrawEthCall::SELECTOR => {
                let output = ArbSys::withdrawEthCall::abi_encode_returns(&U256::ONE);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbSys::sendMerkleTreeStateCall::SELECTOR => {
                let output = ArbSys::sendMerkleTreeStateCall::abi_encode_returns(
                    &ArbSys::sendMerkleTreeStateReturn {
                        size: U256::ZERO,
                        root: B256::ZERO,
                        partials: vec![],
                    },
                );

                interpreter_return!(gas, Bytes::from(output));
            }
            _ => interpreter_revert!(gas, Bytes::from("Unknown function selector")),
        }
    }
}

const ADDRESS_ALIAS_OFFSET: FixedBytes<32> =
    fixed_bytes!("0x0000000000000000000000001111000000000000000000000000000000001111");

fn remap_l1_address(l1_addr: &Address) -> Address {
    let mut sum: U256 = U256::from_be_bytes(B256::left_padding_from(l1_addr.as_slice()).0);
    sum = sum.saturating_add(U256::from_be_bytes(ADDRESS_ALIAS_OFFSET.0));
    let sum_bytes: [u8; 32] = sum.to_be_bytes();
    let aliased_bytes = &sum_bytes[12..32];
    Address::from_slice(aliased_bytes)
}
