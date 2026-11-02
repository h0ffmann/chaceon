#!/usr/bin/env bash
# ww3_bound and boundary input in a run: every case of scripts/bound_pair.sh, ww3_tr1's inputs and the ones scripts/bound_gen.py
# writes. Exit 0 only when every file, log and status is identical, including the status of the runs that must stop.
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/bound_gen.py
i=work/bound_in
bad=0
p() { env "$@" bash scripts/bound_pair.sh "$tag" || bad=1; }
c() { tag="$1"; shift; p "$@"; }
c tr1 BP_SHEL=$i/full.sed
c nonest BP_SHEL=$i/full.sed BP_NONEST=1
c edge BP_SHEL=$i/full.sed BP_OVERLAY=$i/edge
c two BP_SHEL=$i/full.sed BP_OVERLAY=$i/two
c early BP_SHEL=$i/early.sed BP_OVERLAY=$i/edge
c late BP_SHEL=$i/late.sed BP_OVERLAY=$i/edge
c eof BP_SHEL=$i/eof.sed BP_OVERLAY=$i/edge
c threads BP_SHEL=$i/full.sed BP_OVERLAY=$i/edge BP_THREADS=4
c small BP_SHEL=$i/full.sed BP_OVERLAY=$i/edge BP_GRID=$i/gsmall.sed
c level BP_SHEL=$i/level.sed BP_OVERLAY=$i/edge
c lowf BP_SHEL=$i/full.sed BP_OVERLAY=$i/lowf
c highf BP_SHEL=$i/full.sed BP_OVERLAY=$i/highf
c fewf BP_SHEL=$i/full.sed BP_OVERLAY=$i/fewf
c rot BP_SHEL=$i/full.sed BP_OVERLAY=$i/rot
c spec BP_SHEL=$i/full.sed BP_GRID=$i/gspec.sed BP_NEST=bound/edge/ref/nest.ww3
c coarse BP_SHEL=$i/full.sed BP_GRID=$i/gcoarse.sed BP_NEST=bound/edge/ref/nest.ww3
c near BP_SHEL=$i/full.sed BP_GRID=$i/gnear.sed BP_NEST=bound/edge/ref/nest.ww3
c parent BP_SHEL=$i/parent.sed BP_OVERLAY=$i/edge BP_GRID=$i/gparent.sed
c child BP_SHEL=$i/full.sed BP_GRID=$i/gchild.sed BP_NEST=bound/parent/ref/nest01.ww3
for t in interp1 quiet read nofile noname; do c "nml_$t" BP_NORUN=1 BP_OVERLAY=$i/nml_$t; done
c inp BP_NORUN=1 BP_INP=1
for t in manyf wrongx wrongth mixed; do c "stop_$t" BP_NORUN=1 BP_OVERLAY=$i/$t; done
# The stops of W3IOBC.
python3 - "$i" <<'P'
import struct, sys
i = sys.argv[1]
b = open("work/bound/edge/ref/nest.ww3", "rb").read()
n = struct.unpack(">i", b[:4])[0]
n2 = struct.unpack(">i", b[n + 8:n + 12])[0]
open(f"{i}/nest_badid", "wb").write(b[:4] + b"X" + b[5:])
open(f"{i}/nest_badver", "wb").write(b[:36] + b"1999" + b[40:])
open(f"{i}/nest_nodata", "wb").write(b[:n + 8 + n2 + 8])
P
c fault_id BP_SHEL=$i/full.sed BP_NEST=bound_in/nest_badid
c fault_ver BP_SHEL=$i/full.sed BP_NEST=bound_in/nest_badver
c fault_nodata BP_SHEL=$i/full.sed BP_NEST=bound_in/nest_nodata
c fault_far BP_SHEL=$i/full.sed BP_GRID=$i/gfar.sed BP_NEST=bound/edge/ref/nest.ww3
c fault_inner BP_SHEL=$i/full.sed BP_GRID=$i/ginner.sed BP_NEST=bound/edge/ref/nest.ww3
exit "$bad"
