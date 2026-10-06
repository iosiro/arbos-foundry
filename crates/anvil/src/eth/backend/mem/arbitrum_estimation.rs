//! Nitro-derived L1 poster-gas estimation for unsigned RPC requests.

use alloy_consensus::{TxEip1559, TxEip7702, transaction::RlpEcdsaEncodableTx};
use alloy_evm::Database;
use alloy_primitives::{Bytes, TxKind, U256, keccak256};
use alloy_rlp::Encodable;
use alloy_rpc_types::TransactionRequest;
use arbos_revm::{
    ArbitrumContext,
    l1_fee::{calculate_poster_gas, compressed_data_units},
    state::{ArbState, ArbStateGetter, types::StorageBackedTr},
};

/// Uses Nitro's envelope and price padding, adapted to Anvil's configurable L2 base fee.
pub(super) fn poster_gas<DB: Database>(
    context: &mut ArbitrumContext<DB>,
    request: &TransactionRequest,
) -> Result<u64, String> {
    if context.block.basefee == 0 {
        return Ok(0);
    }
    let (price_per_unit, compression_level) = {
        let mut state = context.arb_state(None, false);
        (
            state.l1_pricing().price_per_unit().get().map_err(|err| err.to_string())?,
            state.brotli_compression_level().get().map_err(|err| err.to_string())?,
        )
    };
    let units = compressed_data_units(&fake_envelope(request), compression_level)?;
    // The signature and some fields are unknown: add 16 bytes, then another 1%.
    let units = units.saturating_add(16 * 16).saturating_mul(10_100) / 10_000;
    // Account for a 10% L1 price increase and a 1/8 L2 price decrease before inclusion.
    let poster_cost =
        price_per_unit.saturating_mul(U256::from(units)).saturating_mul(U256::from(11_000))
            / U256::from(10_000);
    // Nitro floors this at its minimum base fee. Anvil can set or decay below that minimum,
    // so flooring here would underestimate the gas actually charged by local mining.
    let gas_price =
        (U256::from(context.block.basefee) * U256::from(7) / U256::from(8)).max(U256::from(1));
    Ok(calculate_poster_gas(poster_cost, gas_price))
}

/// Nitro's fake DynamicFeeTx is deliberately invalid; only its compressed size is used.
/// Its random gas limit stays fixed throughout the estimator's binary search.
/// Requests carrying authorizations use the corresponding envelope to retain those bytes.
fn fake_envelope(request: &TransactionRequest) -> Bytes {
    let nonce = request
        .nonce
        .filter(|nonce| *nonce != 0)
        .unwrap_or_else(|| u64::from_be_bytes(keccak256("Nonce")[..8].try_into().unwrap()));
    let random_u32 = |label: &str| u32::from_be_bytes(keccak256(label)[..4].try_into().unwrap());
    let tx = TxEip1559 {
        chain_id: 0,
        nonce,
        gas_limit: u64::from(random_u32("Gas")),
        max_fee_per_gas: request
            .max_fee_per_gas
            .or(request.gas_price)
            .filter(|fee| *fee != 0)
            .unwrap_or_else(|| u128::from(random_u32("GasFeeCap"))),
        max_priority_fee_per_gas: request
            .max_priority_fee_per_gas
            .or(request.gas_price)
            .filter(|fee| *fee != 0)
            .unwrap_or_else(|| u128::from(random_u32("GasTipCap"))),
        to: request.to.unwrap_or(TxKind::Create),
        value: request.value.unwrap_or_default(),
        access_list: request.access_list.clone().unwrap_or_default(),
        input: request.input.input().cloned().unwrap_or_default(),
    };
    if let Some(authorization_list) = &request.authorization_list {
        let tx = TxEip7702 {
            chain_id: tx.chain_id,
            nonce: tx.nonce,
            gas_limit: tx.gas_limit,
            max_fee_per_gas: tx.max_fee_per_gas,
            max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
            to: tx.to.to().copied().unwrap_or_default(),
            value: tx.value,
            access_list: tx.access_list,
            authorization_list: authorization_list.clone(),
            input: tx.input,
        };
        return encode_fake_signed(&tx, 4);
    }
    encode_fake_signed(&tx, 2)
}

fn encode_fake_signed(tx: &impl RlpEcdsaEncodableTx, transaction_type: u8) -> Bytes {
    let v = 42_161_u64 * 3;
    let r = U256::from_be_bytes(keccak256("R").0);
    let s = U256::from_be_bytes(keccak256("S").0);
    let mut encoded = vec![transaction_type];
    alloy_rlp::Header {
        list: true,
        payload_length: tx.rlp_encoded_fields_length() + v.length() + r.length() + s.length(),
    }
    .encode(&mut encoded);
    tx.rlp_encode_fields(&mut encoded);
    v.encode(&mut encoded);
    r.encode(&mut encoded);
    s.encode(&mut encoded);
    encoded.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimation_envelope_does_not_depend_on_gas_search_limit() {
        let mut request = TransactionRequest::default();
        let envelope = fake_envelope(&request);
        request.gas = Some(21_000);
        assert_eq!(fake_envelope(&request), envelope);
        request.gas = Some(30_000_000);
        assert_eq!(fake_envelope(&request), envelope);
    }
}
