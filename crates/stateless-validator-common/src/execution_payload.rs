//! ExecutionPayload construction utilities.

use lighthouse_types::{
    Address as LighthouseAddress, EthSpec, ExecutionBlockHash, ExecutionPayload,
    ExecutionPayloadBellatrix, ExecutionPayloadCapella, ExecutionPayloadDeneb,
    ExecutionPayloadElectra, FixedVector, ForkName, Hash256, MainnetEthSpec, Transactions, Uint256,
    VariableList, Withdrawals,
};

type MainnetExecutionPayload = ExecutionPayload<MainnetEthSpec>;

/// Intermediate representation with all fields already converted to lighthouse types.
///
/// This struct allows sharing the ExecutionPayload construction logic between
/// different input formats (reth's `StatelessInput` and ethrex's `ProgramInput`).
#[derive(Debug)]
pub struct ExecutionPayloadFields {
    /// The fork variant to construct.
    pub fork: ForkName,
    /// Parent block hash.
    pub parent_hash: ExecutionBlockHash,
    /// Fee recipient (coinbase/beneficiary).
    pub fee_recipient: LighthouseAddress,
    /// State root after block execution.
    pub state_root: Hash256,
    /// Receipts root.
    pub receipts_root: Hash256,
    /// Logs bloom filter.
    pub logs_bloom: FixedVector<u8, <MainnetEthSpec as EthSpec>::BytesPerLogsBloom>,
    /// Previous RANDAO value (mix hash).
    pub prev_randao: Hash256,
    /// Block number.
    pub block_number: u64,
    /// Gas limit.
    pub gas_limit: u64,
    /// Gas used.
    pub gas_used: u64,
    /// Block timestamp.
    pub timestamp: u64,
    /// Extra data.
    pub extra_data: VariableList<u8, <MainnetEthSpec as EthSpec>::MaxExtraDataBytes>,
    /// Base fee per gas.
    pub base_fee_per_gas: Uint256,
    /// Block hash.
    pub block_hash: ExecutionBlockHash,
    /// RLP-encoded transactions.
    pub transactions: Transactions<MainnetEthSpec>,
    /// Withdrawals (Capella+).
    pub withdrawals: Option<Withdrawals<MainnetEthSpec>>,
    /// Blob gas used (Deneb+).
    pub blob_gas_used: Option<u64>,
    /// Excess blob gas (Deneb+).
    pub excess_blob_gas: Option<u64>,
}

impl ExecutionPayloadFields {
    /// Converts the intermediate fields into an [`ExecutionPayload`].
    ///
    /// The fork variant determines which payload type is constructed.
    pub fn into_payload(self) -> MainnetExecutionPayload {
        match self.fork {
            ForkName::Bellatrix => ExecutionPayload::Bellatrix(ExecutionPayloadBellatrix {
                parent_hash: self.parent_hash,
                fee_recipient: self.fee_recipient,
                state_root: self.state_root,
                receipts_root: self.receipts_root,
                logs_bloom: self.logs_bloom,
                prev_randao: self.prev_randao,
                block_number: self.block_number,
                gas_limit: self.gas_limit,
                gas_used: self.gas_used,
                timestamp: self.timestamp,
                extra_data: self.extra_data,
                base_fee_per_gas: self.base_fee_per_gas,
                block_hash: self.block_hash,
                transactions: self.transactions,
            }),
            ForkName::Capella => ExecutionPayload::Capella(ExecutionPayloadCapella {
                parent_hash: self.parent_hash,
                fee_recipient: self.fee_recipient,
                state_root: self.state_root,
                receipts_root: self.receipts_root,
                logs_bloom: self.logs_bloom,
                prev_randao: self.prev_randao,
                block_number: self.block_number,
                gas_limit: self.gas_limit,
                gas_used: self.gas_used,
                timestamp: self.timestamp,
                extra_data: self.extra_data,
                base_fee_per_gas: self.base_fee_per_gas,
                block_hash: self.block_hash,
                transactions: self.transactions,
                withdrawals: self.withdrawals.unwrap_or_default(),
            }),
            ForkName::Deneb => ExecutionPayload::Deneb(ExecutionPayloadDeneb {
                parent_hash: self.parent_hash,
                fee_recipient: self.fee_recipient,
                state_root: self.state_root,
                receipts_root: self.receipts_root,
                logs_bloom: self.logs_bloom,
                prev_randao: self.prev_randao,
                block_number: self.block_number,
                gas_limit: self.gas_limit,
                gas_used: self.gas_used,
                timestamp: self.timestamp,
                extra_data: self.extra_data,
                base_fee_per_gas: self.base_fee_per_gas,
                block_hash: self.block_hash,
                transactions: self.transactions,
                withdrawals: self.withdrawals.unwrap_or_default(),
                blob_gas_used: self.blob_gas_used.unwrap_or_default(),
                excess_blob_gas: self.excess_blob_gas.unwrap_or_default(),
            }),
            ForkName::Electra | ForkName::Fulu | ForkName::Gloas => {
                ExecutionPayload::Electra(ExecutionPayloadElectra {
                    parent_hash: self.parent_hash,
                    fee_recipient: self.fee_recipient,
                    state_root: self.state_root,
                    receipts_root: self.receipts_root,
                    logs_bloom: self.logs_bloom,
                    prev_randao: self.prev_randao,
                    block_number: self.block_number,
                    gas_limit: self.gas_limit,
                    gas_used: self.gas_used,
                    timestamp: self.timestamp,
                    extra_data: self.extra_data,
                    base_fee_per_gas: self.base_fee_per_gas,
                    block_hash: self.block_hash,
                    transactions: self.transactions,
                    withdrawals: self.withdrawals.unwrap_or_default(),
                    blob_gas_used: self.blob_gas_used.unwrap_or_default(),
                    excess_blob_gas: self.excess_blob_gas.unwrap_or_default(),
                })
            }
            _ => panic!("unsupported fork: {}", self.fork),
        }
    }
}
