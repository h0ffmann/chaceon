-- SPDX-License-Identifier: MPL-2.0
import Mathlib

/-!
# Why widening an `f32` division to `f64` gives the correctly rounded `f32` quotient

`rust/tools/spirv_exact` rewrites each `f32` division `a / b` to an `f64` division, then rounds to `f32`
with `OpFConvert`. Vulkan allows 2.5 ULP of error in `OpFDiv`, so the `f64` quotient `r` is not the
exact quotient `q`. The claim: `r` rounds to the same `f32` as `q`, for a quotient in the normal range.

## Scaling

Write the operands as `a = A * 2^ea`, `b = B * 2^eb` with integer significands `0 < A, B < 2^24`.
Then `a / b = (A / B) * 2^(ea - eb)`. The midpoint between two adjacent 24-bit `f32` values near the
quotient is `M * 2^j` with `M` odd and `2^24 <= M < 2^25`. The scale `2^e` of the quotient and `2^j` of
the midpoint differ by a power of two, so dividing both by the quotient's scale leaves
`q = A / B` against `m = M / 2^t` for an integer `t` that may be negative. Every statement below is
about `q` and `m` after this common factor is removed. Multiplying by a power of two is exact in
`f32` and `f64` inside the normal range, so no rounding hides in it.

## What is proved

* `no_tie`: `q` is never a midpoint.
* `margin`: `q` is at least `m / 2^49` away from a midpoint `m`.
* `same_side`: a value within `5/2 * 2^-52` of `q` (relative) is on the same side of `m` as `q`.
* `closer_lo`, `rounds_same`: the decision "round to the lower of two adjacent grid points" is the
  comparison with their midpoint, so the perturbed `r` and the exact `q` get the same decision at
  every midpoint, hence the same rounded value.

## What is not proved

Subnormal results (`small_midpoint_ties` shows `no_tie` failing there), overflow, zero, infinity and
NaN operands; that widening `f32` to `f64` is exact; that `OpFConvert` rounds to nearest; that a device
meets 2.5 ULP. The first two are facts about the `f32` format, the last three are assumptions about the
device. `docs/LEAN_202610.md` lists them.
-/

namespace Chaceon.Division

/-- The midpoint `M / 2^t`, with `t` an integer: negative `t` is a midpoint above `2^24`. -/
def mid (M : ℕ) (t : ℤ) : ℚ := (M : ℚ) / (2 : ℚ) ^ t

/-- 1. A quotient of two 24-bit significands is never a 25-bit midpoint. -/
theorem no_tie (A B M : ℕ) (t : ℤ) (hA0 : 0 < A) (hA : A < 2 ^ 24) (hB : 0 < B)
    (hM : 2 ^ 24 ≤ M) (hodd : Odd M) : (A : ℚ) / B ≠ mid M t := by
  intro h
  have hB' : (B : ℚ) ≠ 0 := by positivity
  cases t with
  | ofNat n =>
    unfold mid at h
    simp only [Int.ofNat_eq_natCast, zpow_natCast] at h
    rw [div_eq_div_iff hB' (by positivity)] at h
    have h' : A * 2 ^ n = M * B := by exact_mod_cast h
    have hd : M ∣ A * 2 ^ n := ⟨B, by rw [h']⟩
    have hc : Nat.Coprime M (2 ^ n) := Nat.Coprime.pow_right n (Odd.coprime_two_right hodd)
    have := Nat.le_of_dvd hA0 (hc.dvd_of_dvd_mul_right hd)
    omega
  | negSucc n =>
    unfold mid at h
    rw [zpow_negSucc, div_inv_eq_mul, div_eq_iff hB'] at h
    have h' : A = M * 2 ^ (n + 1) * B := by exact_mod_cast h
    have : M ≤ A := by
      rw [h']
      have : 1 ≤ 2 ^ (n + 1) * B := Nat.one_le_iff_ne_zero.mpr (by positivity)
      calc M = M * 1 := by ring
        _ ≤ M * (2 ^ (n + 1) * B) := Nat.mul_le_mul_left _ this
        _ = M * 2 ^ (n + 1) * B := by ring
    omega

/-- 2. The distance from a quotient to a midpoint is at least `m / 2^49`. -/
theorem margin (A B M : ℕ) (t : ℤ) (hA0 : 0 < A) (hA : A < 2 ^ 24) (hB0 : 0 < B)
    (hB : B < 2 ^ 24) (hM : 2 ^ 24 ≤ M) (hM' : M < 2 ^ 25) (hodd : Odd M) :
    mid M t / 2 ^ 49 ≤ |(A : ℚ) / B - mid M t| := by
  have hBq : (0 : ℚ) < B := by exact_mod_cast hB0
  have hBlt : (B : ℚ) < 2 ^ 24 := by exact_mod_cast hB
  have hMlt : (M : ℚ) < 2 ^ 25 := by exact_mod_cast hM'
  have hMge : (2 : ℚ) ^ 24 ≤ M := by exact_mod_cast hM
  have hAlt : (A : ℚ) < 2 ^ 24 := by exact_mod_cast hA
  have hA1 : (1 : ℚ) ≤ A := by exact_mod_cast hA0
  have hB1 : (1 : ℚ) ≤ B := by exact_mod_cast hB0
  cases t with
  | ofNat n =>
    -- q - m = (A 2^n - M B) / (B 2^n); the numerator is a nonzero integer
    have hne := no_tie A B M (Int.ofNat n) hA0 hA hB0 hM hodd
    unfold mid at hne ⊢
    simp only [Int.ofNat_eq_natCast, zpow_natCast] at hne ⊢
    have hp : (0 : ℚ) < 2 ^ n := by positivity
    have hN : (A : ℤ) * 2 ^ n - M * B ≠ 0 := by
      intro h0
      apply hne
      rw [div_eq_div_iff hBq.ne' hp.ne']
      have : (A : ℚ) * 2 ^ n - M * B = 0 := by exact_mod_cast h0
      linarith
    have hN1 : (1 : ℚ) ≤ |(((A : ℤ) * 2 ^ n - M * B : ℤ) : ℚ)| := by
      have : (1 : ℤ) ≤ |(A : ℤ) * 2 ^ n - M * B| := Int.one_le_abs hN
      rw [← Int.cast_abs]
      exact_mod_cast this
    have e : (A : ℚ) / B - M / 2 ^ n = (((A : ℤ) * 2 ^ n - M * B : ℤ) : ℚ) / (B * 2 ^ n) := by
      push_cast; field_simp
    rw [e, abs_div, abs_of_pos (by positivity : (0 : ℚ) < B * 2 ^ n)]
    rw [div_div, div_le_div_iff₀ (by positivity) (by positivity)]
    have h49 : (B : ℚ) * M ≤ 2 ^ 49 := by nlinarith
    have h2 : (B : ℚ) * M * 2 ^ n ≤ 2 ^ 49 * 2 ^ n := mul_le_mul_of_nonneg_right h49 hp.le
    have h3 := mul_le_mul_of_nonneg_right hN1 (by positivity : (0 : ℚ) ≤ 2 ^ n * 2 ^ 49)
    nlinarith
  | negSucc n =>
    -- m = M 2^(n+1) >= 2^25 > q, so the gap is at least m / 2
    unfold mid
    rw [zpow_negSucc, div_inv_eq_mul]
    have hp : (2 : ℚ) ≤ 2 ^ (n + 1) := by
      calc (2 : ℚ) = 2 ^ 1 := by norm_num
        _ ≤ 2 ^ (n + 1) := pow_le_pow_right₀ (by norm_num) (by omega)
    have hq : (A : ℚ) / B ≤ A := div_le_self (by positivity) hB1
    have hm : (M : ℚ) * 2 ^ (n + 1) ≥ 2 * M := by nlinarith
    have hgap : (M : ℚ) * 2 ^ (n + 1) / 2 ≤ M * 2 ^ (n + 1) - A / B := by nlinarith
    have hmp : (0 : ℚ) < M * 2 ^ (n + 1) := by positivity
    rw [abs_sub_comm, abs_of_nonneg (by nlinarith)]
    have : (M : ℚ) * 2 ^ (n + 1) / 2 ^ 49 ≤ M * 2 ^ (n + 1) / 2 :=
      div_le_div_of_nonneg_left hmp.le (by norm_num) (by norm_num)
    linarith

/-- 3. A value within 2.5 `f64` ULP (relative `5/2 * 2^-52`) of a `q` that keeps the margin from `m`
is on the same side of `m` as `q`. -/
theorem side_lt (q m r : ℚ) (hq : 0 < q) (hm : 0 < m) (hmar : m / 2 ^ 49 ≤ |q - m|)
    (hr : |r - q| ≤ 5 / 2 * (1 / 2 ^ 52) * q) (h : q < m) : r < m := by
  rw [abs_sub_comm, abs_of_pos (by linarith)] at hmar
  have h1 := (abs_le.mp hr).2
  nlinarith [mul_pos hq hq, mul_pos hm hm, mul_pos hq hm]

theorem side_gt (q m r : ℚ) (hq : 0 < q) (hm : 0 < m) (hmar : m / 2 ^ 49 ≤ |q - m|)
    (hr : |r - q| ≤ 5 / 2 * (1 / 2 ^ 52) * q) (h : m < q) : m < r := by
  rw [abs_of_pos (by linarith)] at hmar
  have h1 := (abs_le.mp hr).1
  nlinarith [mul_pos hq hq, mul_pos hm hm, mul_pos hq hm]

theorem same_side (q m r : ℚ) (hq : 0 < q) (hm : 0 < m) (hmar : m / 2 ^ 49 ≤ |q - m|)
    (hr : |r - q| ≤ 5 / 2 * (1 / 2 ^ 52) * q) : r < m ↔ q < m := by
  have hne : q ≠ m := by
    intro h; subst h; simp at hmar; linarith [show (0:ℚ) < q / 2 ^ 49 by positivity]
  constructor
  · intro h
    by_contra hc
    have := side_gt q m r hq hm hmar hr (lt_of_le_of_ne (not_lt.mp hc) (Ne.symm hne))
    linarith
  · exact side_lt q m r hq hm hmar hr

/-- Round-to-nearest between two adjacent grid points `lo < hi` is the comparison with their midpoint:
`x` is strictly closer to `lo` exactly when it is below `(lo + hi) / 2`. -/
theorem closer_lo (lo hi x : ℚ) (h : lo < hi) :
    |x - lo| < |x - hi| ↔ x < (lo + hi) / 2 := by
  rw [← sq_lt_sq]
  constructor <;> intro h' <;> nlinarith

/-- 4. The corollary. Take adjacent grid points `lo = (M-1)/2^t`, `hi = (M+1)/2^t` of a 24-bit grid
(`M` odd in `[2^24, 2^25)`), the exact quotient `q = A/B`, and any `r` within `5/2 * 2^-52` of `q`.
Then `r` is closer to `lo` iff `q` is, and `q` is never equidistant. At each midpoint the widened
quotient therefore makes the same rounding decision as the exact one. -/
theorem rounds_same (A B M : ℕ) (t : ℤ) (r : ℚ) (hA0 : 0 < A) (hA : A < 2 ^ 24) (hB0 : 0 < B)
    (hB : B < 2 ^ 24) (hM : 2 ^ 24 ≤ M) (hM' : M < 2 ^ 25) (hodd : Odd M)
    (hr : |r - (A : ℚ) / B| ≤ 5 / 2 * (1 / 2 ^ 52) * ((A : ℚ) / B)) :
    let lo := ((M : ℚ) - 1) / 2 ^ t
    let hi := ((M : ℚ) + 1) / 2 ^ t
    (|r - lo| < |r - hi| ↔ |(A : ℚ) / B - lo| < |(A : ℚ) / B - hi|) ∧
      |(A : ℚ) / B - lo| ≠ |(A : ℚ) / B - hi| := by
  intro lo hi
  have hq : (0 : ℚ) < (A : ℚ) / B := by positivity
  have hp : (0 : ℚ) < 2 ^ t := zpow_pos (by norm_num) t
  have hm : (0 : ℚ) < mid M t := by unfold mid; positivity
  have hlh : lo < hi := by
    show ((M : ℚ) - 1) / 2 ^ t < ((M : ℚ) + 1) / 2 ^ t
    exact div_lt_div_of_pos_right (by linarith) hp
  have hmid : (lo + hi) / 2 = mid M t := by
    show (((M : ℚ) - 1) / 2 ^ t + ((M : ℚ) + 1) / 2 ^ t) / 2 = (M : ℚ) / 2 ^ t
    field_simp; ring
  have hmar := margin A B M t hA0 hA hB0 hB hM hM' hodd
  have hne := no_tie A B M t hA0 hA hB0 hM hodd
  refine ⟨?_, ?_⟩
  · rw [closer_lo lo hi _ hlh, closer_lo lo hi _ hlh, hmid]
    exact same_side _ _ _ hq hm hmar hr
  · intro h
    have := (closer_lo lo hi ((A : ℚ) / B) hlh)
    apply hne
    have h2 : ((A : ℚ) / B - lo) ^ 2 = ((A : ℚ) / B - hi) ^ 2 := by
      rw [← sq_abs, h, sq_abs]
    have h3 : (2 * ((A : ℚ) / B) - lo - hi) * (hi - lo) = 0 := by nlinarith
    rcases mul_eq_zero.mp h3 with h4 | h4
    · rw [← hmid]; linarith
    · linarith

/-- The hypothesis `2^24 <= M` cannot be dropped: a subnormal result has a midpoint with fewer
bits, and then the quotient can be exactly a midpoint (`1 / 2 = 1 / 2^1`: `A = 1`, `B = 2`, `M = 1`,
`t = 1`), so `no_tie` and the margin both fail. -/
theorem small_midpoint_ties :
    ∃ A B M : ℕ, ∃ t : ℤ, 0 < A ∧ A < 2 ^ 24 ∧ 0 < B ∧ B < 2 ^ 24 ∧ Odd M ∧ M < 2 ^ 24 ∧
      (A : ℚ) / B = mid M t ∧ |(A : ℚ) / B - mid M t| < mid M t / 2 ^ 49 := by
  refine ⟨1, 2, 1, 1, by norm_num, by norm_num, by norm_num, by norm_num, by decide, by norm_num,
    ?_, ?_⟩ <;> norm_num [mid]

/-- The same counterexample as a plain equation. -/
example : (1 : ℚ) / 2 = mid 1 1 := by norm_num [mid]

end Chaceon.Division
