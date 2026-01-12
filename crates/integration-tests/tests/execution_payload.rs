//! Test for StatelessInput <-> ExecutionPayload conversion
//!
//! The prover input data is StatelessInput constructed from debug_executionWitness.
/// The guest program input is ExecutionData.
///
/// The following tests check proper conversion between these types.
use std::collections::HashMap;

use alloy_primitives::{B256, b256};
use integration_tests::get_fixtures;
use stateless_validator_reth::{
    execution_payload::{execution_data_to_block, create_new_execution_payload_request},
    host::to_execution_data,
};

/// Verify that StatelessInput is converted to ExecutionPayload correctly against precomputed roots.
/// This verifies that StatelessInput -> ExecutionData -> ExecutionPaylaod is correct.
#[test]
fn test_stateless_input_to_execution_payload() {
    let expected_roots = expected_execution_payload_tree_roots();
    for fixture in get_fixtures() {
        let block_hash = fixture.stateless_input.block.hash_slow();
        let expected_root = *expected_roots.get(&block_hash).unwrap();

        let execution_data = to_execution_data(&fixture.stateless_input);
        let execution_payload_tree_root = create_new_execution_payload_request(&execution_data);

        assert_eq!(
            execution_payload_tree_root, expected_root,
            "ExecutionPayload tree root mismatch for block hash: {block_hash}"
        );
    }
}

// The guest program input is ExecutionData, but the prover input is StatelessInput.
// This test verifies that the guest program reconstructs the same block as the original StatelessInput.
#[test]
fn test_block_rountrip() {
    for fixture in get_fixtures() {
        // Simulate the preparation the prover does to send input to the guest.
        let execution_data = to_execution_data(&fixture.stateless_input);

        // In the guest, reconstruct the block from ExecutionData.
        let guest_block = execution_data_to_block(execution_data).unwrap();

        // Assert that the reconstructed block matches the original block in StatelessInput.
        let guest_block_hash = guest_block.hash_slow();
        let stateless_input_block_hash = fixture.stateless_input.block.hash_slow();
        assert_eq!(
            stateless_input_block_hash, guest_block_hash,
            "Block hash mismatch for fixture: {}",
            fixture.name
        );
    }
}

fn expected_execution_payload_tree_roots() -> HashMap<B256, B256> {
    HashMap::from([
        (
            b256!("e6e4c256069674f7939f82fc808d0cd104210533c83add12d2c33d274fc3c027"),
            b256!("4991dfd5accbdc079105d3ed7e7a09c597c9d1f084ea27c3af68e34cc1f1c321"),
        ),
        (
            b256!("74356579507633dcd34faa38c64f8ec46bc23ab5c13bbb1f2ce46786147baf54"),
            b256!("cedad8811945f9b7334203e924bb27b24a3b413a6a90997986100d9d56595d25"),
        ),
        (
            b256!("f72d095aaf5db3e99dbb76ec7f1dee9e6a3fe4cda536c073c7403ff160be356c"),
            b256!("8fcbcc62d160dd689f3cd3d2f9c5734e313413e69d181cf5d51cc8de31920643"),
        ),
        (
            b256!("eca0cdd3433d05468326534f1fd7b64a23b7d01c3cec0791f4c5e16e0caa4228"),
            b256!("97407320770bf4cee394517864b687788805fbfa89b3eb51353c582f4c1a0f75"),
        ),
        (
            b256!("bdee559b347d195bd65a82cac27533e2b9f94a5ba9dfb662e05033d12fb0ca4d"),
            b256!("0d38e935b7cb4d6f61cc79795f62f66aa50a03d1ec8faa058e47ad4df8c1877c"),
        ),
        (
            b256!("8466849d8c0855c92e9732b70f58e0d228a03d7741f7f0344ad9457eda4dab99"),
            b256!("4bdd9250e875a8e9135cb923c45e3c411065a307f72d23fe34d91d0351334eeb"),
        ),
        (
            b256!("b19d0861de72dbc6f40d6a118b05a975c3b7a525ad19db37b2bd975d60f0648f"),
            b256!("0868cba4f5f931770c7805da4ebe1cbf2c57e530b3f009c624db019838a5bb29"),
        ),
        (
            b256!("7c6cf5941884a4c9a3183bf4d8c0025e771838929ea3651353f9a09ddb0f56de"),
            b256!("5878a54885780e4c016d20aaa56c72fbd053e58b4a0840264222d421d2f5df67"),
        ),
        (
            b256!("2c041b9467dcf0899f85681d164d7edb08b992a552e796761c50d88dbffb6598"),
            b256!("69ce2500b328289f119ffd1890a638c36c18de41976e01e6feb63c6f42081e4f"),
        ),
        (
            b256!("c8e14160123e5e2f8037857b7fdc414bc1687b7c9173218c7ba25320e9448f24"),
            b256!("a4b307762bcb5d447c72f41d1c654d383d5fe87b60b36ebca70e2b384fd4a0df"),
        ),
        (
            b256!("a6ee6b71a5c245a00e2724f3e92cf1b25e12fcc8844a343c241e00020d48500e"),
            b256!("fc9e8d26c359346d03ea61bdcc8c611714ce571291183c03144fdc4e6fe4dfbc"),
        ),
        (
            b256!("cdaf26ec02a13a84ca0a3fc0047584290e57eb972dff3d19ebf2978733f1735f"),
            b256!("b8d7f9e265a3c21dad4e28aab5098e5bea2077e9b260088ced3383235cc04eda"),
        ),
        (
            b256!("c8ac491bec27d1fbf6fa9e894b4f1ba593491e84bb593b9a81dfb89f29027149"),
            b256!("56a3d4d19e43c8bb3c7a47a00c53cba34eab72f50a2aca603d3a93108975a63b"),
        ),
        (
            b256!("e4bd1c4dc22a58a0a9a8e789e2c54b4ace2d1ebc16a605c3976723b52fc011f1"),
            b256!("6ddcf3c3252c62e0f23a756932d81c76c732470c4b041dc5b72364d0137ce56b"),
        ),
        (
            b256!("ba11cc5f2a0d42cc2d1c6ecee10b0c2c3c17dc685b17584be3474d6cafb14140"),
            b256!("5d06d79d8356fa891c9ba32e85bf8f9d6da4e3316a3ed82b37cf784c2624f257"),
        ),
        (
            b256!("444460fa6bf40df3a2b419d55450fb68424c3b5dff248581afb87741be7f92b9"),
            b256!("5e07b4129616627d9ceadd84f406bc42528572819a1de0e3abd0682dc4e1b47b"),
        ),
        (
            b256!("cec65cbf796165f17dc68b583aff9bb8e2f5ccd0fb41c03ac53d57b4740b6534"),
            b256!("55662888e654c9c35836fa57c83108972b81a39f5f2ae5f9197fdbb342d5f039"),
        ),
    ])
}
