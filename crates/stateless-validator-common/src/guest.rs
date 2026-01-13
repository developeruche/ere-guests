//! Stateless validator common types and utilities for guest.

use crate::execution_payload::NewPayloadRequest;

/// Static size of [`StatelessValidatorOutput`].
pub const STATELESS_VALIDATOR_OUTPUT_SIZE: usize = size_of::<StatelessValidatorOutput>();

/// Output of stateless validator guest program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatelessValidatorOutput {
    /// New execution payload request root.
    pub new_payload_request_root: [u8; 32],
    /// Stateless validation is successful or not.
    pub successful_block_validation: bool,
}

impl StatelessValidatorOutput {
    /// Constructs a new [`StatelessValidatorOutput`].
    pub fn new(new_payload_request: &NewPayloadRequest, successful_block_validation: bool) -> Self {
        let new_payload_request_root = new_payload_request.tree_hash_root();
        Self {
            new_payload_request_root,
            successful_block_validation,
        }
    }

    /// Returns serialized output.
    pub fn serialize(&self) -> [u8; STATELESS_VALIDATOR_OUTPUT_SIZE] {
        let mut buf = [0; STATELESS_VALIDATOR_OUTPUT_SIZE];
        buf[0..32].copy_from_slice(&self.new_payload_request_root);
        buf[32] = self.successful_block_validation as u8;
        buf
    }
}
