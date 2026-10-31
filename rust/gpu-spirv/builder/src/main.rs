// SPDX-License-Identifier: MPL-2.0
//! Writes the kernels' SPIR-V. `Float64` and `Int64` come from `ww3-libm`: ADR-0004.
//!
//! rust-gpu's own `spirv-opt` pass runs before `spirv_exact` can decorate, and folds `(a-c1)-c2` into
//! `a-(c1+c2)`, which `atan2f` alone turned into a 1 ULP error on 29% of its inputs. So it is off here,
//! and `scripts/spirv.sh` runs `spirv-opt` after the decoration, which it honours.

use spirv_builder::{Capability, SpirvBuilder, SpirvMetadata};

fn main() {
    let result = SpirvBuilder::new("shader", "spirv-unknown-vulkan1.2")
        .capability(Capability::Float64)
        .capability(Capability::Int64)
        .spirv_metadata(SpirvMetadata::NameVariables)
        .extra_arg("--no-spirv-opt")
        .build()
        .expect("SPIR-V build");
    println!("{:?}", result.module);
}
