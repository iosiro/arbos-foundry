use foundry_config::fs_permissions::PathPermission;
use foundry_test_utils::{str, util::OTHER_SOLC_VERSION};

forgetest_init!(stylus_host_inspection, |prj, cmd| {
    prj.update_config(|config| {
        config.solc = Some(OTHER_SOLC_VERSION.into());
        config.fs_permissions.add(PathPermission::read("."));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/fixtures/Stylus/foundry_stylus_inspector.wasm");
    std::fs::copy(fixture, prj.root().join("inspector.wasm")).unwrap();
    prj.add_test("StylusInspector.t.sol", include_str!("../../fixtures/StylusInspector.t.sol"));
    for isolate in [false, true] {
        prj.update_config(|config| config.isolate = isolate);
        cmd.forge_fuse()
            .args(["test", "--arbos-version", "61", "--mc", "StylusInspectorTest", "-vvvv"])
            .assert_success();
    }
});

forgetest_init!(stylus_event_inspection, |prj, cmd| {
    prj.update_config(|config| {
        config.solc = Some(OTHER_SOLC_VERSION.into());
        config.fs_permissions.add(PathPermission::read("."));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/fixtures/Stylus/foundry_stylus_events.wasm");
    std::fs::copy(fixture, prj.root().join("events.wasm")).unwrap();
    prj.add_test("StylusEvents.t.sol", include_str!("../../fixtures/StylusEvents.t.sol"));
    for isolate in [false, true] {
        prj.update_config(|config| config.isolate = isolate);
        cmd.forge_fuse()
            .args([
                "test",
                "--arbos-version",
                "61",
                "--mc",
                "StylusEventsTest",
                "--no-match-test",
                "testReject",
            ])
            .assert_success();
        for name in
            ["testRejectZeroCount", "testRejectZeroCountDelegate", "testRejectPrecompileZeroCount"]
        {
            cmd.forge_fuse()
                .args(["test", "--arbos-version", "61", "--mt", &format!(r"^{name}\(\)$")])
                .assert_failure()
                .stdout_eq(str![[r#"
...
[FAIL: log emitted 1 time, expected 0] [..]
...
"#]]);
        }
        cmd.forge_fuse()
            .args(["test", "--arbos-version", "61", "--mt", "testRejectAnonymousTemplate"])
            .assert_failure()
            .stdout_eq(str![[r#"
...
[FAIL: use vm.expectEmitAnonymous to match anonymous events] [..]
...
"#]]);
        cmd.forge_fuse()
            .args(["test", "--arbos-version", "61", "--mt", "testRejectWrongData"])
            .assert_failure()
            .stdout_eq(str![[r#"
...
[FAIL: [..]] testRejectWrongData() [..]
...
"#]]);
    }
});
