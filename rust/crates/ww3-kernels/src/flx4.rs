// SPDX-License-Identifier: MPL-2.0
//! `W3FLX4`, the drag stress of Hwang (2011). Translated from `model/src/w3flx4md.F90:105-231` (the computation is `:199-220`) at
//! metocean/WW3 `92e2cf66`. Every variable is `REAL`, so everything is `f32`; the literals have no
//! `D` and stay `f32` (`docs/FLOAT_REPRESENTATION_202610.md`). Nothing fuses (`-ffp-contract=off`).
//! `FLX4A0` is the namelist's `CDFAC` (`w3gridmd.F90:1651-1655`), a module variable of `W3GDATMD`
//! that the kernel takes as an input.

use crate::real::Real;
use ww3_libm::{expf, sqrtf};

/// One call's inputs: `ZWND`, `U10`, `U10D` (`w3flx4md.F90:181`) and `FLX4A0`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Flx4In {
    pub zwnd: Real,
    pub u10: Real,
    pub u10d: Real,
    pub flx4a0: Real,
}

/// `UST`, `USTD`, `Z0` and `CD` (`REAL, INTENT(OUT)`), and the test of `w3flx4md.F90:199`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Flx4Out {
    pub ust: Real,
    pub ustd: Real,
    pub z0: Real,
    pub cd: Real,
    /// 1 where the Fortran writes an error and calls `EXTCDE (1)`: `ABS(ZWND-10.) > 0.01`. A kernel
    /// cannot stop the process, so it raises the flag and still computes; the host decides.
    pub err: u32,
}

/// `W3FLX4`, `w3flx4md.F90:199-220`.
#[inline]
pub fn w3flx4(i: &Flx4In) -> Flx4Out {
    let err = ((i.zwnd - 10.).abs() > 0.01) as u32;
    let (ust, cd);
    if i.u10 >= 50.33 {
        // The cap at extreme winds: `UST = 2.026*SQRT(FLX4A0)`, `CD = (UST/U10)**2`, a product `x*x`.
        ust = 2.026 * sqrtf(i.flx4a0);
        let r = ust / i.u10;
        cd = r * r;
    } else {
        // Left to right: `FLX4A0 * (...) * 1E-4`, and `8.058 + 0.967*U10 - 0.016*U10**2` as
        // `(8.058 + 0.967*U10) - 0.016*(U10*U10)`.
        let poly = (8.058 + 0.967 * i.u10) - 0.016 * (i.u10 * i.u10);
        cd = i.flx4a0 * poly * 1e-4;
        ust = i.u10 * sqrtf(cd);
    }
    // Fortran reads `-0.4 / SQRT(CD)` as `-(0.4 / SQRT(CD))`; see `flx1.rs` for the NaN sign this
    // keeps and for why the quotient is hidden from LLVM and the sign flipped as an integer.
    let q = core::hint::black_box(0.4 / sqrtf(cd));
    let z0 = i.zwnd * expf(f32::from_bits(q.to_bits() ^ 0x8000_0000));
    Flx4Out { ust, ustd: i.u10d, z0, cd, err }
}
