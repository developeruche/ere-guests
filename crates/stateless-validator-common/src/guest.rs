//! Stateless validator common types and utilities for guest.

use lighthouse_types::{ExecutionPayload, ExecutionPayloadHeader, MainnetEthSpec};
use tree_hash::TreeHash;

/// Static size of [`StatelessValidatorOutput`].
pub const STATELESS_VALIDATOR_OUTPUT_SIZE: usize = size_of::<StatelessValidatorOutput>();

/// Output of stateless validator guest program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatelessValidatorOutput {
    /// Execution Payload Header hash
    pub execution_payload_header_hash: [u8; 32],
    /// Beacon root
    pub beacon_root: [u8; 32],
    /// Stateless validation is successful or not.
    pub successful_block_validation: bool,
}

impl StatelessValidatorOutput {
    /// Constructs a new [`StatelessValidatorOutput`].
    pub fn new(
        execution_payload_header_hash: impl Into<[u8; 32]>,
        beacon_root: impl Into<[u8; 32]>,
        successful_block_validation: bool,
    ) -> Self {
        Self {
            execution_payload_header_hash: execution_payload_header_hash.into(),
            beacon_root: beacon_root.into(),
            successful_block_validation,
        }
    }

    /// Returns serialized output.
    pub fn serialize(&self) -> [u8; STATELESS_VALIDATOR_OUTPUT_SIZE] {
        let mut buf = [0; STATELESS_VALIDATOR_OUTPUT_SIZE];
        buf[0..32].copy_from_slice(&self.execution_payload_header_hash);
        buf[32..64].copy_from_slice(&self.beacon_root);
        buf[64] = self.successful_block_validation as u8;
        buf
    }
}

/// Computes the execution payload header hash from the execution payload.
pub fn execution_payload_to_header_hash(
    execution_payload: &ExecutionPayload<MainnetEthSpec>,
) -> [u8; 32] {
    let execution_payload_header: ExecutionPayloadHeader<MainnetEthSpec> =
        execution_payload.to_ref().into();
    execution_payload_header.tree_hash_root().into()
}
