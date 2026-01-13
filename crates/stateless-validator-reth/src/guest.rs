//! [`Guest`] implementation for Reth stateless validator.

use alloc::{format, sync::Arc, vec::Vec};

use alloy_genesis::ChainConfig;
use alloy_rpc_types_engine::ExecutionData;
use ere_io::serde::{IoSerde, bincode::BincodeLegacy};
use reth_chainspec::ChainSpec;
use reth_evm_ethereum::EthEvmConfig;
use reth_payload_validator::cancun::ensure_matching_blob_versioned_hashes;
use reth_stateless::{
    ExecutionWitness, Genesis, UncompressedPublicKey, stateless_validation_with_trie,
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sparsestate::SparseState;

use crate::{
    execution_payload::{create_new_payload_request, execution_data_to_block},
    serde_bincode_compat::ExecutionDataCompat,
};

#[rustfmt::skip]
pub use {
    guest::*,
    stateless_validator_common::guest::StatelessValidatorOutput,
};

/// Input for the stateless validator guest program.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatelessValidatorRethInput {
    /// Execution data from the beacon block.
    #[serde_as(as = "ExecutionDataCompat")]
    pub execution_data: ExecutionData,
    /// Execution witness for the EL block.
    pub witness: ExecutionWitness,
    /// Chain configuration for the stateless validation function
    #[serde_as(as = "alloy_genesis::serde_bincode_compat::ChainConfig<'_>")]
    pub chain_config: ChainConfig,
    /// The recovered signers for the transactions in the block.
    pub public_keys: Vec<UncompressedPublicKey>,
}

/// [`Io`] implementation of Reth stateless validator.
pub type StatelessValidatorRethIo =
    IoSerde<StatelessValidatorRethInput, StatelessValidatorOutput, BincodeLegacy>;

/// [`Guest`] implementation for Reth stateless validator.
#[derive(Debug, Clone)]
pub struct StatelessValidatorRethGuest;

impl Guest for StatelessValidatorRethGuest {
    type Io = StatelessValidatorRethIo;

    fn compute<P: Platform>(input: GuestInput<Self>) -> GuestOutput<Self> {
        let cancun_sidecar = input.execution_data.sidecar.cancun().cloned();
        let (chain_spec, evm_config, block_result) =
            P::cycle_scope("validation_inputs_preparation", || {
                let genesis = Genesis {
                    config: input.chain_config,
                    ..Default::default()
                };
                let chain_spec: Arc<ChainSpec> = Arc::new(genesis.into());
                let evm_config = EthEvmConfig::new(chain_spec.clone());
                let block_result = execution_data_to_block(input.execution_data.clone());
                (chain_spec, evm_config, block_result)
            });
        let block = match block_result {
            Ok(block) => block,
            Err(err) => {
                P::print(&format!("Failed to convert to reth block: {err}\n"));
                return StatelessValidatorOutput::default(); // TODO
            }
        };

        // Validate versioned_hashes with the block transactions.
        if let Err(err) =
            ensure_matching_blob_versioned_hashes(&block.body, cancun_sidecar.as_ref())
        {
            P::print(&format!("Versioned hashes validation failed: {err}\n"));
            return StatelessValidatorOutput::default(); // TODO
        }

        let res = P::cycle_scope("validation", || {
            stateless_validation_with_trie::<SparseState, _, _>(
                block,
                input.public_keys,
                input.witness,
                chain_spec,
                evm_config,
            )
        });

        match res {
            Ok((_, output)) => {
                let Ok(new_payload_request) =
                    create_new_payload_request(&input.execution_data, &output.requests)
                else {
                    P::print("Failed to create new execution payload request\n");
                    return StatelessValidatorOutput::default(); // TODO
                };
                StatelessValidatorOutput::new(new_payload_request, true)
            }
            Err(err) => {
                P::print(&format!("Block validation failed: {err}\n"));
                StatelessValidatorOutput::default() // TODO
            }
        }
    }
}
