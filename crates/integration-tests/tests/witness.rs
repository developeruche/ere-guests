//! Tests for stateless validator witness transformations using different sparse MPT implementations.

use std::fs;

use guest::{Guest, Platform};
use integration_tests::{
    fixtures_dir, stateless_validator::StatelessValidatorFixture, untar_fixtures,
};
use sparsestate::SparseState;
// use openvm_mpt::statelesstrie::OpenVMStatelessSparseTrie;
use stateless_validator_reth::guest::{
    StatelessValidatorRethGuestWithTrie, StatelessValidatorRethInput,
};

#[tokio::test(flavor = "multi_thread")]
async fn sparse_mpts() {
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
        // Reth with Risc0 sparse MPT.
        StatelessValidatorRethGuestWithTrie::<SparseState>::compute::<NoopPlatform>(input.clone());

        // // Reth with OpenVM sparse MPT.
        // {
        //     let input = transform_witness(input);

        //     RethStatelessValidatorGuest::<OpenVMStatelessSparseTrie>::compute::<NoopPlatform>(
        //         input,
        //     );
        // }
    }
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
