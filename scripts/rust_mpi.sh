#!/usr/bin/env bash
# Builds ww3_shel with the `mpi` feature in a Debian bookworm toolchain and runs it under mpirun inside the
# reference image, so the libc is the reference's. Paths under work/ are /work/ inside.
#   rust_mpi.sh --image                       build both images of docker/Dockerfile.mpi (needs chaceon-ref)
#   rust_mpi.sh --build                       build only
#   rust_mpi.sh <ranks> <case dir> [args]     build, then mpirun -n <ranks> ww3_shel <case dir> [args]
set -euo pipefail
cd "$(dirname "$0")/.."
builder=chaceon-rust-mpi:bookworm
runner=chaceon-ref-mpi:switch_chaceon_shrd
target=/rust/target/linux-mpi

image() {
  docker build --platform linux/arm64 --target builder -t "$builder" -f docker/Dockerfile.mpi .
  docker build --platform linux/arm64 --target run -t "$runner" -f docker/Dockerfile.mpi .
}

build() {
  docker image inspect "$builder" "$runner" > /dev/null 2>&1 || image
  docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust" -v chaceon-cargo:/usr/local/cargo/registry \
    -e CARGO_TARGET_DIR="$target" -w /rust "$builder" \
    cargo build --release -q --features mpi --bin ww3_shel
}

case "${1:-}" in
  --image) image; exit 0 ;;
  --build) build; exit 0 ;;
  "" | -*) echo "usage: rust_mpi.sh --image | --build | <ranks> <case dir> [--threads N]" >&2; exit 64 ;;
esac
ranks="$1"; shift
if [ -z "${MPI_SKIP_BUILD:-}" ]; then build; fi
# OpenMPI as root in a container: allow it, allow more ranks than cores, and keep shared memory off CMA.
docker run --rm --platform linux/arm64 -v "$PWD/rust:/rust:ro" -v "$PWD/work:/work" \
  -e ST4_NO_TABLES -e LIBM_F64_ONLY -e OMPI_MCA_btl_vader_single_copy_mechanism=none \
  --entrypoint mpirun "$runner" --allow-run-as-root --oversubscribe -n "$ranks" "$target/release/ww3_shel" "$@"
