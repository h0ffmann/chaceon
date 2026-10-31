// SPDX-License-Identifier: MPL-2.0
//! Runs a case in Rust, in a directory holding `mod_def.ww3`, `ww3_shel.nml` and the point list. It reads
//! only the namelist keys the implemented cases use, and refuses a key it would have to ignore.

use std::collections::VecDeque;
use ww3_host::moddef::ModDef;
use ww3_host::shel::{Run, run};
use ww3_host::wtime::Time;

fn time(s: &str) -> Time {
    let mut w = s.split_whitespace();
    [w.next().unwrap().parse().unwrap(), w.next().unwrap().parse().unwrap()]
}

/// `KEY = value` pairs, comments and blank lines out, quotes kept.
fn pairs(text: &str) -> Vec<(String, String)> {
    let mut v = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('!') || !line.contains('=') {
            continue;
        }
        let (k, val) = line.split_once('=').unwrap();
        v.push((k.trim().to_uppercase(), val.trim().to_string()));
    }
    v
}

fn quoted(s: &str) -> Vec<String> {
    s.split('\'').skip(1).step_by(2).map(str::to_string).collect()
}

fn main() {
    let dir = std::env::args().nth(1).expect("case directory");
    let m = ModDef::read(&format!("{dir}/mod_def.ww3"));
    let p = pairs(&std::fs::read_to_string(format!("{dir}/ww3_shel.nml")).unwrap());
    let get = |k: &str| p.iter().find(|(a, _)| a == k).map(|(_, b)| b.clone());
    let date = |k: &str, dflt: &str| time(&quoted(&get(k).unwrap_or(format!("'{dflt}'")))[0]);
    assert_eq!(get("INPUT%FORCING%WINDS").as_deref().map(|s| s.trim_matches('\'')), Some("H"), "only H winds");
    let span = |k: &str| {
        let q = quoted(&get(k).unwrap());
        (time(&q[0]), q[1].parse::<i32>().unwrap(), time(&q[2]))
    };

    let n: usize = get("HOMOG_COUNT%N_CUR").map_or(0, |s| s.parse().unwrap())
        + get("HOMOG_COUNT%N_WND").map_or(0, |s| s.parse().unwrap());
    let mut winds = VecDeque::new();
    for i in 1..=n {
        let k = |f: &str| format!("HOMOG_INPUT({i})%{f}");
        if get(&k("NAME")).map(|s| s.contains("WND")) != Some(true) {
            continue;
        }
        let num = |f: &str| get(&k(f)).map_or(0., |s| s.parse::<f32>().unwrap());
        winds.push_back((date(&k("DATE"), "19680606 000000"), num("VALUE1"), num("VALUE2"), num("VALUE3")));
    }

    let list = quoted(&get("TYPE%POINT%FILE").unwrap());
    let path = format!("{dir}/{}", list[0]);
    let line = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut w = line.split_whitespace();
    let (xpt, ypt) = (w.next().unwrap().parse().unwrap(), w.next().unwrap().parse().unwrap());
    let fields = get("TYPE%FIELD%LIST").unwrap_or_default();
    let r = Run {
        time0: date("DOMAIN%START", "19680606 000000"),
        timen: date("DOMAIN%STOP", "19680607 000000"),
        field: span("DATE%FIELD"),
        point: span("DATE%POINT"),
        winds,
        xpt,
        ypt,
        pname: quoted(&line).first().cloned().unwrap_or_default(),
        flogrd: (fields.contains("WCC"), fields.contains("WCF")),
    };
    run(&m, r, &dir);
}
