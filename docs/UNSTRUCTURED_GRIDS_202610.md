# Unstructured triangular grids: how to run them, and what porting them takes

WW3's triangle-based grids are the work of Aron Roland and Fabrice Ardhuin: `model/src/w3profsmd.F90` carries both
names, and `model/src/w3triamd.F90` too. Roland's thesis adapted contour residual distribution (CRD) schemes to the
wave action equation (`manual/num/space_tri.tex` at metocean/WW3 `92e2cf66`), and
[Roland and Ardhuin (2014)](https://doi.org/10.1007/s10236-014-0711-z) set out why coastal work needs them.

**State, 2026-10-31:** the reference runs these grids. **Rust does not:** `ModDef::read` stops on `GTYPE` 3 with
"only rectilinear and curvilinear grids are implemented" (v, `just tp-pair ww3_tp2.6`). They are Batch 8 of
[the plan](RUST_GPU_PLAN_202610.md). Everything below about Rust is a plan, not a result ⚠.

## Why triangles

A triangle mesh puts nodes where gradients in the wave field are. In the authors' words, irregular grids "allow a
much faster simulation than a regular grids with high resolution everywhere", and that efficiency comes "from the
reduction in the number of nodes and thus in the time needed to compute source terms" (v, Roland and Ardhuin 2014,
§4, read 2026-10-31). On their North Sea, Channel and Biscay mesh, spatial advection with the N scheme "only accounts
for 22% of the total computation cost" (v, same place). That number is theirs, on their mesh and machine: it has
not been measured here ⚠.

There are "no constrains with respect to the triangle shape" (v, `manual/num/space_tri.tex`). The manual names
PolyMesh, SMS and OceanMesh2D for generating meshes ⚠ (none was run here).

## The method

Action is held at the nodes. For each spectral component, the advection equation is solved on the median dual cell
of each node, and the flux convergence is redistributed over the nodes (v, `manual/num/space_tri.tex`). All triangle
geometry assumes a locally flat Earth, and gradients come from linear shape functions in a triangle (v, same).

| `&UNST` switch | Scheme | Order | Notes, from the manual and the paper |
|---|---|---|---|
| `EXPFSN` | Explicit CRD-N | 1st | The default, "the most efficient but diffusive". Enough cross-diffusion to compensate the garden sprinkler effect |
| `EXPFSPSI` | Explicit CRD-PSI | Better than 1st, 2nd on structured triangles | May show a significant garden sprinkler effect |
| `EXPFSFCT` | Explicit CRD-FCT | 2nd in space and time | Same |
| `IMPFSN` | Implicit N | 1st | Splitting errors "greater than 20%" on the paper's case, against at most 5% for explicit N (v, §2.3.1) |
| `EXPTOTAL` | Block explicit N | | Needs `PDLIB`: W3INIT stops without it (v, `w3initmd.F90:639-647`) |
| `IMPTOTAL` | Fully implicit, no splitting | | Needs `PDLIB`. With `IMPREFRACTION`, `IMPFREQSHIFT`, `IMPSOURCE` and the `JGS_*` solver options |

The structured grids' averaging correction does not apply: these schemes "do not include corrections for the garden
sprinkler effect" (v, `manual/num/space_tri.tex`).

The explicit schemes keep WW3's fractional steps, with the number of sub-iterations "estimated automatically based
on the maximal CFL number in the domain" (v, same). In the code (`W3XYPFSN2`), `ITER(IK,ITH)` comes from the smallest
`SI(IP)/KKSUM(IP)` over the nodes, once per component (v, `w3profsmd.F90:644`). So the smallest cell sets the sub-step of
the whole grid.

The core of explicit N, per sub-step, is short (`w3profsmd.F90:666-670`):

```fortran
DO IE = 1, NTRI
  NI     = TRIGP(:,IE)
  UTILDE = NM(IE) * (DOT_PRODUCT(FLALL(:,IE),U(NI)))
  ST(NI) = ST(NI) + KELEM(:,IE) * (U(NI) - UTILDE)
END DO
```

## Running one on the reference

These commands were run on the reference image on 2026-10-31 (v), on `ww3_tp2.6`: Limón harbour, 1 778 nodes,
3 328 triangles and 228 boundary elements of Gmsh type 15 (v, counted from `limon_ll.msh`). The switch needs no
change for the explicit schemes: `SHRD` with `PR3 UQ` builds them, and the choice is a namelist (v, the regtest's
`info` and `ww3_grid.log`: "N Explicit (Fluctuation Splitting)").

```bash
docker run --rm --platform linux/arm64 -v "$PWD/WW3/regtests/ww3_tp2.6/input:/i:ro" \
  --entrypoint sh chaceon-ref:switch_chaceon_shrd -c '
    mkdir -p /r/work && cp -R /i /r/input && cd /r/work
    cp ../input/ww3_grid.nml ../input/ww3_strt.inp ../input/ww3_shel.nml .
    ww3_grid > grid.log && ww3_strt > strt.log && ww3_shel > shel.log
    ls -l out_grd.ww3 out_pnt.ww3'
```

Or `just tp-pair ww3_tp2.6`, which keeps the reference's files in `work/pair/ww3_tp2.6/` and then reports that Rust
stops. That run wrote `out_grd.ww3` of 6 142 562 bytes and `out_pnt.ww3` of 115 907 bytes (v).

To set up your own case, in `ww3_grid.nml`:

```fortran
&GRID_NML
  GRID%TYPE  = 'UNST'
  GRID%COORD = 'SPHE'
  GRID%CLOS  = 'NONE'
  GRID%ZLIM  = 1.0
  GRID%DMIN  = 0.10
/
&UNST_NML
  UNST%SF       = -1.
  UNST%FILENAME = './../input/limon_ll.msh'
  UNST%IDLA     = 4
  UNST%FORMAT   = '(20f10.2)'
/
```

- The mesh is a Gmsh file, here `MeshFormat` 2, with depth as the node's third coordinate (v, `model/nml/ww3_grid.nml:252-310`
  and `limon_ll.msh`). `UNST%SF = -1.` turns positive depth into WW3's negative bottom.
- Open boundaries come from type 15 elements with `UGOBCAUTO = T`, from `INBND_POINT` lines with `UGOBCAUTO = F`, as
  here, or from a file with `UGOBCFILE`, as in `ww3_tp2.7` (v, `model/nml/namelists.nml:286-290`).
- The scheme is the `&UNST` namelist in the file `GRID%NML` names. Only one may be on (v, `w3gridmd.F90:2453-2454`):

```fortran
&UNST
 UGOBCAUTO = F, UGOBCDEPTH = -10.,
 EXPFSN = T, EXPFSPSI = F, EXPFSFCT = F, IMPFSN = F, IMPTOTAL = F, EXPTOTAL = F,
/
```

`PDLIB`, the domain decomposition, needs `DIST MPI` and a partitioner: the manual names ParMetis, and the regtest's
`switch_PDLIB` and `model/bin/switch_ite_pdlib` use `SCOTCH` (v). The domain decomposed run has not been tried here ⚠:
the reference image is `SHRD`.

Regtests with this grid at the pin: `ww3_tp2.6`, `ww3_tp2.7` (needs `REF1`, not in `switch_chaceon_shrd`),
`ww3_tp1.11`, `ww3_tp2.17`, `ww3_tp2.21` (v, `grep UNST regtests/*/input/ww3_grid*.nml`). Only `ww3_tp2.6` was run.

## What porting takes

**A first target:** `W3XYPUG` with explicit N on `SHRD`, then `ww3_tp2.6`, compared by `cmp` like every other case.
Source terms need nothing new: they are per point and already bit-identical. Reading the code, these are the
changes ⚠:

| Piece | Where | Why it is not a small change |
|---|---|---|
| Grid setup | `w3triamd.F90`, 2 999 lines, and `W3IOGR`'s `UNGTYPE` record | Most of it is geometry already in `mod_def.ww3`, so the reader may need only that record ⚠ |
| Advection | `W3XYPUG`, `W3XYPFSN2` | Below |
| `W3MAPT`, gradients | `UG_GRADIENTS`, `W3CFLUG` | New on this grid |
| Boundaries | `IOBP`, `IOBPD`, `IOBPA`, `SETDEPTH` | Direction-dependent masks at the coast |
| Points | `IS_IN_UNGRID` | Replaces `W3GRMP` |

Three things need care:

1. **The kernel is `REAL*8`.** `KELEM`, `FLALL`, `U`, `ST` are double precision and `C` is single (v,
   `w3profsmd.F90`). The propagation kernel here is `f32` throughout, so this is the first spectral kernel that
   needs `Float64`, on every sub-step ⚠. `f64` division on a device has Vulkan's 2.5 ULP, and the argument that
   makes `f32` division exact does not reach it, so a bit-identical device result is an open question ⚠.
2. **`ST(NI) = ST(NI) + ...` is a scatter with conflicts.** Two triangles share a node, so looping triangles in
   parallel races. Bit parity needs each node's sum in the Fortran's order, and sums in a different order round differently. One way that keeps the order: compute per triangle, then sum per node over its triangles in ascending `IE`,
   with the connectivity WW3 already holds (`CCON`, `IE_CELL`, `POS_CELL` in `w3gdatmd.F90`). That those lists are
   ascending has not been checked ⚠. Colouring triangles would change the order of each sum, so it cannot be used
   here without the owner's decision (`AGENTS.md`, reductions).
3. **`ITER` changes per component.** Sub-step counts differ across components, so threads do unequal work and a
   device dispatch loops a different number of times in each.

`IMPTOTAL` and `EXPTOTAL` need `PDLIB`, which the reference does not build, and a block solver. They come after, if
the owner wants them.

## Evidence and what is unverified

| Source | How it was checked |
|---|---|
| Roland, A., Ardhuin, F. (2014), *Ocean Dynamics* 64, 833-846, [10.1007/s10236-014-0711-z](https://doi.org/10.1007/s10236-014-0711-z) | (v) Crossref, and the author's PDF read 2026-10-31 |
| Ardhuin, F., Roland, A. (2012), *J. Geophys. Res. Oceans* 117, [10.1029/2011JC007832](https://doi.org/10.1029/2011JC007832) | (v) Crossref for the record; content not read ⚠ |
| Abdolali, A. et al. (2020), *Coastal Engineering* 157, 103656, [10.1016/j.coastaleng.2020.103656](https://doi.org/10.1016/j.coastaleng.2020.103656) | (v) Crossref for the record; content not read ⚠ |
| Csík, Á., Ricchiuto, M., Deconinck, H. (2002), *J. Comput. Phys.* 179, 286-312, [10.1006/jcph.2002.7057](https://doi.org/10.1006/jcph.2002.7057) | (v) Crossref. The 2014 paper's list gives volume 172, and Crossref gives 179 |
| Roland, A. (2008), Ph.D. thesis, TU Darmstadt | ⚠ cited by the code and the manual, not read here |

The scheme orders are the manual's, not measured. The implicit and explicit error figures are the 2014 paper's
case, not a general bound.
