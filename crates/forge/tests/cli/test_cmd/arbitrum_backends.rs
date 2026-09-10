use alloy_provider::Provider;
use foundry_config::{RpcEndpointUrl, RpcEndpoints, fs_permissions::PathPermission};
use foundry_test_utils::util::OTHER_SOLC_VERSION;

forgetest!(arbitrum_blockhash_cheatcodes_keep_l1_and_l2_separate, |prj, cmd| {
    foundry_test_utils::util::initialize(prj.root());
    prj.add_test(
        "BlockhashDomains.t.sol",
        r#"
pragma solidity >=0.8.20;
import {Test} from "forge-std/Test.sol";

interface ArbSysHashes { function arbBlockHash(uint256 number) external view returns (bytes32); }

contract ArbitrumBlockhashDomainsTest is Test {
    function hash(uint256 number) external view returns (bytes32) { return blockhash(number); }

    function test_roll_and_explicit_hash_preserve_l2_and_snapshot_state() public {
        vm.roll(300);
        bytes32 original = keccak256("299");
        assertEq(this.hash(299), original);
        vm.setBlockhash(299, bytes32(uint256(42)));
        assertEq(this.hash(299), bytes32(uint256(42)));
        assertEq(ArbSysHashes(address(0x64)).arbBlockHash(299), original, "L1 override polluted L2 history");
        uint256 snapshot = vm.snapshotState();
        vm.setBlockhash(299, bytes32(uint256(99)));
        vm.roll(301);
        assertEq(this.hash(299), bytes32(uint256(99)));
        assertTrue(vm.revertToState(snapshot));
        assertEq(block.number, 300);
        assertEq(this.hash(299), bytes32(uint256(42)));
        vm.roll(600);
        assertEq(this.hash(299), bytes32(0), "expired override remained visible");
        vm.roll(300);
        assertEq(this.hash(299), bytes32(uint256(42)), "backward roll lost override");
    }

    function test_current_hash_override_does_not_alias_oldest_block() public {
        vm.roll(300);
        vm.setBlockhash(44, bytes32(uint256(44)));
        vm.setBlockhash(300, bytes32(uint256(300)));
        assertEq(this.hash(44), bytes32(uint256(44)));
        assertEq(this.hash(300), bytes32(0));
        vm.roll(301);
        assertEq(this.hash(44), bytes32(0));
        assertEq(this.hash(300), bytes32(uint256(300)));
    }
}
"#,
    );
    cmd.args(["test", "--arbos-version", "61", "--mc", "ArbitrumBlockhashDomainsTest", "-vvvv"])
        .assert_success();
    cmd.arg("--isolate").assert_success();
});

forgetest_async!(arbitrum_fork_blockhash_overrides_follow_lifecycle, |prj, cmd| {
    foundry_test_utils::util::initialize(prj.root());
    let (_api, handle) = anvil::spawn({
        let mut node = anvil::NodeConfig::test().with_chain_id(Some(421_614_u64));
        node.stylus_config.arbos_version = Some(61);
        node
    })
    .await;
    prj.update_config(|config| {
        // These independent local chains intentionally share a chain ID and height.
        config.no_storage_caching = true;
        config.rpc_endpoints =
            RpcEndpoints::new([("source", RpcEndpointUrl::Url(handle.http_endpoint()))]);
    });
    prj.add_test(
        "ForkHashOverrides.t.sol",
        r#"
pragma solidity >=0.8.20;
import {Test} from "forge-std/Test.sol";

contract ArbitrumForkHashOverridesTest is Test {
    function hash(uint256 number) external view returns (bytes32) { return blockhash(number); }

    function test_fork_overrides_restore_and_roll_independently() public {
        uint256 first = vm.createSelectFork("source");
        vm.roll(300);
        vm.setBlockhash(299, bytes32(uint256(42)));
        uint256 second = vm.createSelectFork("source");
        vm.roll(300);
        assertEq(this.hash(299), bytes32(0), "first fork override leaked");
        vm.setBlockhash(299, bytes32(uint256(99)));
        vm.selectFork(first);
        assertEq(this.hash(299), bytes32(uint256(42)));
        uint256 snapshot = vm.snapshotState();
        vm.selectFork(second);
        assertEq(this.hash(299), bytes32(uint256(99)));
        vm.rollFork(first, uint256(0));
        assertEq(vm.activeFork(), second);
        assertEq(this.hash(299), bytes32(uint256(99)), "inactive roll changed active override");
        vm.selectFork(first);
        vm.roll(300);
        assertEq(this.hash(299), bytes32(0), "rolled fork retained override");
        assertTrue(vm.revertToState(snapshot));
        assertEq(vm.activeFork(), first);
        assertEq(this.hash(299), bytes32(uint256(42)), "snapshot lost fork-local override");
        vm.rollFork(uint256(0));
        vm.roll(300);
        assertEq(this.hash(299), bytes32(0), "active roll retained override");
    }
}
"#,
    );
    cmd.args(["test", "--arbos-version", "61", "--mc", "ArbitrumForkHashOverridesTest", "-vvvv"])
        .assert_success();
    cmd.arg("--isolate").assert_success();
});

forgetest_init!(test_stylus_block_cache_gas, |prj, cmd| {
    prj.update_config(|config| {
        config.solc = Some(OTHER_SOLC_VERSION.into());
        config.fs_permissions.add(PathPermission::read("."));
    });
    let fixtures =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/fixtures/Stylus");
    let wat = std::fs::read_to_string(fixtures.join("foundry_stylus_program.wat")).unwrap();
    for (name, wat) in [
        ("foundry_stylus_program.wasm", wat.clone()),
        ("foundry_stylus_debug.wasm", {
            let mut distinct = wat.clone();
            distinct.insert_str(distinct.rfind(')').unwrap(), "(func (export \"distinct\"))");
            distinct
        }),
    ] {
        std::fs::write(prj.root().join(name), arbos_revm::utils::wat2wasm(wat.as_bytes()).unwrap())
            .unwrap();
    }
    prj.add_test(
        "BlockCache.t.sol",
        r#"
pragma solidity >=0.8.20;
import {Test} from "forge-std/Test.sol";

interface StylusCode { function getStylusCode(string calldata) external view returns (bytes memory); }

contract BlockCacheTest is Test {
    address constant A = address(0xaaa);
    address constant B = address(0xbbb);

    function callGas(address target) internal returns (uint256 used) {
        uint256 beforeGas = gasleft();
        (bool ok, bytes memory output) = target.call(hex"deadbeef");
        used = beforeGas - gasleft();
        assertTrue(ok);
        assertEq(output, hex"deadbeef");
    }

    function test_eviction_and_block_lifetime() public {
        vm.etch(A, StylusCode(address(vm)).getStylusCode("foundry_stylus_program.wasm"));
        vm.etch(B, StylusCode(address(vm)).getStylusCode("foundry_stylus_debug.wasm"));
        // Activate both programs before measuring their initialization gas.
        callGas(A);
        callGas(B);
        vm.roll(block.number + 1);
        uint256 cold = callGas(A);
        callGas(B);
        uint256 evicted = callGas(A);
        uint256 warm = callGas(A);
        assertApproxEqAbs(cold, evicted, 64, "size zero and one must evict A after B");
        assertGt(evicted, warm + 1000, "same-block calls must share the recent cache");
        vm.roll(block.number + 1);
        uint256 nextBlock = callGas(A);
        assertApproxEqAbs(nextBlock, cold, 64, "new block must clear the cache");
    }
}
"#,
    );
    for isolated in [false, true] {
        prj.update_config(|config| config.isolate = isolated);
        for capacity in ["0", "1"] {
            cmd.forge_fuse()
                .args([
                    "test",
                    "--arbos-version",
                    "61",
                    "--stylus-disable-auto-cache",
                    "--stylus-debug",
                    "--stylus-block-cache-size",
                    capacity,
                    "--mc",
                    "BlockCacheTest",
                    "-vvvv",
                ])
                .assert_success();
        }
    }
});

forgetest_async!(
    arbitrum_fork_arbsys_reads_rpc_hashes_without_changing_l1_blockhash,
    |prj, cmd| {
        foundry_test_utils::util::initialize(prj.root());
        let (api, handle) = anvil::spawn({
            let mut node = anvil::NodeConfig::test().with_chain_id(Some(421_614_u64));
            node.stylus_config.arbos_version = Some(61);
            node
        })
        .await;
        for _ in 0..3 {
            api.mine_one().await;
        }
        let block = handle.http_provider().get_block_by_number(2.into()).await.unwrap().unwrap();
        let parent = block.header.hash;
        let grandparent = block.header.parent_hash;
        // L2 is ahead of L1, as on Arbitrum. An L1-keyed fork anchor would reject L2 parent hashes.
        let endpoint = foundry_test_utils::rpc::spawn_rpc_proxy_with_l1_block_number(
            handle.http_endpoint(),
            1,
        )
        .await;
        prj.update_config(|config| {
            // These independent local chains intentionally share a chain ID and height.
            config.no_storage_caching = true;
            config.rpc_endpoints = RpcEndpoints::new([("source", RpcEndpointUrl::Url(endpoint))]);
        });
        prj.add_test(
            "ForkHashDomains.t.sol",
            &r#"
pragma solidity >=0.8.20;
import {Test} from "forge-std/Test.sol";
interface ArbSysHashes {
    function arbBlockHash(uint256 number) external view returns (bytes32);
    function arbBlockNumber() external view returns (uint256);
}

contract ArbitrumForkHashDomainsTest is Test {
    function hash(uint256 number) external view returns (bytes32) { return blockhash(number); }
    function number() external view returns (uint256) { return block.number; }

    function test_rpc_hashes_survive_fork_roll_and_l1_overrides() public {
        vm.createSelectFork("source", uint256(3));
        assertEq(this.number(), 1, "execution must use the L1 number");
        assertEq(ArbSysHashes(address(0x64)).arbBlockNumber(), 3, "ArbSys must use the L2 number");
        assertEq(this.hash(0), bytes32(0), "L1 history is empty");
        assertEq(this.hash(2), bytes32(0), "L2 hash leaked into L1 BLOCKHASH");
        assertEq(ArbSysHashes(address(0x64)).arbBlockHash(2), <parent>);
        vm.roll(300);
        vm.setBlockhash(299, bytes32(uint256(42)));
        assertEq(this.hash(299), bytes32(uint256(42)));
        assertEq(ArbSysHashes(address(0x64)).arbBlockHash(2), <parent>);
        uint256 snapshot = vm.snapshotState();
        vm.rollFork(uint256(2));
        assertEq(this.number(), 1, "fork roll must preserve L1/L2 separation");
        assertEq(ArbSysHashes(address(0x64)).arbBlockNumber(), 2);
        assertEq(ArbSysHashes(address(0x64)).arbBlockHash(1), <grandparent>);
        assertEq(this.hash(1), bytes32(0));
        assertTrue(vm.revertToState(snapshot));
        assertEq(ArbSysHashes(address(0x64)).arbBlockHash(2), <parent>);
        assertEq(this.hash(299), bytes32(uint256(42)));
    }
}
"#
            .replace("<parent>", &format!("bytes32({parent})"))
            .replace("<grandparent>", &format!("bytes32({grandparent})")),
        );
        cmd.args(["test", "--arbos-version", "61", "--mc", "ArbitrumForkHashDomainsTest", "-vvvv"])
            .assert_success();
        cmd.arg("--isolate").assert_success();
    }
);

forgetest_init!(arbitrum_rejects_blob_basefee, |prj, cmd| {
    prj.add_test(
        "BlobBasefee.t.sol",
        r#"
pragma solidity >=0.8.20;
import {Test} from "forge-std/Test.sol";

contract ArbitrumBlobBasefeeTest is Test {
    function test_blob_basefee_halts_nested_execution() public {
        address target = address(0x123456);
        vm.etch(target, hex"4a60005260206000f3");
        (bool success, bytes memory output) = target.staticcall{gas: 100_000}("");
        assertFalse(success, "Nitro rejects BLOBBASEFEE even after Cancun");
        assertEq(output.length, 0, "an exceptional halt must not return a blob fee");
    }
}
"#,
    );
    cmd.args(["test", "--arbos-version", "61", "--mc", "ArbitrumBlobBasefeeTest", "-vvvv"])
        .assert_success();
    cmd.arg("--isolate").assert_success();
});

forgetest_async!(arbitrum_fork_selects_opcode_rules_from_source_state, |prj, cmd| {
    foundry_test_utils::util::initialize(prj.root());
    let remote = alloy_primitives::address!("0000000000000000000000000000000000123456");
    let mut handles = Vec::new();
    for version in [20, 50] {
        let mut node = anvil::NodeConfig::test().with_chain_id(Some(421_614_u64));
        node.stylus_config.arbos_version = Some(version);
        let (api, handle) = anvil::spawn(node).await;
        // CLZ(0) returns 256 at ArbOS 50 and halts under older opcode rules.
        api.anvil_set_code(remote, alloy_primitives::hex!("60001e60005260206000f3").into())
            .await
            .unwrap();
        let version_slot = arbos_revm::state::types::map_address(
            &alloy_primitives::B256::ZERO,
            &alloy_primitives::B256::ZERO,
        );
        assert_eq!(
            handle
                .http_provider()
                .get_storage_at(
                    arbos_revm::constants::ARBOS_STATE_ADDRESS,
                    alloy_primitives::U256::from_be_bytes(version_slot.0),
                )
                .await
                .unwrap(),
            alloy_primitives::U256::from(version),
        );
        handles.push(handle);
    }
    prj.update_config(|config| {
        config.solc = Some(OTHER_SOLC_VERSION.into());
        // These independent local chains intentionally share a chain ID and height.
        config.no_storage_caching = true;
        config.rpc_endpoints = RpcEndpoints::new([
            ("older", RpcEndpointUrl::Url(handles[0].http_endpoint())),
            ("newer", RpcEndpointUrl::Url(handles[1].http_endpoint())),
        ]);
    });
    prj.add_test(
        "ForkOpcodeRules.t.sol",
        r#"
pragma solidity >=0.8.20;
import {Test} from "forge-std/Test.sol";

contract ArbitrumForkOpcodeRulesTest is Test {
    function assertOpcode(bool enabled) external view {
        (bool ok, bytes memory output) = address(0x123456).staticcall{gas: 100_000}("");
        assertEq(ok, enabled, "CLZ activation disagrees with persisted ArbOS version");
        if (enabled) assertEq(abi.decode(output, (uint256)), 256);
        else assertEq(output.length, 0);
        // Empty input is invalid for BLS G1ADD, but succeeds at an unassigned address.
        (ok, output) = address(0x0b).staticcall{gas: 100_000}("");
        assertEq(ok, !enabled, "BLS precompile activation disagrees with selected fork");
        assertEq(output.length, 0);
    }

    function test_opcode_rules_follow_fork_selection_and_snapshot() public {
        uint256 older = vm.createSelectFork("older");
        assertEq(uint256(vm.load(address(0xA4b05FffffFffFFFFfFFfffFfffFFfffFfFfFFFf),
            bytes32(uint256(keccak256(new bytes(31))) & ~uint256(255)))), 20, "fork must load persisted ArbOS version");
        this.assertOpcode(false);
        uint256 snapshot = vm.snapshotState();
        uint256 newer = vm.createSelectFork("newer");
        this.assertOpcode(true);
        vm.selectFork(older);
        this.assertOpcode(false);
        vm.selectFork(newer);
        this.assertOpcode(true);
        assertTrue(vm.revertToState(snapshot));
        assertEq(vm.activeFork(), older);
        this.assertOpcode(false);
    }
}
"#,
    );
    cmd.args(["test", "--arbos-version", "61", "--mc", "ArbitrumForkOpcodeRulesTest", "-vvvv"])
        .assert_success();
});
