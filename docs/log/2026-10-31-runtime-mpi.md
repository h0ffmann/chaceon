# Runtime MPI for ww3_shel: 120 runs byte-identical

2026-10-31, agent session `cd86e8f5` for the owner ("chaceon must also allow MPI/OpenMP runtime option"); no issue.
Branch `goal/cd86e8f5-4471-runtime-mpi`. Report: `rust/port_report/WW3_SHEL_TS1.md`, "Ranks".

## Question

Can `mpirun -n R ww3_shel <case> [--threads N]` write the same bytes as the reference for every R, with a build that
needs no MPI library by default?

## Result

Yes on eight cases: R of 1 to 4 and R=2 with two threads, three runs each, 120 of 120 byte-identical (v,
`bash scripts/mpi_pair.sh`, 37 minutes on a laptop in Docker). The distribution is WW3's `DIST`: round-robin sea
points, `IAPPRO` for components, `W3GATH` and `W3SCAT` as `MPI_Alltoallv`, rank 0 writes.

## What went wrong on the way

- **Threads plus ranks crashed (abort, exit 134) on all five T=2 cases.** The column buffer was sized for the thread
  path (`NSPEC` columns) when ranks were also on, and a rank owns fewer components. Ranks now size it.
- **A fault that proved nothing.** Skipping the scatter of component 0 on rank 1 left `ww3_tp1.1` identical, because
  the case does not change that component. Skipping the even components made it differ. A fault test must reach data the
  case moves.
- The first runs of `mpi_pair.sh` came from a build older than the fix; the table above is from the rebuilt one.

## Not done

Ranks with a device, several hosts, other MPI libraries, speed. Every rank keeps the whole state; WW3 keeps only its own
points, so memory per rank does not fall.
