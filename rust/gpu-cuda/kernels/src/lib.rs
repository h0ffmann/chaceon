// SPDX-License-Identifier: MPL-2.0
//! PTX entry points. One thread is one sea point: its spectrum, group velocities and scratch are the
//! point's ranges of the buffers.

use cuda_std::prelude::*;
use ww3_kernels::real::Real;
use ww3_kernels::sds4::{Sds4Params, Sds4Point, w3sds4};
use ww3_kernels::snl1::{Snl1Params, w3snl1};
use ww3_kernels::{sds4, snl1};

/// # Safety
/// `s`, `d` and `w` hold `points` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn snl1(
    p: &Snl1Params,
    sig: &[Real],
    af11: &[Real],
    pm: &[i32],
    ic: &[i32],
    a: &[Real],
    cg: &[Real],
    kdmean: &[Real],
    s: *mut Real,
    d: *mut Real,
    w: *mut Real,
) {
    let point = thread::index_1d() as usize;
    if point >= kdmean.len() {
        return;
    }
    let nspec = p.nspec as usize;
    let len = snl1::work_len(p);
    // The kernel addresses the whole buffer through offsets, as on SPIR-V, so these slices span it.
    let points = kdmean.len();
    let (s, d, w) = unsafe {
        (
            core::slice::from_raw_parts_mut(s, points * nspec),
            core::slice::from_raw_parts_mut(d, points * nspec),
            core::slice::from_raw_parts_mut(w, points * len),
        )
    };
    let (spec_off, cg_off, w_off) = (point * nspec, point * p.nk as usize, point * len);
    w3snl1(p, sig, af11, pm, ic, a, cg, kdmean[point], s, d, w, spec_off, cg_off, w_off);
}

/// # Safety
/// The output buffers hold `points.len()` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn sds4(
    p: &Sds4Params,
    sig: &[Real],
    dden: &[Real],
    ec2: &[Real],
    es2: &[Real],
    esc: &[Real],
    ecos: &[Real],
    esin: &[Real],
    satindices: &[i32],
    satweights: &[Real],
    cumulw: &[Real],
    iktab: &[i32],
    dcki: &[Real],
    qbi: &[Real],
    a: &[Real],
    k: &[Real],
    cg: &[Real],
    points: &[Sds4Point],
    srhs: *mut Real,
    ddiag: *mut Real,
    brlambda: *mut Real,
    whitecap: *mut Real,
    w: *mut Real,
    iw: *mut i32,
) {
    let point = thread::index_1d() as usize;
    if point >= points.len() {
        return;
    }
    let n = points.len();
    let nspec = p.nspec as usize;
    let (lw, li) = sds4::work_len(p);
    // As in `snl1`: whole buffers and offsets.
    let (srhs, ddiag, brlambda, whitecap, w, iw) = unsafe {
        (
            core::slice::from_raw_parts_mut(srhs, n * nspec),
            core::slice::from_raw_parts_mut(ddiag, n * nspec),
            core::slice::from_raw_parts_mut(brlambda, n * nspec),
            core::slice::from_raw_parts_mut(whitecap, n * 4),
            core::slice::from_raw_parts_mut(w, n * lw),
            core::slice::from_raw_parts_mut(iw, n * li),
        )
    };
    w3sds4(
        p, sig, dden, ec2, es2, esc, ecos, esin, satindices, satweights, cumulw, iktab, dcki, qbi, a, k, cg,
        &points[point], srhs, ddiag, brlambda, whitecap, w, iw,
        point * nspec, point * p.nk as usize, point * 4, point * lw, point * li,
    );
}
