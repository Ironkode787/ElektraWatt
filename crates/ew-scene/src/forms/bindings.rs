// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Placeholders and their bindings (doc 09 §2.6): `{scope.path | filter …}` templates filled from a
//! [`Bindings`] source.
//!
//! A template is literal text with `{…}` placeholders; `{{` and `}}` print `{` and `}`. A placeholder is
//! `scope.segment(.segment)*` followed by `|`-separated filters, each `name` or `name(args)` with
//! comma-separated arguments (double-quoted strings without escapes, or bare words). Implemented filters:
//! `date` (the bindings' date format), `upper`, `default(x)` and `tag(full | displayed)`; the others of doc 09
//! (`supply`, `qty`, `ml`, `range`, …) are reported as [`BindingFinding::UnsupportedFilter`] and pass the value
//! through unchanged.
//!
//! Nothing is silently dropped: an unresolved path, an unknown scope and a computed cell (`{= …}`, not in v0)
//! print the placeholder verbatim and are reported, and a malformed template prints verbatim as a whole.
//! [`eval_visible_if`] fills an expression the same way and tests the text for truthiness.
//!
//! Validating placeholders against the form type (FR-REP-034) is not done here.

use std::collections::BTreeMap;

use ew_mltext::{LangTag, MlText};
use ew_schema::{Date, DateFormat};

/// The scopes a frame or title-block placeholder may name (doc 09 §2.6). `ew` only carries `ew.version`.
pub const SCOPES: [&str; 7] = ["project", "party", "revision", "page", "doc", "export", "ew"];

/// A value a placeholder path resolves to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BoundValue {
    /// Plain text.
    Text(String),
    /// Multi-language text, printed in the bindings' language.
    Ml(MlText),
    /// A date, printed in the bindings' date format.
    Date(Date),
    /// A tag in its two printed forms; prints `displayed` unless `tag(full)` asks otherwise.
    Tag {
        /// The full tag (`=P01+J-E-2416-F1`).
        full: String,
        /// The displayed tag (`-F1`).
        displayed: String,
    },
    /// An asset by SHA-256 (a logo); prints no text, the cell draws the image.
    Asset([u8; 32]),
}

/// A source of placeholder values.
pub trait Bindings {
    /// The value at `path` (`page.title`, `party.end_user.name`), without braces or filters.
    fn resolve(&self, path: &str) -> Option<BoundValue>;
    /// The display language for [`BoundValue::Ml`] values and multi-language labels.
    fn language(&self) -> Option<&LangTag> {
        None
    }
    /// The project's date format (FR-PRJ-023).
    fn date_format(&self) -> DateFormat {
        DateFormat::DdMmYyyy
    }
}

/// Why a path cannot be bound.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BindingError {
    /// The path is not `scope.segment(.segment)*` with a scope of [`SCOPES`].
    InvalidPath(String),
}

impl std::fmt::Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BindingError::InvalidPath(p) => write!(f, "invalid binding path `{p}`"),
        }
    }
}

impl std::error::Error for BindingError {}

/// The bindings of one page, filled by the caller (`ew-derive` later) for the `project`, `party`,
/// `revision`, `page`, `doc`, `export` and `ew` scopes.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PageBindings {
    /// The display language.
    pub language: Option<LangTag>,
    /// The date format; `None` prints dd-mm-yyyy.
    pub date_format: Option<DateFormat>,
    values: BTreeMap<String, BoundValue>,
}

impl PageBindings {
    /// Empty bindings.
    pub fn new() -> PageBindings {
        PageBindings::default()
    }

    /// Binds `path` to `value`.
    ///
    /// # Errors
    ///
    /// [`BindingError::InvalidPath`] if `path` is not a valid path of a known scope.
    pub fn set(&mut self, path: &str, value: BoundValue) -> Result<&mut PageBindings, BindingError> {
        match split_path(path) {
            Some(segments) if SCOPES.contains(&segments[0]) => {
                self.values.insert(path.to_owned(), value);
                Ok(self)
            }
            _ => Err(BindingError::InvalidPath(path.to_owned())),
        }
    }

    /// Binds `path` to plain text.
    ///
    /// # Errors
    ///
    /// As [`set`](PageBindings::set).
    pub fn text(&mut self, path: &str, text: &str) -> Result<&mut PageBindings, BindingError> {
        self.set(path, BoundValue::Text(text.to_owned()))
    }
}

impl Bindings for PageBindings {
    fn resolve(&self, path: &str) -> Option<BoundValue> {
        self.values.get(path).cloned()
    }
    fn language(&self) -> Option<&LangTag> {
        self.language.as_ref()
    }
    fn date_format(&self) -> DateFormat {
        self.date_format.clone().unwrap_or(DateFormat::DdMmYyyy)
    }
}

/// Something worth reporting while filling a template.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BindingFinding {
    /// The path resolved to nothing; the placeholder prints verbatim.
    Unresolved {
        /// The placeholder as written, braces included.
        placeholder: String,
    },
    /// The scope is not one of [`SCOPES`]; the placeholder prints verbatim.
    UnknownScope {
        /// The placeholder as written.
        placeholder: String,
    },
    /// A filter this version does not implement; the value passes through unchanged.
    UnsupportedFilter {
        /// The placeholder as written.
        placeholder: String,
        /// The filter name.
        filter: String,
    },
    /// A computed cell (`{= …}`), not implemented in v0; prints verbatim.
    Computed {
        /// The placeholder as written.
        placeholder: String,
    },
    /// Unbalanced braces or an unparsable placeholder; the whole template prints verbatim.
    Malformed {
        /// The template.
        template: String,
    },
}

/// A filled template.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Filled {
    /// The text to print.
    pub text: String,
    /// The first asset a placeholder resolved to, if any.
    pub asset: Option<[u8; 32]>,
    /// What to report.
    pub findings: Vec<BindingFinding>,
}

/// Fills `template` from `bindings`.
pub fn fill(template: &str, bindings: &dyn Bindings) -> Filled {
    fill_with(template, bindings, false)
}

/// Evaluates a `visible_if` expression: the filled text, trimmed, is visible unless it is empty, `false` or `0`;
/// a leading `!` negates. A malformed expression is visible.
pub fn eval_visible_if(expr: &str, bindings: &dyn Bindings) -> (bool, Vec<BindingFinding>) {
    let expr = expr.trim();
    let (negate, rest) = match expr.strip_prefix('!') {
        Some(rest) => (true, rest.trim()),
        None => (false, expr),
    };
    let filled = fill_with(rest, bindings, true);
    if filled
        .findings
        .iter()
        .any(|f| matches!(f, BindingFinding::Malformed { .. }))
    {
        return (true, filled.findings);
    }
    let text = filled.text.trim();
    let visible = !(text.is_empty() || text.eq_ignore_ascii_case("false") || text == "0");
    (visible != negate, filled.findings)
}

/// One piece of a parsed template.
enum Piece {
    Literal(String),
    Placeholder { raw: String, kind: PlaceholderKind },
}

enum PlaceholderKind {
    Computed,
    UnknownScope,
    Path { path: String, filters: Vec<Filter> },
}

struct Filter {
    name: String,
    args: Vec<String>,
}

/// The segments of a valid `scope.segment(.segment)*` path.
fn split_path(path: &str) -> Option<Vec<&str>> {
    let segments: Vec<&str> = path.split('.').collect();
    let ok = segments.len() >= 2
        && segments.iter().all(|s| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        });
    ok.then_some(segments)
}

/// Splits `s` on `sep` outside double quotes and parentheses; `None` if quotes or parentheses are unbalanced.
fn split_top(s: &str, sep: char) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let (mut quoted, mut depth, mut from) = (false, 0usize, 0);
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth = depth.checked_sub(1)?,
            c if c == sep && !quoted && depth == 0 => {
                parts.push(&s[from..i]);
                from = i + c.len_utf8();
            }
            _ => {}
        }
    }
    if quoted || depth != 0 {
        return None;
    }
    parts.push(&s[from..]);
    Some(parts)
}

fn parse_filter(s: &str) -> Option<Filter> {
    let (name, args) = match s.find('(') {
        None => (s, Vec::new()),
        Some(open) => {
            let inner = s[open + 1..].strip_suffix(')')?;
            let mut args = Vec::new();
            if !inner.trim().is_empty() {
                for raw in split_top(inner, ',')? {
                    let raw = raw.trim();
                    if let Some(q) = raw.strip_prefix('"') {
                        let q = q.strip_suffix('"')?;
                        if q.contains('"') {
                            return None;
                        }
                        args.push(q.to_owned());
                    } else {
                        // Parentheses inside the arguments are not part of the grammar.
                        if raw.is_empty() || raw.contains(['"', '(', ')']) {
                            return None;
                        }
                        args.push(raw.to_owned());
                    }
                }
            }
            (s[..open].trim_end(), args)
        }
    };
    let name_ok = !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    name_ok.then(|| Filter {
        name: name.to_owned(),
        args,
    })
}

fn parse_placeholder(content: &str) -> Option<PlaceholderKind> {
    if content.trim_start().starts_with('=') {
        return Some(PlaceholderKind::Computed);
    }
    let parts = split_top(content, '|')?;
    let path = parts[0].trim();
    let segments = split_path(path)?;
    let filters = parts[1..]
        .iter()
        .map(|p| parse_filter(p.trim()))
        .collect::<Option<Vec<Filter>>>()?;
    if !SCOPES.contains(&segments[0]) {
        return Some(PlaceholderKind::UnknownScope);
    }
    Some(PlaceholderKind::Path {
        path: path.to_owned(),
        filters,
    })
}

/// Parses a whole template; `None` if it is malformed.
fn parse_template(template: &str) -> Option<Vec<Piece>> {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut chars = template.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '{' if matches!(chars.peek(), Some((_, '{'))) => {
                chars.next();
                literal.push('{');
            }
            '}' if matches!(chars.peek(), Some((_, '}'))) => {
                chars.next();
                literal.push('}');
            }
            '}' => return None,
            '{' => {
                let rest = &template[i + 1..];
                let close = rest.find(['{', '}'])?;
                if rest.as_bytes()[close] == b'{' {
                    return None;
                }
                let kind = parse_placeholder(&rest[..close])?;
                if !literal.is_empty() {
                    pieces.push(Piece::Literal(std::mem::take(&mut literal)));
                }
                let end = i + 1 + close + 1;
                pieces.push(Piece::Placeholder {
                    raw: template[i..end].to_owned(),
                    kind,
                });
                while chars.peek().is_some_and(|&(j, _)| j < end) {
                    chars.next();
                }
            }
            c => literal.push(c),
        }
    }
    if !literal.is_empty() {
        pieces.push(Piece::Literal(literal));
    }
    Some(pieces)
}

fn fill_with(template: &str, bindings: &dyn Bindings, unresolved_empty: bool) -> Filled {
    let mut out = Filled::default();
    let Some(pieces) = parse_template(template) else {
        out.text = template.to_owned();
        out.findings.push(BindingFinding::Malformed {
            template: template.to_owned(),
        });
        return out;
    };
    for piece in pieces {
        match piece {
            Piece::Literal(l) => out.text.push_str(&l),
            Piece::Placeholder { raw, kind } => match kind {
                PlaceholderKind::Computed => {
                    out.text.push_str(&raw);
                    out.findings.push(BindingFinding::Computed { placeholder: raw });
                }
                PlaceholderKind::UnknownScope => {
                    out.text.push_str(&raw);
                    out.findings
                        .push(BindingFinding::UnknownScope { placeholder: raw });
                }
                PlaceholderKind::Path { path, filters } => {
                    match eval_placeholder(&raw, &path, &filters, bindings, &mut out.findings) {
                        Some(BoundValue::Asset(h)) => {
                            out.asset.get_or_insert(h);
                        }
                        Some(v) => out.text.push_str(&render(&v, bindings)),
                        None => {
                            if !unresolved_empty {
                                out.text.push_str(&raw);
                            }
                            out.findings.push(BindingFinding::Unresolved { placeholder: raw });
                        }
                    }
                }
            },
        }
    }
    out
}

fn eval_placeholder(
    raw: &str,
    path: &str,
    filters: &[Filter],
    bindings: &dyn Bindings,
    findings: &mut Vec<BindingFinding>,
) -> Option<BoundValue> {
    let mut state = bindings.resolve(path);
    let mut unsupported = |filter: &str| {
        findings.push(BindingFinding::UnsupportedFilter {
            placeholder: raw.to_owned(),
            filter: filter.to_owned(),
        });
    };
    for f in filters {
        match (f.name.as_str(), f.args.as_slice()) {
            ("date", []) => {
                state = state.map(|v| match v {
                    BoundValue::Date(d) => BoundValue::Text(format_date(d, &bindings.date_format())),
                    BoundValue::Text(t) => match t.parse::<Date>() {
                        Ok(d) => BoundValue::Text(format_date(d, &bindings.date_format())),
                        Err(_) => BoundValue::Text(t),
                    },
                    other => other,
                });
            }
            ("upper", []) => {
                state = state.map(|v| BoundValue::Text(render(&v, bindings).to_uppercase()));
            }
            ("default", [x]) => {
                if state.as_ref().is_none_or(|v| render(v, bindings).is_empty()) {
                    state = Some(BoundValue::Text(x.clone()));
                }
            }
            ("tag", [form]) if form == "full" || form == "displayed" => {
                if let Some(BoundValue::Tag { full, displayed }) = &state {
                    let t = if form == "full" { full } else { displayed };
                    state = Some(BoundValue::Text(t.clone()));
                }
            }
            (name, _) => unsupported(name),
        }
    }
    state
}

fn format_date(d: Date, format: &DateFormat) -> String {
    match format {
        DateFormat::DdMmYyyy => format!("{:02}-{:02}-{:04}", d.day(), d.month(), d.year()),
        DateFormat::Iso8601 | DateFormat::Unknown(_) => {
            format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day())
        }
    }
}

fn render(value: &BoundValue, bindings: &dyn Bindings) -> String {
    match value {
        BoundValue::Text(t) => t.clone(),
        BoundValue::Ml(ml) => ml_text(ml, bindings.language()).to_owned(),
        BoundValue::Date(d) => format_date(*d, &bindings.date_format()),
        BoundValue::Tag { displayed, .. } => displayed.clone(),
        BoundValue::Asset(_) => String::new(),
    }
}

/// The text of `ml` in `lang`: the neutral text, else exactly that language, else the first language in
/// tag order, else empty.
pub fn ml_text<'a>(ml: &'a MlText, lang: Option<&LangTag>) -> &'a str {
    if let Some(s) = ml.as_neutral() {
        return s;
    }
    lang.and_then(|l| ml.get(l))
        .or_else(|| ml.languages().next().and_then(|l| ml.get(l)))
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b() -> PageBindings {
        let mut b = PageBindings::new();
        b.text("page.folio", "101").unwrap();
        b.text("doc.page_count", "36").unwrap();
        b.text("party.end_user.name", "Acme").unwrap();
        b.text("page.empty", "").unwrap();
        b.text("ew.version", "1.2.3").unwrap();
        b.set(
            "project.created",
            BoundValue::Date(Date::new(2025, 3, 3).unwrap()),
        )
        .unwrap();
        b.text("project.iso", "2025-03-03").unwrap();
        b.text("project.plain", "not a date").unwrap();
        b.set(
            "page.tag",
            BoundValue::Tag {
                full: "=P01+J-E-2416-F1".to_owned(),
                displayed: "-F1".to_owned(),
            },
        )
        .unwrap();
        b.set("page.logo", BoundValue::Asset([7; 32])).unwrap();
        b
    }

    fn lang(s: &str) -> LangTag {
        s.parse::<LangTag>().unwrap()
    }

    fn st(s: &str) -> String {
        s.to_owned()
    }

    #[test]
    fn literal_only() {
        let f = fill("plain text", &b());
        assert_eq!(f.text, "plain text");
        assert!(f.findings.is_empty());
        assert_eq!(f.asset, None);
    }

    #[test]
    fn single_and_surrounded_placeholders() {
        assert_eq!(fill("{page.folio}", &b()).text, "101");
        let f = fill("Page {page.folio} of {doc.page_count}", &b());
        assert_eq!(f.text, "Page 101 of 36");
        assert!(f.findings.is_empty());
        assert_eq!(fill("v{ew.version}", &b()).text, "v1.2.3");
    }

    #[test]
    fn escapes() {
        let f = fill("{{x}}", &b());
        assert_eq!(f.text, "{x}");
        assert!(f.findings.is_empty());
    }

    #[test]
    fn malformed_templates() {
        for t in [
            "{page.title",
            "page}",
            "{a{b}}",
            "{page}",
            "{page.ti tle}",
            "{page.folio |}",
            "{page.folio | }",
            "ok {page.folio} {x",
            "{page.folio | default(\"a)}",
            "{page.folio | default(x}",
        ] {
            let f = fill(t, &b());
            assert_eq!(f.text, t, "{t}");
            assert_eq!(
                f.findings,
                vec![BindingFinding::Malformed { template: st(t) }],
                "{t}"
            );
        }
    }

    #[test]
    fn unknown_scope() {
        let f = fill("a {foo.bar} b", &b());
        assert_eq!(f.text, "a {foo.bar} b");
        assert_eq!(
            f.findings,
            vec![BindingFinding::UnknownScope {
                placeholder: st("{foo.bar}")
            }]
        );
    }

    #[test]
    fn unresolved_prints_verbatim_with_filters() {
        let f = fill("{page.missing | upper}", &b());
        assert_eq!(f.text, "{page.missing | upper}");
        assert_eq!(
            f.findings,
            vec![BindingFinding::Unresolved {
                placeholder: st("{page.missing | upper}")
            }]
        );
    }

    #[test]
    fn date_filter() {
        assert_eq!(fill("{project.created | date}", &b()).text, "03-03-2025");
        assert_eq!(fill("{project.iso | date}", &b()).text, "03-03-2025");
        let mut iso = b();
        iso.date_format = Some(DateFormat::Iso8601);
        assert_eq!(fill("{project.created | date}", &iso).text, "2025-03-03");
        assert_eq!(fill("{project.iso | date}", &iso).text, "2025-03-03");
        assert_eq!(fill("{project.created}", &iso).text, "2025-03-03");
        assert_eq!(fill("{project.created}", &b()).text, "03-03-2025");
        let f = fill("{project.plain | date}", &b());
        assert_eq!(f.text, "not a date");
        assert!(f.findings.is_empty());
    }

    #[test]
    fn upper_and_default() {
        assert_eq!(fill("{party.end_user.name | upper}", &b()).text, "ACME");
        assert_eq!(fill("{page.missing | default(\"—\")}", &b()).text, "—");
        assert_eq!(fill("{page.empty | default(\"—\")}", &b()).text, "—");
        assert_eq!(fill("{page.folio | default(\"—\")}", &b()).text, "101");
        assert!(fill("{page.missing | default(\"—\")}", &b()).findings.is_empty());
        let t = "{party.end_user.name | upper | default(\"—\")}";
        assert_eq!(fill(t, &b()).text, "ACME");
        assert_eq!(fill(t, &PageBindings::new()).text, "—");
    }

    #[test]
    fn tag_filter() {
        assert_eq!(fill("{page.tag}", &b()).text, "-F1");
        assert_eq!(fill("{page.tag | tag(full)}", &b()).text, "=P01+J-E-2416-F1");
        assert_eq!(fill("{page.tag | tag(displayed)}", &b()).text, "-F1");
        let f = fill("{page.tag | tag(bogus)}", &b());
        assert_eq!(f.text, "-F1");
        assert_eq!(
            f.findings,
            vec![BindingFinding::UnsupportedFilter {
                placeholder: st("{page.tag | tag(bogus)}"),
                filter: st("tag")
            }]
        );
        assert_eq!(fill("{page.tag | tag}", &b()).findings.len(), 1);
    }

    #[test]
    fn unsupported_filters() {
        let f = fill("{page.folio | supply}", &b());
        assert_eq!(f.text, "101");
        assert_eq!(
            f.findings,
            vec![BindingFinding::UnsupportedFilter {
                placeholder: st("{page.folio | supply}"),
                filter: st("supply")
            }]
        );
        let f = fill("{page.folio | qty(A, 1)}", &b());
        assert_eq!(f.text, "101");
        assert_eq!(
            f.findings,
            vec![BindingFinding::UnsupportedFilter {
                placeholder: st("{page.folio | qty(A, 1)}"),
                filter: st("qty")
            }]
        );
        // Filter findings come before the unresolved finding.
        let f = fill("{page.nope | ml(nl-NL)}", &b());
        assert!(matches!(f.findings[0], BindingFinding::UnsupportedFilter { .. }));
        assert!(matches!(f.findings[1], BindingFinding::Unresolved { .. }));
    }

    #[test]
    fn computed_cell() {
        let f = fill("x {= 1 + 1} y", &b());
        assert_eq!(f.text, "x {= 1 + 1} y");
        assert_eq!(
            f.findings,
            vec![BindingFinding::Computed {
                placeholder: st("{= 1 + 1}")
            }]
        );
    }

    #[test]
    fn ml_values() {
        let ml = MlText::from_langs([(lang("en-GB"), "Hello"), (lang("nl-NL"), "Hallo")]);
        let mut x = PageBindings::new();
        x.set("page.title", BoundValue::Ml(ml)).unwrap();
        x.language = Some(lang("nl-NL"));
        assert_eq!(fill("{page.title}", &x).text, "Hallo");
        x.language = Some(lang("en-GB"));
        assert_eq!(fill("{page.title}", &x).text, "Hello");
        // A missing language falls back to the first language in tag order.
        x.language = Some(lang("de-DE"));
        assert_eq!(fill("{page.title}", &x).text, "Hello");
        x.language = None;
        assert_eq!(fill("{page.title}", &x).text, "Hello");
        x.set("page.title", BoundValue::Ml(MlText::neutral("Neutral")))
            .unwrap();
        x.language = Some(lang("nl-NL"));
        assert_eq!(fill("{page.title}", &x).text, "Neutral");
    }

    #[test]
    fn asset_sets_asset() {
        let f = fill("a{page.logo}b", &b());
        assert_eq!(f.text, "ab");
        assert_eq!(f.asset, Some([7; 32]));
        assert!(f.findings.is_empty());
    }

    #[test]
    fn visible_if() {
        let vis = |e: &str| eval_visible_if(e, &b()).0;
        assert!(vis("{page.folio}"));
        assert!(!vis("{page.empty}"));
        let mut x = b();
        x.text("page.zero", "0").unwrap();
        x.text("page.f", "false").unwrap();
        x.text("page.ff", "FALSE").unwrap();
        for p in ["{page.zero}", "{page.f}", "{page.ff}"] {
            assert!(!eval_visible_if(p, &x).0, "{p}");
        }
        assert!(!vis("!{page.folio}"));
        assert!(vis("! {page.empty}"));
        let (v, findings) = eval_visible_if("{page.missing}", &b());
        assert!(!v);
        assert_eq!(
            findings,
            vec![BindingFinding::Unresolved {
                placeholder: st("{page.missing}")
            }]
        );
        let (v, findings) = eval_visible_if("{page.folio", &b());
        assert!(v);
        assert!(matches!(findings[0], BindingFinding::Malformed { .. }));
    }

    #[test]
    fn set_validates_paths() {
        let mut x = PageBindings::new();
        for p in ["page.title", "party.end_user.name", "ew.version"] {
            assert!(x.text(p, "v").is_ok(), "{p}");
            assert_eq!(x.resolve(p), Some(BoundValue::Text(st("v"))));
        }
        for p in ["page", "foo.bar", "page..x", "page.ti tle", ""] {
            assert_eq!(
                x.text(p, "v").unwrap_err(),
                BindingError::InvalidPath(st(p)),
                "{p}"
            );
        }
        x.text("page.title", "w").unwrap();
        assert_eq!(x.resolve("page.title"), Some(BoundValue::Text(st("w"))));
        assert_eq!(
            BindingError::InvalidPath(st("a")).to_string(),
            "invalid binding path `a`"
        );
    }
}
