# ADR-0006: Translated files are LGPL-3.0-only

| | |
|---|---|
| **Status** | Proposed: the owner decides, with legal advice if the code is to be distributed |
| **Date** | 2026-10-30 |
| **Deciders** | Hoffmann |

## Context

`LICENSE` here is MPL-2.0. WW3 is under the LGPLv3 (`WW3/LICENSE.md`, v). A line-by-line translation
is very likely a derivative work of the Fortran, so it cannot simply take a different licence. This
is an engineering reading, not legal advice ⚠. MPL-2.0 is compatible with the LGPL through its
Secondary License terms, unless a file is marked otherwise (v, MPL 2.0 FAQ, Q14, read 2026-10-30), so
the two can sit in one repository. The obligations start at distribution, and the repository is
private today. The `ww3-libm` glibc copies of Batch 0 derive from LGPL-2.1 code too.

## Decision

A file translated from WW3 or glibc carries its upstream's SPDX identifier, `LGPL-3.0-only` for WW3.
Original tooling and host code stay MPL-2.0. The `LICENSE` file is unchanged. Alternatives for the
owner: make the whole repository LGPL-3.0, or keep this per-file split.

## Consequences

`ww3-kernels` as a crate is LGPL-3.0-only. The same question applies to ww3-gpu's Kokkos port,
whose repository is MIT, and is out of scope here.
