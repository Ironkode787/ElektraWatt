// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The CPs of a template-origin function (doc 03 §7 "CP instantiation", FT-655): which template entry a function
//! follows and how that entry's pattern CPs pair with its function definition's CPs.
//!
//! One rule, read by every place that needs a template-origin function's CP indices: the derived `function_cps`
//! (`ew-derive`), the tables importer's connection endpoints (`ew-import-tables`), the endpoint remapping of
//! `library.update` (`ew-commands`) and the place catalogue's preview (`ew-api`).

use crate::library::{Part, TemplateFn};

/// One CP of a template-origin function: the pattern entry it is drawn as, its function CP index and the FnDef CP
/// it stands for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TemplateCp {
    /// The `CpData.idx` of the part pattern entry the CP is drawn as.
    pub pattern: u16,
    /// The function CP index (what `Endpoint::Cp` names).
    pub idx: u16,
    /// The `CpDef.idx` of the FnDef CP the CP stands for; `None` when the FnDef has none for it.
    pub def: Option<u16>,
}

impl TemplateFn {
    /// The CPs of a function following this entry, in function CP index order. `pattern` lists the part's
    /// `cp_pattern` indices and `def_cps` the FnDef copy's CP indices (each in any order; empty when the copy is
    /// missing).
    ///
    /// When `cps` is not empty, names as many pattern entries as the FnDef has CPs and every index it lists is in
    /// the pattern, they pair **by position**: the FnDef's `k`-th CP (in `idx` order) is drawn as pattern entry
    /// `cps[k]` and keeps the FnDef CP's `idx` (a relay's second changeover contact, pattern `[5, 6, 7]`, gets CPs
    /// 0, 1 and 2). Otherwise (an empty `cps` for the whole pattern, another count, an index outside the pattern)
    /// the selected pattern entries keep their own `idx`, in `idx` order, and stand for the FnDef CP of the same
    /// `idx`. Where the pattern's indices are the FnDef's, both readings agree.
    #[must_use]
    pub fn function_cps(&self, pattern: &[u16], def_cps: &[u16]) -> Vec<TemplateCp> {
        pair_template_cps(&self.cps, pattern, def_cps)
    }
}

/// [`TemplateFn::function_cps`] for an entry given by its `cps` alone (a reader that holds the entry in another
/// shape, such as the tables importer).
#[must_use]
pub fn pair_template_cps(entry_cps: &[u16], pattern: &[u16], def_cps: &[u16]) -> Vec<TemplateCp> {
    let mut def: Vec<u16> = def_cps.to_vec();
    def.sort_unstable();
    def.dedup();
    let positional = !entry_cps.is_empty()
        && entry_cps.len() == def.len()
        && entry_cps.iter().all(|i| pattern.contains(i));
    if positional {
        return entry_cps
            .iter()
            .zip(&def)
            .map(|(p, d)| TemplateCp {
                pattern: *p,
                idx: *d,
                def: Some(*d),
            })
            .collect();
    }
    let mut selected: Vec<u16> = pattern
        .iter()
        .copied()
        .filter(|c| entry_cps.is_empty() || entry_cps.contains(c))
        .collect();
    selected.sort_unstable();
    selected.dedup();
    selected
        .into_iter()
        .map(|p| TemplateCp {
            pattern: p,
            idx: p,
            def: def.binary_search(&p).ok().map(|_| p),
        })
        .collect()
}

/// The template entry a function follows among its device's part uses (FT-655): `parts` gives each part use, in
/// `Device.parts` order, as a caller's key and the part copy (a use whose copy is missing is left out);
/// `template_entry` is `Function.template_entry`; `names_def` tells whether an entry names the function's FnDef copy.
///
/// The entry is the one at `template_entry` in the first part use whose template has, at that index, an entry naming
/// the FnDef; without the index, or when it names no such entry in any part use, it is the first entry naming the
/// FnDef, in part-use order. `None` when no entry names it (the function is not template-origin).
pub fn template_entry_of<'a, K, I, F>(
    parts: I,
    template_entry: Option<u16>,
    names_def: F,
) -> Option<(K, &'a Part, usize)>
where
    K: Copy,
    I: IntoIterator<Item = (K, &'a Part)> + Clone,
    F: Fn(&TemplateFn) -> bool,
{
    let by_entry = template_entry.and_then(|i| {
        let i = usize::from(i);
        parts.clone().into_iter().find_map(|(k, part)| {
            part.function_template
                .get(i)
                .filter(|t| names_def(t))
                .map(|_| (k, part, i))
        })
    });
    by_entry.or_else(|| {
        parts.into_iter().find_map(|(k, part)| {
            part.function_template
                .iter()
                .position(&names_def)
                .map(|n| (k, part, n))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::TemplateCp;
    use crate::library::TemplateFn;
    use crate::value::LibRef;

    fn entry(cps: &[u16]) -> TemplateFn {
        TemplateFn {
            fndef: serde_json::from_value::<LibRef>(serde_json::json!({
                "library": "00000000-0000-4000-8000-000000000001",
                "item": "00000000-0000-4000-8000-000000000002",
                "version_req": "^0.1"
            }))
            .expect("a LibRef"),
            main: false,
            cps: cps.to_vec(),
            extra: Default::default(),
        }
    }

    fn cp(pattern: u16, idx: u16, def: Option<u16>) -> TemplateCp {
        TemplateCp { pattern, idx, def }
    }

    #[test]
    fn as_many_pattern_cps_as_fndef_cps_pair_by_position() {
        // a contactor's aux contact 13/14 at pattern 8 and 9 of a two-CP FnDef
        let got = entry(&[8, 9]).function_cps(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9], &[1, 0]);
        assert_eq!(got, vec![cp(8, 0, Some(0)), cp(9, 1, Some(1))]);
    }

    #[test]
    fn another_count_keeps_the_pattern_indices() {
        let got = entry(&[3, 1]).function_cps(&[0, 1, 2, 3], &[0, 1, 2]);
        assert_eq!(got, vec![cp(1, 1, Some(1)), cp(3, 3, None)]);
    }

    #[test]
    fn an_empty_entry_takes_the_whole_pattern() {
        let got = entry(&[]).function_cps(&[2, 0, 1], &[0, 1]);
        assert_eq!(got, vec![cp(0, 0, Some(0)), cp(1, 1, Some(1)), cp(2, 2, None)]);
    }

    #[test]
    fn an_index_outside_the_pattern_falls_back() {
        let got = entry(&[0, 7]).function_cps(&[0, 1], &[0, 1]);
        assert_eq!(got, vec![cp(0, 0, Some(0))]);
    }
}
