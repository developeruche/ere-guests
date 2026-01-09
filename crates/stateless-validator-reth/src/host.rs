//! Implementations for host environment.

use alloc::{format, vec::Vec};

use anyhow::Context;
use ere_zkvm_interface::Input;
use guest::{GuestIo, Io};
use lighthouse_types::{ExecutionPayloadHeader, MainnetEthSpec};
use reth_ethereum_primitives::TransactionSigned;
use reth_stateless::UncompressedPublicKey;
use tree_hash::TreeHash;

use crate::execution_payload::to_execution_payload;
use crate::guest::{StatelessValidatorRethGuest, StatelessValidatorRethInput};

pub use crate::execution_payload::to_execution_payload as to_execution_payload_reth;
pub use reth_stateless::StatelessInput;
pub use stateless_validator_common::guest::StatelessValidatorOutput;

/// Constructs a [`StatelessValidatorOutput`] from [`StatelessInput`] and a success flag.
pub fn output_from_stateless_input(
    stateless_input: &StatelessInput,
    success: bool,
) -> StatelessValidatorOutput {
    let payload = to_execution_payload(stateless_input);
    let payload_header: ExecutionPayloadHeader<MainnetEthSpec> = payload.to_ref().into();
    let _payload_header_hash = payload_header.tree_hash_root();

    StatelessValidatorOutput::new(
        stateless_input.block.hash_slow(),
        stateless_input.block.parent_hash,
        stateless_input
            .block
            .parent_beacon_block_root
            .unwrap_or_default(),
        success,
    )
}

impl StatelessValidatorRethInput {
    /// Construct [`StatelessValidatorRethInput`] given [`StatelessInput`].
    pub fn new(stateless_input: &StatelessInput) -> anyhow::Result<Self> {
        let signers = recover_signers(&stateless_input.block.body.transactions)?;

        Ok(Self {
            stateless_input: stateless_input.clone(),
            public_keys: signers,
        })
    }

    /// Returns [`Input`] to [`zkVM`] methods.
    ///
    /// [`zkVM`]: ere_zkvm_interface::zkVM
    pub fn to_zkvm_input(&self) -> anyhow::Result<Input> {
        let stdin = GuestIo::<StatelessValidatorRethGuest>::serialize_input(self)?;
        Ok(Input::new().with_prefixed_stdin(stdin))
    }
}

/// Recover public keys from transaction signatures.
pub fn recover_signers<'a, I>(txs: I) -> anyhow::Result<Vec<UncompressedPublicKey>>
where
    I: IntoIterator<Item = &'a TransactionSigned>,
{
    txs.into_iter()
        .enumerate()
        .map(|(i, tx)| {
            tx.signature()
                .recover_from_prehash(&tx.signature_hash())
                .map(|key| key.to_encoded_point(false).as_bytes().try_into().unwrap())
                .map(UncompressedPublicKey)
                .with_context(|| format!("failed to recover signature for tx #{i}"))
        })
        .collect()
}

#[cfg(test)]
mod test {
    use crate::guest::{Io, StatelessValidatorOutput, StatelessValidatorRethIo};

    #[test]
    fn serialize_output() {
        for output in [
            StatelessValidatorOutput::new([0x00; 32], [0x00; 32], [0x00; 32], false),
            StatelessValidatorOutput::new([0xff; 32], [0xff; 32], [0xff; 32], true),
        ] {
            assert_eq!(
                StatelessValidatorRethIo::serialize_output(&output).unwrap(),
                output.serialize()
            );
        }
    }
}
