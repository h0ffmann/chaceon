# W3ULEV, W3UICE: water levels and ice concentration in a run

`shel.rs` translates `W3ULEV` (`model/src/w3updtmd.F90:2012-2539` at `92e2cf66`) and `W3UICE` (`:1755-1998`) for `IC0`, with
the scalar inputs `W3FLDH` and `W3FLDG` give them. `grid.rs` has the ice part of `W3UTRN`. Host only, no kernel.

**Status: `host`.** Nothing ran on a GPU ⚠: `ww3_shel` refuses both on a device, because they change `VA`, `WN` and `CG`
between the kernels.

## Result

`just forcing-all`: `out_grd.ww3`, `out_pnt.ww3` and the last restart are byte-identical on `forcing_gen.py`'s variants on
`ww3_tp2.15`'s grid: levels that dry and wet cells, small levels that remap spectra, input that starts late and ends early, ice
with `CICE0 < CICEN`, linear and exponential, both together, homogeneous input, a restart in mid-run, and `ww3_tic2.2` and
`ww3_ts3` (v). `just bound-all`'s `level` adds a homogeneous level with no other input to break up the last segment.

## Traps

- `FLLSTL` is set when the input ends, and `INFLAGS1(1)` goes false only after the next `W3WAVE` (`ww3_shel.F90:2565-2582`).
  That call still has `TLN` at `TN` plus a second, so `W3ULEV` fires at the midpoint and `TLEV` in the restart becomes
  `TN + 1`. Another input that splits the segment before the midpoint hides this, as file winds did in `forcing-all`'s
  `homog`; the first version turned the field off at once and passed that case.
- `FLLSTI` is set only in `W3FLDG`'s branch: homogeneous ice stays on with `TIN` at `TN` plus a second.
- A field at `T0` itself ends `W3FLDG`'s search for levels and ice, unlike interpolated fields.
- `W3DIMA` starts `CFLXYMAX`, `CHARN` and the fluxes at `UNDEF`, and a point `W3SRCE` has not reached is written so.

## What stops

- `RD2` in `W3ULEV` is read unset when no old wavenumber reaches the new first one. Rust keeps the value of the point
  before, which is not proved to be what that stack slot holds ⚠.
- Icebergs (`ISI`), `TRFLAG` 1 and 2, `IC1` to `IC5`, tides.

## Timing

None.
