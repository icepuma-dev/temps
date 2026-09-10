# Claude Development Guidelines for temps

This document provides guidelines for Claude when working on the temps codebase.

## Project Structure

This is a Rust workspace project (edition 2024, resolver 3, MSRV 1.88) with five crates:
- `temps-core` - Lexer, grammar and language-independent types. Depends only on `chumsky` (parsing), `ariadne` (diagnostics) and `thiserror`
- `temps-chrono` - Chrono integration for time operations
- `temps-jiff` - Jiff integration for time operations
- `temps-testhelpers` - Shared mocks/helpers, used only as a dev-dependency
- `temps` - Main crate that re-exports functionality from the sub-crates

## Development Workflow

### After Completing Any Task

**ALWAYS run `just check` after making changes** to ensure:
- Code is properly formatted
- All clippy warnings are addressed
- All tests pass
- The workspace builds successfully

Run it from the workspace root:

```bash
just check
```

**IMPORTANT**: Only use `just test` to run tests. Do not use `cargo test` directly.

### If just check Reports Issues

1. **Formatting issues**: The script runs `cargo fmt --all` automatically
2. **Clippy warnings**: Fix all warnings before considering the task complete
3. **Test failures**: Debug and fix failing tests
4. **Build errors**: Resolve compilation issues

### Common Issues and Fixes

#### Unused imports/variables
- Remove unused imports
- Prefix unused variables with underscore (e.g., `_unused_var`)

#### Method naming conventions
- Methods starting with `from_`, `to_`, `as_`, `into_` should follow Rust conventions
- Consider renaming methods that trigger clippy's `wrong_self_convention` warning

## Parser Architecture

The parsers are built with **chumsky** (there is no nom in this workspace) and run in two
stages:

1. **Lex** (`temps-core/src/lexer.rs`): the input string becomes a flat `Vec<(Token, SimpleSpan)>`
   of `Token::Word` / `Token::Number` / `Token::Punct` / `Token::Space`. Words and digit runs are
   consumed maximally and carry their source slice; spans are byte offsets so diagnostics can
   point back at the original text.
2. **Parse** (`temps-core/src/lib.rs`, module `common`, plus `temps-core/src/language/{english,german}.rs`):
   the grammar runs over those tokens, never over characters.

### Why the lexer exists

The grammar used to match keywords character by character, so `"day"` matched inside `"days"` and
`"m"` inside `"min"`. Correctness then depended on hand-ordering every alternation longest-first —
a convention that fails *silently* when broken. Now a keyword is compared against a whole token
(`word_ci`, `word_cs`, `phrase_ci`, `phrase_cs`), so it can never match part of a longer word,
whatever order the alternatives appear in. Do not reintroduce character-level or prefix matching.

### The invariant the grammar relies on

chumsky's `choice` commits to the **first alternative that succeeds**. Tokenizing killed the
sub-word version of this hazard but not the phrase-level one, so:

> No alternative may succeed on a proper **token-prefix** of another alternative's match.

Two mechanisms uphold it:

- **Keyword tables** built with `phrases_ci` / `phrases_cs` sort their entries internally (most
  tokens first), so the source order of a table is irrelevant, not load-bearing. Prefer them over
  a hand-written `choice` over `phrase_ci` calls.
- **Families sharing a leading token are left-factored** into one rule that parses the shared
  prefix once and treats the rest as an optional tail — e.g. `day_reference().then(tail.or_not())`
  in `day_expr` covers `tomorrow`, `tomorrow morning` and `tomorrow at 3:30 pm`;
  `later_expr` does the same for `later` / `later today`.

**Therefore: if you add an expression that extends an existing one, extend that rule's tail.**
Adding it as a sibling alternative in a `choice` will be silently shadowed by the shorter form,
which commits first and strands the remaining tokens against `end()`.

### Whitespace

`Token::Space` is a real token, not something skipped implicitly: `in 5 minutes` parses and
`in 5minutes` does not. (A bare `5 minutes` is *not* a time expression — an amount only stands alone
with a leading `in` or a trailing `ago`.) Use `space()` where a gap is required, `opt_space()` where
it is optional, and remember that a space inside a `phrase_ci` pattern requires one in the input.

## Testing Guidelines

### Running Tests
**Always use `just test` to run tests or `just check` for a complete check.** This ensures:
- Code is formatted before testing (with `just check`)
- Clippy checks are run (with `just check`)
- Tests are run with nextest for better output
- All features are properly enabled

### Adding Tests
- Place integration tests in the `tests/` directory of each crate
- Use descriptive test names that explain what is being tested
- For time-dependent behaviour, pin the clock on the **real** provider —
  `ChronoProvider::at(datetime)` / `JiffProvider::at(zoned)` — instead of reimplementing its
  logic in a mock. Mocks that duplicate provider logic drift from production and hide bugs
- `TZ` is process-wide and tests run in parallel, so never set it per-test; construct instants in
  an explicit timezone instead
- Clean up test files by removing unused imports and variables

## Code Quality Standards

1. **No approximations**: Use proper date arithmetic methods (e.g., `checked_add_months` for months/years)
2. **Error handling**: Use proper error types and messages
3. **Documentation**: Add doc comments for public APIs
4. **Feature flags**: Respect feature boundaries - chrono-specific code only when chrono feature is enabled

## The Justfile Commands

The `just check` command performs the following in order:
1. `just format` - Runs `cargo fmt --all` to format all code
2. `just lint` - Runs `cargo clippy --workspace --tests --examples --all-features --all-targets` for clippy checks
3. `just test` - Runs `cargo nextest run --workspace --all-features` for all tests
4. `just doc-test` - Runs `cargo test --doc --workspace --all-features` (nextest does not run doctests)
5. `just examples` - Runs both chrono and jiff examples to ensure they compile and execute

Available Just commands:
- `just` or `just check` - Run complete check (format, lint, test, doc-test, examples)
- `just format` - Format all code
- `just lint` - Run clippy checks
- `just test` - Run all tests with nextest
- `just doc-test` - Run doctests only
- `just examples` - Run all examples
- `just example-chrono` - Run chrono example only
- `just example-jiff` - Run jiff example only

**Always use these Just commands instead of running cargo commands directly.**

## Important Notes

- This is a library project - avoid creating unnecessary binaries or examples
- All crates share workspace-level package metadata; the two examples live in `examples/` and are
  wired up as `[[example]]` targets of the `temps` crate
- Always verify changes work by running `just check`
- Never commit code without running `just check` first