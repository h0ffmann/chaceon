// SPDX-License-Identifier: MPL-2.0
//! Replays what `regtests/capture/snl1.py` recorded: rebuilds the tables with `INSNL1` and runs
//! `W3SNL1` on each captured spectrum, comparing bits with the reference. A diagnostic of ADR-0002,
//! never a test. Exit 0 only when no bit differs.

use ww3_host::capture::Snl1Capture;
use ww3_kernels::snl1::{w3snl1, work_len};

fn ulps(a: f32, b: f32) -> u64 {
    let key = |x: f32| {
        let u = x.to_bits() as i64;
        if u & 0x8000_0000 != 0 { 0x8000_0000 - u } else { u }
    };
    (key(a) - key(b)).unsigned_abs()
}

#[derive(Default)]
struct Tally {
    n: u64,
    bad: u64,
    max: u64,
    nonzero: u64,
    peak: f32,
}

impl Tally {
    fn add(&mut self, ours: &[f32], theirs: &[f32]) {
        for (a, b) in ours.iter().zip(theirs) {
            self.n += 1;
            self.nonzero += (*b != 0.) as u64;
            self.peak = self.peak.max(b.abs());
            if a.to_bits() != b.to_bits() {
                self.bad += 1;
                self.max = self.max.max(ulps(*a, *b));
            }
        }
    }
    fn show(&self, name: &str) {
        println!(
            "{name:8} {:>9} values, {} differ, max {} ULP; {} nonzero, peak {:e}",
            self.n, self.bad, self.max, self.nonzero, self.peak
        );
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("capture file");
    let c = Snl1Capture::read(&path);
    let (ints, f, af11, pm, ic) = (&c.ints, &c.reals, &c.af11, &c.pm, &c.ic);
    let (nspec, t) = (ints[2], c.rebuild());
    let p = t.params;

    let mut ok = true;
    let scalars = [p.dal1, p.dal2, p.dal3];
    let mut tab = Tally::default();
    tab.add(&scalars, &f[10..13]);
    tab.add(&p.awg, &f[13..21]);
    tab.add(&p.swg, &f[21..29]);
    tab.add(&t.af11, af11);
    tab.show("real tab");
    ok &= tab.bad == 0;
    let same = [p.nfrhgh, p.nfrchg, p.nspecx, p.nspecy] == [ints[4], ints[5], ints[6], ints[7]];
    println!("sizes    {}", if same { "equal" } else { "DIFFER" });
    println!("pm       {} of {} equal", t.pm.iter().zip(pm).filter(|(a, b)| a == b).count(), pm.len());
    println!("ic       {} of {} equal", t.ic.iter().zip(ic).filter(|(a, b)| a == b).count(), ic.len());
    ok &= same && t.pm == *pm && t.ic == *ic;

    let mut w = vec![0.0; work_len(&p)];
    let (mut s, mut d) = (vec![0.0; nspec as usize], vec![0.0; nspec as usize]);
    let (mut ts, mut td) = (Tally::default(), Tally::default());
    for call in &c.calls {
        let (a, cg, kd) = (&call.a, &call.cg, call.kdmean);
        w3snl1(&p, &c.sig, &t.af11, &t.pm, &t.ic, a, cg, kd, &mut s, &mut d, &mut w, 0, 0, 0);
        ts.add(&s, &call.s);
        td.add(&d, &call.d);
    }
    let calls = c.calls.len();
    println!("calls    {calls}");
    ts.show("S");
    td.show("D");
    ok &= calls > 0 && ts.bad == 0 && td.bad == 0 && ts.nonzero > 0 && td.nonzero > 0;
    println!("{}", if ok { "PARITY: every bit equal" } else { "DIFFERENT" });
    std::process::exit(!ok as i32);
}
