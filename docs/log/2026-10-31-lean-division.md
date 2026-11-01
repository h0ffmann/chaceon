# Lean proof of the SPIR-V division widening

2026-10-31, agent session `cd86e8f5` for the owner, branch `goal/cd86e8f5-6693-lean-division`; no issue. Machine: the
owner's Apple Silicon Mac, OrbStack, image `chaceon-lean` (linux/arm64).

## Question

`spirv_exact` rewrites an `f32` division to `f64` and rounds back, because Vulkan allows 2.5 ULP in `OpFDiv`. Its
comment gave "53 >= 2*24+2" as the reason. Is the quotient still correctly rounded when the `f64` division is off by
2.5 ULP, and for which quotients?

## Result

Yes for a quotient in the normal range, proved in `lean/Chaceon/Division.lean` (v, `just lean`, Lean `v4.35.0`,
mathlib `v4.35.0`): no tie (`no_tie`), a relative margin `2^-49` to every midpoint (`margin`), and a perturbation of
`2.5 * 2^-52` stays on the same side (`same_side`, `rounds_same`). `#print axioms` shows `propext`,
`Classical.choice`, `Quot.sound` for all six declarations; no `sorry`.

The old wording was incomplete, not wrong where it was used: the argument needs the midpoint to have 25 bits. In the
subnormal range it has fewer, and `small_midpoint_ties` is a checked counterexample (`A = 1`, `B = 2`, `M = 1`, `t = 1`:
`1/2` is exactly a midpoint). The comment and `docs/FLOAT_REPRESENTATION_202610.md` now say so.

The planned proof of `margin` was right for `t >= 0` only. For `t < 0` (a midpoint `M * 2^s` above `2^24`) the
integer-numerator argument gives a margin of `1/B`, which is not `>= m/2^49` once `s` is large. Another argument closes
it: `q < 2^24 <= M <= m/2`, so `|q - m| >= m/2`. The statement was true; only the proof differed.

## Negative checks (v)

Changing `2 ^ 49` to `2 ^ 48` in `margin` stops the build in the `t >= 0` case: the closing `nlinarith` cannot get `|N| * (2^n * 2^48) >= M * (B * 2^n)` from `B * M < 2^49`.
Changing `M < 2 ^ 25` to `M < 2 ^ 26` in `margin` stops it at the cast of that bound, and `rounds_same` no longer
applies it. Both restored.

## Not done

Device behaviour: that `OpFConvert` rounds to nearest, that widening is exact, that the device meets 2.5 ULP. See
[`docs/LEAN_202610.md`](../LEAN_202610.md).
