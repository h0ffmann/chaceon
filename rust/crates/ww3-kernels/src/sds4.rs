// SPDX-License-Identifier: MPL-2.0
//! `W3SDS4`, ST4's dissipation. Translated from `model/src/w3src4md.F90:1872-2599` at metocean/WW3
//! `92e2cf66`; the bits are the reference's (`rust/port_report/W3SDS4.md`).
//!
//! Like `snl1`: whole buffers and offsets, no closures, the automatic arrays in `w` and `iw`. `IK1` is
//! 1, because the `IG1` switch is not built. `GAM` and `XT` of case 2 are dead in the Fortran and are
//! left out, and so is `PB` of case 3.
#![allow(clippy::assign_op_pattern)] // `x = x + y` keeps the shape of the Fortran line.

use crate::constants::{DWAT, GRAV, PI, TPI};
use crate::real::Real;
use ww3_libm::{atan2f, expf, logf, powf, sincosf, sqrtf, tanhf};

const KHSMAX: Real = 2.;
const FAC_KD2: Real = 1000.;
/// `LOG(FAC_KD1)`, folded by gfortran with one rounding.
const LOG_FAC_KD1: Real = f32::from_bits(0x3c2306ac);
/// `2.*PI**2.`, folded by gfortran.
const TWO_PI2: Real = f32::from_bits(0x419de9e7);
const GRAV2: Real = f32::from_bits(0x42c050b5);
/// `0.8 * 2*PI` and `5. * 2*PI`, folded.
const TSTR_NUM: Real = f32::from_bits(0x40a0d97c);
const TMAX_NUM: Real = f32::from_bits(0x41fb53d2);

/// Scalars of `W3GDATMD` and `W3SRC4MD` that `W3SDS4` reads. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sds4Params {
    pub nk: i32,
    pub nth: i32,
    pub nspec: i32,
    pub dikcumul: i32,
    pub ssdsbrfdf: i32,
    pub ssdsiso: i32,
    pub nkhs: i32,
    pub nkd: i32,
    pub ndtab: i32,
    /// First extent of `SATINDICES` and `SATWEIGHTS`.
    pub sat_n: i32,
    /// First extent of `CUMULW`.
    pub cumul_n: i32,
    /// `FLOGRD(5,7)` and `FLOGRD(5,8)`.
    pub flogrd57: u32,
    pub flogrd58: u32,
    pub ssdsc: [Real; 21],
    /// `SSDSBM(0:4)`.
    pub ssdsbm: [Real; 5],
    pub ssdsbr: Real,
    pub ssdsbt: Real,
    pub ssdsp: Real,
    pub ssdsdth: Real,
    pub dth: Real,
    pub aairgb: Real,
    pub aaircmin: Real,
}

/// The scalars of one call.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sds4Point {
    pub ustar: Real,
    pub usdir: Real,
    pub depth: Real,
    pub dair: Real,
    pub dlwmean: Real,
}

/// Lengths of the scratch ranges of one call: `(w, iw)`.
#[inline]
pub fn work_len(p: &Sds4Params) -> (usize, usize) {
    let (nk, nth, nspec) = (p.nk as usize, p.nth as usize, p.nspec as usize);
    (14 * nk + 4 * nspec + 5 * nk + 2 * nk * nth + 2 * nth, 4 * nk)
}

/// gfortran's `NINT`: nearest, ties away from zero.
#[inline]
fn nint(x: Real) -> i32 {
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
fn floor(x: Real) -> i32 {
    let t = x as i32;
    if (t as Real) > x { t - 1 } else { t }
}

/// Fortran `SIGN(A, B)`.
#[inline]
fn sign(a: Real, b: Real) -> Real {
    f32::from_bits((a.to_bits() & 0x7fffffff) | (b.to_bits() & 0x80000000))
}

#[inline]
fn pow3(x: Real) -> Real {
    let x2 = x * x;
    x2 * x
}

/// `x**5` as GCC's power tree expands it.
#[inline]
fn pow5(x: Real) -> Real {
    let x2 = x * x;
    let x3 = x2 * x;
    x3 * x2
}

/// One spectrum. `sig` is `SIG(0:NK+1)`, indexed by `IFR`. `a`, `srhs`, `ddiag` and `brlambda` start
/// at `spec_off`, `k` and `cg` at `cg_off`, `whitecap` at `wc_off`, the scratch at `w_off`, `iw_off`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sds4(
    p: &Sds4Params,
    sig: &[Real],
    dden: &[Real],
    ec2: &[Real],
    es2: &[Real],
    esc: &[Real],
    ecos: &[Real],
    esin: &[Real],
    satindices: &[i32],
    satweights: &[Real],
    cumulw: &[Real],
    iktab: &[i32],
    dcki: &[Real],
    qbi: &[Real],
    a: &[Real],
    k: &[Real],
    cg: &[Real],
    pt: &Sds4Point,
    srhs: &mut [Real],
    ddiag: &mut [Real],
    brlambda: &mut [Real],
    whitecap: &mut [Real],
    w: &mut [Real],
    iw: &mut [i32],
    spec_off: usize,
    cg_off: usize,
    wc_off: usize,
    w_off: usize,
    iw_off: usize,
) {
    let (nk, nth, nspec) = (p.nk, p.nth, p.nspec);
    let (unk, unth, unspec) = (nk as usize, nth as usize, nspec as usize);
    let (ustar, usdir, depth, dair, dlwmean) = (pt.ustar, pt.usdir, pt.depth, pt.dair, pt.dlwmean);
    let (dth, ssdsbr, ssdsbrfdf) = (p.dth, p.ssdsbr, p.ssdsbrfdf);

    // The Fortran's 1-based arrays. `ssdsc!(n)` is SSDSC(n).
    macro_rules! ssdsc {
        ($i:expr) => {
            p.ssdsc[$i - 1]
        };
    }
    macro_rules! ix {
        ($o:expr, $i:expr) => {
            $o + (($i) - 1) as usize
        };
    }
    macro_rules! a {
        ($is:expr) => {
            a[ix!(spec_off, $is)]
        };
    }
    macro_rules! kk {
        ($ik:expr) => {
            k[ix!(cg_off, $ik)]
        };
    }
    macro_rules! cg {
        ($ik:expr) => {
            cg[ix!(cg_off, $ik)]
        };
    }
    macro_rules! sig {
        ($ik:expr) => {
            sig[($ik) as usize]
        };
    }
    macro_rules! srhs {
        ($is:expr) => {
            srhs[ix!(spec_off, $is)]
        };
    }
    macro_rules! ddiag {
        ($is:expr) => {
            ddiag[ix!(spec_off, $is)]
        };
    }
    macro_rules! brl {
        ($is:expr) => {
            brlambda[ix!(spec_off, $is)]
        };
    }

    // Offsets of the automatic arrays.
    let o_s1 = w_off;
    let o_e1 = o_s1 + unk;
    let o_dk = o_e1 + unk;
    let o_hs = o_dk + unk;
    let o_kbar = o_hs + unk;
    let o_dck = o_kbar + unk;
    let o_efdf = o_dck + unk;
    let o_bth0 = o_efdf + unk;
    let o_bth0s = o_bth0 + unk;
    let o_qb = o_bth0s + unk;
    let o_s2 = o_qb + unk;
    let o_brm12 = o_s2 + unk;
    let o_coef4 = o_brm12 + unk;
    let o_coef5 = o_coef4 + unk;
    let o_bth = o_coef5 + unk;
    let o_bths = o_bth + unspec;
    let o_pb = o_bths + unspec;
    let o_pb2 = o_pb + unspec;
    let o_msssum = o_pb2 + unspec;
    let o_msssum2 = o_msssum + 5 * unk;
    let o_msslong = o_msssum2 + unk * unth;
    let o_wthsum = o_msslong + unk * unth;
    let o_lmod = o_wthsum + unth;
    let (o_nsmooth, o_ntimes, o_iksup, o_imssmax) = (iw_off, iw_off + unk, iw_off + 2 * unk, iw_off + 3 * unk);

    macro_rules! v {
        ($o:expr, $i:expr) => {
            w[ix!($o, $i)]
        };
    }
    macro_rules! n {
        ($o:expr, $i:expr) => {
            iw[ix!($o, $i)]
        };
    }
    macro_rules! msssum {
        ($ik:expr, $j:expr) => {
            w[o_msssum + (($ik - 1) + ($j - 1) * nk) as usize]
        };
    }
    macro_rules! msssum2 {
        ($ik:expr, $ith:expr) => {
            w[o_msssum2 + (($ik - 1) + ($ith - 1) * nk) as usize]
        };
    }
    macro_rules! msslong {
        ($ik:expr, $ith:expr) => {
            w[o_msslong + (($ik - 1) + ($ith - 1) * nk) as usize]
        };
    }
    // Inclusive loop, as a Fortran DO with step 1.
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

    // 0.  Pre-Initialization
    each!(i, 0, unk - 1, {
        iw[o_nsmooth + i] = 0;
        iw[o_ntimes + i] = 0;
        iw[o_iksup + i] = 0;
        iw[o_imssmax + i] = 0;
        w[o_s1 + i] = 0.;
        w[o_e1 + i] = 0.;
        w[o_dk + i] = 0.;
        w[o_hs + i] = 0.;
        w[o_kbar + i] = 0.;
        w[o_dck + i] = 0.;
        w[o_efdf + i] = 0.;
        w[o_bth0 + i] = 0.;
        w[o_bth0s + i] = 0.;
        w[o_qb + i] = 0.;
        w[o_s2 + i] = 0.;
        w[o_brm12 + i] = 0.;
    });
    each!(i, 0, unspec - 1, {
        w[o_bth + i] = 0.;
        w[o_bths + i] = 0.;
        w[o_pb + i] = 0.;
        w[o_pb2 + i] = 0.;
        ddiag[spec_off + i] = 0.;
        srhs[spec_off + i] = 0.;
    });
    each!(i, 0, 5 * unk - 1, {
        w[o_msssum + i] = 0.;
    });

    // 1.  Initialization and numerical factors
    let facturb = ssdsc!(5) * (ustar * ustar) / GRAV * dair / DWAT;
    let ik1 = 1;
    let nthsum = floor(ssdsc!(10) + 0.5).min(nth - 1);
    if nthsum > 0 {
        each!(i, 1, nthsum, {
            v!(o_wthsum, i) = 1.;
        });
        v!(o_wthsum, nthsum + 1) = ssdsc!(10) + 0.5 - nthsum as Real;
    } else {
        v!(o_wthsum, 1) = 2. * ssdsc!(10);
    }

    // 1.b MSS parameters used for Modulation factors for B or lambda
    if ssdsc!(8) > 0. || ssdsc!(11) > 0. || ssdsc!(18) > 0. {
        each!(i, 0, unk * unth - 1, {
            w[o_msssum2 + i] = 0.;
        });
        each!(ik, 1, nk, {
            n!(o_imssmax, ik) = 1;
            let mut mssp = 0.;
            let mut msspc2 = 0.;
            let mut mssps2 = 0.;
            let mut msspcs = 0.;
            each!(ith, 1, nth, {
                let is = ith + (ik - 1) * nth;
                msslong!(ik, ith) = powf(kk!(ik), ssdsc!(20)) * a!(is) * dden[ix!(0, ik)] / cg!(ik);
            });
            each!(ith, 1, nth, {
                each!(jth, -nthsum, nthsum, {
                    let ith2 = 1 + (ith - 1 + jth + nth) % nth;
                    msssum2!(ik, ith) =
                        msssum2!(ik, ith) + msslong!(ik, ith2) * v!(o_wthsum, jth.abs() + 1);
                });
                msspc2 = msspc2 + msslong!(ik, ith) * ec2[ix!(0, ith)];
                mssps2 = mssps2 + msslong!(ik, ith) * es2[ix!(0, ith)];
                msspcs = msspcs + msslong!(ik, ith) * esc[ix!(0, ith)];
                mssp = mssp + msslong!(ik, ith);
            });
            each!(ik3, ik, nk, {
                msssum!(ik3, 1) = msssum!(ik3, 1) + mssp;
                msssum!(ik3, 3) = msssum!(ik3, 3) + msspc2;
                msssum!(ik3, 4) = msssum!(ik3, 4) + mssps2;
                msssum!(ik3, 5) = msssum!(ik3, 5) + msspcs;
            });
            let mut mssd = 0.5 * atan2f(2. * msssum!(ik, 5), msssum!(ik, 3) - msssum!(ik, 4));
            if mssd < 0. {
                mssd = mssd + PI;
            }
            n!(o_imssmax, ik) = 1 + nint(mssd * nth as Real / TPI);
            let (s, co) = sincosf(mssd);
            msssum!(ik, 2) = (msssum!(ik, 4) * (co * co) - 2. * msssum!(ik, 5) * s * co
                + msssum!(ik, 3) * (s * s))
                .max(0.);
        });
    }

    // 2.   Estimation of spontaneous breaking from local saturation
    match nint(ssdsc!(1)) {
        1 => {
            // 2.a  Case of a direction-dependent breaking term following Ardhuin et al. 2010
            let epsr = sqrtf(ssdsbr);
            each!(ik, ik1, nk, {
                let facsat = sig!(ik) * pow3(kk!(ik)) * dth;
                let is0 = (ik - 1) * nth;
                v!(o_bth, is0 + 1) = 0.;
                let mut asum = 0.;
                each!(i, is0 + 1, is0 + nth, {
                    asum = asum + a!(i);
                });
                v!(o_bth0, ik) = asum * facsat;
                let ikc = (ik - p.dikcumul).max(1);
                let kloc = powf(kk!(ik), 2. - ssdsc!(20));

                if p.ssdsdth >= 180. {
                    each!(i, is0 + 1, is0 + nth, {
                        v!(o_bth, i) = v!(o_bth0, ik);
                    });
                } else {
                    each!(ith, 1, nth, {
                        let is = ith + (ik - 1) * nth;
                        let facstrainb = if ssdsc!(8) > 0. || ssdsc!(11) > 0. {
                            let d = 1 + (ith - n!(o_imssmax, ikc)).abs();
                            let mssth = (msssum!(ikc, 1) - msssum!(ikc, 2)) * ec2[ix!(0, d)]
                                + msssum!(ikc, 2) * es2[ix!(0, d)] * kloc;
                            1. + ssdsc!(8) * sqrtf(mssth)
                                + ssdsc!(11) * sqrtf(msssum2!(ikc, ith) * kloc)
                        } else {
                            1.
                        };
                        let mut dot = 0.;
                        each!(j, 0, p.sat_n - 1, {
                            let at = (j + (ith - 1) * p.sat_n) as usize;
                            dot = dot + satweights[at] * a!(is0 + satindices[at]);
                        });
                        v!(o_bth, is) = dot * facsat * facstrainb;
                    });
                    if p.ssdsiso != 1 {
                        let mut mx = v!(o_bth, is0 + 1);
                        each!(i, is0 + 2, is0 + nth, {
                            if v!(o_bth, i) > mx {
                                mx = v!(o_bth, i);
                            }
                        });
                        v!(o_bth0, ik) = mx;
                    }
                }
            });

            // Optional smoothing of B and B0 over frequencies
            if ssdsbrfdf > 0 && ssdsbrfdf < nk / 2 {
                each!(i, 1, nk, {
                    v!(o_bth0s, i) = v!(o_bth0, i);
                    n!(o_nsmooth, i) = 1;
                });
                each!(i, 1, nspec, {
                    v!(o_bths, i) = v!(o_bth, i);
                });
                each!(ik, 1, ssdsbrfdf, {
                    v!(o_bth0s, 1 + ssdsbrfdf) = v!(o_bth0s, 1 + ssdsbrfdf) + v!(o_bth0, ik);
                    n!(o_nsmooth, 1 + ssdsbrfdf) = n!(o_nsmooth, 1 + ssdsbrfdf) + 1;
                    each!(ith, 1, nth, {
                        let is = ith + (ik - 1) * nth;
                        v!(o_bths, ith + ssdsbrfdf * nth) = v!(o_bths, ith + ssdsbrfdf * nth) + v!(o_bth, is);
                    });
                });
                each!(ik, ik1 + 1 + ssdsbrfdf, 1 + 2 * ssdsbrfdf, {
                    v!(o_bth0s, 1 + ssdsbrfdf) = v!(o_bth0s, 1 + ssdsbrfdf) + v!(o_bth0, ik);
                    n!(o_nsmooth, 1 + ssdsbrfdf) = n!(o_nsmooth, 1 + ssdsbrfdf) + 1;
                    each!(ith, 1, nth, {
                        let is = ith + (ik - 1) * nth;
                        v!(o_bths, ith + ssdsbrfdf * nth) = v!(o_bths, ith + ssdsbrfdf * nth) + v!(o_bth, is);
                    });
                });
                let mut ik = ssdsbrfdf;
                while ik >= ik1 {
                    v!(o_bth0s, ik) = v!(o_bth0s, ik + 1) - v!(o_bth0, ik + ssdsbrfdf + 1);
                    n!(o_nsmooth, ik) = n!(o_nsmooth, ik + 1) - 1;
                    each!(ith, 1, nth, {
                        let is = ith + (ik - 1) * nth;
                        v!(o_bths, is) = v!(o_bths, is + nth) - v!(o_bth, is + (ssdsbrfdf + 1) * nth);
                    });
                    ik -= 1;
                }
                each!(ik, ik1 + 1 + ssdsbrfdf, nk - ssdsbrfdf, {
                    v!(o_bth0s, ik) =
                        v!(o_bth0s, ik - 1) - v!(o_bth0, ik - ssdsbrfdf - 1) + v!(o_bth0, ik + ssdsbrfdf);
                    n!(o_nsmooth, ik) = n!(o_nsmooth, ik - 1);
                    each!(ith, 1, nth, {
                        let is = ith + (ik - 1) * nth;
                        v!(o_bths, is) = v!(o_bths, is - nth) - v!(o_bth, is - (ssdsbrfdf + 1) * nth)
                            + v!(o_bth, is + ssdsbrfdf * nth);
                    });
                });
                each!(ik, nk - ssdsbrfdf + 1, nk, {
                    v!(o_bth0s, ik) = v!(o_bth0s, ik - 1) - v!(o_bth0, ik - ssdsbrfdf);
                    n!(o_nsmooth, ik) = n!(o_nsmooth, ik - 1) - 1;
                    each!(ith, 1, nth, {
                        let is = ith + (ik - 1) * nth;
                        v!(o_bths, is) = v!(o_bths, is - nth) - v!(o_bth, is - (ssdsbrfdf + 1) * nth);
                    });
                });
                each!(i, 1, nk, {
                    v!(o_bth0, i) = (v!(o_bth0s, i) / n!(o_nsmooth, i) as Real).max(0.);
                });
                each!(ik, ik1, nk, {
                    let is0 = (ik - 1) * nth;
                    each!(i, is0 + 1, is0 + nth, {
                        v!(o_bth, i) = (v!(o_bths, i) / n!(o_nsmooth, ik) as Real).max(0.);
                    });
                });
            }

            // 2.a.2  Computes spontaneous breaking dissipation rate
            each!(ik, ik1, nk, {
                let miche = if p.ssdsbm[0] == 1. {
                    1.
                } else {
                    let x = tanhf((kk!(ik) * depth).min(10.));
                    let b = &p.ssdsbm;
                    let y = x * (b[1] + x * (b[2] + x * (b[3] + x * b[4])));
                    y * y
                };
                let coef1 = ssdsbr * miche;
                let e = (v!(o_bth0, ik) / coef1 - 1.).max(0.);
                let sdiagiso = ssdsc!(2) * sig!(ik) * ssdsc!(6) * (e * e);
                let coef2 = ssdsc!(2) * sig!(ik) * (1. - ssdsc!(6)) / (coef1 * coef1);
                each!(i, (ik - 1) * nth + 1, ik * nth, {
                    ddiag!(i) = sdiagiso + coef2 * powf((v!(o_bth, i) - coef1).max(0.), p.ssdsp);
                });
            });

            // Computes Breaking probability
            each!(i, 1, nspec, {
                let e = (sqrtf(v!(o_bth, i)) - epsr).max(0.);
                let pb = (e * e) * 28.16;
                v!(o_pb, i) = pb;
                brl!(i) = pb / TWO_PI2;
                srhs!(i) = ddiag!(i) * a!(i);
            });
        }
        2 => {
            // 2.b  Computes spontaneous breaking for T500 (Filipot et al. JGR 2010)
            each!(ik, ik1, nk, {
                v!(o_e1, ik) = 0.;
                each!(ith, 1, nth, {
                    let is = ith + (ik - 1) * nth;
                    v!(o_e1, ik) = v!(o_e1, ik) + (a!(is) * sig!(ik)) * dth;
                });
                v!(o_dk, ik) = dden[ix!(0, ik)] / (dth * sig!(ik) * cg!(ik));
            });

            // Gets windows indices of IKTAB
            let mut id = nint(depth).min(p.ndtab);
            if id < 1 {
                id = 1;
            } else if id > p.ndtab {
                id = p.ndtab;
            }

            // loop over wave scales
            let mut nkl = 0;
            each!(ikl, 1, nk, {
                n!(o_iksup, ikl) = iktab[((ikl - 1) + (id - 1) * nk) as usize];
                if n!(o_iksup, ikl) <= nk {
                    let mut dot = 0.;
                    each!(i, ikl, n!(o_iksup, ikl) - 1, {
                        dot = dot + v!(o_e1, i) * v!(o_dk, i);
                    });
                    v!(o_efdf, ikl) = dot;
                    if v!(o_efdf, ikl) != 0. {
                        let mut dot = 0.;
                        each!(i, ikl, n!(o_iksup, ikl) - 1, {
                            dot = dot + (kk!(i) * v!(o_e1, i)) * v!(o_dk, i);
                        });
                        v!(o_kbar, ikl) = dot / v!(o_efdf, ikl);
                    } else {
                        v!(o_kbar, ikl) = 0.;
                    }
                    v!(o_hs, ikl) = 4. * sqrtf(v!(o_efdf, ikl));
                    nkl += 1;
                }
            });

            // Computes Dissipation and breaking probability in each scale
            let dkhs = KHSMAX / p.nkhs as Real;
            each!(ikl, 1, nkl, {
                if v!(o_hs, ikl) != 0. && v!(o_kbar, ikl) != 0. {
                    let kbar = v!(o_kbar, ikl);
                    let mut ikd = (FAC_KD2 + nint(logf(kbar * depth) / LOG_FAC_KD1) as Real) as i32;
                    let mut ikhs = (1. + nint(kbar * v!(o_hs, ikl) / dkhs) as Real) as i32;
                    if ikd > p.nkd {
                        ikd = p.nkd;
                    } else if ikd < 1 {
                        ikd = 1;
                    }
                    if ikhs > p.nkhs {
                        ikhs = p.nkhs;
                    } else if ikhs < 1 {
                        ikhs = 1;
                    }
                    let at = ((ikhs - 1) + (ikd - 1) * p.nkhs) as usize;
                    v!(o_dck, ikl) = (powf(kbar, -2.5) * (kbar / TPI)) * dcki[at];
                    v!(o_qb, ikl) = qbi[at];
                } else {
                    v!(o_dck, ikl) = 0.;
                    v!(o_qb, ikl) = 0.;
                }
            });

            // Distributes scale dissipation over the frequency spectrum
            each!(ikl, 1, nkl, {
                if v!(o_efdf, ikl) > 0. {
                    each!(i, ikl, n!(o_iksup, ikl), {
                        v!(o_s1, i) = v!(o_s1, i) + v!(o_dck, ikl) * v!(o_e1, i) / v!(o_efdf, ikl);
                        v!(o_s2, i) = v!(o_s2, i) + v!(o_qb, ikl) * v!(o_e1, i) / v!(o_efdf, ikl);
                        n!(o_ntimes, i) = n!(o_ntimes, i) + 1;
                    });
                }
            });

            // Finish the average
            each!(i, 1, nk, {
                if n!(o_ntimes, i) > 0 {
                    v!(o_s1, i) = v!(o_s1, i) / n!(o_ntimes, i) as Real;
                    v!(o_s2, i) = v!(o_s2, i) / n!(o_ntimes, i) as Real;
                } else {
                    v!(o_s1, i) = 0.;
                    v!(o_s2, i) = 0.;
                }
                v!(o_s1, i) = v!(o_s1, i) / sig!(i);
            });

            // Makes Isotropic distribution
            each!(ik, 1, nk, {
                let mut sum = 0.;
                each!(i, (ik - 1) * nth + 1, ik * nth, {
                    sum = sum + a!(i);
                });
                let asum = sum * dth;
                each!(is, 1 + (ik - 1) * nth, ik * nth, {
                    if asum > 1.0e-8 {
                        ddiag!(is) = v!(o_s1, ik) / asum;
                        v!(o_pb2, is) = v!(o_s2, ik) / asum;
                    } else {
                        ddiag!(is) = 0.;
                        v!(o_pb2, is) = 0.;
                    }
                });
                v!(o_bth0, ik) = if v!(o_pb2, 1 + (ik - 1) * nth) > 0.001 { 2. * ssdsbr } else { 0. };
            });

            each!(i, 1, nspec, {
                v!(o_pb, i) = (1. - ssdsc!(1)) * v!(o_pb2, i) * a!(i) + ssdsc!(1) * v!(o_pb, i);
                brl!(i) = v!(o_pb, i) / TWO_PI2;
            });
        }
        3 => {
            // 2c Romero (GRL 2019)
            let t = 28. / ssdsc!(16);
            let ko = (GRAV / (1.0e-6 + ustar * ustar)) / (t * t);
            let (sin_d, cos_d) = sincosf(dlwmean);
            each!(ik, 1, nk, {
                let is0 = (ik - 1) * nth;
                let kloc = powf(kk!(ik), 2. - ssdsc!(20));
                let k3 = pow3(kk!(ik));
                each!(ith, 1, nth, {
                    v!(o_bth, ith) = (a!(is0 + ith) * sig!(ik) * k3).max(0.00000000000001);
                });

                if ssdsc!(8) > 0. {
                    each!(ith, 1, nth, {
                        let d = 1 + (ith - n!(o_imssmax, ik)).abs();
                        let mssth = (msssum!(ik, 1) - msssum!(ik, 2)) * ec2[ix!(0, d)]
                            + msssum!(ik, 2) * es2[ix!(0, d)] * kloc;
                        let f = 1. + ssdsc!(8) * sqrtf(mssth) + ssdsc!(11) * sqrtf(msssum2!(ik, ith)) * kloc;
                        v!(o_bth, ith) = v!(o_bth, ith) * f;
                    });
                }

                let ci = sig!(ik) / kk!(ik);
                let mut bth0 = 0.;
                each!(ith, 1, nth, {
                    bth0 = bth0 + v!(o_bth, ith) * dth;
                });
                v!(o_bth0, ik) = bth0;
                if ssdsc!(18) > 0. {
                    each!(ith, 1, nth, {
                        let facstrainl = if ssdsc!(11) > 0. {
                            let d = 1 + (ith - n!(o_imssmax, ik)).abs();
                            let mssth = (msssum!(ik, 1) - msssum!(ik, 2)) * ec2[ix!(0, d)]
                                + msssum!(ik, 2) * es2[ix!(0, d)] * kloc;
                            1. + ssdsc!(18) * sqrtf(mssth) + ssdsc!(11) * sqrtf(msssum2!(ik, ith) * kloc)
                        } else {
                            let e = ecos[ix!(0, ith)] * cos_d + esin[ix!(0, ith)] * sin_d;
                            1. + ssdsc!(18) * (powf(msssum!(ik, 1) * kloc, ssdsc!(14)) * (e * e))
                        };
                        v!(o_lmod, ith) = powf(facstrainl, ssdsc!(19));
                    });
                } else {
                    each!(ith, 1, nth, {
                        v!(o_lmod, ith) = 1.;
                    });
                }

                // Lambda
                each!(ith, 1, nth, {
                    brl!(is0 + ith) = ssdsc!(9)
                        * expf(-ssdsbr / v!(o_bth, ith))
                        * (1.0 + ssdsc!(13) * powf((kk!(ik) / ko).max(1.), ssdsc!(15)))
                        / (ssdsc!(13) + 1.)
                        * v!(o_lmod, ith);
                });
                // Breaking strength : generalisation of Duncan's b parameter
                let btover = sqrtf(v!(o_bth0, ik)) - sqrtf(p.ssdsbt);
                v!(o_brm12, ik) = ssdsc!(2) * powf(btover.max(0.), 2.5) / sig!(ik);
                let c5 = pow5(ci);
                each!(ith, 1, nth, {
                    let is = is0 + ith;
                    brl!(is) = sign(brl!(is), btover).max(0.);
                    srhs!(is) = v!(o_brm12, ik) / GRAV2 * brl!(is) * c5;
                    ddiag!(is) = srhs!(is) * ssdsbr / v!(o_bth, ith).max(1.0e-20) / a!(is).max(1.0e-20);
                });
            });
        }
        _ => {}
    }

    // WAVE-TURBULENCE INTERACTION AND CUMULATIVE EFFECT
    if ssdsc!(3) != 0. || ssdsc!(5) != 0. || ssdsc!(21) != 0. {
        let (sin_u, cos_u) = sincosf(usdir);
        each!(ik, ik1, nk, {
            let facturb2 = -2. * sig!(ik) * kk!(ik) * facturb;
            let dvisc = -4. * ssdsc!(21) * kk!(ik) * kk!(ik);
            each!(ith, 1, nth, {
                let is = ith + (ik - 1) * nth;
                let mut renewalfreq = 0.;
                if ssdsc!(3) != 0. && ik > p.dikcumul {
                    each!(ik2, ik1, ik - p.dikcumul, {
                        if v!(o_bth0, ik2) > ssdsbr {
                            let is2 = (ik2 - 1) * nth;
                            let mut dot = 0.;
                            each!(i, is2 + 1, is2 + nth, {
                                dot = dot + cumulw[((i - 1) + (is - 1) * p.cumul_n) as usize] * brl!(i);
                            });
                            renewalfreq = renewalfreq + dot;
                        }
                    });
                }
                let coswind = ecos[ix!(0, ith)] * cos_u + esin[ix!(0, ith)] * sin_u;
                let dturb = facturb2 * coswind.max(0.);
                let diag2 = ssdsc!(3) * renewalfreq + dturb + dvisc;
                ddiag!(is) = ddiag!(is) + diag2;
                srhs!(is) = srhs!(is) + a!(is) * diag2;
            });
        });
    }

    // COMPUTES WHITECAP PARAMETERS
    if p.flogrd57 == 0 && p.flogrd58 == 0 {
        return;
    }
    whitecap[wc_off] = 0.;
    whitecap[wc_off + 1] = 0.;
    let ikw = floor(p.aaircmin).min(nk);
    each!(ik, 1, ikw, {
        let ci = sig!(ik) / kk!(ik);
        let is0 = (ik - 1) * nth;
        let brm12 = v!(o_brm12, ik);
        let mut sum4 = 0.;
        let mut sum5 = 0.;
        each!(i, is0 + 1, is0 + nth, {
            sum4 = sum4 + brl!(i);
            sum5 = sum5 + brl!(i) * brm12;
        });
        v!(o_coef4, ik) = ci * ci * sum4 * 2. * PI / GRAV * ssdsc!(7) * dden[ix!(0, ik)] / (sig!(ik) * cg!(ik));
        v!(o_coef5, ik) = pow3(ci) * sum5 * p.aairgb / GRAV * dden[ix!(0, ik)] / (sig!(ik) * cg!(ik));
    });
    each!(ik, ikw, nk, {
        v!(o_coef4, ik) = 0.;
        v!(o_coef5, ik) = 0.;
    });

    if p.flogrd57 != 0 {
        each!(ik, ik1, ikw, {
            whitecap[wc_off] = whitecap[wc_off] + v!(o_coef4, ik) * (1. - whitecap[wc_off]);
            whitecap[wc_off + 3] = whitecap[wc_off + 3] + v!(o_coef5, ik);
        });
    }

    if p.flogrd58 != 0 {
        each!(ik, ik1, nk, {
            let tstr = TSTR_NUM / sig!(ik);
            let tmax = TMAX_NUM / sig!(ik);
            let dt = tmax / 50.;
            let mut mft = 0.;
            each!(it, 1, 50, {
                let t = it as Real * dt;
                if t < tstr {
                    mft = mft + 0.4 / (kk!(ik) * tstr) * t * dt;
                } else {
                    mft = mft + 0.4 / kk!(ik) * expf(-(t - tstr) / 3.8) * dt;
                }
            });
            mft = mft / tmax;
            whitecap[wc_off + 1] = whitecap[wc_off + 1] + v!(o_coef4, ik) * mft;
        });
    }
}
