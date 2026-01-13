//! Execution payload types for zkVM guest programs.
//!
//! This module provides execution payload types that store full transaction and
//! withdrawal data (not just tree roots), enabling conversion to alloy types.

#![allow(missing_docs)]

use alloc::vec::Vec;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_with::{Bytes, serde_as};
use ssz::{Decode, Encode};
use ssz_types::{FixedVector, VariableList};
use tree_hash::{Hash256, TreeHash, TreeHashType, merkle_root, mix_in_length};
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

// Constants for transaction root computation
pub const MAX_BYTES_PER_TRANSACTION: usize = 1 << 30; // 2^30
pub const MAX_TRANSACTIONS_PER_PAYLOAD: usize = 1 << 20; // 2^20
const BYTES_PER_CHUNK: usize = 32;

/// Withdrawal struct (neutral type compatible with both Reth and Ethrex)
#[derive(Debug, Clone, Serialize, Deserialize, TreeHash)]
pub struct Withdrawal {
    pub index: u64,
    pub validator_index: u64,
    pub address: Address20,
    pub amount: u64,
}

/// DepositRequest from EIP-6110: Supply validator deposits on chain
#[serde_as]
#[derive(
    Debug, Clone, Serialize, Deserialize, TreeHash, ssz_derive::Encode, ssz_derive::Decode,
)]
pub struct DepositRequest {
    #[serde_as(as = "Bytes")]
    pub pubkey: Bytes48,
    pub withdrawal_credentials: Hash32,
    pub amount: u64,
    #[serde_as(as = "Bytes")]
    pub signature: Bytes96,
    pub index: u64,
}

/// WithdrawalRequest from EIP-7002: Execution layer triggerable withdrawals
#[serde_as]
#[derive(
    Debug, Clone, TreeHash, Serialize, Deserialize, ssz_derive::Encode, ssz_derive::Decode,
)]
pub struct WithdrawalRequest {
    pub source_address: Address20,
    #[serde_as(as = "Bytes")]
    pub validator_pubkey: Bytes48,
    pub amount: u64,
}

/// ConsolidationRequest from EIP-7251: Increase the MAX_EFFECTIVE_BALANCE
#[serde_as]
#[derive(
    Debug, Clone, TreeHash, Serialize, Deserialize, ssz_derive::Encode, ssz_derive::Decode,
)]
pub struct ConsolidationRequest {
    pub source_address: Address20,
    #[serde_as(as = "Bytes")]
    pub source_pubkey: Bytes48,
    #[serde_as(as = "Bytes")]
    pub target_pubkey: Bytes48,
}

/// ExecutionRequests container for Electra fork
#[derive(Debug, Clone, Default, Serialize, Deserialize, TreeHash)]
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

/// ExecutionPayloadV1 (Bellatrix)
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPayloadV1 {
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
    #[serde_as(as = "Vec<Bytes>")]
    pub transactions: Vec<Vec<u8>>,
}

/// ExecutionPayloadV2 (Capella)
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPayloadV2 {
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
    #[serde_as(as = "Vec<Bytes>")]
    pub transactions: Vec<Vec<u8>>,
    pub withdrawals: Vec<Withdrawal>,
}

/// ExecutionPayloadV3 (Deneb/Electra)
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPayloadV3 {
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
    #[serde_as(as = "Vec<Bytes>")]
    pub transactions: Vec<Vec<u8>>,
    pub withdrawals: Vec<Withdrawal>,
    pub blob_gas_used: u64,
    pub excess_blob_gas: u64,
}

/// Computes the SSZ tree hash root of the transactions list.
fn compute_transactions_root(transactions: &[Vec<u8>]) -> Hash32 {
    let tx_leaf_limit = MAX_BYTES_PER_TRANSACTION / BYTES_PER_CHUNK;

    let tx_roots: Vec<Hash256> = transactions
        .iter()
        .map(|tx| {
            let root = merkle_root(tx.as_ref(), tx_leaf_limit);
            mix_in_length(&root, tx.len())
        })
        .collect();

    let roots_bytes: Vec<u8> = tx_roots.iter().flat_map(|h| h.0).collect();
    let list_root = merkle_root(&roots_bytes, MAX_TRANSACTIONS_PER_PAYLOAD);
    mix_in_length(&list_root, transactions.len()).0
}

/// Computes the SSZ tree hash root of the withdrawals list.
fn compute_withdrawals_root(withdrawals: &[Withdrawal]) -> Hash32 {
    type Withdrawals = VariableList<Withdrawal, MaxWithdrawalsPerPayload>;
    Withdrawals::from(withdrawals.to_vec()).tree_hash_root().0
}

impl TreeHash for ExecutionPayloadV1 {
    fn tree_hash_type() -> TreeHashType {
        TreeHashType::Container
    }

    fn tree_hash_packed_encoding(&self) -> tree_hash::PackedEncoding {
        unreachable!("Container types should not be packed")
    }

    fn tree_hash_packing_factor() -> usize {
        unreachable!("Container types should not be packed")
    }

    fn tree_hash_root(&self) -> Hash256 {
        // Compute transactions root from actual data
        let transactions_root = compute_transactions_root(&self.transactions);

        // Build header struct for tree hashing
        #[derive(TreeHash)]
        struct HeaderV1 {
            parent_hash: Hash32,
            fee_recipient: Address20,
            state_root: Hash32,
            receipts_root: Hash32,
            logs_bloom: LogsBloom,
            prev_randao: Hash32,
            block_number: u64,
            gas_limit: u64,
            gas_used: u64,
            timestamp: u64,
            extra_data: ExtraData,
            base_fee_per_gas: Uint256Bytes,
            block_hash: Hash32,
            transactions_root: Hash32,
        }

        let header = HeaderV1 {
            parent_hash: self.parent_hash,
            fee_recipient: self.fee_recipient,
            state_root: self.state_root,
            receipts_root: self.receipts_root,
            logs_bloom: self.logs_bloom.clone(),
            prev_randao: self.prev_randao,
            block_number: self.block_number,
            gas_limit: self.gas_limit,
            gas_used: self.gas_used,
            timestamp: self.timestamp,
            extra_data: self.extra_data.clone(),
            base_fee_per_gas: self.base_fee_per_gas,
            block_hash: self.block_hash,
            transactions_root,
        };

        header.tree_hash_root()
    }
}

impl TreeHash for ExecutionPayloadV2 {
    fn tree_hash_type() -> TreeHashType {
        TreeHashType::Container
    }

    fn tree_hash_packed_encoding(&self) -> tree_hash::PackedEncoding {
        unreachable!("Container types should not be packed")
    }

    fn tree_hash_packing_factor() -> usize {
        unreachable!("Container types should not be packed")
    }

    fn tree_hash_root(&self) -> Hash256 {
        let transactions_root = compute_transactions_root(&self.transactions);
        let withdrawals_root = compute_withdrawals_root(&self.withdrawals);

        #[derive(TreeHash)]
        struct HeaderV2 {
            parent_hash: Hash32,
            fee_recipient: Address20,
            state_root: Hash32,
            receipts_root: Hash32,
            logs_bloom: LogsBloom,
            prev_randao: Hash32,
            block_number: u64,
            gas_limit: u64,
            gas_used: u64,
            timestamp: u64,
            extra_data: ExtraData,
            base_fee_per_gas: Uint256Bytes,
            block_hash: Hash32,
            transactions_root: Hash32,
            withdrawals_root: Hash32,
        }

        let header = HeaderV2 {
            parent_hash: self.parent_hash,
            fee_recipient: self.fee_recipient,
            state_root: self.state_root,
            receipts_root: self.receipts_root,
            logs_bloom: self.logs_bloom.clone(),
            prev_randao: self.prev_randao,
            block_number: self.block_number,
            gas_limit: self.gas_limit,
            gas_used: self.gas_used,
            timestamp: self.timestamp,
            extra_data: self.extra_data.clone(),
            base_fee_per_gas: self.base_fee_per_gas,
            block_hash: self.block_hash,
            transactions_root,
            withdrawals_root,
        };

        header.tree_hash_root()
    }
}

impl TreeHash for ExecutionPayloadV3 {
    fn tree_hash_type() -> TreeHashType {
        TreeHashType::Container
    }

    fn tree_hash_packed_encoding(&self) -> tree_hash::PackedEncoding {
        unreachable!("Container types should not be packed")
    }

    fn tree_hash_packing_factor() -> usize {
        unreachable!("Container types should not be packed")
    }

    fn tree_hash_root(&self) -> Hash256 {
        let transactions_root = compute_transactions_root(&self.transactions);
        let withdrawals_root = compute_withdrawals_root(&self.withdrawals);

        #[derive(TreeHash)]
        struct HeaderV3 {
            parent_hash: Hash32,
            fee_recipient: Address20,
            state_root: Hash32,
            receipts_root: Hash32,
            logs_bloom: LogsBloom,
            prev_randao: Hash32,
            block_number: u64,
            gas_limit: u64,
            gas_used: u64,
            timestamp: u64,
            extra_data: ExtraData,
            base_fee_per_gas: Uint256Bytes,
            block_hash: Hash32,
            transactions_root: Hash32,
            withdrawals_root: Hash32,
            blob_gas_used: u64,
            excess_blob_gas: u64,
        }

        let header = HeaderV3 {
            parent_hash: self.parent_hash,
            fee_recipient: self.fee_recipient,
            state_root: self.state_root,
            receipts_root: self.receipts_root,
            logs_bloom: self.logs_bloom.clone(),
            prev_randao: self.prev_randao,
            block_number: self.block_number,
            gas_limit: self.gas_limit,
            gas_used: self.gas_used,
            timestamp: self.timestamp,
            extra_data: self.extra_data.clone(),
            base_fee_per_gas: self.base_fee_per_gas,
            block_hash: self.block_hash,
            transactions_root,
            withdrawals_root,
            blob_gas_used: self.blob_gas_used,
            excess_blob_gas: self.excess_blob_gas,
        };

        header.tree_hash_root()
    }
}

// ============================================================================
// NewPayloadRequest types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash)]
pub struct NewPayloadRequestBellatrix {
    pub execution_payload: ExecutionPayloadV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash)]
pub struct NewPayloadRequestCapella {
    pub execution_payload: ExecutionPayloadV2,
}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash)]
pub struct NewPayloadRequestDeneb {
    pub execution_payload: ExecutionPayloadV3,
    pub versioned_hashes: VariableList<Hash32, MaxBlobCommitmentsPerBlock>,
    pub parent_beacon_block_root: Hash32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash)]
pub struct NewPayloadRequestElectra {
    pub execution_payload: ExecutionPayloadV3,
    pub versioned_hashes: VariableList<Hash32, MaxBlobCommitmentsPerBlock>,
    pub parent_beacon_block_root: Hash32,
    pub execution_requests: ExecutionRequests,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NewPayloadRequest {
    Bellatrix(NewPayloadRequestBellatrix),
    Capella(NewPayloadRequestCapella),
    Deneb(NewPayloadRequestDeneb),
    Electra(NewPayloadRequestElectra),
}

impl NewPayloadRequest {
    pub fn new_bellatrix(execution_payload: ExecutionPayloadV1) -> Self {
        NewPayloadRequest::Bellatrix(NewPayloadRequestBellatrix { execution_payload })
    }

    pub fn new_capella(execution_payload: ExecutionPayloadV2) -> Self {
        NewPayloadRequest::Capella(NewPayloadRequestCapella { execution_payload })
    }

    pub fn new_deneb(
        execution_payload: ExecutionPayloadV3,
        versioned_hashes: Vec<Hash32>,
        parent_beacon_block_root: Hash32,
    ) -> Result<Self> {
        let versioned_hashes =
            VariableList::<Hash32, MaxBlobCommitmentsPerBlock>::new(versioned_hashes).map_err(
                |err| {
                    anyhow::anyhow!(
                    "Versioned hashes length should be within bounds for MaxBlobCommitmentsPerBlock: {:?}",
                    err
                )
                },
            )?;
        Ok(NewPayloadRequest::Deneb(NewPayloadRequestDeneb {
            execution_payload,
            versioned_hashes,
            parent_beacon_block_root,
        }))
    }

    pub fn new_electra(
        execution_payload: ExecutionPayloadV3,
        versioned_hashes: Vec<Hash32>,
        parent_beacon_block_root: Hash32,
        execution_requests: &[impl AsRef<[u8]>],
    ) -> Result<Self> {
        let versioned_hashes =
            VariableList::<Hash32, MaxBlobCommitmentsPerBlock>::new(versioned_hashes).map_err(
                |err| {
                    anyhow::anyhow!(
                    "Versioned hashes length should be within bounds for MaxBlobCommitmentsPerBlock: {:?}",
                    err
                )
                },
            )?;
        let execution_requests = decode_execution_requests(execution_requests)
            .context("Decoding execution requests failed")?;
        Ok(NewPayloadRequest::Electra(NewPayloadRequestElectra {
            execution_payload,
            versioned_hashes,
            parent_beacon_block_root,
            execution_requests,
        }))
    }

    /// Returns the tree hash root of this request.
    pub fn tree_hash_root(&self) -> [u8; 32] {
        match self {
            NewPayloadRequest::Bellatrix(req) => req.tree_hash_root().0,
            NewPayloadRequest::Capella(req) => req.tree_hash_root().0,
            NewPayloadRequest::Deneb(req) => req.tree_hash_root().0,
            NewPayloadRequest::Electra(req) => req.tree_hash_root().0,
        }
    }

    /// Returns the versioned hashes if this is a Deneb or Electra request.
    pub fn versioned_hashes(&self) -> Option<&VariableList<Hash32, MaxBlobCommitmentsPerBlock>> {
        match self {
            NewPayloadRequest::Bellatrix(_) | NewPayloadRequest::Capella(_) => None,
            NewPayloadRequest::Deneb(req) => Some(&req.versioned_hashes),
            NewPayloadRequest::Electra(req) => Some(&req.versioned_hashes),
        }
    }

    /// Returns the parent beacon block root if this is a Deneb or Electra request.
    pub fn parent_beacon_block_root(&self) -> Option<Hash32> {
        match self {
            NewPayloadRequest::Bellatrix(_) | NewPayloadRequest::Capella(_) => None,
            NewPayloadRequest::Deneb(req) => Some(req.parent_beacon_block_root),
            NewPayloadRequest::Electra(req) => Some(req.parent_beacon_block_root),
        }
    }

    /// Returns the execution requests if this is an Electra request.
    pub fn execution_requests(&self) -> Option<&ExecutionRequests> {
        match self {
            NewPayloadRequest::Electra(req) => Some(&req.execution_requests),
            _ => None,
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
