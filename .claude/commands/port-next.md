---
description: Start the Rust translation of one WW3 routine, in plan order. Reads the Fortran at the pin, then adds the kernel, the ledger row and the port report.
---

Port one routine, `$ARGUMENTS`, or the next one if none is named.

1. Read `AGENTS.md`, the current batch in `docs/RUST_GPU_PLAN_202610.md`, `rust/ports.tsv` and
   `docs/FLOAT_REPRESENTATION_202610.md`.
2. Read the whole routine in `WW3/model/src` at the pin. List its arguments, kinds and bounds.
3. Translate it into `rust/crates/`, with `// SPDX-License-Identifier: LGPL-3.0-only` and the
   `file:line` range it came from. Keep the Fortran's order of operations.
4. Add the row to `rust/ports.tsv` as `translated` and write `rust/port_report/<ROUTINE>.md`.
5. Run `just check`. Write no Rust test. The verdict comes from the comparison the batch names.
