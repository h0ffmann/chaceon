#!/usr/bin/env bash
# ADR-0002: the port has no Rust tests. WW3's regtests against the reference are its only evidence.
set -euo pipefail
cd "$(dirname "$0")/../rust"
found=$(grep -rnE '#\[(cfg\()?test\b|#\[bench\]|\[dev-dependencies\]' --include='*.rs' --include='Cargo.toml' . || true)
dirs=$(find . -type d \( -name tests -o -name benches \) -not -path './target/*' || true)
if [ -n "$found$dirs" ]; then
  printf 'Rust tests are not allowed here (ADR-0002):\n%s\n%s\n' "$found" "$dirs" >&2
  exit 1
fi
echo 'no Rust tests'
