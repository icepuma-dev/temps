//! Backend-independent regression tests.
//!
//! Every test here pins a behaviour that was once wrong at the parser or
//! tokenizer level: a phrase that did not parse, two phrases that collided, a
//! value that could not be represented, or a diagnostic that pointed nowhere.
//! Nothing in this file needs a datetime backend — it is all `temps-core`.

use chumsky::prelude::*;
use temps_core::common::{token_stream, word_ci};
use temps_core::lexer::{Token, lex};
use temps_core::time_utils::calculate_weekday_offset;
use temps_core::{
    AbsoluteTime, DayReference, Direction, Language, RelativeTime, TempsError, TimeExpression,
    TimeUnit, Timezone, Weekday, WeekdayModifier, parse,
};

fn parse_en(input: &str) -> TimeExpression {
    parse(input, Language::English).unwrap_or_else(|e| panic!("failed to parse {input:?}: {e}"))
}

fn parse_de(input: &str) -> TimeExpression {
    parse(input, Language::German).unwrap_or_else(|e| panic!("failed to parse {input:?}: {e}"))
}

// ===== Colloquial quantities =====

/// "a couple of X" is the same quantity however the speaker spells it out. The
/// `of` variant used to be the one that failed, so all three spellings are
/// pinned together.
#[test]
fn a_couple_means_two_however_it_is_phrased() {
    let expected = TimeExpression::Relative(RelativeTime {
        amount: 2,
        unit: TimeUnit::Day,
        direction: Direction::Past,
    });

    for input in [
        "a couple of days ago",
        "a couple days ago",
        "couple of days ago",
    ] {
        assert_eq!(parse_en(input), expected, "mismatch for {input:?}");
    }
}

/// A known gap, pinned so that closing it is a deliberate act: the article and
/// the `of` are each optional on their own, but dropping *both* leaves nothing
/// to anchor `couple` and the phrase is rejected.
#[test]
fn couple_needs_either_its_article_or_its_of() {
    assert!(
        parse("couple days ago", Language::English).is_err(),
        "if this now parses, drop this test and fold the phrase into \
         `a_couple_means_two_however_it_is_phrased`"
    );
}

#[test]
fn a_couple_reads_the_same_in_the_future_direction() {
    assert_eq!(
        parse_en("in a couple of days"),
        TimeExpression::Relative(RelativeTime {
            amount: 2,
            unit: TimeUnit::Day,
            direction: Direction::Future,
        })
    );
}

// ===== Weekend references =====

/// "this weekend" and "next weekend" both name a Saturday, so they are only
/// distinguishable by their modifier. They used to produce the *same*
/// `DayReference`, which made them resolve to the same date.
#[test]
fn this_weekend_and_next_weekend_are_different_references() {
    let this = parse_en("this weekend");
    let next = parse_en("next weekend");

    assert_eq!(
        this,
        TimeExpression::Day(DayReference::Weekday {
            day: Weekday::Saturday,
            modifier: Some(WeekdayModifier::This),
        })
    );
    assert_eq!(
        next,
        TimeExpression::Day(DayReference::Weekday {
            day: Weekday::Saturday,
            modifier: Some(WeekdayModifier::Next),
        })
    );
    assert_ne!(
        this, next,
        "`this weekend` and `next weekend` must not collide"
    );
}

/// Sunday is the day the two used to agree on: with `This` unimplemented, both
/// walked forward to the coming Saturday. `This` now looks *back* to the
/// Saturday of the current Monday-to-Sunday week.
#[test]
fn this_and_next_weekend_resolve_to_different_days_when_asked_on_a_sunday() {
    // Offsets from Monday: Monday = 0 ... Saturday = 5, Sunday = 6.
    let sunday = 6;
    let saturday = 5;

    let this = calculate_weekday_offset(sunday, saturday, Some(WeekdayModifier::This));
    let next = calculate_weekday_offset(sunday, saturday, Some(WeekdayModifier::Next));

    assert_eq!(
        this, -1,
        "`this weekend` on a Sunday is yesterday's Saturday"
    );
    assert_eq!(next, 6, "`next weekend` on a Sunday is six days out");
    assert_ne!(this, next);
}

/// The unmodified weekday keeps its own rule (the next occurrence, today
/// included), so it must not be confused with either modifier.
#[test]
fn a_bare_weekday_takes_the_next_occurrence_including_today() {
    let sunday = 6;
    assert_eq!(calculate_weekday_offset(sunday, sunday, None), 0);
    assert_eq!(calculate_weekday_offset(sunday, 5, None), 6);
    assert_eq!(
        calculate_weekday_offset(sunday, sunday, Some(WeekdayModifier::Next)),
        7,
        "`next Sunday` asked on a Sunday must not mean today"
    );
    assert_eq!(
        calculate_weekday_offset(sunday, sunday, Some(WeekdayModifier::Last)),
        -7,
        "`last Sunday` asked on a Sunday must not mean today"
    );
}

// ===== "later" vs "later today" =====

/// "later today" is clamped to today; "later" is a plain two-hour offset that
/// may cross midnight. They are different expressions and must stay so — the
/// bug was `later today` parsing as `later` plus ignored input.
#[test]
fn later_today_is_clamped_but_bare_later_is_relative() {
    assert_eq!(parse_en("later today"), TimeExpression::LaterToday);
    assert_eq!(
        parse_en("later"),
        TimeExpression::Relative(RelativeTime {
            amount: 2,
            unit: TimeUnit::Hour,
            direction: Direction::Future,
        })
    );
    assert_ne!(parse_en("later today"), parse_en("later"));
}

// ===== Calendar days rather than durations =====

/// "the day before yesterday" names a calendar day, not "48 hours ago"; it used
/// to parse as a duration, which drifts across a DST transition.
#[test]
fn the_day_before_yesterday_is_a_calendar_day() {
    let expected = TimeExpression::Day(DayReference::DayBeforeYesterday);
    for input in ["the day before yesterday", "day before yesterday"] {
        assert_eq!(parse_en(input), expected, "mismatch for {input:?}");
    }
}

#[test]
fn the_day_after_tomorrow_is_a_calendar_day() {
    let expected = TimeExpression::Day(DayReference::DayAfterTomorrow);
    for input in ["the day after tomorrow", "day after tomorrow"] {
        assert_eq!(parse_en(input), expected, "mismatch for {input:?}");
    }
}

// ===== Timezone offsets =====

/// A negative sub-hour offset has a zero hour field, so the sign lives only on
/// the minutes. Storing hours and minutes separately made `-00:30` inexpressible
/// (it came back as `+00:30`); a single signed minute count fixes it.
#[test]
fn negative_sub_hour_timezone_offsets_keep_their_sign() {
    let TimeExpression::Absolute(AbsoluteTime { timezone, .. }) =
        parse_en("2024-01-15T23:59:00-00:30")
    else {
        panic!("expected an absolute datetime");
    };
    assert_eq!(timezone, Some(Timezone::Offset { total_minutes: -30 }));
}

#[test]
fn timezone_offsets_round_trip_across_the_sign_boundary() {
    let cases = [
        ("2024-01-15T23:59:00-00:30", -30),
        ("2024-01-15T23:59:00+00:30", 30),
        ("2024-01-15T23:59:00-05:30", -330),
        ("2024-01-15T23:59:00+05:45", 345),
    ];
    for (input, total_minutes) in cases {
        let TimeExpression::Absolute(AbsoluteTime { timezone, .. }) = parse_en(input) else {
            panic!("expected an absolute datetime for {input:?}");
        };
        assert_eq!(
            timezone,
            Some(Timezone::Offset { total_minutes }),
            "mismatch for {input:?}"
        );
    }
}

// ===== German capitalisation =====

/// German capitalises the first word of a sentence, so a modifier can arrive
/// capitalised. Matching was ASCII-only case folding, which leaves `Ä` alone
/// and so rejected `Nächsten`. (The weekday *noun* stays case-sensitive by
/// design — `Montag` is a proper noun — so only the modifier varies here.)
#[test]
fn german_modifiers_parse_whatever_their_case() {
    let expected = TimeExpression::Day(DayReference::Weekday {
        day: Weekday::Monday,
        modifier: Some(WeekdayModifier::Next),
    });
    for input in [
        "Nächsten Montag",
        "nächsten Montag",
        "NÄCHSTEN Montag",
        "nÄcHsTeN Montag",
    ] {
        assert_eq!(parse_de(input), expected, "mismatch for {input:?}");
    }
}

/// `nächste` and `letzte` are mirror images; whatever case one accepts, the
/// other must accept too, including uppercased umlauts (`ä` -> `Ä`).
#[test]
fn german_next_and_last_accept_the_same_cases() {
    for case in ["NÄCHSTE MO", "nächste Mo", "Nächste MO"] {
        assert_eq!(
            parse_de(case),
            TimeExpression::Day(DayReference::Weekday {
                day: Weekday::Monday,
                modifier: Some(WeekdayModifier::Next),
            }),
            "mismatch for {case:?}"
        );
    }
    for case in ["LETZTE MO", "letzte Mo", "Letzte MO"] {
        assert_eq!(
            parse_de(case),
            TimeExpression::Day(DayReference::Weekday {
                day: Weekday::Monday,
                modifier: Some(WeekdayModifier::Last),
            }),
            "mismatch for {case:?}"
        );
    }
}

// ===== Diagnostics on non-ASCII input =====

const UMLAUT_FAILURE: &str = "in fünf TageX";

/// Character index of `needle` in `haystack` — the unit a rendered report and
/// `ParseError::position` both speak in, and the one a byte offset is easy to
/// mistake for.
fn char_index_of(haystack: &str, needle: &str) -> usize {
    let byte = haystack.find(needle).expect("needle is present");
    haystack[..byte].chars().count()
}

fn umlaut_parse_error() -> (String, Option<usize>) {
    match parse(UMLAUT_FAILURE, Language::German) {
        Err(TempsError::ParseError {
            message, position, ..
        }) => (message, position),
        other => panic!("expected a parse error, got {other:?}"),
    }
}

/// The renderer indexes its source by character while parser spans are byte
/// offsets. Feeding one to the other silently dropped the underline on any
/// input containing an umlaut, leaving a diagnostic that pointed at nothing.
#[test]
fn a_failure_on_umlaut_input_still_underlines_the_offending_token() {
    let (message, _) = umlaut_parse_error();

    assert!(
        message.contains(UMLAUT_FAILURE),
        "the diagnostic should quote the source line:\n{message}"
    );

    let (source_line, marker_line) = message
        .lines()
        .zip(message.lines().skip(1))
        .find(|(line, _)| line.contains(UMLAUT_FAILURE))
        .expect("the report should show the source line");

    // ariadne draws the pointer as `──┬──` under the span; a plain `^` caret
    // would do just as well. What matters is that *something* points.
    let caret = marker_line
        .chars()
        .position(|c| c == '┬' || c == '^')
        .unwrap_or_else(|| {
            panic!("no caret under the offending token:\n{message}");
        });

    // Both lines carry the same gutter, so character columns line up.
    let token_start = char_index_of(source_line, "TageX");
    let token_end = token_start + "TageX".chars().count();
    assert!(
        (token_start..token_end).contains(&caret),
        "the caret sits at column {caret}, outside the offending token's columns \
         {token_start}..{token_end}:\n{message}"
    );
}

/// The diagnostic is folded into an error message that callers may log, embed,
/// or compare, so it must not carry terminal colour codes.
#[test]
fn diagnostics_carry_no_ansi_escapes() {
    let (message, _) = umlaut_parse_error();
    assert!(
        !message.contains('\u{1b}'),
        "the rendered diagnostic must be plain text:\n{message:?}"
    );
}

/// `position` is documented as a character position. On `in fünf TageX` the
/// byte offset of the offending token is 9 but its character index is 8, so the
/// two can be told apart.
#[test]
fn parse_error_position_is_a_character_index_not_a_byte_offset() {
    let (_, position) = umlaut_parse_error();

    let byte_offset = UMLAUT_FAILURE.find("TageX").expect("token is present");
    let char_index = char_index_of(UMLAUT_FAILURE, "TageX");
    assert_ne!(
        byte_offset, char_index,
        "this input must contain a multi-byte character for the test to mean anything"
    );

    assert_eq!(position, Some(char_index));
}

// ===== Tokenizer guarantees =====

/// The parsers used to run over characters, so `word_ci("day")` matched the
/// `day` *inside* `days` and correctness depended on hand-ordering every
/// alternation longest-first. Words are now consumed maximally and compared
/// whole, so ordering cannot matter.
#[test]
fn a_keyword_never_matches_part_of_a_longer_word() {
    assert_eq!(
        lex("days").into_iter().map(|(t, _)| t).collect::<Vec<_>>(),
        vec![Token::Word("days")],
        "`days` must lex as one word"
    );

    let tokens = lex("days");
    let matched = word_ci("day")
        .then_ignore(end())
        .parse(token_stream("days", &tokens))
        .into_result()
        .is_ok();
    assert!(!matched, "the keyword `day` must not match the word `days`");

    let tokens = lex("day");
    let matched = word_ci("day")
        .then_ignore(end())
        .parse(token_stream("day", &tokens))
        .into_result()
        .is_ok();
    assert!(matched, "the keyword `day` must still match the word `day`");
}

/// Whole-word matching also means a keyword cannot be extended by trailing
/// junk: `daysx` is one word and matches no unit.
#[test]
fn trailing_junk_on_a_keyword_is_a_parse_error() {
    for input in ["in 5 daysx", "in 5 dayss", "tomorrowx", "nowish"] {
        assert!(
            parse(input, Language::English).is_err(),
            "{input:?} should not parse"
        );
    }
}

/// Whitespace is a token, not something the lexer throws away: `5 minutes` is a
/// quantity and `5minutes` is not, and the parser can only tell them apart if
/// the gap survives lexing.
///
/// Note the leading `in`: a bare `<amount> <unit>` is *not* an expression — an
/// amount only stands alone with `in` or `ago` — so the demonstrating pair has
/// to carry one. (The docs in `lexer.rs` and `common::space` claimed otherwise.)
#[test]
fn whitespace_separates_a_number_from_its_unit() {
    assert_eq!(
        lex("in 5 minutes")
            .into_iter()
            .map(|(t, _)| t)
            .collect::<Vec<_>>(),
        vec![
            Token::Word("in"),
            Token::Space,
            Token::Number("5"),
            Token::Space,
            Token::Word("minutes")
        ],
    );
    assert_eq!(
        lex("in 5minutes")
            .into_iter()
            .map(|(t, _)| t)
            .collect::<Vec<_>>(),
        vec![
            Token::Word("in"),
            Token::Space,
            Token::Number("5"),
            Token::Word("minutes")
        ],
        "the missing gap must be visible to the parser"
    );

    assert!(
        parse("in 5 minutes", Language::English).is_ok(),
        "`in 5 minutes` should parse"
    );
    assert!(
        parse("in 5minutes", Language::English).is_err(),
        "`in 5minutes` should not parse"
    );
    assert!(
        parse("5 minutes", Language::English).is_err(),
        "a bare quantity is not a time expression"
    );
}

// ===== Diagnostics =====

/// An amount too large for `i64` has to say so. The number token used to be
/// labelled, and chumsky's `LabelledWith` rewrites a custom reason into an
/// `ExpectedFound` — whose `found` is `None` for a custom reason — so the user
/// was told "expected number, found end of input" while the offending digits
/// were underlined.
#[test]
fn an_oversized_amount_names_the_overflow_instead_of_denying_the_number() {
    let error = parse("in 99999999999999999999 minutes", Language::English)
        .expect_err("20 digits do not fit in an i64");

    let rendered = error.to_string();
    assert!(
        rendered.contains("too large"),
        "the message should name the overflow, got: {rendered}"
    );
    assert!(
        !rendered.contains("found end of input"),
        "the underlined input is a number, not the end of it: {rendered}"
    );
}

/// An impossible date must be reported as an impossible date. The grammar's
/// calendar check used to be a `try_map`, whose error chumsky registers at the
/// cursor where the parser *started* — so it was discarded in favour of
/// whichever alternative had read further, and `2024-02-30` was answered with
/// a complaint about the `-` separator that valid dates accept.
#[test]
fn an_impossible_calendar_date_reports_the_calendar_problem() {
    for (input, language) in [
        ("2024-02-30", Language::English),
        ("2024-13-01", Language::English),
        ("2023-02-29", Language::English),
        ("31/02/2024", Language::English),
        ("29/02/2023", Language::English),
        ("31.02.2024", Language::German),
        ("2024-02-30", Language::German),
    ] {
        let error = parse(input, language).expect_err("not a real calendar date");
        let rendered = error.to_string();
        assert!(
            rendered.contains("invalid calendar date"),
            "expected the calendar diagnosis for {input:?} ({language:?}), got: {rendered}"
        );
    }

    // ...while the dates that *are* real keep parsing.
    for input in ["2024-02-29", "29/02/2024", "31/12/2025", "2024-12-31"] {
        assert!(
            parse(input, Language::English).is_ok(),
            "{input:?} is a real date and should parse"
        );
    }
}

/// Rejecting an impossible date must not depend on the message surviving: the
/// value the validator produced is still discarded.
#[test]
fn an_impossible_calendar_date_is_still_rejected_by_every_spelling() {
    for (input, language) in [
        ("2024-02-30", Language::English),
        ("31/02/2024", Language::English),
        ("30.02.2024", Language::German),
        ("2024-02-30T12:00:00Z", Language::English),
    ] {
        assert!(
            parse(input, language).is_err(),
            "{input:?} ({language:?}) must not parse"
        );
    }
}

/// Time components outside the 24-hour clock are diagnosed the same way.
#[test]
fn an_impossible_clock_time_reports_the_time_problem() {
    for input in ["2024-01-15T25:00", "2024-01-15T12:60", "25:00"] {
        let error = parse(input, Language::English).expect_err("not a real clock time");
        let rendered = error.to_string();
        assert!(
            rendered.contains("invalid time"),
            "expected the time diagnosis for {input:?}, got: {rendered}"
        );
    }
}

/// A fractional expression with an impossible hour has to keep the reason
/// `raw_hour` emitted. `fractional_time` used to close with its own `try_map`,
/// which failed the whole alternative — and chumsky rolls emitted errors back
/// when their branch fails, so the deliberate "hour must be 0-23" was discarded
/// and the user got the generic list of every top-level alternative instead.
#[test]
fn a_fractional_time_with_an_impossible_hour_keeps_its_reason() {
    for input in ["half past 25", "quarter past 24", "quarter to 26"] {
        let error = parse(input, Language::English).expect_err("hour is out of range");
        let rendered = error.to_string();
        assert!(
            rendered.contains("hour must be 0-23"),
            "expected the hour diagnosis for {input:?}, got: {rendered}"
        );
    }

    // The valid forms still parse, and still mean what they say. `raw_hour`
    // takes a digit, `noon` or `midnight` — not a spelled-out hour.
    for (input, hour, minute) in [
        ("half past 3", 3, 30),
        ("quarter past 3", 3, 15),
        ("quarter to 4", 3, 45),
        ("quarter to 12", 11, 45),
        ("quarter to noon", 11, 45),
        ("half past midnight", 0, 30),
    ] {
        match parse(input, Language::English) {
            Ok(TimeExpression::Time(time)) => assert_eq!(
                (time.hour, time.minute),
                (hour, minute),
                "{input:?} resolved to the wrong clock time"
            ),
            other => panic!("{input:?} should be a plain time, got {other:?}"),
        }
    }
}

/// A real date written with two different separators is not an impossible date,
/// and must not be told that it is: `15/03-2024` names 15 March 2024, and the
/// only wrong thing about it is the `-`.
#[test]
fn mismatched_date_separators_are_not_reported_as_an_impossible_date() {
    for input in ["15/03-2024", "15-03/2024", "31/12-2025"] {
        let error = parse(input, Language::English).expect_err("mixed separators are rejected");
        let rendered = error.to_string();
        assert!(
            rendered.contains("date separators must match"),
            "expected a separator diagnosis for {input:?}, got: {rendered}"
        );
        assert!(
            !rendered.contains("invalid calendar date"),
            "{input:?} names a real date; the separators are the problem: {rendered}"
        );
    }

    // Genuinely impossible dates still say so.
    for input in ["31/02/2024", "2024-02-30"] {
        let rendered = parse(input, Language::English)
            .expect_err("not a real date")
            .to_string();
        assert!(
            rendered.contains("invalid calendar date"),
            "expected the calendar diagnosis for {input:?}, got: {rendered}"
        );
    }
}

// ===== Weekday offset arithmetic =====

/// The offsets are weekday numbers in `0..=6` for every caller in the
/// workspace, but this is a public helper: it used to panic on arguments near
/// `i64::MIN`/`i64::MAX` in an overflow-checked build.
#[test]
fn weekday_offsets_saturate_instead_of_panicking() {
    for current in [i64::MIN, i64::MIN + 1, -1, 0, 6, i64::MAX - 1, i64::MAX] {
        for target in [i64::MIN, -1, 0, 6, i64::MAX] {
            for modifier in [
                None,
                Some(WeekdayModifier::Next),
                Some(WeekdayModifier::This),
                Some(WeekdayModifier::Last),
            ] {
                let _ = calculate_weekday_offset(current, target, modifier);
            }
        }
    }
}

/// The in-domain answers are unchanged by the saturating rewrite.
#[test]
fn weekday_offsets_are_unchanged_for_real_weekdays() {
    // (current, target, modifier) -> days to add, Monday = 0.
    for (current, target, modifier, expected) in [
        (0, 0, None, 0),                         // Monday -> Monday
        (0, 0, Some(WeekdayModifier::Next), 7),  // next Monday
        (0, 0, Some(WeekdayModifier::Last), -7), // last Monday
        (2, 0, None, 5),                         // Wednesday -> Monday
        (2, 0, Some(WeekdayModifier::Last), -2), // last Monday
        (0, 2, Some(WeekdayModifier::Next), 2),  // next Wednesday
        (6, 0, Some(WeekdayModifier::This), -6), // this Monday, asked on Sunday
        (3, 3, Some(WeekdayModifier::This), 0),  // this Thursday, asked on Thursday
    ] {
        assert_eq!(
            calculate_weekday_offset(current, target, modifier),
            expected,
            "current={current} target={target} modifier={modifier:?}"
        );
    }
}
