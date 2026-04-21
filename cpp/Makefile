SHELL  = /bin/bash
.SHELLFLAGS = -o pipefail -c

# Portable CPU count: nproc on Linux, sysctl on macOS.
NPROC := $(shell nproc 2>/dev/null || sysctl -n hw.logicalcpu)

# Resolve the directory containing this Makefile so cmake -S works even when
# make is invoked from a different working directory.
ROOT := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))

.PHONY: all guest_sp1 clean

all: guest_sp1

# Outputs: build/sp1/z6m_guest.{elf,bin,text}
guest_sp1:
	cmake -S $(ROOT)/zkvm/sp1 -B $(ROOT)/build/sp1 \
		-DCMAKE_TOOLCHAIN_FILE=$(ROOT)/zkvm/sp1/cmake/riscv64im-sp1.cmake \
		-DCMAKE_BUILD_TYPE=Release
	cmake --build $(ROOT)/build/sp1 -j$(NPROC)

clean:
	rm -rf $(ROOT)/build
