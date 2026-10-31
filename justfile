# Every recipe is a thin wrapper over a script or cargo. There is no CI: these are the gates.
# rustup first: Homebrew's cargo is too old for edition 2024.
export PATH := env("HOME") + "/.cargo/bin:" + env("PATH")
default:
    @just --list

# Type-checks the host workspace and refuses Rust tests
check:
    bash scripts/no_rust_tests.sh
    cd rust && cargo check --workspace

# The gates of a change to scripts
lint-sh:
    bash -n scripts/*.sh
    shellcheck scripts/*.sh

# Builds the Fortran reference, metocean/WW3 at the pin
ref-build switch="switch_chaceon_shrd":
    bash scripts/ref_build.sh {{switch}}

# Captures W3SNL1 from the reference running ww3_ts1 at a depth in metres
capture depth="2500" stride="1":
    bash scripts/capture_snl1.sh {{stride}} {{depth}}

# Replays a capture through the host build, every bit
snl1-cmp depth="2500":
    bash scripts/rust_in_ref.sh snl1_cmp /work/capture/snl1_ts1_{{depth}}m.bin

# ww3-libm against the reference's glibc, every f32
libm-bits:
    bash scripts/rust_in_ref.sh libm_bits

# Compiles the kernels to SPIR-V
spirv:
    bash scripts/spirv.sh

# Runs the compiled SPIR-V on a capture, on CPU Vulkan
spirv-replay depth="2500":
    bash scripts/spirv_replay.sh capture/snl1_ts1_{{depth}}m.bin

# Compiles the kernels to PTX and assembles sm_89 and sm_90 binaries
ptx:
    bash scripts/ptx.sh

# Lines per batch and routines per status
port-status:
    bash scripts/port_status.sh

# Double-precision declarations and literals per WW3 file
f64-census:
    bash scripts/f64_census.sh
