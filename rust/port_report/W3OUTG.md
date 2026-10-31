# W3OUTG and W3IOGO: field output

`rust/crates/ww3-host/src/iogo.rs` translates `W3OUTG`, `W3IOGO`, `CALC_WBT` and the `USF` path of
`CALC_U3STOKES`, and writes `out_grd.ww3`. `W3CFLXY` is in `shel.rs`.

**Status: `host`.** This code runs on the host and has no device form.

`out_grd.ww3` is byte-identical on every case of `W3PRO3.md` and all 18 `ww3_ts1` variants (v, `just tp-all`,
`just ts1-pair`). The field lists of those cases reach these names: `DPT CUR WND ICE WLV HS T02 T0M1 T01 FP
DIR SPR DP HIG MXE MXES MXH MXHC SDMH SDMHC WBT EF TH1M STH1M MSS MSD WCC WCF WCH WCM FAW FOC TAW CHA FWS TWO BHD
SXY USS USF FC CFX P2L`. `P2L` is cleared when `mod_def.ww3` gives it no bounds, as `W3FLGRDUPDT` does.

## Written, and no case asks for it ⚠

`AST LM TP UST CGE TWA TUS P2S ABR UBR FBB MSC MCD QP DTD`. A field outside `IDS` in `iogo.rs` stops the run
with its name: partitions, `TH2M`, `WN`, `USP` and the rest.

## What it took

- `W3IOGO` loops `IFI` outside `IFJ`, and a field is one record per array.
- `W3IOGO` blanks fields to `UNDEF` at dry points, and a shorter list also at `MAPSTA = 2`, and it writes `UST`
  back as `UNDEF` at dry points, which a later restart keeps.
- WW3 ends with `EET1 = ET1 + FT1*EBAND**2`, so `QP` starts from `ET1` and what the loop summed into `EET1` is
  lost. It is translated as written.
- gfortran joins `COS(UD)` and `SIN(UD)` into one `sincosf`.
- `ESIG**4.` has a real exponent, so `CALC_WBT` keeps `powf`.
