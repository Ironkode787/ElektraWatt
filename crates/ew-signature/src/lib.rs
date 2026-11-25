// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The parameter signature grammar of the window's actions (`ew-viewmodel`'s action table, doc 16 §4.3): one parser,
//! which the window reads its actions' signatures with and the persona harness (`xtask blind`) reads the action list
//! with, so that the two never know different types.
//!
//! ```text
//! signature := "" | param (", " param)*
//! param     := name ["?"] ":" type
//! type      := "mm" | "int" | "uint" | "bool" | "text" | "page" | "path" | "json" | "ref"
//!            | "enum(" word ("|" word)* ")" | "enum(@" set ")" | "list(" type ")"
//! ```
//!
//! A named set (`enum(@PAGE_TYPES)`) is spelt out as its words by the reader's own sets ([`parse`]); a reader without
//! sets (the harness, whose list has them spelt out already) refuses one. A line of the harness's action list is read
//! here too ([`parse_action_line`]), so that the window's test that writes the list reads it back as the harness does.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// One parameter of an action's signature.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParamSpec {
    /// The parameter's name, `[a-z][a-z0-9_]*`.
    pub name: String,
    /// Whether the parameter may be left out (`?`).
    pub optional: bool,
    /// The type, as the signature spells it (`mm`, `enum(a|b)`, `list(text)`, ...), a named set by its words.
    pub ty: String,
}

/// The scalar types of the grammar; `enum(…)` and `list(…)` are the two constructors. A `page` is a schematic page
/// something is put on, by its folio, or `new` for a new one; a form offers the pages.
pub const PARAM_TYPES: [&str; 9] = ["mm", "int", "uint", "bool", "text", "page", "path", "json", "ref"];

/// Whether `w` is a word of the grammar, `[a-z][a-z0-9_]*`.
fn is_word(w: &str) -> bool {
    let mut chars = w.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `ty` with a named set `enum(@<name>)` spelt out as its words (`enum(a|b)`), in a list too; `None` for a type
/// outside the grammar or a set `named_set` does not know.
fn resolved_type<'a>(ty: &str, named_set: &impl Fn(&str) -> Option<Vec<&'a str>>) -> Option<String> {
    if PARAM_TYPES.contains(&ty) {
        return Some(ty.to_owned());
    }
    if let Some(inner) = ty.strip_prefix("enum(").and_then(|t| t.strip_suffix(')')) {
        if let Some(name) = inner.strip_prefix('@') {
            return named_set(name).map(|words| format!("enum({})", words.join("|")));
        }
        return (!inner.is_empty() && inner.split('|').all(is_word)).then(|| ty.to_owned());
    }
    if let Some(inner) = ty.strip_prefix("list(").and_then(|t| t.strip_suffix(')')) {
        return resolved_type(inner, named_set).map(|t| format!("list({t})"));
    }
    None
}

/// Parses a signature by the grammar of the crate text, a named set spelt out as the words `named_set` gives for its
/// name.
///
/// # Errors
///
/// A message naming what is wrong: an empty parameter, a name that is not `[a-z][a-z0-9_]*`, a repeated name or a
/// type outside the grammar.
pub fn parse<'a>(
    sig: &str,
    named_set: impl Fn(&str) -> Option<Vec<&'a str>>,
) -> Result<Vec<ParamSpec>, String> {
    if sig.is_empty() {
        return Ok(Vec::new());
    }
    let mut out: Vec<ParamSpec> = Vec::new();
    for part in sig.split(", ") {
        let Some((name, ty)) = part.split_once(':') else {
            return Err(format!("`{part}` has no `:type`"));
        };
        let (name, optional) = match name.strip_suffix('?') {
            Some(n) => (n, true),
            None => (name, false),
        };
        if !is_word(name) {
            return Err(format!("`{name}` is not a parameter name"));
        }
        let Some(ty) = resolved_type(ty, &named_set) else {
            return Err(format!("`{ty}` is not a type of the grammar"));
        };
        if out.iter().any(|p| p.name == name) {
            return Err(format!("`{name}` is repeated"));
        }
        out.push(ParamSpec {
            name: name.to_owned(),
            optional,
            ty,
        });
    }
    Ok(out)
}

/// One action of the action list `actions.txt`, which `ew-viewmodel`'s `blind_reference` test writes from the action
/// table and the persona harness reads ([`parse_action_line`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ActionLine {
    /// The action ID.
    pub id: String,
    /// Whether the action is of kind `View` (else `Command`).
    pub view: bool,
    /// The parameters of its signature.
    pub params: Vec<ParamSpec>,
}

/// Reads one line of `actions.txt`, `<id> <View|Command> "<label>" … params "<signature>"`: the ID, the kind and the
/// signature, which has its named sets spelt out (a set name is refused). The words between are the writer's and the
/// reader's own.
///
/// # Errors
///
/// What is wrong with the line: no ID or kind, a kind of another word, no signature or one not closed, or the
/// signature's fault ([`parse`]) after the action's ID.
pub fn parse_action_line(line: &str) -> Result<ActionLine, String> {
    let mut words = line.split(' ');
    let (Some(id), Some(kind)) = (words.next().filter(|w| !w.is_empty()), words.next()) else {
        return Err("no ID or kind".to_owned());
    };
    let view = match kind {
        "View" => true,
        "Command" => false,
        other => return Err(format!("kind `{other}`")),
    };
    let marker = " params \"";
    let start = line.rfind(marker).ok_or("no signature")?;
    let sig = line[start + marker.len()..]
        .strip_suffix('"')
        .ok_or("the signature is not closed")?;
    let params = parse(sig, |_| None).map_err(|e| format!("{id}: {e}"))?;
    Ok(ActionLine {
        id: id.to_owned(),
        view,
        params,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare(sig: &str) -> Result<Vec<ParamSpec>, String> {
        parse(sig, |_| None)
    }

    #[test]
    fn the_grammar_refuses_what_it_does_not_hold() {
        assert!(bare("a:nope").is_err());
        assert!(bare("A:int").is_err());
        assert!(
            bare("a:int,b:int").is_err(),
            "the separator is a comma and a space"
        );
        assert!(bare("a:int, a:uint").is_err());
        assert!(bare("a:enum()").is_err());
        assert!(bare("a:list(enum(x|y))").is_ok());
        assert!(bare("a:enum(@SET)").is_err(), "a set the reader does not know");
    }

    #[test]
    fn an_action_line_gives_its_id_its_kind_and_its_parameters() {
        let line = r#"edit.move_to_page Command "Move to page…" menu "Page" shortcut none context canvas params "page:page""#;
        let read = parse_action_line(line).unwrap();
        assert_eq!((read.id.as_str(), read.view), ("edit.move_to_page", false));
        assert_eq!(read.params, bare("page:page").unwrap());
        assert_eq!(
            parse_action_line("a Other params \"\""),
            Err("kind `Other`".to_owned())
        );
        assert_eq!(
            parse_action_line("a View params \"x:nope\""),
            Err("a: `nope` is not a type of the grammar".to_owned())
        );
        assert_eq!(parse_action_line("a View"), Err("no signature".to_owned()));
    }

    #[test]
    fn a_named_set_is_spelt_out_by_the_readers_sets() {
        let specs = parse("by?:enum(@SET), at:list(enum(@SET))", |n| {
            (n == "SET").then(|| vec!["x", "y"])
        })
        .unwrap();
        let types: Vec<(&str, bool, &str)> = specs
            .iter()
            .map(|p| (p.name.as_str(), p.optional, p.ty.as_str()))
            .collect();
        assert_eq!(
            types,
            [("by", true, "enum(x|y)"), ("at", false, "list(enum(x|y))")]
        );
    }
}
