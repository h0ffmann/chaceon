# Every recipe is a thin wrapper over a script or cargo. There is no CI: these are the gates.
# rustup first: Homebrew's cargo is too old for edition 2024.
export PATH := env("HOME") + "/.cargo/bin:" + env("PATH")
default:
    @just --list

# Type-checks the host workspace and refuses Rust tests
check:
    bash scripts/no_rust_tests.sh
    cd rust && cargo check --workspace

# The gates of a change to scripts
lint-sh:
    bash -n scripts/*.sh
    shellcheck scripts/*.sh

# Builds the Fortran reference, metocean/WW3 at the pin
ref-build switch="switch_chaceon_shrd":
    bash scripts/ref_build.sh {{switch}}

# Records a routine from the reference running ww3_ts1
capture routine="snl1" variant="default" depth="2500" stride="1":
    bash scripts/capture.sh {{routine}} {{variant}} {{depth}} {{stride}}

# Replays a W3SNL1 capture through the host build, every bit
snl1-cmp depth="2500":
    bash scripts/rust_in_ref.sh snl1_cmp /work/capture/snl1_default_{{depth}}m.bin

# Replays a W3SDS4 capture through the host build, every bit
sds4-cmp variant="default" depth="2500":
    bash scripts/rust_in_ref.sh sds4_cmp /work/capture/sds4_{{variant}}_{{depth}}m.bin

# Replays W3SIN4 or W3SPR4 calls and rebuilds ST4's tables, every bit
st4-cmp routine="sin4" variant="default" depth="2500":
    bash scripts/rust_in_ref.sh st4_cmp {{routine}} /work/capture/{{routine}}_{{variant}}_{{depth}}m.bin

# Replays W3SBT4 calls (switch_chaceon_bt4 reference) and rebuilds ERFTABLE, every bit
sbt4-cmp *captures:
    bash scripts/rust_in_ref.sh sbt4_cmp {{captures}}

# Replays W3SIS1 calls (switch_chaceon_ice1 reference), every bit
sis1-cmp *captures:
    bash scripts/rust_in_ref.sh sis1_cmp {{captures}}

# Replays W3SDS3 calls (switch_chaceon_st3 reference), every bit
sds3-cmp *captures:
    bash scripts/rust_in_ref.sh sds3_cmp {{captures}}

# Replays W3FLX1 calls (switch_chaceon_st1 reference), every bit
flx1-cmp *captures:
    bash scripts/rust_in_ref.sh flx1_cmp {{captures}}

# Replays W3SPR1 calls (switch_chaceon_st1 reference), every bit
spr1-cmp *captures:
    bash scripts/rust_in_ref.sh spr1_cmp {{captures}}

# Replays W3SIN1 calls (switch_chaceon_st1 reference), every bit; `--fault <name> <captures>` runs a fault that must differ
sin1-cmp *captures:
    bash scripts/rust_in_ref.sh sin1_cmp {{captures}}

# Rebuilds ST3's TAUT and TAUHFT (INSIN3) and compares them with the reference's, every bit
insin3-cmp *captures:
    bash scripts/rust_in_ref.sh insin3_cmp {{captures}}

# Replays W3SDS1 calls (switch_chaceon_st1 reference), every bit
sds1-cmp *captures:
    bash scripts/rust_in_ref.sh sds1_cmp {{captures}}

# Host verdict: the reference and Rust run ww3_ts1, and cmp judges out_pnt.ww3
ts1-pair variant="default" depth="2500":
    bash scripts/ts1_pair.sh {{variant}} {{depth}}

# Host verdict on a propagation regtest: out_pnt.ww3 and a last-step restart, by cmp
tp-pair case suffix="":
    bash scripts/tp_pair.sh {{case}} {{suffix}}

# Every propagation regtest the host driver implements
tp-all:
    bash scripts/tp_all.sh

# Winds, water levels and ice from files and namelists, against the reference, by cmp
forcing-all:
    bash scripts/forcing_all.sh

# ww3_bound and boundary input in a run, against the reference, by cmp
bound-all:
    bash scripts/bound_all.sh

# One fault at a time through the boundary code: each must differ from the reference
bound-faults:
    bash scripts/bound_faults.sh

# Track output, ww3_trck and ww3_trnc against the reference, by cmp
track-all:
    bash scripts/track_all.sh

# One fault at a time through the track code: each must differ from the reference
track-faults:
    bash scripts/track_faults.sh

# A prepared case with propagation in compiled SPIR-V on lavapipe, by cmp
spirv-pair tag:
    bash scripts/spirv_pair.sh {{tag}}

# The port's size by WW3 area, written into README.md
port-summary *args:
    python3 scripts/port_summary.py {{args}}

# Compares the mod_def reader with the reference's memory, for a case with captures
moddef-cmp mod_def tag="default_2500m":
    bash scripts/rust_in_ref.sh moddef_cmp {{mod_def}} {{tag}}

# Replays W3SRCE calls of a case, every bit; needs that case's snl1, sds4, sin4 and spr4 captures
srce-cmp tag="default_2500m":
    bash scripts/rust_in_ref.sh srce_cmp {{tag}}

# ww3-libm against the reference's glibc: every f32, and samples for two arguments
libm-bits samples="8000000000":
    bash scripts/rust_in_ref.sh libm_bits {{samples}}

# Compiles the kernels to SPIR-V
spirv:
    bash scripts/spirv.sh

# Runs the compiled SPIR-V on a capture, on CPU Vulkan
spirv-replay kernel="snl1" variant="default" depth="2500":
    bash scripts/spirv_replay.sh {{kernel}} capture/{{kernel}}_{{variant}}_{{depth}}m.bin

# ww3-libm in SPIR-V against the same source on the host, on random inputs; at most 3900000 per function
spirv-libm inputs="2000000":
    bash scripts/spirv_replay.sh libm {{inputs}}

# Compiles the kernels to PTX and assembles sm_89 and sm_90 binaries
ptx:
    bash scripts/ptx.sh

# Lines per batch and routines per status
port-status:
    bash scripts/port_status.sh

# Double-precision declarations and literals per WW3 file
f64-census:
    bash scripts/f64_census.sh

# Host verdict on ww3_strt: the reference's restart.ww3 and Rust's, by cmp
strt-pair case suffix="":
    bash scripts/strt_pair.sh {{case}} {{suffix}}

# Host verdict on ww3_prep: the reference's forcing file (current.ww3, wind.ww3, ice.ww3, ...) and Rust's, by cmp
prep-pair case input suffix="":
    bash scripts/prep_pair.sh {{case}} {{input}} {{suffix}}

# Every single-grid regtest ww3_prep input, through prep-pair
prep-all:
    bash scripts/prep_all.sh

# ww3_prep inputs no regtest has (other field ids, layouts, formats, a comma in a field, LL on the model's grid, DAT)
prep-variants:
    bash scripts/prep_variants.sh

# ww3_prep types LL, F1, F2 and DAT, binary input and format reversal on inputs of regtests/prep, the stops whose
# exit status Rust repeats and the forms Rust refuses. mode: gen, run or all; pattern selects cases by name
prep-interp mode="all" pattern=".":
    bash scripts/prep_interp.sh {{mode}} {{pattern}}

# ww3_strt start type 2 (JONSWAP): ww3_tp2.11 and ww3_tp2.5 and variants of the input
strt-variants:
    bash scripts/strt_variants.sh

# Host verdict on ww3_prnc: the reference's forcing file, log and status and Rust's, by cmp. <case> is a directory of regtests/prnc_inputs
prnc-pair case:
    bash scripts/prnc_pair.sh {{case}}

# The cases shaped like the ww3_prnc inputs of WW3's regtests (the NetCDF data are synthetic, built from CDL), through prnc-pair
prnc-all:
    bash scripts/prnc_all.sh

# ww3_prnc cases no regtest has (other fields, units, ww3_prnc.inp, errors) and the forms Rust refuses
prnc-variants:
    bash scripts/prnc_variants.sh

# Rewrites the CDL, namelist and case.conf files of regtests/prnc_inputs/ (deterministic)
prnc-gen:
    python3 scripts/prnc_cdl_gen.py

# Host verdict on ww3_grid: the reference's mod_def.ww3 and Rust's, by cmp
grid-pair case suffix="":
    bash scripts/grid_pair.sh {{case}} {{suffix}}

# Host verdict on ww3_outp: the reference's log and files and Rust's, by cmp. <input> is a path under WW3/regtests
outp-pair case input name="":
    bash scripts/outp_pair.sh {{case}} {{input}} {{name}}

# Host verdict on ww3_outf, the same way
outf-pair case input name="":
    bash scripts/outf_pair.sh {{case}} {{input}} {{name}}

# Every ww3_outp input of the regtests, and the ones written here
outp-all:
    bash scripts/outp_all.sh

# Every ww3_outf input of the regtests, and the ones written here
outf-all:
    bash scripts/outf_all.sh

# Every regtest ww3_grid is judged on
grid-all:
    bash scripts/grid_all.sh

# The mod_def.ww3 writer alone: Rust reads each work/grid/<tag>/mod_def.reference, writes it again, cmp judges
iogr-roundtrip *dirs:
    bash scripts/iogr_roundtrip.sh {{dirs}}

# Host verdict on ww3_ounf: the reference's NetCDF files and Rust's, by cmp (variant: a namelist variant of scripts/ounf_pair.sh)
ounf-pair case variant="":
    bash scripts/ounf_pair.sh {{case}} {{variant}}

# Every case in work/pair that has an out_grd.reference, through ww3_ounf
ounf-all variant="":
    bash scripts/ounf_all.sh {{variant}}

# The namelist variants of ww3_ounf on the cases that reach every field group
ounf-variants *cases:
    bash scripts/ounf_variants.sh {{cases}}

# Whether the reference's ww3_ounf writes the same bytes twice (the basis of cmp as the verdict)
ounf-selfcmp case="--all" variant="":
    bash scripts/ounf_selfcmp.sh {{case}} {{variant}}

# Host verdict on ww3_ounp: the reference's NetCDF files, nml log, stdout and status and Rust's, by cmp (variant: see scripts/ounp_pair.sh)
ounp-pair case variant="":
    bash scripts/ounp_pair.sh {{case}} {{variant}}

# Every case in work/pair (and the ts2 of ounp-src) that has a ww3_ounp namelist in its regtest input, through ww3_ounp
ounp-all variant="":
    bash scripts/ounp_all.sh {{variant}}

# The namelist variants of ww3_ounp on the cases that reach the types, flags and stops the regtests miss
ounp-variants *cases:
    bash scripts/ounp_variants.sh {{cases}}

# Whether the reference's ww3_ounp writes the same bytes twice (the basis of cmp as the verdict)
ounp-selfcmp case="--all" variant="":
    bash scripts/ounp_selfcmp.sh {{case}} {{variant}}

# The reference's mod_def.ww3 and out_pnt.ww3 for a regtest that work/pair does not hold (ww3_ts2), in work/ounp_src
ounp-src regtest="ww3_ts2" tag="ts2":
    bash scripts/ounp_src.sh {{regtest}} {{tag}}

# Builds the NetCDF images (docker/Dockerfile.netcdf)
netcdf-image:
    bash scripts/rust_netcdf.sh --image

# Runtime MPI: ww3_shel under mpirun for R of 1 to 4, byte-compared with the reference
mpi-pair *cases:
    bash scripts/mpi_pair.sh {{cases}}

# A run with no Fortran program: Rust's ww3_grid, ww3_strt, ww3_prep, ww3_shel on every case tp_all and ts1_pair prepared
rust-only:
    bash scripts/rust_only_all.sh

# Builds the Lean proofs in lean/ in Docker and prints the axioms each theorem uses
lean:
    bash scripts/lean.sh check

# Partition fields (W3PART) and boundary output files (W3IOBC WRITE), byte-compared with the reference
bnd-all:
    bash scripts/bnd_all.sh

# Rust's ww3_shel on every case in work/pair, run in copies so the shared directory is not written
bnd-regress:
    bash scripts/bnd_regress.sh
