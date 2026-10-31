// SPDX-License-Identifier: MPL-2.0
//! The grid maps and fields `W3WAVE` builds before it propagates, for a rectilinear grid: `W3MAP3`,
//! `W3MAPT` (`model/src/w3pro3md.F90`), `W3UTRN`, `W3DZXY` (`w3updtmd.F90`), and the metrics `W3GNTX`
//! gets from `W3CGDM` (`w3gsrumd.F90`), which WW3 calls with `SPHERE=.FALSE.`, `DX` and `DY`.

use crate::moddef::ModDef;
use ww3_kernels::constants::{DERA, RADIUS};
use ww3_kernels::real::Real;

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
}

impl Metrics {
    pub fn new(m: &ModDef) -> Self {
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
        }
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

/// `W3UTRN`, ice and icebergs out: `TRFLAG` of 3 and above stops in the driver. `ATRNX(NY*NX,-1:1)` is
/// `atrnx[ixy-1 + (j+1)*NY*NX]`.
pub fn w3utrn(m: &ModDef, mapsta: &[i32], atrnx: &mut [Real], atrny: &mut [Real]) {
    atrnx.fill(1.);
    atrny.fill(1.);
    if m.trflag == 0 {
        return;
    }
    assert!(m.trflag < 3, "ice in the transparencies is not implemented");
    let (nx, ny, nsea) = (m.nx, m.ny, m.nsea as usize);
    let n = (nx * ny) as usize;
    let (tx, ty) = (&m.trnx, &m.trny);
    let at = |i: i32| (i - 1) as usize;
    let sta = |iy: i32, ix: i32| mapsta[((iy - 1) + (ix - 1) * ny) as usize];
    for i in 0..nsea {
        let (ix, iy, ixy) = (m.mapsf[i], m.mapsf[i + nsea], m.mapsf[i + 2 * nsea]);
        let c = at(ixy);
        if m.trflag == 1 {
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
