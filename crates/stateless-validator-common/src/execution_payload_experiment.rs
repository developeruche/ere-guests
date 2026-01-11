//! Experimental ExecutionPayloadHeader tree hash computation from ExecutionData.
//!
//! This module provides functionality to compute the SSZ tree hash root of an
//! ExecutionPayloadHeader directly from alloy's ExecutionData type.

// Allow missing docs for experimental module
#![allow(missing_docs)]

use alloc::vec::Vec;

use alloy_eips::eip4895::Withdrawal;
use alloy_primitives::{B256, Bytes};
use alloy_rpc_types_engine::{ExecutionData, ExecutionPayload};
use ssz_types::{FixedVector, VariableList};
use tree_hash::{BYTES_PER_CHUNK, Hash256, TreeHash, merkle_root, mix_in_length};
use tree_hash_derive::TreeHash;

// Type aliases for SSZ-compatible primitives that implement TreeHash
type Hash32 = [u8; 32];
type Address20 = [u8; 20];
type LogsBloom = FixedVector<u8, typenum::U256>;
type ExtraData = VariableList<u8, typenum::U32>;
type Uint256Bytes = [u8; 32];

// SSZ list bounds from consensus specs
type MaxWithdrawalsPerPayload = typenum::U16;

// Constants for zero-copy transaction root computation
const MAX_BYTES_PER_TRANSACTION: usize = 1 << 30; // 2^30
const MAX_TRANSACTIONS_PER_PAYLOAD: usize = 1 << 20; // 2^20

/// ExecutionPayloadHeader for Bellatrix (no withdrawals, no blob gas).
#[derive(Debug, Clone, TreeHash)]
pub struct ExecutionPayloadHeaderBellatrix {
    pub parent_hash: Hash32,
    pub fee_recipient: Address20,
    pub state_root: Hash32,
    pub receipts_root: Hash32,
    pub logs_bloom: LogsBloom,
    pub prev_randao: Hash32,
    pub block_number: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    pub extra_data: ExtraData,
    pub base_fee_per_gas: Uint256Bytes,
    pub block_hash: Hash32,
    pub transactions_root: Hash32,
}

/// ExecutionPayloadHeader for Capella (adds withdrawals_root).
#[derive(Debug, Clone, TreeHash)]
pub struct ExecutionPayloadHeaderCapella {
    pub parent_hash: Hash32,
    pub fee_recipient: Address20,
    pub state_root: Hash32,
    pub receipts_root: Hash32,
    pub logs_bloom: LogsBloom,
    pub prev_randao: Hash32,
    pub block_number: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    pub extra_data: ExtraData,
    pub base_fee_per_gas: Uint256Bytes,
    pub block_hash: Hash32,
    pub transactions_root: Hash32,
    pub withdrawals_root: Hash32,
}

/// ExecutionPayloadHeader for Deneb (adds blob gas fields).
#[derive(Debug, Clone, TreeHash)]
pub struct ExecutionPayloadHeaderDeneb {
    pub parent_hash: Hash32,
    pub fee_recipient: Address20,
    pub state_root: Hash32,
    pub receipts_root: Hash32,
    pub logs_bloom: LogsBloom,
    pub prev_randao: Hash32,
    pub block_number: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    pub extra_data: ExtraData,
    pub base_fee_per_gas: Uint256Bytes,
    pub block_hash: Hash32,
    pub transactions_root: Hash32,
    pub withdrawals_root: Hash32,
    pub blob_gas_used: u64,
    pub excess_blob_gas: u64,
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

/// Computes the tree_hash_root of ExecutionPayloadHeader from ExecutionData.
///
/// This function converts the execution layer payload into a consensus layer
/// header representation and computes its SSZ tree hash root.
pub fn execution_payload_tree_root(execution_data: &ExecutionData) -> B256 {
    match &execution_data.payload {
        ExecutionPayload::V1(v1) => {
            let transactions_root = compute_transactions_root(&v1.transactions);
            let header = ExecutionPayloadHeaderBellatrix {
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
            B256::from(header.tree_hash_root().0)
        }
        ExecutionPayload::V2(v2) => {
            // V2 is nested: v2.payload_inner contains V1 fields
            let v1 = &v2.payload_inner;
            let transactions_root = compute_transactions_root(&v1.transactions);
            let withdrawals_root = compute_withdrawals_root(&v2.withdrawals);
            let header = ExecutionPayloadHeaderCapella {
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
            B256::from(header.tree_hash_root().0)
        }
        ExecutionPayload::V3(v3) => {
            // V3 is doubly nested: v3.payload_inner.payload_inner contains V1 fields
            let v2 = &v3.payload_inner;
            let v1 = &v2.payload_inner;
            let transactions_root = compute_transactions_root(&v1.transactions);
            let withdrawals_root = compute_withdrawals_root(&v2.withdrawals);
            let header = ExecutionPayloadHeaderDeneb {
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
            B256::from(header.tree_hash_root().0)
        }
    }
}
