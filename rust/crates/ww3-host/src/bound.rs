// SPDX-License-Identifier: MPL-2.0
//! `ww3_bound` (`model/src/ww3_bound.F90:22-751` at `92e2cf66`) and `W3NMLBOUND` (`model/src/w3nmlboundmd.F90`), for
//! `switch_chaceon_shrd`: `nest.ww3` from point spectra in WW3's text format, and the `READ` mode that prints such a
//! file. Log, exit status and file follow the reference. The heap of `ABPIN` is not cleared by the Fortran, so the
//! bins past `NK1*NTH` when the spectra have fewer frequencies than the grid are whatever `malloc` returned; here they
//! are zero ⚠. Unstructured grids stop in `ModDef::read`.

use crate::finp::{self, Inp, ReadErr};
use crate::fortfmt as ff;
use crate::grid::unpack;
use crate::gridio;
use crate::iobc::{IDSTRBC, VERBPTBC, rec, text};
use crate::moddef::{ModDef, Records};
use std::io::Write;
use std::path::Path;
use ww3_kernels::constants::{PI, TPI, TPIINV};
use ww3_kernels::real::Real;

/// `EXTCDE`: `NDSE` is unit 6 here, so the message is in the log.
fn die(code: i32, msg: &str) -> ! {
    print!("{msg}");
    std::io::stdout().flush().unwrap();
    std::process::exit(code);
}

/// What the runtime stops on, where the Fortran has no `ERR=`: the status is libgfortran's.
fn runtime(what: &str) -> ! {
    std::io::stdout().flush().unwrap();
    eprintln!("Fortran runtime error: {what}");
    std::process::exit(2);
}

fn premature() -> ! {
    die(61, "\n *** WAVEWATCH-III ERROR IN W3BOUND : \n     PREMATURE END OF INPUT FILE\n\n")
}

/// `NEXTLN`, whose end of file stops with status 1.
fn nextln(i: &mut Inp) {
    if i.nextln().is_err() {
        die(1, "\n *** WAVEWATCH III ERROR IN NEXTLN : \n     PREMATURE END OF INPUT FILE\n\n");
    }
}

/// A list-directed `REAL` item, with its separator.
fn lreal(x: Real) -> String {
    let a = (x as f64).abs();
    if x == 0. || (0.1..1.0e9).contains(&a) { format!(" {}", ff::g(16, 9, x as f64)) } else { format!(" {}", ff::es(16, 8, x as f64)) }
}

/// A list-directed `INTEGER` item.
fn lint(n: i32) -> String {
    ff::i(12, n as i64)
}

/// `Fw.2`, or blanks for 0, from a field cut out of a record.
fn fixed(field: &str) -> Real {
    let t = field.trim();
    if t.is_empty() {
        return 0.;
    }
    let v = if t.contains('.') || t.contains(['e', 'E', 'd', 'D']) { t.replace(['d', 'D'], "e") } else { format!("{t}e-2") };
    v.parse().unwrap_or_else(|_| runtime(&format!("Bad real number in item 1 of list input: {t:?}")))
}

/// `__powisf2`: `X**N` with a variable `N`.
fn powi(mut x: Real, m: i32) -> Real {
    let mut n = m.unsigned_abs();
    let mut y = if n % 2 == 1 { x } else { 1. };
    n >>= 1;
    while n != 0 {
        x = x * x;
        if n % 2 == 1 {
            y = y * x;
        }
        n >>= 1;
    }
    if m < 0 { 1. / y } else { y }
}

/// The first `n` columns of a record, or fewer, on a character boundary: these files are ASCII.
fn col(l: &str, from: usize, to: usize) -> &str {
    l.get(from..to.min(l.len())).unwrap_or("")
}

struct Spec {
    i: Inp,
    /// `IOSTAT` of a list read: 0, or not 0 at the end or on a bad item.
    list_ok: bool,
}

impl Spec {
    /// `READ (200+IP,*,IOSTAT=IERR)`.
    fn list(&mut self, n: usize) -> Option<Vec<Option<String>>> {
        match self.i.list(n) {
            Ok(v) => Some(v),
            Err(_) => {
                self.list_ok = false;
                None
            }
        }
    }
}

fn reals(v: &[Option<String>], out: &mut [Real]) -> Result<(), ReadErr> {
    for (o, t) in out.iter_mut().zip(v) {
        *o = finp::real(t, *o)?;
    }
    Ok(())
}

struct Mode {
    inxout: String,
    interp: i32,
    verbose: i32,
    specfiles: Vec<String>,
}

/// `W3NMLBOUND`, then the count and the names of the list file.
fn from_nml(dir: &Path) -> Mode {
    let path = dir.join("ww3_bound.nml");
    let t = std::fs::read_to_string(&path).unwrap();
    let (mut inxout, mut interp, mut verbose, mut file) = ("WRITE".to_string(), 2, 1, "spec.list".to_string());
    if let Some(g) = gridio::readnl(&t, "BOUN") {
        for (k, v) in &g.items {
            match k.as_str() {
                "BOUND%MODE" => inxout = gridio::text(v),
                "BOUND%INTERP" => interp = gridio::int(gridio::first(v)),
                "BOUND%VERBOSE" => verbose = gridio::int(gridio::first(v)),
                "BOUND%FILE" => file = gridio::text(v),
                other => panic!("{other} is not in BOUND_NML"),
            }
        }
    }
    let log = format!(
        "\nBOUND %  MODE       = {inxout}\nBOUND %  INTERP     = {}\nBOUND %  VERBOSE    = {}\nBOUND %  FILE       = {file}\n",
        ff::i(8, interp as i64),
        ff::i(8, verbose as i64)
    );
    std::fs::write(dir.join("ww3_bound.nml.log"), log).unwrap();
    let text = match std::fs::read(dir.join(&file)) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => die(69, &format!("\n *** WAVEWATCH III ERROR IN W3BOUND : \n     ERROR IN OPENING SPEC FILE: {file:<128}\n     IOSTAT ={}\n\n", ff::i(5, e.raw_os_error().unwrap_or(2) as i64))),
    };
    let specfiles = text.lines().map(|l| l.trim_end_matches('\r').chars().take(120).collect::<String>().trim_end().to_string()).collect();
    Mode { inxout, interp, verbose, specfiles }
}

/// `ww3_bound.inp`, whose spectra end with `'STOPSTRING'`.
fn from_inp(dir: &Path) -> Mode {
    let path = dir.join("ww3_bound.inp");
    let mut i = match Inp::open(path.to_str().unwrap()) {
        Ok(i) => i,
        Err(e) => {
            print!("\n *** WAVEWATCH III ERROR IN W3BOUNC : \n     ERROR IN OPENING INPUT FILE: ");
            die(63, &format!("\n     IOSTAT ={}\n\n", ff::i(5, e.raw_os_error().unwrap_or(2) as i64)));
        }
    };
    if i.comment_char().is_err() {
        premature();
    }
    nextln(&mut i);
    let inxout = i.list(1).unwrap_or_else(|_| runtime("Bad item in list input"))[0].clone().unwrap_or_default();
    nextln(&mut i);
    let interp = finp::int(&i.list(1).unwrap_or_else(|_| runtime("Bad item in list input"))[0], 0).unwrap_or_else(|_| runtime("Bad item in list input"));
    nextln(&mut i);
    let verbose = finp::int(&i.list(1).unwrap_or_else(|_| runtime("Bad item in list input"))[0], 0).unwrap_or_else(|_| runtime("Bad item in list input"));
    nextln(&mut i);
    let mut specfiles = Vec::new();
    loop {
        nextln(&mut i);
        let l = i.raw_line().unwrap_or_else(|_| premature());
        let name: String = l.chars().take(120).collect::<String>().trim_end().to_string();
        if name == "'STOPSTRING'" {
            break;
        }
        specfiles.push(name);
    }
    Mode { inxout, interp, verbose, specfiles }
}

fn read_mode(dir: &Path, verbose: i32) {
    let p = dir.join("nest.ww3");
    if !p.exists() {
        runtime("Cannot open file 'nest.ww3': No such file or directory");
    }
    let mut r = Records::open(p.to_str().unwrap());
    let (idtst, vertest, nk1, nth1, xfr, fr1i, th1i, nbi);
    {
        let mut c = r.next();
        idtst = c.text(32);
        vertest = c.text(10);
        (nk1, nth1) = (c.i(), c.i());
        (xfr, fr1i, th1i) = (c.f(), c.f(), c.f());
        nbi = c.i();
    }
    let nspec1 = (nk1 * nth1) as usize;
    let nb = nbi as usize;
    if idtst != IDSTRBC {
        print!("\n *** WAVEWATCH-III ERROR IN W3IOBC :\n     ILEGAL IDSTR, READ : {idtst}\n                  CHECK : {IDSTRBC}\n\n");
    }
    println!(" FORMAT VERSION: '{vertest}'");
    println!(" FILE TYPE: '{idtst}'");
    if verbose == 1 {
        println!(
            "NK,NTH,XFR, FR1I, TH1I, NBI :{}{}{}{}{}{}",
            ff::i(5, nk1 as i64),
            ff::i(5, nth1 as i64),
            ff::f(12, 6, xfr as f64),
            ff::f(12, 6, fr1i as f64),
            ff::f(12, 6, th1i as f64),
            ff::i(5, nbi as i64)
        );
    }
    let (xbpi, ybpi, ipbpi, rdbpi);
    {
        let mut c = r.next();
        xbpi = c.fs(nb);
        ybpi = c.fs(nb);
        ipbpi = c.is(4 * nb);
        rdbpi = c.fs(4 * nb);
    }
    if verbose == 1 {
        println!(" XBPI:{}", xbpi.iter().map(|v| lreal(*v)).collect::<String>());
        println!(" YBPI:{}", ybpi.iter().map(|v| lreal(*v)).collect::<String>());
        println!(" IPBPI:");
        for i in 0..nb {
            println!("{} interpolated from:{}", lint(i as i32 + 1), (0..4).map(|j| lint(ipbpi[i + nb * j])).collect::<String>());
            println!("{} with coefficient :{}", lint(i as i32 + 1), (0..4).map(|j| lreal(rdbpi[i + nb * j])).collect::<String>());
        }
    }
    while let Some(mut c) = r.try_next() {
        let (t0, t1, nbi2) = (c.i(), c.i(), c.i());
        if verbose == 1 {
            println!(" TIME2,NBI2:{}{}{}{}", lint(t0), lint(t1), lint(nbi2), lint(0));
        }
        for _ in 0..nbi2 {
            r.next().fs(nspec1);
        }
    }
}

pub fn run(dir: &str) {
    let dir = Path::new(dir);
    let m = ModDef::read(dir.join("mod_def.ww3").to_str().unwrap());
    println!("  Grid name : {:<30}\n", m.gname);
    let mode = if dir.join("ww3_bound.nml").exists() { from_nml(dir) } else { from_inp(dir) };
    let (interp, verbose) = (mode.interp, mode.verbose);
    if mode.inxout == "READ" {
        read_mode(dir, verbose);
    }
    if mode.inxout == "WRITE" && m.flbpi {
        write_mode(dir, &m, &mode.specfiles, interp, verbose);
    }
    std::io::stdout().flush().unwrap();
}

fn write_mode(dir: &Path, m: &ModDef, specfiles: &[String], interp: i32, verbose: i32) {
    let (nk, nth, nsea, ny) = (m.nk as usize, m.nth as usize, m.nsea as usize, m.ny as usize);
    let (mapsta, _) = unpack(&m.maptmp);
    let (mut xbpo, mut ybpo) = (Vec::new(), Vec::new());
    for isea in 0..nsea {
        let (ix, iy) = (m.mapsf[isea], m.mapsf[isea + nsea]);
        let c = (iy - 1) as usize + (ix - 1) as usize * ny;
        if mapsta[c] == 2 {
            if m.gtype == 1 {
                xbpo.push(m.x0 + m.sx * (ix - 1) as Real);
                ybpo.push(m.y0 + m.sy * (iy - 1) as Real);
            } else {
                xbpo.push(m.xgrd[c]);
                ybpo.push(m.ygrd[c]);
            }
        }
    }
    let nbo = xbpo.len();
    let nbo2 = specfiles.len();
    let mut ndsb = std::io::BufWriter::new(std::fs::File::create(dir.join("nest.ww3")).unwrap());
    let (mut lats, mut lons) = (vec![0.; nbo2], vec![0.; nbo2]);
    let (mut nk1, mut nth1, mut nki, mut nthi) = (0usize, 0usize, 0usize, 0usize);
    let (mut freq, mut theta): (Vec<Real>, Vec<Real>) = (Vec::new(), Vec::new());
    let mut abpin: Vec<Real> = Vec::new();
    let mut files = Vec::new();
    for (ip, name) in specfiles.iter().enumerate() {
        let opened = Inp::open(dir.join(name).to_str().unwrap());
        let ierr = opened.as_ref().err().map_or(0, |e| e.raw_os_error().unwrap_or(2));
        if verbose == 1 {
            println!("IP, file, I/O stat:{}, {}, {}", ff::i(5, ip as i64 + 1), name, ff::i(5, ierr as i64));
        }
        let Ok(i) = opened else {
            die(70, &format!("\n *** WAVEWATCH III ERROR IN W3BOUND : \n     SPEC FILE NOT EXISTING: {name:<120}\n\n"));
        };
        let mut f = Spec { i, list_ok: true };
        let head = f.i.raw_line().unwrap_or_else(|_| premature());
        let field = |a: usize, b: usize| col(&head, a, b).trim().parse::<usize>().unwrap_or(0);
        (nki, nthi) = (field(25, 31), field(31, 37));
        if verbose == 1 {
            println!("IP and spectral dimensions:{}{}{}", ff::i(5, ip as i64 + 1), ff::i(5, nki as i64), ff::i(5, nthi as i64));
        }
        if ip == 0 {
            (nk1, nth1) = (nki, nthi);
            freq = vec![0.; nk1];
            theta = vec![0.; nth1];
            abpin = vec![0.; nk * nth1 * nbo2];
        } else if nk1 != nki || nth1 != nthi {
            println!("ERROR, SPECTRAL GRID IN FILE:{name}{}{}{}{}", ff::i(5, nk1 as i64), ff::i(5, nki as i64), ff::i(5, nth1 as i64), ff::i(5, nthi as i64));
            std::io::stdout().flush().unwrap();
            std::process::exit(0);
        }
        let items = f.i.list(nk1).unwrap_or_else(|_| runtime("End of file"));
        reals(&items, &mut freq).unwrap_or_else(|_| runtime("Bad real number in list input"));
        let items = f.i.list(nth1).unwrap_or_else(|_| runtime("End of file"));
        reals(&items, &mut theta).unwrap_or_else(|_| runtime("Bad real number in list input"));
        files.push(f);
    }
    if nki > nk {
        die(68, &format!("\n *** WAVEWATCH III ERROR IN W3BOUND: \n     ILLEGAL NK, NK ={} DIFFERS FROM NKI ={}\n     IT WILL BE MANAGED SOON BY SPCONV\n", ff::i(3, nk as i64), ff::i(3, nki as i64)));
    }
    // From 1 here, as the Fortran's indices enter the arithmetic of the bins.
    let (mut ifmin, mut ifmin2, mut ifmax) = (1usize, 1usize, nk1);
    let fq = |j: usize| freq[j - 1];
    if ((fq(ifmin + 1) / fq(ifmin)) - m.xfr).abs() > 0.005 {
        die(66, &format!("\n *** WAVEWATCH III ERROR IN W3BOUND: \n     ILLEGAL XFR, XFR ={}\n\n", ff::f(12, 6, m.xfr as f64)));
    }
    if nthi != nth {
        die(67, &format!("\n *** WAVEWATCH III ERROR IN W3BOUND: \n     ILLEGAL NTH, NTH ={} DIFFERS FROM NTHI ={}\n\n", ff::i(3, nth as i64), ff::i(3, nthi as i64)));
    }
    let fr1 = m.fr1;
    if (fr1 - fq(1)) / fr1 > 0.03 {
        for j in 1..=nk1.min(nk) {
            if (fq(j) - fr1).abs() < (fq(ifmin) - fr1).abs() {
                ifmin = j;
            }
        }
    }
    if (fq(1) - fr1) / fr1 > 0.03 {
        for j in 1..=nk.min(nk1) {
            if (fq(j) - fr1 * powi(m.xfr, j as i32 - 1)).abs() < (fq(ifmin2) - fr1).abs() {
                ifmin2 = j;
            }
        }
    }
    if (fq(nk1) - fr1 * powi(m.xfr, nk as i32 - 1)) / fq(nk1) > 0.03 {
        for j in 1..=nk {
            // The Fortran reads FREQ past NK1 here, from the heap; zero is far from any target, as that padding was on the
            // one input that reached it ⚠.
            let fj = if j <= nk1 { fq(j) } else { 0. };
            if (fj - fr1 * powi(m.xfr, nk1 as i32 - 1)).abs() < (fq(ifmax) - fr1 * powi(m.xfr, nk1 as i32 - 1)).abs() {
                ifmax = j;
            }
        }
    }
    let mut spec2d = vec![0.; nk1 * nth1];
    let (mut time1, mut time2) = ([0; 2], [0; 2]);
    let mut itime = 0;
    let mut ierr = false;
    while !ierr {
        for ip in 0..nbo2 {
            let f = &mut files[ip];
            f.list_ok = true;
            if let Some(v) = f.list(2) {
                let (a, b) = (finp::int(&v[0], time2[0]), finp::int(&v[1], time2[1]));
                match (a, b) {
                    (Ok(a), Ok(b)) => time2 = [a, b],
                    _ => f.list_ok = false,
                }
            }
            ierr = !f.list_ok;
            if f.list_ok {
                if ip == 0 {
                    time1 = time2;
                } else if time1 != time2 {
                    println!(" AT POINT {}, BAD TIMES:{}{}{}{}", lint(ip as i32 + 1), lint(time1[0]), lint(time1[1]), lint(time2[0]), lint(time2[1]));
                }
                let l = f.i.raw_line().unwrap_or_else(|_| runtime("End of file"));
                lats[ip] = fixed(col(&l, 12, 19));
                lons[ip] = fixed(col(&l, 19, 26));
                f.list_ok = false;
                if let Some(v) = f.list(nk1 * nth1) {
                    f.list_ok = reals(&v, &mut spec2d).is_ok();
                }
                ierr = !f.list_ok;
                let a = &mut abpin[nk * nth1 * ip..nk * nth1 * (ip + 1)];
                if ifmin2 > 1 {
                    a[..(ifmin2 - 1) * nth].fill(0.);
                }
                for i in ifmin..=ifmax {
                    for j in 1..=nth {
                        a[(i - ifmin + (ifmin2 - 1)) * nth + j - 1] = spec2d[i - 1 + (j - 1) * nk1] * TPIINV;
                    }
                }
                if ifmax - ifmin + ifmin2 < nk1 {
                    a[(ifmax - ifmin + ifmin2) * nth..nk1 * nth].fill(0.);
                }
            }
        }
        if !ierr {
            if itime == 0 {
                if (theta[0] - 0.5 * PI).abs() < 0.01 {
                    theta[0] = 0.5 * PI;
                }
                let mut h = Vec::new();
                text(&mut h, IDSTRBC, 32);
                text(&mut h, VERBPTBC, 10);
                h.extend((nk1 as i32).to_be_bytes());
                h.extend((nth as i32).to_be_bytes());
                h.extend(m.xfr.to_be_bytes());
                h.extend(freq[0].to_be_bytes());
                h.extend(((2.5 * PI - theta[0]) % TPI).to_be_bytes());
                h.extend((nbo as i32).to_be_bytes());
                rec(&mut ndsb, &h);
                let mut ipbpo = vec![1i32; nbo * 4];
                let mut rdbpo = vec![0.; nbo * 4];
                rdbpo[..nbo].fill(1.);
                for ip1 in 0..nbo {
                    let (mut dmin, mut dmin2): (Real, Real) = (360. + 180., 360. + 180.);
                    for ip in 0..nbo2 {
                        let (dx, dy) = (lons[ip] - xbpo[ip1], lats[ip] - ybpo[ip1]);
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dmin == 360. + 180. {
                            if dist < dmin {
                                ipbpo[ip1] = ip as i32 + 1;
                                dmin = dist;
                            }
                        } else if dist < dmin2 {
                            if dist < dmin {
                                ipbpo[ip1 + nbo] = ipbpo[ip1];
                                dmin2 = dmin;
                                ipbpo[ip1] = ip as i32 + 1;
                                dmin = dist;
                            } else {
                                ipbpo[ip1 + nbo] = ip as i32 + 1;
                                dmin2 = dist;
                            }
                        }
                    }
                    if interp > 1 && nbo2 > 1 {
                        let (a, b) = ((ipbpo[ip1] - 1) as usize, (ipbpo[ip1 + nbo] - 1) as usize);
                        let (dx, dy) = (lons[a] - lons[b], lats[a] - lats[b]);
                        let dist = (dx * dx + dy * dy).sqrt();
                        let cos1 = ((xbpo[ip1] - lons[a]) * (lons[b] - lons[a]) + (ybpo[ip1] - lats[a]) * (lats[b] - lats[a])) / (dist * dist);
                        rdbpo[ip1] = 1. - (1f32).min((0f32).max(cos1));
                        rdbpo[ip1 + nbo] = (1f32).min((0f32).max(cos1));
                    }
                    if verbose == 1 {
                        println!(" IPBP:{}{}", lint(ip1 as i32 + 1), (0..4).map(|j| lint(ipbpo[ip1 + nbo * j])).collect::<String>());
                        println!(" RDBP:{}{}", lint(ip1 as i32 + 1), (0..4).map(|j| lreal(rdbpo[ip1 + nbo * j])).collect::<String>());
                    }
                }
                let mut p = Vec::new();
                for v in xbpo.iter().chain(&ybpo) {
                    p.extend(v.to_be_bytes());
                }
                for v in &ipbpo {
                    p.extend(v.to_be_bytes());
                }
                for v in &rdbpo {
                    p.extend(v.to_be_bytes());
                }
                rec(&mut ndsb, &p);
            }
            println!(" Writing boundary data for time:{}{}{}", lint(time2[0]), lint(time2[1]), lint(nbo2 as i32));
            let mut t = Vec::new();
            for v in [time2[0], time2[1], nbo2 as i32] {
                t.extend(v.to_be_bytes());
            }
            rec(&mut ndsb, &t);
            for ip in 0..nbo2 {
                let b: Vec<u8> = abpin[nk * nth1 * ip..nk * nth1 * (ip + 1)].iter().flat_map(|v| v.to_be_bytes()).collect();
                rec(&mut ndsb, &b);
            }
            itime += 1;
        }
    }
    ndsb.flush().unwrap();
}
