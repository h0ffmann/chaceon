// SPDX-License-Identifier: MPL-2.0
//! `TICK21` and `DSEC21` (`model/src/w3timemd.F90`), for the `standard` calendar. `DSEC21` is `f32`, as in
//! WW3: it enters the wind interpolation.

use ww3_kernels::nint;
use ww3_kernels::real::Real;

pub type Time = [i32; 2];

fn leap(ny: i32) -> bool {
    ny % 400 == 0 || (ny % 4 == 0 && ny % 100 != 0)
}

const NDPM: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn iymd21(nymd: i32, m: i32) -> i32 {
    let mut ny = nymd / 10000;
    let mut nm = (nymd % 10000 / 100).clamp(1, 12);
    let mut nd = nymd % 100 + m;
    let lp = leap(ny);
    if nd == 0 {
        nm -= 1;
        if nm == 0 {
            nm = 12;
            ny -= 1;
        }
        nd = NDPM[(nm - 1) as usize];
        if nm == 2 && lp {
            nd = 29;
        }
    }
    if !(nd == 29 && nm == 2 && lp) && nd > NDPM[(nm - 1) as usize] {
        nd = 1;
        nm += 1;
        if nm > 12 {
            nm = 1;
            ny += 1;
        }
    }
    ny * 10000 + nm * 100 + nd
}

pub fn tick21(time: &mut Time, dtime: Real) {
    let (mut nymd, nhms) = (time[0], time[1]);
    if dtime == 0. {
        nymd = iymd21(nymd, -1);
        nymd = iymd21(nymd, 1);
    }
    let mut nsec = nhms / 10000 * 3600 + nhms % 10000 / 100 * 60 + nhms % 100 + nint(dtime);
    while nsec >= 86400 {
        nsec -= 86400;
        nymd = iymd21(nymd, 1);
    }
    while nsec < 0 {
        nsec += 86400;
        nymd = iymd21(nymd, -1);
    }
    *time = [nymd, nsec / 3600 * 10000 + nsec % 3600 / 60 * 100 + nsec % 60];
}

fn mymd21(nymd: i32) -> i32 {
    let (ny, mut nm, mut nd) = (nymd / 10000, nymd % 10000 / 100, nymd % 100);
    if nm > 2 && leap(ny) {
        nd += 1;
    }
    while nm > 1 {
        nm -= 1;
        nd += NDPM[(nm - 1) as usize];
    }
    nd
}

fn seconds(hms: i32) -> i32 {
    hms / 10000 * 3600 + hms % 10000 / 100 * 60 + hms % 100
}

pub fn dsec21(t1: Time, t2: Time) -> Real {
    let (ny1, mut ny2) = (t1[0] / 10000, t2[0] / 10000);
    let mut nd = mymd21(t2[0]) - mymd21(t1[0]);
    while ny1 != ny2 {
        if ny2 > ny1 {
            ny2 -= 1;
            nd += mymd21(ny2 * 10000 + 1231);
        } else {
            nd -= mymd21(ny2 * 10000 + 1231);
            ny2 += 1;
        }
    }
    (seconds(t2[1]) - seconds(t1[1])) as Real + 86400. * nd as Real
}
