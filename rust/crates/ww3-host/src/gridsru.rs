// SPDX-License-Identifier: MPL-2.0
//! The part of the grid search utility that `W3GRID` uses for output boundary points: `GSU_CREATE`
//! (`model/src/w3gsrumd.F90:7354-7849`), `W3GFCL_R8` (`:1369-1669`, without the closest-cell search),
//! `W3CKCL_R8` (`:3615-3840`), `GETPQR` (`:7853-7960`) and `W3GRMP_R8` (`:2570-2895`) at metocean/WW3
//! `92e2cf66`: the bilinear branch for `ww3_grid` (grid stored `XG(J,I)`, `IJG` false, no mask), and for
//! `ww3_prep` the same on `XG(I,J)` (`IJG` true) with the land mask, the nearest-neighbour branch for a fully
//! masked cell (`W3NNSC`, `W3SORT`, `W3ISRT`, `W3DIST`) and the pole cell (`W3SPLX`). Closure `ICLO_NONE` and
//! `ICLO_SMPL` only; the closest-cell search (`FNCL`, `DCIN` > 0) is not translated because neither caller
//! asks for it.

const D360: f64 = 360.;
const D180: f64 = 180.;
const D90: f64 = 90.;
const EPS_DEFAULT: f64 = 1.0e-6;
const NEAR_POLE: f64 = 10.0;
const MAX_FNCL_LEVEL: i32 = 3;
const NCB_DEFAULT: i32 = 10;
const NNP_DEFAULT: i32 = 2;
const ICLO_NONE: i32 = -1;
const ICLO_SMPL: i32 = 2;

pub struct Gsu {
    ijg: bool,
    llg: bool,
    iclo: i32,
    lclo: bool,
    l360: bool,
    nx: i32,
    ny: i32,
    /// `XG(J,I)` (`j + i*ny`) or, with `ijg`, `XG(I,J)` (`i + j*nx`), both from 0.
    xg: Vec<f64>,
    yg: Vec<f64>,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    nbx: i32,
    nby: i32,
    dxb: f64,
    dyb: f64,
    /// `B(JB,IB)%I, J`: the cells of each bucket in the order `GSU_CREATE` adds them.
    b: Vec<Vec<(i32, i32)>>,
}

fn minval(v: &[f64]) -> f64 {
    v.iter().copied().fold(f64::INFINITY, f64::min)
}
fn maxval(v: &[f64]) -> f64 {
    v.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}

impl Gsu {
    /// The position of `XG(J,I)`, or of `XG(I,J)` when the grid is stored that way.
    pub fn at(&self, j: i32, i: i32) -> usize {
        if self.ijg {
            (i - 1) as usize + (j - 1) as usize * self.nx as usize
        } else {
            (j - 1) as usize + (i - 1) as usize * self.ny as usize
        }
    }

    /// The four corners of cell `(i,j)`, with the closure applied: `IC(1:4), JC(1:4)`.
    fn corners(&self, i: i32, j: i32) -> ([i32; 4], [i32; 4]) {
        let mut ic = [i, i + 1, i + 1, i];
        let mut jc = [j, j, j + 1, j + 1];
        for l in 0..4 {
            if self.iclo % 2 == 0 {
                ic[l] = 1 + (self.nx - 1 + (ic[l] - 1 + 1) % self.nx) % self.nx;
            }
            if self.iclo % 3 == 0 {
                jc[l] = 1 + (self.ny - 1 + (jc[l] - 1 + 1) % self.ny) % self.ny;
            }
        }
        (ic, jc)
    }

    /// `GSU_CREATE` with `LBX = LBY = 1`.
    pub fn create(ijg: bool, llg: bool, iclo: i32, nx: i32, ny: i32, xg: Vec<f64>, yg: Vec<f64>) -> Gsu {
        assert!(iclo == ICLO_NONE || iclo == ICLO_SMPL, "closure {iclo} is not implemented in the search utility");
        let mut g = Gsu {
            ijg,
            llg,
            iclo,
            lclo: llg && iclo != ICLO_NONE,
            l360: false,
            nx,
            ny,
            xg,
            yg,
            xmin: 0.,
            xmax: 0.,
            ymin: 0.,
            ymax: 0.,
            nbx: 0,
            nby: 0,
            dxb: 0.,
            dyb: 0.,
            b: Vec::new(),
        };
        let (lxc, lyc) = (1, 1);
        let (uxc, uyc) = if iclo == ICLO_NONE { (nx - 1, ny - 1) } else { (nx, ny - 1) };
        let (mut lnpl, mut lspl) = (false, false);
        let cell = |g: &Gsu, i: i32, j: i32| {
            let (ic, jc) = g.corners(i, j);
            let mut xc = [0.; 4];
            let mut yc = [0.; 4];
            for l in 0..4 {
                xc[l] = g.xg[g.at(jc[l], ic[l])];
                yc[l] = g.yg[g.at(jc[l], ic[l])];
            }
            (ic, jc, xc, yc)
        };
        for i in lxc..=uxc {
            for j in lyc..=uyc {
                let (_, _, xc, yc) = cell(&g, i, j);
                if llg {
                    let mut n = 0;
                    for l in 0..4 {
                        let k = (l + 1) % 4;
                        if (xc[k] - xc[l]).abs() > D180 {
                            n += 1;
                        }
                    }
                    let lpl = n == 1 || yc.iter().filter(|y| y.abs() == D90).count() == 1;
                    if lpl && minval(&yc) > 0. {
                        lnpl = true;
                    }
                    if lpl && maxval(&yc) < 0. {
                        lspl = true;
                    }
                    if n > 0 {
                        g.lclo = true;
                    }
                }
            }
        }
        g.xmin = minval(&g.xg);
        g.xmax = maxval(&g.xg);
        g.ymin = minval(&g.yg);
        g.ymax = maxval(&g.yg);
        if g.lclo {
            g.xmin = 0.;
            g.xmax = D360;
        }
        if lspl {
            g.ymin = -D90;
        }
        if lnpl {
            g.ymax = D90;
        }
        g.l360 = g.xmin >= 0.;
        g.nbx = 1.max(nx / NCB_DEFAULT);
        g.nby = 1.max(ny / NCB_DEFAULT);
        g.dxb = (g.xmax - g.xmin) / g.nbx as f32 as f64;
        g.dyb = (g.ymax - g.ymin) / g.nby as f32 as f64;
        g.b = vec![Vec::new(); (g.nbx * g.nby) as usize];
        let bk = |g: &Gsu, jb: i32, ib: i32| ((jb - 1) + (ib - 1) * g.nby) as usize;
        for i in lxc..=uxc {
            for j in lyc..=uyc {
                let (ic, jc, mut xc, yc) = cell(&g, i, j);
                let (mut lpl, mut lbc) = (false, false);
                if llg {
                    for x in xc.iter_mut() {
                        *x %= D360;
                    }
                    if g.lclo || g.l360 {
                        for x in xc.iter_mut() {
                            if *x < 0. {
                                *x += D360;
                            }
                        }
                    } else {
                        for x in xc.iter_mut() {
                            if *x > D180 {
                                *x -= D360;
                            }
                        }
                    }
                    let mut n = 0;
                    for l in 0..4 {
                        let k = (l + 1) % 4;
                        if (xc[k] - xc[l]).abs() > D180 {
                            n += 1;
                        }
                    }
                    lbc = n > 1;
                    lpl = n == 1 || yc.iter().filter(|y| y.abs() == D90).count() == 1;
                }
                let mut ibc = [0i32; 4];
                let mut jbc = [0i32; 4];
                for l in 0..4 {
                    ibc[l] = ((xc[l] - g.xmin) / g.dxb) as i32 + 1;
                    if !g.lclo {
                        ibc[l] = ibc[l].min(g.nbx);
                    }
                    jbc[l] = (((yc[l] - g.ymin) / g.dyb) as i32 + 1).min(g.nby);
                }
                let (mut ib1, mut ib2, mut jb1, mut jb2) = ([0; 2], [0; 2], [0; 2], [0; 2]);
                let ns;
                let (jmin, jmax) = (*jbc.iter().min().unwrap(), *jbc.iter().max().unwrap());
                if lpl {
                    ns = 1;
                    ib1[0] = 1;
                    ib2[0] = g.nbx;
                    if minval(&yc) > 0. {
                        jb1[0] = 1.max(jmin);
                        jb2[0] = g.nby;
                    }
                    if maxval(&yc) < 0. {
                        jb1[0] = 1;
                        jb2[0] = g.nby.min(jmax);
                    }
                } else if lbc {
                    ns = 2;
                    ib1[0] = g.nbx;
                    ib2[0] = g.nbx;
                    ib1[1] = 1;
                    ib2[1] = 1;
                    for l in 0..4 {
                        if ibc[l] > g.nbx / 2 {
                            ib1[0] = ib1[0].min(ibc[l]);
                        } else {
                            ib2[1] = ib2[1].max(ibc[l]);
                        }
                    }
                    jb1 = [1.max(jmin); 2];
                    jb2 = [g.nby.min(jmax); 2];
                } else {
                    ns = 1;
                    ib1[0] = 1.max(*ibc.iter().min().unwrap());
                    ib2[0] = g.nbx.min(*ibc.iter().max().unwrap());
                    jb1[0] = 1.max(jmin);
                    jb2[0] = g.nby.min(jmax);
                }
                for k in 0..ns {
                    for ib in ib1[k]..=ib2[k] {
                        for jb in jb1[k]..=jb2[k] {
                            let at = bk(&g, jb, ib);
                            g.b[at].push((ic[0], jc[0]));
                        }
                    }
                }
            }
        }
        g
    }
}

// The pole sub-cell test subtracts a corner from itself, as the Fortran does (`:3743-3757`).
#[allow(clippy::eq_op)]
/// `W3CKCL_R8` for a 4-corner cell: whether the target is in it (`POLE` cells use the sub-cell test),
/// and the target and corners shifted across a branch cut, which the Fortran returns in `XT` and `XS`.
fn ckcl(llg: bool, xt: f64, yt: f64, xs: &[f64; 4], ys: &[f64; 4]) -> (bool, f64, [f64; 4]) {
    let leps = EPS_DEFAULT;
    let ns = 4;
    let (mut xxt, yyt) = (xt, yt);
    let (mut xxs, yys) = (*xs, *ys);
    let (pole, bcut);
    if llg {
        let mut n = 0;
        for i in 0..ns {
            let j = (i + 1) % ns;
            if (xxs[j] - xxs[i]).abs() > D180 {
                n += 1;
            }
        }
        bcut = n > 1;
        pole = n == 1 || yys.iter().filter(|y| (D90 - y.abs()).abs() <= leps).count() == 1;
    } else {
        pole = false;
        bcut = false;
    }
    if bcut {
        if minval(&xxs) >= 0. {
            for x in xxs.iter_mut() {
                if *x > D180 {
                    *x -= D360;
                }
            }
            if xxt > D180 {
                xxt -= D360;
            }
        } else {
            for x in xxs.iter_mut() {
                if *x < 0. {
                    *x += D360;
                }
            }
            if xxt < 0. {
                xxt += D360;
            }
        }
    }
    for i in 0..ns {
        if (xxt - xxs[i]).abs() <= leps && (yyt - yys[i]).abs() <= leps {
            return if bcut { (true, xxt, xxs) } else { (true, xt, *xs) };
        }
    }
    let sign = |a: f64, b: f64| a.abs().copysign(b);
    if pole {
        let s90 = if maxval(ys) < 0. { -D90 } else { D90 };
        'sub: for i in 0..ns {
            let j = (i + 1) % ns;
            let mut sign1 = 0.0f64;
            for k in 1..=4 {
                let (mut v1x, v1y, mut v2x, v2y);
                match k {
                    1 => {
                        v1x = xxs[j] - xxs[i];
                        v1y = yys[j] - yys[i];
                        v2x = xxt - xxs[i];
                        v2y = yyt - yys[i];
                    }
                    2 => {
                        v1x = xxs[j] - xxs[j];
                        v1y = s90 - yys[j];
                        v2x = xxt - xxs[j];
                        v2y = yyt - yys[j];
                    }
                    3 => {
                        v1x = xxs[i] - xxs[j];
                        v1y = s90 - s90;
                        v2x = xxt - xxs[j];
                        v2y = yyt - s90;
                    }
                    _ => {
                        v1x = xxs[i] - xxs[i];
                        v1y = yys[i] - s90;
                        v2x = xxt - xxs[i];
                        v2y = yyt - s90;
                    }
                }
                if v1x.abs() > D180 {
                    v1x -= sign(D360, v1x);
                }
                if v2x.abs() > D180 {
                    v2x -= sign(D360, v2x);
                }
                let mut cross = v1x * v2y - v1y * v2x;
                if cross.abs() < leps {
                    cross = 0.;
                }
                if sign1.abs() <= leps {
                    if cross.abs() > leps {
                        sign1 = sign(1., cross);
                    }
                } else if cross.abs() > leps && sign(1., cross) != sign1 {
                    continue 'sub;
                }
            }
            return (true, xt, *xs);
        }
        return (false, xt, *xs);
    }
    let mut sign1 = 0.0f64;
    for i in 0..ns {
        let j = (i + 1) % ns;
        let v1x = xxs[j] - xxs[i];
        let v1y = yys[j] - yys[i];
        let v2x = xxt - xxs[i];
        let v2y = yyt - yys[i];
        let mut cross = v1x * v2y - v1y * v2x;
        if cross.abs() < leps {
            cross = 0.;
        }
        if sign1.abs() <= leps {
            if cross.abs() > leps {
                sign1 = sign(1., cross);
            }
        } else if cross.abs() > leps && sign(1., cross) != sign1 {
            return (false, xt, *xs);
        }
    }
    if bcut { (true, xxt, xxs) } else { (true, xt, *xs) }
}

/// `GETPQR`: the position of the target inside the cell, by Newton iteration.
fn getpqr(xt: f64, yt: f64, xs: &[f64; 4], ys: &[f64; 4]) -> (f64, f64) {
    let leps = EPS_DEFAULT;
    for k in 0..4 {
        if (xt - xs[k]).abs() <= leps && (yt - ys[k]).abs() <= leps {
            return match k {
                0 => (0., 0.),
                1 => (1., 0.),
                2 => (1., 1.),
                _ => (0., 1.),
            };
        }
    }
    let (mut pr, mut qr) = (0.5f64, 0.5f64);
    let dyt = yt - ys[0];
    let dy1 = ys[1] - ys[0];
    let dy2 = ys[3] - ys[0];
    let dy3 = ys[2] - ys[1] - dy2;
    let dxt = xt - xs[0];
    let dx1 = xs[1] - xs[0];
    let dx2 = xs[3] - xs[0];
    let dx3 = xs[2] - xs[1] - dx2;
    for iter in 1..=11 {
        assert!(iter <= 10, "GETPQR -- ERROR: exceeded max iteration count");
        let dyp = dyt - dy1 * pr - dy2 * qr - dy3 * pr * qr;
        let dxp = dxt - dx1 * pr - dx2 * qr - dx3 * pr * qr;
        let mat1 = dy1 + dy3 * qr;
        let mat2 = dy2 + dy3 * pr;
        let mat3 = dx1 + dx3 * qr;
        let mat4 = dx2 + dx3 * pr;
        let det = mat1 * mat4 - mat2 * mat3;
        let delp = (dyp * mat4 - mat2 * dxp) / det;
        let delq = (mat1 * dxp - dyp * mat3) / det;
        pr += delp;
        qr += delq;
        if delp.abs() < 1e-6 && delq.abs() < 1e-6 {
            break;
        }
    }
    (pr, qr)
}

/// The enclosing cell: `IS`, `JS`, `XS`, `YS`.
type Cell = ([i32; 4], [i32; 4], [f64; 4], [f64; 4]);

/// `W3GRMP_R4` (and the `REAL(8)` routine it calls), as `ww3_prep` and `W3FLDP` call it.
pub struct Grmp {
    pub ingrid: bool,
    pub is: [i32; 4],
    pub js: [i32; 4],
    pub rw: [f32; 4],
    /// `MSKC_NONE` 0, `MSKC_PART` 1, `MSKC_FULL` 2; 0 when there is no mask or the point is outside.
    pub mskc: i32,
    /// `NNBR` on return: the number of unmasked neighbours a fully masked cell found.
    pub nnbr: i32,
    /// The enclosing cell has its centre within `NEAR_POLE` degrees of a pole: the weights went through `W3SPLX`.
    pub near_pole: bool,
}

/// `W3SPLX` (point, `REAL(8)`): the stereographic projection about `(lam0, phi0)`, `C0` = 0 here.
fn splx(lam0: f64, phi0: f64, c0: f64, lam: f64, phi: f64) -> (f64, f64) {
    const D2R: f64 = std::f64::consts::PI / 180.;
    const REARTH: f64 = 4.0e7 / (2. * std::f64::consts::PI);
    let clam = ((lam - lam0) * D2R).cos();
    let slam = ((lam - lam0) * D2R).sin();
    let cphi0 = (phi0 * D2R).cos();
    let cphi = (phi * D2R).cos();
    let sphi0 = (phi0 * D2R).sin();
    let sphi = (phi * D2R).sin();
    let k0 = (0.5 * c0 * D2R).cos();
    let k0 = k0 * k0;
    let k = 2. * k0 * REARTH / (1. + sphi0 * sphi + cphi0 * cphi * clam);
    (k * cphi * slam, k * (cphi0 * sphi - sphi0 * cphi * clam))
}

/// `W3DIST_R8` with `DIST_WITH_SINE` (`:5879-5930`), the `REAL(8)` entry `W3GRMP` calls.
fn w3dist(llg: bool, xt: f64, yt: f64, xs: f64, ys: f64) -> f64 {
    const D2R: f64 = std::f64::consts::PI / 180.;
    const R2D: f64 = 1. / D2R;
    let mut dx = xt - xs;
    let dy = yt - ys;
    if llg {
        if dx.abs() > 270. {
            dx -= D360.copysign(dx);
        }
        let slam = (0.5 * dx * D2R).sin();
        let sphi = (0.5 * dy * D2R).sin();
        let argd = (f64::cos(yt * D2R) * f64::cos(ys * D2R) * slam * slam + sphi * sphi).sqrt();
        R2D * 2. * argd.asin()
    } else {
        (dx * dx + dy * dy).sqrt()
    }
}

type Offsets = Vec<(i32, i32)>;

/// `W3NNSC ( 2 )` (`:6819-6879`): `N1(l)`, `N2(l)` and the index deltas `DI(n)`, `DJ(n)`.
fn nns(nlvl: i32) -> (Offsets, Offsets) {
    let mut n12 = vec![(0, 0)];
    let mut d = vec![(0, 0)];
    for l in 1..=nlvl {
        n12.push(((2 * l - 1) * (2 * l - 1), (2 * l + 1) * (2 * l + 1) - 1));
        for i in -l..l {
            d.push((i, -l));
        }
        for j in -l..l {
            d.push((l, j));
        }
        for i in (-l + 1..=l).rev() {
            d.push((i, l));
        }
        for j in (-l + 1..=l).rev() {
            d.push((-l, j));
        }
    }
    (n12, d)
}

/// `W3GFCL ( GSU, XT, YT, IS, JS, XS, YS )` for `REAL(4)` arguments, with no `FNCL`: the enclosing cell, or `None`.
/// `XT` comes back as the search left it.
pub fn w3gfcl_r4(g: &Gsu, xtin: &mut f32, ytin: f32) -> Option<([i32; 4], [i32; 4], [f32; 4], [f32; 4])> {
    let mut xt = *xtin as f64;
    let yt = ytin as f64;
    if g.llg {
        xt %= D360;
        if g.lclo || g.l360 {
            if xt < 0. {
                xt += D360;
            }
        } else if xt > D180 {
            xt -= D360;
        }
    }
    *xtin = xt as f32;
    if xt < g.xmin - EPS_DEFAULT || xt > g.xmax + EPS_DEFAULT || yt < g.ymin - EPS_DEFAULT || yt > g.ymax + EPS_DEFAULT {
        return None;
    }
    let mut ib = (((xt - g.xmin) / g.dxb) as i32 + 1).max(1);
    if !g.lclo {
        ib = ib.min(g.nbx);
    }
    let jb = (((yt - g.ymin) / g.dyb) as i32 + 1).max(1).min(g.nby);
    for &(ci, cj) in &g.b[((jb - 1) + (ib - 1) * g.nby) as usize] {
        let (is, js) = g.corners_of(ci, cj);
        let (mut xs, mut ys) = ([0.; 4], [0.; 4]);
        for l in 0..4 {
            xs[l] = g.xg[g.at(js[l], is[l])];
            ys[l] = g.yg[g.at(js[l], is[l])];
            if g.llg {
                xs[l] %= D360;
                if g.lclo || g.l360 {
                    if xs[l] < 0. {
                        xs[l] += D360;
                    }
                } else if xs[l] > D180 {
                    xs[l] -= D360;
                }
            }
        }
        let (incell, xt2, xs2) = ckcl(g.llg, xt, yt, &xs, &ys);
        if incell {
            *xtin = xt2 as f32;
            return Some((is, js, xs2.map(|v| v as f32), ys.map(|v| v as f32)));
        }
    }
    None
}

/// `W3GFPT ( GSU, X, Y, IX, IY, DCIN=dcin )` for `REAL(4)` arguments, so `dcin` widens from `REAL(4)`: the grid
/// point closest to the target, or `None` outside the grid by more than `dcin` cell widths. `W3GFCL_R8` is inside,
/// with `FNCL` true, `model/src/w3gsrumd.F90:1369-1669` and `:2078-2192`.
pub fn w3gfpt(g: &Gsu, xtin: f32, ytin: f32, dcin: f32) -> Option<(i32, i32)> {
    const BIG: f64 = 1e16;
    let ldcin = dcin as f64;
    let mut xt = xtin as f64;
    let yt = ytin as f64;
    if g.llg {
        xt %= D360;
        if g.lclo || g.l360 {
            if xt < 0. {
                xt += D360;
            }
        } else if xt > D180 {
            xt -= D360;
        }
    }
    let corners = |ci: i32, cj: i32| {
        let (is, js) = g.corners_of(ci, cj);
        let (mut xs, mut ys) = ([0.; 4], [0.; 4]);
        for l in 0..4 {
            xs[l] = g.xg[g.at(js[l], is[l])];
            ys[l] = g.yg[g.at(js[l], is[l])];
            if g.llg {
                xs[l] %= D360;
                if g.lclo || g.l360 {
                    if xs[l] < 0. {
                        xs[l] += D360;
                    }
                } else if xs[l] > D180 {
                    xs[l] -= D360;
                }
            }
        }
        (is, js, xs, ys)
    };
    let mut ib = (((xt - g.xmin) / g.dxb) as i32 + 1).max(1);
    if !g.lclo {
        ib = ib.min(g.nbx);
    }
    let jb = (((yt - g.ymin) / g.dyb) as i32 + 1).max(1).min(g.nby);
    let mut found = None;
    for &(ci, cj) in &g.b[((jb - 1) + (ib - 1) * g.nby) as usize] {
        let (is, js, xs, ys) = corners(ci, cj);
        let (incell, xt2, xs2) = ckcl(g.llg, xt, yt, &xs, &ys);
        if incell {
            xt = xt2;
            found = Some((is, js, xs2, ys));
            break;
        }
    }
    let ingrid = found.is_some();
    // The closest cell: its centre, over rings of buckets until MAX_FNCL_LEVEL rings held cells.
    if found.is_none() {
        let nlvl = (0.5 * g.nbx.max(g.nby) as f32).round() as i32;
        let (n12, d) = nns(nlvl);
        let (mut nlevel, mut dd1) = (0, f64::MAX);
        'levels: for lvl in 0..=nlvl as usize {
            let mut any = false;
            let (n1, n2) = n12[lvl];
            for &(di, dj) in &d[n1 as usize..=n2 as usize] {
                let (ibn, jbn) = (ib + di, jb + dj);
                if ibn < 1 || ibn > g.nbx || jbn < 1 || jbn > g.nby {
                    continue;
                }
                for &(ci, cj) in &g.b[((jbn - 1) + (ibn - 1) * g.nby) as usize] {
                    let (is, js, xs, ys) = corners(ci, cj);
                    let xsm = (xs[0] + xs[1] + xs[2] + xs[3]) / 4.;
                    let ysm = (ys[0] + ys[1] + ys[2] + ys[3]) / 4.;
                    let dd = w3dist(g.llg, xt, yt, xsm, ysm);
                    if dd < dd1 {
                        dd1 = dd;
                        found = Some((is, js, xs, ys));
                    }
                    any = true;
                }
            }
            if any {
                nlevel += 1;
            }
            if nlevel >= MAX_FNCL_LEVEL {
                break 'levels;
            }
        }
    }
    let (is, js, xs, ys) = found?;
    if !ingrid {
        let lon0 = (xs[0] + xs[1] + xs[2] + xs[3]) / 4.;
        let lat0 = (ys[0] + ys[1] + ys[2] + ys[3]) / 4.;
        let (ixr, jxr) = if D90 - lat0.abs() > NEAR_POLE {
            getpqr(xt, yt, &xs, &ys)
        } else {
            let (xtc, ytc) = splx(lon0, lat0, 0., xt, yt);
            let (mut xsc, mut ysc) = ([0.; 4], [0.; 4]);
            for i in 0..4 {
                (xsc[i], ysc[i]) = splx(lon0, lat0, 0., xs[i], ys[i]);
            }
            getpqr(xtc, ytc, &xsc, &ysc)
        };
        let dd = 0.5 + ldcin;
        if !((ixr - 0.5).abs() <= dd && (jxr - 0.5).abs() <= dd) {
            return None;
        }
    }
    let mut dmin = BIG;
    let (mut ix, mut iy) = (0, 0);
    for l in 0..4 {
        let dd = w3dist(g.llg, xt, yt, xs[l], ys[l]);
        if dd < dmin {
            (dmin, ix, iy) = (dd, is[l], js[l]);
        }
    }
    Some((ix, iy))
}

/// `W3GRMP ( GSU, X, Y, IS, JS, RW )` for a `REAL(4)` target. `mask` (true = land, `LMSK`) is stored as the
/// coordinates are; with `nnbr` too it is `W3FLDP`'s call, `NNBR = 2`.
pub fn w3grmp_mask(g: &Gsu, xtin: f32, ytin: f32, mask: Option<&[bool]>, nnbr: Option<i32>) -> Grmp {
    const BIG: f64 = 1e16;
    const SMALL: f64 = 1e-6;
    let mut xt = xtin as f64;
    let yt = ytin as f64;
    let mut out = Grmp { ingrid: false, is: [0; 4], js: [0; 4], rw: [0.; 4], mskc: 0, nnbr: nnbr.unwrap_or(0), near_pole: false };
    // W3GFCL_R8
    let mut found: Option<Cell> = None;
    if g.llg {
        xt %= D360;
        if g.lclo || g.l360 {
            if xt < 0. {
                xt += D360;
            }
        } else if xt > D180 {
            xt -= D360;
        }
    }
    let inside = !(xt < g.xmin - EPS_DEFAULT || xt > g.xmax + EPS_DEFAULT || yt < g.ymin - EPS_DEFAULT || yt > g.ymax + EPS_DEFAULT);
    if inside {
        let mut ib = (((xt - g.xmin) / g.dxb) as i32 + 1).max(1);
        if !g.lclo {
            ib = ib.min(g.nbx);
        }
        let jb = (((yt - g.ymin) / g.dyb) as i32 + 1).max(1).min(g.nby);
        for &(ci, cj) in &g.b[((jb - 1) + (ib - 1) * g.nby) as usize] {
            let (is, js) = g.corners_of(ci, cj);
            let mut xs = [0.; 4];
            let mut ys = [0.; 4];
            for l in 0..4 {
                xs[l] = g.xg[g.at(js[l], is[l])];
                ys[l] = g.yg[g.at(js[l], is[l])];
                if g.llg {
                    xs[l] %= D360;
                    if g.lclo || g.l360 {
                        if xs[l] < 0. {
                            xs[l] += D360;
                        }
                    } else if xs[l] > D180 {
                        xs[l] -= D360;
                    }
                }
            }
            let (incell, xt2, xs2) = ckcl(g.llg, xt, yt, &xs, &ys);
            if incell {
                xt = xt2;
                found = Some((is, js, xs2, ys));
                break;
            }
        }
    }
    let Some((mut is, mut js, xs, ys)) = found else {
        return out;
    };
    let lon0 = (xs[0] + xs[1] + xs[2] + xs[3]) / 4.;
    let lat0 = (ys[0] + ys[1] + ys[2] + ys[3]) / 4.;
    let (ixr, jxr) = if D90 - lat0.abs() > NEAR_POLE {
        getpqr(xt, yt, &xs, &ys)
    } else {
        out.near_pole = true;
        let (xtc, ytc) = splx(lon0, lat0, 0., xt, yt);
        let mut xsc = [0.; 4];
        let mut ysc = [0.; 4];
        for i in 0..4 {
            (xsc[i], ysc[i]) = splx(lon0, lat0, 0., xs[i], ys[i]);
        }
        getpqr(xtc, ytc, &xsc, &ysc)
    };
    let mut dw = [(1. - ixr) * (1. - jxr), ixr * (1. - jxr), ixr * jxr, (1. - ixr) * jxr];
    let narrow = |d: &[f64; 4]| [d[0] as f32, d[1] as f32, d[2] as f32, d[3] as f32];
    out.ingrid = true;
    out.is = is;
    out.js = js;
    out.rw = narrow(&dw);
    let Some(mask) = mask else { return out };
    // 4. A partially masked cell.
    let msk: Vec<bool> = (0..4).map(|l| mask[g.at(js[l], is[l])]).collect();
    let mut dsum = 0.;
    let mut ns = 4;
    for l in 0..4 {
        if msk[l] {
            ns -= 1;
            dw[l] = 0.;
        }
        dsum += dw[l];
    }
    if ns == 4 {
        return out;
    }
    if ns > 0 && dsum > SMALL {
        for d in dw.iter_mut() {
            *d /= dsum;
        }
        out.rw = narrow(&dw);
        out.mskc = 1;
        return out;
    }
    out.mskc = 2;
    let Some(want) = nnbr else { return out };
    // 5. A fully masked cell: the closest unmasked points of the neighbourhood, weighted by inverse distance.
    let mut dmin = BIG;
    let (mut icc, mut jcc) = (0, 0);
    for l in 0..4 {
        let dd = w3dist(g.llg, xt, yt, xs[l], ys[l]);
        if dd < dmin {
            dmin = dd;
            icc = is[l];
            jcc = js[l];
        }
    }
    let (n12, dij) = nns(NNP_DEFAULT);
    let mut d = [BIG; 4];
    let mut ns = 0usize;
    'level: for &(n1, n2) in &n12 {
        for n in n1..=n2 {
            let (di, dj) = dij[n as usize];
            let (mut i, mut j) = (icc + di, jcc + dj);
            if g.iclo == ICLO_NONE && (i < 1 || i > g.nx || j < 1 || j > g.ny) {
                continue;
            }
            if g.iclo % 2 == 0 {
                i = 1 + (g.nx - 1 + (i - 1 + 1) % g.nx) % g.nx;
            }
            if g.iclo % 3 == 0 {
                j = 1 + (g.ny - 1 + (j - 1 + 1) % g.ny) % g.ny;
            }
            if mask[g.at(j, i)] {
                continue;
            }
            let dd = w3dist(g.llg, xt, yt, g.xg[g.at(j, i)], g.yg[g.at(j, i)]);
            if (ns as i32) < want {
                is[ns] = i;
                js[ns] = j;
                d[ns] = dd;
                ns += 1;
                if ns as i32 == want {
                    // W3SORT
                    for k in 0..ns.saturating_sub(1) {
                        for l in k + 1..ns {
                            if d[l] < d[k] {
                                is.swap(k, l);
                                js.swap(k, l);
                                d.swap(k, l);
                            }
                        }
                    }
                }
            } else {
                // W3ISRT
                for k in 0..ns {
                    if dd < d[k] {
                        for l in (k + 1..ns).rev() {
                            is[l] = is[l - 1];
                            js[l] = js[l - 1];
                            d[l] = d[l - 1];
                        }
                        is[k] = i;
                        js[k] = j;
                        d[k] = dd;
                        break;
                    }
                }
            }
        }
        if ns as i32 == want {
            break 'level;
        }
    }
    out.is = is;
    out.js = js;
    out.nnbr = ns as i32;
    if ns == 0 {
        return out;
    }
    let mut dsum = 0.;
    for dl in d.iter().take(ns) {
        dsum += 1. / (dl + SMALL);
    }
    for l in 0..ns {
        dw[l] = 1. / (d[l] + SMALL) / dsum;
    }
    out.rw = narrow(&dw);
    out
}

/// `W3GRMP` without a mask, for `ww3_grid`: `(INGRID, IS, JS, RW)`, `RW` narrowed to `REAL`.
pub fn w3grmp(g: &Gsu, xtin: f32, ytin: f32) -> (bool, [i32; 4], [i32; 4], [f32; 4]) {
    let r = w3grmp_mask(g, xtin, ytin, None, None);
    (r.ingrid, r.is, r.js, r.rw)
}

impl Gsu {
    fn corners_of(&self, i: i32, j: i32) -> ([i32; 4], [i32; 4]) {
        self.corners(i, j)
    }
}
