// SPDX-License-Identifier: MPL-2.0
//! The libm functions WW3's kernels call, as `no_std` Rust with no intrinsic, so the host, PTX and
//! SPIR-V all run the same arithmetic. Each reproduces the reference's glibc 2.36 on aarch64;
//! `rust/tools/libm_bits` proves it over every input.
//!
//! `expf`, `logf`, `powf`, `sincosf`: Arm optimized-routines (MIT), which glibc imports under
//! `sysdeps/ieee754/flt-32/`. `tanhf`, `expm1f`, `atan2f`, `atanf`, `sqrtf`: glibc's fdlibm code.
//! Results are glibc's, so its error flags, `errno` and rounding-mode branches are left out.
#![no_std]
#![allow(clippy::excessive_precision)]

mod tables;
use tables::*;

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

#[inline]
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    (s, (a - (s - bb)) + (b - bb))
}

/// Round to odd: `s + e` to the neighbour of `s` with an odd mantissa, so that a later rounding to
/// fewer bits is the correct rounding of the exact value (Boldo and Melquiond, 2008).
#[inline]
fn to_odd(s: f64, e: f64) -> f64 {
    let bits = s.to_bits();
    if e == 0.0 || bits & 1 != 0 {
        return s;
    }
    f64::from_bits(if (s > 0.0) == (e > 0.0) { bits + 1 } else { bits - 1 })
}

/// Exact `a*b+c` in `f32`. glibc's aarch64 build fuses where GCC could, so the fused sites here are
/// read from the reference's `libm.so.6`; `expf` and `logf` match without any. Neither `core` nor
/// every device offers a fused operation with these bits, so this uses IEEE `+` and `*` only. The
/// `f64` product of two `f32` is exact.
#[inline]
fn fma32(a: f32, b: f32, c: f32) -> f32 {
    let (s, e) = two_sum(a as f64 * b as f64, c as f64);
    to_odd(s, e) as f32
}

/// Exact `a*b+c` in `f64` (Boldo and Melquiond, 2008), for arguments far from overflow and
/// underflow, as all of `sincosf`'s and `powf`'s are.
#[inline]
fn fma64(a: f64, b: f64, c: f64) -> f64 {
    const SPLIT: f64 = 134217729.0;
    let (t, u) = (a * SPLIT, b * SPLIT);
    let (ah, bh) = (t - (t - a), u - (u - b));
    let (al, bl) = (a - ah, b - bh);
    let uh = a * b;
    let ul = ((ah * bh - uh) + ah * bl + al * bh) + al * bl;
    let (th, tl) = two_sum(c, ul);
    let (vh, vl) = two_sum(uh, th);
    let (zh, zl) = two_sum(tl, vl);
    vh + to_odd(zh, zl)
}

/// `__math_xflowf`: `(sign ? -y : y) * y`.
#[inline]
fn xflowf(sign: bool, y: f32) -> f32 {
    (if sign { -y } else { y }) * y
}

const OFLOW: f32 = f32::from_bits(0x60000000); // 0x1p97
const UFLOW: f32 = f32::from_bits(0x10000000); // 0x1p-95
const MAY_UFLOW: f32 = f32::from_bits(0x1a200000); // 0x1.4p-75

#[inline]
fn fb(x: u64) -> f64 {
    f64::from_bits(x)
}

/// glibc 2.36 `logf`.
#[inline]
pub fn logf(x: f32) -> f32 {
    let mut ix = x.to_bits();
    if ix == 0x3f800000 {
        return 0.0;
    }
    if ix.wrapping_sub(0x00800000) >= 0x7f800000 - 0x00800000 {
        if ix.wrapping_mul(2) == 0 {
            return f32::NEG_INFINITY;
        }
        if ix == 0x7f800000 {
            return x;
        }
        if (ix & 0x80000000) != 0 || ix.wrapping_mul(2) >= 0xff000000 {
            return f32::NAN;
        }
        ix = (x * f32::from_bits(0x4b000000)).to_bits();
        ix = ix.wrapping_sub(23 << 23);
    }
    let tmp = ix.wrapping_sub(0x3f330000);
    let i = ((tmp >> (23 - 4)) % 16) as usize;
    let k = (tmp as i32) >> 23;
    let iz = ix.wrapping_sub(tmp & (0x1ff << 23));
    let invc = fb(LOGF_TAB[2 * i]);
    let logc = fb(LOGF_TAB[2 * i + 1]);
    let z = f32::from_bits(iz) as f64;
    let r = z * invc - 1.0;
    let y0 = logc + k as f64 * fb(LOGF_LN2);
    let r2 = r * r;
    let y = fb(LOGF_POLY[1]) * r + fb(LOGF_POLY[2]);
    let y = fb(LOGF_POLY[0]) * r2 + y;
    let y = y * r2 + (y0 + r);
    y as f32
}

#[inline]
fn log2_inline(ix: u32) -> f64 {
    let tmp = ix.wrapping_sub(0x3f330000);
    let i = ((tmp >> (23 - 4)) % 16) as usize;
    let top = tmp & 0xff800000;
    let iz = ix.wrapping_sub(top);
    let k = (top as i32) >> (23 - 5);
    let invc = fb(POWF_TAB[2 * i]);
    let logc = fb(POWF_TAB[2 * i + 1]);
    let z = f32::from_bits(iz) as f64;
    let r = fma64(z, invc, -1.0);
    let y0 = logc + k as f64;
    let r2 = r * r;
    let y = fma64(fb(POWF_POLY[0]), r, fb(POWF_POLY[1]));
    let p = fma64(fb(POWF_POLY[2]), r, fb(POWF_POLY[3]));
    let r4 = r2 * r2;
    let q = fma64(fb(POWF_POLY[4]), r, y0);
    let q = fma64(p, r2, q);
    fma64(y, r4, q)
}

#[inline]
fn exp2_inline(xd: f64, sign_bias: u64) -> f64 {
    let ki = round_to_int(xd);
    let r = xd - ki as f64;
    let t = TAB[(ki as u64 % 32) as usize];
    let ski = (ki as u64).wrapping_add(sign_bias);
    let t = t.wrapping_add(ski << 47);
    let s = f64::from_bits(t);
    let z = fma64(C0, r, C1);
    let r2 = r * r;
    let y = fma64(C2, r, 1.0);
    let y = fma64(z, r2, y);
    y * s
}

#[inline]
fn checkint(iy: u32) -> u32 {
    let e = (iy >> 23) & 0xff;
    if e < 0x7f {
        return 0;
    }
    if e > 0x7f + 23 {
        return 2;
    }
    if iy & ((1 << (0x7f + 23 - e)) - 1) != 0 {
        return 0;
    }
    if iy & (1 << (0x7f + 23 - e)) != 0 {
        return 1;
    }
    2
}

#[inline]
fn zeroinfnan(ix: u32) -> bool {
    ix.wrapping_mul(2).wrapping_sub(1) >= 2u32.wrapping_mul(0x7f800000).wrapping_sub(1)
}

#[inline]
fn issignaling(ix: u32) -> bool {
    (ix & 0x7fffffff) > 0x7f800000 && (ix & 0x00400000) == 0
}

const SIGN_BIAS: u64 = 1 << (5 + 11);

/// glibc 2.36 `powf`.
#[inline]
pub fn powf(x: f32, y: f32) -> f32 {
    let mut sign_bias = 0u64;
    let mut ix = x.to_bits();
    let iy = y.to_bits();
    if ix.wrapping_sub(0x00800000) >= 0x7f800000 - 0x00800000 || zeroinfnan(iy) {
        if zeroinfnan(iy) {
            if iy.wrapping_mul(2) == 0 {
                return if issignaling(ix) { x + y } else { 1.0 };
            }
            if ix == 0x3f800000 {
                return if issignaling(iy) { x + y } else { 1.0 };
            }
            if ix.wrapping_mul(2) > 2u32.wrapping_mul(0x7f800000)
                || iy.wrapping_mul(2) > 2u32.wrapping_mul(0x7f800000)
            {
                return x + y;
            }
            if ix.wrapping_mul(2) == 2u32.wrapping_mul(0x3f800000) {
                return 1.0;
            }
            if (ix.wrapping_mul(2) < 2 * 0x3f800000) == ((iy & 0x80000000) == 0) {
                return 0.0;
            }
            return y * y;
        }
        if zeroinfnan(ix) {
            let mut x2 = x * x;
            if ix & 0x80000000 != 0 && checkint(iy) == 1 {
                x2 = -x2;
            }
            return if iy & 0x80000000 != 0 { 1. / x2 } else { x2 };
        }
        if ix & 0x80000000 != 0 {
            let yint = checkint(iy);
            if yint == 0 {
                return f32::NAN;
            }
            if yint == 1 {
                sign_bias = SIGN_BIAS;
            }
            ix &= 0x7fffffff;
        }
        if ix < 0x00800000 {
            ix = (x * f32::from_bits(0x4b000000)).to_bits();
            ix &= 0x7fffffff;
            ix = ix.wrapping_sub(23 << 23);
        }
    }
    let logx = log2_inline(ix);
    let ylogx = y as f64 * logx;
    if (ylogx.to_bits() >> 47) & 0xffff >= (126.0_f64 * 32.0).to_bits() >> 47 {
        let negative = sign_bias != 0;
        if ylogx > f64::from_bits(0x405fffffffd1d571) * 32.0 {
            return xflowf(negative, OFLOW);
        }
        if ylogx <= -150.0 * 32.0 {
            return xflowf(negative, UFLOW);
        }
        if ylogx < -149.0 * 32.0 {
            return xflowf(negative, MAY_UFLOW);
        }
    }
    exp2_inline(ylogx, sign_bias) as f32
}

#[inline]
fn sc(table: u32, i: usize) -> f64 {
    fb(if table == 0 { SINCOSF_0[i] } else { SINCOSF_1[i] })
}

#[inline]
fn abstop12(x: f32) -> u32 {
    (x.to_bits() >> 20) & 0x7ff
}

/// `sincosf_poly`: `(sin, cos)`.
#[inline]
fn sincosf_poly(x: f64, x2: f64, table: u32, n: i32) -> (f32, f32) {
    let x4 = x2 * x2;
    let x3 = x2 * x;
    let c2 = fma64(x2, sc(table, 10), sc(table, 9));
    let s1 = fma64(x2, sc(table, 13), sc(table, 12));
    let c1 = fma64(x2, sc(table, 7), sc(table, 6));
    let x5 = x3 * x2;
    let x6 = x4 * x2;
    let s = fma64(x3, sc(table, 11), x);
    let c = fma64(x4, sc(table, 8), c1);
    let a = fma64(x5, s1, s) as f32;
    let b = fma64(x6, c2, c) as f32;
    if n & 1 != 0 { (b, a) } else { (a, b) }
}

#[inline]
fn reduce_large(xi: u32) -> (f64, i32) {
    let off = ((xi >> 26) & 15) as usize;
    let shift = (xi >> 23) & 7;
    let xi = ((xi & 0xffffff) | 0x800000) << shift;
    let res0 = xi.wrapping_mul(INV_PIO4[off]) as u64;
    let res1 = xi as u64 * INV_PIO4[off + 4] as u64;
    let res2 = xi as u64 * INV_PIO4[off + 8] as u64;
    let res0 = (res2 >> 32) | (res0 << 32);
    let res0 = res0.wrapping_add(res1);
    let n = res0.wrapping_add(1 << 61) >> 62;
    let res0 = res0.wrapping_sub(n << 62);
    let pi63 = f64::from_bits(0x3c1921fb54442d18);
    (res0 as i64 as f64 * pi63, n as i32)
}

/// glibc 2.36 `sincosf`: `(sin, cos)`. GCC merges `SIN(X)` and `COS(X)` of one argument into it.
#[inline]
pub fn sincosf(y: f32) -> (f32, f32) {
    let x = y as f64;
    let pio4 = f32::from_bits(0x3f490fdb);
    if abstop12(y) < abstop12(pio4) {
        let x2 = x * x;
        if abstop12(y) < abstop12(f32::from_bits(0x39800000)) {
            return (y, 1.0);
        }
        sincosf_poly(x, x2, 0, 0)
    } else if abstop12(y) < abstop12(120.0) {
        let r = x * sc(0, 4);
        let n = round_to_int(r);
        let x = fma64(-(n as f64), sc(0, 5), x);
        let n = n as i32;
        let s = sc(0, (n & 3) as usize);
        let table = if n & 2 != 0 { 1 } else { 0 };
        sincosf_poly(x * s, x * x, table, n)
    } else if abstop12(y) < abstop12(f32::INFINITY) {
        let xi = y.to_bits();
        let sign = (xi >> 31) as i32;
        let (x, n) = reduce_large(xi);
        let s = sc(0, ((n + sign) & 3) as usize);
        let table = if (n + sign) & 2 != 0 { 1 } else { 0 };
        sincosf_poly(x * s, x * x, table, n)
    } else {
        // Inf or NaN gives NaN, as glibc's `y - y`.
        (f32::NAN, f32::NAN)
    }
}

#[inline]
fn set_exp(y: f32, k: i32) -> f32 {
    f32::from_bits((y.to_bits() as i32).wrapping_add(k << 23) as u32)
}

/// glibc 2.36 `expm1f`, fdlibm's.
#[inline]
pub fn expm1f(x0: f32) -> f32 {
    const O_THRESHOLD: f32 = f32::from_bits(0x42b17180);
    const LN2_HI: f32 = f32::from_bits(0x3f317180);
    const LN2_LO: f32 = f32::from_bits(0x3717f7d1);
    const INVLN2: f32 = f32::from_bits(0x3fb8aa3b);
    const Q1: f32 = f32::from_bits(0xbd088889);
    const Q2: f32 = f32::from_bits(0x3ad00d01);
    const Q3: f32 = f32::from_bits(0xb8a670cd);
    const Q4: f32 = f32::from_bits(0x36867e54);
    const Q5: f32 = f32::from_bits(0xb457edbb);
    let tiny = 1.0e-30_f32;
    let mut x = x0;
    let bits = x.to_bits();
    let negative = bits & 0x80000000 != 0;
    let hx = bits & 0x7fffffff;
    if hx >= 0x4195b844 {
        if hx >= 0x42b17218 {
            if hx > 0x7f800000 {
                return x + x;
            }
            if hx == 0x7f800000 {
                return if negative { -1.0 } else { x };
            }
            if x > O_THRESHOLD {
                return f32::INFINITY;
            }
        }
        if negative {
            return tiny - 1.0;
        }
    }
    let (c, k);
    if hx > 0x3eb17218 {
        let (hi, lo);
        if hx < 0x3F851592 {
            if !negative {
                hi = x - LN2_HI;
                lo = LN2_LO;
                k = 1;
            } else {
                hi = x + LN2_HI;
                lo = -LN2_LO;
                k = -1;
            }
        } else {
            k = fma32(INVLN2, x, if negative { -0.5 } else { 0.5 }) as i32;
            let t = k as f32;
            hi = fma32(-t, LN2_HI, x);
            lo = t * LN2_LO;
        }
        x = hi - lo;
        c = (hi - x) - lo;
    } else if hx < 0x33000000 {
        return x;
    } else {
        k = 0;
        c = 0.0;
    }
    let hfx = 0.5_f32 * x;
    let hxs = x * hfx;
    let q = fma32(hxs, Q5, Q4);
    let q = fma32(hxs, q, Q3);
    let q = fma32(hxs, q, Q2);
    let q = fma32(hxs, q, Q1);
    let r1 = fma32(hxs, q, 1.0);
    let t = fma32(-r1, hfx, 3.0);
    let mut e = hxs * ((r1 - t) / fma32(-x, t, 6.0));
    if k == 0 {
        return x - fma32(x, e, -hxs);
    }
    e = fma32(x, e - c, -c);
    e -= hxs;
    if k == -1 {
        return fma32(0.5, x - e, -0.5);
    }
    if k == 1 {
        return if x < -0.25 { -2.0 * (e - (x + 0.5)) } else { 1.0 + 2.0 * (x - e) };
    }
    if k <= -2 || k > 56 {
        return set_exp(1.0 - (e - x), k) - 1.0;
    }
    if k < 23 {
        let t = f32::from_bits(0x3f800000 - (0x1000000u32 >> k));
        set_exp(t - (e - x), k)
    } else {
        let t = f32::from_bits(((0x7f - k) as u32) << 23);
        set_exp((x - (e + t)) + 1.0, k)
    }
}

/// glibc 2.36 `tanhf`.
#[inline]
pub fn tanhf(x: f32) -> f32 {
    let jx = x.to_bits() as i32;
    let ix = jx & 0x7fffffff;
    if ix >= 0x7f800000 {
        return if jx >= 0 { 1.0 / x + 1.0 } else { 1.0 / x - 1.0 };
    }
    let z;
    if ix < 0x41b00000 {
        if ix == 0 {
            return x;
        }
        if ix < 0x24000000 {
            return x * (1.0 + x);
        }
        let ax = f32::from_bits(ix as u32);
        if ix >= 0x3f800000 {
            let t = expm1f(2.0 * ax);
            z = 1.0 - 2.0 / (t + 2.0);
        } else {
            let t = expm1f(-2.0 * ax);
            z = -t / (t + 2.0);
        }
    } else {
        z = 1.0 - 1.0e-30_f32;
    }
    if jx >= 0 { z } else { -z }
}

/// glibc 2.36 `atanf`.
#[inline]
pub fn atanf(x0: f32) -> f32 {
    const ATANHI: [u32; 4] = [0x3eed6338, 0x3f490fda, 0x3f7b985e, 0x3fc90fda];
    const ATANLO: [u32; 4] = [0x31ac3769, 0x33222168, 0x33140fb4, 0x33a22168];
    const AT: [u32; 11] = [
        0x3eaaaaab, 0xbe4ccccd, 0x3e124925, 0xbde38e38, 0x3dba2e6e, 0xbd9d8795, 0x3d886b35, 0xbd6ef16b,
        0x3d4bda59, 0xbd15a221, 0x3c8569d7,
    ];
    let hx = x0.to_bits() as i32;
    let ix = hx & 0x7fffffff;
    let mut x = x0;
    let id: i32;
    if ix >= 0x4c000000 {
        if ix > 0x7f800000 {
            return x + x;
        }
        let v = f32::from_bits(ATANHI[3]) + f32::from_bits(ATANLO[3]);
        return if hx > 0 { v } else { -f32::from_bits(ATANHI[3]) - f32::from_bits(ATANLO[3]) };
    }
    if ix < 0x3ee00000 {
        if ix < 0x31000000 {
            return x;
        }
        id = -1;
    } else {
        x = f32::from_bits(ix as u32);
        if ix < 0x3f980000 {
            if ix < 0x3f300000 {
                id = 0;
                x = (2.0 * x - 1.0) / (2.0 + x);
            } else {
                id = 1;
                x = (x - 1.0) / (x + 1.0);
            }
        } else if ix < 0x401c0000 {
            id = 2;
            x = (x - 1.5) / fma32(1.5, x, 1.0);
        } else {
            id = 3;
            x = -1.0 / x;
        }
    }
    macro_rules! a {
        ($i:expr) => {
            f32::from_bits(AT[$i])
        };
    }
    let z = x * x;
    let w = z * z;
    let even = fma32(w, a!(10), a!(8));
    let odd = fma32(w, a!(9), a!(7));
    let even = fma32(even, w, a!(6));
    let odd = fma32(odd, w, a!(5));
    let even = fma32(even, w, a!(4));
    let odd = fma32(odd, w, a!(3));
    let even = fma32(even, w, a!(2));
    let odd = fma32(odd, w, a!(1));
    let even = fma32(even, w, a!(0));
    // GCC fuses `s1 + s2`, and `x*(s1+s2)` is one product shared by both branches below.
    let xs = x * fma32(even, z, odd * w);
    if id < 0 {
        x - xs
    } else {
        let i = id as usize;
        let z = f32::from_bits(ATANHI[i]) - ((xs - f32::from_bits(ATANLO[i])) - x);
        if hx < 0 { -z } else { z }
    }
}

/// glibc 2.36 `atan2f`.
#[inline]
pub fn atan2f(y: f32, x: f32) -> f32 {
    let tiny = 1.0e-30_f32;
    let pi_o_4 = f32::from_bits(0x3f490fdb);
    let pi_o_2 = f32::from_bits(0x3fc90fdb);
    let pi = f32::from_bits(0x40490fdb);
    let pi_lo = f32::from_bits(0xb3bbbd2e);
    let hx = x.to_bits() as i32;
    let ix = hx & 0x7fffffff;
    let hy = y.to_bits() as i32;
    let iy = hy & 0x7fffffff;
    if ix > 0x7f800000 || iy > 0x7f800000 {
        return x + y;
    }
    if hx == 0x3f800000 {
        return atanf(y);
    }
    let m = ((hy >> 31) & 1) | ((hx >> 30) & 2);
    if iy == 0 {
        match m {
            0 | 1 => return y,
            2 => return pi + tiny,
            _ => return -pi - tiny,
        }
    }
    if ix == 0 {
        return if hy < 0 { -pi_o_2 - tiny } else { pi_o_2 + tiny };
    }
    if ix == 0x7f800000 {
        if iy == 0x7f800000 {
            return match m {
                0 => pi_o_4 + tiny,
                1 => -pi_o_4 - tiny,
                2 => 3.0 * pi_o_4 + tiny,
                _ => -3.0 * pi_o_4 - tiny,
            };
        }
        return match m {
            0 => 0.0,
            1 => -0.0,
            2 => pi + tiny,
            _ => -pi - tiny,
        };
    }
    if iy == 0x7f800000 {
        return if hy < 0 { -pi_o_2 - tiny } else { pi_o_2 + tiny };
    }
    let k = (iy - ix) >> 23;
    let z = if k > 60 {
        pi_o_2 + 0.5 * pi_lo
    } else if hx < 0 && k < -60 {
        0.0
    } else {
        atanf(f32::from_bits((y / x).to_bits() & 0x7fffffff))
    };
    match m {
        0 => z,
        1 => f32::from_bits(z.to_bits() ^ 0x80000000),
        2 => pi - (z - pi_lo),
        _ => (z - pi_lo) - pi,
    }
}

/// glibc 2.36 `log10f`, fdlibm's over `logf`.
#[inline]
pub fn log10f(x0: f32) -> f32 {
    const TWO25: f32 = f32::from_bits(0x4c000000);
    const IVLN10: f32 = f32::from_bits(0x3ede5bd9);
    const LOG10_2HI: f32 = f32::from_bits(0x3e9a2080);
    const LOG10_2LO: f32 = f32::from_bits(0x355427db);
    let mut x = x0;
    let mut hx = x.to_bits() as i32;
    let mut k = 0;
    if hx & 0x7fffffff == 0 {
        return f32::NEG_INFINITY;
    }
    if hx < 0 {
        return f32::NAN;
    }
    if hx < 0x00800000 {
        k -= 25;
        x *= TWO25;
        hx = x.to_bits() as i32;
    }
    if hx >= 0x7f800000 {
        return x + x;
    }
    k += (hx >> 23) - 127;
    let i = ((k as u32) & 0x80000000) >> 31;
    let hx = (hx & 0x007fffff) | ((0x7f - i as i32) << 23);
    let y = (k + i as i32) as f32;
    let x = f32::from_bits(hx as u32);
    let z = fma32(y, LOG10_2LO, IVLN10 * logf(x));
    fma32(y, LOG10_2HI, z)
}

/// glibc's `sqrtf` by digit recurrence, correctly rounded with no hardware square root, so it is
/// the same on every target. A device's IEEE `sqrt` can replace it for speed once Batch 6 shows
/// that the bits agree.
#[inline]
pub fn sqrtf(x: f32) -> f32 {
    let sign = 0x80000000u32 as i32;
    let mut ix = x.to_bits() as i32;
    if (ix & 0x7f800000) == 0x7f800000 {
        return x * x + x;
    }
    if ix <= 0 {
        if (ix & !sign) == 0 {
            return x;
        } else if ix < 0 {
            return f32::NAN;
        }
    }
    let mut m = ix >> 23;
    if m == 0 {
        let mut i = 0;
        while (ix & 0x00800000) == 0 {
            ix <<= 1;
            i += 1;
        }
        m -= i - 1;
    }
    m -= 127;
    ix = (ix & 0x007fffff) | 0x00800000;
    if m & 1 != 0 {
        ix += ix;
    }
    m >>= 1;
    ix += ix;
    let (mut q, mut s) = (0i32, 0i32);
    let mut r = 0x01000000i32;
    while r != 0 {
        let t = s + r;
        if t <= ix {
            s = t + r;
            ix -= t;
            q += r;
        }
        ix += ix;
        r >>= 1;
    }
    if ix != 0 {
        q += q & 1;
    }
    let ix = (q >> 1) + 0x3f000000 + (m << 23);
    f32::from_bits(ix as u32)
}
