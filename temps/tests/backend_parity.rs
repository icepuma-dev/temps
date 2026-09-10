//! The two backends must agree with each other.
//!
//! `temps-jiff`'s documentation used to claim that the year-9999 range limit was
//! "the one place the two backends disagree" — while, at the same time, chrono's
//! month/year arithmetic was erroring out on any target that landed in a DST
//! fold, and its resolution of a fold's upper boundary was returning an instant
//! an hour early. Both were reachable from ordinary input.
//!
//! The cases pinned here are *zone-independent*: every input names its own
//! offset, so the instant it denotes does not depend on the process `TZ` and the
//! assertions hold wherever the suite runs. Zone-dependent parity (day
//! references, wall-clock times, calendar arithmetic) is pinned in the
//! zone-pinned tests of `temps-chrono`, which is the crate that can re-run
//! itself under a chosen `TZ`.

#![cfg(all(feature = "chrono", feature = "jiff"))]

use temps_core::Language;

/// The instant a string denotes must not depend on which backend resolves it.
#[test]
fn both_backends_agree_on_absolute_instants() {
    for input in [
        "2024-01-15T12:00:00Z",
        "2024-01-15T12:00:00+02:00",
        "2024-01-15T12:00:00-05:30",
        "2024-01-15T00:00:00Z",
        "2024-06-30T23:59:59Z",
        "2024-02-29T00:00:00Z",
        "2025-01-01T00:00:00-12:00",
        "2025-01-01T00:00:00+14:00",
    ] {
        let chrono = temps::chrono::parse_to_datetime(input, Language::English)
            .unwrap_or_else(|error| panic!("chrono failed on {input:?}: {error}"));
        let jiff = temps::jiff::parse_to_zoned(input, Language::English)
            .unwrap_or_else(|error| panic!("jiff failed on {input:?}: {error}"));

        let chrono_instant = chrono
            .timestamp_nanos_opt()
            .unwrap_or_else(|| panic!("chrono's instant for {input:?} is out of nanosecond range"));

        assert_eq!(
            i128::from(chrono_instant),
            jiff.timestamp().as_nanosecond(),
            "the backends disagree on {input:?}"
        );
    }
}

/// Rejection has to agree too: an input one backend accepts and the other does
/// not is a divergence just as much as two different instants.
#[test]
fn both_backends_reject_the_same_impossible_instants() {
    for input in [
        "2024-02-30T12:00:00Z",
        "2024-13-01T00:00:00Z",
        "2024-01-15T25:00:00Z",
        "2024-01-15T12:60:00Z",
    ] {
        let chrono = temps::chrono::parse_to_datetime(input, Language::English);
        let jiff = temps::jiff::parse_to_zoned(input, Language::English);
        assert!(
            chrono.is_err() && jiff.is_err(),
            "{input:?} must be rejected by both backends (chrono: {chrono:?}, jiff: {jiff:?})"
        );
    }
}
