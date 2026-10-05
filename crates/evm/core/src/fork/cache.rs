//! Disk paths for fork caches shared by execution backends.

use foundry_fork_db::cache::BlockHashMode;
use std::path::PathBuf;

/// Returns a cache file within a block's directory, separated by hash domain.
///
/// Older Forge builds used the block directory itself as a regular file. Keep that file intact
/// and use a sibling `.cache` directory when it prevents creating the canonical directory.
pub fn fork_cache_file(block_dir: PathBuf, file_name: &str, mode: BlockHashMode) -> PathBuf {
    let directory = if block_dir.is_file() { block_dir.with_extension("cache") } else { block_dir };
    match mode {
        BlockHashMode::Evm => directory.join(file_name),
        BlockHashMode::Rpc => directory.join(format!("rpc-{file_name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{B256, U256};
    use foundry_fork_db::{BlockchainDb, cache::BlockchainDbMeta};
    use revm::context::BlockEnv;

    #[test]
    fn fork_cache_files_keep_tool_and_hash_domains_separate() {
        let temp = tempfile::tempdir().unwrap();
        let block_dir = temp.path().join("42");
        let mut caches = Vec::new();
        for file_name in ["storage.json", "storage-endpoint.json"] {
            for mode in [BlockHashMode::Evm, BlockHashMode::Rpc] {
                let path = fork_cache_file(block_dir.clone(), file_name, mode);
                assert_eq!(path.parent(), Some(block_dir.as_path()));
                assert!(!caches.iter().any(|(_, previous)| previous == &path));
                let meta = BlockchainDbMeta::<BlockEnv>::default().with_block_hash_mode(mode);
                let db = BlockchainDb::new(meta.clone(), Some(path.clone()));
                let hash = B256::with_last_byte(caches.len() as u8 + 1);
                db.block_hashes().write().insert(U256::from(41), hash);
                db.cache().flush();
                assert!(path.is_file());
                caches.push(((meta, hash), path));
            }
        }
        for ((meta, hash), path) in caches {
            let db = BlockchainDb::new(meta, Some(path));
            assert_eq!(db.block_hashes().read().get(&U256::from(41)), Some(&hash));
        }
    }

    #[test]
    fn fork_cache_files_preserve_legacy_flat_cache() {
        let temp = tempfile::tempdir().unwrap();
        let block_dir = temp.path().join("42");
        std::fs::write(&block_dir, b"legacy cache").unwrap();
        for mode in [BlockHashMode::Evm, BlockHashMode::Rpc] {
            let path = fork_cache_file(block_dir.clone(), "storage.json", mode);
            assert_ne!(path, block_dir);
            let meta = BlockchainDbMeta::<BlockEnv>::default().with_block_hash_mode(mode);
            let db = BlockchainDb::new(meta.clone(), Some(path.clone()));
            db.block_hashes().write().insert(U256::from(41), B256::with_last_byte(42));
            db.cache().flush();
            assert!(path.is_file());
            let reloaded = BlockchainDb::new(meta, Some(path.clone()));
            assert_eq!(
                reloaded.block_hashes().read().get(&U256::from(41)),
                Some(&B256::with_last_byte(42))
            );
            assert_eq!(fork_cache_file(block_dir.clone(), "storage.json", mode), path);
        }
        assert_eq!(std::fs::read(block_dir).unwrap(), b"legacy cache");
    }
}
