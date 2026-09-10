# Red-team report, round 2 — `temps` on `fix/redteam-defects` (`0c48c80`)

Second adversarial pass, aimed at **the fixes themselves** (new, unreviewed code is the likeliest
place for new bugs) and at the surfaces round 1 never touched.

**9 lenses → 13 findings → 13 independently refuted → 7 confirmed, 6 refuted.** Of the 7
confirmations, 5 are defects I introduced in `0c48c80`; all 5 are fixed on this branch and verified.
The remaining one is a pre-existing medium-severity defect, left unfixed and described below.

## What the attack *failed* to break

Two lenses came back empty, and that is the most useful result here:

- **`resolve-local-oracle`** attacked the rewritten `resolve_local` with an independent oracle
  (`python3 zoneinfo` plus chrono's reliable UTC→Local direction) across every DST transition in
  ten zones for 2010–2030, every local minute in a ±3 h window around each one: **0 disagreements**.
  The look-back window, the fold-boundary case that round 1 found, whole-day gaps, 30-minute and
  2-hour shifts, and the `LaterToday` clamp all held.
- **`grammar-families`** probed `fortnight`, `week_from_now`, `later`, `named_time`,
  `standalone_daytime`, `this_part_of_day`, fractional time and colloquial quantities for meaning and
  for the prefix-shadowing invariant: **0 findings**.

Round 1's three high-severity fixes look sound under a much harder attack than the one that found
them.

## Confirmed findings

| # | Sev | Finding | Origin | Status |
|---|---|---|---|---|
| R1 | **critical** | `include_str!("../../README.md")` escapes the package root: `cargo package`/`publish` fail their verify build, blocking release-plz | mine | **fixed** |
| R2 | low | The README blocks were the only ungated doctests: feature-less `cargo test --doc -p temps` went green → 3 failures | mine | **fixed** |
| R3 | medium | `AbsoluteTime`'s `second`/`nanosecond` are silently discarded when `hour` is `None` — both backends return `Ok` at midnight, while the sibling minute case is rejected | pre-existing | **fixed** |
| R4 | low | The emitted `invalid calendar date` asserted a false cause: `15/03-2024` names a real date, and the mismatched separator was never mentioned | mine | **fixed** |
| R5 | low | Emitted diagnostics are rolled back with their failing branch, so `raw_hour`'s "hour must be 0-23" was unreachable and `half past 25` got the generic alternative list | mine | **fixed** |
| R6 | low | `fractional_time`'s leftover `try_map` swallowed `raw_hour`'s emit (same root cause as R5, found by a second lens) | mine | **fixed** |
| R7 | low | The new `local_civil` doc claimed `time()` and `weekday()` panic on an out-of-range local reading; they do not — they silently wrap, which is worse | mine | **fixed** |

### R1 — the critical one, in full

```
$ cargo package -p temps --allow-dirty
    Verifying temps v5.0.0
error: couldn't read `src/../../README.md`: No such file or directory (os error 2)
   --> src/lib.rs:103:10
PACKAGE EXIT=101
```

Cargo copies an out-of-package `readme` to the package **root**, so from the packaged `src/lib.rs`
the path resolves one level *above* the tarball. This blocks `cargo publish` and therefore
release-plz, and a force-published crate would not compile for consumers (the refuter extracted the
tarball into a registry-shaped directory and reproduced the same failure). The same change caused R2.

Fixed with a single guarded attribute:

```rust
#![cfg_attr(
    all(doctest, feature = "chrono"),
    doc = include_str!("../../README.md")
)]
```

`doctest` keeps the path from ever being resolved outside a doctest run, and the feature gate stops
the README's `temps::chrono` examples from breaking a feature-less doctest run. Verified:
`--all-features` 6/6, no features 3/3, `--features chrono` 6/6, `cargo package` exit 0.

### R3 — the pre-existing one

```
hour=None minute=Some(30)             chrono: Err Invalid time: 00:30:00 | jiff: Err
hour=None second=Some(30)             chrono: Ok 2024-01-15T00:00:00Z    | jiff: Ok   <- silently dropped
hour=None nanosecond=Some(5e8)        chrono: Ok 2024-01-15T00:00:00Z    | jiff: Ok   <- silently dropped
```

The guard the code itself documents — *"A minute without an hour is not a time we can honour; say so
rather than silently falling through to midnight"* — applies equally to `second` and `nanosecond`,
and they had no guard at all. 12 of 64 component combinations were silently truncated, identically
on both backends.

**Not reachable from a plain `&str`** (the grammar always sets `hour` when it sets a time part), so
it needs a caller building `AbsoluteTime` by hand or completing a parsed date-only value — hence
medium, not high.

Fixed by extending the existing guard to all three sub-hour fields on both backends, so the shape is
now rejected rather than silently truncated. A matching assertion confirms the *same* component is
still honoured once an hour is present, so the guard is about the missing hour and not about the
field. This changes accepted input into rejected input for a shape that was returning a wrong instant,
which is the same call the code already made for `minute`.

## Refuted (6)

| Claim | Why refuted |
|---|---|
| Mixed separators reported as `invalid calendar date` (second filing) | Duplicate of R4; refuted as a separate finding, not as an observation |
| `in <20 digits>` with no unit still loses the overflow reason | Same mechanism as R5, but here the branch legitimately fails later (the unit is missing); the surviving message is true if terse |
| `now` returns the unreadable pinned clock verbatim | The value is an echo of the caller's own `at(..)` argument |
| Examples print errors in their `Standard Date Formats` section | `31.12.2024` is the *German* form and parses as German; `12/25/2024` is US MM/DD, supported by no documentation. Cosmetic residual: the section mixes languages without an `(expected)` marker |
| `hour must be 0-23` emit unreachable | Duplicate of R5/R6 |
| chrono accepts `nanosecond` 1e9..2e9 as a leap second when `second == 59`; jiff rejects | AST-only (the grammar caps fractions at 9 digits), so out of the documented input domain |

## Method notes

- **Round 1's own advice needed correcting.** That report says to prefer `validate` + `emitter.emit`
  for diagnostics that must survive. Round 2 found the limit of that: chumsky truncates emitted errors
  on `rewind`, so an emit **inside a branch that subsequently fails** is discarded. `validate` only
  guarantees survival when the emitting alternative *succeeds*. R5/R6 are exactly that trap — the fix
  is to remove the downstream `try_map` that was failing the branch, not to add another emit.
- **Duplicate pairs.** Two pairs of lenses filed the same behaviour with opposite verdicts
  (R4/R6 and their twins), because one refuter judged the observation and the other judged the filing.
  Worth knowing when reading these reports: "refuted" sometimes means "already filed", not "not a bug".
- **Shared-target-dir hazard.** Agents shared one warm `CARGO_TARGET_DIR`; a refuter caught the
  directory transiently serving a stale `temps-core` rlib while a sibling rebuilt it, which made a
  tracked regression test fail once and then pass. Any evidence gathered from a shared build dir
  during a concurrent run deserves a re-check — I re-ran every headline repro against a settled tree.
- Findings were written to `.redteam2/*.json` as well as returned, so a truncated workflow result
  could not lose them. That directory grew to **1.9 GB** of dumps and has been removed, along with all
  `zz_r2_*` scratch test files. The tree is clean.

## Verification after the fixes

`just check` → exit 0, **177 tests** (2 new), clippy clean, doctests green, both examples run.
`cargo package -p temps` → exit 0.
