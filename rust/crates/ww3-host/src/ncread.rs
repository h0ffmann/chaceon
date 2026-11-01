// SPDX-License-Identifier: MPL-2.0
//! The read side of the NetCDF C library (`libnetcdf.so.19`, Debian bookworm `libnetcdf-dev` 4.9.0) that
//! `ww3_prnc` uses through the Fortran interface `NF90_*`. Every call returns the library's status code in
//! `Err`, because the Fortran program tests `IRET` and `CHECK_ERR` prints the library's message itself.
//! Dimension lists, `start` and `count` are in C order (the slowest-varying dimension first); `prnc.rs`
//! reverses them where the Fortran lists them the other way. Without the Cargo feature `netcdf` every call
//! stops with status 64, so `cargo check --workspace` needs no NetCDF on the host.

#[cfg(feature = "netcdf")]
mod imp {
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_double, c_float, c_int};

    #[link(name = "netcdf")]
    unsafe extern "C" {
        fn nc_open(path: *const c_char, mode: c_int, ncidp: *mut c_int) -> c_int;
        fn nc_close(ncid: c_int) -> c_int;
        fn nc_inq_varid(ncid: c_int, name: *const c_char, varidp: *mut c_int) -> c_int;
        fn nc_inq_varndims(ncid: c_int, varid: c_int, ndimsp: *mut c_int) -> c_int;
        fn nc_inq_vardimid(ncid: c_int, varid: c_int, dimids: *mut c_int) -> c_int;
        fn nc_inq_dimname(ncid: c_int, dimid: c_int, name: *mut c_char) -> c_int;
        fn nc_inq_dimlen(ncid: c_int, dimid: c_int, lenp: *mut usize) -> c_int;
        fn nc_inq_attlen(ncid: c_int, varid: c_int, name: *const c_char, lenp: *mut usize) -> c_int;
        fn nc_get_att_text(ncid: c_int, varid: c_int, name: *const c_char, ip: *mut c_char) -> c_int;
        fn nc_get_att_float(ncid: c_int, varid: c_int, name: *const c_char, ip: *mut c_float) -> c_int;
        fn nc_get_vara_float(ncid: c_int, varid: c_int, start: *const usize, count: *const usize, ip: *mut c_float) -> c_int;
        fn nc_get_vara_double(ncid: c_int, varid: c_int, start: *const usize, count: *const usize, ip: *mut c_double) -> c_int;
        fn nc_strerror(ncerr: c_int) -> *const c_char;
    }

    /// `NC_MAX_NAME` is 256.
    const MAX_NAME: usize = 257;

    fn c(s: &str) -> CString {
        CString::new(s).unwrap_or_else(|_| panic!("{s:?} holds a NUL"))
    }

    fn st<T>(ret: c_int, v: T) -> Result<T, i32> {
        if ret == 0 { Ok(v) } else { Err(ret) }
    }

    pub fn open(path: &str) -> Result<i32, i32> {
        let mut id = 0;
        // SAFETY: the path is NUL-terminated and `id` is a valid out pointer for the call. Mode 0 is NF90_NOWRITE.
        st(unsafe { nc_open(c(path).as_ptr(), 0, &mut id) }, id)
    }

    pub fn close(ncid: i32) {
        // SAFETY: plain integer; the status is ignored at the end of the run, as the Fortran never closes the file.
        let _ = unsafe { nc_close(ncid) };
    }

    pub fn inq_varid(ncid: i32, name: &str) -> Result<i32, i32> {
        let mut id = 0;
        // SAFETY: as open.
        st(unsafe { nc_inq_varid(ncid, c(name).as_ptr(), &mut id) }, id)
    }

    pub fn inq_varndims(ncid: i32, varid: i32) -> Result<usize, i32> {
        let mut n = 0;
        // SAFETY: `n` is a valid out pointer.
        st(unsafe { nc_inq_varndims(ncid, varid, &mut n) }, n as usize)
    }

    /// The dimension ids of a variable, in C order.
    pub fn inq_vardimids(ncid: i32, varid: i32) -> Result<Vec<i32>, i32> {
        let n = inq_varndims(ncid, varid)?;
        let mut ids = vec![0 as c_int; n.max(1)];
        // SAFETY: `ids` holds at least the variable's rank.
        st(unsafe { nc_inq_vardimid(ncid, varid, ids.as_mut_ptr()) }, ())?;
        ids.truncate(n);
        Ok(ids)
    }

    /// Name and length of a dimension.
    pub fn inq_dim(ncid: i32, dimid: i32) -> Result<(String, usize), i32> {
        let mut name = [0 as c_char; MAX_NAME];
        let mut len = 0usize;
        // SAFETY: `name` holds NC_MAX_NAME + 1 bytes; `len` is a valid out pointer.
        st(unsafe { nc_inq_dimname(ncid, dimid, name.as_mut_ptr()) }, ())?;
        st(unsafe { nc_inq_dimlen(ncid, dimid, &mut len) }, ())?;
        // SAFETY: nc_inq_dimname NUL-terminates.
        let s = unsafe { CStr::from_ptr(name.as_ptr()) }.to_string_lossy().into_owned();
        Ok((s, len))
    }

    /// A character attribute, as the bytes the file holds.
    pub fn get_att_text(ncid: i32, varid: i32, name: &str) -> Result<String, i32> {
        let mut len = 0usize;
        // SAFETY: as open.
        st(unsafe { nc_inq_attlen(ncid, varid, c(name).as_ptr(), &mut len) }, ())?;
        let mut buf = vec![0 as c_char; len.max(1)];
        // SAFETY: `buf` holds the attribute's length; the call does not NUL-terminate.
        st(unsafe { nc_get_att_text(ncid, varid, c(name).as_ptr(), buf.as_mut_ptr()) }, ())?;
        let bytes: Vec<u8> = buf[..len].iter().map(|b| *b as u8).collect();
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// `NF90_GET_ATT` into a `REAL`: the library converts from the attribute's type.
    pub fn get_att_float(ncid: i32, varid: i32, name: &str) -> Result<f32, i32> {
        let mut v = 0.0f32;
        // SAFETY: one float.
        st(unsafe { nc_get_att_float(ncid, varid, c(name).as_ptr(), &mut v) }, v)
    }

    pub fn get_vara_float(ncid: i32, varid: i32, start: &[usize], count: &[usize]) -> Result<Vec<f32>, i32> {
        let mut v = vec![0.0f32; count.iter().product::<usize>()];
        // SAFETY: `v` holds the product of `count` values; `start` and `count` have the variable's rank.
        st(unsafe { nc_get_vara_float(ncid, varid, start.as_ptr(), count.as_ptr(), v.as_mut_ptr()) }, ())?;
        Ok(v)
    }

    pub fn get_vara_double(ncid: i32, varid: i32, start: &[usize], count: &[usize]) -> Result<Vec<f64>, i32> {
        let mut v = vec![0.0f64; count.iter().product::<usize>()];
        // SAFETY: as get_vara_float.
        st(unsafe { nc_get_vara_double(ncid, varid, start.as_ptr(), count.as_ptr(), v.as_mut_ptr()) }, ())?;
        Ok(v)
    }

    pub fn strerror(code: i32) -> String {
        // SAFETY: nc_strerror returns a static NUL-terminated string for any code.
        unsafe { CStr::from_ptr(nc_strerror(code)) }.to_string_lossy().into_owned()
    }
}

#[cfg(not(feature = "netcdf"))]
mod imp {
    fn off() -> ! {
        eprintln!("ww3_prnc: built without the netcdf feature (scripts/rust_netcdf.sh builds with it)");
        std::process::exit(64)
    }
    pub fn open(_: &str) -> Result<i32, i32> {
        off()
    }
    pub fn close(_: i32) {
        off()
    }
    pub fn inq_varid(_: i32, _: &str) -> Result<i32, i32> {
        off()
    }
    pub fn inq_varndims(_: i32, _: i32) -> Result<usize, i32> {
        off()
    }
    pub fn inq_vardimids(_: i32, _: i32) -> Result<Vec<i32>, i32> {
        off()
    }
    pub fn inq_dim(_: i32, _: i32) -> Result<(String, usize), i32> {
        off()
    }
    pub fn get_att_text(_: i32, _: i32, _: &str) -> Result<String, i32> {
        off()
    }
    pub fn get_att_float(_: i32, _: i32, _: &str) -> Result<f32, i32> {
        off()
    }
    pub fn get_vara_float(_: i32, _: i32, _: &[usize], _: &[usize]) -> Result<Vec<f32>, i32> {
        off()
    }
    pub fn get_vara_double(_: i32, _: i32, _: &[usize], _: &[usize]) -> Result<Vec<f64>, i32> {
        off()
    }
    pub fn strerror(_: i32) -> String {
        off()
    }
}

pub use imp::*;
