//! Tokenizer for natural-language time expressions.
//!
//! The parsers used to run directly over `&str`, fusing lexing and parsing into
//! a single character-level pass — a "larser". That design makes keyword
//! matching prefix-based: `"day"` matches inside `"days"`, `"m"` inside
//! `"min"`, so every alternation has to be hand-ordered longest-first, a
//! convention that fails silently when broken.
//!
//! Splitting the lexer out removes that class of bug structurally. A word is
//! consumed maximally and then compared as a whole, so a keyword can never
//! match part of a longer word regardless of the order alternatives appear in.
//!
//! Tokenising does not by itself fix the *phrase*-level version of the same
//! hazard — `choice` still commits to the first alternative that succeeds, so a
//! bare `tomorrow` could shadow `tomorrow morning`. That one is handled in the
//! grammar rather than here, by left-factoring the shared prefix and making the
//! remainder optional (`day_reference().then(part_of_day().or_not())`), and
//! inside keyword tables by [`phrases_ci`](crate::common::phrases_ci), which
//! sorts its entries so the table's source order stays irrelevant.

use chumsky::span::SimpleSpan;

/// A single lexical unit of a time expression.
///
/// [`Token::Word`] and [`Token::Number`] carry their source slice rather than a
/// parsed value: callers need the original text to compare keywords
/// case-insensitively and to tell `01` from `1` when validating field widths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Token<'a> {
    /// A maximal run of alphabetic characters, e.g. `tomorrow`, `übermorgen`.
    Word(&'a str),
    /// A maximal run of ASCII digits, kept as text to preserve width.
    Number(&'a str),
    /// A single non-alphanumeric, non-whitespace character.
    Punct(char),
    /// A run of whitespace.
    ///
    /// Whitespace is significant here: `in 5 minutes` is a time expression
    /// while `in 5minutes` is not, so the tokens have to record where the gaps
    /// were.
    Space,
}

impl std::fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Word(w) => write!(f, "{w}"),
            Token::Number(n) => write!(f, "{n}"),
            Token::Punct(c) => write!(f, "{c}"),
            Token::Space => write!(f, "whitespace"),
        }
    }
}

/// Split `input` into tokens, each paired with its byte span in the source.
///
/// Spans are byte offsets into `input` so that diagnostics can point back at
/// the original text.
#[must_use]
pub fn lex(input: &str) -> Vec<(Token<'_>, SimpleSpan)> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some(&(start, c)) = chars.peek() {
        let kind = if c.is_whitespace() {
            CharKind::Space
        } else if c.is_alphabetic() {
            CharKind::Word
        } else if c.is_ascii_digit() {
            CharKind::Number
        } else {
            CharKind::Punct
        };

        if kind == CharKind::Punct {
            chars.next();
            let end = start + c.len_utf8();
            tokens.push((Token::Punct(c), SimpleSpan::from(start..end)));
            continue;
        }

        // Consume the whole run so a word is never matched piecemeal.
        let mut end = start;
        while let Some(&(offset, next)) = chars.peek() {
            let next_kind = if next.is_whitespace() {
                CharKind::Space
            } else if next.is_alphabetic() {
                CharKind::Word
            } else if next.is_ascii_digit() {
                CharKind::Number
            } else {
                CharKind::Punct
            };
            if next_kind != kind {
                break;
            }
            end = offset + next.len_utf8();
            chars.next();
        }

        let slice = &input[start..end];
        let token = match kind {
            CharKind::Word => Token::Word(slice),
            CharKind::Number => Token::Number(slice),
            CharKind::Space => Token::Space,
            CharKind::Punct => unreachable!("punctuation is handled above"),
        };
        tokens.push((token, SimpleSpan::from(start..end)));
    }

    tokens
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CharKind {
    Word,
    Number,
    Punct,
    Space,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(input: &str) -> Vec<Token<'_>> {
        lex(input).into_iter().map(|(t, _)| t).collect()
    }

    #[test]
    fn words_are_consumed_maximally() {
        // The whole point: "day" cannot be seen inside "days".
        assert_eq!(kinds("days"), vec![Token::Word("days")]);
        assert_eq!(kinds("min"), vec![Token::Word("min")]);
    }

    #[test]
    fn numbers_keep_their_width() {
        assert_eq!(kinds("01"), vec![Token::Number("01")]);
        assert_eq!(kinds("2024"), vec![Token::Number("2024")]);
    }

    #[test]
    fn whitespace_is_preserved_as_a_token() {
        assert_eq!(
            kinds("5 min"),
            vec![Token::Number("5"), Token::Space, Token::Word("min")]
        );
        assert_eq!(kinds("5min"), vec![Token::Number("5"), Token::Word("min")]);
    }

    #[test]
    fn non_ascii_words_stay_whole() {
        assert_eq!(kinds("übermorgen"), vec![Token::Word("übermorgen")]);
        assert_eq!(kinds("nächsten"), vec![Token::Word("nächsten")]);
    }

    #[test]
    fn spans_are_byte_offsets_into_the_source() {
        let input = "in fünf Tagen";
        let tokens = lex(input);
        let (last, span) = tokens.last().copied().expect("non-empty");
        assert_eq!(last, Token::Word("Tagen"));
        assert_eq!(&input[span.start..span.end], "Tagen");
    }

    #[test]
    fn iso_datetimes_split_into_fields() {
        assert_eq!(
            kinds("2024-01-15T14:30"),
            vec![
                Token::Number("2024"),
                Token::Punct('-'),
                Token::Number("01"),
                Token::Punct('-'),
                Token::Number("15"),
                Token::Word("T"),
                Token::Number("14"),
                Token::Punct(':'),
                Token::Number("30"),
            ]
        );
    }
}
