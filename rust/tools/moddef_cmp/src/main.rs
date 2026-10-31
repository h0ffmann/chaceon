// SPDX-License-Identifier: MPL-2.0
//! Compares `ModDef` with what the reference held in memory, from the captures of the same case. A
//! diagnostic of ADR-0002. Usage: moddef_cmp <mod_def.ww3> <tag>.

use ww3_host::capture::{Sds4Capture, Sin4Capture, Snl1Capture, Spr4Capture, SrceCapture};
use ww3_host::moddef::ModDef;

fn eq(bad: &mut u32, name: &str, a: &[f32], b: &[f32]) {
    let d = a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    if a.len() != b.len() || d != 0 {
        println!("{name}: {} and {} values, {d} differ", a.len(), b.len());
        *bad += 1;
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (path, tag) = (args.next().expect("mod_def"), args.next().expect("tag"));
    let m = ModDef::read(&path);
    let cap = |r: &str| format!("/work/capture/{r}_{tag}.bin");
    let (sn, sd, si, sp, sr) = (
        Snl1Capture::read(&cap("snl1")),
        Sds4Capture::read(&cap("sds4")),
        Sin4Capture::read(&cap("sin4")),
        Spr4Capture::read(&cap("spr4")),
        SrceCapture::read(&cap("srce")),
    );
    let mut b = 0;
    let t = m.snl1();
    eq(&mut b, "sig", &m.sig, &si.sig);
    eq(&mut b, "sig2", &m.sig2, &si.sig2);
    eq(&mut b, "dden", &m.dden, &si.dden);
    eq(&mut b, "dden2", &m.dden2, &si.dden2);
    eq(&mut b, "ecos", &m.ecos, &si.ecos);
    eq(&mut b, "esin", &m.esin, &si.esin);
    eq(&mut b, "ec2", &m.ec2, &sd.ec2);
    eq(&mut b, "es2", &m.es2, &sd.es2);
    eq(&mut b, "esc", &m.esc, &sd.esc);
    eq(&mut b, "th", &m.th, &si.th);
    eq(&mut b, "dsip", &m.dsip, &si.dsip);
    eq(&mut b, "fwtable", &m.fwtable, &si.fwtable);
    eq(&mut b, "taut", &m.taut, &si.taut);
    eq(&mut b, "tauhft", &m.tauhft, &si.tauhft);
    eq(&mut b, "tauhft2", &m.tauhft2, &si.tauhft2);
    eq(&mut b, "satweights", &m.satweights, &sd.satweights);
    eq(&mut b, "cumulw", &m.cumulw, &sd.cumulw);
    eq(&mut b, "dcki", &m.dcki, &sd.dcki);
    eq(&mut b, "qbi", &m.qbi, &sd.qbi);
    eq(&mut b, "af11", &t.af11, &sn.af11);
    if m.satindices != sd.satindices || m.iktab != sd.iktab || t.pm != sn.pm || t.ic != sn.ic {
        println!("integer tables differ");
        b += 1;
    }
    let bytes = |s: &[u8]| s.iter().map(|x| *x as f32).collect::<Vec<_>>();
    fn raw<T>(v: &T) -> &[u8] {
        unsafe { std::slice::from_raw_parts((v as *const T).cast(), size_of::<T>()) }
    }
    let p = m.srce(t.params, (sd.params.flogrd57 != 0, sd.params.flogrd58 != 0));
    let want = sr.params(sn.rebuild().params, sd.params, si.params(), sp.params);
    eq(&mut b, "SrceParams", &bytes(raw(&p)), &bytes(raw(&want)));
    println!("{} sea points, {}x{}, DTMAX {}, FLSOU {}", m.nsea, m.nx, m.ny, m.dtmax, m.flsou);
    println!("{}", if b == 0 { "PARITY: mod_def equals the reference's memory" } else { "DIFFERENT" });
    std::process::exit((b != 0) as i32);
}
