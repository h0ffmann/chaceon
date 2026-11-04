// SPDX-License-Identifier: MPL-2.0
//! `W3SIN6`, ST6's wind input (Donelan, Babanin, Young, Banner 2006, as Rogers et al. 2012 use it),
//! with the module's helpers `LFACTOR`, `TAU_WAVE_ATMOS`, `IRANGE` and `TAUWINDS`. Translated from
//! `model/src/w3src6md.F90:290-519` (`W3SIN6`), `:769-1044` (`LFACTOR`), `:1066-1197`
//! (`TAU_WAVE_ATMOS`), `:1214-1243` (`IRANGE`) and `:1267-1296` (`TAUWINDS`) at metocean/WW3
//! `92e2cf66`.
//!
//! The order of operations is the one gfortran 12.2 `-O3 -ffp-contract=off` compiled on aarch64, read
//! from the optimised GIMPLE of the module (`rust/port_report/W3SIN6.md`): `LFACTOR` is inlined into
//! `W3SIN6`; `COS(USDIR)` and `SIN(USDIR)` are one `sincosf`; `ALOG`, `EXP`, `TANH` and `XFR**x` call
//! `logf`, `expf`, `tanhf` and `powf`; `DRTAU**SIGN_NEW` is `__powisf2`; every `SUM` adds in index
//! order starting from `0.`; `GRAV*DWAT` is one folded constant; nothing fuses.
//!
//! The Fortran's whole-array statements are written as element loops: each array statement is
//! elementwise, so a value is the same, and the temporaries (`ALLOCATE`, `RESHAPE`) become a `work`
//! buffer the caller owns, since a kernel may not allocate. `CD` is an argument of `W3SIN6` that its
//! body never reads, and the `STME21` and `WRITE` of `LFACTOR` (test output and the warning when
//! `ITERMAX` is reached) are I/O: the warning is `reach` bit 1.
//!
//! `MAX` and `MIN` are `fmaxnm` and `fminnm` at `-O3` (`MAX_EXPR`, `MIN_EXPR`): a NaN operand loses.
//! `max_nm` and `min_nm` are that, spelled so SPIR-V does not pick `FMax`. They differ from `fmaxnm`
//! only for `+0` against `-0`, and no site here can see that: `x - 1.` is never `-0`, and every
//! `MIN(0.,S)` is squared or `ABS`ed.
#![allow(clippy::assign_op_pattern, clippy::manual_clamp)] // `x = x + y` keeps the Fortran line.

use crate::constants::{DAIR, DWAT, GRAV, TPI};
use crate::real::Real;
use crate::util::{pow2, pow3, sign};
use ww3_libm::{expf, logf, powf, sincosf, sqrtf, tanhf};

/// Scalars `W3SIN6` and its helpers read from `W3GDATMD`. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin6Params {
    pub nk: i32,
    pub nth: i32,
    pub nspec: i32,
    pub dth: Real,
    pub xfr: Real,
    /// `SIN6A0`, `SINA0` of the `SIN6` namelist.
    pub sin6a0: Real,
    /// `SIN6WS`, `SINWS` of the `SIN6` namelist.
    pub sin6ws: Real,
}

/// The scalars of one `W3SIN6` call. `CD` is passed in the Fortran and never read.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin6Point {
    pub uabs: Real,
    pub ustar: Real,
    pub usdir: Real,
    pub cd: Real,
    pub dair: Real,
}

/// The scalars one `W3SIN6` call returns.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin6Out {
    pub tauwx: Real,
    pub tauwy: Real,
    pub tauwnx: Real,
    pub tauwny: Real,
    /// `IK` of `LFACTOR` after its loop: 0 when `TAU <= TAU_TOT`, 81 when `ITERMAX` ran out.
    pub iters: i32,
    /// Which branches ran, for reading what a capture reached; the arithmetic never reads it. Bit 0:
    /// the `LFACTOR` iteration ran (`:975`). 1: it reached `ITERMAX` and the warning is written
    /// (`:1017`). 2: `OVERSHOT` slowed `DRTAU` (`:1010`). 3: the grid is extended to 10 Hz
    /// (`NK < NK10Hz`, `:919`). 4: it is not (`:935`). 5: `SUM(LFACT) < NK` reduced the whole spectrum
    /// (`:493`). 6: the negative input ran (`SIN6A0 > 0`, `:505`). 7: a `KMAX` below `1.E-34` set
    /// `K = 1` (`:459`). 8: `TAU_VIS` was capped at `0.95*TAU_TOT` (`:952`). 9: `U10` is above the
    /// viscous stress's zero, `-5.E-5*U10 + 1.1E-3 < 0` (`:950`). 10: the loop's `SIGN_NEW` was -1 at
    /// some pass. 11: the loop left at `ABS(ERR) < 1.54E-4` rather than at `ITERMAX`. 12: `TAU_WAVE_ATMOS`
    /// extended its grid to 10 Hz. 13: some `S` was negative at `TAU_WAVE_ATMOS`.
    pub reach: u32,
}

/// `FRQMAX` and `ITERMAX` of `LFACTOR` (`:874`, `:875`).
const FRQMAX: Real = 10.;
const ITERMAX: i32 = 80;

/// What gfortran's `MAX(x, y)` and `MIN(x, y)` compile to on arm64 (`fmaxnm`, `fminnm`) when `y` is
/// the constant: a NaN `x` returns `y`.
#[inline]
fn max_nm(x: Real, y: Real) -> Real {
    if y != y || x > y { x } else { y }
}
#[inline]
fn min_nm(x: Real, y: Real) -> Real {
    if y != y || x < y { x } else { y }
}

/// `__powisf2` of libgcc, which gfortran calls for `DRTAU**SIGN_NEW` with a run-time `INTEGER` exponent.
#[inline]
fn powi_f(x: Real, m: i32) -> Real {
    let mut n: u32 = if m < 0 { (m as u32).wrapping_neg() } else { m as u32 };
    let mut x = x;
    let mut y: Real = if n % 2 != 0 { x } else { 1. };
    n >>= 1;
    while n != 0 {
        x = x * x;
        if n % 2 != 0 {
            y = y * x;
        }
        n >>= 1;
    }
    if m < 0 { 1. / y } else { y }
}

/// `IRANGE(X0,X1,DX)`, `w3src6md.F90:1214-1243`: its length `INT(REAL(X1-X0)/REAL(DX))+1`.
#[inline]
pub fn irange_len(x0: i32, x1: i32, dx: i32) -> i32 {
    ((x1 - x0) as Real / dx as Real) as i32 + 1
}

/// `IX(I)` of `IRANGE`, `I` from 1: `X0+(I-1)*DX`.
#[inline]
pub fn irange_at(x0: i32, dx: i32, i: i32) -> i32 {
    x0 + (i - 1) * dx
}

/// `NK10Hz` of `LFACTOR` (`:898-899`) and `TAU_WAVE_ATMOS` (`:1151-1152`): `CEILING(ALOG(FRQMAX/(SIG(1)/TPI))
/// /ALOG(XFR))+1`, at least `NK`. gfortran's `CEILING` is `INT(x)` plus one when `x > REAL(INT(x))`.
#[inline]
pub fn nk10hz(nk: i32, sig1: Real, xfr: Real) -> i32 {
    let x = logf(FRQMAX / (sig1 / TPI)) / logf(xfr);
    let t = x as i32;
    let c = if x <= t as Real { t + 1 } else { t + 2 };
    if nk > c { nk } else { c }
}

/// Where each array of one spectrum's scratch lives in `work`, from `base`.
#[derive(Clone, Copy)]
struct Lay {
    dsii: usize,
    sig: usize,
    cinv: usize,
    sqrtbn: usize,
    lfact: usize,
    sdensig: usize,
    sig10: usize,
    cinv10: usize,
    dsii10: usize,
    lf10: usize,
    sdens: usize,
    sdensx: usize,
    sdensy: usize,
    ucinv: usize,
    tmp: usize,
}

#[inline]
fn lay(nk: usize, nspec: usize, nk10: usize, base: usize) -> Lay {
    let h = base + 5 * nk + nspec;
    Lay {
        dsii: base,
        sig: base + nk,
        cinv: base + 2 * nk,
        sqrtbn: base + 3 * nk,
        lfact: base + 4 * nk,
        sdensig: base + 5 * nk,
        sig10: h,
        cinv10: h + nk10,
        dsii10: h + 2 * nk10,
        lf10: h + 3 * nk10,
        sdens: h + 4 * nk10,
        sdensx: h + 5 * nk10,
        sdensy: h + 6 * nk10,
        ucinv: h + 7 * nk10,
        tmp: h + 8 * nk10,
    }
}

/// `Real`s of scratch one `W3SIN6` call needs: the `NK` arrays `DSII`, `SIG`, `CINV`, `SQRTBN`, `LFACT`,
/// `SDENSIG(NSPEC)`, and eight arrays of `NK10Hz` for `LFACTOR` and `TAU_WAVE_ATMOS`, plus one temporary.
#[inline]
pub fn work_len(nk: usize, nspec: usize, nk10: usize) -> usize {
    5 * nk + nspec + 9 * nk10
}

/// `TAUWINDS`, `w3src6md.F90:1267-1296`: `GRAV * DWAT * SUM(SDENSIG*CINV*DSII)` over `n` bands, the
/// three arrays at `so`, `co` and `dof` of `w`.
#[inline]
pub fn tauwinds(w: &[Real], so: usize, co: usize, dof: usize, n: usize) -> Real {
    let mut val: Real = 0.;
    let mut i = 0;
    while i < n {
        val = val + (w[so + i] * w[co + i]) * w[dof + i];
        i += 1;
    }
    GRAV * DWAT * val
}

/// The 10 Hz extension shared by `LFACTOR` and `TAU_WAVE_ATMOS` (`:919-942`, `:1170-1190`): `SIG10Hz`,
/// `CINV10Hz` and `DSII10Hz` for all `NK10Hz` bands, from the model's `SIG`, `CINV` and `DSII`. Returns
/// whether the grid was extended.
#[inline]
fn grid10(p: &Sin6Params, w: &mut [Real], l: &Lay, nk: usize, nk10: usize) -> bool {
    let xfr = p.xfr;
    if nk < nk10 {
        let sig1 = w[l.sig];
        let mut i = 0;
        while (i as i32) < irange_len(1, nk10 as i32, 1) {
            // IK10Hz = REAL(IRANGE(1,NK10Hz,1)), SIG10Hz = SIG(1)*XFR**(IK10Hz-1.0)
            let ik10 = irange_at(1, 1, i as i32 + 1) as Real;
            w[l.sig10 + i] = sig1 * powf(xfr, ik10 - 1.0);
            i += 1;
        }
        i = 0;
        while i < nk {
            w[l.cinv10 + i] = w[l.cinv + i];
            i += 1;
        }
        while i < nk10 {
            w[l.cinv10 + i] = w[l.sig10 + i] * 0.101978; // 1/c = sigma/g
            i += 1;
        }
        let df = xfr - 1.0 / xfr;
        i = 0;
        while i < nk10 {
            w[l.dsii10 + i] = 0.5 * w[l.sig10 + i] * df;
            i += 1;
        }
        // The first and last frequency bin
        w[l.dsii10] = 0.5 * w[l.sig10] * (xfr - 1.0);
        w[l.dsii10 + nk10 - 1] = 0.5 * w[l.sig10 + nk10 - 1] * (xfr - 1.0) / xfr;
        true
    } else {
        let mut i = 0;
        while i < nk10 {
            w[l.sig10 + i] = w[l.sig + i];
            w[l.cinv10 + i] = w[l.cinv + i];
            w[l.dsii10 + i] = w[l.dsii + i];
            i += 1;
        }
        false
    }
}

/// `LFACTOR`, `w3src6md.F90:769-1044`. `SDENSIG(NTH,NK)` is at `l.sdensig` of `w` and `CINV`, `SIG`,
/// `DSII` at theirs; the reduction factor goes to `LFACT` of `w`. Returns `TAUWX`, `TAUWY`, the final
/// `IK` and the `reach` bits. `S` and `W3SIN6`'s `D` stay as they were.
#[inline]
fn lfactor(
    p: &Sin6Params,
    ecos: &[Real],
    esin: &[Real],
    u10: Real,
    ustar: Real,
    usdir: Real,
    w: &mut [Real],
    l: &Lay,
    nk10: usize,
) -> (Real, Real, i32, u32) {
    let (nk, nth) = (p.nk as usize, p.nth as usize);
    let dth = p.dth;
    let mut reach = 0u32;
    let (sinu, cosu) = sincosf(usdir);

    // 1) Either extrapolate up to 10 Hz or use the discrete grid.
    let ext = grid10(p, w, l, nk, nk10);
    reach |= if ext { 1 << 3 } else { 1 << 4 };
    let mut ik = 0usize;
    while ik < nk {
        let (mut sd, mut sx, mut sy): (Real, Real, Real) = (0., 0., 0.);
        let mut ith = 0;
        while ith < nth {
            sd = sd + w[l.sdensig + ith + ik * nth];
            ith += 1;
        }
        ith = 0;
        while ith < nth {
            let s = w[l.sdensig + ith + ik * nth];
            sx = sx + max_nm(s, 0.) * ecos[ith + ik * nth];
            ith += 1;
        }
        ith = 0;
        while ith < nth {
            let s = w[l.sdensig + ith + ik * nth];
            sy = sy + max_nm(s, 0.) * esin[ith + ik * nth];
            ith += 1;
        }
        w[l.sdens + ik] = sd * dth;
        w[l.sdensx + ik] = sx * dth;
        w[l.sdensy + ik] = sy * dth;
        ik += 1;
    }
    if ext {
        // Spectral slope for S_IN(F) is proportional to F**(-2)
        let (s0, sx0, sy0) = (w[l.sdens + nk - 1], w[l.sdensx + nk - 1], w[l.sdensy + nk - 1]);
        let signk = w[l.sig10 + nk - 1];
        let mut i = nk;
        while i < nk10 {
            let r = pow2(signk / w[l.sig10 + i]);
            w[l.sdens + i] = s0 * r;
            w[l.sdensx + i] = sx0 * r;
            w[l.sdensy + i] = sy0 * r;
            i += 1;
        }
    }

    // 2) Stress calculation. TAU_TOT, TAU_VIS and the check that it does not exceed TAU_TOT.
    let tau_tot = pow2(ustar) * DAIR;
    let v = max_nm(-5.0E-5 * u10 + 1.1E-3, 0.0);
    reach |= ((-5.0E-5 * u10 + 1.1E-3 < 0.) as u32) << 9;
    let tau_vis = v * pow2(u10) * DAIR;
    let cap = 0.95 * tau_tot;
    reach |= ((cap < tau_vis) as u32) << 8;
    let tau_vis = min_nm(tau_vis, cap);
    let tauvx = tau_vis * cosu;
    let tauvy = tau_vis * sinu;
    // The wave supported stress
    let mut tauwx = tauwinds(w, l.sdensx, l.cinv10, l.dsii10, nk10);
    let mut tauwy = tauwinds(w, l.sdensy, l.cinv10, l.dsii10, nk10);
    let taux = tauvx + tauwx;
    let tauy = tauvy + tauwy;
    let mut tau = sqrtf(pow2(taux) + pow2(tauy));
    let mut err = (tau - tau_tot) / tau_tot;

    // 3) Find reduced Sin(f) = L(f)*Sin(f) to satisfy TAU <= TAU_TOT
    let mut i = 0;
    while i < nk10 {
        w[l.lf10 + i] = 1.0;
        i += 1;
    }
    let mut ikl: i32 = 0;
    if tau > tau_tot {
        reach |= 1;
        let mut overshot = false;
        let mut rtau = err / 90.;
        let mut drtau: Real = 2.0;
        let mut sign_new = sign(1.0, err) as i32;
        let uproxy = p.sin6ws * ustar;
        i = 0;
        while i < nk10 {
            w[l.ucinv + i] = 1.0 - (uproxy * w[l.cinv10 + i]);
            i += 1;
        }
        ikl = 1;
        while ikl <= ITERMAX {
            i = 0;
            while i < nk10 {
                w[l.lf10 + i] = min_nm(expf(w[l.ucinv + i] * rtau), 1.0);
                i += 1;
            }
            // TAU_NND is only printed (`:991`, test output), so it is not computed.
            i = 0;
            while i < nk10 {
                w[l.tmp + i] = w[l.sdensx + i] * w[l.lf10 + i];
                i += 1;
            }
            tauwx = tauwinds(w, l.tmp, l.cinv10, l.dsii10, nk10);
            i = 0;
            while i < nk10 {
                w[l.tmp + i] = w[l.sdensy + i] * w[l.lf10 + i];
                i += 1;
            }
            tauwy = tauwinds(w, l.tmp, l.cinv10, l.dsii10, nk10);
            let taux = tauvx + tauwx;
            let tauy = tauvy + tauwy;
            tau = sqrtf(pow2(taux) + pow2(tauy));
            err = (tau - tau_tot) / tau_tot;
            let sign_old = sign_new;
            sign_new = sign(1.0, err) as i32;
            reach |= ((sign_new < 0) as u32) << 10;
            // Slow down DRTAU when overshot.
            if sign_new != sign_old {
                overshot = true;
            }
            if overshot {
                reach |= 1 << 2;
                drtau = max_nm(0.5 * (1.0 + drtau), 1.00010);
            }
            rtau = rtau * powi_f(drtau, sign_new);
            let aerr = if err < 0. { -err } else { err };
            if aerr < 1.54E-4 {
                reach |= 1 << 11;
                break;
            }
            ikl += 1;
        }
        if ikl >= ITERMAX {
            reach |= 1 << 1; // WRITE (NDST,280): the warning, no value
        }
    }
    i = 0;
    while i < nk {
        w[l.lfact + i] = w[l.lf10 + i];
        i += 1;
    }
    (tauwx, tauwy, ikl, reach)
}

/// `TAU_WAVE_ATMOS`, `w3src6md.F90:1066-1197`: `TAUNWX`, `TAUNWY` of the negative part of `SDENSIG`,
/// which is at `l.sdensig` of `w`. Returns them and whether it extended the grid.
#[inline]
fn tau_wave_atmos(p: &Sin6Params, ecos: &[Real], esin: &[Real], w: &mut [Real], l: &Lay, nk10: usize) -> (Real, Real, bool) {
    let (nk, nth) = (p.nk as usize, p.nth as usize);
    let dth = p.dth;
    let ext = grid10(p, w, l, nk, nk10);
    let mut ik = 0usize;
    while ik < nk {
        let (mut sx, mut sy): (Real, Real) = (0., 0.);
        let mut ith = 0;
        while ith < nth {
            let s = w[l.sdensig + ith + ik * nth];
            let m = min_nm(s, 0.);
            sx = sx + (if m < 0. { -m } else { m }) * ecos[ith + ik * nth];
            ith += 1;
        }
        ith = 0;
        while ith < nth {
            let s = w[l.sdensig + ith + ik * nth];
            let m = min_nm(s, 0.);
            sy = sy + (if m < 0. { -m } else { m }) * esin[ith + ik * nth];
            ith += 1;
        }
        w[l.sdensx + ik] = sx * dth;
        w[l.sdensy + ik] = sy * dth;
        ik += 1;
    }
    if ext {
        let (sx0, sy0) = (w[l.sdensx + nk - 1], w[l.sdensy + nk - 1]);
        let signk = w[l.sig10 + nk - 1];
        let mut i = nk;
        while i < nk10 {
            let r = pow2(signk / w[l.sig10 + i]);
            w[l.sdensx + i] = sx0 * r;
            w[l.sdensy + i] = sy0 * r;
            i += 1;
        }
    }
    let x = tauwinds(w, l.sdensx, l.cinv10, l.dsii10, nk10);
    let y = tauwinds(w, l.sdensy, l.cinv10, l.dsii10, nk10);
    (x, y, ext)
}

/// `W3SIN6`, `w3src6md.F90:290-519`, on one spectrum. `sig2`, `dden2`, `ecos`, `esin` are the model's
/// `(NSPEC)` tables; `a`, `wn2`, `s`, `d` start at `spec_off`, `cg` at `cg_off`, `work` at `work_off`
/// (`work_len` values) and the result goes to `out[out_off]`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sin6(
    p: &Sin6Params,
    sig2: &[Real],
    dden2: &[Real],
    ecos: &[Real],
    esin: &[Real],
    a: &[Real],
    cg: &[Real],
    wn2: &[Real],
    pt: &Sin6Point,
    s: &mut [Real],
    d: &mut [Real],
    work: &mut [Real],
    out: &mut [Sin6Out],
    spec_off: usize,
    cg_off: usize,
    work_off: usize,
    out_off: usize,
) {
    let (nk, nth, nspec) = (p.nk as usize, p.nth as usize, p.nspec as usize);
    let (ustar, usdir) = (pt.ustar, pt.usdir);
    let dth = p.dth;
    let nk10 = nk10hz(p.nk, sig2[0], p.xfr) as usize;
    let l = lay(nk, nspec, nk10, work_off);
    let mut reach = 0u32;

    // 0) --- set up a basic variables
    let (sinu, cosu) = sincosf(usdir);
    // `UPROXY = SIN6WS * USTAR` scales the friction velocity to wind speed in the boundary layer.
    let uproxy = p.sin6ws * ustar;
    let dair_w = pt.dair / DWAT;

    // IKN = IRANGE(1,NSPEC,NTH); DSII, SIG, CINV on the bands
    let mut ik = 1;
    while ik <= nk as i32 {
        let ikn = irange_at(1, nth as i32, ik) as usize - 1;
        let o = (ik - 1) as usize;
        work[l.dsii + o] = dden2[ikn] / dth / sig2[ikn];
        work[l.sig + o] = sig2[ikn];
        work[l.cinv + o] = wn2[spec_off + ikn] / sig2[ikn];
        ik += 1;
    }

    // 1) to 4)
    let mut ik = 0usize;
    while ik < nk {
        let o = spec_off + ik * nth;
        let sg = work[l.sig + ik];
        let wn = wn2[spec_off + ik * nth];
        // 1) 1d action density spectrum
        let mut sum: Real = 0.;
        let mut ith = 0;
        while ith < nth {
            sum = sum + a[o + ith];
            ith += 1;
        }
        let adensig = sum * sg * dth;
        // 2) normalised directional spectrum K(theta,sigma)
        let mut kmax: Real = f32::NEG_INFINITY;
        ith = 0;
        while ith < nth {
            let v = a[o + ith];
            if v >= kmax {
                kmax = v;
            }
            ith += 1;
        }
        let small = kmax < 1.0E-34;
        reach |= (small as u32) << 7;
        let mut ksum: Real = 0.;
        ith = 0;
        while ith < nth {
            let k = if small { 1. } else { a[o + ith] / kmax };
            ksum = ksum + k;
            ith += 1;
        }
        // 3) normalised spectral saturation BN(IK)
        let anar = 1.0 / (ksum * dth); // directional narrowness
        work[l.sqrtbn + ik] = sqrtf(anar * adensig * pow3(wn));
        ik += 1;
    }

    // 4) growth rate GAMMA and S for all directions for following winds (W1)
    ik = 0;
    while ik < nk {
        let sqrtbn2 = work[l.sqrtbn + ik];
        let cg2 = cg[cg_off + ik];
        let mut ith = 0;
        while ith < nth {
            let is = ith + ik * nth;
            let cinv2 = wn2[spec_off + is] / sig2[is];
            let w1 = pow2(max_nm(uproxy * cinv2 * (ecos[is] * cosu + esin[is] * sinu) - 1., 0.));
            let dv = dair_w * sig2[is] * (2.8 - (1. + tanhf(10. * sqrtbn2 * w1 - 11.))) * sqrtbn2 * w1;
            d[spec_off + is] = dv;
            let sv = dv * a[spec_off + is];
            s[spec_off + is] = sv;
            // 5) SDENSIG = RESHAPE(S*SIG2/CG2,(/ NTH,NK /))
            work[l.sdensig + is] = sv * sig2[is] / cg2;
            ith += 1;
        }
        ik += 1;
    }

    // 5) reduction factor LFACT from the non-directional spectral density of the wind input
    let (tauwx, tauwy, iters, r) = lfactor(p, ecos, esin, pt.uabs, ustar, usdir, work, &l, nk10);
    reach |= r;

    // 6) apply reduction (LFACT) to the entire spectrum
    let mut lsum: Real = 0.;
    ik = 0;
    while ik < nk {
        lsum = lsum + work[l.lfact + ik];
        ik += 1;
    }
    if lsum < nk as Real {
        reach |= 1 << 5;
        ik = 0;
        while ik < nk {
            let lf = work[l.lfact + ik];
            let mut ith = 0;
            while ith < nth {
                let is = spec_off + ith + ik * nth;
                d[is] = d[is] * lf;
                ith += 1;
            }
            ik += 1;
        }
        let mut is = 0;
        while is < nspec {
            s[spec_off + is] = d[spec_off + is] * a[spec_off + is];
            is += 1;
        }
    }

    // 7) negative wind input for adverse winds, a factor SIN6A0 of the favourable one
    let (mut tauwnx, mut tauwny) = (0., 0.);
    if p.sin6a0 > 0.0 {
        reach |= 1 << 6;
        ik = 0;
        while ik < nk {
            let sqrtbn2 = work[l.sqrtbn + ik];
            let cg2 = cg[cg_off + ik];
            let mut ith = 0;
            while ith < nth {
                let is = ith + ik * nth;
                let cinv2 = wn2[spec_off + is] / sig2[is];
                let w2 = pow2(min_nm(uproxy * cinv2 * (ecos[is] * cosu + esin[is] * sinu) - 1., 0.));
                let dv = d[spec_off + is]
                    - (dair_w * sig2[is] * p.sin6a0 * (2.8 - (1. + tanhf(10. * sqrtbn2 * w2 - 11.))) * sqrtbn2 * w2);
                d[spec_off + is] = dv;
                let sv = dv * a[spec_off + is];
                s[spec_off + is] = sv;
                reach |= ((sv < 0.) as u32) << 13;
                // The negative component of the wave supported stress
                work[l.sdensig + is] = sv * sig2[is] / cg2;
                ith += 1;
            }
            ik += 1;
        }
        let (x, y, ext) = tau_wave_atmos(p, ecos, esin, work, &l, nk10);
        tauwnx = x;
        tauwny = y;
        reach |= (ext as u32) << 12;
    }

    out[out_off] = Sin6Out { tauwx, tauwy, tauwnx, tauwny, iters, reach };
}
