// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lenient QET project import (doc 14 §4): small synthetic `.qet` projects, one behaviour each, through the
//! front door `ew_api::import::import_qet`. Repeated element UUIDs, folio-report pairs and chains, terminal
//! strips by label prefix, header and folio fallback, unlabelled elements, thumbnails, the project UUID as
//! lineage anchor and duplicate printed tags.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use ew_api::import::{Imported, StarterLibrary, import_qet};
use ew_schema::{DeviceKind, FunctionKind, IpDirection, LineageAnchor, Record};

// ---------------------------------------------------------------------------------------------------------
// A small `.qet` writer

/// An element definition: collection name, link type, terminal count, `kindInformation type`.
struct Def {
    name: &'static str,
    link_type: &'static str,
    terminals: usize,
    kind: Option<&'static str>,
}

const SIMPLE: usize = 0;
const NEXT: usize = 1;
const PREVIOUS: usize = 2;
const TERMINAL: usize = 3;
const THUMBNAIL: usize = 4;

const DEFS: [Def; 5] = [
    Def {
        name: "simple",
        link_type: "simple",
        terminals: 2,
        kind: None,
    },
    Def {
        name: "next",
        link_type: "next_report",
        terminals: 1,
        kind: None,
    },
    Def {
        name: "previous",
        link_type: "previous_report",
        terminals: 1,
        kind: None,
    },
    Def {
        name: "terminal",
        link_type: "terminal",
        terminals: 2,
        kind: Some("generic"),
    },
    Def {
        name: "thumb",
        link_type: "thumbnail",
        terminals: 1,
        kind: None,
    },
];

/// A UUID text from a number.
fn uuid(n: u64) -> String {
    format!("{{00000000-0000-4000-8000-{n:012x}}}")
}

/// The UUID of terminal `i` of definition `def`.
fn terminal_uuid(def: usize, i: usize) -> String {
    format!("{{00000000-0000-4000-9000-{def:04x}{i:08x}}}")
}

/// One placed element.
struct El {
    def: usize,
    uuid: String,
    x: i64,
    y: i64,
    orientation: u8,
    label: Option<&'static str>,
    prefix: &'static str,
    texts: Vec<&'static str>,
    links: Vec<String>,
}

fn el(def: usize, uuid: String, x: i64, y: i64) -> El {
    El {
        def,
        uuid,
        x,
        y,
        orientation: 0,
        label: None,
        prefix: "",
        texts: Vec::new(),
        links: Vec::new(),
    }
}

impl El {
    fn label(mut self, label: &'static str) -> El {
        self.label = Some(label);
        self
    }

    fn text(mut self, text: &'static str) -> El {
        self.texts.push(text);
        self
    }

    fn link(mut self, uuid: &str) -> El {
        self.links.push(uuid.to_owned());
        self
    }

    fn turned(mut self) -> El {
        self.orientation = 1;
        self
    }

    fn prefix(mut self, prefix: &'static str) -> El {
        self.prefix = prefix;
        self
    }
}

/// One end of a conductor: element UUID, definition index, terminal index.
type End<'a> = (&'a str, usize, usize);

struct Folio {
    folio: &'static str,
    title: &'static str,
    elems: Vec<El>,
    conductors: Vec<(String, usize, usize, String, usize, usize)>,
}

fn folio(folio: &'static str, title: &'static str, elems: Vec<El>) -> Folio {
    Folio {
        folio,
        title,
        elems,
        conductors: Vec::new(),
    }
}

impl Folio {
    fn wire(mut self, a: End<'_>, b: End<'_>) -> Folio {
        self.conductors
            .push((a.0.to_owned(), a.1, a.2, b.0.to_owned(), b.1, b.2));
        self
    }
}

fn write_folio(w: &mut String, order: usize, f: &Folio) {
    writeln!(
        w,
        r#"  <diagram order="{order}" folio="{}" title="{}" cols="17" colsize="60" rows="8" rowsize="80" height="660">"#,
        f.folio, f.title
    )
    .unwrap();
    writeln!(w, "    <elements>").unwrap();
    let mut next_id = 1;
    for e in &f.elems {
        writeln!(
            w,
            r#"      <element type="embed://import/{}.elmt" uuid="{}" x="{}" y="{}" z="10" orientation="{}" prefix="{}">"#,
            DEFS[e.def].name, e.uuid, e.x, e.y, e.orientation, e.prefix
        )
        .unwrap();
        writeln!(w, "        <terminals>").unwrap();
        for i in 0..DEFS[e.def].terminals {
            let (y, o) = if i == 0 { (-10, 'n') } else { (10, 's') };
            writeln!(
                w,
                r#"          <terminal x="0" y="{y}" orientation="{o}" id="{next_id}"/>"#
            )
            .unwrap();
            next_id += 1;
        }
        writeln!(w, "        </terminals>").unwrap();
        writeln!(w, "        <inputs/>").unwrap();
        match e.label {
            Some(l) => writeln!(
                w,
                r#"        <elementInformations><elementInformation name="label" show="1">{l}</elementInformation></elementInformations>"#
            )
            .unwrap(),
            None => writeln!(w, "        <elementInformations/>").unwrap(),
        }
        if e.texts.is_empty() {
            writeln!(w, "        <dynamic_texts/>").unwrap();
        } else {
            writeln!(w, "        <dynamic_texts>").unwrap();
            for (k, t) in e.texts.iter().enumerate() {
                writeln!(
                    w,
                    r#"          <dynamic_elmt_text text_from="UserText" uuid="{}" x="0" y="0" rotation="0" frame="false" text_width="-1"><text>{t}</text></dynamic_elmt_text>"#,
                    uuid(0xD000 + u64::try_from(k).unwrap())
                )
                .unwrap();
            }
            writeln!(w, "        </dynamic_texts>").unwrap();
        }
        writeln!(w, "        <texts_groups/>").unwrap();
        if e.links.is_empty() {
            writeln!(w, "        <links_uuids/>").unwrap();
        } else {
            writeln!(w, "        <links_uuids>").unwrap();
            for l in &e.links {
                writeln!(w, r#"          <link_uuid uuid="{l}"/>"#).unwrap();
            }
            writeln!(w, "        </links_uuids>").unwrap();
        }
        writeln!(w, "      </element>").unwrap();
    }
    writeln!(w, "    </elements>").unwrap();
    writeln!(w, "    <conductors>").unwrap();
    for (ea, da, ta, eb, db, tb) in &f.conductors {
        writeln!(
            w,
            r#"      <conductor element1="{ea}" terminal1="{}" element2="{eb}" terminal2="{}" num="" function="" tension_protocol="" conductor_color="" conductor_section="" type="multi" x="0" y="0"/>"#,
            terminal_uuid(*da, *ta),
            terminal_uuid(*db, *tb)
        )
        .unwrap();
    }
    writeln!(w, "    </conductors>").unwrap();
    writeln!(w, "  </diagram>").unwrap();
}

fn write_collection(w: &mut String) {
    writeln!(w, r#"  <collection><category name="import">"#).unwrap();
    for (n, d) in DEFS.iter().enumerate() {
        writeln!(w, r#"    <element name="{}.elmt">"#, d.name).unwrap();
        writeln!(
            w,
            r#"      <definition type="element" version="0.100.0" link_type="{}" width="20" height="{}" hotspot_x="10" hotspot_y="10">"#,
            d.link_type,
            10 + 10 * d.terminals
        )
        .unwrap();
        writeln!(
            w,
            r#"        <uuid uuid="{}"/>"#,
            uuid(0xE000 + u64::try_from(n).unwrap())
        )
        .unwrap();
        writeln!(w, r#"        <names><name lang="en">{}</name></names>"#, d.name).unwrap();
        if let Some(k) = d.kind {
            writeln!(
                w,
                r#"        <kindInformations><kindInformation name="type" show="1">{k}</kindInformation></kindInformations>"#
            )
            .unwrap();
        }
        writeln!(w, "        <informations/>").unwrap();
        writeln!(w, "        <description>").unwrap();
        for i in 0..d.terminals {
            let (y, o) = if i == 0 { (-10, 'n') } else { (10, 's') };
            writeln!(
                w,
                r#"          <terminal x="0" y="{y}" orientation="{o}" uuid="{}" type="Generic"/>"#,
                terminal_uuid(n, i)
            )
            .unwrap();
        }
        writeln!(w, "        </description>").unwrap();
        writeln!(w, "      </definition>").unwrap();
        writeln!(w, "    </element>").unwrap();
    }
    writeln!(w, "  </category></collection>").unwrap();
}

/// The `.qet` text of a project with the given root attributes and folios.
fn qet(attributes: &str, folios: &[Folio]) -> String {
    let mut w = String::new();
    writeln!(w, r#"<?xml version="1.0" encoding="UTF-8"?>"#).unwrap();
    writeln!(w, r#"<project {attributes} title="Lenient">"#).unwrap();
    writeln!(w, "  <properties/>").unwrap();
    writeln!(
        w,
        r#"  <newdiagrams><border cols="17" colsize="60" rows="8" rowsize="80" displaycols="true" displayrows="true"/><report label="%F-%l%c"/></newdiagrams>"#
    )
    .unwrap();
    for (i, f) in folios.iter().enumerate() {
        write_folio(&mut w, i + 1, f);
    }
    write_collection(&mut w);
    writeln!(w, "</project>").unwrap();
    w
}

fn import(text: &str) -> Imported {
    let lib = StarterLibrary::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content"))
        .expect("the starter content");
    let imported = import_qet("lenient.qet", text.as_bytes().to_vec(), &lib).expect("an importable project");
    assert!(
        imported.session.is_ok(),
        "the session opens: {:?}",
        imported.session.as_ref().err()
    );
    imported
}

fn entries<'a>(imported: &'a Imported, code: &str) -> Vec<&'a str> {
    imported
        .batch
        .report
        .entries
        .iter()
        .filter(|e| e.code.to_string() == code)
        .map(|e| e.message.as_str())
        .collect()
}

fn all_entries(imported: &Imported) -> String {
    imported
        .batch
        .report
        .entries
        .iter()
        .map(|e| format!("{} {:?}: {}", e.code, e.severity, e.message))
        .collect::<Vec<_>>()
        .join("\n")
}

fn devices(imported: &Imported) -> Vec<&ew_schema::Device> {
    imported
        .batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Device(d) => Some(d),
            _ => None,
        })
        .collect()
}

/// The interruption-point functions: name, direction, sort.
fn ips(imported: &Imported) -> Vec<(String, IpDirection, Option<i32>)> {
    imported
        .batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Function(f) if f.device.is_none() => match &f.kind {
                FunctionKind::Ip(spec) => {
                    Some((spec.name.as_str().to_owned(), spec.direction.clone(), spec.sort))
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn connections(imported: &Imported) -> usize {
    imported
        .batch
        .records
        .iter()
        .filter(|r| matches!(r, Record::Connection(_)))
        .count()
}

// ---------------------------------------------------------------------------------------------------------
// Tests

#[test]
fn a_uuid_repeated_on_two_folios_gets_distinct_records() {
    let shared = uuid(1);
    let (b1, b2) = (uuid(2), uuid(3));
    let text = qet(
        r#"version="0.100""#,
        &[
            folio(
                "101",
                "A",
                vec![
                    el(SIMPLE, shared.clone(), 100, 100).label("-1F1"),
                    el(SIMPLE, b1.clone(), 100, 200).label("-1F2"),
                ],
            )
            .wire((&shared, SIMPLE, 1), (&b1, SIMPLE, 0)),
            folio(
                "102",
                "B",
                vec![
                    el(SIMPLE, shared.clone(), 100, 100).label("-2F1"),
                    el(SIMPLE, b2.clone(), 100, 200).label("-2F2"),
                ],
            )
            .wire((&shared, SIMPLE, 1), (&b2, SIMPLE, 0)),
        ],
    );
    let imported = import(&text);
    assert_eq!(
        entries(&imported, "EW-IMP-120").len(),
        1,
        "{}",
        all_entries(&imported)
    );
    let devs = devices(&imported);
    assert_eq!(devs.len(), 4, "{}", all_entries(&imported));
    let ids: BTreeSet<_> = devs.iter().map(|d| d.id).collect();
    assert_eq!(ids.len(), 4, "four distinct devices");
    // The two copies of the shared UUID are the devices tagged F1 on each folio.
    let copies: Vec<_> = devs
        .iter()
        .filter(|d| {
            d.tag.counter == Some(1)
                || d.tag
                    .frozen
                    .as_ref()
                    .is_some_and(|f| f.text.as_str().ends_with("F1"))
        })
        .collect();
    assert_eq!(
        copies.len(),
        2,
        "{:?}",
        devs.iter().map(|d| &d.tag).collect::<Vec<_>>()
    );
    assert_ne!(copies[0].id, copies[1].id);
    assert_eq!(
        imported.batch.report.stats.lost_connections,
        0,
        "{}",
        all_entries(&imported)
    );
    assert_eq!(connections(&imported), 2);
}

#[test]
fn a_folio_report_pair_gets_one_name() {
    // Case 1: both ends carry a text; the source's wins and the differing names are reported.
    let (s, t) = (uuid(1), uuid(2));
    let text = qet(
        r#"version="0.100""#,
        &[
            folio("1", "A", vec![el(NEXT, s.clone(), 100, 100).text("L-1").link(&t)]),
            folio(
                "2",
                "B",
                vec![el(PREVIOUS, t.clone(), 100, 100).text("L-2").link(&s)],
            ),
        ],
    );
    let imported = import(&text);
    let found = ips(&imported);
    assert_eq!(found.len(), 2, "{}", all_entries(&imported));
    assert!(found.iter().all(|(n, _, _)| n == "L-1"), "{found:?}");
    let e115 = entries(&imported, "EW-IMP-115");
    assert_eq!(e115.len(), 1, "{}", all_entries(&imported));
    assert!(e115[0].contains("L-1") && e115[0].contains("L-2"), "{}", e115[0]);

    // Case 2: neither end has a text; both get the same synthesised name.
    let text = qet(
        r#"version="0.100""#,
        &[
            folio("1", "A", vec![el(NEXT, s.clone(), 100, 100).link(&t)]),
            folio("2", "B", vec![el(PREVIOUS, t.clone(), 100, 100).link(&s)]),
        ],
    );
    let imported = import(&text);
    let found = ips(&imported);
    assert_eq!(found.len(), 2, "{}", all_entries(&imported));
    assert_eq!(found[0].0, found[1].0, "{found:?}");
    assert!(found[0].0.starts_with("IP-"), "{found:?}");
}

#[test]
fn two_pairs_with_one_name_get_chain_order() {
    let (s1, t1, s2, t2) = (uuid(1), uuid(2), uuid(3), uuid(4));
    let text = qet(
        r#"version="0.100""#,
        &[
            folio(
                "1",
                "A",
                vec![
                    el(NEXT, s1.clone(), 100, 100).text("N").link(&t1),
                    el(NEXT, s2.clone(), 200, 100).text("N").link(&t2),
                ],
            ),
            folio(
                "2",
                "B",
                vec![
                    el(PREVIOUS, t1.clone(), 100, 100).text("N").link(&s1),
                    el(PREVIOUS, t2.clone(), 200, 100).text("N").link(&s2),
                ],
            ),
        ],
    );
    let imported = import(&text);
    let found = ips(&imported);
    assert_eq!(found.len(), 4, "{}", all_entries(&imported));
    let sorts = |d: IpDirection| -> Vec<Option<i32>> {
        let mut v: Vec<_> = found.iter().filter(|f| f.1 == d).map(|f| f.2).collect();
        v.sort();
        v
    };
    assert_eq!(sorts(IpDirection::Source), vec![Some(0), Some(2)], "{found:?}");
    assert_eq!(sorts(IpDirection::Target), vec![Some(1), Some(3)], "{found:?}");
    let e115 = entries(&imported, "EW-IMP-115");
    assert!(e115.iter().any(|m| m.contains("chain")), "{e115:?}");
}

#[test]
fn strips_group_by_label_prefix_header_and_folio_fallback() {
    let term = |n: u64, x: i64, y: i64| el(TERMINAL, uuid(n), x, y);
    let text = qet(
        r#"version="0.100""#,
        &[
            folio(
                "1",
                "A",
                vec![
                    term(1, 100, 100).label("X1:1"),
                    term(2, 160, 100).label("X1:2"),
                    term(3, 220, 100).label("+B-X1:1"),
                ],
            ),
            folio(
                "2",
                "B",
                vec![
                    term(4, 100, 100).turned().label("-X5"),
                    term(5, 100, 120).turned(),
                    term(6, 100, 140).turned(),
                    term(7, 300, 200),
                    term(8, 360, 300),
                ],
            ),
        ],
    );
    let imported = import(&text);
    let report = all_entries(&imported);
    let devs = devices(&imported);
    let strips: Vec<_> = devs.iter().filter(|d| d.aspects.strip.is_some()).collect();
    assert_eq!(strips.len(), 4, "{report}");

    // The terminals of each strip: (seq, designation) in sequence order.
    let mut by_strip: BTreeMap<_, Vec<(u32, String)>> = BTreeMap::new();
    for d in &devs {
        if let Some(t) = &d.aspects.terminal {
            assert_eq!(d.kind, Some(DeviceKind::Terminal));
            by_strip
                .entry(t.strip)
                .or_default()
                .push((t.seq, t.designation.as_str().to_owned()));
        }
    }
    for v in by_strip.values_mut() {
        v.sort();
    }
    let of = |s: &&&&ew_schema::Device| by_strip.get(&s.id).cloned().unwrap_or_default();
    let named = |structure: bool| {
        strips
            .iter()
            .filter(|s| s.extra.contains_key("qet.structure") == structure)
            .collect::<Vec<_>>()
    };
    // `+B-X1` is its own strip with the structure prefix kept.
    let b = named(true);
    assert_eq!(b.len(), 1, "{report}");
    assert_eq!(of(&b[0]), vec![(1, "1".to_owned())]);
    let plain = named(false);
    assert_eq!(plain.len(), 3, "{report}");
    let mut shapes: Vec<Vec<(u32, String)>> = plain.iter().map(of).collect();
    shapes.sort();
    // X1 has terminals 1 and 2; X5 has its header and two unlabelled elements, designations by sequence.
    let two_labelled = vec![(1, "1".to_owned()), (2, "2".to_owned())];
    let three = vec![(1, "1".to_owned()), (2, "2".to_owned()), (3, "3".to_owned())];
    assert!(shapes.contains(&two_labelled), "{shapes:?}\n{report}");
    assert!(shapes.contains(&three), "{shapes:?}\n{report}");
    // The fallback strip: two terminals, one EW-IMP-005 entry.
    assert_eq!(shapes.iter().filter(|s| s.len() == 2).count(), 2, "{shapes:?}");
    assert_eq!(entries(&imported, "EW-IMP-005").len(), 1, "{report}");
    let fallback = strips
        .iter()
        .find(|s| {
            s.tag
                .frozen
                .as_ref()
                .is_some_and(|f| f.text.as_str().starts_with("X?"))
        })
        .expect("the fallback strip");
    assert_eq!(by_strip.get(&fallback.id).map(Vec::len), Some(2));
}

#[test]
fn unlabelled_elements_are_reported_with_their_own_code() {
    let text = qet(
        r#"version="0.100""#,
        &[folio("1", "A", vec![el(SIMPLE, uuid(1), 100, 100).prefix("K")])],
    );
    let imported = import(&text);
    assert_eq!(
        entries(&imported, "EW-IMP-122").len(),
        1,
        "{}",
        all_entries(&imported)
    );
    assert!(
        entries(&imported, "EW-IMP-112").is_empty(),
        "{}",
        all_entries(&imported)
    );
    let devs = devices(&imported);
    assert_eq!(devs.len(), 1);
    assert_eq!(devs[0].tag.letter.as_str(), "K");
}

#[test]
fn a_thumbnail_is_not_a_device() {
    let text = qet(
        r#"version="0.100""#,
        &[folio("1", "A", vec![el(THUMBNAIL, uuid(1), 100, 100)])],
    );
    let imported = import(&text);
    assert!(devices(&imported).is_empty(), "{}", all_entries(&imported));
    let e004 = entries(&imported, "EW-IMP-004");
    assert!(
        e004.iter().any(|m| m.to_lowercase().contains("thumbnail")),
        "{}",
        all_entries(&imported)
    );
}

#[test]
fn a_project_uuid_anchors_the_lineage() {
    let attrs = r#"version="0.200" uuid="{6F1C9A0E-3B7D-4E2F-8A5C-1D0E9B8A7C6F}""#;
    let build = |title: &'static str| {
        let (a, b) = (uuid(1), uuid(2));
        qet(
            attrs,
            &[folio(
                "1",
                title,
                vec![
                    el(SIMPLE, a.clone(), 100, 100).label("-1F1"),
                    el(SIMPLE, b.clone(), 100, 200).label("-1F2"),
                ],
            )
            .wire((&a, SIMPLE, 1), (&b, SIMPLE, 0))],
        )
    };
    let first = import(&build("Title"));
    let lineage = first
        .batch
        .records
        .iter()
        .find_map(|r| match r {
            Record::ImportLineage(l) => Some(l),
            _ => None,
        })
        .expect("an import lineage");
    assert!(
        matches!(lineage.anchor, LineageAnchor::QetProjectUuid(_)),
        "{:?}",
        lineage.anchor
    );
    let ids = |i: &Imported| i.batch.records.iter().map(Record::id).collect::<Vec<_>>();
    let again = import(&build("Title"));
    assert_eq!(ids(&first), ids(&again), "the same bytes give the same IDs");
    let retitled = import(&build("Another title"));
    let device_ids = |i: &Imported| devices(i).iter().map(|d| d.id).collect::<BTreeSet<_>>();
    assert_eq!(device_ids(&first).len(), 2);
    assert_eq!(
        device_ids(&first),
        device_ids(&retitled),
        "a new title keeps the device IDs"
    );
}

#[test]
fn duplicate_printed_tags_are_lenient_findings() {
    let text = qet(
        r#"version="0.100""#,
        &[folio(
            "1",
            "A",
            vec![
                el(SIMPLE, uuid(1), 100, 100).label("S1"),
                el(SIMPLE, uuid(2), 100, 200).label("S1"),
            ],
        )],
    );
    let imported = import(&text);
    let group = imported
        .live
        .iter()
        .find(|g| g.rule == "EW-RDS-004")
        .unwrap_or_else(|| panic!("{:?}\n{}", imported.live, all_entries(&imported)));
    assert_eq!(group.count, 1, "{:?}", group.messages);
}
