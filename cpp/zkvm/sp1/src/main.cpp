/* SP1 HyperCube — guest entry point.
 *
 * This file is SP1-specific: all I/O uses SP1 hint-stream ecalls.
 * The EVM execution itself is delegated to z6m::execute_*() from core/,
 * which has no knowledge of SP1 or any other zkVM.
 *
 * Execution flow (invoked from __start() in sp1_runtime.cpp):
 *
 *   1. Read a 1-byte mode flag from the SP1 hint stream.
 *        0x00 → production mode (RLP-encoded block + state)
 *        0x01 → test mode       (JSON EIP state-test fixture)
 *   2. Read the payload from the SP1 hint stream.
 *   3. Call z6m::execute_rlp() or z6m::execute_test_json() from core/.
 *   4. Serialise gas_used as 8 little-endian bytes and write to the
 *      SP1 public-values file descriptor.
 *   5. Return to __start(), which SHA-256 hashes the public values,
 *      commits the digest, and calls syscall_halt(0).
 */

#include <z6m/executor.hpp>        // platform-agnostic EVM core
#include "include/sp1_syscalls.hpp" // SP1 ecall wrappers

#include <cstdint>
#include <format>

extern "C" int main()
{
    /* ── 1. Read the mode flag ─────────────────────────────────────────── */
    ReadVecResult mode_buf = read_vec_raw();
    const bool is_test = (mode_buf.len > 0 && mode_buf.ptr[0] != 0);

    /* ── 2. Read the payload ───────────────────────────────────────────── */
    ReadVecResult input_buf = read_vec_raw();

    sys_println("Zilkworm guest initialised");

    /* ── 3. Execute — delegated entirely to the platform-agnostic core ── */
    const z6m::ExecutionResult result = is_test
        ? z6m::execute_test_json(input_buf.ptr, input_buf.len)
        : z6m::execute_rlp(input_buf.ptr, input_buf.len);

    sys_println(std::format("[executor] gas used: {}", result.gas_used));

    /* ── 4. Publish result via SP1 public-values channel ─────────────── */
    uint8_t out[8];
    for (int i = 0; i < 8; ++i)
        out[i] = static_cast<uint8_t>(result.gas_used >> (i * 8));

    syscall_write(SP1_FD_PUBLIC_VALUES, out, sizeof(out));

    return 0;
}
