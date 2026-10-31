// SPDX-License-Identifier: MPL-2.0
//! Compute entry points. One invocation is one sea point: its spectrum, group velocities and scratch
//! are the point's ranges of the bound buffers.
#![no_std]

use spirv_std::glam::UVec3;
use spirv_std::spirv;
use ww3_kernels::real::Real;
use ww3_kernels::snl1::{Snl1Params, w3snl1, work_len};

#[spirv(compute(threads(64)))]
#[allow(clippy::too_many_arguments)]
pub fn snl1(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] p: &Snl1Params,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] sig: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] af11: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] pm: &[i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] ic: &[i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] a: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] cg: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] kdmean: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] s: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] d: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 10)] w: &mut [Real],
) {
    let point = id.x as usize;
    // A dispatch rounds up to a whole workgroup.
    if point >= kdmean.len() {
        return;
    }
    let spec_off = point * p.nspec as usize;
    let cg_off = point * p.nk as usize;
    let w_off = point * work_len(p);
    w3snl1(p, sig, af11, pm, ic, a, cg, kdmean[point], s, d, w, spec_off, cg_off, w_off);
}
