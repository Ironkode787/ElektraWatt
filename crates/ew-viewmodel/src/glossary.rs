// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The product glossary (WP-S2.08 D5 (b), FR-UI-045): `glossary/glossary.toml`, compiled in and parsed once. It is
//! the one source of the command palette's aliases ([`aliases_of`]), of the user guide's glossary page (`cargo xtask
//! docs-gen` reads the same file) and of the glossary panel of a later part. A parse failure is a test failure
//! (`the_shipped_glossary_parses`), never a run-time error: [`terms`] then holds no term.

use std::sync::OnceLock;

/// The compiled-in glossary file.
pub const SOURCE: &str = include_str!("../glossary/glossary.toml");

/// One term of the glossary.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Term {
    /// The EW term (English).
    pub ew: String,
    /// The term per UI language, as (locale, text) pairs in file order (`en-GB`, `nl-NL`).
    pub labels: Vec<(String, String)>,
    /// What an EPLAN user calls it.
    pub eplan: Vec<String>,
    /// What a QElectroTech user calls it.
    pub qet: Vec<String>,
    /// The definition per UI language, as (locale, text) pairs in file order.
    pub definition: Vec<(String, String)>,
    /// The action IDs its aliases find; empty for a term the palette cannot act on.
    pub actions: Vec<String>,
}

impl Term {
    /// The label in `locale`, else the `en-GB` one, else the EW term.
    pub fn label(&self, locale: &str) -> &str {
        pick(&self.labels, locale).unwrap_or(&self.ew)
    }

    /// The definition in `locale`, else the `en-GB` one, else empty.
    pub fn definition(&self, locale: &str) -> &str {
        pick(&self.definition, locale).unwrap_or("")
    }

    /// The palette aliases of the term: the EW term, then the EPLAN and the QET terms, in file order.
    pub fn aliases(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.ew.as_str())
            .chain(self.eplan.iter().map(String::as_str))
            .chain(self.qet.iter().map(String::as_str))
    }
}

fn pick<'a>(pairs: &'a [(String, String)], locale: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(l, _)| l == locale)
        .or_else(|| pairs.iter().find(|(l, _)| l == "en-GB"))
        .map(|(_, t)| t.as_str())
}

/// Parses a glossary file.
///
/// # Errors
///
/// A message naming the term and the member that is missing or of the wrong type, or the TOML syntax error.
pub fn parse(text: &str) -> Result<Vec<Term>, String> {
    let table: toml::Table = text.parse().map_err(|e| format!("glossary: {e}"))?;
    let Some(list) = table.get("term").and_then(toml::Value::as_array) else {
        return Err("glossary: no [[term]]".to_owned());
    };
    let mut out = Vec::with_capacity(list.len());
    for (i, v) in list.iter().enumerate() {
        let Some(t) = v.as_table() else {
            return Err(format!("glossary: term {i} is not a table"));
        };
        let ew = t
            .get("ew")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("glossary: term {i} has no `ew`"))?
            .to_owned();
        let strings = |key: &str| -> Result<Vec<String>, String> {
            match t.get(key) {
                None => Ok(Vec::new()),
                Some(toml::Value::Array(a)) => a
                    .iter()
                    .map(|s| {
                        s.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| format!("glossary: `{ew}`.{key} holds a non-string"))
                    })
                    .collect(),
                Some(_) => Err(format!("glossary: `{ew}`.{key} is not a list")),
            }
        };
        let locales = |key: &str| -> Result<Vec<(String, String)>, String> {
            let Some(tab) = t.get(key).and_then(toml::Value::as_table) else {
                return Err(format!("glossary: `{ew}` has no [term.{key}]"));
            };
            let pairs: Vec<(String, String)> = tab
                .iter()
                .map(|(l, s)| {
                    s.as_str()
                        .map(|s| (l.clone(), s.to_owned()))
                        .ok_or_else(|| format!("glossary: `{ew}`.{key}.{l} is not a string"))
                })
                .collect::<Result<_, _>>()?;
            if !pairs.iter().any(|(l, _)| l == "en-GB") {
                return Err(format!("glossary: `{ew}`.{key} has no en-GB"));
            }
            Ok(pairs)
        };
        let (eplan, qet, actions) = (strings("eplan")?, strings("qet")?, strings("actions")?);
        out.push(Term {
            labels: locales("labels")?,
            eplan,
            qet,
            definition: locales("definition")?,
            actions,
            ew,
        });
    }
    Ok(out)
}

/// The shipped glossary, parsed once; empty if the file does not parse (a test guards against that).
pub fn terms() -> &'static [Term] {
    static TERMS: OnceLock<Vec<Term>> = OnceLock::new();
    TERMS.get_or_init(|| parse(SOURCE).unwrap_or_default())
}

/// The palette aliases of action `id`: the aliases of every term that names it, in file order.
pub fn aliases_of(id: &str) -> Vec<&'static str> {
    terms()
        .iter()
        .filter(|t| t.actions.iter().any(|a| a == id))
        .flat_map(Term::aliases)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_glossary_parses() {
        let terms = parse(SOURCE).unwrap();
        assert!(!terms.is_empty());
        assert_eq!(super::terms(), terms.as_slice());
    }

    #[test]
    fn every_named_action_exists_and_every_term_has_both_languages() {
        for t in terms() {
            for a in &t.actions {
                assert!(crate::actions::find(a).is_some(), "{}: unknown action {a}", t.ew);
            }
            for locale in ["en-GB", "nl-NL"] {
                assert!(
                    t.labels.iter().any(|(l, _)| l == locale),
                    "{}: label {locale}",
                    t.ew
                );
                assert!(
                    t.definition.iter().any(|(l, _)| l == locale),
                    "{}: definition {locale}",
                    t.ew
                );
            }
        }
    }

    #[test]
    fn the_nfr_doc_05_terms_are_aliases() {
        let all: Vec<String> = terms()
            .iter()
            .flat_map(Term::aliases)
            .map(str::to_lowercase)
            .collect();
        for term in [
            "function",
            "device",
            "part",
            "cdp",
            "interruption point",
            "basic project",
            "form",
            "message management",
            "element",
            "master/slave",
            "folio report",
            "conductor",
        ] {
            assert!(all.iter().any(|a| a == term), "{term}");
        }
    }

    #[test]
    fn a_malformed_file_is_an_error_naming_the_term() {
        assert!(parse("x = 1").is_err());
        let e = parse("[[term]]\new = \"A\"\n[term.labels]\nnl-NL = \"a\"\n").unwrap_err();
        assert!(e.contains("`A`") && e.contains("en-GB"), "{e}");
        let e = parse("[[term]]\new = \"A\"\neplan = \"x\"\n").unwrap_err();
        assert!(e.contains("eplan"), "{e}");
    }

    #[test]
    fn labels_fall_back_to_english() {
        let t = &parse(
            "[[term]]\new = \"A\"\n[term.labels]\nen-GB = \"A\"\n[term.definition]\nen-GB = \"d\"\nnl-NL = \"n\"\n",
        )
        .unwrap()[0];
        assert_eq!(t.label("nl-NL"), "A");
        assert_eq!(t.definition("nl-NL"), "n");
        assert_eq!(t.definition("fr-FR"), "d");
    }
}
