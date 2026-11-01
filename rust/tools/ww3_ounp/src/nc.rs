// SPDX-License-Identifier: MPL-2.0
//! The calls of the NetCDF C library (`libnetcdf.so.19`, Debian bookworm `libnetcdf-dev` 4.9.0) that `ww3_ounp` makes
//! through the Fortran interface `NF90_*`, with the declarations of `ww3_ounf` (`rust/tools/ww3_ounf/src/nc.rs`)
//! extended by `nc_put_vara_int`, `nc_put_vara_text`, `nc_sync` and `nc_inq_varndims`.
//!
//! Unlike `ww3_ounf`, a call returns its status and the caller decides, because `ww3_ounp` ignores most of them
//! (`IRET` is overwritten) and `CHECK_ERR` stops on the others. The arguments are in the Fortran's order: `NF90_*`
//! reverses a dimension list, a `start` and a `count` against the C functions, and so do the wrappers here. A `start`
//! is 1-based. `NF90_PUT_VAR` of an array with no `count` takes the array's shape for the first dimensions and 1 for
//! the rest (`netcdf_expanded.f90` of netcdf-fortran), which `put_*` repeats with `shape`.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

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
    fn nc_put_vara_int(ncid: c_int, varid: c_int, start: *const usize, count: *const usize, op: *const c_int) -> c_int;
    fn nc_put_vara_double(ncid: c_int, varid: c_int, start: *const usize, count: *const usize, op: *const f64) -> c_int;
    fn nc_put_vara_text(ncid: c_int, varid: c_int, start: *const usize, count: *const usize, op: *const c_char) -> c_int;
    fn nc_redef(ncid: c_int) -> c_int;
    fn nc_enddef(ncid: c_int) -> c_int;
    fn nc_sync(ncid: c_int) -> c_int;
    fn nc_close(ncid: c_int) -> c_int;
    fn nc_inq_varndims(ncid: c_int, varid: c_int, ndimsp: *mut c_int) -> c_int;
    fn nc_strerror(ncerr: c_int) -> *const c_char;
    fn nc_inq_libvers() -> *const c_char;
}

pub const NC_CHAR: i32 = 2;
pub const NC_SHORT: i32 = 3;
pub const NC_INT: i32 = 4;
pub const NC_FLOAT: i32 = 5;
pub const NC_DOUBLE: i32 = 6;
pub const NC_GLOBAL: i32 = -1;
/// `NF90_NETCDF4`; `NF90_CLOBBER` is 0.
pub const NC_NETCDF4: i32 = 0x1000;
/// `NF90_FILL_SHORT`, `NF90_FILL_INT` and `NF90_FILL_FLOAT`.
pub const FILL_SHORT: i16 = -32767;
pub const FILL_INT: i32 = -2_147_483_647;
pub const FILL_FLOAT: f32 = f32::from_bits(0x7cf0_0000);
/// `DEFLATE=1` of `W3CRNC`.
pub const DEFLATE: i32 = 1;

fn c(s: &str) -> CString {
    CString::new(s).unwrap_or_else(|_| panic!("{s:?} holds a NUL"))
}

/// `NF90_STRERROR`.
pub fn strerror(code: i32) -> String {
    // SAFETY: nc_strerror returns a static NUL-terminated string for any code.
    unsafe { CStr::from_ptr(nc_strerror(code)) }.to_string_lossy().into_owned()
}

/// `NF90_INQ_LIBVERS`.
pub fn libvers() -> String {
    // SAFETY: nc_inq_libvers returns a static NUL-terminated string.
    unsafe { CStr::from_ptr(nc_inq_libvers()) }.to_string_lossy().into_owned()
}

pub fn create(path: &str, cmode: i32, ncid: &mut i32) -> i32 {
    // SAFETY: the path is NUL-terminated and `ncid` is a valid out pointer.
    unsafe { nc_create(c(path).as_ptr(), cmode, ncid) }
}

pub fn def_dim(ncid: i32, name: &str, len: usize, id: &mut i32) -> i32 {
    // SAFETY: as create.
    unsafe { nc_def_dim(ncid, c(name).as_ptr(), len, id) }
}

/// `dims` in the Fortran's order, the fastest-varying dimension first.
pub fn def_var(ncid: i32, name: &str, xtype: i32, dims: &[i32], id: &mut i32) -> i32 {
    let rev: Vec<i32> = dims.iter().rev().copied().collect();
    // SAFETY: `rev` holds `rev.len()` ids.
    unsafe { nc_def_var(ncid, c(name).as_ptr(), xtype, rev.len() as c_int, rev.as_ptr(), id) }
}

pub fn def_var_deflate(ncid: i32, varid: i32, shuffle: i32, deflate: i32, level: i32) -> i32 {
    // SAFETY: plain integers.
    unsafe { nc_def_var_deflate(ncid, varid, shuffle, deflate, level) }
}

/// `NF90_PUT_ATT` of a character value writes `len_trim` bytes.
pub fn put_att_text(ncid: i32, varid: i32, name: &str, value: &str) -> i32 {
    let v = value.trim_end_matches(' ');
    // SAFETY: `v` is `v.len()` bytes; the call copies them.
    unsafe { nc_put_att_text(ncid, varid, c(name).as_ptr(), v.len(), v.as_ptr().cast()) }
}

pub fn put_att_float(ncid: i32, varid: i32, name: &str, v: f32) -> i32 {
    // SAFETY: one float.
    unsafe { nc_put_att_float(ncid, varid, c(name).as_ptr(), NC_FLOAT, 1, &v) }
}

pub fn put_att_int(ncid: i32, varid: i32, name: &str, v: i32) -> i32 {
    // SAFETY: one int.
    unsafe { nc_put_att_int(ncid, varid, c(name).as_ptr(), NC_INT, 1, &v) }
}

pub fn put_att_short(ncid: i32, varid: i32, name: &str, v: i16) -> i32 {
    // SAFETY: one short.
    unsafe { nc_put_att_short(ncid, varid, c(name).as_ptr(), NC_SHORT, 1, &v) }
}

/// The C `start` and `count` of a Fortran `start` (1-based, `NF90_PUT_VAR`'s `start`), `count` and the shape of the
/// values: `None` when the variable cannot be queried.
fn region(ncid: i32, varid: i32, start: &[usize], count: Option<&[usize]>, shape: &[usize]) -> Option<(Vec<usize>, Vec<usize>)> {
    let mut nd: c_int = 0;
    // SAFETY: `nd` is a valid out pointer.
    if unsafe { nc_inq_varndims(ncid, varid, &mut nd) } != 0 {
        return None;
    }
    let nd = nd as usize;
    let mut lc: Vec<usize> = (0..nd).map(|i| shape.get(i).copied().unwrap_or(1)).collect();
    if let Some(cn) = count {
        for (i, v) in cn.iter().enumerate().take(nd) {
            lc[i] = *v;
        }
    }
    let ls: Vec<usize> = (0..nd).map(|i| start.get(i).copied().unwrap_or(1)).collect();
    // A start of 0 is not a Fortran index; the library would see a huge unsigned value.
    let cs = (0..nd).map(|k| ls[nd - 1 - k].wrapping_sub(1)).collect();
    let cc = (0..nd).map(|k| lc[nd - 1 - k]).collect();
    Some((cs, cc))
}

/// `NF90_PUT_VAR` of a default `REAL` scalar or array.
pub fn put_real(ncid: i32, varid: i32, start: &[usize], count: Option<&[usize]>, shape: &[usize], v: &[f32]) -> i32 {
    let Some((s, n)) = region(ncid, varid, start, count, shape) else { return -49 };
    // SAFETY: `v` holds the product of the Fortran `count` values for every call here, and `s`, `n` have the rank.
    unsafe { nc_put_vara_float(ncid, varid, s.as_ptr(), n.as_ptr(), v.as_ptr()) }
}

/// `NF90_PUT_VAR` of a default `INTEGER` scalar or array.
pub fn put_int(ncid: i32, varid: i32, start: &[usize], count: Option<&[usize]>, shape: &[usize], v: &[i32]) -> i32 {
    let Some((s, n)) = region(ncid, varid, start, count, shape) else { return -49 };
    // SAFETY: as put_real.
    unsafe { nc_put_vara_int(ncid, varid, s.as_ptr(), n.as_ptr(), v.as_ptr()) }
}

/// `NF90_PUT_VAR` of a `DOUBLE PRECISION` scalar.
pub fn put_double(ncid: i32, varid: i32, start: &[usize], v: f64) -> i32 {
    let Some((s, n)) = region(ncid, varid, start, None, &[]) else { return -49 };
    // SAFETY: one double.
    unsafe { nc_put_vara_double(ncid, varid, s.as_ptr(), n.as_ptr(), &v) }
}

/// `NF90_PUT_VAR` of a character scalar with `start` and `count`: the first `count` bytes of `text`.
pub fn put_text(ncid: i32, varid: i32, start: &[usize], count: &[usize], text: &str) -> i32 {
    let Some((s, n)) = region(ncid, varid, start, Some(count), &[text.len()]) else { return -49 };
    // SAFETY: `text` holds at least `count[0]` bytes in every call here (`len_trim`).
    unsafe { nc_put_vara_text(ncid, varid, s.as_ptr(), n.as_ptr(), text.as_ptr().cast()) }
}

pub fn redef(ncid: i32) -> i32 {
    // SAFETY: plain integer.
    unsafe { nc_redef(ncid) }
}

pub fn enddef(ncid: i32) -> i32 {
    // SAFETY: plain integer.
    unsafe { nc_enddef(ncid) }
}

pub fn sync(ncid: i32) -> i32 {
    // SAFETY: plain integer.
    unsafe { nc_sync(ncid) }
}

pub fn close(ncid: i32) -> i32 {
    // SAFETY: plain integer.
    unsafe { nc_close(ncid) }
}
