// SPDX-License-Identifier: MPL-2.0
//! `W3FLX3`, the stress of Tolman and Chalikov (1996) with a cap on the drag coefficient. Translated
//! from `model/src/w3flx3md.F90:95-249` at metocean/WW3 `92e2cf66` (the arithmetic is `:201-240`).
//! Every variable is `REAL`, so everything is `f32`, and no literal has a `D`
//! (`docs/FLOAT_REPRESENTATION_202610.md`). The order of operations is the one gfortran 12.2
//! `-O3 -ffp-contract=off` compiled on aarch64, read from the optimised GIMPLE of `w3flx3`
//! (`rust/port_report/W3FLX3.md`): `**1.5` and `**1.23` stay `powf` calls, `UNZ**2` is a product, `SQRT(CD)`
//! is computed once, `LOG` and `EXP` call `logf` and `expf`, `TANH` calls `tanhf`, and nothing fuses.
//!
//! `MAX` and `MIN` are `fmaxnm` and `fminnm` (`MAX_EXPR`, `MIN_EXPR`): a NaN operand loses, and `+0`
//! beats `-0` in a `MAX`. The inlined `WAVNU1` reads `DSIE`, `N1MAX` and `EWN1` from `W3DISPMD`, which
//! the host builds with `DISTAB` (`rust/crates/ww3-host/src/disp.rs`) and the capture records.
//! A negative `DEPTH` makes `I1` negative, and the Fortran reads `EWN1` below its first element
//! (undefined); `W3SRCE` never calls with a dry point, so that is not translated ⚠.
#![allow(clippy::assign_op_pattern, clippy::manual_clamp)] // `x = x + y` keeps the Fortran line.

use crate::constants::{GRAV, TPI};
use crate::real::Real;
use ww3_libm::{expf, logf, powf, sqrtf, tanhf};

/// What `W3FLX3` reads from `W3GDATMD` (`NITTIN`, `CINXSI`, `CD_MAX`, `CAP_ID`, `w3flx3md.F90:164`)
/// and from `W3DISPMD` (`N1MAX`, `DSIE`, `:168`). The table `EWN1(0:121)` is a separate buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Flx3Params {
    pub nittin: i32,
    pub cap_id: i32,
    pub n1max: i32,
    pub cinxsi: Real,
    pub cd_max: Real,
    pub dsie: Real,
}

/// One call's inputs: `ZWIND`, `DEPTH`, `FP`, `U`, `UDIR` and `UST`, which is `INTENT(INOUT)`: its
/// value at entry is the first iteration's `MAX (UST,0.0001)` (`w3flx3md.F90:224`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Flx3In {
    pub zwind: Real,
    pub depth: Real,
    pub fp: Real,
    pub u: Real,
    pub udir: Real,
    pub ust: Real,
}

/// `UST`, `USTD`, `Z0` and `CD`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Flx3Out {
    pub ust: Real,
    pub ustd: Real,
    pub z0: Real,
    pub cd: Real,
}

/// gfortran's `MAX` on aarch64 is `fmaxnm`: the other operand when one is NaN, `+0` for `+0` against `-0`.
/// SPIR-V's `FMax` returned `-0` there on lavapipe, so the comparison is written out.
#[inline]
fn max_ww3(x: Real, y: Real) -> Real {
    if x != x {
        y
    } else if y != y || x > y {
        x
    } else if y > x {
        y
    } else {
        Real::from_bits(x.to_bits() & y.to_bits())
    }
}

/// `MIN` is `fminnm`: a NaN loses, and `-0` beats `+0`.
#[inline]
fn min_ww3(x: Real, y: Real) -> Real {
    if x != x {
        y
    } else if y != y || x < y {
        x
    } else if y < x {
        y
    } else {
        Real::from_bits(x.to_bits() | y.to_bits())
    }
}

/// `-0.4 / SQRT(CD)` is `-(0.4 / SQRT(CD))` in Fortran, an `fneg` after the `fdiv` (`_54 = -_53` in the
/// GIMPLE). For a negative `CD` the quotient is a NaN and the sign survives `expf`; LLVM and `spirv-opt`
/// would move the negation into the dividend and change that sign, so the quotient is held in `black_box`
/// for the host (rust-gpu ignores it) and the sign flipped as an integer for SPIR-V, as `flx1.rs` does.
#[inline]
fn neg_quot(sqrt_cd: Real) -> Real {
    let q = core::hint::black_box(0.4 / sqrt_cd);
    Real::from_bits(q.to_bits() ^ 0x8000_0000)
}

/// `W3FLX3`, `w3flx3md.F90:201-240`. `ewn1` holds `EWN1(0:NAR1D)`, 122 values, indexed from 0.
#[inline]
pub fn w3flx3(p: &Flx3Params, ewn1: &[Real], i: &Flx3In) -> Flx3Out {
    // 1. Peak phase velocity, inlined and reduced WAVNU1 (:201-214).
    let afp = TPI * max_ww3(i.fp, 0.001);
    let sqrth = sqrtf(i.depth);
    let six = afp * sqrth;
    let i1 = (six / p.dsie) as i32;
    let wnp = if i1 <= p.n1max {
        let r1 = six / p.dsie - i1 as Real;
        let k = i1 as usize;
        ((1. - r1) * ewn1[k] + r1 * ewn1[k + 1]) / i.depth
    } else {
        afp * afp / GRAV
    };
    let cp = afp / wnp;
    // 2. Iterative stress computation (:219-229). `UNZ**2` is `UNZ*UNZ`; `ZWIND*GRAV` does not depend on
    // the iteration. The loop's `Z0` is dead, the one after the cap overwrites it.
    let unz = max_ww3(0.01, i.u);
    let unz2 = unz * unz;
    let zg = i.zwind * GRAV;
    let mut ust = i.ust;
    let mut cd: Real = 0.;
    let mut itt = 0;
    while itt < p.nittin {
        let alpha = 0.57 / powf(cp / max_ww3(ust, 0.0001), 1.5);
        let rdch = max_ww3(0., logf(zg / ((sqrtf(alpha) * p.cinxsi) * unz2)));
        cd = 0.001 * (0.021 + 10.4 / (powf(rdch, 1.23) + 1.85));
        ust = sqrtf(cd) * unz;
        itt += 1;
    }
    // 3. Limit on the drag coefficient (:233-240).
    if p.cap_id == 0 {
        cd = min_ww3(p.cd_max, cd);
    } else {
        cd = p.cd_max * tanhf(cd / p.cd_max);
    }
    let sqrt_cd = sqrtf(cd);
    let ust = sqrt_cd * unz;
    let z0 = i.zwind * expf(neg_quot(sqrt_cd));
    Flx3Out { ust, ustd: i.udir, z0, cd }
}
