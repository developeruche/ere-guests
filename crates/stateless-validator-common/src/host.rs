//! Stateless validator common types and utilities for host.

use sha2::{Digest, Sha256};

use crate::guest::StatelessValidatorOutput;

impl StatelessValidatorOutput {
    /// Returns sha256 digest of serialized output.
    pub fn sha256(&self) -> [u8; 32] {
        Sha256::digest(self.serialize()).into()
    }
}
