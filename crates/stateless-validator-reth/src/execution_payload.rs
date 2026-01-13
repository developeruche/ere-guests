//! Reth -> ExecutionPayload conversion.

use alloc::vec::Vec;

use alloy_eips::{eip4895::Withdrawal, eip7685::Requests};
use alloy_genesis::ChainConfig;
use alloy_primitives::Bytes;
use alloy_rpc_types_engine::{ExecutionData, ExecutionPayload, PayloadError};
use anyhow::Result;
use ssz_types::{FixedVector, VariableList};
use stateless_validator_common::execution_payload::{
    Address20, ExecutionPayloadHeaderV1, ExecutionPayloadHeaderV2, ExecutionPayloadHeaderV3,
    ForkName, Hash32, MAX_BYTES_PER_TRANSACTION, MAX_TRANSACTIONS_PER_PAYLOAD,
    MaxWithdrawalsPerPayload, NewPayloadRequest,
};
use tree_hash::{BYTES_PER_CHUNK, Hash256, TreeHash, merkle_root, mix_in_length};
use tree_hash_derive::TreeHash;

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

/// Converts an [`ExecutionData`] into a reth [`Block`].
///
/// This uses alloy's built-in `try_into_block` method to decode the execution
/// payload transactions and construct a block.
pub fn new_payload_request_to_block(
    new_payload_request: NewPayloadRequest,
) -> Result<alloy_consensus::Block<reth_ethereum_primitives::TransactionSigned>, PayloadError> {
    // TODO: map new_payload_request to alloy_rpc_types_engine.

    execution_data.try_into_block()
}

/// Creates a new execution payload request.
pub fn create_new_payload_request(
    execution_data: &ExecutionData,
    requests: &Requests,
) -> Result<NewPayloadRequest> {
    match &execution_data.payload {
        ExecutionPayload::V1(v1) => {
            let transactions_root = compute_transactions_root(&v1.transactions);
            let header = ExecutionPayloadHeaderV1 {
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
                transactions_root,
            };
            Ok(NewPayloadRequest::new_bellatrix(header))
        }
        ExecutionPayload::V2(v2) => {
            // V2 is nested: v2.payload_inner contains V1 fields
            let v1 = &v2.payload_inner;
            let transactions_root = compute_transactions_root(&v1.transactions);
            let withdrawals_root = compute_withdrawals_root(&v2.withdrawals);
            let header = ExecutionPayloadHeaderV2 {
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
                transactions_root,
                withdrawals_root,
            };
            Ok(NewPayloadRequest::new_capella(header))
        }
        ExecutionPayload::V3(v3) => {
            // V3 is doubly nested: v3.payload_inner.payload_inner contains V1 fields
            let v2 = &v3.payload_inner;
            let v1 = &v2.payload_inner;
            let transactions_root = compute_transactions_root(&v1.transactions);
            let withdrawals_root = compute_withdrawals_root(&v2.withdrawals);
            let header = ExecutionPayloadHeaderV3 {
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
                transactions_root,
                withdrawals_root,
                blob_gas_used: v3.blob_gas_used,
                excess_blob_gas: v3.excess_blob_gas,
            };
            let sidecar = &execution_data.sidecar;
            match (sidecar.cancun(), sidecar.prague()) {
                // Deneb
                (Some(c), None) => {
                    let versioned_hashes = c.versioned_hashes.iter().map(|h| h.0).collect();
                    let parent_beacon_block_root = c.parent_beacon_block_root.0;
                    NewPayloadRequest::new_deneb(header, versioned_hashes, parent_beacon_block_root)
                }
                // Electra
                (Some(c), Some(_)) => {
                    let versioned_hashes = c.versioned_hashes.iter().map(|h| h.0).collect();
                    let parent_beacon_block_root = c.parent_beacon_block_root.0;
                    NewPayloadRequest::new_electra(
                        header,
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

/// SSZ Withdrawal container for tree hash computation.
#[derive(Debug, Clone, TreeHash)]
struct SszWithdrawal {
    index: u64,
    validator_index: u64,
    address: Address20,
    amount: u64,
}

/// Computes the SSZ tree hash root of the transactions list.
///
/// This implementation avoids copying transaction bytes by computing the tree hash
/// directly from borrowed slices using `tree_hash::merkle_root`.
fn compute_transactions_root(transactions: &[Bytes]) -> Hash32 {
    // For each transaction (List<uint8, MAX_BYTES_PER_TRANSACTION>):
    // tree_hash = mix_in_length(merkle_root(bytes, limit/32), len)
    let tx_leaf_limit = MAX_BYTES_PER_TRANSACTION / BYTES_PER_CHUNK;

    let tx_roots: Vec<Hash256> = transactions
        .iter()
        .map(|tx| {
            let root = merkle_root(tx.as_ref(), tx_leaf_limit);
            mix_in_length(&root, tx.len())
        })
        .collect();

    // For the outer list (List<Transaction, MAX_TRANSACTIONS_PER_PAYLOAD>):
    // Concatenate the 32-byte roots and merkleize with the list limit
    let roots_bytes: Vec<u8> = tx_roots.iter().flat_map(|h| h.0).collect();
    let list_root = merkle_root(&roots_bytes, MAX_TRANSACTIONS_PER_PAYLOAD);
    mix_in_length(&list_root, transactions.len()).0
}

/// Computes the SSZ tree hash root of the withdrawals list.
fn compute_withdrawals_root(withdrawals: &[Withdrawal]) -> Hash32 {
    type Withdrawals = VariableList<SszWithdrawal, MaxWithdrawalsPerPayload>;

    let list: Vec<SszWithdrawal> = withdrawals
        .iter()
        .map(|w| SszWithdrawal {
            index: w.index,
            validator_index: w.validator_index,
            address: w.address.0.0,
            amount: w.amount,
        })
        .collect();
    Withdrawals::from(list).tree_hash_root().0
}
