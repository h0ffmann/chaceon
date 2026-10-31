// SPDX-License-Identifier: MPL-2.0
//! Generated from glibc 2.36 `sysdeps/ieee754/flt-32/*_data.c`, which import Arm optimized-routines
//! (MIT). `f64` values are stored as bits, so no decimal rounding happens on the way.

pub const LOGF_TAB: [u64; 32] = [
    0x3ff661ec79f8f3be, 0xbfd57bf7808caade, 0x3ff571ed4aaf883d,
    0xbfd2bef0a7c06ddb, 0x3ff49539f0f010b0, 0xbfd01eae7f513a67,
    0x3ff3c995b0b80385, 0xbfcb31d8a68224e9, 0x3ff30d190c8864a5,
    0xbfc6574f0ac07758, 0x3ff25e227b0b8ea0, 0xbfc1aa2bc79c8100,
    0x3ff1bb4a4a1a343f, 0xbfba4e76ce8c0e5e, 0x3ff12358f08ae5ba,
    0xbfb1973c5a611ccc, 0x3ff0953f419900a7, 0xbfa252f438e10c1e,
    0x3ff0000000000000, 0x0000000000000000, 0x3fee608cfd9a47ac,
    0x3faaa5aa5df25984, 0x3feca4b31f026aa0, 0x3fbc5e53aa362eb4,
    0x3feb2036576afce6, 0x3fc526e57720db08, 0x3fe9c2d163a1aa2d,
    0x3fcbc2860d224770, 0x3fe886e6037841ed, 0x3fd1058bc8a07ee1,
    0x3fe767dcf5534862, 0x3fd4043057b6ee09,
];

pub const LOGF_POLY: [u64; 3] = [
    0xbfd00ea348b88334, 0x3fd5575b0be00b6a, 0xbfdffffef20a4123,
];

pub const LOGF_LN2: u64 = 0x3fe62e42fefa39ef;

// logc is `* POWF_SCALE`: 32, because aarch64 has TOINT_INTRINSICS.
pub const POWF_TAB: [u64; 32] = [
    0x3ff661ec79f8f3be, 0xc02efec65b963019, 0x3ff571ed4aaf883d,
    0xc02b0b6832d4fca4, 0x3ff49539f0f010b0, 0xc027418b0a1fb77b,
    0x3ff3c995b0b80385, 0xc0239de91a6dcf7b, 0x3ff30d190c8864a5,
    0xc0201d9bf3f2b631, 0x3ff25e227b0b8ea0, 0xc0197c1d1b3b7af0,
    0x3ff1bb4a4a1a343f, 0xc012f9e393af3c9f, 0x3ff12358f08ae5ba,
    0xc00960cbbf788d5c, 0x3ff0953f419900a7, 0xbffa6f9db6475fce,
    0x3ff0000000000000, 0x0000000000000000, 0x3fee608cfd9a47ac,
    0x400338ca9f24f53d, 0x3feca4b31f026aa0, 0x401476a9543891ba,
    0x3feb2036576afce6, 0x401e840b4ac4e4d2, 0x3fe9c2d163a1aa2d,
    0x40240645f0c6651c, 0x3fe886e6037841ed, 0x40288e9c2c1b9ff8,
    0x3fe767dcf5534862, 0x402ce0a44eb17bcc,
];

// `A[i] * POWF_SCALE`.
pub const POWF_POLY: [u64; 5] = [
    0x40227616c9496e0b, 0xc0271969a075c67a, 0x402ec70a6ca7badd,
    0xc037154748bef6c8, 0x40471547652ab82b,
];

// __sincosf_table: sign[4], hpi_inv, hpi, c0..c4, s1..s3, twice; hpi_inv unscaled on aarch64.
pub const SINCOSF_0: [u64; 14] = [
    0x3ff0000000000000, 0xbff0000000000000, 0xbff0000000000000,
    0x3ff0000000000000, 0x3fe45f306dc9c883, 0x3ff921fb54442d18,
    0x3ff0000000000000, 0xbfdffffffd0c621c, 0x3fa55553e1068f19,
    0xbf56c087e89a359d, 0x3ef99343027bf8c3, 0xbfc555545995a603,
    0x3f81107605230bc4, 0xbf2994eb3774cf24,
];

pub const SINCOSF_1: [u64; 14] = [
    0x3ff0000000000000, 0xbff0000000000000, 0xbff0000000000000,
    0x3ff0000000000000, 0x3fe45f306dc9c883, 0x3ff921fb54442d18,
    0xbff0000000000000, 0x3fdffffffd0c621c, 0xbfa55553e1068f19,
    0x3f56c087e89a359d, 0xbef99343027bf8c3, 0xbfc555545995a603,
    0x3f81107605230bc4, 0xbf2994eb3774cf24,
];

pub const INV_PIO4: [u32; 24] = [
    0x000000a2, 0x0000a2f9, 0x00a2f983,
    0xa2f9836e, 0xf9836e4e, 0x836e4e44,
    0x6e4e4415, 0x4e441529, 0x441529fc,
    0x1529fc27, 0x29fc2757, 0xfc2757d1,
    0x2757d1f5, 0x57d1f534, 0xd1f534dd,
    0xf534ddc0, 0x34ddc0db, 0xddc0db62,
    0xc0db6295, 0xdb629599, 0x6295993c,
    0x95993c43, 0x993c4390, 0x3c439041,
];

