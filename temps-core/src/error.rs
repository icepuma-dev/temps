//! Error types for the temps library.
//!
//! This module defines the error types used throughout the temps ecosystem.
//! All parsing and date calculation operations return `Result<T, TempsError>`.
//!
//! # Error Categories
//!
//! - **Parse Errors**: When input cannot be parsed as a valid time expression
//! - **Date Calculation Errors**: When date arithmetic results in invalid dates
//! - **Invalid Component Errors**: When date/time components are out of range
//! - **Backend Errors**: When the underlying datetime library reports an error
//!
//! # Examples
//!
//! ```
//! use temps_core::{parse, Language, TempsError};
//!
//! // Parse error example
//! let result = parse("invalid input", Language::English);
//! match result {
//!     Err(TempsError::ParseError { message, input, position }) => {
//!         println!("Parse failed: {}", message);
//!     }
//!     _ => {}
//! }
//! ```

use thiserror::Error;

/// The main error type for the temps library.
///
/// This enum represents all possible errors that can occur during
/// parsing and time calculation operations.
#[derive(Error, Debug, Clone, PartialEq, Eq, Hash)]
pub enum TempsError {
    /// Error that occurs during parsing of time expressions.
    ///
    /// This error is returned when the input string cannot be parsed
    /// as a valid time expression in the specified language.
    ///
    /// # Example
    ///
    /// ```
    /// use temps_core::TempsError;
    ///
    /// let err = TempsError::parse_error("Unrecognized time unit", "in 5 blargs");
    /// ```
    #[error("Failed to parse time expression: {message}")]
    ParseError {
        /// The specific parsing error message
        message: String,
        /// The input that failed to parse
        input: String,
        /// Optional position in the input where parsing failed
        position: Option<usize>,
    },

    /// Error that occurs during date/time calculations.
    ///
    /// This error is returned when date arithmetic operations fail,
    /// such as when adding months to January 31st would result in
    /// February 31st (which doesn't exist).
    ///
    /// When a `context` is present it is rendered after the message: it carries
    /// the underlying backend's own diagnosis (jiff's range errors, say), which
    /// used to be captured and then never shown to anyone.
    ///
    /// # Example
    ///
    /// ```
    /// use temps_core::TempsError;
    ///
    /// let err = TempsError::date_calculation("Month overflow");
    /// ```
    #[error(
        "Date calculation error: {message}{}",
        .context.as_deref().map(|cause| format!(": {cause}")).unwrap_or_default()
    )]
    DateCalculationError {
        /// The specific calculation error message
        message: String,
        /// Optional context about what caused the error
        context: Option<String>,
    },

    /// Error for invalid date components
    #[error("Invalid date: year={year}, month={month}, day={day}")]
    InvalidDate {
        /// The year component
        year: u16,
        /// The month component (1-12)
        month: u8,
        /// The day component (1-31)
        day: u8,
    },

    /// Error for invalid time components
    #[error("Invalid time: {hour:02}:{minute:02}:{second:02}")]
    InvalidTime {
        /// The hour component (0-23)
        hour: u8,
        /// The minute component (0-59)
        minute: u8,
        /// The second component (0-59)
        second: u8,
    },

    /// Error for invalid timezone offset
    #[error("{}", temps_core_format_offset(*total_minutes))]
    InvalidTimezoneOffset {
        /// The offset from UTC in minutes (-720 to +840)
        total_minutes: i16,
    },

    /// Error for ambiguous local time (e.g., during DST transitions)
    #[error("Ambiguous local time: {message}")]
    AmbiguousTime {
        /// Description of the ambiguity
        message: String,
    },

    /// Error for arithmetic overflow in date calculations
    #[error("Arithmetic overflow: {operation}")]
    ArithmeticOverflow {
        /// The operation that caused the overflow
        operation: String,
    },

    /// Error for unsupported operations
    #[error("Unsupported operation: {operation}")]
    UnsupportedOperation {
        /// Description of the unsupported operation
        operation: String,
    },

    /// Error from the underlying datetime backend (chrono, jiff, etc.)
    #[error("Backend error: {message}")]
    BackendError {
        /// The error message from the backend
        message: String,
        /// The backend that produced the error
        backend: String,
    },
}

impl TempsError {
    /// Creates a new parse error without position information.
    ///
    /// Use this when you know parsing failed but don't have a specific
    /// position in the input where the error occurred.
    ///
    /// # Arguments
    ///
    /// * `message` - Description of what went wrong
    /// * `input` - The input string that failed to parse
    ///
    /// # Example
    ///
    /// ```
    /// use temps_core::TempsError;
    ///
    /// let err = TempsError::parse_error(
    ///     "Expected time unit",
    ///     "in 5"
    /// );
    /// ```
    #[must_use]
    pub fn parse_error(message: impl Into<String>, input: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
            input: input.into(),
            position: None,
        }
    }

    /// Creates a new parse error with position information.
    ///
    /// Use this when you know exactly where in the input the parse error occurred.
    ///
    /// # Arguments
    ///
    /// * `message` - Description of what went wrong
    /// * `input` - The input string that failed to parse
    /// * `position` - Character position where parsing failed
    ///
    /// # Example
    ///
    /// ```
    /// use temps_core::TempsError;
    ///
    /// let err = TempsError::parse_error_with_position(
    ///     "Unexpected character",
    ///     "in 5 minuts",
    ///     9  // Points to the 't' in "minuts"
    /// );
    /// ```
    #[must_use]
    pub fn parse_error_with_position(
        message: impl Into<String>,
        input: impl Into<String>,
        position: usize,
    ) -> Self {
        Self::ParseError {
            message: message.into(),
            input: input.into(),
            position: Some(position),
        }
    }

    /// Creates a new date calculation error.
    ///
    /// Use this for errors that occur during date arithmetic operations.
    ///
    /// # Example
    ///
    /// ```
    /// use temps_core::TempsError;
    ///
    /// let err = TempsError::date_calculation(
    ///     "Cannot subtract 13 months from January"
    /// );
    /// ```
    #[must_use]
    pub fn date_calculation(message: impl Into<String>) -> Self {
        Self::DateCalculationError {
            message: message.into(),
            context: None,
        }
    }

    /// Creates a new date calculation error with additional context.
    ///
    /// Use this when you want to include information about what caused
    /// the calculation to fail (e.g., an error from the backend library).
    ///
    /// # Example
    ///
    /// ```
    /// use temps_core::TempsError;
    ///
    /// let err = TempsError::date_calculation_with_source(
    ///     "Failed to add months",
    ///     "chronos error: date out of range"
    /// );
    /// ```
    #[must_use]
    pub fn date_calculation_with_source(
        message: impl Into<String>,
        context: impl Into<String>,
    ) -> Self {
        Self::DateCalculationError {
            message: message.into(),
            context: Some(context.into()),
        }
    }

    /// Creates an invalid date error
    #[must_use]
    pub fn invalid_date(year: u16, month: u8, day: u8) -> Self {
        Self::InvalidDate { year, month, day }
    }

    /// Creates an invalid time error
    #[must_use]
    pub fn invalid_time(hour: u8, minute: u8, second: u8) -> Self {
        Self::InvalidTime {
            hour,
            minute,
            second,
        }
    }

    /// Creates an invalid timezone offset error
    #[must_use]
    pub fn invalid_timezone_offset(total_minutes: i16) -> Self {
        Self::InvalidTimezoneOffset { total_minutes }
    }

    /// Creates an ambiguous time error
    #[must_use]
    pub fn ambiguous_time(message: impl Into<String>) -> Self {
        Self::AmbiguousTime {
            message: message.into(),
        }
    }

    /// Creates an arithmetic overflow error
    #[must_use]
    pub fn arithmetic_overflow(operation: impl Into<String>) -> Self {
        Self::ArithmeticOverflow {
            operation: operation.into(),
        }
    }

    /// Creates an unsupported operation error
    #[must_use]
    pub fn unsupported_operation(operation: impl Into<String>) -> Self {
        Self::UnsupportedOperation {
            operation: operation.into(),
        }
    }

    /// Creates a backend error
    #[must_use]
    pub fn backend_error(message: impl Into<String>, backend: impl Into<String>) -> Self {
        Self::BackendError {
            message: message.into(),
            backend: backend.into(),
        }
    }
}

/// Result type alias for temps operations.
///
/// All parsing and time calculation operations in the temps library
/// return this result type.
///
/// # Example
///
/// ```
/// use temps_core::Result;
///
/// fn parse_time(input: &str) -> Result<String> {
///     // Implementation
///     Ok("parsed".to_string())
/// }
/// ```
pub type Result<T> = std::result::Result<T, TempsError>;

/// Convert a collection of chumsky parser errors into a [`TempsError`]
/// and an ariadne-rendered diagnostic string.
///
/// The first error's span is used for the position field. The full
/// rendered report (with source context) is folded into the error's
/// message so callers that simply display the error still get a useful,
/// human-readable diagnostic.
/// The parsers run over tokens, so their spans are the lexer's BYTE offsets
/// into the original source — see [`crate::lexer::lex`]. That is exactly what
/// the byte-to-character translation below needs, and it is why umlaut input
/// still gets a caret in the right place.
#[must_use]
pub fn rich_errors_to_temps_error(
    input: &str,
    errors: Vec<chumsky::error::Rich<'_, crate::lexer::Token<'_>>>,
) -> TempsError {
    use ariadne::{Color, Config, Label, Report, ReportKind, Source};
    use chumsky::error::RichReason;

    // Token spans are BYTE offsets, but ariadne's `Source` indexes by
    // CHARACTER. Feeding one to the other mislocates the caret on any
    // non-ASCII input and silently drops the label once the byte offset runs
    // past the character count. Translate up front.
    let byte_to_char = |byte: usize| -> usize {
        input
            .char_indices()
            .position(|(b, _)| b >= byte)
            .unwrap_or_else(|| input.chars().count())
    };
    let char_len = input.chars().count();

    if input.is_empty() {
        return TempsError::parse_error_with_position(
            "input is empty; expected a time expression like `now`, `in 5 minutes`, or an ISO date",
            input,
            0,
        );
    }

    // Prefer a deliberate diagnostic. The grammar raises `Rich::custom` exactly
    // where it *knows* what is wrong — an impossible calendar date, an amount
    // too large for an `i64`, a timezone offset out of range — whereas the
    // errors it competes with are "expected X, found Y" from whichever
    // alternative happened to read furthest into the input. Ranking those
    // alternatives is chumsky's job; deciding which survivor is worth showing
    // to a user is ours, and a stated reason beats a guess.
    let mut ordered: Vec<&chumsky::error::Rich<'_, crate::lexer::Token<'_>>> = errors
        .iter()
        .filter(|e| matches!(e.reason(), RichReason::Custom(_)))
        .collect();
    ordered.extend(
        errors
            .iter()
            .filter(|e| !matches!(e.reason(), RichReason::Custom(_))),
    );

    // A parse that failed always carries at least one error; the `0` is only
    // for the empty vector.
    let position = ordered.first().map_or(0, |e| byte_to_char(e.span().start));

    let source_id: &str = "input";
    let mut rendered = String::new();
    for err in &ordered {
        let span = err.span();
        let start = byte_to_char(span.start);
        let end = byte_to_char(span.end).max(start + 1).min(char_len.max(1));
        let range = start..end;
        let mut buf = Vec::new();
        let (headline, detail) = format_rich(err);
        let report = Report::build(ReportKind::Error, (source_id, range.clone()))
            .with_config(Config::default().with_color(false))
            .with_message(headline)
            .with_label(
                Label::new((source_id, range))
                    .with_message(detail)
                    .with_color(Color::Red),
            )
            .finish();

        if report
            .write((source_id, Source::from(input)), &mut buf)
            .is_ok()
        {
            rendered.push_str(&String::from_utf8_lossy(&buf));
        } else {
            rendered.push_str(&err.to_string());
            rendered.push('\n');
        }
    }

    let message = if rendered.is_empty() {
        "Failed to parse time expression".to_string()
    } else {
        rendered.trim_end().to_string()
    };

    TempsError::parse_error_with_position(message, input, position)
}

/// Render a chumsky [`Rich`](chumsky::error::Rich) error as a `(headline, detail)`
/// pair suitable for an ariadne report.
fn format_rich(err: &chumsky::error::Rich<'_, crate::lexer::Token<'_>>) -> (String, String) {
    use crate::lexer::Token;
    use chumsky::error::RichReason;

    match err.reason() {
        RichReason::Custom(msg) => ("invalid time expression".to_string(), msg.clone()),
        _ => {
            // `Token`'s `Display` already spells `Space` out as "whitespace",
            // which reads badly inside backticks.
            let found = match err.found() {
                Some(Token::Space) => "whitespace".to_string(),
                Some(token) => format!("`{token}`"),
                None => "end of input".to_string(),
            };

            let mut seen = std::collections::BTreeSet::new();
            let mut expected: Vec<String> = Vec::new();
            for pat in err.expected() {
                let rendered = pat.to_string();
                if seen.insert(rendered.clone()) {
                    expected.push(rendered);
                }
            }

            let detail = match expected.as_slice() {
                [] => format!("unexpected {found}"),
                [one] => format!("expected {one}, found {found}"),
                many => {
                    let last = many.last().expect("non-empty");
                    let head = &many[..many.len() - 1];
                    format!(
                        "expected one of {} or {}, found {found}",
                        head.join(", "),
                        last
                    )
                }
            };

            ("could not parse time expression".to_string(), detail)
        }
    }
}

/// Render an offset for [`TempsError::InvalidTimezoneOffset`]'s `Display`.
fn temps_core_format_offset(total_minutes: i16) -> String {
    crate::errors::format_invalid_timezone_offset(total_minutes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = TempsError::invalid_date(2024, 13, 32);
        assert_eq!(err.to_string(), "Invalid date: year=2024, month=13, day=32");

        let err = TempsError::invalid_time(25, 61, 61);
        assert_eq!(err.to_string(), "Invalid time: 25:61:61");

        let err = TempsError::parse_error("unexpected token", "in 5 minuts");
        assert_eq!(
            err.to_string(),
            "Failed to parse time expression: unexpected token"
        );
    }

    #[test]
    fn test_error_creation_helpers() {
        let err = TempsError::date_calculation("month out of range");
        match err {
            TempsError::DateCalculationError { message, context } => {
                assert_eq!(message, "month out of range");
                assert!(context.is_none());
            }
            _ => panic!("Wrong error type"),
        }

        let err = TempsError::backend_error("conversion failed", "chrono");
        match err {
            TempsError::BackendError { message, backend } => {
                assert_eq!(message, "conversion failed");
                assert_eq!(backend, "chrono");
            }
            _ => panic!("Wrong error type"),
        }
    }

    /// The lexer's spans are byte offsets, ariadne indexes characters. An
    /// umlaut before the error site is what tells the two apart.
    #[test]
    fn parse_error_position_is_a_character_offset() {
        use crate::common::{ParserError, TokenInput, space, token_stream, word_ci};
        use crate::lexer::lex;
        use chumsky::prelude::*;

        fn expects_zwei<'t, 's: 't, I>() -> impl Parser<'t, I, (), ParserError<'t, 's>> + Clone
        where
            I: TokenInput<'t, 's>,
        {
            word_ci("in")
                .then_ignore(space())
                .then_ignore(word_ci("zwei"))
                .ignored()
        }

        let input = "in fünf";
        let tokens = lex(input);
        let errors = expects_zwei()
            .then_ignore(end())
            .parse(token_stream(input, &tokens))
            .into_result()
            .expect_err("`fünf` is not `zwei`");

        match rich_errors_to_temps_error(input, errors) {
            TempsError::ParseError {
                message, position, ..
            } => {
                // Byte offset 3, and character offset 3 too — but the label
                // ariadne draws spans `fünf`, which is 5 bytes and 4 chars.
                assert_eq!(position, Some(3));
                assert!(message.contains("fünf"), "{message}");
                assert!(message.contains("zwei"), "{message}");
            }
            other => panic!("expected a parse error, got {other:?}"),
        }
    }

    #[test]
    fn empty_input_gets_a_dedicated_message() {
        let err = rich_errors_to_temps_error("", Vec::new());
        match err {
            TempsError::ParseError {
                message, position, ..
            } => {
                assert_eq!(position, Some(0));
                assert!(message.contains("input is empty"), "{message}");
            }
            other => panic!("expected a parse error, got {other:?}"),
        }
    }
}
