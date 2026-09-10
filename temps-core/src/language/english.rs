use chumsky::{error::Rich, prelude::*};

use crate::{
    DayReference, DayTime, Direction, LanguageParser, Meridiem, RelativeTime, Result, StandardDate,
    Time, TimeExpression, TimeUnit, Weekday, WeekdayModifier,
    common::{
        ParserError, TokenInput, digit_number, four_digit_number, iso_datetime, opt_space,
        phrase_ci, phrases_ci, punct, space, token_stream, two_digit_number, word_ci,
    },
    error::rich_errors_to_temps_error,
    errors::{INVALID_CALENDAR_DATE, INVALID_TIME_OF_DAY},
    lexer::lex,
    time_utils,
};

/// Parser for English natural language time expressions.
pub struct EnglishParser;

fn number<'t, 's: 't, I>() -> impl Parser<'t, I, i64, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    choice((
        digit_number(),
        phrases_ci([
            ("a", 1i64),
            ("an", 1),
            ("one", 1),
            ("two", 2),
            ("three", 3),
            ("four", 4),
            ("five", 5),
            ("six", 6),
            ("seven", 7),
            ("eight", 8),
            ("nine", 9),
            ("ten", 10),
            ("a couple", 2),
            ("a couple of", 2),
            ("couple of", 2),
            ("a few", 3),
            ("a dozen", 12),
        ]),
    ))
    .labelled("number")
}

fn time_unit<'t, 's: 't, I>() -> impl Parser<'t, I, TimeUnit, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    phrases_ci([
        ("second", TimeUnit::Second),
        ("seconds", TimeUnit::Second),
        ("sec", TimeUnit::Second),
        ("secs", TimeUnit::Second),
        ("s", TimeUnit::Second),
        ("minute", TimeUnit::Minute),
        ("minutes", TimeUnit::Minute),
        ("min", TimeUnit::Minute),
        ("mins", TimeUnit::Minute),
        ("m", TimeUnit::Minute),
        ("hour", TimeUnit::Hour),
        ("hours", TimeUnit::Hour),
        ("hr", TimeUnit::Hour),
        ("hrs", TimeUnit::Hour),
        ("h", TimeUnit::Hour),
        ("day", TimeUnit::Day),
        ("days", TimeUnit::Day),
        ("d", TimeUnit::Day),
        ("week", TimeUnit::Week),
        ("weeks", TimeUnit::Week),
        ("wk", TimeUnit::Week),
        ("wks", TimeUnit::Week),
        ("w", TimeUnit::Week),
        ("month", TimeUnit::Month),
        ("months", TimeUnit::Month),
        ("mo", TimeUnit::Month),
        ("mos", TimeUnit::Month),
        ("year", TimeUnit::Year),
        ("years", TimeUnit::Year),
        ("yr", TimeUnit::Year),
        ("yrs", TimeUnit::Year),
        ("y", TimeUnit::Year),
    ])
    .labelled("time unit")
}

/// A unit as written, paired with how many of [`TimeUnit`] one of it is worth.
///
/// `TimeUnit` has no `Fortnight` variant and gains none here: a fortnight is
/// two weeks, so the colloquial unit is carried as the ordinary
/// [`TimeUnit::Week`] plus a multiplier that [`amount_and_unit`] applies to the
/// parsed amount.
fn scaled_time_unit<'t, 's: 't, I>()
-> impl Parser<'t, I, (TimeUnit, i64), ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    choice((
        phrases_ci([("fortnight", ()), ("fortnights", ())]).to((TimeUnit::Week, 2i64)),
        time_unit().map(|unit| (unit, 1i64)),
    ))
    .labelled("time unit")
}

/// `<number> <unit>`, with a colloquial unit's multiplier already folded into
/// the amount.
///
/// The multiplication is checked because the amount comes straight from user
/// input: `in 9223372036854775807 fortnights` must be rejected as
/// unrepresentable rather than wrap into a plausible-looking past date.
fn amount_and_unit<'t, 's: 't, I>()
-> impl Parser<'t, I, (i64, TimeUnit), ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    number()
        .then_ignore(space())
        .then(scaled_time_unit())
        .try_map(|(amount, (unit, per_unit)), span| {
            amount
                .checked_mul(per_unit)
                .map(|amount| (amount, unit))
                .ok_or_else(|| Rich::custom(span, "amount out of range"))
        })
}

fn weekday<'t, 's: 't, I>() -> impl Parser<'t, I, Weekday, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    phrases_ci([
        ("monday", Weekday::Monday),
        ("mon", Weekday::Monday),
        ("tuesday", Weekday::Tuesday),
        ("tue", Weekday::Tuesday),
        ("wednesday", Weekday::Wednesday),
        ("wed", Weekday::Wednesday),
        ("thursday", Weekday::Thursday),
        ("thu", Weekday::Thursday),
        ("friday", Weekday::Friday),
        ("fri", Weekday::Friday),
        ("saturday", Weekday::Saturday),
        ("sat", Weekday::Saturday),
        ("sunday", Weekday::Sunday),
        ("sun", Weekday::Sunday),
    ])
    .labelled("weekday")
}

fn day_shortcuts<'t, 's: 't, I>() -> impl Parser<'t, I, DayReference, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    phrases_ci([
        ("today", DayReference::Today),
        ("yesterday", DayReference::Yesterday),
        ("tomorrow", DayReference::Tomorrow),
        ("day after tomorrow", DayReference::DayAfterTomorrow),
        ("day before yesterday", DayReference::DayBeforeYesterday),
    ])
}

fn weekday_modifier<'t, 's: 't, I>()
-> impl Parser<'t, I, WeekdayModifier, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    choice((
        word_ci("last").to(WeekdayModifier::Last),
        word_ci("next").to(WeekdayModifier::Next),
    ))
}

fn modified_weekday<'t, 's: 't, I>() -> impl Parser<'t, I, DayReference, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    weekday_modifier()
        .then_ignore(space())
        .then(weekday())
        .map(|(modifier, day)| DayReference::Weekday {
            day,
            modifier: Some(modifier),
        })
}

fn simple_weekday<'t, 's: 't, I>() -> impl Parser<'t, I, DayReference, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    weekday().map(|day| DayReference::Weekday {
        day,
        modifier: None,
    })
}

fn day_reference<'t, 's: 't, I>() -> impl Parser<'t, I, DayReference, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    // A plain `choice` needs no left-factoring here: no alternative below can
    // succeed on a proper token-prefix of another one's match, which is the
    // only way `choice`'s commit-on-success could pick the wrong branch.
    //
    // The near misses, all of which resolve by backtracking:
    //   - `the day after tomorrow` / `the day before yesterday` share `the day`
    //     but diverge on the third token, so neither ever succeeds first;
    //   - `next weekend` and `next Monday` share `next`, and `weekend` is not a
    //     weekday, so `modified_weekday` fails without consuming;
    //   - likewise `this weekend` against `this Monday`.
    choice((
        the_day_reference(),
        day_shortcuts(),
        modified_weekday(),
        this_weekday(),
        weekend_ref(),
        simple_weekday(),
    ))
}

fn meridiem<'t, 's: 't, I>() -> impl Parser<'t, I, Meridiem, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    phrases_ci([
        ("am", Meridiem::AM),
        ("pm", Meridiem::PM),
        ("a.m.", Meridiem::AM),
        ("p.m.", Meridiem::PM),
    ])
    .labelled("am/pm")
}

fn time_with_minutes<'t, 's: 't, I>()
-> impl Parser<'t, I, (u8, u8, u8, Option<Meridiem>), ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    two_digit_number()
        .then_ignore(punct(':'))
        .then(two_digit_number())
        .then(punct(':').ignore_then(two_digit_number()).or_not())
        .then(opt_space().ignore_then(meridiem()).or_not())
        .validate(|(((hour, minute), second), mer), extra, emitter| {
            let second = second.unwrap_or(0);
            // Emitted, not returned: a `try_map` error is registered where the
            // parser started and loses chumsky's furthest-error ranking, so
            // `25:00` was answered with a complaint about the `:` separator.
            if !time_utils::is_valid_time(hour, minute, second, mer) {
                emitter.emit(Rich::custom(extra.span(), INVALID_TIME_OF_DAY));
            }
            (hour, minute, second, mer)
        })
}

fn hour_meridiem<'t, 's: 't, I>()
-> impl Parser<'t, I, (u8, u8, u8, Option<Meridiem>), ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    two_digit_number()
        .then(opt_space().ignore_then(meridiem()))
        .validate(|(hour, mer), extra, emitter| {
            if !time_utils::is_valid_time(hour, 0, 0, Some(mer)) {
                emitter.emit(Rich::custom(extra.span(), INVALID_TIME_OF_DAY));
            }
            (hour, 0, 0, Some(mer))
        })
}

fn time_digits<'t, 's: 't, I>()
-> impl Parser<'t, I, (u8, u8, u8, Option<Meridiem>), ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    choice((time_with_minutes(), hour_meridiem()))
}

/// Parse a raw hour (number or named time like "noon") for use in fractional expressions.
fn raw_hour<'t, 's: 't, I>() -> impl Parser<'t, I, u8, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    choice((
        two_digit_number().validate(|h, extra, emitter| {
            if h > 23 {
                emitter.emit(Rich::custom(extra.span(), "hour must be 0-23"));
            }
            h
        }),
        word_ci("noon").to(12u8),
        word_ci("midnight").to(0u8),
    ))
}

/// Parse fractional time: "half past X", "quarter past X", "quarter to X".
fn fractional_time<'t, 's: 't, I>()
-> impl Parser<'t, I, (u8, u8, u8, Option<Meridiem>), ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    let half_past = phrase_ci("half past")
        .ignore_then(space())
        .ignore_then(raw_hour())
        .map(|h| (h, 30u8, 0u8, None::<Meridiem>));

    let quarter_past = phrase_ci("quarter past")
        .ignore_then(space())
        .ignore_then(raw_hour())
        .map(|h| (h, 15u8, 0u8, None::<Meridiem>));

    let quarter_to = phrase_ci("quarter to")
        .ignore_then(space())
        .ignore_then(raw_hour())
        .map(|h| {
            if h == 0 {
                (23u8, 45u8, 0u8, None::<Meridiem>)
            } else {
                (h - 1, 45u8, 0u8, None::<Meridiem>)
            }
        });

    choice((half_past, quarter_past, quarter_to)).try_map(|(hour, minute, second, mer), span| {
        if time_utils::is_valid_time(hour, minute, second, mer) {
            Ok((hour, minute, second, mer))
        } else {
            Err(Rich::custom(span, "invalid time"))
        }
    })
}

fn time_expr<'t, 's: 't, I>() -> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    choice((
        fractional_time().map(|(hour, minute, second, meridiem)| {
            TimeExpression::Time(Time {
                hour,
                minute,
                second,
                meridiem,
            })
        }),
        time_digits().map(|(hour, minute, second, meridiem)| {
            TimeExpression::Time(Time {
                hour,
                minute,
                second,
                meridiem,
            })
        }),
    ))
}

fn named_time<'t, 's: 't, I>() -> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    choice((
        word_ci("noon").to(TimeExpression::Time(Time {
            hour: 12,
            minute: 0,
            second: 0,
            meridiem: None,
        })),
        word_ci("midnight").to(TimeExpression::Time(Time {
            hour: 0,
            minute: 0,
            second: 0,
            meridiem: None,
        })),
        word_ci("teatime").to(TimeExpression::Time(Time {
            hour: 16,
            minute: 0,
            second: 0,
            meridiem: None,
        })),
    ))
}

/// Parse part-of-day: "morning", "afternoon", "evening", "night".
/// Returns a Time with a default hour.
fn part_of_day<'t, 's: 't, I>() -> impl Parser<'t, I, Time, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    choice((
        word_ci("morning").to(Time {
            hour: 8,
            minute: 0,
            second: 0,
            meridiem: None,
        }),
        word_ci("afternoon").to(Time {
            hour: 13,
            minute: 0,
            second: 0,
            meridiem: None,
        }),
        word_ci("evening").to(Time {
            hour: 18,
            minute: 0,
            second: 0,
            meridiem: None,
        }),
        word_ci("night").to(Time {
            hour: 20,
            minute: 0,
            second: 0,
            meridiem: None,
        }),
    ))
}

/// "this" + day-like expression: "this morning", "this afternoon", "this evening".
fn this_part_of_day<'t, 's: 't, I>()
-> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    word_ci("this")
        .ignore_then(space())
        .ignore_then(part_of_day())
        .map(|time| {
            TimeExpression::DayTime(DayTime {
                day: DayReference::Today,
                time,
            })
        })
}

/// "this" + weekday: "this Monday", "this Friday".
fn this_weekday<'t, 's: 't, I>() -> impl Parser<'t, I, DayReference, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    word_ci("this")
        .ignore_then(space())
        .ignore_then(weekday())
        .map(|day| DayReference::Weekday {
            day,
            modifier: None,
        })
}

/// "this weekend" / "next weekend".
fn weekend_ref<'t, 's: 't, I>() -> impl Parser<'t, I, DayReference, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    choice((
        phrase_ci("this weekend").to(DayReference::Weekday {
            day: Weekday::Saturday,
            modifier: Some(WeekdayModifier::This),
        }),
        phrase_ci("next weekend").to(DayReference::Weekday {
            day: Weekday::Saturday,
            modifier: Some(WeekdayModifier::Next),
        }),
    ))
}

/// Standalone expressions that map to DayTime.
fn standalone_daytime<'t, 's: 't, I>()
-> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    choice((
        word_ci("tonight").to(TimeExpression::DayTime(DayTime {
            day: DayReference::Today,
            time: Time {
                hour: 20,
                minute: 0,
                second: 0,
                meridiem: None,
            },
        })),
        choice((
            word_ci("eod"),
            phrase_ci("end of day"),
            phrase_ci("end of the day"),
        ))
        .to(TimeExpression::DayTime(DayTime {
            day: DayReference::Today,
            time: Time {
                hour: 17,
                minute: 0,
                second: 0,
                meridiem: None,
            },
        })),
    ))
}

/// The `the`-prefixed synonyms: "the day after tomorrow", "the day before
/// yesterday".
///
/// The shared `the` is factored out rather than repeated in two competing
/// alternatives.
fn the_day_reference<'t, 's: 't, I>()
-> impl Parser<'t, I, DayReference, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    word_ci("the").ignore_then(space()).ignore_then(phrases_ci([
        ("day after tomorrow", DayReference::DayAfterTomorrow),
        ("day before yesterday", DayReference::DayBeforeYesterday),
    ]))
}

/// Bare "fortnight" = 2 weeks (future direction assumed for scheduling).
///
/// This is only the standalone reading. As a *unit* — `in a fortnight`,
/// `three fortnights ago` — a fortnight is handled by [`scaled_time_unit`],
/// which those rules reach through their own leading token (`in`) or trailing
/// one (`ago`), so neither can succeed on a prefix of the other.
fn fortnight<'t, 's: 't, I>() -> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    word_ci("fortnight").to(TimeExpression::Relative(RelativeTime {
        amount: 2,
        unit: TimeUnit::Week,
        direction: Direction::Future,
    }))
}

/// "later" / "later today" — vague future (~2 hours).
fn later_expr<'t, 's: 't, I>() -> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    // Left-factored on the shared `later`, so the bare form can no longer
    // shadow `later today` and the order of the two readings is irrelevant.
    word_ci("later")
        .ignore_then(space().ignore_then(word_ci("today")).or_not())
        .map(|today| match today {
            Some(()) => TimeExpression::LaterToday,
            None => TimeExpression::Relative(RelativeTime {
                amount: 2,
                unit: TimeUnit::Hour,
                direction: Direction::Future,
            }),
        })
}

/// "a week from now" / "a week from today".
fn week_from_now<'t, 's: 't, I>() -> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    choice((phrase_ci("a week from today"), phrase_ci("a week from now"))).to(
        TimeExpression::Relative(RelativeTime {
            amount: 1,
            unit: TimeUnit::Week,
            direction: Direction::Future,
        }),
    )
}

/// A day reference, optionally qualified by a time of day.
///
/// This is the left-factored form of what used to be three competing top-level
/// alternatives — `tomorrow at 3:30 pm`, `tomorrow morning` and a bare
/// `tomorrow`. They all start with the same [`day_reference`], so under an
/// ordered `choice` the bare form would commit on `tomorrow` and strand the
/// rest of the input. Parsing the shared prefix once and treating the time as
/// an optional tail removes the ambiguity instead of papering over it.
fn day_expr<'t, 's: 't, I>() -> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    let at_time = word_ci("at")
        .ignore_then(space())
        .ignore_then(time_digits())
        .map(|(hour, minute, second, meridiem)| Time {
            hour,
            minute,
            second,
            meridiem,
        });

    day_reference()
        .then(
            space()
                .ignore_then(choice((at_time, part_of_day())))
                .or_not(),
        )
        .map(|(day, time)| match time {
            Some(time) => TimeExpression::DayTime(DayTime { day, time }),
            None => TimeExpression::Day(day),
        })
}

fn relative_past<'t, 's: 't, I>() -> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    amount_and_unit()
        .then_ignore(space())
        .then_ignore(word_ci("ago"))
        .map(|(amount, unit)| {
            TimeExpression::Relative(RelativeTime {
                amount,
                unit,
                direction: Direction::Past,
            })
        })
}

fn relative_future<'t, 's: 't, I>()
-> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    word_ci("in")
        .ignore_then(space())
        .ignore_then(amount_and_unit())
        .map(|(amount, unit)| {
            TimeExpression::Relative(RelativeTime {
                amount,
                unit,
                direction: Direction::Future,
            })
        })
}

fn now_expr<'t, 's: 't, I>() -> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    word_ci("now").to(TimeExpression::Now)
}

fn date_format<'t, 's: 't, I>() -> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>> + Clone
where
    I: TokenInput<'t, 's>,
{
    // Note: a `YYYY-MM-DD` alternative would be dead code here — `iso_datetime()`
    // is tried first at the top level and accepts exactly that shape.
    let separator = choice((punct('/').to('/'), punct('-').to('-')));

    two_digit_number()
        .then(separator.clone())
        .then(two_digit_number())
        .then(separator)
        .then(four_digit_number())
        .validate(|((((day, first), month), second), year), extra, emitter| {
            if first != second || !time_utils::is_valid_calendar_date(year, month, day) {
                // Emitted, not returned: a `try_map` error would be registered
                // at the cursor where this parser started and lose chumsky's
                // furthest-error ranking to an unrelated alternative, leaving
                // the caller with a caret on a separator that valid dates
                // accept. `into_result` still fails because an emitted error
                // discards the output.
                emitter.emit(Rich::custom(extra.span(), INVALID_CALENDAR_DATE));
            }
            TimeExpression::Date(StandardDate { day, month, year })
        })
}

fn parser<'t, 's: 't, I>() -> impl Parser<'t, I, TimeExpression, ParserError<'t, 's>>
where
    I: TokenInput<'t, 's>,
{
    // An ordered `choice`, which is only safe because the grammar is
    // left-factored: every family of expressions sharing a leading token is
    // parsed by a single alternative that treats the rest as an optional tail
    // (see [`day_expr`], [`later_expr`], [`iso_datetime`]). What remains are
    // alternatives that either start on different tokens or fail without
    // committing, so none can succeed on a proper prefix of another's match
    // and strand the rest of the input against `end()`. The order below is
    // therefore documentation, not semantics: reversing it parses every
    // supported expression identically.
    choice((
        iso_datetime().labelled("ISO 8601 datetime"),
        date_format().labelled("calendar date"),
        day_expr().labelled("day, optionally with a time"),
        now_expr().labelled("`now`"),
        standalone_daytime().labelled("standalone (tonight, EOD)"),
        this_part_of_day().labelled("this morning/afternoon/evening"),
        named_time().labelled("named time (noon, midnight, teatime)"),
        time_expr().labelled("time of day"),
        relative_past().labelled("`<n> <unit> ago`"),
        relative_future().labelled("`in <n> <unit>`"),
        week_from_now().labelled("a week from now/today"),
        fortnight().labelled("fortnight"),
        later_expr().labelled("later/later today"),
    ))
    .padded_by(opt_space())
    .then_ignore(end())
}

impl LanguageParser for EnglishParser {
    fn parse(&self, input: &str) -> Result<TimeExpression> {
        let tokens = lex(input);
        parser()
            .parse(token_stream(input, &tokens))
            .into_result()
            .map_err(|errs| rich_errors_to_temps_error(input, errs))
    }
}
