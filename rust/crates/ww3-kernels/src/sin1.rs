// SPDX-License-Identifier: MPL-2.0
//! `W3SIN1`, WAM cycle 3 wind input (switch ST1: Snyder et al. 1981, Komen et al. 1984). Translated
//! from `model/src/w3src1md.F90:255-417` at metocean/WW3 `92e2cf66`. The `W3_T*` and `W3_S` test
//! output is not in the build. `COS(USDIR)` and `SIN(USDIR)` share one `sincosf` call, as in the
//! other source terms (`rust/port_report/W3SIN1.md`).
#![allow(clippy::manual_clamp)]

use crate::real::Real;
use ww3_libm::sincosf;

/// Scalars of `W3GDATMD` that `W3SIN1` reads. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin1Params {
    pub nk: i32,
    pub nth: i32,
    pub nspec: i32,
    /// `SINC1 = 28. * CINP * DAIR / DWAT` (`w3gridmd.F90:1744`).
    pub sinc1: Real,
}

/// The scalars of one `W3SIN1` call.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin1Point {
    pub ustar: Real,
    pub usdir: Real,
}

/// `W3SIN1` on one spectrum. `sig2` is `SIG2(NSPEC)`, `ecos` and `esin` are `(NSPEC+NTH)` and read
/// at `IS` (`:384-386`). `a`, `k`, `s` and `d` start at `spec_off`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sin1(
    p: &Sin1Params,
    sig2: &[Real],
    ecos: &[Real],
    esin: &[Real],
    a: &[Real],
    k: &[Real],
    pt: &Sin1Point,
    s: &mut [Real],
    d: &mut [Real],
    spec_off: usize,
) {
    // 1.  Preparations
    let (sinu, cosu) = sincosf(pt.usdir);

    // 2.  Diagonal. `0.035714` is a REAL literal; the product is `(SINC1*SIG2) * MAX(...)`.
    let mut is = 1;
    while is <= p.nspec {
        let i = (is - 1) as usize;
        let o = spec_off + i;
        d[o] = p.sinc1 * sig2[i] * (pt.ustar * (ecos[i] * cosu + esin[i] * sinu) * k[o] / sig2[i] - 0.035714).max(0.);
        is += 1;
    }

    // S = D * A
    let mut is = 1;
    while is <= p.nspec {
        let o = spec_off + (is - 1) as usize;
        s[o] = d[o] * a[o];
        is += 1;
    }
}
