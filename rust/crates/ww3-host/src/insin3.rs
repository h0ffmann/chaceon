// SPDX-License-Identifier: MPL-2.0
//! ST3's tables, built once on the host: `INSIN3`, `TABU_STRESS` and `TABU_TAUHF`
//! (`model/src/w3src3md.F90:726-1116`) at metocean/WW3 `92e2cf66`, built for `ST3`, `STAB0` and `FLX0`.
//!
//! Same arithmetic as ST4's tables (`insin4.rs`) except `OMEGAC = TPI*FRMAX`, where `INSIN3` passes
//! `FRMAX = SIG(NK)*TPIINV`: two roundings, which `-O3` cannot fold away. GCC turns `x**2.` into
//! `x*x` and keeps other real exponents as `powf`.

#![allow(clippy::approx_constant, clippy::excessive_precision, clippy::assign_op_pattern)]

use ww3_kernels::constants::{GRAV, KAPPA, TPI, TPIINV};
use ww3_kernels::real::Real;
use ww3_libm::{expf, logf, powf, sqrtf};

/// `w3src3md.F90:95-101`.
pub const ITAUMAX: i32 = 200;
pub const JUMAX: i32 = 200;
pub const IUSTAR: i32 = 100;
pub const IALPHA: i32 = 200;
const UMAX: Real = 50.;
const TAUWMAX: Real = 2.2361;
const JTOT: i32 = 250;

fn p2(x: Real) -> Real {
    x * x
}

fn p4(x: Real) -> Real {
    let x2 = x * x;
    x2 * x2
}

/// What `INSIN3` reads from `W3GDATMD`: `ZZWND AALPHA BBETA ZZALP ZZ0MAX` and `SIG(NK)`.
pub struct Insin3Input {
    pub sig_nk: Real,
    pub zzwnd: Real,
    pub aalpha: Real,
    pub bbeta: Real,
    pub zzalp: Real,
    pub zz0max: Real,
}

/// The module variables `INSIN3` sets: `TAUT(0:ITAUMAX,0:JUMAX)`, `TAUHFT(0:IUSTAR,0:IALPHA)`, column-major.
pub struct Insin3Output {
    pub deltauw: Real,
    pub delu: Real,
    pub taut: Vec<Real>,
    pub delust: Real,
    pub delalp: Real,
    pub tauhft: Vec<Real>,
}

/// `w3src3md.F90:726-813`.
pub fn insin3(p: &Insin3Input) -> Insin3Output {
    let (deltauw, delu, taut) = tabu_stress(p.zzwnd, p.aalpha, p.zz0max);
    let (delust, delalp, tauhft) = tabu_tauhf(p.sig_nk * TPIINV, p.aalpha, p.bbeta, p.zzalp, p.zz0max);
    Insin3Output { deltauw, delu, taut, delust, delalp, tauhft }
}

/// `w3src3md.F90:822-943`. `(DELTAUW, DELU, TAUT(0:ITAUMAX,0:JUMAX))`.
pub fn tabu_stress(zzwnd: Real, aalpha: Real, zz0max: Real) -> (Real, Real, Vec<Real>) {
    const NITER: i32 = 10;
    const XM: Real = 0.50;
    const EPS1: Real = 0.00001;
    let delu = UMAX / JUMAX as Real;
    let deltauw = TAUWMAX / ITAUMAX as Real;
    let n = (ITAUMAX + 1) as usize;
    let mut taut = vec![0.; n * (JUMAX + 1) as usize];
    for i in 0..=ITAUMAX {
        let ztauw = p2(i as Real * deltauw);
        for j in 0..=JUMAX {
            let utop = j as Real * delu;
            let cdrag: Real = 0.0012875;
            let wcd = sqrtf(cdrag);
            let ustold = utop * wcd;
            let mut tauold = p2(ustold).max(ztauw + EPS1);
            for _ in 0..NITER {
                let x = ztauw / tauold;
                let ust = sqrtf(tauold);
                let mut zz00 = aalpha * tauold / GRAV;
                if zz0max != 0. {
                    zz00 = zz00.min(zz0max);
                }
                let zz0 = zz00 / powf(1. - x, XM);
                let l = logf(zzwnd / zz0);
                let f = ust - KAPPA * utop / l;
                let delf = 1. - KAPPA * utop / p2(l) * 2. / ust * (1. - (XM + 1.) * x) / (1. - x);
                let ust = ust - f / delf;
                tauold = p2(ust).max(ztauw + EPS1);
            }
            taut[(i + j * (ITAUMAX + 1)) as usize] = sqrtf(tauold);
        }
    }
    for i in 0..=ITAUMAX {
        taut[i as usize] = 0.0;
    }
    (deltauw, delu, taut)
}

/// `w3src3md.F90:958-1116`. `(DELUST, DELALP, TAUHFT(0:IUSTAR,0:IALPHA))`; `frmax` is in hertz.
pub fn tabu_tauhf(frmax: Real, aalpha: Real, bbeta: Real, zzalp: Real, zz0max: Real) -> (Real, Real, Vec<Real>) {
    let ustarm: Real = 5.;
    let alpham = 20. * aalpha;
    let delust = ustarm / IUSTAR as Real;
    let delalp = alpham / IALPHA as Real;
    let const1 = bbeta / p2(KAPPA);
    let omegac = TPI * frmax;
    let mut w = vec![1.; JTOT as usize];
    w[0] = 0.5;
    w[(JTOT - 1) as usize] = 0.5;
    let x0: Real = 0.05;
    let mut t = vec![0.; ((IUSTAR + 1) * (IALPHA + 1)) as usize];
    for l in 0..=IALPHA {
        for k in 0..=IUSTAR {
            let ust = (k as Real * delust).max(0.000001);
            let mut zz00 = p2(ust) * aalpha / GRAV;
            if zz0max != 0. {
                zz00 = zz00.min(zz0max);
            }
            let zz0 = zz00 * (1. + l as Real * delalp / aalpha);
            let omegacc = omegac.max(x0 * GRAV / ust);
            let yc = omegacc * sqrtf(zz0 / GRAV);
            let dely = ((1. - yc) / JTOT as Real).max(0.);
            let mut sum = 0.;
            for j in 1..=JTOT {
                let y = yc + (j - 1) as Real * dely;
                let omega = y * sqrtf(GRAV / zz0);
                let cm = GRAV / omega;
                let zx = ust / cm + zzalp;
                let zarg = (KAPPA / zx).min(20.);
                let zmu = (GRAV * zz0 / p2(cm) * expf(zarg)).min(1.);
                let zlog = logf(zmu).min(0.);
                let zbeta = const1 * zmu * p4(zlog);
                sum = sum + w[(j - 1) as usize] * zbeta / y * dely;
            }
            t[(k + l * (IUSTAR + 1)) as usize] = sum;
        }
    }
    (delust, delalp, t)
}
