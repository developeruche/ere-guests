/* z6m core executor — EVM execution wrapper.
 *
 * Implements the public API declared in <z6m/executor.hpp>.
 *
 * This translation unit is the only place in the codebase that depends
 * on zilkworm types.  All zilkworm headers are confined here — the public
 * interface in executor.hpp exposes only <cstdint> and <cstddef>.
 */

#include <z6m/executor.hpp>

#include <zilk_core/dev/state_transition.hpp>

#include <cstdint>
#include <cstddef>
#include <string_view>

namespace z6m {

ExecutionResult execute_rlp(const uint8_t* data, size_t len)
{
    silkworm::ByteView view{data, len};
    auto st = silkworm::cmd::state_transition::StateTransition(view);
    return ExecutionResult{st.run_rlp()};
}

ExecutionResult execute_test_json(const uint8_t* data, size_t len)
{
    std::string_view json{reinterpret_cast<const char*>(data), len};
    auto st = silkworm::cmd::state_transition::StateTransition(
        json, /*trace=*/false, /*json_output=*/true);
    return ExecutionResult{st.run()};
}

} // namespace z6m
