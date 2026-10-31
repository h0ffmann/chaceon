// SPDX-License-Identifier: MPL-2.0
//! `W3SIN4`, `W3SPR4` and `CALC_USTAR`, ST4's wind input and mean parameters. Translated from
//! `model/src/w3src4md.F90:143-385`, `:424-906` and `:1738-1840` at metocean/WW3 `92e2cf66`, with
//! MetOcean's `USTAR` floor (`:680`) and `ISNAN` guards (`:780`, `:835`). The bits are the reference's
//! (`rust/port_report/W3SIN4.md`).
//!
//! Built for `STAB0`, so `ISTAB` is 3 and `DSTAB(3,:)` is `D`. `FACLN1` and `FACLN2` are dead. For
//! `FLX0`, not `FLX5`.
#![allow(clippy::assign_op_pattern)] // `x = x + y` keeps the shape of the Fortran line.

use crate::constants::{GRAV, KAPPA, NU_AIR, TPI, TPIINV};
use crate::real::Real;
use crate::util::{pow2, pow3, pow4, pow5};
use ww3_libm::{atan2f, expf, log10f, logf, powf, sincosf, sqrtf, tanhf};

pub const ITAUMAX: i32 = 200;
pub const JUMAX: i32 = 200;
pub const IUSTAR: i32 = 100;
pub const IALPHA: i32 = 200;
pub const ILEVTAIL: i32 = 50;
pub const SIZEFWTABLE: i32 = 300;
pub const ABMIN: Real = -1.;
pub const TAUWMAX: Real = 2.2361;

/// Scalars `W3SIN4` reads. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin4Params {
    pub nk: i32,
    pub nth: i32,
    pub nspec: i32,
    pub bbeta: Real,
    pub zzwnd: Real,
    pub aalpha: Real,
    pub zzalp: Real,
    pub ttauwshelter: Real,
    pub ssinthp: Real,
    pub zz0rat: Real,
    pub dth: Real,
    pub delust: Real,
    pub deltail: Real,
    pub delalp: Real,
    pub delab: Real,
    pub sswellf: [Real; 7],
}

/// The scalars of one `W3SIN4` call.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin4Point {
    pub ustar: Real,
    pub drat: Real,
    pub usdir: Real,
    pub z0: Real,
}

/// The scalars one `W3SIN4` call returns.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sin4Out {
    pub tauwx: Real,
    pub tauwy: Real,
    pub tauwnx: Real,
    pub tauwny: Real,
}

/// `TAUHFT2(ind, j, i)`, with bounds `0:IUSTAR`, `0:IALPHA`, `0:ILEVTAIL`.
#[inline]
pub fn tauhft2_at(ind: i32, j: i32, i: i32) -> usize {
    (ind + j * (IUSTAR + 1) + i * (IUSTAR + 1) * (IALPHA + 1)) as usize
}

/// `TAUHFT(ind, j)`, with bounds `0:IUSTAR`, `0:IALPHA`.
#[inline]
pub fn tauhft_at(ind: i32, j: i32) -> usize {
    (ind + j * (IUSTAR + 1)) as usize
}

/// One spectrum. `sig` is `SIG(0:NK+1)`. `llws` is `LLWS`, 0 or 1. `a`, `k`, `s`, `d` and `llws` start
/// at `spec_off` and `cg` at `cg_off`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3sin4(
    p: &Sin4Params,
    sig: &[Real],
    sig2: &[Real],
    dden: &[Real],
    dden2: &[Real],
    ecos: &[Real],
    esin: &[Real],
    fwtable: &[Real],
    tauhft: &[Real],
    tauhft2: &[Real],
    a: &[Real],
    cg: &[Real],
    k: &[Real],
    pt: &Sin4Point,
    s: &mut [Real],
    d: &mut [Real],
    llws: &mut [u32],
    out: &mut [Sin4Out],
    spec_off: usize,
    cg_off: usize,
    out_off: usize,
) {
    let (nk, nth, nspec) = (p.nk, p.nth, p.nspec);
    let (ustar, drat, usdir, z0) = (pt.ustar, pt.drat, pt.usdir, pt.z0);
    let shelter = p.ttauwshelter.abs();

    macro_rules! ix {
        ($o:expr, $i:expr) => {
            $o + (($i) - 1) as usize
        };
    }
    macro_rules! swell {
        ($i:expr) => {
            p.sswellf[$i - 1]
        };
    }
    macro_rules! a {
        ($is:expr) => {
            a[ix!(spec_off, $is)]
        };
    }
    macro_rules! k {
        ($is:expr) => {
            k[ix!(spec_off, $is)]
        };
    }
    macro_rules! cg {
        ($ik:expr) => {
            cg[ix!(cg_off, $ik)]
        };
    }
    macro_rules! d {
        ($is:expr) => {
            d[ix!(spec_off, $is)]
        };
    }

    // 1.a  estimation of surface roughness parameters
    let z0visc = 0.1 * NU_AIR / ustar.max(0.0001);
    let z0noz = z0visc.max(p.zz0rat * z0);

    // 1.b  estimation of surface orbital velocity and displacement
    let mut uorb = 0.;
    let mut aorb = 0.;
    each!(ik, 1, nk, {
        let mut eb = 0.;
        each!(ith, 1, nth, {
            let is = ith + (ik - 1) * nth;
            eb = eb + a!(is);
        });
        let sigk = sig[ik as usize];
        uorb = uorb + eb * pow2(sigk) * dden[ix!(0, ik)] / cg!(ik);
        aorb = aorb + eb * dden[ix!(0, ik)] / cg!(ik);
    });

    let uorb = 2. * sqrtf(uorb);
    let aorb1 = 2. * powf(aorb, 1. - 0.5 * swell!(6));
    let re = 4. * uorb * aorb1 / NU_AIR;

    // Defines the swell dissipation based on the "Reynolds number"
    let (pturb, pvisc);
    if swell!(4) > 0. {
        if swell!(7) > 0. {
            let smooth = 0.5 * tanhf((re - swell!(4)) / swell!(7));
            pturb = 0.5 + smooth;
            pvisc = 0.5 - smooth;
        } else if re <= swell!(4) {
            pturb = 0.;
            pvisc = 1.;
        } else {
            pturb = 1.;
            pvisc = 0.;
        }
    } else {
        pturb = 1.;
        pvisc = 1.;
    }

    let (fw, fu, fud);
    if swell!(2) == 0. {
        fw = swell!(3).abs().max(0.);
        fu = 0.;
        fud = 0.;
    } else {
        fu = swell!(3).abs();
        fud = swell!(2);
        let aorb = 2. * sqrtf(aorb);
        let xi = (log10f((aorb / z0noz).max(3.)) - ABMIN) / p.delab;
        let ind = (SIZEFWTABLE - 1).min(xi as i32);
        let deli1 = (xi - ind as Real).min(1.);
        let deli2 = 1. - deli1;
        fw = fwtable[ind as usize] * deli2 + fwtable[(ind + 1) as usize] * deli1;
    }

    // 2.  Diagonal
    let ust = ustar.max(0.0001);
    let (sin_d, cos_d) = sincosf(usdir);
    let taux = pow2(ust) * cos_d;
    let tauy = pow2(ust) * sin_d;

    // STRESSSTAB(3,1:2) and STRESSSTABN(3,1:2)
    let (mut st1, mut st2, mut stn1, mut stn2) = (0., 0., 0., 0.);
    let const0 = p.bbeta * drat / pow2(KAPPA);

    let (mut cosu, mut sinu) = (0., 0.);
    each!(ik, 1, nk, {
        let taupx = taux - shelter * st1;
        let taupy = tauy - shelter * st2;
        let ustp = powf(pow2(taupx) + pow2(taupy), 0.25).min(ust.max(0.3));
        let usdirp = atan2f(taupy, taupx);
        let (sn, cs) = sincosf(usdirp);
        cosu = cs;
        sinu = sn;
        let is1 = 1 + (ik - 1) * nth;
        let cm = k!(is1) / sig2[ix!(0, is1)];
        let ucn = ustp * cm + p.zzalp;
        let const2 =
            dden2[ix!(0, is1)] / cg!(ik) * GRAV / (sig[ik as usize] / k!(is1) * drat);
        let cnst = sig2[ix!(0, is1)] * const0;
        let zcn = logf(k!(is1) * z0);

        // precomputes swell factors
        let swellcoefv =
            -(swell!(5) * drat * 2. * k!(is1) * sqrtf(2. * NU_AIR * sig2[ix!(0, is1)]));
        let swellcoeft = -(drat * swell!(1) * 16. * pow2(sig2[ix!(0, is1)]) / GRAV);

        each!(ith, 1, nth, {
            let is = ith + (ik - 1) * nth;
            let coswind = ecos[ix!(0, is)] * cosu + esin[ix!(0, is)] * sinu;
            let mut lw;
            if coswind > 0.01 {
                let x = coswind * ucn;
                let zarg = KAPPA / x;
                let zlog = zcn + zarg;
                if zlog < 0. {
                    d!(is) = cnst * expf(zlog) * pow4(zlog) * ucn * ucn * powf(coswind, p.ssinthp);
                    lw = 1;
                } else {
                    d!(is) = 0.;
                    lw = 0;
                }
                // Added for consistency with ECWAM implsch.F
                if 28. * cm * ustar * coswind >= 1. {
                    lw = 1;
                }
            } else {
                d!(is) = 0.;
                lw = 0;
            }
            llws[ix!(spec_off, is)] = lw;

            if (swell!(1) != 0. && d!(is) < 1.0e-7 * sig2[ix!(0, is)]) || swell!(3) > 0. {
                let dvisc = swellcoefv;
                let dturb = swellcoeft * (fw * uorb + (fu + fud * coswind) * ustp);
                d!(is) = d!(is) + pturb * dturb + pvisc * dvisc;
            }

            // Sums up the wave-supported stress
            let mut temp2 = const2 * d!(is) * a!(is);
            if temp2.is_nan() {
                temp2 = 0.;
            }
            if d!(is) < 0. {
                stn1 = stn1 + temp2 * ecos[ix!(0, is)];
                stn2 = stn2 + temp2 * esin[ix!(0, is)];
            } else {
                st1 = st1 + temp2 * ecos[ix!(0, is)];
                st2 = st2 + temp2 * esin[ix!(0, is)];
            }
        });
    });

    let (xstress, ystress) = (st1, st2);
    let (tauwnx, tauwny) = (stn1, stn2);
    each!(is, 1, nspec, {
        s[ix!(spec_off, is)] = d!(is) * a!(is);
    });

    // Computes the high-frequency contribution
    let signk = sig[nk as usize];
    let const0 = p.dth * pow5(signk) / (pow2(GRAV) * TPI) * TPI * signk / cg!(nk);
    let mut temp = 0.;
    each!(ith, 1, nth, {
        let is = ith + (nk - 1) * nth;
        let coswind = ecos[ix!(0, is)] * cosu + esin[ix!(0, is)] * sinu;
        temp = temp + a!(is) * pow3(coswind.max(0.));
        if temp.is_nan() {
            temp = 0.;
        }
    });

    let taupx = taux - shelter * xstress;
    let taupy = tauy - shelter * ystress;
    let ustp = powf(pow2(taupx) + pow2(taupy), 0.25);
    let usdirp = atan2f(taupy, taupx);

    let ust = ustp;
    // finds the values in the tabulated stress TAUHFT
    let xi = ust / p.delust;
    let ind = (IUSTAR - 1).min(xi as i32).max(1);
    let deli1 = (xi - ind as Real).min(1.).max(0.);
    let deli2 = 1. - deli1;
    let xj = ((GRAV * z0 / pow2(ust.max(0.00001)) - p.aalpha) / p.delalp).max(0.);
    let j = (IALPHA - 1).min(xj as i32).max(1);
    let delj1 = (xj - j as Real).min(1.).max(0.);
    let delj2 = 1. - delj1;
    let tau1;
    if p.ttauwshelter > 0. {
        let xk = const0 * temp / p.deltail;
        let i = (ILEVTAIL - 1).min(xk as i32);
        let delk1 = (xk - i as Real).min(1.);
        let delk2 = 1. - delk1;
        tau1 = ((tauhft2[tauhft2_at(ind, j, i)] * deli2 + tauhft2[tauhft2_at(ind + 1, j, i)] * deli1) * delj2
            + (tauhft2[tauhft2_at(ind, j + 1, i)] * deli2 + tauhft2[tauhft2_at(ind + 1, j + 1, i)] * deli1)
                * delj1)
            * delk2
            + ((tauhft2[tauhft2_at(ind, j, i + 1)] * deli2
                + tauhft2[tauhft2_at(ind + 1, j, i + 1)] * deli1)
                * delj2
                + (tauhft2[tauhft2_at(ind, j + 1, i + 1)] * deli2
                    + tauhft2[tauhft2_at(ind + 1, j + 1, i + 1)] * deli1)
                    * delj1)
                * delk1;
    } else {
        tau1 = (tauhft[tauhft_at(ind, j)] * deli2 + tauhft[tauhft_at(ind + 1, j)] * deli1) * delj2
            + (tauhft[tauhft_at(ind, j + 1)] * deli2 + tauhft[tauhft_at(ind + 1, j + 1)] * deli1) * delj1;
    }
    let tauhf = const0 * temp * pow2(ust) * tau1;
    let (sn, cs) = sincosf(usdirp);
    let mut tauwx = xstress + tauhf * cs;
    let mut tauwy = ystress + tauhf * sn;

    // Reduces tail effect to make sure that wave-supported stress is less than total stress
    let eps1: Real = 0.00001;
    let eps2: Real = 0.000001;
    let tauw = sqrtf(pow2(tauwx) + pow2(tauwy));
    let ust2 = pow2(ustar.max(eps2));
    let tauwb = tauw.min((ust2 - eps1).max(pow2(eps2)));
    if tauwb < tauw {
        tauwx = tauwx * tauwb / tauw;
        tauwy = tauwy * tauwb / tauw;
    }
    out[out_off] = Sin4Out { tauwx, tauwy, tauwnx, tauwny };
}

/// Scalars `W3SPR4` and `CALC_USTAR` read. Shared with the device.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr4Params {
    pub nk: i32,
    pub nth: i32,
    pub nspec: i32,
    pub wwnmeanp: Real,
    pub wwnmeanptail: Real,
    pub fte: Real,
    pub ftf: Real,
    pub sstxftftail: Real,
    pub sstxftwn: Real,
    pub zzwnd: Real,
    pub aalpha: Real,
    pub deltauw: Real,
    pub delu: Real,
}

/// The scalars of one `W3SPR4` call.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr4Point {
    pub u: Real,
    pub udir: Real,
    pub ustar: Real,
    pub usdir: Real,
    pub tauwx: Real,
    pub tauwy: Real,
}

/// The scalars one `W3SPR4` call returns.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Spr4Out {
    pub emean: Real,
    pub fmean: Real,
    pub fmean1: Real,
    pub wnmean: Real,
    pub amax: Real,
    pub ustar: Real,
    pub usdir: Real,
    pub cd: Real,
    pub z0: Real,
    pub charn: Real,
    pub fmeanws: Real,
    pub dlwmean: Real,
}

/// `TAUT(i, j)`, with bounds `0:ITAUMAX`, `0:JUMAX`.
#[inline]
pub fn taut_at(i: i32, j: i32) -> usize {
    (i + j * (ITAUMAX + 1)) as usize
}

/// `CALC_USTAR`: `(USTAR, Z0, CHARN)`.
#[inline]
pub fn calc_ustar(p: &Spr4Params, taut: &[Real], windspeed: Real, tauw: Real) -> (Real, Real, Real) {
    let tauw_local = tauw.min(TAUWMAX).max(0.);
    let xi = sqrtf(tauw_local) / p.deltauw;
    let ind = (ITAUMAX - 1).min(xi as i32);
    let deli1 = (xi - ind as Real).min(1.);
    let deli2 = 1. - deli1;
    let xj = windspeed / p.delu;
    let j = (JUMAX - 1).min(xj as i32);
    let delj1 = (xj - j as Real).min(1.);
    let delj2 = 1. - delj1;
    let ustar = (taut[taut_at(ind, j)] * deli2 + taut[taut_at(ind + 1, j)] * deli1) * delj2
        + (taut[taut_at(ind, j + 1)] * deli2 + taut[taut_at(ind + 1, j + 1)] * deli1) * delj1;

    // Determines roughness length
    if ustar > 0.001 {
        let sqrtcdm1 = (windspeed / ustar).min(100.0);
        let z0 = p.zzwnd * expf(-KAPPA * sqrtcdm1);
        (ustar, z0, GRAV * z0 / pow2(ustar))
    } else if ustar > 0. {
        let sqrtcdm1 = (windspeed / ustar).min(100.0);
        (ustar, p.zzwnd * expf(-KAPPA * sqrtcdm1), p.aalpha)
    } else {
        (ustar, p.aalpha * 0.001 * 0.001 / GRAV, p.aalpha)
    }
}

/// One spectrum. `eb` and `eb2` are the Fortran's automatic arrays, `NK` each.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn w3spr4(
    p: &Spr4Params,
    sig: &[Real],
    dden: &[Real],
    ecos: &[Real],
    esin: &[Real],
    taut: &[Real],
    a: &[Real],
    cg: &[Real],
    wn: &[Real],
    llws: &[u32],
    pt: &Spr4Point,
    w: &mut [Real],
    out: &mut [Spr4Out],
    spec_off: usize,
    cg_off: usize,
    w_off: usize,
    out_off: usize,
) {
    let (nk, nth) = (p.nk, p.nth);
    let o_eb = w_off;
    let o_eb2 = w_off + nk as usize;
    macro_rules! ix {
        ($o:expr, $i:expr) => {
            $o + (($i) - 1) as usize
        };
    }
    macro_rules! cg {
        ($ik:expr) => {
            cg[ix!(cg_off, $ik)]
        };
    }
    macro_rules! dd {
        ($ik:expr) => {
            dden[ix!(0, $ik)]
        };
    }
    macro_rules! sg {
        ($ik:expr) => {
            sig[($ik) as usize]
        };
    }

    // 1.  Integrate over directions
    let (mut emean, mut emeanws, mut fmeanws, mut fmean, mut fmean1) = (0., 0., 0., 0., 0.);
    let (mut wnmean, mut amax, mut elcs, mut elsn) = (0., 0_f32, 0., 0.);
    each!(ik, 1, nk, {
        w[ix!(o_eb, ik)] = 0.;
        w[ix!(o_eb2, ik)] = 0.;
        each!(ith, 1, nth, {
            let is = ith + (ik - 1) * nth;
            let av = a[ix!(spec_off, is)];
            w[ix!(o_eb, ik)] = w[ix!(o_eb, ik)] + av;
            elcs = elcs + av * ecos[ix!(0, is)] * dd!(ik) / cg!(ik);
            elsn = elsn + av * esin[ix!(0, is)] * dd!(ik) / cg!(ik);
            if llws[ix!(spec_off, is)] != 0 {
                w[ix!(o_eb2, ik)] = w[ix!(o_eb2, ik)] + av;
            }
            amax = amax.max(av);
        });
    });

    let dlwmean = atan2f(elsn, elcs);

    // 2.  Integrate over wavenumbers
    each!(ik, 1, nk, {
        w[ix!(o_eb, ik)] = w[ix!(o_eb, ik)] * dd!(ik) / cg!(ik);
        w[ix!(o_eb2, ik)] = w[ix!(o_eb2, ik)] * dd!(ik) / cg!(ik);
        let (eb, eb2) = (w[ix!(o_eb, ik)], w[ix!(o_eb2, ik)]);
        emean = emean + eb;
        fmean = fmean + eb / sg!(ik);
        fmean1 = fmean1 + eb * powf(sg!(ik), 2. * p.wwnmeanptail);
        wnmean = wnmean + eb * powf(wn[ix!(cg_off, ik)], p.wwnmeanp);
        emeanws = emeanws + eb2;
        fmeanws = fmeanws + eb2 * powf(sg!(ik), 2. * p.wwnmeanptail);
    });

    // 3.  Add tail beyond discrete spectrum and get mean pars
    let eband = w[ix!(o_eb, nk)] / dd!(nk);
    emean = emean + eband * p.fte;
    fmean = fmean + eband * p.ftf;
    fmean1 = fmean1 + eband * p.sstxftftail;
    wnmean = wnmean + eband * p.sstxftwn;
    let eband = w[ix!(o_eb2, nk)] / dd!(nk);
    emeanws = emeanws + eband * p.fte;
    fmeanws = fmeanws + eband * p.sstxftftail;

    // 4.  Final processing
    let fmean = TPIINV * emean / fmean.max(1.0e-7);
    let fmean1 = if fmean1 < 1.0e-7 {
        TPIINV * sg!(nk)
    } else {
        TPIINV * powf(fmean1.max(1.0e-7) / emean.max(1.0e-7), 1. / (2. * p.wwnmeanptail))
    };
    let wnmean = powf(wnmean.max(1.0e-7) / emean.max(1.0e-7), 1. / p.wwnmeanp);
    let fmeanws = if fmeanws < 1.0e-7 || emeanws < 1.0e-7 {
        TPIINV * sg!(nk)
    } else {
        TPIINV * powf(fmeanws.max(1.0e-7) / emeanws.max(1.0e-7), 1. / (2. * p.wwnmeanptail))
    };

    // 5.  Cd and z0
    let tauw = sqrtf(pow2(pt.tauwx) + pow2(pt.tauwy));
    let (ustar, z0, charn) = calc_ustar(p, taut, pt.u, tauw);
    let unz = pt.u.max(0.01);
    let cd = pow2(ustar / unz);
    out[out_off] = Spr4Out {
        emean,
        fmean,
        fmean1,
        wnmean,
        amax,
        ustar,
        usdir: pt.udir,
        cd,
        z0,
        charn,
        fmeanws,
        dlwmean,
    };
}
