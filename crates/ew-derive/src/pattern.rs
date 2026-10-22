// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The designation pattern language of doc 10 §2.4 and §2.6 (FT-318): one expander for the automatic channel
//! designation of a part (`ChannelSpec.designation`, read by `channel_texts`) and the pin designation of its
//! channel → pin map (`PlcSection.pin_of`, read by [`channel_pin`](crate::channel_pin), so by normaliser N3).
//!
//! A pattern is text in which each `{<expression>}` is a map-language expression in `n` (`{n}`, `{n+1}`,
//! `{2n+1}`; `ew_plcaddr::MapExpr`) and every other character is literal: `"X2:{n+1}"` gives `X2:1` for
//! channel 0. There is no escape for a literal brace, and the map language has no subtraction (`{n-1}` does
//! not parse). A pattern the expander cannot expand is an error that names the reason, never a panic; what a
//! reader does with it (no designation, no pin, and a notice) is the reader's rule.

use core::fmt;

use ew_plcaddr::{MapExpr, MapVar};

/// Why a designation pattern could not be expanded.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PatternError {
    /// A `{` without its `}`.
    UnclosedBrace,
    /// A `}` without its `{`.
    StrayBrace,
    /// The text between the braces is not a map-language expression (`{}`, `{n-1}`, `{x}`).
    BadExpression(String),
    /// The expression is in `p`, the position variable of the solenoid-position map, not in `n`.
    WrongVariable(String),
    /// The expression overflows at this channel number.
    Overflow(String),
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatternError::UnclosedBrace => f.write_str("a `{` is not closed"),
            PatternError::StrayBrace => f.write_str("a `}` has no `{`"),
            PatternError::BadExpression(e) => {
                write!(
                    f,
                    "`{{{e}}}` is not an expression in n (`n`, `2n`, `n+1`, `2n+1`)"
                )
            }
            PatternError::WrongVariable(e) => write!(f, "`{{{e}}}` is an expression in p, not in n"),
            PatternError::Overflow(e) => write!(f, "`{{{e}}}` overflows"),
        }
    }
}

impl std::error::Error for PatternError {}

/// Expands a designation pattern for channel number `n`: each `{<expression in n>}` is replaced by its value
/// and every other character is kept (see the module documentation). An empty pattern gives the empty text;
/// whether that is a designation is the caller's rule.
///
/// # Errors
///
/// [`PatternError`] for an unclosed `{`, a stray `}`, an expression that does not parse or is in `p`, and an
/// overflow.
pub fn expand_pattern(pattern: &str, n: u32) -> Result<String, PatternError> {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        let (literal, tail) = (&rest[..open], &rest[open + 1..]);
        if literal.contains('}') {
            return Err(PatternError::StrayBrace);
        }
        out.push_str(literal);
        let close = tail.find('}').ok_or(PatternError::UnclosedBrace)?;
        let text = &tail[..close];
        // a second `{` before the closing brace is part of a malformed expression
        let expr = MapExpr::parse(text).ok_or_else(|| PatternError::BadExpression(text.to_owned()))?;
        if expr.var != MapVar::N {
            return Err(PatternError::WrongVariable(text.to_owned()));
        }
        let value = expr
            .eval(n)
            .ok_or_else(|| PatternError::Overflow(text.to_owned()))?;
        out.push_str(&value.to_string());
        rest = &tail[close + 1..];
    }
    if rest.contains('}') {
        return Err(PatternError::StrayBrace);
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(pattern: &str, n: u32) -> String {
        expand_pattern(pattern, n).unwrap_or_else(|e| panic!("`{pattern}` should expand: {e}"))
    }

    #[test]
    fn placeholders_are_expressions_in_n() {
        assert_eq!(ok("{n}", 3), "3");
        assert_eq!(ok("DO {n}", 0), "DO 0");
        assert_eq!(ok("X2:{n+1}", 0), "X2:1");
        assert_eq!(ok("X2:{n+1}", 7), "X2:8");
        assert_eq!(ok("{2n+1}", 3), "7");
        assert_eq!(ok("{4n}", 5), "20");
        assert_eq!(ok("A{n}.{n+1}", 2), "A2.3");
        assert_eq!(ok("{n}{n}", 4), "44");
    }

    #[test]
    fn literal_text_is_kept() {
        assert_eq!(ok("", 5), "");
        assert_eq!(ok("COM", 5), "COM");
        assert_eq!(ok("a:b/c-d é", 5), "a:b/c-d é");
        assert_eq!(ok("{n}é", 1), "1é");
    }

    /// The pin map of `pin_of` and the automatic designation use this expander (FT-318).
    #[test]
    fn the_pin_map_expands_with_the_same_function() {
        use crate::{channel_pin, pin_pattern_error};
        use ew_schema::PinMap;
        let pattern = |p: &str| PinMap::Pattern(p.to_owned());
        assert_eq!(channel_pin(&pattern("X2:{n+1}"), 0).as_deref(), Some("X2:1"));
        assert_eq!(channel_pin(&pattern("{2n+1}"), 3).as_deref(), Some("7"));
        assert_eq!(channel_pin(&pattern("Out{n"), 0), None);
        assert_eq!(
            pin_pattern_error(&pattern("Out{n"), 0),
            Some(PatternError::UnclosedBrace)
        );
        assert_eq!(pin_pattern_error(&pattern("Out{n}"), 0), None);
        // an empty result gives no pin, though the expansion itself is `Ok`
        assert_eq!(expand_pattern("", 0), Ok(String::new()));
        assert_eq!(channel_pin(&pattern(""), 0), None);
        let list = PinMap::List(vec!["A".parse().expect("ident"), "B".parse().expect("ident")]);
        assert_eq!(channel_pin(&list, 1).as_deref(), Some("B"));
        assert_eq!(channel_pin(&list, 2), None);
    }

    #[test]
    fn a_pattern_that_cannot_be_expanded_is_an_error() {
        let err = |p: &str| expand_pattern(p, 1).expect_err(p);
        assert_eq!(err("X{n"), PatternError::UnclosedBrace);
        assert_eq!(err("{"), PatternError::UnclosedBrace);
        assert_eq!(err("X}"), PatternError::StrayBrace);
        assert_eq!(err("}{n}"), PatternError::StrayBrace);
        assert_eq!(err("{n}}"), PatternError::StrayBrace);
        assert_eq!(err("{}"), PatternError::BadExpression(String::new()));
        // the map language has no subtraction
        assert_eq!(err("{n-1}"), PatternError::BadExpression("n-1".into()));
        assert_eq!(err("{x}"), PatternError::BadExpression("x".into()));
        assert_eq!(err("{n:02}"), PatternError::BadExpression("n:02".into()));
        assert_eq!(err("{{n}}"), PatternError::BadExpression("{n".into()));
        assert_eq!(err("{p}"), PatternError::WrongVariable("p".into()));
        assert_eq!(err("{2p+1}"), PatternError::WrongVariable("2p+1".into()));
        assert_eq!(
            expand_pattern("{4294967295n}", 2),
            Err(PatternError::Overflow("4294967295n".into()))
        );
        for e in [
            PatternError::UnclosedBrace,
            PatternError::StrayBrace,
            PatternError::BadExpression("n-1".into()),
            PatternError::WrongVariable("p".into()),
            PatternError::Overflow("n".into()),
        ] {
            assert!(!e.to_string().is_empty());
        }
    }
}
