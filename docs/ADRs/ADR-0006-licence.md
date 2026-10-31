# ADR-0006: Translated files are MPL-2.0

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-30 |
| **Deciders** | Hoffmann |

## Context

`LICENSE` here is MPL-2.0. WW3 is under the LGPLv3 (`WW3/LICENSE.md`, v), and the glibc copies of
`ww3-libm` derive from LGPL-2.1 code. A line-by-line translation may be a derivative work of the
Fortran. This is an engineering reading, not legal advice ⚠. The obligations start at distribution,
and the repository is private today.

The owner, 2026-10-30, verbatim: "Mark translated files as MPL2."

## Decision

Every file carries `SPDX-License-Identifier: MPL-2.0`, translated files included. `ww3-kernels` as a
crate is MPL-2.0.

## Consequences

The compatibility of this with WW3's LGPLv3 is not settled by this ADR. Revisit it, with legal advice,
before the code is distributed outside the private repository ⚠.
