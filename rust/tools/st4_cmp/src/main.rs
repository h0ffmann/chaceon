// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SIN4` or `W3SPR4` calls from `regtests/capture/` and compares bits with the reference.
//! Usage: st4_cmp <sin4|spr4> <capture>. A diagnostic of ADR-0002, never a test.

use ww3_host::capture::{Sin4Capture, Spr4Capture};
use ww3_host::insin4::{Insin4Input, insin4, tabu_fw, tabu_stress, tabu_tauhf, tabu_tauhf2};
use ww3_kernels::sin4::{Sin4Out, Spr4Out, w3sin4, w3spr4};

#[derive(Default)]
struct Tally {
    n: u64,
    bad: u64,
    max: u64,
    nonzero: u64,
    first: Option<(usize, f32, f32)>,
}

fn key(x: f32) -> i64 {
    let u = x.to_bits() as i64;
    if u & 0x8000_0000 != 0 { 0x8000_0000 - u } else { u }
}

impl Tally {
    fn add(&mut self, ours: &[f32], theirs: &[f32]) {
        for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
            self.n += 1;
            self.nonzero += (*b != 0.) as u64;
            if a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan()) {
                self.bad += 1;
                self.max = self.max.max((key(*a) - key(*b)).unsigned_abs());
                self.first.get_or_insert((i, *a, *b));
            }
        }
    }
    fn show(&self, name: &str) -> bool {
        println!(
            "{name:9}{:>10} values, {} differ, max {} ULP; {} nonzero {}",
            self.n,
            self.bad,
            self.max,
            self.nonzero,
            self.first.map_or(String::new(), |f| format!("first {f:?}"))
        );
        self.bad == 0 && self.n > 0
    }
}

fn sin4(path: &str) -> bool {
    let c = Sin4Capture::read(path);
    let p = c.params();
    let n = p.nspec as usize;
    let (mut s, mut d, mut l) = (vec![0.0; n], vec![0.0; n], vec![0u32; n]);
    let mut o = [Sin4Out::default()];
    let (mut ts, mut td, mut tl, mut to) =
        (Tally::default(), Tally::default(), 0u64, Tally::default());
    for k in &c.calls {
        w3sin4(
            &p, &c.sig, &c.sig2, &c.dden, &c.dden2, &c.ecos, &c.esin, &c.fwtable, &c.tauhft, &c.tauhft2, &k.a,
            &k.cg, &k.k, &k.point, &mut s, &mut d, &mut l, &mut o, 0, 0, 0,
        );
        ts.add(&s, &k.s);
        td.add(&d, &k.d);
        tl += l.iter().zip(&k.llws).filter(|(a, b)| a != b).count() as u64;
        let (a, b) = (o[0], k.out);
        to.add(&[a.tauwx, a.tauwy, a.tauwnx, a.tauwny], &[b.tauwx, b.tauwy, b.tauwnx, b.tauwny]);
    }
    println!("calls    {}", c.calls.len());
    let ustar = c.calls.iter().map(|k| k.point.ustar).fold(f32::INFINITY, f32::min);
    let bad_a = c.calls.iter().filter(|k| k.a.iter().any(|x| !x.is_finite())).count();
    let nan_s = c.calls.iter().filter(|k| k.s.iter().any(|x| !x.is_finite())).count();
    println!("reached  min USTAR {ustar:e}; {bad_a} calls with a non-finite A, {nan_s} with a non-finite S");
    println!("LLWS     {tl} differ");
    let calls = ts.show("S") & td.show("D") & to.show("TAUW") & (tl == 0);
    calls & (std::env::var_os("ST4_NO_TABLES").is_some() || tables(&c))
}

fn table(name: &str, ours: &[f32], theirs: &[f32]) -> bool {
    assert_eq!(ours.len(), theirs.len(), "{name}");
    let mut t = Tally::default();
    t.add(ours, theirs);
    t.show(name)
}

fn ints(name: &str, ours: &[i32], theirs: &[i32]) -> bool {
    assert_eq!(ours.len(), theirs.len(), "{name}");
    let bad = ours.iter().zip(theirs).filter(|(a, b)| a != b).count();
    println!("{name:9}{:>10} values, {bad} differ", ours.len());
    bad == 0
}

/// Rebuilds every table from what the namelists give and compares it with the reference's.
fn tables(c: &Sin4Capture) -> bool {
    let (i, f) = (&c.ints, &c.reals);
    let (bbeta, zzwnd, aalpha, zzalp, shelter, dth) = (f[0], f[1], f[2], f[3], f[4], f[7]);
    let (zz0max, xfr) = (f[8], f[10]);
    let (nk, nth) = (i[0], i[1]);
    let sigmax = c.sig[nk as usize];
    let mut ok = true;
    let (deltauw, delu, taut) = tabu_stress(zzwnd, aalpha, zz0max);
    ok &= table("TAUT", &taut, &c.taut);
    let (delust, delalp, tauhft) = tabu_tauhf(sigmax, aalpha, bbeta, zzalp, zz0max);
    if shelter > 0. {
        let (deltail, t1, t2) = tabu_tauhf2(sigmax, aalpha, bbeta, zzalp, delust, delalp);
        ok &= table("TAUHFT", &t1, &c.tauhft);
        ok &= table("TAUHFT2", &t2, &c.tauhft2);
        ok &= table("DELTAIL", &[deltail], &f[20..21]);
    } else {
        ok &= table("TAUHFT", &tauhft, &c.tauhft);
    }
    let (delab, fw) = tabu_fw();
    ok &= table("FWTABLE", &fw, &c.fwtable);
    ok &= table("DELS", &[delust, deltauw, delu, delalp, delab], &[f[19], f[21], f[22], f[23], f[24]]);
    let o = insin4(&Insin4Input {
        nk,
        nth,
        nkhs: i[3],
        nkd: i[4],
        ndtab: i[5],
        sat_n: c.satindices.extent,
        dth,
        xfr,
        ssdsdth: f[18],
        ssdscos: f[11],
        ssdsbrf1: f[12],
        ssdsbint: f[13],
        ssdspbk: f[14],
        ssdsabk: f[15],
        ssdshck: f[16],
        ssdsbck: f[17],
        ssdsc3: c.ssdsc[2],
        sig: &c.sig,
        dsip: &c.dsip,
        th: &c.th,
        ecos: &c.ecos,
    });
    ok &= ints("SATINDIC", &o.satindices, &c.satindices.data);
    ok &= table("SATWEIGH", &o.satweights, &c.satweights.data);
    ok &= table("CUMULW", &o.cumulw, &c.cumulw.data);
    ok &= ints("IKTAB", &o.iktab, &c.iktab.data);
    ok &= table("DCKI", &o.dcki, &c.dcki.data);
    ok &= table("QBI", &o.qbi, &c.qbi.data);
    ok &= ints("DIKCUMUL", &[o.dikcumul], &[i[7]]);
    ok
}

fn spr4(path: &str) -> bool {
    let c = Spr4Capture::read(path);
    let p = c.params;
    let mut w = vec![0.0; 2 * p.nk as usize];
    let mut o = [Spr4Out::default()];
    let mut t = Tally::default();
    for k in &c.calls {
        w3spr4(
            &p, &c.sig, &c.dden, &c.ecos, &c.esin, &c.taut, &k.a, &k.cg, &k.wn, &k.llws, &k.point, &mut w, &mut o,
            0, 0, 0, 0,
        );
        let a = o[0];
        let ours = [
            a.emean, a.fmean, a.fmean1, a.wnmean, a.amax, a.ustar, a.usdir, a.cd, a.z0, a.charn, a.fmeanws,
            a.dlwmean,
        ];
        t.add(&ours, &k.out);
    }
    println!("calls    {}", c.calls.len());
    t.show("W3SPR4")
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (what, path) = (args.next().expect("sin4|spr4"), args.next().expect("capture"));
    let ok = match what.as_str() {
        "sin4" => sin4(&path),
        "spr4" => spr4(&path),
        other => panic!("unknown {other}"),
    };
    println!("{}", if ok { "PARITY: every bit equal" } else { "DIFFERENT" });
    std::process::exit(!ok as i32);
}
