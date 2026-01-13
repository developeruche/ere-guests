//! Execution payload conversion between NewPayloadRequest and alloy types.

use alloc::{sync::Arc, vec::Vec};

use alloy_consensus::Block;
use alloy_eips::eip4895::Withdrawal as AlloyWithdrawal;
use alloy_genesis::ChainConfig;
use alloy_primitives::{Address, B256, Bloom, Bytes, U256};
use alloy_rpc_types_engine::{
    CancunPayloadFields, ExecutionData, ExecutionPayload as AlloyExecutionPayload,
    ExecutionPayloadSidecar, ExecutionPayloadV1 as AlloyExecutionPayloadV1,
    ExecutionPayloadV2 as AlloyExecutionPayloadV2, ExecutionPayloadV3 as AlloyExecutionPayloadV3,
    PayloadError,
};
use anyhow::{Context, Result};
use reth_chainspec::{ChainSpec, EthereumHardforks};
use reth_payload_validator::{cancun, prague, shanghai};
use reth_primitives_traits::{Block as _, SealedBlock, SignedTransaction};
use ssz_types::{FixedVector, VariableList};
use stateless_validator_common::execution_payload::{
    ExecutionPayloadV1, ExecutionPayloadV2, ExecutionPayloadV3, ForkName, NewPayloadRequest,
    Withdrawal,
};

/// Determines the fork name based on alloy chain config and block timestamp.
pub fn determine_fork_name(chain_config: &ChainConfig, timestamp: u64) -> ForkName {
    // Check forks in reverse chronological order
    if chain_config
        .prague_time
        .is_some_and(|prague_time| timestamp >= prague_time)
    {
        return ForkName::Electra;
    }
    if chain_config
        .cancun_time
        .is_some_and(|cancun_time| timestamp >= cancun_time)
    {
        return ForkName::Deneb;
    }
    if chain_config
        .shanghai_time
        .is_some_and(|shanghai_time| timestamp >= shanghai_time)
    {
        return ForkName::Capella;
    }
    // Default to Bellatrix for post-merge blocks
    ForkName::Bellatrix
}

/// Converts a [`NewPayloadRequest`] into a validated reth [`SealedBlock`].
///
/// This converts the request to `ExecutionData`, then uses
/// `EthereumExecutionPayloadValidator` to validate the payload and return a sealed block.
pub fn new_payload_request_to_block(
    new_payload_request: NewPayloadRequest,
    chain_spec: Arc<ChainSpec>,
) -> Result<alloy_consensus::Block<reth_ethereum_primitives::TransactionSigned>> {
    let execution_data = new_payload_request_to_execution_data(new_payload_request);
    let sealed_block: SealedBlock<
        Block<alloy_consensus::EthereumTxEnvelope<alloy_consensus::TxEip4844>>,
    > = ensure_well_formed_payload(chain_spec, execution_data)
        .context("Payload validation failed")?;
    Ok(sealed_block.into_block())
}

pub fn ensure_well_formed_payload<ChainSpec, T>(
    chain_spec: ChainSpec,
    payload: ExecutionData,
) -> Result<SealedBlock<Block<T>>, PayloadError>
where
    ChainSpec: EthereumHardforks,
    T: SignedTransaction,
{
    let ExecutionData { payload, sidecar } = payload;

    let expected_hash = payload.block_hash();

    // First parse the block
    let sealed_block = payload.try_into_block_with_sidecar(&sidecar)?.seal_slow();

    // Ensure the hash included in the payload matches the block hash
    if expected_hash != sealed_block.hash() {
        return Err(PayloadError::BlockHash {
            execution: sealed_block.hash(),
            consensus: expected_hash,
        });
    }

    shanghai::ensure_well_formed_fields(
        sealed_block.body(),
        chain_spec.is_shanghai_active_at_timestamp(sealed_block.timestamp),
    )?;

    cancun::ensure_well_formed_fields(
        &sealed_block,
        sidecar.cancun(),
        chain_spec.is_cancun_active_at_timestamp(sealed_block.timestamp),
    )?;

    prague::ensure_well_formed_fields(
        sealed_block.body(),
        sidecar.prague(),
        chain_spec.is_prague_active_at_timestamp(sealed_block.timestamp),
    )?;

    Ok(sealed_block)
}

// ============================================================================
// Conversion: NewPayloadRequest -> ExecutionData
// ============================================================================

/// Converts a [`NewPayloadRequest`] into an alloy [`ExecutionData`].
pub fn new_payload_request_to_execution_data(req: NewPayloadRequest) -> ExecutionData {
    match req {
        NewPayloadRequest::Bellatrix(b) => {
            let v1 = convert_v1_to_alloy(b.execution_payload);
            ExecutionData::new(
                AlloyExecutionPayload::V1(v1),
                ExecutionPayloadSidecar::none(),
            )
        }
        NewPayloadRequest::Capella(c) => {
            let (v1, withdrawals) = convert_v2_to_alloy(c.execution_payload);
            let v2 = AlloyExecutionPayloadV2 {
                payload_inner: v1,
                withdrawals,
            };
            ExecutionData::new(
                AlloyExecutionPayload::V2(v2),
                ExecutionPayloadSidecar::none(),
            )
        }
        NewPayloadRequest::Deneb(d) => {
            let (v1, withdrawals) = convert_v2_to_alloy_from_v3(&d.execution_payload);
            let v3 = AlloyExecutionPayloadV3 {
                payload_inner: AlloyExecutionPayloadV2 {
                    payload_inner: v1,
                    withdrawals,
                },
                blob_gas_used: d.execution_payload.blob_gas_used,
                excess_blob_gas: d.execution_payload.excess_blob_gas,
            };

            let versioned_hashes: Vec<B256> = d
                .versioned_hashes
                .into_iter()
                .map(|h| B256::from(h))
                .collect();
            let parent_beacon_block_root = B256::from(d.parent_beacon_block_root);
            let cancun_fields =
                CancunPayloadFields::new(parent_beacon_block_root, versioned_hashes);
            let sidecar = ExecutionPayloadSidecar::v3(cancun_fields);

            ExecutionData::new(AlloyExecutionPayload::V3(v3), sidecar)
        }
        NewPayloadRequest::Electra(e) => {
            let (v1, withdrawals) = convert_v2_to_alloy_from_v3(&e.execution_payload);
            let v3 = AlloyExecutionPayloadV3 {
                payload_inner: AlloyExecutionPayloadV2 {
                    payload_inner: v1,
                    withdrawals,
                },
                blob_gas_used: e.execution_payload.blob_gas_used,
                excess_blob_gas: e.execution_payload.excess_blob_gas,
            };

            let versioned_hashes: Vec<B256> = e
                .versioned_hashes
                .into_iter()
                .map(|h| B256::from(h))
                .collect();
            let parent_beacon_block_root = B256::from(e.parent_beacon_block_root);
            let cancun_fields =
                CancunPayloadFields::new(parent_beacon_block_root, versioned_hashes);

            // For Electra, compute requests_hash from execution_requests
            // The requests_hash is stored in the sidecar
            let requests_hash = compute_requests_hash(&e.execution_requests);
            let prague_fields = alloy_rpc_types_engine::PraguePayloadFields::new(requests_hash);
            let sidecar = ExecutionPayloadSidecar::v4(cancun_fields, prague_fields);

            ExecutionData::new(AlloyExecutionPayload::V3(v3), sidecar)
        }
    }
}

/// Converts ExecutionPayloadV1 to alloy's ExecutionPayloadV1
fn convert_v1_to_alloy(payload: ExecutionPayloadV1) -> AlloyExecutionPayloadV1 {
    AlloyExecutionPayloadV1 {
        parent_hash: B256::from(payload.parent_hash),
        fee_recipient: Address::from(payload.fee_recipient),
        state_root: B256::from(payload.state_root),
        receipts_root: B256::from(payload.receipts_root),
        logs_bloom: Bloom::from_slice(&payload.logs_bloom[..]),
        prev_randao: B256::from(payload.prev_randao),
        block_number: payload.block_number,
        gas_limit: payload.gas_limit,
        gas_used: payload.gas_used,
        timestamp: payload.timestamp,
        extra_data: Bytes::from(payload.extra_data.to_vec()),
        base_fee_per_gas: U256::from_le_bytes(payload.base_fee_per_gas),
        block_hash: B256::from(payload.block_hash),
        transactions: payload.transactions.into_iter().map(Bytes::from).collect(),
    }
}

/// Converts ExecutionPayloadV2 to alloy's (V1, withdrawals)
fn convert_v2_to_alloy(
    payload: ExecutionPayloadV2,
) -> (AlloyExecutionPayloadV1, Vec<AlloyWithdrawal>) {
    let v1 = AlloyExecutionPayloadV1 {
        parent_hash: B256::from(payload.parent_hash),
        fee_recipient: Address::from(payload.fee_recipient),
        state_root: B256::from(payload.state_root),
        receipts_root: B256::from(payload.receipts_root),
        logs_bloom: Bloom::from_slice(&payload.logs_bloom[..]),
        prev_randao: B256::from(payload.prev_randao),
        block_number: payload.block_number,
        gas_limit: payload.gas_limit,
        gas_used: payload.gas_used,
        timestamp: payload.timestamp,
        extra_data: Bytes::from(payload.extra_data.to_vec()),
        base_fee_per_gas: U256::from_le_bytes(payload.base_fee_per_gas),
        block_hash: B256::from(payload.block_hash),
        transactions: payload.transactions.into_iter().map(Bytes::from).collect(),
    };

    let withdrawals = payload
        .withdrawals
        .into_iter()
        .map(convert_withdrawal)
        .collect();

    (v1, withdrawals)
}

/// Converts ExecutionPayloadV3 to alloy's (V1, withdrawals) - used for Deneb/Electra
fn convert_v2_to_alloy_from_v3(
    payload: &ExecutionPayloadV3,
) -> (AlloyExecutionPayloadV1, Vec<AlloyWithdrawal>) {
    let v1 = AlloyExecutionPayloadV1 {
        parent_hash: B256::from(payload.parent_hash),
        fee_recipient: Address::from(payload.fee_recipient),
        state_root: B256::from(payload.state_root),
        receipts_root: B256::from(payload.receipts_root),
        logs_bloom: Bloom::from_slice(&payload.logs_bloom[..]),
        prev_randao: B256::from(payload.prev_randao),
        block_number: payload.block_number,
        gas_limit: payload.gas_limit,
        gas_used: payload.gas_used,
        timestamp: payload.timestamp,
        extra_data: Bytes::from(payload.extra_data.to_vec()),
        base_fee_per_gas: U256::from_le_bytes(payload.base_fee_per_gas),
        block_hash: B256::from(payload.block_hash),
        transactions: payload
            .transactions
            .iter()
            .map(|tx| Bytes::from(tx.clone()))
            .collect(),
    };

    let withdrawals = payload
        .withdrawals
        .iter()
        .map(|w| convert_withdrawal(w.clone()))
        .collect();

    (v1, withdrawals)
}

/// Converts our Withdrawal to alloy's Withdrawal
fn convert_withdrawal(w: Withdrawal) -> AlloyWithdrawal {
    AlloyWithdrawal {
        index: w.index,
        validator_index: w.validator_index,
        address: Address::from(w.address),
        amount: w.amount,
    }
}

/// Computes the requests hash for Electra from ExecutionRequests per EIP-7685.
fn compute_requests_hash(
    requests: &stateless_validator_common::execution_payload::ExecutionRequests,
) -> B256 {
    use sha2::{Digest, Sha256};
    use ssz::Encode;

    let mut outer_hasher = Sha256::new();

    // Deposit requests (type 0x00)
    let mut deposits_bytes = vec![0x00u8];
    for deposit in requests.deposits.iter() {
        deposits_bytes.extend(deposit.as_ssz_bytes());
    }
    if deposits_bytes.len() > 1 {
        outer_hasher.update(Sha256::digest(&deposits_bytes));
    }

    // Withdrawal requests (type 0x01)
    let mut withdrawals_bytes = vec![0x01u8];
    for withdrawal in requests.withdrawals.iter() {
        withdrawals_bytes.extend(withdrawal.as_ssz_bytes());
    }
    if withdrawals_bytes.len() > 1 {
        outer_hasher.update(Sha256::digest(&withdrawals_bytes));
    }

    // Consolidation requests (type 0x02)
    let mut consolidations_bytes = vec![0x02u8];
    for consolidation in requests.consolidations.iter() {
        consolidations_bytes.extend(consolidation.as_ssz_bytes());
    }
    if consolidations_bytes.len() > 1 {
        outer_hasher.update(Sha256::digest(&consolidations_bytes));
    }

    B256::from_slice(&outer_hasher.finalize())
}

// ============================================================================
// Conversion: ExecutionData -> NewPayloadRequest (for creating requests from blocks)
// ============================================================================

/// Creates a new execution payload request from ExecutionData.
///
/// This extracts the transaction and withdrawal data from the ExecutionData
/// and creates the appropriate NewPayloadRequest variant.
pub fn create_new_payload_request(
    execution_data: &ExecutionData,
    requests: &alloy_eips::eip7685::Requests,
) -> anyhow::Result<NewPayloadRequest> {
    match &execution_data.payload {
        AlloyExecutionPayload::V1(v1) => {
            let payload = ExecutionPayloadV1 {
                parent_hash: v1.parent_hash.0,
                fee_recipient: v1.fee_recipient.0.0,
                state_root: v1.state_root.0,
                receipts_root: v1.receipts_root.0,
                logs_bloom: FixedVector::from(v1.logs_bloom.0.to_vec()),
                prev_randao: v1.prev_randao.0,
                block_number: v1.block_number,
                gas_limit: v1.gas_limit,
                gas_used: v1.gas_used,
                timestamp: v1.timestamp,
                extra_data: VariableList::from(v1.extra_data.to_vec()),
                base_fee_per_gas: v1.base_fee_per_gas.to_le_bytes(),
                block_hash: v1.block_hash.0,
                transactions: v1.transactions.iter().map(|tx| tx.to_vec()).collect(),
            };
            Ok(NewPayloadRequest::new_bellatrix(payload))
        }
        AlloyExecutionPayload::V2(v2) => {
            let v1 = &v2.payload_inner;
            let payload = ExecutionPayloadV2 {
                parent_hash: v1.parent_hash.0,
                fee_recipient: v1.fee_recipient.0.0,
                state_root: v1.state_root.0,
                receipts_root: v1.receipts_root.0,
                logs_bloom: FixedVector::from(v1.logs_bloom.0.to_vec()),
                prev_randao: v1.prev_randao.0,
                block_number: v1.block_number,
                gas_limit: v1.gas_limit,
                gas_used: v1.gas_used,
                timestamp: v1.timestamp,
                extra_data: VariableList::from(v1.extra_data.to_vec()),
                base_fee_per_gas: v1.base_fee_per_gas.to_le_bytes(),
                block_hash: v1.block_hash.0,
                transactions: v1.transactions.iter().map(|tx| tx.to_vec()).collect(),
                withdrawals: v2
                    .withdrawals
                    .iter()
                    .map(convert_alloy_withdrawal)
                    .collect(),
            };
            Ok(NewPayloadRequest::new_capella(payload))
        }
        AlloyExecutionPayload::V3(v3) => {
            let v2 = &v3.payload_inner;
            let v1 = &v2.payload_inner;
            let payload = ExecutionPayloadV3 {
                parent_hash: v1.parent_hash.0,
                fee_recipient: v1.fee_recipient.0.0,
                state_root: v1.state_root.0,
                receipts_root: v1.receipts_root.0,
                logs_bloom: FixedVector::from(v1.logs_bloom.0.to_vec()),
                prev_randao: v1.prev_randao.0,
                block_number: v1.block_number,
                gas_limit: v1.gas_limit,
                gas_used: v1.gas_used,
                timestamp: v1.timestamp,
                extra_data: VariableList::from(v1.extra_data.to_vec()),
                base_fee_per_gas: v1.base_fee_per_gas.to_le_bytes(),
                block_hash: v1.block_hash.0,
                transactions: v1.transactions.iter().map(|tx| tx.to_vec()).collect(),
                withdrawals: v2
                    .withdrawals
                    .iter()
                    .map(convert_alloy_withdrawal)
                    .collect(),
                blob_gas_used: v3.blob_gas_used,
                excess_blob_gas: v3.excess_blob_gas,
            };

            let sidecar = &execution_data.sidecar;
            match (sidecar.cancun(), sidecar.prague()) {
                // Deneb
                (Some(c), None) => {
                    let versioned_hashes = c.versioned_hashes.iter().map(|h| h.0).collect();
                    let parent_beacon_block_root = c.parent_beacon_block_root.0;
                    NewPayloadRequest::new_deneb(
                        payload,
                        versioned_hashes,
                        parent_beacon_block_root,
                    )
                }
                // Electra
                (Some(c), Some(_)) => {
                    let versioned_hashes = c.versioned_hashes.iter().map(|h| h.0).collect();
                    let parent_beacon_block_root = c.parent_beacon_block_root.0;
                    NewPayloadRequest::new_electra(
                        payload,
                        versioned_hashes,
                        parent_beacon_block_root,
                        requests,
                    )
                }
                _ => anyhow::bail!("Missing sidecar for Deneb execution payload"),
            }
        }
    }
}

/// Converts alloy's Withdrawal to our Withdrawal
fn convert_alloy_withdrawal(w: &AlloyWithdrawal) -> Withdrawal {
    Withdrawal {
        index: w.index,
        validator_index: w.validator_index,
        address: w.address.0.0,
        amount: w.amount,
    }
}
