// SPDX-License-Identifier: MPL-2.0
//! `W3GRID`, the grid preprocessor (`model/src/w3gridmd.F90:1151-7366`, `READNL` `:7392-7705`) at
//! metocean/WW3 `92e2cf66`, as `switch_chaceon_shrd` compiles it, for rectilinear and curvilinear grids
//! read from `ww3_grid.nml`. It builds a `ModDef` and `iogr::write` writes it. Grids this port does not
//! implement stop with a message: unstructured and SMC grids, the old `ww3_grid.inp`, formatted or
//! unformatted (`IDFM` 2 and 3) data files, and output boundary lines.
//!
//! Every expression keeps the Fortran's operand order and kind: `REAL` is `f32`, `x**2` is `x*x`,
//! a `PARAMETER` power such as `GRAV**4` is folded by gfortran with one rounding (`fold_pow`), and any
//! other real power stays `powf`.

use crate::gridio::{self, Group, ListFile};
use crate::gridsru;
use crate::insin4::{Insin4Input, insin4, tabu_fw, tabu_stress, tabu_tauhf, tabu_tauhf2};
use crate::iogr;
use crate::moddef::{ModDef, NDTAB, NKD, NKHS};
use std::collections::HashMap;
use ww3_kernels::constants::{DAIR, DERA, DWAT, GRAV, PI, RADIUS, TPI};
use ww3_libm::{logf, powf};

type Real = f32;

/// `ICLOSE_NONE`, `ICLOSE_SMPL`, `ICLOSE_TRPL` (`model/src/w3gsrumd.F90:314-319`).
const ICLOSE_NONE: i32 = -1;
const ICLOSE_SMPL: i32 = 2;
const ICLOSE_TRPL: i32 = 8;

/// `x**n` of a `REAL, PARAMETER`: gfortran folds it with MPFR at 24 bits, one rounding of the exact power.
fn fold_pow(x: Real, n: u32) -> Real {
    let bits = x.to_bits();
    let (biased, frac) = ((bits >> 23) & 0xff, bits & 0x7f_ffff);
    let (m, e) = if biased == 0 { (frac as u128, -149) } else { ((frac | 0x80_0000) as u128, biased as i32 - 150) };
    let mut p: u128 = 1;
    for _ in 0..n {
        p *= m;
    }
    let mut exp = e * n as i32;
    let len = 128 - p.leading_zeros();
    if len > 24 {
        let sh = len - 24;
        let mut q = p >> sh;
        let rem = p & ((1u128 << sh) - 1);
        let half = 1u128 << (sh - 1);
        if rem > half || (rem == half && q & 1 == 1) {
            q += 1;
        }
        p = q;
        exp += sh as i32;
    }
    (p as Real) * (2.0 as Real).powi(exp)
}

#[derive(Clone)]
enum Val {
    R(Real),
    D(f64),
    I(i32),
    L(bool),
    S(String),
    A(Vec<Real>),
}

/// The variables of one namelist, with the Fortran's defaults set before the read.
struct Vars(HashMap<&'static str, Val>);

impl Vars {
    fn new(v: Vec<(&'static str, Val)>) -> Self {
        Vars(v.into_iter().collect())
    }
    fn r(&self, k: &str) -> Real {
        match self.0.get(k) {
            Some(Val::R(v)) => *v,
            _ => panic!("{k} is not a real"),
        }
    }
    fn d(&self, k: &str) -> f64 {
        match self.0.get(k) {
            Some(Val::D(v)) => *v,
            _ => panic!("{k} is not a double"),
        }
    }
    fn i(&self, k: &str) -> i32 {
        match self.0.get(k) {
            Some(Val::I(v)) => *v,
            _ => panic!("{k} is not an integer"),
        }
    }
    fn l(&self, k: &str) -> bool {
        match self.0.get(k) {
            Some(Val::L(v)) => *v,
            _ => panic!("{k} is not a logical"),
        }
    }
    fn s(&self, k: &str) -> String {
        match self.0.get(k) {
            Some(Val::S(v)) => v.clone(),
            _ => panic!("{k} is not a string"),
        }
    }
    /// A `READ (NML=...)`: each `key = value` of the group replaces a variable.
    fn apply(&mut self, g: &Group, prefix: &str) {
        for (key, value) in &g.items {
            let k = key.strip_prefix(prefix).unwrap_or_else(|| panic!("{key} is not a variable of {}", g.name));
            let (name, idx) = match k.split_once('(') {
                Some((n, i)) => (n, Some(i.trim_end_matches(')').parse::<usize>().unwrap_or(0))),
                None => (k, None),
            };
            let (slot, _) = self.0.get_key_value(name).unwrap_or_else(|| panic!("{key} is not a variable of {}", g.name));
            let slot = *slot;
            let new = match (&self.0[slot], idx) {
                (Val::R(_), _) => Val::R(gridio::real(gridio::first(value))),
                (Val::D(_), _) => Val::D(gridio::first(value).replace(['d', 'D'], "e").parse().expect("real")),
                (Val::I(_), _) => Val::I(gridio::int(gridio::first(value))),
                (Val::L(_), _) => Val::L(gridio::logical(gridio::first(value))),
                (Val::S(_), _) => Val::S(gridio::text(value)),
                (Val::A(a), i) => {
                    let mut a = a.clone();
                    let vals: Vec<Real> = value
                        .split(|c: char| c.is_whitespace() || c == ',')
                        .filter(|s| !s.is_empty())
                        .map(gridio::real)
                        .collect();
                    let from = i.map_or(1, |i| i) - 1;
                    for (n, v) in vals.iter().enumerate() {
                        a[from + n] = *v;
                    }
                    Val::A(a)
                }
            };
            self.0.insert(slot, new);
        }
    }
}

fn fmin(a: Real, b: Real) -> Real {
    if a < b { a } else { b }
}
fn fmax(a: Real, b: Real) -> Real {
    if a > b { a } else { b }
}

/// What `ww3_grid` refuses to do, stated once.
fn refuse(what: &str) -> ! {
    eprintln!("ww3_grid: {what} is not implemented in this port");
    std::process::exit(2);
}

pub fn run(dir: &str) {
    let (m, reached) = build(dir);
    let path = format!("{dir}/mod_def.ww3");
    iogr::write(&m, &path);
    let sea = m.nsea;
    println!(
        "ww3_grid: {} {}x{}, {} sea points of {}, NK {}, NTH {}",
        m.gname,
        m.nx,
        m.ny,
        sea,
        m.nx * m.ny,
        m.nk,
        m.nth
    );
    println!("ww3_grid: reached: {}", reached.join("; "));
}

/// `ww3_grid.nml` and what it names, as `W3GRID` turns them into the model definition.
/// The second value lists the branches this run took, for the port report.
pub fn build(dir: &str) -> (ModDef, Vec<String>) {
    let mut reached: Vec<String> = Vec::new();
    let nml_path = format!("{dir}/ww3_grid.nml");
    if !std::path::Path::new(&nml_path).exists() {
        refuse("ww3_grid.inp (only ww3_grid.nml)");
    }
    let nml_text = std::fs::read_to_string(&nml_path).unwrap_or_else(|e| panic!("{nml_path}: {e}"));
    let groups = gridio::groups(&nml_text);
    // W3NMLGRID: the first group of each name, defaults when the file has none.
    let group = |name: &str, defaults: Vec<(&'static str, Val)>, prefix: &str| {
        let mut v = Vars::new(defaults);
        if let Some(g) = groups.iter().find(|g| g.name == name) {
            v.apply(g, prefix);
        }
        v
    };
    let sp = group(
        "SPECTRUM_NML",
        vec![("XFR", Val::R(0.)), ("FREQ1", Val::R(0.)), ("NK", Val::I(0)), ("NTH", Val::I(0)), ("THOFF", Val::R(0.))],
        "SPECTRUM%",
    );
    let run = group(
        "RUN_NML",
        ["FLDRY", "FLCX", "FLCY", "FLCTH", "FLCK", "FLSOU"].iter().map(|k| (*k, Val::L(false))).collect(),
        "RUN%",
    );
    let ts = group(
        "TIMESTEPS_NML",
        ["DTMAX", "DTXY", "DTKTH", "DTMIN"].iter().map(|k| (*k, Val::R(0.))).collect(),
        "TIMESTEPS%",
    );
    let gr = group(
        "GRID_NML",
        vec![
            ("NAME", Val::S("unset".into())),
            ("NML", Val::S("namelists.nml".into())),
            ("TYPE", Val::S("unset".into())),
            ("COORD", Val::S("unset".into())),
            ("CLOS", Val::S("unset".into())),
            ("ZLIM", Val::R(0.)),
            ("DMIN", Val::R(0.)),
        ],
        "GRID%",
    );
    let file = |unit: i32| -> Vec<(&'static str, Val)> {
        vec![
            ("SF", Val::R(1.)),
            ("FILENAME", Val::S("unset".into())),
            ("IDF", Val::I(unit)),
            ("IDLA", Val::I(1)),
            ("IDFM", Val::I(1)),
            ("FORMAT", Val::S("(....)".into())),
            ("FROM", Val::S("NAME".into())),
        ]
    };
    let depth = group("DEPTH_NML", file(50), "DEPTH%");
    let mask = group("MASK_NML", file(60), "MASK%");
    let obst = group("OBST_NML", file(70), "OBST%");

    let gstrg = gr.s("TYPE");
    match gstrg.trim() {
        "RECT" | "CURV" => {}
        "UNST" => refuse("an unstructured grid (GRID%TYPE = 'UNST')"),
        "SMCG" => refuse("an SMC grid (GRID%TYPE = 'SMCG')"),
        other => {
            eprintln!(" *** WAVEWATCH III ERROR IN W3GRID : ILLEGAL GRID TYPE {other:?}");
            std::process::exit(25);
        }
    }
    let gtype = if gstrg.trim() == "RECT" { 1 } else { 2 };

    let mut m = ModDef::default();
    m.idstr = "WAVEWATCH III MODEL DEFINITION FILE".into();
    m.vergrd = "2021-08-06".into();
    // TNAME0..6, TNAMEP, TNAMEG, TNAMEF, TNAMEI of W3IOGR for this switch: LN1 ST4 NL1 BT1 DB1, PR3 UQ.
    m.fnames = [
        "Cavaleri and M.-R. (1982)",
        "Ardhuin et al. (2009+)",
        "Discrete Interaction Approx.",
        "JONSWAP",
        "Battjes and Janssen (1978)",
        "Not defined",
        "Not defined",
        "3rd order UQ scheme",
        "Averaging operator",
        "------------------------------",
        "------------------------------",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    // The spectral grid.
    m.gname = gr.s("NAME").trim_end().chars().take(30).collect();
    let (rxfr, rfr1, nki, nthi) = (sp.r("XFR"), sp.r("FREQ1"), sp.i("NK"), sp.i("NTH"));
    let mut rth0 = sp.r("THOFF");
    let (nk, nth) = (nki, nthi);
    let (nku, nthu) = (nk as usize, nth as usize);
    let nspec = nku * nthu;
    let nfull = nspec + nthu;
    let xfr = fmax(rxfr, 1.00001);
    let fr1 = fmax(rfr1, 1e-6);
    let dth = TPI / nth as Real;
    rth0 = fmax(-0.5, fmin(0.5, rth0));
    let mut th = vec![0.; nthu];
    let (mut esin, mut ecos, mut es2, mut esc, mut ec2) =
        (vec![0.; nfull], vec![0.; nfull], vec![0.; nfull], vec![0.; nfull], vec![0.; nfull]);
    for ith in 0..nthu {
        th[ith] = dth * (rth0 + ith as Real);
        esin[ith] = th[ith].sin();
        ecos[ith] = th[ith].cos();
        if esin[ith].abs() < 1e-5 {
            esin[ith] = 0.;
            ecos[ith] = if ecos[ith] > 0.5 { 1. } else { -1. };
        }
        if ecos[ith].abs() < 1e-5 {
            ecos[ith] = 0.;
            esin[ith] = if esin[ith] > 0.5 { 1. } else { -1. };
        }
        es2[ith] = esin[ith] * esin[ith];
        ec2[ith] = ecos[ith] * ecos[ith];
        esc[ith] = esin[ith] * ecos[ith];
    }
    for ik in 1..nku + 1 {
        for ith in 0..nthu {
            let at = ik * nthu + ith;
            esin[at] = esin[ith];
            ecos[at] = ecos[ith];
            es2[at] = es2[ith];
            ec2[at] = ec2[ith];
            esc[at] = esc[ith];
        }
    }
    let mut sigma = fr1 * TPI / (xfr * xfr);
    let sxfr = 0.5 * (xfr - 1. / xfr);
    let (mut sig, mut dsip) = (vec![0.; nku + 2], vec![0.; nku + 2]);
    for ik in 0..nku + 2 {
        sigma = sigma * xfr;
        sig[ik] = sigma;
        dsip[ik] = sigma * sxfr;
    }
    let mut dsii = vec![0.; nku];
    dsii[0] = 0.5 * sig[1] * (xfr - 1.);
    for ik in 2..nku {
        dsii[ik - 1] = dsip[ik];
    }
    dsii[nku - 1] = 0.5 * sig[nku] * (xfr - 1.) / xfr;
    let dden: Vec<Real> = (1..=nku).map(|ik| dth * dsii[ik - 1] * sig[ik]).collect();
    let sig2: Vec<Real> = (0..nspec).map(|isp| sig[1 + isp / nthu]).collect();
    let dden2: Vec<Real> = (0..nspec).map(|isp| dden[isp / nthu]).collect();

    (m.fldry, m.flcx, m.flcy, m.flcth, m.flck, m.flsou) =
        (run.l("FLDRY"), run.l("FLCX"), run.l("FLCY"), run.l("FLCTH"), run.l("FLCK"), run.l("FLSOU"));
    if !(m.fldry || m.flcx || m.flcy || m.flck || m.flcth || m.flsou) {
        eprintln!(" *** WAVEWATCH III ERROR IN W3GRID : NO PROCESS SELECTED");
        std::process::exit(2);
    }
    m.dtmax = fmax(1., ts.r("DTMAX"));
    m.dtcfl = ts.r("DTXY");
    m.dtcfli = ts.r("DTKTH");
    m.dtmin = fmin(m.dtmax, fmax(0., ts.r("DTMIN")));

    // The namelist file GRID%NML, read by READNL: one scan per name.
    let nmlf = format!("{dir}/{}", gr.s("NML").trim());
    let nmlf_text = std::fs::read_to_string(&nmlf).unwrap_or_else(|e| panic!("{nmlf}: {e}"));
    let readnl = |name: &str, v: &mut Vars| {
        if let Some(g) = gridio::readnl(&nmlf_text, name) {
            v.apply(&g, "");
        }
    };
    let r = |k: &'static str, x: Real| (k, Val::R(x));

    // SLN1
    let mut v = Vars::new(vec![r("CLIN", 80.), r("RFPM", 1.), r("RFHF", 0.5)]);
    readnl("SLN1", &mut v);
    let clin = fmax(0., v.r("CLIN"));
    let rfpm = fmax(0., v.r("RFPM"));
    let rfhf = fmax(0., fmin(1., v.r("RFHF")));
    m.slnc1 = clin * fold_pow(DAIR / DWAT, 2) / fold_pow(GRAV, 2);
    m.fspm = rfpm;
    m.fshf = rfhf;

    // SIN4
    let mut v = Vars::new(vec![
        r("ZWND", 10.),
        r("ALPHA0", 0.0095),
        r("Z0MAX", 0.0),
        r("Z0RAT", 0.04),
        r("BETAMAX", 1.43),
        r("SINTHP", 2.),
        r("SWELLF", 0.66),
        ("SWELLFPAR", Val::I(1)),
        r("SWELLF2", -0.018),
        r("SWELLF3", 0.022),
        r("SWELLF4", 1.5E5),
        r("SWELLF5", 1.2),
        r("SWELLF6", 0.),
        r("SWELLF7", 360000.),
        r("TAUWSHELTER", 0.3),
        r("ZALP", 0.006),
        r("SINBR", 0.),
    ]);
    readnl("SIN4", &mut v);
    m.zzwnd = v.r("ZWND");
    m.aalpha = v.r("ALPHA0");
    m.bbeta = v.r("BETAMAX");
    m.ssinbr = v.r("SINBR");
    m.ssinthp = v.r("SINTHP");
    m.zz0max = v.r("Z0MAX");
    m.zz0rat = v.r("Z0RAT");
    m.zzalp = v.r("ZALP");
    m.ttauwshelter = v.r("TAUWSHELTER");
    m.sswellf = vec![v.r("SWELLF"), v.r("SWELLF2"), v.r("SWELLF3"), v.r("SWELLF4"), v.r("SWELLF5"), v.r("SWELLF6"), v.r("SWELLF7")];
    m.sswellfpar = v.i("SWELLFPAR");

    // SNL1: FLST4 is true in this build, so NLPROP starts at 2.50E7.
    let mut v = Vars::new(vec![
        r("LAMBDA", 0.25),
        r("NLPROP", 2.50E7),
        r("KDCONV", 0.75),
        r("KDMIN", 0.50),
        r("SNLCS1", 5.5),
        r("SNLCS2", 0.833),
        r("SNLCS3", -1.25),
    ]);
    readnl("SNL1", &mut v);
    m.snlc1 = v.r("NLPROP") / fold_pow(GRAV, 4);
    m.lam = v.r("LAMBDA");
    m.kdcon = v.r("KDCONV");
    m.kdmn = v.r("KDMIN");
    m.snls1 = v.r("SNLCS1");
    m.snls2 = v.r("SNLCS2");
    m.snls3 = v.r("SNLCS3");

    // SDS4
    let fachf: Real = 5.;
    let mut v = Vars::new(vec![
        r("WNMEANP", 0.5),
        r("FXFM3", 2.5),
        r("FXFMAGE", 0.),
        r("FXPM3", 4.),
        r("WNMEANPTAIL", -0.5),
        r("SDSBCHOICE", 1.),
        r("SDSC2", -2.2E-5),
        r("SDSCUM", -0.40344),
        r("SDSC4", 1.),
        r("SDSC5", 0.),
        r("SDSNUW", 0.),
        r("SDSC6", 0.3),
        r("SDSBR", 0.90E-3),
        ("SDSBRFDF", Val::I(0)),
        r("SDSBRF1", 0.5),
        r("SDSP", 2.),
        r("SDSDTH", 80.),
        r("SDSCOS", 2.),
        ("SDSISO", Val::I(2)),
        r("SDSBM0", 1.),
        r("SDSBM1", 0.),
        r("SDSBM2", 0.),
        r("SDSBM3", 0.),
        r("SDSBM4", 0.),
        r("SDSBCK", 0.),
        r("SDSABK", 1.5),
        r("SDSPBK", 4.),
        r("SDSBINT", 0.3),
        r("SDSHCK", 1.5),
        r("WHITECAPWIDTH", 0.3),
        r("SDSSTRAIN", 0.),
        r("SDSFACMTF", 400.),
        r("SDSSTRAINA", 15.),
        r("SDSSTRAIN2", 0.),
        r("WHITECAPDUR", 0.56),
        r("SDSBT", 1.100E-3),
        r("SDSL", 3.5000e-05),
        r("SPMSS", 0.5),
        r("SDSNMTF", 1.5),
        r("SDSCUMP", 2.),
        r("SDSMWD", 0.9),
        r("SDSMWPOW", 1.),
        r("SDKOF", 3.),
    ]);
    readnl("SDS4", &mut v);
    // SDSL and SPMSS are not in the namelist, so a user value for them is an error in the Fortran too.
    for k in ["SDSL", "SPMSS"] {
        if let Some(g) = gridio::readnl(&nmlf_text, "SDS4") {
            assert!(!g.items.iter().any(|(a, _)| a == k), "{k} is not an SDS4 variable");
        }
    }
    m.ssdsc = vec![
        v.r("SDSBCHOICE"),
        v.r("SDSC2"),
        v.r("SDSCUM"),
        v.r("SDSC4"),
        v.r("SDSC5"),
        v.r("SDSC6"),
        v.r("WHITECAPWIDTH"),
        v.r("SDSSTRAIN"),
        v.r("SDSL"),
        v.r("SDSSTRAINA") * nth as Real / 360.,
        v.r("SDSSTRAIN2"),
        v.r("SDSBT"),
        v.r("SDSMWD"),
        v.r("SPMSS"),
        v.r("SDSMWPOW"),
        v.r("SDKOF"),
        v.r("WHITECAPDUR"),
        v.r("SDSFACMTF"),
        v.r("SDSNMTF"),
        v.r("SDSCUMP"),
        v.r("SDSNUW"),
    ];
    m.ssdsbr = v.r("SDSBR");
    m.ssdsbrf1 = v.r("SDSBRF1");
    m.ssdsbrfdf = v.i("SDSBRFDF");
    m.ssdsbm = vec![v.r("SDSBM0"), v.r("SDSBM1"), v.r("SDSBM2"), v.r("SDSBM3"), v.r("SDSBM4")];
    m.ssdsbt = v.r("SDSBT");
    m.ssdsiso = v.i("SDSISO");
    m.ssdscos = v.r("SDSCOS");
    m.ssdsp = v.r("SDSP");
    m.ssdsdth = v.r("SDSDTH");
    m.wwnmeanp = v.r("WNMEANP");
    m.ffxfm = v.r("FXFM3") * TPI;
    m.ffxfa = v.r("FXFMAGE") * TPI;
    m.ffxpm = v.r("FXPM3") * GRAV / 28.;
    m.wwnmeanptail = v.r("WNMEANPTAIL");
    m.ssdsbck = v.r("SDSBCK");
    m.ssdsabk = v.r("SDSABK");
    m.ssdspbk = v.r("SDSPBK");
    m.ssdsbint = v.r("SDSBINT");
    m.ssdshck = v.r("SDSHCK");
    let (wnmeanp, wnmeanptail) = (v.r("WNMEANP"), v.r("WNMEANPTAIL"));

    // SBT1, SDB1
    let mut v = Vars::new(vec![r("GAMMA", -0.067)]);
    readnl("SBT1", &mut v);
    m.sbtc1 = 2. * v.r("GAMMA") / GRAV;
    let mut v = Vars::new(vec![r("BJALFA", 1.), r("BJGAM", 0.73), ("BJFLAG", Val::L(true))]);
    readnl("SDB1", &mut v);
    m.sdbc1 = fmax(0., v.r("BJALFA"));
    m.sdbc2 = fmax(0., v.r("BJGAM"));
    m.fdonly = v.l("BJFLAG");

    // UNST: only UGOBCFILE matters for a grid that is not unstructured (the J.EQ.2 test below).
    let mut v = Vars::new(vec![
        ("UGOBCAUTO", Val::L(true)),
        ("UGBCCFL", Val::L(true)),
        r("UGOBCDEPTH", -10.),
        ("UGOBCFILE", Val::S("unset".into())),
        ("EXPFSN", Val::L(true)),
        ("EXPFSPSI", Val::L(false)),
        ("EXPFSFCT", Val::L(false)),
        ("IMPFSN", Val::L(false)),
        ("IMPTOTAL", Val::L(false)),
        ("EXPTOTAL", Val::L(false)),
        ("IMPREFRACTION", Val::L(false)),
        ("IMPFREQSHIFT", Val::L(false)),
        ("IMPSOURCE", Val::L(false)),
        ("SETUP_APPLY_WLV", Val::L(true)),
        ("SOLVERTHR_SETUP", Val::D(1e-6)),
        ("CRIT_DEP_SETUP", Val::D(0.1)),
        ("JGS_TERMINATE_MAXITER", Val::L(true)),
        ("JGS_TERMINATE_DIFFERENCE", Val::L(true)),
        ("JGS_TERMINATE_NORM", Val::L(false)),
        ("JGS_LIMITER", Val::L(false)),
        ("JGS_LIMITER_FUNC", Val::I(1)),
        ("JGS_BLOCK_GAUSS_SEIDEL", Val::L(true)),
        ("JGS_USE_JACOBI", Val::L(true)),
        ("JGS_MAXITER", Val::I(100)),
        ("JGS_PMIN", Val::D(1.)),
        ("JGS_DIFF_THR", Val::D(1e-10)),
        ("JGS_NORM_THR", Val::D(1e-20)),
        ("JGS_NLEVEL", Val::I(0)),
        ("JGS_SOURCE_NONLINEAR", Val::L(false)),
    ]);
    readnl("UNST", &mut v);
    let ugobcfile_len = v.s("UGOBCFILE").trim_end().len();

    // PRO3
    let mut v = Vars::new(vec![r("CFLTM", 0.7), r("WDTHCG", 1.5), r("WDTHTH", 1.5)]);
    readnl("PRO3", &mut v);
    m.wdcg = v.r("WDTHCG");
    m.wdth = v.r("WDTHTH");
    m.ctmax = fmax(0., v.r("CFLTM"));

    // OUTS
    let ii = |k: &'static str, x: i32| (k, Val::I(x));
    let mut stk = vec![0.; 25];
    stk[0] = TPI / 100.;
    let mut v = Vars::new(vec![
        ii("E3D", 0),
        ii("I1E3D", 1),
        ii("I2E3D", nk),
        ii("P2SF", 0),
        ii("I1P2SF", 1),
        ii("I2P2SF", 15),
        ii("US3D", 0),
        ii("I1US3D", 1),
        ii("I2US3D", nk),
        ii("USSP", 0),
        ii("IUSSP", 1),
        ("STK_WN", Val::A(stk)),
        ii("TH1MF", 0),
        ii("I1TH1M", 1),
        ii("I2TH1M", nk),
        ii("STH1MF", 0),
        ii("I1STH1M", 1),
        ii("I2STH1M", nk),
        ii("TH2MF", 0),
        ii("I1TH2M", 1),
        ii("I2TH2M", nk),
        ii("STH2MF", 0),
        ii("I1STH2M", 1),
        ii("I2STH2M", nk),
    ]);
    readnl("OUTS", &mut v);
    let cl = |x: i32| x.max(1).min(nk);
    m.e3df = vec![
        v.i("E3D"), cl(v.i("I1E3D")), cl(v.i("I2E3D")),
        v.i("TH1MF"), cl(v.i("I1TH1M")), cl(v.i("I2TH1M")),
        v.i("STH1MF"), cl(v.i("I1STH1M")), cl(v.i("I2STH1M")),
        v.i("TH2MF"), cl(v.i("I1TH2M")), cl(v.i("I2TH2M")),
        v.i("STH2MF"), cl(v.i("I1STH2M")), cl(v.i("I2STH2M")),
    ];
    m.p2msf = vec![v.i("P2SF"), cl(v.i("I1P2SF")), cl(v.i("I2P2SF"))];
    m.us3df = vec![v.i("US3D"), cl(v.i("I1US3D")), cl(v.i("I2US3D"))];
    m.usspf = vec![v.i("USSP"), v.i("IUSSP").max(1).min(25)];
    if v.i("IUSSP") > 25 {
        eprintln!(" *** WAVEWATCH III ERROR in ww3_grid: Stokes drift partition outputs not intended for use with more than 25 partitions.");
        std::process::exit(31);
    }
    let stk = match &v.0["STK_WN"] {
        Val::A(a) => a.clone(),
        _ => unreachable!(),
    };
    m.ussp_wn = vec![0.; 25];
    for j in 0..m.usspf[1] as usize {
        m.ussp_wn[j] = stk[j];
    }

    // MISC
    let mut v = Vars::new(vec![
        r("CICE0", 0.5),
        r("CICEN", 0.5),
        r("LICE", 0.),
        r("XSEED", 1.),
        ii("FLAGTR", 0),
        r("XP", 0.15),
        r("XR", 0.10),
        r("XFILT", 0.05),
        r("PMOVE", 0.5),
        ii("IHM", 100),
        r("HSPM", 0.05),
        r("WSM", 1.7),
        r("WSC", 0.333),
        ("FLC", Val::L(true)),
        r("FMICHE", 1.6),
        r("RWNDC", 1.),
        r("FACBERG", 1.),
        ii("NOSW", 5),
        ("GSHIFT", Val::D(0.0)),
        r("WCOR1", 99.),
        r("WCOR2", 0.),
        r("STDX", -1.),
        r("STDY", -1.),
        r("STDT", -1.),
        r("ICEHMIN", 0.2),
        r("ICEHINIT", 0.5),
        ("ICEDISP", Val::L(false)),
        r("ICESLN", 1.0),
        r("ICEWIND", 1.0),
        r("ICESNL", 1.0),
        r("ICESDS", 1.0),
        r("ICEHFAC", 1.0),
        r("ICEHDISP", 0.6),
        r("ICEDDISP", 80.),
        r("ICEFDISP", 2.),
        ("CALTYPE", Val::S("standard".into())),
        ("TRCKCMPR", Val::L(true)),
        ii("PTM", 1),
        r("PTFC", 0.1),
        r("BTBET", 1.2),
    ]);
    readnl("MISC", &mut v);
    let mut flagtr = v.i("FLAGTR");
    if !(0..=6).contains(&flagtr) {
        flagtr = 0;
    }
    let cicen = fmin(1., fmax(0., v.r("CICEN")));
    let icesln = fmin(1., fmax(0., v.r("ICESLN")));
    let icewind = fmin(1., fmax(0., v.r("ICEWIND")));
    let icesds = fmin(1., fmax(0., v.r("ICESDS")));
    let icesnl = fmin(1., fmax(0., v.r("ICESNL")));
    m.ficen = cicen;
    m.gridshift = v.d("GSHIFT");
    m.icescales = vec![icesln, icewind, icesnl, icesds];
    m.cmprtrck = v.l("TRCKCMPR");
    let mut cice0 = fmin(cicen, fmax(0., v.r("CICE0")));
    m.ficel = v.r("LICE");
    m.iicehmin = v.r("ICEHMIN");
    m.iicehfac = v.r("ICEHFAC");
    m.iicehinit = v.r("ICEHINIT");
    m.iicedisp = v.l("ICEDISP");
    m.iicehdisp = v.r("ICEHDISP");
    m.iiceddisp = v.r("ICEDDISP");
    m.iicefdisp = v.r("ICEFDISP");
    m.pfmove = fmax(0., v.r("PMOVE"));
    m.btbeta = fmin(fmax(1., v.r("BTBET")), 2.);
    m.aaircmin = logf(GRAV / 2.0 / sig[1]) / logf(xfr) + 1.;
    m.aairgb = 0.2;
    if flagtr < 3 && cice0 != cicen {
        cice0 = cicen;
    }
    if cice0 == cicen && flagtr >= 3 {
        flagtr -= 2;
    }
    m.fice0 = cice0;
    m.stexu = v.r("STDX");
    let stdy = if v.r("STDY") <= 0. { v.r("STDX") } else { v.r("STDY") };
    m.steyu = stdy;
    m.stedu = v.r("STDT");
    m.facsd = v.r("XSEED");
    let xp = fmax(1e-6, v.r("XP"));
    let xr = fmax(1e-6, v.r("XR"));
    m.xrel = xr;
    m.xflt = fmax(0., v.r("XFILT"));
    m.facp = xp / PI * 0.62E-3 * fold_pow(TPI, 4) / fold_pow(GRAV, 2);
    m.ihmax = v.i("IHM").max(50);
    m.hspmin = fmax(0.0001, v.r("HSPM"));
    m.wsmult = fmax(1., v.r("WSM"));
    m.wscut = fmin(1.0001, fmax(0., v.r("WSC")));
    m.flcomb = v.l("FLC");
    m.noswll = v.i("NOSW").max(1);
    m.ptmeth = v.i("PTM");
    m.ptfcut = v.r("PTFC");
    match m.ptmeth {
        1 | 2 => {}
        3 => m.wscut = 0.0,
        4 | 5 => {
            m.wscut = 0.0;
            m.noswll = 1;
        }
        _ => {
            eprintln!("*** Error - unknown partitioning method (PTM)! ***");
            std::process::exit(1);
        }
    }
    m.fhmax = fmax(0.01, v.r("FMICHE"));
    m.caltype = v.s("CALTYPE").trim_end().to_string();
    if !["standard", "360_day", "365_day"].contains(&m.caltype.as_str()) {
        eprintln!(" *** WAVEWATCH III ERROR IN W3GRID : ILLEGAL CALTYPE {}", m.caltype);
        std::process::exit(64);
    }
    let facberg = v.r("FACBERG");

    // The time and frequency tail constants.
    let sign = sig[nku];
    m.fte = 0.25 * sign * dth * sign;
    m.ftf = 0.20 * dth * sign;
    m.ftwn = 0.20 * crate::gridpre::sqrt_real(GRAV) * dth * sign;
    m.fttr = m.ftf;
    m.ftwl = GRAV / 6. / sign * dth * sign;
    m.sstxftf = 1. / (fachf - 1. - wnmeanp * 2.) * powf(sign, 2. + wnmeanp * 2.) * dth;
    m.sstxftftail = 1. / (fachf - 1. - wnmeanptail * 2.) * powf(sign, 2. + wnmeanptail * 2.) * dth;
    m.sstxftwn = 1. / (fachf - 1. - wnmeanp * 2.) * (sign * sign) * powf(sign / crate::gridpre::sqrt_real(GRAV), wnmeanp * 2.) * dth;
    m.fxfm = 2.5 * TPI;
    m.fxpm = 4.0 * GRAV / 28.;
    m.xfc = 3.0;
    m.facti1 = 1. / logf(xfr);
    m.facti2 = 1. - logf(TPI * fr1) * m.facti1;
    m.fachfa = powf(xfr, -fachf - 2.);
    m.fachfe = powf(xfr, -fachf);

    // The grid: type, coordinates, closure.
    m.flagll = gr.s("COORD").trim() != "CART";
    let factor: Real = if m.flagll { 1. } else { 1e-3 };
    let _ = factor;
    let cstrg = gr.s("CLOS");
    m.iclose = match cstrg.trim() {
        "NONE" => ICLOSE_NONE,
        "SMPL" => ICLOSE_SMPL,
        "TRPL" => {
            eprintln!("\n *** WARNING WW3_GRID: TRIPOLE GRID CLOSURE IMPLEMENTATION IS INCOMPLETE ***");
            if gtype == 1 {
                eprintln!(" *** ERROR: TRIPOLE CLOSURE NEEDS A CURVILINEAR GRID");
                std::process::exit(25);
            }
            refuse("tripole closure (CLOS = 'TRPL')")
        }
        "T" | "t" | ".TRU" | ".tru" => ICLOSE_SMPL,
        "F" | "f" | ".FAL" | ".fal" => ICLOSE_NONE,
        other => {
            eprintln!(" *** WAVEWATCH III ERROR IN W3GRID : ILLEGAL GRID CLOSURE {other:?}");
            std::process::exit(25);
        }
    };
    if m.iclose != ICLOSE_NONE && !m.flagll {
        eprintln!(" *** WAVEWATCH III ERROR IN W3GRID : GRID CLOSURE REQUIRES SPHERICAL COORDINATES");
        std::process::exit(25);
    }
    m.gtype = gtype;
    reached.push(format!(
        "{} {} CLOS={}",
        gstrg.trim(),
        if m.flagll { "SPHE" } else { "CART" },
        cstrg.trim()
    ));
    let (nx, ny);
    let rect = group(
        "RECT_NML",
        vec![
            ("NX", Val::I(0)),
            ("NY", Val::I(0)),
            ("SX", Val::R(0.)),
            ("SY", Val::R(0.)),
            ("SF", Val::R(1.)),
            ("X0", Val::R(0.)),
            ("Y0", Val::R(0.)),
            ("SF0", Val::R(1.)),
        ],
        "RECT%",
    );
    let coord = |unit: i32| -> Vec<(&'static str, Val)> {
        vec![
            ("SF", Val::R(1.)),
            ("OFF", Val::R(0.)),
            ("FILENAME", Val::S("unset".into())),
            ("IDF", Val::I(unit)),
            ("IDLA", Val::I(1)),
            ("IDFM", Val::I(1)),
            ("FORMAT", Val::S("(....)".into())),
            ("FROM", Val::S("NAME".into())),
        ]
    };
    // CURV%XCOORD%SF and CURV%YCOORD%SF share one group, so they are read into two variable sets.
    let curv_nx_ny = {
        let mut v = Vars::new(vec![("NX", Val::I(0)), ("NY", Val::I(0))]);
        if let Some(g) = groups.iter().find(|g| g.name == "CURV_NML") {
            for (k, val) in &g.items {
                if let Some(k) = k.strip_prefix("CURV%") {
                    if k == "NX" || k == "NY" {
                        v.apply(&Group { name: g.name.clone(), items: vec![(k.to_string(), val.clone())] }, "");
                    }
                }
            }
        }
        (v.i("NX"), v.i("NY"))
    };
    let mut xin: Vec<Real>;
    let mut yin: Vec<Real>;
    let path = |name: &str| format!("{dir}/{name}");
    if gtype == 1 {
        nx = rect.i("NX").max(3) as usize;
        ny = rect.i("NY").max(3) as usize;
        let vsc = fmax(1e-7, rect.r("SF"));
        let mut sx = rect.r("SX") / vsc;
        let mut sy = rect.r("SY") / vsc;
        sx = fmax(1e-7, sx);
        sy = fmax(1e-7, sy);
        if m.iclose == ICLOSE_SMPL {
            sx = 360. / nx as Real;
        }
        let vsc0 = fmax(1e-7, rect.r("SF0"));
        let x0 = rect.r("X0") / vsc0;
        let y0 = rect.r("Y0") / vsc0;
        (m.sx, m.sy, m.x0, m.y0) = (sx, sy, x0, y0);
        xin = vec![0.; nx * ny];
        yin = vec![0.; nx * ny];
        for iy in 0..ny {
            for ix in 0..nx {
                xin[ix + iy * nx] = x0 + ix as Real * sx;
                yin[ix + iy * nx] = y0 + iy as Real * sy;
            }
        }
    } else {
        nx = curv_nx_ny.0.max(3) as usize;
        ny = curv_nx_ny.1.max(3) as usize;
        let read = |axis: &str, unit: i32| {
            let mut v = Vars::new(coord(unit));
            if let Some(g) = groups.iter().find(|g| g.name == "CURV_NML") {
                let own = format!("CURV%{axis}%");
                let items = g.items.iter().filter(|(k, _)| k.starts_with(&own)).cloned().collect();
                v.apply(&Group { name: g.name.clone(), items }, &own);
            }
            let (idla, idfm) = (v.i("IDLA"), v.i("IDFM"));
            let (idla, idfm) = (if (1..=4).contains(&idla) { idla } else { 1 }, if (1..=3).contains(&idfm) { idfm } else { 1 });
            if v.s("FROM").trim() != "NAME" {
                refuse("a coordinate file read from the input unit (FROM other than NAME)");
            }
            let mut f = ListFile::open(&path(v.s("FILENAME").trim()));
            gridio::ina2r(&mut f, nx, ny, idfm, idla, v.r("SF"), v.r("OFF"))
        };
        xin = read("XCOORD", 21);
        yin = read("YCOORD", 22);
        // (XGRDIN(2,1)-XGRDIN(1,1))*(YGRDIN(1,2)-YGRDIN(1,1)) < (YGRDIN(2,1)-YGRDIN(1,1))*(XGRDIN(1,2)-XGRDIN(1,1))
        let (x11, x21, x12) = (xin[0], xin[1], xin[nx]);
        let (y11, y21, y12) = (yin[0], yin[1], yin[nx]);
        if (x21 - x11) * (y12 - y11) < (y21 - y11) * (x12 - x11) {
            eprintln!(" *** WAVEWATCH III ERROR IN W3GRID : THE COORDINATES ARE NOT RIGHT-HANDED");
            std::process::exit(25);
        }
    }
    (m.nx, m.ny) = (nx as i32, ny as i32);
    let at_in = |ix: usize, iy: usize| ix + iy * nx;
    // (NY,NX) arrays: iy fast.
    let at = |iy: i32, ix: i32| ((iy - 1) + (ix - 1) * ny as i32) as usize;

    // Depth.
    let zlim = gr.r("ZLIM");
    m.dmin = fmax(1e-3, gr.r("DMIN"));
    let mut vsc = depth.r("SF");
    if vsc.abs() < 1e-7 {
        vsc = 1.;
    }
    let (idla, idfm) = (depth.i("IDLA"), depth.i("IDFM"));
    let (idla, idfm) = (if (1..=4).contains(&idla) { idla } else { 1 }, if (1..=3).contains(&idfm) { idfm } else { 1 });
    if depth.s("FROM").trim() != "NAME" {
        refuse("a depth file read from the input unit (FROM other than NAME)");
    }
    let ndsg = depth.i("IDF");
    let mut f = ListFile::open(&path(depth.s("FILENAME").trim()));
    let zbin = gridio::ina2r(&mut f, nx, ny, idfm, idla, vsc, 0.0);

    let mut tmpsta = vec![0i32; nx * ny];
    for iy in 1..=ny as i32 {
        for ix in 1..=nx as i32 {
            if zbin[at_in(ix as usize - 1, iy as usize - 1)] <= zlim {
                tmpsta[at(iy, ix)] = 1;
            }
        }
    }
    m.trflag = flagtr;
    reached.push(format!("depth IDLA {idla}, TRFLAG {flagtr}"));
    let (mut obsx, mut obsy) = (vec![0.0f32; nx * ny], vec![0.0f32; nx * ny]);
    if m.trflag != 0 {
        let mut vsc = obst.r("SF");
        if vsc.abs() < 1e-7 {
            vsc = 1.;
        }
        let (idla, idft) = (obst.i("IDLA"), obst.i("IDFM"));
        let (idla, idft) = (if (1..=4).contains(&idla) { idla } else { 1 }, if (1..=3).contains(&idft) { idft } else { 1 });
        if obst.s("FROM").trim() != "NAME" {
            refuse("an obstruction file read from the input unit (FROM other than NAME)");
        }
        if obst.i("IDF") == ndsg {
            refuse("an obstruction file sharing the depth file's unit");
        }
        let mut f = ListFile::open(&path(obst.s("FILENAME").trim()));
        obsx = gridio::ina2r(&mut f, nx, ny, idft, idla, vsc, 0.0);
        obsy = gridio::ina2r(&mut f, nx, ny, idft, idla, vsc, 0.0);
        for k in 0..nx * ny {
            obsx[k] = fmax(0., fmin(1., obsx[k]));
            obsy[k] = fmax(0., fmin(1., obsy[k]));
        }
    }

    // Status map: boundary points, excluded points and bodies, or a mask file.
    let mut mask_name = gridio::text(&mask.s("FILENAME")).trim().to_string();
    let mut from = mask.s("FROM");
    if mask_name == "unset" || mask_name == "UNSET" {
        from = "PART".into();
    }
    let _ = &mut mask_name;
    let nstat: i32 = -1;
    let n_of = |name: &str, key: &str| -> usize {
        let mut v = Vars::new(vec![("N_POINT", Val::I(0)), ("N_BODY", Val::I(0)), ("N_LINE", Val::I(0))]);
        if let Some(g) = groups.iter().find(|g| g.name == name) {
            for (k, val) in &g.items {
                let sub = k.rsplit('%').next().unwrap();
                v.apply(&Group { name: g.name.clone(), items: vec![(sub.to_string(), val.clone())] }, "");
            }
        }
        v.i(key) as usize
    };
    // INBND_POINT(i) = x y connect, EXCL_POINT(i) = x y connect, EXCL_BODY(i) = x y.
    let triples = |prefix: &str, n: usize| -> Vec<(i32, i32, bool)> {
        let mut out = vec![(0, 0, false); n];
        for g in groups.iter().filter(|g| g.name.starts_with(&format!("{}_", prefix.split('(').next().unwrap()))) {
            for (k, val) in &g.items {
                if let Some(rest) = k.strip_prefix(prefix) {
                    let i: usize = rest.trim_start_matches('(').split(')').next().unwrap().parse().unwrap();
                    let toks: Vec<&str> = val.split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()).collect();
                    out[i - 1] = (
                        gridio::int(toks[0]),
                        gridio::int(toks[1]),
                        toks.get(2).is_some_and(|t| gridio::logical(t)),
                    );
                }
            }
        }
        out
    };
    if from.trim() == "PART" {
        let inb = triples("INBND_POINT", n_of("INBND_COUNT_NML", "N_POINT"));
        let excl_n = n_of("EXCL_COUNT_NML", "N_POINT");
        let exclp = triples("EXCL_POINT", excl_n);
        // READ_EXCLUDED_NML reads EXCL_POINT_NML where it means EXCL_BODY_NML (w3nmlgridmd.F90:2243), so the
        // bodies keep their (0,0) defaults and the body loop ends at once.
        let exclb = vec![(0, 0, false); n_of("EXCL_COUNT_NML", "N_BODY")];
        reached.push(format!(
            "mask PART ({} inbound points, {} connected, {} excluded points, {} bodies)",
            inb.len(),
            inb.iter().filter(|p| p.2).count(),
            exclp.len(),
            exclb.len()
        ));
        let (nxi, nyi) = (nx as i32, ny as i32);
        let j_quirk = ugobcfile_len as i32;
        for iloop in 1..=2 {
            let nstat_l = if iloop == 1 { 2 } else { nstat };
            let pts = if iloop == 1 { &inb } else { &exclp };
            let (mut ixo, mut iyo) = (0, 0);
            let mut first = true;
            for &(ix0, iy0, connct) in pts.iter() {
                let (mut ix, mut iy) = (ix0, iy0);
                if ix == 0 && iy == 0 {
                    break;
                }
                if ix < 1 || ix > nxi || iy < 1 || iy > nyi {
                    continue;
                }
                if connct && !first {
                    let mut idx = ix - ixo;
                    let mut idy = iy - iyo;
                    if idx == 0 || idy == 0 || idx.abs() == idy.abs() {
                        let nba = (idx.abs().max(idy.abs()) - 1).max(0);
                        if idx != 0 {
                            idx = idx.signum();
                        }
                        if idy != 0 {
                            idy = idy.signum();
                        }
                        ix = ixo;
                        iy = iyo;
                        for _ in 1..=nba {
                            ix += idx;
                            iy += idy;
                            if tmpsta[at(iy, ix)] == 1 || j_quirk == 2 {
                                tmpsta[at(iy, ix)] = nstat_l;
                            }
                        }
                        ix += idx;
                        iy += idy;
                    }
                }
                if tmpsta[at(iy, ix)] == 1 || j_quirk == 2 {
                    tmpsta[at(iy, ix)] = nstat_l;
                }
                ixo = ix;
                iyo = iy;
                first = false;
            }
            if iloop == 2 {
                for &(ix, iy, _) in exclb.iter() {
                    if ix == 0 && iy == 0 {
                        break;
                    }
                    if ix < 1 || ix > nxi || iy < 1 || iy > nyi {
                        continue;
                    }
                    if tmpsta[at(iy, ix)] == nstat_l {
                        continue;
                    }
                    let tmpmap = tmpsta.clone();
                    let mut j = 1;
                    let (ix1, iy1) = (ix, iy);
                    let jj = tmpsta[at(iy, ix)];
                    tmpsta[at(iy, ix)] = nstat_l;
                    loop {
                        let mut nbt = 0;
                        for ix in (ix1 - j).max(1)..=(ix1 + j).min(nxi) {
                            for iy in (iy1 - j).max(1)..=(iy1 + j).min(nyi) {
                                if tmpsta[at(iy, ix)] == jj {
                                    if ix > 1 && tmpsta[at(iy, ix - 1)] == nstat_l && tmpmap[at(iy, ix - 1)] == jj {
                                        tmpsta[at(iy, ix)] = nstat_l;
                                    }
                                    if ix < nxi && tmpsta[at(iy, ix + 1)] == nstat_l && tmpmap[at(iy, ix + 1)] == jj {
                                        tmpsta[at(iy, ix)] = nstat_l;
                                    }
                                    if iy < nyi && tmpsta[at(iy + 1, ix)] == nstat_l && tmpmap[at(iy + 1, ix)] == jj {
                                        tmpsta[at(iy, ix)] = nstat_l;
                                    }
                                    if iy > 1 && tmpsta[at(iy - 1, ix)] == nstat_l && tmpmap[at(iy - 1, ix)] == jj {
                                        tmpsta[at(iy, ix)] = nstat_l;
                                    }
                                    if tmpsta[at(iy, ix)] == nstat_l {
                                        nbt += 1;
                                    }
                                }
                            }
                        }
                        if nbt != 0 {
                            j += 1;
                        } else {
                            break;
                        }
                    }
                }
                for ix in 1..=nxi {
                    if tmpsta[at(1, ix)] == 1 {
                        tmpsta[at(1, ix)] = nstat_l;
                    }
                    if tmpsta[at(nyi, ix)] == 1 {
                        tmpsta[at(nyi, ix)] = nstat_l;
                    }
                }
                if m.iclose == ICLOSE_NONE {
                    for iy in 2..nyi {
                        if tmpsta[at(iy, 1)] == 1 {
                            tmpsta[at(iy, 1)] = nstat_l;
                        }
                        if tmpsta[at(iy, nxi)] == 1 {
                            tmpsta[at(iy, nxi)] = nstat_l;
                        }
                    }
                }
            }
        }
    } else {
        if from.trim() != "NAME" {
            refuse("a mask read from the input unit (FROM other than NAME or PART)");
        }
        let (idla, idft) = (mask.i("IDLA"), mask.i("IDFM"));
        let (idla, idft) = (if (1..=4).contains(&idla) { idla } else { 1 }, if (1..=3).contains(&idft) { idft } else { 1 });
        reached.push(format!("mask NAME IDLA {idla}"));
        let mut f = ListFile::open(&path(&mask_name));
        let mut readmp = gridio::ina2i(&mut f, nx, ny, idft, idla);
        let rm = |ix: usize, iy: usize| ix - 1 + (iy - 1) * nx;
        if m.iclose == ICLOSE_NONE {
            for iy in 2..ny {
                if readmp[rm(1, iy)] == 1 {
                    readmp[rm(1, iy)] = 3;
                }
                if readmp[rm(nx, iy)] == 1 {
                    readmp[rm(nx, iy)] = 3;
                }
            }
        }
        for ix in 1..=nx {
            if readmp[rm(ix, 1)] == 1 {
                readmp[rm(ix, 1)] = 3;
            }
            if readmp[rm(ix, ny)] == 1 && m.iclose != ICLOSE_TRPL {
                readmp[rm(ix, ny)] = 3;
            }
        }
        for iy in 1..=ny {
            for ix in 1..=nx {
                let t = at(iy as i32, ix as i32);
                if readmp[rm(ix, iy)] == 3 {
                    tmpsta[t] = nstat;
                } else {
                    tmpsta[t] = readmp[rm(ix, iy)];
                    if zbin[at_in(ix - 1, iy - 1)] > zlim {
                        tmpsta[t] = 0;
                    }
                }
            }
        }
    }

    let nsea = tmpsta.iter().filter(|s| **s > 0).count();
    m.nbi = tmpsta.iter().filter(|s| **s == 2).count() as i32;
    m.flbpi = m.nbi > 0;
    m.nsea = nsea as i32;
    m.nth = nth;
    m.nk = nk;
    let ffacberg = facberg;
    m.ffacberg = ffacberg;

    // Grid arrays: the status maps and everything per sea point.
    let (mut mapsta, mut mapst2) = (vec![0i32; nx * ny], vec![0i32; nx * ny]);
    m.mapfs = vec![0; nx * ny];
    m.zb = vec![0.; nsea];
    m.mapsf = vec![0; nsea * 3];
    m.clat0 = [1., 1., 1.];
    m.clats = vec![0.; nsea];
    m.clatis = vec![0.; nsea];
    m.cthg0s = vec![0.; nsea];
    m.xgrd = vec![0.; nx * ny];
    m.ygrd = vec![0.; nx * ny];
    for iy in 1..=ny {
        for ix in 1..=nx {
            m.xgrd[at(iy as i32, ix as i32)] = xin[at_in(ix - 1, iy - 1)];
            m.ygrd[at(iy as i32, ix as i32)] = yin[at_in(ix - 1, iy - 1)];
        }
    }
    if m.trflag != 0 {
        m.trnx = vec![0.; nx * ny];
        m.trny = vec![0.; nx * ny];
    }
    let mut isea = 0usize;
    for iy in 1..=ny as i32 {
        for ix in 1..=nx as i32 {
            let t = at(iy, ix);
            if tmpsta[t] == nstat {
                mapsta[t] = 0;
                mapst2[t] = 1;
                tmpsta[t] = 3;
            } else {
                mapsta[t] = tmpsta[t];
                mapst2[t] = 0;
            }
            if mapsta[t] != 0 {
                m.mapfs[t] = (isea + 1) as i32;
                m.zb[isea] = zbin[at_in(ix as usize - 1, iy as usize - 1)];
                m.mapsf[isea] = ix;
                m.mapsf[isea + nsea] = iy;
                m.mapsf[isea + 2 * nsea] = iy + (ix - 1) * ny as i32;
                if m.flagll {
                    let y = m.ygrd[t];
                    m.clats[isea] = (y * DERA).cos();
                    m.clatis[isea] = 1. / m.clats[isea];
                    m.cthg0s[isea] = -((DERA * y).tan()) / RADIUS;
                } else {
                    m.clats[isea] = 1.;
                    m.clatis[isea] = 1.;
                    m.cthg0s[isea] = 0.;
                }
                isea += 1;
            }
            if m.trflag != 0 {
                m.trnx[t] = 1. - obsx[at_in(ix as usize - 1, iy as usize - 1)];
                m.trny[t] = 1. - obsy[at_in(ix as usize - 1, iy as usize - 1)];
            }
        }
    }
    m.maptmp = (0..nx * ny).map(|t| mapsta[t] + 8 * mapst2[t]).collect();
    m.mapwn = (0..nfull).map(|isp| 1 + (isp / nthu) as i32).collect();
    m.mapth = (0..nfull).map(|isp| 1 + (isp % nthu) as i32).collect();

    // Output boundary points: the OUTBND lines, each point in a cell found by W3GRMP.
    let n_line = n_of("OUTBND_COUNT_NML", "N_LINE");
    let mut lines = vec![[0.0f32; 5]; n_line];
    for g in groups.iter().filter(|g| g.name == "OUTBND_LINE_NML") {
        for (k, val) in &g.items {
            let i: usize = k.trim_start_matches("OUTBND_LINE(").split(')').next().unwrap().parse().unwrap();
            let toks: Vec<&str> = val.split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()).collect();
            for (n, t) in toks.iter().take(5).enumerate() {
                lines[i - 1][n] = gridio::real(t);
            }
        }
    }
    m.nfbpo = 0;
    m.nbo = vec![0];
    m.nbo2 = vec![0];
    let gsu = if n_line > 0 {
        let xg: Vec<f64> = m.xgrd.iter().map(|x| *x as f64).collect();
        let yg: Vec<f64> = m.ygrd.iter().map(|y| *y as f64).collect();
        Some(gridsru::Gsu::create(m.flagll, m.iclose, nx as i32, ny as i32, xg, yg))
    } else {
        None
    };
    let mut nbotot_final = 0usize;
    for iloop in 1..=2 {
        let mut inml = 0usize;
        let mut nbotot = 0usize;
        let mut nfbpo = 0usize;
        let mut nbo = vec![0i32];
        let mut nbo2 = vec![0i32];
        let mut first = true;
        if iloop == 2 {
            let nb = *m.nbo.last().unwrap() as usize;
            m.xbpo = vec![0.; nb];
            m.ybpo = vec![0.; nb];
            m.rdbpo = vec![0.; nb * 4];
            m.ipbpo = vec![0; nb * 4];
            m.isbpo = vec![0; nb * 4];
        }
        let nbot = *m.nbo.last().unwrap() as usize;
        loop {
            let (xo0, yo0, dxo, dyo, mut npo);
            if inml < n_line {
                let l = lines[inml];
                (xo0, yo0, dxo, dyo, npo) = (l[0], l[1], l[2], l[3], l[4] as i32);
                inml += 1;
            } else {
                (xo0, yo0, dxo, dyo, npo) = (0., 0., 0., 0., 0);
            }
            first = first || npo <= 0;
            npo = npo.abs();
            if first {
                first = false;
                if npo == 0 {
                    break;
                }
                nfbpo += 1;
                if nfbpo > 99 {
                    eprintln!(" *** WAVEWATCH III ERROR IN W3GRID : TOO MANY OUTPUT BOUNDARY POINT SPECTRA FILES");
                    std::process::exit(50);
                }
                nbo2.push(nbo2[nfbpo - 1]);
                nbo.push(nbotot as i32);
            }
            for ip in 1..=npo {
                let xo = xo0 + (ip - 1) as Real * dxo;
                let yo = yo0 + (ip - 1) as Real * dyo;
                let (ingrid, mut ixr, mut iyr, mut rd) = gridsru::w3grmp(gsu.as_ref().unwrap(), xo, yo);
                ixr.swap(2, 3);
                iyr.swap(2, 3);
                rd.swap(2, 3);
                if !ingrid {
                    continue;
                }
                let sea = (0..4).filter(|j| mapsta[at(iyr[*j], ixr[*j])] > 0).count();
                nbotot += 1;
                if iloop == 1 {
                    continue;
                }
                let k = nbotot - 1;
                m.xbpo[k] = xo;
                m.ybpo[k] = yo;
                let mut rdtot: Real = 0.;
                for j in 0..4 {
                    m.rdbpo[k + j * nbot] = if sea >= 2 && mapsta[at(iyr[j], ixr[j])] > 0 && rd[j] > 0.05 { rd[j] } else { 0. };
                    rdtot = rdtot + m.rdbpo[k + j * nbot];
                }
                if rdtot < 1e-10 {
                    for j in 0..4 {
                        m.rdbpo[k + j * nbot] = 0.;
                    }
                } else {
                    for j in 0..4 {
                        m.rdbpo[k + j * nbot] = m.rdbpo[k + j * nbot] / rdtot;
                    }
                }
                let iseai: Vec<i32> = (0..4).map(|j| m.mapfs[at(iyr[j], ixr[j])]).collect();
                for j in 0..4 {
                    if iseai[j] == 0 || m.rdbpo[k + j * nbot] == 0. {
                        m.ipbpo[k + j * nbot] = 0;
                    } else {
                        let mut flnew = true;
                        for ist in nbo2[nfbpo - 1] + 1..=nbo2[nfbpo] {
                            if iseai[j] == m.isbpo[ist as usize - 1] {
                                flnew = false;
                                m.ipbpo[k + j * nbot] = ist - nbo2[nfbpo - 1];
                            }
                        }
                        if flnew {
                            nbo2[nfbpo] += 1;
                            m.ipbpo[k + j * nbot] = nbo2[nfbpo] - nbo2[nfbpo - 1];
                            m.isbpo[nbo2[nfbpo] as usize - 1] = iseai[j];
                        }
                    }
                }
            }
            nbo[nfbpo] = nbotot as i32;
        }
        m.nfbpo = nfbpo as i32;
        m.nbo = nbo;
        m.nbo2 = nbo2;
        nbotot_final = nbotot;
    }
    m.flbpo = nbotot_final > 0;
    reached.push(format!("output boundary: {} lines, {} files, {} points", n_line, m.nfbpo, nbotot_final));

    // The spectral grid goes into the model definition.
    m.dth = dth;
    m.th = th;
    m.esin = esin;
    m.ecos = ecos;
    m.es2 = es2;
    m.esc = esc;
    m.ec2 = ec2;
    m.xfr = xfr;
    m.fr1 = fr1;
    m.sig = sig;
    m.sig2 = sig2;
    m.dsip = dsip;
    m.dsii = dsii;
    m.dden = dden;
    m.dden2 = dden2;

    // TABU_FW, and W3IOGR's INSIN4(.TRUE.) before the write.
    let (delab, fwtable) = tabu_fw();
    m.delab = delab;
    m.fwtable = fwtable;
    let sigmax = m.sig[nku];
    let (deltauw, delu, taut) = tabu_stress(m.zzwnd, m.aalpha, m.zz0max);
    let (delust, delalp, tauhft) = tabu_tauhf(sigmax, m.aalpha, m.bbeta, m.zzalp, m.zz0max);
    m.deltauw = deltauw;
    m.delu = delu;
    m.taut = taut;
    m.delust = delust;
    m.delalp = delalp;
    if m.ttauwshelter > 0. {
        let (deltail, t1, t2) = tabu_tauhf2(sigmax, m.aalpha, m.bbeta, m.zzalp, delust, delalp);
        m.deltail = deltail;
        m.tauhft = t1;
        m.tauhft2 = t2;
    } else {
        m.deltail = 0.;
        m.tauhft = tauhft;
        m.tauhft2 = vec![0.; 101 * 201 * 51];
    }
    m.sat_n = 2 * (nth / 2 - 1) + 1;
    let o = insin4(&Insin4Input {
        nk,
        nth,
        nkhs: NKHS as i32,
        nkd: NKD as i32,
        ndtab: NDTAB as i32,
        sat_n: m.sat_n,
        dth: m.dth,
        xfr: m.xfr,
        ssdsdth: m.ssdsdth,
        ssdscos: m.ssdscos,
        ssdsbrf1: m.ssdsbrf1,
        ssdsbint: m.ssdsbint,
        ssdspbk: m.ssdspbk,
        ssdsabk: m.ssdsabk,
        ssdshck: m.ssdshck,
        ssdsbck: m.ssdsbck,
        ssdsc3: m.ssdsc[2],
        sig: &m.sig,
        dsip: &m.dsip,
        th: &m.th,
        ecos: &m.ecos,
    });
    reached.push(format!(
        "ST4 shelter {}, SDSBCK>0 {}, SDSCUM!=0 {}, SDSDTH<180 {}",
        m.ttauwshelter > 0.,
        m.ssdsbck > 0.,
        m.ssdsc[2] != 0.,
        m.ssdsdth < 180.
    ));
    m.satindices = o.satindices;
    m.satweights = o.satweights;
    m.iktab = o.iktab;
    m.dcki = o.dcki;
    m.qbi = o.qbi;
    m.cumulw = o.cumulw;
    m.dikcumul = o.dikcumul;
    (m, reached)
}

/// `SQRT(GRAV)`, which gfortran folds: the correctly rounded root of the `REAL` constant.
pub fn sqrt_real(x: Real) -> Real {
    x.sqrt()
}
