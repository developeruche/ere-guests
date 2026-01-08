//! Tests for stateless validator witness transformations using different sparse MPT implementations.

use std::fs;

use guest::{Guest, Platform};
use integration_tests::{
    fixtures_dir, stateless_validator::StatelessValidatorFixture, untar_fixtures,
};
use openvm_mpt::statelesstrie::OpenVMStatelessSparseTrie;
use reth_stateless::trie::StatelessSparseTrie;
use sparsestate::SparseState;
use stateless_validator_reth::guest::{
    StatelessValidatorRethGuestWithTrie, StatelessValidatorRethInput,
};

#[test]
fn sparse_mpts() {
    println!("Starting transform_witness test...");
    untar_fixtures().unwrap();
    let fixtures: Vec<_> = fs::read_dir(fixtures_dir().join("block"))
        .unwrap()
        .map(|file| {
            let bytes = fs::read(file.unwrap().path()).unwrap();
            let fixture: StatelessValidatorFixture = serde_json::from_slice(&bytes).unwrap();
            fixture
        })
        .collect();

    for fixture in fixtures {
        if !fixture.success {
            // Skip fixtures with non-success expected outcome just to avoid confusion.
            continue;
        }
        println!("Processing fixture: {}", fixture.name);
        let input = StatelessValidatorRethInput::new(&fixture.stateless_input).unwrap();

        // Reth with default sparse MPT from Reth repo.
        StatelessValidatorRethGuestWithTrie::<StatelessSparseTrie>::compute::<NoopPlatform>(
            input.clone(),
        );

        // Reth with Risc0 sparse MPT.
        StatelessValidatorRethGuestWithTrie::<SparseState>::compute::<NoopPlatform>(input.clone());

        // Reth with OpenVM sparse MPT.
        {
            let input = transform_witness(input);

            StatelessValidatorRethGuestWithTrie::<OpenVMStatelessSparseTrie>::compute::<NoopPlatform>(
                input.clone(),
            );
        }
    }
}

/// Transforms the witness in the input from Risc0 MPT format to OpenVM MPT format.
pub fn transform_witness(mut input: StatelessValidatorRethInput) -> StatelessValidatorRethInput {
    let pre_state_root = state_root_from_headers(
        input.stateless_input.block.number - 1,
        &input.stateless_input.witness.headers,
    );
    let tries_bytes = openvm_mpt::from_proof::from_execution_witness(
        pre_state_root,
        &input.stateless_input.witness,
    )
    .unwrap()
    .encode_to_state_bytes();

    // This is just a hacky way to pass the EthereumStateBytes to the guest program
    // without changing the interface. The guest program will simply take the first
    // item of the `witness.state` vector and decode it as EthereumStateBytes.
    let bytes = bincode::serialize(&tries_bytes).unwrap();
    input.stateless_input.witness.state = vec![bytes.into()];
    input
}

fn state_root_from_headers(block_num: u64, headers: &[impl AsRef<[u8]>]) -> alloy_primitives::B256 {
    headers
        .iter()
        .find_map(|h| {
            let header = alloy_rlp::decode_exact::<alloy_consensus::Header>(h).unwrap();
            (header.number == block_num).then_some(header.state_root)
        })
        .unwrap()
}

pub(crate) struct NoopPlatform;

impl Platform for NoopPlatform {
    fn read_whole_input() -> impl std::ops::Deref<Target = [u8]> {
        panic!("NoopPlatform does not implement read_whole_input");
        #[allow(unreachable_code)]
        Vec::<u8>::new()
    }

    fn write_whole_output(output: &[u8]) {
        println!(
            "NoopPlatform: received output with length: {}",
            output.len()
        );
    }

    fn print(message: &str) {
        println!("NoopPlatform: message: {}", message);
    }
}
