// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command palette's matching (WP-S2.08 part 1, FR-UI-012): the scorer and the ranking of D2 (doc 16 §15.1), the
//! recents of D3, the prefixes of D4 and the parameter forms of D5 (a). [`crate::shell::ShellModel::palette_entries`]
//! builds the entries with these pieces; this module holds no shell state.
//!
//! **Score** (`score`): the best subsequence alignment of the query in a text, by dynamic programming over at most 64
//! query characters: +32 for a match at the text's first character; +16 at a word start (after a space, `-`, `.`, `_`,
//! at a digit/letter boundary or a lower-to-upper case boundary); +8 for a match that follows the previous one; +1
//! when the case is the same; −1 per skipped character, at most −16 per gap (the gap before the first match counts).
//! No alignment: no score, and the entry is left out.
//!
//! **Rank** (`rank`): `max(score(label), 0.9·score(alias), 0.8·score(id)) + min(20, 5·uses in the last 50) + 10 when
//! enabled`, kept in tenths so that it is an integer. **Order** (`order`): enabled entries first, then rank
//! descending, then the lower-cased label by Unicode code point, then the ID (D2: no ICU collation, decision O1).

use std::collections::VecDeque;

use crate::actions::{ActionMeta, ParamSpec, parse_signature};

/// The longest query the scorer aligns, in characters; the rest is ignored.
pub const MAX_QUERY: usize = 64;
/// How many invocations the recents keep (D3).
pub const RECENT_CAP: usize = 50;
/// How many recent distinct actions an empty query lists first (D3, PL-4).
pub const RECENT_SHOWN: usize = 8;
/// The most object hits an `@` query lists (D4).
pub const MAX_OBJECTS: usize = 50;

const MATCH_FIRST: i32 = 32;
const MATCH_WORD: i32 = 16;
const MATCH_NEXT: i32 = 8;
const SAME_CASE: i32 = 1;
const GAP_CAP: i32 = 16;

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Whether `s[j]` starts a word: after a space, `-`, `.` or `_`, at a digit/letter boundary or where a lower-case
/// letter is followed by an upper-case one.
fn word_start(s: &[char], j: usize) -> bool {
    if j == 0 {
        return true;
    }
    let (p, c) = (s[j - 1], s[j]);
    matches!(p, ' ' | '-' | '.' | '_')
        || (p.is_ascii_digit() != c.is_ascii_digit() && p.is_alphanumeric() && c.is_alphanumeric())
        || (p.is_lowercase() && c.is_uppercase())
}

/// The score of `query` in `text` (see the module text); `None` when the query is not a subsequence of the text
/// (case-insensitively). An empty query scores 0.
pub fn score(query: &str, text: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().take(MAX_QUERY).collect();
    if q.is_empty() {
        return Some(0);
    }
    let s: Vec<char> = text.chars().collect();
    let (m, n) = (q.len(), s.len());
    if m > n {
        return None;
    }
    // A cheap subsequence test first: most entries do not match at all.
    let mut at = 0;
    for &c in &q {
        let c = fold(c);
        at = at + s[at..].iter().position(|&x| fold(x) == c)? + 1;
    }
    const NONE: i32 = i32::MIN / 2;
    let bonus: Vec<i32> = (0..n)
        .map(|j| {
            if j == 0 {
                MATCH_FIRST
            } else if word_start(&s, j) {
                MATCH_WORD
            } else {
                0
            }
        })
        .collect();
    let mut prev = vec![NONE; n];
    let mut cur = vec![NONE; n];
    for (i, &qc) in q.iter().enumerate() {
        let qf = fold(qc);
        // The best `prev[k] - 16` over the k so far back (gap ≥ 16, k ≤ j − 17) that the penalty is capped; the
        // nearer k are scored one by one.
        let cap = GAP_CAP as usize;
        let mut far = NONE;
        for j in 0..n {
            cur[j] = NONE;
            if i > 0 && j > cap && prev[j - cap - 1] > NONE {
                far = far.max(prev[j - cap - 1] - GAP_CAP);
            }
            if j < i || fold(s[j]) != qf {
                continue;
            }
            let base = bonus[j] + if s[j] == qc { SAME_CASE } else { 0 };
            let best = if i == 0 {
                -(j as i32).min(GAP_CAP)
            } else {
                let mut best = far;
                for k in j.saturating_sub(cap)..j {
                    if prev[k] > NONE {
                        let gap = (j - k - 1) as i32;
                        let v = prev[k] - gap.min(GAP_CAP) + if gap == 0 { MATCH_NEXT } else { 0 };
                        best = best.max(v);
                    }
                }
                best
            };
            if best > NONE {
                cur[j] = best + base;
            }
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev.into_iter().filter(|&v| v > NONE).max()
}

/// The character indices of `text` that the best alignment of `query` matches (for the label's emphasis), ascending;
/// empty when the query does not align. Of equal alignments the one ending earliest is kept.
pub fn matched(query: &str, text: &str) -> Vec<usize> {
    let q: Vec<char> = query.chars().take(MAX_QUERY).collect();
    let s: Vec<char> = text.chars().collect();
    let (m, n) = (q.len(), s.len());
    if m == 0 || m > n || score(query, text).is_none() {
        return Vec::new();
    }
    const NONE: i32 = i32::MIN / 2;
    let mut dp = vec![vec![NONE; n]; m];
    let mut back = vec![vec![0usize; n]; m];
    for i in 0..m {
        for j in i..n {
            if fold(s[j]) != fold(q[i]) {
                continue;
            }
            let start = if j == 0 {
                MATCH_FIRST
            } else if word_start(&s, j) {
                MATCH_WORD
            } else {
                0
            };
            let base = start + if s[j] == q[i] { SAME_CASE } else { 0 };
            if i == 0 {
                dp[0][j] = base - (j as i32).min(GAP_CAP);
                continue;
            }
            for k in i - 1..j {
                if dp[i - 1][k] == NONE {
                    continue;
                }
                let gap = (j - k - 1) as i32;
                let v = dp[i - 1][k] - gap.min(GAP_CAP) + if gap == 0 { MATCH_NEXT } else { 0 } + base;
                if v > dp[i][j] {
                    dp[i][j] = v;
                    back[i][j] = k;
                }
            }
        }
    }
    let mut end: Option<usize> = None;
    for j in 0..n {
        if dp[m - 1][j] > NONE && end.is_none_or(|e| dp[m - 1][j] > dp[m - 1][e]) {
            end = Some(j);
        }
    }
    let Some(mut j) = end else {
        return Vec::new();
    };
    let mut out = vec![0; m];
    for i in (0..m).rev() {
        out[i] = j;
        if i > 0 {
            j = back[i][j];
        }
    }
    out
}

/// The rank of an entry in tenths (see the module text); `None` when neither the label, an alias nor the ID aligns.
pub fn rank<'a>(
    query: &str,
    label: &str,
    aliases: impl IntoIterator<Item = &'a str>,
    id: &str,
    uses: usize,
    enabled: bool,
) -> Option<i64> {
    let label = score(query, label).map(|s| 10 * i64::from(s));
    let alias = aliases
        .into_iter()
        .filter_map(|a| score(query, a))
        .max()
        .map(|s| 9 * i64::from(s));
    let id = score(query, id).map(|s| 8 * i64::from(s));
    let best = [label, alias, id].into_iter().flatten().max()?;
    let recency = 10 * (5 * uses as i64).min(20);
    Some(best + recency + if enabled { 100 } else { 0 })
}

/// The palette's order for two ranked entries: enabled first, then rank descending, then the lower-cased label by
/// code point, then the ID (D2, PL-2).
pub fn order(a: (bool, i64, &str, &str), b: (bool, i64, &str, &str)) -> std::cmp::Ordering {
    b.0.cmp(&a.0)
        .then(b.1.cmp(&a.1))
        .then_with(|| a.2.to_lowercase().cmp(&b.2.to_lowercase()))
        .then_with(|| a.3.cmp(b.3))
}

/// The last [`RECENT_CAP`] palette or shortcut invocations of the session, most recent last (D3); in memory only.
#[derive(Clone, Default, Debug)]
pub struct Recents {
    uses: VecDeque<&'static str>,
}

impl Recents {
    /// Counts one invocation of action `id`.
    pub fn note(&mut self, id: &'static str) {
        if self.uses.len() == RECENT_CAP {
            self.uses.pop_front();
        }
        self.uses.push_back(id);
    }

    /// How often `id` was invoked among the kept invocations.
    pub fn uses(&self, id: &str) -> usize {
        self.uses.iter().filter(|u| **u == id).count()
    }

    /// The distinct actions invoked, most recent first.
    pub fn distinct(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for id in self.uses.iter().rev() {
            if !out.contains(id) {
                out.push(id);
            }
        }
        out
    }
}

/// What the palette searches, by the query's first character (D4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prefix {
    /// None or `>`: the actions.
    Actions,
    /// `@`: objects through the find index.
    Objects,
    /// `#`: pages by folio or title.
    Pages,
    /// `:`: a cell in the project's cross-reference format.
    Cell,
    /// `?`: the rule IDs of the shipped catalogue and the Help menu's actions.
    Help,
}

/// The prefix of `query` and the text after it, trimmed.
pub fn split_prefix(query: &str) -> (Prefix, &str) {
    let q = query.trim_start();
    let prefix = match q.chars().next() {
        Some('>') => Prefix::Actions,
        Some('@') => Prefix::Objects,
        Some('#') => Prefix::Pages,
        Some(':') => Prefix::Cell,
        Some('?') => Prefix::Help,
        _ => return (Prefix::Actions, q.trim()),
    };
    (prefix, q[1..].trim())
}

/// How the palette runs an action (D5 (a)).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FormPlan {
    /// The action runs as it is (it needs no parameter, or asks for them itself).
    Direct,
    /// An inline form with one row per parameter of the signature.
    Form(Vec<ParamSpec>),
    /// Listed, disabled, with this reason key.
    Disabled(&'static str),
}

/// Whether a signature type is one an inline form edits: `bool`, `uint`, `int`, `text` or an enumeration.
fn form_type(ty: &str) -> bool {
    matches!(ty, "bool" | "uint" | "int" | "text") || ty.starts_with("enum(")
}

/// Whether a signature type needs a file or a selection: `path`, `ref`, or a list of either.
fn file_or_selection(ty: &str) -> bool {
    let inner = ty
        .strip_prefix("list(")
        .and_then(|t| t.strip_suffix(')'))
        .unwrap_or(ty);
    matches!(inner, "path" | "ref")
}

/// The form plan of an action (D5 (a)): an action without `needs_parameters` runs directly; one whose parameters are
/// all of a form type gets a form; one with a `path` or reference parameter is disabled with "needs a file or a
/// selection"; any other (a `json` or `mm` parameter) is disabled until its form exists.
pub fn form_plan(meta: &ActionMeta) -> FormPlan {
    if !meta.needs_parameters {
        return FormPlan::Direct;
    }
    let Ok(specs) = parse_signature(meta.params) else {
        return FormPlan::Disabled("reason-parameters-later");
    };
    if specs.iter().any(|p| file_or_selection(&p.ty)) {
        return FormPlan::Disabled("reason-palette-needs-file");
    }
    if specs.is_empty() || !specs.iter().all(|p| form_type(&p.ty)) {
        return FormPlan::Disabled("reason-parameters-later");
    }
    FormPlan::Form(specs)
}

/// One row of a parameter form.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PaletteField {
    /// The parameter's name.
    pub name: String,
    /// Its signature type (`bool`, `uint`, `int`, `text`, `enum(a|b)`).
    pub ty: String,
    /// Whether it may be left empty (then it is not sent).
    pub optional: bool,
    /// The value as text.
    pub value: String,
}

impl PaletteField {
    /// The choices of an enumeration or a `bool`; empty for a typed-in value.
    pub fn choices(&self) -> Vec<&str> {
        if self.ty == "bool" {
            return vec!["false", "true"];
        }
        self.ty
            .strip_prefix("enum(")
            .and_then(|t| t.strip_suffix(')'))
            .map(|t| t.split('|').collect())
            .unwrap_or_default()
    }

    /// The row's label: the name with `_` as spaces and its first letter in upper case (`keep_gaps` → "Keep gaps").
    pub fn label(&self) -> String {
        let text = self.name.replace('_', " ");
        let mut chars = text.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().chain(chars).collect(),
            None => text,
        }
    }
}

/// An open parameter form of the palette (D5 (a)).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PaletteForm {
    /// The action it runs.
    pub action: &'static str,
    /// The rows, in signature order.
    pub fields: Vec<PaletteField>,
    /// The message of the last refused or failed run.
    pub error: Option<String>,
}

impl PaletteForm {
    /// The form of `action` pre-filled from the defaults: an enumeration on its first choice, a `bool` false, a
    /// required number 0, everything else empty.
    pub fn new(action: &'static str, specs: &[ParamSpec]) -> PaletteForm {
        let fields = specs
            .iter()
            .map(|p| {
                let value = if p.ty == "bool" {
                    "false".to_owned()
                } else if let Some(t) = p.ty.strip_prefix("enum(") {
                    t.split(['|', ')']).next().unwrap_or("").to_owned()
                } else if matches!(p.ty.as_str(), "uint" | "int") && !p.optional {
                    "0".to_owned()
                } else {
                    String::new()
                };
                PaletteField {
                    name: p.name.clone(),
                    ty: p.ty.clone(),
                    optional: p.optional,
                    value,
                }
            })
            .collect();
        PaletteForm {
            action,
            fields,
            error: None,
        }
    }

    /// The parameters the form runs with, typed by the signature; an empty optional row is left out.
    ///
    /// # Errors
    ///
    /// The name of the first row whose text is not of its type.
    pub fn params(&self) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        let mut out = serde_json::Map::new();
        for f in &self.fields {
            let text = f.value.trim();
            if text.is_empty() && (f.optional || f.ty == "text") {
                if !f.optional {
                    out.insert(f.name.clone(), serde_json::Value::from(""));
                }
                continue;
            }
            let value = match f.ty.as_str() {
                "bool" => match text {
                    "true" => serde_json::Value::Bool(true),
                    "false" => serde_json::Value::Bool(false),
                    _ => return Err(f.name.clone()),
                },
                "uint" => text
                    .parse::<u64>()
                    .map(serde_json::Value::from)
                    .map_err(|_| f.name.clone())?,
                "int" => text
                    .parse::<i64>()
                    .map(serde_json::Value::from)
                    .map_err(|_| f.name.clone())?,
                _ if f.ty.starts_with("enum(") => {
                    if !f.choices().contains(&text) {
                        return Err(f.name.clone());
                    }
                    serde_json::Value::from(text)
                }
                _ => serde_json::Value::from(f.value.as_str()),
            };
            out.insert(f.name.clone(), value);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_match_at_the_start_earns_32_and_the_same_case_1() {
        assert_eq!(score("r", "renumber"), Some(32 + 1));
        assert_eq!(score("r", "Renumber"), Some(32));
    }

    #[test]
    fn a_match_at_a_word_start_earns_16() {
        // "d" at index 9 of "renumber devices": 9 skipped (−9), word start (+16), same case (+1).
        assert_eq!(score("d", "renumber devices"), Some(-9 + 16 + 1));
        assert!(word_start(&['a', '-', 'b'], 2));
        assert!(word_start(&['a', '.', 'b'], 2));
        assert!(word_start(&['a', '_', 'b'], 2));
        assert!(word_start(&['a', '1'], 1), "a digit after a letter");
        assert!(word_start(&['1', 'a'], 1), "a letter after a digit");
        assert!(word_start(&['a', 'B'], 1), "a case boundary");
        assert!(!word_start(&['a', 'b'], 1));
        assert!(!word_start(&['A', 'b'], 1));
    }

    #[test]
    fn consecutive_matches_earn_8_each() {
        // r(32+1) e(8+1) n(8+1)
        assert_eq!(score("ren", "renumber"), Some(33 + 9 + 9));
    }

    #[test]
    fn a_skipped_character_costs_1_and_a_gap_at_most_16() {
        // "rb": r at 0 (33), b at 5 after four skipped (−4, +1 same case).
        assert_eq!(score("rb", "renumber"), Some(33 - 4 + 1));
        // The leading gap counts: "x" at index 3.
        assert_eq!(score("x", "abcx"), Some(-3 + 1));
        // A gap of 30 costs 16.
        let long = format!("a{}b", "c".repeat(30));
        assert_eq!(score("ab", &long), Some(33 - 16 + 1));
        assert_eq!(score("b", &format!("{}b", "c".repeat(30))), Some(-16 + 1));
    }

    #[test]
    fn the_best_alignment_wins() {
        // "dv" in "devices": d at 0 then v at 2 (one skipped) beats any other placement.
        assert_eq!(score("dv", "devices"), Some(33 - 1 + 1));
        // "rd" prefers the word start of "devices" (three skipped, +16) over the plain "d" one skip away.
        assert_eq!(score("rd", "rad devices"), Some(33 - 3 + 16 + 1));
        assert_eq!(score("rnmdv", "Renumber devices…").map(|s| s > 0), Some(true));
    }

    #[test]
    fn no_alignment_is_none_and_an_empty_query_scores_zero() {
        assert_eq!(score("zz", "renumber"), None);
        assert_eq!(score("renumberx", "renumber"), None);
        assert_eq!(score("", "anything"), Some(0));
    }

    /// The oracle: every alignment of `q` in `s` scored by the rules of the module text, the best kept.
    fn brute(q: &[char], s: &[char], qi: usize, from: usize, last: Option<usize>, acc: i32) -> Option<i32> {
        if qi == q.len() {
            return Some(acc);
        }
        let mut best: Option<i32> = None;
        for j in from..s.len() {
            if fold(q[qi]) != fold(s[j]) {
                continue;
            }
            let gap = match last {
                None => j as i32,
                Some(k) => (j - k - 1) as i32,
            };
            let mut v = acc - gap.min(GAP_CAP);
            v += if j == 0 {
                MATCH_FIRST
            } else if word_start(s, j) {
                MATCH_WORD
            } else {
                0
            };
            if last.is_some() && gap == 0 {
                v += MATCH_NEXT;
            }
            if q[qi] == s[j] {
                v += SAME_CASE;
            }
            if let Some(r) = brute(q, s, qi + 1, j + 1, Some(j), v) {
                best = Some(best.map_or(r, |b| b.max(r)));
            }
        }
        best
    }

    #[test]
    fn the_dynamic_programme_equals_the_exhaustive_oracle() {
        // A fixed linear congruential sequence: deterministic, no dependency.
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) as usize
        };
        let alphabet: Vec<char> = "abAB -.1_".chars().collect();
        for _ in 0..3000 {
            let n = next() % 26;
            let m = 1 + next() % 4;
            let s: Vec<char> = (0..n).map(|_| alphabet[next() % alphabet.len()]).collect();
            let q: Vec<char> = (0..m).map(|_| alphabet[next() % 4]).collect();
            let (qs, ss): (String, String) = (q.iter().collect(), s.iter().collect());
            assert_eq!(score(&qs, &ss), brute(&q, &s, 0, 0, None, 0), "{qs:?} in {ss:?}");
        }
    }

    #[test]
    fn matched_characters_follow_the_best_alignment() {
        assert_eq!(matched("rd", "rad devices"), [0, 4]);
        assert_eq!(matched("ren", "Renumber"), [0, 1, 2]);
        assert!(matched("zz", "renumber").is_empty());
        assert!(matched("", "renumber").is_empty());
    }

    #[test]
    fn the_query_is_cut_at_64_characters() {
        let text = "a".repeat(100);
        assert_eq!(score(&"a".repeat(80), &text), score(&"a".repeat(64), &text));
    }

    #[test]
    fn rank_takes_the_best_of_label_alias_and_id_with_their_weights() {
        // The label wins outright.
        assert_eq!(rank("ab", "ab", [], "x.y", 0, false), Some(10 * 42));
        // An alias weighs 0.9.
        assert_eq!(rank("ab", "zz", ["ab"], "x.y", 0, false), Some(9 * 42));
        // The ID weighs 0.8.
        assert_eq!(rank("ab", "zz", [], "ab", 0, false), Some(8 * 42));
        assert_eq!(rank("ab", "zz", [], "x.y", 0, true), None);
    }

    #[test]
    fn rank_adds_5_per_use_up_to_20_and_10_when_enabled() {
        let base = rank("", "a", [], "a", 0, false).unwrap();
        assert_eq!(rank("", "a", [], "a", 1, false), Some(base + 50));
        assert_eq!(rank("", "a", [], "a", 4, false), Some(base + 200));
        assert_eq!(rank("", "a", [], "a", 9, false), Some(base + 200));
        assert_eq!(rank("", "a", [], "a", 0, true), Some(base + 100));
    }

    #[test]
    fn order_puts_enabled_first_then_rank_then_label_then_id() {
        use std::cmp::Ordering::*;
        assert_eq!(order((true, 0, "b", "b"), (false, 99, "a", "a")), Less);
        assert_eq!(order((true, 5, "b", "b"), (true, 3, "a", "a")), Less);
        assert_eq!(order((true, 5, "B", "b"), (true, 5, "a", "a")), Greater);
        assert_eq!(order((true, 5, "a", "x.b"), (true, 5, "A", "x.a")), Greater);
    }

    #[test]
    fn recents_keep_the_last_50_and_list_distinct_most_recent_first() {
        let mut r = Recents::default();
        for _ in 0..60 {
            r.note("a.a");
        }
        assert_eq!(r.uses("a.a"), RECENT_CAP);
        r.note("b.b");
        r.note("c.c");
        r.note("b.b");
        assert_eq!(r.uses("a.a"), RECENT_CAP - 3);
        assert_eq!(r.distinct(), ["b.b", "c.c", "a.a"]);
    }

    #[test]
    fn prefixes_split_off_their_character() {
        assert_eq!(split_prefix("renum"), (Prefix::Actions, "renum"));
        assert_eq!(split_prefix("> renum"), (Prefix::Actions, "renum"));
        assert_eq!(split_prefix("@-102F"), (Prefix::Objects, "-102F"));
        assert_eq!(split_prefix("#103"), (Prefix::Pages, "103"));
        assert_eq!(split_prefix(":103-A1"), (Prefix::Cell, "103-A1"));
        assert_eq!(split_prefix("?EW-POT-013"), (Prefix::Help, "EW-POT-013"));
        assert_eq!(split_prefix(""), (Prefix::Actions, ""));
    }

    fn meta_with(params: &'static str, needs: bool) -> ActionMeta {
        let mut m = *crate::actions::find("help.about").unwrap();
        m.params = params;
        m.needs_parameters = needs;
        m
    }

    #[test]
    fn form_plans_follow_the_signature() {
        assert_eq!(form_plan(&meta_with("dir?:path", false)), FormPlan::Direct);
        assert!(matches!(
            form_plan(&meta_with("by:enum(folio|type), n?:uint, b?:bool, t:text, i:int", true)),
            FormPlan::Form(p) if p.len() == 5
        ));
        assert_eq!(
            form_plan(&meta_with("file:path", true)),
            FormPlan::Disabled("reason-palette-needs-file")
        );
        assert_eq!(
            form_plan(&meta_with("ids:list(ref)", true)),
            FormPlan::Disabled("reason-palette-needs-file")
        );
        assert_eq!(
            form_plan(&meta_with("scale:json", true)),
            FormPlan::Disabled("reason-parameters-later")
        );
        assert_eq!(
            form_plan(&meta_with("x:mm", true)),
            FormPlan::Disabled("reason-parameters-later")
        );
    }

    #[test]
    fn a_form_is_prefilled_and_types_its_parameters() {
        let specs =
            parse_signature("by:enum(folio|type), index:uint, keep?:bool, name?:text, step?:int").unwrap();
        let mut form = PaletteForm::new("x.y", &specs);
        let values: Vec<&str> = form.fields.iter().map(|f| f.value.as_str()).collect();
        assert_eq!(values, ["folio", "0", "false", "", ""]);
        assert_eq!(form.fields[2].label(), "Keep");
        assert_eq!(
            serde_json::Value::Object(form.params().unwrap()),
            serde_json::json!({"by": "folio", "index": 0, "keep": false})
        );
        form.fields[1].value = "x".into();
        assert_eq!(form.params(), Err("index".to_owned()));
        form.fields[1].value = "2".into();
        form.fields[0].value = "nope".into();
        assert_eq!(form.params(), Err("by".to_owned()));
        form.fields[0].value = "type".into();
        form.fields[4].value = "-3".into();
        assert_eq!(form.params().unwrap()["step"], serde_json::json!(-3));
    }
}
