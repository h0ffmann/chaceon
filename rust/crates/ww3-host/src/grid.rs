// SPDX-License-Identifier: MPL-2.0
//! The grid maps and fields `W3WAVE` builds before it propagates, for a rectilinear grid: `W3MAP3`,
//! `W3MAPT` (`model/src/w3pro3md.F90`), `W3UTRN`, `W3DZXY` (`w3updtmd.F90`), and the metrics `W3GNTX`
//! gets from `W3CGDM` (`w3gsrumd.F90`), which WW3 calls with `SPHERE=.FALSE.`, `DX` and `DY`.

#![allow(clippy::approx_constant)]

use crate::moddef::ModDef;
use ww3_kernels::constants::{DERA, RADIUS};
use ww3_kernels::real::Real;
use ww3_libm::expf;

/// `MAPSTA` and `MAPST2`, unpacked from `mod_def.ww3`'s `MAPTMP`.
pub fn unpack(maptmp: &[i32]) -> (Vec<i32>, Vec<i32>) {
    let sta: Vec<i32> = maptmp.iter().map(|m| (m + 2) % 8 - 2).collect();
    let st2 = maptmp.iter().zip(&sta).map(|(m, s)| (m - s) / 8).collect();
    (sta, st2)
}

/// `DPDX DPDY DQDX DQDY GSQRT`, indexed by `IXY`. `W3CGDM` works in `REAL(8)`.
pub struct Metrics {
    pub dpdx: Vec<Real>,
    pub dpdy: Vec<Real>,
    pub dqdx: Vec<Real>,
    pub dqdy: Vec<Real>,
    pub gsqrt: Vec<Real>,
    pub hpfac: Vec<Real>,
    pub hqfac: Vec<Real>,
}

const PI: f64 = 3.14159265358979323846;
const D2R: f64 = PI / 180.;
const R2D: f64 = 1. / D2R;
const REARTH: f64 = 4.0e7 / (2. * PI);

/// `W3FDWT` with `M = 1`, `Z = 0`: the weights of a first derivative, `C(:,1)`.
fn fdwt(x: &[f64]) -> Vec<f64> {
    let n = x.len() - 1;
    let mut c = vec![[0f64; 2]; n + 1];
    let (mut c1, mut c4) = (1f64, x[0]);
    c[0][0] = 1.;
    for i in 1..=n {
        let mn = i.min(1);
        let (mut c2, c5) = (1f64, c4);
        c4 = x[i];
        for j in 0..i {
            let c3 = x[i] - x[j];
            c2 *= c3;
            if j == i - 1 {
                for k in (1..=mn).rev() {
                    c[i][k] = c1 * (k as f64 * c[i - 1][k - 1] - c5 * c[i - 1][k]) / c2;
                }
                c[i][0] = -c1 * c5 * c[i - 1][0] / c2;
            }
            for k in (1..=mn).rev() {
                c[j][k] = (c4 * c[j][k] - k as f64 * c[j][k - 1]) / c3;
            }
            c[j][0] = c4 * c[j][0] / c3;
        }
        c1 = c2;
    }
    c.iter().map(|v| v[1]).collect()
}

/// `W3SPLX` from a pole, `(LON0, LAT0) = (0, ±90)`.
fn splx(lat0: f64, c0: f64, lam: f64, phi: f64) -> (f64, f64) {
    let (clam, slam) = ((lam * D2R).cos(), (lam * D2R).sin());
    let (cphi0, cphi) = ((lat0 * D2R).cos(), (phi * D2R).cos());
    let (sphi0, sphi) = ((lat0 * D2R).sin(), (phi * D2R).sin());
    let k0 = (0.5 * c0 * D2R).cos();
    let k0 = k0 * k0;
    let k = 2. * k0 * REARTH / (1. + sphi0 * sphi + cphi0 * cphi * clam);
    (k * cphi * slam, k * (cphi0 * sphi - sphi0 * cphi * clam))
}

/// `SPDDP` and `SPDDQ`, which are the same arithmetic: `(DX,DY)` in the plane to `(DLAM,DPHI)`.
fn spdd(c0: f64, ihem: f64, lam: f64, phi: f64, d1: f64, d2: f64) -> (f64, f64) {
    let k0 = (0.5 * c0 * D2R).cos();
    let k0 = k0 * k0;
    let mu = lam * D2R;
    let a = ihem;
    let nu = PI / 4. - a * 0.5 * phi * D2R;
    let nu = nu.abs().max(1.0e-6).copysign(nu);
    let fac = R2D * 0.5 / REARTH / k0;
    let (cosmu, sinmu) = (mu.cos(), mu.sin());
    let cosnu2 = nu.cos();
    let cosnu2 = cosnu2 * cosnu2;
    let cotnu = 1. / nu.tan();
    let dlamdx = fac * cotnu * cosmu;
    let dlamdy = a * fac * cotnu * sinmu;
    let dphidx = -a * 2. * fac * cosnu2 * sinmu;
    let dphidy = 2. * fac * cosnu2 * cosmu;
    (d1 * dlamdx + d2 * dlamdy, d1 * dphidx + d2 * dphidy)
}

/// `DXYDP` or `DXYDQ` at grid point `(p, q)`, `along_p` picks the direction. `N` is 2 or 4.
fn dxyd(m: &ModDef, n: i32, along_p: bool, p: i32, q: i32) -> (f64, f64) {
    let (nx, ny) = (m.nx, m.ny);
    let at = |ix: i32, iy: i32| ((iy - 1) + (ix - 1) * ny) as usize;
    let (pos, range) = if along_p { (p, nx) } else { (q, ny) };
    let closed = along_p && m.iclose != -1;
    let i = if closed {
        n / 2
    } else if pos - 1 < n / 2 {
        pos - 1
    } else if range - pos < n / 2 {
        n + pos - range
    } else {
        n / 2
    };
    let (mut xp, mut yp, mut ks) = (Vec::new(), Vec::new(), Vec::new());
    for j in 0..=n {
        let mut k = pos + (j - i);
        if closed {
            k = (k - 1).rem_euclid(range) + 1;
        }
        let c = if along_p { at(k, q) } else { at(p, k) };
        xp.push(m.xgrd[c] as f64);
        yp.push(m.ygrd[c] as f64);
        ks.push((j - i) as f64);
    }
    let c = fdwt(&ks);
    let dot = |v: &[f64]| c.iter().zip(v).fold(0f64, |s, (a, b)| s + a * b);
    if !m.flagll {
        return (dot(&xp), dot(&yp));
    }
    let (x0, y0) = (m.xgrd[at(p, q)] as f64, m.ygrd[at(p, q)] as f64);
    let ihem = if yp.iter().cloned().fold(f64::MIN, f64::max) < 0. { -1. } else { 1. };
    let c0 = 90. - y0.abs();
    let (mut up, mut vp) = (Vec::new(), Vec::new());
    for l in 0..xp.len() {
        let (u, v) = splx(90f64.copysign(ihem), c0, xp[l], yp[l]);
        up.push(u);
        vp.push(v);
    }
    spdd(c0, ihem, x0, y0, dot(&up), dot(&vp))
}

impl Metrics {
    pub fn new(m: &ModDef) -> Self {
        if m.gtype == 2 {
            return Self::curvilinear(m);
        }
        let n = (m.nx * m.ny) as usize;
        let (dxdp, dydq) = (m.sx as f64, m.sy as f64);
        let (dxdq, dydp) = (0f64, 0f64);
        let g = (dxdp * dydq - dxdq * dydp).max(1e-15);
        Self {
            dpdx: vec![(dydq / g) as Real; n],
            dpdy: vec![(-dxdq / g) as Real; n],
            dqdx: vec![(-dydp / g) as Real; n],
            dqdy: vec![(dxdp / g) as Real; n],
            gsqrt: vec![g as Real; n],
            hpfac: vec![(dxdp * dxdp + dydp * dydp).max(1e-15).sqrt() as Real; n],
            hqfac: vec![(dxdq * dxdq + dydq * dydq).max(1e-15).sqrt() as Real; n],
        }
    }
}

impl Metrics {
    /// `W3CGDM` with `NFD = 4`, which falls back to 2 where the metric turns left-handed.
    fn curvilinear(m: &ModDef) -> Self {
        let n = (m.nx * m.ny) as usize;
        let mut g = Self {
            dpdx: vec![0.; n],
            dpdy: vec![0.; n],
            dqdx: vec![0.; n],
            dqdy: vec![0.; n],
            gsqrt: vec![0.; n],
            hpfac: vec![0.; n],
            hqfac: vec![0.; n],
        };
        for ix in 1..=m.nx {
            for iy in 1..=m.ny {
                let c = ((iy - 1) + (ix - 1) * m.ny) as usize;
                let (mut dxdp, mut dydp) = dxyd(m, 4, true, ix, iy);
                let (mut dxdq, mut dydq) = dxyd(m, 4, false, ix, iy);
                let mut gs = dxdp * dydq - dxdq * dydp;
                if gs < 0. {
                    (dxdp, dydp) = dxyd(m, 2, true, ix, iy);
                    (dxdq, dydq) = dxyd(m, 2, false, ix, iy);
                    gs = dxdp * dydq - dxdq * dydp;
                }
                assert!(gs >= 0., "W3CGDM: not a right-handed grid");
                let gs = gs.max(1.0e-15);
                g.dpdx[c] = (dydq / gs) as Real;
                g.dpdy[c] = (-dxdq / gs) as Real;
                g.dqdx[c] = (-dydp / gs) as Real;
                g.dqdy[c] = (dxdp / gs) as Real;
                g.gsqrt[c] = gs as Real;
                g.hpfac[c] = (dxdp * dxdp + dydp * dydp).max(1.0e-15).sqrt() as Real;
                g.hqfac[c] = (dxdq * dxdq + dydq * dydq).max(1.0e-15).sqrt() as Real;
            }
        }
        g
    }
}

/// `W3MAP3`'s output.
#[derive(Default)]
pub struct Maps {
    pub nmx: [i32; 3],
    pub nmy: [i32; 3],
    pub nact: i32,
    pub ncent: i32,
    pub mapx2: Vec<i32>,
    pub mapy2: Vec<i32>,
    pub mapaxy: Vec<i32>,
    pub mapcxy: Vec<i32>,
    pub mapth2: Vec<i32>,
    pub mapwn2: Vec<i32>,
}

/// `W3MAP3`. It reads `MAPSTA` one row past the edge for a sea point on the first or last row, which
/// is the next column in Fortran's storage, so this keeps that storage.
pub fn w3map3(m: &ModDef, mapsta: &[i32], mp: &mut Maps) {
    let (nx, ny, nsea) = (m.nx, m.ny, m.nsea);
    let sta = |iy: i32, ix: i32| mapsta.get(((iy - 1) + (ix - 1) * ny) as usize).copied().unwrap_or(0);
    let n = (nx * ny) as usize;
    (mp.mapx2, mp.mapy2, mp.mapaxy) = (vec![0; n], vec![0; n], vec![0; n]);
    mp.mapcxy = vec![0; nsea as usize];

    let mut c = 0;
    for pass in 0..3 {
        for ix in 1..=nx {
            let ix2 = 1 + ix % nx;
            for iy in 2..ny {
                let (a, b) = (sta(iy, ix) == 1, sta(iy, ix2) == 1);
                if [a && b, a && !b, !a && b][pass] {
                    mp.mapx2[c] = (ix - 1) * ny + iy;
                    c += 1;
                }
            }
        }
        mp.nmx[pass] = c as i32;
    }
    let mut c = 0;
    for pass in 0..3 {
        for ix in 1..=nx {
            for iy in 1..ny {
                let (a, b) = (sta(iy, ix) == 1, sta(iy + 1, ix) == 1);
                if [a && b, a && !b, !a && b][pass] {
                    mp.mapy2[c] = (ix - 1) * ny + iy;
                    c += 1;
                }
            }
        }
        mp.nmy[pass] = c as i32;
    }
    let mut c = 0;
    for ix in 1..=nx {
        for iy in 2..ny {
            if sta(iy, ix) == 1 {
                mp.mapaxy[c] = (ix - 1) * ny + iy;
                c += 1;
            }
        }
    }
    mp.nact = c as i32;

    let (mut ncent, mut ncentc) = (0, nsea);
    for isea in 1..=nsea {
        let i = (isea - 1) as usize;
        let (ix, iy) = (m.mapsf[i], m.mapsf[i + nsea as usize]);
        let (ix0, ix2) = (if ix == 1 { nx } else { ix - 1 }, if ix == nx { 1 } else { ix + 1 });
        let (iy0, iy2) = (iy - 1, iy + 1);
        let st = sta(iy, ix);
        let inner = sta(iy0, ix0) >= 1
            && sta(iy0, ix) >= 1
            && sta(iy0, ix2) >= 1
            && sta(iy, ix0) >= 1
            && sta(iy, ix2) >= 1
            && sta(iy2, ix0) >= 1
            && sta(iy2, ix) >= 1
            && sta(iy2, ix2) >= 1;
        if st != 2 && st >= 0 && inner {
            ncent += 1;
            mp.mapcxy[(ncent - 1) as usize] = isea;
        } else {
            mp.mapcxy[(ncentc - 1) as usize] = isea;
            ncentc -= 1;
        }
    }
    mp.ncent = ncent;

    if !mp.mapth2.is_empty() {
        return;
    }
    let (nk, nth) = (m.nk, m.nth);
    let nspec = (nk * nth) as usize;
    mp.mapth2 = vec![0; nspec];
    mp.mapwn2 = vec![0; nspec + nth as usize];
    for ik in 1..=nk {
        for ith in 1..=nth {
            mp.mapth2[(ith + (ik - 1) * nth - 1) as usize] = (ik + 1) + (ith - 1) * (nk + 2);
        }
    }
    let mut c = 0;
    for ik in 1..nk {
        for ith in 1..=nth {
            mp.mapwn2[c] = (ik + 1) + (ith - 1) * (nk + 2);
            c += 1;
        }
    }
    for ith in 1..=nth {
        mp.mapwn2[c] = nk + 1 + (ith - 1) * (nk + 2);
        c += 1;
    }
    for ith in 1..=nth {
        mp.mapwn2[c] = 1 + (ith - 1) * (nk + 2);
        c += 1;
    }
}

/// `W3UTRN`, for `IC0`. `ATRNX(NY*NX,-1:1)` is `atrnx[ixy-1 + (j+1)*NY*NX]`. `ice` and `berg` are by `ISEA`,
/// from 0.
pub fn w3utrn(m: &ModDef, g: &Metrics, mapsta: &[i32], ice: &[Real], berg: &[Real], atrnx: &mut [Real], atrny: &mut [Real]) {
    atrnx.fill(1.);
    atrny.fill(1.);
    if m.trflag == 0 {
        return;
    }
    let (nx, ny, nsea) = (m.nx, m.ny, m.nsea as usize);
    let n = (nx * ny) as usize;
    let (tx, ty) = (&m.trnx, &m.trny);
    let at = |i: i32| (i - 1) as usize;
    let sta = |iy: i32, ix: i32| mapsta[((iy - 1) + (ix - 1) * ny) as usize];
    for i in 0..nsea {
        let (ix, iy, ixy) = (m.mapsf[i], m.mapsf[i + nsea], m.mapsf[i + 2 * nsea]);
        let c = at(ixy);
        if m.trflag % 2 == 1 {
            atrnx[c] = if ix == 1 { tx[at(iy + (nx - 1) * ny)] } else { tx[at(ixy - ny)] };
            atrnx[c + 2 * n] = if ix == nx { tx[at(iy)] } else { tx[at(ixy)] };
            atrny[c] = ty[at(ixy - 1)];
            atrny[c + 2 * n] = ty[at(ixy)];
            continue;
        }
        let ixn = if ix == 1 { iy + (nx - 1) * ny } else { iy + (ix - 2) * ny };
        let ixp = if ix == nx { iy } else { iy + ix * ny };
        let iyn = if iy == 1 { ixy } else { ixy - 1 };
        let iyp = if iy == ny { ixy } else { ixy + 1 };
        atrnx[c] = (1. + tx[c]) * tx[at(ixn)] / (1. + tx[at(ixn)]);
        atrnx[c + 2 * n] = (1. + tx[c]) * tx[at(ixp)] / (1. + tx[at(ixp)]);
        atrny[c] = (1. + ty[c]) * ty[at(iyn)] / (1. + ty[at(iyn)]);
        atrny[c + 2 * n] = (1. + ty[c]) * ty[at(iyp)] / (1. + ty[at(iyp)]);
        if sta(iy, ix) == 2 {
            if ix == 1 || sta(iy, ix - 1) <= 0 {
                atrnx[c] = 1.;
            }
            if ix == nx || sta(iy, ix + 1) <= 0 {
                atrnx[c + 2 * n] = 1.;
            }
            if iy == 1 || sta(iy - 1, ix) <= 0 {
                atrny[c] = 1.;
            }
            if iy == ny || sta(iy + 1, ix) <= 0 {
                atrny[c + 2 * n] = 1.;
            }
        }
    }
    if m.trflag < 3 || m.ficen - m.fice0 < 1.0e-6 {
        return;
    }
    let (mut trix, mut triy) = (vec![1.; n], vec![1.; n]);
    for i in 0..nsea {
        let c = at(m.mapsf[i + 2 * nsea]);
        let (mut dx, mut dy) = (g.hpfac[c], g.hqfac[c]);
        if m.flagll {
            dx = dx * RADIUS * DERA;
            dy = dy * RADIUS * DERA;
        }
        let (ic, bg) = (ice[i + 1], berg[i + 1]);
        if ic > 0. {
            if m.ficel > 0. {
                trix[c] = expf(-ic * dx / m.ficel);
                triy[c] = expf(-ic * dy / m.ficel);
            } else {
                let (lice0, licen) = (m.fice0 * dx, m.ficen * dx);
                trix[c] = (licen - ic * dx) / (licen - lice0);
                let (lice0, licen) = (m.fice0 * dy, m.ficen * dy);
                triy[c] = (licen - ic * dy) / (licen - lice0);
            }
            trix[c] = (0f32).max((1f32).min(trix[c]));
            triy[c] = (0f32).max((1f32).min(triy[c]));
        }
        if bg > 0. {
            trix[c] = trix[c] * expf(-bg * m.ffacberg * dx * 0.0001);
            triy[c] = triy[c] * expf(-bg * m.ffacberg * dy * 0.0001);
        }
    }
    for i in 0..nsea {
        let (ix, iy, ixy) = (m.mapsf[i], m.mapsf[i + nsea], m.mapsf[i + 2 * nsea]);
        let c = at(ixy);
        let ixn = if ix == 1 { iy + (nx - 1) * ny } else { iy + (ix - 2) * ny };
        let ixp = if ix == nx { iy } else { iy + ix * ny };
        let iyn = if iy == 1 { ixy } else { ixy - 1 };
        let iyp = if iy == ny { ixy } else { ixy + 1 };
        atrnx[c] = atrnx[c] * (1. + trix[c]) * trix[at(ixn)] / (1. + trix[at(ixn)]);
        atrnx[c + 2 * n] = atrnx[c + 2 * n] * (1. + trix[c]) * trix[at(ixp)] / (1. + trix[at(ixp)]);
        atrny[c] = atrny[c] * (1. + triy[c]) * triy[at(iyn)] / (1. + triy[at(iyn)]);
        atrny[c + 2 * n] = atrny[c + 2 * n] * (1. + triy[c]) * triy[at(iyp)] / (1. + triy[at(iyp)]);
    }
}

/// `W3MAPT`.
pub fn w3mapt(m: &ModDef, atrnx: &[Real], atrny: &[Real], maptrn: &mut [u32]) {
    let n = (m.nx * m.ny) as usize;
    let nsea = m.nsea as usize;
    for i in 0..nsea {
        let c = (m.mapsf[i + 2 * nsea] - 1) as usize;
        let lo = atrnx[c + 2 * n].min(atrnx[c]).min(atrny[c + 2 * n]).min(atrny[c]);
        maptrn[c] = (lo < 0.95) as u32;
    }
}

/// `W3DZXY` for this grid: `ZZ(ISEA)`, 1-based through `zz[isea]`, to `DZZDX`, `DZZDY` by `IXY`.
pub fn w3dzxy(m: &ModDef, g: &Metrics, mapsta: &[i32], zz: &[Real], dx: &mut [Real], dy: &mut [Real]) {
    let (nx, ny) = (m.nx, m.ny);
    dx.fill(0.);
    dy.fill(0.);
    let dfac: Real = if m.flagll { 1. / (DERA * RADIUS) } else { 1. };
    let (from, to) = if m.iclose == -1 { (2, nx - 1) } else { (1, nx) };
    assert!(m.iclose == -1 || m.iclose == 2, "only ICLOSE_NONE and ICLOSE_SMPL are implemented");
    let at = |iy: i32, ix: i32| ((iy - 1) + (ix - 1) * ny) as usize;
    let z = |iy: i32, ix: i32| zz[m.mapfs[at(iy, ix)] as usize];
    for iy in 2..ny {
        for ix in from..=to {
            let c = at(iy, ix);
            if mapsta[c] == 0 {
                continue;
            }
            let (mut stx, mut sty): (Real, Real) = (0.5, 0.5);
            let (mut ixp, mut ixm) = (if ix == nx { 1 } else { ix + 1 }, if ix == 1 { nx } else { ix - 1 });
            let (mut iyp, mut iym) = (iy + 1, iy - 1);
            if mapsta[at(iy, ixp)] == 0 {
                ixp = ix;
                stx = 1.;
            }
            if mapsta[at(iy, ixm)] == 0 {
                ixm = ix;
                stx = 1.;
            }
            if mapsta[at(iyp, ix)] == 0 {
                iyp = iy;
                sty = 1.;
            }
            if mapsta[at(iym, ix)] == 0 {
                iym = iy;
                sty = 1.;
            }
            let (a, b) = (z(iy, ixp) - z(iy, ixm), z(iyp, ix) - z(iym, ix));
            dx[c] = (a * stx * g.dpdx[c] + b * sty * g.dqdx[c]) * dfac;
            dy[c] = (a * stx * g.dpdy[c] + b * sty * g.dqdy[c]) * dfac;
        }
    }
}
