// SPDX-License-Identifier: MPL-2.0
//! Compute entry points. One invocation is one sea point: its spectrum, group velocities and scratch
//! are the point's ranges of the bound buffers.
#![no_std]

use spirv_std::glam::UVec3;
use spirv_std::spirv;
use ww3_kernels::real::Real;
use ww3_kernels::sds4::{Sds4Params, Sds4Point, w3sds4};
use ww3_kernels::sin4::{Sin4Out, Sin4Params, Sin4Point, Spr4Out, Spr4Params, Spr4Point, w3sin4, w3spr4};
use ww3_kernels::snl1::{Snl1Params, w3snl1};
use ww3_kernels::{sds4, snl1};

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
    let w_off = point * snl1::work_len(p);
    w3snl1(p, sig, af11, pm, ic, a, cg, kdmean[point], s, d, w, spec_off, cg_off, w_off);
}

#[spirv(compute(threads(64)))]
#[allow(clippy::too_many_arguments)]
pub fn sds4(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] p: &Sds4Params,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] sig: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] dden: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] ec2: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] es2: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] esc: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] ecos: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] esin: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] satindices: &[i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] satweights: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 10)] cumulw: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] iktab: &[i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] dcki: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 13)] qbi: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] a: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 15)] k: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] cg: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 17)] points: &[Sds4Point],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 18)] srhs: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 19)] ddiag: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 20)] brlambda: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 21)] whitecap: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 22)] w: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 23)] iw: &mut [i32],
) {
    let point = id.x as usize;
    if point >= points.len() {
        return;
    }
    let pt = points[point];
    let (lw, li) = sds4::work_len(p);
    w3sds4(
        p, sig, dden, ec2, es2, esc, ecos, esin, satindices, satweights, cumulw, iktab, dcki, qbi, a, k, cg,
        &pt, srhs, ddiag, brlambda, whitecap, w, iw,
        point * p.nspec as usize, point * p.nk as usize, point * 4, point * lw, point * li,
    );
}

#[spirv(compute(threads(64)))]
#[allow(clippy::too_many_arguments)]
pub fn sin4(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] p: &Sin4Params,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] sig: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] sig2: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] dden: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] dden2: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] ecos: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] esin: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] fwtable: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] tauhft: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] tauhft2: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 10)] a: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] cg: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] k: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 13)] points: &[Sin4Point],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] s: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 15)] d: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] llws: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 17)] out: &mut [Sin4Out],
) {
    let point = id.x as usize;
    if point >= points.len() {
        return;
    }
    let pt = points[point];
    w3sin4(
        p, sig, sig2, dden, dden2, ecos, esin, fwtable, tauhft, tauhft2, a, cg, k, &pt, s, d, llws, out,
        point * p.nspec as usize, point * p.nk as usize, point,
    );
}

#[spirv(compute(threads(64)))]
#[allow(clippy::too_many_arguments)]
pub fn spr4(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] p: &Spr4Params,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] sig: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] dden: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] ecos: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] esin: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] taut: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] a: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] cg: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] wn: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] llws: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 10)] points: &[Spr4Point],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] w: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] out: &mut [Spr4Out],
) {
    let point = id.x as usize;
    if point >= points.len() {
        return;
    }
    let pt = points[point];
    let nk = p.nk as usize;
    w3spr4(
        p, sig, dden, ecos, esin, taut, a, cg, wn, llws, &pt, w, out,
        point * p.nspec as usize, point * nk, point * 2 * nk, point,
    );
}
