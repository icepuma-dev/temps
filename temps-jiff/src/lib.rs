//! # temps-jiff
//!
//! Jiff integration for the temps time expression parser.
//!
//! This crate provides a `JiffProvider` that implements the `TimeParser` trait
//! using the jiff datetime library. It enables parsing natural language time
//! expressions into jiff's `Zoned` type.
//!
//! ## Features
//!
//! - Full implementation of the temps `TimeParser` trait
//! - Support for all time expression types
//! - Proper handling of month/year arithmetic using jiff's `Span`
//! - Timezone support (UTC and fixed offsets)
//! - Precise time calculations with nanosecond precision
//!
//! ## Example
//!
//! ```
//! use temps_jiff::{JiffProvider, parse_to_zoned};
//! use temps_core::{Language, TimeParser};
//!
//! // Parse using the convenience function
//! let datetime = parse_to_zoned("in 5 minutes", Language::English).unwrap();
//! println!("In 5 minutes: {}", datetime);
//!
//! // Or use the provider directly
//! let provider = JiffProvider::new();
//! let expr = temps_core::parse("tomorrow at 3:30 pm", Language::English).unwrap();
//! let datetime = provider.parse_expression(expr).unwrap();
//! ```
//!
//! ## Month and Year Arithmetic
//!
//! This implementation uses jiff's `Span` type for date arithmetic, which
//! provides correct handling of edge cases:
//!
//! - January 31 + 1 month = February 29 (leap year) or February 28 (non-leap year)
//! - February 29, 2024 + 1 year = February 28, 2025
//!
//! ## Error Handling
//!
//! All parsing operations return `Result<Zoned, TempsError>`. Common errors include:
//!
//! - `ParseError`: Invalid input that cannot be parsed
//! - `DateCalculationError`: Date arithmetic that results in invalid dates
//! - `InvalidDate`/`InvalidTime`: Components that are out of valid ranges
//! - `BackendError`: Errors from the jiff library

use jiff::{Span, Zoned};
use temps_core::{
    DayReference, Direction, Language, Result, TempsError, TimeExpression, TimeParser, TimeUnit,
    Weekday,
    errors::*,
    time_utils::{
        calculate_timezone_offset_seconds, calculate_weekday_offset, convert_12_to_24_hour,
        is_valid_time, is_valid_timezone_offset,
    },
};

/// Jiff-based implementation of the TimeParser trait.
///
/// This provider uses jiff's `Zoned` as its datetime type, providing
/// high-precision time calculations and comprehensive timezone support.
///
/// ## Upper range limit
///
/// `jiff::Timestamp::MAX` is `9999-12-30T22:00:00.999999999Z` — earlier than the
/// end of year 9999 that a civil date allows — so the final hours of year 9999
/// are out of reach here. A civil datetime resolves only when its instant lands
/// at or before that timestamp, which makes the last accepted local datetime
/// depend on the zone's offset: `9999-12-30T22:00:00` in UTC,
/// `9999-12-30T17:00:00` at `-05:00`, `9999-12-31T07:00:00` at `+09:00`. Since a
/// date-only expression resolves to local midnight, `9999-12-31` fails in UTC
/// but succeeds in `Asia/Tokyo`.
///
/// Past that point, absolute expressions fail with `TempsError::BackendError`
/// and relative ones (`in 1 day` from `9999-12-30T12:00`, say) with
/// `TempsError::DateCalculationError`. This is a range limit of the underlying
/// library, not a defect: `ChronoProvider` has no equivalent limit and accepts
/// the same expressions. It is the one range in which the two backends are
/// expected to differ; everywhere else they are held to the same answers.
///
/// ## Example
///
/// ```
/// use temps_jiff::JiffProvider;
/// use temps_core::{TimeParser, parse, Language};
///
/// let provider = JiffProvider::new();
/// let expr = parse("next Monday", Language::English).unwrap();
/// let datetime = provider.parse_expression(expr).unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct JiffProvider {
    /// Fixed instant to resolve against, or `None` to read the system clock.
    now: Option<Zoned>,
}

impl JiffProvider {
    /// A provider that resolves expressions against the system clock.
    #[must_use]
    pub fn new() -> Self {
        Self { now: None }
    }

    /// A provider pinned to a fixed instant.
    ///
    /// Resolution of "now", "tomorrow" and every relative expression is
    /// relative to this instant, which makes results reproducible — including
    /// around daylight-saving transitions, where behaviour otherwise depends on
    /// the day the code happens to run.
    ///
    /// The pinned instant's time zone is also the zone every *zone-less*
    /// expression resolves in, so a pinned provider is reproducible in its zone
    /// as well as its instant: `tomorrow at 9:00 am` and `2024-06-02 09:00` name
    /// the same wall clock and resolve to the same instant. An expression that
    /// carries its own offset or `Z` is converted into this zone.
    ///
    /// # Examples
    ///
    /// ```
    /// use jiff::civil::date;
    /// use jiff::tz::TimeZone;
    /// use temps_jiff::JiffProvider;
    /// use temps_core::{Language, TimeParser, parse};
    ///
    /// let fixed = date(2024, 1, 31).at(10, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap();
    /// let provider = JiffProvider::at(fixed);
    /// let expr = parse("tomorrow", Language::English).unwrap();
    /// let resolved = provider.parse_expression(expr).unwrap();
    /// assert_eq!(resolved.date().to_string(), "2024-02-01");
    /// ```
    #[must_use]
    pub fn at(now: Zoned) -> Self {
        Self { now: Some(now) }
    }
}

fn jiff_date_components(year: u16, month: u8, day: u8) -> Result<(i16, i8, i8)> {
    Ok((
        i16::try_from(year).map_err(|_| TempsError::invalid_date(year, month, day))?,
        i8::try_from(month).map_err(|_| TempsError::invalid_date(year, month, day))?,
        i8::try_from(day).map_err(|_| TempsError::invalid_date(year, month, day))?,
    ))
}

fn jiff_time_components(
    hour: u8,
    minute: u8,
    second: u8,
    nanosecond: u32,
) -> Result<(i8, i8, i8, i32)> {
    Ok((
        i8::try_from(hour).map_err(|_| TempsError::invalid_time(hour, minute, second))?,
        i8::try_from(minute).map_err(|_| TempsError::invalid_time(hour, minute, second))?,
        i8::try_from(second).map_err(|_| TempsError::invalid_time(hour, minute, second))?,
        i32::try_from(nanosecond)
            .map_err(|_| TempsError::backend_error("Invalid nanosecond component", "jiff"))?,
    ))
}

/// Resolve a civil datetime to an instant, honouring the expression's own zone
/// when it carried one, and otherwise the provider's zone.
///
/// Shared by every `Absolute` shape, so that a date with no time is treated
/// exactly like a datetime: previously the timezone field was read only when an
/// hour was present, and a date-only value silently resolved to *provider-zone*
/// midnight even when it named UTC or an explicit offset.
fn resolve_with_zone(
    datetime: jiff::civil::DateTime,
    timezone: Option<&temps_core::Timezone>,
    provider_zone: jiff::tz::TimeZone,
) -> Result<Zoned> {
    use jiff::tz::{Offset, TimeZone};

    fn converted<E: std::fmt::Display>(result: std::result::Result<Zoned, E>) -> Result<Zoned> {
        result.map_err(|e| {
            TempsError::backend_error(format!("{ERR_TIMEZONE_CONVERSION}: {e}"), "jiff")
        })
    }

    match timezone {
        Some(temps_core::Timezone::Utc) => converted(datetime.to_zoned(TimeZone::UTC))
            .map(|zoned| zoned.with_time_zone(provider_zone)),
        Some(temps_core::Timezone::Offset { total_minutes }) => {
            if !is_valid_timezone_offset(temps_core::Timezone::Offset {
                total_minutes: *total_minutes,
            }) {
                return Err(TempsError::invalid_timezone_offset(*total_minutes));
            }

            let offset = Offset::from_seconds(calculate_timezone_offset_seconds(*total_minutes))
                .map_err(|_| TempsError::invalid_timezone_offset(*total_minutes))?;
            converted(datetime.to_zoned(TimeZone::fixed(offset)))
                .map(|zoned| zoned.with_time_zone(provider_zone))
        }
        None => converted(datetime.to_zoned(provider_zone)),
    }
}

impl TimeParser for JiffProvider {
    type DateTime = Zoned;

    fn now(&self) -> Self::DateTime {
        self.now.clone().unwrap_or_else(Zoned::now)
    }

    fn parse_expression(&self, expr: TimeExpression) -> Result<Self::DateTime> {
        match expr {
            TimeExpression::Now => Ok(self.now()),
            TimeExpression::Relative(rel) => {
                if rel.amount < 0 {
                    return Err(TempsError::date_calculation(
                        ERR_RELATIVE_AMOUNT_NON_NEGATIVE,
                    ));
                }

                let now = self.now();

                // Create a span based on the time unit
                // The `try_*` builders are essential: the plain setters panic when
                // the amount exceeds jiff's per-unit range, and `rel.amount` comes
                // straight from user input.
                let span = match rel.unit {
                    TimeUnit::Second => Span::new().try_seconds(rel.amount),
                    TimeUnit::Minute => Span::new().try_minutes(rel.amount),
                    TimeUnit::Hour => Span::new().try_hours(rel.amount),
                    TimeUnit::Day => Span::new().try_days(rel.amount),
                    TimeUnit::Week => Span::new().try_weeks(rel.amount),
                    TimeUnit::Month => Span::new().try_months(rel.amount),
                    TimeUnit::Year => Span::new().try_years(rel.amount),
                }
                .map_err(|_| TempsError::arithmetic_overflow(ERR_AMOUNT_OUT_OF_RANGE))?;

                // Apply the span in the correct direction
                match rel.direction {
                    Direction::Past => now.checked_sub(span).map_err(|e| {
                        TempsError::date_calculation_with_source(ERR_DATE_CALC_ERROR, e.to_string())
                    }),
                    Direction::Future => now.checked_add(span).map_err(|e| {
                        TempsError::date_calculation_with_source(ERR_DATE_CALC_ERROR, e.to_string())
                    }),
                }
            }
            TimeExpression::Absolute(abs) => {
                use jiff::civil::{Date, DateTime, Time};

                // The zone every zone-less expression resolves against. Reading
                // `TimeZone::system()` here instead made one pinned provider
                // answer the same wall clock in two zones — the relative, day
                // and time arms follow `at(..)`, these arms followed the process
                // `TZ` — which contradicts `at`'s promise that pinning an
                // instant makes results reproducible.
                let provider_zone = self.now().time_zone().clone();

                let (year, month, day) = jiff_date_components(abs.year, abs.month, abs.day)?;
                let date = Date::new(year, month, day)
                    .map_err(|e| TempsError::backend_error(e.to_string(), "jiff"))?;

                // A sub-hour component without an hour is not a time this can
                // honour; say so rather than silently falling through to
                // midnight. `minute` was the only one guarded, so an explicit
                // `second` or `nanosecond` was quietly dropped and the value
                // resolved to midnight instead — the same silent discard, one
                // field further down.
                if abs.hour.is_none()
                    && (abs.minute.is_some() || abs.second.is_some() || abs.nanosecond.is_some())
                {
                    return Err(TempsError::invalid_time(
                        0,
                        abs.minute.unwrap_or(0),
                        abs.second.unwrap_or(0),
                    ));
                }

                if let Some(hour) = abs.hour {
                    // Default only the components below the one supplied; dropping a
                    // supplied hour would silently return local midnight instead.
                    let minute = abs.minute.unwrap_or(0);
                    // Validate hour is in valid range (0-23)
                    if hour > 23 {
                        return Err(TempsError::invalid_time(
                            hour,
                            minute,
                            abs.second.unwrap_or(0),
                        ));
                    }
                    // Validate minute is in valid range (0-59)
                    if minute > 59 {
                        return Err(TempsError::invalid_time(
                            hour,
                            minute,
                            abs.second.unwrap_or(0),
                        ));
                    }
                    // Validate second is in valid range (0-59)
                    if let Some(second) = abs.second
                        && second > 59
                    {
                        return Err(TempsError::invalid_time(hour, minute, second));
                    }

                    let second = abs.second.unwrap_or(0);
                    let nanosecond = abs.nanosecond.unwrap_or(0);
                    let (hour, minute, second, nanosecond) =
                        jiff_time_components(hour, minute, second, nanosecond)?;

                    let time = Time::new(hour, minute, second, nanosecond)
                        .map_err(|e| TempsError::backend_error(e.to_string(), "jiff"))?;

                    let datetime = DateTime::from_parts(date, time);
                    resolve_with_zone(datetime, abs.timezone.as_ref(), provider_zone)
                } else {
                    // Date only, set time to midnight — in the expression's own
                    // zone when it named one, rather than silently discarding it.
                    let datetime = date.at(0, 0, 0, 0);
                    resolve_with_zone(datetime, abs.timezone.as_ref(), provider_zone)
                }
            }
            TimeExpression::Day(day_ref) => {
                let now = self.now();
                match day_ref {
                    DayReference::Today => {
                        let date = now.date();
                        date.at(0, 0, 0, 0)
                            .to_zoned(now.time_zone().clone())
                            .map_err(|e| {
                                TempsError::date_calculation_with_source(
                                    "Failed to create today's date",
                                    e.to_string(),
                                )
                            })
                    }
                    DayReference::Yesterday => {
                        let date = now.date().checked_sub(Span::new().days(1)).map_err(|e| {
                            TempsError::date_calculation_with_source(
                                "Failed to calculate yesterday",
                                e.to_string(),
                            )
                        })?;
                        date.at(0, 0, 0, 0)
                            .to_zoned(now.time_zone().clone())
                            .map_err(|e| {
                                TempsError::date_calculation_with_source(
                                    "Failed to create yesterday's date",
                                    e.to_string(),
                                )
                            })
                    }
                    DayReference::Tomorrow => {
                        let date = now.date().checked_add(Span::new().days(1)).map_err(|e| {
                            TempsError::date_calculation_with_source(
                                "Failed to calculate tomorrow",
                                e.to_string(),
                            )
                        })?;
                        date.at(0, 0, 0, 0)
                            .to_zoned(now.time_zone().clone())
                            .map_err(|e| {
                                TempsError::date_calculation_with_source(
                                    "Failed to create tomorrow's date",
                                    e.to_string(),
                                )
                            })
                    }
                    DayReference::DayBeforeYesterday => {
                        let date = now.date().checked_sub(Span::new().days(2)).map_err(|e| {
                            TempsError::date_calculation_with_source(
                                "Failed to calculate day before yesterday",
                                e.to_string(),
                            )
                        })?;
                        date.at(0, 0, 0, 0)
                            .to_zoned(now.time_zone().clone())
                            .map_err(|e| {
                                TempsError::date_calculation_with_source(
                                    "Failed to create day before yesterday's date",
                                    e.to_string(),
                                )
                            })
                    }
                    DayReference::DayAfterTomorrow => {
                        let date = now.date().checked_add(Span::new().days(2)).map_err(|e| {
                            TempsError::date_calculation_with_source(
                                "Failed to calculate day after tomorrow",
                                e.to_string(),
                            )
                        })?;
                        date.at(0, 0, 0, 0)
                            .to_zoned(now.time_zone().clone())
                            .map_err(|e| {
                                TempsError::date_calculation_with_source(
                                    "Failed to create day after tomorrow's date",
                                    e.to_string(),
                                )
                            })
                    }
                    DayReference::Weekday { day, modifier } => {
                        let target_weekday = match day {
                            Weekday::Monday => jiff::civil::Weekday::Monday,
                            Weekday::Tuesday => jiff::civil::Weekday::Tuesday,
                            Weekday::Wednesday => jiff::civil::Weekday::Wednesday,
                            Weekday::Thursday => jiff::civil::Weekday::Thursday,
                            Weekday::Friday => jiff::civil::Weekday::Friday,
                            Weekday::Saturday => jiff::civil::Weekday::Saturday,
                            Weekday::Sunday => jiff::civil::Weekday::Sunday,
                        };

                        let current_weekday = now.weekday();
                        let current_offset = current_weekday.to_monday_zero_offset() as i64;
                        let target_offset = target_weekday.to_monday_zero_offset() as i64;

                        let days_to_add =
                            calculate_weekday_offset(current_offset, target_offset, modifier);
                        let date = now
                            .date()
                            .checked_add(Span::new().days(days_to_add))
                            .map_err(|e| {
                                TempsError::date_calculation_with_source(
                                    "Failed to calculate weekday",
                                    e.to_string(),
                                )
                            })?;
                        date.at(0, 0, 0, 0)
                            .to_zoned(now.time_zone().clone())
                            .map_err(|e| {
                                TempsError::date_calculation_with_source(
                                    "Failed to create weekday date",
                                    e.to_string(),
                                )
                            })
                    }
                }
            }
            TimeExpression::Time(time) => {
                let now = self.now();
                let date = now.date();

                if !is_valid_time(time.hour, time.minute, time.second, time.meridiem) {
                    return Err(TempsError::invalid_time(
                        time.hour,
                        time.minute,
                        time.second,
                    ));
                }

                let hour = convert_12_to_24_hour(time.hour, time.meridiem.as_ref());

                let (hour, minute, second, nanosecond) =
                    jiff_time_components(hour, time.minute, time.second, 0)?;

                date.at(hour, minute, second, nanosecond)
                    .to_zoned(now.time_zone().clone())
                    .map_err(|e| {
                        TempsError::backend_error(format!("Failed to create time: {e}"), "jiff")
                    })
            }
            TimeExpression::DayTime(day_time) => {
                // First get the day
                let day_result = self.parse_expression(TimeExpression::Day(day_time.day))?;
                let date = day_result.date();

                if !is_valid_time(
                    day_time.time.hour,
                    day_time.time.minute,
                    day_time.time.second,
                    day_time.time.meridiem,
                ) {
                    return Err(TempsError::invalid_time(
                        day_time.time.hour,
                        day_time.time.minute,
                        day_time.time.second,
                    ));
                }

                let hour =
                    convert_12_to_24_hour(day_time.time.hour, day_time.time.meridiem.as_ref());

                let (hour, minute, second, nanosecond) =
                    jiff_time_components(hour, day_time.time.minute, day_time.time.second, 0)?;

                date.at(hour, minute, second, nanosecond)
                    .to_zoned(day_result.time_zone().clone())
                    .map_err(|e| {
                        TempsError::backend_error(format!("Failed to create day time: {e}"), "jiff")
                    })
            }
            TimeExpression::LaterToday => {
                let now = self.now();
                let later = now.checked_add(Span::new().hours(2)).map_err(|e| {
                    TempsError::date_calculation_with_source(ERR_DATE_CALC_ERROR, e.to_string())
                })?;
                // Clamp against the true end of the local day rather than a fixed
                // 23:59:59, which need not exist and would drop sub-second precision.
                let tomorrow_start = now
                    .date()
                    .tomorrow()
                    .map_err(|e| {
                        TempsError::date_calculation_with_source(ERR_DATE_CALC_ERROR, e.to_string())
                    })?
                    .at(0, 0, 0, 0)
                    .to_zoned(now.time_zone().clone())
                    .map_err(|e| {
                        TempsError::date_calculation_with_source(ERR_DATE_CALC_ERROR, e.to_string())
                    })?;

                if later < tomorrow_start {
                    return Ok(later);
                }
                let last_today = tomorrow_start
                    .checked_sub(Span::new().nanoseconds(1))
                    .map_err(|e| {
                        TempsError::date_calculation_with_source(ERR_DATE_CALC_ERROR, e.to_string())
                    })?;
                // Never resolve into the past.
                Ok(if last_today < now { now } else { last_today })
            }
            TimeExpression::Date(date) => {
                use jiff::civil::Date;

                let (year, month, day) = jiff_date_components(date.year, date.month, date.day)?;
                let jiff_date = Date::new(year, month, day)
                    .map_err(|_| TempsError::invalid_date(date.year, date.month, date.day))?;

                jiff_date
                    .at(0, 0, 0, 0)
                    // The provider's zone, matching every other arm — see the
                    // note on `provider_zone` in the `Absolute` arm.
                    .to_zoned(self.now().time_zone().clone())
                    .map_err(|e| {
                        TempsError::backend_error(format!("Failed to create date: {e}"), "jiff")
                    })
            }
        }
    }
}

/// Parse a natural language time expression into a jiff `Zoned` datetime.
///
/// This is a convenience function that combines parsing and time calculation
/// in a single call.
///
/// # Arguments
///
/// * `input` - The natural language time expression to parse
/// * `language` - The language to use for parsing
///
/// # Returns
///
/// Returns `Ok(Zoned)` if parsing succeeds, or `Err(TempsError)`
/// if the input cannot be parsed or the date calculation fails.
///
/// # Examples
///
/// ```
/// use temps_jiff::parse_to_zoned;
/// use temps_core::Language;
///
/// // Parse English expressions
/// let dt = parse_to_zoned("in 30 minutes", Language::English).unwrap();
/// let dt = parse_to_zoned("tomorrow at 12:00", Language::English).unwrap();
/// let dt = parse_to_zoned("last Monday", Language::English).unwrap();
///
/// // Parse German expressions  
/// let dt = parse_to_zoned("in 30 Minuten", Language::German).unwrap();
/// let dt = parse_to_zoned("morgen um 15:30", Language::German).unwrap();
/// ```
///
/// # Errors
///
/// This function will return an error if:
/// - The input cannot be parsed as a valid time expression
/// - Date calculation results in an invalid date
/// - Components are out of valid ranges (e.g., month 13)
/// - The jiff library returns an error during calculations
pub fn parse_to_zoned(input: &str, language: Language) -> Result<Zoned> {
    let expr = temps_core::parse(input, language)?;
    JiffProvider::new().parse_expression(expr)
}
