# guest_program — Multi-zkVM EVM Guest

A modular bare-metal C++ guest program for zero-knowledge EVM execution.

The core EVM logic lives in **one place** (`core/`). Each supported zkVM has
its own self-contained directory under `zkvm/` that handles only the
zkVM-specific I/O and runtime.  Adding a new zkVM requires zero changes to
`core/`.

---

## Architecture

```
guest_program/
│
├── core/                          Platform-agnostic EVM execution library
│   ├── CMakeLists.txt
│   ├── include/z6m/
│   │   └── executor.hpp           Public API (no zilkworm types)
│   └── src/
│       └── executor.cpp           Wraps zilkworm StateTransition
│
└── zkvm/
    └── sp1/                       SP1 HyperCube target
        ├── CMakeLists.txt         Self-contained: fetches zilkworm, builds ELF
        ├── cmake/
        │   └── riscv64im-sp1.cmake  RISC-V toolchain (auto-detects xPack)
        ├── linker/
        │   └── elf64lriscv.xn       SP1 memory-layout linker script
        └── src/
            ├── main.cpp             Hint-stream I/O → z6m::execute() → PV write
            ├── sp1_entrypoint.S     _start: sets GP/SP, calls __start()
            ├── sp1_runtime.cpp      __start, heap, SP1 ecall implementations
            ├── atomic_stubs.c       Bare-metal atomic builtins
            ├── memcpy.cpp           rv64im-optimised memcpy (clang musl)
            ├── memmove.cpp          memmove with overlap detection
            └── include/
                └── sp1_syscalls.hpp SP1 ecall declarations + inline wrappers
```

### Layer separation

| Layer | What it knows | What it doesn't know |
|---|---|---|
| `core/` | zilkworm, EVM execution | SP1, ecalls, hint streams |
| `zkvm/sp1/` | SP1 ecalls, hint streams, linker | zilkworm internals |
| `zkvm/*/main.cpp` | zkVM I/O | EVM execution details |

---

## Prerequisites

### RISC-V Toolchain (one-time, global install)

The SP1 build requires **xPack riscv-none-elf-gcc**, which bundles a newlib
sysroot.  No `package.json` or `.xpacks` folder is added to the project.

```bash
npm install --location=global xpm@latest
xpm install @xpack-dev-tools/riscv-none-elf-gcc@latest --global
```

CMake auto-detects the toolchain from `~/Library/xPacks/` (macOS) or
`~/.local/xPacks/` (Linux).  No `PATH` export required.

### Other Requirements

- CMake ≥ 3.28
- Git (FetchContent clones zilkworm on first build)
- Internet access on first build

---

## Building

### From the repo root (recommended)

```bash
make guest_sp1
```

### Manually

```bash
cmake -S guest_program/zkvm/sp1 -B guest_program/build/sp1 \
    -DCMAKE_TOOLCHAIN_FILE=$(pwd)/guest_program/zkvm/sp1/cmake/riscv64im-sp1.cmake \
    -DCMAKE_BUILD_TYPE=Release

cmake --build guest_program/build/sp1 -j$(nproc 2>/dev/null || sysctl -n hw.logicalcpu)
```

### Outputs

```
guest_program/build/sp1/
├── z6m_guest.elf    ← RISC-V ELF loaded by the SP1 prover
├── z6m_guest.bin    ← flat binary (code + data)
└── z6m_guest.text   ← code section only
```

---

## Adding a new zkVM

1. Create `zkvm/<name>/` with:
   - `CMakeLists.txt` (fetch zilkworm, add `../../core`, build your ELF)
   - `cmake/<name>-toolchain.cmake` (your ISA's toolchain file)
   - `linker/<name>.xn` (memory layout for your zkVM)
   - `src/main.cpp` — read input via your zkVM's I/O, call `z6m::execute_*()`, write output
   - `src/<name>_runtime.cpp` — zkVM-specific startup and syscalls

2. Add a Makefile target in the repo root:
   ```makefile
   guest_<name>:
       cmake -S guest_program/zkvm/<name> -B guest_program/build/<name> \
           -DCMAKE_TOOLCHAIN_FILE=$(CURDIR)/guest_program/zkvm/<name>/cmake/<name>-toolchain.cmake \
           -DCMAKE_BUILD_TYPE=Release
       cmake --build guest_program/build/<name> -j$(NPROC)
   ```

3. **Zero changes to `core/`.**

---

## How it works

```
_start (sp1_entrypoint.S)
  └── __start (sp1_runtime.cpp)         C++ runtime init, global ctors
        └── main (zkvm/sp1/src/main.cpp) SP1 I/O: read hint stream
              └── z6m::execute_rlp()     core/src/executor.cpp
                    └── StateTransition  zilkworm EVM engine
              └── syscall_write(PV_FD)   write gas_used to SP1 PV channel
        └── syscall_halt(0)             SHA-256 hash PV, commit, halt
```

---

## Dependency on zilkworm

Each zkVM's `CMakeLists.txt` fetches zilkworm from
`https://github.com/erigontech/zilkworm` at configure time via CMake's
`FetchContent`.  Only two subdirectories are imported:

- `third_party/` — evmone, evmc, blst, intx, nlohmann_json
- `zilk_core/` — silkworm_core (EVM types, RLP, trie) + silkworm_dev (StateTransition)
