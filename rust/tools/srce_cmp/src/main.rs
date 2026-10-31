// SPDX-License-Identifier: MPL-2.0
//! Replays `W3SRCE` calls from `regtests/capture/` and compares bits with the reference. Tables come
//! from the same case's other captures. Usage: srce_cmp <variant>_<depth>m. A diagnostic of ADR-0002.

use ww3_host::capture::{Sds4Capture, Sin4Capture, Snl1Capture, Spr4Capture, SrceCapture};
use ww3_kernels::sin4::{Sin4Out, Spr4Out};
use ww3_kernels::srce::{SrceOut, SrceTables, SrceWork, w3srce, work_lens, work_small};

#[derive(Default)]
struct Tally {
    n: u64,
    bad: u64,
    max: u64,
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
            if a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan()) {
                self.bad += 1;
                self.max = self.max.max((key(*a) - key(*b)).unsigned_abs());
                self.first.get_or_insert((call, i, *a, *b));
            }
        }
    }
    fn show(&self, name: &str) -> bool {
        println!(
            "{name:9}{:>9} values, {} differ, max {} ULP {}",
            self.n,
            self.bad,
            self.max,
            self.first.map_or(String::new(), |f| format!("first {f:?}"))
        );
        self.bad == 0 && self.n > 0
    }
}

fn outs(o: &SrceOut) -> [f32; 25] {
    [
        o.ustar, o.ustdir, o.dtdyn, o.fcut, o.tauwx, o.tauwy, o.tauox, o.tauoy, o.tauwix, o.tauwiy, o.tauwnx,
        o.tauwny, o.phiaw, o.charn, o.tws, o.phioc, o.phibbl, o.taubbl[0], o.taubbl[1], o.tauice[0], o.tauice[1],
        o.phice, o.tauocx, o.tauocy, o.wnmean,
    ]
}

fn main() {
    let tag = std::env::args().nth(1).expect("variant_depthm");
    let path = |r: &str| format!("/work/capture/{r}_{tag}.bin");
    let c = SrceCapture::read(&path("srce"));
    let snl = Snl1Capture::read(&path("snl1"));
    let sds = Sds4Capture::read(&path("sds4"));
    let sin = Sin4Capture::read(&path("sin4"));
    let spr = Spr4Capture::read(&path("spr4"));
    let st = snl.rebuild();
    let p = c.params(st.params, sds.params, sin.params(), spr.params);
    let t = SrceTables {
        sig: &sin.sig,
        sig2: &sin.sig2,
        dden: &sin.dden,
        dden2: &sin.dden2,
        ec2: &sds.ec2,
        es2: &sds.es2,
        esc: &sds.esc,
        ecos: &sin.ecos,
        esin: &sin.esin,
        af11: &st.af11,
        pm: &st.pm,
        ic: &st.ic,
        satindices: &sds.satindices,
        satweights: &sds.satweights,
        cumulw: &sds.cumulw,
        iktab: &sds.iktab,
        dcki: &sds.dcki,
        qbi: &sds.qbi,
        fwtable: &sin.fwtable,
        tauhft: &sin.tauhft,
        tauhft2: &sin.tauhft2,
        taut: &sin.taut,
    };
    let n = p.nspec as usize;
    let (l_snl, l_sds, l_ids) = work_lens(&p);
    let mut bufs: Vec<Vec<f32>> = (0..17).map(|_| vec![0.0; n]).collect();
    let (mut llws, mut w_snl, mut w_sds, mut iw_sds) =
        (vec![0u32; n], vec![0.0; l_snl], vec![0.0; l_sds], vec![0i32; l_ids]);
    let (mut w_small, mut whitecap) = (vec![0.0; work_small(&p)], vec![0.0; 4]);
    let (mut sin_out, mut spr_out) = ([Sin4Out::default()], [Spr4Out::default()]);
    let (mut ts, mut to, mut tw) = (Tally::default(), Tally::default(), Tally::default());
    let mut out = [SrceOut::default()];
    let mut seen = [0u64; 6];
    for (i, k) in c.calls.iter().enumerate() {
        let mut spec = k.spec.clone();
        let [specinit, dam, wn2, vsln, vsin, vdin, vsnl, vdnl, vsds, vdds, vsbt, vdbt, vsdb, vddb, vs, vd, brlambda] =
            bufs.iter_mut().map(|b| b.as_mut_slice()).collect::<Vec<_>>().try_into().ok().unwrap();
        let mut w = SrceWork {
            specinit, dam, wn2, vsln, vsin, vdin, vsnl, vdnl, vsds, vdds, vsbt, vdbt, vsdb, vddb, vs, vd, brlambda,
            llws: &mut llws,
            w_snl: &mut w_snl,
            w_sds: &mut w_sds,
            iw_sds: &mut iw_sds,
            w_small: &mut w_small,
            whitecap: &mut whitecap,
            sin_out: &mut sin_out,
            spr_out: &mut spr_out,
        };
        w3srce(&p, &t, &k.point, &mut spec, &k.wn1, &k.cg1, &mut w, &mut out, 0);
        ts.add(i, &spec, &k.spec_out);
        to.add(i, &outs(&out[0]), &k.out);
        tw.add(i, &whitecap, &k.whitecap);
        seen[0] += (k.point.it == 0) as u64;
        seen[1] += (k.point.srce_call != 0) as u64;
        seen[2] += out[0].phibbl.abs().gt(&0.) as u64;
        seen[3] += (k.spec.iter().sum::<f32>() < 1e-14) as u64;
        // vsdb is `bufs[12]` and vsbt `bufs[10]`, from the last pass of the integration.
        seen[4] += bufs[12].iter().any(|x| *x != 0.) as u64;
        seen[5] += bufs[10].iter().any(|x| *x != 0.) as u64;
    }
    println!(
        "calls    {}; IT=0: {}; implicit: {}; with PHIBBL: {}; near-empty spectrum: {}; W3SDB1 active: {}; W3SBT1 active: {}",
        c.calls.len(), seen[0], seen[1], seen[2], seen[3], seen[4], seen[5]
    );
    let ok = ts.show("SPEC") & to.show("scalars") & tw.show("WHITECAP");
    println!("{}", if ok { "PARITY: every bit equal" } else { "DIFFERENT" });
    std::process::exit(!ok as i32);
}
