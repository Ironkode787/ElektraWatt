// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arms for pages, sections, structure nodes, report blocks and external references.

use super::*;

pub(super) fn page_renumber(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(10) {
        0 => {
            let (section, first) = rng.pick(&live.sections)?;
            Some(Step::cmd(
                "page.renumber",
                json!({"scope": {"section": section}, "start": first, "step": 0}),
            ))
        }
        1 => Some(
            Step::cmd(
                "page.renumber",
                json!({"scope": {"section": rng.uuid()}, "start": 1, "step": 1}),
            )
            .refuse(),
        ),
        _ => {
            let (section, first) = rng.pick(&live.sections)?;
            let step = 1 + rng.below(2);
            Some(
                Step::cmd(
                    "page.renumber",
                    json!({"scope": {"section": section}, "start": first, "step": step}),
                )
                .counts(&[(Table::Pages, 0)]),
            )
        }
    }
}

/// The order `page.reorder` sees the pages of one section in: folio number, then label (none first, natural order),
/// then ID.
pub(super) fn folio_order(a: &PageInfo, b: &PageInfo) -> std::cmp::Ordering {
    let label = |p: &PageInfo| {
        p.label
            .as_deref()
            .and_then(|l| l.parse::<ew_mltext::Ident>().ok())
    };
    a.number
        .cmp(&b.number)
        .then_with(|| match (label(a), label(b)) {
            (Some(x), Some(y)) => x.natural_cmp(&y),
            (x, y) => x.is_some().cmp(&y.is_some()),
        })
        .then_with(|| id_text(&a.id).cmp(&id_text(&b.id)))
}

pub(super) fn page_reorder(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    // The pages of each section (of no section: the front matter), in folio order; only sections with two pages.
    let mut groups: std::collections::BTreeMap<Option<String>, Vec<&PageInfo>> = Default::default();
    for p in &live.page_info {
        groups.entry(p.section.as_ref().map(id_text)).or_default().push(p);
    }
    let mut groups: Vec<Vec<&PageInfo>> = groups.into_values().filter(|g| g.len() >= 2).collect();
    for g in &mut groups {
        g.sort_by(|a, b| folio_order(a, b));
    }
    let group = rng.pick(&groups)?;
    let i = rng.below(group.len());
    let a = group[i];
    match rng.below(10) {
        0 => Some(Step::cmd("page.reorder", json!({"pages": [a.id], "before": rng.uuid()})).refuse()),
        1 => Some(Step::cmd("page.reorder", json!({"pages": []}))),
        _ => {
            // Any other page of the section to put `a` before; `a` directly before it is no change.
            let mut j = rng.below(group.len() - 1);
            if j >= i {
                j += 1;
            }
            let b = group[j];
            let step = Step::cmd("page.reorder", json!({"pages": [a.id], "before": b.id}));
            Some(if i + 1 == j {
                step
            } else {
                step.counts(&[(Table::Pages, 0)])
            })
        }
    }
}

pub(super) fn numbering_renumber(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    if rng.chance(12) {
        return Some(
            Step::cmd(
                "numbering.renumber",
                json!({"scope": {"pages": [rng.uuid()]}, "options": {}, "include_frozen": false}),
            )
            .refuse(),
        );
    }
    let scope = match rng.below(3) {
        0 => json!("project"),
        1 => json!({"pages": [rng.pick(&live.pages)?]}),
        _ => {
            let n = 1 + rng.below(4);
            let selection: Vec<&Value> = (0..n).filter_map(|_| rng.pick(&live.plain_devices)).collect();
            json!({"selection": selection})
        }
    };
    let options = json!({"close_gaps": rng.chance(2), "only_nonconforming": rng.chance(2)});
    Some(
        Step::cmd(
            "numbering.renumber",
            json!({"scope": scope, "options": options, "include_frozen": rng.chance(4)}),
        )
        .counts(&[(Table::Devices, 0)]),
    )
}

pub(super) fn page_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let kind = *rng.pick(&["multi_line", "single_line", "overview", "fluid", "graphic"])?;
    let title = rng.word(10);
    match rng.below(12) {
        // The command refuses: report pages are generated (EW-INV-012).
        0 => Some(Step::cmd(
            "page.create",
            json!({"number": 5000, "kind": "report"}),
        )),
        // The command refuses: no folio.
        1 => Some(Step::cmd("page.create", json!({"kind": kind}))),
        // The command refuses: a folio another page of the section has.
        2 => {
            let p = rng.pick(&live.page_info)?;
            let mut params = json!({"number": p.number, "kind": kind});
            if let Some(s) = &p.section {
                params["section"] = s.clone();
            }
            if let Some(l) = &p.label {
                params["label"] = json!(l);
            }
            Some(Step::cmd("page.create", params))
        }
        // Refused: the section does not exist.
        3 => Some(
            Step::cmd(
                "page.create",
                json!({"number": 4000, "section": rng.uuid(), "kind": kind}),
            )
            .refuse(),
        ),
        // Refused: the page to insert after does not exist.
        4 => Some(Step::cmd("page.create", json!({"after": rng.uuid(), "kind": kind})).refuse()),
        // After a page: the folio is proposed.
        5 | 6 => {
            let p = rng.pick(&live.pages)?;
            Some(
                Step::cmd("page.create", json!({"after": p, "kind": kind, "title": title}))
                    .counts(&[(Table::Pages, 1)]),
            )
        }
        // A page of no section.
        7 => Some(
            Step::cmd(
                "page.create",
                json!({"number": 10_000 + rng.below(10_000), "kind": kind, "title": title}),
            )
            .counts(&[(Table::Pages, 1)]),
        ),
        // An explicit free folio in the range of a section.
        _ => {
            let s = rng.pick(&live.section_info)?;
            let taken: std::collections::BTreeSet<u32> = live
                .page_info
                .iter()
                .filter(|p| p.section.as_ref() == Some(&s.id))
                .map(|p| p.number)
                .chain(std::iter::once(s.range.0))
                .collect();
            let span = s.range.1.saturating_sub(s.range.0).min(60);
            let free: Vec<u32> = (s.range.0..=s.range.0 + span)
                .filter(|n| !taken.contains(n))
                .collect();
            let n = *rng.pick(&free)?;
            Some(
                Step::cmd(
                    "page.create",
                    json!({"section": s.id, "number": n, "kind": kind, "title": title}),
                )
                .counts(&[(Table::Pages, 1)]),
            )
        }
    }
}

pub(super) fn page_delete(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(10) {
        0 => Some(Step::cmd("page.delete", json!({"pages": [rng.uuid()]})).refuse()),
        1 => Some(Step::cmd("page.delete", json!({"pages": []}))),
        _ => {
            // The golden project keeps most of its pages: the run deletes while there are enough.
            if live.pages.len() < 6 {
                return None;
            }
            let page = rng.pick(&live.pages)?.clone();
            let delete_devices = rng.chance(3);
            let id = page.clone();
            Some(
                Step::cmd(
                    "page.delete",
                    json!({"pages": [page], "delete_devices": delete_devices}),
                )
                .check(move |before, after| {
                    let id = typed(&id).ok_or("a page ID")?;
                    if after.pages().contains(id) {
                        return Err("the deleted page still exists".into());
                    }
                    let (b, a) = (before.pages().len(), after.pages().len());
                    (a + 1 == b)
                        .then_some(())
                        .ok_or_else(|| format!("{b} pages before and {a} after: one page was deleted"))
                }),
            )
        }
    }
}

pub(super) fn section_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    // A section that ends at the last folio leaves no free range after it: no step then.
    let number = live
        .section_info
        .iter()
        .map(|s| s.number)
        .max()
        .unwrap_or(0)
        .checked_add(1)?;
    let free_from = live
        .section_info
        .iter()
        .map(|s| s.range.1)
        .max()
        .unwrap_or(0)
        .checked_add(1)?;
    match rng.below(10) {
        // The command refuses: the range starts after it ends.
        0 => Some(Step::cmd(
            "section.create",
            json!({"number": number, "folio_range": [free_from.checked_add(50)?, free_from]}),
        )),
        // The command refuses: a number a section has.
        1 => {
            let s = rng.pick(&live.section_info)?;
            Some(Step::cmd(
                "section.create",
                json!({"number": s.number, "folio_range": [free_from, free_from.checked_add(9)?]}),
            ))
        }
        // The command refuses: overlaps the range of a section.
        2 => {
            let s = rng.pick(&live.section_info)?;
            Some(Step::cmd(
                "section.create",
                json!({"number": number, "folio_range": [s.range.0, s.range.0.checked_add(3)?]}),
            ))
        }
        _ => {
            let from = free_from.checked_add(u32::try_from(rng.below(40)).unwrap_or(0))?;
            let to = from.checked_add(u32::try_from(rng.below(50)).unwrap_or(0))?;
            Some(
                Step::cmd(
                    "section.create",
                    json!({"number": number, "folio_range": [from, to], "title": rng.word(8), "divider": rng.chance(2)}),
                )
                .counts(&[(Table::Sections, 1)]),
            )
        }
    }
}

/// The aspects `structure.create_node` takes, read from its schema (never a copy of the list).
pub(super) fn aspects() -> &'static [&'static str] {
    static LIST: OnceLock<Vec<&'static str>> = OnceLock::new();
    LIST.get_or_init(|| registry_strings("structure.create_node", &["aspect"]))
}

pub(super) fn structure_create_node(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let aspect = *rng.pick(aspects())?;
    let code = format!("FZ{}", rng.below(100_000));
    match rng.below(10) {
        // The command refuses: a parent of another aspect (EW-INV-011).
        0 => {
            let other = live.nodes.iter().find(|n| n.aspect != aspect)?;
            Some(Step::cmd(
                "structure.create_node",
                json!({"aspect": aspect, "code": code, "parent": other.id}),
            ))
        }
        // The command refuses: a sibling with the same code.
        1 => {
            let n = rng.pick(&live.nodes)?;
            let mut p = json!({"aspect": n.aspect, "code": n.code});
            if let Some(parent) = &n.parent {
                p["parent"] = parent.clone();
            }
            Some(Step::cmd("structure.create_node", p))
        }
        // Refused: the product aspect is the device tree.
        2 => Some(
            Step::cmd(
                "structure.create_node",
                json!({"aspect": "product", "code": code}),
            )
            .refuse(),
        ),
        // Refused: the parent does not exist.
        3 => Some(
            Step::cmd(
                "structure.create_node",
                json!({"aspect": aspect, "code": code, "parent": rng.uuid()}),
            )
            .refuse(),
        ),
        _ => {
            let same: Vec<_> = live.nodes.iter().filter(|n| n.aspect == aspect).collect();
            let mut p = json!({"aspect": aspect, "code": code, "description": rng.word(10)});
            if rng.chance(2)
                && let Some(parent) = rng.pick(&same)
            {
                p["parent"] = parent.id.clone();
            }
            if rng.chance(4) {
                p["aliases"] = json!([format!("AL{}", rng.below(100))]);
            }
            Some(Step::cmd("structure.create_node", p).counts(&[(Table::Nodes, 1)]))
        }
    }
}

pub(super) fn report_block_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    static KINDS: OnceLock<Vec<&'static str>> = OnceLock::new();
    let kinds = KINDS.get_or_init(|| registry_strings("report_block.create", &["source", "kind"]));
    let form = rng.pick(&live.forms)?;
    let kind = *rng.pick(kinds)?;
    match rng.below(12) {
        // The form from the library with its closure; the golden project holds the same items, so the
        // held copies are used and only the block is new.
        10 => {
            let (nogrid, title_block) = shipped_report_forms();
            let s = rng.pick(&live.section_info)?;
            Some(
                Step::cmd(
                    "report_block.create",
                    json!({"source": {"kind": kind}, "form": nogrid["id"], "form_copies": [nogrid, title_block],
                           "section": s.id, "first_folio": 6100 + rng.below(50)}),
                )
                .counts(&[(Table::ReportBlocks, 1)]),
            )
        }
        // The command refuses: a form copy whose hash is not its item's.
        11 => {
            let (mut nogrid, _) = shipped_report_forms();
            nogrid["sheet"]["width"] = json!(1 + rng.below(1000));
            Some(Step::cmd(
                "report_block.create",
                json!({"source": {"kind": kind}, "form": nogrid["id"], "form_copies": [nogrid],
                           "first_folio": 6200}),
            ))
        }
        // The command refuses: a page-oriented block needs its first folio (EW-INV-014).
        0 => Some(Step::cmd(
            "report_block.create",
            json!({"source": {"kind": kind}, "form": form}),
        )),
        // Refused: the form copy does not exist.
        1 => Some(
            Step::cmd(
                "report_block.create",
                json!({"source": {"kind": kind}, "form": rng.uuid(), "first_folio": 6000}),
            )
            .refuse(),
        ),
        _ => {
            let s = rng.pick(&live.section_info)?;
            let taken: std::collections::BTreeSet<u32> = live
                .page_info
                .iter()
                .filter(|p| p.section.as_ref() == Some(&s.id))
                .map(|p| p.number)
                .chain(std::iter::once(s.range.0))
                .collect();
            let free: Vec<u32> = (s.range.0..=s.range.1.min(s.range.0 + 80))
                .filter(|n| !taken.contains(n))
                .collect();
            let n = *rng.pick(&free)?;
            Some(
                Step::cmd(
                    "report_block.create",
                    json!({"source": {"kind": kind}, "form": form, "section": s.id, "first_folio": n, "title": rng.word(8)}),
                )
                .counts(&[(Table::ReportBlocks, 1)]),
            )
        }
    }
}

/// The shipped frame without the grid and the title block it names (`content/forms`), in the `lib/` JSON form.
pub(super) fn shipped_report_forms() -> (Value, Value) {
    let parse = |text: &str| serde_json::from_str::<Value>(text).expect("a shipped form parses");
    (
        parse(include_str!(
            "../../../../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json"
        )),
        parse(include_str!(
            "../../../../../../content/forms/ew.titleblock.qet-a4.json"
        )),
    )
}

pub(super) fn external_ref_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(8) {
        // The command refuses: the node is not a location.
        0 => {
            let n = live.nodes.iter().find(|n| n.aspect != "location")?;
            Some(Step::cmd(
                "external_ref.create",
                json!({"location": n.id, "device": "X1"}),
            ))
        }
        // Refused: the node does not exist.
        1 => Some(
            Step::cmd(
                "external_ref.create",
                json!({"location": rng.uuid(), "device": "X1"}),
            )
            .refuse(),
        ),
        _ => {
            let location = rng.pick(&live.locations)?;
            let mut p = json!({"location": location, "device": format!("JE-{}", rng.word(6))});
            if rng.chance(2) {
                p["connection"] = json!(rng.word(2));
            }
            if rng.chance(3) {
                p["project"] = json!(format!("P{}", rng.below(100)));
            }
            Some(Step::cmd("external_ref.create", p).counts(&[(Table::ExternalRefs, 1)]))
        }
    }
}
