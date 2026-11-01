// SPDX-License-Identifier: MPL-2.0
//! `W3PARTMD` (`model/src/w3partmd.F90` at `92e2cf66`): `W3PART`, `PTSORT`, `PTNGHB`, `PT_FLD` and `PTMEAN`, the
//! partitioning of one spectrum. Arrays that the Fortran indexes from 1 keep a spare element 0 here, so the
//! labels, the queue and the neighbour table read as the Fortran does.

use crate::moddef::ModDef;
use ww3_kernels::constants::{DERA, RADE, TPI, TPIINV};
use ww3_kernels::real::Real;
use ww3_libm::{atan2f, logf, sincosf, sqrtf};

/// `W3ODATMD`'s `DIMP`: the values kept per partition.
pub const DIMP: usize = 15;
/// `UNDEF` of `CONSTANTS`.
pub const UNDEF: Real = -999.9;

/// The `PTNGHB` table, for one `NK` and `NTH`. `NEIGH(9,N)` holds the count.
pub struct Neigh {
    n: Vec<[i32; 9]>,
}

impl Neigh {
    /// `PTNGHB`, built once; the Fortran keeps it in `NEIGH` and rebuilds it only when `NK` or `NTH` change.
    pub fn new(nk: usize, nth: usize) -> Self {
        let nspec = nk * nth;
        let mut tab = vec![[0i32; 9]; nspec + 1];
        let (nk_, nth_, nspec_) = (nk as i32, nth as i32, nspec as i32);
        for n in 1..=nspec_ {
            let j = (n - 1) / nk_ + 1;
            let i = n - (j - 1) * nk_;
            let mut k = 0usize;
            let e = &mut tab[n as usize];
            let mut add = |e: &mut [i32; 9], v: i32| {
                e[k] = v;
                k += 1;
            };
            if i != 1 {
                add(e, n - 1);
            }
            if i != nk_ {
                add(e, n + 1);
            }
            if j != 1 {
                add(e, n - nk_);
            }
            if j == 1 {
                add(e, nspec_ - (nk_ - i));
            }
            if j != nth_ {
                add(e, n + nk_);
            }
            if j == nth_ {
                add(e, n - (nth_ - 1) * nk_);
            }
            if i != 1 && j != 1 {
                add(e, n - nk_ - 1);
            }
            if i != 1 && j == 1 {
                add(e, n - 1 + nk_ * (nth_ - 1));
            }
            if i != nk_ && j != 1 {
                add(e, n - nk_ + 1);
            }
            if i != nk_ && j == 1 {
                add(e, n + 1 + nk_ * (nth_ - 1));
            }
            if i != 1 && j != nth_ {
                add(e, n + nk_ - 1);
            }
            if i != 1 && j == nth_ {
                add(e, n - 1 - nk_ * (nth_ - 1));
            }
            if i != nk_ && j != nth_ {
                add(e, n + nk_ + 1);
            }
            if i != nk_ && j == nth_ {
                add(e, n + 1 - nk_ * (nth_ - 1));
            }
            e[8] = k as i32;
        }
        Self { n: tab }
    }
}

/// What the partitioning reads of the grid, from `mod_def.ww3`.
pub struct Part<'a> {
    pub m: &'a ModDef,
    pub neigh: Neigh,
    /// `WAVNU1`.
    pub wavnu: &'a dyn Fn(Real, Real) -> (Real, Real),
}

/// `TPIINV**3` as gfortran folds a literal power of a parameter: one rounding of the exact cube.
fn cube_once(x: Real) -> Real {
    let b = x.to_bits();
    let (e, mant) = (((b >> 23) & 0xff) as i32, (b & 0x7fffff) | 0x800000);
    assert!(e != 0 && e != 255 && b >> 31 == 0);
    let c = (mant as u128) * (mant as u128) * (mant as u128);
    let bits = 128 - c.leading_zeros() as i32;
    let shift = bits - 24;
    let mut q = c >> shift;
    let rem = c & ((1u128 << shift) - 1);
    let half = 1u128 << (shift - 1);
    if rem > half || (rem == half && (q & 1) == 1) {
        q += 1;
    }
    // value = c * 2^(3*(e-127-23)); q * 2^shift = c
    let exp = 3 * (e - 127 - 23) + shift;
    (q as f64 * 2f64.powi(exp)) as Real
}

/// `PT_FLD`'s circular queue (`FIFO_ADD`, `FIFO_EMPTY`, `FIFO_FIRST`), `NSPEC` long.
struct Fifo {
    iq: Vec<i32>,
    start: usize,
    end: usize,
    n: usize,
}

impl Fifo {
    fn add(&mut self, iv: i32) {
        self.iq[self.end] = iv;
        self.end += 1;
        if self.end > self.n {
            self.end = 1;
        }
    }
    fn empty(&self) -> bool {
        self.start == self.end
    }
    fn first(&mut self) -> i32 {
        let v = self.iq[self.start];
        self.start += 1;
        if self.start > self.n {
            self.start = 1;
        }
        v
    }
}

/// `x**5` as gfortran expands a literal power: its power tree, then `1/` for the negative exponent.
fn inv_pow5(x: Real) -> Real {
    let x2 = x * x;
    let x3 = x * x2;
    1. / (x2 * x3)
}

fn sign(a: Real, b: Real) -> Real {
    a.copysign(b)
}

/// `W3PART`'s outputs: the count and `XP(DIMP,0:DIMXP)`, column `ip` at `ip * DIMP`.
pub struct Parts {
    pub np: i32,
    pub xp: Vec<Real>,
}

impl Parts {
    fn new(dimxp: usize) -> Self {
        Self { np: 0, xp: vec![0.; DIMP * (dimxp + 1)] }
    }
    pub fn col(&self, ip: usize) -> &[Real] {
        &self.xp[ip * DIMP..(ip + 1) * DIMP]
    }
}

impl Part<'_> {
    /// `W3PART ( SPEC, UABS, UDIR, DEPTH, WN, NP, XP, DIMXP )`. `spec` is `SPEC(NK,NTH)`, `wn` is `WN(1:NK)`.
    pub fn w3part(&self, spec: &[Real], uabs: Real, udir: Real, depth: Real, wn: &[Real], dimxp: usize) -> Parts {
        let m = self.m;
        let (nk, nth) = (m.nk as usize, m.nth as usize);
        let nspec = nk * nth;
        let mut out = Parts::new(dimxp);
        let mut zp = vec![0.; nspec + 1];
        for ith in 0..nth {
            for ik in 0..nk {
                zp[1 + ith * nk + ik] = spec[ik + ith * nk];
            }
        }
        let mut imo = vec![0i32; nspec + 1];
        let mut pmap = vec![0i32; dimxp + 1];

        if m.ptmeth == 4 {
            for ik in 1..=nk {
                for ith in 1..=nth {
                    let isp = ik + (ith - 1) * nk;
                    let upar = m.wsmult * uabs * (0.0 as Real).max(sincosf(m.th[ith - 1] - DERA * udir).1);
                    let c = m.sig[ik] / wn[ik - 1];
                    imo[isp] = if upar <= c { 2 } else { 1 };
                }
            }
            self.ptmean(2, &imo, &zp, depth, uabs, udir, wn, dimxp, &mut out, &mut pmap);
            return out;
        }
        if m.ptmeth == 5 {
            let sigcut = TPI * m.ptfcut;
            for ik in 1..=nk {
                let ip = if m.sig[ik] <= sigcut { 2 } else { 1 };
                for ith in 1..=nth {
                    imo[ik + (ith - 1) * nk] = ip;
                }
            }
            self.ptmean(2, &imo, &zp, depth, uabs, udir, wn, dimxp, &mut out, &mut pmap);
            return out;
        }

        let zmin = zp[1..].iter().copied().fold(zp[1], Real::min);
        let zmax = zp[1..].iter().copied().fold(zp[1], Real::max);
        if zmax - zmin < 1.0e-9 {
            return out;
        }
        let ihmax = m.ihmax;
        let fact = (ihmax - 1) as Real / (zmax - zmin);
        let mut imi = vec![0i32; nspec + 1];
        for n in 1..=nspec {
            let z = zmax - zp[n];
            imi[n] = (1. + z * fact).round().min(ihmax as Real).max(1.) as i32;
        }
        let ind = ptsort(&imi, ihmax as usize, nspec);
        let mut np_max = self.pt_fld(&imi, &ind, &mut imo, &zp);
        self.ptmean(np_max, &imo, &zp, depth, uabs, udir, wn, dimxp, &mut out, &mut pmap);
        if out.np > 0 && m.ptmeth == 2 {
            let mut wind_part = np_max;
            for ik in 1..=nk {
                for ith in 1..=nth {
                    let isp = ik + (ith - 1) * nk;
                    let upar = m.wsmult * uabs * (0.0 as Real).max(sincosf(m.th[ith - 1] - DERA * udir).1);
                    let c = m.sig[ik] / wn[ik - 1];
                    if c < upar {
                        wind_part = np_max + 1;
                        imo[isp] = wind_part;
                    }
                }
            }
            if wind_part != np_max {
                np_max = wind_part;
                self.ptmean(np_max, &imo, &zp, depth, uabs, udir, wn, dimxp, &mut out, &mut pmap);
            }
        }
        if out.np <= 1 {
            return out;
        }
        let np = out.np as usize;

        // TP(:,1:NP) = XP(:,1:NP), then XP(:,1:NP) = 0.
        let mut tp: Vec<[Real; DIMP]> = vec![[0.; DIMP]; np + 1];
        let take = |out: &mut Parts, tp: &mut Vec<[Real; DIMP]>, np: usize| {
            for ip in 1..=np {
                tp[ip].copy_from_slice(out.col(ip));
                out.xp[ip * DIMP..(ip + 1) * DIMP].fill(0.);
            }
        };
        // MAXLOC(TP(row,1:NP)): the first maximum.
        let maxloc = |tp: &Vec<[Real; DIMP]>, row: usize, np: usize| {
            let mut it = 1;
            for i in 2..=np {
                if tp[i][row] > tp[it][row] {
                    it = i;
                }
            }
            it
        };
        take(&mut out, &mut tp, np);

        if m.ptmeth == 3 {
            for ip in 1..=np {
                let it = maxloc(&tp, 0, np);
                out.xp[ip * DIMP..(ip + 1) * DIMP].copy_from_slice(&tp[it]);
                tp[it][0] = -1.;
            }
            return out;
        }

        let mut index = vec![0usize; np + 1];
        let mut nws = 0;
        for ip in 1..=np {
            let it = maxloc(&tp, 5, np);
            index[ip] = it;
            out.xp[ip * DIMP..(ip + 1) * DIMP].copy_from_slice(&tp[it]);
            if tp[it][5] >= m.wscut {
                nws += 1;
            }
            tp[it][5] = -1.;
        }
        let mut np = np;
        if nws > 1 && m.flcomb {
            let ipw = pmap[index[1]];
            for ip in 2..=nws {
                let ipt = pmap[index[ip]];
                for isp in 1..=nspec {
                    if imo[isp] == ipt {
                        imo[isp] = ipw;
                    }
                }
            }
            self.ptmean(np_max, &imo, &zp, depth, uabs, udir, wn, dimxp, &mut out, &mut pmap);
            if out.np <= 1 {
                return out;
            }
            np = out.np as usize;
            if tp.len() < np + 1 {
                tp.resize(np + 1, [0.; DIMP]);
            }
            take(&mut out, &mut tp, np);
            index = vec![0usize; np + 1];
            nws = 0;
            for ip in 1..=np {
                let it = maxloc(&tp, 5, np);
                index[ip] = it;
                out.xp[ip * DIMP..(ip + 1) * DIMP].copy_from_slice(&tp[it]);
                if tp[it][5] >= m.wscut {
                    nws += 1;
                }
                tp[it][5] = -1.;
            }
        }
        nws = nws.min(1);
        take(&mut out, &mut tp, np);
        if nws > 0 {
            out.xp[DIMP..2 * DIMP].copy_from_slice(&tp[1]);
            tp[1][0] = -1.;
            nws = 1;
        }
        for ip in nws + 1..=np {
            let it = maxloc(&tp, 0, np);
            out.xp[ip * DIMP..(ip + 1) * DIMP].copy_from_slice(&tp[it]);
            tp[it][0] = -1.;
        }
        out
    }

    /// `PT_FLD`: the watershed on the quantised spectrum. Returns `NPART`.
    fn pt_fld(&self, imi: &[i32], ind: &[i32], imo: &mut [i32], zp: &[Real]) -> i32 {
        let nspec = self.m.nk as usize * self.m.nth as usize;
        let neigh = &self.neigh.n;
        const MASK: i32 = -2;
        const INIT: i32 = -1;
        const IWSHED: i32 = 0;
        const IFICT: i32 = -100;
        imo.fill(INIT);
        let mut ic_label = 0;
        let mut imd = vec![0i32; nspec + 1];
        let mut q = Fifo { iq: vec![0i32; nspec + 1], start: 1, end: 1, n: nspec };
        let zpmax = zp[1..].iter().copied().fold(zp[1], Real::max);
        let mut m = 1usize;
        for ih in 1..=self.m.ihmax {
            let msave = m;
            loop {
                let ip = ind[m] as usize;
                if imi[ip] != ih {
                    break;
                }
                imo[ip] = MASK;
                for i in 0..neigh[ip][8] as usize {
                    let ipp = neigh[ip][i] as usize;
                    if imo[ipp] > 0 || imo[ipp] == IWSHED {
                        imd[ip] = 1;
                        q.add(ip as i32);
                        break;
                    }
                }
                if m + 1 > nspec {
                    break;
                } else {
                    m += 1;
                }
            }
            let mut ic_dist = 1;
            q.add(IFICT);
            loop {
                let mut ip = q.first();
                if ip == IFICT {
                    if q.empty() {
                        break;
                    } else {
                        q.add(IFICT);
                        ic_dist += 1;
                        ip = q.first();
                    }
                }
                let ip = ip as usize;
                for i in 0..neigh[ip][8] as usize {
                    let ipp = neigh[ip][i] as usize;
                    if imd[ipp] < ic_dist && (imo[ipp] > 0 || imo[ipp] == IWSHED) {
                        if imo[ipp] > 0 {
                            if imo[ip] == MASK || imo[ip] == IWSHED {
                                imo[ip] = imo[ipp];
                            } else if imo[ip] != imo[ipp] {
                                imo[ip] = IWSHED;
                            }
                        } else if imo[ip] == MASK {
                            imo[ip] = IWSHED;
                        }
                    } else if imo[ipp] == MASK && imd[ipp] == 0 {
                        imd[ipp] = ic_dist + 1;
                        q.add(ipp as i32);
                    }
                }
            }
            m = msave;
            loop {
                let ip = ind[m] as usize;
                if imi[ip] != ih {
                    break;
                }
                imd[ip] = 0;
                if imo[ip] == MASK {
                    ic_label += 1;
                    q.add(ip as i32);
                    imo[ip] = ic_label;
                    while !q.empty() {
                        let ipp = q.first() as usize;
                        for i in 0..neigh[ipp][8] as usize {
                            let ippp = neigh[ipp][i] as usize;
                            if imo[ippp] == MASK {
                                q.add(ippp as i32);
                                imo[ippp] = ic_label;
                            }
                        }
                    }
                }
                if m + 1 > nspec {
                    break;
                } else {
                    m += 1;
                }
            }
        }
        for _ in 1..=5 {
            let mut imd2 = imo.to_vec();
            for jl in 1..=nspec {
                let mut ipt = -1i32;
                if imo[jl] == 0 {
                    let mut ep1 = zpmax;
                    for jn in 1..=neigh[jl][8] as usize {
                        let nb = neigh[jl][jn - 1] as usize;
                        let diff = (zp[jl] - zp[nb]).abs();
                        if diff <= ep1 && imo[nb] != 0 {
                            ep1 = diff;
                            ipt = jn as i32;
                        }
                    }
                    if ipt > 0 {
                        imd2[jl] = imo[neigh[jl][ipt as usize - 1] as usize];
                    }
                }
            }
            imo.copy_from_slice(&imd2);
            if imo[1..].iter().copied().min().unwrap() > 0 {
                break;
            }
        }
        ic_label
    }

    /// `PTMEAN`: parameters of each partition into `XP`, the count into `NPO`, and `PMAP`.
    #[allow(clippy::too_many_arguments)]
    fn ptmean(
        &self,
        npi: i32,
        imo: &[i32],
        zp: &[Real],
        depth: Real,
        uabs: Real,
        udir: Real,
        wn: &[Real],
        dimxp: usize,
        out: &mut Parts,
        pmap: &mut [i32],
    ) {
        let m = self.m;
        let (nk, nth) = (m.nk as usize, m.nth as usize);
        let sig = &m.sig;
        out.np = 0;
        out.xp.fill(0.);
        if npi == 0 {
            return;
        }
        let npi = npi as usize;
        let nk2 = nk + 2;
        let fi = |ik: usize, ip: usize| ik + nk2 * ip;
        let gi = |ik: usize, ip: usize| (ik - 1) + nk * ip;
        let mut sumf = vec![0.; nk2 * (npi + 1)];
        let mut sumfw = vec![0.; nk * (npi + 1)];
        let mut sumfx = vec![0.; nk * (npi + 1)];
        let mut sumfy = vec![0.; nk * (npi + 1)];
        let mut sume = vec![0.; npi + 1];
        let mut sume1 = vec![0.; npi + 1];
        let mut sume2 = vec![0.; npi + 1];
        let mut sumem1 = vec![0.; npi + 1];
        let mut sumqp = vec![0.; npi + 1];
        let mut sumew = vec![0.; npi + 1];
        let mut sumex = vec![0.; npi + 1];
        let mut sumey = vec![0.; npi + 1];
        let mut ifpmax = vec![0usize; npi + 1];
        let mut efpmax = vec![0.; npi + 1];
        let mut c = vec![0.; nk + 1];
        for ik in 1..=nk {
            c[ik] = sig[ik] / wn[ik - 1];
        }
        let mut fcdir = vec![0.; nth + 1];
        for ith in 1..=nth {
            let upar = m.wsmult * uabs * (0.0 as Real).max(sincosf(m.th[ith - 1] - DERA * udir).1);
            if upar < c[nk] {
                fcdir[ith] = sig[nk + 1];
            } else {
                // DO IK=NK-1, 2, -1 leaves IK at 1 when the loop runs out.
                let mut ik = nk as i32 - 1;
                let mut found = false;
                while ik >= 2 {
                    if upar < c[ik as usize] {
                        found = true;
                        break;
                    }
                    ik -= 1;
                }
                if !found {
                    ik = if nk > 2 { 1 } else { nk as i32 - 1 };
                }
                let mut rd = (c[ik as usize] - upar) / (c[ik as usize] - c[ik as usize + 1]);
                if rd < 0. {
                    ik = 0;
                    rd = (0.0 as Real).max(rd + 1.);
                }
                fcdir[ith] = rd * sig[ik as usize + 1] + (1. - rd) * sig[ik as usize];
            }
        }
        for ik in 1..=nk {
            for ith in 1..=nth {
                let isp = ik + (ith - 1) * nk;
                let ip = imo[isp] as usize;
                let fact = (0.0 as Real).max((1. as Real).min(1. - (fcdir[ith] - 0.5 * (sig[ik - 1] + sig[ik])) / m.dsip[ik]));
                sumf[fi(ik, 0)] += zp[isp];
                sumfw[gi(ik, 0)] += zp[isp] * fact;
                sumfx[gi(ik, 0)] += zp[isp] * m.ecos[ith - 1];
                sumfy[gi(ik, 0)] += zp[isp] * m.esin[ith - 1];
                if ip == 0 {
                    continue;
                }
                sumf[fi(ik, ip)] += zp[isp];
                sumfw[gi(ik, ip)] += zp[isp] * fact;
                sumfx[gi(ik, ip)] += zp[isp] * m.ecos[ith - 1];
                sumfy[gi(ik, ip)] += zp[isp] * m.esin[ith - 1];
            }
        }
        for ip in 0..=npi {
            sumf[fi(nk + 1, ip)] = sumf[fi(nk, ip)] * m.fachfe;
        }
        for ip in 0..=npi {
            for ik in 1..=nk {
                let (f, d, s) = (sumf[fi(ik, ip)], m.dsii[ik - 1], sig[ik]);
                sume[ip] += f * d;
                sumqp[ip] += f * f * d * s;
                sume1[ip] += f * d * s;
                sume2[ip] += f * d * (s * s);
                sumem1[ip] += f * d / s;
                sumew[ip] += sumfw[gi(ik, ip)] * d;
                sumex[ip] += sumfx[gi(ik, ip)] * d;
                sumey[ip] += sumfy[gi(ik, ip)] * d;
                if f > efpmax[ip] {
                    ifpmax[ip] = ik;
                    efpmax[ip] = f;
                }
            }
            let fteii = m.fte / (m.dth * sig[nk]);
            let fnk = sumf[fi(nk, ip)];
            sume[ip] += fnk * fteii;
            sume1[ip] += fnk * sig[nk] * fteii * (0.3333 / 0.25);
            sume2[ip] += fnk * (sig[nk] * sig[nk]) * fteii * (0.5 / 0.25);
            sumem1[ip] += fnk / sig[nk] * fteii * (0.2 / 0.25);
            sumqp[ip] += fnk * fteii;
            sumew[ip] += sumfw[gi(nk, ip)] * fteii;
            sumex[ip] += sumfx[gi(nk, ip)] * fteii;
            sumey[ip] += sumfy[gi(nk, ip)] * fteii;
        }
        let tpiinv2 = TPIINV * TPIINV;
        let tpiinv3 = cube_once(TPIINV);
        let dth = m.dth;
        let mut npo = -1i32;
        for ip in 0..=npi {
            let m0 = sume[ip] * dth * TPIINV;
            let hs = 4. * sqrtf(m0.max(0.));
            if hs < m.hspmin {
                if m.ptmeth == 4 || m.ptmeth == 5 {
                    npo += 1;
                    let col = npo as usize * DIMP;
                    out.xp[col..col + DIMP].fill(UNDEF);
                    out.xp[col + 5] = 0.0;
                }
                continue;
            }
            if npo >= dimxp as i32 {
                // Label 2000: the Fortran prints the message and returns with NPO as it stands.
                println!("\n *** WAVEWATCH III ERROR IN PTMEAN :\n     XP ARRAY TOO SMALL AT PARTITION{:>6}\n", npo + 1);
                out.np = npo;
                return;
            }
            npo += 1;
            if ip > 0 {
                if npo < 1 {
                    continue;
                }
                pmap[npo as usize] = ip as i32;
            }
            let m1 = sume1[ip] * dth * tpiinv2;
            let m2 = sume2[ip] * dth * tpiinv3;
            let mm1 = sumem1[ip] * dth;
            let dq = dth * TPIINV;
            let qp = sumqp[ip] * (dq * dq);
            let xl = 1. / m.xfr - 1.;
            let xh = m.xfr - 1.;
            let (xl2, xh2) = (xl * xl, xh * xh);
            let ipk = ifpmax[ip];
            if ipk == 0 {
                eprintln!("ptmean: partition {ip} has no peak; the Fortran reads SUMF(-1,IP) there, which is not implemented");
                std::process::exit(64);
            }
            let el = sumf[fi(ipk - 1, ip)] - sumf[fi(ipk, ip)];
            let eh = sumf[fi(ipk + 1, ip)] - sumf[fi(ipk, ip)];
            let mut denom = xl * eh - xh * el;
            let mut sigp = sig[ipk];
            if denom != 0. {
                sigp *= 1. + 0.5 * (xl2 * eh - xh2 * el) / sign(denom.abs(), denom);
            }
            let (wnp, _cgp) = (self.wavnu)(sigp, depth);
            let ik = ipk;
            efpmax[ip] = sumf[fi(ik, ip)] * dth;
            if ik > 1 && ik < nk {
                let el = sumf[fi(ik - 1, ip)] * dth;
                let eh = sumf[fi(ik + 1, ip)] * dth;
                let numer = 0.125 * ((el - eh) * (el - eh));
                denom = el - 2. * efpmax[ip] + eh;
                if denom != 0. {
                    efpmax[ip] -= numer / sign(denom.abs(), denom);
                }
            }
            let mut fsprd = UNDEF;
            let (mut sumy, mut sumxy, mut sumxxy, mut sumylogy, mut sumxylogy) = (0., 0., 0., 0., 0.);
            for ik in 1..=nk {
                let y = sumf[fi(ik, ip)] * dth;
                if y >= 1.0e-15 {
                    let yhat = logf(y);
                    let q = (sig[ik] - sigp) * TPIINV;
                    let xhat = -0.5 * (q * q);
                    sumy += y;
                    sumxy += xhat * yhat;
                    sumxxy += xhat * xhat * y;
                    sumylogy += y * yhat;
                    sumxylogy += sumxy * yhat;
                }
            }
            let numer = sumy * sumxxy - sumxy * sumxy;
            let denom = sumy * sumxylogy - sumxy * sumylogy;
            if denom != 0. {
                fsprd = sqrtf(numer / sign(denom.abs(), numer));
            }
            let sumexp = sumfx[gi(ipk, ip)] * m.dsii[ipk - 1];
            let sumeyp = sumfy[gi(ipk, ip)] * m.dsii[ipk - 1];
            let col = npo as usize * DIMP;
            let x = &mut out.xp[col..col + DIMP];
            x[0] = hs;
            x[1] = TPI / sigp;
            x[2] = TPI / wnp;
            x[3] = (630. - atan2f(sumey[ip], sumex[ip]) * RADE) % 360.;
            x[4] = RADE
                * sqrtf((0.0 as Real).max(
                    2. * (1. - sqrtf((0.0 as Real).max((sumex[ip] * sumex[ip] + sumey[ip] * sumey[ip]) / (sume[ip] * sume[ip])))),
                ));
            x[5] = sumew[ip] / sume[ip];
            x[6] = (630. - atan2f(sumeyp, sumexp) * RADE) % 360.;
            x[7] = sqrtf((1. as Real).max(m2 * m0 / (m1 * m1)) - 1.);
            let sp2 = sigp * sigp;
            let alp_pm = 0.3125 * (hs * hs) * (sp2 * sp2);
            let epm_fp = alp_pm * TPI * inv_pow5(sigp) * 2.865048E-1;
            x[8] = (efpmax[ip] / epm_fp).max(1.0);
            x[9] = 2. * qp / (m0 * m0);
            x[10] = fsprd;
            x[11] = mm1 / m0;
            x[12] = m0 / m1;
            x[13] = sqrtf(m0 / m2);
            x[14] = efpmax[ip];
        }
        out.np = npo;
    }
}

/// `PTSORT`: the counting sort of the quantised spectrum.
fn ptsort(imi: &[i32], ihmax: usize, nspec: usize) -> Vec<i32> {
    let mut numv = vec![0i32; ihmax + 1];
    for i in 1..=nspec {
        numv[imi[i] as usize] += 1;
    }
    let mut iaddr = vec![0i32; ihmax + 1];
    iaddr[1] = 1;
    for i in 1..ihmax {
        iaddr[i + 1] = iaddr[i] + numv[i];
    }
    let mut iorder = vec![0i32; nspec + 1];
    for i in 1..=nspec {
        let iv = imi[i] as usize;
        let inn = iaddr[iv];
        iorder[i] = inn;
        iaddr[iv] = inn + 1;
    }
    let mut ind = vec![0i32; nspec + 1];
    for i in 1..=nspec {
        ind[iorder[i] as usize] = i as i32;
    }
    ind
}

/// `ICPRT` and `DTPRT` of `W3ODATMD`, as `W3CPRT` leaves them. Arrays over `JSEA` start at 1 (element 0 unused);
/// partition `i` of the storage (1-based) is `dt[(i - 1) * DIMP..i * DIMP]`.
pub struct Prt {
    pub cnt: Vec<i32>,
    pub start: Vec<usize>,
    pub dt: Vec<Real>,
}

/// What `W3CPRT` reads of the run. Per-point arrays follow `shel.rs`: from `ISEA = 0`, `NK + 2` values per
/// point for `CG` and `WN`, `NSPEC` per point for `VA`.
pub struct CprtInput<'a> {
    pub va: &'a [Real],
    pub cg: &'a [Real],
    pub wn: &'a [Real],
    pub dw: &'a [Real],
    pub u10: &'a [Real],
    pub u10d: &'a [Real],
    pub asf: &'a [Real],
    pub mapsta: &'a [i32],
}

/// `W3CPRT` (`model/src/w3iosfmd.F90:108-344`): partitions every sea point of this process. Single process, so
/// `JSEA` is `ISEA`.
pub fn w3cprt(p: &Part, g: &CprtInput) -> Prt {
    let m = p.m;
    let (nk, nth, nsea) = (m.nk as usize, m.nth as usize, m.nsea as usize);
    let nspec = nk * nth;
    let nk2 = nk + 2;
    let dimxp = if m.ptmeth == 4 || m.ptmeth == 5 { 2 } else { ((nk + 1) / 2) * ((nth - 1) / 2) };
    let mut prt = Prt { cnt: vec![0; nsea + 2], start: vec![0; nsea + 2], dt: Vec::new() };
    prt.start[1] = 1;
    let mut e2 = vec![0.; nk * nth];
    for jsea in 1..=nsea {
        let c = (m.mapsf[jsea - 1 + 2 * nsea] - 1) as usize;
        prt.start[jsea + 1] = prt.start[jsea];
        if g.mapsta[c] < 0 {
            continue;
        }
        let uabs = g.u10[jsea] * g.asf[jsea];
        let udir = g.u10d[jsea] * RADE;
        let depth = g.dw[jsea];
        for ik in 1..=nk {
            let fact = TPI * m.sig[ik] / g.cg[ik + nk2 * jsea];
            for ith in 1..=nth {
                e2[(ik - 1) + (ith - 1) * nk] = g.va[nspec * jsea + (ith - 1) + (ik - 1) * nth] * fact;
            }
        }
        if depth != depth {
            println!("IOSF:{jsea}");
            println!(" FOUND NaN in depth");
            println!("STOP CRITICAL ERROR IN DEPTH ARRAY");
            std::process::exit(0);
        }
        let wn = &g.wn[1 + nk2 * jsea..1 + nk2 * jsea + nk];
        let r = p.w3part(&e2, uabs, udir, depth, wn, dimxp);
        if r.np >= 0 {
            let np = r.np as usize;
            prt.cnt[jsea] = r.np + 1;
            prt.start[jsea + 1] = prt.start[jsea] + np + 1;
            prt.dt.extend_from_slice(&r.xp[..(np + 1) * DIMP]);
        }
    }
    if prt.dt.is_empty() {
        // DTPRT is allocated with MAX(1,FINSIZ) columns and set to 0.
        prt.dt = vec![0.; DIMP];
    }
    prt
}
