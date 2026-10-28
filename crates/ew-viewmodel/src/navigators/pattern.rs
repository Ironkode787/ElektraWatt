// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pattern fill's template language (WP-S2.06 part 3, §8.5; D12): a closed grammar, parsed once and rendered per
//! marked row.
//!
//! ```text
//! template   := ( literal | "{{" | "}}" | "{" expr "}" )*
//! literal    := any character except "{" and "}"
//! expr       := counter | "col:" column-key | "prev"
//! counter    := "n" [ "*" uint ] [ ("+" | "-") uint ] [ ":" "0" digit+ ]
//! ```
//!
//! `n` is the row's 1-based position among the marked rows; `n*k+j` and `n*k-j` compute in `i64`, an overflow or a
//! negative result is an error for that row; `:0w` pads the decimal with zeros to `w` digits. `{col:<key>}` is the
//! row's cell text of that column (an unknown key is an error of the whole pattern, [`Template::check_columns`]);
//! `{prev}` is the text the fill produced for the previous row (for the first row, the caller's start text).

/// One piece of a parsed template.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Piece {
    /// Literal text (`{{` and `}}` already read as `{` and `}`).
    Literal(String),
    /// The counter `n*mul+add` (`add` may be negative), padded with zeros to `width` digits.
    Counter {
        /// The factor (1 without `*`).
        mul: i64,
        /// The offset (0 without `+` or `-`).
        add: i64,
        /// The zero-padded width (0: no padding).
        width: usize,
    },
    /// The row's cell text of a column.
    Column(String),
    /// The text produced for the previous row.
    Prev,
}

/// A parsed template.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Template(pub Vec<Piece>);

/// Why a pattern does not parse (an error of the whole pattern).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PatternError {
    /// A `{` without its `}`.
    Unclosed,
    /// A `}` that is neither `}}` nor the end of an expression.
    StrayClose,
    /// An expression that is not `n…`, `col:<key>` or `prev` (the text between the braces).
    Unknown(String),
    /// A number too large for `i64` in a counter, or a width over 64.
    Number(String),
    /// A `{col:<key>}` whose key is not a column of the table.
    UnknownColumn(String),
}

impl PatternError {
    /// The English message.
    pub fn message(&self) -> String {
        match self {
            PatternError::Unclosed => {
                "unclosed brace: a `{` has no `}` (write `{{` for a literal brace)".to_owned()
            }
            PatternError::StrayClose => "a `}` closes nothing (write `}}` for a literal brace)".to_owned(),
            PatternError::Unknown(e) => {
                format!(
                    "unknown expression {{{e}}}: use {{n}}, {{n*2+1}}, {{n:02}}, {{col:<column>}} or {{prev}}"
                )
            }
            PatternError::Number(e) => format!("the number in {{{e}}} is too large"),
            PatternError::UnknownColumn(k) => format!("unknown column {k} in {{col:{k}}}"),
        }
    }
}

/// Why one row's text cannot be produced.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RowError {
    /// `n*k+j` overflows `i64`.
    Overflow,
    /// `n*k-j` is negative.
    Negative,
}

impl RowError {
    /// The English message.
    pub fn message(&self) -> &'static str {
        match self {
            RowError::Overflow => "the counter overflows",
            RowError::Negative => "the counter is negative",
        }
    }
}

/// An unsigned decimal at the start of `s`: (value, the rest); `None` without a digit.
fn uint(s: &str) -> Option<(&str, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    (end > 0).then(|| s.split_at(end))
}

/// A counter expression (`n`, `n*2`, `n+3`, `n*2-1`, `n:03`), or `None` when `e` is not one.
fn counter(e: &str) -> Option<Result<Piece, PatternError>> {
    let mut rest = e.strip_prefix('n')?;
    let number = |digits: &str| -> Result<i64, PatternError> {
        digits
            .parse::<i64>()
            .map_err(|_| PatternError::Number(e.to_owned()))
    };
    let mut mul = 1;
    if let Some(r) = rest.strip_prefix('*') {
        let (d, r) = uint(r)?;
        mul = match number(d) {
            Ok(v) => v,
            Err(err) => return Some(Err(err)),
        };
        rest = r;
    }
    let mut add = 0;
    if let Some((sign, r)) = rest
        .strip_prefix('+')
        .map(|r| (1, r))
        .or_else(|| rest.strip_prefix('-').map(|r| (-1, r)))
    {
        let (d, r) = uint(r)?;
        add = match number(d) {
            Ok(v) => sign * v,
            Err(err) => return Some(Err(err)),
        };
        rest = r;
    }
    let mut width = 0;
    if let Some(r) = rest.strip_prefix(':') {
        let digits = r.strip_prefix('0')?;
        let (d, r) = uint(digits)?;
        width = match d.parse::<usize>() {
            Ok(w) if w <= 64 => w,
            _ => return Some(Err(PatternError::Number(e.to_owned()))),
        };
        rest = r;
    }
    rest.is_empty().then_some(Ok(Piece::Counter { mul, add, width }))
}

impl Template {
    /// Parses a pattern.
    ///
    /// # Errors
    ///
    /// [`PatternError`] for an unclosed or stray brace, an unknown expression or a number too large.
    pub fn parse(pattern: &str) -> Result<Template, PatternError> {
        let mut pieces: Vec<Piece> = Vec::new();
        let mut literal = String::new();
        let mut chars = pattern.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' if chars.peek() == Some(&'{') => {
                    chars.next();
                    literal.push('{');
                }
                '}' if chars.peek() == Some(&'}') => {
                    chars.next();
                    literal.push('}');
                }
                '}' => return Err(PatternError::StrayClose),
                '{' => {
                    let mut expr = String::new();
                    let mut closed = false;
                    for d in chars.by_ref() {
                        if d == '}' {
                            closed = true;
                            break;
                        }
                        if d == '{' {
                            return Err(PatternError::Unclosed);
                        }
                        expr.push(d);
                    }
                    if !closed {
                        return Err(PatternError::Unclosed);
                    }
                    if !literal.is_empty() {
                        pieces.push(Piece::Literal(std::mem::take(&mut literal)));
                    }
                    let piece = if expr == "prev" {
                        Piece::Prev
                    } else if let Some(key) = expr.strip_prefix("col:") {
                        if key.is_empty() {
                            return Err(PatternError::Unknown(expr));
                        }
                        Piece::Column(key.to_owned())
                    } else {
                        match counter(&expr) {
                            Some(p) => p?,
                            None => return Err(PatternError::Unknown(expr)),
                        }
                    };
                    pieces.push(piece);
                }
                other => literal.push(other),
            }
        }
        if !literal.is_empty() {
            pieces.push(Piece::Literal(literal));
        }
        Ok(Template(pieces))
    }

    /// Checks every `{col:<key>}` against the column keys of the table.
    ///
    /// # Errors
    ///
    /// [`PatternError::UnknownColumn`] for the first key that is not a column.
    pub fn check_columns(&self, columns: &[&str]) -> Result<(), PatternError> {
        for p in &self.0 {
            if let Piece::Column(k) = p
                && !columns.contains(&k.as_str())
            {
                return Err(PatternError::UnknownColumn(k.clone()));
            }
        }
        Ok(())
    }

    /// The text for the row at 1-based position `n`, with `cell(key)` the row's cell texts and `prev` the previous
    /// row's produced text.
    ///
    /// # Errors
    ///
    /// [`RowError`] when the counter overflows or is negative.
    pub fn render(&self, n: i64, cell: &dyn Fn(&str) -> String, prev: &str) -> Result<String, RowError> {
        let mut out = String::new();
        for p in &self.0 {
            match p {
                Piece::Literal(t) => out.push_str(t),
                Piece::Prev => out.push_str(prev),
                Piece::Column(k) => out.push_str(&cell(k)),
                Piece::Counter { mul, add, width } => {
                    let v = n
                        .checked_mul(*mul)
                        .and_then(|v| v.checked_add(*add))
                        .ok_or(RowError::Overflow)?;
                    if v < 0 {
                        return Err(RowError::Negative);
                    }
                    out.push_str(&format!("{v:0width$}", width = *width));
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(pattern: &str, rows: i64) -> Vec<String> {
        let t = Template::parse(pattern).unwrap();
        let mut prev = String::new();
        (1..=rows)
            .map(|n| {
                let s = t.render(n, &|k| format!("<{k}{n}>"), &prev).unwrap();
                prev.clone_from(&s);
                s
            })
            .collect()
    }

    #[test]
    fn every_branch_of_the_grammar() {
        assert_eq!(fill("Spare_{n:02}", 3), ["Spare_01", "Spare_02", "Spare_03"]);
        assert_eq!(fill("Reserve", 2), ["Reserve", "Reserve"]);
        assert_eq!(fill("{n}", 2), ["1", "2"]);
        assert_eq!(fill("{n*2}", 2), ["2", "4"]);
        assert_eq!(fill("{n+10}", 2), ["11", "12"]);
        assert_eq!(fill("{n*3-1}", 2), ["2", "5"]);
        assert_eq!(fill("{n*10+5:004}", 1), ["0015"]);
        assert_eq!(fill("{col:label}", 2), ["<label1>", "<label2>"]);
        assert_eq!(fill("{prev}x", 3), ["x", "xx", "xxx"]);
        assert_eq!(fill("{{n}}", 1), ["{n}"], "literal braces");
        assert_eq!(fill("a}}b{{c", 1), ["a}b{c"]);
        assert_eq!(fill("", 1), [""]);
    }

    /// Random patterns over the grammar's characters never make the parser or the renderer panic.
    #[test]
    fn fuzz_random_patterns_never_panic() {
        const PIECES: [&str; 14] = [
            "{",
            "}",
            "n",
            "*",
            "+",
            "-",
            ":",
            "0",
            "9",
            "col:",
            "prev",
            "x",
            "922337203685477",
            "{n",
        ];
        let mut s: u64 = 0x5EED_2026;
        let mut next = |n: usize| {
            s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            usize::try_from((s >> 33) % n as u64).unwrap_or(0)
        };
        for _ in 0..4000 {
            let len = next(12);
            let p: String = (0..len).map(|_| PIECES[next(PIECES.len())]).collect();
            if let Ok(t) = Template::parse(&p) {
                let _ = t.check_columns(&["x"]);
                for n in [1, 2, i64::MAX] {
                    let _ = t.render(n, &|k| k.to_owned(), "p");
                }
            }
        }
    }

    #[test]
    fn every_error() {
        assert_eq!(Template::parse("Spare_{n"), Err(PatternError::Unclosed));
        assert_eq!(Template::parse("{n{"), Err(PatternError::Unclosed));
        assert_eq!(Template::parse("a}b"), Err(PatternError::StrayClose));
        assert_eq!(Template::parse("{x}"), Err(PatternError::Unknown("x".into())));
        assert_eq!(Template::parse("{n*}"), Err(PatternError::Unknown("n*".into())));
        assert_eq!(Template::parse("{n:2}"), Err(PatternError::Unknown("n:2".into())));
        assert_eq!(
            Template::parse("{col:}"),
            Err(PatternError::Unknown("col:".into()))
        );
        assert_eq!(
            Template::parse("{n*99999999999999999999}"),
            Err(PatternError::Number("n*99999999999999999999".into()))
        );
        let t = Template::parse("{col:nope}").unwrap();
        assert_eq!(
            t.check_columns(&["label", "terminal"]),
            Err(PatternError::UnknownColumn("nope".into()))
        );
        assert!(
            Template::parse("{col:label}")
                .unwrap()
                .check_columns(&["label"])
                .is_ok()
        );
        let over = Template::parse("{n*4611686018427387904}").unwrap();
        assert_eq!(over.render(2, &|_| String::new(), ""), Err(RowError::Overflow));
        let neg = Template::parse("{n-3}").unwrap();
        assert_eq!(neg.render(2, &|_| String::new(), ""), Err(RowError::Negative));
        assert_eq!(neg.render(3, &|_| String::new(), ""), Ok("0".into()));
        for e in [
            PatternError::Unclosed,
            PatternError::StrayClose,
            PatternError::Unknown("x".into()),
            PatternError::Number("n".into()),
            PatternError::UnknownColumn("k".into()),
        ] {
            assert!(!e.message().is_empty());
        }
    }
}
