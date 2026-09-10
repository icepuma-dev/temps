//! Integration tests for the jiff backend.
//!
//! Every test here drives the real [`JiffProvider`]. Time-dependent behaviour is
//! made reproducible with `JiffProvider::at(fixed)` rather than by
//! reimplementing the provider against a mock clock — a mock inevitably drifts
//! from production and hides the very bugs these tests exist to catch.

use jiff::{
    Span, Zoned,
    civil::{DateTime, date},
    tz::TimeZone,
};
use temps_core::*;
use temps_jiff::*;

/// A `Zoned` in an explicitly named IANA zone.
///
/// Zone-specific behaviour is pinned this way instead of via the `TZ`
/// environment variable, which is process-wide and would make the suite
/// order-dependent under a parallel test runner.
fn at_zone(zone: &str, y: i16, m: i8, d: i8, hour: i8, minute: i8) -> Zoned {
    date(y, m, d)
        .at(hour, minute, 0, 0)
        .in_tz(zone)
        .unwrap_or_else(|e| {
            panic!("{zone} {y}-{m}-{d} {hour}:{minute} is not a valid instant: {e}")
        })
}

/// A `Zoned` in UTC, for tests that only care about arithmetic.
fn utc(y: i16, m: i8, d: i8, hour: i8, minute: i8) -> Zoned {
    date(y, m, d)
        .at(hour, minute, 0, 0)
        .to_zoned(TimeZone::UTC)
        .unwrap()
}

fn resolve(provider: &JiffProvider, input: &str, language: Language) -> Zoned {
    let expr = parse(input, language).unwrap_or_else(|e| panic!("failed to parse {input:?}: {e}"));
    provider
        .parse_expression(expr)
        .unwrap_or_else(|e| panic!("failed to resolve {input:?}: {e}"))
}

const ALL_UNITS: [TimeUnit; 7] = [
    TimeUnit::Second,
    TimeUnit::Minute,
    TimeUnit::Hour,
    TimeUnit::Day,
    TimeUnit::Week,
    TimeUnit::Month,
    TimeUnit::Year,
];

const BOTH_DIRECTIONS: [Direction; 2] = [Direction::Past, Direction::Future];

// ===== Provider plumbing =====

#[test]
fn system_clock_provider_returns_a_plausible_now() {
    let provider = JiffProvider::new();
    let now = provider.now();
    assert!(now > Zoned::default());
}

#[test]
fn now_expression_returns_the_pinned_instant_exactly() {
    let fixed = utc(2024, 3, 15, 10, 30);
    let provider = JiffProvider::at(fixed.clone());

    assert_eq!(provider.now(), fixed);
    assert_eq!(resolve(&provider, "now", Language::English), fixed);
    assert_eq!(resolve(&provider, "jetzt", Language::German), fixed);
}

#[test]
fn pinned_provider_keeps_the_zone_of_the_instant_it_was_given() {
    let provider = JiffProvider::at(at_zone("America/New_York", 2024, 6, 15, 9, 0));
    let today = resolve(&provider, "today", Language::English);

    assert_eq!(today.time_zone().iana_name(), Some("America/New_York"));
    assert_eq!(today.date().to_string(), "2024-06-15");
    assert_eq!(today.hour(), 0);
}

// ===== Relative arithmetic =====

#[test]
fn english_relative_expressions_resolve_against_the_pinned_instant() {
    let base = utc(2024, 3, 15, 10, 30);
    let provider = JiffProvider::at(base.clone());

    let cases = vec![
        ("in 30 seconds", Span::new().seconds(30)),
        ("45 seconds ago", Span::new().seconds(-45)),
        ("in a second", Span::new().seconds(1)),
        ("in 5 minutes", Span::new().minutes(5)),
        ("10 minutes ago", Span::new().minutes(-10)),
        ("in a minute", Span::new().minutes(1)),
        ("a minute ago", Span::new().minutes(-1)),
        ("in 2 hours", Span::new().hours(2)),
        ("3 hours ago", Span::new().hours(-3)),
        ("in an hour", Span::new().hours(1)),
        ("an hour ago", Span::new().hours(-1)),
        ("in 1 day", Span::new().days(1)),
        ("2 days ago", Span::new().days(-2)),
        ("in a day", Span::new().days(1)),
        ("in one day", Span::new().days(1)),
        ("in 1 week", Span::new().weeks(1)),
        ("2 weeks ago", Span::new().weeks(-2)),
        ("in a week", Span::new().weeks(1)),
        ("one week ago", Span::new().weeks(-1)),
        ("in 1 month", Span::new().months(1)),
        ("1 month ago", Span::new().months(-1)),
        ("in 3 months", Span::new().months(3)),
        ("in 1 year", Span::new().years(1)),
        ("1 year ago", Span::new().years(-1)),
        ("in 2 years", Span::new().years(2)),
    ];

    for (input, span) in cases {
        let expected = base.checked_add(span).unwrap();
        assert_eq!(
            resolve(&provider, input, Language::English),
            expected,
            "wrong result for {input:?}"
        );
    }
}

#[test]
fn german_relative_expressions_resolve_against_the_pinned_instant() {
    let base = utc(2024, 3, 15, 10, 30);
    let provider = JiffProvider::at(base.clone());

    let cases = vec![
        ("in 30 Sekunden", Span::new().seconds(30)),
        ("vor 45 Sekunden", Span::new().seconds(-45)),
        ("in einer Sekunde", Span::new().seconds(1)),
        ("vor einer Sekunde", Span::new().seconds(-1)),
        ("in 5 Minuten", Span::new().minutes(5)),
        ("vor 10 Minuten", Span::new().minutes(-10)),
        ("in einer Minute", Span::new().minutes(1)),
        ("in 2 Stunden", Span::new().hours(2)),
        ("vor 3 Stunden", Span::new().hours(-3)),
        ("in einer Stunde", Span::new().hours(1)),
        ("vor einer Stunde", Span::new().hours(-1)),
        ("in 1 Tag", Span::new().days(1)),
        ("vor 2 Tagen", Span::new().days(-2)),
        ("in einem Tag", Span::new().days(1)),
        ("vor einem Tag", Span::new().days(-1)),
        ("in 1 Woche", Span::new().weeks(1)),
        ("vor 2 Wochen", Span::new().weeks(-2)),
        ("in einer Woche", Span::new().weeks(1)),
        ("in einem Monat", Span::new().months(1)),
        ("vor einem Monat", Span::new().months(-1)),
        ("in einem Jahr", Span::new().years(1)),
        ("vor einem Jahr", Span::new().years(-1)),
    ];

    for (input, span) in cases {
        let expected = base.checked_add(span).unwrap();
        assert_eq!(
            resolve(&provider, input, Language::German),
            expected,
            "wrong result for {input:?}"
        );
    }
}

#[test]
fn zero_day_offset_is_an_exact_identity() {
    // "in 0 days" must not drift: no rounding to midnight, no DST nudge, and
    // the identity has to hold in a zone that is mid-transition on the day.
    for base in [
        utc(2024, 3, 15, 10, 30),
        at_zone("America/New_York", 2024, 3, 9, 23, 30),
        at_zone("America/New_York", 2024, 11, 3, 12, 0),
    ] {
        let provider = JiffProvider::at(base.clone());

        for input in ["in 0 days", "in 0 seconds", "in 0 months", "in 0 years"] {
            assert_eq!(
                resolve(&provider, input, Language::English),
                base,
                "{input:?} moved the instant in {}",
                base.time_zone().iana_name().unwrap_or("UTC")
            );
        }

        assert_eq!(resolve(&provider, "0 days ago", Language::English), base);
    }
}

// ===== Calendar-aware month and year arithmetic =====

#[test]
fn adding_a_month_clamps_to_the_end_of_a_shorter_month() {
    let leap = JiffProvider::at(utc(2024, 1, 31, 10, 0));
    let result = resolve(&leap, "in 1 month", Language::English);
    assert_eq!(result.date().to_string(), "2024-02-29");

    let non_leap = JiffProvider::at(utc(2023, 1, 31, 10, 0));
    let result = resolve(&non_leap, "in einem Monat", Language::German);
    assert_eq!(result.date().to_string(), "2023-02-28");
}

#[test]
fn adding_a_year_to_a_leap_day_lands_on_february_28() {
    let provider = JiffProvider::at(utc(2024, 2, 29, 12, 0));
    let result = resolve(&provider, "in 1 year", Language::English);
    assert_eq!(result.date().to_string(), "2025-02-28");
}

#[test]
fn month_arithmetic_crosses_the_year_boundary() {
    let provider = JiffProvider::at(utc(2023, 10, 15, 9, 0));
    let result = resolve(&provider, "in 6 months", Language::English);
    assert_eq!(result.date().to_string(), "2024-04-15");

    let result = resolve(&provider, "6 months ago", Language::English);
    assert_eq!(result.date().to_string(), "2023-04-15");

    let result = resolve(&provider, "in 18 months", Language::English);
    assert_eq!(result.date().to_string(), "2025-04-15");
}

#[test]
fn relative_expressions_move_in_the_requested_direction() {
    let base = utc(2024, 3, 15, 10, 30);
    let provider = JiffProvider::at(base.clone());

    for unit in ALL_UNITS {
        let future = provider
            .parse_expression(TimeExpression::Relative(RelativeTime {
                amount: 1,
                unit,
                direction: Direction::Future,
            }))
            .unwrap();
        let past = provider
            .parse_expression(TimeExpression::Relative(RelativeTime {
                amount: 1,
                unit,
                direction: Direction::Past,
            }))
            .unwrap();

        assert!(future > base, "1 {unit:?} into the future did not advance");
        assert!(past < base, "1 {unit:?} into the past did not go back");
    }
}

// ===== Overflow: parseable input must never panic =====

#[test]
fn huge_amounts_overflow_instead_of_panicking_for_every_unit_and_direction() {
    // `Span`'s plain setters panic outside jiff's per-unit range, and the
    // grammar accepts any digit run up to i64::MAX, so an unchecked builder
    // here would abort the process from an API that returns `Result`.
    let provider = JiffProvider::at(utc(2024, 3, 15, 10, 30));

    for unit in ALL_UNITS {
        for direction in BOTH_DIRECTIONS {
            for amount in [i64::MAX, i64::MAX - 1, 999_999_999_999] {
                let result = provider.parse_expression(TimeExpression::Relative(RelativeTime {
                    amount,
                    unit,
                    direction,
                }));

                assert!(
                    matches!(result, Err(TempsError::ArithmeticOverflow { .. })),
                    "{amount} {unit:?} {direction:?} should overflow cleanly, got {result:?}"
                );
            }
        }
    }
}

#[test]
fn huge_amounts_from_parsed_text_overflow_instead_of_panicking() {
    let provider = JiffProvider::at(utc(2024, 3, 15, 10, 30));

    let inputs = [
        ("999999999999 days ago", Language::English),
        ("in 999999999999 days", Language::English),
        ("9223372036854775807 seconds ago", Language::English),
        ("in 9223372036854775807 years", Language::English),
        ("vor 999999999999 Tagen", Language::German),
        ("in 999999999999 Wochen", Language::German),
    ];

    for (input, language) in inputs {
        let expr = parse(input, language).unwrap_or_else(|e| panic!("{input:?} must parse: {e}"));
        let result = provider.parse_expression(expr);
        assert!(
            matches!(result, Err(TempsError::ArithmeticOverflow { .. })),
            "{input:?} should overflow cleanly, got {result:?}"
        );
    }
}

#[test]
fn amounts_within_span_range_but_beyond_the_calendar_fail_without_panicking() {
    // Large-but-representable spans get past the `Span` builder and have to be
    // rejected by the arithmetic instead.
    let provider = JiffProvider::at(utc(2024, 3, 15, 10, 30));

    let result = provider.parse_expression(TimeExpression::Relative(RelativeTime {
        amount: 100_000,
        unit: TimeUnit::Year,
        direction: Direction::Future,
    }));
    assert!(
        result.is_err(),
        "100000 years should not resolve, got {result:?}"
    );
}

// ===== Absolute times =====

#[test]
fn an_hour_without_a_minute_is_honoured_rather_than_collapsing_to_midnight() {
    let provider = JiffProvider::new();

    let result = provider
        .parse_expression(TimeExpression::Absolute(AbsoluteTime {
            year: 2024,
            month: 6,
            day: 15,
            hour: Some(14),
            minute: None,
            second: None,
            nanosecond: None,
            timezone: None,
        }))
        .expect("an absolute time with only an hour should resolve");

    assert_eq!(result.date().to_string(), "2024-06-15");
    assert_eq!(result.hour(), 14, "the supplied hour was dropped");
    assert_eq!(result.minute(), 0);
    assert_eq!(result.second(), 0);
}

#[test]
fn an_hour_without_a_minute_is_honoured_in_an_explicit_utc_offset() {
    let provider = JiffProvider::new();

    let result = provider
        .parse_expression(TimeExpression::Absolute(AbsoluteTime {
            year: 2024,
            month: 6,
            day: 15,
            hour: Some(14),
            minute: None,
            second: None,
            nanosecond: None,
            timezone: Some(Timezone::Utc),
        }))
        .expect("an absolute UTC time with only an hour should resolve");

    let expected = utc(2024, 6, 15, 14, 0);
    assert_eq!(
        result.timestamp(),
        expected.timestamp(),
        "the supplied hour was dropped"
    );
}

#[test]
fn a_minute_without_an_hour_is_an_error() {
    let provider = JiffProvider::new();

    let result = provider.parse_expression(TimeExpression::Absolute(AbsoluteTime {
        year: 2024,
        month: 6,
        day: 15,
        hour: None,
        minute: Some(30),
        second: None,
        nanosecond: None,
        timezone: None,
    }));

    assert!(
        matches!(result, Err(TempsError::InvalidTime { minute: 30, .. })),
        "a minute with no hour should be rejected, got {result:?}"
    );

    // A second or a nanosecond without an hour is the same shape, and used to be
    // silently discarded: the value resolved to midnight with the supplied
    // component dropped.
    for (second, nanosecond) in [
        (Some(30), None),
        (None, Some(500_000_000)),
        (Some(59), Some(999_999_999)),
    ] {
        let sub_hour_without_hour =
            provider.parse_expression(TimeExpression::Absolute(AbsoluteTime {
                year: 2024,
                month: 6,
                day: 15,
                hour: None,
                minute: None,
                second,
                nanosecond,
                timezone: Some(Timezone::Utc),
            }));
        assert!(
            matches!(sub_hour_without_hour, Err(TempsError::InvalidTime { .. })),
            "second={second:?} nanosecond={nanosecond:?} with no hour should be rejected, \
             got {sub_hour_without_hour:?}"
        );
    }

    // With an hour the same component is honoured, so the guard is about the
    // missing hour rather than about the field.
    let honoured = provider
        .parse_expression(TimeExpression::Absolute(AbsoluteTime {
            year: 2024,
            month: 6,
            day: 15,
            hour: Some(0),
            minute: None,
            second: Some(30),
            nanosecond: None,
            timezone: Some(Timezone::Utc),
        }))
        .expect("a second with an hour is a valid time");
    assert_eq!(honoured.timestamp().to_string(), "2024-06-15T00:00:30Z");
}

#[test]
fn a_date_only_absolute_time_is_local_midnight() {
    let provider = JiffProvider::new();

    let result = provider
        .parse_expression(TimeExpression::Absolute(AbsoluteTime {
            year: 2024,
            month: 6,
            day: 15,
            hour: None,
            minute: None,
            second: None,
            nanosecond: None,
            timezone: None,
        }))
        .unwrap();

    assert_eq!(result.date().to_string(), "2024-06-15");
    assert_eq!(result.hour(), 0);
    assert_eq!(result.minute(), 0);
}

#[test]
fn rfc3339_input_round_trips_to_the_same_instant() {
    let provider = JiffProvider::new();

    let cases = [
        ("2024-01-15T14:30:00Z", "2024-01-15T14:30:00Z"),
        ("2024-01-15T14:30:00+02:00", "2024-01-15T12:30:00Z"),
        ("2024-01-15T14:30:00.123Z", "2024-01-15T14:30:00.123Z"),
    ];

    for (input, expected) in cases {
        let resolved = resolve(&provider, input, Language::English);
        let expected: jiff::Timestamp = expected.parse().unwrap();
        assert_eq!(
            resolved.timestamp(),
            expected,
            "wrong instant for {input:?}"
        );
    }
}

// ===== Day references around DST transitions =====

#[test]
fn day_references_use_calendar_days_across_spring_forward() {
    // 23:30 the evening before a spring-forward: a fixed 24-hour step would
    // skip a day, because the following local day is only 23 hours long.
    let provider = JiffProvider::at(at_zone("America/New_York", 2024, 3, 9, 23, 30));

    let cases = [
        ("today", "2024-03-09"),
        ("tomorrow", "2024-03-10"),
        ("yesterday", "2024-03-08"),
        ("the day after tomorrow", "2024-03-11"),
        ("the day before yesterday", "2024-03-07"),
    ];

    for (input, expected) in cases {
        let resolved = resolve(&provider, input, Language::English);
        assert_eq!(
            resolved.date().to_string(),
            expected,
            "wrong date for {input:?}"
        );
        assert_eq!(resolved.hour(), 0, "{input:?} should be local midnight");
        assert_eq!(resolved.time_zone().iana_name(), Some("America/New_York"));
    }
}

#[test]
fn day_references_use_calendar_days_across_fall_back() {
    // The local day 2024-11-03 in New York is 25 hours long.
    let provider = JiffProvider::at(at_zone("America/New_York", 2024, 11, 3, 23, 30));

    let cases = [
        ("today", "2024-11-03"),
        ("tomorrow", "2024-11-04"),
        ("yesterday", "2024-11-02"),
        ("the day after tomorrow", "2024-11-05"),
        ("the day before yesterday", "2024-11-01"),
    ];

    for (input, expected) in cases {
        let resolved = resolve(&provider, input, Language::English);
        assert_eq!(
            resolved.date().to_string(),
            expected,
            "wrong date for {input:?}"
        );
        assert_eq!(resolved.hour(), 0, "{input:?} should be local midnight");
    }
}

#[test]
fn german_day_references_use_calendar_days_across_the_eu_transition() {
    // Europe/Berlin springs forward on 2024-03-31 at 02:00 local.
    let provider = JiffProvider::at(at_zone("Europe/Berlin", 2024, 3, 30, 23, 30));

    // The German grammar has no words for the two-day references, so those are
    // covered programmatically here and by their English spellings above.
    let cases = [
        ("heute", "2024-03-30"),
        ("morgen", "2024-03-31"),
        ("gestern", "2024-03-29"),
    ];

    for (input, expected) in cases {
        let resolved = resolve(&provider, input, Language::German);
        assert_eq!(
            resolved.date().to_string(),
            expected,
            "wrong date for {input:?}"
        );
        assert_eq!(resolved.hour(), 0, "{input:?} should be local midnight");
    }

    for (day_ref, expected) in [
        (DayReference::DayAfterTomorrow, "2024-04-01"),
        (DayReference::DayBeforeYesterday, "2024-03-28"),
    ] {
        let resolved = provider
            .parse_expression(TimeExpression::Day(day_ref))
            .unwrap();
        assert_eq!(
            resolved.date().to_string(),
            expected,
            "wrong date for {day_ref:?}"
        );
        assert_eq!(resolved.hour(), 0);
    }
}

#[test]
fn a_nonexistent_local_midnight_shifts_forward_by_the_gap() {
    // Cuba springs forward at midnight: 2024-03-10 00:00 does not exist and
    // 01:00 is the first instant of the day.
    let provider = JiffProvider::at(at_zone("America/Havana", 2024, 3, 10, 12, 0));

    let today = resolve(&provider, "today", Language::English);
    assert_eq!(today.date().to_string(), "2024-03-10");
    assert_eq!(
        today.hour(),
        1,
        "a nonexistent midnight should shift forward by the gap"
    );

    // Reaching the same day from the day before must agree.
    let eve = JiffProvider::at(at_zone("America/Havana", 2024, 3, 9, 22, 0));
    assert_eq!(resolve(&eve, "tomorrow", Language::English), today);
}

#[test]
fn an_ambiguous_local_midnight_resolves_to_the_earlier_instant() {
    // Cuba falls back at 01:00 on 2024-11-03, so local midnight occurs twice.
    let provider = JiffProvider::at(at_zone("America/Havana", 2024, 11, 3, 12, 0));

    let today = resolve(&provider, "today", Language::English);
    assert_eq!(today.date().to_string(), "2024-11-03");
    assert_eq!(today.hour(), 0);
    assert_eq!(
        today.offset(),
        jiff::tz::offset(-4),
        "an ambiguous midnight should pick the earlier (pre-transition) instant"
    );
}

#[test]
fn a_day_skipped_at_the_date_line_does_not_error() {
    // Pacific/Apia jumped straight from 2011-12-29 to 2011-12-31; the whole
    // local day 2011-12-30 is missing.
    let provider = JiffProvider::at(at_zone("Pacific/Apia", 2011, 12, 29, 12, 0));

    let tomorrow = provider
        .parse_expression(TimeExpression::Day(DayReference::Tomorrow))
        .expect("a skipped calendar day must not be an error");

    assert_eq!(
        tomorrow.date().to_string(),
        "2011-12-31",
        "the whole skipped day should shift forward by the gap"
    );
}

// ===== Weekdays =====

#[test]
fn weekday_references_resolve_relative_to_the_pinned_day() {
    // 2024-03-15 is a Friday.
    let provider = JiffProvider::at(utc(2024, 3, 15, 10, 30));

    let cases = [
        ("monday", "2024-03-18"),
        ("next monday", "2024-03-18"),
        ("last monday", "2024-03-11"),
        ("friday", "2024-03-15"),
        ("next friday", "2024-03-22"),
        ("last friday", "2024-03-08"),
        ("sunday", "2024-03-17"),
    ];

    for (input, expected) in cases {
        let resolved = resolve(&provider, input, Language::English);
        assert_eq!(
            resolved.date().to_string(),
            expected,
            "wrong date for {input:?}"
        );
        assert_eq!(resolved.hour(), 0);
    }
}

#[test]
fn weekday_references_stay_on_calendar_days_across_a_transition() {
    // Friday 2024-03-08 in New York; the following Sunday is the short day.
    let provider = JiffProvider::at(at_zone("America/New_York", 2024, 3, 8, 20, 0));

    let cases = [
        ("sunday", "2024-03-10"),
        ("monday", "2024-03-11"),
        ("next friday", "2024-03-15"),
    ];

    for (input, expected) in cases {
        let resolved = resolve(&provider, input, Language::English);
        assert_eq!(
            resolved.date().to_string(),
            expected,
            "wrong date for {input:?}"
        );
        assert_eq!(resolved.hour(), 0);
    }
}

// ===== Times and day-at-time =====

#[test]
fn times_resolve_on_the_pinned_day() {
    let provider = JiffProvider::at(utc(2024, 3, 15, 10, 30));

    let cases = [
        ("3:30 pm", 15, 30),
        ("10:15 am", 10, 15),
        ("14:30", 14, 30),
        ("9:00 PM", 21, 0),
        ("12:00 PM", 12, 0),
        ("12:00 AM", 0, 0),
    ];

    for (input, hour, minute) in cases {
        let resolved = resolve(&provider, input, Language::English);
        assert_eq!(resolved.date().to_string(), "2024-03-15");
        assert_eq!(resolved.hour(), hour, "wrong hour for {input:?}");
        assert_eq!(resolved.minute(), minute, "wrong minute for {input:?}");
    }
}

#[test]
fn day_at_time_combines_the_calendar_day_with_the_time() {
    let provider = JiffProvider::at(utc(2024, 3, 15, 10, 30));

    let resolved = resolve(&provider, "tomorrow at 3:30 pm", Language::English);
    assert_eq!(resolved.date().to_string(), "2024-03-16");
    assert_eq!(resolved.hour(), 15);
    assert_eq!(resolved.minute(), 30);

    let resolved = resolve(&provider, "next monday at 9:00 am", Language::English);
    assert_eq!(resolved.date().to_string(), "2024-03-18");
    assert_eq!(resolved.hour(), 9);
    assert_eq!(resolved.minute(), 0);
}

#[test]
fn day_at_time_across_spring_forward_keeps_the_calendar_day() {
    let provider = JiffProvider::at(at_zone("America/New_York", 2024, 3, 9, 23, 30));

    let resolved = resolve(&provider, "tomorrow at 3:30 pm", Language::English);
    assert_eq!(resolved.date().to_string(), "2024-03-10");
    assert_eq!(resolved.hour(), 15);
    assert_eq!(resolved.minute(), 30);
    assert_eq!(resolved.offset(), jiff::tz::offset(-4), "should be on DST");
}

// ===== later today =====

#[test]
fn later_today_advances_two_hours_when_that_stays_on_the_same_day() {
    let base = utc(2024, 3, 15, 10, 30);
    let provider = JiffProvider::at(base.clone());

    let resolved = resolve(&provider, "later today", Language::English);
    assert_eq!(resolved, base.checked_add(Span::new().hours(2)).unwrap());
}

#[test]
fn later_today_never_leaves_today_and_never_goes_backwards() {
    let base = utc(2024, 3, 15, 23, 30);
    let provider = JiffProvider::at(base.clone());

    let resolved = resolve(&provider, "later today", Language::English);
    assert!(resolved >= base, "later today must not move into the past");
    assert_eq!(
        resolved.date().to_string(),
        "2024-03-15",
        "later today must not cross midnight"
    );
}

// ===== Calendar dates =====

#[test]
fn calendar_dates_resolve_to_local_midnight() {
    let provider = JiffProvider::new();

    let cases = [
        ("15/03/2024", "2024-03-15"),
        ("31-12-2025", "2025-12-31"),
        ("01/01/2023", "2023-01-01"),
    ];

    for (input, expected) in cases {
        let resolved = resolve(&provider, input, Language::English);
        assert_eq!(resolved.date().to_string(), expected);
        assert_eq!(resolved.hour(), 0);
        assert_eq!(resolved.minute(), 0);
    }
}

// ===== Programmatic rejection =====

#[test]
fn invalid_programmatic_inputs_are_rejected() {
    let provider = JiffProvider::new();

    let invalid_time = TimeExpression::Time(Time {
        hour: 0,
        minute: 30,
        second: 0,
        meridiem: Some(Meridiem::PM),
    });
    assert!(matches!(
        provider.parse_expression(invalid_time),
        Err(TempsError::InvalidTime { hour: 0, .. })
    ));

    let invalid_timezone = TimeExpression::Absolute(AbsoluteTime {
        year: 2024,
        month: 1,
        day: 15,
        hour: Some(12),
        minute: Some(0),
        second: Some(0),
        nanosecond: None,
        timezone: Some(Timezone::Offset {
            total_minutes: -750,
        }),
    });
    assert!(matches!(
        provider.parse_expression(invalid_timezone),
        Err(TempsError::InvalidTimezoneOffset {
            total_minutes: -750
        })
    ));

    let negative_relative = TimeExpression::Relative(RelativeTime {
        amount: -1,
        unit: TimeUnit::Hour,
        direction: Direction::Future,
    });
    assert!(matches!(
        provider.parse_expression(negative_relative),
        Err(TempsError::DateCalculationError { .. })
    ));
}

#[test]
fn the_convenience_function_uses_the_system_clock() {
    let before = Zoned::now();
    let resolved = parse_to_zoned("now", Language::English).unwrap();
    let after = Zoned::now();

    assert!(resolved >= before && resolved <= after);

    // And it still resolves the whole grammar.
    for (input, language) in [
        ("tomorrow", Language::English),
        ("in 5 minutes", Language::English),
        ("morgen um 15:30", Language::German),
    ] {
        assert!(
            parse_to_zoned(input, language).is_ok(),
            "{input:?} should resolve"
        );
    }
}

#[test]
fn civil_datetime_input_is_accepted_by_the_pinned_provider() {
    // Guards the `DateTime` -> `Zoned` construction used to pin the provider.
    let fixed = DateTime::constant(2024, 3, 15, 10, 30, 0, 0)
        .to_zoned(TimeZone::UTC)
        .unwrap();
    let provider = JiffProvider::at(fixed.clone());
    assert_eq!(provider.now(), fixed);
}

// ===== Pin, zone, and error-message contracts =====

/// A pinned provider answers every zone-less expression in its own zone.
///
/// The absolute and bare-date arms used to resolve against `TimeZone::system()`
/// while the relative, day and time arms used the pinned zone. One provider
/// could therefore return two instants nine hours apart for the same wall
/// clock, and the answer depended on the process `TZ` rather than on the
/// instant it had been pinned to — which is exactly what `at` promises not to
/// happen.
#[test]
fn a_pinned_provider_resolves_every_expression_in_its_own_zone() {
    let provider = JiffProvider::at(at_zone("Asia/Tokyo", 2024, 6, 1, 10, 0));

    // The same wall clock, named two ways: a day reference with a time, and an
    // absolute datetime.
    assert_eq!(
        resolve(&provider, "tomorrow at 9:00 am", Language::English).timestamp(),
        resolve(&provider, "2024-06-02 09:00", Language::English).timestamp(),
    );
    assert_eq!(
        resolve(&provider, "tomorrow", Language::English).timestamp(),
        resolve(&provider, "2024-06-02", Language::English).timestamp(),
    );
    assert_eq!(
        resolve(&provider, "today", Language::English).timestamp(),
        resolve(&provider, "2024-06-01", Language::English).timestamp(),
    );

    // Every answer is labelled with the pinned zone — including the ones that
    // name their own offset, which are converted into it.
    for input in [
        "tomorrow at 9:00 am",
        "2024-06-02 09:00",
        "tomorrow",
        "2024-06-02",
        "2024-06-02T09:00:00Z",
        "in 2 hours",
        "9:00 am",
    ] {
        assert_eq!(
            resolve(&provider, input, Language::English)
                .time_zone()
                .iana_name(),
            Some("Asia/Tokyo"),
            "wrong zone for {input:?}"
        );
    }
}

/// A date with no time still honours the zone the expression names: the
/// timezone field used to be read only when an hour was present.
#[test]
fn a_date_only_absolute_honours_its_timezone() {
    let midnight_in = |timezone| {
        JiffProvider::new()
            .parse_expression(TimeExpression::Absolute(AbsoluteTime {
                year: 2024,
                month: 1,
                day: 15,
                hour: None,
                minute: None,
                second: None,
                nanosecond: None,
                timezone,
            }))
            .expect("a real date resolves")
            .timestamp()
    };

    assert_eq!(
        midnight_in(Some(Timezone::Utc)).to_string(),
        "2024-01-15T00:00:00Z"
    );
    assert_eq!(
        midnight_in(Some(Timezone::Offset {
            total_minutes: -300
        }))
        .to_string(),
        "2024-01-15T05:00:00Z",
        "-05:00 midnight is five hours after UTC midnight"
    );
    assert_eq!(
        midnight_in(Some(Timezone::Offset { total_minutes: 330 })).to_string(),
        "2024-01-14T18:30:00Z",
        "+05:30 midnight is the previous UTC day"
    );
}

/// A range-limit failure used to print "Date calculation error: Date
/// calculation error", with jiff's actual diagnosis captured in a field that
/// was never rendered.
#[test]
fn a_range_limit_failure_explains_itself_once() {
    let provider = JiffProvider::at(utc(2024, 6, 1, 10, 0));
    let error = provider
        .parse_expression(parse("in 10000 years", Language::English).expect("the input parses"))
        .expect_err("10000 years is beyond jiff's range");

    let rendered = error.to_string();
    assert!(
        !rendered.contains("Date calculation error: Date calculation error"),
        "the category must not be repeated: {rendered}"
    );
    assert!(
        rendered.starts_with("Date calculation error: "),
        "the message should still name the category: {rendered}"
    );
    assert!(
        rendered.contains("range"),
        "the message should carry the backend's own cause: {rendered}"
    );
}
