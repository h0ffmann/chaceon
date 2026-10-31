# ADR-0002: No CI and no Rust tests

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-30 |
| **Deciders** | Hoffmann |

## Context

The owner, 2026-10-30, verbatim: "with same structure, but WITHOUT any CIs" and "Dont write rust
tests."

## Decision

No `.github/`, workflow or hosted runner. No `#[test]`, `#[cfg(test)]`, `tests/`, `benches/`,
doctest or `dev-dependencies`. `scripts/no_rust_tests.sh`, run by `just check`, refuses them.
A diagnostic binary that writes values for comparison with the reference is a tool, not a test,
and it lives under `rust/tools/` or stays out of the repository.

## Consequences

The evidence is WW3's regtests against the reference, run by hand. Nothing stops a commit that
skipped its gates, so the `Tested:` trailer is the only record, and it must be true.
