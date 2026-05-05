/* SP1 stateless-validator guest entry point.
 *
 * Spec ref: stateless_guest.py§run_stateless_guest
 *
 * Execution flow (invoked from __start() in sp1_runtime.cpp):
 *
 *   1. Read the SSZ-encoded SszStatelessInput from the SP1 hint stream
 *      (a single read_vec_raw() call — no mode-flag prefix).
 *   2. Call z6m::run_stateless_guest() which:
 *        a. SSZ-decodes SszStatelessInput.
 *        b. Computes hash_tree_root(new_payload_request).
 *        c. Attempts stateless EVM execution (currently stubbed — see stateless.cpp).
 *        d. Returns StatelessValidatorOutput{root[32], successful_validation}.
 *   3. Serialise the output as 33 raw bytes: root[0..32] || success[32].
 *   4. SHA-256 the 33 bytes.
 *   5. Write the 32-byte SHA-256 digest to SP1_FD_PUBLIC_VALUES.
 *      The SP1 runtime will SHA-256 that again at halt and commit the
 *      final digest — matching the Rust run_output_sha256 convention.
 *
 * SPEC vs RUST discrepancy (output format):
 *   - Python spec output: SSZ-encoded SszStatelessValidationResult (41 bytes
 *     = root[32] || success[1] || chain_config.chain_id[8]).
 *   - Rust guest output:  raw StatelessValidatorOutput (33 bytes
 *     = root[32] || success[1]); chain_config is NOT included.
 *   We follow the Rust convention (33 bytes) so the C++ and Rust guests
 *   produce identical public-value commitments.  The chain_config omission
 *   is a known divergence from the Python spec; see stateless.py TODO.
 */

#include <z6m/stateless.hpp>        // z6m::run_stateless_guest
#include "include/sp1_syscalls.hpp" // read_vec_raw, syscall_write, SP1_FD_PUBLIC_VALUES

#include <cstdint>
#include <cstring>
#include <evmone_precompiles/sha256.hpp>

extern "C" int main()
{
    /* ── 1. Read SSZ-encoded SszStatelessInput ─────────────────────────── */
    ReadVecResult input_buf = read_vec_raw();

    /* ── 2. Run spec-compliant stateless validation ─────────────────────── */
    const z6m::StatelessValidatorOutput result =
        z6m::run_stateless_guest(input_buf.ptr, input_buf.len);

    /* ── 3. Serialise: root[0..32] || success[32]  (33 bytes) ─────────── */
    uint8_t raw[33];
    std::memcpy(raw, result.new_payload_request_root, 32);
    raw[32] = result.successful_validation ? 1 : 0;

    /* ── 4. SHA-256 the 33-byte output (mirrors Rust run_output_sha256) ── */
    uint8_t digest[32];
    evmone::crypto::sha256(
        reinterpret_cast<std::byte*>(digest),
        reinterpret_cast<const std::byte*>(raw), 33);

    /* ── 5. Write digest to public-values FD; runtime will SHA-256 again ─ */
    syscall_write(SP1_FD_PUBLIC_VALUES, digest, sizeof(digest));

    return 0;
}
