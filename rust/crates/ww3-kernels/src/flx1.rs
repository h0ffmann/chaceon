// SPDX-License-Identifier: MPL-2.0
//! `W3FLX1`, the wind stress of Wu (1980). Translated from `model/src/w3flx1md.F90:178-188` at
//! metocean/WW3 `92e2cf66`. Every variable is `REAL`, so everything is `f32`; the literals have no
//! `D` and stay `f32` (`docs/FLOAT_REPRESENTATION_202610.md`). Nothing fuses (`-ffp-contract=off`).
//! The order of operations is the Fortran's; `SQRT(CD)` appears twice in the source and may be
//! once in the compiled code, which changes no bit.

use crate::real::Real;
use ww3_libm::{expf, sqrtf};

/// One call's inputs: `ZWND`, `U10`, `U10D` (`w3flx1md.F90:132`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Flx1In {
    pub zwnd: Real,
    pub u10: Real,
    pub u10d: Real,
}

/// `UST`, `USTD`, `Z0` and `CD` (`REAL, INTENT(OUT)`), and the test of `w3flx1md.F90:178`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Flx1Out {
    pub ust: Real,
    pub ustd: Real,
    pub z0: Real,
    pub cd: Real,
    /// 1 where the Fortran writes an error and calls `EXTCDE (1)`: `ABS(ZWND-10.) > 0.01`. A kernel
    /// cannot stop the process, so it raises the flag and still computes; the host decides.
    pub err: u32,
}

/// `W3FLX1`, `w3flx1md.F90:178-188`.
#[inline]
pub fn w3flx1(i: &Flx1In) -> Flx1Out {
    let err = ((i.zwnd - 10.).abs() > 0.01) as u32;
    let cd = 0.001 * (0.8 + 0.065 * i.u10);
    // Fortran reads `-0.4 / SQRT(CD)` as `-(0.4 / SQRT(CD))`, an `fneg` after the division
    // (`fdiv` then `fneg` in `__w3flx1md_MOD_w3flx1`). For a negative `CD` the quotient is a NaN
    // and the `fneg` flips its sign, which the result then carries. LLVM may turn `-(a / b)` into
    // `(-a) / b`, which keeps the NaN's sign. The host build hides the quotient from LLVM with `black_box`;
    // rust-gpu ignores it (it warns), so the sign is also flipped as an integer, which `spirv-opt` does not fold.
    let q = core::hint::black_box(0.4 / sqrtf(cd));
    let z0 = i.zwnd * expf(f32::from_bits(q.to_bits() ^ 0x8000_0000));
    let ust = i.u10 * sqrtf(cd);
    Flx1Out { ust, ustd: i.u10d, z0, cd, err }
}
