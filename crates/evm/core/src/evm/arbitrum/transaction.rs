//! Nitro RPC envelopes at the Arbitrum execution boundary.

use alloy_eips::Typed2718;
use alloy_evm::FromRecoveredTx;
use alloy_network::{AnyRpcTransaction, AnyTxEnvelope, TransactionResponse, UnknownTxEnvelope};
use alloy_primitives::{Address, B256, U64, U128, U256, keccak256};
use alloy_rlp::{Encodable, Header};
use alloy_sol_types::SolCall;
use arbos_revm::{
    constants::{
        ARBITRUM_CONTRACT_TX_TYPE, ARBITRUM_DEPOSIT_TX_TYPE, ARBITRUM_INTERNAL_TX_TYPE,
        ARBITRUM_RETRY_TX_TYPE, ARBITRUM_SUBMIT_RETRYABLE_TX_TYPE, ARBITRUM_UNSIGNED_TX_TYPE,
        ARBOS_ADDRESS,
    },
    handler::submitRetryableCall,
    transaction::{
        ArbitrumDepositTx, ArbitrumInternalTx, ArbitrumRetryTx, ArbitrumTransaction,
        arbitrum_submit_retryable_tx_hash,
    },
};
use eyre::{Context, ensure};
use revm::context::TxEnv;
use serde::de::DeserializeOwned;

use crate::FromAnyRpcTransaction;

/// Unlike Alloy's permissive unknown-envelope accessors, required execution fields must not
/// silently become zero when absent, malformed, or wider than the execution environment.
fn field<T: DeserializeOwned>(tx: &UnknownTxEnvelope, name: &str) -> eyre::Result<T> {
    tx.inner
        .fields
        .get_deserialized(name)
        .ok_or_else(|| eyre::eyre!("missing Arbitrum transaction field {name}"))?
        .wrap_err_with(|| format!("invalid Arbitrum transaction field {name}"))
}

impl FromAnyRpcTransaction for ArbitrumTransaction {
    fn from_any_rpc_transaction(tx: &AnyRpcTransaction) -> eyre::Result<Self> {
        if let Some(envelope) = tx.as_envelope() {
            return Ok(Self::from_recovered_tx(envelope, tx.from()));
        }
        let AnyTxEnvelope::Unknown(native) = &*tx.inner.inner else {
            eyre::bail!("expected an Arbitrum transaction envelope");
        };
        let ty = native.ty();
        ensure!(
            matches!(
                ty,
                ARBITRUM_DEPOSIT_TX_TYPE
                    | ARBITRUM_UNSIGNED_TX_TYPE
                    | ARBITRUM_CONTRACT_TX_TYPE
                    | ARBITRUM_RETRY_TX_TYPE
                    | ARBITRUM_SUBMIT_RETRYABLE_TX_TYPE
                    | ARBITRUM_INTERNAL_TX_TYPE
            ),
            "unsupported Arbitrum transaction type {ty:#x} (pre-Nitro Classic replay is unsupported)"
        );
        let chain_id = field::<U64>(native, "chainId")?.to();
        let to = field::<Option<Address>>(native, "to")?;
        let base = TxEnv {
            tx_type: ty,
            caller: tx.from(),
            gas_limit: field::<U64>(native, "gas")?.to(),
            gas_price: if matches!(ty, ARBITRUM_DEPOSIT_TX_TYPE | ARBITRUM_INTERNAL_TX_TYPE) {
                field::<U128>(native, "gasPrice")?.to()
            } else {
                field::<U128>(native, "maxFeePerGas")?.to()
            },
            gas_priority_fee: Some(0),
            kind: to.into(),
            value: field(native, "value")?,
            data: field(native, "input")?,
            nonce: field::<U64>(native, "nonce")?.to(),
            chain_id: Some(chain_id),
            ..Default::default()
        };
        let mut result = Self::new(base);
        let base = &result.base;
        let expected_hash = match ty {
            ARBITRUM_DEPOSIT_TX_TYPE => {
                ensure!(
                    base.gas_limit == 0
                        && base.gas_price == 0
                        && base.nonce == 0
                        && base.data.is_empty(),
                    "invalid Arbitrum deposit execution fields"
                );
                ArbitrumDepositTx::new(
                    chain_id,
                    field(native, "requestId")?,
                    base.caller,
                    to.ok_or_else(|| eyre::eyre!("Arbitrum deposit requires a recipient"))?,
                    base.value,
                )
                .hash()
            }
            ARBITRUM_INTERNAL_TX_TYPE => {
                ensure!(
                    base.caller == ARBOS_ADDRESS
                        && to == Some(ARBOS_ADDRESS)
                        && base.gas_limit == 0
                        && base.gas_price == 0
                        && base.nonce == 0
                        && base.value.is_zero(),
                    "invalid Arbitrum internal execution fields"
                );
                ArbitrumInternalTx::new(chain_id, base.data.clone()).hash()
            }
            ARBITRUM_RETRY_TX_TYPE => {
                let retry = ArbitrumRetryTx {
                    chain_id: U256::from(chain_id),
                    nonce: base.nonce,
                    from: base.caller,
                    gas_fee_cap: U256::from(base.gas_price),
                    gas_limit: base.gas_limit,
                    to,
                    value: base.value,
                    data: base.data.clone(),
                    ticket_id: field(native, "ticketId")?,
                    refund_to: field(native, "refundTo")?,
                    max_refund: field(native, "maxRefund")?,
                    submission_fee_refund: field(native, "submissionFeeRefund")?,
                };
                let hash = retry.hash();
                result.retry = Some(retry);
                hash
            }
            ARBITRUM_SUBMIT_RETRYABLE_TX_TYPE => {
                // Nitro exposes both the synthetic precompile input and the original ticket
                // fields. Check their consistency before passing the input to ArbOS.
                let retry_to = native
                    .inner
                    .fields
                    .get_deserialized::<Option<Address>>("retryTo")
                    .transpose()
                    .wrap_err("invalid Arbitrum retryTo")?
                    .flatten();
                let call = submitRetryableCall {
                    requestId: field(native, "requestId")?,
                    l1BaseFee: field(native, "l1BaseFee")?,
                    deposit: field(native, "depositValue")?,
                    callvalue: field(native, "retryValue")?,
                    gasFeeCap: U256::from(base.gas_price),
                    gasLimit: base.gas_limit,
                    maxSubmissionFee: field(native, "maxSubmissionFee")?,
                    feeRefundAddress: field(native, "refundTo")?,
                    beneficiary: field(native, "beneficiary")?,
                    retryTo: retry_to.unwrap_or_default(),
                    retryData: field(native, "retryData")?,
                };
                ensure!(
                    to == Some(Address::with_last_byte(0x6e))
                        && base.value.is_zero()
                        && base.nonce == 0
                        && base.data.as_ref() == call.abi_encode(),
                    "inconsistent Arbitrum submit-retryable execution fields"
                );
                arbitrum_submit_retryable_tx_hash(
                    U256::from(chain_id),
                    call.requestId,
                    base.caller,
                    call.l1BaseFee,
                    call.deposit,
                    call.gasFeeCap,
                    call.gasLimit,
                    retry_to,
                    call.callvalue,
                    call.beneficiary,
                    call.maxSubmissionFee,
                    call.feeRefundAddress,
                    &call.retryData,
                )
            }
            ARBITRUM_UNSIGNED_TX_TYPE | ARBITRUM_CONTRACT_TX_TYPE => {
                let mut payload = Vec::new();
                chain_id.encode(&mut payload);
                if ty == ARBITRUM_CONTRACT_TX_TYPE {
                    ensure!(base.nonce == 0, "Arbitrum contract transaction nonce must be zero");
                    field::<B256>(native, "requestId")?.encode(&mut payload);
                }
                base.caller.encode(&mut payload);
                if ty == ARBITRUM_UNSIGNED_TX_TYPE {
                    base.nonce.encode(&mut payload);
                }
                base.gas_price.encode(&mut payload);
                base.gas_limit.encode(&mut payload);
                base.kind.encode(&mut payload);
                base.value.encode(&mut payload);
                base.data.encode(&mut payload);
                let mut encoded = vec![ty];
                Header { list: true, payload_length: payload.len() }.encode(&mut encoded);
                encoded.extend_from_slice(&payload);
                keccak256(encoded)
            }
            _ => unreachable!("native type checked above"),
        };
        ensure!(
            expected_hash == native.hash,
            "Arbitrum transaction hash does not match its native envelope"
        );
        Ok(result.with_canonical_hash(expected_hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Bytes;
    use arbos_revm::transaction::ArbitrumTxProvenance;
    use serde_json::{Value, json};

    // Generated with Nitro's go-ethereum 0f618f330b8d78457524839997f0041d86f3cd1a:
    // types.NewTx(...).MarshalJSON(), with the mandatory ethapi RPC fields filled from
    // Transaction's getters. Hashes and synthetic submit calldata are produced by Go, not Rust.
    fn vectors() -> Vec<Value> {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/testdata/arbitrum-native-transactions.json"
        )))
        .unwrap()
    }

    #[test]
    fn decodes_nitro_native_hashes_and_retry_settlement() {
        let vectors = vectors();
        assert_eq!(vectors.len(), 10);
        for vector in vectors {
            let rpc: AnyRpcTransaction = serde_json::from_value(vector.clone()).unwrap();
            let tx = ArbitrumTransaction::from_any_rpc_transaction(&rpc).unwrap();
            assert_eq!(tx.base.tx_type, rpc.ty());
            assert_eq!(tx.canonical_hash, Some(rpc.tx_hash()));
            assert_eq!(tx.base.caller, rpc.from());
            assert_eq!(tx.base.chain_id, Some(421_614));
            assert_eq!(
                tx.base.data,
                serde_json::from_value::<Bytes>(vector["input"].clone()).unwrap()
            );
            assert_eq!(tx.provenance, ArbitrumTxProvenance::Exempt);
            assert!(tx.enveloped_tx.is_none());
            if let Some(retry) = tx.retry {
                assert_eq!(retry.max_refund, U256::from(123));
                assert_eq!(retry.submission_fee_refund, U256::from(456));
                assert_eq!(retry.refund_to, Address::repeat_byte(0x33));
                assert_eq!(retry.hash(), rpc.tx_hash());
            } else {
                assert_ne!(rpc.ty(), ARBITRUM_RETRY_TX_TYPE);
            }
        }
    }

    fn rejects(vector: Value, expected: &str) {
        let rpc: AnyRpcTransaction = serde_json::from_value(vector).unwrap();
        let error = ArbitrumTransaction::from_any_rpc_transaction(&rpc).unwrap_err();
        assert!(error.to_string().contains(expected), "unexpected error: {error:#}");
    }

    #[test]
    fn rejects_missing_malformed_and_inconsistent_native_fields() {
        let vectors = vectors();
        for vector in &vectors {
            for field in ["chainId", "gas", "nonce", "input", "value"] {
                let mut invalid = vector.clone();
                invalid.as_object_mut().unwrap().remove(field);
                rejects(invalid, field);
            }
            let mut invalid = vector.clone();
            invalid["hash"] = json!(B256::ZERO);
            rejects(invalid, "hash does not match");
            let mut invalid = vector.clone();
            invalid["chainId"] = json!("0x10000000000000000");
            rejects(invalid, "chainId");
        }
        for field in ["ticketId", "refundTo", "maxRefund", "submissionFeeRefund"] {
            let mut invalid = vectors[4].clone();
            invalid.as_object_mut().unwrap().remove(field);
            rejects(invalid, field);
        }
        let mut invalid = vectors[2].clone();
        invalid["maxFeePerGas"] = json!("0x100000000000000000000000000000000");
        rejects(invalid, "maxFeePerGas");
        let mut invalid = vectors[1].clone();
        invalid["from"] = json!(Address::ZERO);
        rejects(invalid, "internal execution fields");
        let mut invalid = vectors[5].clone();
        invalid["depositValue"] = json!("0x1");
        rejects(invalid, "submit-retryable execution fields");
        let mut invalid = vectors[0].clone();
        invalid["type"] = json!("0x78");
        rejects(invalid, "pre-Nitro Classic");
    }
}
