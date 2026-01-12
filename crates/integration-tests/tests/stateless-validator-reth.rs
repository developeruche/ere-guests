//! Execution tests for `stateless-validator-reth` guest program

use ere_dockerized::zkVMKind;
use guest::Guest;
use integration_tests::{NoopPlatform, TestCase, get_fixtures};
use stateless_validator_reth::guest::{StatelessValidatorRethGuest, StatelessValidatorRethInput};

fn test_execution(zkvm_kind: zkVMKind) {
    let fixtures = get_fixtures();
    let inputs = fixtures.into_iter().map(|fixture| {
        let input = StatelessValidatorRethInput::new(&fixture.stateless_input).unwrap();
        let output = StatelessValidatorRethGuest::compute::<NoopPlatform>(input.clone());
        assert_eq!(output.successful_block_validation, fixture.success);

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
