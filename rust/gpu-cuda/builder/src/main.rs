// SPDX-License-Identifier: MPL-2.0
//! Writes the kernels' PTX. These settings are physics, not tuning: `AGENTS.md`, "Porting".

use cuda_builder::{CudaBuilder, NvvmArch};

fn main() {
    CudaBuilder::new("kernels")
        // sm_89 is the RTX 4090; PTX is forward compatible, so the H100 (sm_90) runs it too.
        .arch(NvvmArch::Compute89)
        .fma_contraction(false)
        .override_libm(false)
        .ftz(false)
        .fast_sqrt(false)
        .fast_div(false)
        .copy_to("out/chaceon.ptx")
        .build()
        .expect("PTX build");
}
