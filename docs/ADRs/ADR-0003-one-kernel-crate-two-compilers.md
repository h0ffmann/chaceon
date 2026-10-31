# ADR-0003: One kernel crate, Rust-CUDA first, rust-gpu second

| | |
|---|---|
| **Status** | Proposed |
| **Date** | 2026-10-30 |
| **Deciders** | Hoffmann |

## Context

The owner named both compilers. They pin different nightlies (`2026-04-02`, `2026-08-15`) (v), and
both call themselves early-stage (v, READMEs, 2026-10-30). Rust-CUDA reaches the H100 with `f64` and
can disable contraction. rust-gpu reaches any Vulkan device, but cannot promise the same bits
([float document](../FLOAT_REPRESENTATION_202610.md#device-float-behaviour)).

## Decision

One `no_std` crate, `ww3-kernels`, holds the physics. `rust/gpu-cuda/` and `rust/gpu-spirv/` are
separate workspaces, each on its own nightly, holding entry points only. The host loads `.ptx` or
`.spv` files, so it stays on stable Rust. Rust-CUDA is the production path on the H100, and each
batch lands there first. rust-gpu follows.

## Consequences

Kernels stay in the Rust both compilers accept. If Rust-CUDA fails on a real kernel, NVIDIA's
cuda-oxide (NVlabs, Apache-2.0, early-stage, v 2026-10-30) is the fallback the owner may choose: it
would need a third entry crate, and no change to the physics.
