//! Consensus types to support new payload requests.

#![allow(missing_docs)]

use alloc::{vec, vec::Vec};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use ssz_types::{FixedVector, VariableList};
use tree_hash::TreeHash;
use tree_hash_derive::TreeHash;
use typenum::Prod;

/// Helper module for serializing fixed-size byte arrays as hex strings.
#[cfg(feature = "serde")]
mod bytes_hex {
    use alloc::vec::Vec;
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub(crate) fn serialize<S: Serializer, const N: usize>(
        bytes: &[u8; N],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&const_hex::encode_prefixed(bytes))
        } else {
            serializer.serialize_bytes(bytes)
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        deserializer: D,
    ) -> Result<[u8; N], D::Error> {
        if deserializer.is_human_readable() {
            let s: &str = Deserialize::deserialize(deserializer)?;
            let bytes = const_hex::decode(s).map_err(D::Error::custom)?;
            bytes.try_into().map_err(|v: Vec<u8>| {
                D::Error::custom(alloc::format!("expected {} bytes, got {}", N, v.len()))
            })
        } else {
            let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
            bytes.try_into().map_err(|v: Vec<u8>| {
                D::Error::custom(alloc::format!("expected {} bytes, got {}", N, v.len()))
            })
        }
    }
}

/// Primitive types
pub type Hash32 = [u8; 32];
pub type Bytes48 = [u8; 48];
pub type Bytes96 = [u8; 96];
pub type Address20 = [u8; 20];
pub type Uint256Bytes = [u8; 32];
pub type LogsBloom = FixedVector<u8, typenum::U256>;
pub type ExtraData = VariableList<u8, typenum::U32>;

/// Limits
pub type MaxBytesPerTransaction = Prod<MaxTransactionsPerPayload, typenum::U1024>; // 2^30
pub type MaxWithdrawalsPerPayload = typenum::U16; // 16
pub type MaxTransactionsPerPayload = Prod<typenum::U1024, typenum::U1024>; // 2^20
pub type MaxBlobCommitmentsPerBlock = typenum::U4096; // 4096
pub type MaxDepositRequestsPerPayload = typenum::U8192; // 2^13
pub type MaxWithdrawalRequestsPerPayload = typenum::U16; // 2^4
pub type MaxConsolidationRequestsPerPayload = typenum::U2; // 2^1

/// Composite types
pub type Transaction = VariableList<u8, MaxBytesPerTransaction>;
pub type Transactions = VariableList<Transaction, MaxTransactionsPerPayload>;
pub type Withdrawals = VariableList<Withdrawal, MaxWithdrawalsPerPayload>;

#[derive(Debug, Clone, TreeHash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Withdrawal {
    pub index: u64,
    pub validator_index: u64,
    pub address: Address20,
    pub amount: u64,
}

#[derive(Debug, Clone, TreeHash, ssz_derive::Encode, ssz_derive::Decode)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DepositRequest {
    #[cfg_attr(feature = "serde", serde(with = "bytes_hex"))]
    pub pubkey: Bytes48,
    pub withdrawal_credentials: Hash32,
    pub amount: u64,
    #[cfg_attr(feature = "serde", serde(with = "bytes_hex"))]
    pub signature: Bytes96,
    pub index: u64,
}

#[derive(Debug, Clone, TreeHash, ssz_derive::Encode, ssz_derive::Decode)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct WithdrawalRequest {
    pub source_address: Address20,
    #[cfg_attr(feature = "serde", serde(with = "bytes_hex"))]
    pub validator_pubkey: Bytes48,
    pub amount: u64,
}

#[derive(Debug, Clone, TreeHash, ssz_derive::Encode, ssz_derive::Decode)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ConsolidationRequest {
    pub source_address: Address20,
    #[cfg_attr(feature = "serde", serde(with = "bytes_hex"))]
    pub source_pubkey: Bytes48,
    #[cfg_attr(feature = "serde", serde(with = "bytes_hex"))]
    pub target_pubkey: Bytes48,
}

#[derive(Debug, Clone, Default, TreeHash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ExecutionRequests {
    pub deposits: VariableList<DepositRequest, MaxDepositRequestsPerPayload>,
    pub withdrawals: VariableList<WithdrawalRequest, MaxWithdrawalRequestsPerPayload>,
    pub consolidations: VariableList<ConsolidationRequest, MaxConsolidationRequestsPerPayload>,
}

#[derive(Debug, Clone, Copy)]
pub enum ForkName {
    Bellatrix,
    Capella,
    Deneb,
    Electra,
    Fulu,
}

#[derive(Debug, Clone, TreeHash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    pub transactions: Transactions,
}

#[derive(Debug, Clone, TreeHash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    pub transactions: Transactions,
    pub withdrawals: Withdrawals,
}

#[derive(Debug, Clone, TreeHash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    pub transactions: Transactions,
    pub withdrawals: Withdrawals,
    pub blob_gas_used: u64,
    pub excess_blob_gas: u64,
}

#[derive(Debug, Clone, TreeHash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NewPayloadRequestBellatrix {
    pub execution_payload: ExecutionPayloadV1,
}

#[derive(Debug, Clone, TreeHash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NewPayloadRequestCapella {
    pub execution_payload: ExecutionPayloadV2,
}

#[derive(Debug, Clone, TreeHash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NewPayloadRequestDeneb {
    pub execution_payload: ExecutionPayloadV3,
    pub versioned_hashes: VariableList<Hash32, MaxBlobCommitmentsPerBlock>,
    pub parent_beacon_block_root: Hash32,
}

#[derive(Debug, Clone, TreeHash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NewPayloadRequestElectraFulu {
    pub execution_payload: ExecutionPayloadV3,
    pub versioned_hashes: VariableList<Hash32, MaxBlobCommitmentsPerBlock>,
    pub parent_beacon_block_root: Hash32,
    pub execution_requests: ExecutionRequests,
}

#[derive(Debug, Clone, TreeHash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NewPayloadRequestFulu {
    pub execution_payload: ExecutionPayloadV3,
    pub versioned_hashes: VariableList<Hash32, MaxBlobCommitmentsPerBlock>,
    pub parent_beacon_block_root: Hash32,
    pub execution_requests: ExecutionRequests,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NewPayloadRequest {
    Bellatrix(NewPayloadRequestBellatrix),
    Capella(NewPayloadRequestCapella),
    Deneb(NewPayloadRequestDeneb),
    ElectraFulu(NewPayloadRequestElectraFulu),
}

impl NewPayloadRequest {
    pub fn tree_hash_root(&self) -> [u8; 32] {
        match self {
            NewPayloadRequest::Bellatrix(req) => req.tree_hash_root().0,
            NewPayloadRequest::Capella(req) => req.tree_hash_root().0,
            NewPayloadRequest::Deneb(req) => req.tree_hash_root().0,
            NewPayloadRequest::ElectraFulu(req) => req.tree_hash_root().0,
        }
    }
}

/// Computes the requests hash for EL block construction.
pub fn compute_requests_hash(requests: &ExecutionRequests) -> [u8; 32] {
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

    outer_hasher.finalize().into()
}
