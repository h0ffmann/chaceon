// SPDX-License-Identifier: MPL-2.0
//! `W3SIC1`, ice damping with one uniform exponential decay rate, `S = D*A` with
//! `D = -2*Cg*k_i`. Translated from `model/src/w3sic1md.F90:93-369` at metocean/WW3 `92e2cf66`.
//! The arithmetic is one sum, one product and a negation per band, and one product per spectral
//! bin; there is no library call and nothing for gfortran to fuse or reassociate. The order is the
//! one gfortran 12.2 `-O3 -ffp-contract=off` compiled on aarch64, read from
//! `__w3sic1md_MOD_w3sic1` in the reference's `ww3_shel` (`rust/port_report/W3SIC1.md`): `-2.*CG*WN_I`
//! is `-((CG+CG)*WN_I)` (`fadd`, `fmul`, `fneg`). The value is the same as `(-2*CG)*WN_I`; only a
//! NaN differs, in its sign bit, which the `fneg` flips.

use crate::real::Real;

/// `fneg`, written as a sign-bit flip. gfortran's `fneg` flips the sign of a NaN. A float negation
/// is value-exact when it moves into a neighbouring product (`-(a*b)` is `(-a)*b`), and both
/// spirv-opt and NVPTX do that move, which keeps a NaN's sign bit and so differs from the
/// reference on a NaN `ICEP1`. A bit flip is not a float operation, so nothing folds it.
#[inline]
fn neg(x: Real) -> Real {
    Real::from_bits(x.to_bits() ^ 0x8000_0000)
}

/// The scalars of `W3GDATMD` that `W3SIC1` reads. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sic1Params {
    pub nk: i32,
    pub nth: i32,
}

/// What `W3SIC1` returns besides `S` and `D`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sic1Out {
    /// Which branch ran, for reading what a capture reached: 0 no ice (`ICEP1(IX,IY) == 0`,
    /// `w3sic1md.F90:309-311`), 1 ice (`:313-337`).
    pub branch: i32,
}

/// `W3SIC1` on one spectrum. `icecoef1` is `ICEP1(IX,IY)`, which the caller reads from the input
/// field; `W3SIC1`'s own check that ice parameter 1 was selected (`INFLAGS2(-7)`, `:299-302`, an
/// `EXTCDE`) belongs to the host. `a`, `s` and `d` start at `o`, `cg` at `k_off`. `mapwn` is
/// `MAPWN(1:NSPEC)`, the 1-based band of each bin.
///
/// `D1D(NK)` and `WN_I(NK)` are not stored: `D1D(MAPWN(IKTH))` is recomputed per bin, the same two
/// products on the same operands, because a kernel may not allocate.
#[inline]
pub fn w3sic1(
    p: &Sic1Params,
    mapwn: &[i32],
    a: &[Real],
    cg: &[Real],
    icecoef1: Real,
    s: &mut [Real],
    d: &mut [Real],
    o: usize,
    k_off: usize,
) -> Sic1Out {
    let nspec = (p.nk * p.nth) as usize;
    let mut branch = 0;
    let mut ikth = 0;
    while ikth < nspec {
        d[o + ikth] = 0.;
        ikth += 1;
    }
    // `IF ( ICECOEF1==0. )` is false for a NaN, and so is Rust's `!= 0.` true: both take the ice branch.
    if icecoef1 != 0. {
        branch = 1;
        let mut ikth = 0;
        while ikth < nspec {
            let ik = (mapwn[ikth] - 1) as usize;
            let c = cg[k_off + ik];
            d[o + ikth] = neg((c + c) * icecoef1);
            ikth += 1;
        }
    }
    let mut ikth = 0;
    while ikth < nspec {
        s[o + ikth] = d[o + ikth] * a[o + ikth];
        ikth += 1;
    }
    Sic1Out { branch }
}
