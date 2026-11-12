// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wire colours as a person types them (the Wire marker's Colours field, the cores of the Cable form): a code in
//! any case (`BK`, `bk`), two codes of one wire joined by `+` or `/` or written together (`GN+YE`, `GNYE`), a company
//! code the project's colour profile declares, or a colour's name from that profile (`Black`). Several wires take a
//! list, separated by commas (`BK, BU`) or, without a comma, by spaces (`BK BU`).

use std::collections::BTreeSet;

use ew_derive::ew_schema::IecColour;
use serde_json::Value;

use crate::Session;

/// A typed colour [`Session::typed_colours`] does not know: the entry as typed, and the codes the project knows (the
/// IEC 60757 codes, then the profile's company codes).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnknownColour {
    /// The entry as typed.
    pub entry: String,
    /// The codes the project knows, in order.
    pub choices: Vec<String>,
}

impl Session {
    /// The colours of `text`, one entry per wire, each one or two codes in their file spelling; empty for a blank
    /// text (module documentation).
    ///
    /// # Errors
    ///
    /// [`UnknownColour`] for the first entry that is neither a code the project knows, two of them, nor a colour's
    /// name of the colour profile.
    pub fn typed_colours(&self, text: &str) -> Result<Vec<Vec<String>>, UnknownColour> {
        let profile = self
            .query()
            .project()
            .settings
            .extra
            .get("colour_profile")
            .cloned();
        let company: Vec<String> = profile
            .as_ref()
            .map(|p| ew_derive::company_codes(p).into_iter().map(|(c, _)| c).collect())
            .unwrap_or_default();
        let names = profile.as_ref().map(colour_names).unwrap_or_default();
        let entries: Vec<&str> = if text.contains(',') {
            text.split(',').map(str::trim).filter(|e| !e.is_empty()).collect()
        } else {
            text.split_whitespace().collect()
        };
        entries
            .into_iter()
            .map(|entry| {
                entry_codes(entry, &company, &names).ok_or_else(|| UnknownColour {
                    entry: entry.to_owned(),
                    choices: IecColour::ALL
                        .iter()
                        .map(|c| c.as_str().to_owned())
                        .chain(company.iter().cloned())
                        .collect(),
                })
            })
            .collect()
    }
}

/// The colour names of a colour profile: each row's `colour_name` in every language it gives, lower case, with
/// the row's codes.
fn colour_names(profile: &Value) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for row in profile
        .get("rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let codes: Option<Vec<String>> = row
            .get("colour")
            .and_then(Value::as_array)
            .map(|c| c.iter().filter_map(Value::as_str).map(str::to_owned).collect());
        let Some(codes) = codes.filter(|c| (1..=2).contains(&c.len())) else {
            continue;
        };
        let names = row.get("colour_name").and_then(Value::as_object);
        for name in names
            .into_iter()
            .flat_map(|n| n.values())
            .filter_map(Value::as_str)
        {
            let name = name.trim().to_lowercase();
            if seen.insert(name.clone()) {
                out.push((name, codes.clone()));
            }
        }
    }
    out
}

/// One code in its file spelling: an IEC 60757 code or a declared company code, whatever the case typed.
fn code(typed: &str, company: &[String]) -> Option<String> {
    let upper = typed.to_uppercase();
    if upper.parse::<IecColour>().is_ok() {
        return Some(upper);
    }
    company.iter().find(|c| c.eq_ignore_ascii_case(typed)).cloned()
}

/// The codes of one entry: a colour's name, codes joined by `+` or `/`, one code, or two IEC codes written together.
fn entry_codes(entry: &str, company: &[String], names: &[(String, Vec<String>)]) -> Option<Vec<String>> {
    let lower = entry.trim().to_lowercase();
    if let Some((_, codes)) = names.iter().find(|(n, _)| *n == lower) {
        return Some(codes.clone());
    }
    let parts: Vec<&str> = entry.split(['+', '/']).map(str::trim).collect();
    if parts.len() > 2 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let codes: Option<Vec<String>> = parts.iter().map(|p| code(p, company)).collect();
    if let Some(codes) = codes {
        return Some(codes);
    }
    // Two IEC codes written together (`GNYE`).
    let upper = entry.to_uppercase();
    if parts.len() == 1 && upper.len() == 4 && upper.is_ascii() {
        let (a, b) = upper.split_at(2);
        if a.parse::<IecColour>().is_ok() && b.parse::<IecColour>().is_ok() {
            return Some(vec![a.to_owned(), b.to_owned()]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_is_a_name_codes_or_a_pair_written_together() {
        let company = vec!["DBU".to_owned()];
        let names = vec![("black".to_owned(), vec!["BK".to_owned()])];
        let codes = |e: &str| entry_codes(e, &company, &names);
        assert_eq!(codes("bk"), Some(vec!["BK".to_owned()]));
        assert_eq!(codes("Black"), Some(vec!["BK".to_owned()]));
        assert_eq!(codes("dbu"), Some(vec!["DBU".to_owned()]));
        assert_eq!(codes("gn+ye"), Some(vec!["GN".to_owned(), "YE".to_owned()]));
        assert_eq!(codes("GN/YE"), Some(vec!["GN".to_owned(), "YE".to_owned()]));
        assert_eq!(codes("gnye"), Some(vec!["GN".to_owned(), "YE".to_owned()]));
        assert_eq!(codes("XX"), None);
        assert_eq!(codes("BK+BU+GN"), None);
        assert_eq!(codes("BK++BU"), None);
    }
}
