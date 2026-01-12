//! Execution tests for `stateless-validator-ethrex` guest program

use ere_dockerized::zkVMKind;
use guest::Guest;
use integration_tests::{NoopPlatform, TestCase, get_fixtures};
use stateless_validator_ethrex::guest::{
    StatelessValidatorEthrexGuest, StatelessValidatorEthrexInput,
};

fn test_execution(zkvm_kind: zkVMKind) {
    let fixtures = get_fixtures();
    let inputs = fixtures.into_iter().map(|fixture| {
        let input = StatelessValidatorEthrexInput::new(&fixture.stateless_input).unwrap();
        let output = StatelessValidatorEthrexGuest::compute::<NoopPlatform>(input.clone());
        assert_eq!(output.successful_block_validation, fixture.success);

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
