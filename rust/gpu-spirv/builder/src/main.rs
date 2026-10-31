// SPDX-License-Identifier: MPL-2.0
//! Writes the kernels' SPIR-V. `Float64` and `Int64` come from `ww3-libm`: ADR-0004.

use spirv_builder::{Capability, SpirvBuilder, SpirvMetadata};

fn main() {
    let result = SpirvBuilder::new("shader", "spirv-unknown-vulkan1.2")
        .capability(Capability::Float64)
        .capability(Capability::Int64)
        .spirv_metadata(SpirvMetadata::NameVariables)
        .build()
        .expect("SPIR-V build");
    println!("{:?}", result.module);
}
