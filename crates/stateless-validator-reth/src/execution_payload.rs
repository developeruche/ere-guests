//! Reth -> ExecutionPayload conversion.

use alloc::vec::Vec;

use alloy_genesis::ChainConfig;
use alloy_rlp::Encodable;
use lighthouse_types::{
    Address as LighthouseAddress, EthSpec, ExecutionBlockHash, ExecutionPayload, FixedVector,
    ForkName, Hash256, MainnetEthSpec, Transactions, Uint256, VariableList, Withdrawal,
    Withdrawals,
};
use reth_primitives_traits::Block;
use reth_stateless::StatelessInput;
use stateless_validator_common::execution_payload::ExecutionPayloadFields;

/// Determines the fork name based on alloy chain config and block timestamp.
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
fn to_logs_bloom(
    bloom: &alloy_primitives::Bloom,
) -> FixedVector<u8, <MainnetEthSpec as EthSpec>::BytesPerLogsBloom> {
    FixedVector::from(bloom.as_slice().to_vec())
}

/// Converts alloy Bytes to lighthouse VariableList for extra data.
fn to_extra_data(
    data: &alloy_primitives::Bytes,
) -> VariableList<u8, <MainnetEthSpec as EthSpec>::MaxExtraDataBytes> {
    VariableList::from(data.to_vec())
}

/// Converts reth transactions to lighthouse Transactions format (RLP encoded).
fn convert_transactions<'a>(
    txs: impl Iterator<Item = &'a reth_ethereum_primitives::TransactionSigned>,
) -> Transactions<MainnetEthSpec> {
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
fn convert_withdrawals(
    withdrawals: &[alloy_eips::eip4895::Withdrawal],
) -> Withdrawals<MainnetEthSpec> {
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

/// Creates [`ExecutionPayloadFields`] from a reth [`StatelessInput`].
fn execution_payload_fields_from_reth(stateless_input: &StatelessInput) -> ExecutionPayloadFields {
    let header = stateless_input.block.header();
    let body = stateless_input.block.body();
    let fork = determine_fork_name(&stateless_input.chain_config, header.timestamp);

    let withdrawals = body
        .withdrawals
        .as_ref()
        .map(|w| convert_withdrawals(w.as_slice()));

    ExecutionPayloadFields {
        fork,
        parent_hash: to_execution_block_hash(header.parent_hash),
        fee_recipient: to_address(header.beneficiary),
        state_root: to_hash256(header.state_root),
        receipts_root: to_hash256(header.receipts_root),
        logs_bloom: to_logs_bloom(&header.logs_bloom),
        prev_randao: to_hash256(header.mix_hash),
        block_number: header.number,
        gas_limit: header.gas_limit,
        gas_used: header.gas_used,
        timestamp: header.timestamp,
        extra_data: to_extra_data(&header.extra_data),
        base_fee_per_gas: to_uint256(alloy_primitives::U256::from(
            header.base_fee_per_gas.unwrap_or_default(),
        )),
        block_hash: to_execution_block_hash(stateless_input.block.hash_slow()),
        transactions: convert_transactions(body.transactions()),
        withdrawals,
        blob_gas_used: header.blob_gas_used,
        excess_blob_gas: header.excess_blob_gas,
    }
}

/// Converts a [`StatelessInput`] to a lighthouse [`ExecutionPayload`].
///
/// This function determines the appropriate fork variant based on the chain config
/// and block timestamp, then constructs the corresponding ExecutionPayload variant.
pub fn to_execution_payload(stateless_input: &StatelessInput) -> ExecutionPayload<MainnetEthSpec> {
    execution_payload_fields_from_reth(stateless_input).into_payload()
}
