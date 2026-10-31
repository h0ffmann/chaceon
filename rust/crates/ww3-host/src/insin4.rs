// SPDX-License-Identifier: MPL-2.0
//! ST4's tables, built once on the host: `INSIN4`, `TABU_STRESS`, `TABU_TAUHF`, `TABU_TAUHF2`
//! (`model/src/w3src4md.F90:917-1720`), `TABU_FW`, `KZEONE`, `KERKEI` (`model/src/constants.F90:119-440`)
//! and `WAVNU2` (`model/src/w3dispmd.F90:203-344`), at metocean/WW3 `92e2cf66`.
//!
//! GCC turns `x**2.` into `x*x` at any optimisation level, and keeps other real exponents as `powf`.
//! `ST4TABUHF2.bin` is a cache of `TABU_TAUHF2`, so it is not read here.

#![allow(clippy::approx_constant, clippy::excessive_precision, clippy::assign_op_pattern)]

use ww3_kernels::constants::{GRAV, KAPPA, RADE};
use ww3_kernels::nint;
use ww3_kernels::real::{Dbl, Real};
use ww3_kernels::sin4::{IALPHA, ILEVTAIL, ITAUMAX, IUSTAR, JUMAX, SIZEFWTABLE, TAUWMAX};
use ww3_libm::{expf, logf, powf, sincosf, sqrtf, tanhf};

const UMAX: Real = 50.;
const NKHI: i32 = 100;
const FAC_KD2: i32 = 1000;
const FAC_KD1: Real = 1.01;
const KHSMAX: Real = 2.;
const KHMAX: Real = 2.;
const JTOT: i32 = 250;

fn p2(x: Real) -> Real {
    x * x
}

fn p3(x: Real) -> Real {
    x * x * x
}

fn p4(x: Real) -> Real {
    let x2 = x * x;
    x2 * x2
}

/// libgcc's `__powisf2`, for a run-time integer exponent.
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

/// `(DELTAUW, DELU, TAUT(0:ITAUMAX,0:JUMAX))`.
pub fn tabu_stress(zzwnd: Real, aalpha: Real, zz0max: Real) -> (Real, Real, Vec<Real>) {
    const NITER: i32 = 10;
    const XM: Real = 0.50;
    const EPS1: Real = 0.00001;
    let delu = UMAX / JUMAX as Real;
    let deltauw = TAUWMAX / ITAUMAX as Real;
    let n = (ITAUMAX + 1) as usize;
    let mut taut = vec![0.; n * (JUMAX + 1) as usize];
    for i in 0..=ITAUMAX {
        let ztauw = p2(i as Real * deltauw);
        for j in 0..=JUMAX {
            let utop = j as Real * delu;
            let cdrag: Real = 0.0012875;
            let wcd = sqrtf(cdrag);
            let ustold = utop * wcd;
            let mut tauold = p2(ustold).max(ztauw + EPS1);
            for _ in 0..NITER {
                let x = ztauw / tauold;
                let ust = sqrtf(tauold);
                let mut zz00 = aalpha * tauold / GRAV;
                if zz0max != 0. {
                    zz00 = zz00.min(zz0max);
                }
                let zz0 = zz00 / powf(1. - x, XM);
                let l = logf(zzwnd / zz0);
                let f = ust - KAPPA * utop / l;
                let delf = 1. - KAPPA * utop / p2(l) * 2. / ust * (1. - (XM + 1.) * x) / (1. - x);
                let ust = ust - f / delf;
                tauold = p2(ust).max(ztauw + EPS1);
            }
            taut[(i + j * (ITAUMAX + 1)) as usize] = sqrtf(tauold);
        }
    }
    for i in 0..=ITAUMAX {
        taut[i as usize] = 0.0;
    }
    (deltauw, delu, taut)
}

fn weights() -> Vec<Real> {
    let mut w = vec![1.; JTOT as usize];
    w[0] = 0.5;
    w[(JTOT - 1) as usize] = 0.5;
    w
}

/// `(DELUST, DELALP, TAUHFT(0:IUSTAR,0:IALPHA))`.
pub fn tabu_tauhf(sigmax: Real, aalpha: Real, bbeta: Real, zzalp: Real, zz0max: Real) -> (Real, Real, Vec<Real>) {
    let ustarm: Real = 5.;
    let alpham = 20. * aalpha;
    let delust = ustarm / IUSTAR as Real;
    let delalp = alpham / IALPHA as Real;
    let const1 = bbeta / p2(KAPPA);
    let omegac = sigmax;
    let w = weights();
    let x0: Real = 0.05;
    let mut t = vec![0.; ((IUSTAR + 1) * (IALPHA + 1)) as usize];
    for l in 0..=IALPHA {
        for k in 0..=IUSTAR {
            let ust = (k as Real * delust).max(0.000001);
            let mut zz00 = p2(ust) * aalpha / GRAV;
            if zz0max != 0. {
                zz00 = zz00.min(zz0max);
            }
            let zz0 = zz00 * (1. + l as Real * delalp / aalpha);
            let omegacc = omegac.max(x0 * GRAV / ust);
            let yc = omegacc * sqrtf(zz0 / GRAV);
            let dely = ((1. - yc) / JTOT as Real).max(0.);
            let mut sum = 0.;
            for j in 1..=JTOT {
                let y = yc + (j - 1) as Real * dely;
                let omega = y * sqrtf(GRAV / zz0);
                let cm = GRAV / omega;
                let zx = ust / cm + zzalp;
                let zarg = (KAPPA / zx).min(20.);
                let zmu = (GRAV * zz0 / p2(cm) * expf(zarg)).min(1.);
                let zlog = logf(zmu).min(0.);
                let zbeta = const1 * zmu * p4(zlog);
                sum = sum + w[(j - 1) as usize] * zbeta / y * dely;
            }
            t[(k + l * (IUSTAR + 1)) as usize] = sum;
        }
    }
    (delust, delalp, t)
}

/// `(DELTAIL, TAUHFT, TAUHFT2)`. `TAUHFT` comes out differently from `tabu_tauhf`, and it is the one
/// WW3 keeps when `TTAUWSHELTER > 0`.
pub fn tabu_tauhf2(
    sigmax: Real,
    aalpha: Real,
    bbeta: Real,
    zzalp: Real,
    delust: Real,
    delalp: Real,
) -> (Real, Vec<Real>, Vec<Real>) {
    let alpham = 20. * aalpha;
    let deltail = alpham / ILEVTAIL as Real;
    let const1 = bbeta / p2(KAPPA);
    let omegac = sigmax;
    let w = weights();
    let x0: Real = 0.05;
    let (n1, n2) = (IUSTAR + 1, IALPHA + 1);
    let mut t1 = vec![0.; (n1 * n2) as usize];
    let mut t2 = vec![0.; (n1 * n2 * (ILEVTAIL + 1)) as usize];
    for k in 0..=IUSTAR {
        let ust0 = (k as Real * delust).max(0.000001);
        for l in 0..=IALPHA {
            let mut ust = ust0;
            let zz0 = p2(ust0) * (aalpha + l as Real * delalp) / GRAV;
            let omegacc = omegac.max(x0 * GRAV / ust);
            let yc = omegacc * sqrtf(zz0 / GRAV);
            let dely = ((1. - yc) / JTOT as Real).max(0.);
            for i in 0..=ILEVTAIL {
                let levtail = i as Real * deltail;
                let (mut sum1, mut sum2) = (0., 0.);
                let tauw0 = p2(ust0);
                let mut tauw = tauw0;
                for j in 1..=JTOT {
                    let wj = w[(j - 1) as usize];
                    let y = yc + (j - 1) as Real * dely;
                    let omega = y * sqrtf(GRAV / zz0);
                    let cm = GRAV / omega;
                    let zx = ust0 / cm + zzalp;
                    let zarg = (KAPPA / zx).min(20.);
                    let zmu = (GRAV * zz0 / p2(cm) * expf(zarg)).min(1.);
                    let zlog = logf(zmu).min(0.);
                    let zbeta = const1 * zmu * p4(zlog);
                    sum1 = sum1 + wj * zbeta / y * dely;
                    let zx = ust / cm + zzalp;
                    let zarg = (KAPPA / zx).min(20.);
                    let zmu = (GRAV * zz0 / p2(cm) * expf(zarg)).min(1.);
                    let zlog = logf(zmu).min(0.);
                    let zbeta = const1 * zmu * p4(zlog);
                    sum2 = sum2 + wj * zbeta * p2(ust / ust0) / y * dely;
                    tauw = tauw - wj * p2(ust) * zbeta * levtail / y * dely;
                    ust = sqrtf(tauw.max(0.0001));
                }
                t1[(k + l * n1) as usize] = sum1;
                t2[(k + l * n1 + i * n1 * n2) as usize] = sum2;
            }
        }
    }
    (deltail, t1, t2)
}

/// `KZEONE`: `(RE0, IM0, RE1, IM1)`.
#[allow(clippy::many_single_char_names)]
fn kzeone(x: Dbl, y: Dbl) -> (Dbl, Dbl, Dbl, Dbl) {
    const EXSQ: [Dbl; 8] = [
        0.5641003087264, 0.4120286874989, 0.1584889157959, 0.3078003387255e-1, 0.2778068842913e-2,
        0.1000044412325e-3, 0.1059115547711e-5, 0.1522475804254e-8,
    ];
    const TSQ: [Dbl; 8] = [
        0.0, 3.19303633920635e-1, 1.29075862295915, 2.95837445869665, 5.40903159724444, 8.80407957805676,
        1.34685357432515e1, 2.02499163658709e1,
    ];
    let r2 = x * x + y * y;
    if r2 >= 1.96e2 {
        let (mut rterm, mut iterm) = (1.0, 0.0);
        let (mut re0, mut im0, mut re1, mut im1) = (1.0, 0.0, 1.0, 0.0);
        let p1 = 8.0 * r2;
        let p2 = r2.sqrt();
        let l = 3.91 + 8.12e1 / p2;
        let (mut r1, mut r2) = (1.0, 1.0);
        let (mut m, mut k) = (-8, 3);
        for n in 1..=(l as i32) {
            m += 8;
            k -= m;
            r1 = ((k - 4) as f32 as Dbl) * r1;
            r2 = (k as f32 as Dbl) * r2;
            let t1 = (n as f32 as Dbl) * p1;
            let t2 = rterm;
            rterm = (t2 * x + iterm * y) / t1;
            iterm = (-t2 * y + iterm * x) / t1;
            re0 = re0 + r1 * rterm;
            im0 = im0 + r1 * iterm;
            re1 = re1 + r2 * rterm;
            im1 = im1 + r2 * iterm;
        }
        let t1 = (p2 + x).sqrt();
        let t2 = -y / t1;
        let p1 = 8.86226925452758e-1 / p2;
        let (rterm, iterm) = (p1 * y.cos(), -p1 * y.sin());
        let r1 = re0 * rterm - im0 * iterm;
        let r2 = re0 * iterm + im0 * rterm;
        let (re0, im0) = (t1 * r1 - t2 * r2, t1 * r2 + t2 * r1);
        let r1 = re1 * rterm - im1 * iterm;
        let r2 = re1 * iterm + im1 * rterm;
        return (re0, im0, t1 * r1 - t2 * r2, t1 * r2 + t2 * r1);
    }
    if r2 >= 1.849e1 {
        let (x2, y2) = (2.0 * x, 2.0 * y);
        let r1 = y2 * y2;
        let p1 = (x2 * x2 + r1).sqrt();
        let p2 = (p1 + x2).sqrt();
        let t1 = EXSQ[0] / (2.0 * p1);
        let (mut re0, mut im0, mut re1, mut im1) = (t1 * p2, t1 / p2, 0.0, 0.0);
        for n in 1..8 {
            let t2 = x2 + TSQ[n];
            let p1 = (t2 * t2 + r1).sqrt();
            let p2 = (p1 + t2).sqrt();
            let t1 = EXSQ[n] / p1;
            re0 = re0 + t1 * p2;
            im0 = im0 + t1 / p2;
            let t1 = EXSQ[n] * TSQ[n];
            re1 = re1 + t1 * p2;
            im1 = im1 + t1 / p2;
        }
        let t2 = -y2 * im0;
        let re1 = re1 / r2;
        let r2 = y2 * im1 / r2;
        let rterm = 1.41421356237309 * y.cos();
        let iterm = -1.41421356237309 * y.sin();
        let im0 = re0 * iterm + t2 * rterm;
        let re0 = re0 * rterm - t2 * iterm;
        let t1 = re1 * rterm - r2 * iterm;
        let t2 = re1 * iterm + r2 * rterm;
        return (re0, im0, t1 * x + t2 * y, -t1 * y + t2 * x);
    }
    let (x2, y2) = (x / 2.0, y / 2.0);
    let (mut p1, mut p2) = (x2 * x2, y2 * y2);
    let mut t1 = -((p1 + p2).ln() / 2.0 + 0.5772156649015329);
    let t2 = -y.atan2(x);
    let x2 = p1 - p2;
    let y2 = x * y2;
    let (mut rterm, mut iterm) = (1.0, 0.0);
    let (mut re0, mut im0) = (t1, t2);
    t1 = t1 + 0.5;
    let (mut re1, mut im1) = (t1, t2);
    p2 = r2.sqrt();
    let mut l = 2.106 * p2 + 4.4;
    if p2 < 8.0e-1 {
        l = 2.129 * p2 + 4.0;
    }
    for n in 1..=(l as i32) {
        p1 = n as Dbl;
        p2 = (n * n) as Dbl;
        let r1 = rterm;
        rterm = (r1 * x2 - iterm * y2) / p2;
        iterm = (r1 * y2 + iterm * x2) / p2;
        t1 = t1 + 0.5 / p1;
        re0 = re0 + t1 * rterm - t2 * iterm;
        im0 = im0 + t1 * iterm + t2 * rterm;
        p1 = p1 + 1.0;
        t1 = t1 + 0.5 / p1;
        re1 = re1 + (t1 * rterm - t2 * iterm) / p1;
        im1 = im1 + (t1 * iterm + t2 * rterm) / p1;
    }
    let r1 = x / r2 - 0.5 * (x * re1 - y * im1);
    let r2 = -y / r2 - 0.5 * (x * im1 + y * re1);
    let p1 = x.exp();
    (p1 * re0, p1 * im0, p1 * r1, p1 * r2)
}

/// `KERKEI`: `(KER, KEI)`.
fn kerkei(x: Real) -> (Real, Real) {
    let zr = x as Dbl * 0.50 * (2.0 as Dbl).sqrt();
    let (cyr, cyi, _, _) = kzeone(zr, zr);
    ((cyr / zr.exp()) as Real, (cyi / zr.exp()) as Real)
}

/// `(DELAB, FWTABLE(0:SIZEFWTABLE))`.
pub fn tabu_fw() -> (Real, Vec<Real>) {
    const ABMIN: Real = -1.;
    const ABMAX: Real = 8.;
    let delab = (ABMAX - ABMIN) / SIZEFWTABLE as Real;
    let l10 = logf(10.);
    let mut t = vec![0.; (SIZEFWTABLE + 1) as usize];
    for i in 0..=SIZEFWTABLE {
        let abrlog = ABMIN + i as Real * delab;
        let abr = expf(abrlog * l10);
        let fact = 1. / abr / (21.2 * KAPPA);
        let mut fsubw: Real = 0.05;
        let mut dzeta0: Real = 0.;
        for _ in 0..100 {
            let fsubwmemo = fsubw;
            let dzeta0memo = dzeta0;
            dzeta0 = fact * powf(fsubw, -0.5);
            let (ker, kei) = kerkei(2. * sqrtf(dzeta0));
            fsubw = 0.08 / (p2(ker) + p2(kei));
            fsubw = 0.5 * (fsubwmemo + fsubw);
            dzeta0 = 0.5 * (dzeta0memo + dzeta0);
        }
        t[i as usize] = fsubw.min(0.5);
    }
    (delab, t)
}

/// `WAVNU2`: `(K, CG)`, for positive `w`.
pub fn wavnu2_cg(w: Real, h: Real, eps: Real, nmax: i32) -> (Real, Real) {
    let k = wavnu2(w, h, eps, nmax);
    let w0 = w.abs();
    let cg = if 2. * k * h > 25. {
        w0 / k * 0.5
    } else {
        w0 / k * 0.5 * (1. + (2. * k * h / (2. * k * h).sinh()))
    };
    (k, cg)
}

/// `WAVNU2`: `K`, for positive `w`.
pub fn wavnu2(w: Real, h: Real, eps: Real, nmax: i32) -> Real {
    let w0 = w.abs();
    let mut k = if w0 < sqrtf(GRAV / h) { w0 / sqrtf(GRAV * h) } else { w0 * w0 / GRAV };
    let mut kold: Real = 0.;
    for _ in 0..nmax {
        let dif = (k - kold).abs();
        let rdif = if k != 0. { dif / k } else { 0. };
        if dif < eps && rdif < eps {
            return k;
        }
        kold = k;
        let f = GRAV * kold * tanhf(kold * h) - p2(w0);
        let fd = if kold * h > 25. {
            GRAV * tanhf(kold * h)
        } else {
            GRAV * tanhf(kold * h) + GRAV * kold * h / p2((kold * h).cosh())
        };
        k = kold - f / fd;
    }
    k
}

/// What `INSIN4` reads.
pub struct Insin4Input<'a> {
    pub nk: i32,
    pub nth: i32,
    pub nkhs: i32,
    pub nkd: i32,
    pub ndtab: i32,
    pub sat_n: i32,
    pub dth: Real,
    pub xfr: Real,
    pub ssdsdth: Real,
    pub ssdscos: Real,
    pub ssdsbrf1: Real,
    pub ssdsbint: Real,
    pub ssdspbk: Real,
    pub ssdsabk: Real,
    pub ssdshck: Real,
    pub ssdsbck: Real,
    pub ssdsc3: Real,
    /// `SIG(0:NK+1)`.
    pub sig: &'a [Real],
    /// `DSIP(0:NK+1)`.
    pub dsip: &'a [Real],
    pub th: &'a [Real],
    pub ecos: &'a [Real],
}

/// What `INSIN4` makes, past the `TABU_` tables.
pub struct Insin4Output {
    pub satindices: Vec<i32>,
    pub satweights: Vec<Real>,
    pub iktab: Vec<i32>,
    pub dcki: Vec<Real>,
    pub qbi: Vec<Real>,
    pub cumulw: Vec<Real>,
    pub dikcumul: i32,
}

/// `INSIN4` past the `TABU_` calls.
pub fn insin4(p: &Insin4Input) -> Insin4Output {
    let (nk, nth) = (p.nk, p.nth);
    let nspec = (nk * nth) as usize;
    let sn = p.sat_n as usize;
    let mut satindices = vec![1; sn * nth as usize];
    let mut satweights = vec![0.; sn * nth as usize];
    if p.ssdsdth < 180. {
        let sdsnth = nint(p.ssdsdth / (p.dth * RADE)).min(nth / 2 - 1);
        for ith in 1..=nth {
            for i_int in ith - sdsnth..=ith + sdsnth {
                let mut j_int = i_int;
                if i_int < 1 {
                    j_int = i_int + nth;
                }
                if i_int > nth {
                    j_int = i_int - nth;
                }
                let at = (i_int - (ith - sdsnth)) as usize + (ith - 1) as usize * sn;
                satindices[at] = j_int;
                let cos = sincosf(p.th[(ith - 1) as usize] - p.th[(j_int - 1) as usize]).1;
                satweights[at] = powf(cos, p.ssdscos);
            }
        }
    } else {
        satweights.fill(1.);
    }

    let (nkhs, nkd, ndtab) = (p.nkhs, p.nkd, p.ndtab);
    let mut iktab = vec![1; (nk * ndtab) as usize];
    let mut dcki = vec![0.; (nkhs * nkd) as usize];
    let mut qbi = vec![0.; (nkhs * nkd) as usize];
    if p.ssdsbck > 0. {
        let binf = 1. - p.ssdsbint;
        let bsup = 1. + p.ssdsbint;
        iktab.fill(0);
        for id in 1..=ndtab {
            let mut toto = 0;
            let prof = id as Real;
            for ikl in 1..=nk {
                let k1 = wavnu2(p.sig[ikl as usize], prof, 1.0e-7, 15);
                let k2 = p2(bsup / binf) * k1;
                let sigtab = sqrtf(GRAV * k2 * tanhf(k2 * id as Real));
                let at = ((ikl - 1) + (id - 1) * nk) as usize;
                if sigtab <= p.sig[1] {
                    iktab[at] = 1;
                }
                if sigtab > p.sig[nk as usize] {
                    iktab[at] = nk + toto;
                    toto = 1;
                }
                for ik in 1..nk {
                    let (s0, s1) = (p.sig[ik as usize], p.sig[(ik + 1) as usize]);
                    if s0 < sigtab && s1 >= sigtab {
                        let diff1 = sigtab - s0;
                        let diff2 = s1 - sigtab;
                        iktab[at] = if diff1 < diff2 { ik } else { ik + 1 };
                    }
                }
            }
        }

        let dhs = KHSMAX / nkhs as Real;
        let dkh = KHMAX / NKHI as Real;
        let sqrt2 = sqrtf(2.);
        for ikd in 1..=nkd {
            let mut khs: Real = 0.;
            // `-O3` peels the first pass and folds `1.01**(-999)` with one rounding; every other
            // pass calls `__powisf2`, whose rounded chain gives 0x384a1eb3 for that value.
            let kd = if ikd == 1 { f32::from_bits(0x384a1eff) } else { powi(FAC_KD1, ikd - FAC_KD2) };
            let xt = tanhf(kd);
            let gam = 1.0314 * p3(xt) - 1.9958 * p2(xt) + 1.5522 * xt + 0.1885;
            let gam = gam / 2.15;
            for ikhs in 1..=nkhs {
                let mut kh: Real = 0.;
                khs = khs + dhs;
                let (mut d, mut q) = (0., 0.);
                for _ in 1..=NKHI {
                    kh = kh + dkh;
                    let pr = (4. * kh / p2(khs)) * expf(-(2. * p2(kh / khs)));
                    let w = p.ssdsabk
                        * p2(khs / (sqrt2 * gam * xt))
                        * (1. - expf(-powf(kh / (gam * xt), p.ssdspbk)));
                    let eps = -(powf(p.ssdsbck / powf(xt, p.ssdshck) * kh, 3.) / 4.) * sqrtf(GRAV / xt);
                    d = d + pr * w * eps * dkh;
                    q = q + pr * w * dkh;
                }
                let at = ((ikhs - 1) + (ikd - 1) * nkhs) as usize;
                dcki[at] = d;
                qbi[at] = q;
            }
        }
        for q in qbi.iter_mut() {
            if *q > 1. {
                *q = 1.;
            }
        }
    }

    let mut cumulw = vec![0.; nspec * nspec];
    let mut dikcumul = 0;
    if p.ssdsc3 != 0. {
        dikcumul = nint(p.ssdsbrf1 / (p.xfr - 1.));
        for ik in 1..=nk {
            let c = GRAV / p.sig[ik as usize];
            for ith in 1..=nth {
                let is = ith + (ik - 1) * nth;
                for ik2 in 1..=ik - dikcumul {
                    let c2 = GRAV / p.sig[ik2 as usize];
                    for ith2 in 1..=nth {
                        let is2 = ith2 + (ik2 - 1) * nth;
                        let v = sqrtf(p2(c) + p2(c2) - 2. * c * c2 * p.ecos[(ith2 - ith).unsigned_abs() as usize])
                            * p.dsip[ik2 as usize]
                            / (0.5 * c2)
                            * p.dth;
                        cumulw[(is2 - 1) as usize + (is - 1) as usize * nspec] = v;
                    }
                }
            }
        }
    }
    Insin4Output { satindices, satweights, iktab, dcki, qbi, cumulw, dikcumul }
}
