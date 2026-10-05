//! EIP-2935 history storage helpers.

use alloy_primitives::{Address, B256, U256, keccak256};
use revm::{
    context::{ContextTr, JournalTr},
    inspector::JournalExt,
};

pub use alloy_eips::eip2935::{
    HISTORY_SERVE_WINDOW, HISTORY_STORAGE_ADDRESS, HISTORY_STORAGE_CODE,
};

/// Returns whether `address` is the EIP-2935 history storage contract.
#[inline]
pub fn is_history_storage_address(address: &Address) -> bool {
    *address == HISTORY_STORAGE_ADDRESS
}

/// Returns the history storage ring slot for `block_number`.
#[inline]
pub fn history_storage_slot(block_number: U256) -> U256 {
    block_number % U256::from(HISTORY_SERVE_WINDOW)
}

/// Encodes a block hash as the history contract storage value.
#[inline]
pub const fn history_storage_value(block_hash: B256) -> U256 {
    U256::from_be_bytes(block_hash.0)
}

/// Returns the first block in the valid EIP-2935 window for `current_block`.
#[inline]
pub fn history_window_start(current_block: U256) -> U256 {
    current_block.saturating_sub(U256::from(HISTORY_SERVE_WINDOW))
}

/// Returns the first block to backfill when rolling forward from `old_block` to `new_block`.
#[inline]
pub fn forward_fill_start(old_block: U256, new_block: U256) -> U256 {
    old_block.max(history_window_start(new_block))
}

/// Updates canonical EIP-2935 storage without changing account or slot warmth.
pub(crate) fn set_blockhash<CTX: ContextTr<Journal: JournalExt>>(
    context: &mut CTX,
    block_number: U256,
    block_hash: B256,
) -> eyre::Result<()> {
    let account_was_cold = context
        .journal_mut()
        .load_account(HISTORY_STORAGE_ADDRESS)
        .map_err(|error| eyre::eyre!("{error:?}"))?
        .is_cold;
    let account =
        context.journal_mut().evm_state().get(&HISTORY_STORAGE_ADDRESS).expect("account is loaded");
    if account.info.code_hash != keccak256(&HISTORY_STORAGE_CODE) {
        restore_cold_state(context, account_was_cold, None);
        return Ok(());
    }
    let slot = history_storage_slot(block_number);
    let slot_was_cold = context
        .journal_mut()
        .sstore(HISTORY_STORAGE_ADDRESS, slot, history_storage_value(block_hash))
        .map_err(|error| eyre::eyre!("failed to store EIP-2935 history slot: {error:?}"))?
        .is_cold;
    restore_cold_state(context, account_was_cold, Some((slot, slot_was_cold)));
    Ok(())
}

fn restore_cold_state<CTX: ContextTr<Journal: JournalExt>>(
    context: &mut CTX,
    account_was_cold: bool,
    slot_state: Option<(U256, bool)>,
) {
    let Some(account) = context.journal_mut().evm_state_mut().get_mut(&HISTORY_STORAGE_ADDRESS)
    else {
        return;
    };
    if account_was_cold {
        account.mark_cold();
    }
    if let Some((slot, slot_was_cold)) = slot_state
        && slot_was_cold
        && let Some(storage_slot) = account.storage.get_mut(&slot)
    {
        storage_slot.is_cold = true;
    }
}
