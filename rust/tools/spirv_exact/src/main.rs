// SPDX-License-Identifier: MPL-2.0
//! rust-gpu emits no `NoContraction`, so a Vulkan driver may fuse `a*b+c` (ADR-0005). This adds the
//! decoration to every `OpFAdd`, `OpFSub`, `OpFMul`, `OpFDiv` and `OpFNegate` result. Usage:
//! spirv_exact <in.spv> <out.spv>.

const OP_DECORATE: u32 = 71;
const NO_CONTRACTION: u32 = 42;
/// `OpFNegate`, `OpFAdd`, `OpFSub`, `OpFMul`, `OpFDiv`.
const FLOAT_OPS: [u32; 5] = [127, 129, 131, 133, 136];
/// `OpTypeVoid` to `OpTypeForwardPointer`, then constants and variables: decorations come before these.
fn after_annotations(op: u32) -> bool {
    (19..=39).contains(&op) || (41..=52).contains(&op) || op == 59
}

fn main() {
    let mut a = std::env::args().skip(1);
    let (src, dst) = (a.next().expect("in"), a.next().expect("out"));
    let b = std::fs::read(&src).unwrap();
    let w: Vec<u32> = b.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
    assert_eq!(w[0], 0x0723_0203, "not SPIR-V");
    let (mut p, mut insert, mut ids) = (5, None, Vec::new());
    while p < w.len() {
        let (n, op) = ((w[p] >> 16) as usize, w[p] & 0xffff);
        if insert.is_none() && after_annotations(op) {
            insert = Some(p);
        }
        if FLOAT_OPS.contains(&op) {
            ids.push(w[p + 2]);
        }
        p += n;
    }
    let at = insert.expect("a module with no types");
    let mut out = w[..at].to_vec();
    for id in &ids {
        out.extend([(3 << 16) | OP_DECORATE, *id, NO_CONTRACTION]);
    }
    out.extend_from_slice(&w[at..]);
    let bytes: Vec<u8> = out.iter().flat_map(|x| x.to_le_bytes()).collect();
    std::fs::write(&dst, bytes).unwrap();
    println!("{} float operations decorated", ids.len());
}
