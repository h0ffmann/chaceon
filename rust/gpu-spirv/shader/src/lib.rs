// SPDX-License-Identifier: MPL-2.0
//! Compute entry points. One invocation is one sea point: its spectrum, group velocities and scratch
//! are the point's ranges of the bound buffers.
#![no_std]

use spirv_std::glam::UVec3;
use spirv_std::spirv;
use ww3_kernels::pro3::{
    Ktp3Params, Ktp3Point, Ktp3Tables, Ktp3Work, NoBoundary, Xyp3Params, Xyp3Tables, Xyp3Work, w3ktp3, w3xyp3_va,
};
use ww3_kernels::real::Real;
use ww3_kernels::sds4::{Sds4Params, Sds4Point, w3sds4};
use ww3_kernels::sin4::{Sin4Out, Sin4Params, Sin4Point, Spr4Out, Spr4Params, Spr4Point, w3sin4, w3spr4};
use ww3_kernels::snl1::{Snl1Params, w3snl1};
use ww3_kernels::srce::{SrceOut, SrceParams, SrcePoint, SrceTables, SrceWork, w3srce};
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

#[spirv(compute(threads(64)))]
#[allow(clippy::too_many_arguments)]
pub fn srce(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] p: &SrceParams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] sig: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] sig2: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] dden: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] dden2: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] ec2: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] es2: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] esc: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] ecos: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] esin: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 10)] af11: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] pm: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] ic: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 13)] satindices: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] satweights: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 15)] cumulw: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] iktab: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 17)] dcki: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 18)] qbi: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 19)] fwtable: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 20)] tauhft: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 21)] tauhft2: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 22)] taut: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 23)] points: & [SrcePoint],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 24)] wn1: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 25)] cg1: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 26)] spec: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 27)] out: &mut [SrceOut],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 28)] specinit: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 29)] dam: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 30)] wn2: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 31)] vsln: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 32)] vsin: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 33)] vdin: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 34)] vsnl: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 35)] vdnl: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 36)] vsds: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 37)] vdds: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 38)] vsbt: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 39)] vdbt: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 40)] vsdb: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 41)] vddb: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 42)] vs: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 43)] vd: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 44)] brlambda: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 45)] llws: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 46)] w_snl: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 47)] w_sds: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 48)] iw_sds: &mut [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 49)] w_small: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 50)] whitecap: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 51)] sin_out: &mut [Sin4Out],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 52)] spr_out: &mut [Spr4Out],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 53)] active: &[u32],
) {
    let point = id.x as usize;
    if point >= points.len() || active[point] == 0 {
        return;
    }
    let pt = points[point];
    let t = SrceTables { sig, sig2, dden, dden2, ec2, es2, esc, ecos, esin, af11, pm, ic, satindices, satweights, cumulw, iktab, dcki, qbi, fwtable, tauhft, tauhft2, taut };
    let mut w = SrceWork { specinit, dam, wn2, vsln, vsin, vdin, vsnl, vdnl, vsds, vdds, vsbt, vdbt, vsdb, vddb, vs, vd, brlambda, llws, w_snl, w_sds, iw_sds, w_small, whitecap, sin_out, spr_out };
    w3srce(p, &t, &pt, spec, wn1, cg1, &mut w, out, point);
}

/// One thread per spectral component: `vq` and the work arrays hold a block per component. `call` is
/// `YFIRST`, `dtg` the step.
#[spirv(compute(threads(64)))]
#[allow(clippy::too_many_arguments)]
pub fn xyp3(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] p: &Xyp3Params,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] call: &[i32; 2],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] dtg: &Real,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] sig: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] ecos: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] esin: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] mapsf: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] mapsta: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] mapstx: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] mapaxy: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 10)] mapcxy: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] mapx2: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] mapy2: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 13)] maptrn: & [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] clats: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 15)] dpdx: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] dpdy: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 17)] dqdx: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 18)] dqdy: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 19)] gsqrt: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 20)] cg: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 21)] cx: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 22)] cy: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 23)] atrnx: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 24)] atrny: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 25)] vq: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 26)] iixy1: &mut [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 27)] iixy2: &mut [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 28)] iixy3: &mut [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 29)] iixy4: &mut [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 30)] rdi1: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 31)] rdi2: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 32)] rdi3: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 33)] rdi4: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 34)] vlcflx: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 35)] vlcfly: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 36)] cxtot: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 37)] cytot: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 38)] aq: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 39)] fla: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 40)] va: &mut [Real],
) {
    let comp = id.x as usize;
    if comp as i32 >= call[0] {
        return;
    }
    let t = Xyp3Tables { sig, ecos, esin, mapsf, mapsta, mapstx, mapaxy, mapcxy, mapx2, mapy2, maptrn, clats, dpdx, dpdy, dqdx, dqdy, gsqrt, cg, cx, cy, atrnx, atrny };
    let mut w = Xyp3Work { iixy1, iixy2, iixy3, iixy4, rdi1, rdi2, rdi3, rdi4, vlcflx, vlcfly, cxtot, cytot, aq, fla };
    w3xyp3_va(p, &t, comp as i32 + 1, *dtg, call[1] != 0, comp, call[0] as usize, 0, va, vq, &mut w, NoBoundary);
}

/// One thread per active sea point.
#[spirv(compute(threads(64)))]
#[allow(clippy::too_many_arguments)]
pub fn ktp3(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] p: &Ktp3Params,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] points: &[Ktp3Point],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] sig: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] dsip: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] ecos: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] esin: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] ec2: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] esc: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] es2: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] mapwn: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 10)] mapth2: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] mapwn2: & [i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] cg: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 13)] wn: & [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] va: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 15)] dsdd: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] frk: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 17)] frg: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 18)] fkc: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 19)] vq: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 20)] db: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 21)] dm: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 22)] vcflt: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 23)] cflk: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 24)] fla: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 25)] active: &[u32],
) {
    let point = id.x as usize;
    if point >= points.len() || active[point] == 0 {
        return;
    }
    let t = Ktp3Tables { sig, dsip, ecos, esin, ec2, esc, es2, mapwn, mapth2, mapwn2 };
    let mut w = Ktp3Work { dsdd, frk, frg, fkc, vq, db, dm, vcflt, cflk, fla };
    let q = points[point];
    w3ktp3(p, &t, &q, point, cg, wn, va, &mut w);
}

/// Each `ww3-libm` function alone, so `spirv_replay libm` can compare it with the host's build.
#[spirv(compute(threads(64)))]
pub fn libm_probe(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] which: &u32,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] x: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] y: &[Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] r: &mut [Real],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] r2: &mut [Real],
) {
    let i = id.x as usize;
    if i >= x.len() {
        return;
    }
    let (a, b) = (x[i], y[i]);
    r[i] = match *which {
        0 => ww3_libm::expf(a),
        1 => ww3_libm::logf(a),
        2 => ww3_libm::powf(a, b),
        3 => {
            let (s, c) = ww3_libm::sincosf(a);
            r2[i] = c;
            s
        }
        4 => ww3_libm::tanhf(a),
        5 => ww3_libm::atan2f(a, b),
        6 => ww3_libm::log10f(a),
        7 => ww3_libm::sqrtf(a),
        8 => ww3_libm::atanf(a),
        _ => ww3_libm::expm1f(a),
    };
}
