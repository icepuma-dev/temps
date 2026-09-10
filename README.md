# temps

[![](https://img.shields.io/crates/v/temps.svg)](https://crates.io/crates/temps)
[![](https://github.com/icepuma/temps/actions/workflows/ci.yml/badge.svg)](https://github.com/icepuma/temps/actions/workflows/ci.yml)
[![](https://img.shields.io/crates/d/temps.svg)](https://crates.io/crates/temps)

`temps` or `[tã]` is a library for working with time and dates in Rust. Parse human-readable time expressions.

```rust
use temps::chrono::{parse_to_datetime, Language};

// English
let dt = parse_to_datetime("in 3 hours", Language::English).unwrap();
let dt = parse_to_datetime("5 minutes ago", Language::English).unwrap();
let dt = parse_to_datetime("tomorrow", Language::English).unwrap();
let dt = parse_to_datetime("next monday", Language::English).unwrap();
let dt = parse_to_datetime("2024-12-25T15:30:00Z", Language::English).unwrap();

// German  
let dt = parse_to_datetime("in 3 Stunden", Language::German).unwrap();
let dt = parse_to_datetime("vor 5 Minuten", Language::German).unwrap();
let dt = parse_to_datetime("morgen", Language::German).unwrap();
let dt = parse_to_datetime("nächsten Montag", Language::German).unwrap();
```

## Features

- 🌍 Multiple languages (English, German)
- 📅 Relative times (`in 2 hours`, `3 days ago`)
- 📆 Day references (`today`, `yesterday`, `tomorrow`)
- 📅 Weekdays (`monday`, `next friday`, `last wed`)
- 🕐 Time parsing (`3:30 pm`, `14:45`, `9:00 am`)
- 📅 Combined expressions (`tomorrow at 3:30 pm`, `next monday at 9:00`)
- 📆 Date formats (`15/03/2024`, `31-12-2025`, `15.03.2024`)
- 🕐 ISO 8601 dates (`2024-12-25T15:30:00Z`)
- 🔧 Works with `chrono` and `jiff`

## Installation

```toml
[dependencies]
# With chrono
temps = { version = "5", features = ["chrono"] }

# With jiff
temps = { version = "5", features = ["jiff"] }
```

## Usage

### Basic

```rust
use temps::chrono::{parse_to_datetime, Language};

// Relative times
let meeting = parse_to_datetime("in 2 hours", Language::English).unwrap();
let deadline = parse_to_datetime("in 3 days", Language::English).unwrap();
let reminder = parse_to_datetime("in 30 Minuten", Language::German).unwrap();

// Day references
let today = parse_to_datetime("today", Language::English).unwrap();
let tomorrow = parse_to_datetime("morgen", Language::German).unwrap();

// Weekdays
let next_meeting = parse_to_datetime("next tuesday", Language::English).unwrap();
let last_friday = parse_to_datetime("last friday", Language::English).unwrap();

// Times
let afternoon = parse_to_datetime("3:30 pm", Language::English).unwrap();
let morning = parse_to_datetime("09:00", Language::German).unwrap();

// Combined day and time
let appointment = parse_to_datetime("tomorrow at 2:00 pm", Language::English).unwrap();
let termin = parse_to_datetime("Montag um 15:30", Language::German).unwrap();

// Date formats
let birthday = parse_to_datetime("15/03/2024", Language::English).unwrap();
let holiday = parse_to_datetime("24.12.2024", Language::German).unwrap();

// Absolute times
let christmas = parse_to_datetime("2024-12-25T00:00:00Z", Language::English).unwrap();
```

### Supported Formats

**Relative times**:
- English: `in 5 minutes`, `2 hours ago`
- German: `in 5 Minuten`, `vor 2 Stunden`

**Day references**:
- English: `today`, `yesterday`, `tomorrow`
- German: `heute`, `gestern`, `morgen`

**Weekdays**:
- English: `monday`/`mon`, `tuesday`/`tue`, etc.
- Modifiers: `next monday`, `last friday`
- German: `Montag`/`mo`, `Dienstag`/`di`, etc.
- Modifiers: `nächsten Montag`, `letzten Freitag`

**Time formats**:
- English: `3:30 pm`, `10:15 am`, `14:30`
- German: `14:30`, `9:45 Uhr`

**Combined day and time**:
- English: `tomorrow at 3:30 pm`, `next monday at 9:00 am`
- German: `morgen um 14:30`, `nächsten Montag um 21:00 Uhr`

**Date formats**:
- English: `15/03/2024`, `31-12-2025` (DD/MM/YYYY or DD-MM-YYYY)
- German: `15.03.2024` (DD.MM.YYYY)

**Special keywords**:
- English: `now`, `later today`
- German: `jetzt`

**ISO 8601**: `2024-01-15T10:30:00Z`

Time units: seconds, minutes, hours, days, weeks, months, years

### Advanced

```rust
// Direct parser access
use temps_core::{parse, Language, TimeExpression};

let expr = parse("in 3 hours", Language::English).unwrap();
match expr {
    TimeExpression::Relative(rel) => println!("{} {:?} {:?}", rel.amount, rel.unit, rel.direction),
    TimeExpression::Absolute(abs) => println!("ISO date: {:04}-{:02}-{:02}", abs.year, abs.month, abs.day),
    TimeExpression::Now => println!("Right now!"),
    TimeExpression::Day(day) => println!("Day reference: {:?}", day),
    TimeExpression::Time(time) => println!("Time: {:02}:{:02}", time.hour, time.minute),
    TimeExpression::DayTime(dt) => println!("Day + time: {:?} at {:02}:{:02}", dt.day, dt.time.hour, dt.time.minute),
    TimeExpression::Date(date) => println!("Date: {:02}/{:02}/{:04}", date.day, date.month, date.year),
    TimeExpression::LaterToday => println!("Later today!"),
}
```

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
