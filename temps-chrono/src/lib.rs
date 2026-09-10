//! # temps-chrono
//!
//! Chrono integration for the temps time expression parser.
//!
//! This crate provides a `ChronoProvider` that implements the `TimeParser` trait
//! using the chrono datetime library. It enables parsing natural language time
//! expressions into chrono's `DateTime<Local>` type.
//!
//! ## Features
//!
//! - Full implementation of the temps `TimeParser` trait
//! - Support for all time expression types
//! - Proper handling of month/year arithmetic
//! - Timezone support (UTC and fixed offsets)
//! - DST-aware local time handling
//!
//! ## Example
//!
//! ```
//! use temps_chrono::{ChronoProvider, parse_to_datetime};
//! use temps_core::{Language, TimeParser};
//!
//! // Parse using the convenience function
//! let datetime = parse_to_datetime("in 5 minutes", Language::English).unwrap();
//! println!("In 5 minutes: {}", datetime);
//!
//! // Or use the provider directly
//! let provider = ChronoProvider::new();
//! let expr = temps_core::parse("tomorrow at 3:30 pm", Language::English).unwrap();
//! let datetime = provider.parse_expression(expr).unwrap();
//! ```
//!
//! ## Month and Year Arithmetic
//!
//! This implementation uses chrono's `checked_add_months` and `checked_sub_months`
//! for proper month/year arithmetic. This handles edge cases correctly:
//!
//! - January 31 + 1 month = February 29 (leap year) or February 28 (non-leap year)
//! - February 29, 2024 + 1 year = February 28, 2025
//!
//! ## Error Handling
//!
//! All parsing operations return `Result<DateTime<Local>, TempsError>`. Common errors include:
//!
//! - `ParseError`: Invalid input that cannot be parsed
//! - `DateCalculationError`: Date arithmetic that results in invalid dates
//! - `AmbiguousTime`: A local time that could not be resolved at all — note that
//!   a DST fold is *not* this case: an ambiguous local time is resolved to the
//!   earlier of its two instants, matching jiff's `compatible` disambiguation
//! - `InvalidDate`/`InvalidTime`: Components that are out of valid ranges

use chrono::{
    DateTime, Datelike, Days, Duration, Local, Months, NaiveDateTime, Offset, TimeDelta, TimeZone,
    Utc,
};

/// The local civil time of `instant`, or `None` when the zone offset would push
/// it outside chrono's `NaiveDateTime` range.
///
/// `DateTime::naive_local`, `date_naive`, `time` and `weekday` all `expect()`
/// their way past that range check, so a clock pinned within a zone offset of
/// `NaiveDate::MIN`/`MAX` used to make this `Result`-returning API panic. This
/// reports the same condition without unwinding, and is exactly what
/// `naive_local` does internally: add the offset to the UTC reading.
fn local_civil(instant: DateTime<Local>) -> Option<NaiveDateTime> {
    instant
        .naive_utc()
        .checked_add_offset(instant.offset().fix())
}

/// The pinned clock's local date, or an error when its local reading falls
/// outside chrono's representable range.
///
/// Companion to [`local_civil`] for the arms that only need the date; see there
/// for why `DateTime::date_naive` is not usable.
fn local_date(now: DateTime<Local>) -> Result<chrono::NaiveDate> {
    local_civil(now)
        .map(|civil| civil.date())
        .ok_or_else(|| TempsError::arithmetic_overflow(ERR_AMOUNT_OUT_OF_RANGE))
}

/// Shift a civil time that does not exist forward by the width of the gap that
/// swallowed it, by interpreting it with the offset in force *before* the gap.
///
/// jiff's `compatible` disambiguation defines the same result, and this works
/// for any gap width — including whole days skipped at the date line, where a
/// fixed-size probe would give up.
fn shift_out_of_gap(naive: NaiveDateTime) -> Option<DateTime<Local>> {
    let pre_gap_offset = (1..=3).find_map(|days| {
        naive
            .checked_sub_days(Days::new(days))?
            .and_local_timezone(Local)
            .earliest()
            .map(|dt| *dt.offset())
    })?;
    let utc = naive.checked_sub_offset(pre_gap_offset)?;
    Some(Utc.from_utc_datetime(&utc).with_timezone(&Local))
}

/// Resolve a naive local datetime to a concrete instant, matching the jiff
/// backend's default `compatible` disambiguation.
///
/// Every `LocalResult` branch needs care, and in two of them chrono's own
/// report cannot be taken at face value:
///
/// * **`Single`** is normally an unremarkable local time, and re-reading it
///   through UTC is a no-op. It is not always unremarkable: a nonexistent local
///   time is not reliably reported as `None`. When a spring-forward gap begins
///   exactly at midnight, chrono returns `Single` carrying the offset from
///   *before* the gap — in `America/Havana`, `2024-03-10 00:00:00` comes back as
///   `Single` at `-05:00`, a wall-clock reading that never occurred, while
///   `00:30:00` on the same date is `None`. Re-reading the instant under the
///   offset actually in force yields `2024-03-10 01:00:00-04:00`, the same
///   forward shift jiff performs.
/// * **`Ambiguous`** is a fall-back fold, and its two instants are not ordered —
///   `.earliest()` can hand back the *later* one — so choose by comparison. It
///   is also over-reported: chrono marks the fold's **upper** boundary as
///   ambiguous, because its tz_info comparison is inclusive at the upper end,
///   even though a civil time there occurs exactly once, in the
///   post-transition offset. Trusting the report made
///   `2024-10-27T03:00:00` in `Europe/Paris` resolve an hour early, to an
///   offset that was not in force. Candidates are therefore filtered to those
///   that genuinely read back as the requested civil time.
/// * **`None`** is the ordinary report for a nonexistent local time. See
///   [`shift_out_of_gap`].
fn resolve_local(naive: NaiveDateTime) -> Option<DateTime<Local>> {
    use chrono::offset::LocalResult;

    // The instant chrono attached to `naive`, re-read under the offset that is
    // actually in force there. This is what turns a stale offset into the
    // instant it really denotes.
    let reread = |dt: DateTime<Local>| Utc.from_utc_datetime(&dt.naive_utc()).with_timezone(&Local);

    match naive.and_local_timezone(Local) {
        LocalResult::Single(dt) => {
            let instant = reread(dt);
            if local_civil(instant) == Some(naive) {
                Some(instant)
            } else {
                // The stale pre-gap offset described above.
                shift_out_of_gap(naive)
            }
        }
        LocalResult::Ambiguous(first, second) => {
            let mut candidates: Vec<DateTime<Local>> = [first, second]
                .into_iter()
                .map(reread)
                .filter(|dt| local_civil(*dt) == Some(naive))
                .collect();

            if candidates.is_empty() {
                // Neither candidate renders back as `naive`, so it is really a
                // gap that chrono mislabelled; fall back to the forward shift.
                return shift_out_of_gap(naive);
            }

            // A genuine fold leaves two candidates and `compatible` takes the
            // earlier instant.
            candidates.sort_unstable();
            candidates.into_iter().next()
        }
        LocalResult::None => shift_out_of_gap(naive),
    }
}

/// Resolve a civil datetime to an instant, honouring the expression's own zone
/// when it carried one and falling back to the local zone otherwise.
///
/// Shared by every `Absolute` shape, including a date with no time: the
/// timezone field used to be read only when an hour was present, so
/// `AbsoluteTime { hour: None, timezone: Some(Utc) }` silently resolved to
/// *local* midnight — an instant a whole zone offset away from the one the
/// value names.
fn resolve_with_timezone(
    naive: NaiveDateTime,
    timezone: Option<&temps_core::Timezone>,
) -> Result<DateTime<Local>> {
    use chrono::FixedOffset;

    match timezone {
        Some(temps_core::Timezone::Utc) => Ok(Utc.from_utc_datetime(&naive).with_timezone(&Local)),
        Some(temps_core::Timezone::Offset { total_minutes }) => {
            let requested = temps_core::Timezone::Offset {
                total_minutes: *total_minutes,
            };
            if !is_valid_timezone_offset(requested) {
                return Err(TempsError::invalid_timezone_offset(*total_minutes));
            }

            let offset = FixedOffset::east_opt(calculate_timezone_offset_seconds(*total_minutes))
                .ok_or_else(|| TempsError::invalid_timezone_offset(*total_minutes))?;
            offset
                .from_local_datetime(&naive)
                .single()
                .ok_or_else(|| TempsError::ambiguous_time(ERR_AMBIGUOUS_TIME))
                .map(|dt| dt.with_timezone(&Local))
        }
        None => resolve_local(naive).ok_or_else(|| TempsError::ambiguous_time(ERR_AMBIGUOUS_TIME)),
    }
}
use temps_core::{
    DayReference, Direction, Language, Result, TempsError, TimeExpression, TimeParser, TimeUnit,
    Weekday,
    constants::{DAYS_PER_WEEK, MONTHS_PER_YEAR},
    errors::*,
    time_utils::{
        calculate_timezone_offset_seconds, calculate_weekday_offset, convert_12_to_24_hour,
        is_valid_time, is_valid_timezone_offset,
    },
};

/// Chrono-based implementation of the TimeParser trait.
///
/// This provider uses chrono's `DateTime<Local>` as its datetime type,
/// providing full support for timezones, DST, and proper date arithmetic.
///
/// ## Example
///
/// ```
/// use temps_chrono::ChronoProvider;
/// use temps_core::{TimeParser, parse, Language};
///
/// let provider = ChronoProvider::new();
/// let expr = parse("next Monday", Language::English).unwrap();
/// let datetime = provider.parse_expression(expr).unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct ChronoProvider {
    /// Fixed instant to resolve against, or `None` to read the system clock.
    now: Option<DateTime<Local>>,
}

impl ChronoProvider {
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
    /// # Examples
    ///
    /// ```
    /// use chrono::{Local, TimeZone};
    /// use temps_chrono::ChronoProvider;
    /// use temps_core::{Language, TimeParser, parse};
    ///
    /// let fixed = Local.with_ymd_and_hms(2024, 1, 31, 10, 0, 0).unwrap();
    /// let provider = ChronoProvider::at(fixed);
    /// let expr = parse("tomorrow", Language::English).unwrap();
    /// let resolved = provider.parse_expression(expr).unwrap();
    /// assert_eq!(resolved.date_naive().to_string(), "2024-02-01");
    /// ```
    #[must_use]
    pub fn at(now: DateTime<Local>) -> Self {
        Self { now: Some(now) }
    }
}

impl TimeParser for ChronoProvider {
    type DateTime = DateTime<Local>;

    fn now(&self) -> Self::DateTime {
        self.now.unwrap_or_else(Local::now)
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

                if rel.amount == 0 {
                    return Ok(now);
                }

                // Handle months and years separately for proper date arithmetic
                match rel.unit {
                    TimeUnit::Month | TimeUnit::Year => {
                        // Convert years to months so both share one path.
                        let months_count = match rel.unit {
                            TimeUnit::Year => rel
                                .amount
                                .checked_mul(MONTHS_PER_YEAR as i64)
                                .ok_or_else(|| {
                                    TempsError::arithmetic_overflow(ERR_YEAR_OVERFLOW)
                                })?,
                            _ => rel.amount,
                        };
                        let months = Months::new(months_count.try_into().map_err(|_| {
                            TempsError::arithmetic_overflow(ERR_AMOUNT_OUT_OF_RANGE)
                        })?);

                        // Shift the CIVIL time and resolve it like every other arm.
                        // `DateTime<Local>::checked_add_months` resolves through
                        // `.and_local_timezone(..).single()`, which is `None` for any
                        // target inside a DST gap or fold — turning "in 1 month" into a
                        // hard error where "in 31 days" succeeds — and at a gap
                        // boundary it hands back an unnormalised DateTime whose local
                        // reading never occurred.
                        let civil = local_civil(now).ok_or_else(|| {
                            TempsError::arithmetic_overflow(ERR_AMOUNT_OUT_OF_RANGE)
                        })?;
                        let shifted = match rel.direction {
                            Direction::Past => civil.checked_sub_months(months),
                            Direction::Future => civil.checked_add_months(months),
                        }
                        .ok_or_else(|| TempsError::date_calculation(ERR_DATE_CALC_INVALID))?;

                        resolve_local(shifted)
                            .ok_or_else(|| TempsError::ambiguous_time(ERR_AMBIGUOUS_TIME))
                    }
                    TimeUnit::Day | TimeUnit::Week => {
                        // Calendar-aware, matching the jiff backend: "in 3 days"
                        // keeps the wall-clock time across a DST transition.
                        let days = if matches!(rel.unit, TimeUnit::Week) {
                            rel.amount.checked_mul(i64::from(DAYS_PER_WEEK))
                        } else {
                            Some(rel.amount)
                        }
                        .and_then(|d| u64::try_from(d).ok())
                        .ok_or_else(|| TempsError::arithmetic_overflow(ERR_AMOUNT_OUT_OF_RANGE))?;

                        let civil = local_civil(now).ok_or_else(|| {
                            TempsError::arithmetic_overflow(ERR_AMOUNT_OUT_OF_RANGE)
                        })?;
                        let date = match rel.direction {
                            Direction::Past => civil.date().checked_sub_days(Days::new(days)),
                            Direction::Future => civil.date().checked_add_days(Days::new(days)),
                        }
                        .ok_or_else(|| TempsError::arithmetic_overflow(ERR_AMOUNT_OUT_OF_RANGE))?;

                        let naive = date.and_time(civil.time());
                        resolve_local(naive)
                            .ok_or_else(|| TempsError::ambiguous_time(ERR_AMBIGUOUS_TIME))
                    }
                    _ => {
                        // Fixed-length units. Use the fallible constructors and a
                        // checked add: a large parsed amount must be an error, not
                        // a panic.
                        let duration = match rel.unit {
                            TimeUnit::Second => TimeDelta::try_seconds(rel.amount),
                            TimeUnit::Minute => TimeDelta::try_minutes(rel.amount),
                            TimeUnit::Hour => TimeDelta::try_hours(rel.amount),
                            _ => unreachable!(), // Day/Week/Month/Year handled above
                        }
                        .ok_or_else(|| TempsError::arithmetic_overflow(ERR_AMOUNT_OUT_OF_RANGE))?;

                        let shifted = match rel.direction {
                            Direction::Past => now.checked_sub_signed(duration),
                            Direction::Future => now.checked_add_signed(duration),
                        }
                        .ok_or_else(|| TempsError::arithmetic_overflow(ERR_AMOUNT_OUT_OF_RANGE))?;

                        // The instant can be representable while its *local*
                        // reading is not, and every civil-time accessor on the
                        // returned value (`naive_local`, `date_naive`, `time`)
                        // would then panic for the caller. Reject it here rather
                        // than hand back a value that is only usable through
                        // `Display`.
                        if local_civil(shifted).is_none() {
                            return Err(TempsError::arithmetic_overflow(ERR_AMOUNT_OUT_OF_RANGE));
                        }
                        Ok(shifted)
                    }
                }
            }
            TimeExpression::Absolute(abs) => {
                use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

                let date =
                    NaiveDate::from_ymd_opt(abs.year as i32, abs.month as u32, abs.day as u32)
                        .ok_or_else(|| TempsError::invalid_date(abs.year, abs.month, abs.day))?;

                if abs.hour.is_none() && abs.minute.is_some() {
                    // A minute without an hour is not a time we can honour; say so
                    // rather than silently falling through to midnight.
                    return Err(TempsError::invalid_time(
                        0,
                        abs.minute.unwrap_or(0),
                        abs.second.unwrap_or(0),
                    ));
                }

                let naive = if let Some(hour) = abs.hour {
                    // Default only the components below the one supplied.
                    let minute = abs.minute.unwrap_or(0);
                    let time = NaiveTime::from_hms_nano_opt(
                        hour as u32,
                        minute as u32,
                        abs.second.unwrap_or(0) as u32,
                        abs.nanosecond.unwrap_or(0),
                    )
                    .ok_or_else(|| {
                        TempsError::invalid_time(hour, minute, abs.second.unwrap_or(0))
                    })?;

                    NaiveDateTime::new(date, time)
                } else {
                    // Date only, set time to midnight — in the expression's own
                    // zone, if it named one.
                    date.and_hms_opt(0, 0, 0)
                        .ok_or_else(|| TempsError::date_calculation(ERR_MIDNIGHT_FAILED))?
                };

                resolve_with_timezone(naive, abs.timezone.as_ref())
            }
            TimeExpression::Day(day_ref) => {
                let now = self.now();
                match day_ref {
                    DayReference::Today => {
                        let midnight = local_date(now)?
                            .and_hms_opt(0, 0, 0)
                            .ok_or_else(|| TempsError::date_calculation(ERR_MIDNIGHT_FAILED))?;
                        resolve_local(midnight)
                            .ok_or_else(|| TempsError::ambiguous_time(ERR_AMBIGUOUS_TIME))
                    }
                    DayReference::Yesterday => {
                        let midnight = local_date(now)?
                            .checked_sub_days(Days::new(1))
                            .ok_or_else(|| TempsError::date_calculation(ERR_DATE_CALC_INVALID))?
                            .and_hms_opt(0, 0, 0)
                            .ok_or_else(|| TempsError::date_calculation(ERR_MIDNIGHT_FAILED))?;
                        resolve_local(midnight)
                            .ok_or_else(|| TempsError::ambiguous_time(ERR_AMBIGUOUS_TIME))
                    }
                    DayReference::Tomorrow => {
                        let midnight = local_date(now)?
                            .checked_add_days(Days::new(1))
                            .ok_or_else(|| TempsError::date_calculation(ERR_DATE_CALC_INVALID))?
                            .and_hms_opt(0, 0, 0)
                            .ok_or_else(|| TempsError::date_calculation(ERR_MIDNIGHT_FAILED))?;
                        resolve_local(midnight)
                            .ok_or_else(|| TempsError::ambiguous_time(ERR_AMBIGUOUS_TIME))
                    }
                    DayReference::DayBeforeYesterday => {
                        let midnight = local_date(now)?
                            .checked_sub_days(Days::new(2))
                            .ok_or_else(|| TempsError::date_calculation(ERR_DATE_CALC_INVALID))?
                            .and_hms_opt(0, 0, 0)
                            .ok_or_else(|| TempsError::date_calculation(ERR_MIDNIGHT_FAILED))?;
                        resolve_local(midnight)
                            .ok_or_else(|| TempsError::ambiguous_time(ERR_AMBIGUOUS_TIME))
                    }
                    DayReference::DayAfterTomorrow => {
                        let midnight = local_date(now)?
                            .checked_add_days(Days::new(2))
                            .ok_or_else(|| TempsError::date_calculation(ERR_DATE_CALC_INVALID))?
                            .and_hms_opt(0, 0, 0)
                            .ok_or_else(|| TempsError::date_calculation(ERR_MIDNIGHT_FAILED))?;
                        resolve_local(midnight)
                            .ok_or_else(|| TempsError::ambiguous_time(ERR_AMBIGUOUS_TIME))
                    }
                    DayReference::Weekday { day, modifier } => {
                        let target_weekday = match day {
                            Weekday::Monday => chrono::Weekday::Mon,
                            Weekday::Tuesday => chrono::Weekday::Tue,
                            Weekday::Wednesday => chrono::Weekday::Wed,
                            Weekday::Thursday => chrono::Weekday::Thu,
                            Weekday::Friday => chrono::Weekday::Fri,
                            Weekday::Saturday => chrono::Weekday::Sat,
                            Weekday::Sunday => chrono::Weekday::Sun,
                        };

                        let current_weekday = local_civil(now)
                            .ok_or_else(|| {
                                TempsError::arithmetic_overflow(ERR_AMOUNT_OUT_OF_RANGE)
                            })?
                            .weekday();
                        let current_offset = current_weekday.num_days_from_monday() as i64;
                        let target_offset = target_weekday.num_days_from_monday() as i64;

                        let days_to_add =
                            calculate_weekday_offset(current_offset, target_offset, modifier);
                        let base = local_date(now)?;
                        let target_date = if days_to_add >= 0 {
                            base.checked_add_days(Days::new(days_to_add.unsigned_abs()))
                        } else {
                            base.checked_sub_days(Days::new(days_to_add.unsigned_abs()))
                        }
                        .ok_or_else(|| TempsError::date_calculation(ERR_DATE_CALC_INVALID))?;

                        let midnight = target_date
                            .and_hms_opt(0, 0, 0)
                            .ok_or_else(|| TempsError::date_calculation(ERR_MIDNIGHT_FAILED))?;
                        resolve_local(midnight)
                            .ok_or_else(|| TempsError::ambiguous_time(ERR_AMBIGUOUS_TIME))
                    }
                }
            }
            TimeExpression::Time(time) => {
                let now = self.now();
                if !is_valid_time(time.hour, time.minute, time.second, time.meridiem) {
                    return Err(TempsError::invalid_time(
                        time.hour,
                        time.minute,
                        time.second,
                    ));
                }

                let hour = convert_12_to_24_hour(time.hour, time.meridiem.as_ref()) as u32;

                let naive = local_date(now)?
                    .and_hms_opt(hour, time.minute as u32, time.second as u32)
                    .ok_or_else(|| TempsError::invalid_time(time.hour, time.minute, time.second))?;
                Ok(resolve_local(naive)
                    .ok_or_else(|| TempsError::ambiguous_time("Ambiguous local time"))?)
            }
            TimeExpression::DayTime(day_time) => {
                // First get the day
                let day_result = self.parse_expression(TimeExpression::Day(day_time.day))?;
                let date = local_date(day_result)?;

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
                    convert_12_to_24_hour(day_time.time.hour, day_time.time.meridiem.as_ref())
                        as u32;

                let naive = date
                    .and_hms_opt(
                        hour,
                        day_time.time.minute as u32,
                        day_time.time.second as u32,
                    )
                    .ok_or_else(|| {
                        TempsError::invalid_time(
                            day_time.time.hour,
                            day_time.time.minute,
                            day_time.time.second,
                        )
                    })?;
                Ok(resolve_local(naive)
                    .ok_or_else(|| TempsError::ambiguous_time("Ambiguous local time"))?)
            }
            TimeExpression::LaterToday => {
                let now = self.now();
                let later = now
                    .checked_add_signed(Duration::hours(2))
                    .ok_or_else(|| TempsError::arithmetic_overflow(ERR_AMOUNT_OUT_OF_RANGE))?;

                // Clamp against the true end of the local day. A fixed 23:59:59
                // is wrong in zones where the day is cut short by a transition,
                // and would drop sub-second precision.
                let tomorrow = local_date(now)?
                    .checked_add_days(Days::new(1))
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .ok_or_else(|| TempsError::date_calculation(ERR_DATE_CALC_INVALID))?;
                let tomorrow_start = resolve_local(tomorrow)
                    .ok_or_else(|| TempsError::ambiguous_time(ERR_AMBIGUOUS_TIME))?;

                if later < tomorrow_start {
                    return Ok(later);
                }
                let last_today = tomorrow_start
                    .checked_sub_signed(TimeDelta::nanoseconds(1))
                    .ok_or_else(|| TempsError::date_calculation(ERR_DATE_CALC_INVALID))?;
                // Never resolve into the past.
                Ok(if last_today < now { now } else { last_today })
            }
            TimeExpression::Date(date) => {
                use chrono::NaiveDate;

                NaiveDate::from_ymd_opt(date.year as i32, date.month as u32, date.day as u32)
                    .ok_or_else(|| TempsError::invalid_date(date.year, date.month, date.day))?
                    .and_hms_opt(0, 0, 0)
                    .ok_or_else(|| TempsError::date_calculation(ERR_MIDNIGHT_FAILED))
                    .and_then(|naive| {
                        resolve_local(naive)
                            .ok_or_else(|| TempsError::ambiguous_time("Ambiguous local time"))
                    })
            }
        }
    }
}

/// Parse a natural language time expression into a chrono `DateTime<Local>`.
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
/// Returns `Ok(DateTime<Local>)` if parsing succeeds, or `Err(TempsError)`
/// if the input cannot be parsed or the date calculation fails.
///
/// # Examples
///
/// ```
/// use temps_chrono::parse_to_datetime;
/// use temps_core::Language;
///
/// // Parse English expressions
/// let dt = parse_to_datetime("in 30 minutes", Language::English).unwrap();
/// let dt = parse_to_datetime("tomorrow at 12:00", Language::English).unwrap();
/// let dt = parse_to_datetime("last Monday", Language::English).unwrap();
///
/// // Parse German expressions  
/// let dt = parse_to_datetime("in 30 Minuten", Language::German).unwrap();
/// let dt = parse_to_datetime("morgen um 15:30", Language::German).unwrap();
/// ```
///
/// # Errors
///
/// This function will return an error if:
/// - The input cannot be parsed as a valid time expression
/// - Date calculation results in an invalid date
/// - A local time cannot be resolved against the zone database at all
///
/// A local time that merely *repeats* (a DST fall-back fold) is not an error: it
/// resolves to the earlier of its two instants, as jiff does.
pub fn parse_to_datetime(input: &str, language: Language) -> Result<DateTime<Local>> {
    let expr = temps_core::parse(input, language)?;
    ChronoProvider::new().parse_expression(expr)
}
