#!/bin/sh
# Gives ww3_ts1's ww3_shel.nml homogeneous ice concentration and ice parameter 1 (IC1), so that
# W3SRCE reaches W3SIC1 (w3srcemd.F90:2079 needs INFLAGS2(4) and ICE > 0; W3SIC1 stops without
# INFLAGS2(-7)). Usage: ice1_shel.sh <ww3_shel.nml> <spec>, spec = entries "CONC:ICEP1" or
# "CONC@YYYYMMDDHHMMSS:ICEP1" joined by commas; an entry without a date starts at the run's start.
set -eu
nml="$1"
spec="$2"
n=$(echo "$spec" | awk -F, '{print NF}')
inputs=$(echo "$spec" | awk -F, '{
  k = 6
  for (i = 1; i <= NF; i++) {
    split($i, e, ":"); split(e[1], c, "@")
    d = (c[2] == "") ? "" : sprintf("  HOMOG_INPUT(%d)%%DATE = \047%s %s\047\n", k + 1, substr(c[2], 1, 8), substr(c[2], 9, 6))
    printf "  HOMOG_INPUT(%d)%%NAME = \047ICE\047\n%s  HOMOG_INPUT(%d)%%VALUE1 = %s\n", k + 1, d, k + 1, c[1]
    d2 = (c[2] == "") ? "" : sprintf("  HOMOG_INPUT(%d)%%DATE = \047%s %s\047\n", k + 2, substr(c[2], 1, 8), substr(c[2], 9, 6))
    printf "  HOMOG_INPUT(%d)%%NAME = \047IC1\047\n%s  HOMOG_INPUT(%d)%%VALUE1 = %s\n", k + 2, d2, k + 2, e[2]
    k += 2
  }
}')
printf '%s\n' "$inputs" > /tmp/ice1_inputs.txt
sed -i \
  -e "s/INPUT%FORCING%WINDS         = 'H'/&\n  INPUT%FORCING%ICE_CONC      = 'H'\n  INPUT%FORCING%ICE_PARAM1    = 'H'/" \
  -e "s/HOMOG_COUNT%N_WND                =  5/&\n  HOMOG_COUNT%N_ICE                =  $n\n  HOMOG_COUNT%N_IC1                =  $n/" \
  -e "/HOMOG_INPUT(6)%VALUE3/r /tmp/ice1_inputs.txt" "$nml"
