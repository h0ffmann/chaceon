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


// __exp_data, N = 128: invln2N, negln2hiN, negln2loN, poly C2..C5, then tail and sbits per entry.
pub const EXP_INVLN2N: u64 = 0x40671547652b82fe;
pub const EXP_NEGLN2HI: u64 = 0xbf762e42fefa0000;
pub const EXP_NEGLN2LO: u64 = 0xbd0cf79abc9e3b3a;
pub const EXP_POLY: [u64; 4] = [0x3fdffffffffffdbd, 0x3fc555555555543c, 0x3fa55555cf172b91, 0x3f81111167a4d017];

pub const EXP_TAB: [u64; 256] = [
    0x0000000000000000, 0x3ff0000000000000, 0x3c9b3b4f1a88bf6e,
    0x3feff63da9fb3335, 0xbc7160139cd8dc5d, 0x3fefec9a3e778061,
    0xbc905e7a108766d1, 0x3fefe315e86e7f85, 0x3c8cd2523567f613,
    0x3fefd9b0d3158574, 0xbc8bce8023f98efa, 0x3fefd06b29ddf6de,
    0x3c60f74e61e6c861, 0x3fefc74518759bc8, 0x3c90a3e45b33d399,
    0x3fefbe3ecac6f383, 0x3c979aa65d837b6d, 0x3fefb5586cf9890f,
    0x3c8eb51a92fdeffc, 0x3fefac922b7247f7, 0x3c3ebe3d702f9cd1,
    0x3fefa3ec32d3d1a2, 0xbc6a033489906e0b, 0x3fef9b66affed31b,
    0xbc9556522a2fbd0e, 0x3fef9301d0125b51, 0xbc5080ef8c4eea55,
    0x3fef8abdc06c31cc, 0xbc91c923b9d5f416, 0x3fef829aaea92de0,
    0x3c80d3e3e95c55af, 0x3fef7a98c8a58e51, 0xbc801b15eaa59348,
    0x3fef72b83c7d517b, 0xbc8f1ff055de323d, 0x3fef6af9388c8dea,
    0x3c8b898c3f1353bf, 0x3fef635beb6fcb75, 0xbc96d99c7611eb26,
    0x3fef5be084045cd4, 0x3c9aecf73e3a2f60, 0x3fef54873168b9aa,
    0xbc8fe782cb86389d, 0x3fef4d5022fcd91d, 0x3c8a6f4144a6c38d,
    0x3fef463b88628cd6, 0x3c807a05b0e4047d, 0x3fef3f49917ddc96,
    0x3c968efde3a8a894, 0x3fef387a6e756238, 0x3c875e18f274487d,
    0x3fef31ce4fb2a63f, 0x3c80472b981fe7f2, 0x3fef2b4565e27cdd,
    0xbc96b87b3f71085e, 0x3fef24dfe1f56381, 0x3c82f7e16d09ab31,
    0x3fef1e9df51fdee1, 0xbc3d219b1a6fbffa, 0x3fef187fd0dad990,
    0x3c8b3782720c0ab4, 0x3fef1285a6e4030b, 0x3c6e149289cecb8f,
    0x3fef0cafa93e2f56, 0x3c834d754db0abb6, 0x3fef06fe0a31b715,
    0x3c864201e2ac744c, 0x3fef0170fc4cd831, 0x3c8fdd395dd3f84a,
    0x3feefc08b26416ff, 0xbc86a3803b8e5b04, 0x3feef6c55f929ff1,
    0xbc924aedcc4b5068, 0x3feef1a7373aa9cb, 0xbc9907f81b512d8e,
    0x3feeecae6d05d866, 0xbc71d1e83e9436d2, 0x3feee7db34e59ff7,
    0xbc991919b3ce1b15, 0x3feee32dc313a8e5, 0x3c859f48a72a4c6d,
    0x3feedea64c123422, 0xbc9312607a28698a, 0x3feeda4504ac801c,
    0xbc58a78f4817895b, 0x3feed60a21f72e2a, 0xbc7c2c9b67499a1b,
    0x3feed1f5d950a897, 0x3c4363ed60c2ac11, 0x3feece086061892d,
    0x3c9666093b0664ef, 0x3feeca41ed1d0057, 0x3c6ecce1daa10379,
    0x3feec6a2b5c13cd0, 0x3c93ff8e3f0f1230, 0x3feec32af0d7d3de,
    0x3c7690cebb7aafb0, 0x3feebfdad5362a27, 0x3c931dbdeb54e077,
    0x3feebcb299fddd0d, 0xbc8f94340071a38e, 0x3feeb9b2769d2ca7,
    0xbc87deccdc93a349, 0x3feeb6daa2cf6642, 0xbc78dec6bd0f385f,
    0x3feeb42b569d4f82, 0xbc861246ec7b5cf6, 0x3feeb1a4ca5d920f,
    0x3c93350518fdd78e, 0x3feeaf4736b527da, 0x3c7b98b72f8a9b05,
    0x3feead12d497c7fd, 0x3c9063e1e21c5409, 0x3feeab07dd485429,
    0x3c34c7855019c6ea, 0x3feea9268a5946b7, 0x3c9432e62b64c035,
    0x3feea76f15ad2148, 0xbc8ce44a6199769f, 0x3feea5e1b976dc09,
    0xbc8c33c53bef4da8, 0x3feea47eb03a5585, 0xbc845378892be9ae,
    0x3feea34634ccc320, 0xbc93cedd78565858, 0x3feea23882552225,
    0x3c5710aa807e1964, 0x3feea155d44ca973, 0xbc93b3efbf5e2228,
    0x3feea09e667f3bcd, 0xbc6a12ad8734b982, 0x3feea012750bdabf,
    0xbc6367efb86da9ee, 0x3fee9fb23c651a2f, 0xbc80dc3d54e08851,
    0x3fee9f7df9519484, 0xbc781f647e5a3ecf, 0x3fee9f75e8ec5f74,
    0xbc86ee4ac08b7db0, 0x3fee9f9a48a58174, 0xbc8619321e55e68a,
    0x3fee9feb564267c9, 0x3c909ccb5e09d4d3, 0x3feea0694fde5d3f,
    0xbc7b32dcb94da51d, 0x3feea11473eb0187, 0x3c94ecfd5467c06b,
    0x3feea1ed0130c132, 0x3c65ebe1abd66c55, 0x3feea2f336cf4e62,
    0xbc88a1c52fb3cf42, 0x3feea427543e1a12, 0xbc9369b6f13b3734,
    0x3feea589994cce13, 0xbc805e843a19ff1e, 0x3feea71a4623c7ad,
    0xbc94d450d872576e, 0x3feea8d99b4492ed, 0x3c90ad675b0e8a00,
    0x3feeaac7d98a6699, 0x3c8db72fc1f0eab4, 0x3feeace5422aa0db,
    0xbc65b6609cc5e7ff, 0x3feeaf3216b5448c, 0x3c7bf68359f35f44,
    0x3feeb1ae99157736, 0xbc93091fa71e3d83, 0x3feeb45b0b91ffc6,
    0xbc5da9b88b6c1e29, 0x3feeb737b0cdc5e5, 0xbc6c23f97c90b959,
    0x3feeba44cbc8520f, 0xbc92434322f4f9aa, 0x3feebd829fde4e50,
    0xbc85ca6cd7668e4b, 0x3feec0f170ca07ba, 0x3c71affc2b91ce27,
    0x3feec49182a3f090, 0x3c6dd235e10a73bb, 0x3feec86319e32323,
    0xbc87c50422622263, 0x3feecc667b5de565, 0x3c8b1c86e3e231d5,
    0x3feed09bec4a2d33, 0xbc91bbd1d3bcbb15, 0x3feed503b23e255d,
    0x3c90cc319cee31d2, 0x3feed99e1330b358, 0x3c8469846e735ab3,
    0x3feede6b5579fdbf, 0xbc82dfcd978e9db4, 0x3feee36bbfd3f37a,
    0x3c8c1a7792cb3387, 0x3feee89f995ad3ad, 0xbc907b8f4ad1d9fa,
    0x3feeee07298db666, 0xbc55c3d956dcaeba, 0x3feef3a2b84f15fb,
    0xbc90a40e3da6f640, 0x3feef9728de5593a, 0xbc68d6f438ad9334,
    0x3feeff76f2fb5e47, 0xbc91eee26b588a35, 0x3fef05b030a1064a,
    0x3c74ffd70a5fddcd, 0x3fef0c1e904bc1d2, 0xbc91bdfbfa9298ac,
    0x3fef12c25bd71e09, 0x3c736eae30af0cb3, 0x3fef199bdd85529c,
    0x3c8ee3325c9ffd94, 0x3fef20ab5fffd07a, 0x3c84e08fd10959ac,
    0x3fef27f12e57d14b, 0x3c63cdaf384e1a67, 0x3fef2f6d9406e7b5,
    0x3c676b2c6c921968, 0x3fef3720dcef9069, 0xbc808a1883ccb5d2,
    0x3fef3f0b555dc3fa, 0xbc8fad5d3ffffa6f, 0x3fef472d4a07897c,
    0xbc900dae3875a949, 0x3fef4f87080d89f2, 0x3c74a385a63d07a7,
    0x3fef5818dcfba487, 0xbc82919e2040220f, 0x3fef60e316c98398,
    0x3c8e5a50d5c192ac, 0x3fef69e603db3285, 0x3c843a59ac016b4b,
    0x3fef7321f301b460, 0xbc82d52107b43e1f, 0x3fef7c97337b9b5f,
    0xbc892ab93b470dc9, 0x3fef864614f5a129, 0x3c74b604603a88d3,
    0x3fef902ee78b3ff6, 0x3c83c5ec519d7271, 0x3fef9a51fbc74c83,
    0xbc8ff7128fd391f0, 0x3fefa4afa2a490da, 0xbc8dae98e223747d,
    0x3fefaf482d8e67f1, 0x3c8ec3bc41aa2008, 0x3fefba1bee615a27,
    0x3c842b94c3a9eb32, 0x3fefc52b376bba97, 0x3c8a64a931d185ee,
    0x3fefd0765b6e4540, 0xbc8e37bae43be3ed, 0x3fefdbfdad9cbe14,
    0x3c77893b4d91cd9d, 0x3fefe7c1819e90d8, 0x3c5305c14160cc89,
    0x3feff3c22b8f71f1,
];
