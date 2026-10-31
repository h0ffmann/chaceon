// SPDX-License-Identifier: MPL-2.0
//! `ww3_strt` (`model/src/ww3_strt.F90`) for `switch_chaceon_shrd`: the initial conditions of a cold start.
//! Types 1 (Gaussian in space and frequency, cosine power in direction), 3 (fetch-limited wind, a dummy
//! restart), 4 (user spectrum) and 5 (calm) are translated; type 2 (JONSWAP) is not used by any regtest
//! and stops with a message. The output channels `O4`, `O5` and `O6` of this switch only print, and
//! `W3_DIST`, `W3_MPI`, `W3_WRST` and `W3_EXPORTWWM` are off, so the program is the `W3_SHRD` path.

#![allow(clippy::approx_constant, clippy::excessive_precision, clippy::needless_range_loop)]

use crate::disp::Dispersion;
use crate::grid::Metrics;
use crate::iors;
use crate::moddef::ModDef;
use ww3_kernels::constants::{DERA, TPI, TPIINV};
use ww3_kernels::nint;
use ww3_kernels::real::Real;
use ww3_libm::{expf, logf, sincosf};

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

/// `W3DIST` (`w3gsrumd.F90:5802-5877`): the `REAL(4)` entry converts to `REAL(8)`, `DIST_WITH_SINE` is on.
fn w3dist(llg: bool, xt: Real, yt: Real, xs: Real, ys: Real) -> Real {
    const PI: f64 = 3.14159265358979323846;
    const D2R: f64 = PI / 180.;
    const R2D: f64 = 1. / D2R;
    let (xt, yt, xs, ys) = (xt as f64, yt as f64, xs as f64, ys as f64);
    let mut dx = xt - xs;
    let dy = yt - ys;
    let d = if llg {
        if dx.abs() > 270. {
            dx -= 360f64.copysign(dx);
        }
        let slam = (0.5 * dx * D2R).sin();
        let sphi = (0.5 * dy * D2R).sin();
        let argd = (f64::cos(yt * D2R) * f64::cos(ys * D2R) * slam * slam + sphi * sphi).sqrt();
        R2D * 2. * argd.asin()
    } else {
        (dx * dx + dy * dy).sqrt()
    };
    d as Real
}

/// The text of `ww3_strt.inp` as `NEXTLN` and list-directed `READ` see it.
struct Inp {
    lines: Vec<String>,
    at: usize,
    comstr: char,
}

impl Inp {
    fn open(path: &str) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("ww3_strt.inp: {e}"));
        let lines: Vec<String> = text.lines().map(str::to_string).collect();
        assert!(!lines.is_empty(), "ww3_strt.inp: premature end of input file");
        // READ (NDSI,'(A)') COMSTR; IF (COMSTR.EQ.' ') COMSTR = '$'
        let comstr = match lines[0].chars().next() {
            None | Some(' ') => '$',
            Some(c) => c,
        };
        Self { lines, at: 1, comstr }
    }

    /// `NEXTLN`: skip comment and blank lines, stay on the next one.
    fn nextln(&mut self) {
        loop {
            let line = self.lines.get(self.at).unwrap_or_else(|| panic!("NEXTLN: premature end of input file"));
            let t = line.trim_start_matches([' ', '\t']);
            if t.starts_with(self.comstr) || t.trim().is_empty() {
                self.at += 1;
            } else {
                return;
            }
        }
    }

    /// `READ (NDSI,*) a, b, ...` of `n` items: tokens run on to the next records until `n` are read, and
    /// the rest of the last record is dropped.
    fn list(&mut self, n: usize) -> Vec<String> {
        let mut out = Vec::new();
        while out.len() < n {
            let line = self.lines.get(self.at).unwrap_or_else(|| panic!("ww3_strt.inp: end of file in a list-directed READ"));
            self.at += 1;
            for t in line.split([' ', '\t', ',']).filter(|t| !t.is_empty()) {
                assert!(
                    !t.contains(['/', '*', '\'', '"']),
                    "ww3_strt.inp: list-directed forms with a slash, a repeat count or a string are not implemented"
                );
                if out.len() < n {
                    out.push(t.to_string());
                }
            }
        }
        out
    }

    fn real(&mut self, n: usize) -> Vec<Real> {
        self.list(n)
            .iter()
            .map(|t| t.replace(['d', 'D'], "e").parse::<Real>().unwrap_or_else(|_| panic!("ww3_strt.inp: '{t}' is not a real")))
            .collect()
    }
}

/// `COS`; glibc's `cosf` and `sincosf` share their kernels.
fn cosf(x: Real) -> Real {
    sincosf(x).1
}

/// Reads `mod_def.ww3` and `ww3_strt.inp` from `dir` and writes `restart.ww3` there.
pub fn run(dir: &str) {
    let m = ModDef::read(&format!("{dir}/mod_def.ww3"));
    let mut inp = Inp::open(&format!("{dir}/ww3_strt.inp"));
    assert!(m.gtype == 1 || m.gtype == 2, "ww3_strt: only rectilinear and curvilinear grids are implemented");
    let (nk, nth, nsea) = (m.nk as usize, m.nth as usize, m.nsea as usize);
    let ny = m.ny as usize;
    let nspec = nk * nth;
    let dmin = m.dmin;

    inp.nextln();
    let itype = inp.list(1)[0].parse::<i32>().unwrap_or_else(|_| panic!("ww3_strt.inp: ITYPE is not an integer"));
    assert!((1..=5).contains(&itype), "W3STRT: ITYPE = {itype} is not 1 to 5");
    assert!(itype != 2, "ww3_strt: ITYPE = 2 (JONSWAP) is not implemented: no regtest uses it");

    // VA(NSPEC, 0:NSEA), as iors lays it out.
    let mut va = vec![0.; nspec * (nsea + 1)];
    let kind;
    let mut log = format!("ITYPE={itype}");
    match itype {
        1 => {
            kind = "COLD";
            inp.nextln();
            let v = inp.list(9);
            let rd = |i: usize| v[i].replace(['d', 'D'], "e").parse::<Real>().unwrap_or_else(|_| panic!("ww3_strt.inp: '{}' is not a real", v[i]));
            let (mut fp, mut sip, mut thm) = (rd(0), rd(1), rd(2));
            let ncos = v[3].parse::<i32>().unwrap_or_else(|_| panic!("ww3_strt.inp: NCOS '{}' is not an integer", v[3]));
            let (xm, mut six, ym, mut siy, hmax) = (rd(4), rd(5), rd(6), rd(7), rd(8));
            let sig = |ik: usize| m.sig[ik];

            fp = (0.5 * TPIINV * sig(1)).max(fp);
            sip = sip.max(0.);
            while thm < 0. {
                thm += 360.;
            }
            thm %= 360.;
            let ncos = 0.max(2 * (ncos / 2));

            let nosix = six < 0.;
            let met = Metrics::new(&m);
            // HPFAC(IY,IX), XGRD(IY,IX) at the sea point's IX = MAPSF(ISEA,1), IY = MAPSF(ISEA,2)
            let sea_ixy = |isea: usize| {
                let (ix, iy) = (m.mapsf[isea - 1] as usize, m.mapsf[isea - 1 + nsea] as usize);
                (iy - 1) + (ix - 1) * ny
            };
            let mut hpqmax: Real = -999.0;
            for isea in 1..=nsea {
                let h = met.hpfac[sea_ixy(isea)];
                if h > hpqmax {
                    hpqmax = h;
                }
            }
            let six_floor = 0.01 * hpqmax;
            let six_floored = six_floor > six;
            six = six_floor.max(six);
            hpqmax = -999.0;
            for isea in 1..=nsea {
                let h = met.hqfac[sea_ixy(isea)];
                if h > hpqmax {
                    hpqmax = h;
                }
            }
            let siy_floor = 0.01 * hpqmax;
            let siy_floored = siy_floor > siy;
            siy = siy_floor.max(siy);
            let hmax = hmax.max(0.);

            fp *= TPI;
            sip *= TPI;
            thm = (630. - thm) % 360. * DERA;

            // 4.b 1-D spectrum
            let chsip = 0.1 * m.dsip[1];
            let flone = sip < chsip;
            let ikm = nint(1. + (logf(fp) - logf(m.fr1 * TPI)) / logf(m.xfr));
            let ikm = 1.max(ikm.min(nk as i32));
            let mut e1 = vec![0.; nk + 1];
            for ik in 1..=nk {
                e1[ik] = if flone {
                    if ik as i32 == ikm { 1. } else { 0. }
                } else {
                    let frrel = (sig(ik) - fp) / sip;
                    if frrel.abs() < 10. { expf(-0.125 * (frrel * frrel)) } else { 0. }
                };
            }

            // 4.c directional distribution
            let flone_dir = ncos > 20;
            let ithm = 1 + nint(thm / m.dth);
            let mut dd = vec![0.; nth + 1];
            for ith in 1..=nth {
                dd[ith] = if flone_dir {
                    if ith as i32 == ithm { 1. } else { 0. }
                } else {
                    powi(cosf(m.th[ith - 1] - thm).max(0.), ncos)
                };
            }

            // 4.d 2-D energy spectrum
            let mut e2 = vec![0.; nspec];
            let mut etot: Real = 0.;
            for ik in 1..=nk {
                let mut e1i: Real = 0.;
                for ith in 1..=nth {
                    let c = (ith - 1) + (ik - 1) * nth;
                    e2[c] = e1[ik] * dd[ith];
                    e1i += e2[c];
                }
                etot += e1i * m.dsip[ik];
            }
            etot *= m.dth;
            let factor = (hmax * hmax) / (16. * etot);
            for e in &mut e2 {
                *e *= factor;
            }

            // 4.e distribute over the grid
            let mut far = 0usize;
            for isea in 1..=nsea {
                let c = sea_ixy(isea);
                let (x, y) = (m.xgrd[c], m.ygrd[c]);
                let rdsqr = if nosix {
                    let d = w3dist(m.flagll, x, y, xm, ym) / siy;
                    d * d
                } else {
                    let a = (x - xm) / six;
                    let b = (y - ym) / siy;
                    a * a + b * b
                };
                let factor = if rdsqr > 40. {
                    far += 1;
                    0.
                } else {
                    expf(-0.5 * rdsqr)
                };
                for (i, e) in e2.iter().enumerate() {
                    va[i + nspec * isea] = factor * e;
                }
            }
            log += &format!(
                " nosix={nosix} frequency={} direction={} six_from_grid={six_floored} siy_from_grid={siy_floored} points_beyond_40={far}/{nsea}",
                if flone { "single-bin" } else { "gaussian" },
                if flone_dir { "single-bin" } else { "cosine-power" }
            );
        }
        3 => kind = "WIND",
        4 => {
            kind = "COLD";
            inp.nextln();
            let mut facs = inp.real(1)[0];
            if facs <= 0. {
                facs = 1.;
            }
            inp.nextln();
            // FINP(IK,ITH), IK innermost
            let finp = inp.real(nspec);
            for ith in 1..=nth {
                for ik in 1..=nk {
                    let f = finp[(ik - 1) + (ith - 1) * nk] * facs / TPI;
                    for isea in 1..=nsea {
                        va[(ith - 1) + (ik - 1) * nth + nspec * isea] = f;
                    }
                }
            }
            log += &format!(" facs={facs}");
        }
        _ => kind = "CALM",
    }

    // 9. E(sigma) to N(k)
    if itype != 3 && itype != 5 {
        let disp = Dispersion::new();
        for isea in 1..=nsea {
            let depth = dmin.max(-m.zb[isea - 1]);
            for ik in 1..=nk {
                let (_, cg) = disp.wavnu1(m.sig[ik], depth);
                for ith in 1..=nth {
                    let c = (ith - 1) + (ik - 1) * nth + nspec * isea;
                    va[c] = va[c] * cg / m.sig[ik];
                }
            }
        }
    }
    iors::write_start(&format!("{dir}/restart.ww3"), &m, kind, &va);
    println!("ww3_strt: {log} -> restart.ww3 {kind}");
}
