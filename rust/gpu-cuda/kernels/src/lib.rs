// SPDX-License-Identifier: MPL-2.0
//! PTX entry points. One thread is one sea point: its spectrum, group velocities and scratch are the
//! point's ranges of the buffers.

use cuda_std::prelude::*;
use ww3_kernels::real::Real;
use ww3_kernels::sds4::{Sds4Params, Sds4Point, w3sds4};
use ww3_kernels::sin4::{Sin4Out, Sin4Params, Sin4Point, Spr4Out, Spr4Params, Spr4Point, w3sin4, w3spr4};
use ww3_kernels::snl1::{Snl1Params, w3snl1};
use ww3_kernels::srce::{SrceOut, SrceParams, SrcePoint, SrceTables, SrceWork, w3srce, work_lens, work_small};
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

/// # Safety
/// The output buffers hold `points.len()` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn sin4(
    p: &Sin4Params,
    sig: &[Real],
    sig2: &[Real],
    dden: &[Real],
    dden2: &[Real],
    ecos: &[Real],
    esin: &[Real],
    fwtable: &[Real],
    tauhft: &[Real],
    tauhft2: &[Real],
    a: &[Real],
    cg: &[Real],
    k: &[Real],
    points: &[Sin4Point],
    s: *mut Real,
    d: *mut Real,
    llws: *mut u32,
    out: *mut Sin4Out,
) {
    let point = thread::index_1d() as usize;
    if point >= points.len() {
        return;
    }
    let n = points.len();
    let nspec = p.nspec as usize;
    let (s, d, llws, out) = unsafe {
        (
            core::slice::from_raw_parts_mut(s, n * nspec),
            core::slice::from_raw_parts_mut(d, n * nspec),
            core::slice::from_raw_parts_mut(llws, n * nspec),
            core::slice::from_raw_parts_mut(out, n),
        )
    };
    w3sin4(
        p, sig, sig2, dden, dden2, ecos, esin, fwtable, tauhft, tauhft2, a, cg, k, &points[point], s, d, llws,
        out, point * nspec, point * p.nk as usize, point,
    );
}

/// # Safety
/// `w` and `out` hold `points.len()` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn spr4(
    p: &Spr4Params,
    sig: &[Real],
    dden: &[Real],
    ecos: &[Real],
    esin: &[Real],
    taut: &[Real],
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
    llws: &[u32],
    points: &[Spr4Point],
    w: *mut Real,
    out: *mut Spr4Out,
) {
    let point = thread::index_1d() as usize;
    if point >= points.len() {
        return;
    }
    let n = points.len();
    let nk = p.nk as usize;
    let (w, out) = unsafe {
        (core::slice::from_raw_parts_mut(w, n * 2 * nk), core::slice::from_raw_parts_mut(out, n))
    };
    w3spr4(
        p, sig, dden, ecos, esin, taut, a, cg, wn, llws, &points[point], w, out, point * p.nspec as usize,
        point * nk, point * 2 * nk, point,
    );
}

/// # Safety
/// `spec`, `out` and every work buffer hold `points.len()` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn srce(
    p: &SrceParams,
    sig: &[Real],
    sig2: &[Real],
    dden: &[Real],
    dden2: &[Real],
    ec2: &[Real],
    es2: &[Real],
    esc: &[Real],
    ecos: &[Real],
    esin: &[Real],
    af11: &[Real],
    pm: &[i32],
    ic: &[i32],
    satindices: &[i32],
    satweights: &[Real],
    cumulw: &[Real],
    iktab: &[i32],
    dcki: &[Real],
    qbi: &[Real],
    fwtable: &[Real],
    tauhft: &[Real],
    tauhft2: &[Real],
    taut: &[Real],
    points: &[SrcePoint],
    wn1: &[Real],
    cg1: &[Real],
    spec: *mut Real,
    out: *mut SrceOut,
    specinit: *mut Real,
    dam: *mut Real,
    wn2: *mut Real,
    vsln: *mut Real,
    vsin: *mut Real,
    vdin: *mut Real,
    vsnl: *mut Real,
    vdnl: *mut Real,
    vsds: *mut Real,
    vdds: *mut Real,
    vsbt: *mut Real,
    vdbt: *mut Real,
    vsdb: *mut Real,
    vddb: *mut Real,
    vs: *mut Real,
    vd: *mut Real,
    brlambda: *mut Real,
    llws: *mut u32,
    w_snl: *mut Real,
    w_sds: *mut Real,
    iw_sds: *mut i32,
    w_small: *mut Real,
    whitecap: *mut Real,
    sin_out: *mut Sin4Out,
    spr_out: *mut Spr4Out,
) {
    let point = thread::index_1d() as usize;
    if point >= points.len() {
        return;
    }
    let n = points.len();
    let nspec = p.nspec as usize;
    let (l_snl, l_sds, l_ids) = work_lens(p);
    let t = SrceTables { sig, sig2, dden, dden2, ec2, es2, esc, ecos, esin, af11, pm, ic, satindices, satweights, cumulw, iktab, dcki, qbi, fwtable, tauhft, tauhft2, taut };
    let mut w = SrceWork {
        specinit: unsafe { core::slice::from_raw_parts_mut(specinit, n * nspec) },
        dam: unsafe { core::slice::from_raw_parts_mut(dam, n * nspec) },
        wn2: unsafe { core::slice::from_raw_parts_mut(wn2, n * nspec) },
        vsln: unsafe { core::slice::from_raw_parts_mut(vsln, n * nspec) },
        vsin: unsafe { core::slice::from_raw_parts_mut(vsin, n * nspec) },
        vdin: unsafe { core::slice::from_raw_parts_mut(vdin, n * nspec) },
        vsnl: unsafe { core::slice::from_raw_parts_mut(vsnl, n * nspec) },
        vdnl: unsafe { core::slice::from_raw_parts_mut(vdnl, n * nspec) },
        vsds: unsafe { core::slice::from_raw_parts_mut(vsds, n * nspec) },
        vdds: unsafe { core::slice::from_raw_parts_mut(vdds, n * nspec) },
        vsbt: unsafe { core::slice::from_raw_parts_mut(vsbt, n * nspec) },
        vdbt: unsafe { core::slice::from_raw_parts_mut(vdbt, n * nspec) },
        vsdb: unsafe { core::slice::from_raw_parts_mut(vsdb, n * nspec) },
        vddb: unsafe { core::slice::from_raw_parts_mut(vddb, n * nspec) },
        vs: unsafe { core::slice::from_raw_parts_mut(vs, n * nspec) },
        vd: unsafe { core::slice::from_raw_parts_mut(vd, n * nspec) },
        brlambda: unsafe { core::slice::from_raw_parts_mut(brlambda, n * nspec) },
        llws: unsafe { core::slice::from_raw_parts_mut(llws, n * nspec) },
        w_snl: unsafe { core::slice::from_raw_parts_mut(w_snl, n * l_snl) },
        w_sds: unsafe { core::slice::from_raw_parts_mut(w_sds, n * l_sds) },
        iw_sds: unsafe { core::slice::from_raw_parts_mut(iw_sds, n * l_ids) },
        w_small: unsafe { core::slice::from_raw_parts_mut(w_small, n * work_small(p)) },
        whitecap: unsafe { core::slice::from_raw_parts_mut(whitecap, n * 4) },
        sin_out: unsafe { core::slice::from_raw_parts_mut(sin_out, n) },
        spr_out: unsafe { core::slice::from_raw_parts_mut(spr_out, n) },
    };
    let spec = unsafe { core::slice::from_raw_parts_mut(spec, n * nspec) };
    let out = unsafe { core::slice::from_raw_parts_mut(out, n) };
    w3srce(p, &t, &points[point], spec, wn1, cg1, &mut w, out, point);
}
