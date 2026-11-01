// SPDX-License-Identifier: MPL-2.0
//! The part of the grid search utility that `W3GRID` uses for output boundary points: `GSU_CREATE`
//! (`model/src/w3gsrumd.F90:7354-7849`), `W3GFCL_R8` (`:1369-1669`, without the closest-cell search),
//! `W3CKCL_R8` (`:3615-3840`), `GETPQR` (`:7853-7960`) and the bilinear branch of `W3GRMP_R8`
//! (`:2570-2895`) at metocean/WW3 `92e2cf66`, for a grid stored `XG(J,I)` (`IJG` false), `REAL(8)`
//! coordinates and no mask. Closure `ICLO_NONE` and `ICLO_SMPL` only. A cell within 10 degrees of a pole is projected with `W3SPLX` (`:5971-5995`) before `GETPQR`, as `ww3_prnc` needs for a global input.

const D360: f64 = 360.;
const D180: f64 = 180.;
const D90: f64 = 90.;
const EPS_DEFAULT: f64 = 1.0e-6;
const NEAR_POLE: f64 = 10.0;
const NCB_DEFAULT: i32 = 10;
const ICLO_NONE: i32 = -1;
const ICLO_SMPL: i32 = 2;

pub struct Gsu {
    llg: bool,
    iclo: i32,
    lclo: bool,
    l360: bool,
    nx: i32,
    ny: i32,
    /// `XG(J,I)`: `j + i*ny` with both from 0.
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
    fn at(&self, j: i32, i: i32) -> usize {
        (j - 1) as usize + (i - 1) as usize * self.ny as usize
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
    pub fn create(llg: bool, iclo: i32, nx: i32, ny: i32, xg: Vec<f64>, yg: Vec<f64>) -> Gsu {
        assert!(iclo == ICLO_NONE || iclo == ICLO_SMPL, "closure {iclo} is not implemented in the search utility");
        let mut g = Gsu {
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

/// `W3SPLX_0D_R8` with `C0 = 0`: stereographic projection about `(lam0, phi0)`, in metres.
fn splx(lam0: f64, phi0: f64, lam: f64, phi: f64) -> (f64, f64) {
    const PI: f64 = std::f64::consts::PI;
    let d2r = PI / 180.;
    let rearth = 4.0e7 / (2. * PI);
    let clam = ((lam - lam0) * d2r).cos();
    let slam = ((lam - lam0) * d2r).sin();
    let cphi0 = (phi0 * d2r).cos();
    let cphi = (phi * d2r).cos();
    let sphi0 = (phi0 * d2r).sin();
    let sphi = (phi * d2r).sin();
    let k0 = (0.5 * 0. * d2r).cos().powi(2);
    let k = 2. * k0 * rearth / (1. + sphi0 * sphi + cphi0 * cphi * clam);
    (k * cphi * slam, k * (cphi0 * sphi - sphi0 * cphi * clam))
}

/// `W3GRMP` without a mask: `(INGRID, IS, JS, RW)`, `RW` narrowed to `REAL`.
pub fn w3grmp(g: &Gsu, xtin: f32, ytin: f32) -> (bool, [i32; 4], [i32; 4], [f32; 4]) {
    let mut xt = xtin as f64;
    let yt = ytin as f64;
    // W3GFCL_R8
    let mut found: Option<([i32; 4], [i32; 4], [f64; 4], [f64; 4])> = None;
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
    let Some((is, js, xs, ys)) = found else {
        return (false, [0; 4], [0; 4], [0.; 4]);
    };
    let lon0 = (xs[0] + xs[1] + xs[2] + xs[3]) / 4.;
    let lat0 = (ys[0] + ys[1] + ys[2] + ys[3]) / 4.;
    let (ixr, jxr) = if D90 - lat0.abs() > NEAR_POLE {
        getpqr(xt, yt, &xs, &ys)
    } else {
        // Near a pole the cell is projected onto the plane tangent at its centre (`W3SPLX`, `:5971-5995`).
        let (xtc, ytc) = splx(lon0, lat0, xt, yt);
        let (mut xsc, mut ysc) = ([0.; 4], [0.; 4]);
        for i in 0..4 {
            (xsc[i], ysc[i]) = splx(lon0, lat0, xs[i], ys[i]);
        }
        getpqr(xtc, ytc, &xsc, &ysc)
    };
    let dw = [(1. - ixr) * (1. - jxr), ixr * (1. - jxr), ixr * jxr, (1. - ixr) * jxr];
    (true, is, js, [dw[0] as f32, dw[1] as f32, dw[2] as f32, dw[3] as f32])
}

impl Gsu {
    fn corners_of(&self, i: i32, j: i32) -> ([i32; 4], [i32; 4]) {
        self.corners(i, j)
    }
}
