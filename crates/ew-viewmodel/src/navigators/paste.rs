// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Paste from Excel and copy (WP-S2.06 part 3, §8.6; D12): the tab-separated text Excel puts on the clipboard.
//!
//! **Parse.** Rows split on CRLF or LF (a lone CR too), cells on TAB. A cell that starts with a double quote is quoted:
//! it runs to the next quote that is not doubled, may hold TAB and line breaks, and `""` inside is one quote; text after
//! the closing quote up to the next TAB or line break is kept as Excel does. One trailing empty row is dropped. More
//! than [`MAX_CELLS`] cells or one cell over [`MAX_CELL_CHARS`] characters refuse the whole paste ("too large"); a quote
//! that does not close refuses it ("unclosed quote in row <n>").
//!
//! **Write.** Rows joined by CRLF with a final CRLF, cells by TAB; a cell holding a TAB, a line break or a double quote
//! is quoted with its quotes doubled, as Excel writes them.

/// The most cells one paste takes.
pub const MAX_CELLS: usize = 100_000;

/// The most characters one pasted cell takes.
pub const MAX_CELL_CHARS: usize = 10_000;

/// Why a pasted text is refused as a whole.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TsvError {
    /// More than [`MAX_CELLS`] cells, or a cell over [`MAX_CELL_CHARS`] characters.
    TooLarge,
    /// A quoted cell that does not close, in this 1-based row.
    UnclosedQuote(usize),
}

impl TsvError {
    /// The English message.
    pub fn message(&self) -> String {
        match self {
            TsvError::TooLarge => format!(
                "too large: a paste takes at most {MAX_CELLS} cells of at most {MAX_CELL_CHARS} characters"
            ),
            TsvError::UnclosedQuote(n) => format!("unclosed quote in row {n}"),
        }
    }
}

/// The rows of a tab-separated text.
///
/// # Errors
///
/// [`TsvError`] when it is too large or a quote does not close.
pub fn parse_tsv(text: &str) -> Result<Vec<Vec<String>>, TsvError> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut cell_chars = 0usize;
    let mut cells = 0usize;
    let mut chars = text.chars().peekable();
    // Whether the cell being read began with a quote and the quote is still open.
    let mut at_start = true;
    let push_cell = |row: &mut Vec<String>, cell: &mut String, cells: &mut usize| -> Result<(), TsvError> {
        *cells += 1;
        if *cells > MAX_CELLS {
            return Err(TsvError::TooLarge);
        }
        row.push(std::mem::take(cell));
        Ok(())
    };
    while let Some(c) = chars.next() {
        if at_start && c == '"' {
            at_start = false;
            // A quoted cell: up to the closing quote.
            let mut closed = false;
            while let Some(d) = chars.next() {
                if d == '"' {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        cell.push('"');
                        cell_chars += 1;
                    } else {
                        closed = true;
                        break;
                    }
                } else {
                    cell.push(d);
                    cell_chars += 1;
                }
                if cell_chars > MAX_CELL_CHARS {
                    return Err(TsvError::TooLarge);
                }
            }
            if !closed {
                return Err(TsvError::UnclosedQuote(rows.len() + 1));
            }
            continue;
        }
        at_start = false;
        match c {
            '\t' => {
                push_cell(&mut row, &mut cell, &mut cells)?;
                cell_chars = 0;
                at_start = true;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                push_cell(&mut row, &mut cell, &mut cells)?;
                cell_chars = 0;
                rows.push(std::mem::take(&mut row));
                at_start = true;
            }
            other => {
                cell.push(other);
                cell_chars += 1;
                if cell_chars > MAX_CELL_CHARS {
                    return Err(TsvError::TooLarge);
                }
            }
        }
    }
    if !(at_start && row.is_empty() && cell.is_empty()) {
        push_cell(&mut row, &mut cell, &mut cells)?;
        rows.push(row);
    } else if text.is_empty() {
        rows.push(vec![String::new()]);
    }
    // One trailing empty row: a text that ends with a line break has none after it (the loop above added none), and a
    // final row of one empty cell (two line breaks at the end) is dropped once.
    if rows.len() > 1 && rows.last().is_some_and(|r| r.len() == 1 && r[0].is_empty()) {
        rows.pop();
    }
    Ok(rows)
}

/// One cell as Excel writes it.
fn quote(cell: &str) -> String {
    if cell.contains(['\t', '\n', '\r', '"']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_owned()
    }
}

/// The tab-separated text of `rows`: CRLF after every row, cells quoted as Excel quotes them.
pub fn write_tsv(rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    for r in rows {
        out.push_str(&r.iter().map(|c| quote(c)).collect::<Vec<_>>().join("\t"));
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(v: &[&[&str]]) -> Vec<Vec<String>> {
        v.iter()
            .map(|r| r.iter().map(|c| (*c).to_owned()).collect())
            .collect()
    }

    #[test]
    fn rows_cells_and_line_endings() {
        assert_eq!(
            parse_tsv("a\tb\r\nc\td\r\n").unwrap(),
            rows(&[&["a", "b"], &["c", "d"]])
        );
        assert_eq!(
            parse_tsv("a\tb\nc\td").unwrap(),
            rows(&[&["a", "b"], &["c", "d"]])
        );
        assert_eq!(parse_tsv("a\t\tb").unwrap(), rows(&[&["a", "", "b"]]));
        assert_eq!(
            parse_tsv("x\n\n").unwrap(),
            rows(&[&["x"]]),
            "one trailing empty row dropped"
        );
        assert_eq!(parse_tsv("x\n\n\n").unwrap(), rows(&[&["x"], &[""]]), "only one");
        assert_eq!(parse_tsv("").unwrap(), rows(&[&[""]]));
        assert_eq!(parse_tsv("a\rb").unwrap(), rows(&[&["a"], &["b"]]));
    }

    #[test]
    fn excel_quoting() {
        assert_eq!(
            parse_tsv("\"a\tb\"\t\"line\r\nbreak\"\t\"say \"\"hi\"\"\"\n").unwrap(),
            rows(&[&["a\tb", "line\r\nbreak", "say \"hi\""]])
        );
        assert_eq!(parse_tsv("\"q\"x\ty").unwrap(), rows(&[&["qx", "y"]]));
        assert_eq!(
            parse_tsv("a\"b").unwrap(),
            rows(&[&["a\"b"]]),
            "a quote inside a cell is text"
        );
        assert_eq!(
            parse_tsv("ok\n\"open\tcell").unwrap_err(),
            TsvError::UnclosedQuote(2)
        );
        assert_eq!(TsvError::UnclosedQuote(2).message(), "unclosed quote in row 2");
    }

    #[test]
    fn limits() {
        let long = "x".repeat(MAX_CELL_CHARS + 1);
        assert_eq!(parse_tsv(&long).unwrap_err(), TsvError::TooLarge);
        assert_eq!(parse_tsv(&format!("\"{long}\"")).unwrap_err(), TsvError::TooLarge);
        assert!(parse_tsv(&"x".repeat(MAX_CELL_CHARS)).is_ok());
        let many = "a\t".repeat(MAX_CELLS);
        assert_eq!(parse_tsv(&many).unwrap_err(), TsvError::TooLarge);
        assert!(parse_tsv(&"a\t".repeat(MAX_CELLS - 1)).is_ok());
    }

    /// SplitMix64: a seeded, deterministic source for the fuzz arm.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        fn below(&mut self, n: usize) -> usize {
            usize::try_from(self.next() % n as u64).unwrap_or(0)
        }
    }

    /// TB4's fuzz arm: random text, dense in quotes, TABs and line breaks, never makes the parser panic; a text that
    /// parses writes back to a text that parses to the same rows (except a final row of one empty cell, which the
    /// one-trailing-empty-row rule drops).
    #[test]
    fn fuzz_random_text_never_panics_and_round_trips() {
        const ALPHABET: [char; 10] = ['"', '\t', '\r', '\n', 'a', 'b', ' ', '1', ',', '−'];
        let mut rng = Rng(0x5EED_2026);
        for _ in 0..4000 {
            let len = rng.below(40);
            let text: String = (0..len).map(|_| ALPHABET[rng.below(ALPHABET.len())]).collect();
            let Ok(rows) = parse_tsv(&text) else { continue };
            assert!(!rows.is_empty(), "{text:?}");
            if rows.last().is_some_and(|r| r.len() == 1 && r[0].is_empty()) {
                continue;
            }
            assert_eq!(parse_tsv(&write_tsv(&rows)).as_ref(), Ok(&rows), "{text:?}");
        }
    }

    #[test]
    fn write_then_parse_round_trips() {
        let r = rows(&[
            &["Strip", "Terminal"],
            &["-X1", "a\tb"],
            &["q\"", "line\nx"],
            &["", ""],
        ]);
        let text = write_tsv(&r);
        assert_eq!(
            text,
            "Strip\tTerminal\r\n-X1\t\"a\tb\"\r\n\"q\"\"\"\t\"line\nx\"\r\n\t\r\n"
        );
        assert_eq!(parse_tsv(&text).unwrap(), r);
    }
}
