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

# Records a routine from the reference running ww3_ts1
capture routine="snl1" variant="default" depth="2500" stride="1":
    bash scripts/capture.sh {{routine}} {{variant}} {{depth}} {{stride}}

# Replays a W3SNL1 capture through the host build, every bit
snl1-cmp depth="2500":
    bash scripts/rust_in_ref.sh snl1_cmp /work/capture/snl1_default_{{depth}}m.bin

# Replays a W3SDS4 capture through the host build, every bit
sds4-cmp variant="default" depth="2500":
    bash scripts/rust_in_ref.sh sds4_cmp /work/capture/sds4_{{variant}}_{{depth}}m.bin

# Replays W3SIN4 or W3SPR4 calls and rebuilds ST4's tables, every bit
st4-cmp routine="sin4" variant="default" depth="2500":
    bash scripts/rust_in_ref.sh st4_cmp {{routine}} /work/capture/{{routine}}_{{variant}}_{{depth}}m.bin

# Replays W3SRCE calls of a case, every bit; needs that case's snl1, sds4, sin4 and spr4 captures
srce-cmp tag="default_2500m":
    bash scripts/rust_in_ref.sh srce_cmp {{tag}}

# ww3-libm against the reference's glibc: every f32, and samples for two arguments
libm-bits samples="8000000000":
    bash scripts/rust_in_ref.sh libm_bits {{samples}}

# Compiles the kernels to SPIR-V
spirv:
    bash scripts/spirv.sh

# Runs the compiled SPIR-V on a capture, on CPU Vulkan
spirv-replay kernel="snl1" variant="default" depth="2500":
    bash scripts/spirv_replay.sh {{kernel}} capture/{{kernel}}_{{variant}}_{{depth}}m.bin

# Compiles the kernels to PTX and assembles sm_89 and sm_90 binaries
ptx:
    bash scripts/ptx.sh

# Lines per batch and routines per status
port-status:
    bash scripts/port_status.sh

# Double-precision declarations and literals per WW3 file
f64-census:
    bash scripts/f64_census.sh
