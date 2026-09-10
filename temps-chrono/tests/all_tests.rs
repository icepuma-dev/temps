//! Integration tests for the chrono backend.
//!
//! Every test drives the real [`ChronoProvider`]. Time-dependent behaviour is
//! pinned with `ChronoProvider::at(...)` instead of being reimplemented against
//! a mock: a mock that duplicates the provider's arithmetic can only ever
//! assert that the duplicate is self-consistent, and drifts away from the code
//! it is supposed to guard.

use chrono::{
    DateTime, Datelike, Days, Duration, Local, NaiveDate, Offset, TimeZone, Timelike, Utc,
};
use temps_chrono::{ChronoProvider, parse_to_datetime};
use temps_core::*;
use temps_testhelpers::chrono::{fixed_datetime, test_dates};

// ===== Helpers =====

/// Parse `input` and resolve it against `provider`, panicking with the input on
/// failure so a broken case names itself.
fn resolve(provider: &ChronoProvider, input: &str, language: Language) -> DateTime<Local> {
    let expr =
        parse(input, language).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"));
    provider
        .parse_expression(expr)
        .unwrap_or_else(|error| panic!("failed to resolve {input:?}: {error}"))
}

/// Resolve `input` without unwrapping, for cases expected to fail.
fn try_resolve(
    provider: &ChronoProvider,
    input: &str,
    language: Language,
) -> Result<DateTime<Local>> {
    provider.parse_expression(parse(input, language)?)
}

/// The local instant whose UTC reading is the given clock time.
///
/// Naming an instant by its UTC reading is unambiguous in every zone, which
/// matters where the local wall clock is ambiguous or skipped.
fn instant_at_utc(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Local> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
        .single()
        .expect("UTC clock reading is always unambiguous")
        .with_timezone(&Local)
}

/// A mid-June instant, far from any zone's daylight-saving transition, for
/// tests whose expectations must hold in whatever zone the suite runs in.
fn quiet_instant() -> DateTime<Local> {
    fixed_datetime(2024, 6, 12, 10, 30, 15)
}

fn every_unit() -> [TimeUnit; 7] {
    [
        TimeUnit::Second,
        TimeUnit::Minute,
        TimeUnit::Hour,
        TimeUnit::Day,
        TimeUnit::Week,
        TimeUnit::Month,
        TimeUnit::Year,
    ]
}

// ===== Provider basics =====

#[test]
fn default_provider_reads_the_system_clock() {
    let provider = ChronoProvider::new();
    let before = Local::now();
    let now = provider.now();
    let after = Local::now();

    assert!(now >= before && now <= after, "now() must read the clock");

    let parsed = provider.parse_expression(TimeExpression::Now).unwrap();
    assert!(
        parsed.signed_duration_since(now).num_seconds().abs() < 1,
        "'now' should resolve within a second of the clock"
    );
}

#[test]
fn pinned_provider_resolves_now_to_its_own_instant() {
    let base = quiet_instant();
    let provider = ChronoProvider::at(base);

    assert_eq!(provider.now(), base);
    assert_eq!(
        provider.parse_expression(TimeExpression::Now).unwrap(),
        base
    );
    assert_eq!(resolve(&provider, "now", Language::English), base);
    assert_eq!(resolve(&provider, "jetzt", Language::German), base);
}

// ===== Relative expressions =====

#[test]
fn english_sub_day_offsets_shift_by_an_exact_duration() {
    let base = quiet_instant();
    let provider = ChronoProvider::at(base);

    let cases = [
        ("in 30 seconds", Duration::seconds(30)),
        ("45 seconds ago", Duration::seconds(-45)),
        ("in 5 minutes", Duration::minutes(5)),
        ("10 minutes ago", Duration::minutes(-10)),
        ("in a minute", Duration::minutes(1)),
        ("in 2 hours", Duration::hours(2)),
        ("3 hours ago", Duration::hours(-3)),
        ("an hour ago", Duration::hours(-1)),
        ("in a second", Duration::seconds(1)),
        ("in an hour", Duration::hours(1)),
        ("a minute ago", Duration::minutes(-1)),
    ];

    for (input, offset) in cases {
        assert_eq!(
            resolve(&provider, input, Language::English),
            base + offset,
            "wrong result for {input:?}"
        );
    }
}

#[test]
fn german_sub_day_offsets_shift_by_an_exact_duration() {
    let base = quiet_instant();
    let provider = ChronoProvider::at(base);

    let cases = [
        ("in 30 Sekunden", Duration::seconds(30)),
        ("vor 45 Sekunden", Duration::seconds(-45)),
        ("in 5 Minuten", Duration::minutes(5)),
        ("vor 10 Minuten", Duration::minutes(-10)),
        ("in einer Minute", Duration::minutes(1)),
        ("in 2 Stunden", Duration::hours(2)),
        ("vor 3 Stunden", Duration::hours(-3)),
        ("in einer Sekunde", Duration::seconds(1)),
        ("in einer Stunde", Duration::hours(1)),
        ("vor einer Sekunde", Duration::seconds(-1)),
    ];

    for (input, offset) in cases {
        assert_eq!(
            resolve(&provider, input, Language::German),
            base + offset,
            "wrong result for {input:?}"
        );
    }
}

/// Day and week offsets are calendar arithmetic, not fixed multiples of 24
/// hours: the wall-clock time is carried over to the target date.
#[test]
fn day_and_week_offsets_move_the_calendar_date_and_keep_the_wall_clock() {
    let base = quiet_instant();
    let provider = ChronoProvider::at(base);

    let cases = [
        ("in 1 day", Language::English, 1_i64),
        ("2 days ago", Language::English, -2),
        ("in a day", Language::English, 1),
        ("in one day", Language::English, 1),
        ("a day ago", Language::English, -1),
        ("a couple of days ago", Language::English, -2),
        ("in 1 week", Language::English, 7),
        ("2 weeks ago", Language::English, -14),
        ("in a week", Language::English, 7),
        ("one week ago", Language::English, -7),
        ("in 1 Tag", Language::German, 1),
        ("vor 2 Tagen", Language::German, -2),
        ("in einem Tag", Language::German, 1),
        ("vor einem Tag", Language::German, -1),
        ("in 1 Woche", Language::German, 7),
        ("vor 2 Wochen", Language::German, -14),
        ("in einer Woche", Language::German, 7),
    ];

    for (input, language, days) in cases {
        let result = resolve(&provider, input, language);
        let expected_date = if days >= 0 {
            base.date_naive().checked_add_days(Days::new(days as u64))
        } else {
            base.date_naive()
                .checked_sub_days(Days::new(days.unsigned_abs()))
        }
        .expect("date within range");

        assert_eq!(
            result.date_naive(),
            expected_date,
            "wrong date for {input:?}"
        );
        assert_eq!(result.time(), base.time(), "wall clock moved for {input:?}");
    }
}

#[test]
fn month_arithmetic_clamps_to_the_end_of_a_shorter_month() {
    // January 31 + 1 month lands on the last day of February.
    let provider = ChronoProvider::at(test_dates::jan_31_2024());
    let result = resolve(&provider, "in 1 month", Language::English);
    assert_eq!(
        (result.year(), result.month(), result.day()),
        (2024, 2, 29),
        "2024 is a leap year"
    );

    let provider = ChronoProvider::at(fixed_datetime(2023, 1, 31, 10, 0, 0));
    let result = resolve(&provider, "in einem Monat", Language::German);
    assert_eq!((result.year(), result.month(), result.day()), (2023, 2, 28));
}

#[test]
fn year_arithmetic_clamps_a_leap_day() {
    let provider = ChronoProvider::at(test_dates::feb_29_2024());
    let result = resolve(&provider, "in 1 year", Language::English);
    assert_eq!((result.year(), result.month(), result.day()), (2025, 2, 28));

    let result = resolve(&provider, "1 year ago", Language::English);
    assert_eq!((result.year(), result.month(), result.day()), (2023, 2, 28));
}

#[test]
fn month_arithmetic_crosses_the_year_boundary() {
    let provider = ChronoProvider::at(fixed_datetime(2023, 10, 15, 9, 0, 0));

    let result = resolve(&provider, "in 6 months", Language::English);
    assert_eq!((result.year(), result.month(), result.day()), (2024, 4, 15));

    let result = resolve(&provider, "in 18 months", Language::English);
    assert_eq!((result.year(), result.month(), result.day()), (2025, 4, 15));

    let result = resolve(&provider, "6 months ago", Language::English);
    assert_eq!((result.year(), result.month(), result.day()), (2023, 4, 15));

    let result = resolve(&provider, "in 5 years", Language::English);
    assert_eq!(
        (result.year(), result.month(), result.day()),
        (2028, 10, 15)
    );

    let result = resolve(&provider, "10 years ago", Language::English);
    assert_eq!(
        (result.year(), result.month(), result.day()),
        (2013, 10, 15)
    );
}

/// A zero amount is the identity, down to the nanosecond — no rounding to the
/// second, no trip through a date and back.
#[test]
fn a_zero_amount_resolves_to_exactly_now() {
    let base = quiet_instant()
        .with_nanosecond(123_456_789)
        .expect("valid nanosecond");
    let provider = ChronoProvider::at(base);

    for input in [
        "in 0 seconds",
        "0 seconds ago",
        "in 0 minutes",
        "in 0 hours",
        "in 0 days",
        "0 days ago",
        "in 0 weeks",
        "in 0 months",
        "in 0 years",
    ] {
        let result = resolve(&provider, input, Language::English);
        assert_eq!(result, base, "{input:?} should be the identity");
        assert_eq!(
            result.nanosecond(),
            base.nanosecond(),
            "{input:?} dropped sub-second precision"
        );
    }
}

// ===== Overflow =====

/// `digit_number` accepts any run of digits, so an amount far outside chrono's
/// range reaches the provider. It must come back as an error: this is a
/// `Result`-returning API and used to abort the process instead.
#[test]
fn huge_amounts_report_overflow_for_every_unit_and_direction() {
    let provider = ChronoProvider::at(quiet_instant());

    for unit in every_unit() {
        for direction in [Direction::Past, Direction::Future] {
            let expr = TimeExpression::Relative(RelativeTime {
                amount: i64::MAX,
                unit,
                direction,
            });

            let result = provider.parse_expression(expr);
            assert!(
                matches!(result, Err(TempsError::ArithmeticOverflow { .. })),
                "expected overflow for i64::MAX {unit:?} {direction:?}, got {result:?}"
            );
        }
    }
}

#[test]
fn huge_amounts_in_parsed_input_report_overflow_rather_than_panicking() {
    let provider = ChronoProvider::at(quiet_instant());

    let cases = [
        ("999999999999 days ago", Language::English),
        ("in 999999999999 days", Language::English),
        ("999999999999 weeks ago", Language::English),
        ("in 999999999999 weeks", Language::English),
        ("999999999999 months ago", Language::English),
        ("in 999999999999 months", Language::English),
        ("999999999999 years ago", Language::English),
        ("in 999999999999 years", Language::English),
        ("999999999999 hours ago", Language::English),
        ("in 999999999999 hours", Language::English),
        ("999999999999 minutes ago", Language::English),
        ("999999999999999 seconds ago", Language::English),
        ("in 999999999999999 seconds", Language::English),
        ("in 9223372036854775807 days", Language::English),
        ("vor 999999999999 Tagen", Language::German),
        ("in 999999999999 Wochen", Language::German),
    ];

    for (input, language) in cases {
        let result = try_resolve(&provider, input, language);
        assert!(
            matches!(result, Err(TempsError::ArithmeticOverflow { .. })),
            "expected overflow for {input:?}, got {result:?}"
        );
    }

    // The same must hold for the convenience entry point, which resolves
    // against the system clock.
    assert!(matches!(
        parse_to_datetime("999999999999 days ago", Language::English),
        Err(TempsError::ArithmeticOverflow { .. })
    ));
}

// ===== Day references =====

#[test]
fn day_references_resolve_to_midnight_of_the_right_date() {
    let base = quiet_instant();
    let provider = ChronoProvider::at(base);

    let cases = [
        ("today", Language::English, 0_i64),
        ("yesterday", Language::English, -1),
        ("tomorrow", Language::English, 1),
        ("day after tomorrow", Language::English, 2),
        ("the day before yesterday", Language::English, -2),
        ("heute", Language::German, 0),
        ("gestern", Language::German, -1),
        ("morgen", Language::German, 1),
    ];

    for (input, language, days) in cases {
        let result = resolve(&provider, input, language);
        let expected_date = if days >= 0 {
            base.date_naive().checked_add_days(Days::new(days as u64))
        } else {
            base.date_naive()
                .checked_sub_days(Days::new(days.unsigned_abs()))
        }
        .expect("date within range");

        assert_eq!(
            result.date_naive(),
            expected_date,
            "wrong date for {input:?}"
        );
        assert_eq!(
            (result.hour(), result.minute(), result.second()),
            (0, 0, 0),
            "{input:?} should be midnight"
        );
    }
}

#[test]
fn weekday_references_resolve_to_the_expected_dates() {
    // 2024-06-12 is a Wednesday.
    let provider = ChronoProvider::at(quiet_instant());

    let cases = [
        ("monday", 17),
        ("tuesday", 18),
        ("wednesday", 12),
        ("thursday", 13),
        ("friday", 14),
        ("saturday", 15),
        ("sunday", 16),
        ("mon", 17),
        ("tue", 18),
        ("wed", 12),
        ("thu", 13),
        ("fri", 14),
        ("sat", 15),
        ("sun", 16),
        ("next wednesday", 19),
        ("last wednesday", 5),
        ("this wednesday", 12),
        ("last monday", 10),
    ];

    for (input, day) in cases {
        let result = resolve(&provider, input, Language::English);
        assert_eq!(
            (result.year(), result.month(), result.day()),
            (2024, 6, day),
            "wrong date for {input:?}"
        );
        assert_eq!(
            (result.hour(), result.minute(), result.second()),
            (0, 0, 0),
            "{input:?} should be midnight"
        );
    }
}

/// German writes modifiers capitalised at the start of a sentence, and the
/// umlaut in "nächsten" must survive the case-insensitive match.
#[test]
fn capitalised_german_weekday_modifiers_are_accepted() {
    let provider = ChronoProvider::at(quiet_instant());

    for (input, day) in [
        ("nächsten Montag", 17),
        ("Nächsten Montag", 17),
        ("letzten Montag", 10),
        ("Letzten Montag", 10),
    ] {
        let result = resolve(&provider, input, Language::German);
        assert_eq!(
            (result.year(), result.month(), result.day()),
            (2024, 6, day),
            "wrong date for {input:?}"
        );
    }
}

#[test]
fn a_day_reference_can_be_combined_with_a_time() {
    let provider = ChronoProvider::at(quiet_instant());

    let result = resolve(&provider, "tomorrow at 3:30 pm", Language::English);
    assert_eq!((result.year(), result.month(), result.day()), (2024, 6, 13));
    assert_eq!((result.hour(), result.minute()), (15, 30));

    let result = resolve(&provider, "next monday at 9:00 am", Language::English);
    assert_eq!((result.year(), result.month(), result.day()), (2024, 6, 17));
    assert_eq!((result.hour(), result.minute()), (9, 0));

    let result = resolve(
        &provider,
        "day after tomorrow at 5:00 pm",
        Language::English,
    );
    assert_eq!((result.year(), result.month(), result.day()), (2024, 6, 14));
    assert_eq!((result.hour(), result.minute()), (17, 0));
}

// ===== Times of day =====

#[test]
fn times_resolve_on_the_pinned_day() {
    let base = quiet_instant();
    let provider = ChronoProvider::at(base);

    let cases = [
        ("3:30 pm", 15, 30),
        ("10:15 am", 10, 15),
        ("14:30", 14, 30),
        ("9:00 PM", 21, 0),
        ("12:00 PM", 12, 0),
        ("12:00 AM", 0, 0),
        ("noon", 12, 0),
        ("midnight", 0, 0),
    ];

    for (input, hour, minute) in cases {
        let result = resolve(&provider, input, Language::English);
        assert_eq!(result.date_naive(), base.date_naive(), "{input:?}");
        assert_eq!(
            (result.hour(), result.minute()),
            (hour, minute),
            "{input:?}"
        );
        assert_eq!(result.second(), 0, "{input:?}");
    }
}

/// "later today" means later *today*: it may be clamped, but it never lands on
/// another date and never resolves into the past.
#[test]
fn later_today_stays_within_the_current_day() {
    let base = fixed_datetime(2024, 6, 12, 10, 30, 0);
    let provider = ChronoProvider::at(base);
    let result = resolve(&provider, "later today", Language::English);
    assert_eq!(result, base + Duration::hours(2));

    let late = fixed_datetime(2024, 6, 12, 23, 30, 0);
    let provider = ChronoProvider::at(late);
    let result = resolve(&provider, "later today", Language::English);
    assert_eq!(
        result.date_naive(),
        late.date_naive(),
        "later today must not spill into tomorrow"
    );
    assert!(result >= late, "later today must not resolve into the past");
}

// ===== Absolute dates and times =====

#[test]
fn iso_datetimes_resolve_to_the_instant_they_name() {
    let provider = ChronoProvider::at(quiet_instant());

    let cases = [
        (
            "2024-01-15T14:30:00Z",
            Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 0).unwrap(),
        ),
        (
            "2024-01-15T14:30:00+02:00",
            Utc.with_ymd_and_hms(2024, 1, 15, 12, 30, 0).unwrap(),
        ),
        (
            "2024-01-15T14:30:00-05:00",
            Utc.with_ymd_and_hms(2024, 1, 15, 19, 30, 0).unwrap(),
        ),
        // A negative offset smaller than an hour: the sign belongs to the whole
        // offset, not to its hour component.
        (
            "2024-01-15T14:30:00-00:30",
            Utc.with_ymd_and_hms(2024, 1, 15, 15, 0, 0).unwrap(),
        ),
        (
            "2024-01-15T14:30:00+00:30",
            Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap(),
        ),
    ];

    for (input, expected) in cases {
        let result = resolve(&provider, input, Language::English);
        assert_eq!(result, expected, "wrong instant for {input:?}");
    }

    let result = resolve(&provider, "2024-01-15T14:30:00.123Z", Language::English);
    assert_eq!(result.nanosecond(), 123_000_000);
}

#[test]
fn dates_resolve_to_local_midnight() {
    let provider = ChronoProvider::at(quiet_instant());

    let cases = [
        ("15/03/2024", 2024, 3, 15),
        ("31-12-2025", 2025, 12, 31),
        ("01/01/2023", 2023, 1, 1),
    ];

    for (input, year, month, day) in cases {
        let result = resolve(&provider, input, Language::English);
        assert_eq!(
            (result.year(), result.month(), result.day()),
            (year, month, day),
            "wrong date for {input:?}"
        );
        assert_eq!(
            (result.hour(), result.minute(), result.second()),
            (0, 0, 0),
            "{input:?} should be midnight"
        );
    }
}

#[test]
fn invalid_programmatic_inputs_are_rejected() {
    let provider = ChronoProvider::at(quiet_instant());

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

    // A minute without an hour is not a time that can be honoured.
    let minute_without_hour = TimeExpression::Absolute(AbsoluteTime {
        year: 2024,
        month: 1,
        day: 15,
        hour: None,
        minute: Some(30),
        second: None,
        nanosecond: None,
        timezone: None,
    });
    assert!(matches!(
        provider.parse_expression(minute_without_hour),
        Err(TempsError::InvalidTime { .. })
    ));

    // ...and neither is a second or a nanosecond without one. Only `minute` used
    // to be guarded, so these two were silently discarded and the value resolved
    // to midnight — a supplied component dropped without a word.
    for (second, nanosecond) in [
        (Some(30), None),
        (None, Some(500_000_000)),
        (Some(59), Some(999_999_999)),
    ] {
        let sub_hour_without_hour = TimeExpression::Absolute(AbsoluteTime {
            year: 2024,
            month: 1,
            day: 15,
            hour: None,
            minute: None,
            second,
            nanosecond,
            timezone: Some(Timezone::Utc),
        });
        assert!(
            matches!(
                provider.parse_expression(sub_hour_without_hour),
                Err(TempsError::InvalidTime { .. })
            ),
            "second={second:?} nanosecond={nanosecond:?} with no hour must be rejected, \
             not resolved to midnight"
        );
    }

    // The same components *with* an hour are honoured, so the guard is about the
    // missing hour and not about the fields themselves.
    let honoured = provider
        .parse_expression(TimeExpression::Absolute(AbsoluteTime {
            year: 2024,
            month: 1,
            day: 15,
            hour: Some(0),
            minute: None,
            second: Some(30),
            nanosecond: None,
            timezone: Some(Timezone::Utc),
        }))
        .expect("a second with an hour is a valid time");
    assert_eq!(
        honoured.with_timezone(&Utc).to_string(),
        "2024-01-15 00:00:30 UTC"
    );
}

/// A date with no time still honours the zone the expression names.
///
/// The timezone field used to be read only when an hour was present, so a
/// date-only `Absolute` resolved to *local* midnight whatever it said — an
/// instant a whole zone offset away from the one the value names.
#[test]
fn a_date_only_absolute_honours_its_timezone() {
    let midnight_in = |timezone| {
        ChronoProvider::new()
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
            .with_timezone(&Utc)
    };

    assert_eq!(
        midnight_in(Some(Timezone::Utc)).to_string(),
        "2024-01-15 00:00:00 UTC"
    );
    assert_eq!(
        midnight_in(Some(Timezone::Offset {
            total_minutes: -300
        }))
        .to_string(),
        "2024-01-15 05:00:00 UTC",
        "-05:00 midnight is five hours after UTC midnight"
    );
    assert_eq!(
        midnight_in(Some(Timezone::Offset { total_minutes: 330 })).to_string(),
        "2024-01-14 18:30:00 UTC",
        "+05:30 midnight is the previous UTC day"
    );

    // No zone named still means local midnight, which is the old behaviour and
    // stays pinned by `dates_resolve_to_local_midnight`.
    assert_eq!(
        midnight_in(None)
            .with_timezone(&Local)
            .naive_local()
            .to_string(),
        "2024-01-15 00:00:00"
    );
}

/// Whatever this API returns must be readable.
///
/// A fixed-length shift can land on an instant whose *local* reading falls
/// outside chrono's `NaiveDateTime` range even though the instant itself is
/// representable. Such a value used to be returned as `Ok`, and every civil-time
/// accessor on it — `naive_local`, `date_naive`, `time` — then panicked in the
/// caller's hands.
#[test]
fn a_returned_value_is_always_readable() {
    let now = Local::now();
    let provider = ChronoProvider::at(now);

    // The largest whole-hour amount that still parses, plus neighbours: these
    // are the shifts that reach chrono's absolute range edge.
    let to_the_edge = (NaiveDate::MAX
        .and_hms_opt(23, 59, 59)
        .expect("valid civil time")
        - now.naive_utc())
    .num_hours();
    let mut amounts = vec![
        1_i64,
        24,
        100_000,
        to_the_edge - 1,
        to_the_edge,
        to_the_edge + 1,
    ];
    amounts.retain(|amount| *amount > 0);

    for amount in amounts {
        let expr = parse(&format!("in {amount} hours"), Language::English)
            .unwrap_or_else(|e| panic!("`in {amount} hours` should parse: {e}"));
        match provider.parse_expression(expr) {
            // Rejected as unrepresentable: fine.
            Err(_) => {}
            // Accepted: the value has to be usable. `naive_local` and
            // `date_naive` unwrap chrono's range check internally, so reaching
            // the next line is the assertion.
            //
            // `time()` and `weekday()` are deliberately *not* used as the check:
            // they do not panic when the local reading is out of range, they
            // quietly wrap — a clock time that never existed, and the weekday of
            // a local date that does not exist. They would make this test look
            // stronger while asserting nothing.
            Ok(resolved) => {
                let civil = resolved.naive_local();
                assert_eq!(
                    civil.date(),
                    resolved.date_naive(),
                    "the two local-date accessors disagree"
                );
            }
        }
    }
}

// ===== Zone-dependent behaviour =====
//
// `TZ` is process-wide, so a test that sets it would corrupt every test running
// beside it. The tests in `zone_pinned` are therefore `#[ignore]`d and re-run
// below in a child process that owns its own environment; each `*_suite` test
// is the runner for one zone.

/// Re-run this test binary with `TZ` set, executing only the ignored tests
/// whose name contains `filter`.
fn run_zone_suite(zone: &str, filter: &str) {
    use std::path::PathBuf;
    use std::process::Command;

    let tzdir = std::env::var("TZDIR").unwrap_or_else(|_| "/usr/share/zoneinfo".to_string());
    let zone_file = PathBuf::from(&tzdir).join(zone);
    if !zone_file.exists() {
        eprintln!("skipping {filter}: no time zone database entry at {zone_file:?}");
        return;
    }

    let exe = std::env::current_exe().expect("path of the running test binary");
    let output = Command::new(&exe)
        .env("TZ", zone)
        .args(["--ignored", "--test-threads=1", "--nocapture", filter])
        .output()
        .unwrap_or_else(|error| panic!("failed to re-run {exe:?} with TZ={zone}: {error}"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "TZ={zone} {filter} failed\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    assert!(
        !stdout.contains("0 passed"),
        "TZ={zone} {filter} matched no tests; has one been renamed?\n{stdout}"
    );
}

#[test]
fn us_eastern_suite() {
    run_zone_suite("America/New_York", "zone_pinned::us_eastern");
}

#[test]
fn havana_suite() {
    run_zone_suite("America/Havana", "zone_pinned::havana");
}

#[test]
fn apia_suite() {
    run_zone_suite("Pacific/Apia", "zone_pinned::apia");
}

/// Tests that only make sense in a specific time zone.
///
/// Each is `#[ignore]`d because it needs `TZ` set for the whole process; the
/// `*_suite` tests above run them in a child process. To run one group by hand:
///
/// ```text
/// TZ=America/New_York cargo test -p temps-chrono --test all_tests -- --ignored us_eastern
/// ```
mod zone_pinned {
    use super::*;
    use chrono::NaiveDate;

    /// Every instant that really renders as local midnight on the given date:
    /// empty where the civil time is skipped by a transition, two entries where
    /// it happens twice.
    ///
    /// chrono's `LocalResult` alone is not enough to tell those apart — for a
    /// nonexistent local time it can answer `Single` carrying the offset from
    /// the wrong side of the gap, so each candidate is round-tripped through
    /// UTC and kept only if it still reads back as the requested civil time.
    /// This reads chrono directly, so a failure here points at the time zone
    /// database rather than at temps.
    fn instants_at_local_midnight(year: i32, month: u32, day: u32) -> Vec<DateTime<Local>> {
        let naive = NaiveDate::from_ymd_opt(year, month, day)
            .and_then(|date| date.and_hms_opt(0, 0, 0))
            .expect("valid civil midnight");

        let candidates = match naive.and_local_timezone(Local) {
            chrono::LocalResult::Single(dt) => vec![dt],
            chrono::LocalResult::Ambiguous(first, second) => vec![first, second],
            chrono::LocalResult::None => vec![],
        };

        candidates
            .into_iter()
            .map(|dt| Utc.from_utc_datetime(&dt.naive_utc()).with_timezone(&Local))
            .filter(|dt| dt.naive_local() == naive)
            .collect()
    }

    // --- America/New_York: spring forward 2024-03-10 02:00, fall back 2024-11-03 02:00 ---

    /// Day references are calendar arithmetic. Adding a fixed 24 hours to
    /// 2024-03-09 23:30 EST lands on 2024-03-11 — a day late — because the
    /// following day is only 23 hours long.
    #[test]
    #[ignore = "requires TZ=America/New_York; run via us_eastern_suite"]
    fn us_eastern_day_references_cross_spring_forward_by_calendar_days() {
        // 2024-03-09 23:30 EST
        let base = instant_at_utc(2024, 3, 10, 4, 30);
        assert_eq!(base.date_naive().to_string(), "2024-03-09");
        assert_eq!(base.hour(), 23, "expected 23:30 local in US Eastern");

        let provider = ChronoProvider::at(base);

        for (input, expected) in [
            ("today", "2024-03-09"),
            ("yesterday", "2024-03-08"),
            ("tomorrow", "2024-03-10"),
            ("day after tomorrow", "2024-03-11"),
            ("the day before yesterday", "2024-03-07"),
        ] {
            let result = resolve(&provider, input, Language::English);
            assert_eq!(
                result.date_naive().to_string(),
                expected,
                "wrong date for {input:?}"
            );
            assert_eq!(
                (result.hour(), result.minute(), result.second()),
                (0, 0, 0),
                "{input:?} should be midnight"
            );
        }
    }

    /// "in N days" keeps the wall-clock time across a transition, so the
    /// elapsed absolute time is 23 hours, not 24.
    #[test]
    #[ignore = "requires TZ=America/New_York; run via us_eastern_suite"]
    fn us_eastern_day_and_week_offsets_keep_the_wall_clock_across_spring_forward() {
        let base = instant_at_utc(2024, 3, 10, 4, 30); // 2024-03-09 23:30 EST
        let provider = ChronoProvider::at(base);

        let next_day = resolve(&provider, "in 1 day", Language::English);
        assert_eq!(next_day.date_naive().to_string(), "2024-03-10");
        assert_eq!(next_day.time(), base.time(), "wall clock should be kept");
        assert_ne!(
            next_day.offset().fix(),
            base.offset().fix(),
            "the transition should have been crossed"
        );
        assert_eq!(
            next_day.signed_duration_since(base),
            Duration::hours(23),
            "the day of a spring-forward is 23 hours long"
        );

        let next_week = resolve(&provider, "in 1 week", Language::English);
        assert_eq!(next_week.date_naive().to_string(), "2024-03-16");
        assert_eq!(next_week.time(), base.time());
        assert_eq!(
            next_week.signed_duration_since(base),
            Duration::hours(7 * 24 - 1)
        );
    }

    /// The mirror image: the day of a fall-back is 25 hours long, and the wall
    /// clock is still preserved.
    #[test]
    #[ignore = "requires TZ=America/New_York; run via us_eastern_suite"]
    fn us_eastern_day_offsets_keep_the_wall_clock_across_fall_back() {
        let base = instant_at_utc(2024, 11, 3, 3, 30); // 2024-11-02 23:30 EDT
        assert_eq!(base.date_naive().to_string(), "2024-11-02");
        let provider = ChronoProvider::at(base);

        let next_day = resolve(&provider, "in 1 day", Language::English);
        assert_eq!(next_day.date_naive().to_string(), "2024-11-03");
        assert_eq!(next_day.time(), base.time());
        assert_eq!(
            next_day.signed_duration_since(base),
            Duration::hours(25),
            "the day of a fall-back is 25 hours long"
        );

        let tomorrow = resolve(&provider, "tomorrow", Language::English);
        assert_eq!(tomorrow.date_naive().to_string(), "2024-11-03");
        let day_after = resolve(&provider, "day after tomorrow", Language::English);
        assert_eq!(day_after.date_naive().to_string(), "2024-11-04");
    }

    // --- America/Havana: transitions at local midnight ---

    /// Cuba ends daylight saving at 01:00 local, so 2024-11-03 00:00–00:59
    /// happens twice. `.single()` returns `None` for such a fold, which used to
    /// make plain "today" fail; the earlier of the two instants is chosen.
    #[test]
    #[ignore = "requires TZ=America/Havana; run via havana_suite"]
    fn havana_today_resolves_when_local_midnight_is_ambiguous() {
        let instants = instants_at_local_midnight(2024, 11, 3);
        assert_eq!(
            instants.len(),
            2,
            "expected 2024-11-03 00:00 to happen twice in America/Havana, got {instants:?}"
        );
        let earlier = *instants.iter().min().expect("two instants");

        // Noon on the fold day.
        let provider = ChronoProvider::at(instant_at_utc(2024, 11, 3, 17, 0));

        let today = resolve(&provider, "today", Language::English);
        assert_eq!(
            today, earlier,
            "an ambiguous midnight resolves to the earlier instant"
        );
        assert_eq!(today.date_naive().to_string(), "2024-11-03");
        assert_eq!(today.hour(), 0);

        // The same civil midnight reached as "tomorrow" from the day before.
        let provider = ChronoProvider::at(instant_at_utc(2024, 11, 2, 16, 0));
        assert_eq!(resolve(&provider, "tomorrow", Language::English), earlier);
        assert_eq!(resolve(&provider, "midnight", Language::English).hour(), 0);
    }

    /// Cuba starts daylight saving at midnight, so 2024-03-10 00:00 does not
    /// exist at all. A nonexistent time is shifted forward by the width of the
    /// gap, matching jiff's `compatible` disambiguation.
    #[test]
    #[ignore = "requires TZ=America/Havana; run via havana_suite"]
    fn havana_day_references_resolve_when_local_midnight_does_not_exist() {
        assert!(
            instants_at_local_midnight(2024, 3, 10).is_empty(),
            "expected 2024-03-10 00:00 not to exist in America/Havana"
        );

        // Noon on the gap day.
        let provider = ChronoProvider::at(instant_at_utc(2024, 3, 10, 16, 0));
        let today = resolve(&provider, "today", Language::English);
        assert_eq!(today.date_naive().to_string(), "2024-03-10");
        assert_eq!(
            (today.hour(), today.minute()),
            (1, 0),
            "midnight is shifted forward by the one-hour gap"
        );

        // Reached as "tomorrow" from the previous day.
        let provider = ChronoProvider::at(instant_at_utc(2024, 3, 9, 16, 0));
        let tomorrow = resolve(&provider, "tomorrow", Language::English);
        assert_eq!(tomorrow.date_naive().to_string(), "2024-03-10");
        assert_eq!((tomorrow.hour(), tomorrow.minute()), (1, 0));
    }

    // --- Pacific/Apia: 2011-12-30 was skipped entirely ---

    /// Samoa moved across the date line at the end of 2011: 2011-12-30 has no
    /// instants whatsoever. The gap is 24 hours wide, which a fixed-size probe
    /// for the surrounding offset would miss.
    #[test]
    #[ignore = "requires TZ=Pacific/Apia; run via apia_suite"]
    fn apia_tomorrow_resolves_over_a_whole_skipped_day() {
        assert!(
            instants_at_local_midnight(2011, 12, 30).is_empty(),
            "expected 2011-12-30 not to exist in Pacific/Apia"
        );

        // 2011-12-29 12:00 local (UTC-10).
        let base = instant_at_utc(2011, 12, 29, 22, 0);
        assert_eq!(base.date_naive().to_string(), "2011-12-29");
        let provider = ChronoProvider::at(base);

        let tomorrow = resolve(&provider, "tomorrow", Language::English);
        assert_eq!(
            tomorrow.date_naive().to_string(),
            "2011-12-31",
            "the skipped day is shifted forward by the width of the gap"
        );
        assert_eq!((tomorrow.hour(), tomorrow.minute()), (0, 0));

        assert_eq!(
            resolve(&provider, "today", Language::English)
                .date_naive()
                .to_string(),
            "2011-12-29"
        );
    }

    /// The *upper* boundary of a fall-back fold is a civil time that occurs
    /// exactly once, in the post-transition offset — but chrono still reports it
    /// as `Ambiguous`, and taking its first candidate put the result an hour
    /// early, on an offset that was not in force at that instant.
    #[test]
    #[ignore = "requires TZ=America/New_York; run via us_eastern_suite"]
    fn us_eastern_fold_upper_boundary_resolves_to_its_only_instant() {
        // In US Eastern 2024-11-03 01:00-02:00 happens twice; 02:00 happens
        // once, at EST (-05:00), which is 07:00 UTC.
        let provider = ChronoProvider::new();

        let boundary = resolve(&provider, "2024-11-03T02:00", Language::English);
        assert_eq!(
            boundary.with_timezone(&Utc).to_string(),
            "2024-11-03 07:00:00 UTC"
        );
        assert_eq!(
            boundary.naive_local().to_string(),
            "2024-11-03 02:00:00",
            "the returned value must read back as the wall clock that was asked for"
        );

        // The day-and-time arms share `resolve_local` and were wrong the same way.
        let on_the_day = instant_at_utc(2024, 11, 3, 17, 0); // 2024-11-03 12:00 EST
        assert_eq!(on_the_day.date_naive().to_string(), "2024-11-03");
        assert_eq!(
            resolve(
                &ChronoProvider::at(on_the_day),
                "today at 02:00",
                Language::English
            )
            .with_timezone(&Utc)
            .to_string(),
            "2024-11-03 07:00:00 UTC"
        );

        let day_before = instant_at_utc(2024, 11, 2, 16, 0); // 2024-11-02 12:00 EDT
        assert_eq!(day_before.date_naive().to_string(), "2024-11-02");
        assert_eq!(
            resolve(
                &ChronoProvider::at(day_before),
                "tomorrow at 02:00",
                Language::English
            )
            .with_timezone(&Utc)
            .to_string(),
            "2024-11-03 07:00:00 UTC"
        );

        // A fold *interior* genuinely has two instants, and takes the earlier.
        let interior = resolve(&provider, "2024-11-03T01:30", Language::English);
        assert_eq!(
            interior.with_timezone(&Utc).to_string(),
            "2024-11-03 05:30:00 UTC",
            "an ambiguous local time resolves to the earlier instant"
        );
    }

    /// Calendar arithmetic landing inside a fold used to be a hard error.
    /// `DateTime<Local>::checked_add_months` resolves through
    /// `.and_local_timezone(..).single()`, which is `None` for any ambiguous
    /// target, so `in 1 month` failed where `in 31 days` — the very same civil
    /// target — succeeded.
    #[test]
    #[ignore = "requires TZ=America/New_York; run via us_eastern_suite"]
    fn us_eastern_month_arithmetic_lands_in_a_dst_fold() {
        // 2024-10-03 01:30 EDT; one month later is 2024-11-03 01:30, which
        // happens twice.
        let base = instant_at_utc(2024, 10, 3, 5, 30);
        assert_eq!(base.naive_local().to_string(), "2024-10-03 01:30:00");
        let provider = ChronoProvider::at(base);

        let by_month = resolve(&provider, "in 1 month", Language::English);
        let by_days = resolve(&provider, "in 31 days", Language::English);
        assert_eq!(
            by_month, by_days,
            "one month and 31 days name the same civil time and must agree"
        );
        assert_eq!(
            by_month.with_timezone(&Utc).to_string(),
            "2024-11-03 05:30:00 UTC",
            "the earlier of the fold's two instants"
        );
        assert_eq!(by_month.naive_local().to_string(), "2024-11-03 01:30:00");

        // The past direction has to round-trip.
        assert_eq!(
            resolve(
                &ChronoProvider::at(by_month),
                "1 month ago",
                Language::English
            )
            .with_timezone(&Utc)
            .to_string(),
            base.with_timezone(&Utc).to_string()
        );
    }

    /// A clock pinned at chrono's absolute range edge must be answered with an
    /// error, never a panic: `date_naive`, `time`, `weekday` and `naive_local`
    /// all unwrap their range check internally. In US Eastern (`-05:00`) it is
    /// the *minimum* end of the range that a local reading falls off.
    #[test]
    #[ignore = "requires TZ=America/New_York; run via us_eastern_suite"]
    fn us_eastern_extreme_pinned_clocks_error_rather_than_panic() {
        for (label, civil) in [
            (
                "NaiveDate::MIN",
                NaiveDate::MIN
                    .and_hms_opt(0, 0, 0)
                    .expect("valid civil time"),
            ),
            (
                "NaiveDate::MAX",
                NaiveDate::MAX
                    .and_hms_opt(23, 59, 59)
                    .expect("valid civil time"),
            ),
        ] {
            let pinned = Utc.from_utc_datetime(&civil).with_timezone(&Local);
            let provider = ChronoProvider::at(pinned);

            for input in [
                "tomorrow",
                "yesterday",
                "today",
                "3:00 pm",
                "in 1 day",
                "1 hour ago",
                "later today",
                "2024-01-15",
                "next monday",
            ] {
                // Reaching the end of this loop is the assertion — a panic here
                // is the defect. A value that does come back must be readable.
                let outcome = try_resolve(&provider, input, Language::English);
                eprintln!(
                    "{label} / {input:?} -> {}",
                    if outcome.is_ok() { "Ok" } else { "Err" }
                );
                if let Ok(resolved) = outcome {
                    let _ = resolved.naive_local();
                }
            }

            eprintln!("{label} answered without panicking");
        }
    }

    /// A shift can be representable as an instant while its *local* reading is
    /// not, and every civil-time accessor on the returned value unwraps that
    /// range check. So a returned value must be readable, and a shift that
    /// cannot be read back has to be an error.
    ///
    /// Note the zone matters here: where the zone database has no entry for an
    /// instant as extreme as chrono's range edge, `Local` falls back to UTC and
    /// the local reading is in range after all. The assertion below is therefore
    /// the invariant rather than a fixed outcome — either an error, or a value
    /// whose every accessor returns.
    #[test]
    #[ignore = "requires TZ=America/New_York; run via us_eastern_suite"]
    fn us_eastern_a_shift_that_poisons_the_local_view_is_rejected() {
        // Three hours into chrono's range, then a shift further in: the largest
        // local reading the zone can absorb is only a few hours wide.
        let pinned = Utc
            .from_utc_datetime(
                &NaiveDate::MIN
                    .and_hms_opt(3, 0, 0)
                    .expect("valid civil time"),
            )
            .with_timezone(&Local);
        let provider = ChronoProvider::at(pinned);

        for input in ["in 1 hour", "in 24 hours", "in 100 years", "in 1 second"] {
            match try_resolve(&provider, input, Language::English) {
                // Rejected as unreadable: the point of the guard.
                Err(_) => {}
                // Accepted: then it must be usable, because `naive_local` and
                // `date_naive` unwrap their range check rather than returning
                // when the local reading is out of range.
                Ok(resolved) => {
                    let _ = resolved.naive_local();
                    let _ = resolved.date_naive();
                }
            }
        }
    }
}
