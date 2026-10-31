# Every recipe is a thin wrapper over a script or cargo. There is no CI: these are the gates.
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

# Lines per batch and routines per status
port-status:
    bash scripts/port_status.sh

# Double-precision declarations and literals per WW3 file
f64-census:
    bash scripts/f64_census.sh
