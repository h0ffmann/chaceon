// SPDX-License-Identifier: MPL-2.0
//! `W3SDS6`, ST6's whitecapping dissipation after Babanin et al. (2010) and Rogers et al. (2012).
//! Translated from `model/src/w3src6md.F90:546-741` at metocean/WW3 `92e2cf66`
//! (`rust/port_report/W3SDS6.md`). The `W3_T6` and `W3_S` blocks are left out: all output.
//!
//! Every variable is `REAL`, so everything is `f32`. Array statements of the Fortran become loops
//! in the order gfortran evaluates them; each `SUM` starts at 0 and adds left to right (no
//! `-ffast-math`, so no reassociation). `NEXDENS**SDS6P1` has a run-time integer exponent and is
//! libgcc's `__powisf2`.

use crate::constants::TPI;
use crate::real::Real;

/// Scalars of `W3GDATMD` that `W3SDS6` reads. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sds6Params {
    pub nk: i32,
    pub nth: i32,
    /// `SDS6ET`: 1 for `.TRUE.`.
    pub sds6et: i32,
    pub sds6p1: i32,
    pub sds6p2: i32,
    pub sds6a1: Real,
    pub sds6a2: Real,
    pub dth: Real,
    pub xfr: Real,
}

/// Length of the scratch range of one call: `EDENS`, `ETDENS`, `NEXDENS`, `ADF`, each `NK`.
#[inline]
pub fn work_len(p: &Sds6Params) -> usize {
    4 * p.nk as usize
}

/// `W3SDS6` on one spectrum. `a`, `s` and `d` start at `o`, `cg` and `wn` (`CG(NK)`, `WN(NK)`) at
/// `k_off`, the scratch `w` at `w_off`. `sig2` is `SIG2(NSPEC)` and `dsii` `DSII(NK)`, one copy for
/// all points. `D` is written in full by the `DO ITH` loop, not zeroed first.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sds6(
    p: &Sds6Params,
    sig2: &[Real],
    dsii: &[Real],
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
    s: &mut [Real],
    d: &mut [Real],
    w: &mut [Real],
    o: usize,
    k_off: usize,
    w_off: usize,
) {
    let (nk, nth) = (p.nk as usize, p.nth as usize);
    let nspec = nk * nth;
    let (edens, etdens, nex, adf) = (w_off, w_off + nk, w_off + 2 * nk, w_off + 3 * nk);
    // `BNT = 0.035**2`, folded.
    let bnt: Real = 0.035 * 0.035;
    // `IKN = IRANGE(1,NSPEC,NTH)`: `IKN(IK) = 1 + (IK-1)*NTH`, `NK` entries.
    // 1) `ETDENS = (TPI*BNT) / (ANAR * CG * WN**3)`, `EDENS = SUM(..)*TPI*SIG2(IKN)*DTH/CG`.
    let mut ik = 1;
    while ik <= nk {
        let c = cg[k_off + ik - 1];
        let k = wn[k_off + ik - 1];
        w[etdens + ik - 1] = (TPI * bnt) / ((1.0 * c) * ((k * k) * k));
        let mut sum: Real = 0.0;
        let mut ith = 1;
        while ith <= nth {
            sum += a[o + (ith - 1) + (ik - 1) * nth];
            ith += 1;
        }
        w[edens + ik - 1] = (((sum * TPI) * sig2[(ik - 1) * nth]) * p.dth) / c;
        w[nex + ik - 1] = 0.0;
        ik += 1;
    }
    // `EXDENS = MAX(0.0,EDENS-ETDENS)` and the normalisation (`SDS6ET`). `EXDENS` is not stored: it
    // is read once, at the same `IK`.
    if p.sds6et != 0 {
        let mut ik = 1;
        while ik <= nk {
            let ex = max0(w[edens + ik - 1] - w[etdens + ik - 1]);
            w[nex + ik - 1] = ex / w[etdens + ik - 1];
            ik += 1;
        }
    } else {
        // `EDENSMAX = MAXVAL(EDENS)*1E-5`
        let edensmax = maxval(w, edens, nk) * 1.0E-5;
        let mut all = true;
        let mut ik = 1;
        while ik <= nk {
            all &= w[edens + ik - 1] > edensmax;
            ik += 1;
        }
        let mut ik = 1;
        while ik <= nk {
            // The `ALL` branch normalises every `IK`, the `ELSE` branch only those above `EDENSMAX`.
            if all || w[edens + ik - 1] > edensmax {
                let ex = max0(w[edens + ik - 1] - w[etdens + ik - 1]);
                w[nex + ik - 1] = ex / w[edens + ik - 1];
            }
            ik += 1;
        }
    }
    // 3) `ADF = ANAR * (NEXDENS**SDS6P2)`.
    let mut ik = 1;
    while ik <= nk {
        w[adf + ik - 1] = 1.0 * powi(w[nex + ik - 1], p.sds6p2);
        ik += 1;
    }
    let xfac = (1.0 - 1.0 / p.xfr) / (p.xfr - 1.0 / p.xfr);
    let mut ik = 1;
    while ik <= nk {
        // 2) `T1 = SDS6A1 * ANAR * FREQ * (NEXDENS**SDS6P1)`, with `FREQ = SIG2(IKN)/TPI`.
        let freq = sig2[(ik - 1) * nth] / TPI;
        let t1 = ((p.sds6a1 * 1.0) * freq) * powi(w[nex + ik - 1], p.sds6p1);
        // `DFII = DSII/TPI` again for every `IK`; only `DFII(IK)` takes `XFAC`, and only inside.
        let mut sum: Real = 0.0;
        let mut j = 1;
        while j <= ik {
            let mut dfii = dsii[j - 1] / TPI;
            if j == ik && ik > 1 && ik < nk {
                dfii *= xfac;
            }
            sum += w[adf + j - 1] * dfii;
            j += 1;
        }
        let t2 = p.sds6a2 * sum;
        // 4) `T12 = -1.0 * (MAX(0.0,T1)+MAX(0.0,T2))`: gfortran folds `-1.0 * x` to a negation.
        let t12 = neg(max0(t1) + max0(t2));
        let mut ith = 1;
        while ith <= nth {
            d[o + (ith - 1) + (ik - 1) * nth] = t12;
            ith += 1;
        }
        ik += 1;
    }
    // `S = D * A`.
    let mut is = 0;
    while is < nspec {
        s[o + is] = d[o + is] * a[o + is];
        is += 1;
    }
}

/// libgcc's `__powisf2`, for a run-time integer exponent.
#[inline]
fn powi(mut x: Real, n: i32) -> Real {
    let mut m = n.unsigned_abs();
    let mut y = if m & 1 != 0 { x } else { 1. };
    loop {
        m >>= 1;
        if m == 0 {
            break;
        }
        x *= x;
        if m & 1 != 0 {
            y *= x;
        }
    }
    if n < 0 { 1. / y } else { y }
}

/// `MAX(0.0, x)` as gfortran compiled it: `fmaxnm` with a zero (`fmaxnm v0.4s, v0.4s, v2.4s` and its
/// scalar tails in `__w3src6md_MOD_w3sds6`). A NaN gives `+0`, and so does `-0`, because `fmaxnm`
/// ranks `+0` above `-0`. Only a value above zero is returned as it is.
#[inline]
fn max0(x: Real) -> Real {
    if x > 0.0 { x } else { 0.0 }
}

/// `MAXVAL` of `n` values at `w[off..]`.
#[inline]
fn maxval(w: &[Real], off: usize, n: usize) -> Real {
    let mut m = w[off];
    let mut i = 1;
    while i < n {
        let v = w[off + i];
        if v > m || m != m {
            m = v;
        }
        i += 1;
    }
    m
}

/// A negation as an integer sign flip, so neither LLVM nor `spirv-opt` moves it into an operand.
#[inline]
fn neg(x: Real) -> Real {
    f32::from_bits(x.to_bits() ^ 0x8000_0000)
}
