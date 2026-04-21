# z6m_core — platform-agnostic EVM execution library.
#
# This CMakeLists.txt defines the z6m_core static library.
# It is NOT a standalone CMake project — it is always consumed as a
# subdirectory by a zkVM-specific CMakeLists.txt (e.g. zkvm/sp1/CMakeLists.txt)
# which has already fetched zilkworm and defined the required targets:
#   - silkworm_dev
#   - silkworm_core
#
# Usage in a zkVM CMakeLists.txt:
#   # After zilkworm targets exist:
#   add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../../core" "${CMAKE_BINARY_DIR}/z6m_core")
#   target_link_libraries(my_guest PRIVATE z6m_core)

add_library(z6m_core STATIC
    src/executor.cpp
)

# Public include: consumers get <z6m/executor.hpp> with no extra setup.
target_include_directories(z6m_core
    PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include"
)

# zilkworm is a private dependency — it does not appear in z6m_core's
# public interface, so downstream targets (the zkVM binary) are not
# automatically linked against all of zilkworm.
target_link_libraries(z6m_core
    PRIVATE
        silkworm_dev
        silkworm_core
)

target_compile_features(z6m_core PUBLIC cxx_std_23)
