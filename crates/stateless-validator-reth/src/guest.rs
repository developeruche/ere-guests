//! [`Guest`] implementation for Reth stateless validator.

use alloc::{format, sync::Arc, vec::Vec};

use alloy_genesis::ChainConfig;
use ere_io::serde::{IoSerde, bincode::BincodeLegacy};
use reth_chainspec::ChainSpec;
use reth_evm_ethereum::EthEvmConfig;
use reth_stateless::{
    ExecutionWitness, Genesis, UncompressedPublicKey, stateless_validation_with_trie,
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sparsestate::SparseState;
use stateless_validator_common::execution_payload::NewPayloadRequest;

use crate::execution_payload::new_payload_request_to_block;

#[rustfmt::skip]
pub use {
    guest::*,
    stateless_validator_common::guest::StatelessValidatorOutput,
};

/// Input for the stateless validator guest program.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatelessValidatorRethInput {
    /// New payload request data.
    pub new_payload_request: NewPayloadRequest,
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
        let new_payload_request_root = input.new_payload_request.tree_hash_root();

        let (chain_spec, evm_config, block_result) =
            P::cycle_scope("validation_inputs_preparation", || {
                let genesis = Genesis {
                    config: input.chain_config.clone(),
                    ..Default::default()
                };
                let chain_spec: Arc<ChainSpec> = Arc::new(genesis.into());
                let evm_config = EthEvmConfig::new(chain_spec.clone());
                let block_result =
                    new_payload_request_to_block(input.new_payload_request, chain_spec.clone());
                (chain_spec, evm_config, block_result)
            });

        let block = match block_result {
            Ok(block) => block,
            Err(err) => {
                P::print(&format!("Failed to convert to reth block: {err}\n"));
                return StatelessValidatorOutput::default(); // TODO
            }
        };

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
            Ok(_) => StatelessValidatorOutput {
                // TODO: change to use constructor
                new_payload_request_root,
                successful_block_validation: true,
            },
            Err(err) => {
                P::print(&format!("Block validation failed: {err}\n"));
                StatelessValidatorOutput::default() // TODO
            }
        }
    }
}
