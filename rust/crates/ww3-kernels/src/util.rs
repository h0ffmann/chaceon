// SPDX-License-Identifier: MPL-2.0
//! What several kernels share: Fortran's intrinsics and `-O3`'s power trees.

use crate::real::Real;

/// Inclusive loop, as a Fortran DO with step 1. A macro, because closures and iterators leave
/// pointers in `SPIR-V` that logical addressing refuses.
macro_rules! each {
    ($i:ident, $lo:expr, $hi:expr, $body:block) => {
        let mut $i = $lo;
        let hi_ = $hi;
        while $i <= hi_ {
            $body
            $i += 1;
        }
    };
}

/// gfortran's `NINT`: nearest, ties away from zero.
#[inline]
pub fn nint(x: Real) -> i32 {
    let t = x as i32;
    let frac = x - t as Real;
    if frac >= 0.5 {
        t + 1
    } else if frac <= -0.5 {
        t - 1
    } else {
        t
    }
}

#[inline]
pub fn floor(x: Real) -> i32 {
    let t = x as i32;
    if (t as Real) > x { t - 1 } else { t }
}

/// Fortran `SIGN(A, B)`.
#[inline]
pub fn sign(a: Real, b: Real) -> Real {
    f32::from_bits((a.to_bits() & 0x7fffffff) | (b.to_bits() & 0x80000000))
}

#[inline]
pub fn pow2(x: Real) -> Real {
    x * x
}

#[inline]
pub fn pow3(x: Real) -> Real {
    let x2 = x * x;
    x2 * x
}

#[inline]
pub fn pow4(x: Real) -> Real {
    let x2 = x * x;
    x2 * x2
}

/// `x**5` as GCC's power tree expands it.
#[inline]
pub fn pow5(x: Real) -> Real {
    let x2 = x * x;
    let x3 = x2 * x;
    x3 * x2
}
