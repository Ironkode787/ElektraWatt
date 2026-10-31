// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! QET project import spike, acceptance item 1 (doc 14 §4, doc 20 §2.3): a synthetic `.qet` built from the
//! public golden tables (folios 101 to 103, `tests/qet_synth`) imports to a model whose page order, device
//! tags, potentials and wire count are compared with the golden model's, for those folios, through the
//! derivation. The differences are listed in `tests/qet_import/expected_diff.txt` (blessed with
//! `EW_BLESS=1`, compared otherwise).
//!
//! Tests:
//!
//! - `fixture_is_deterministic`: two builds give the same text.
//! - `fixture_is_well_formed`: tags balance, every element names a definition with the right number of
//!   terminals, UUIDs are unique (references excepted), every conductor and link names existing UUIDs or
//!   integer terminal IDs of its folio, the element counts match the builder's.
//! - `synthetic_import_matches_golden_folios_101_to_103`: the import is `Ok`, its session opens, the drawn
//!   page order is `101 102 103`, no connection is lost, the wire count is the number of conductors drawn;
//!   then the metrics of both sides and the difference listing.

// Test target: bless and diagnostic environment flags (EW_*), which only choose whether a committed file is rewritten or a line is printed; the compared result never depends on them.
#![allow(clippy::disallowed_methods)]

use crate::golden_common;
use crate::qet_synth;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use ew_api::Session;
use ew_api::import::{StarterLibrary, import_qet};
use ew_derive::{DeviceTag, PageOrder, PageRef, PotentialPartition};
use ew_schema::ConnectionKind;
use golden_common::{batch, repo, session};
use qet_synth::scan::{Tag, norm_uuid, scan};
use qet_synth::{FOLIOS, Fixture, build};

// ---------------------------------------------------------------------------------------------------------
// The fixture

#[test]
fn fixture_is_deterministic() {
    let a = build();
    let b = build();
    assert_eq!(a.text, b.text);
    assert_eq!(a.folios, b.folios);
    assert_eq!(a.skipped, b.skipped);
    assert!(a.text.starts_with("<?xml"), "an XML declaration first");
}

fn ancestor(tags: &[Tag], mut i: usize, name: &str) -> Option<usize> {
    loop {
        i = tags[i].parent?;
        if tags[i].name == name {
            return Some(i);
        }
    }
}

#[test]
fn fixture_is_well_formed() {
    let fx = build();
    let tags = scan(&fx.text).expect("well-formed XML");
    assert_eq!(tags[0].name, "project");
    assert_eq!(tags[0].attr("version"), "0.100");

    // Definitions: name → terminal UUIDs in definition order.
    let mut def_terminals: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (i, t) in tags.iter().enumerate() {
        if t.name == "terminal" && !t.attr("uuid").is_empty() {
            let holder = ancestor(&tags, i, "element").expect("a definition holder");
            let uuid = norm_uuid(t.attr("uuid")).expect("a terminal UUID");
            def_terminals
                .entry(tags[holder].attr("name").to_owned())
                .or_default()
                .push(uuid);
        }
    }
    assert!(!def_terminals.is_empty());
    let mut def_names: BTreeSet<String> = BTreeSet::new();
    for t in &tags {
        if t.name == "element" && !t.attr("name").is_empty() {
            assert!(
                def_names.insert(t.attr("name").to_owned()),
                "definition {} twice",
                t.attr("name")
            );
        }
    }

    // UUIDs: the defining attributes are unique, every reference names one of them.
    let mut defined: Vec<String> = Vec::new();
    for t in &tags {
        let defines = matches!(
            t.name.as_str(),
            "element" | "dynamic_elmt_text" | "uuid" | "terminal"
        );
        if defines && !t.attr("uuid").is_empty() {
            defined.push(
                norm_uuid(t.attr("uuid")).unwrap_or_else(|| panic!("<{}> UUID `{}`", t.name, t.attr("uuid"))),
            );
        }
    }
    let unique: BTreeSet<&String> = defined.iter().collect();
    assert_eq!(unique.len(), defined.len(), "a UUID is defined twice");
    let upper = tags
        .iter()
        .filter(|t| t.name == "element" && !t.attr("type").is_empty())
        .filter(|t| t.attr("uuid").bytes().any(|b| b.is_ascii_uppercase()))
        .count();
    let placed = tags
        .iter()
        .filter(|t| t.name == "element" && !t.attr("type").is_empty())
        .count();
    assert!(
        upper > 0 && upper < placed,
        "element UUIDs are upper case on some elements only"
    );

    // Per diagram.
    let mut element_uuid: BTreeMap<String, usize> = BTreeMap::new(); // uuid → tag index
    for (i, t) in tags.iter().enumerate() {
        if t.name == "element" && !t.attr("type").is_empty() {
            element_uuid.insert(norm_uuid(t.attr("uuid")).unwrap(), i);
        }
    }
    let diagrams: Vec<usize> = (0..tags.len()).filter(|i| tags[*i].name == "diagram").collect();
    assert_eq!(diagrams.len(), FOLIOS.len());
    for (n, d) in diagrams.iter().enumerate() {
        let diag = &tags[*d];
        assert_eq!(diag.attr("folio"), FOLIOS[n]);
        assert_eq!(diag.attr("order"), (n + 1).to_string());
        let mut ids: BTreeSet<u32> = BTreeSet::new();
        let mut elements = 0;
        let mut conductors = 0;
        let mut integer = 0;
        for (i, t) in tags.iter().enumerate() {
            if ancestor(&tags, i, "diagram") != Some(*d) {
                continue;
            }
            match t.name.as_str() {
                "element" => {
                    elements += 1;
                    let name = t
                        .attr("type")
                        .strip_prefix("embed://import/synth/")
                        .unwrap_or_else(|| panic!("element type `{}`", t.attr("type")));
                    assert!(def_names.contains(name), "no definition {name}");
                    let x: i64 = t.attr("x").parse().unwrap();
                    let y: i64 = t.attr("y").parse().unwrap();
                    assert!(
                        x % 10 == 0 && y % 10 == 0,
                        "{name} at ({x}, {y}) is off the 10 px grid"
                    );
                    assert!(
                        (0..=1040).contains(&x) && (0..=660).contains(&y),
                        "{name} at ({x}, {y}) is off the frame"
                    );
                    let own: Vec<&Tag> = tags
                        .iter()
                        .enumerate()
                        .filter(|(j, c)| {
                            c.name == "terminal"
                                && ancestor(&tags, *j, "element") == Some(i)
                                && !c.attr("id").is_empty()
                        })
                        .map(|(_, c)| c)
                        .collect();
                    assert_eq!(own.len(), def_terminals[name].len(), "terminals of {name}");
                    for c in own {
                        assert!(
                            ids.insert(c.attr("id").parse().unwrap()),
                            "terminal id {} twice",
                            c.attr("id")
                        );
                    }
                }
                "link_uuid" => {
                    let target = norm_uuid(t.attr("uuid")).unwrap();
                    let from = ancestor(&tags, i, "element").unwrap();
                    let to = *element_uuid.get(&target).expect("a link names an element");
                    // Links are reciprocal.
                    let back = tags.iter().enumerate().any(|(j, l)| {
                        l.name == "link_uuid"
                            && ancestor(&tags, j, "element") == Some(to)
                            && norm_uuid(l.attr("uuid")) == norm_uuid(tags[from].attr("uuid"))
                    });
                    assert!(back, "link {target} is not reciprocal");
                }
                _ => {}
            }
        }
        for (i, t) in tags.iter().enumerate() {
            if t.name != "conductor" || ancestor(&tags, i, "diagram") != Some(*d) {
                continue;
            }
            if t.attr("element1").is_empty() {
                integer += 1;
                assert!(
                    t.attr("element2").is_empty(),
                    "both ends of a conductor use the same notation"
                );
                for e in ["terminal1", "terminal2"] {
                    let id: u32 = t
                        .attr(e)
                        .parse()
                        .unwrap_or_else(|_| panic!("integer end `{}`", t.attr(e)));
                    assert!(
                        ids.contains(&id),
                        "conductor names terminal id {id}, not on folio {}",
                        FOLIOS[n]
                    );
                }
            } else {
                for (e, term) in [("element1", "terminal1"), ("element2", "terminal2")] {
                    let uuid = norm_uuid(t.attr(e)).unwrap();
                    let el = *element_uuid
                        .get(&uuid)
                        .unwrap_or_else(|| panic!("conductor names unknown element {uuid}"));
                    assert_eq!(
                        ancestor(&tags, el, "diagram"),
                        Some(*d),
                        "element on another folio"
                    );
                    let def = tags[el]
                        .attr("type")
                        .strip_prefix("embed://import/synth/")
                        .unwrap();
                    let terminal = norm_uuid(t.attr(term)).unwrap();
                    assert!(
                        def_terminals[def].contains(&terminal),
                        "terminal {terminal} is not a terminal of {def}"
                    );
                }
            }
            conductors += 1;
        }
        let stats = &fx.folios[n];
        assert_eq!(
            (elements, conductors, integer),
            (stats.elements, stats.conductors, stats.integer_ends)
        );
        assert_eq!(integer, conductors / 3, "every third conductor has integer ends");
        assert_eq!(
            stats.elements,
            stats.devices + stats.terminals + stats.reports + stats.junctions
        );
    }
}

// ---------------------------------------------------------------------------------------------------------
// The metrics of a model, for the folios of the fixture

#[derive(Debug)]
struct Side {
    /// The folio labels of every drawn page, in page order.
    drawn: Vec<String>,
    /// The drawn folio labels among the fixture's folios.
    pages: Vec<String>,
    /// The printed tags of the devices represented on the folios (interruption points excluded).
    tags: BTreeSet<String>,
    /// The sizes of the potentials with a CP of a represented function, counted in such CPs, descending.
    potentials: Vec<usize>,
    /// Wire connections by drawn folio (the fixture's folios).
    wires: BTreeMap<String, usize>,
}

impl Side {
    fn wire_total(&self) -> usize {
        self.wires.values().sum()
    }
}

/// The metrics of `session` through the derivation. A function is in scope when a representation on one of
/// the folios draws it, or when it is a function of a terminal whose strip is drawn there.
fn metrics(session: &Session) -> Side {
    let q = session.query();
    let order = q.get::<PageOrder>(());
    let mut label_of: BTreeMap<PageRef, String> = BTreeMap::new();
    let mut drawn = Vec::new();
    for e in &order.entries {
        if matches!(e.page, PageRef::Drawn(_)) {
            label_of.insert(e.page, e.folio.clone());
            drawn.push(e.folio.clone());
        }
    }
    let on_folios = |page: ew_ids::Id<ew_schema::Page>| {
        label_of
            .get(&PageRef::Drawn(page))
            .is_some_and(|l| FOLIOS.contains(&l.as_str()))
    };
    let pages: Vec<String> = drawn
        .iter()
        .filter(|l| FOLIOS.contains(&l.as_str()))
        .cloned()
        .collect();

    let mut scope = BTreeSet::new();
    for (_, rep) in q.reps().iter() {
        if on_folios(rep.page) {
            scope.extend(rep.functions.iter().copied());
        }
    }
    let mut devices = BTreeSet::new();
    for f in &scope {
        if let Some(d) = q.functions().get(*f).and_then(|f| f.device) {
            devices.insert(d);
        }
    }
    let strips: BTreeSet<_> = devices
        .iter()
        .copied()
        .filter(|d| q.devices().get(*d).is_some_and(|d| d.aspects.strip.is_some()))
        .collect();
    let terminals: BTreeSet<_> = q
        .devices()
        .iter()
        .filter(|(_, d)| {
            d.aspects
                .terminal
                .as_ref()
                .is_some_and(|t| strips.contains(&t.strip))
        })
        .map(|(id, _)| id)
        .collect();
    for (id, f) in q.functions().iter() {
        if f.device.is_some_and(|d| terminals.contains(&d)) {
            scope.insert(id);
        }
    }
    devices.extend(terminals);

    let tags = devices
        .iter()
        .map(|d| {
            q.get::<DeviceTag>(*d)
                .as_ref()
                .as_ref()
                .map_or_else(|| "?".to_owned(), ew_derive::Tag::short)
        })
        .collect();

    let partition = q.get::<PotentialPartition>(());
    let mut potentials: Vec<usize> = partition
        .potentials()
        .map(|p| p.cps.iter().filter(|c| scope.contains(&c.function)).count())
        .filter(|n| *n > 0)
        .collect();
    potentials.sort_unstable_by(|a, b| b.cmp(a));

    let mut wires: BTreeMap<String, usize> = FOLIOS.iter().map(|f| ((*f).to_owned(), 0)).collect();
    for (_, c) in q.connections().iter() {
        if let (ConnectionKind::Wire, Some(page)) = (&c.kind, c.page)
            && let Some(l) = label_of
                .get(&PageRef::Drawn(page))
                .filter(|l| FOLIOS.contains(&l.as_str()))
        {
            *wires.get_mut(l).unwrap() += 1;
        }
    }
    Side {
        drawn,
        pages,
        tags,
        potentials,
        wires,
    }
}

// ---------------------------------------------------------------------------------------------------------
// The difference listing

fn sizes(v: &[usize]) -> String {
    let mut counts: BTreeMap<usize, usize> = BTreeMap::new();
    for n in v {
        *counts.entry(*n).or_default() += 1;
    }
    counts
        .iter()
        .rev()
        .map(|(size, n)| format!("{size}x{n}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn list(items: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let v: Vec<String> = items.into_iter().map(|s| s.as_ref().to_owned()).collect();
    if v.is_empty() { "-".to_owned() } else { v.join(" ") }
}

fn wires_text(s: &Side) -> String {
    let per: Vec<String> = s.wires.iter().map(|(f, n)| format!("{f}={n}")).collect();
    format!("{} (total {})", per.join(" "), s.wire_total())
}

/// The listing: both sides' metrics, whether they agree, and the differences known by construction.
fn listing(fx: &Fixture, golden: &Side, import: &Side) -> String {
    let mut t = String::new();
    let w = &mut t;
    let verdict = |equal: bool| if equal { "equal" } else { "DIFFERENT" };
    writeln!(
        w,
        "QET import of the synthetic folios {} against the golden tables",
        FOLIOS.join(" ")
    )
    .unwrap();
    writeln!(
        w,
        "Each metric is computed through the derivation on both sides, for these folios only."
    )
    .unwrap();

    writeln!(w, "\n[1] drawn page order").unwrap();
    writeln!(w, "golden: {}", list(&golden.pages)).unwrap();
    writeln!(w, "import: {}", list(&import.pages)).unwrap();
    writeln!(w, "{}", verdict(golden.pages == import.pages)).unwrap();

    writeln!(w, "\n[2] printed device tags (devices with a representation on the folios, interruption points excluded)").unwrap();
    writeln!(w, "golden: {} tags", golden.tags.len()).unwrap();
    writeln!(w, "import: {} tags", import.tags.len()).unwrap();
    let only_golden: Vec<&String> = golden.tags.difference(&import.tags).collect();
    let only_import: Vec<&String> = import.tags.difference(&golden.tags).collect();
    writeln!(w, "only in golden: {}", list(only_golden)).unwrap();
    writeln!(w, "only in import: {}", list(only_import)).unwrap();
    writeln!(w, "{}", verdict(golden.tags == import.tags)).unwrap();

    writeln!(
        w,
        "\n[3] potentials with a CP of a function represented on the folios"
    )
    .unwrap();
    writeln!(w, "sizes count those CPs, as <size>x<number of potentials>").unwrap();
    writeln!(
        w,
        "golden: {} potentials, sizes {}",
        golden.potentials.len(),
        sizes(&golden.potentials)
    )
    .unwrap();
    writeln!(
        w,
        "import: {} potentials, sizes {}",
        import.potentials.len(),
        sizes(&import.potentials)
    )
    .unwrap();
    writeln!(w, "{}", verdict(golden.potentials == import.potentials)).unwrap();

    writeln!(w, "\n[4] wire connections drawn on the folios").unwrap();
    writeln!(w, "golden: {}", wires_text(golden)).unwrap();
    writeln!(w, "import: {}", wires_text(import)).unwrap();
    writeln!(w, "{}", verdict(golden.wires == import.wires)).unwrap();

    writeln!(w, "\n[5] differences known by construction").unwrap();
    writeln!(w, "folios drawn by the fixture: {}", list(FOLIOS)).unwrap();
    for f in &fx.folios {
        writeln!(
            w,
            "  {}: {} elements ({} devices, {} terminals, {} reports, {} junctions), {} conductors ({} with integer ends)",
            f.folio, f.elements, f.devices, f.terminals, f.reports, f.junctions, f.conductors, f.integer_ends
        )
        .unwrap();
    }
    let kind = |k: &str| -> Vec<&str> {
        fx.skipped
            .iter()
            .filter(|s| s.kind == k)
            .map(|s| s.key.as_str())
            .collect()
    };
    let bridges = kind("bridge");
    let cores = kind("core");
    writeln!(
        w,
        "bridges not drawn (QET 0.100+ stores them in <terminal_strips>, which the importer does not read yet): {}: {}",
        bridges.len(),
        list(&bridges)
    )
    .unwrap();
    writeln!(
        w,
        "cable cores not drawn (kind core): {}: {}",
        cores.len(),
        list(&cores)
    )
    .unwrap();
    writeln!(
        w,
        "interruption points: {} pairs linked on the folios, {} without a partner on them",
        fx.ip_pairs, fx.ip_unpaired
    )
    .unwrap();
    writeln!(
        w,
        "terminal strips drawn terminal by terminal (one element per terminal): {}",
        list(&fx.strips)
    )
    .unwrap();
    writeln!(
        w,
        "not drawn: devices, cables and pages outside the folios, parts, potential definitions, bonds,"
    )
    .unwrap();
    writeln!(
        w,
        "  report blocks, wire colours and cross-sections; the golden joins potentials through"
    )
    .unwrap();
    writeln!(
        w,
        "  bridges and through interruption points that lead to folios outside the fixture."
    )
    .unwrap();
    t
}

// ---------------------------------------------------------------------------------------------------------
// Check or bless

fn expected_path() -> PathBuf {
    repo().join("crates/ew-api/tests/qet_import/expected_diff.txt")
}

fn blessing() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// The differing lines of two texts, `-` for expected and `+` for actual, at most 60 of them.
fn line_diff(expected: &str, actual: &str) -> String {
    let (e, a): (Vec<&str>, Vec<&str>) = (expected.lines().collect(), actual.lines().collect());
    let mut out = String::new();
    let mut shown = 0;
    for i in 0..e.len().max(a.len()) {
        let (x, y) = (e.get(i), a.get(i));
        if x != y {
            if shown == 60 {
                out.push_str("... more differing lines\n");
                break;
            }
            shown += 1;
            writeln!(out, "line {}:", i + 1).unwrap();
            if let Some(x) = x {
                writeln!(out, "  - {x}").unwrap();
            }
            if let Some(y) = y {
                writeln!(out, "  + {y}").unwrap();
            }
        }
    }
    out
}

/// Compares `text` with the committed listing, or writes it when blessing (`EW_BLESS=1`).
/// The licence header of the committed file (REUSE), followed by a blank line and the listing.
const SPDX_HEADER: &str = "# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

";

fn check_or_bless(listing: &str) {
    let text = format!("{SPDX_HEADER}{listing}");
    let text = text.as_str();
    let path = expected_path();
    if blessing() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("tests/qet_import/expected_diff.txt: {e} (run with EW_BLESS=1 to generate it)")
    });
    assert!(
        committed == text,
        "the difference listing differs from tests/qet_import/expected_diff.txt (EW_BLESS=1 rewrites it):\n{}\nactual listing:\n{text}",
        line_diff(&committed, text)
    );
}

// ---------------------------------------------------------------------------------------------------------
// The comparison

#[test]
fn synthetic_import_matches_golden_folios_101_to_103() {
    let fx = build();
    let library = StarterLibrary::from_dir(&repo().join("content")).expect("the starter content");
    let imported = import_qet("synthetic.qet", fx.text.clone().into_bytes(), &library)
        .expect("the import reads the file");
    let imported_session = imported
        .session
        .as_ref()
        .unwrap_or_else(|e| panic!("the model refuses the imported batch: {e}"));

    let golden_batch = batch();
    let golden_session = session(&golden_batch);
    let (golden, import) = (metrics(&golden_session), metrics(imported_session));

    // The hard assertions of the item.
    assert_eq!(import.drawn, FOLIOS, "the drawn page order of the import");
    let lost = usize::try_from(imported.batch.report.stats.lost_connections).unwrap();
    assert_eq!(lost, 0, "connections lost by the import");
    assert_eq!(
        import.wire_total(),
        fx.conductors() - lost,
        "wires imported against conductors drawn ({} per folio)",
        fx.folios
            .iter()
            .map(|f| f.conductors.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );

    check_or_bless(&listing(&fx, &golden, &import));
}
