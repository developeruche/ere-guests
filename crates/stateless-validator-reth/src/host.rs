//! Implementations for host environment.

use alloc::{format, vec::Vec};

use alloy_eips::Encodable2718;
use alloy_primitives::{Bytes, U256};
use alloy_rpc_types_engine::{
    CancunPayloadFields, ExecutionData, ExecutionPayload as AlloyExecutionPayload,
    ExecutionPayloadSidecar, ExecutionPayloadV1, ExecutionPayloadV2, ExecutionPayloadV3,
    PraguePayloadFields,
};
use anyhow::Context;
use ere_zkvm_interface::Input;
use guest::{GuestIo, Io};
use reth_ethereum_primitives::TransactionSigned;
use reth_primitives_traits::Block;
pub use reth_stateless::StatelessInput;
use reth_stateless::UncompressedPublicKey;
use stateless_validator_common::execution_payload::ForkName;
pub use stateless_validator_common::guest::StatelessValidatorOutput;

use crate::{
    execution_payload::determine_fork_name,
    guest::{StatelessValidatorRethGuest, StatelessValidatorRethInput},
};

impl StatelessValidatorRethInput {
    /// Construct [`StatelessValidatorRethInput`] given [`StatelessInput`].
    pub fn new(stateless_input: &StatelessInput) -> anyhow::Result<Self> {
        let execution_data = to_execution_data(stateless_input);
        let signers = recover_signers(&stateless_input.block.body.transactions)?;

        Ok(Self {
            execution_data,
            witness: stateless_input.witness.clone(),
            chain_config: stateless_input.chain_config.clone(),
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

/// Converts a [`StatelessInput`] to an alloy [`ExecutionData`].
///
/// This creates both the execution payload and the appropriate sidecar
/// based on the fork (pre-Cancun, Cancun/Deneb, or Prague/Electra).
pub fn to_execution_data(stateless_input: &StatelessInput) -> ExecutionData {
    // TODO: move to host.rs?
    use alloy_consensus::transaction::Transaction;

    let header = stateless_input.block.header();
    let body = stateless_input.block.body();
    let fork = determine_fork_name(&stateless_input.chain_config, header.timestamp);

    // Convert transactions to RLP-encoded bytes
    let transactions: Vec<Bytes> = body
        .transactions()
        .map(|tx| {
            let mut buf = Vec::new();
            tx.encode_2718(&mut buf);
            buf.into()
        })
        .collect();

    // Build the base V1 payload
    let v1 = ExecutionPayloadV1 {
        parent_hash: header.parent_hash,
        fee_recipient: header.beneficiary,
        state_root: header.state_root,
        receipts_root: header.receipts_root,
        logs_bloom: header.logs_bloom,
        prev_randao: header.mix_hash,
        block_number: header.number,
        gas_limit: header.gas_limit,
        gas_used: header.gas_used,
        timestamp: header.timestamp,
        extra_data: header.extra_data.clone(),
        base_fee_per_gas: U256::from(header.base_fee_per_gas.unwrap_or_default()),
        block_hash: stateless_input.block.hash_slow(),
        transactions,
    };

    // Build payload and sidecar based on fork
    let (payload, sidecar) = match fork {
        ForkName::Bellatrix => (
            AlloyExecutionPayload::V1(v1),
            ExecutionPayloadSidecar::none(),
        ),
        ForkName::Capella => {
            let withdrawals = body
                .withdrawals
                .as_ref()
                .map(|w| w.to_vec())
                .unwrap_or_default();
            let v2 = ExecutionPayloadV2 {
                payload_inner: v1,
                withdrawals,
            };
            (
                AlloyExecutionPayload::V2(v2),
                ExecutionPayloadSidecar::none(),
            )
        }
        ForkName::Deneb => {
            let withdrawals = body
                .withdrawals
                .as_ref()
                .map(|w| w.to_vec())
                .unwrap_or_default();
            let v3 = ExecutionPayloadV3 {
                payload_inner: ExecutionPayloadV2 {
                    payload_inner: v1,
                    withdrawals,
                },
                blob_gas_used: header.blob_gas_used.unwrap_or_default(),
                excess_blob_gas: header.excess_blob_gas.unwrap_or_default(),
            };

            // Collect blob versioned hashes from all blob transactions
            let versioned_hashes: Vec<_> = body
                .transactions()
                .filter_map(|tx| tx.blob_versioned_hashes())
                .flatten()
                .copied()
                .collect();

            let parent_beacon_block_root = stateless_input
                .block
                .parent_beacon_block_root
                .unwrap_or_default();

            let cancun_fields =
                CancunPayloadFields::new(parent_beacon_block_root, versioned_hashes);
            let sidecar = ExecutionPayloadSidecar::v3(cancun_fields);

            (AlloyExecutionPayload::V3(v3), sidecar)
        }
        ForkName::Electra => {
            let withdrawals = body
                .withdrawals
                .as_ref()
                .map(|w| w.to_vec())
                .unwrap_or_default();
            let v3 = ExecutionPayloadV3 {
                payload_inner: ExecutionPayloadV2 {
                    payload_inner: v1,
                    withdrawals,
                },
                blob_gas_used: header.blob_gas_used.unwrap_or_default(),
                excess_blob_gas: header.excess_blob_gas.unwrap_or_default(),
            };

            // Collect blob versioned hashes from all blob transactions
            let versioned_hashes: Vec<_> = body
                .transactions()
                .filter_map(|tx| tx.blob_versioned_hashes())
                .flatten()
                .copied()
                .collect();

            let parent_beacon_block_root = stateless_input
                .block
                .parent_beacon_block_root
                .unwrap_or_default();

            let cancun_fields =
                CancunPayloadFields::new(parent_beacon_block_root, versioned_hashes);

            // For Electra, include the requests_hash in the sidecar
            let requests_hash = header.requests_hash.unwrap_or_default();
            let prague_fields = PraguePayloadFields::new(requests_hash);
            let sidecar = ExecutionPayloadSidecar::v4(cancun_fields, prague_fields);

            (AlloyExecutionPayload::V3(v3), sidecar)
        }
    };

    ExecutionData { payload, sidecar }
}

#[cfg(test)]
mod test {
    use crate::guest::{Io, StatelessValidatorOutput, StatelessValidatorRethIo};

    #[test]
    fn serialize_output() {
        for output in [
            StatelessValidatorOutput::new([0x00; 32], [0x00; 32], false),
            StatelessValidatorOutput::new([0xff; 32], [0xff; 32], true),
        ] {
            assert_eq!(
                StatelessValidatorRethIo::serialize_output(&output).unwrap(),
                output.serialize()
            );
        }
    }
}
