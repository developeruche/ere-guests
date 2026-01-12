//! Execution tests for `stateless-validator-ethrex` guest program

use ere_dockerized::zkVMKind;
use integration_tests::{TestCase, get_fixtures};
use stateless_validator_ethrex::guest::{
    StatelessValidatorEthrexGuest, StatelessValidatorEthrexInput, StatelessValidatorOutput,
};
use stateless_validator_reth::{
    execution_payload::create_new_execution_payload_request, host::to_execution_data,
};

fn test_execution(zkvm_kind: zkVMKind) {
    let fixtures = get_fixtures();
    let inputs = fixtures.into_iter().map(|fixture| {
        let input = StatelessValidatorEthrexInput::new(&fixture.stateless_input).unwrap();

        let execution_data = to_execution_data(&fixture.stateless_input);
        let execution_payload_header_hash = create_new_execution_payload_request(&execution_data);
        let beacon_root = fixture
            .stateless_input
            .block
            .header
            .parent_beacon_block_root
            .unwrap_or_default();
        let output = StatelessValidatorOutput::new(
            execution_payload_header_hash,
            beacon_root,
            fixture.success,
        );
        TestCase::new::<StatelessValidatorEthrexGuest>(fixture.name, input, output).output_sha256()
    });
    integration_tests::test_execution("stateless-validator-ethrex", zkvm_kind, inputs);
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
