//! Filesystem tests.

use crate::{
    config::*,
    test_helpers::{RE_PATH_SEPARATOR, TEST_DATA_DEFAULT},
};
use foundry_config::{FsPermissions, GasLimit, fs_permissions::PathPermission};
use foundry_test_utils::Filter;

#[tokio::test(flavor = "multi_thread")]
async fn test_stylus_hostio() {
    let runner = TEST_DATA_DEFAULT.runner_with(|config| {
        config.fs_permissions = FsPermissions::new(vec![PathPermission::read("./fixtures")]);
        config.isolate = true;
        config.gas_limit = GasLimit(100_000_000);
    });
    let filter = Filter::new(".*", ".*", &format!(".*stylus{RE_PATH_SEPARATOR}*"));
    TestConfig::with_filter(runner, filter).run().await;
}
