//! Experimental ExecutionPayloadHeader tree hash computation from ExecutionData.
//!
//! This module provides functionality to compute the SSZ tree hash root of an
//! ExecutionPayloadHeader directly from alloy's ExecutionData type.

// Allow missing docs for experimental module
#![allow(missing_docs)]

use ssz_types::{FixedVector, VariableList};
use tree_hash_derive::TreeHash;

// Type aliases for SSZ-compatible primitives that implement TreeHash
pub type Hash32 = [u8; 32];
pub type Address20 = [u8; 20];
pub type LogsBloom = FixedVector<u8, typenum::U256>;
pub type ExtraData = VariableList<u8, typenum::U32>;
pub type Uint256Bytes = [u8; 32];

// SSZ list bounds from consensus specs
pub type MaxWithdrawalsPerPayload = typenum::U16;

// Constants for zero-copy transaction root computation
pub const MAX_BYTES_PER_TRANSACTION: usize = 1 << 30; // 2^30
pub const MAX_TRANSACTIONS_PER_PAYLOAD: usize = 1 << 20; // 2^20

/// Enum representing different Ethereum fork names.
#[derive(Debug, Clone, Copy)]
pub enum ForkName {
    Bellatrix,
    Capella,
    Deneb,
    Electra,
}

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
