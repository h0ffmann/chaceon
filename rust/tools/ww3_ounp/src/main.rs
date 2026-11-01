// SPDX-License-Identifier: MPL-2.0
//! `ww3_ounp` in Rust: reads `mod_def.ww3`, `out_pnt.ww3` and `ww3_ounp.nml` from a case directory and writes the NetCDF
//! files there, as WW3's `ww3_ounp` does in its working directory. It links the system NetCDF C library
//! (`scripts/rust_netcdf.sh` builds it); without the `netcdf` feature it stops at once.

#[cfg(feature = "netcdf")]
mod crnc;
#[cfg(feature = "netcdf")]
mod exnc;
#[cfg(feature = "netcdf")]
mod nc;

#[cfg(feature = "netcdf")]
fn main() {
    let dir = std::env::args().nth(1).expect("case directory");
    std::env::set_current_dir(&dir).unwrap_or_else(|e| panic!("{dir}: {e}"));
    exnc::run()
}

#[cfg(not(feature = "netcdf"))]
fn main() {
    eprintln!("ww3_ounp was built without the `netcdf` feature: build it with scripts/rust_netcdf.sh");
    std::process::exit(64);
}
