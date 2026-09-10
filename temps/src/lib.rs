//! # temps
//!
//! A natural language time expression parser for Rust.
//!
//! This crate provides a unified interface for parsing human-readable time expressions
//! like "in 5 minutes", "tomorrow at 3pm", or "next Monday" into concrete datetime
//! values. It supports multiple datetime backends through feature flags.
//!
//! ## Features
//!
//! - **Natural language parsing**: Parse expressions like "in 2 hours", "yesterday", "next Friday"
//! - **Multiple languages**: Currently supports English and German
//! - **Backend flexibility**: Choose between chrono and jiff datetime libraries
//! - **Type-safe**: Strong typing for time units, directions, and expressions
//! - **Comprehensive error handling**: Detailed error types for debugging
//!
//! ## Quick Start
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! temps = { version = "5", features = ["chrono"] }
//! # or
//! temps = { version = "5", features = ["jiff"] }
//! ```
//!
//! ## Examples
//!
//! ### Using with chrono
//!
//! ```rust
//! # #[cfg(feature = "chrono")] {
//! use temps::chrono::{parse_to_datetime, Language};
//!
//! let dt = parse_to_datetime("in 30 minutes", Language::English).unwrap();
//! println!("In 30 minutes: {}", dt);
//!
//! let dt = parse_to_datetime("morgen um 15:30", Language::German).unwrap();
//! println!("Tomorrow at 15:30: {}", dt);
//! # }
//! ```
//!
//! ### Using with jiff
//!
//! ```rust
//! # #[cfg(feature = "jiff")] {
//! use temps::jiff::{parse_to_zoned, Language};
//!
//! let dt = parse_to_zoned("next Monday at 9:00 am", Language::English).unwrap();
//! println!("Next Monday at 9:00 am: {}", dt);
//! # }
//! ```
//!
//! ### Lower-level API
//!
//! ```rust
//! # #[cfg(feature = "chrono")] {
//! use temps::chrono::{parse, Language, TimeExpression, ChronoProvider, TimeParser};
//!
//! // Parse to an expression first
//! let expr = parse("in 2 hours", Language::English).unwrap();
//!
//! // Then convert to datetime
//! let provider = ChronoProvider::new();
//! let dt = provider.parse_expression(expr).unwrap();
//! # }
//! ```
//!
//! ## Supported Expressions
//!
//! ### Relative Time
//! - "in 5 minutes" / "in 5 Minuten"
//! - "2 hours ago" / "vor 2 Stunden"
//! - "in 3 days" / "in 3 Tagen"
//!
//! ### Day References
//! - "today" / "heute"
//! - "tomorrow" / "morgen"
//! - "yesterday" / "gestern"
//! - "next Monday" / "nächsten Montag"
//! - "last Friday" / "letzten Freitag"
//!
//! ### Absolute Times
//! - "3:30 pm" / "15:30"
//! - "tomorrow at 10:00 am" / "morgen um 10:00"
//! - "2024-12-25T15:30:00Z" (ISO format)
//!
//! ## Feature Flags
//!
//! - `chrono`: Enable chrono datetime backend
//! - `jiff`: Enable jiff datetime backend
//!
//! At least one backend must be enabled.
//!
//! # README
//!
//! The project README is included here so its examples are compiled by
//! `cargo test --doc`. Nothing else in the workspace imports it — it is only the
//! `readme` field of the manifest — so without this the published examples can
//! rot silently: an earlier revision still matched `TimeExpression` with seven
//! arms after `LaterToday` made it eight, and no build ever noticed.
#![doc = include_str!("../../README.md")]

/// Chrono backend support.
///
/// This module is available when the `chrono` feature is enabled.
/// It provides chrono-specific types and functions for parsing time expressions
/// into `chrono::DateTime<Local>` values.
///
/// This module re-exports all necessary types for using temps with chrono.
#[cfg(feature = "chrono")]
pub mod chrono {

    /// The chrono-based time parser implementation
    pub use temps_chrono::ChronoProvider;
    /// Convenience function to parse directly to DateTime<Local>
    pub use temps_chrono::parse_to_datetime;
    /// Re-export all core types
    pub use temps_core::*;
}

/// Jiff backend support.
///
/// This module is available when the `jiff` feature is enabled.
/// It provides jiff-specific types and functions for parsing time expressions
/// into `jiff::Zoned` values.
///
/// This module re-exports all necessary types for using temps with jiff.
#[cfg(feature = "jiff")]
pub mod jiff {

    /// Re-export all core types
    pub use temps_core::*;
    /// The jiff-based time parser implementation
    pub use temps_jiff::JiffProvider;
    /// Convenience function to parse directly to Zoned
    pub use temps_jiff::parse_to_zoned;
}
