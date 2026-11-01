// SPDX-License-Identifier: MPL-2.0
//! The calls of the NetCDF C library (`libnetcdf.so.19`, Debian bookworm `libnetcdf-dev` 4.9.0) that
//! `ww3_ounf` makes through the Fortran interface `NF90_*`. `NF90_*` calls the C functions with the
//! dimensions in the reverse order; here every dimension list and every `start` and `count` is in C order.
//! A call that returns an error stops the run with the library's message, as `CHECK_ERR` does.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_longlong};

#[link(name = "netcdf")]
unsafe extern "C" {
    fn nc_create(path: *const c_char, cmode: c_int, ncidp: *mut c_int) -> c_int;
    fn nc_def_dim(ncid: c_int, name: *const c_char, len: usize, idp: *mut c_int) -> c_int;
    fn nc_def_var(ncid: c_int, name: *const c_char, xtype: c_int, ndims: c_int, dimids: *const c_int, varidp: *mut c_int) -> c_int;
    fn nc_def_var_deflate(ncid: c_int, varid: c_int, shuffle: c_int, deflate: c_int, level: c_int) -> c_int;
    fn nc_put_att_text(ncid: c_int, varid: c_int, name: *const c_char, len: usize, op: *const c_char) -> c_int;
    fn nc_put_att_float(ncid: c_int, varid: c_int, name: *const c_char, xtype: c_int, len: usize, op: *const f32) -> c_int;
    fn nc_put_att_int(ncid: c_int, varid: c_int, name: *const c_char, xtype: c_int, len: usize, op: *const c_int) -> c_int;
    fn nc_put_att_short(ncid: c_int, varid: c_int, name: *const c_char, xtype: c_int, len: usize, op: *const i16) -> c_int;
    fn nc_put_vara_float(ncid: c_int, varid: c_int, start: *const usize, count: *const usize, op: *const f32) -> c_int;
    fn nc_put_vara_short(ncid: c_int, varid: c_int, start: *const usize, count: *const usize, op: *const i16) -> c_int;
    fn nc_put_vara_double(ncid: c_int, varid: c_int, start: *const usize, count: *const usize, op: *const f64) -> c_int;
    fn nc_put_vara_longlong(ncid: c_int, varid: c_int, start: *const usize, count: *const usize, op: *const c_longlong) -> c_int;
    fn nc_redef(ncid: c_int) -> c_int;
    fn nc_enddef(ncid: c_int) -> c_int;
    fn nc_close(ncid: c_int) -> c_int;
    fn nc_inq_varid(ncid: c_int, name: *const c_char, varidp: *mut c_int) -> c_int;
    fn nc_inq_dimid(ncid: c_int, name: *const c_char, idp: *mut c_int) -> c_int;
    fn nc_inq_dimlen(ncid: c_int, dimid: c_int, lenp: *mut usize) -> c_int;
    fn nc_strerror(ncerr: c_int) -> *const c_char;
}

pub const NC_SHORT: c_int = 3;
pub const NC_INT: c_int = 4;
pub const NC_FLOAT: c_int = 5;
pub const NC_DOUBLE: c_int = 6;
pub const NC_INT64: c_int = 10;
pub const NC_GLOBAL: c_int = -1;
/// `NF90_NETCDF4`; `NF90_CLOBBER` is 0.
pub const NC_NETCDF4: c_int = 0x1000;
pub const NC_UNLIMITED: usize = 0;
/// `NF90_FILL_SHORT` and `NF90_FILL_FLOAT`.
pub const FILL_SHORT: i16 = -32767;
pub const FILL_FLOAT: f32 = f32::from_bits(0x7cf0_0000);

fn ck(ret: c_int, what: &str) {
    if ret != 0 {
        // SAFETY: nc_strerror returns a static NUL-terminated string for any code.
        let msg = unsafe { CStr::from_ptr(nc_strerror(ret)) }.to_string_lossy().into_owned();
        panic!(" *** WAVEWATCH III ERROR IN OUNF : NETCDF ERROR MESSAGE: {msg} (in {what})");
    }
}

fn c(s: &str) -> CString {
    CString::new(s).unwrap_or_else(|_| panic!("{s:?} holds a NUL"))
}

pub fn create(path: &str, cmode: c_int) -> i32 {
    let mut id = 0;
    // SAFETY: the path is NUL-terminated and `id` is a valid out pointer for the call.
    ck(unsafe { nc_create(c(path).as_ptr(), cmode, &mut id) }, "nc_create");
    id
}

pub fn def_dim(ncid: i32, name: &str, len: usize) -> i32 {
    let mut id = 0;
    // SAFETY: as above.
    ck(unsafe { nc_def_dim(ncid, c(name).as_ptr(), len, &mut id) }, "nc_def_dim");
    id
}

/// `dims` in C order: the slowest-varying dimension first.
pub fn def_var(ncid: i32, name: &str, xtype: i32, dims: &[i32]) -> i32 {
    let mut id = 0;
    // SAFETY: `dims` holds `dims.len()` ids.
    ck(unsafe { nc_def_var(ncid, c(name).as_ptr(), xtype, dims.len() as c_int, dims.as_ptr(), &mut id) }, "nc_def_var");
    id
}

pub fn def_var_deflate(ncid: i32, varid: i32, level: i32) {
    // SAFETY: plain integers.
    ck(unsafe { nc_def_var_deflate(ncid, varid, 1, 1, level) }, "nc_def_var_deflate");
}

pub fn att_text(ncid: i32, varid: i32, name: &str, value: &str) {
    let v = value.trim_end_matches(' ');
    // SAFETY: `v` is `v.len()` bytes; the call copies them.
    ck(unsafe { nc_put_att_text(ncid, varid, c(name).as_ptr(), v.len(), v.as_ptr().cast()) }, "nc_put_att_text");
}

pub fn att_float(ncid: i32, varid: i32, name: &str, v: f32) {
    // SAFETY: one float.
    ck(unsafe { nc_put_att_float(ncid, varid, c(name).as_ptr(), NC_FLOAT, 1, &v) }, "nc_put_att_float");
}

pub fn att_int(ncid: i32, varid: i32, name: &str, v: i32) {
    // SAFETY: one int.
    ck(unsafe { nc_put_att_int(ncid, varid, c(name).as_ptr(), NC_INT, 1, &v) }, "nc_put_att_int");
}

pub fn att_short(ncid: i32, varid: i32, name: &str, v: i16) {
    // SAFETY: one short.
    ck(unsafe { nc_put_att_short(ncid, varid, c(name).as_ptr(), NC_SHORT, 1, &v) }, "nc_put_att_short");
}

pub fn put_f32(ncid: i32, varid: i32, start: &[usize], count: &[usize], v: &[f32]) {
    assert_eq!(v.len(), count.iter().product::<usize>());
    // SAFETY: `v` holds the product of `count` values; `start` and `count` have the variable's rank.
    ck(unsafe { nc_put_vara_float(ncid, varid, start.as_ptr(), count.as_ptr(), v.as_ptr()) }, "nc_put_vara_float");
}

pub fn put_i16(ncid: i32, varid: i32, start: &[usize], count: &[usize], v: &[i16]) {
    assert_eq!(v.len(), count.iter().product::<usize>());
    // SAFETY: as put_f32.
    ck(unsafe { nc_put_vara_short(ncid, varid, start.as_ptr(), count.as_ptr(), v.as_ptr()) }, "nc_put_vara_short");
}

pub fn put_f64(ncid: i32, varid: i32, start: &[usize], count: &[usize], v: &[f64]) {
    assert_eq!(v.len(), count.iter().product::<usize>());
    // SAFETY: as put_f32.
    ck(unsafe { nc_put_vara_double(ncid, varid, start.as_ptr(), count.as_ptr(), v.as_ptr()) }, "nc_put_vara_double");
}

pub fn put_i64(ncid: i32, varid: i32, start: &[usize], count: &[usize], v: &[i64]) {
    assert_eq!(v.len(), count.iter().product::<usize>());
    // SAFETY: as put_f32.
    ck(unsafe { nc_put_vara_longlong(ncid, varid, start.as_ptr(), count.as_ptr(), v.as_ptr()) }, "nc_put_vara_longlong");
}

pub fn redef(ncid: i32) {
    // SAFETY: plain integer.
    ck(unsafe { nc_redef(ncid) }, "nc_redef");
}

pub fn enddef(ncid: i32) {
    // SAFETY: plain integer.
    ck(unsafe { nc_enddef(ncid) }, "nc_enddef");
}

pub fn close(ncid: i32) {
    // SAFETY: plain integer.
    ck(unsafe { nc_close(ncid) }, "nc_close");
}

pub fn inq_varid(ncid: i32, name: &str) -> i32 {
    let mut id = 0;
    // SAFETY: as def_dim.
    ck(unsafe { nc_inq_varid(ncid, c(name).as_ptr(), &mut id) }, "nc_inq_varid");
    id
}

pub fn inq_dimid(ncid: i32, name: &str) -> i32 {
    let mut id = 0;
    // SAFETY: as def_dim.
    ck(unsafe { nc_inq_dimid(ncid, c(name).as_ptr(), &mut id) }, "nc_inq_dimid");
    id
}

pub fn inq_dimlen(ncid: i32, dimid: i32) -> usize {
    let mut n = 0;
    // SAFETY: `n` is a valid out pointer.
    ck(unsafe { nc_inq_dimlen(ncid, dimid, &mut n) }, "nc_inq_dimlen");
    n
}
