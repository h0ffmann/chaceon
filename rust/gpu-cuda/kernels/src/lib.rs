// SPDX-License-Identifier: MPL-2.0
//! PTX entry points. One thread is one sea point: its spectrum, group velocities and scratch are the
//! point's ranges of the buffers.

use cuda_std::prelude::*;
use ww3_kernels::real::Real;
use ww3_kernels::snl1::{Snl1Params, w3snl1, work_len};

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
    let len = work_len(p);
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
