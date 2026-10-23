// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ID alias `@n` (decision log ID-630, FT-422): the one grammar for record IDs in machine text.
//!
//! **Form.** `@` and a decimal number from 1 without a leading zero, as a whole token (in JSON the string `"@3"`).
//! An `@` inside a longer word is never an alias (`ew.iec.mcb-1pn@qet10`, a profile's `id@version`).
//!
//! **Printing.** Every hyphenated UUID the shell writes into a transcript line (`state`, `buffer`, `error`, `message`)
//! or into the describe text is printed as its alias, minted or stable alike ([`alias_text`]). The table is numbered
//! in the order of first appearance in that output, so one script on equal inputs prints the same aliases (ID-474's
//! byte equality). It belongs to the shell model, lives for one session (in the headless mode, the script run; a new,
//! opened or imported project starts a new table) and is stored nowhere. The model, the window's widgets and the
//! `--record` log keep real UUIDs.
//!
//! **Input.** Where an action parameter takes a UUID or a reference ([`resolve_params`]), `@n` resolves through the
//! same table before anything else reads the parameter; an alias the table does not hold is refused naming it, and
//! `--record` writes the resolved UUID because the action only ever sees it.

use serde_json::{Map, Value};

/// The session's aliases: alias `@n` is entry `n - 1`.
#[derive(Clone, Debug, Default)]
pub struct AliasTable {
    /// The UUID texts (lower case) in order of first appearance.
    ids: Vec<String>,
    /// The number of each UUID text.
    numbers: std::collections::BTreeMap<String, usize>,
}

impl AliasTable {
    /// An empty table.
    pub fn new() -> AliasTable {
        AliasTable::default()
    }

    /// How many aliases the table holds.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether the table holds no alias.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Forgets every alias (a new session).
    pub fn clear(&mut self) {
        self.ids.clear();
        self.numbers.clear();
    }

    /// The number of `uuid` (a hyphenated UUID text, either case), given the next free number on its first call.
    pub fn number(&mut self, uuid: &str) -> usize {
        let key = uuid.to_ascii_lowercase();
        if let Some(n) = self.numbers.get(&key) {
            return *n;
        }
        self.ids.push(key.clone());
        let n = self.ids.len();
        self.numbers.insert(key, n);
        n
    }

    /// The number of `uuid` if it has one, without assigning.
    pub fn number_of(&self, uuid: &str) -> Option<usize> {
        self.numbers.get(&uuid.to_ascii_lowercase()).copied()
    }

    /// The UUID text (lower case) of alias number `n`.
    pub fn uuid(&self, n: usize) -> Option<&str> {
        n.checked_sub(1).and_then(|i| self.ids.get(i)).map(String::as_str)
    }
}

/// Whether `w` is a hyphenated UUID: 8-4-4-4-12 hexadecimal digits.
fn is_uuid(w: &[u8]) -> bool {
    w.len() == 36
        && w.iter().enumerate().all(|(k, c)| match k {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// The first hyphenated UUID of `text` and its byte offset, if any.
pub fn find_uuid(text: &str) -> Option<(usize, &str)> {
    let b = text.as_bytes();
    (0..b.len().saturating_sub(35))
        .find(|&i| is_uuid(&b[i..i + 36]))
        .map(|i| (i, &text[i..i + 36]))
}

/// `text` with every hyphenated UUID replaced by its alias `@n`, numbering a UUID the table does not hold yet in the
/// order of first appearance.
pub fn alias_text(table: &mut AliasTable, text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut copied = 0;
    while i + 36 <= b.len() {
        // A UUID is ASCII, so `i` and `i + 36` are character boundaries when it matches.
        if is_uuid(&b[i..i + 36]) {
            out.push_str(&text[copied..i]);
            out.push('@');
            out.push_str(&table.number(&text[i..i + 36]).to_string());
            i += 36;
            copied = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// The number of `token` if it is an alias: `@`, then a decimal number from 1 without a leading zero, nothing else.
pub fn parse_alias(token: &str) -> Option<usize> {
    let digits = token.strip_prefix('@')?;
    let first = digits.bytes().next()?;
    if first == b'0' || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Whether `token` is an alias (a value token like a number, not a word of the describe vocabulary).
pub fn is_alias(token: &str) -> bool {
    parse_alias(token).is_some()
}

/// The UUID text that alias `token` names in `table`; `None` when `token` is no alias or the table does not hold it.
pub fn resolve<'a>(table: &'a AliasTable, token: &str) -> Option<&'a str> {
    parse_alias(token).and_then(|n| table.uuid(n))
}

/// `token` resolved when it is an alias, else unchanged; `Err` with the alias when the table does not hold it.
fn resolve_whole(table: &AliasTable, token: &str) -> Result<Option<String>, String> {
    if !is_alias(token) {
        return Ok(None);
    }
    resolve(table, token)
        .map(|u| Some(u.to_owned()))
        .ok_or_else(|| token.to_owned())
}

/// Whether `c` continues a word, so that an `@` after it or a digit run before it is no alias boundary.
fn word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@')
}

/// `text` with every alias that stands as its own token inside it (between characters that do not continue a word,
/// as in a navigator key `s:1/p:@3`) resolved; `Ok(None)` when it holds none; `Err` with the first alias the table
/// does not hold.
fn resolve_embedded(table: &AliasTable, text: &str) -> Result<Option<String>, String> {
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut prev: Option<char> = None;
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c == '@' && !prev.is_some_and(word_char) {
            let digits = rest[1..].bytes().take_while(u8::is_ascii_digit).count();
            let end = 1 + digits;
            let next = rest[end..].chars().next();
            if digits > 0 && !next.is_some_and(word_char) {
                let token = &rest[..end];
                if let Some(n) = parse_alias(token) {
                    let uuid = table.uuid(n).ok_or_else(|| token.to_owned())?;
                    out.push_str(uuid);
                    changed = true;
                    prev = token.chars().last();
                    rest = &rest[end..];
                    continue;
                }
            }
        }
        out.push(c);
        prev = Some(c);
        rest = &rest[c.len_utf8()..];
    }
    Ok(changed.then_some(out))
}

/// `value` with every command-log reference `{"ref": "@n"}` inside it replaced by `{"ref": "id:<uuid>"}`.
fn resolve_refs(table: &AliasTable, value: &Value) -> Result<Option<Value>, String> {
    match value {
        Value::Object(map)
            if map.len() == 1 && map.get("ref").and_then(Value::as_str).is_some_and(is_alias) =>
        {
            let token = map["ref"].as_str().unwrap_or_default();
            let uuid = resolve(table, token).ok_or_else(|| token.to_owned())?;
            Ok(Some(serde_json::json!({ "ref": format!("id:{uuid}") })))
        }
        Value::Object(map) => {
            let mut out: Option<Map<String, Value>> = None;
            for (k, v) in map {
                if let Some(new) = resolve_refs(table, v)? {
                    out.get_or_insert_with(|| map.clone()).insert(k.clone(), new);
                }
            }
            Ok(out.map(Value::Object))
        }
        Value::Array(items) => {
            let mut out: Option<Vec<Value>> = None;
            for (i, v) in items.iter().enumerate() {
                if let Some(new) = resolve_refs(table, v)? {
                    out.get_or_insert_with(|| items.clone())[i] = new;
                }
            }
            Ok(out.map(Value::Array))
        }
        _ => Ok(None),
    }
}

/// How an action parameter takes a UUID or a reference.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParamForm {
    /// A text that is a UUID (or a key, a part number, a tag) as a whole: `"@3"` becomes the UUID.
    Whole,
    /// A list of UUID texts: each `"@3"` becomes the UUID.
    List,
    /// A list of command-log reference texts (`tag:…`, `id:…`): each `"@3"` becomes `id:<uuid>`.
    RefList,
    /// A text with UUIDs inside it (a navigator row key, `s:1/p:@3`).
    Embedded,
    /// Command parameters, whose command-log references `{"ref": "@3"}` resolve to `{"ref": "id:<uuid>"}`.
    CommandRefs,
}

/// The action parameters that take a UUID or a reference (ID-630 (c)): action ID, parameter, form.
pub const ALIAS_PARAMS: &[(&str, &str, ParamForm)] = &[
    ("select.set", "ids", ParamForm::List),
    ("select.set", "refs", ParamForm::RefList),
    ("place.pick", "symbol", ParamForm::Whole),
    ("place.pick", "part", ParamForm::Whole),
    ("place.pick", "function", ParamForm::Whole),
    ("place.pick", "strip", ParamForm::Whole),
    ("rep.exchange", "symbol", ParamForm::Whole),
    ("pages.toggle", "key", ParamForm::Embedded),
    ("pages.row", "key", ParamForm::Embedded),
    ("pages.open", "key", ParamForm::Embedded),
    ("command.run", "params", ParamForm::CommandRefs),
];

/// The parameters of action `action` with every alias in a parameter of [`ALIAS_PARAMS`] resolved through `table`:
/// `Ok(None)` when nothing changed; `Err` with the first alias the table does not hold. A parameter of another type
/// than its form expects is left for the action to refuse.
pub fn resolve_params(
    table: &AliasTable,
    action: &str,
    params: &Map<String, Value>,
) -> Result<Option<Map<String, Value>>, String> {
    let mut out: Option<Map<String, Value>> = None;
    for (_, name, form) in ALIAS_PARAMS.iter().filter(|(a, _, _)| *a == action) {
        let Some(value) = params.get(*name) else {
            continue;
        };
        let new = match (form, value) {
            (ParamForm::Whole, Value::String(s)) => resolve_whole(table, s)?.map(Value::String),
            (ParamForm::Embedded, Value::String(s)) => resolve_embedded(table, s)?.map(Value::String),
            (ParamForm::List | ParamForm::RefList, Value::Array(items)) => {
                let prefix = if *form == ParamForm::RefList { "id:" } else { "" };
                let mut list: Option<Vec<Value>> = None;
                for (i, v) in items.iter().enumerate() {
                    if let Some(s) = v.as_str()
                        && let Some(u) = resolve_whole(table, s)?
                    {
                        list.get_or_insert_with(|| items.clone())[i] = Value::String(format!("{prefix}{u}"));
                    }
                }
                list.map(Value::Array)
            }
            (ParamForm::CommandRefs, v) => resolve_refs(table, v)?,
            _ => None,
        };
        if let Some(new) = new {
            out.get_or_insert_with(|| params.clone())
                .insert((*name).to_owned(), new);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const A: &str = "9eb2d865-fca4-59b1-87bf-c073d7190ceb";
    const B: &str = "51b0b676-8978-50af-91cc-cde58ea478d9";

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn uuids_print_as_aliases_in_order_of_first_appearance() {
        let mut t = AliasTable::new();
        let text = format!(
            "{{\"selection\":[\"{B}\",\"{A}\"],\"hover\":\"{B}\"}} ‹{}›",
            A.to_uppercase()
        );
        assert_eq!(
            alias_text(&mut t, &text),
            "{\"selection\":[\"@1\",\"@2\"],\"hover\":\"@1\"} ‹@2›"
        );
        assert_eq!(t.len(), 2);
        assert_eq!(t.uuid(1), Some(B));
        assert_eq!(t.number_of(A), Some(2));
        // A later text keeps the numbers.
        assert_eq!(alias_text(&mut t, &format!("x {A}")), "x @2");
        // Text without a UUID, a UUID-like run that is too short, and non-ASCII text pass unchanged.
        let plain = "Ölstand · 9eb2d865-fca4-59b1-87bf c073 ‹varies›";
        assert_eq!(alias_text(&mut t, plain), plain);
        assert_eq!(find_uuid(plain), None);
        assert_eq!(find_uuid(&format!("id:{A}")), Some((3, A)));
    }

    #[test]
    fn the_alias_grammar_is_a_whole_token_from_one_without_a_leading_zero() {
        assert_eq!(parse_alias("@1"), Some(1));
        assert_eq!(parse_alias("@42"), Some(42));
        for no in [
            "@0",
            "@01",
            "@",
            "@x",
            "@1a",
            "1",
            "x@1",
            "ew.iec.mcb-1pn@qet10",
            "id@1.2",
            " @1",
        ] {
            assert_eq!(parse_alias(no), None, "{no}");
        }
    }

    #[test]
    fn aliases_resolve_and_an_unknown_one_is_named() {
        let mut t = AliasTable::new();
        t.number(A);
        t.number(B);
        assert_eq!(resolve(&t, "@2"), Some(B));
        assert_eq!(resolve(&t, "@3"), None);
        assert_eq!(resolve(&t, B), None);
        let p = obj(json!({"ids": ["@2", A, "@1"]}));
        assert_eq!(
            resolve_params(&t, "select.set", &p).unwrap().unwrap(),
            obj(json!({"ids": [B, A, A]}))
        );
        let p = obj(json!({"refs": ["tag:-101F1", "@9"]}));
        assert_eq!(resolve_params(&t, "select.set", &p), Err("@9".to_owned()));
        // A reference list takes the alias as an `id:` reference.
        let p = obj(json!({"refs": ["tag:-101F1", "@1"]}));
        assert_eq!(
            resolve_params(&t, "select.set", &p).unwrap().unwrap(),
            obj(json!({"refs": ["tag:-101F1", format!("id:{A}")]}))
        );
        // Nothing to resolve: no copy.
        let p = obj(json!({"refs": ["tag:-101F1"]}));
        assert_eq!(resolve_params(&t, "select.set", &p), Ok(None));
        // An action without ID parameters, and a free-text parameter, keep `@n` as text.
        let p = obj(json!({"text": "@1"}));
        assert_eq!(resolve_params(&t, "props.input", &p), Ok(None));
        assert_eq!(
            resolve_params(&t, "place.pick", &obj(json!({"letter": "@1"}))),
            Ok(None)
        );
        // A symbol key with an `@` inside is no alias.
        let p = obj(json!({"symbol": "ew.iec.contact-no@qet10", "function": "@1"}));
        assert_eq!(
            resolve_params(&t, "place.pick", &p).unwrap().unwrap(),
            obj(json!({"symbol": "ew.iec.contact-no@qet10", "function": A}))
        );
    }

    #[test]
    fn keys_and_command_references_resolve_inside() {
        let mut t = AliasTable::new();
        t.number(A);
        let p = obj(json!({"key": "s:1/p:@1"}));
        assert_eq!(
            resolve_params(&t, "pages.open", &p).unwrap().unwrap(),
            obj(json!({ "key": format!("s:1/p:{A}") }))
        );
        assert_eq!(
            resolve_params(&t, "pages.row", &obj(json!({"key": "f:x@1/l:@12a"}))),
            Ok(None)
        );
        assert_eq!(
            resolve_params(&t, "pages.row", &obj(json!({"key": "p:@4"}))),
            Err("@4".to_owned())
        );
        let p = obj(
            json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "device", "id": {"ref": "@1"}}],
            "note": "@1", "other": {"ref": "tag:-101F1"}}}),
        );
        let r = resolve_params(&t, "command.run", &p).unwrap().unwrap();
        assert_eq!(
            r["params"],
            json!({"targets": [{"kind": "device", "id": {"ref": format!("id:{A}")}}], "note": "@1",
                "other": {"ref": "tag:-101F1"}})
        );
        let p = obj(json!({"cmd": "x", "params": {"id": {"ref": "@2"}}}));
        assert_eq!(resolve_params(&t, "command.run", &p), Err("@2".to_owned()));
    }
}
