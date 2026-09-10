# Red-team report — `temps` v5.0.0 @ `43dfc7f`

Adversarial multi-agent run: **9 attack lenses → 21 findings → 14 independently refuted by fresh
skeptic agents**. Result: **9 confirmed, 5 refuted, 7 low-severity left unrefuted.**

Every headline defect below was **re-reproduced by me directly** with a hand-written harness calling
only the public API — not taken on an agent's word. Anything I did not personally reproduce is
labelled as such.

---

# Resolution — all defects fixed

Every confirmed defect and every reported-but-unrefuted item below has been fixed, with a
regression test that fails on the old behaviour. `just check` is green: **175 tests, clippy,
doctests, and both examples**.

| # | Sev | Fix | Guarding test |
|---|---|---|---|
| D1 | high | `temps-chrono`: the Month/Year arm now shifts the **civil** time and resolves it through `resolve_local`, like every other arm. | `us_eastern_month_arithmetic_lands_in_a_dst_fold` |
| D2 | high | `temps-chrono::resolve_local` filters its candidates to those that genuinely read back as the requested civil time, so chrono's over-reported fold upper boundary can no longer resolve an hour early. | `us_eastern_fold_upper_boundary_resolves_to_its_only_instant` |
| D3 | high | `temps-jiff`: every arm — absolute, bare date, and the explicit-offset conversions — resolves against `now.time_zone()` instead of `TimeZone::system()`. | `a_pinned_provider_resolves_every_expression_in_its_own_zone` |
| D4 | medium | Backends now agree; the false "one place the two backends disagree" claim is corrected. | `backend_parity.rs` (new target in the `temps` crate) |
| D5 | low | The overflow check in `digit_number` is **emitted** rather than returned, so the enclosing `number()` label can no longer rewrite it into "expected number, found end of input". | `an_oversized_amount_names_the_overflow_instead_of_denying_the_number` |
| D6 | low | All deliberate diagnostics are emitted rather than returned (dates, times, hours, amounts), and `rich_errors_to_temps_error` prefers a `Custom` error over a generic expectation mismatch. | `an_impossible_calendar_date_reports_the_calendar_problem` |
| D7 | low | `local_civil`/`local_date` replace every panicking accessor (`date_naive`, `time`, `weekday`, `naive_local`) in the chrono provider. | `us_eastern_extreme_pinned_clocks_error_rather_than_panic` |
| D8 | low | A date-only `Absolute` honours its timezone on both backends (`resolve_with_timezone` / `resolve_with_zone`). | `a_date_only_absolute_honours_its_timezone` (both backends) |
| D9 | low | README's match gains its `LaterToday` arm, the install snippets say `5`, and the README is now `include_str!`-ed into `temps`' crate docs so its examples run as doctests. | `cargo test --doc -p temps --all-features` (6 tests, 3 from the README) |
| D10 | low | Three doc comments corrected to the `in 5 minutes` / `in 5minutes` pair (`lib.rs`, `lexer.rs`, `CLAUDE.md`). | `whitespace_separates_a_number_from_its_unit` |
| U1 | low | `calculate_weekday_offset` uses saturating arithmetic. | `weekday_offsets_saturate_instead_of_panicking` |
| U2 | low | `ERR_DATE_CALC_ERROR` no longer repeats the `Display` prefix, and the previously-hidden `context` field is now rendered. | `a_range_limit_failure_explains_itself_once` |
| U3 | low | `parse_to_datetime`'s `# Errors` list no longer claims a fold is an error. | doc only |
| U4 | low | A fixed-unit shift whose local reading is unrepresentable is rejected instead of returned as an unreadable `Ok`. | `a_returned_value_is_always_readable`, `us_eastern_a_shift_that_poisons_the_local_view_is_rejected` |

The same probes used to *find* these bugs were re-run afterwards and now pass under
`TZ=Europe/Paris` and `TZ=America/New_York`.

## Method notes worth keeping

- **The `try_map` trap.** A `try_map` error is registered at the cursor where the mapped parser
  *started*, and chumsky keeps whichever alternative read furthest — so every deliberate diagnostic
  written as `try_map(.. Rich::custom ..)` was silently replaced by "expected X, found Y" from an
  unrelated alternative. Emitted errors (`validate` + `emitter.emit`) are exempt from that ranking
  and still fail the parse, because `ParseResult::into_result` discards output whenever any error
  exists. **Prefer `validate` for diagnostics that must survive.**
- **`labelled` destroys custom reasons.** `Rich::label_with` rewrites a `Custom` reason into an
  `ExpectedFound`, and `take_found()` is `None` for a custom reason — hence the self-contradictory
  "expected number, found end of input" *while underlining the number*.
- **`Months::new`/`Days::new` do not panic** in chrono 0.4.45 (both are plain constructors), so the
  overflow discipline there is sound; the real chrono hazards were the panicking civil-time
  accessors and `checked_add_months`'s `.single()` resolution.
- The zone-pinned test pattern (`#[ignore]` + a runner that re-executes the binary with `TZ`) is the
  right home for any `TZ`-dependent assertion; it is reused by every new DST test above.

---

## How to reproduce

```bash
cd /Users/icepuma/workspace/repos/github.com/icepuma/temps
export CARGO_TARGET_DIR=$PWD/target          # ~/.cargo/target is not writable in the sandbox
TZ=Europe/Paris      cargo nextest run -p temps --all-features --test <harness> --no-capture
```

`TZ` must be set for the **whole process** — it is never set inside a test (it is process-wide and
tests run in parallel).

---

## Confirmed defects

### D1 — `high` — chrono: `in N months` / `in N years` fails or mislabels whenever the target lands in a DST transition

`temps-chrono/src/lib.rs:186-219`. This is the **only** arm of `ChronoProvider::parse_expression`
that does not route its shifted civil time through `resolve_local` (every other arm does: 238, 318,
327, 341, 351, 361, 371, 381, 412, 433, 471, 488, 508). `DateTime::<Local>::checked_add_months`
resolves internally via `.and_local_timezone(Tz::from_offset(&self.offset)).single()`
(`chrono-0.4.45/src/datetime/mod.rs:459-467`), which is `None` inside a gap or fold. Two distinct
defects follow.

**(a) Hard error on a resolvable date.** `TZ=America/New_York`, pinned `now = 2024-10-03 01:30 -04:00`:

```
chrono in 1 month   Err Date calculation error: Date calculation resulted in invalid date
chrono in 31 days   Ok  local=2024-11-03 01:30:00 -04:00  instant=2024-11-03T05:30:00Z
jiff   in 1 month   Ok  2024-11-03T01:30:00-04:00[America/New_York]  instant=2024-11-03T05:30:00Z
```

The identical civil target succeeds via `in 31 days`, via the absolute input `2024-11-03T01:30`,
and via jiff — but `in 1 month` returns `Err`. The returned variant is `DateCalculationError`, while
the crate documents `AmbiguousTime` for exactly this case (`temps-chrono/src/lib.rs:47`).

**(b) Stale local label at a gap boundary.** At the first instant of a gap chrono's `.single()`
*succeeds*, carrying the pre-gap offset, and the arm returns it unnormalised. `TZ=Pacific/Apia`,
`now = 2011-11-30 00:00 -10:00`: `in 1 month` yields a `DateTime<Local>` whose `naive_local()` is
`2011-12-30 00:00:00 -10:00` — a civil **date Samoa skipped entirely** — while the instant it holds
actually renders as `2011-12-31T00:00:00+14:00`, which is what `in 30 days`, the absolute input, and
jiff all return. The instant is right; the wall clock a user reads is wrong by up to a whole day.

**Fix:** shift the civil `NaiveDateTime` (`now.naive_local().checked_add_months(...)`) and pass it
through `resolve_local`, exactly as the Day/Week arm does. Nothing in the repo pins the current
behaviour.

### D2 — `high` — chrono: silent **wrong instant** at the end of a DST fall-back fold

`temps-chrono/src/lib.rs:77-96`, `Ambiguous` arm at line 83. Found independently by the differential
lens and confirmed by me. `TZ=Europe/Paris`:

```
chrono 03:00 absolute   Ok  local=2024-10-27 03:00:00 offset=+02:00  instant=2024-10-27T01:00:00Z
jiff   03:00 absolute   Ok  2024-10-27T03:00:00+01:00[Europe/Paris]   instant=2024-10-27T02:00:00Z
oracle (python zoneinfo)                                              = 2024-10-27T02:00:00Z
chrono value re-read in Local = 02:00:00   (requested 03:00)  <- internally inconsistent
```

The returned instant is **one DST shift early** and carries an offset not in force at that instant.
Reached by every arm that uses `resolve_local`, from plain strings: `2024-10-27T03:00:00`,
`today at 03:00`, `tomorrow at 03:00`, `in 1 day`. Affects all fall-back zones, with the error equal
to the zone's own shift (3600 s Paris/New York/Havana/Santiago, 1800 s Lord Howe, 7200 s Troll).

Root cause: chrono 0.4.45 reports the fold's **upper** boundary — a civil time that occurs only once,
in the post-transition offset — as `LocalResult::Ambiguous`, because its tz_info comparison is
inclusive at the upper end (`local_leap_time <= transition_start`). The `Ambiguous` arm trusts that
report, picks the earlier instant, and unlike the `Single` arm (line 81) never validates the result
by round-tripping it back. Fix shape already used in-tree: keep only candidates that still read back
as the requested civil time (`temps-chrono/tests/all_tests.rs:723-737`).

This also **falsifies the doc claim** at `temps-chrono/src/lib.rs:54-55` ("matching the jiff
backend's default `compatible` disambiguation").

### D3 — `high` — jiff: one pinned provider resolves zone-less expressions in **two different zones**

`temps-jiff/src/lib.rs`: the relative/day/time/daytime arms use `now.time_zone()`
(310, 326, 342, 358, 374, 409, 437, 467, 486); the absolute and bare-date arms use
`TimeZone::system()` (288, 299, 511). Pinned to Asia/Tokyo while the process runs `TZ=UTC`:

```
pinned now = 2024-06-01T10:00:00+09:00[Asia/Tokyo]   process TZ = UTC
tomorrow at 9:00 am   Ok  2024-06-02T09:00:00+09:00[Asia/Tokyo]  ts=2024-06-02T00:00:00Z
2024-06-02 09:00      Ok  2024-06-02T09:00:00+00:00[UTC]         ts=2024-06-02T09:00:00Z
tomorrow              Ok  2024-06-02T00:00:00+09:00[Asia/Tokyo]  ts=2024-06-01T15:00:00Z
2024-06-02            Ok  2024-06-02T00:00:00+00:00[UTC]         ts=2024-06-02T00:00:00Z
```

Same wall clock, two spellings, **9 hours apart** — the instants differ, not just the labels. The
absolute/date result is a function of the ambient `TZ`, which directly contradicts the `at()` doc's
promise that pinning "makes results reproducible" (lines 106-112). Re-running under `TZ=Asia/Tokyo`
makes the pairs agree (delta 0), proving the pin is ignored for those arms. It also fires in the
doc example's own configuration (pin UTC in a `Europe/Vienna` process: −2 h), and at jiff's
`Timestamp::MAX` it turns into an outright Ok-vs-Err split for the same civil date.

The inline comment "No timezone specified, treat as system timezone" (line 287) predates `at()`
(introduced later, verified with `git log -S`), so it documents the pre-pinning world where provider
zone and process zone were necessarily identical. Fix: use `now.time_zone()` in those three places;
no existing test depends on the ambient-zone behaviour (every pristine absolute/date test uses
`JiffProvider::new()`, where the two coincide).

### D4 — `medium` — chrono and jiff disagree on documented expressions

Beyond D1/D2, the differential lens found the failure-**set** divergence: `in N years` / `in N
months` is `Err` on chrono and `Ok` on jiff whenever the target wall clock falls in a fold or gap.
`temps-jiff/src/lib.rs:66-81` states that the year-9999 `Timestamp::MAX` limit "is the one place the
two backends disagree" — that claim is false as of this commit.

### D5 — `low` — numeric overflow is reported as a diagnostic that contradicts its own span

`temps-core/src/lib.rs:1062-1073`. `digit_number()` builds a custom "number too large" error, then
`.labelled("number")` (line 1072) sits exactly where chumsky's `TryMap` registers it, so
`Rich::label_with` rewrites the reason to `ExpectedFound` and takes `found` from `take_found()`,
which is `None` for a custom reason. Verified:

```
in 99999999999999999999 minutes -> expected number, found end of input
                                   ^^^^^^^^^^^^^^^^^^^^ (the number itself is underlined)
```

The message is factually false about its own underlined span and never mentions the real cause
(i64 overflow). `position: Some(3)` is correct.

### D6 — `low` — invalid calendar dates never surface the calendar reason

`temps-core/src/language/english.rs:651-657`, `temps-core/src/lib.rs:1177-1183`,
`german.rs:302-308`. The grammar builds `Rich::custom(span, "invalid date")`, but `choice` merges
alternatives and keeps the error registered furthest along, so the custom reason is unreachable
through `parse()`. Verified:

```
2024-02-30  -> expected whitespace, found `-`   (position 4)
31/02/2024  -> expected one of 'whitespace', am/pm, ':' or whitespace, found `/`
2024-13-01  -> expected whitespace, found `-`
```

The date is correctly *rejected* — only the diagnosis is wrong, pointing at a separator that valid
sibling inputs accept. 0 of 44 invalid-date inputs surfaced the calendar reason.

### D7 — `low` — a panic escapes a `Result`-returning public API

`temps-chrono/src/lib.rs:356` (and the same unguarded accessors at 337, 401/395, 429, 475-483,
232/237). `date_naive()` → `naive_local()` `expect()`s
(`chrono-0.4.45/src/datetime/mod.rs:579`). Verified, and it is **TZ-dependent**:

```
TZ=Europe/Vienna     NaiveDate::MAX 23:59:59  *** PANIC escaped a Result-returning API ***
TZ=America/New_York  NaiveDate::MIN 00:00:00  *** PANIC escaped a Result-returning API ***
TZ=UTC               (neither panics — the offset cannot push the local view over the edge)
```

**Not reachable from a plain `&str`** (year is `u16`, and the largest string-produced year is well
inside chrono's range) — it needs the embedder to hand `at()` a clock within ~26 h of chrono's
absolute range edge. Hence low, but it is an undocumented precondition on a safe API.

### D8 — `low` — date-only `Absolute` silently ignores a supplied timezone

`temps-chrono/src/lib.rs:322-329` (and the jiff equivalent at 296-302). Verified under
`TZ=America/New_York` via the public AST:

```
timezone=None            Ok  local=2024-01-15 00:00:00-05:00  instant=2024-01-15T05:00:00Z
timezone=Utc             Ok  local=2024-01-15 00:00:00-05:00  instant=2024-01-15T05:00:00Z
timezone=Offset{-300}    Ok  local=2024-01-15 00:00:00-05:00  instant=2024-01-15T05:00:00Z
```

The `else` branch never reads `abs.timezone`, so all three produce the same instant — 5 h after the
UTC instant the AST names. Not reachable from text in this form (`2024-01-15Z` is a parse error), so
it only bites callers constructing `AbsoluteTime` directly.

### D9 — `low` — the README's published example does not compile

`README.md:128-136` matches `TimeExpression` with 7 arms and no wildcard, but the enum has 8
variants — `LaterToday` (`temps-core/src/lib.rs:91`) is missing, and the enum is not
`#[non_exhaustive]`. Verified by inspection: a match missing a variant without a wildcard is a hard
E0004. Reachable from plain input (`parse("later today")`), so the example is stale rather than
hypothetically incomplete. CI never catches it: no `include_str!`/`#[doc = include_str!]` in
`temps/src`, so `cargo test --doc` never compiles README. The same README still installs
`temps = { version = "4" }` on the 5.0.0 crate page.

### D10 — `low` — three doc comments claim `5 minutes` is a time expression; it is not

Verified:

```
"5 minutes"    -> Err   (English and German)
"5minutes"     -> Err
"in 5 minutes" -> Ok(Relative { amount: 5, unit: Minute, direction: Future })
"in 5minutes"  -> Err
```

A bare `<amount> <unit>` is deliberately rejected without `in`/`ago`, and the corpus pins that
(`parser_corpus.tsv:333-334`). The whitespace-significance rationale is true, but the demonstrating
pair is wrong in **`temps-core/src/lib.rs:849-850`**, **`temps-core/src/lexer.rs:38-39`**, and
**`CLAUDE.md:92-93`** — the last of which is the developer guide this project steers agents by.

---

## Refuted — do not chase these

Five findings survived reproduction but failed the contract test. Their *observations* are real; the
claimed "expected" behaviour was invented, documented-against, or out of domain.

| Reported | Claim | Why refuted |
|---|---|---|
| medium | `tomorrow morning at 3:30 pm` is stranded by `day_expr`'s single-slot tail | Observation exact, but `<day> at <time>` is the only combination documented anywhere (README, rustdoc). Not a shadowing defect. |
| medium | `at` is hard-wired to `time_digits()`, so `tomorrow at noon` fails | Reproduced, but no doc promises named times after `at`; `noon` alone and `tomorrow at 12:00` both work. |
| medium | German keywords are case-*sensitive*, so `MONTAG` fails | **Documented by design** twice: `german.rs:15-20` (nouns case-sensitive, abbreviations not) and `word_cs`'s own doc naming `Montag`/`Tagen` as meaning-bearing capitalisation. |
| medium | `InvalidTime` names components that are valid (missing hour, huge nanosecond) | Missing-hour case is intentional *and test-pinned* in both crates, with an in-code comment; the nanosecond case is outside the documented domain. |
| low | No Unicode normalization, so NFD German input is rejected | `temps` compares exact bytes; canonical equivalence is not part of Rust `str` identity and is claimed nowhere. 16/16 documented German spellings parse. |

## Reported but not independently refuted (all `low`)

- `calculate_weekday_offset()` panics on i64 subtraction overflow for extreme arguments — a public
  `#[must_use]` helper with no `# Panics` section. **Unreachable from input** (both backends pass
  `0..=6`).
- jiff range-limit failures display as the doubled, cause-less
  `"Date calculation error: Date calculation error"`; the real jiff cause is stored only in a
  never-rendered context field.
- `parse_to_datetime`'s `# Errors` list claims ambiguity "will return an error"; ambiguous local
  times are actually resolved to the earlier instant (the behaviour is correct and matches jiff —
  the doc block contradicts `resolve_local`'s own doc in the same file).
- The largest accepted `in N hours` amount can return `Ok` with a `DateTime<Local>` whose
  `naive_local()` panics — the `Display` path survives via `overflowing_naive_local()`, so the value
  is silently poisoned rather than rejected.

## Notable negative results

The lexer/grammar architecture held up well under attack. The documented
token-prefix-shadowing invariant was probed hard by a dedicated lens and produced **no confirmed
violation**; `phrases_ci`/`phrases_cs` sorting and the left-factored `day_expr` tail behaved as
documented. A 3570-input × 4-zone sweep of extreme amounts (`in 357913941 years`,
`in 9223372036854775807 days`, `in 2147483648 weeks`) produced **0 panics** on both backends — the
`try_*`/checked discipline in the jiff backend and in chrono's fixed-unit branch is sound. The
`resolve_local` gap/fold logic itself was compared against an independent oracle at every transition
in 39 zones over 2010-2026 with 0 mismatches — its only defect is the `Ambiguous` upper-bound case
in D2.

## Suggested priority

1. **D2** — silently wrong instant from a plain string, and the value is internally inconsistent.
2. **D1** — everyday input (`in 1 month`) returns a hard error.
3. **D3** — pinned providers are not reproducible, which is the documented purpose of `at()`.
4. **D4** — fix the "one place the backends disagree" doc claim once D1-D3 are resolved.
5. D5-D10 are documentation and diagnostic quality; D9 and D10 are five-minute fixes.
