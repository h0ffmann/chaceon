// SPDX-License-Identifier: MPL-2.0
//! rust-gpu emits no `NoContraction`, so a Vulkan driver may fuse `a*b+c` (ADR-0005). This adds the
//! decoration to every `OpFAdd`, `OpFSub`, `OpFMul`, `OpFDiv` and `OpFNegate` result. Vulkan also allows
//! 2.5 ULP in `OpFDiv`, so an `f32` division becomes an `f64` one, rounded to `f32`: double rounding is
//! innocuous for a quotient, since 53 >= 2*24+2. Usage: spirv_exact <in.spv> <out.spv>, with
//! `SPIRV_EXACT=contract,div` to leave out the steps named, for a measurement.

const OP_TYPE_FLOAT: u32 = 22;
const OP_DECORATE: u32 = 71;
const OP_FCONVERT: u32 = 115;
const OP_FDIV: u32 = 136;
const NO_CONTRACTION: u32 = 42;
/// `OpFNegate`, `OpFAdd`, `OpFSub`, `OpFMul`, `OpFDiv`.
const FLOAT_OPS: [u32; 5] = [127, 129, 131, 133, OP_FDIV];
/// `OpTypeVoid` to `OpTypeForwardPointer`, then constants and variables: decorations come before these.
fn after_annotations(op: u32) -> bool {
    (19..=39).contains(&op) || (41..=52).contains(&op) || op == 59
}

fn main() {
    let mut a = std::env::args().skip(1);
    let (src, dst) = (a.next().expect("in"), a.next().expect("out"));
    let skip = std::env::var("SPIRV_EXACT").unwrap_or_default();
    let (contract, div) = (!skip.contains("contract"), !skip.contains("div"));
    let b = std::fs::read(&src).unwrap();
    let w: Vec<u32> = b.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
    assert_eq!(w[0], 0x0723_0203, "not SPIR-V");
    let (mut p, mut insert, mut f32_ty, mut f64_ty) = (5, None, None, None);
    while p < w.len() {
        let (n, op) = ((w[p] >> 16) as usize, w[p] & 0xffff);
        if insert.is_none() && after_annotations(op) {
            insert = Some(p);
        }
        if op == OP_TYPE_FLOAT {
            match w[p + 2] {
                32 => f32_ty = Some(w[p + 1]),
                64 => f64_ty = Some(w[p + 1]),
                _ => {}
            }
        }
        p += n;
    }
    let at = insert.expect("a module with no types");
    let mut bound = w[3];
    let mut fresh = || {
        bound += 1;
        bound - 1
    };

    let (mut body, mut ids, mut divs) = (Vec::with_capacity(w.len()), Vec::new(), 0);
    let mut p = at;
    while p < w.len() {
        let (n, op) = ((w[p] >> 16) as usize, w[p] & 0xffff);
        if div && op == OP_FDIV && Some(w[p + 1]) == f32_ty {
            let d = f64_ty.expect("an f32 division in a module without f64");
            let (result, x, y) = (w[p + 2], w[p + 3], w[p + 4]);
            let (wx, wy, q) = (fresh(), fresh(), fresh());
            let cvt = (4 << 16) | OP_FCONVERT;
            body.extend([cvt, d, wx, x, cvt, d, wy, y]);
            body.extend([(5 << 16) | OP_FDIV, d, q, wx, wy]);
            body.extend([cvt, w[p + 1], result, q]);
            ids.push(q);
            divs += 1;
        } else {
            if FLOAT_OPS.contains(&op) {
                ids.push(w[p + 2]);
            }
            body.extend_from_slice(&w[p..p + n]);
        }
        p += n;
    }

    let mut out = w[..at].to_vec();
    out[3] = bound;
    if contract {
        for id in &ids {
            out.extend([(3 << 16) | OP_DECORATE, *id, NO_CONTRACTION]);
        }
    }
    out.extend_from_slice(&body);
    let bytes: Vec<u8> = out.iter().flat_map(|x| x.to_le_bytes()).collect();
    std::fs::write(&dst, bytes).unwrap();
    println!("{} float operations decorated, {divs} f32 divisions widened", if contract { ids.len() } else { 0 });
}
