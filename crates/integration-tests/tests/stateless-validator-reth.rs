//! Execution tests for `stateless-validator-reth` guest program

use ere_dockerized::zkVMKind;
use integration_tests::{TestCase, get_fixtures};
use stateless_validator_common::guest::execution_payload_to_header_hash;
use stateless_validator_reth::{
    guest::{StatelessValidatorOutput, StatelessValidatorRethGuest, StatelessValidatorRethInput},
    host::to_execution_payload_reth,
};

fn test_execution(zkvm_kind: zkVMKind) {
    let fixtures = get_fixtures();
    let inputs = fixtures.map(|fixture| {
        let input = StatelessValidatorRethInput::new(&fixture.stateless_input).unwrap();

        let execution_payload = to_execution_payload_reth(&fixture.stateless_input);
        let execution_payload_header_hash = execution_payload_to_header_hash(&execution_payload);
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
