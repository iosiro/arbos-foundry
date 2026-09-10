//! Shared cache layout for Forge and Anvil forks.

use std::path::PathBuf;

/// Returns a file in a block's cache directory, preserving legacy flat cache files.
pub fn fork_cache_file(block_dir: PathBuf, file_name: &str) -> PathBuf {
    let directory = if block_dir.is_file() { block_dir.with_extension("cache") } else { block_dir };
    directory.join(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{B256, U256};
    use foundry_fork_db::{BlockchainDb, cache::BlockchainDbMeta};

    #[test]
    fn fork_cache_flushes_and_reloads_both_layouts() {
        let temp = tempfile::tempdir().unwrap();
        for legacy in [false, true] {
            let directory = temp.path().join(if legacy { "42" } else { "43" });
            if legacy {
                std::fs::write(&directory, b"legacy cache").unwrap();
            }
            let path = fork_cache_file(directory.clone(), "storage.json");
            let meta = BlockchainDbMeta::default();
            let db = BlockchainDb::new(meta.clone(), Some(path.clone()));
            db.block_hashes().write().insert(U256::from(41), B256::repeat_byte(42));
            db.cache().flush();
            assert!(path.is_file());
            let reloaded = BlockchainDb::new(meta, Some(path));
            assert_eq!(
                reloaded.block_hashes().read().get(&U256::from(41)),
                Some(&B256::repeat_byte(42))
            );
            if legacy {
                assert_eq!(std::fs::read(directory).unwrap(), b"legacy cache");
            }
        }
    }

    #[test]
    fn fork_cache_preserves_legacy_file() {
        let temp = tempfile::tempdir().unwrap();
        let block = temp.path().join("42");
        std::fs::write(&block, b"legacy cache").unwrap();
        let path = fork_cache_file(block.clone(), "storage.json");
        assert_eq!(path, temp.path().join("42.cache/storage.json"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"new cache").unwrap();
        assert_eq!(std::fs::read(block).unwrap(), b"legacy cache");
        assert_eq!(std::fs::read(path).unwrap(), b"new cache");
    }
}
