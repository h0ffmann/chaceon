// SPDX-License-Identifier: MPL-2.0
//! `W3SNL1`, the Discrete Interaction Approximation. Translated from `model/src/w3snl1md.F90:99-457`
//! at metocean/WW3 `92e2cf66`; the bits are the reference's (`rust/port_report/W3SNL1.md`).
//!
//! The Fortran's automatic arrays live in `w`, one slice for all of them, so a device can give every
//! point its own range. Slices are passed with offsets, not sub-slices, because SPIR-V's logical
//! addressing has no general pointer arithmetic.

use crate::constants::TPIINV;
use crate::real::Real;

/// Scalars of `W3GDATMD` and `W3ADATMD` that `W3SNL1` reads. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Snl1Params {
    pub nk: i32,
    pub nth: i32,
    pub nspec: i32,
    pub nfrhgh: i32,
    pub nfrchg: i32,
    pub nspecx: i32,
    pub nspecy: i32,
    pub kdcon: Real,
    pub kdmn: Real,
    pub snlc1: Real,
    pub snls1: Real,
    pub snls2: Real,
    pub snls3: Real,
    pub fachfe: Real,
    pub dal1: Real,
    pub dal2: Real,
    pub dal3: Real,
    pub awg: [Real; 8],
    pub swg: [Real; 8],
}

/// `IP11..IP14, IM11..IM14, IP21..IP24, IM21..IM24`, 16 per `ISP`, in `pm`.
pub const PM_COLS: usize = 16;
/// `IC11, IC12, IC21, IC22 .. IC81, IC82`, 16 per `ISP`, in `ic`.
pub const IC_COLS: usize = 16;

/// Length of the scratch range one call needs.
#[inline]
pub fn work_len(p: &Snl1Params) -> usize {
    let (nth, x, y) = (p.nth as usize, p.nspecx as usize, p.nspecy as usize);
    (y + nth) + 8 * (x + nth) + p.nspec as usize
}

/// Index of Fortran element `i` of an array with lower bound `1-NTH` that starts at `o`.
#[inline]
fn at(o: usize, nth: i32, i: i32) -> usize {
    o + (i + nth - 1) as usize
}

/// One spectrum. `sig` is `SIG(0:NK+1)`, indexed by `IFR`. `a`, `s`, `d` start at `spec_off`, `cg` at
/// `cg_off` and the scratch at `w_off`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3snl1(
    p: &Snl1Params,
    sig: &[Real],
    af11: &[Real],
    pm: &[i32],
    ic: &[i32],
    a: &[Real],
    cg: &[Real],
    kdmean: Real,
    s: &mut [Real],
    d: &mut [Real],
    w: &mut [Real],
    spec_off: usize,
    cg_off: usize,
    w_off: usize,
) {
    let nth = p.nth;
    let nspec = p.nspec;
    let nspecx = p.nspecx;
    // Offsets of UE(1-NTH:NSPECY), the eight (1-NTH:NSPECX) arrays and CON(NSPEC), in the
    // Fortran's declaration order.
    let lx = (nspecx + nth) as usize;
    let o_ue = w_off;
    let o_sa1 = o_ue + (p.nspecy + nth) as usize;
    let o_sa2 = o_sa1 + lx;
    let o_da1c = o_sa2 + lx;
    let o_da1p = o_da1c + lx;
    let o_da1m = o_da1p + lx;
    let o_da2c = o_da1m + lx;
    let o_da2p = o_da2c + lx;
    let o_da2m = o_da2p + lx;
    let o_con = o_da2m + lx;
    // No closures here: one capturing a local by reference is a pointer to a pointer, which
    // SPIR-V's logical addressing refuses.
    macro_rules! at {
        ($o:expr, $i:expr) => {
            at($o, nth, $i)
        };
    }
    let [awg1, awg2, awg3, awg4, awg5, awg6, awg7, awg8] = p.awg;
    let [swg1, swg2, swg3, swg4, swg5, swg6, swg7, swg8] = p.swg;
    let (dal1, dal2, dal3) = (p.dal1, p.dal2, p.dal3);

    // 1.  Calculate prop. constant
    let x = (p.kdcon * kdmean).max(p.kdmn);
    let x2 = (-1.0e15_f32).max(p.snls3 * x);
    let cons = p.snlc1 * (1. + p.snls1 / x * (1. - p.snls2 * x) * ww3_libm::expf(x2));

    // 2.  Prepare auxiliary spectrum and arrays
    let mut ifr = 1;
    while ifr <= p.nk {
        let conx = TPIINV / sig[ifr as usize] * cg[cg_off + (ifr - 1) as usize];
        let mut ith = 1;
        while ith <= nth {
            let isp = ith + (ifr - 1) * nth;
            w[at!(o_ue, isp)] = a[spec_off + (isp - 1) as usize] / conx;
            w[o_con + (isp - 1) as usize] = conx;
            ith += 1;
        }
        ifr += 1;
    }

    let mut ifr = p.nk + 1;
    while ifr <= p.nfrhgh {
        let mut ith = 1;
        while ith <= nth {
            let isp = ith + (ifr - 1) * nth;
            w[at!(o_ue, isp)] = w[at!(o_ue, isp - nth)] * p.fachfe;
            ith += 1;
        }
        ifr += 1;
    }

    let mut isp = 1 - nth;
    while isp <= 0 {
        w[at!(o_ue, isp)] = 0.;
        w[at!(o_sa1, isp)] = 0.;
        w[at!(o_sa2, isp)] = 0.;
        w[at!(o_da1c, isp)] = 0.;
        w[at!(o_da1p, isp)] = 0.;
        w[at!(o_da1m, isp)] = 0.;
        w[at!(o_da2c, isp)] = 0.;
        w[at!(o_da2p, isp)] = 0.;
        w[at!(o_da2m, isp)] = 0.;
        isp += 1;
    }

    // 3.  Calculate interactions for extended spectrum
    let mut isp = 1;
    while isp <= nspecx {
        let row = (isp - 1) as usize * PM_COLS;
        macro_rules! ue {
            ($j:expr) => {
                w[at!(o_ue, pm[row + $j])]
            };
        }

        // 3.a Energy at interacting bins
        let e00 = w[at!(o_ue, isp)];
        let ep1 = awg1 * ue!(0) + awg2 * ue!(1) + awg3 * ue!(2) + awg4 * ue!(3);
        let em1 = awg5 * ue!(4) + awg6 * ue!(5) + awg7 * ue!(6) + awg8 * ue!(7);
        let ep2 = awg1 * ue!(8) + awg2 * ue!(9) + awg3 * ue!(10) + awg4 * ue!(11);
        let em2 = awg5 * ue!(12) + awg6 * ue!(13) + awg7 * ue!(14) + awg8 * ue!(15);

        // 3.b Contribution to interactions
        let af = af11[(isp - 1) as usize];
        let factor = (cons * af) * e00;

        let sa1a = e00 * (ep1 * dal1 + em1 * dal2);
        let sa1b = sa1a - ep1 * em1 * dal3;
        let sa2a = e00 * (ep2 * dal1 + em2 * dal2);
        let sa2b = sa2a - ep2 * em2 * dal3;

        w[at!(o_sa1, isp)] = factor * sa1b;
        w[at!(o_sa2, isp)] = factor * sa2b;

        w[at!(o_da1c, isp)] = (cons * af) * (sa1a + sa1b);
        w[at!(o_da1p, isp)] = factor * (dal1 * e00 - dal3 * em1);
        w[at!(o_da1m, isp)] = factor * (dal2 * e00 - dal3 * ep1);

        w[at!(o_da2c, isp)] = (cons * af) * (sa2a + sa2b);
        w[at!(o_da2p, isp)] = factor * (dal1 * e00 - dal3 * em2);
        w[at!(o_da2m, isp)] = factor * (dal2 * e00 - dal3 * ep2);
        isp += 1;
    }

    // 4.  Put source and diagonal term together
    let mut isp = 1;
    while isp <= nspec {
        let row = (isp - 1) as usize * IC_COLS;
        macro_rules! pair {
            ($o1:expr, $o2:expr, $j:expr) => {
                w[at!($o1, ic[row + $j])] + w[at!($o2, ic[row + $j + 1])]
            };
        }

        s[spec_off + (isp - 1) as usize] = w[o_con + (isp - 1) as usize]
            * (-2. * (w[at!(o_sa1, isp)] + w[at!(o_sa2, isp)])
                + awg1 * pair!(o_sa1, o_sa2, 0)
                + awg2 * pair!(o_sa1, o_sa2, 2)
                + awg3 * pair!(o_sa1, o_sa2, 4)
                + awg4 * pair!(o_sa1, o_sa2, 6)
                + awg5 * pair!(o_sa1, o_sa2, 8)
                + awg6 * pair!(o_sa1, o_sa2, 10)
                + awg7 * pair!(o_sa1, o_sa2, 12)
                + awg8 * pair!(o_sa1, o_sa2, 14));

        d[spec_off + (isp - 1) as usize] = -2. * (w[at!(o_da1c, isp)] + w[at!(o_da2c, isp)])
            + swg1 * pair!(o_da1p, o_da2p, 0)
            + swg2 * pair!(o_da1p, o_da2p, 2)
            + swg3 * pair!(o_da1p, o_da2p, 4)
            + swg4 * pair!(o_da1p, o_da2p, 6)
            + swg5 * pair!(o_da1m, o_da2m, 8)
            + swg6 * pair!(o_da1m, o_da2m, 10)
            + swg7 * pair!(o_da1m, o_da2m, 12)
            + swg8 * pair!(o_da1m, o_da2m, 14);
        isp += 1;
    }
}
