#pragma once

#include <cstddef>
#include <cstdint>

/// z6m — platform-agnostic EVM execution core.
///
/// This header is the only interface between the zkVM-specific entry points
/// (zkvm/sp1/, zkvm/risc0/, …) and the EVM execution engine (zilkworm).
///
/// Design rules:
///   - No zilkworm types in this header.
///   - No zkVM-specific types in this header.
///   - Depends only on <cstdint> and <cstddef>.
///
/// To add support for a new zkVM:
///   1. Create zkvm/<name>/ with its own CMakeLists.txt, toolchain, linker script.
///   2. #include <z6m/executor.hpp> in the new entrypoint.
///   3. Call z6m::execute_rlp() or z6m::execute_test_json() with the raw payload.
///   4. Publish the result via the new zkVM's output mechanism.

namespace z6m {

/// Result of a single block execution.
struct ExecutionResult
{
    uint64_t gas_used; ///< Total gas consumed by all transactions in the block.
};

/// Execute an Ethereum block from its RLP-encoded block+state blob.
///
/// @param data  Pointer to the encoded block data.
/// @param len   Length in bytes.
/// @returns     ExecutionResult with gas_used set.
ExecutionResult execute_rlp(const uint8_t* data, size_t len);

/// Execute an EIP state-test fixture from its JSON string.
///
/// @param data  Pointer to the JSON bytes (not NUL-terminated).
/// @param len   Length in bytes.
/// @returns     ExecutionResult with gas_used set.
ExecutionResult execute_test_json(const uint8_t* data, size_t len);

} // namespace z6m
