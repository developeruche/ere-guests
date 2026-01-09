//! Stateless validator common types and utilities for host.

use alloy_genesis::ChainConfig;
use lighthouse_types::{
    Address as LighthouseAddress, EthSpec, ExecutionBlockHash, ExecutionPayload,
    ExecutionPayloadBellatrix, ExecutionPayloadCapella, ExecutionPayloadDeneb,
    ExecutionPayloadElectra, ExecutionPayloadHeader, FixedVector, ForkName, Hash256,
    MainnetEthSpec, Transactions, Uint256, VariableList, Withdrawal, Withdrawals,
};
use reth_primitives_traits::Block;
use sha2::{Digest, Sha256};
use tree_hash::TreeHash;

use crate::guest::StatelessValidatorOutput;

#[rustfmt::skip]
pub use reth_stateless::StatelessInput;

// Type alias for the execution payload with MainnetEthSpec
type MainnetExecutionPayload = ExecutionPayload<MainnetEthSpec>;

/// Determines the fork name based on chain config and block timestamp.
fn determine_fork_name(chain_config: &ChainConfig, timestamp: u64) -> ForkName {
    // Check forks in reverse chronological order
    if chain_config
        .prague_time
        .is_some_and(|prague_time| timestamp >= prague_time)
    {
        return ForkName::Electra;
    }
    if chain_config
        .cancun_time
        .is_some_and(|cancun_time| timestamp >= cancun_time)
    {
        return ForkName::Deneb;
    }
    if chain_config
        .shanghai_time
        .is_some_and(|shanghai_time| timestamp >= shanghai_time)
    {
        return ForkName::Capella;
    }
    // Default to Bellatrix for post-merge blocks
    ForkName::Bellatrix
}

/// Converts alloy B256 to lighthouse Hash256.
fn to_hash256(hash: alloy_primitives::B256) -> Hash256 {
    Hash256::from_slice(hash.as_slice())
}

/// Converts alloy B256 to lighthouse ExecutionBlockHash.
fn to_execution_block_hash(hash: alloy_primitives::B256) -> ExecutionBlockHash {
    ExecutionBlockHash::from_root(to_hash256(hash))
}

/// Converts alloy Address to lighthouse Address.
fn to_address(addr: alloy_primitives::Address) -> LighthouseAddress {
    LighthouseAddress::from_slice(addr.as_slice())
}

/// Converts alloy U256 to lighthouse Uint256.
fn to_uint256(value: alloy_primitives::U256) -> Uint256 {
    Uint256::from_le_bytes(value.to_le_bytes::<32>())
}

/// Converts alloy Bloom to lighthouse FixedVector for logs bloom.
fn to_logs_bloom<E: EthSpec>(
    bloom: &alloy_primitives::Bloom,
) -> FixedVector<u8, E::BytesPerLogsBloom> {
    FixedVector::from(bloom.as_slice().to_vec())
}

/// Converts alloy Bytes to lighthouse VariableList for extra data.
fn to_extra_data<E: EthSpec>(
    data: &alloy_primitives::Bytes,
) -> VariableList<u8, E::MaxExtraDataBytes> {
    VariableList::from(data.to_vec())
}

/// Converts transactions to lighthouse Transactions format (RLP encoded).
fn convert_transactions<'a, E: EthSpec>(
    txs: impl Iterator<Item = &'a reth_ethereum_primitives::TransactionSigned>,
) -> Transactions<E> {
    use alloy_rlp::Encodable;

    let encoded: Vec<_> = txs
        .map(|tx| {
            let mut buf = Vec::new();
            tx.encode(&mut buf);
            VariableList::from(buf)
        })
        .collect();
    VariableList::from(encoded)
}

/// Converts alloy withdrawals to lighthouse Withdrawals.
fn convert_withdrawals<E: EthSpec>(
    withdrawals: &[alloy_eips::eip4895::Withdrawal],
) -> Withdrawals<E> {
    let converted: Vec<_> = withdrawals
        .iter()
        .map(|w| Withdrawal {
            index: w.index,
            validator_index: w.validator_index,
            address: to_address(w.address),
            amount: w.amount,
        })
        .collect();
    VariableList::from(converted)
}

/// Converts a [`StatelessInput`] to a lighthouse [`ExecutionPayload`].
///
/// This function determines the appropriate fork variant based on the chain config
/// and block timestamp, then constructs the corresponding ExecutionPayload variant.
pub fn to_execution_payload(stateless_input: &StatelessInput) -> MainnetExecutionPayload {
    let header = stateless_input.block.header();
    let body = stateless_input.block.body();
    let fork = determine_fork_name(&stateless_input.chain_config, header.timestamp);

    // Common fields for all variants
    let parent_hash = to_execution_block_hash(header.parent_hash);
    let fee_recipient = to_address(header.beneficiary);
    let state_root = to_hash256(header.state_root);
    let receipts_root = to_hash256(header.receipts_root);
    let logs_bloom = to_logs_bloom::<MainnetEthSpec>(&header.logs_bloom);
    let prev_randao = to_hash256(header.mix_hash);
    let block_number = header.number;
    let gas_limit = header.gas_limit;
    let gas_used = header.gas_used;
    let timestamp = header.timestamp;
    let extra_data = to_extra_data::<MainnetEthSpec>(&header.extra_data);
    let base_fee_per_gas = to_uint256(alloy_primitives::U256::from(
        header.base_fee_per_gas.unwrap_or_default(),
    ));
    let block_hash = to_execution_block_hash(stateless_input.block.hash_slow());
    let transactions = convert_transactions::<MainnetEthSpec>(body.transactions());

    match fork {
        ForkName::Bellatrix => ExecutionPayload::Bellatrix(ExecutionPayloadBellatrix {
            parent_hash,
            fee_recipient,
            state_root,
            receipts_root,
            logs_bloom,
            prev_randao,
            block_number,
            gas_limit,
            gas_used,
            timestamp,
            extra_data,
            base_fee_per_gas,
            block_hash,
            transactions,
        }),
        ForkName::Capella => {
            let withdrawals = convert_withdrawals::<MainnetEthSpec>(
                body.withdrawals
                    .as_ref()
                    .map(|w| w.as_slice())
                    .unwrap_or(&[]),
            );
            ExecutionPayload::Capella(ExecutionPayloadCapella {
                parent_hash,
                fee_recipient,
                state_root,
                receipts_root,
                logs_bloom,
                prev_randao,
                block_number,
                gas_limit,
                gas_used,
                timestamp,
                extra_data,
                base_fee_per_gas,
                block_hash,
                transactions,
                withdrawals,
            })
        }
        ForkName::Deneb => {
            let withdrawals = convert_withdrawals::<MainnetEthSpec>(
                body.withdrawals
                    .as_ref()
                    .map(|w| w.as_slice())
                    .unwrap_or(&[]),
            );
            let blob_gas_used = header.blob_gas_used.unwrap_or_default();
            let excess_blob_gas = header.excess_blob_gas.unwrap_or_default();
            ExecutionPayload::Deneb(ExecutionPayloadDeneb {
                parent_hash,
                fee_recipient,
                state_root,
                receipts_root,
                logs_bloom,
                prev_randao,
                block_number,
                gas_limit,
                gas_used,
                timestamp,
                extra_data,
                base_fee_per_gas,
                block_hash,
                transactions,
                withdrawals,
                blob_gas_used,
                excess_blob_gas,
            })
        }
        ForkName::Electra | ForkName::Fulu | ForkName::Gloas => {
            let withdrawals = convert_withdrawals::<MainnetEthSpec>(
                body.withdrawals
                    .as_ref()
                    .map(|w| w.as_slice())
                    .unwrap_or(&[]),
            );
            let blob_gas_used = header.blob_gas_used.unwrap_or_default();
            let excess_blob_gas = header.excess_blob_gas.unwrap_or_default();
            ExecutionPayload::Electra(ExecutionPayloadElectra {
                parent_hash,
                fee_recipient,
                state_root,
                receipts_root,
                logs_bloom,
                prev_randao,
                block_number,
                gas_limit,
                gas_used,
                timestamp,
                extra_data,
                base_fee_per_gas,
                block_hash,
                transactions,
                withdrawals,
                blob_gas_used,
                excess_blob_gas,
            })
        }
        _ => panic!("unsupported fork: {fork}"),
    }
}

impl StatelessValidatorOutput {
    /// Constructs a output from [`StatelessInput`] and an bool indicating
    /// whehter the stateless validation is successful or not.
    pub fn from_stateless_input(stateless_input: &StatelessInput, success: bool) -> Self {
        let payload = to_execution_payload(stateless_input);
        let payload_header: ExecutionPayloadHeader<MainnetEthSpec> = payload.to_ref().into();
        let payload_header_hash = payload_header.tree_hash_root();

        let beacon_root = stateless_input
            .block
            .parent_beacon_block_root
            .unwrap_or_default();

        Self::new(
            stateless_input.block.hash_slow(),
            stateless_input.block.parent_hash,
            stateless_input
                .block
                .parent_beacon_block_root
                .unwrap_or_default(),
            success,
        )
    }

    /// Returns sha256 digest of serialized output.
    pub fn sha256(&self) -> [u8; 32] {
        Sha256::digest(self.serialize()).into()
    }
}
