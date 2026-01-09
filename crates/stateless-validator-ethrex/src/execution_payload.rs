//! Ethrex -> ExecutionPayload conversion.

use alloc::vec::Vec;

use ethrex_common::types::{
    Block as EthrexBlock, ChainConfig as EthrexChainConfig, Transaction as EthrexTransaction,
    Withdrawal as EthrexWithdrawal,
};
use ethrex_rlp::encode::RLPEncode;
use lighthouse_types::{
    Address as LighthouseAddress, EthSpec, ExecutionBlockHash, ExecutionPayload, FixedVector,
    ForkName, Hash256, MainnetEthSpec, Transactions, Uint256, VariableList, Withdrawal,
    Withdrawals,
};
use stateless_validator_common::execution_payload::ExecutionPayloadFields;

/// Determines the fork name based on ethrex chain config and block timestamp.
fn determine_fork_name(chain_config: &EthrexChainConfig, timestamp: u64) -> ForkName {
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

/// Converts ethrex H256 to lighthouse Hash256.
fn to_hash256(hash: ethrex_common::H256) -> Hash256 {
    Hash256::from_slice(hash.as_bytes())
}

/// Converts ethrex H256 to lighthouse ExecutionBlockHash.
fn to_execution_block_hash(hash: ethrex_common::H256) -> ExecutionBlockHash {
    ExecutionBlockHash::from_root(to_hash256(hash))
}

/// Converts ethrex Address to lighthouse Address.
fn to_address(addr: ethrex_common::Address) -> LighthouseAddress {
    LighthouseAddress::from_slice(addr.as_bytes())
}

/// Converts ethrex Bloom to lighthouse FixedVector for logs bloom.
fn to_logs_bloom(
    bloom: &ethrex_common::Bloom,
) -> FixedVector<u8, <MainnetEthSpec as EthSpec>::BytesPerLogsBloom> {
    FixedVector::from(bloom.as_bytes().to_vec())
}

/// Converts ethrex Bytes to lighthouse VariableList for extra data.
fn to_extra_data(
    data: &ethrex_common::Bytes,
) -> VariableList<u8, <MainnetEthSpec as EthSpec>::MaxExtraDataBytes> {
    VariableList::from(data.to_vec())
}

/// Converts ethrex transactions to lighthouse Transactions format (RLP encoded).
fn convert_transactions(txs: &[EthrexTransaction]) -> Transactions<MainnetEthSpec> {
    let encoded: Vec<_> = txs
        .iter()
        .map(|tx| {
            let mut buf = Vec::new();
            tx.encode(&mut buf);
            VariableList::from(buf)
        })
        .collect();
    VariableList::from(encoded)
}

/// Converts ethrex withdrawals to lighthouse Withdrawals.
fn convert_withdrawals(withdrawals: &[EthrexWithdrawal]) -> Withdrawals<MainnetEthSpec> {
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

/// Creates [`ExecutionPayloadFields`] from an ethrex block and chain config.
fn execution_payload_fields_from_ethrex(
    block: &EthrexBlock,
    chain_config: &EthrexChainConfig,
) -> ExecutionPayloadFields {
    let header = &block.header;
    let body = &block.body;
    let fork = determine_fork_name(chain_config, header.timestamp);

    let withdrawals = body
        .withdrawals
        .as_ref()
        .map(|w| convert_withdrawals(w.as_slice()));

    ExecutionPayloadFields {
        fork,
        parent_hash: to_execution_block_hash(header.parent_hash),
        fee_recipient: to_address(header.coinbase),
        state_root: to_hash256(header.state_root),
        receipts_root: to_hash256(header.receipts_root),
        logs_bloom: to_logs_bloom(&header.logs_bloom),
        prev_randao: to_hash256(header.prev_randao),
        block_number: header.number,
        gas_limit: header.gas_limit,
        gas_used: header.gas_used,
        timestamp: header.timestamp,
        extra_data: to_extra_data(&header.extra_data),
        base_fee_per_gas: Uint256::from(header.base_fee_per_gas.unwrap_or_default()),
        block_hash: to_execution_block_hash(header.compute_block_hash()),
        transactions: convert_transactions(&body.transactions),
        withdrawals,
        blob_gas_used: header.blob_gas_used,
        excess_blob_gas: header.excess_blob_gas,
    }
}

/// Converts an ethrex block to a lighthouse [`ExecutionPayload`].
pub fn to_execution_payload_ethrex(
    block: &EthrexBlock,
    chain_config: &EthrexChainConfig,
) -> ExecutionPayload<MainnetEthSpec> {
    execution_payload_fields_from_ethrex(block, chain_config).into_payload()
}
