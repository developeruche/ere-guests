//! Bincode-compatible serde implementations for `alloy-rpc-types-engine` types.
//!
//! The standard serde implementations for some types use `#[serde(flatten)]` and
//! `#[serde(untagged)]` which are incompatible with bincode serialization.
//! This module provides wrapper types only for the problematic types.
//!
//! Types that need wrappers:
//! - `ExecutionPayload` - uses `#[serde(untagged)]`
//! - `ExecutionPayloadV2` - uses `#[serde(flatten)]` on `payload_inner`
//! - `ExecutionPayloadV3` - uses `#[serde(flatten)]` on `payload_inner`
//! - `RequestsOrHash` - uses `#[serde(untagged)]`
//!
//! Types that don't need wrappers but contain problematic nested types:
//! - `ExecutionPayloadSidecar` - contains `PraguePayloadFields` → `RequestsOrHash`
//! - `PraguePayloadFields` - contains `RequestsOrHash`

use alloc::vec::Vec;

use alloy_eips::{
    eip4895::Withdrawal,
    eip7685::{Requests, RequestsOrHash},
};
use alloy_primitives::{Address, B256, Bloom, Bytes, U256};
pub use alloy_rpc_types_engine::ExecutionData;
use alloy_rpc_types_engine::{
    CancunPayloadFields, ExecutionPayloadSidecar, ExecutionPayloadV1, ExecutionPayloadV2,
    ExecutionPayloadV3, PraguePayloadFields,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{DeserializeAs, SerializeAs};

/// Bincode-compatible [`ExecutionData`] serde implementation.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionDataCompat {
    payload: ExecutionPayloadCompat,
    sidecar: ExecutionPayloadSidecarCompat,
}

impl SerializeAs<ExecutionData> for ExecutionDataCompat {
    fn serialize_as<S>(value: &ExecutionData, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ExecutionDataCompat::from(value).serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, ExecutionData> for ExecutionDataCompat {
    fn deserialize_as<D>(deserializer: D) -> Result<ExecutionData, D::Error>
    where
        D: Deserializer<'de>,
    {
        ExecutionDataCompat::deserialize(deserializer).map(Into::into)
    }
}

impl From<&ExecutionData> for ExecutionDataCompat {
    fn from(value: &ExecutionData) -> Self {
        Self {
            payload: ExecutionPayloadCompat::from(&value.payload),
            sidecar: ExecutionPayloadSidecarCompat::from(&value.sidecar),
        }
    }
}

impl From<ExecutionDataCompat> for ExecutionData {
    fn from(value: ExecutionDataCompat) -> Self {
        Self::new(value.payload.into(), value.sidecar.into())
    }
}

/// Bincode-compatible [`ExecutionPayload`] serde implementation.
///
/// The original uses `#[serde(untagged)]` which is incompatible with bincode.
/// This version uses explicit enum discriminants.
#[derive(Debug, Serialize, Deserialize)]
pub enum ExecutionPayloadCompat {
    /// V1 payload (no flatten issues, use original type directly)
    V1(ExecutionPayloadV1),
    /// V2 payload (has flatten, needs compat wrapper)
    V2(ExecutionPayloadV2Compat),
    /// V3 payload (has flatten, needs compat wrapper)
    V3(ExecutionPayloadV3Compat),
}

impl From<&alloy_rpc_types_engine::ExecutionPayload> for ExecutionPayloadCompat {
    fn from(value: &alloy_rpc_types_engine::ExecutionPayload) -> Self {
        match value {
            alloy_rpc_types_engine::ExecutionPayload::V1(v1) => Self::V1(v1.clone()),
            alloy_rpc_types_engine::ExecutionPayload::V2(v2) => {
                Self::V2(ExecutionPayloadV2Compat::from(v2))
            }
            alloy_rpc_types_engine::ExecutionPayload::V3(v3) => {
                Self::V3(ExecutionPayloadV3Compat::from(v3))
            }
        }
    }
}

impl From<ExecutionPayloadCompat> for alloy_rpc_types_engine::ExecutionPayload {
    fn from(value: ExecutionPayloadCompat) -> Self {
        match value {
            ExecutionPayloadCompat::V1(v1) => Self::V1(v1),
            ExecutionPayloadCompat::V2(v2) => Self::V2(v2.into()),
            ExecutionPayloadCompat::V3(v3) => Self::V3(v3.into()),
        }
    }
}

/// Bincode-compatible [`ExecutionPayloadV2`] serde implementation.
///
/// The original uses `#[serde(flatten)]` on `payload_inner` which is incompatible with bincode.
/// This version inlines all V1 fields.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionPayloadV2Compat {
    // V1 fields (inlined instead of flattened)
    parent_hash: B256,
    fee_recipient: Address,
    state_root: B256,
    receipts_root: B256,
    logs_bloom: Bloom,
    prev_randao: B256,
    block_number: u64,
    gas_limit: u64,
    gas_used: u64,
    timestamp: u64,
    extra_data: Bytes,
    base_fee_per_gas: U256,
    block_hash: B256,
    transactions: Vec<Bytes>,
    // V2 fields
    withdrawals: Vec<Withdrawal>,
}

impl From<&ExecutionPayloadV2> for ExecutionPayloadV2Compat {
    fn from(value: &ExecutionPayloadV2) -> Self {
        let v1 = &value.payload_inner;
        Self {
            parent_hash: v1.parent_hash,
            fee_recipient: v1.fee_recipient,
            state_root: v1.state_root,
            receipts_root: v1.receipts_root,
            logs_bloom: v1.logs_bloom,
            prev_randao: v1.prev_randao,
            block_number: v1.block_number,
            gas_limit: v1.gas_limit,
            gas_used: v1.gas_used,
            timestamp: v1.timestamp,
            extra_data: v1.extra_data.clone(),
            base_fee_per_gas: v1.base_fee_per_gas,
            block_hash: v1.block_hash,
            transactions: v1.transactions.clone(),
            withdrawals: value.withdrawals.clone(),
        }
    }
}

impl From<ExecutionPayloadV2Compat> for ExecutionPayloadV2 {
    fn from(value: ExecutionPayloadV2Compat) -> Self {
        Self {
            payload_inner: ExecutionPayloadV1 {
                parent_hash: value.parent_hash,
                fee_recipient: value.fee_recipient,
                state_root: value.state_root,
                receipts_root: value.receipts_root,
                logs_bloom: value.logs_bloom,
                prev_randao: value.prev_randao,
                block_number: value.block_number,
                gas_limit: value.gas_limit,
                gas_used: value.gas_used,
                timestamp: value.timestamp,
                extra_data: value.extra_data,
                base_fee_per_gas: value.base_fee_per_gas,
                block_hash: value.block_hash,
                transactions: value.transactions,
            },
            withdrawals: value.withdrawals,
        }
    }
}

/// Bincode-compatible [`ExecutionPayloadV3`] serde implementation.
///
/// The original uses `#[serde(flatten)]` on `payload_inner` which is incompatible with bincode.
/// This version inlines all V1 and V2 fields.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionPayloadV3Compat {
    // V1 fields (inlined)
    parent_hash: B256,
    fee_recipient: Address,
    state_root: B256,
    receipts_root: B256,
    logs_bloom: Bloom,
    prev_randao: B256,
    block_number: u64,
    gas_limit: u64,
    gas_used: u64,
    timestamp: u64,
    extra_data: Bytes,
    base_fee_per_gas: U256,
    block_hash: B256,
    transactions: Vec<Bytes>,
    // V2 fields (inlined)
    withdrawals: Vec<Withdrawal>,
    // V3 fields
    blob_gas_used: u64,
    excess_blob_gas: u64,
}

impl From<&ExecutionPayloadV3> for ExecutionPayloadV3Compat {
    fn from(value: &ExecutionPayloadV3) -> Self {
        let v2 = &value.payload_inner;
        let v1 = &v2.payload_inner;
        Self {
            parent_hash: v1.parent_hash,
            fee_recipient: v1.fee_recipient,
            state_root: v1.state_root,
            receipts_root: v1.receipts_root,
            logs_bloom: v1.logs_bloom,
            prev_randao: v1.prev_randao,
            block_number: v1.block_number,
            gas_limit: v1.gas_limit,
            gas_used: v1.gas_used,
            timestamp: v1.timestamp,
            extra_data: v1.extra_data.clone(),
            base_fee_per_gas: v1.base_fee_per_gas,
            block_hash: v1.block_hash,
            transactions: v1.transactions.clone(),
            withdrawals: v2.withdrawals.clone(),
            blob_gas_used: value.blob_gas_used,
            excess_blob_gas: value.excess_blob_gas,
        }
    }
}

impl From<ExecutionPayloadV3Compat> for ExecutionPayloadV3 {
    fn from(value: ExecutionPayloadV3Compat) -> Self {
        Self {
            payload_inner: ExecutionPayloadV2 {
                payload_inner: ExecutionPayloadV1 {
                    parent_hash: value.parent_hash,
                    fee_recipient: value.fee_recipient,
                    state_root: value.state_root,
                    receipts_root: value.receipts_root,
                    logs_bloom: value.logs_bloom,
                    prev_randao: value.prev_randao,
                    block_number: value.block_number,
                    gas_limit: value.gas_limit,
                    gas_used: value.gas_used,
                    timestamp: value.timestamp,
                    extra_data: value.extra_data,
                    base_fee_per_gas: value.base_fee_per_gas,
                    block_hash: value.block_hash,
                    transactions: value.transactions,
                },
                withdrawals: value.withdrawals,
            },
            blob_gas_used: value.blob_gas_used,
            excess_blob_gas: value.excess_blob_gas,
        }
    }
}

/// Bincode-compatible [`ExecutionPayloadSidecar`] serde implementation.
///
/// The sidecar itself doesn't use flatten/untagged, but it contains
/// `PraguePayloadFields` which contains `RequestsOrHash` (untagged).
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionPayloadSidecarCompat {
    // CancunPayloadFields doesn't have any problematic attributes, use directly
    cancun: Option<CancunPayloadFields>,
    // PraguePayloadFields contains RequestsOrHash which uses untagged
    prague: Option<PraguePayloadFieldsCompat>,
}

impl From<&ExecutionPayloadSidecar> for ExecutionPayloadSidecarCompat {
    fn from(value: &ExecutionPayloadSidecar) -> Self {
        Self {
            cancun: value.cancun().cloned(),
            prague: value.prague().map(PraguePayloadFieldsCompat::from),
        }
    }
}

impl From<ExecutionPayloadSidecarCompat> for ExecutionPayloadSidecar {
    fn from(value: ExecutionPayloadSidecarCompat) -> Self {
        let prague: Option<PraguePayloadFields> = value.prague.map(Into::into);
        match (value.cancun, prague) {
            (Some(c), Some(p)) => Self::v4(c, p),
            (Some(c), None) => Self::v3(c),
            _ => Self::none(),
        }
    }
}

/// Bincode-compatible [`PraguePayloadFields`] serde implementation.
///
/// Contains `RequestsOrHash` which uses `#[serde(untagged)]`.
#[derive(Debug, Serialize, Deserialize)]
pub struct PraguePayloadFieldsCompat {
    requests: RequestsOrHashCompat,
}

impl From<&PraguePayloadFields> for PraguePayloadFieldsCompat {
    fn from(value: &PraguePayloadFields) -> Self {
        Self {
            requests: RequestsOrHashCompat::from(&value.requests),
        }
    }
}

impl From<PraguePayloadFieldsCompat> for PraguePayloadFields {
    fn from(value: PraguePayloadFieldsCompat) -> Self {
        Self::new(value.requests)
    }
}

/// Bincode-compatible [`RequestsOrHash`] serde implementation.
///
/// The original uses `#[serde(untagged)]` which is incompatible with bincode.
/// This version uses explicit enum discriminants.
#[derive(Debug, Serialize, Deserialize)]
pub enum RequestsOrHashCompat {
    /// List of requests
    Requests(Requests),
    /// Precomputed hash
    Hash(B256),
}

impl From<&RequestsOrHash> for RequestsOrHashCompat {
    fn from(value: &RequestsOrHash) -> Self {
        match value {
            RequestsOrHash::Requests(r) => Self::Requests(r.clone()),
            RequestsOrHash::Hash(h) => Self::Hash(*h),
        }
    }
}

impl From<RequestsOrHashCompat> for RequestsOrHash {
    fn from(value: RequestsOrHashCompat) -> Self {
        match value {
            RequestsOrHashCompat::Requests(r) => Self::Requests(r),
            RequestsOrHashCompat::Hash(h) => Self::Hash(h),
        }
    }
}
