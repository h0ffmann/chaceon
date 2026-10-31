// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SDS4` calls recorded by `regtests/capture/sds4.py` and compares bits with the
//! reference. A diagnostic of ADR-0002, never a test. Exit 0 only when no bit differs.

use ww3_host::capture::Sds4Capture;
use ww3_kernels::sds4::{w3sds4, work_len};

#[derive(Default)]
struct Tally {
    n: u64,
    bad: u64,
    max: u64,
    nonzero: u64,
    peak: f32,
    first: Option<(usize, usize, f32, f32)>,
}

fn key(x: f32) -> i64 {
    let u = x.to_bits() as i64;
    if u & 0x8000_0000 != 0 { 0x8000_0000 - u } else { u }
}

impl Tally {
    fn add(&mut self, call: usize, ours: &[f32], theirs: &[f32]) {
        for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
            self.n += 1;
            self.nonzero += (*b != 0.) as u64;
            self.peak = self.peak.max(b.abs());
            if a.to_bits() != b.to_bits() {
                self.bad += 1;
                self.max = self.max.max((key(*a) - key(*b)).unsigned_abs());
                self.first.get_or_insert((call, i, *a, *b));
            }
        }
    }
    fn show(&self, name: &str) {
        println!(
            "{name:9}{:>9} values, {} differ, max {} ULP; {} nonzero, peak {:e} {}",
            self.n,
            self.bad,
            self.max,
            self.nonzero,
            self.peak,
            self.first.map_or(String::new(), |f| format!("first {f:?}"))
        );
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("capture file");
    let c = Sds4Capture::read(&path);
    let p = c.params;
    println!(
        "scheme {} ssdsc8 {} ssdsc11 {} ssdsc18 {} brfdf {} dth {} bm0 {}",
        p.ssdsc[0], p.ssdsc[7], p.ssdsc[10], p.ssdsc[17], p.ssdsbrfdf, p.ssdsdth, p.ssdsbm[0]
    );
    let (lw, li) = work_len(&p);
    let (mut w, mut iw) = (vec![0.0; lw], vec![0; li]);
    let n = p.nspec as usize;
    let (mut s, mut d, mut b) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    let mut t = [Tally::default(), Tally::default(), Tally::default(), Tally::default()];
    for (i, k) in c.calls.iter().enumerate() {
        let mut wc = k.whitecap_in.clone();
        w3sds4(
            &p, &c.sig, &c.dden, &c.ec2, &c.es2, &c.esc, &c.ecos, &c.esin, &c.satindices, &c.satweights,
            &c.cumulw, &c.iktab, &c.dcki, &c.qbi, &k.a, &k.k, &k.cg, &k.point, &mut s, &mut d, &mut b, &mut wc,
            &mut w, &mut iw, 0, 0, 0, 0, 0,
        );
        t[0].add(i, &s, &k.srhs);
        t[1].add(i, &d, &k.ddiag);
        t[2].add(i, &b, &k.brlambda);
        t[3].add(i, &wc, &k.whitecap);
    }
    // Again in the layout a device uses: one buffer, every point at its offset.
    let np = c.calls.len();
    let cat = |f: fn(&ww3_host::capture::Sds4Call) -> &Vec<f32>| -> Vec<f32> {
        c.calls.iter().flat_map(|x| f(x).iter().copied()).collect()
    };
    let (a, k, cg) = (cat(|x| &x.a), cat(|x| &x.k), cat(|x| &x.cg));
    let (mut bs, mut bd, mut bb) = (vec![0.0; n * np], vec![0.0; n * np], vec![0.0; n * np]);
    let mut bw = cat(|x| &x.whitecap_in);
    let (mut w, mut iw) = (vec![0.0; lw * np], vec![0; li * np]);
    for (i, x) in c.calls.iter().enumerate() {
        w3sds4(
            &p, &c.sig, &c.dden, &c.ec2, &c.es2, &c.esc, &c.ecos, &c.esin, &c.satindices, &c.satweights,
            &c.cumulw, &c.iktab, &c.dcki, &c.qbi, &a, &k, &cg, &x.point, &mut bs, &mut bd, &mut bb, &mut bw,
            &mut w, &mut iw, i * n, i * p.nk as usize, i * 4, i * lw, i * li,
        );
    }
    let mut b = [Tally::default(), Tally::default(), Tally::default(), Tally::default()];
    b[0].add(0, &bs, &cat(|x| &x.srhs));
    b[1].add(0, &bd, &cat(|x| &x.ddiag));
    b[2].add(0, &bb, &cat(|x| &x.brlambda));
    b[3].add(0, &bw, &cat(|x| &x.whitecap));
    println!("calls    {}", c.calls.len());
    b[0].show("batch S");
    b[1].show("batch D");
    b[2].show("batch B");
    b[3].show("batch W");
    t[0].show("SRHS");
    t[1].show("DDIAG");
    t[2].show("BRLAMBDA");
    t[3].show("WHITECAP");
    let ok = !c.calls.is_empty()
        && t.iter().chain(b.iter()).all(|x| x.bad == 0)
        && t[0].nonzero > 0;
    println!("{}", if ok { "PARITY: every bit equal" } else { "DIFFERENT" });
    std::process::exit(!ok as i32);
}
