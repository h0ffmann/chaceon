// SPDX-License-Identifier: MPL-2.0
//! `W3PRO3MD` with `W3UQCKMD` (`model/src/w3pro3md.F90`, `w3uqckmd.F90` at the `WW3/` pin): the third-order
//! ULTIMATE QUICKEST scheme, `W3XYP3` in space and `W3KTP3` in spectral space. Translated for the `UQ`
//! switch. Currents, moving grids and input boundaries stop in the host before they reach this file.
//!
//! Fortran's bounds stay in the index: `Q(1-MY:MY*(MX+2))` is `q[(ixy + my - 1) as usize]`.

use crate::constants::{DERA, GRAV, RADIUS};
use crate::real::Real;
use crate::util::{pow2, sign};

/// The geometry of one call of `W3QCK1`, `W3QCK2` or `W3QCK3`.
#[derive(Clone, Copy)]
pub struct Qck {
    pub mx: i32,
    pub my: i32,
    pub nx: i32,
    pub ny: i32,
    pub close: bool,
    pub inc: i32,
    pub nact: i32,
    pub nb0: i32,
    pub nb1: i32,
    pub nb2: i32,
}

/// The low bound of `Q` and `FLA` is `1-MY`.
#[inline]
fn lo(i: i32, my: i32) -> usize {
    (i + my - 1) as usize
}

#[inline]
fn up(i: i32) -> usize {
    (i - 1) as usize
}

/// `INT ( MIN ( 0. , SIGN (1.1,CFL) ) )`.
#[inline]
fn neg(cfl: Real) -> i32 {
    0f32.min(sign(1.1, cfl)) as i32
}

/// The ULTIMATE limiter, `QB` to the limited value. Lines 1129-1143 of `W3QCK3`, the same in all three.
#[inline]
fn limited(q: &[Real], qb: Real, cfl: Real, ixyc: i32, inc: i32, my: i32) -> Real {
    let ixyu = ixyc - inc * sign(1.1, cfl) as i32;
    let ixyd = 2 * ixyc - ixyu;
    let qu = q[lo(ixyu, my)];
    let qc = q[lo(ixyc, my)];
    let dq = q[lo(ixyd, my)] - qu;
    let dqnz = sign(1.0e-15f32.max(dq.abs()), dq);
    let qcn = (qc - qu) / dqnz;
    let qcn = (1.1f32).min((-0.1f32).max(qcn));
    let qbn = ((qb - qu) / dqnz).max(qcn);
    let qbn = qbn.min(1.).min(qcn / 1.0e-10f32.max(cfl.abs()));
    let qbr = qu + qbn * dq;
    let cfac = (2. * (qcn - 0.5).abs()) as i32 as Real;
    (1. - cfac) * qbr + cfac * qc
}

/// Lines 1078-1103: the ghost columns of a closed direction.
#[inline]
fn close_q(g: &Qck, cfll: &mut [Real], q: &mut [Real]) {
    let (my, iad00, iad02) = (g.my, -g.my, g.my);
    let (iadn0, iadn1, iadn2) = (iad00 + my * g.nx, my * g.nx, iad02 + my * g.nx);
    each!(iy, 1, g.ny, {
        q[lo(iy + iad00, my)] = q[lo(iy + iadn0, my)];
        q[lo(iy + iadn1, my)] = q[lo(iy, my)];
        q[lo(iy + iadn2, my)] = q[lo(iy + iad02, my)];
        cfll[up(iy + iadn1)] = cfll[up(iy)];
    });
}

/// Lines 1231-1238.
#[inline]
fn close_fla(g: &Qck, fla: &mut [Real]) {
    let my = g.my;
    each!(iy, 1, g.ny, {
        fla[lo(iy - my, my)] = fla[lo(iy - my + my * g.nx, my)];
    });
}

/// `W3QCK1` and `W3QCK3` compute the same fluxes: lines 1121-1225 of `W3QCK3`.
#[inline]
fn fluxes(g: &Qck, cfll: &[Real], q: &[Real], mapbou: &[i32], fla: &mut [Real]) {
    let (inc, my) = (g.inc, g.my);
    each!(ip, 1, g.nb0, {
        let ixy = mapbou[up(ip)];
        let cfl = 0.5 * (cfll[up(ixy)] + cfll[up(ixy + inc)]);
        let ixyc = ixy - inc * neg(cfl);
        let qb = 0.5 * ((1. - cfl) * q[lo(ixy + inc, my)] + (1. + cfl) * q[lo(ixy, my)])
            - (1. - pow2(cfl)) / 6.
                * (q[lo(ixyc - inc, my)] - 2. * q[lo(ixyc, my)] + q[lo(ixyc + inc, my)]);
        let qb = limited(q, qb, cfl, ixyc, inc, my);
        fla[lo(ixy, my)] = cfl * qb;
    });
    each!(ip, g.nb0 + 1, g.nb1, {
        let ixy = mapbou[up(ip)];
        let cfl = cfll[up(ixy)];
        let ixyc = ixy - inc * neg(cfl);
        fla[lo(ixy, my)] = cfl * q[lo(ixyc, my)];
    });
    each!(ip, g.nb1 + 1, g.nb2, {
        let ixy = mapbou[up(ip)];
        let cfl = cfll[up(ixy + inc)];
        let ixyc = ixy - inc * neg(cfl);
        fla[lo(ixy, my)] = cfl * q[lo(ixyc, my)];
    });
}

#[inline]
fn zero(v: &mut [Real]) {
    let mut i = 0;
    while i < v.len() {
        v[i] = 0.;
        i += 1;
    }
}

/// `W3QCK1`.
pub fn w3qck1(g: &Qck, cfll: &mut [Real], q: &mut [Real], mapact: &[i32], mapbou: &[i32], fla: &mut [Real]) {
    zero(fla);
    if g.close {
        close_q(g, cfll, q);
    }
    fluxes(g, cfll, q, mapbou, fla);
    if g.close {
        close_fla(g, fla);
    }
    let (inc, my) = (g.inc, g.my);
    each!(ip, 1, g.nact, {
        let ixy = mapact[up(ip)];
        q[lo(ixy, my)] = 0f32.max(q[lo(ixy, my)] + fla[lo(ixy - inc, my)] - fla[lo(ixy, my)]);
    });
}

/// `W3QCK2`, with `CLOSE = .FALSE.`, its only use here. `DX2` keeps the low bound `1-MY`.
#[allow(clippy::too_many_arguments)]
pub fn w3qck2(
    g: &Qck,
    velo: &[Real],
    dt: Real,
    dx1: &[Real],
    dx2: &[Real],
    q: &mut [Real],
    mapact: &[i32],
    mapbou: &[i32],
    fla: &mut [Real],
) {
    zero(fla);
    let (inc, my) = (g.inc, g.my);
    each!(ip, 1, g.nb0, {
        let ixy = mapbou[up(ip)];
        let vel = 0.5 * (velo[up(ixy)] + velo[up(ixy + inc)]);
        let cfl = dt * vel / dx2[lo(ixy, my)];
        let ixyc = ixy - inc * neg(cfl);
        let d2 = dx2[lo(ixy, my)];
        let qb = 0.5 * ((1. - cfl) * q[lo(ixy + inc, my)] + (1. + cfl) * q[lo(ixy, my)])
            - pow2(d2) / dx1[up(ixyc)] * (1. - pow2(cfl)) / 6.
                * ((q[lo(ixyc + inc, my)] - q[lo(ixyc, my)]) / dx2[lo(ixyc, my)]
                    - (q[lo(ixyc, my)] - q[lo(ixyc - inc, my)]) / dx2[lo(ixyc - inc, my)]);
        let qb = limited(q, qb, cfl, ixyc, inc, my);
        fla[lo(ixy, my)] = vel * qb;
    });
    each!(ip, g.nb0 + 1, g.nb1, {
        let ixy = mapbou[up(ip)];
        let vel = velo[up(ixy)];
        let ixyc = ixy - inc * neg(vel);
        fla[lo(ixy, my)] = vel * q[lo(ixyc, my)];
    });
    each!(ip, g.nb1 + 1, g.nb2, {
        let ixy = mapbou[up(ip)];
        let vel = velo[up(ixy + inc)];
        let ixyc = ixy - inc * neg(vel);
        fla[lo(ixy, my)] = vel * q[lo(ixyc, my)];
    });
    each!(ip, 1, g.nact, {
        let ixy = mapact[up(ip)];
        q[lo(ixy, my)] =
            0f32.max(q[lo(ixy, my)] + dt / dx1[up(ixy)] * (fla[lo(ixy - inc, my)] - fla[lo(ixy, my)]));
    });
}

/// `W3QCK3`. `TRANS(MY*MX,-1:1)` is `trans[ixy-1 + (j+1)*MY*MX]`.
pub fn w3qck3(
    g: &Qck,
    cfll: &mut [Real],
    trans: &[Real],
    q: &mut [Real],
    mapact: &[i32],
    mapbou: &[i32],
    fla: &mut [Real],
) {
    zero(fla);
    if g.close {
        close_q(g, cfll, q);
    }
    fluxes(g, cfll, q, mapbou, fla);
    if g.close {
        close_fla(g, fla);
    }
    let (inc, my) = (g.inc, g.my);
    let n = (my * g.mx) as usize;
    each!(ip, 1, g.nact, {
        let ixy = mapact[up(ip)];
        let jn = if fla[lo(ixy - inc, my)] > 0. { 0 } else { 1 };
        let jp = if fla[lo(ixy, my)] < 0. { 2 } else { 1 };
        q[lo(ixy, my)] = 0f32.max(
            q[lo(ixy, my)] + trans[up(ixy) + jn * n] * fla[lo(ixy - inc, my)]
                - trans[up(ixy) + jp * n] * fla[lo(ixy, my)],
        );
    });
}

/// The scalars of `W3XYP3`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Xyp3Params {
    pub nx: i32,
    pub ny: i32,
    pub nsea: i32,
    pub nth: i32,
    pub nact: i32,
    pub ncent: i32,
    pub nmx0: i32,
    pub nmx1: i32,
    pub nmx2: i32,
    pub nmy0: i32,
    pub nmy1: i32,
    pub nmy2: i32,
    pub flcx: u32,
    pub flcy: u32,
    pub global: u32,
    pub flagll: u32,
    pub dtcfl: Real,
    pub wdth: Real,
    pub wdcg: Real,
    pub dth: Real,
    pub xfr: Real,
}

/// What does not change between components. Maps are 1-based, as WW3 stores them; `dpdx` and the other
/// metrics are indexed by `IXY`.
pub struct Xyp3Tables<'a> {
    pub sig: &'a [Real],
    pub ecos: &'a [Real],
    pub esin: &'a [Real],
    pub mapsf: &'a [i32],
    pub mapsta: &'a [i32],
    pub mapaxy: &'a [i32],
    pub mapcxy: &'a [i32],
    pub mapx2: &'a [i32],
    pub mapy2: &'a [i32],
    pub maptrn: &'a [u32],
    pub clats: &'a [Real],
    pub dpdx: &'a [Real],
    pub dpdy: &'a [Real],
    pub dqdx: &'a [Real],
    pub dqdy: &'a [Real],
    pub gsqrt: &'a [Real],
    /// `CG(0:NK+1,0:NSEA)`.
    pub cg: &'a [Real],
    pub atrnx: &'a [Real],
    pub atrny: &'a [Real],
}

/// `W3XYP3`'s automatic arrays, whose stack a kernel may not allocate.
pub struct Xyp3Work<'a> {
    pub iixy1: &'a mut [i32],
    pub iixy2: &'a mut [i32],
    pub iixy3: &'a mut [i32],
    pub iixy4: &'a mut [i32],
    pub rdi1: &'a mut [Real],
    pub rdi2: &'a mut [Real],
    pub rdi3: &'a mut [Real],
    pub rdi4: &'a mut [Real],
    pub mapstx: &'a mut [i32],
    pub vlcflx: &'a mut [Real],
    pub vlcfly: &'a mut [Real],
    pub cxtot: &'a mut [Real],
    pub cytot: &'a mut [Real],
    pub aq: &'a mut [Real],
    pub fla: &'a mut [Real],
}

/// The low bound of `MAPSTX` is `1-2*NY`.
#[inline]
fn stx(s: &[i32], i: i32, ny: i32) -> i32 {
    s[(i + 2 * ny - 1) as usize]
}

/// The dissipated corners of one sea point: lines 951-969 and 981-999, the garden sprinkler correction.
#[inline]
fn corners(dxcgc: Real, dycgc: Real, ny: i32) -> (i32, i32, Real, Real) {
    let ixc = if dxcgc < 0. { -ny } else { ny };
    let iyc = if dycgc < 0. { -1 } else { 1 };
    let iixy1 = ixc + iyc;
    if dxcgc.abs() > dycgc.abs() {
        (iixy1, ixc, (dycgc / dxcgc).abs(), dxcgc.abs())
    } else {
        let rd1 = if dycgc.abs() > 1.0e-5 { (dxcgc / dycgc).abs() } else { 1. };
        (iixy1, iyc, rd1, dycgc.abs())
    }
}

/// `Q(IXY+D) += F`, or `Q(IXY) += F` where the corner is blocked: lines 1151-1159 and the like.
#[inline]
fn spread(vq: &mut [Real], ixy: i32, d: i32, open: bool, f: Real, ny: i32) {
    let t = if open { ixy + d } else { ixy };
    vq[lo(t, ny)] = vq[lo(t, ny)] + f;
}

/// `W3XYP3` for component `ISP`. `yfirst` comes from the clock, which the host owns: lines 875-878.
pub fn w3xyp3(p: &Xyp3Params, t: &Xyp3Tables, isp: i32, dtg: Real, yfirst: bool, vq: &mut [Real], w: &mut Xyp3Work) {
    let (nx, ny, nsea) = (p.nx, p.ny, p.nsea);
    let nxy = nx * ny;
    let nk2 = t.sig.len() as i32;
    let global = p.global != 0;
    let flagll = p.flagll != 0;
    let ith = 1 + (isp - 1) % p.nth;
    let ik = 1 + (isp - 1) / p.nth;
    let ecos = t.ecos[up(ith)];
    let esin = t.esin[up(ith)];

    let cg0 = 0.575 * GRAV / t.sig[1];
    let cga = 0.575 * GRAV / t.sig[ik as usize];
    let cgx = cga * ecos;
    let cgy = cga * esin;
    let cgn = cgx.abs().max(cgy.abs()).max(0.).max(0.).max(0.001 * cg0);

    let ntloc = 1 + (dtg / (p.dtcfl * cg0 / cgn)) as i32;
    let dtloc = dtg / ntloc as Real;
    let mut dtrad = dtloc;
    if flagll {
        dtrad = dtrad / (DERA * RADIUS);
    }
    let mut yfirst = yfirst;

    zero(w.vlcflx);
    zero(w.vlcfly);
    zero(w.cxtot);
    zero(w.cytot);

    let s = (2 * ny) as usize;
    each!(i, 1, nxy, {
        w.mapstx[up(i) + s] = t.mapsta[up(i)];
    });
    each!(i, 1, 2 * ny, {
        w.mapstx[up(i)] = if global { t.mapsta[up(i + (nx - 2) * ny)] } else { 0 };
        w.mapstx[up(i + nxy) + s] = if global { t.mapsta[up(i)] } else { 0 };
    });

    let mut fth = 1. * p.wdth * p.dth * dtloc;
    let mut fcg = 1. * p.wdcg * 0.5 * (p.xfr - 1. / p.xfr) * dtloc;
    if flagll {
        fth = fth / RADIUS / DERA;
        fcg = fcg / RADIUS / DERA;
    }
    fcg = fcg / ntloc as Real;
    let fthx = -fth * esin;
    let fthy = fth * ecos;
    let fcgx = fcg * ecos;
    let fcgy = fcg * esin;

    let ns = nsea as usize;
    each!(isea, 1, nsea, {
        let i = up(isea);
        let ixy = t.mapsf[i + 2 * ns];
        let c = up(ixy);
        let tmpx = fthx / t.clats[i];
        let tmpy = fthy;
        let dxcgn = t.dpdx[c] * tmpx + t.dpdy[c] * tmpy;
        let dycgn = t.dqdx[c] * tmpx + t.dqdy[c] * tmpy;
        let tmpx = fcgx / t.clats[i];
        let tmpy = fcgy;
        let dxcgs = t.dpdx[c] * tmpx + t.dpdy[c] * tmpy;
        let dycgs = t.dqdx[c] * tmpx + t.dqdy[c] * tmpy;
        let (a, b, r1, r2) = corners(dxcgn + dxcgs, dycgn + dycgs, ny);
        w.iixy1[i] = a;
        w.iixy2[i] = b;
        w.rdi1[i] = r1;
        w.rdi2[i] = r2;
        let (a, b, r1, r2) = corners(dxcgn - dxcgs, dycgn - dycgs, ny);
        w.iixy3[i] = a;
        w.iixy4[i] = b;
        w.rdi3[i] = r1;
        w.rdi4[i] = r2;
    });

    let cgi = ik as usize;
    let cgs = nk2 as usize;
    each!(isea, 1, nsea, {
        let i = up(isea);
        let ixy = t.mapsf[i + 2 * ns];
        let cg = t.cg[cgi + cgs * (isea as usize)];
        vq[lo(ixy, ny)] = vq[lo(ixy, ny)] / cg * t.clats[i];
        w.cxtot[up(ixy)] = ecos * cg / t.clats[i];
        w.cytot[up(ixy)] = esin * cg;
    });

    each!(isea, 1, nsea, {
        let i = up(isea);
        let c = up(t.mapsf[i + 2 * ns]);
        let cp = w.cxtot[c] * t.dpdx[c] + w.cytot[c] * t.dpdy[c];
        let cq = w.cxtot[c] * t.dqdx[c] + w.cytot[c] * t.dqdy[c];
        w.vlcflx[c] = cp * dtrad;
        w.vlcfly[c] = cq * dtrad;
    });

    let gx = Qck {
        mx: nx,
        my: ny,
        nx,
        ny,
        close: global,
        inc: ny,
        nact: p.nact,
        nb0: p.nmx0,
        nb1: p.nmx1,
        nb2: p.nmx2,
    };
    let gy = Qck { close: false, inc: 1, nb0: p.nmy0, nb1: p.nmy1, nb2: p.nmy2, ..gx };
    let nq = vq.len();

    each!(_itloc, 1, ntloc, {
        let mut i = 0;
        while i < nq {
            w.aq[i] = vq[i];
            vq[i] = 0.;
            i += 1;
        }

        // Interior points need no test of their neighbours.
        each!(ip, 1, p.ncent, {
            let isea = t.mapcxy[up(ip)];
            let i = up(isea);
            let ixy = t.mapsf[i + 2 * ns];
            let aq = w.aq[lo(ixy, ny)];
            if t.maptrn[up(ixy)] != 0 {
                vq[lo(ixy, ny)] = aq;
            } else {
                let cg = t.cg[cgi + cgs * (isea as usize)];
                let rd1 = w.rdi1[i];
                let rd2 = 1f32.min(w.rdi2[i] * cg);
                let rd3 = w.rdi3[i];
                let rd4 = 1f32.min(w.rdi4[i] * cg);
                let (a1, a2, a3, a4) = (w.iixy1[i], w.iixy2[i], w.iixy3[i], w.iixy4[i]);
                vq[lo(ixy, ny)] = vq[lo(ixy, ny)] + aq * (3. - rd2 - rd4) / 3.;
                spread(vq, ixy, a1, true, aq * rd2 * rd1 / 6., ny);
                spread(vq, ixy, a2, true, aq * (1. - rd1) * rd2 / 6., ny);
                spread(vq, ixy, a3, true, aq * rd4 * rd3 / 6., ny);
                spread(vq, ixy, a4, true, aq * (1. - rd3) * rd4 / 6., ny);
                spread(vq, ixy, -a1, true, aq * rd2 * rd1 / 6., ny);
                spread(vq, ixy, -a2, true, aq * (1. - rd1) * rd2 / 6., ny);
                spread(vq, ixy, -a3, true, aq * rd4 * rd3 / 6., ny);
                spread(vq, ixy, -a4, true, aq * (1. - rd3) * rd4 / 6., ny);
            }
        });

        // Points next to land or a boundary.
        each!(ip, p.ncent + 1, nsea, {
            let isea = t.mapcxy[up(ip)];
            let i = up(isea);
            let ixy = t.mapsf[i + 2 * ns];
            if t.mapsta[up(ixy)] > 0 {
                let aq = w.aq[lo(ixy, ny)];
                if t.maptrn[up(ixy)] != 0 {
                    vq[lo(ixy, ny)] = aq;
                } else {
                    let cg = t.cg[cgi + cgs * (isea as usize)];
                    let rd1 = w.rdi1[i];
                    let rd3 = w.rdi3[i];
                    let rd2 = 1f32.min(w.rdi2[i] * cg);
                    let rd4 = 1f32.min(w.rdi4[i] * cg);
                    let (a1, a2, a3, a4) = (w.iixy1[i], w.iixy2[i], w.iixy3[i], w.iixy4[i]);
                    let m = &*w.mapstx;
                    vq[lo(ixy, ny)] = vq[lo(ixy, ny)] + aq * (3. - rd2 - rd4) / 3.;

                    let ixc = if a1 >= 0 { ny } else { -ny };
                    let iyc = a1 - ixc;
                    let f = aq * rd2 * rd1 / 6.;
                    let o = stx(m, ixy + a1, ny) >= 1
                        && !(stx(m, ixy + ixc, ny) <= 0 && stx(m, ixy + iyc, ny) <= 0);
                    spread(vq, ixy, a1, o, f, ny);
                    let o = stx(m, ixy - a1, ny) >= 1
                        && !(stx(m, ixy - ixc, ny) <= 0 && stx(m, ixy - iyc, ny) <= 0);
                    spread(vq, ixy, -a1, o, f, ny);

                    let f = aq * (1. - rd1) * rd2 / 6.;
                    spread(vq, ixy, a2, stx(m, ixy + a2, ny) >= 1, f, ny);
                    spread(vq, ixy, -a2, stx(m, ixy - a2, ny) >= 1, f, ny);

                    let ixc = if a3 >= 0 { ny } else { -ny };
                    let iyc = a3 - ixc;
                    let f = aq * rd4 * rd3 / 6.;
                    let o = stx(m, ixy + a3, ny) >= 1
                        && !(stx(m, ixy + ixc, ny) <= 0 && stx(m, ixy + iyc, ny) <= 0);
                    spread(vq, ixy, a3, o, f, ny);
                    let o = stx(m, ixy - a3, ny) >= 1
                        && !(stx(m, ixy - ixc, ny) <= 0 && stx(m, ixy - iyc, ny) <= 0);
                    spread(vq, ixy, -a3, o, f, ny);

                    let f = aq * (1. - rd3) * rd4 / 6.;
                    spread(vq, ixy, a4, stx(m, ixy + a4, ny) >= 1, f, ny);
                    spread(vq, ixy, -a4, stx(m, ixy - a4, ny) >= 1, f, ny);
                }
            }
        });

        each!(ixy, 1, nxy, {
            if t.mapsta[up(ixy)] == 2 {
                vq[lo(ixy, ny)] = w.aq[lo(ixy, ny)];
            }
        });

        if global {
            each!(iy, 1, ny, {
                vq[lo(iy, ny)] = vq[lo(iy, ny)] + vq[lo(nxy + iy, ny)];
                vq[lo((nx - 1) * ny + iy, ny)] = vq[lo((nx - 1) * ny + iy, ny)] + vq[lo(iy - ny, ny)];
            });
        }

        each!(isea, 1, nsea, {
            let ixy = t.mapsf[up(isea) + 2 * ns];
            vq[lo(ixy, ny)] = vq[lo(ixy, ny)] * t.gsqrt[up(ixy)];
        });

        if yfirst {
            if p.flcy != 0 {
                w3qck3(&gy, w.vlcfly, t.atrny, vq, t.mapaxy, t.mapy2, w.fla);
            }
            if p.flcx != 0 {
                w3qck3(&gx, w.vlcflx, t.atrnx, vq, t.mapaxy, t.mapx2, w.fla);
            }
        } else {
            if p.flcx != 0 {
                w3qck3(&gx, w.vlcflx, t.atrnx, vq, t.mapaxy, t.mapx2, w.fla);
            }
            if p.flcy != 0 {
                w3qck3(&gy, w.vlcfly, t.atrny, vq, t.mapaxy, t.mapy2, w.fla);
            }
        }

        each!(isea, 1, nsea, {
            let ixy = t.mapsf[up(isea) + 2 * ns];
            vq[lo(ixy, ny)] = vq[lo(ixy, ny)] / t.gsqrt[up(ixy)];
        });

        yfirst = !yfirst;
    });

    each!(isea, 1, nsea, {
        let i = up(isea);
        let ixy = t.mapsf[i + 2 * ns];
        if t.mapsta[up(ixy)] > 0 {
            let cg = t.cg[cgi + cgs * (isea as usize)];
            vq[lo(ixy, ny)] = 0f32.max(cg / t.clats[i] * vq[lo(ixy, ny)]);
        }
    });
}

/// The scalars of `W3KTP3`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Ktp3Params {
    pub nk: i32,
    pub nth: i32,
    pub nspec: i32,
    pub flcth: u32,
    pub flck: u32,
    pub dmin: Real,
    pub ctmax: Real,
    pub fachfa: Real,
}

pub struct Ktp3Tables<'a> {
    pub sig: &'a [Real],
    pub dsip: &'a [Real],
    pub ecos: &'a [Real],
    pub esin: &'a [Real],
    pub ec2: &'a [Real],
    pub esc: &'a [Real],
    pub es2: &'a [Real],
    pub mapwn: &'a [i32],
    pub mapth2: &'a [i32],
    pub mapwn2: &'a [i32],
}

/// One sea point of `W3KTP3`. With no currents, `FLCUR` is false and `DCDX`, `DCDY` are unused.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Ktp3Point {
    pub itime: i32,
    pub facth: Real,
    pub fack: Real,
    pub cthg0: Real,
    pub dw: Real,
    pub dddx: Real,
    pub dddy: Real,
    pub cx: Real,
    pub cy: Real,
    pub dcxdx: Real,
    pub dcxdy: Real,
    pub dcydx: Real,
    pub dcydy: Real,
}

pub struct Ktp3Work<'a> {
    pub dsdd: &'a mut [Real],
    pub frk: &'a mut [Real],
    pub frg: &'a mut [Real],
    pub fkc: &'a mut [Real],
    pub vq: &'a mut [Real],
    pub db: &'a mut [Real],
    pub dm: &'a mut [Real],
    pub vcflt: &'a mut [Real],
    pub cflk: &'a mut [Real],
    pub fla: &'a mut [Real],
}

/// `W3KTP3`. `cg` and `wn` are `(0:NK+1)` of this sea point.
pub fn w3ktp3(p: &Ktp3Params, t: &Ktp3Tables, q: &Ktp3Point, cg: &[Real], wn: &[Real], va: &mut [Real], w: &mut Ktp3Work) {
    let (nk, nth, nspec) = (p.nk, p.nth, p.nspec);
    let nk2 = nk + 2;
    let (flcth, flck) = (p.flcth != 0, p.flck != 0);
    let depth = p.dmin.max(q.dw);
    zero(w.vq);
    if flcth {
        zero(w.vcflt);
    }
    if flck {
        zero(w.cflk);
    }
    each!(ik, 0, nk + 1, {
        let k = ik as usize;
        w.dsdd[k] = if depth * wn[k] < 5. {
            0f32.max(cg[k] * wn[k] - 0.5 * t.sig[k]) / depth
        } else {
            0.
        };
    });
    each!(isp, 1, nspec, {
        w.vq[lo(t.mapth2[up(isp)], nk2)] = va[up(isp)];
    });

    if flcth {
        let fdg = q.facth * q.cthg0;
        each!(ik, 1, nk, {
            let k = ik as usize;
            w.frk[up(ik)] = q.facth * w.dsdd[k] / wn[k];
            w.frg[up(ik)] = fdg * cg[k];
        });
        each!(isp, 1, nspec, {
            let i = up(isp);
            let c = up(t.mapth2[i]);
            let n = up(t.mapwn[i]);
            let velnofilt = w.vcflt[c]
                + w.frg[n] * t.ecos[i]
                + w.frk[n] * (t.esin[i] * q.dddx - t.ecos[i] * q.dddy);
            w.vcflt[c] = sign(velnofilt.abs().min(p.ctmax), velnofilt);
        });
    }

    let nk2u = nk2 as usize;
    if flck {
        let dcxx = -q.dcxdx;
        let dcxyyx = -(q.dcxdy + q.dcydx);
        let dcyy = -q.dcydy;
        let fkd = q.cx * q.dddx + q.cy * q.dddy;
        each!(ith, 1, nth, {
            let i = up(ith);
            w.fkc[i] = t.ec2[i] * dcxx + t.esc[i] * dcxyyx + t.es2[i] * dcyy;
        });
        // `DM(NK2,0:NTH+1)`: column ITH starts at `ITH*NK2`.
        each!(ik, 0, nk, {
            let k = ik as usize;
            w.db[k] = t.dsip[k] / cg[k];
            w.dm[k + nk2u] = wn[k + 1] - wn[k];
        });
        w.db[nk2u - 1] = t.dsip[nk2u - 1] / cg[nk2u - 1];
        w.dm[2 * nk2u - 1] = 0.;
        each!(ith, 2, nth, {
            let c = up(ith) * nk2u;
            each!(ik, 0, nk2 - 1, {
                let k = ik as usize;
                w.db[k + c] = w.db[k];
                w.dm[k + c + nk2u] = w.dm[k + nk2u];
            });
        });
        each!(ik, 0, nk + 1, {
            let k = ik as usize;
            let fkd0 = fkd / cg[k] * w.dsdd[k];
            let velfac = q.fack / w.db[k];
            each!(ith, 1, nth, {
                let velnofilt = (fkd0 + wn[k] * w.fkc[up(ith)]) * velfac;
                w.cflk[k + up(ith) * nk2u] = sign(velnofilt.abs().min(p.ctmax), velnofilt) / velfac;
            });
        });
    }

    let gk = Qck {
        mx: nth,
        my: nk2,
        nx: nth,
        ny: nk2,
        close: false,
        inc: 1,
        nact: nspec,
        nb0: nspec - nth,
        nb1: nspec,
        nb2: nspec + nth,
    };
    let gt = Qck { close: true, inc: nk2, nb0: nspec, nb1: nspec, nb2: nspec, ..gk };
    if q.itime % 2 == 0 {
        if flck {
            each!(ith, 1, nth, {
                w.vq[lo(nk + 2 + (ith - 1) * nk2, nk2)] = p.fachfa * w.vq[lo(nk + 1 + (ith - 1) * nk2, nk2)];
            });
            w3qck2(&gk, w.cflk, q.fack, w.db, w.dm, w.vq, t.mapth2, t.mapwn2, w.fla);
        }
        if flcth {
            w3qck1(&gt, w.vcflt, w.vq, t.mapth2, t.mapth2, w.fla);
        }
    } else {
        if flcth {
            w3qck1(&gt, w.vcflt, w.vq, t.mapth2, t.mapth2, w.fla);
        }
        if flck {
            each!(ith, 1, nth, {
                w.vq[lo(nk + 2 + (ith - 1) * nk2, nk2)] = p.fachfa * w.vq[lo(nk + 1 + (ith - 1) * nk2, nk2)];
            });
            w3qck2(&gk, w.cflk, q.fack, w.db, w.dm, w.vq, t.mapth2, t.mapwn2, w.fla);
        }
    }
    each!(isp, 1, nspec, {
        va[up(isp)] = w.vq[lo(t.mapth2[up(isp)], nk2)];
    });
}
