// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The clipboard below the shell on Golden A (WP-S2.05 part 3, acceptance checks CB1 to CB3 and the session half of
//! CB4 to CB6): the fragment `Session::copy_fragment` writes (byte-equal, against a committed file, the closure rules),
//! `parse_fragment`'s refusals of hostile text, and `edit.paste` (every rejection at its pointer, the child device, the
//! library copies, the structure rebinding, the exact grid-step translation, one undo step). The LINKS oracle of CB3 is
//! the unit test `mandatory_and_optional_follow_the_oracle_for_every_link` of `ew-commands`.

use std::collections::BTreeSet;
use std::path::Path;

use ew_api::output::{CopyError, FragmentError, parse_fragment};
use ew_api::{
    AnyCommand, CmdError, NewProjectKind, Origin, ProjectTemplate, Receipt, Session, SessionOptions,
};
use ew_commands::testkit::seeded_ids;
use ew_ids::{Id, Uuid};
use ew_schema::{Device, Kind, TagScheme};
use ew_units::{Grid, Nm};
use serde_json::{Value, json};

/// Golden A's fixture, read-only (no lock; commands run in memory).
fn golden() -> Session {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
    Session::open_read_only(&dir, "ElektraWatt test")
        .expect("the golden fixture opens")
        .0
}

/// A new project as the wizard makes it (one empty multi-line page `1` on the QET grid).
fn wizard(seed: u64) -> Session {
    Session::new_project(
        ProjectTemplate::Builtin,
        NewProjectKind::Schematic,
        SessionOptions {
            ids: Some(seeded_ids(seed)),
            ..SessionOptions::default()
        },
    )
    .expect("a new project")
}

/// The representations on `folio` of the devices printed as `tags`, in page order.
fn reps_of(s: &Session, folio: &str, tags: &[&str]) -> Vec<Uuid> {
    let devices: Vec<Uuid> = tags
        .iter()
        .map(|t| s.resolve_reference(&format!("tag:{t}")).expect(t))
        .collect();
    let out: Vec<Uuid> = s
        .page_tables(folio)
        .expect("a drawn page")
        .reps
        .iter()
        .filter(|r| r.device.is_some_and(|d| devices.contains(&d)))
        .map(|r| r.id)
        .collect();
    assert!(!out.is_empty(), "{tags:?} on {folio}");
    out
}

/// The page ID of `folio`.
fn page_of(s: &Session, folio: &str) -> Uuid {
    s.page_tables(folio).expect("a drawn page").page
}

/// The fragment parameter of `ids` (copied, then read back like clipboard text).
fn fragment(s: &Session, ids: &[Uuid]) -> Value {
    parse_fragment(&s.copy_fragment(ids).expect("copied").text)
        .expect("the copy reads")
        .params
}

/// The QET grid of Golden A's and the wizard's pages.
const QET: Grid = Grid::QET_10PX;

/// The anchor of `fragment` moved by `steps` grid steps.
fn at_steps(fragment: &Value, steps: (i64, i64)) -> [i64; 2] {
    let step = |v: &Value, k: i64| {
        let x = Nm(v.as_i64().unwrap());
        QET.at(QET.snap(x) + k).unwrap().0
    };
    [
        step(&fragment["anchor"][0], steps.0),
        step(&fragment["anchor"][1], steps.1),
    ]
}

fn paste_cmd(page: Uuid, at: [i64; 2], fragment: &Value) -> AnyCommand {
    AnyCommand {
        name: "edit.paste".into(),
        params: json!({"page": page.to_string(), "at": at, "fragment": fragment}),
    }
}

/// `edit.paste` through preview and commit.
fn paste(s: &mut Session, folio: &str, at: [i64; 2], fragment: &Value) -> Result<Receipt, CmdError> {
    let cmd = paste_cmd(page_of(s, folio), at, fragment);
    let preview = s.preview(&cmd)?;
    s.commit(preview)
}

/// The pointer of a parameter refusal.
fn pointer(e: CmdError) -> String {
    match e {
        CmdError::InvalidParams { path, .. } => path,
        other => panic!("expected a parameter refusal, got {other}"),
    }
}

/// The created keys of `kind`.
fn created(r: &Receipt, kind: Kind) -> Vec<Uuid> {
    r.created
        .iter()
        .filter(|k| k.0 == kind)
        .map(|k| k.1.as_uuid())
        .collect()
}

fn device(s: &Session, id: Uuid) -> Device {
    (**s.query().devices().get(Id::from_uuid(id)).expect("the device")).clone()
}

/// Whether `EW_BLESS=1` asks the golden-text tests to rewrite their expected files (WP-S6.13).
// Allowed: the program boundary of a test, read once; the value never reaches a result.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

// ---------------------------------------------------------------------------------------------------------------------
// CB1: the fragment

/// CB1: the fragment of -102F1's representation on 102 is byte-equal on two runs and equals the committed file; it
/// holds the device, its function, the representation, the potential definition at its CP, the part, symbol and FnDef
/// copies and the two structure nodes, in that order, and no import lineage.
#[test]
fn cb1_the_fragment_of_102f1_is_byte_equal_and_matches_the_expected_file() {
    let s = golden();
    let reps = reps_of(&s, "102", &["-102F1"]);
    let a = s.copy_fragment(&reps).unwrap();
    let b = golden().copy_fragment(&reps).unwrap();
    assert_eq!(a.text, b.text, "two runs give equal bytes");
    assert_eq!((a.records, a.copies, a.dropped_connections), (4, 3, 0));
    assert!(a.skipped.is_empty());
    assert!(
        a.text
            .starts_with("#ew-fragment v1\n{\"$fragment\":{\"format\":\"0.1\",\"project\":")
    );
    assert!(a.text.ends_with('\n'));
    assert!(!a.text.contains("source_ref"), "a copy drops the import lineage");
    let kinds: Vec<String> = a
        .text
        .lines()
        .skip(2)
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            v.get("$k")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| v.as_object().and_then(|o| o.keys().next().cloned()))
                .unwrap()
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "device",
            "function",
            "representation",
            "potential_def",
            "$lib",
            "$lib",
            "$lib",
            "$node",
            "$node"
        ]
    );
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expected/fragment_102F1.txt");
    if bless() {
        std::fs::write(&path, &a.text).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("the expected fragment; bless with EW_BLESS=1");
    assert_eq!(a.text, expected.replace("\r\n", "\n"));
}

/// CB1: a terminal brings its strip device (with only the copied terminal), not the strip's other terminals: 1X1:1 on
/// folio 101.
#[test]
fn cb1_a_terminal_brings_its_strip() {
    let s = golden();
    let rep = s
        .page_tables("101")
        .unwrap()
        .reps
        .iter()
        .find(|r| {
            r.device.is_some_and(|d| {
                s.tag_list(&[d])
                    == "1X1:1
"
            })
        })
        .expect("1X1:1 on 101")
        .clone();
    let terminal = rep.device.unwrap();
    let frag = fragment(&s, &[rep.id]);
    let devices: Vec<&Value> = frag["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["$k"] == "device")
        .collect();
    let strip = device(&s, terminal).aspects.terminal.unwrap().strip;
    assert_eq!(devices.len(), 2, "the terminal and its strip");
    assert!(
        devices
            .iter()
            .any(|d| d["id"] == strip.to_string() && d.get("strip").is_some())
    );
    assert!(devices.iter().any(|d| d["id"] == terminal.to_string()));
}

/// CB1: copying -102G1 and -102F1 brings the junction where their wires meet and its three connections; a selected
/// wire with one end outside is dropped and counted.
#[test]
fn cb1_internal_connections_and_junctions_come_along_and_an_outside_wire_is_dropped() {
    let s = golden();
    let mut ids = reps_of(&s, "102", &["-102G1", "-102F1"]);
    let tables = s.page_tables("102").unwrap();
    let f1 = s.resolve_reference("tag:-102F1").unwrap();
    let f1_fn = tables
        .reps
        .iter()
        .find(|r| r.device == Some(f1))
        .unwrap()
        .functions[0];
    // -102F1's CP 1 runs to a junction whose other wires go to a device that is not copied.
    let outside = tables
        .connections
        .iter()
        .find(|c| matches!(c.a, ew_api::output::TableEnd::Cp { function, idx: 1 } if function == f1_fn))
        .expect("-102F1:2's wire")
        .id;
    ids.push(outside);
    let c = s.copy_fragment(&ids).unwrap();
    assert_eq!(c.dropped_connections, 1);
    let frag = parse_fragment(&c.text).unwrap().params;
    let count = |k: &str| {
        frag["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["$k"] == k)
            .count()
    };
    assert_eq!(count("junction"), 1, "the junction between them");
    assert_eq!(count("connection"), 3, "its three wires");
    let junction = frag["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["$k"] == "junction")
        .unwrap();
    assert_eq!(junction["sequence"].as_array().unwrap().len(), 3);
    assert!(!c.text.contains(&outside.to_string()));
}

/// CB1: what is not copied: nothing, a device alone, generated content, two pages.
#[test]
fn cb1_the_copy_refusals() {
    let s = golden();
    assert_eq!(s.copy_fragment(&[]), Err(CopyError::Empty));
    let dev = s.resolve_reference("tag:-102F1").unwrap();
    assert_eq!(s.copy_fragment(&[dev]), Err(CopyError::Empty));
    assert_eq!(s.copy_fragment(&[Uuid::from_u128(7)]), Err(CopyError::Generated));
    let mut two = reps_of(&s, "102", &["-102F1"]);
    two.extend(reps_of(&s, "101", &["-101F1"]));
    assert_eq!(s.copy_fragment(&two), Err(CopyError::NotOnePage));
}

/// FR-UI-040's tag list: -101F1, S51 and F55 in natural order, one per line.
#[test]
fn the_tag_list_is_natural_order_one_per_line() {
    let s = golden();
    let ids = reps_of(&s, "101", &["S51", "-101F1", "F55"]);
    assert_eq!(s.tag_list(&ids), "-101F1\nF55\nS51\n");
    assert_eq!(s.tag_list(&[]), "");
}

// ---------------------------------------------------------------------------------------------------------------------
// CB2: parsing

fn valid_text() -> String {
    let s = golden();
    s.copy_fragment(&reps_of(&s, "102", &["-102F1"])).unwrap().text
}

/// The text with line `n` (1-based) replaced.
fn with_line(text: &str, n: usize, line: &str) -> String {
    text.lines()
        .enumerate()
        .map(|(i, l)| if i + 1 == n { line.to_owned() } else { l.to_owned() })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// The text with `line` added after the header.
fn plus_line(text: &str, line: &str) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    lines.insert(2, line);
    lines.join("\n") + "\n"
}

fn header_with(text: &str, f: impl Fn(&mut Value)) -> String {
    let mut h: Value = serde_json::from_str(text.lines().nth(1).unwrap()).unwrap();
    f(&mut h["$fragment"]);
    with_line(text, 2, &h.to_string())
}

/// CB2: every refusal of §8.3 on hostile input.
#[test]
fn cb2_every_refusal_of_the_parser() {
    let text = valid_text();
    assert_eq!(parse_fragment(&text).unwrap().records, 4);
    let record = text.lines().nth(2).unwrap().to_owned();
    let invalid = |r: Result<_, FragmentError>| matches!(r, Err(FragmentError::Invalid { .. }));
    // Not a fragment, a later fragment version.
    assert_eq!(parse_fragment("hello"), Err(FragmentError::NotAFragment));
    assert_eq!(parse_fragment(""), Err(FragmentError::NotAFragment));
    assert_eq!(
        parse_fragment(&text.replacen("v1", "v2", 1)),
        Err(FragmentError::Newer { version: "v2".into() })
    );
    // Oversized: the whole text, one line, the records, the nesting.
    let big = format!("{text}{}", " ".repeat(4 << 20));
    assert!(matches!(parse_fragment(&big), Err(FragmentError::TooLarge(_))));
    let long = plus_line(
        &text,
        &format!("{{\"$k\":\"device\",\"x\":\"{}\"}}", "a".repeat(1 << 20)),
    );
    assert!(matches!(parse_fragment(&long), Err(FragmentError::TooLarge(_))));
    let deep = plus_line(&text, &format!("{}{}", "[".repeat(65), "]".repeat(65)));
    assert!(matches!(parse_fragment(&deep), Err(FragmentError::TooLarge(_))));
    // Repeated member names, unknown `$` lines, foreign kinds, malformed records, a line that is no JSON.
    let repeated = plus_line(&text, &record.replacen("\"tag\":", "\"tag\":{},\"tag\":", 1));
    assert!(invalid(parse_fragment(&repeated)));
    assert!(invalid(parse_fragment(&plus_line(&text, r#"{"$macro":{}}"#))));
    assert!(invalid(parse_fragment(&plus_line(
        &text,
        r#"{"$node":{"id":"0190a000-0000-7000-8000-000000000009","aspect":"location","path":["X"],"aliases":[],"extra":1}}"#
    ))));
    let foreign = r#"{"$k":"bond","id":"0190a000-0000-7000-8000-000000000001","a":{"junction":"0190a000-0000-7000-8000-000000000002"},"b":{"junction":"0190a000-0000-7000-8000-000000000003"}}"#;
    assert!(invalid(parse_fragment(&plus_line(&text, foreign))));
    assert!(invalid(parse_fragment(&plus_line(
        &text,
        r#"{"$k":"page_item","id":"0190a000-0000-7000-8000-000000000001"}"#
    ))));
    assert!(invalid(parse_fragment(&plus_line(
        &text,
        &record.replacen("\"letter\":\"F\"", "\"letter\":7", 1)
    ))));
    assert!(invalid(parse_fragment(&plus_line(&text, "{not json"))));
    assert!(invalid(parse_fragment(&plus_line(&text, r#"{"no":"kind"}"#))));
    // The same record twice.
    assert!(invalid(parse_fragment(&plus_line(&text, &record))));
    // A newer and an older format; a record of another `$v`.
    let newer = header_with(&text, |h| h["format"] = json!("0.2"));
    assert_eq!(
        parse_fragment(&newer),
        Err(FragmentError::Newer {
            version: "0.2".into()
        })
    );
    let older = header_with(&text, |h| h["format"] = json!("0.0"));
    assert!(invalid(parse_fragment(&older)));
    let v2 = with_line(
        &text,
        3,
        &record.replacen("{\"$k\":\"device\",", "{\"$k\":\"device\",\"$v\":2,", 1),
    );
    assert!(matches!(parse_fragment(&v2), Err(FragmentError::Newer { .. })));
    // A header without a known member, or with a malformed one.
    assert!(invalid(parse_fragment(&header_with(&text, |h| {
        h.as_object_mut().unwrap().remove("grid");
    }))));
    assert!(invalid(parse_fragment(
        &header_with(&text, |h| h["project"] = json!("nope"))
    )));
    // No header at all.
    assert!(invalid(parse_fragment("#ew-fragment v1\n")));
}

/// CB2: an unknown `$fragment` member is ignored (a later minor addition such as WP-S1.19's), CRLF line ends read, and a byte-order mark is skipped.
#[test]
fn cb2_an_unknown_header_member_is_ignored_and_crlf_reads() {
    let text = valid_text();
    let more = header_with(&text, |h| h["macro"] = json!({"name": "later"}));
    let a = parse_fragment(&more).unwrap();
    let b = parse_fragment(&text).unwrap();
    assert_eq!(a.params, b.params);
    assert_eq!(
        parse_fragment(&format!("\u{feff}{text}")).unwrap().params,
        b.params
    );
    assert_eq!(
        parse_fragment(&text.replace('\n', "\r\n")).unwrap().params,
        b.params
    );
    assert_eq!(b.grid, (7_937_500, 3));
    assert_eq!(b.ghost.len(), 1);
}

/// SplitMix64, the fuzzer's generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n.max(1) as u64).unwrap_or(0)
    }
}

/// CB2: random text and random mutations of a valid fragment never panic `parse_fragment`.
#[test]
fn cb2_random_text_never_panics_the_parser() {
    let text = valid_text();
    let mut rng = Rng(0x5EED);
    let alphabet: Vec<char> = "{}[]\":,$kv0123456789abcdef-\\ \n\r#ewfragment".chars().collect();
    for _ in 0..3000 {
        let mut chars: Vec<char> = text.chars().collect();
        for _ in 0..1 + rng.below(8) {
            let i = rng.below(chars.len());
            match rng.below(3) {
                0 => chars[i] = alphabet[rng.below(alphabet.len())],
                1 => {
                    chars.remove(i);
                }
                _ => chars.insert(i, alphabet[rng.below(alphabet.len())]),
            }
        }
        let mutated: String = chars.into_iter().collect();
        let _ = parse_fragment(&mutated);
        let random: String = (0..rng.below(200))
            .map(|_| alphabet[rng.below(alphabet.len())])
            .collect();
        let _ = parse_fragment(&format!("#ew-fragment v1\n{random}"));
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// CB3: edit.paste

/// CB3: every rejection of §8.4 at its pointer.
#[test]
fn cb3_every_rejection_at_its_pointer() {
    let mut s = golden();
    let frag = fragment(&s, &reps_of(&s, "102", &["-102F1"]));
    let at = at_steps(&frag, (2, 2));
    let page = page_of(&s, "102");
    let mut run = |page: Uuid, at: [i64; 2], f: &Value| s.preview(&paste_cmd(page, at, f)).map(|_| ());
    assert_eq!(pointer(run(Uuid::from_u128(9), at, &frag).unwrap_err()), "/page");
    let mut grid = frag.clone();
    grid["grid"] = json!([4_000_000, 1]);
    assert_eq!(pointer(run(page, at, &grid).unwrap_err()), "/fragment/grid");
    assert_eq!(pointer(run(page, [at[0] + 1, at[1]], &frag).unwrap_err()), "/at");
    let mut format = frag.clone();
    format["format"] = json!("0.2");
    assert_eq!(pointer(run(page, at, &format).unwrap_err()), "/fragment/format");
    // A record of a kind that is not pasted.
    let mut foreign = frag.clone();
    foreign["records"]
        .as_array_mut()
        .unwrap()
        .push(json!({"$k": "section", "id": Uuid::from_u128(3).to_string(),
        "number": 9, "title": "x"}));
    assert_eq!(
        pointer(run(page, at, &foreign).unwrap_err()),
        "/fragment/records/4"
    );
    // A mandatory reference that resolves to nothing: the representation's function is not in the fragment.
    let mut orphan = frag.clone();
    orphan["records"].as_array_mut().unwrap().remove(1);
    let p = pointer(run(page, at, &orphan).unwrap_err());
    assert_eq!(p, "/fragment/records/1", "the representation, now second");
    // Another version of a held copy; a copy library.embed refuses; the same copy twice.
    let mut other = frag.clone();
    other["copies"][0]["sha256"] = json!(format!("sha256:{}", "0".repeat(64)));
    assert_eq!(pointer(run(page, at, &other).unwrap_err()), "/fragment/copies/0");
    let mut twice = frag.clone();
    let first = twice["copies"][0].clone();
    twice["copies"].as_array_mut().unwrap().push(first);
    assert_eq!(pointer(run(page, at, &twice).unwrap_err()), "/fragment/copies/3");
    // More than 10,000 records.
    let mut many = frag.clone();
    many["records"] = json!(vec![Value::Null; 10_001]);
    assert_eq!(pointer(run(page, at, &many).unwrap_err()), "/fragment/records");
    // A page whose type takes no symbols: Golden A's title page.
    let title = s
        .query()
        .pages()
        .iter()
        .find(|(_, p)| p.kind == ew_schema::PageType::TitlePage)
        .map(|(id, _)| id.as_uuid())
        .expect("Golden A has a title page");
    assert_eq!(
        pointer(s.preview(&paste_cmd(title, at, &frag)).unwrap_err()),
        "/page"
    );
}

/// CB3: a child device whose parent is not copied loses its parent, position and child scheme, with a notice.
#[test]
fn cb3_a_child_whose_parent_is_not_copied() {
    let mut s = golden();
    let child = s.resolve_reference("tag:-201U4-Q2").unwrap();
    assert_eq!(device(&s, child).tag.scheme, TagScheme::Child);
    let (folio, rep) = s
        .page_labels()
        .into_iter()
        .find_map(|f| {
            let t = s.page_tables(&f)?;
            let r = t.reps.iter().find(|r| r.device == Some(child))?.id;
            Some((f, r))
        })
        .expect("-201U4-Q2 is drawn");
    let frag = fragment(&s, &[rep]);
    let receipt = paste(&mut s, &folio, at_steps(&frag, (0, 4)), &frag).unwrap();
    let new = created(&receipt, Kind::Device);
    assert_eq!(new.len(), 1);
    let d = device(&s, new[0]);
    assert_eq!((d.parent, d.position), (None, None));
    assert_eq!(d.tag.scheme, TagScheme::Default);
    assert!(d.tag.counter.is_some(), "numbered by the target's scheme");
    assert!(
        receipt
            .outcome
            .notices
            .iter()
            .any(|n| n == "-Q2 was a child of a device that was not copied"),
        "{:?}",
        receipt.outcome.notices
    );
}

/// CB3: a fragment whose records carry an import lineage (one written elsewhere) pastes; the pasted records have none.
#[test]
fn cb3_an_import_lineage_is_dropped_not_refused() {
    let mut s = golden();
    let mut frag = fragment(&s, &reps_of(&s, "102", &["-102F1"]));
    for r in frag["records"].as_array_mut().unwrap() {
        r["source_ref"] = json!({"lineage": Uuid::from_u128(77).to_string(), "key": "elsewhere"});
    }
    let receipt = paste(&mut s, "102", at_steps(&frag, (0, 6)), &frag).unwrap();
    let d = device(&s, created(&receipt, Kind::Device)[0]);
    assert_eq!(d.source_ref, None);
}

/// CB3 and CB4: the held library copies are reused in the same project: -102F1 pasted on 102 is -102F3 with part
/// A9F78116, no connection, and no copy is inserted.
#[test]
fn cb3_held_copies_are_reused_and_cb4_the_paste_of_102f1_is_102f3() {
    let mut s = golden();
    let frag = fragment(&s, &reps_of(&s, "102", &["-102F1"]));
    let parts = s.query().parts().len();
    let receipt = paste(&mut s, "102", at_steps(&frag, (0, 6)), &frag).unwrap();
    assert_eq!(s.query().parts().len(), parts, "the part copy is reused");
    assert!(created(&receipt, Kind::Part).is_empty());
    assert!(created(&receipt, Kind::Connection).is_empty());
    let new = created(&receipt, Kind::Device);
    assert_eq!(s.tag_list(&new), "-102F3\n");
    let d = device(&s, new[0]);
    let part = s.query().parts().get(d.parts[0].part.copy).unwrap().clone();
    assert_eq!(part.item.part_number.as_str(), "A9F78116");
    // Structure by path: the nodes are this project's own.
    assert_eq!(
        d.structure,
        device(&s, s.resolve_reference("tag:-102F1").unwrap()).structure
    );
}

/// CB3: in another project the copies are embedded in the paste's transaction, once, and one undo removes them with
/// the records; a copy the target holds in another version is refused.
#[test]
fn cb3_copies_are_embedded_once_and_undone_with_the_records() {
    let g = golden();
    let frag = fragment(&g, &reps_of(&g, "101", &["T55", "E55", "E56"]));
    let mut w = wizard(11);
    let before = (
        w.query().parts().len(),
        w.query().symbols().len(),
        w.query().devices().len(),
    );
    let receipt = paste(&mut w, "1", at_steps(&frag, (0, 0)), &frag).unwrap();
    let devices = created(&receipt, Kind::Device);
    assert_eq!(devices.len(), 3);
    let parts = created(&receipt, Kind::Part);
    let distinct: BTreeSet<Uuid> = parts.iter().copied().collect();
    assert_eq!(parts.len(), distinct.len(), "each copy embedded once");
    assert!(!parts.is_empty());
    for d in &devices {
        let dev = device(&w, *d);
        assert_eq!(dev.tag.frozen, None, "no frozen tag");
        assert!(dev.tag.counter.is_some(), "numbered by the project's scheme");
    }
    // A second paste reuses what the first embedded.
    let again = paste(&mut w, "1", at_steps(&frag, (0, 30)), &frag).unwrap();
    assert!(created(&again, Kind::Part).is_empty());
    w.undo(&Origin::Gui).unwrap();
    w.undo(&Origin::Gui).unwrap();
    let after = (
        w.query().parts().len(),
        w.query().symbols().len(),
        w.query().devices().len(),
    );
    assert_eq!(
        after, before,
        "one undo step each removes the records and the embedded copies"
    );
    // Another version of a copy the project holds.
    paste(&mut w, "1", at_steps(&frag, (0, 0)), &frag).unwrap();
    let mut other = frag.clone();
    let part = other["copies"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c["$k"] == "part")
        .unwrap();
    other["copies"][part]["sha256"] = json!(format!("sha256:{}", "0".repeat(64)));
    let err = w.execute(&paste_cmd(page_of(&w, "1"), at_steps(&frag, (0, 30)), &other));
    assert_eq!(pointer(err.unwrap_err()), format!("/fragment/copies/{part}"));
}

/// CB3: structure nodes rebind by aspect and path, else by an alias of the leaf code; an unmatched node clears the slot
/// with a notice; no node is created.
#[test]
fn cb3_structure_rebinds_by_path_and_alias_or_clears() {
    let g = golden();
    let frag = fragment(&g, &reps_of(&g, "102", &["-102F1"]));
    let mut w = wizard(12);
    let node = w
        .execute(&AnyCommand {
            name: "structure.create_node".into(),
            params: json!({"aspect": "location", "code": "CAB", "aliases": ["J-E-2416"]}),
        })
        .unwrap()
        .outcome
        .primary
        .unwrap()
        .1
        .as_uuid();
    let nodes = w.query().structure_nodes().len();
    let receipt = paste(&mut w, "1", at_steps(&frag, (0, 0)), &frag).unwrap();
    assert_eq!(w.query().structure_nodes().len(), nodes, "no node is created");
    let d = device(&w, created(&receipt, Kind::Device)[0]);
    assert_eq!(
        serde_json::to_value(&d.structure).unwrap(),
        json!({"location": node.to_string()}),
        "the location by alias; the function node is not in this project"
    );
    assert!(
        receipt
            .outcome
            .notices
            .contains(&"structure function P01 is not in this project: inherits the page".to_owned()),
        "{:?}",
        receipt.outcome.notices
    );
}

/// CB3: the translation in grid steps is exact on the QET grid (`7_937_500/3` nm): -101F1, S51 and T55 sit at y grid
/// indices 12, 14 and 22 (0, 2 and 1 mod 3); each pasted representation is at its index plus the steps on both axes.
#[test]
fn cb3_the_step_translation_is_exact_on_the_qet_grid() {
    let mut s = golden();
    let reps = reps_of(&s, "101", &["-101F1", "S51", "T55"]);
    let tables = s.page_tables("101").unwrap();
    let before: Vec<(i64, i64)> = reps
        .iter()
        .map(|r| tables.reps.iter().find(|t| t.id == *r).unwrap().pos)
        .collect();
    let mods: BTreeSet<i64> = before.iter().map(|p| QET.snap(Nm(p.1)).rem_euclid(3)).collect();
    assert_eq!(mods.len(), 3, "three residues mod 3");
    let frag = fragment(&s, &reps);
    let steps = (1, 7);
    let receipt = paste(&mut s, "101", at_steps(&frag, steps), &frag).unwrap();
    let after = s.page_tables("101").unwrap();
    let mut got: Vec<(i64, i64)> = created(&receipt, Kind::Representation)
        .iter()
        .map(|r| after.reps.iter().find(|t| t.id == *r).unwrap().pos)
        .collect();
    let mut want: Vec<(i64, i64)> = before
        .iter()
        .map(|p| {
            (
                QET.at(QET.snap(Nm(p.0)) + steps.0).unwrap().0,
                QET.at(QET.snap(Nm(p.1)) + steps.1).unwrap().0,
            )
        })
        .collect();
    got.sort_unstable();
    want.sort_unstable();
    assert_eq!(got, want);
}

/// Effect 6: the copies are numbered in the fragment's position order, whatever IDs the session mints: two sessions
/// with other ID seeds give the same tags at the same places, and of E55 and E56 the left one is numbered first.
#[test]
fn cb3_the_numbering_follows_the_fragment_order_not_the_minted_ids() {
    let g = golden();
    let frag = fragment(&g, &reps_of(&g, "101", &["T55", "E55", "E56"]));
    let placed = |seed: u64| {
        let mut w = wizard(seed);
        let receipt = paste(&mut w, "1", at_steps(&frag, (0, 0)), &frag).unwrap();
        let tables = w.page_tables("1").unwrap();
        let mut out: Vec<((i64, i64), String)> = created(&receipt, Kind::Representation)
            .iter()
            .map(|r| {
                let t = tables.reps.iter().find(|t| t.id == *r).unwrap();
                (t.pos, w.tag_list(&[t.device.unwrap()]))
            })
            .collect();
        out.sort();
        out
    };
    let a = placed(31);
    assert_eq!(a, placed(32));
    assert_eq!(a, placed(33));
    let e: Vec<&String> = a
        .iter()
        .filter(|(_, t)| t.contains('E'))
        .map(|(_, t)| t)
        .collect();
    assert_eq!(e.len(), 2);
    assert!(e[0].trim_end().ends_with("E1"), "{a:?}");
    assert!(e[1].trim_end().ends_with("E2"), "{a:?}");
}
