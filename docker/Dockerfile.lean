# Lean 4 and mathlib for the proofs in lean/. Lean is never installed on the host.
# The toolchain comes from lean/lean-toolchain, so the pin lives in one place.
FROM ubuntu:24.04
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl git xz-utils \
    && rm -rf /var/lib/apt/lists/*
ENV ELAN_HOME=/opt/elan PATH=/opt/elan/bin:$PATH MATHLIB_CACHE_DIR=/cache/mathlib
RUN curl -sSf https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh | sh -s -- -y --default-toolchain none
COPY lean-toolchain /tmp/lean-toolchain
RUN elan toolchain install "$(cat /tmp/lean-toolchain)" && elan default "$(cat /tmp/lean-toolchain)"
WORKDIR /proj/lean
