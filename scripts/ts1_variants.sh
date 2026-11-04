# shellcheck shell=bash disable=SC2034,SC2154
# The caller sets $variant and reads the rest.
# Sourced by capture.sh and ts1_pair.sh. A variant changes the case's namelists only, never WW3.
sds4=''
wind=12
poke=''
limit=0
ice=''
case "$variant" in
  default) ;;
  t700) sds4='&SDS4 SDSBCHOICE=3, SDSC2=-3.8, SDSBR=0.005, SDSSTRAIN=0., SDSSTRAIN2=0., FXFM3=20., SDSFACMTF=400., SDSCUM=0., SDSC5=0. /' ;;
  filipot) sds4='&SDS4 SDSBCHOICE=2, SDSC2=0.0, SDSBCK=0.185, SDSHCK=1.5 /' ;;
  strain) sds4='&SDS4 SDSSTRAIN=2., SDSSTRAIN2=0.5, SDSBRFDF=2, SDSISO=1, SDSBM0=0., SDSBM1=0.3, SDSBM2=0.3, SDSBM3=0.2, SDSBM4=0.1 /' ;;
  circle) sds4='&SDS4 SDSDTH=180. /' ;;
  poke) poke=1; limit=500 ;;
  calm) wind=0 ;;
  storm) wind=30 ;;
  light) wind=6 ;;
  noshelter) sds4='&SIN4 TAUWSHELTER=0. /' ;;
  flatswell) sds4='&SIN4 SWELLF2=0., SWELLF3=0.02, SWELLF4=0., SWELLF6=1., SWELLF7=0. /' ;;
  sharp) sds4='&SIN4 SWELLF7=0. /' ;;
  romero2) sds4='&SDS4 SDSBCHOICE=3, SDSC2=-3.8, SDSBR=0.005, SDSSTRAIN=2., SDSSTRAIN2=0.5, FXFM3=20., SDSFACMTF=400., SDSCUM=0., SDSC5=0. /' ;;
  bt4) ;;
  bt4storm) wind=30 ;;
  bt4flat) sds4='&SBT4 SIGDEPTH=0. /' ;;
  bt4ripple) sds4='&SBT4 SIGDEPTH=0., RIPFAC3=0.05 /' ;;
  bt4coarse) sds4='&SBT4 SED_D50_UNIFORM=0.002, RIPFAC3=0.3 /' ;;
  ice) ice=0.4 ;;
  ice2) ice=0.2 ;;
  icesplit) ice=0.45; sds4='&SIS1 ISC1=0.5, ISC2=0.1 /' ;;
  iceoff) ice=0.4; sds4='&SIS1 ISC1=-1., ISC2=0.4 /' ;;
  icepoke) ice=0.4; poke=1; limit=500 ;;
  iceoffpoke) ice=0.4; sds4='&SIS1 ISC1=-1., ISC2=0.4 /'; poke=1; limit=500 ;;
  *) echo "unknown variant $variant" >&2; exit 64 ;;
esac
