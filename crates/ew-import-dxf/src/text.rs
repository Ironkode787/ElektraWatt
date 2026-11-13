// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The printed text of TEXT, ATTRIB and MTEXT entities (doc 14 §6.1) and the SHX font substitution table (§6.3,
//! FR-IMP-033).

/// SHX fonts are proprietary and never shipped (doc 14 §6.3): each name of this table is drawn in the bundled family
/// it names, every other SHX font in the default family. The table is data; a text in an SHX font is reported once
/// per font (EW-IMP-252).
pub const SHX_SUBSTITUTES: &[(&str, &str)] = &[
    ("romans.shx", "Noto Sans"),
    ("romand.shx", "Noto Sans"),
    ("romanc.shx", "Noto Sans"),
    ("simplex.shx", "Noto Sans"),
    ("isocp.shx", "Noto Sans"),
    ("isocpeur.shx", "Noto Sans"),
    ("iso.shx", "Noto Sans"),
    ("txt.shx", "Noto Sans"),
    ("monotxt.shx", "Noto Sans"),
    ("arial.shx", "Noto Sans"),
    ("gothice.shx", "Noto Sans"),
    ("italic.shx", "Noto Sans"),
];

/// The family every SHX font without a row of [`SHX_SUBSTITUTES`] is drawn in, the bundled default (ADR-0028).
pub const DEFAULT_FAMILY: &str = "Noto Sans";

/// The bundled family an SHX font file is drawn in, and whether the file is an SHX font at all (a TrueType font or
/// none is no substitution). A style without a file extension names an SHX font (AutoCAD's convention).
pub fn shx_substitute(font: &str) -> Option<&'static str> {
    let file = font.rsplit(['/', '\\']).next().unwrap_or(font).trim().to_ascii_lowercase();
    if file.is_empty() {
        return None;
    }
    let shx = std::path::Path::new(&file)
        .extension()
        .is_none_or(|e| e.eq_ignore_ascii_case("shx"));
    if !shx {
        return None;
    }
    let with_ext = if file.ends_with(".shx") { file } else { format!("{file}.shx") };
    Some(
        SHX_SUBSTITUTES
            .iter()
            .find(|(name, _)| *name == with_ext)
            .map_or(DEFAULT_FAMILY, |(_, family)| family),
    )
}

/// The control codes of a TEXT string resolved: `%%c` ⌀ (as Ø), `%%d` °, `%%p` ±, `%%%` %, `%%nnn` the character
/// of code nnn, and the underline and overline toggles `%%u`, `%%o` removed.
pub fn percent_codes(s: &str) -> String {
    if !s.contains("%%") {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && chars.get(i + 1) == Some(&'%') {
            match chars.get(i + 2).map(char::to_ascii_lowercase) {
                Some('c') => out.push('Ø'),
                Some('d') => out.push('°'),
                Some('p') => out.push('±'),
                Some('%') => out.push('%'),
                Some('u' | 'o' | 'k') => {}
                Some(d) if d.is_ascii_digit() => {
                    let digits: String = chars[i + 2..].iter().take(3).take_while(|c| c.is_ascii_digit()).collect();
                    if let Some(c) = digits.parse::<u32>().ok().and_then(char::from_u32) {
                        out.push(c);
                    }
                    i += 2 + digits.len();
                    continue;
                }
                _ => {
                    out.push_str("%%");
                    i += 2;
                    continue;
                }
            }
            i += 3;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// The lines of an MTEXT string with its formatting removed (doc 14 §6.1): `\P` breaks a line; `\f`, `\F`, `\H`, `\C`,
/// `\c`, `\T`, `\Q`, `\W`, `\A` and `\p` up to their `;` are dropped; a stack `\S a^b;` prints `a/b`; the toggles
/// `\L \l \O \o \K \k` and the braces of a group are dropped; `\~` is a space; `\\`, `\{` and `\}` print their
/// character. `%%` codes resolve as in TEXT.
pub fn mtext_lines(s: &str) -> Vec<String> {
    let mut lines = vec![String::new()];
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '{' | '}' => i += 1,
            '\\' => {
                let Some(&code) = chars.get(i + 1) else {
                    break;
                };
                match code {
                    'P' | 'X' => {
                        lines.push(String::new());
                        i += 2;
                    }
                    '~' => {
                        tail(&mut lines).push(' ');
                        i += 2;
                    }
                    '\\' | '{' | '}' => {
                        tail(&mut lines).push(code);
                        i += 2;
                    }
                    'L' | 'l' | 'O' | 'o' | 'K' | 'k' | 'N' => i += 2,
                    'S' => {
                        let end = chars[i + 2..].iter().position(|&c| c == ';').map_or(chars.len(), |p| i + 2 + p);
                        let stack: String = chars[i + 2..end]
                            .iter()
                            .map(|&c| if matches!(c, '^' | '#') { '/' } else { c })
                            .collect();
                        tail(&mut lines).push_str(stack.trim());
                        i = end + 1;
                    }
                    'f' | 'F' | 'H' | 'C' | 'c' | 'T' | 'Q' | 'W' | 'A' | 'p' => {
                        i = chars[i + 2..].iter().position(|&c| c == ';').map_or(chars.len(), |p| i + 3 + p);
                    }
                    _ => {
                        tail(&mut lines).push(code);
                        i += 2;
                    }
                }
            }
            _ => {
                tail(&mut lines).push(c);
                i += 1;
            }
        }
    }
    lines.into_iter().map(|l| percent_codes(&l)).collect()
}

/// The last line of a non-empty list of lines.
fn tail(lines: &mut Vec<String>) -> &mut String {
    if lines.is_empty() {
        lines.push(String::new());
    }
    let last = lines.len() - 1;
    &mut lines[last]
}
