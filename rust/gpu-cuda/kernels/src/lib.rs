// SPDX-License-Identifier: MPL-2.0
//! PTX entry points. One thread is one sea point: its spectrum, group velocities and scratch are the
//! point's ranges of the buffers.

use cuda_std::prelude::*;
use ww3_kernels::pro3::{
    Ktp3Params, Ktp3Point, Ktp3Tables, Ktp3Work, NoBoundary, Xyp3Params, Xyp3Tables, Xyp3Work, w3ktp3, w3xyp3_va,
};
use ww3_kernels::flx1::{Flx1In, Flx1Out, w3flx1};
use ww3_kernels::real::Real;
use ww3_kernels::sbt4::{Sbt4Out, Sbt4Params, Sbt4Point, Sbt4Tables, w3sbt4};
use ww3_kernels::sin3::{Sin3Out, Sin3Params, Sin3Point, w3sin3};
use ww3_kernels::sds3::{Sds3Params, Sds3Point, w3sds3};
use ww3_kernels::sds1::{Sds1Params, Sds1Point, w3sds1};
use ww3_kernels::sds4::{Sds4Params, Sds4Point, w3sds4};
use ww3_kernels::sic1::{Sic1Out, Sic1Params, w3sic1};
use ww3_kernels::sin1::{Sin1Params, Sin1Point, w3sin1};
use ww3_kernels::sin4::{Sin4Out, Sin4Params, Sin4Point, Spr4Out, Spr4Params, Spr4Point, w3sin4, w3spr4};
use ww3_kernels::sis1::{Sis1Params, Sis1Point, w3sis1};
use ww3_kernels::snl1::{Snl1Params, w3snl1};
use ww3_kernels::spr1::{Spr1Out, Spr1Params, w3spr1};
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

/// `W3SIC1`. `icecoef1` holds `ICEP1(IX,IY)` of each point.
///
/// # Safety
/// The output buffers hold `icecoef1.len()` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn sic1(
    p: &Sic1Params,
    mapwn: &[i32],
    a: &[Real],
    cg: &[Real],
    icecoef1: &[Real],
    s: *mut Real,
    d: *mut Real,
    out: *mut Sic1Out,
) {
    let point = thread::index_1d() as usize;
    if point >= icecoef1.len() {
        return;
    }
    let n = icecoef1.len();
    let nk = p.nk as usize;
    let nspec = nk * p.nth as usize;
    // As in `snl1`: whole buffers and offsets.
    let (s, d) = unsafe {
        (core::slice::from_raw_parts_mut(s, n * nspec), core::slice::from_raw_parts_mut(d, n * nspec))
    };
    let o = w3sic1(p, mapwn, a, cg, icecoef1[point], s, d, point * nspec, point * nk);
    unsafe { *out.add(point) = o };
}

/// `W3FLX1`, one point per thread.
///
/// # Safety
/// `out` holds `points.len()` records, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions)]
pub unsafe fn flx1(points: &[Flx1In], out: *mut Flx1Out) {
    let point = thread::index_1d() as usize;
    if point >= points.len() {
        return;
    }
    unsafe { *out.add(point) = w3flx1(&points[point]) };
}

/// `W3SDS1`. `k` is `WN2`, `NSPEC` values per point.
///
/// # Safety
/// The output buffers hold `points.len()` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn sds1(p: &Sds1Params, a: &[Real], k: &[Real], points: &[Sds1Point], s: *mut Real, d: *mut Real) {
    let point = thread::index_1d() as usize;
    if point >= points.len() {
        return;
    }
    let nspec = (p.nk * p.nth) as usize;
    // As in `snl1`: whole buffers and offsets.
    let (s, d) = unsafe {
        (
            core::slice::from_raw_parts_mut(s, points.len() * nspec),
            core::slice::from_raw_parts_mut(d, points.len() * nspec),
        )
    };
    w3sds1(p, &points[point], a, k, s, d, point * nspec, point * nspec);
}

/// `W3SBT4`. `bedform` holds three values per point, in and out; `w` holds `NK` per point.
///
/// # Safety
/// The output buffers hold `points.len()` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn sbt4(
    p: &Sbt4Params,
    sig: &[Real],
    dden: &[Real],
    ecos: &[Real],
    esin: &[Real],
    fwtable: &[Real],
    erftable: &[Real],
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
    points: &[Sbt4Point],
    bedform: *mut Real,
    s: *mut Real,
    d: *mut Real,
    w: *mut Real,
    out: *mut Sbt4Out,
) {
    let point = thread::index_1d() as usize;
    if point >= points.len() {
        return;
    }
    let n = points.len();
    let nk = p.nk as usize;
    let nspec = nk * p.nth as usize;
    // As in `snl1`: whole buffers and offsets.
    let (bedform, s, d, w) = unsafe {
        (
            core::slice::from_raw_parts_mut(bedform, n * 3),
            core::slice::from_raw_parts_mut(s, n * nspec),
            core::slice::from_raw_parts_mut(d, n * nspec),
            core::slice::from_raw_parts_mut(w, n * nk),
        )
    };
    let t = Sbt4Tables { sig, dden, ecos, esin, fwtable, erftable };
    let o = w3sbt4(p, &t, a, cg, wn, &points[point], bedform, s, d, w, point * nspec, point * nk, point * nk, point * 3);
    unsafe { *out.add(point) = o };
}

/// `W3SIN3`.
///
/// # Safety
/// The output buffers hold `points.len()` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn sin3(
    p: &Sin3Params,
    sig: &[Real],
    sig2: &[Real],
    dden2: &[Real],
    ecos: &[Real],
    esin: &[Real],
    tauhft: &[Real],
    a: &[Real],
    cg: &[Real],
    k: &[Real],
    points: &[Sin3Point],
    s: *mut Real,
    d: *mut Real,
    llws: *mut u32,
    out: *mut Sin3Out,
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
    w3sin3(
        p, sig, sig2, dden2, ecos, esin, tauhft, a, cg, k, &points[point], s, d, llws, out, point * nspec,
        point * p.nk as usize, point,
    );
}

/// `W3SIN1`.
///
/// # Safety
/// The output buffers hold `points.len()` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn sin1(
    p: &Sin1Params,
    sig2: &[Real],
    ecos: &[Real],
    esin: &[Real],
    a: &[Real],
    k: &[Real],
    points: &[Sin1Point],
    s: *mut Real,
    d: *mut Real,
) {
    let point = thread::index_1d() as usize;
    if point >= points.len() {
        return;
    }
    let n = points.len();
    let nspec = p.nspec as usize;
    let (s, d) = unsafe {
        (core::slice::from_raw_parts_mut(s, n * nspec), core::slice::from_raw_parts_mut(d, n * nspec))
    };
    w3sin1(p, sig2, ecos, esin, a, k, &points[point], s, d, point * nspec);
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

/// `W3SPR1`. `eb` holds `NK` values per point; `a` holds `NSPEC`, `cg` and `wn` `NK`.
///
/// # Safety
/// `eb` and `out` hold `n` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn spr1(
    p: &Spr1Params,
    sig: &[Real],
    dden: &[Real],
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
    n: u32,
    eb: *mut Real,
    out: *mut Spr1Out,
) {
    let point = thread::index_1d() as usize;
    if point >= n as usize {
        return;
    }
    let nk = p.nk as usize;
    let eb = unsafe { core::slice::from_raw_parts_mut(eb, n as usize * nk) };
    let o = w3spr1(p, sig, dden, a, cg, wn, eb, point * nk * p.nth as usize, point * nk, point * nk);
    unsafe { *out.add(point) = o };
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
    active: &[u32],
    erftable: &[Real],
    bedform: *mut Real,
) {
    let point = thread::index_1d() as usize;
    if point >= points.len() || active[point] == 0 {
        return;
    }
    let n = points.len();
    let nspec = p.nspec as usize;
    let (l_snl, l_sds, l_ids) = work_lens(p);
    let t = SrceTables { sig, sig2, dden, dden2, ec2, es2, esc, ecos, esin, af11, pm, ic, satindices, satweights, cumulw, iktab, dcki, qbi, fwtable, tauhft, tauhft2, taut, erftable };
    // BEDFORM is three per point with BT4; a BT1 run may pass an empty buffer, never read.
    let nb = if p.btopt == ww3_kernels::srce::SRCE_BT4 { n * 3 } else { 0 };
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
        bedform: unsafe { core::slice::from_raw_parts_mut(bedform, nb) },
    };
    let spec = unsafe { core::slice::from_raw_parts_mut(spec, n * nspec) };
    let out = unsafe { core::slice::from_raw_parts_mut(out, n) };
    w3srce(p, &t, &points[point], spec, wn1, cg1, &mut w, out, point);
}

/// One thread per spectral component: `vq` and the work arrays hold a block per component.
///
/// # Safety
/// Each pointer holds its length, and each thread writes only its own block.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn xyp3(
    p: &Xyp3Params,
    comps: u32,
    yfirst: u32,
    dtg: Real,
    sig: &[Real],
    ecos: &[Real],
    esin: &[Real],
    mapsf: &[i32],
    mapsta: &[i32],
    mapstx: &[i32],
    mapaxy: &[i32],
    mapcxy: &[i32],
    mapx2: &[i32],
    mapy2: &[i32],
    maptrn: &[u32],
    clats: &[Real],
    dpdx: &[Real],
    dpdy: &[Real],
    dqdx: &[Real],
    dqdy: &[Real],
    gsqrt: &[Real],
    cg: &[Real],
    cx: &[Real],
    cy: &[Real],
    atrnx: &[Real],
    atrny: &[Real],
    vq: *mut Real,
    vq_len: usize,
    iixy1: *mut i32,
    iixy1_len: usize,
    iixy2: *mut i32,
    iixy2_len: usize,
    iixy3: *mut i32,
    iixy3_len: usize,
    iixy4: *mut i32,
    iixy4_len: usize,
    rdi1: *mut Real,
    rdi1_len: usize,
    rdi2: *mut Real,
    rdi2_len: usize,
    rdi3: *mut Real,
    rdi3_len: usize,
    rdi4: *mut Real,
    rdi4_len: usize,
    vlcflx: *mut Real,
    vlcflx_len: usize,
    vlcfly: *mut Real,
    vlcfly_len: usize,
    cxtot: *mut Real,
    cxtot_len: usize,
    cytot: *mut Real,
    cytot_len: usize,
    aq: *mut Real,
    aq_len: usize,
    fla: *mut Real,
    fla_len: usize,
    va: *mut Real,
    va_len: usize,
) {
    let comp = thread::index_1d() as usize;
    if comp as u32 >= comps {
        return;
    }
    let vq = unsafe { core::slice::from_raw_parts_mut(vq, vq_len) };
    let iixy1 = unsafe { core::slice::from_raw_parts_mut(iixy1, iixy1_len) };
    let iixy2 = unsafe { core::slice::from_raw_parts_mut(iixy2, iixy2_len) };
    let iixy3 = unsafe { core::slice::from_raw_parts_mut(iixy3, iixy3_len) };
    let iixy4 = unsafe { core::slice::from_raw_parts_mut(iixy4, iixy4_len) };
    let rdi1 = unsafe { core::slice::from_raw_parts_mut(rdi1, rdi1_len) };
    let rdi2 = unsafe { core::slice::from_raw_parts_mut(rdi2, rdi2_len) };
    let rdi3 = unsafe { core::slice::from_raw_parts_mut(rdi3, rdi3_len) };
    let rdi4 = unsafe { core::slice::from_raw_parts_mut(rdi4, rdi4_len) };
    let vlcflx = unsafe { core::slice::from_raw_parts_mut(vlcflx, vlcflx_len) };
    let vlcfly = unsafe { core::slice::from_raw_parts_mut(vlcfly, vlcfly_len) };
    let cxtot = unsafe { core::slice::from_raw_parts_mut(cxtot, cxtot_len) };
    let cytot = unsafe { core::slice::from_raw_parts_mut(cytot, cytot_len) };
    let aq = unsafe { core::slice::from_raw_parts_mut(aq, aq_len) };
    let fla = unsafe { core::slice::from_raw_parts_mut(fla, fla_len) };
    let va = unsafe { core::slice::from_raw_parts_mut(va, va_len) };
    let t = Xyp3Tables { sig, ecos, esin, mapsf, mapsta, mapstx, mapaxy, mapcxy, mapx2, mapy2, maptrn, clats, dpdx, dpdy, dqdx, dqdy, gsqrt, cg, cx, cy, atrnx, atrny };
    let mut w = Xyp3Work { iixy1, iixy2, iixy3, iixy4, rdi1, rdi2, rdi3, rdi4, vlcflx, vlcfly, cxtot, cytot, aq, fla };
    w3xyp3_va(p, &t, comp as i32 + 1, dtg, yfirst != 0, comp, comps as usize, 0, va, vq, &mut w, NoBoundary);
}

/// One thread per active sea point.
///
/// # Safety
/// Each pointer holds its length, and each thread writes only its own block.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn ktp3(
    p: &Ktp3Params,
    points: &[Ktp3Point],
    sig: &[Real],
    dsip: &[Real],
    ecos: &[Real],
    esin: &[Real],
    ec2: &[Real],
    esc: &[Real],
    es2: &[Real],
    mapwn: &[i32],
    mapth2: &[i32],
    mapwn2: &[i32],
    cg: &[Real],
    wn: &[Real],
    va: *mut Real,
    va_len: usize,
    dsdd: *mut Real,
    dsdd_len: usize,
    frk: *mut Real,
    frk_len: usize,
    frg: *mut Real,
    frg_len: usize,
    fkc: *mut Real,
    fkc_len: usize,
    vq: *mut Real,
    vq_len: usize,
    db: *mut Real,
    db_len: usize,
    dm: *mut Real,
    dm_len: usize,
    vcflt: *mut Real,
    vcflt_len: usize,
    cflk: *mut Real,
    cflk_len: usize,
    fla: *mut Real,
    fla_len: usize,
    active: &[u32],
) {
    let point = thread::index_1d() as usize;
    if point >= points.len() || active[point] == 0 {
        return;
    }
    let va = unsafe { core::slice::from_raw_parts_mut(va, va_len) };
    let dsdd = unsafe { core::slice::from_raw_parts_mut(dsdd, dsdd_len) };
    let frk = unsafe { core::slice::from_raw_parts_mut(frk, frk_len) };
    let frg = unsafe { core::slice::from_raw_parts_mut(frg, frg_len) };
    let fkc = unsafe { core::slice::from_raw_parts_mut(fkc, fkc_len) };
    let vq = unsafe { core::slice::from_raw_parts_mut(vq, vq_len) };
    let db = unsafe { core::slice::from_raw_parts_mut(db, db_len) };
    let dm = unsafe { core::slice::from_raw_parts_mut(dm, dm_len) };
    let vcflt = unsafe { core::slice::from_raw_parts_mut(vcflt, vcflt_len) };
    let cflk = unsafe { core::slice::from_raw_parts_mut(cflk, cflk_len) };
    let fla = unsafe { core::slice::from_raw_parts_mut(fla, fla_len) };
    let t = Ktp3Tables { sig, dsip, ecos, esin, ec2, esc, es2, mapwn, mapth2, mapwn2 };
    let mut w = Ktp3Work { dsdd, frk, frg, fkc, vq, db, dm, vcflt, cflk, fla };
    w3ktp3(p, &t, &points[point], point, cg, wn, va, &mut w);
}

/// `W3SPR3`. `w` holds `2 * NK` per point.
///
/// # Safety
/// `w` and `out` hold `points.len()` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn spr3(
    p: &ww3_kernels::spr3::Spr3Params,
    sig: &[Real],
    dden: &[Real],
    taut: &[Real],
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
    llws: &[u32],
    points: &[ww3_kernels::spr3::Spr3Point],
    w: *mut Real,
    out: *mut ww3_kernels::spr3::Spr3Out,
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
    ww3_kernels::spr3::w3spr3(
        p, sig, dden, taut, a, cg, wn, llws, &points[point], w, out, point * p.nspec as usize, point * nk,
        point * 2 * nk, point,
    );
}

/// `W3SIS1`. `a` and `s` hold `NSPEC` per point.
///
/// # Safety
/// The output buffer holds `points.len()` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions)]
pub unsafe fn sis1(p: &Sis1Params, a: &[Real], points: &[Sis1Point], s: *mut Real) {
    let point = thread::index_1d() as usize;
    if point >= points.len() {
        return;
    }
    let nspec = (p.nk * p.nth) as usize;
    // As in `snl1`: whole buffers and offsets.
    let s = unsafe { core::slice::from_raw_parts_mut(s, points.len() * nspec) };
    w3sis1(p, a, &points[point], s, point * nspec);
}

/// `W3SDS3`. `k` holds `NK` per point.
///
/// # Safety
/// The output buffers hold `points.len()` ranges, and each thread writes only its own.
#[kernel]
#[allow(improper_ctypes_definitions, clippy::too_many_arguments)]
pub unsafe fn sds3(p: &Sds3Params, a: &[Real], k: &[Real], points: &[Sds3Point], s: *mut Real, d: *mut Real) {
    let point = thread::index_1d() as usize;
    if point >= points.len() {
        return;
    }
    let n = points.len();
    let nk = p.nk as usize;
    let nspec = nk * p.nth as usize;
    // As in `snl1`: whole buffers and offsets.
    let (s, d) =
        unsafe { (core::slice::from_raw_parts_mut(s, n * nspec), core::slice::from_raw_parts_mut(d, n * nspec)) };
    w3sds3(p, k, a, &points[point], s, d, point * nspec, point * nk);
}
