//! Execution tests for `stateless-validator-reth` guest program

use ere_dockerized::zkVMKind;
use integration_tests::{TestCase, get_fixtures};
use stateless_validator_common::{
    execution_payload_experiment::execution_payload_tree_root,
    guest::execution_payload_to_header_hash,
};
use stateless_validator_reth::{
    execution_payload::{to_execution_data, to_reth_block},
    guest::{StatelessValidatorOutput, StatelessValidatorRethGuest, StatelessValidatorRethInput},
    host::to_execution_payload_reth,
};

fn test_execution(zkvm_kind: zkVMKind) {
    let fixtures = get_fixtures();
    let inputs = fixtures.map(|fixture| {
        let input = StatelessValidatorRethInput::new(&fixture.stateless_input).unwrap();

        let execution_payload = to_execution_payload_reth(&fixture.stateless_input);
        let execution_payload_header_hash = execution_payload_to_header_hash(&execution_payload);

        let execution_data = to_execution_data(&fixture.stateless_input);
        let hash2 = execution_payload_tree_root(execution_data.clone());
        println!(
            "Computed execution payload header hash: {:x?}",
            execution_payload_header_hash
        );
        println!("Computed execution payload hash2: {:x?}", hash2);
        assert_eq!(execution_payload_header_hash.as_slice(), hash2.as_slice());

        let block2 = to_reth_block(execution_data);
        let block_hash1 = fixture.stateless_input.block.hash_slow();
        let block_hash2 = block2.unwrap().hash_slow();
        println!("Original block hash: {:x?}", block_hash1);
        println!("Reconstructed block hash: {:x?}", block_hash2);
        assert_eq!(block_hash1, block_hash2);

        let beacon_root = fixture
            .stateless_input
            .block
            .parent_beacon_block_root
            .unwrap_or_default();
        let output = StatelessValidatorOutput::new(
            execution_payload_header_hash,
            beacon_root,
            fixture.success,
        );
        TestCase::new::<StatelessValidatorRethGuest>(fixture.name, input, output).output_sha256()
    });
    integration_tests::test_execution("stateless-validator-reth", zkvm_kind, inputs);
}

#[test]
fn test_execution_airbender() {
    test_execution(zkVMKind::Airbender);
}

#[test]
fn test_execution_openvm() {
    test_execution(zkVMKind::OpenVM);
}

#[test]
fn test_execution_pico() {
    test_execution(zkVMKind::Pico);
}

#[test]
fn test_execution_risc0() {
    test_execution(zkVMKind::Risc0);
}

#[test]
fn test_execution_sp1() {
    test_execution(zkVMKind::SP1);
}

#[test]
fn test_execution_zisk() {
    test_execution(zkVMKind::Zisk);
}
