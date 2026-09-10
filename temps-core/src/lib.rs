//! # temps-core
//!
//! Core functionality for parsing human-readable time expressions.
//!
//! This crate provides the fundamental types and traits for parsing natural language
//! time expressions like "in 5 minutes", "yesterday at 3pm", or "next Monday".
//! It is designed to be backend-agnostic, allowing different datetime libraries
//! (chrono, jiff, etc.) to implement the parsing logic.
//!
//! ## Overview
//!
//! The crate consists of several key components:
//!
//! - **Types**: Core data structures representing different time expressions
//! - **Traits**: Interfaces for implementing time parsing with different backends
//! - **Parsers**: Language-specific parsers (English and German)
//! - **Utilities**: Helper functions for time calculations and conversions
//!
//! ## Example
//!
//! ```
//! use temps_core::{parse, Language, TimeExpression};
//!
//! // Parse a relative time expression
//! let expr = parse("in 5 minutes", Language::English).unwrap();
//! match expr {
//!     TimeExpression::Relative(rel) => {
//!         println!("Amount: {}, Unit: {:?}", rel.amount, rel.unit);
//!     }
//!     _ => {}
//! }
//!
//! // Parse with German language
//! let expr = parse("in 5 Minuten", Language::German).unwrap();
//! ```
//!
//! ## Supported Languages
//!
//! - English
//! - German
//!
//! ## Error Handling
//!
//! All parsing operations return a `Result<T, TempsError>` where `TempsError`
//! provides detailed information about what went wrong during parsing or
//! date calculations.

// ===== Error Module =====
pub mod error;
pub mod lexer;
pub use error::{Result, TempsError};

// ===== Core Types =====

/// Represents a parsed time expression.
///
/// This is the main output type of the parsing functions. It can represent
/// various forms of time expressions from natural language input.
///
/// # Examples
///
/// ```
/// use temps_core::{parse, Language, TimeExpression};
///
/// // "now" -> TimeExpression::Now
/// // "in 5 minutes" -> TimeExpression::Relative(...)
/// // "2024-01-15T14:30:00Z" -> TimeExpression::Absolute(...)
/// // "tomorrow" -> TimeExpression::Day(...)
/// // "3:30 pm" -> TimeExpression::Time(...)
/// // "tomorrow at 3:30 pm" -> TimeExpression::DayTime(...)
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum TimeExpression {
    /// The current moment in time (e.g., "now", "jetzt")
    Now,
    /// A time relative to now (e.g., "in 5 minutes", "3 days ago")
    Relative(RelativeTime),
    /// An absolute date/time (e.g., "2024-01-15T14:30:00Z")
    Absolute(AbsoluteTime),
    /// A day reference (e.g., "tomorrow", "next Monday")
    Day(DayReference),
    /// A time of day (e.g., "3:30 pm", "14:30")
    Time(Time),
    /// A calendar date (e.g., "15/03/2024", "31-12-2025")
    Date(StandardDate),
    /// A day with a specific time (e.g., "tomorrow at 3:30 pm")
    DayTime(DayTime),
    /// A short way into the future, clamped so it cannot leave today
    /// (e.g., "later today"). Resolves to `now + 2h`, or the last second of
    /// today if that would cross midnight.
    LaterToday,
}

/// Represents a time relative to the current moment.
///
/// # Examples
///
/// ```
/// use temps_core::{RelativeTime, TimeUnit, Direction};
///
/// // "in 5 minutes"
/// let future = RelativeTime {
///     amount: 5,
///     unit: TimeUnit::Minute,
///     direction: Direction::Future,
/// };
///
/// // "3 days ago"
/// let past = RelativeTime {
///     amount: 3,
///     unit: TimeUnit::Day,
///     direction: Direction::Past,
/// };
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct RelativeTime {
    /// The numeric amount (e.g., 5 in "5 minutes")
    pub amount: i64,
    /// The time unit (second, minute, hour, etc.)
    pub unit: TimeUnit,
    /// Whether this is in the past or future
    pub direction: Direction,
}

/// Represents an absolute date and time.
///
/// This type can represent various levels of precision, from just a date
/// to a full timestamp with timezone and nanosecond precision.
///
/// # Examples
///
/// ```
/// use temps_core::{AbsoluteTime, Timezone};
///
/// // Date only: "2024-01-15"
/// let date_only = AbsoluteTime {
///     year: 2024,
///     month: 1,
///     day: 15,
///     hour: None,
///     minute: None,
///     second: None,
///     nanosecond: None,
///     timezone: None,
/// };
///
/// // Full timestamp: "2024-01-15T14:30:00Z"
/// let full_timestamp = AbsoluteTime {
///     year: 2024,
///     month: 1,
///     day: 15,
///     hour: Some(14),
///     minute: Some(30),
///     second: Some(0),
///     nanosecond: None,
///     timezone: Some(Timezone::Utc),
/// };
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct AbsoluteTime {
    /// The year (e.g., 2024)
    pub year: u16,
    /// The month (1-12)
    pub month: u8,
    /// The day of month (1-31)
    pub day: u8,
    /// The hour (0-23), if specified
    pub hour: Option<u8>,
    /// The minute (0-59), if specified
    pub minute: Option<u8>,
    /// The second (0-59), if specified
    pub second: Option<u8>,
    /// The nanosecond (0-999999999), if specified
    pub nanosecond: Option<u32>,
    /// The timezone, if specified
    pub timezone: Option<Timezone>,
}

/// Represents a timezone specification.
///
/// # Examples
///
/// ```
/// use temps_core::Timezone;
///
/// // UTC timezone ("Z")
/// let utc = Timezone::Utc;
///
/// // Offset timezone ("+02:00")
/// let offset = Timezone::Offset { total_minutes: 120 };
///
/// // Negative offset ("-05:30")
/// let negative = Timezone::Offset { total_minutes: -330 };
///
/// // Negative sub-hour offset ("-00:30")
/// let half_hour_west = Timezone::Offset { total_minutes: -30 };
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Timezone {
    /// UTC timezone (represented as "Z" in ISO format)
    Utc,
    /// Timezone offset from UTC, in minutes east of UTC.
    ///
    /// A single signed field so that negative sub-hour offsets such as
    /// `-00:30` are representable; a split hour/minute pair cannot carry the
    /// sign when the hour component is zero.
    Offset {
        /// Offset from UTC in minutes, from -720 (-12:00) to +840 (+14:00)
        total_minutes: i16,
    },
}

/// Represents a reference to a specific day.
///
/// # Examples
///
/// ```
/// use temps_core::{DayReference, Weekday, WeekdayModifier};
///
/// // "today"
/// let today = DayReference::Today;
///
/// // "next Monday"
/// let next_monday = DayReference::Weekday {
///     day: Weekday::Monday,
///     modifier: Some(WeekdayModifier::Next),
/// };
///
/// // "Friday" (upcoming Friday)
/// let friday = DayReference::Weekday {
///     day: Weekday::Friday,
///     modifier: None,
/// };
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum DayReference {
    /// Today's date
    Today,
    /// Yesterday's date
    Yesterday,
    /// Tomorrow's date
    Tomorrow,
    /// The day before yesterday's date
    DayBeforeYesterday,
    /// Day after tomorrow's date
    DayAfterTomorrow,
    /// A specific weekday
    Weekday {
        /// The day of the week
        day: Weekday,
        /// Optional modifier (next/last)
        modifier: Option<WeekdayModifier>,
    },
}

/// Represents a time of day.
///
/// Can represent both 12-hour (with AM/PM) and 24-hour formats.
///
/// # Examples
///
/// ```
/// use temps_core::{Time, Meridiem};
///
/// // "3:30 PM"
/// let afternoon = Time {
///     hour: 3,
///     minute: 30,
///     second: 0,
///     meridiem: Some(Meridiem::PM),
/// };
///
/// // "14:30" (24-hour format)
/// let military = Time {
///     hour: 14,
///     minute: 30,
///     second: 0,
///     meridiem: None,
/// };
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct Time {
    /// Hour (0-23 for 24-hour format, 1-12 for 12-hour format)
    pub hour: u8,
    /// Minute (0-59)
    pub minute: u8,
    /// Second (0-59)
    pub second: u8,
    /// AM/PM indicator for 12-hour format
    pub meridiem: Option<Meridiem>,
}

/// Represents a calendar date.
///
/// Used for parsing date formats like "15/03/2024" or "31-12-2025".
///
/// # Examples
///
/// ```
/// use temps_core::StandardDate;
///
/// // "15/03/2024"
/// let date = StandardDate {
///     day: 15,
///     month: 3,
///     year: 2024,
/// };
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct StandardDate {
    /// Day of month (1-31)
    pub day: u8,
    /// Month (1-12)
    pub month: u8,
    /// Year (e.g., 2024)
    pub year: u16,
}

/// Represents a combination of a day reference and a specific time.
///
/// Used for expressions like "tomorrow at 3:30 pm" or "next Monday at 9:00 am".
///
/// # Examples
///
/// ```
/// use temps_core::{DayTime, DayReference, Time, Meridiem};
///
/// // "tomorrow at 3:30 pm"
/// let tomorrow_afternoon = DayTime {
///     day: DayReference::Tomorrow,
///     time: Time {
///         hour: 3,
///         minute: 30,
///         second: 0,
///         meridiem: Some(Meridiem::PM),
///     },
/// };
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct DayTime {
    /// The day reference
    pub day: DayReference,
    /// The specific time on that day
    pub time: Time,
}

/// Units of time used in relative expressions.
///
/// # Examples
///
/// ```
/// use temps_core::TimeUnit;
///
/// // Used in expressions like:
/// // "5 seconds", "10 minutes", "2 hours", "3 days",
/// // "1 week", "6 months", "2 years"
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum TimeUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

/// Direction of time relative to now.
///
/// # Examples
///
/// ```
/// use temps_core::Direction;
///
/// // "5 minutes ago" -> Direction::Past
/// // "in 5 minutes" -> Direction::Future
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Direction {
    Past,
    Future,
}

/// Days of the week.
///
/// Used in expressions like "next Monday" or "last Friday".
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

/// Modifiers for weekday references.
///
/// # Examples
///
/// ```
/// use temps_core::WeekdayModifier;
///
/// // "last Monday" -> WeekdayModifier::Last
/// // "next Friday" -> WeekdayModifier::Next
/// // "Monday" (no modifier) -> finds the next occurrence
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum WeekdayModifier {
    Last,
    Next,
    /// The occurrence within the current Monday-to-Sunday week, which may be
    /// in the past (e.g. "this weekend" asked on a Sunday).
    This,
}

/// AM/PM indicator for 12-hour time format.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Meridiem {
    AM,
    PM,
}

/// Supported languages for parsing time expressions.
///
/// # Examples
///
/// ```
/// use temps_core::{parse, Language};
///
/// // Parse English
/// let expr = parse("in 5 minutes", Language::English);
///
/// // Parse German
/// let expr = parse("in 5 Minuten", Language::German);
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Language {
    English,
    German,
}

// ===== Traits =====

/// Trait for implementing time parsing with a specific datetime backend.
///
/// This trait should be implemented by datetime libraries (chrono, jiff, etc.)
/// to provide the actual time calculation logic.
///
/// # Examples
///
/// ```
/// use temps_core::{TimeParser, TimeExpression, Result};
///
/// struct MyTimeParser;
///
/// impl TimeParser for MyTimeParser {
///     type DateTime = String; // Your datetime type
///
///     fn now(&self) -> Self::DateTime {
///         "2024-01-15T14:30:00Z".to_string()
///     }
///
///     fn parse_expression(&self, expr: TimeExpression) -> Result<Self::DateTime> {
///         // Implementation here
///         Ok(self.now())
///     }
/// }
/// ```
pub trait TimeParser {
    /// The datetime type used by this implementation
    type DateTime;

    /// Get the current date and time
    fn now(&self) -> Self::DateTime;

    /// Parse a time expression into a concrete datetime
    ///
    /// # Errors
    ///
    /// Returns `TempsError` if:
    /// - Date calculation results in an invalid date
    /// - Arithmetic overflow occurs
    /// - The backend library returns an error
    fn parse_expression(&self, expr: TimeExpression) -> Result<Self::DateTime>;
}

/// Trait for implementing language-specific parsers.
///
/// This trait is implemented by language modules to provide
/// natural language parsing for different languages.
///
/// # Examples
///
/// ```
/// use temps_core::{LanguageParser, TimeExpression, Result};
///
/// struct MyLanguageParser;
///
/// impl LanguageParser for MyLanguageParser {
///     fn parse(&self, input: &str) -> Result<TimeExpression> {
///         // Parse language-specific input
///         Ok(TimeExpression::Now)
///     }
/// }
/// ```
pub trait LanguageParser {
    /// Parse a natural language time expression
    ///
    /// # Errors
    ///
    /// Returns `TempsError::ParseError` if the input cannot be parsed
    fn parse(&self, input: &str) -> Result<TimeExpression>;
}

// ===== Constants Module =====

pub mod constants {
    //! Common constants used across the temps library

    /// Number of seconds in one hour
    pub const SECONDS_PER_HOUR: i32 = 3600;

    /// Number of seconds in one minute  
    pub const SECONDS_PER_MINUTE: i32 = 60;

    /// Number of minutes in one hour
    pub const MINUTES_PER_HOUR: i32 = 60;

    /// Number of hours in one day
    pub const HOURS_PER_DAY: i32 = 24;

    /// Number of days in one week
    pub const DAYS_PER_WEEK: i32 = 7;

    /// Number of months in one year
    pub const MONTHS_PER_YEAR: i32 = 12;
}

// ===== Errors Module =====

pub mod errors {
    //! Common error messages and error handling utilities

    /// Error message for when month amount must be positive
    pub const ERR_MONTH_POSITIVE: &str = "Month amount must be a positive number";

    /// Error message for when year amount must be positive
    pub const ERR_YEAR_POSITIVE: &str = "Year amount must be a positive number";

    /// Error message for invalid date calculation
    pub const ERR_DATE_CALC_INVALID: &str = "Date calculation resulted in invalid date";

    /// Error message for year calculation overflow
    pub const ERR_YEAR_OVERFLOW: &str = "Year calculation overflow";

    /// Error message for a relative amount too large for the backend to represent
    pub const ERR_AMOUNT_OUT_OF_RANGE: &str = "Relative amount is too large to represent as a date";

    /// Error message for invalid date
    pub const ERR_INVALID_DATE: &str = "Invalid date";

    /// Error message for invalid time
    pub const ERR_INVALID_TIME: &str = "Invalid time";

    /// Error message for ambiguous local time
    pub const ERR_AMBIGUOUS_TIME: &str = "Ambiguous or invalid local time";

    /// Error message for failed midnight time creation
    pub const ERR_MIDNIGHT_FAILED: &str = "Failed to create midnight time";

    /// Error message for date calculation errors
    ///
    /// This is the `message` of a [`TempsError::DateCalculationError`], whose
    /// `Display` already prefixes it with "Date calculation error: ". The text
    /// here must therefore not repeat that phrase — it used to read
    /// "Date calculation error", which rendered as the doubled, cause-less
    /// "Date calculation error: Date calculation error".
    pub const ERR_DATE_CALC_ERROR: &str = "Failed to apply the requested time span";

    /// Parse diagnostic for a date whose components are not a real calendar
    /// date, such as `2024-02-30` or `31/02/2024`.
    pub const INVALID_CALENDAR_DATE: &str = "invalid calendar date";

    /// Parse diagnostic for a date written with two different separators, such
    /// as `15/03-2024`. The date itself may be perfectly real, so this must not
    /// be reported as an invalid calendar date.
    pub const MISMATCHED_DATE_SEPARATORS: &str = "date separators must match";

    /// Parse diagnostic for time components outside the 24-hour clock, such as
    /// `2024-01-15T25:00`.
    pub const INVALID_TIME_OF_DAY: &str = "invalid time";

    /// Error message for timezone conversion errors
    pub const ERR_TIMEZONE_CONVERSION: &str = "Timezone conversion error";

    /// Error message for negative relative amounts
    pub const ERR_RELATIVE_AMOUNT_NON_NEGATIVE: &str = "Relative amount must be non-negative";

    /// Format error message for invalid date with components
    #[must_use]
    pub fn format_invalid_date(year: u16, month: u8, day: u8) -> String {
        format!("Invalid date: {year}-{month}-{day}")
    }

    /// Format error message for invalid time with components
    #[must_use]
    pub fn format_invalid_time(hour: u8, minute: u8, second: u8) -> String {
        format!("Invalid time: {hour}:{minute}:{second}")
    }

    /// Format error message for invalid timezone offset
    #[must_use]
    pub fn format_invalid_timezone_offset(total_minutes: i16) -> String {
        let sign = if total_minutes < 0 { '-' } else { '+' };
        let magnitude = total_minutes.unsigned_abs();
        format!(
            "Invalid timezone offset: {sign}{:02}:{:02}",
            magnitude / 60,
            magnitude % 60
        )
    }
}

// ===== Time Utils Module =====

pub mod time_utils {
    //! Time conversion and calculation utilities

    use crate::{Meridiem, Timezone, WeekdayModifier, constants::SECONDS_PER_MINUTE};

    /// Convert 12-hour time format to 24-hour format
    ///
    /// # Examples
    /// ```
    /// use temps_core::{Meridiem, time_utils::convert_12_to_24_hour};
    ///
    /// assert_eq!(convert_12_to_24_hour(12, Some(&Meridiem::AM)), 0);  // 12 AM -> 0
    /// assert_eq!(convert_12_to_24_hour(12, Some(&Meridiem::PM)), 12); // 12 PM -> 12
    /// assert_eq!(convert_12_to_24_hour(3, Some(&Meridiem::PM)), 15);  // 3 PM -> 15
    /// assert_eq!(convert_12_to_24_hour(14, None), 14);                // 24-hour format
    /// ```
    #[must_use]
    pub fn convert_12_to_24_hour(hour: u8, meridiem: Option<&Meridiem>) -> u8 {
        match meridiem {
            Some(Meridiem::AM) => {
                if hour == 12 {
                    0
                } else {
                    hour
                }
            }
            Some(Meridiem::PM) => {
                if hour >= 12 {
                    // 12 PM is noon; anything above 12 is not a valid 12-hour
                    // clock hour, so pass it through rather than overflowing.
                    hour
                } else {
                    hour + 12
                }
            }
            None => hour,
        }
    }

    /// Calculate total seconds for a timezone offset
    ///
    /// Uses saturating arithmetic to prevent overflow
    #[must_use]
    pub fn calculate_timezone_offset_seconds(total_minutes: i16) -> i32 {
        i32::from(total_minutes).saturating_mul(SECONDS_PER_MINUTE)
    }

    /// Check whether the date components form a real calendar date.
    #[must_use]
    pub fn is_valid_calendar_date(year: u16, month: u8, day: u8) -> bool {
        let days_in_month = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if is_leap_year(year) => 29,
            2 => 28,
            _ => return false,
        };

        (1..=days_in_month).contains(&day)
    }

    /// Check whether the time components form a valid 24-hour clock time.
    #[must_use]
    pub fn is_valid_24_hour_time(hour: u8, minute: u8, second: u8) -> bool {
        hour <= 23 && minute <= 59 && second <= 59
    }

    /// Check whether time components are valid for either 24-hour or AM/PM notation.
    #[must_use]
    pub fn is_valid_time(hour: u8, minute: u8, second: u8, meridiem: Option<Meridiem>) -> bool {
        match meridiem {
            Some(_) => (1..=12).contains(&hour) && minute <= 59 && second <= 59,
            None => is_valid_24_hour_time(hour, minute, second),
        }
    }

    /// Check whether a timezone offset is in the supported UTC-12:00..=UTC+14:00 range.
    #[must_use]
    pub fn is_valid_timezone_offset(offset: Timezone) -> bool {
        match offset {
            Timezone::Utc => true,
            Timezone::Offset { total_minutes } => (-720..=840).contains(&total_minutes),
        }
    }

    /// Calculate the day offset for weekday calculations
    ///
    /// Returns the number of days to add/subtract to reach the target weekday
    ///
    /// # Arguments
    /// * `current_day_offset` - Current weekday as offset from Monday (0-6)
    /// * `target_day_offset` - Target weekday as offset from Monday (0-6)
    /// * `modifier` - Whether to get next, last, or closest occurrence
    #[must_use]
    pub fn calculate_weekday_offset(
        current_day_offset: i64,
        target_day_offset: i64,
        modifier: Option<WeekdayModifier>,
    ) -> i64 {
        // Saturating arithmetic: both offsets are weekday numbers in `0..=6` for
        // every in-tree caller (chrono's and jiff's `num_days_from_monday`), but
        // this is a public helper, and a plain `-` panicked on `i64::MIN`/
        // `i64::MAX` arguments in an overflow-checked build.
        let days_diff = target_day_offset.saturating_sub(current_day_offset);

        match modifier {
            None => {
                // Get the next occurrence (including today if it matches)
                if days_diff >= 0 {
                    days_diff
                } else {
                    7_i64.saturating_add(days_diff)
                }
            }
            Some(WeekdayModifier::Next) => {
                // Next occurrence (not including today)
                if days_diff > 0 {
                    days_diff
                } else {
                    7_i64.saturating_add(days_diff)
                }
            }
            Some(WeekdayModifier::This) => {
                // Same Monday-to-Sunday week, looking backwards if already passed
                days_diff
            }
            Some(WeekdayModifier::Last) => {
                // Previous occurrence (not including today)
                if days_diff < 0 {
                    days_diff
                } else {
                    days_diff.saturating_sub(7)
                }
            }
        }
    }

    #[must_use]
    fn is_leap_year(year: u16) -> bool {
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
    }
}

// ===== Common Parsing Module =====

/// Common parsing utilities shared across language implementations.
///
/// Every parser here consumes [`Token`](crate::lexer::Token)s produced by
/// [`lex`](crate::lexer::lex) rather than characters. Lexing first is what
/// makes keyword matching *whole-word* matching: `word_ci("day")` compares the
/// entire `Word("days")` slice and fails, where the old character-level
/// `keyword_ci("day")` matched the prefix and needed a hand-rolled word-boundary
/// assertion plus longest-first ordering to stay correct.
///
/// # Writing a parser against this module
///
/// Parsers are generic over the input so they compose with whatever concrete
/// token stream the caller builds:
///
/// ```
/// use chumsky::prelude::*;
/// use temps_core::common::{ParserError, TokenInput, word_ci};
///
/// fn now_expr<'t, 's: 't, I>() -> impl Parser<'t, I, (), ParserError<'t, 's>> + Clone
/// where
///     I: TokenInput<'t, 's>,
/// {
///     word_ci("now")
/// }
/// ```
///
/// and are driven by lexing the source and mapping the token slice into an
/// input:
///
/// ```
/// use chumsky::prelude::*;
/// use temps_core::{common::{token_stream, word_ci}, lexer::lex};
///
/// let input = "now";
/// let tokens = lex(input);
/// let result = word_ci("now")
///     .then_ignore(end())
///     .parse(token_stream(input, &tokens))
///     .into_result();
/// assert!(result.is_ok());
/// ```
pub mod common {
    use super::{AbsoluteTime, TimeExpression, Timezone, time_utils};
    use crate::errors::{INVALID_CALENDAR_DATE, INVALID_TIME_OF_DAY};
    use crate::lexer::{Token, lex};
    use chumsky::{input::ValueInput, prelude::*};

    /// The error type used throughout the parsers.
    ///
    /// `'t` is the lifetime of the token slice being parsed; `'s` is the
    /// lifetime of the source string those tokens borrow their slices from.
    /// `'s` always outlives `'t`.
    pub type ParserError<'t, 's> = extra::Err<Rich<'t, Token<'s>>>;

    /// The input bound every parser in this module is generic over.
    ///
    /// This is a blanket-implemented convenience for
    /// `ValueInput<'t, Token = Token<'s>, Span = SimpleSpan>`, which is what
    /// [`token_stream`] produces. Written out in a `where` clause on every
    /// parser function that bound is most of the signature; naming it keeps the
    /// grammar readable.
    pub trait TokenInput<'t, 's>: ValueInput<'t, Token = Token<'s>, Span = SimpleSpan> {}

    impl<'t, 's, I> TokenInput<'t, 's> for I where
        I: ValueInput<'t, Token = Token<'s>, Span = SimpleSpan>
    {
    }

    /// A boxed token parser.
    ///
    /// Needed wherever a homogeneous collection of parsers is required — most
    /// notably when [`phrase`] folds a phrase's tokens into a single parser.
    pub type BoxedParser<'t, 's, I, O> = chumsky::Boxed<'t, 't, I, O, ParserError<'t, 's>>;

    /// Turn a source string and its lexed tokens into a parser input.
    ///
    /// The end-of-input span is `source.len()..source.len()` so that an error
    /// at the end of the input still carries a byte offset the diagnostics
    /// layer can translate.
    ///
    /// ```
    /// use temps_core::{common::token_stream, lexer::lex};
    ///
    /// let input = "in 5 minutes";
    /// let tokens = lex(input);
    /// let stream = token_stream(input, &tokens);
    /// ```
    pub fn token_stream<'t, 's: 't>(
        source: &'s str,
        tokens: &'t [(Token<'s>, SimpleSpan)],
    ) -> impl TokenInput<'t, 's> {
        let eoi = SimpleSpan::from(source.len()..source.len());
        tokens.map(eoi, |(token, span)| (token, span))
    }

    // ----- Whitespace and punctuation -----

    /// Match exactly one [`Token::Space`].
    ///
    /// Whitespace is a token rather than something skipped implicitly because
    /// `in 5 minutes` is a time expression and `in 5minutes` is not. A `Space`
    /// token stands for a whole run of whitespace, so this also covers the
    /// repeated `one_of(" \t\n\r").at_least(1)` the character-level grammar
    /// used.
    pub fn space<'t, 's: 't, I>() -> impl Parser<'t, I, (), ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        just(Token::Space).ignored().labelled("whitespace")
    }

    /// Match an optional [`Token::Space`].
    ///
    /// The token-level replacement for `text::whitespace()`. Combine it with
    /// [`Parser::padded_by`] to replace a top-level `.padded()`.
    pub fn opt_space<'t, 's: 't, I>() -> impl Parser<'t, I, (), ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        just(Token::Space).or_not().ignored()
    }

    /// Match a single punctuation character, e.g. `punct(':')`.
    ///
    /// The lexer emits every non-alphanumeric, non-whitespace character as its
    /// own [`Token::Punct`], so this is the token-level `just(':')`.
    pub fn punct<'t, 's: 't, I>(c: char) -> impl Parser<'t, I, (), ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        just(Token::Punct(c)).ignored()
    }

    // ----- Words -----

    /// Match a whole [`Token::Word`] against `target`, case-insensitively.
    ///
    /// The comparison is Unicode-aware (`char::to_lowercase`), not
    /// `eq_ignore_ascii_case`, because German keywords contain umlauts:
    /// `word_ci("nächsten")` must accept `Nächsten`.
    ///
    /// Matching is whole-slice: `word_ci("day")` never matches `days`, and
    /// `word_ci("m")` never matches `min`, whatever order alternatives appear
    /// in. Use [`phrase_ci`] for anything containing a space or punctuation.
    pub fn word_ci<'t, 's: 't, I>(
        target: &'static str,
    ) -> impl Parser<'t, I, (), ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        select! { Token::Word(word) if eq_ignore_case(word, target) => () }.labelled(target)
    }

    /// Match a whole [`Token::Word`] against `target`, case-**sensitively**.
    ///
    /// For languages where capitalisation carries meaning — German nouns
    /// (`Tagen`, `Montag`) and the ISO 8601 `T` and `Z` designators.
    pub fn word_cs<'t, 's: 't, I>(
        target: &'static str,
    ) -> impl Parser<'t, I, (), ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        select! { Token::Word(word) if word == target => () }.labelled(target)
    }

    /// Compare two strings for equality under Unicode simple lowercase folding.
    fn eq_ignore_case(a: &str, b: &str) -> bool {
        let mut a = a.chars().flat_map(char::to_lowercase);
        let mut b = b.chars().flat_map(char::to_lowercase);
        loop {
            match (a.next(), b.next()) {
                (None, None) => return true,
                (x, y) if x == y => (),
                _ => return false,
            }
        }
    }

    // ----- Phrases -----

    /// Match a multi-token phrase case-insensitively, e.g.
    /// `phrase_ci("day after tomorrow")` or `phrase_ci("a.m.")`.
    ///
    /// `target` is lexed with the very same [`lex`] the input goes through, and
    /// the resulting tokens are matched in sequence. A space in `target`
    /// therefore requires a [`Token::Space`] in the input (one whitespace run,
    /// of any width), and punctuation matches punctuation.
    ///
    /// A single-word `target` is simply [`word_ci`], so this is always the safe
    /// choice when the phrase is built from a table of keywords.
    pub fn phrase_ci<'t, 's: 't, I>(
        target: &'static str,
    ) -> impl Parser<'t, I, (), ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        phrase(target, Case::Insensitive)
    }

    /// Case-sensitive counterpart of [`phrase_ci`].
    pub fn phrase_cs<'t, 's: 't, I>(
        target: &'static str,
    ) -> impl Parser<'t, I, (), ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        phrase(target, Case::Sensitive)
    }

    /// Build a case-insensitive alternation over `(phrase, value)` pairs,
    /// trying the phrase with the most tokens first.
    ///
    /// Tokenising removes *sub-word* shadowing but not *phrase-prefix*
    /// shadowing: `choice` still commits to the first alternative that
    /// succeeds, so `"a"` listed before `"a couple of"` would consume the `a`
    /// of `a couple of days ago`, leave `couple` behind, and doom the enclosing
    /// rule. Sorting here makes the source order of the table irrelevant
    /// instead of load-bearing.
    ///
    /// # Panics
    ///
    /// Panics if `pairs` is empty.
    pub fn phrases_ci<'t, 's: 't, I, T>(
        pairs: impl IntoIterator<Item = (&'static str, T)>,
    ) -> impl Parser<'t, I, T, ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
        T: Clone + 't,
    {
        phrase_alternation(pairs, Case::Insensitive)
    }

    /// Case-sensitive counterpart of [`phrases_ci`].
    ///
    /// # Panics
    ///
    /// Panics if `pairs` is empty.
    pub fn phrases_cs<'t, 's: 't, I, T>(
        pairs: impl IntoIterator<Item = (&'static str, T)>,
    ) -> impl Parser<'t, I, T, ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
        T: Clone + 't,
    {
        phrase_alternation(pairs, Case::Sensitive)
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Case {
        Sensitive,
        Insensitive,
    }

    /// Match one token of a lexed phrase pattern.
    fn pattern_token<'t, 's: 't, I>(token: Token<'static>, case: Case) -> BoxedParser<'t, 's, I, ()>
    where
        I: TokenInput<'t, 's>,
    {
        match token {
            Token::Word(word) => match case {
                Case::Sensitive => word_cs(word).boxed(),
                Case::Insensitive => word_ci(word).boxed(),
            },
            Token::Number(digits) => {
                select! { Token::Number(found) if found == digits => () }.boxed()
            }
            Token::Punct(c) => punct(c).boxed(),
            Token::Space => space().boxed(),
        }
    }

    /// Lex `target` and match its tokens in sequence.
    fn phrase<'t, 's: 't, I>(target: &'static str, case: Case) -> BoxedParser<'t, 's, I, ()>
    where
        I: TokenInput<'t, 's>,
    {
        let mut tokens = lex(target).into_iter().map(|(token, _)| token);
        let first = tokens.next().expect("phrase must be non-empty");
        let mut parser = pattern_token(first, case);
        for token in tokens {
            parser = parser.then_ignore(pattern_token(token, case)).boxed();
        }
        parser.labelled(target).boxed()
    }

    fn phrase_alternation<'t, 's: 't, I, T>(
        pairs: impl IntoIterator<Item = (&'static str, T)>,
        case: Case,
    ) -> BoxedParser<'t, 's, I, T>
    where
        I: TokenInput<'t, 's>,
        T: Clone + 't,
    {
        let mut pairs: Vec<(&'static str, T)> = pairs.into_iter().collect();
        // Most tokens first; character count breaks ties so the ordering is
        // total and deterministic.
        pairs.sort_by_key(|(phrase, _)| {
            std::cmp::Reverse((lex(phrase).len(), phrase.chars().count()))
        });

        let mut pairs = pairs.into_iter();
        let (first_phrase, first_value) = pairs.next().expect("phrase set must be non-empty");
        let mut parser = phrase(first_phrase, case).to(first_value).boxed();
        for (pattern, value) in pairs {
            parser = parser.or(phrase(pattern, case).to(value)).boxed();
        }
        parser
    }

    // ----- Numbers -----

    /// Parse a [`Token::Number`] of any width as an `i64`.
    ///
    /// The range check is *emitted* rather than returned. A returned custom
    /// error is registered at the cursor where this parser started, so the
    /// enclosing `number()` table's `.labelled("number")` rewrites it — and
    /// `Rich::label_with` takes `found` from `take_found()`, which is `None` for
    /// a custom reason. An out-of-range amount therefore used to be reported as
    /// "expected number, found end of input" *while the offending digits were
    /// underlined*. Emitted errors are exempt from both the furthest-error
    /// ranking and the relabelling, and `into_result` still fails the parse.
    pub fn digit_number<'t, 's: 't, I>() -> impl Parser<'t, I, i64, ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        select! { Token::Number(digits) => digits }.validate(|digits: &str, extra, emitter| {
            match digits.parse::<i64>() {
                Ok(value) => value,
                Err(error) => {
                    emitter.emit(Rich::custom(extra.span(), error.to_string()));
                    // The parse fails on the emitted error above, so this value
                    // only has to be representable.
                    i64::MAX
                }
            }
        })
    }

    /// Parse a 1 or 2 digit [`Token::Number`] as a `u8`.
    ///
    /// The width check is what makes `123:45` fail: the lexer produces a single
    /// `Number("123")` token, which cannot be split into `12` plus a leftover
    /// `3`, so no alternative can quietly consume part of it.
    pub fn two_digit_number<'t, 's: 't, I>() -> impl Parser<'t, I, u8, ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        select! { Token::Number(digits) if matches!(digits.len(), 1 | 2) => digits }.try_map(
            |digits: &str, span| {
                digits
                    .parse::<u8>()
                    .map_err(|e| Rich::custom(span, e.to_string()))
            },
        )
    }

    /// Parse an exactly-4-digit [`Token::Number`] as a `u16`.
    pub fn four_digit_number<'t, 's: 't, I>() -> impl Parser<'t, I, u16, ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        select! { Token::Number(digits) if digits.len() == 4 => digits }
            .try_map(|digits: &str, span| {
                digits
                    .parse::<u16>()
                    .map_err(|e| Rich::custom(span, e.to_string()))
            })
            .labelled("4-digit year")
    }

    // ----- ISO 8601 -----

    fn offset_timezone<'t, 's: 't, I>() -> impl Parser<'t, I, Timezone, ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        select! { Token::Punct(sign) if sign == '+' || sign == '-' => sign }
            .then(two_digit_number())
            .then(punct(':').ignore_then(two_digit_number()).or_not())
            .try_map(|((sign, hours), minutes), span| {
                let minutes = minutes.unwrap_or(0);
                if minutes > 59 {
                    return Err(Rich::custom(span, "timezone minute offset out of range"));
                }
                let magnitude = i16::from(hours)
                    .checked_mul(60)
                    .and_then(|h| h.checked_add(i16::from(minutes)))
                    .ok_or_else(|| Rich::custom(span, "timezone hour offset out of range"))?;
                let total_minutes = if sign == '+' { magnitude } else { -magnitude };
                let offset = Timezone::Offset { total_minutes };

                if time_utils::is_valid_timezone_offset(offset) {
                    Ok(offset)
                } else {
                    Err(Rich::custom(span, "invalid timezone offset"))
                }
            })
    }

    fn timezone<'t, 's: 't, I>() -> impl Parser<'t, I, Timezone, ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        // `Z` is a designator, not a word to be case-folded: `z` is not UTC.
        choice((word_cs("Z").to(Timezone::Utc), offset_timezone()))
    }

    fn fractional_seconds<'t, 's: 't, I>() -> impl Parser<'t, I, u32, ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        select! { Token::Number(digits) => digits }.try_map(|s: &str, span| {
            let fraction = if s.len() > 9 { &s[..9] } else { s };
            let parsed: u32 = fraction
                .parse()
                .map_err(|e: std::num::ParseIntError| Rich::custom(span, e.to_string()))?;
            let fraction_len =
                u32::try_from(fraction.len()).expect("fraction length is capped at 9 digits");
            Ok(parsed * 10_u32.pow(9 - fraction_len))
        })
    }

    /// Parse ISO 8601 datetime format.
    ///
    /// Supports:
    /// - Date only: `2024-01-15`
    /// - Date and time: `2024-01-15T14:30:00`
    /// - With timezone: `2024-01-15T14:30:00Z`
    /// - With offset: `2024-01-15T14:30:00+02:00`
    /// - With fractional seconds: `2024-01-15T14:30:00.123Z`
    pub fn iso_datetime<'t, 's: 't, I>()
    -> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
    where
        I: TokenInput<'t, 's>,
    {
        let date = four_digit_number()
            .then_ignore(punct('-'))
            .then(two_digit_number())
            .then_ignore(punct('-'))
            .then(two_digit_number())
            .validate(|((year, month), day), extra, emitter| {
                if !time_utils::is_valid_calendar_date(year, month, day) {
                    // Emit instead of failing with `try_map`. A `try_map` error
                    // is registered at the cursor where the mapped parser
                    // *started*, and chumsky keeps the competing alternative
                    // that reached furthest — so the deliberate diagnostic was
                    // silently replaced by an unrelated "expected whitespace,
                    // found `-`" from an alternative that had merely read the
                    // year as an amount. Emitted errors are exempt from that
                    // ranking, and `ParseResult::into_result` still fails the
                    // parse because it discards output whenever any error
                    // exists, so the date is rejected exactly as before.
                    emitter.emit(Rich::custom(extra.span(), INVALID_CALENDAR_DATE));
                }
                (year, month, day)
            });

        // The date/time separator is either the ISO `T` designator — lexed as a
        // one-letter word between two numbers — or a space.
        let separator = choice((word_cs("T"), space()));

        let time = separator
            .ignore_then(two_digit_number())
            .then_ignore(punct(':'))
            .then(two_digit_number())
            .then(
                punct(':')
                    .ignore_then(two_digit_number())
                    .then(punct('.').ignore_then(fractional_seconds()).or_not())
                    .or_not(),
            )
            .then(timezone().or_not())
            .validate(|(((hour, minute), sec_part), tz), extra, emitter| {
                let second = sec_part.as_ref().map_or(0, |(s, _)| *s);
                if !time_utils::is_valid_24_hour_time(hour, minute, second) {
                    // Emitted for the same reason as the calendar-date check
                    // above: a `try_map` failure here loses chumsky's
                    // furthest-error ranking to an unrelated alternative.
                    emitter.emit(Rich::custom(extra.span(), INVALID_TIME_OF_DAY));
                }
                (hour, minute, sec_part, tz)
            });

        date.then(time.or_not())
            .map(|((year, month, day), time_opt)| match time_opt {
                Some((h, m, sec_part, tz)) => {
                    let (second, nanosecond) = match sec_part {
                        Some((s, frac)) => (Some(s), frac),
                        None => (None, None),
                    };
                    TimeExpression::Absolute(AbsoluteTime {
                        year,
                        month,
                        day,
                        hour: Some(h),
                        minute: Some(m),
                        second,
                        nanosecond,
                        timezone: tz,
                    })
                }
                None => TimeExpression::Absolute(AbsoluteTime {
                    year,
                    month,
                    day,
                    hour: None,
                    minute: None,
                    second: None,
                    nanosecond: None,
                    timezone: None,
                }),
            })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Lex `$input`, run `$parser` over the whole of it, and yield
        /// `Option<Output>`.
        ///
        /// A macro rather than a function because the input type
        /// [`token_stream`] returns is opaque, so a caller cannot name it in a
        /// `where` clause.
        macro_rules! run {
            ($input:expr, $parser:expr) => {{
                let input: &str = $input;
                let tokens = lex(input);
                $parser
                    .then_ignore(end())
                    .parse(token_stream(input, &tokens))
                    .into_result()
                    .ok()
            }};
        }

        #[test]
        fn word_ci_matches_whole_words_only() {
            assert!(run!("day", word_ci("day")).is_some());
            assert!(run!("DAY", word_ci("day")).is_some());
            // The bug the lexer exists to prevent: `day` inside `days`.
            assert!(run!("days", word_ci("day")).is_none());
            assert!(run!("min", word_ci("m")).is_none());
        }

        #[test]
        fn word_ci_folds_umlauts() {
            assert!(run!("nächsten", word_ci("nächsten")).is_some());
            assert!(run!("Nächsten", word_ci("nächsten")).is_some());
            assert!(run!("NÄCHSTEN", word_ci("nächsten")).is_some());
        }

        #[test]
        fn word_cs_respects_case() {
            assert!(run!("Montag", word_cs("Montag")).is_some());
            assert!(run!("montag", word_cs("Montag")).is_none());
        }

        #[test]
        fn phrases_span_spaces_and_punctuation() {
            assert!(run!("day after tomorrow", phrase_ci("day after tomorrow")).is_some());
            assert!(run!("A.M.", phrase_ci("a.m.")).is_some());
            assert!(run!("day after", phrase_ci("day after tomorrow")).is_none());
            // No implicit whitespace: the phrase's space is a real token.
            assert!(run!("halfpast", phrase_ci("half past")).is_none());
        }

        #[test]
        fn phrase_alternation_prefers_the_longer_phrase() {
            let pairs = || [("a", 1i64), ("a couple of", 2), ("a few", 3)];
            assert_eq!(run!("a couple of", phrases_ci(pairs())), Some(2));
            assert_eq!(run!("a few", phrases_ci(pairs())), Some(3));
            assert_eq!(run!("a", phrases_ci(pairs())), Some(1));
        }

        #[test]
        fn number_widths_are_enforced() {
            assert_eq!(run!("7", two_digit_number()), Some(7));
            assert_eq!(run!("07", two_digit_number()), Some(7));
            // A 3-digit number is one token and cannot be truncated to two.
            assert_eq!(run!("123", two_digit_number()), None);
            assert_eq!(run!("2024", four_digit_number()), Some(2024));
            assert_eq!(run!("204", four_digit_number()), None);
            assert_eq!(run!("12345", digit_number()), Some(12345));
        }

        #[test]
        fn iso_datetime_round_trips() {
            let expected = TimeExpression::Absolute(AbsoluteTime {
                year: 2024,
                month: 1,
                day: 15,
                hour: Some(14),
                minute: Some(30),
                second: Some(0),
                nanosecond: None,
                timezone: Some(Timezone::Utc),
            });
            assert_eq!(run!("2024-01-15T14:30:00Z", iso_datetime()), Some(expected));

            assert_eq!(
                run!("2024-01-15T14:30:00-00:30", iso_datetime()),
                Some(TimeExpression::Absolute(AbsoluteTime {
                    year: 2024,
                    month: 1,
                    day: 15,
                    hour: Some(14),
                    minute: Some(30),
                    second: Some(0),
                    nanosecond: None,
                    timezone: Some(Timezone::Offset { total_minutes: -30 }),
                }))
            );

            // Invalid calendar date and invalid clock time are both rejected.
            assert!(run!("2024-02-30", iso_datetime()).is_none());
            assert!(run!("2024-01-15T25:00", iso_datetime()).is_none());
        }

        /// The shadowing hazard the grammar is left-factored to avoid: a bare
        /// `tomorrow` listed first under `choice` commits, strands `morning`,
        /// and the enclosing `end()` then fails. Factoring the shared prefix
        /// and making the tail optional is what removes it.
        fn day_then_optional_part<'t, 's: 't, I>()
        -> impl Parser<'t, I, i64, ParserError<'t, 's>> + Clone
        where
            I: TokenInput<'t, 's>,
        {
            word_ci("tomorrow")
                .ignore_then(space().ignore_then(word_ci("morning")).or_not())
                .map(|morning| if morning.is_some() { 2 } else { 1 })
        }

        #[test]
        fn left_factoring_removes_the_shadowing() {
            assert_eq!(run!("tomorrow morning", day_then_optional_part()), Some(2));
            assert_eq!(run!("tomorrow", day_then_optional_part()), Some(1));
        }
    }
}

// ===== Language Support =====

/// Language-specific parser implementations.
///
/// Each submodule contains a parser for a specific language.
/// All parsers implement the `LanguageParser` trait.
pub mod language {
    /// English language parser.
    ///
    /// Supports expressions like:
    /// - "in 5 minutes", "3 days ago"
    /// - "tomorrow at 3:30 pm"
    /// - "next Monday", "last Friday"
    pub mod english;

    /// German language parser.
    ///
    /// Supports expressions like:
    /// - "in 5 Minuten", "vor 3 Tagen"
    /// - "morgen um 15:30"
    /// - "nächsten Montag", "letzten Freitag"
    pub mod german;
}

// ===== Main Parsing Function =====

/// Parse a natural language time expression.
///
/// This is the main entry point for parsing time expressions. It takes
/// a string input and a language, and returns a parsed `TimeExpression`.
///
/// # Arguments
///
/// * `input` - The natural language time expression to parse
/// * `language` - The language to use for parsing
///
/// # Returns
///
/// Returns `Ok(TimeExpression)` if parsing succeeds, or `Err(TempsError)`
/// if the input cannot be parsed.
///
/// # Examples
///
/// ```
/// use temps_core::{parse, Language, TimeExpression};
///
/// // Parse English expressions
/// let expr = parse("in 5 minutes", Language::English).unwrap();
/// let expr = parse("tomorrow at 3:30 pm", Language::English).unwrap();
/// let expr = parse("next Monday", Language::English).unwrap();
///
/// // Parse German expressions
/// let expr = parse("in 5 Minuten", Language::German).unwrap();
/// let expr = parse("morgen um 15:30", Language::German).unwrap();
/// let expr = parse("nächsten Montag", Language::German).unwrap();
///
/// // Parse ISO datetime (works in any language)
/// let expr = parse("2024-01-15T14:30:00Z", Language::English).unwrap();
/// ```
///
/// # Supported Formats
///
/// ## Relative Time
/// - "in 5 minutes", "5 minutes ago"
/// - "in 2 hours", "an hour ago"
/// - "in 3 days", "2 days ago"
/// - "in a week", "2 weeks ago"
/// - "in 6 months", "a month ago"
/// - "in 2 years", "a year ago"
///
/// ## Day References
/// - "today", "yesterday", "tomorrow"
/// - "Monday", "Tuesday", etc.
/// - "next Monday", "last Friday"
///
/// ## Times
/// - "3:30 pm", "10:15 am"
/// - "14:30", "09:00"
///
/// ## Dates
/// - "15/03/2024", "31-12-2025"
///
/// ## Combined
/// - "tomorrow at 3:30 pm"
/// - "next Monday at 9:00 am"
///
/// ## ISO Format
/// - "2024-01-15T14:30:00Z"
/// - "2024-01-15T14:30:00+02:00"
/// - "2024-01-15T14:30:00.123Z"
pub fn parse(input: &str, language: Language) -> Result<TimeExpression> {
    match language {
        Language::English => language::english::EnglishParser.parse(input),
        Language::German => language::german::GermanParser.parse(input),
    }
}
