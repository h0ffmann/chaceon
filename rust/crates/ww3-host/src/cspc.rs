// SPDX-License-Identifier: MPL-2.0
//! `W3CSPC` (`model/src/w3cspcmd.F90:143-604` at `92e2cf66`): a spectrum carried to another spectral grid, keeping the
//! energy of each bin. The tables `W3CSPC` caches per grid pair are built once by `new`, and `apply` is its section 3.

use ww3_kernels::constants::TPI;
use ww3_kernels::real::Real;

pub struct Cspc {
    nth1: usize,
    nfr1: usize,
    nth2: usize,
    nfr2: usize,
    dth1: Real,
    dth2: Real,
    frq1: Vec<Real>,
    frq2: Vec<Real>,
    xdf1: Real,
    xdf2: Real,
    /// `NFR2T`, from 1.
    nfr2t: usize,
    /// `IDTH(1:IDTH(0,I),I)` from 1 with `RDTH`, per `I`.
    th: Vec<Vec<(usize, Real)>>,
    /// `IDFR(1:IDFR(0,I),I)` from 1 with `RDFR`, per `I`.
    fr: Vec<Vec<(usize, Real)>>,
    ftl: Real,
}

impl Cspc {
    #[allow(clippy::too_many_arguments)]
    pub fn new(nfr1: usize, nth1: usize, xf1: Real, fr1: Real, th1: Real, nfr2: usize, nth2: usize, xf2: Real, fr2: Real, th2: Real, ftl: Real) -> Self {
        if nfr1 < 3 || nth1 < 4 || xf1 <= 1. || fr1 <= 0. || nfr2 < 3 || nth2 < 4 || xf2 <= 1. || fr2 <= 0. {
            println!("\n *** ERROR W3CSPC: ILLEGAL INPUT PARAMETERS ***");
            std::process::exit(1);
        }
        let dth1 = TPI / nth1 as Real;
        let dth2 = TPI / nth2 as Real;
        let nrmax = if dth1 <= dth2 { 2 } else { 2 + (dth1 / dth2) as i32 };
        let mut th = vec![Vec::new(); nth1];
        for (i, t) in th.iter_mut().enumerate() {
            let low = th1 + i as Real * dth1 - 0.5 * dth1;
            let hgh = low + dth1;
            let rlow = 1. + (low - th2) / dth2;
            for j in rlow.round() as i32..=rlow.round() as i32 + nrmax - 1 {
                let blow = th2 + (j - 1) as Real * dth2 - 0.5 * dth2;
                let bhgh = blow + dth2;
                let frac = (bhgh.min(hgh) - blow.max(low)) / (hgh - low);
                if frac > 1.0e-5 {
                    t.push(((1 + (j - 1 + nth2 as i32) % nth2 as i32) as usize, frac));
                }
            }
        }
        let mut frq1 = vec![fr1; nfr1];
        for i in 1..nfr1 {
            frq1[i] = xf1 * frq1[i - 1];
        }
        let mut frq2 = vec![fr2; nfr2];
        for i in 1..nfr2 {
            frq2[i] = xf2 * frq2[i - 1];
        }
        let xdf1 = 0.5 * (xf1 - 1. / xf1);
        let xdf2 = 0.5 * (xf2 - 1. / xf2);
        let mut fr = vec![Vec::new(); nfr1];
        // HGH stays at its last bin for the search of the tail.
        let mut hgh: Real = 0.;
        for (i, f) in fr.iter_mut().enumerate() {
            let low;
            if i == 0 {
                hgh = 0.5 * (frq1[i] + frq1[i + 1]);
                low = hgh - xdf1 * frq1[i];
            } else {
                low = 0.5 * (frq1[i] + frq1[i - 1]);
                hgh = low + xdf1 * frq1[i];
            }
            for j in 0..nfr2 {
                let (blow, bhgh);
                if j == 0 {
                    bhgh = 0.5 * (frq2[j] + frq2[j + 1]);
                    blow = bhgh - xdf2 * frq2[j];
                } else {
                    blow = 0.5 * (frq2[j] + frq2[j - 1]);
                    bhgh = blow + xdf2 * frq2[j];
                }
                if bhgh <= low {
                    continue;
                }
                if blow >= hgh {
                    break;
                }
                let frac = (bhgh.min(hgh) - blow.max(low)) / (hgh - low);
                if frac < 1.0e-5 {
                    continue;
                }
                f.push((j + 1, frac));
            }
        }
        let mut nfr2t = nfr2 + 1;
        for j in (0..nfr2).rev() {
            let bhgh = if j == 0 { 0.5 * (frq2[j] + frq2[j + 1]) } else { 0.5 * (frq2[j] + frq2[j - 1]) + xdf2 * frq2[j] };
            if bhgh > hgh {
                nfr2t = j + 1;
            } else {
                break;
            }
        }
        Self { nth1, nfr1, nth2, nfr2, dth1, dth2, frq1, frq2, xdf1, xdf2, nfr2t, th, fr, ftl }
    }

    /// `SP1(NTH1,NFR1,NSP)` to `SP2(NTH2,NFR2,NSP)`.
    pub fn apply(&self, sp1: &[Real], sp2: &mut [Real], nsp: usize) {
        let (n1, n2) = (self.nth1 * self.nfr1, self.nth2 * self.nfr2);
        sp2[..n2 * nsp].fill(0.);
        for i2 in 0..self.nfr1 {
            for &(j2, r2) in &self.fr[i2] {
                for i1 in 0..self.nth1 {
                    for &(j1, r1) in &self.th[i1] {
                        let frac = r2 * self.frq1[i2] * self.xdf1 * r1 * self.dth1;
                        let (to, from) = (j1 - 1 + (j2 - 1) * self.nth2, i1 + i2 * self.nth1);
                        for isp in 0..nsp {
                            sp2[to + n2 * isp] = sp2[to + n2 * isp] + frac * sp1[from + n1 * isp];
                        }
                    }
                }
            }
        }
        for j2 in 0..self.nfr2 {
            for j1 in 0..self.nth2 {
                let fact = 1. / (self.frq2[j2] * self.xdf2 * self.dth2);
                for isp in 0..nsp {
                    let to = j1 + j2 * self.nth2 + n2 * isp;
                    sp2[to] = fact * sp2[to];
                }
            }
        }
        for j2 in self.nfr2t..=self.nfr2 {
            for j1 in 0..self.nth2 {
                for isp in 0..nsp {
                    sp2[j1 + (j2 - 1) * self.nth2 + n2 * isp] = self.ftl * sp2[j1 + (j2 - 2) * self.nth2 + n2 * isp];
                }
            }
        }
    }
}
