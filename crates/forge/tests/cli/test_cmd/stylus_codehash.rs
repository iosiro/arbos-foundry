use alloy_primitives::{U256, address, hex};
use foundry_config::fs_permissions::PathPermission;
use foundry_test_utils::util::OTHER_SOLC_VERSION;

forgetest_init!(stylus_codehash_matches_account_existence, |prj, cmd| {
    prj.update_config(|config| {
        config.solc = Some(OTHER_SOLC_VERSION.into());
        config.fs_permissions.add(PathPermission::read("."));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/fixtures/Stylus/foundry_stylus_codehash.wasm");
    std::fs::copy(fixture, prj.root().join("codehash.wasm")).unwrap();
    prj.add_test("StylusCodehash.t.sol", include_str!("../../fixtures/StylusCodehash.t.sol"));
    for isolate in [false, true] {
        prj.update_config(|config| config.isolate = isolate);
        cmd.forge_fuse()
            .args(["test", "--arbos-version", "61", "--mc", "StylusCodehashTest", "-vvvv"])
            .assert_success();
    }
});

forgetest_async!(stylus_codehash_fork_matches_account_existence, |prj, cmd| {
    foundry_test_utils::util::initialize(prj.root());
    let mut node = anvil::NodeConfig::test().with_chain_id(Some(421_614_u64));
    node.stylus_config.arbos_version = Some(61);
    let (api, handle) = anvil::spawn(node).await;
    api.anvil_set_balance(address!("0000000000000000000000000000000000010001"), U256::from(1))
        .await
        .unwrap();
    api.anvil_set_nonce(address!("0000000000000000000000000000000000010002"), U256::from(1))
        .await
        .unwrap();
    api.anvil_set_code(
        address!("0000000000000000000000000000000000010003"),
        hex!("60006000f3").into(),
    )
    .await
    .unwrap();
    prj.update_config(|config| {
        config.solc = Some(OTHER_SOLC_VERSION.into());
        config.no_storage_caching = true;
        config.fs_permissions.add(PathPermission::read("."));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/fixtures/Stylus/foundry_stylus_codehash.wasm");
    std::fs::copy(fixture, prj.root().join("codehash.wasm")).unwrap();
    prj.add_test("StylusCodehash.t.sol", include_str!("../../fixtures/StylusCodehash.t.sol"));
    for isolate in [false, true] {
        prj.update_config(|config| config.isolate = isolate);
        cmd.forge_fuse()
            .args(["test", "--arbos-version", "61", "--mc", "StylusForkCodehashTest", "-vvvv"])
            .args(["--fork-url", &handle.http_endpoint()])
            .assert_success();
    }
});
