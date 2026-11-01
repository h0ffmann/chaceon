#!/usr/bin/env bash
# ww3_prep inputs the regtests do not have for the interpolating input types LL, F1 and F2, for DAT, for
# binary input (IDFM 3) and for format reversal, built into regtests/prep/input and judged by prep_pair.sh on
# three model grids: a small spherical grid with a land patch (sph), a global one with simple closure (glob)
# and ww3_ts3's Cartesian grid. Every field is quantised to multiples of 1/32, so the text and the binary
# files of one case hold the same numbers. Then inputs the Fortran stops on, whose exit status Rust must
# repeat (prep_pair.sh with PREP_ERROR=1), and inputs the Fortran accepts and Rust refuses (exit 3).
# Usage: prep_interp.sh [gen|run|all] [name-pattern]. gen only writes the inputs; run needs them.
# Exit 0 when every case that should be identical is, every error repeats and every refusal was refused.
set -euo pipefail
cd "$(dirname "$0")/.."
mode="${1:-all}"
pat="${2:-.}"
export LC_ALL=C
v=regtests/prep/input
mkdir -p "$v"

# ---- data -------------------------------------------------------------------------------------------------
# fld KIND NX NY [PER] [P0 P1 P2]: NX*NY values, row 1 first, NX to a row (or PER to a line). u v t ice lev chk
# are fields of the indices; lin is P0 + P1*(i-1) + P2*(j-1), for coordinates.
fld() {
  awk -v kind="$1" -v nx="$2" -v ny="$3" -v per="${4:-0}" -v p0="${5:-0}" -v p1="${6:-0}" -v p2="${7:-0}" 'BEGIN {
    n = 0
    for (j = 1; j <= ny; j++) {
      for (i = 1; i <= nx; i++) {
        if (kind == "u") x = 5 + 3 * sin(0.35 * i) + 2 * cos(0.2 * j)
        else if (kind == "v") x = -2 + 4 * cos(0.3 * i + 0.1 * j)
        else if (kind == "t") x = 2 * sin(0.1 * (i + j))
        else if (kind == "ice") x = 0.5 + 0.5 * sin(0.4 * i) * cos(0.3 * j)
        else if (kind == "lev") x = 0.8 * sin(0.25 * i) + 0.4 * cos(0.3 * j)
        else if (kind == "chk") x = ((i + j) % 2 == 0) ? 5 : -5
        else if (kind == "lin") x = p0 + p1 * (i - 1) + p2 * (j - 1)
        else { print "fld: unknown kind " kind > "/dev/stderr"; exit 1 }
        q = int(x * 32 + (x < 0 ? -0.5 : 0.5)) / 32 + 0
        n++
        w = (per > 0) ? per : nx
        printf "%s%.5f", (n % w == 1 || w == 1) ? "  " : " ", q
        if (n % w == 0) printf "\n"
      }
    }
    if (n % ((per > 0) ? per : nx) != 0) printf "\n"
  }'
}
# ffld KIND NX NY FORMAT [P0 P1 P2]: the same numbers, one row to a record, each in printf FORMAT.
ffld() {
  fld "$1" "$2" "$3" 1 "${5:-0}" "${6:-0}" "${7:-0}" | tr -d ' ' | awk -v fmt="$4" -v nx="$2" '{ buf = buf sprintf(fmt, $1); if (NR % nx == 0) { print buf; buf = "" } }'
}
# recrows KIND NX NY PER FORMAT: each row of NX values in records of PER values (the last one shorter), each in
# printf FORMAT, so that a format of PER descriptors reverts once or twice to read one row.
recrows() {
  fld "$1" "$2" "$3" 1 | awk -v nx="$2" -v per="$4" -v fmt="$5" '{ k = (NR - 1) % nx; printf fmt, $1; if ((k + 1) % per == 0 || k == nx - 1) printf "\n" }'
}
# mask NX NY X1 X2 Y1 Y2: ones, with zeros in columns X1..X2 of rows Y1..Y2, NX to a row.
mask() {
  awk -v nx="$1" -v ny="$2" -v x1="$3" -v x2="$4" -v y1="$5" -v y2="$6" 'BEGIN {
    for (j = 1; j <= ny; j++) { for (i = 1; i <= nx; i++) printf "%s%d", (i == 1 ? "  " : " "), (i >= x1 && i <= x2 && j >= y1 && j <= y2) ? 0 : 1; printf "\n" }
  }'
}
# fmask: the same in (NXI2) format.
fmask() {
  awk -v nx="$1" -v ny="$2" -v x1="$3" -v x2="$4" -v y1="$5" -v y2="$6" 'BEGIN {
    for (j = 1; j <= ny; j++) { for (i = 1; i <= nx; i++) printf "%2d", (i >= x1 && i <= x2 && j >= y1 && j <= y2) ? 0 : 1; printf "\n" }
  }'
}
# hexrec TYPE [ROWLEN]: the numbers on stdin as unformatted records, big-endian, f = REAL, i = INTEGER, one
# record or one every ROWLEN numbers.
hexrec() {
  awk -v t="$1" -v rl="${2:-0}" '
    function be(n,   hi, lo) { hi = int(n / 65536); lo = n - hi * 65536; return sprintf("%04x%04x", hi, lo) }
    function f32(x,   s, e, m, b) {
      if (x == 0) return "00000000"
      s = 0; if (x < 0) { s = 1; x = -x }
      e = 0
      while (x >= 2) { x /= 2; e++ }
      while (x < 1) { x *= 2; e-- }
      m = int((x - 1) * 8388608 + 0.5)
      b = s * 2147483648 + (e + 127) * 8388608 + m
      return be(b)
    }
    function flush(   k, s) {
      if (n == 0) return
      s = be(n * 4)
      for (k = 1; k <= n; k++) s = s ((t == "f") ? f32(a[k]) : be((a[k] < 0) ? a[k] + 4294967296 : a[k]))
      printf "%s%s", s, be(n * 4)
      n = 0
    }
    { for (k = 1; k <= NF; k++) { a[++n] = $k + 0; if (rl > 0 && n == rl) flush() } }
    END { flush() }' | xxd -r -p
}
hdrrec() { # hdrrec ID NX NY GTYPE: the header record W3FLDO writes
  { printf '%08x' 40; printf '%s' "WAVEWATCH III$1" | xxd -p | tr -d '\n'; printf '%08x%08x%08x%08x%08x%08x' "$2" "$3" "$4" 0 0 0; printf '%08x' 40; } | xxd -r -p
}
timerec() { printf '%s %s\n' "$1" "$2" | hexrec i; }

# ---- the model grids -------------------------------------------------------------------------------------
sphgrid() { # sphgrid NAME CLOS NX NY SX SY X0 Y0 DEPTHFILE
  cat > "$v/ww3_grid_$1.nml" <<EOF
! ww3_prep interpolation tests: a small spherical grid, made by scripts/prep_interp.sh
&SPECTRUM_NML
  SPECTRUM%XFR       =  1.1
  SPECTRUM%FREQ1     =  0.040
  SPECTRUM%NK        =  9
  SPECTRUM%NTH       =  24
/
&RUN_NML
  RUN%FLCX         =  T
  RUN%FLCY         =  T
  RUN%FLCTH        =  T
/
&TIMESTEPS_NML
  TIMESTEPS%DTMAX        =  2200.
  TIMESTEPS%DTXY         =  2200.
  TIMESTEPS%DTKTH        =  2200.
  TIMESTEPS%DTMIN        =  2200.
/
&GRID_NML
  GRID%NAME         =  'PREP INTERPOLATION $1'
  GRID%NML          =  '../input/namelists_2-D.nml'
  GRID%TYPE         =  'RECT'
  GRID%COORD        =  'SPHE'
  GRID%CLOS         =  '$2'
  GRID%ZLIM         =  -5.
  GRID%DMIN         =  5.75
/
&RECT_NML
  RECT%NX           =  $3
  RECT%NY           =  $4
  RECT%SX           =  $5
  RECT%SY           =  $6
  RECT%X0           =  $7
  RECT%Y0           =  $8
/
&DEPTH_NML
  DEPTH%SF        =  -2500.
  DEPTH%FILENAME  =  '../input/$9'
  DEPTH%IDLA      =  1
/
EOF
}
sphgrid sph NONE 24 18 0.5 0.5 10. 40. prep_sph.depth
sphgrid glob SMPL 24 7 15. 15. 0. -45. prep_glob.depth
# Near the north pole (lat 78 to 89), and across the zero meridian written as 355 to 374 degrees.
sphgrid pol NONE 10 12 1. 1. 0. 78. prep_pol.depth
sphgrid wrap NONE 20 12 1. 1. 355. 40. prep_wrap.depth
# Sea is 1, land 0 (DEPTH%SF turns 1 into -2500 m); a land block of 4 by 4 points inland.
mask 24 18 10 13 7 10 > "$v/prep_sph.depth"
mask 24 7 1 0 1 0 > "$v/prep_glob.depth"
mask 10 12 1 0 1 0 > "$v/prep_pol.depth"
mask 20 12 1 0 1 0 > "$v/prep_wrap.depth"

# ---- the cases -------------------------------------------------------------------------------------------
hd() { printf '$ ww3_prep input made by scripts/prep_interp.sh\n  '"'%s' '%s' %s %s"'\n' "$1" "$2" "$3" "$4"; }
t1='  19680606 000000'
t2='  19680606 060000'
# unit FROM IDLA IDFM FORMT FORMF NDS NAME: the data file lines.
unit() { printf "  '%s' %s %s '%s' '%s'\n  %s '%s'\n" "$1" "$2" "$3" "${4:-(I8,1X,I6)}" "${5:-(..F..)}" "$6" "$7"; }
inl() { unit UNIT "${1:-1}" "${2:-1}" "${3:-(I8,1X,I6)}" "${4:-(..F..)}" 10 dummy; }
# gridfile: the two info lines of a lat-lon or mask file: FROM IDLA IDFM FORM, NDS NAME.
gf() { printf "  '%s' %s %s '%s'\n  %s '%s'\n" "$1" "$2" "$3" "$4" "$5" "$6"; }

# The input grids. LL: lon 9.6 .. 22.0 in 17 columns, lat 39.7 .. 48.9 in 14 rows, around the model's
# 10 .. 21.5 and 40 .. 48.5 (sph). F1: 18 by 16 points, sheared, a little wider on the east and north.
LLG='  9.6 22.0 17 39.7 48.9 14'
f1lon() { fld lin 18 16 0 9.5 0.7 0.05; }
f1lat() { fld lin 18 16 0 39.5 0.04 0.6; }
f1lon_f() { ffld lin 18 16 '%10.4f' 9.5 0.7 0.05; }
f1lat_f() { ffld lin 18 16 '%10.4f' 39.5 0.04 0.6; }
# F2: a second grid, 12 by 10, from 9.0 to 20.9 and 39.0 to 48.0.
f2lon() { fld lin 12 10 0 9.0 1.0 0.0; }
f2lat() { fld lin 12 10 0 39.0 0.0 1.0; }

gen() {
  # ---- LL on sph
  for c in "wnd WND" "cur CUR" "tau TAU"; do
    read -r name id <<< "$c"
    { hd "$id" LL F T; echo "$t1"; echo "$LLG"; inl; fld u 17 14; fld v 17 14; } > "$v/ww3_prep_ll_$name.inp"
  done
  { hd WNS LL F T; echo "$t1"; echo "$LLG"; inl; fld u 17 14; fld v 17 14; fld t 17 14; } > "$v/ww3_prep_ll_wns.inp"
  { hd RHO LL F T; echo "$t1"; echo "$LLG"; inl; fld u 17 14; fld v 17 14; } > "$v/ww3_prep_ll_rho.inp"
  { hd ISI LL F T; echo "$t1"; echo "$LLG"; inl; fld u 17 14; fld v 17 14; } > "$v/ww3_prep_ll_isi.inp"
  for c in "ice ICE" "lev LEV" "mdn MDN" "ic2 IC2"; do
    read -r name id <<< "$c"
    { hd "$id" LL F T; echo "$t1"; echo "$LLG"; inl; fld ice 17 14; } > "$v/ww3_prep_ll_$name.inp"
  done
  # An input grid inside the model: the points beyond it are outside, with a factor of 0.
  { hd WND LL F T; echo "$t1"; echo "  12.0 18.0 9 42.0 46.0 9"; inl; fld u 9 9; fld v 9 9; } > "$v/ww3_prep_ll_outside.inp"
  # Opposite signs in neighbouring points: the interpolated vector is shorter than the mean length, and the
  # velocity-conserving factor reaches its limit of 1.5.
  { hd WND LL F T; echo "$t1"; echo "$LLG"; inl; fld chk 17 14; fld chk 17 14; } > "$v/ww3_prep_ll_chk.inp"
  { hd CUR LL F T; echo "$t1"; echo "$LLG"; inl; fld chk 17 14; fld v 17 14; } > "$v/ww3_prep_ll_chkc.inp"
  { hd WND LL T T; echo "$LLG"; inl; echo "$t1"; fld u 17 14; fld v 17 14; echo; echo "$t2"; fld v 17 14; fld u 17 14; echo '  19680607 000000'; fld t 17 14; fld u 17 14; } > "$v/ww3_prep_ll_times.inp"
  for la in 2 3 4; do
    per=10; [ "$la" = 3 ] && per=0
    { hd WND LL F T; echo "$t1"; echo "$LLG"; inl "$la"; fld u 17 14 $per; fld v 17 14 $per; } > "$v/ww3_prep_ll_idla$la.inp"
  done
  { hd WND LL F T; echo "$t1"; echo "$LLG"; unit UNIT 1 2 '(I8,1X,I6)' '(17F8.3)' 10 dummy; ffld u 17 14 '%8.3f'; ffld v 17 14 '%8.3f'; } > "$v/ww3_prep_ll_fmt.inp"
  # Format reversal: 17 values to a row from a format of 10 and 7.
  { hd WND LL F T; echo "$t1"; echo "$LLG"; unit UNIT 1 2 '(I8,1X,I6)' '(17F8.3,/)' 10 dummy; ffld u 17 14 '%8.3f' | awk '{ print; print "" }'; ffld v 17 14 '%8.3f' | awk '{ print; print "" }'; } > "$v/ww3_prep_r_slash.inp"
  { hd WND LL F T; echo "$t1"; echo "$LLG"; unit UNIT 1 2 '(I8,1X,I6)' '(10F8.3)' 10 dummy; recrows u 17 14 10 '%8.3f'; recrows v 17 14 10 '%8.3f'; } > "$v/ww3_prep_ll_rev.inp"
  # IDLA 2 with the same format: one READ for the array, a new record after every 10 values.
  { hd WND LL F T; echo "$t1"; echo "$LLG"; unit UNIT 2 2 '(I8,1X,I6)' '(10F8.3)' 10 dummy
    fld u 17 14 | tr -s ' \n' '\n' | grep -v '^$' | awk '{ printf "%8.3f", $1; if (NR % 10 == 0) printf "\n" } END { if (NR % 10) printf "\n" }'
    fld v 17 14 | tr -s ' \n' '\n' | grep -v '^$' | awk '{ printf "%8.3f", $1; if (NR % 10 == 0) printf "\n" } END { if (NR % 10) printf "\n" }'; } > "$v/ww3_prep_ll_rev2.inp"
  # Binary data in a file, one record for the array, with a time record before each.
  { timerec 19680606 0; fld u 17 14 | hexrec f; fld v 17 14 | hexrec f; timerec 19680606 60000; fld v 17 14 | hexrec f; fld u 17 14 | hexrec f; } > "$v/ll_wnd.bin"
  { hd WND LL T T; echo "$LLG"; unit NAME 2 3 '(I8,1X,I6)' '(..F..)' 21 '../input/ll_wnd.bin'; } > "$v/ww3_prep_ll_bin.inp"
  # ---- LL on glob and cart
  { hd WND LL F T; echo "$t1"; echo "  0.0 342.0 20 -60.0 60.0 9"; inl; fld u 20 9; fld v 20 9; } > "$v/ww3_prep_llg_wnd.inp"
  { hd ICE LL F T; echo "$t1"; echo "  0.0 342.0 20 -60.0 60.0 9"; inl; fld ice 20 9; } > "$v/ww3_prep_llg_ice.inp"
  # Not closed: the same grid with 19 columns of 18 degrees from 0 to 324 leaves 324..360 outside.
  { hd ICE LL F T; echo "$t1"; echo "  0.0 324.0 19 -60.0 60.0 9"; inl; fld ice 19 9; } > "$v/ww3_prep_llg_open.inp"
  { hd WND LL F T; echo "$t1"; echo "  -600.E3 600.E3 25 -600.E3 600.E3 25"; inl; fld u 25 25; fld v 25 25; } > "$v/ww3_prep_llc_wnd.inp"
  { hd CUR LL F T; echo "$t1"; echo "  -400.E3 400.E3 17 -400.E3 400.E3 17"; inl; fld u 17 17; fld chk 17 17; } > "$v/ww3_prep_llc_cur.inp"

  # Near the pole the cell centre is within 10 degrees of it and W3GRMP maps through W3SPLX; the second grid has
  # vertices at 90 degrees, which makes a cell a pole cell.
  { hd WND LL F T; echo "$t1"; echo "  -1.0 12.0 14 77.0 89.0 13"; inl; fld u 14 13; fld v 14 13; } > "$v/ww3_prep_llp_wnd.inp"
  { hd WND LL F T; echo "$t1"; echo "  -1.0 12.0 14 80.0 90.0 11"; inl; fld u 14 11; fld v 14 11; } > "$v/ww3_prep_llp_pole.inp"
  { hd ICE F1 F T; echo "$t1"; echo "  14 13 F"; gf UNIT 1 1 '(..F..)' 10 dummy; fld lin 14 13 0 77.0 0.0 1.0; fld lin 14 13 0 -1.0 1.0 0.0; gf UNIT 1 1 '(..F..)' 10 dummy; mask 14 13 5 8 6 9; inl; fld ice 14 13; } > "$v/ww3_prep_f1p_ice.inp"
  # Longitudes of the model beyond 360, the input from -10 to 20.
  { hd WND LL F T; echo "$t1"; echo "  -10.0 20.0 16 38.0 54.0 9"; inl; fld u 16 9; fld v 16 9; } > "$v/ww3_prep_llw_wnd.inp"
  # IDLA and IDFM out of range are 1.
  { hd WND LL F T; echo "$t1"; echo "$LLG"; unit UNIT 7 9 '(I8,1X,I6)' '(..F..)' 10 dummy; fld u 17 14; fld v 17 14; } > "$v/ww3_prep_ll_clamp.inp"
  { hd WNS F1 F T; echo "$t1"; echo "  18 16 F"; gf UNIT 9 9 '(..F..)' 10 dummy; f1lat; f1lon; gf UNIT 7 7 '(..F..)' 10 dummy; mask 18 16 8 11 6 9; inl; fld u 18 16; fld v 18 16; fld t 18 16; } > "$v/ww3_prep_f1_wns.inp"
  # ---- F1 on sph: lat-lon and mask in the file
  { hd ICE F1 F T; echo "$t1"; echo "  18 16 F"; gf UNIT 1 1 '(..F..)' 10 dummy; f1lat; f1lon; gf UNIT 1 1 '(..F..)' 10 dummy; mask 18 16 8 11 6 9; inl; fld ice 18 16; } > "$v/ww3_prep_f1_ice.inp"
  { hd WND F1 F T; echo "$t1"; echo "  18 16 F"; gf UNIT 1 1 '(..F..)' 10 dummy; f1lat; f1lon; gf UNIT 1 1 '(..F..)' 10 dummy; mask 18 16 8 11 6 9; inl; fld u 18 16; fld v 18 16; } > "$v/ww3_prep_f1_wnd.inp"
  { hd CUR F1 F T; echo "$t1"; echo "  18 16 F"; gf UNIT 1 1 '(..F..)' 10 dummy; f1lat; f1lon; gf UNIT 1 1 '(..F..)' 10 dummy; mask 18 16 8 11 6 9; inl; fld chk 18 16; fld v 18 16; } > "$v/ww3_prep_f1_chk.inp"
  # A block of land wider than the search neighbourhood: fully masked cells with no sea point near.
  { hd ICE F1 F T; echo "$t1"; echo "  18 16 F"; gf UNIT 1 1 '(..F..)' 10 dummy; f1lat; f1lon; gf UNIT 1 1 '(..F..)' 10 dummy; mask 18 16 5 14 4 13; inl; fld ice 18 16; } > "$v/ww3_prep_f1_bigland.inp"
  # All land.
  { hd ICE F1 F T; echo "$t1"; echo "  18 16 F"; gf UNIT 1 1 '(..F..)' 10 dummy; f1lat; f1lon; gf UNIT 1 1 '(..F..)' 10 dummy; mask 18 16 1 18 1 16; inl; fld ice 18 16; } > "$v/ww3_prep_f1_allland.inp"
  # Files by name, text.
  { f1lat; f1lon; } > "$v/f1_latlon.txt"
  mask 18 16 8 11 6 9 > "$v/f1_mask.txt"
  fld ice 18 16 > "$v/f1_ice.txt"
  { hd ICE F1 F T; echo "$t1"; echo "  18 16 F"; gf NAME 1 1 '(..F..)' 21 '../input/f1_latlon.txt'; gf NAME 1 1 '(..F..)' 22 '../input/f1_mask.txt'; unit NAME 1 1 '(I8,1X,I6)' '(..F..)' 23 '../input/f1_ice.txt'; } > "$v/ww3_prep_f1_names.inp"
  # Files by unit number: fort.21, fort.22, fort.23.
  cp "$v/f1_latlon.txt" "$v/fort.21"; cp "$v/f1_mask.txt" "$v/fort.22"; cp "$v/f1_ice.txt" "$v/fort.23"
  { hd ICE F1 F T; echo "$t1"; echo "  18 16 F"; gf UNIT 1 1 '(..F..)' 21 dummy; gf UNIT 1 1 '(..F..)' 22 dummy; unit UNIT 1 1 '(I8,1X,I6)' '(..F..)' 23 dummy; } > "$v/ww3_prep_f1_units.inp"
  # Formatted lat-lon, mask and data, IDLA 3 for the data.
  { f1lat_f; f1lon_f; } > "$v/f1_latlon.fmt"
  fmask 18 16 8 11 6 9 > "$v/f1_mask.fmt"
  ffld ice 18 16 '%8.3f' > "$v/f1_ice.fmt"
  { hd ICE F1 F T; echo "$t1"; echo "  18 16 F"; gf NAME 1 2 '(18F10.4)' 21 '../input/f1_latlon.fmt'; gf NAME 1 2 '(18I2)' 22 '../input/f1_mask.fmt'; unit NAME 1 2 '(I8,1X,I6)' '(18F8.3)' 23 '../input/f1_ice.fmt'; } > "$v/ww3_prep_f1_fmt.inp"
  # Binary lat-lon, mask and data, one record to a row (IDLA 1) and one for the array (IDLA 2).
  { f1lat | hexrec f 18; f1lon | hexrec f 18; } > "$v/f1_latlon.bin"
  mask 18 16 8 11 6 9 | hexrec i 18 > "$v/f1_mask.bin"
  fld ice 18 16 | hexrec f > "$v/f1_ice.bin"
  { hd ICE F1 F T; echo "$t1"; echo "  18 16 F"; gf NAME 1 3 '(..F..)' 21 '../input/f1_latlon.bin'; gf NAME 1 3 '(..F..)' 22 '../input/f1_mask.bin'; unit NAME 2 3 '(I8,1X,I6)' '(..F..)' 23 '../input/f1_ice.bin'; } > "$v/ww3_prep_f1_bin.inp"
  # IDLA 2, 3 and 4 for the lat-lon and mask files (one READ for the array), data with IDLA 4.
  { f1lat; f1lon; } | tr -s ' \n' '\n' | grep -v '^$' | awk '{ printf "%s%s", ($1 ~ /^-/ ? "" : " "), $1; if (NR % 9 == 0) printf "\n" } END { if (NR % 9) printf "\n" }' > "$v/f1_latlon.l2"
  { hd ICE F1 F T; echo "$t1"; echo "  18 16 F"; gf NAME 2 1 '(..F..)' 21 '../input/f1_latlon.l2'; gf UNIT 3 1 '(..F..)' 10 dummy; mask 18 16 8 11 6 9; inl 4 1; fld ice 18 16 18; } > "$v/ww3_prep_f1_idla.inp"
  # ---- F1 closed, on glob; and on the Cartesian grid
  { hd ICE F1 F T; echo "$t1"; echo "  20 9 T"; gf UNIT 1 1 '(..F..)' 10 dummy; fld lin 20 9 0 -60 0 15; fld lin 20 9 0 0 18 0; gf UNIT 1 1 '(..F..)' 10 dummy; mask 20 9 4 6 2 3; inl; fld ice 20 9; } > "$v/ww3_prep_f1g_ice.inp"
  { hd ICE F1 F T; echo "$t1"; echo "  20 9 F"; gf UNIT 1 1 '(..F..)' 10 dummy; fld lin 20 9 0 -60 0 15; fld lin 20 9 0 0 18 0; gf UNIT 1 1 '(..F..)' 10 dummy; mask 20 9 4 6 2 3; inl; fld ice 20 9; } > "$v/ww3_prep_f1g_open.inp"
  { hd ICE F1 F T; echo "$t1"; echo "  25 25 F"; gf UNIT 1 1 '(..F..)' 10 dummy; fld lin 25 25 0 -600000 0 50000; fld lin 25 25 0 -600000 50000 0; gf UNIT 1 1 '(..F..)' 10 dummy; mask 25 25 10 14 10 14; inl; fld ice 25 25; } > "$v/ww3_prep_f1c_ice.inp"
  { hd ICE F1 F T; echo "$t1"; echo "  25 25 F"; gf UNIT 1 1 '(..F..)' 10 dummy; fld lin 25 25 0 -600000 0 50000; fld lin 25 25 0 -600000 50000 0; gf UNIT 1 1 '(..F..)' 10 dummy; mask 25 25 3 22 3 22; inl; fld ice 25 25; } > "$v/ww3_prep_f1c_bigland.inp"

  # ---- F2: two input grids, averaged where both cover a point
  f2case() { # f2case FIELD: two grids, the data follow in the file
    { hd "$1" F2 F T; echo "$t1"
      echo "  18 16 F"; gf UNIT 1 1 '(..F..)' 10 dummy; f1lat; f1lon; gf UNIT 1 1 '(..F..)' 10 dummy; mask 18 16 8 11 6 9
      echo "  12 10 F"; gf UNIT 1 1 '(..F..)' 10 dummy; f2lat; f2lon; gf UNIT 1 1 '(..F..)' 10 dummy; mask 12 10 3 4 3 4
      inl; inl; }
  }
  { f2case ICE; fld ice 18 16; fld lev 12 10; } > "$v/ww3_prep_f2_ice.inp"
  { f2case LEV; fld lev 18 16; fld ice 12 10; } > "$v/ww3_prep_f2_lev.inp"
  { f2case ISI; fld u 18 16; fld lev 12 10; fld v 18 16; } > "$v/ww3_prep_f2_isi.inp"
  # Each grid's data in a file of its own, with times.
  { echo "  19680606 000000"; fld ice 18 16; echo "  19680606 060000"; fld lev 18 16; } > "$v/f2_a.txt"
  { echo "  19680606 000000"; fld lev 12 10; echo "  19680606 060000"; fld ice 12 10; } > "$v/f2_b.txt"
  { hd ICE F2 T T
    echo "  18 16 F"; gf UNIT 1 1 '(..F..)' 10 dummy; f1lat; f1lon; gf UNIT 1 1 '(..F..)' 10 dummy; mask 18 16 8 11 6 9
    echo "  12 10 F"; gf UNIT 1 1 '(..F..)' 10 dummy; f2lat; f2lon; gf UNIT 1 1 '(..F..)' 10 dummy; mask 12 10 3 4 3 4
    unit NAME 1 1 '(I8,1X,I6)' '(..F..)' 21 '../input/f2_a.txt'; unit NAME 1 1 '(I8,1X,I6)' '(..F..)' 22 '../input/f2_b.txt'; } > "$v/ww3_prep_f2_times.inp"
  # The two files' times differ: the Fortran stops with exit 51.
  { echo "  19680606 000000"; fld ice 12 10; echo "  19680606 120000"; fld lev 12 10; } > "$v/f2_c.txt"
  sed 's#f2_b.txt#f2_c.txt#' "$v/ww3_prep_f2_times.inp" > "$v/ww3_prep_e_f2time.inp"


  # ---- binary input to the AI type, ww3_ts3's grid (43 by 43)
  { timerec 19680606 0; fld u 43 43 | hexrec f; fld v 43 43 | hexrec f; timerec 19680606 60000; fld v 43 43 | hexrec f; fld u 43 43 | hexrec f; } > "$v/ai_idla2.bin"
  { hd WND AI T T; unit NAME 2 3 '(I8,1X,I6)' '(..F..)' 21 '../input/ai_idla2.bin'; } > "$v/ww3_prep_ai_bin2.inp"
  { fld u 43 43 | hexrec f 43; fld v 43 43 | hexrec f 43; } > "$v/ai_idla1.bin"
  { hd WND AI F T; echo "$t1"; unit NAME 1 3 '(I8,1X,I6)' '(..F..)' 21 '../input/ai_idla1.bin'; } > "$v/ww3_prep_ai_bin1.inp"
  { hd WND AI F T; echo "$t1"; unit NAME 3 3 '(I8,1X,I6)' '(..F..)' 21 '../input/ai_idla1.bin'; } > "$v/ww3_prep_ai_bin3.inp"
  { fld u 43 43 | hexrec f; fld v 43 43 | hexrec f; } > "$v/ai_nohdr.bin"
  { hd WND AI F T; echo "$t1"; unit NAME 4 3 '(I8,1X,I6)' '(..F..)' 21 '../input/ai_nohdr.bin'; } > "$v/ww3_prep_ai_bin4.inp"
  { hd WND AI F T; echo "$t1"; unit NAME 2 3 '(I8,1X,I6)' '(..F..)' 21 '../input/ai_nohdr.bin'; } > "$v/ww3_prep_ai_bin2f.inp"
  # A WAVEWATCH III file read back: its header is checked and passes, then records follow.
  { hdrrec WND 43 43 1; timerec 19680606 0; fld u 43 43 | hexrec f; fld v 43 43 | hexrec f; } > "$v/ai_ww3.bin"
  { hd WND AI T T; unit NAME 2 3 '(I8,1X,I6)' '(..F..)' 21 '../input/ai_ww3.bin'; } > "$v/ww3_prep_ai_ww3hdr.inp"
  { hd WND AI T T; unit NAME 2 3 '(I8,1X,I6)' '(..F..)' 21 '../input/ll_wnd.bin'; } > "$v/ww3_prep_e_shortrec.inp"
  # Errors the Fortran stops on: the header of another field, and of another size.
  { hdrrec CUR 43 43 1; timerec 19680606 0; fld u 43 43 | hexrec f; fld v 43 43 | hexrec f; } > "$v/ai_ww3_cur.bin"
  { hd WND AI T T; unit NAME 2 3 '(I8,1X,I6)' '(..F..)' 21 '../input/ai_ww3_cur.bin'; } > "$v/ww3_prep_e_hdrfield.inp"
  { hdrrec WND 42 43 1; timerec 19680606 0; fld u 43 43 | hexrec f; fld v 43 43 | hexrec f; } > "$v/ai_ww3_42.bin"
  { hd WND AI T T; unit NAME 2 3 '(I8,1X,I6)' '(..F..)' 21 '../input/ai_ww3_42.bin'; } > "$v/ww3_prep_e_hdrsize.inp"
  : > "$v/empty.bin"
  # A time record of 4 bytes where 8 are read.
  { echo 19680606 | hexrec i; fld u 43 43 | hexrec f; } > "$v/short_time.bin"
  { hd WND AI T T; unit NAME 2 3 '(I8,1X,I6)' '(..F..)' 21 '../input/short_time.bin'; } > "$v/ww3_prep_e_shorttime.inp"
  { hd WND AI T T; unit NAME 2 3 '(I8,1X,I6)' '(..F..)' 21 '../input/empty.bin'; } > "$v/ww3_prep_ai_empty.inp"
  # Format reversal on AI input: 43 values to a row from a format of 10 per record.
  { hd WND AI F T; echo "$t1"; unit UNIT 1 2 '(I8,1X,I6)' '(10F8.3)' 10 dummy; recrows u 43 43 10 '%8.3f'; recrows v 43 43 10 '%8.3f'; } > "$v/ww3_prep_ai_rev.inp"
  # A group in the format: reversal restarts at the group, so the leading blank is in the first record only.
  { hd WND AI F T; echo "$t1"; unit UNIT 2 2 '(I8,1X,I6)' '(1X,2(F8.3))' 10 dummy
    for f in u v; do fld $f 43 43 | tr -s ' \n' '\n' | grep -v '^$' | awk '{ if (NR % 2 == 1) printf "%s%8.3f", (NR == 1 ? " " : ""), $1 * 100 + 2000; else printf "%8.3f\n", $1 * 100 + 2000 } END { if (NR % 2) printf "\n" }'; done; } > "$v/ww3_prep_ai_revgrp.inp"

  # ---- DAT
  dat() { # dat DATTYP RECLDT FLTIME IDLA IDFM FORMT: the lines before the data
    hd DAT AI "$3" T; [ "$3" = F ] && echo "$t1"
    echo "  $1 $2 -999."
    unit UNIT "$4" "$5" "$6" '(..F..)' 10 dummy
  }
  { dat 0 3 F 1 1 '(I8,1X,I6)'; echo "  2"; echo "  1.5 2.5 3.5"; echo "  4.5 5.5 6.5"; } > "$v/ww3_prep_dat_0.inp"
  { dat 1 4 F 1 1 '(I8,1X,I6)'; echo "  3"; echo "  1.5 2.5 3.5 4.5"; echo "  5.5 6.5"; echo "  7.5 8.5 9.5"; echo "  10.5 11.5 12.5 13.5"; } > "$v/ww3_prep_dat_1.inp"
  { dat 2 2 F 1 1 '(I8,1X,I6)'; echo "  1"; echo "  1.0 2.0"; } > "$v/ww3_prep_dat_2.inp"
  { dat 0 3 T 1 1 '(I8,1X,I6)'; echo "$t1"; echo "  2"; echo "  1.5 2.5 3.5"; echo "  4.5 5.5 6.5"; echo "$t2"; echo "  0"; echo '  19680607 000000'; echo "  1"; echo "  7.5 8.5 9.5"; } > "$v/ww3_prep_dat_times.inp"
  { dat 0 3 F 1 1 '(I8,1X,I6)'; echo "  0"; } > "$v/ww3_prep_dat_ndat0.inp"
  # IDFM 2: the data are read with the time format, which the Fortran does; NDAT is read list-directed.
  { dat 0 3 F 1 2 '(3F8.3)'; echo "  2"; echo "   1.500   2.500   3.500"; echo "   4.500   5.500   6.500"; } > "$v/ww3_prep_dat_fmt.inp"
  # Binary: time record, NDAT record, one record per datum.
  { timerec 19680606 0; echo 2 | hexrec i; echo "1.5 2.5 3.5" | hexrec f; echo "4.5 5.5 6.5" | hexrec f; timerec 19680606 60000; echo 1 | hexrec i; echo "7.5 8.5 9.5" | hexrec f; } > "$v/dat.bin"
  { hd DAT AI T T; echo "  0 3 -999."; unit NAME 1 3 '(I8,1X,I6)' '(..F..)' 21 '../input/dat.bin'; } > "$v/ww3_prep_dat_bin.inp"
  # Real data read with the integer time format: the Fortran stops on the data record.
  { dat 0 3 T 1 2 '(I8,1X,I6)'; echo '19680606 000000'; echo "  1"; echo "1.5 2.5 3.5"; } > "$v/ww3_prep_e_dattype.inp"

  # ---- the Fortran stops (exit status and partial output are compared), and refusals
  { hd WND LL F T; echo "$t1"; echo "$LLG"; inl; fld u 17 14 | sed '5s/ [0-9.-]*$/ abc/'; fld v 17 14; } > "$v/ww3_prep_e_badvalue.inp"
  { hd WND LL F T; echo "$t1"; echo "$LLG"; inl; fld u 17 14; fld v 17 5; } > "$v/ww3_prep_e_eof.inp"
  { hd XXX LL F T; echo "$t1"; echo "$LLG"; inl; fld u 17 14; } > "$v/ww3_prep_e_field.inp"
  { hd WND XX F T; echo "$t1"; echo "$LLG"; inl; fld u 17 14; } > "$v/ww3_prep_e_type.inp"
  { hd WND F2 F T; echo "$t1"; inl; } > "$v/ww3_prep_e_f2wnd.inp"
  { hd WND LL F T; echo '  1000 0'; echo "$LLG"; inl; fld u 17 14; } > "$v/ww3_prep_e_time.inp"
  { hd WND LL F T; echo "$t1"; echo "  9.6 22.0 1 39.7 48.9 14"; inl; fld u 17 14; } > "$v/ww3_prep_e_nxi.inp"
  { hd ICE F1 F T; echo "$t1"; echo "  18 1 F"; } > "$v/ww3_prep_e_nyj.inp"
  { hd DAT AI F T; echo "$t1"; echo "  3 3 -999."; inl; } > "$v/ww3_prep_e_dattyp.inp"
  { hd DAT AI F T; echo "$t1"; echo "  0 0 -999."; inl; } > "$v/ww3_prep_e_recldt.inp"
  { hd WND LL F T; echo "$t1"; echo "$LLG"; unit NAME 1 1 '(I8,1X,I6)' '(..F..)' 21 '../input/no_such_file.txt'; } > "$v/ww3_prep_e_open.inp"
  { hd ICE F1 F T; echo "$t1"; echo "  18 16 F"; gf NAME 1 1 '(..F..)' 21 '../input/no_such_file.txt'; } > "$v/ww3_prep_e_openll.inp"
  { hd ICE F1 F T; echo "$t1"; echo "  18 16 F"; gf NAME 1 1 '(..F..)' 21 '../input/f1_latlon.txt'; gf NAME 1 1 '(..F..)' 22 '../input/no_such_file.txt'; } > "$v/ww3_prep_e_openmask.inp"
  { hd WND AI F T; echo "$t1"; unit UNIT 1 3 '(I8,1X,I6)' '(..F..)' 10 dummy; } > "$v/ww3_prep_e_binline.inp"
  { hd DAT AI F T; echo "$t1"; echo "  0 3 -999."; inl; echo "  2"; echo "  1.5 2.5 3.5"; } > "$v/ww3_prep_e_dateof.inp"
  { hd DAT AI F T; echo "$t1"; echo "  0 3 -999."; inl; echo "  x"; } > "$v/ww3_prep_e_datndat.inp"
  { hd WND AI T T; unit NAME 1 1 '(I8,1X,I6)' '(..F..)' 21 '../input/bad_time.txt'; } > "$v/ww3_prep_e_badtime.inp"
  echo "  1968x606 0" > "$v/bad_time.txt"
  # A real read with an integer descriptor, and an integer with a real one.
  { hd WND LL F T; echo "$t1"; echo "$LLG"; unit UNIT 1 2 '(I8,1X,I6)' '(17I8)' 10 dummy; ffld u 17 14 '%8.3f'; ffld v 17 14 '%8.3f'; } > "$v/ww3_prep_e_mismatch.inp"
  { hd ICE F1 F T; echo "$t1"; echo "  18 16 F"; gf NAME 1 1 '(..F..)' 21 '../input/f1_latlon.txt'; gf NAME 1 2 '(18F2.0)' 22 '../input/f1_mask.fmt'; unit NAME 1 1 '(I8,1X,I6)' '(..F..)' 23 '../input/f1_ice.txt'; } > "$v/ww3_prep_e_mismatchi.inp"
  # Refused: a null value, a slash and a descriptor that is not I, F, E, D or X.
  { hd WND LL F T; echo "$t1"; echo "$LLG"; inl; fld u 17 14 | sed '3s/^  \([^ ]*\) [^ ]* /  \1 , , /'; fld v 17 14; } > "$v/ww3_prep_r_null.inp"
  { hd WND LL F T; echo "$t1"; echo "$LLG"; unit UNIT 1 2 '(I8,1X,I6)' '(TR1,17F8.3)' 10 dummy; ffld u 17 14 '%8.3f' | sed 's/^/ /'; ffld v 17 14 '%8.3f' | sed 's/^/ /'; } > "$v/ww3_prep_r_edit.inp"
}

# ---- the runs ----------------------------------------------------------------------------------------------
run_one() { # run_one KIND MODEL INPUT-NAME: KIND ok, err or refuse
  local kind="$1" model="$2" name="$3" base suffix rc=0
  case "$model" in
    sph) base=ww3_tp2.2; suffix=sph ;;
    glob) base=ww3_tp2.2; suffix=glob ;;
    pol) base=ww3_tp2.2; suffix=pol ;;
    wrap) base=ww3_tp2.2; suffix=wrap ;;
    cart) base=ww3_ts3; suffix="" ;;
  esac
  [[ "$name" =~ $pat ]] || return 0
  case "$kind" in
    ok) PREP_OVERLAY="$v" bash scripts/prep_pair.sh "$base" "ww3_prep_$name.inp" $suffix || rc=$? ;;
    err) PREP_ERROR=1 PREP_OVERLAY="$v" bash scripts/prep_pair.sh "$base" "ww3_prep_$name.inp" $suffix || rc=$? ;;
    refuse) PREP_OVERLAY="$v" bash scripts/prep_pair.sh "$base" "ww3_prep_$name.inp" $suffix || rc=$?
      if [ "$rc" = 3 ]; then rc=0; else echo "$name: expected a refusal (status 3), got $rc"; rc=1; fi ;;
  esac
  [ "$rc" = 0 ] || bad=1
}

run() {
  bad=0
  for n in wnd cur tau wns rho isi ice lev mdn ic2 outside chk chkc times idla2 idla3 idla4 fmt rev rev2 bin clamp; do run_one ok sph "ll_$n"; done
  for n in wnd ice open; do run_one ok glob "llg_$n"; done
  for n in wnd cur; do run_one ok cart "llc_$n"; done
  for n in wnd pole; do run_one ok pol "llp_$n"; done
  run_one ok pol f1p_ice
  run_one ok wrap llw_wnd
  for n in ice wnd wns chk bigland allland names units fmt bin idla; do run_one ok sph "f1_$n"; done
  for n in ice open; do run_one ok glob "f1g_$n"; done
  for n in ice bigland; do run_one ok cart "f1c_$n"; done
  for n in ice lev isi times; do run_one ok sph "f2_$n"; done
  for n in bin2 bin1 bin3 bin4 bin2f ww3hdr empty rev revgrp; do run_one ok cart "ai_$n"; done
  for n in 0 1 2 times ndat0 fmt bin; do run_one ok cart "dat_$n"; done
  for n in badvalue eof field type f2wnd time nxi nyj dattyp recldt open openll openmask binline dateof datndat badtime hdrfield hdrsize dattype shortrec mismatch mismatchi; do run_one err sph "e_$n"; done
  run_one err cart e_shorttime
  run_one err sph e_f2time
  for n in null edit slash; do run_one refuse sph "r_$n"; done
  exit "$bad"
}

case "$mode" in
  gen) gen ;;
  run) run ;;
  all) gen; run ;;
  *) echo "usage: prep_interp.sh [gen|run|all] [pattern]" >&2; exit 64 ;;
esac
