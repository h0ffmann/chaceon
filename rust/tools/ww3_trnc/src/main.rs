// SPDX-License-Identifier: MPL-2.0
//! `ww3_trnc` in Rust (`model/src/ww3_trnc.F90` at `92e2cf66`), with `W3NMLTRNC` (`w3nmltrncmd.F90`): reads `track_o.ww3` and
//! `ww3_trnc.nml` or `ww3_trnc.inp` from a directory and writes the NetCDF files there. `CALTYPE` is never set here, as
//! `mod_def.ww3` is not read: it stays eight NUL bytes, which is neither `standard` nor `360_day`, so the file's calendar
//! attribute holds them, `time` has no `long_name` and `TSUB` counts no leap days. `DSEC21` and `TICK21` have no leap days
//! either in the Fortran, and Rust's do: a track across 29 February is not reproduced ⚠. Without the `netcdf` feature the
//! program stops at once.

#[cfg(feature = "netcdf")]
mod crnc;
#[cfg(feature = "netcdf")]
#[path = "../../ww3_ounp/src/nc.rs"]
#[allow(dead_code)]
mod nc;

#[cfg(feature = "netcdf")]
mod run {
    use crate::{crnc, nc};
    use std::io::Write;
    use ww3_host::finp::{self, Inp};
    use ww3_host::fortfmt as ff;
    use ww3_host::gridio;
    use ww3_host::moddef::Records;
    use ww3_host::ounf::t2d;
    use ww3_host::outp::stme21;
    use ww3_host::wtime::{Time, dsec21, tick21};
    use ww3_kernels::constants::RADE;
    use ww3_kernels::real::Real;
    use ww3_libm::atan2f;

    const IDTST: &str = "WAVEWATCH III TRACK OUTPUT SPECTRA";
    const CALTYPE: &str = "\0\0\0\0\0\0\0\0";

    fn die(code: i32, msg: &str) -> ! {
        print!("{msg}");
        std::io::stdout().flush().unwrap();
        std::process::exit(code);
    }

    fn head(t: &str) -> String {
        format!("\n *** WAVEWATCH III ERROR IN W3TRNC : \n{t}\n")
    }

    /// `CHECK_ERR`.
    fn ck(iret: i32) {
        if iret != 0 {
            println!(" *** WAVEWATCH III ERROR IN TRNC :");
            println!(" NETCDF ERROR MESSAGE: ");
            println!(" {}", nc::strerror(iret));
            std::io::stdout().flush().unwrap();
            std::process::exit(59);
        }
    }

    /// `TSUB` with `CALTYPE` neither `standard` nor `360_day`: no leap days.
    fn tsub(t1: &[i32; 8], t2: &[i32; 8]) -> f64 {
        let day = |t: &[i32; 8]| {
            let a = (14 - t[1]) / 12;
            let b = t[0] + 4800 - a;
            let c = t[1] + 12 * a - 3;
            t[2] + (153 * c + 2) / 5 + 365 * b
        };
        let secs = |t: &[i32; 8]| -> f64 {
            let e: Real = 3600.0 * t[4] as Real + 60.0 * (t[5] - t[3]) as Real + t[6] as Real + t[7] as Real / 1000.0;
            f64::from(e)
        };
        f64::from(day(t2) - day(t1)) + (secs(t2) - secs(t1)) / 86400.0
    }

    fn datestr(d: &[i32; 8]) -> String {
        format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", d[0], d[1], d[2], d[4], d[5], d[6])
    }

    struct Req {
        tout: Time,
        dtreq: Real,
        nout: i32,
        nctype: i32,
        prefix: String,
        s3: i32,
    }

    /// `W3NMLTRNC` and the reads of its text, and `ww3_trnc.nml.log`.
    fn from_nml() -> Req {
        let t = std::fs::read_to_string("ww3_trnc.nml").unwrap();
        let (mut start, mut stride, mut count, mut split) = ("19000101 000000".to_string(), "0".to_string(), "1000000000".to_string(), 6);
        let (mut prefix, mut nctype) = ("ww3.".to_string(), 3);
        for g in gridio::groups(&t) {
            for (k, v) in &g.items {
                match k.as_str() {
                    "TRACK%TIMESTART" => start = gridio::text(v),
                    "TRACK%TIMESTRIDE" => stride = gridio::text(v),
                    "TRACK%TIMECOUNT" => count = gridio::text(v),
                    "TRACK%TIMESPLIT" => split = gridio::int(gridio::first(v)),
                    "FILE%PREFIX" => prefix = gridio::text(v),
                    "FILE%NETCDF" => nctype = gridio::int(gridio::first(v)),
                    other => panic!("{other} is not in ww3_trnc.nml"),
                }
            }
        }
        let log = format!(
            "\nTRACK %  TIMESTART  = {start}\nTRACK %  TIMESTRIDE = {stride}\nTRACK %  TIMECOUNT  = {count}\nTRACK %  TIMESPLIT  = {}\n\nFILE %  PREFIX    = {prefix}\nFILE %  NETCDF    = {}\n",
            ff::i(8, split as i64),
            ff::i(8, nctype as i64)
        );
        std::fs::write("ww3_trnc.nml.log", log).unwrap();
        let mut d = start.split_whitespace().map(|s| s.parse::<i32>().unwrap());
        let tout = [d.next().unwrap(), d.next().unwrap()];
        Req { tout, dtreq: gridio::real(&stride), nout: count.trim().parse().unwrap(), nctype, prefix, s3: split }
    }

    fn from_inp() -> Req {
        let mut i = Inp::open("ww3_trnc.inp").unwrap_or_else(|e| die(14, &head(&format!("     ERROR IN OPENING INPUT FILE\n     IOSTAT ={}\n", ff::i(5, e.raw_os_error().unwrap_or(2) as i64)))));
        let end = || -> ! { die(15, &head("     ERROR IN READING FROM INPUT FILE\n     IOSTAT =   -1\n")) };
        let bad = || -> ! { die(16, &head("     ERROR IN OPENING OUTPUT FILE\n     IOSTAT = 5010\n")) };
        let com = i.comment_char().unwrap_or_else(|_| end());
        print!("  Comment character is '{com}'\n\n");
        let mut items = |n: usize| {
            if i.nextln().is_err() {
                die(1, "\n *** WAVEWATCH III ERROR IN NEXTLN : \n     PREMATURE END OF INPUT FILE\n\n");
            }
            i.list(n).unwrap_or_else(|_| end())
        };
        let v = items(4);
        let tout = [finp::int(&v[0], 0).unwrap_or_else(|_| bad()), finp::int(&v[1], 0).unwrap_or_else(|_| bad())];
        let dtreq = finp::real(&v[2], 0.).unwrap_or_else(|_| bad());
        let nout = finp::int(&v[3], 0).unwrap_or_else(|_| bad());
        let nctype = finp::int(&items(1)[0], 0).unwrap_or_else(|_| bad());
        let prefix = items(1)[0].clone().unwrap_or_else(|| "ww3.".to_string());
        let s3 = finp::int(&items(1)[0], 0).unwrap_or_else(|_| bad());
        Req { tout, dtreq, nout, nctype, prefix, s3 }
    }

    struct Rec {
        time: Time,
        x: Real,
        y: Real,
        tst: String,
        id: String,
        v: [Real; 7],
        spec: Vec<Real>,
    }

    #[derive(Default)]
    struct File {
        ncid: i32,
        varid: [i32; 19],
        it: usize,
        iloc: i32,
        ispec: i32,
        timeid: String,
        name: String,
    }

    /// `W3EXNC`.
    fn w3exnc(f: &mut File, q: &Req, r: &Rec, xfr: Real, flagll: bool, th: (Real, Real), freq: &[Real], dsip: &[Real], stop: &str, s3: &mut i32) {
        let (mk, mth) = (freq.len(), r.spec.len() / freq.len().max(1));
        let refdate = [1990, 1, 1, 0, 0, 0, 0, 0];
        *s3 = (*s3).clamp(4, 10);
        let s5 = *s3 - 8;
        let oldid = f.timeid.clone();
        // 10.**(k) is REAL, NINT makes it whole, and FLOOR of the REAL quotient is what the Fortran rounds through.
        let floor = |v: i32, k: i32| ((v as Real) / (ww3_kernels::nint((10f32).powi(k)) as Real)).floor() as i64;
        let id = if *s3 == 10 {
            format!("{:08}T{}Z", r.time[0], ff::im(s5 as usize, s5 as usize, floor(r.time[1], 6 - s5)))
        } else if *s3 == 8 {
            format!("{:08}", r.time[0])
        } else {
            ff::im(*s3 as usize, *s3 as usize, floor(r.time[0], 8 - *s3))
        };
        f.timeid = id.clone();
        let name = format!("{}{id}_trck.nc", q.prefix.trim_end());
        if !std::path::Path::new(&name).exists() {
            if !oldid.is_empty() && oldid != id {
                ck(nc::redef(f.ncid));
                ck(nc::put_att_text(f.ncid, nc::NC_GLOBAL, "stop_date", stop));
                ck(nc::close(f.ncid));
            }
            f.ncid = 0;
        }
        if f.ncid == 0 {
            f.it = 0;
            f.iloc = 0;
            f.ispec = 0;
            let (ncid, varid) = crnc::w3crnc(q.nctype, &name, [0, 0, mk, mth, 32], flagll, CALTYPE, &mut |e| ck(e));
            (f.ncid, f.varid) = (ncid, varid);
            ck(nc::put_att_text(ncid, nc::NC_GLOBAL, "start_date", &datestr(&t2d(r.time))));
            ck(nc::enddef(ncid));
            let n = mk - 1;
            let mut f1: Vec<Real> = freq.iter().map(|v| v - 0.5 * (v - (v / xfr))).collect();
            let mut f2: Vec<Real> = freq.iter().map(|v| v + 0.5 * (-v + (v * xfr))).collect();
            f1[0] = freq[0];
            f2[n] = freq[n];
            let dthd = 360. / mth as Real;
            let rth0 = th.0 / th.1;
            let dir: Vec<Real> = (0..mth).map(|i| (360 as Real - dthd * (rth0 + i as Real)) % 360.).collect();
            let v = &f.varid;
            ck(nc::put_real(ncid, v[2], &[], None, &[mk], freq));
            ck(nc::put_real(ncid, v[3], &[], None, &[mk], &f1));
            ck(nc::put_real(ncid, v[4], &[], None, &[mk], &f2));
            ck(nc::put_real(ncid, v[5], &[], None, &[mk], dsip));
            ck(nc::put_real(ncid, v[6], &[], None, &[mth], &dir));
            println!("NEW NetCDF file was created {name:<50}");
        }
        f.name = name.clone();
        let cur = t2d(r.time);
        println!(
            "Writing new record trck number {} for {}:{:02}:{:02}T{:02}:{:02}:{:02} in file {name}",
            ff::i(6, f.it as i64),
            ff::i(4, cur[0] as i64),
            cur[1],
            cur[2],
            cur[4],
            cur[5],
            cur[6]
        );
        if r.tst != "SEA" {
            return;
        }
        let [dw, cx, cy, wx, wy, mut ust, asv] = r.v;
        let wao = (wx * wx + wy * wy).sqrt();
        let wdo = if wao > 1.0e-7 { (270. - atan2f(wy, wx) * RADE) % 360. } else { 0. };
        let cao = (cx * cx + cy * cy).sqrt();
        let cdo = if cao > 1.0e-7 { (270. - atan2f(cy, cx) * RADE) % 360. } else { 0. };
        f.it += 1;
        if ust < 0. {
            ust = -1.0;
        }
        let (n, v, it) = (f.ncid, &f.varid, f.it);
        let m2km: Real = if flagll { 1. } else { 1.0e-3 };
        ck(nc::put_double(n, v[1], &[it], tsub(&refdate, &cur)));
        ck(nc::put_real(n, v[7], &[it], None, &[], &[m2km * r.x]));
        ck(nc::put_real(n, v[8], &[it], None, &[], &[m2km * r.y]));
        let mut t = Vec::with_capacity(mk * mth);
        for ik in 0..mk {
            for ith in 0..mth {
                t.push(r.spec[ik + ith * mk]);
            }
        }
        ck(nc::put_real(n, v[9], &[1, 1, it], Some(&[mth, mk, 1]), &[mth, mk, 1], &t));
        for (j, x) in [(10, dw), (11, cao), (12, cdo), (13, wao), (14, wdo), (15, ust), (16, asv)] {
            ck(nc::put_real(n, v[j], &[it], None, &[], &[x]));
        }
        let id = r.id.trim_end();
        ck(nc::put_text(n, v[18], &[1, it], &[id.len(), 1], id));
    }

    pub fn main() {
        print!("\n{:15}   *** WAVEWATCH III Track output postp. ***   \n{:15}===============================================\n\n", "", "");
        let mut q = if std::path::Path::new("ww3_trnc.nml").exists() { from_nml() } else { from_inp() };
        q.dtreq = q.dtreq.max(0.);
        if q.dtreq == 0. {
            q.nout = 1;
        }
        q.nout = q.nout.max(1);
        print!("\n  Output time data : \n --------------------------------------------------\n      First time         : {}\n", stme21(q.tout));
        let mut tdum: Time = [0, 0];
        tick21(&mut tdum, q.dtreq);
        let mut idtime = stme21(tdum).into_bytes();
        let day = if q.dtreq >= 86400. { format!("{} ", ff::i(10, (q.dtreq / 86400.) as i64)) } else { " ".repeat(11) };
        idtime[..11].copy_from_slice(day.as_bytes());
        idtime[20..23].copy_from_slice(b"   ");
        println!("      Interval           : {}\n      Number of requests : {}", String::from_utf8_lossy(&idtime), ff::i(10, q.nout as i64));
        if q.nctype < 3 || q.nctype > 4 {
            die(1, &head(&format!("     ILLEGAL TYPE, NCTYPE ={}\n", ff::i(4, q.nctype as i64))));
        }
        if !std::path::Path::new("track_o.ww3").exists() {
            die(10, &head("     ERROR IN OPENING INPUT FILE\n     IOSTAT =    2\n"));
        }
        let mut r = Records::open("track_o.ww3");
        let (idstr, flagll, mk, mth, xfr);
        {
            let mut c = r.next();
            idstr = c.text(34);
            flagll = c.i() != 0;
            (mk, mth) = (c.i() as usize, c.i() as usize);
            xfr = c.f();
        }
        if idstr != IDTST {
            die(20, &head(&format!("     ILLEGAL TYPE, NCTYPE ={}\n", ff::i(4, 0))));
        }
        print!("      Spectral grid size : {} by {}\n\n  Opening file : \n -----------------------------------------------\n", ff::i(3, mk as i64), ff::i(3, mth as i64));
        let (th1, dth, freq, dsip);
        {
            let mut c = r.next();
            (th1, dth) = (c.f(), c.f());
            freq = c.fs(mk);
            dsip = c.fs(mk);
        }
        print!("\n\n  Generating files \n --------------------------------------------------\n");
        // Every record is read here, as the loop's `BACKSPACE` leaves the first in place.
        let mut recs = Vec::new();
        while let Some(mut c) = r.try_next() {
            let time = [c.i(), c.i()];
            let (x, y) = (c.f(), c.f());
            let tst = c.text(3);
            let id = c.text(32);
            let (mut v, mut spec) = ([0.; 7], vec![0.; mk * mth]);
            if tst == "SEA" {
                let mut c = r.next();
                for a in &mut v {
                    *a = c.f();
                }
                spec = c.fs(mk * mth);
            }
            recs.push(Rec { time, x, y, tst, id, v, spec });
        }
        let mut f = File::default();
        let mut stop = String::new();
        let mut iout = 0;
        let mut s3 = q.s3;
        let mut tout = q.tout;
        for (i, rec) in recs.iter().enumerate() {
            if i == 0 {
                while dsec21(rec.time, tout) < 0. {
                    tick21(&mut tout, q.dtreq);
                }
            } else {
                while dsec21(recs[i - 1].time, tout) < 0. {
                    tick21(&mut tout, q.dtreq);
                }
            }
            if rec.time == tout {
                f.iloc += 1;
                if rec.tst == "SEA" {
                    f.ispec += 1;
                }
            }
            if rec.time[0] > tout[0] || rec.time[1] > tout[1] {
                println!("     {} :{} points and{}  spectra.", stme21(rec.time), ff::i(6, f.iloc as i64), ff::i(6, f.ispec as i64));
                f.iloc = 1;
                f.ispec = if rec.tst == "SEA" { 1 } else { 0 };
                tout = rec.time;
            }
            iout += 1;
            println!("      Files for {}", stme21(tout));
            w3exnc(&mut f, &q, rec, xfr, flagll, (th1, dth), &freq, &dsip, &stop, &mut s3);
            stop = datestr(&t2d(tout));
            if iout >= q.nout {
                break;
            }
        }
        if f.ncid != 0 {
            ck(nc::redef(f.ncid));
            ck(nc::put_att_text(f.ncid, nc::NC_GLOBAL, "stop_date", &stop));
            ck(nc::close(f.ncid));
        }
        print!("\n  End of program \n =========================================\n         WAVEWATCH III Track output \n\n");
        std::io::stdout().flush().unwrap();
    }
}

#[cfg(feature = "netcdf")]
fn main() {
    let dir = std::env::args().nth(1).expect("case directory");
    std::env::set_current_dir(&dir).unwrap_or_else(|e| panic!("{dir}: {e}"));
    run::main()
}

#[cfg(not(feature = "netcdf"))]
fn main() {
    eprintln!("ww3_trnc was built without the `netcdf` feature: build it with scripts/rust_netcdf.sh");
    std::process::exit(64);
}
