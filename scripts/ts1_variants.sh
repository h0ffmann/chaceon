# shellcheck shell=bash disable=SC2034,SC2154
# The caller sets $variant and reads the rest.
# Sourced by capture.sh and ts1_pair.sh. A variant changes the case's namelists only, never WW3.
sds4=''
wind=12
wdir=270
poke=''
limit=0
ice1=''
ice=''
grid=''
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
  winddir) wdir=45 ;;
  bt4) ;;
  bt4storm) wind=30 ;;
  bt4flat) sds4='&SBT4 SIGDEPTH=0. /' ;;
  bt4ripple) sds4='&SBT4 SIGDEPTH=0., RIPFAC3=0.05 /' ;;
  bt4coarse) sds4='&SBT4 SED_D50_UNIFORM=0.002, RIPFAC3=0.3 /' ;;
  st3) ;;
  st3storm) wind=30 ;;
  st3light) wind=6 ;;
  st3calm) wind=0 ;;
  st3swell) sds4='&SIN3 SWELLF=1. /' ;;
  st3tune) sds4='&SIN3 BETAMAX=1.52, SINTHP=1., ZALP=0.008, ALPHA0=0.0085, SWELLF=0.8 /' ;;
  st3poke) poke=1; limit=500 ;;
  st3wn) sds4='&SDS3 WNMEANP=1. /' ;;
  st3z) sds4='&SIN3 ZWND=15., ALPHA0=0.0095 /' ;;
  ice1) ice1='0.5:2.0e-5' ;;
  ice1zero) ice1='0.5:0.' ;;
  ice1dated) ice1='0.9:2.0e-5,0.9@19680606120000:0.,0.3@19680607000000:3.5e-6' ;;
  ice1poke) ice1='0.5:2.0e-5'; poke=1; limit=500 ;;
  ice1storm) wind=30; ice1='0.5:5.0e-5' ;;
  ice) ice=0.4 ;;
  ice2) ice=0.2 ;;
  icesplit) ice=0.45; sds4='&SIS1 ISC1=0.5, ISC2=0.1 /' ;;
  iceoff) ice=0.4; sds4='&SIS1 ISC1=-1., ISC2=0.4 /' ;;
  icepoke) ice=0.4; poke=1; limit=500 ;;
  iceoffpoke) ice=0.4; sds4='&SIS1 ISC1=-1., ISC2=0.4 /'; poke=1; limit=500 ;;
  st3wam4) sds4='&SDS3 SDSC1=-4.5, SDSDELTA1=0.5, SDSDELTA2=0.5 /' ;;
  st3wnp) sds4='&SDS3 WNMEANP=-0.5 /' ;;
  st3zmax) sds4='&SIN3 Z0MAX=0.002 /' ;;
  st3tab) sds4='&SIN3 ALPHA0=0.012, BETAMAX=1.4, ZALP=0.008, ZWND=8. /' ;;
  st3grid) grid='s/SPECTRUM%XFR       =  1.10/SPECTRUM%XFR       =  1.07/; s/SPECTRUM%NK        =  36/SPECTRUM%NK        =  40/' ;;
  st3grid2) grid='s/SPECTRUM%XFR       =  1.10/SPECTRUM%XFR       =  1.05/' ;;
  st2flx2) ;;
  st2flx2calm) wind=0 ;;
  st2flx2light) wind=6 ;;
  st2flx2storm) wind=30 ;;
  st2flx2gale) wind=60 ;;
  st2flx2poke) poke=1; limit=2000 ;;
  st2flx2pokecalm) wind=0; poke=1; limit=2000 ;;
  *) echo "unknown variant $variant" >&2; exit 64 ;;
esac
