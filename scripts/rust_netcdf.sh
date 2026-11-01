#!/usr/bin/env bash
# Builds a tool that links the system NetCDF library (`--features netcdf`) in a Debian bookworm toolchain with
# libnetcdf-dev, and runs it in the reference image plus netcdf-bin, so libnetcdf.so.19 and glibc are the reference's.
# Paths under work/ are /work/ inside.
#   rust_netcdf.sh --image             build both images of docker/Dockerfile.netcdf (needs chaceon-ref)
#   rust_netcdf.sh --build <bin>       build only
#   rust_netcdf.sh <bin> [args]        build, then run <bin> with args in the reference image
#   rust_netcdf.sh --run-ref <cmd...>  run a command (ncdump, nccopy, ww3_ounf of the reference) in the run image
# NETCDF_TARGET names another build directory under rust/target (a fault run uses one); NETCDF_SKIP_BUILD skips the build;
# NETCDF_TIMEOUT (seconds) stops a run that does not end, with status 124 (a Fortran loop that never ends is an input to name).
set -euo pipefail
cd "$(dirname "$0")/.."
builder=chaceon-rust-netcdf:bookworm
runner=chaceon-ref-netcdf:switch_chaceon_shrd
target=/rust/target/${NETCDF_TARGET:-linux-netcdf}

image() {
  docker build --platform linux/arm64 --target builder -t "$builder" -f docker/Dockerfile.netcdf .
  docker build --platform linux/arm64 --target run -t "$runner" -f docker/Dockerfile.netcdf .
}

build() {
  docker image inspect "$builder" "$runner" > /dev/null 2>&1 || image
  docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust" -v chaceon-cargo:/usr/local/cargo/registry \
    -e CARGO_TARGET_DIR="$target" -w /rust "$builder" \
    cargo build --release -q -p "$1" --features netcdf --bin "$1"
}

case "${1:-}" in
  --image) image; exit 0 ;;
  --build) build "${2:?usage: rust_netcdf.sh --build <bin>}"; exit 0 ;;
  --run-ref) shift; exec docker run --rm --platform linux/arm64 -v "$PWD/work:/work" --entrypoint "$1" "$runner" "${@:2}" ;;
  "" | -*) echo "usage: rust_netcdf.sh --image | --build <bin> | --run-ref <cmd...> | <bin> [args]" >&2; exit 64 ;;
esac
bin="$1"; shift
if [ -z "${NETCDF_SKIP_BUILD:-}" ]; then build "$bin"; fi
if [ -n "${NETCDF_TIMEOUT:-}" ]; then
  exec docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust:ro" -v "$PWD/work:/work" \
    --entrypoint timeout "$runner" "$NETCDF_TIMEOUT" "$target/release/$bin" "$@"
fi
docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust:ro" -v "$PWD/work:/work" \
  --entrypoint "$target/release/$bin" "$runner" "$@"
