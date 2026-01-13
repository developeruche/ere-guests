//! Implementations for host environment.

use alloc::{format, vec::Vec};

use alloy_eips::{Encodable2718, eip7685::Requests};
use alloy_primitives::U256;
use anyhow::Context;
use ere_zkvm_interface::Input;
use guest::{GuestIo, Io};
use reth_ethereum_primitives::TransactionSigned;
use reth_primitives_traits::Block;
pub use reth_stateless::StatelessInput;
use reth_stateless::UncompressedPublicKey;
use ssz_types::{FixedVector, VariableList};
use stateless_validator_common::execution_payload::{
    ExecutionPayloadV1, ExecutionPayloadV2, ExecutionPayloadV3, ForkName, NewPayloadRequest,
    Withdrawal,
};
pub use stateless_validator_common::guest::StatelessValidatorOutput;

use crate::{
    execution_payload::determine_fork_name,
    guest::{StatelessValidatorRethGuest, StatelessValidatorRethInput},
};

impl StatelessValidatorRethInput {
    /// Construct [`StatelessValidatorRethInput`] given [`StatelessInput`].
    pub fn new(stateless_input: &StatelessInput, requests: Requests) -> anyhow::Result<Self> {
        let new_payload_request = to_new_payload_request(stateless_input, requests)?;
        let signers = recover_signers(&stateless_input.block.body.transactions)?;

        Ok(Self {
            new_payload_request,
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

/// Converts a [`StatelessInput`] to a [`NewPayloadRequest`].
///
/// This creates the appropriate NewPayloadRequest variant based on the fork.
pub fn to_new_payload_request(
    stateless_input: &StatelessInput,
    requests: Requests,
) -> anyhow::Result<NewPayloadRequest> {
    use alloy_consensus::transaction::Transaction;

    let header = stateless_input.block.header();
    let body = stateless_input.block.body();
    let fork = determine_fork_name(&stateless_input.chain_config, header.timestamp);

    // Convert transactions to RLP-encoded bytes
    let transactions: Vec<Vec<u8>> = body
        .transactions()
        .map(|tx| {
            let mut buf = Vec::new();
            tx.encode_2718(&mut buf);
            buf
        })
        .collect();

    // Helper to convert alloy withdrawal to our neutral type
    let convert_withdrawal = |w: &alloy_eips::eip4895::Withdrawal| Withdrawal {
        index: w.index,
        validator_index: w.validator_index,
        address: w.address.0.0,
        amount: w.amount,
    };

    match fork {
        ForkName::Bellatrix => {
            let payload = ExecutionPayloadV1 {
                parent_hash: header.parent_hash.0,
                fee_recipient: header.beneficiary.0.0,
                state_root: header.state_root.0,
                receipts_root: header.receipts_root.0,
                logs_bloom: FixedVector::from(header.logs_bloom.0.to_vec()),
                prev_randao: header.mix_hash.0,
                block_number: header.number,
                gas_limit: header.gas_limit,
                gas_used: header.gas_used,
                timestamp: header.timestamp,
                extra_data: VariableList::from(header.extra_data.to_vec()),
                base_fee_per_gas: U256::from(header.base_fee_per_gas.unwrap_or_default())
                    .to_le_bytes(),
                block_hash: stateless_input.block.hash_slow().0,
                transactions,
            };
            Ok(NewPayloadRequest::new_bellatrix(payload))
        }
        ForkName::Capella => {
            let withdrawals: Vec<Withdrawal> = body
                .withdrawals
                .as_ref()
                .map(|ws| ws.iter().map(convert_withdrawal).collect())
                .unwrap_or_default();

            let payload = ExecutionPayloadV2 {
                parent_hash: header.parent_hash.0,
                fee_recipient: header.beneficiary.0.0,
                state_root: header.state_root.0,
                receipts_root: header.receipts_root.0,
                logs_bloom: FixedVector::from(header.logs_bloom.0.to_vec()),
                prev_randao: header.mix_hash.0,
                block_number: header.number,
                gas_limit: header.gas_limit,
                gas_used: header.gas_used,
                timestamp: header.timestamp,
                extra_data: VariableList::from(header.extra_data.to_vec()),
                base_fee_per_gas: U256::from(header.base_fee_per_gas.unwrap_or_default())
                    .to_le_bytes(),
                block_hash: stateless_input.block.hash_slow().0,
                transactions,
                withdrawals,
            };
            Ok(NewPayloadRequest::new_capella(payload))
        }
        ForkName::Deneb => {
            let withdrawals: Vec<Withdrawal> = body
                .withdrawals
                .as_ref()
                .map(|ws| ws.iter().map(convert_withdrawal).collect())
                .unwrap_or_default();

            let payload = ExecutionPayloadV3 {
                parent_hash: header.parent_hash.0,
                fee_recipient: header.beneficiary.0.0,
                state_root: header.state_root.0,
                receipts_root: header.receipts_root.0,
                logs_bloom: FixedVector::from(header.logs_bloom.0.to_vec()),
                prev_randao: header.mix_hash.0,
                block_number: header.number,
                gas_limit: header.gas_limit,
                gas_used: header.gas_used,
                timestamp: header.timestamp,
                extra_data: VariableList::from(header.extra_data.to_vec()),
                base_fee_per_gas: U256::from(header.base_fee_per_gas.unwrap_or_default())
                    .to_le_bytes(),
                block_hash: stateless_input.block.hash_slow().0,
                transactions,
                withdrawals,
                blob_gas_used: header.blob_gas_used.unwrap_or_default(),
                excess_blob_gas: header.excess_blob_gas.unwrap_or_default(),
            };

            // Collect blob versioned hashes from all blob transactions
            let versioned_hashes: Vec<[u8; 32]> = body
                .transactions()
                .filter_map(|tx| tx.blob_versioned_hashes())
                .flatten()
                .map(|h| h.0)
                .collect();

            let parent_beacon_block_root = stateless_input
                .block
                .parent_beacon_block_root
                .unwrap_or_default()
                .0;

            NewPayloadRequest::new_deneb(payload, versioned_hashes, parent_beacon_block_root)
        }
        ForkName::Electra => {
            let withdrawals: Vec<Withdrawal> = body
                .withdrawals
                .as_ref()
                .map(|ws| ws.iter().map(convert_withdrawal).collect())
                .unwrap_or_default();

            let payload = ExecutionPayloadV3 {
                parent_hash: header.parent_hash.0,
                fee_recipient: header.beneficiary.0.0,
                state_root: header.state_root.0,
                receipts_root: header.receipts_root.0,
                logs_bloom: FixedVector::from(header.logs_bloom.0.to_vec()),
                prev_randao: header.mix_hash.0,
                block_number: header.number,
                gas_limit: header.gas_limit,
                gas_used: header.gas_used,
                timestamp: header.timestamp,
                extra_data: VariableList::from(header.extra_data.to_vec()),
                base_fee_per_gas: U256::from(header.base_fee_per_gas.unwrap_or_default())
                    .to_le_bytes(),
                block_hash: stateless_input.block.hash_slow().0,
                transactions,
                withdrawals,
                blob_gas_used: header.blob_gas_used.unwrap_or_default(),
                excess_blob_gas: header.excess_blob_gas.unwrap_or_default(),
            };

            // Collect blob versioned hashes from all blob transactions
            let versioned_hashes: Vec<[u8; 32]> = body
                .transactions()
                .filter_map(|tx| tx.blob_versioned_hashes())
                .flatten()
                .map(|h| h.0)
                .collect();

            let parent_beacon_block_root = stateless_input
                .block
                .parent_beacon_block_root
                .unwrap_or_default()
                .0;

            NewPayloadRequest::new_electra(
                payload,
                versioned_hashes,
                parent_beacon_block_root,
                &requests,
            )
        }
    }
}

#[cfg(test)]
mod test {
    use stateless_validator_common::execution_payload::{ExecutionPayloadV1, NewPayloadRequest};

    use crate::guest::{Io, StatelessValidatorOutput, StatelessValidatorRethIo};

    #[test]
    fn serialize_output() {
        let dummy_new_payload_request = NewPayloadRequest::new_bellatrix(ExecutionPayloadV1 {
            parent_hash: [1; 32],
            fee_recipient: [2; 20],
            state_root: [3; 32],
            receipts_root: [4; 32],
            logs_bloom: Default::default(),
            prev_randao: [5; 32],
            block_number: 1,
            gas_limit: 2,
            gas_used: 3,
            timestamp: 4,
            extra_data: Default::default(),
            base_fee_per_gas: [6; 32],
            block_hash: [7; 32],
            transactions: vec![],
        });
        for output in [
            StatelessValidatorOutput::new(&dummy_new_payload_request, false),
            StatelessValidatorOutput::new(&dummy_new_payload_request, true),
        ] {
            assert_eq!(
                StatelessValidatorRethIo::serialize_output(&output).unwrap(),
                output.serialize()
            );
        }
    }
}
