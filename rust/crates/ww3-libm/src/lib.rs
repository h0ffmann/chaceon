// SPDX-License-Identifier: MPL-2.0
//! The libm functions WW3's kernels call, as `no_std` Rust with no intrinsic, so the host, PTX and
//! SPIR-V all run the same arithmetic. Each reproduces the reference's glibc 2.36 on aarch64;
//! `rust/tools/libm_bits` proves it over every input.
//!
//! Algorithm and tables: Arm optimized-routines (`math/expf.c`, `math/exp2f_data.c`, MIT), which
//! glibc imports as `sysdeps/ieee754/flt-32/e_expf.c`.
#![no_std]
#![allow(clippy::excessive_precision)]

const INV_LN2_N: f64 = f64::from_bits(0x3ff71547652b82fe) * 32.0;
const C0: f64 = f64::from_bits(0x3fac6af84b912394) / 32.0 / 32.0 / 32.0;
const C1: f64 = f64::from_bits(0x3fcebfce50fac4f3) / 32.0 / 32.0;
const C2: f64 = f64::from_bits(0x3fe62e42ff0c52d6) / 32.0;

const TAB: [u64; 32] = [
    0x3ff0000000000000, 0x3fefd9b0d3158574, 0x3fefb5586cf9890f,
    0x3fef9301d0125b51, 0x3fef72b83c7d517b, 0x3fef54873168b9aa,
    0x3fef387a6e756238, 0x3fef1e9df51fdee1, 0x3fef06fe0a31b715,
    0x3feef1a7373aa9cb, 0x3feedea64c123422, 0x3feece086061892d,
    0x3feebfdad5362a27, 0x3feeb42b569d4f82, 0x3feeab07dd485429,
    0x3feea47eb03a5585, 0x3feea09e667f3bcd, 0x3fee9f75e8ec5f74,
    0x3feea11473eb0187, 0x3feea589994cce13, 0x3feeace5422aa0db,
    0x3feeb737b0cdc5e5, 0x3feec49182a3f090, 0x3feed503b23e255d,
    0x3feee89f995ad3ad, 0x3feeff76f2fb5e47, 0x3fef199bdd85529c,
    0x3fef3720dcef9069, 0x3fef5818dcfba487, 0x3fef7c97337b9b5f,
    0x3fefa4afa2a490da, 0x3fefd0765b6e4540,
];

#[inline]
fn top12(x: f32) -> u32 {
    x.to_bits() >> 20
}

/// glibc's `roundtoint` and `converttoint` on aarch64 (`TOINT_INTRINSICS`): nearest, ties away.
#[inline]
fn round_to_int(z: f64) -> i64 {
    let k = z as i64;
    let frac = z - k as f64;
    if frac >= 0.5 {
        k + 1
    } else if frac <= -0.5 {
        k - 1
    } else {
        k
    }
}

/// glibc 2.36 `expf`.
#[inline]
pub fn expf(x: f32) -> f32 {
    let xd = x as f64;
    let abstop = top12(x) & 0x7ff;
    if abstop >= 0x42b {
        if x.to_bits() == f32::NEG_INFINITY.to_bits() {
            return 0.0;
        }
        if abstop >= 0x7f8 {
            return x + x;
        }
        if x > f32::from_bits(0x42b17217) {
            return f32::INFINITY;
        }
        if x < f32::from_bits(0xc2cff1b4) {
            return 0.0;
        }
    }
    let z = INV_LN2_N * xd;
    let ki = round_to_int(z);
    let kd = ki as f64;
    let r = z - kd;
    let t = TAB[(ki as u64 % 32) as usize].wrapping_add((ki as u64) << 47);
    let s = f64::from_bits(t);
    let z = C0 * r + C1;
    let r2 = r * r;
    let y = C2 * r + 1.0;
    let y = z * r2 + y;
    (y * s) as f32
}
