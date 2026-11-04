// SPDX-License-Identifier: MPL-2.0
//! `W3SIN3`, ST3's wind input (WAM cycle 4, Janssen 1991, with Bidlot's high-frequency tail).
//! Translated from `model/src/w3src3md.F90:384-717` at metocean/WW3 `92e2cf66`. The order of
//! operations is the one gfortran 12.2 `-O3 -ffp-contract=off` compiled on aarch64, read from the
//! optimised GIMPLE of `w3sin3` (`rust/port_report/W3SIN3.md`): every `**2`, `**3`, `**4` and `**5`
//! is a product, `COS` and `SIN` of `USDIR` are one `sincosf`, `ALOG` and `EXP` call `logf` and
//! `expf`, `COSWIND**SSINTHP` calls `powf`, and nothing fuses.
//!
//! Built for `STAB0`, so `ISTAB` is 3 and `DSTAB(3,:)` is `D`; the `STAB3` average over two
//! friction velocities is not in this build. `Z0VISC` (`:549`), `TAUX`, `TAUY`, `EC2`, `ZZWND`,
//! `XFR`, `DDEN`, `TH` and `ZZ0RAT` are dead or unused here.
//!
//! `MAX` and `MIN` are `fmaxnm` and `fminnm` at `-O3` (`MAX_EXPR`), which `f32::max` and `f32::min`
//! are too: a NaN operand loses.
#![allow(clippy::assign_op_pattern, clippy::manual_clamp)] // `x = x + y` keeps the Fortran line.

use crate::constants::{GRAV, KAPPA, TPI};
use crate::real::Real;
use crate::sin4::{IALPHA, IUSTAR};
use crate::util::{pow2, pow3, pow4, pow5};
use ww3_libm::{expf, logf, powf, sincosf, sqrtf};

/// Scalars `W3SIN3` reads from `W3GDATMD` and `W3SRC3MD`. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin3Params {
    pub nk: i32,
    pub nth: i32,
    pub nspec: i32,
    pub bbeta: Real,
    pub aalpha: Real,
    pub zzalp: Real,
    pub ssinthp: Real,
    pub dth: Real,
    /// `DELUST` and `DELALP` of `W3SRC3MD`, set by `INSIN3`.
    pub delust: Real,
    pub delalp: Real,
    /// `SSWELLF(1)`, the only element `W3SIN3` reads.
    pub sswellf1: Real,
}

/// The scalars of one `W3SIN3` call. `U`, `AS`, `CD`, `ICE`, `IX` and `IY` are unused in the source.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin3Point {
    pub ustar: Real,
    pub drat: Real,
    pub usdir: Real,
    pub z0: Real,
}

/// The scalars one `W3SIN3` call returns.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin3Out {
    pub tauwx: Real,
    pub tauwy: Real,
    pub tauwnx: Real,
    pub tauwny: Real,
    /// Which branches ran, for reading what a capture reached; the arithmetic never reads it. Bit 0:
    /// the tail limiter scaled `TAUWX` and `TAUWY` (`:702`). 1 and 2: `IND` clamped to 1 or to
    /// `IUSTAR-1` (`:683`). 3 and 4: `J` clamped to 1 or to `IALPHA-1` (`:687`). 5: some band took
    /// the `EXP(ZLOG)` growth (`:611`). 6: some wind-side band had `ZLOG >= 0` (`:616`). 7: some
    /// `D` was negative (`:627`). 8: some band had `COSWIND <= 0.01` (`:620`).
    pub reach: u32,
}

/// `TAUHFT(ind, j)`, with bounds `0:IUSTAR`, `0:IALPHA` (`w3src3md.F90:99`).
#[inline]
fn tauhft_at(ind: i32, j: i32) -> usize {
    (ind + j * (IUSTAR + 1)) as usize
}

/// `W3SIN3`, `w3src3md.F90:384-717`, on one spectrum. `sig` is `SIG(0:NK+1)`, `tauhft` is
/// `TAUHFT(0:IUSTAR,0:IALPHA)`. `a`, `k`, `s`, `d`, `llws` start at `spec_off`, `cg` at `cg_off`
/// and the result goes to `out[out_off]`. `llws` is `LLWS`, 0 or 1.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sin3(
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
    pt: &Sin3Point,
    s: &mut [Real],
    d: &mut [Real],
    llws: &mut [u32],
    out: &mut [Sin3Out],
    spec_off: usize,
    cg_off: usize,
    out_off: usize,
) {
    let (nk, nth, nspec) = (p.nk, p.nth, p.nspec);
    let (ustar, drat, usdir, z0) = (pt.ustar, pt.drat, pt.usdir, pt.z0);

    macro_rules! ix {
        ($o:expr, $i:expr) => {
            $o + (($i) - 1) as usize
        };
    }
    macro_rules! a {
        ($is:expr) => {
            a[ix!(spec_off, $is)]
        };
    }
    macro_rules! k {
        ($is:expr) => {
            k[ix!(spec_off, $is)]
        };
    }
    macro_rules! cg {
        ($ik:expr) => {
            cg[ix!(cg_off, $ik)]
        };
    }
    macro_rules! d {
        ($is:expr) => {
            d[ix!(spec_off, $is)]
        };
    }

    // 2.  Diagonal (`ISTAB = 3`, `UST = USTAR`)
    let ust = ustar;
    let (sinu, cosu) = sincosf(usdir);

    // STRESSSTAB(3,1:2) and STRESSSTABN(3,1:2)
    let (mut st1, mut st2, mut stn1, mut stn2) = (0., 0., 0., 0.);
    let mut reach = 0u32;
    let const0 = p.bbeta * drat / pow2(KAPPA);
    // SSWELLF(1) is IDAMPING in ECMWAM
    let const3 = p.sswellf1 * 2. * KAPPA * drat;

    each!(ik, 1, nk, {
        let is1 = 1 + (ik - 1) * nth;
        let cm = k!(is1) / sig2[ix!(0, is1)];
        // the inverse wave age
        let ucn = ust * cm + p.zzalp;
        let const2 = dden2[ix!(0, is1)] / cg!(ik) * GRAV / (sig[ik as usize] / k!(is1) * drat);
        let cnst = sig2[ix!(0, is1)] * const0;
        let zcn = logf(k!(is1) * z0);

        each!(ith, 1, nth, {
            let is = ith + (ik - 1) * nth;
            let coswind = ecos[ix!(0, is)] * cosu + esin[ix!(0, is)] * sinu;
            if coswind > 0.01 {
                let x = coswind * ucn;
                let zarg = KAPPA / x;
                let zlog = zcn + zarg;
                let zbeta = const3 * (coswind + KAPPA / (zcn * ust * cm)) * pow2(ucn);
                if zlog < 0. {
                    reach |= 1 << 5;
                    d!(is) = cnst * expf(zlog) * pow4(zlog) * pow2(ucn) * powf(coswind, p.ssinthp);
                } else {
                    reach |= 1 << 6;
                    d!(is) = zbeta;
                }
                llws[ix!(spec_off, is)] = 1;
            } else {
                reach |= 1 << 8;
                let zbeta = const3 * (coswind + KAPPA / (zcn * ust * cm)) * pow2(ucn);
                d!(is) = zbeta;
                llws[ix!(spec_off, is)] = 0;
            }

            let temp2 = const2 * d!(is) * a!(is);
            if d!(is) < 0. {
                reach |= 1 << 7;
                stn1 = stn1 + temp2 * ecos[ix!(0, is)];
                stn2 = stn2 + temp2 * esin[ix!(0, is)];
            }
            st1 = st1 + temp2 * ecos[ix!(0, is)];
            st2 = st2 + temp2 * esin[ix!(0, is)];
        });
    });

    let (xstress, ystress) = (st1, st2);
    let (tauwnx, tauwny) = (stn1, stn2);
    each!(is, 1, nspec, {
        s[ix!(spec_off, is)] = d!(is) * a!(is);
    });

    // Computes the high-frequency contribution
    let signk = sig[nk as usize];
    let const0 = p.dth * pow5(signk) / (pow2(GRAV) * TPI) * TPI * signk / cg!(nk);
    let mut temp = 0.;
    each!(ith, 1, nth, {
        let is = ith + (nk - 1) * nth;
        let coswind = ecos[ix!(0, is)] * cosu + esin[ix!(0, is)] * sinu;
        temp = temp + a!(is) * pow3(coswind.max(0.));
    });

    // finds the values in the tabulated stress TAUHFT
    let xi = ust / p.delust;
    let ind = (IUSTAR - 1).min(xi as i32).max(1);
    reach |= (((xi as i32) < 1) as u32) << 1 | (((xi as i32) > IUSTAR - 1) as u32) << 2;
    let deli1 = (xi - ind as Real).min(1.).max(0.);
    let deli2 = 1. - deli1;
    let xj = ((GRAV * z0 / pow2(ust.max(0.00001)) - p.aalpha) / p.delalp).max(0.);
    let j = (IALPHA - 1).min(xj as i32).max(1);
    reach |= (((xj as i32) < 1) as u32) << 3 | (((xj as i32) > IALPHA - 1) as u32) << 4;
    let delj1 = (xj - j as Real).min(1.).max(0.);
    let delj2 = 1. - delj1;
    let tau1 = (tauhft[tauhft_at(ind, j)] * deli2 + tauhft[tauhft_at(ind + 1, j)] * deli1) * delj2
        + (tauhft[tauhft_at(ind, j + 1)] * deli2 + tauhft[tauhft_at(ind + 1, j + 1)] * deli1) * delj1;
    let tauhf = const0 * temp * pow2(ust) * tau1;
    let mut tauwx = xstress + tauhf * cosu;
    let mut tauwy = ystress + tauhf * sinu;

    // Reduces tail effect to make sure that wave-supported stress is less than total stress
    let eps1: Real = 0.00001;
    let eps2: Real = 0.000001;
    let tauw = sqrtf(pow2(tauwx) + pow2(tauwy));
    let ust2 = pow2(ustar.max(eps2));
    let tauwb = tauw.min((ust2 - eps1).max(pow2(eps2)));
    if tauwb < tauw {
        tauwx = tauwx * tauwb / tauw;
        tauwy = tauwy * tauwb / tauw;
        reach |= 1;
    }

    out[out_off] = Sin3Out { tauwx, tauwy, tauwnx, tauwny, reach };
}
