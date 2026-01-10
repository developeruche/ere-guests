//! Execution tests for `stateless-validator-reth` guest program

use std::fs;

use ere_dockerized::zkVMKind;
use integration_tests::{
    TestCase, fixtures_dir, stateless_validator::StatelessValidatorFixture, untar_fixtures,
};
use stateless_validator_common::{
    execution_payload_experiment::get_root, guest::execution_payload_to_header_hash,
};
use stateless_validator_reth::{
    execution_payload::to_execution_data,
    guest::{StatelessValidatorOutput, StatelessValidatorRethGuest, StatelessValidatorRethInput},
    host::to_execution_payload_reth,
};

fn test_execution(zkvm_kind: zkVMKind) {
    untar_fixtures().unwrap();
    let inputs = fs::read_dir(fixtures_dir().join("block"))
        .unwrap()
        .map(|file| {
            let bytes = fs::read(file.unwrap().path()).unwrap();
            let fixture: StatelessValidatorFixture = serde_json::from_slice(&bytes).unwrap();
            let input = StatelessValidatorRethInput::new(&fixture.stateless_input).unwrap();

            let execution_payload = to_execution_payload_reth(&fixture.stateless_input);
            let execution_payload_header_hash =
                execution_payload_to_header_hash(&execution_payload);

            let execution_data = to_execution_data(&fixture.stateless_input);
            let hash2 = get_root(execution_data);
            println!(
                "Computed execution payload header hash: {:x?}",
                execution_payload_header_hash
            );
            println!("Computed execution payload hash2: {:x?}", hash2);
            assert_eq!(execution_payload_header_hash.as_slice(), hash2.as_slice());

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
            TestCase::new::<StatelessValidatorRethGuest>(fixture.name, input, output)
                .output_sha256()
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
