// SPDX-License-Identifier: MPL-2.0
//! `W3FLX2`, the stress of Tolman and Chalikov (1996) with the drag depending on the peak frequency.
//! Translated from `model/src/w3flx2md.F90:90-231` at metocean/WW3 `92e2cf66`: the inlined, reduced
//! `WAVNU1` (`:190-206`) and the iteration on `UST` (`:212-222`). Every variable is `REAL`, so
//! everything is `f32`, and the literals have no `D`. Nothing fuses (`-ffp-contract=off`).
//!
//! `MAX` is `fmaxnm` on arm64 (`MAX_EXPR`): a NaN argument gives the other, which `FP`, `UST` and the
//! logarithm can be on a poked input. `EWN1`, `DSIE` and `N1MAX` are `W3DISPMD`'s table (`DISTAB`),
//! `NITTIN` and `CINXSI` the grid's (`w3gridmd.F90:1628-1629`): they arrive as input, and `ww3-host`
//! rebuilds the table to prove it equals the reference's.

use crate::constants::{GRAV, TPI};
use crate::real::Real;
use ww3_libm::{expf, logf, powf, sqrtf};

/// What `W3FLX2` takes from `W3GDATMD` and `W3DISPMD` (`:157`, `:163`): the same for every call of a grid.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Flx2Params {
    /// `NITTIN`, the number of iterations.
    pub nittin: i32,
    /// `N1MAX`, the last interval of the table (`NAR1D-1`, 120).
    pub n1max: i32,
    /// `CINXSI`.
    pub cinxsi: Real,
    /// `DSIE`, the table's step in `SI`.
    pub dsie: Real,
}

/// Length of `EWN1(0:NAR1D)` (`w3dispmd.F90:75-77`).
pub const EWN1_LEN: usize = 122;

/// One call's inputs (`:170-172`); `ust` is `INTENT(INOUT)`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Flx2In {
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
pub struct Flx2Out {
    pub ust: Real,
    pub ustd: Real,
    pub z0: Real,
    pub cd: Real,
}

/// gfortran's `MAX(x, y)` on arm64: `fmaxnm`. A NaN gives the other operand and `+0` beats `-0`. Read
/// from `__w3flx2md_MOD_w3flx2` in the reference (see the port report).
#[inline]
fn max_ww3(x: Real, y: Real) -> Real {
    if x != x {
        y
    } else if y != y || x > y {
        x
    } else if y > x {
        y
    } else {
        // Equal: only the two zeros differ, and the larger is +0, the AND of the sign bits.
        Real::from_bits(x.to_bits() & y.to_bits())
    }
}

/// `W3FLX2`, `:190-222`. `ewn1` is `EWN1(0:NAR1D)`.
#[inline]
pub fn w3flx2(p: &Flx2Params, ewn1: &[Real], i: &Flx2In) -> Flx2Out {
    // 1. Peak phase velocity, the reduced WAVNU1 (:190-206).
    let afp = TPI * max_ww3(i.fp, 0.001);
    let sqrth = sqrtf(i.depth);
    let six = afp * sqrth;
    // `INT` of a NaN or an infinity: arm64's `fcvtzs` gives 0 and the saturated integer, as `as` does.
    let i1 = (six / p.dsie) as i32;
    // `IF (I1.LE.N1MAX)` has no lower bound: a negative `I1` would read outside `EWN1`, but `AFP` is
    // positive and `SQRTH` is not negative, so none reaches it (NaN gives 0).
    let wnp = if i1 <= p.n1max {
        let r1 = six / p.dsie - i1 as Real;
        let k = i1 as usize;
        ((1. - r1) * ewn1[k] + r1 * ewn1[k + 1]) / i.depth
    } else {
        afp * afp / GRAV
    };
    let cp = afp / wnp;
    // 2. Iterative stress computation (:212-222).
    let unz = max_ww3(0.01, i.u);
    let mut ust = i.ust;
    // `CD` and `Z0` are not assigned for `NITTIN < 1`; the Fortran leaves them undefined.
    let (mut z0, mut cd) = (0., 0.);
    let mut itt = 1;
    while itt <= p.nittin {
        let alpha = 0.57 / powf(cp / max_ww3(ust, 0.0001), 1.5);
        let rdch = max_ww3(0., logf((i.zwind * GRAV) / (p.cinxsi * sqrtf(alpha) * (unz * unz))));
        // `RDCH**1.23`: a real power, `powf`.
        cd = 0.001 * (0.021 + 10.4 / (powf(rdch, 1.23) + 1.85));
        let sq = sqrtf(cd);
        ust = sq * unz;
        // `-0.4/SQRT(CD)` is `-(0.4/SQRT(CD))`: a divide then a negate, as in `W3FLX1`. `CD` is
        // never negative here, so no NaN's sign is at stake, but the order costs nothing.
        let q = core::hint::black_box(0.4 / sq);
        z0 = i.zwind * expf(Real::from_bits(q.to_bits() ^ 0x8000_0000));
        itt += 1;
    }
    Flx2Out { ust, ustd: i.udir, z0, cd }
}
