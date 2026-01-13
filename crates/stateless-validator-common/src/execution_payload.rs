//! Experimental ExecutionPayloadHeader tree hash computation from ExecutionData.
//!
//! This module provides functionality to compute the SSZ tree hash root of an
//! ExecutionPayloadHeader directly from alloy's ExecutionData type.

// TODO
// Allow missing docs for experimental module
#![allow(missing_docs)]

use anyhow::{Context, Result};
use ssz::{Decode, Encode};
use ssz_types::{FixedVector, VariableList};
use tree_hash::TreeHash;
use tree_hash_derive::TreeHash;

// Type aliases for SSZ-compatible primitives that implement TreeHash
pub type Hash32 = [u8; 32];
pub type Address20 = [u8; 20];
pub type LogsBloom = FixedVector<u8, typenum::U256>;
pub type ExtraData = VariableList<u8, typenum::U32>;
pub type Uint256Bytes = [u8; 32];

// SSZ list bounds from consensus specs
pub type MaxWithdrawalsPerPayload = typenum::U16;
pub type MaxBlobCommitmentsPerBlock = typenum::U4096;

// Electra request bounds (EIP-6110, EIP-7002, EIP-7251)
pub type MaxDepositRequestsPerPayload = typenum::U8192; // 2^13
pub type MaxWithdrawalRequestsPerPayload = typenum::U16; // 2^4
pub type MaxConsolidationRequestsPerPayload = typenum::U2; // 2^1

// Type aliases for Electra request fields
pub type Bytes48 = [u8; 48];
pub type Bytes96 = [u8; 96];

// Constants for zero-copy transaction root computation
pub const MAX_BYTES_PER_TRANSACTION: usize = 1 << 30; // 2^30
pub const MAX_TRANSACTIONS_PER_PAYLOAD: usize = 1 << 20; // 2^20

/// DepositRequest from EIP-6110: Supply validator deposits on chain
#[derive(Debug, Clone, TreeHash, ssz_derive::Encode, ssz_derive::Decode)]
pub struct DepositRequest {
    pub pubkey: Bytes48,
    pub withdrawal_credentials: Hash32,
    pub amount: u64,
    pub signature: Bytes96,
    pub index: u64,
}

/// WithdrawalRequest from EIP-7002: Execution layer triggerable withdrawals
#[derive(Debug, Clone, TreeHash, ssz_derive::Encode, ssz_derive::Decode)]
pub struct WithdrawalRequest {
    pub source_address: Address20,
    pub validator_pubkey: Bytes48,
    pub amount: u64,
}

/// ConsolidationRequest from EIP-7251: Increase the MAX_EFFECTIVE_BALANCE
#[derive(Debug, Clone, TreeHash, ssz_derive::Encode, ssz_derive::Decode)]
pub struct ConsolidationRequest {
    pub source_address: Address20,
    pub source_pubkey: Bytes48,
    pub target_pubkey: Bytes48,
}

/// ExecutionRequests container for Electra fork
#[derive(Debug, Clone, Default, TreeHash)]
pub struct ExecutionRequests {
    pub deposits: VariableList<DepositRequest, MaxDepositRequestsPerPayload>,
    pub withdrawals: VariableList<WithdrawalRequest, MaxWithdrawalRequestsPerPayload>,
    pub consolidations: VariableList<ConsolidationRequest, MaxConsolidationRequestsPerPayload>,
}

/// Enum representing different Ethereum fork names.
#[derive(Debug, Clone, Copy)]
pub enum ForkName {
    Bellatrix,
    Capella,
    Deneb,
    Electra,
}

/// ExecutionPayloadHeaderV1
#[derive(Debug, Clone, TreeHash)]
pub struct ExecutionPayloadHeaderV1 {
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

/// ExecutionPayloadHeaderV2
#[derive(Debug, Clone, TreeHash)]
pub struct ExecutionPayloadHeaderV2 {
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

/// ExecutionPayloadHeaderV3
#[derive(Debug, Clone, TreeHash)]
pub struct ExecutionPayloadHeaderV3 {
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

#[derive(Debug, Clone, TreeHash)]
pub struct NewExecutionPayloadRequestBellatrix {
    pub execution_payload_header: ExecutionPayloadHeaderV1,
}

#[derive(Debug, Clone, TreeHash)]
pub struct NewExecutionPayloadRequestCapella {
    pub execution_payload_header: ExecutionPayloadHeaderV2,
}

#[derive(Debug, Clone, TreeHash)]
pub struct NewExecutionPayloadRequestDeneb {
    pub execution_payload_header: ExecutionPayloadHeaderV3,
    pub versioned_hashes: VariableList<Hash32, MaxBlobCommitmentsPerBlock>,
    pub parent_beacon_block_root: Hash32,
}

#[derive(Debug, Clone, TreeHash)]
pub struct NewExecutionPayloadRequestElectra {
    pub execution_payload_header: ExecutionPayloadHeaderV3,
    pub versioned_hashes: VariableList<Hash32, MaxBlobCommitmentsPerBlock>,
    pub parent_beacon_block_root: Hash32,
    pub execution_requests: ExecutionRequests,
}

#[derive(Debug, Clone)]
pub enum NewExecutionPayloadRequest {
    Bellatrix(NewExecutionPayloadRequestBellatrix),
    Capella(NewExecutionPayloadRequestCapella),
    Deneb(NewExecutionPayloadRequestDeneb),
    Electra(NewExecutionPayloadRequestElectra),
}

impl NewExecutionPayloadRequest {
    pub fn new_bellatrix(execution_payload_header: ExecutionPayloadHeaderV1) -> Self {
        NewExecutionPayloadRequest::Bellatrix(NewExecutionPayloadRequestBellatrix {
            execution_payload_header,
        })
    }

    pub fn new_capella(execution_payload_header: ExecutionPayloadHeaderV2) -> Self {
        NewExecutionPayloadRequest::Capella(NewExecutionPayloadRequestCapella {
            execution_payload_header,
        })
    }

    pub fn new_deneb(
        execution_payload_header: ExecutionPayloadHeaderV3,
        versioned_hashes: Vec<Hash32>,
        parent_beacon_block_root: Hash32,
    ) -> Result<Self> {
        let versioned_hashes = VariableList::<Hash32, MaxBlobCommitmentsPerBlock>::new(versioned_hashes).map_err(|err| anyhow::anyhow!(
            "Versioned hashes length should be within bounds for MaxBlobCommitmentsPerBlock: {:?}",
            err)
        )?;
        Ok(NewExecutionPayloadRequest::Deneb(
            NewExecutionPayloadRequestDeneb {
                execution_payload_header,
                versioned_hashes,
                parent_beacon_block_root,
            },
        ))
    }

    pub fn new_electra(
        execution_payload_header: ExecutionPayloadHeaderV3,
        versioned_hashes: Vec<Hash32>,
        parent_beacon_block_root: Hash32,
        execution_requests: &[impl AsRef<[u8]>],
    ) -> Result<Self> {
        let versioned_hashes = VariableList::<Hash32, MaxBlobCommitmentsPerBlock>::new(versioned_hashes).map_err(|err| anyhow::anyhow!(
            "Versioned hashes length should be within bounds for MaxBlobCommitmentsPerBlock: {:?}",
            err)
        )?;
        let execution_requests = decode_execution_requests(execution_requests)
            .context("Decoding execution requests failed")?;
        Ok(NewExecutionPayloadRequest::Electra(
            NewExecutionPayloadRequestElectra {
                execution_payload_header,
                versioned_hashes,
                parent_beacon_block_root,
                execution_requests,
            },
        ))
    }

    pub fn tree_hash_root(&self) -> [u8; 32] {
        match self {
            NewExecutionPayloadRequest::Bellatrix(req) => req.tree_hash_root().0,
            NewExecutionPayloadRequest::Capella(req) => req.tree_hash_root().0,
            NewExecutionPayloadRequest::Deneb(req) => req.tree_hash_root().0,
            NewExecutionPayloadRequest::Electra(req) => req.tree_hash_root().0,
        }
    }
}

fn decode_execution_requests(requests_list: &[impl AsRef<[u8]>]) -> Result<ExecutionRequests> {
    // EIP-7685: requests are encoded as request_type (1 byte) ++ request_data
    // Request types for Electra (Prague):
    // - 0x00: Deposit requests (EIP-6110)
    // - 0x01: Withdrawal requests (EIP-7002)
    // - 0x02: Consolidation requests (EIP-7251)

    const DEPOSIT_REQUEST_TYPE: u8 = 0x00;
    const WITHDRAWAL_REQUEST_TYPE: u8 = 0x01;
    const CONSOLIDATION_REQUEST_TYPE: u8 = 0x02;

    // Fixed SSZ sizes for each request type (excluding the type byte)
    let deposit_request_size = <DepositRequest as Encode>::ssz_fixed_len();
    let withdrawal_request_size = <WithdrawalRequest as Encode>::ssz_fixed_len();
    let consolidation_request_size = <ConsolidationRequest as Encode>::ssz_fixed_len();

    let mut deposits = Vec::new();
    let mut withdrawals = Vec::new();
    let mut consolidations = Vec::new();

    for (idx, request) in requests_list.iter().enumerate() {
        let request_bytes = request.as_ref();

        anyhow::ensure!(!request_bytes.is_empty(), "Empty request at index {}", idx);

        // Read request type (first byte)
        let request_type = request_bytes[0];
        let data = &request_bytes[1..];

        match request_type {
            DEPOSIT_REQUEST_TYPE => {
                anyhow::ensure!(
                    data.len() == deposit_request_size,
                    "Invalid deposit request size at index {}: expected {}, got {}",
                    idx,
                    deposit_request_size,
                    data.len()
                );

                let deposit = DepositRequest::from_ssz_bytes(data).map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to SSZ decode deposit request at index {}: {:?}",
                        idx,
                        e
                    )
                })?;
                deposits.push(deposit);
            }
            WITHDRAWAL_REQUEST_TYPE => {
                anyhow::ensure!(
                    data.len() == withdrawal_request_size,
                    "Invalid withdrawal request size at index {}: expected {}, got {}",
                    idx,
                    withdrawal_request_size,
                    data.len()
                );

                let withdrawal = WithdrawalRequest::from_ssz_bytes(data).map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to SSZ decode withdrawal request at index {}: {:?}",
                        idx,
                        e
                    )
                })?;
                withdrawals.push(withdrawal);
            }
            CONSOLIDATION_REQUEST_TYPE => {
                anyhow::ensure!(
                    data.len() == consolidation_request_size,
                    "Invalid consolidation request size at index {}: expected {}, got {}",
                    idx,
                    consolidation_request_size,
                    data.len()
                );

                let consolidation = ConsolidationRequest::from_ssz_bytes(data).map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to SSZ decode consolidation request at index {}: {:?}",
                        idx,
                        e
                    )
                })?;
                consolidations.push(consolidation);
            }
            _ => {
                anyhow::bail!("Unknown request type at index {}: {:#x}", idx, request_type);
            }
        }
    }

    Ok(ExecutionRequests {
        deposits: VariableList::new(deposits)
            .map_err(|e| anyhow::anyhow!("Failed to create deposits VariableList: {:?}", e))?,
        withdrawals: VariableList::new(withdrawals)
            .map_err(|e| anyhow::anyhow!("Failed to create withdrawals VariableList: {:?}", e))?,
        consolidations: VariableList::new(consolidations).map_err(|e| {
            anyhow::anyhow!("Failed to create consolidations VariableList: {:?}", e)
        })?,
    })
}
