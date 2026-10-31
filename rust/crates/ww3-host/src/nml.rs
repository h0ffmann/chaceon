// SPDX-License-Identifier: MPL-2.0
//! Reads a case from a directory holding `mod_def.ww3`, `ww3_shel.nml` and the files they name. It reads
//! only the namelist keys the implemented cases use, and refuses a forcing it would have to ignore.

use crate::moddef::ModDef;
use crate::shel::{Forcing, Run};
use crate::wtime::Time;
use std::collections::VecDeque;

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

/// `ww3_shel.nml` and what it names.
pub fn read_case(dir: &str) -> (ModDef, Run) {
    let m = ModDef::read(&format!("{dir}/mod_def.ww3"));
    let p = pairs(&std::fs::read_to_string(format!("{dir}/ww3_shel.nml")).unwrap());
    let get = |k: &str| p.iter().find(|(a, _)| a == k).map(|(_, b)| b.clone());
    let date = |k: &str, dflt: &str| time(&quoted(&get(k).unwrap_or(format!("'{dflt}'")))[0]);
    for (k, v) in &p {
        let on = !v.contains("'F'");
        let known = k == "INPUT%FORCING%WINDS" || k == "INPUT%FORCING%CURRENTS";
        assert!(!k.starts_with("INPUT%") || known || !on, "{k} is not implemented");
    }
    let mode = |k: &str| get(k).map_or("F".to_string(), |s| s.trim_matches('\'').to_string());
    let span = |k: &str| match get(k) {
        Some(v) => {
            let q = quoted(&v);
            (time(&q[0]), q[1].parse::<i32>().unwrap(), time(&q[2]))
        }
        None => ([19000101, 0], 0, [29001231, 0]),
    };

    let n: usize = get("HOMOG_COUNT%N_CUR").map_or(0, |s| s.parse().unwrap())
        + get("HOMOG_COUNT%N_WND").map_or(0, |s| s.parse().unwrap());
    let (mut winds, mut currents) = (VecDeque::new(), VecDeque::new());
    for i in 1..=n {
        let k = |f: &str| format!("HOMOG_INPUT({i})%{f}");
        let name = get(&k("NAME")).unwrap_or_default();
        let num = |f: &str| get(&k(f)).map_or(0., |s| s.parse::<f32>().unwrap());
        let e = (date(&k("DATE"), "19680606 000000"), num("VALUE1"), num("VALUE2"), num("VALUE3"));
        if name.contains("WND") {
            winds.push_back(e);
        } else if name.contains("CUR") {
            currents.push_back(e);
        } else {
            panic!("homogeneous input {name} is not implemented");
        }
    }
    let cells = (m.nx * m.ny) as usize;
    let forcing = |key: &str, file: &str, q: VecDeque<_>| match mode(key).as_str() {
        "H" => Some(Forcing::homogeneous(q, cells)),
        "T" => Some(Forcing::file(&format!("{dir}/{file}"), cells)),
        "F" => None,
        other => panic!("{key} = {other} is not implemented"),
    };

    let mut points = Vec::new();
    if let Some(f) = get("TYPE%POINT%FILE") {
        let path = format!("{dir}/{}", quoted(&f)[0]);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        for line in text.lines().filter(|l| !l.trim().is_empty() && !l.trim().starts_with('$')) {
            let name = quoted(line).first().cloned().unwrap_or_default();
            if name == "STOPSTRING" {
                break;
            }
            let mut w = line.split_whitespace();
            let x: f32 = w.next().unwrap().parse().unwrap();
            points.push((x, w.next().unwrap().parse().unwrap(), name));
        }
    }
    let fields = get("TYPE%FIELD%LIST").unwrap_or_default();
    let r = Run {
        time0: date("DOMAIN%START", "19680606 000000"),
        timen: date("DOMAIN%STOP", "19680607 000000"),
        field: span("DATE%FIELD"),
        point: span("DATE%POINT"),
        restart: span("DATE%RESTART"),
        wind: forcing("INPUT%FORCING%WINDS", "wind.ww3", winds),
        current: forcing("INPUT%FORCING%CURRENTS", "current.ww3", currents),
        points,
        flogrd: (fields.contains("WCC"), fields.contains("WCF")),
        fields: fields.trim_matches(|c| c == '\'' || c == ' ').to_string(),
    };
    (m, r)
}
