// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parts, the one place for parts (doc 07 §5): where a part lives (doc 07 §5.8), what the list finds and that its row
//! lacks something, and Keep in my parts, on a project with a user folder and parts made from nothing.

use crate::common;

use ew_api::parts::{Lives, PartScope};
use ew_api::{AnyCommand, Session, SessionOptions};
use ew_ids::Uuid;

fn run(s: &mut Session, cmd: &AnyCommand) {
    match s.execute(cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => {
            let p = s.preview(cmd).expect("a preview");
            s.commit(p).expect("committed");
        }
        other => {
            other.expect("committed");
        }
    }
}

/// A new project whose person has a user folder under `root`.
fn project(root: &std::path::Path) -> Session {
    Session::new_project(
        ew_api::ProjectTemplate::Builtin,
        ew_api::NewProjectKind::Schematic,
        SessionOptions {
            libraries: Some(common::tiers(root)),
            ..SessionOptions::default()
        },
    )
    .expect("a new project")
}

/// The copy the project holds of the part numbered `number`.
fn copy_of(s: &Session, number: &str) -> Uuid {
    s.query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(id, _)| id.as_uuid())
        .unwrap_or_else(|| panic!("the project holds {number}"))
}

/// A terminal made from nothing (no starter part copied) with its maker and number only, kept to the project
/// (`to_library` off) or saved to My parts.
fn terminal(s: &mut Session, number: &str, to_library: bool) {
    let mut sheet = s.new_part_draft().expect("a new part");
    sheet.to_library = to_library;
    for (f, v) in [
        ("manufacturer", "Example"),
        ("part_number", number),
        ("kind", "terminal"),
    ] {
        s.set_draft_field(&mut sheet, f, v).expect(f);
    }
    assert_eq!(s.sheet_lives(&sheet), Lives::New);
    let made = s.new_part_command(&sheet).expect("the new part's command");
    run(s, &made.command);
}

#[test]
fn a_part_lives_in_my_parts_in_the_project_alone_or_in_the_starter() {
    let tmp = ew_testdir::Tmp::new("parts-place-lives");
    let mut s = project(&tmp.0);
    terminal(&mut s, "FT-4", true);
    terminal(&mut s, "DL-2.5", false);
    let mine = s.part_draft(copy_of(&s, "FT-4")).expect("its sheet");
    assert_eq!(s.sheet_lives(&mine), Lives::Mine);
    let here = s.part_draft(copy_of(&s, "DL-2.5")).expect("its sheet");
    assert_eq!(s.sheet_lives(&here), Lives::OnlyHere);
    // The list finds both in the project, the one of My parts under My parts, and a starter part as not his.
    let numbers = |scope| -> Vec<String> {
        s.parts_find("", scope, Some("terminal"), false, None, 500)
            .into_iter()
            .map(|f| f.offer.part_number)
            .collect()
    };
    let project_parts = numbers(PartScope::Project);
    assert!(project_parts.contains(&"FT-4".to_owned()) && project_parts.contains(&"DL-2.5".to_owned()));
    let my_parts = numbers(PartScope::Mine);
    assert!(my_parts.contains(&"FT-4".to_owned()), "{my_parts:?}");
    assert!(!my_parts.contains(&"DL-2.5".to_owned()), "{my_parts:?}");
    let starter = s
        .parts_find("EW-GEN-TRM-1", PartScope::All, None, false, None, 50)
        .into_iter()
        .find(|f| f.offer.part_number == "EW-GEN-TRM-1")
        .expect("the starter terminal");
    assert!(
        matches!(starter.lives, Lives::NotMine { starter: true, .. }),
        "{:?}",
        starter.lives
    );
    // A terminal of a maker and a number alone lacks its size, its clamps and how it mounts: its row says so.
    let ft4 = s
        .parts_find("FT-4", PartScope::Mine, None, true, None, 50)
        .into_iter()
        .find(|f| f.offer.part_number == "FT-4")
        .expect("found among the parts that lack something");
    assert!(ft4.lacks);
}

/// Keep in my parts (doc 07 §5.8): a part only in this project is written into My parts and the project's devices are
/// given that copy; with none using it, the project takes the copy by `library.embed`, and it lives in My parts.
#[test]
fn a_part_only_in_this_project_is_kept_in_my_parts() {
    let tmp = ew_testdir::Tmp::new("parts-place-keep");
    let mut s = project(&tmp.0);
    terminal(&mut s, "DL-2.5", false);
    let here = copy_of(&s, "DL-2.5");
    let keep = s.keep_in_my_parts(here).expect("kept");
    assert_eq!(keep.name, "part.keep");
    run(&mut s, &keep);
    let kept: Vec<Uuid> = s
        .query()
        .parts()
        .iter()
        .filter(|(_, c)| c.item.part_number.as_str() == "DL-2.5")
        .map(|(id, _)| id.as_uuid())
        .collect();
    assert_eq!(kept.len(), 2, "the project's own copy stays beside the kept one");
    let lives: Vec<Lives> = kept
        .iter()
        .map(|c| s.sheet_lives(&s.part_draft(*c).expect("a sheet")))
        .collect();
    assert!(
        lives.contains(&Lives::Mine) && lives.contains(&Lives::OnlyHere),
        "{lives:?}"
    );
    // Kept again: My parts has it already, said in words.
    assert!(s.keep_in_my_parts(here).is_err());
}

/// Make my own from a starter part (and so Like this): the new part keeps where its data came from and its licence,
/// takes its own number as its key, and starts with the status of a new part; nothing else of the starter part's
/// facts (its confidence) is carried.
#[test]
fn a_part_made_from_another_keeps_its_sources_and_licence() {
    let tmp = ew_testdir::Tmp::new("parts-place-own-meta");
    let mut s = project(&tmp.0);
    let starter = s
        .library_parts()
        .into_iter()
        .find(|c| c.item.part_number.as_str() == "EW-GEN-TRM-1")
        .expect("the starter terminal");
    assert!(
        starter.item.meta.licence.is_some() && !starter.item.meta.sources.is_empty(),
        "the starter terminal says where it came from: {:?}",
        starter.item.meta
    );
    let sheet = s
        .library_part_draft(starter.library_id.as_uuid(), starter.item_id.as_uuid())
        .expect("its sheet");
    let mut own = s.new_part_draft_copying(&sheet).expect("a part of one's own");
    for (f, v) in [("manufacturer", "Example"), ("part_number", "MY-TRM")] {
        s.set_draft_field(&mut own, f, v).expect(f);
    }
    let made = s.new_part_command(&own).expect("the new part's command");
    run(&mut s, &made.command);
    let q = s.query();
    let (_, copy) = q
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == "MY-TRM")
        .expect("the project holds MY-TRM");
    let meta = &copy.item.meta;
    assert_eq!(meta.licence, starter.item.meta.licence);
    assert_eq!(meta.sources, starter.item.meta.sources);
    assert_eq!(meta.key.as_ref().map(|k| k.as_str()), Some("MY-TRM"));
    assert_eq!(meta.status, ew_schema::ItemStatus::default());
    assert!(meta.confidence.is_empty(), "{meta:?}");
}

/// Ready for says what a part lacks only for a feature that reads parts of its kind (doc 07 §5.7), asked of every part
/// of the starter content: the output load of a PLC module or station alone, the coil of a valve with solenoids alone,
/// no footprint or rail of an accessory (a plug, a bridge), and no pins of a piece that has none (an air service unit).
#[test]
fn a_starter_part_lacks_nothing_for_a_feature_that_does_not_read_its_kind() {
    use ew_api::part_fields::{Feature, lacks};
    let s = common::new_project(1);
    for c in s.library_parts() {
        let p = &c.item;
        let number = p.part_number.as_str();
        for l in lacks(p) {
            let fields: Vec<&str> = l.fields.iter().map(|f| f.path).collect();
            match l.feature {
                Feature::OutputLoad => assert!(p.sections.plc.is_some(), "{number}: {fields:?}"),
                Feature::CoilCheck => assert!(
                    p.sections.fluid.as_ref().is_some_and(|f| !f.solenoids.is_empty()),
                    "{number}: {fields:?}"
                ),
                Feature::Footprint | Feature::RailFit => {
                    assert_ne!(p.kind, ew_schema::PartKind::Accessory, "{number}: {fields:?}");
                }
                Feature::Placing => {
                    assert!(
                        !fields.contains(&"function_template[].cps[]"),
                        "{number}: {fields:?}"
                    );
                }
                _ => {}
            }
        }
    }
}

/// A writable session on a copy of the sample whose person has a user folder under `tmp`.
fn sample_with_my_parts(tmp: &ew_testdir::Tmp) -> Session {
    let dir = tmp.sample("sample.ew");
    let mut store = ew_api::StoreOptions::new("0.0.0-test");
    store.state_dir = Some(tmp.0.join("state"));
    let options = SessionOptions {
        libraries: Some(common::tiers(&tmp.0)),
        ..SessionOptions::default()
    };
    Session::open(&dir, store, options, ew_api::Recovery::Keep)
        .expect("the copy opens")
        .0
}

/// Keep in my parts moves every use of the project's copy to the kept one with what it chose, in one undo step: -102F1
/// uses a part only in this project in its variant "B"; kept, it uses the kept copy in the same variant; undone, the
/// project's copy again.
#[test]
fn a_kept_part_keeps_its_devices_and_their_variants_and_undo_brings_them_back() {
    let tmp = ew_testdir::Tmp::new("parts-place-keep-used");
    let mut s = sample_with_my_parts(&tmp);
    let mut sheet = s.new_part_draft().expect("a new part");
    sheet.to_library = false;
    for (f, v) in [
        ("manufacturer", "Example"),
        ("part_number", "MCB-HERE"),
        ("kind", "device"),
    ] {
        s.set_draft_field(&mut sheet, f, v).expect(f);
    }
    assert!(sheet.add_table_row("variants"));
    s.set_draft_field(&mut sheet, "variants[0].key", "B")
        .expect("its key");
    s.set_draft_field(&mut sheet, "variants[0].name", "B")
        .expect("its name");
    s.set_draft_field(&mut sheet, "variants[0].part_number", "MCB-HERE-B")
        .expect("its number");
    let made = s.new_part_command(&sheet).expect("the new part's command");
    run(&mut s, &made.command);
    let here = copy_of(&s, "MCB-HERE");
    let device = s.resolve_reference("tag:-102F1").expect("-102F1");
    let starter = {
        let q = s.query();
        q.devices()
            .get(ew_ids::Id::from_uuid(device))
            .and_then(|d| d.parts.first().map(|u| u.part.copy.as_uuid()))
            .expect("-102F1's part")
    };
    run(
        &mut s,
        &AnyCommand {
            name: "part.assign".into(),
            params: serde_json::json!({
                "devices": [device], "part": here, "mode": "replace", "from": starter, "variant": "B",
            }),
        },
    );
    let use_of = |s: &Session| {
        let q = s.query();
        let d = q
            .devices()
            .get(ew_ids::Id::from_uuid(device))
            .expect("the device")
            .clone();
        d.parts
            .iter()
            .map(|u| {
                (
                    u.part.copy.as_uuid(),
                    u.part.variant.as_ref().map(|v| v.as_str().to_owned()),
                )
            })
            .next()
            .expect("a part use")
    };
    assert_eq!(use_of(&s), (here, Some("B".to_owned())));
    let keep = s.keep_in_my_parts(here).expect("kept");
    assert_eq!(keep.name, "part.keep");
    run(&mut s, &keep);
    let kept = keep.params["to"]
        .as_str()
        .expect("the kept copy")
        .parse::<Uuid>()
        .expect("an ID");
    assert_ne!(kept, here);
    assert_eq!(
        use_of(&s),
        (kept, Some("B".to_owned())),
        "the device keeps its variant"
    );
    let lives = s.sheet_lives(&s.part_draft(kept).expect("its sheet"));
    assert_eq!(lives, Lives::Mine);
    s.undo(&ew_api::Origin::Gui).expect("undo");
    assert_eq!(use_of(&s), (here, Some("B".to_owned())), "one undo step");
}

/// A part of a library that is not installed (a colleague's parts, a library removed) is only in this project: his to
/// change and to keep (doc 07 §5.8). The sample's EW-GEN-MCB-1 is read as a copy of a library nobody has.
#[test]
fn a_part_of_a_library_that_is_not_installed_is_only_in_this_project() {
    let (project, records, mut lib) = common::sample_records();
    let gone = ew_ids::Uuid::from_u128(0x5eed_0000_0000_0000_0000_0000_0000_0001);
    let mut copy_id = None;
    for doc in &mut lib {
        if let ew_schema::LibDocument::Part(c) = doc
            && c.item.part_number.as_str() == "EW-GEN-MCB-1"
        {
            c.library_id = ew_ids::Id::from_uuid(gone);
            copy_id = Some(c.id.as_uuid());
        }
    }
    let s = common::session_of(project, records, lib);
    let sheet = s
        .part_draft(copy_id.expect("the sample holds EW-GEN-MCB-1"))
        .expect("its sheet");
    assert_eq!(s.sheet_lives(&sheet), Lives::OnlyHere);
    assert!(sheet.own, "his to change");
}

/// What a piece is reads as a person calls it, in both languages: the name of the starter symbol that draws its
/// function, without pins (a piece of the schematic table has its own pins, so never "Coil A1-A2" beside the pins 1
/// and 2), never the category's own entries ("A1A2 coil", "Standard check"; doc 07 §5.4).
#[test]
fn a_piece_kind_reads_without_its_functions_pins() {
    let say = |t: &ew_l10n::Text, lang| ew_api::catalogue().text_or_english(t, lang);
    let kinds = ew_api::part_pieces::piece_kinds();
    for (key, en, nl) in [
        ("ew.fn.coil", "Coil, rectangular", "Spoel, rechthoekig"),
        ("ew.fn.contact-co", "Contact, changeover", "Wisselcontact"),
        ("ew.fn.check-valve", "Check valve", "Terugslagklep"),
        (
            "ew.fn.valve-solenoid",
            "Solenoid of a valve, internal coil",
            "Magneetspoel van een ventiel, interne spoel",
        ),
    ] {
        let kind = kinds.iter().find(|k| k.key == key).expect("a piece");
        assert_eq!(say(&kind.words, "en-GB"), en);
        assert_eq!(say(&kind.words, "nl-NL"), nl);
    }
}

/// The sheet and its contents list the groups the start asked for or that hold a value (doc 07 §5.5): a contactor made
/// as a device lists no network ports and no PLC section, new or saved and opened again; a module lists both.
#[test]
fn a_device_lists_no_network_ports_or_plc_and_a_module_does() {
    use ew_api::part_fields::Group;
    let tmp = ew_testdir::Tmp::new("parts-place-groups");
    let mut s = project(&tmp.0);
    let start = |name| ew_api::parts::start_named(name).expect(name);
    let mut sheet = s.start_sheet(start("device")).expect("a device");
    for (f, v) in [("manufacturer", "Example"), ("part_number", "K-1")] {
        s.set_draft_field(&mut sheet, f, v).expect(f);
    }
    let none = |groups: Vec<Group>| !groups.contains(&Group::Ports) && !groups.contains(&Group::Plc);
    assert!(none(sheet.sections()), "{:?}", sheet.sections());
    let made = s.new_part_command(&sheet).expect("the new part's command");
    run(&mut s, &made.command);
    let again = s.part_draft(copy_of(&s, "K-1")).expect("its sheet");
    assert!(again.own, "his own");
    assert!(none(again.sections()), "{:?}", again.sections());
    let module = s.start_sheet(start("module")).expect("a module").sections();
    assert!(
        module.contains(&Group::Ports) && module.contains(&Group::Plc),
        "{module:?}"
    );
}

/// Writes `id` at the dotted `path` of `v` (a link's field of the link table), making each member on the way and one
/// entry of an empty list; `false` where a member on the way is no object (a variant spelt as a word).
fn write_at(v: &mut serde_json::Value, path: &[&str], id: &str) -> bool {
    use serde_json::{Value, json};
    let Some((head, rest)) = path.split_first() else {
        *v = Value::String(id.to_owned());
        return true;
    };
    let (name, each) = head.strip_suffix("[]").map_or((*head, false), |n| (n, true));
    let Some(object) = v.as_object_mut() else {
        return false;
    };
    let member = object.entry(name).or_insert(Value::Null);
    if member.is_null() {
        *member = if each { json!([]) } else { json!({}) };
    }
    if !each {
        return write_at(member, rest, id);
    }
    let Some(items) = member.as_array_mut() else {
        return false;
    };
    if items.is_empty() {
        items.push(json!({}));
    }
    write_at(&mut items[0], rest, id)
}

/// How many members of the dotted `path` `v` holds already: the record that is most like a referrer of the link.
fn depth_at(v: &serde_json::Value, path: &[&str]) -> usize {
    let Some((head, rest)) = path.split_first() else {
        return 0;
    };
    let name = head.strip_suffix("[]").unwrap_or(head);
    match v.get(name) {
        Some(serde_json::Value::Array(items)) => 1 + items.first().map_or(0, |i| depth_at(i, rest)),
        Some(m) if !m.is_null() => 1 + depth_at(m, rest),
        _ => 0,
    }
}

/// `v` read as a record, each member it lacks (serde names it and where) given the first of a few plain values that
/// reads: an entry made for a link has what its record needs beside the part.
fn as_record(mut v: serde_json::Value) -> Result<ew_schema::Record, String> {
    use serde_json::json;
    let read = |v: &serde_json::Value| {
        serde_json::from_value::<ew_schema::Record>(v.clone()).map_err(|e| e.to_string())
    };
    for _ in 0..32 {
        let e = match read(&v) {
            Ok(r) => return Ok(r),
            Err(e) => e,
        };
        let lacks = e
            .split_once("missing field `")
            .and_then(|(_, rest)| rest.split_once('`'))
            .map(|(field, rest)| {
                let at = rest
                    .split_once("at \"")
                    .and_then(|(_, r)| r.split_once('"'))
                    .map_or("", |(a, _)| a);
                (field.to_owned(), at.to_owned())
            });
        let Some((field, at)) = lacks else {
            return Err(e);
        };
        let given = [
            json!(0),
            json!("0"),
            json!(Uuid::nil()),
            json!("x"),
            json!({}),
            json!([]),
        ]
        .into_iter()
        .find_map(|value| {
            let mut t = v.clone();
            t.pointer_mut(&at)?.as_object_mut()?.insert(field.clone(), value);
            let still = read(&t).err().is_some_and(|m| {
                m.contains(&format!("{at}/{field}")) || (m.contains(&format!("`{field}`")) && m.contains(&at))
            });
            (!still).then_some(t)
        });
        match given {
            Some(t) => v = t,
            None => return Err(e),
        }
    }
    Err("too many members lacking".to_owned())
}

/// The link paths through which the project names part copy `copy`, as the model's typed readers see them.
#[allow(clippy::disallowed_methods)]
fn naming(s: &Session, copy: Uuid) -> std::collections::BTreeSet<String> {
    let key = (ew_schema::Kind::Part, ew_ids::Id::from_uuid(copy));
    s.world()
        .model
        .referrers(&key)
        .into_iter()
        .map(|(link, _)| link.path())
        .collect()
}

/// Keep in my parts and a fork move every use of a part copy (doc 07 §5.8): a project in which every part-copy link of
/// the link table names one copy (each written at the link's own path into the record most like its referrer, so a
/// path that does not match the record's form names nothing), kept, names the kept copy through every link and the old
/// one through none; forked, the fork likewise.
#[test]
fn keep_and_a_fork_move_every_use_the_link_table_holds() {
    let tmp = ew_testdir::Tmp::new("parts-place-every-link");
    let (mut project, mut records, mut lib) = common::sample_records();
    // EW-GEN-MCB-1 read as a copy of a library nobody has: only in this project, his to keep.
    let gone = Uuid::from_u128(0x5eed_0000_0000_0000_0000_0000_0000_0002);
    let mut old = None;
    for doc in &mut lib {
        if let ew_schema::LibDocument::Part(c) = doc
            && c.item.part_number.as_str() == "EW-GEN-MCB-1"
        {
            c.library_id = ew_ids::Id::from_uuid(gone);
            old = Some(c.id.as_uuid());
        }
    }
    let old = old.expect("the sample holds EW-GEN-MCB-1");
    let links: std::collections::BTreeSet<String> = ew_model::LINKS
        .iter()
        .filter(|l| l.to == ew_schema::Kind::Part && !l.lib_ref)
        .map(ew_model::LinkDecl::path)
        .collect();
    let mut missing = Vec::new();
    for link in ew_model::LINKS
        .iter()
        .filter(|l| l.to == ew_schema::Kind::Part && !l.lib_ref)
    {
        let path: Vec<&str> = link.field.split('.').collect();
        if link.from == ew_schema::Kind::Project {
            let mut v = serde_json::to_value(&project).expect("the project");
            assert!(write_at(&mut v, &path, &old.to_string()), "{}", link.path());
            project = serde_json::from_value(v).unwrap_or_else(|e| panic!("{}: {e}", link.path()));
            continue;
        }
        let mut candidates: Vec<(usize, usize)> = (records.iter().enumerate())
            .filter(|(_, r)| r.kind() == Some(link.from))
            .map(|(i, r)| (depth_at(&serde_json::to_value(r).expect("a record"), &path), i))
            .collect();
        candidates.sort_by_key(|(d, i)| (std::cmp::Reverse(*d), *i));
        let mut why = String::new();
        let written = candidates.into_iter().find_map(|(_, i)| {
            let mut v = serde_json::to_value(&records[i]).ok()?;
            write_at(&mut v, &path, &old.to_string()).then_some(())?;
            as_record(v).map(|r| (i, r)).map_err(|e| why = e).ok()
        });
        match written {
            Some((i, record)) => records[i] = record,
            // No record of the kind in the sample: a new one, its other members as little as reads.
            None if !records.iter().any(|r| r.kind() == Some(link.from)) => {
                let id = Uuid::from_u128(0x5eed_0000_0000_0000_0000_0000_0001_0000 + records.len() as u128);
                let mut v = serde_json::json!({"$k": link.from.name(), "id": id});
                assert!(write_at(&mut v, &path, &old.to_string()), "{}", link.path());
                match as_record(v) {
                    Ok(r) => records.push(r),
                    Err(e) => missing.push(format!("{}: {e}", link.path())),
                }
            }
            None => missing.push(format!("{}: {why}", link.path())),
        }
    }
    assert_eq!(
        missing,
        Vec::<String>::new(),
        "a record names a copy at every link"
    );
    let options = SessionOptions {
        libraries: Some(common::tiers(&tmp.0)),
        ..SessionOptions::default()
    };
    let mut s = Session::from_records(project, records, lib, options).expect("the records form a model");
    assert_eq!(naming(&s, old), links, "every link names the copy");
    // Every link names the new copy, but an accessory choice: the transaction's stale-choice rule drops a choice whose
    // accessory no part of its device names, and the choice made here names none (its `of` is made up).
    let moved = |new: &std::collections::BTreeSet<String>, how: &str| {
        let lost: Vec<&String> = links.difference(new).collect();
        assert!(
            lost.iter().all(|l| l.contains("accessory_choices")),
            "{how}: {lost:?} name it no more"
        );
    };
    let keep = s.keep_in_my_parts(old).expect("kept");
    run(&mut s, &keep);
    let kept: Uuid = keep.params["to"]
        .as_str()
        .and_then(|t| t.parse().ok())
        .expect("the kept copy");
    moved(&naming(&s, kept), "kept");
    assert!(naming(&s, old).is_empty(), "kept: no link names the old copy");
    s.undo(&ew_api::Origin::Gui).expect("undo");
    assert_eq!(naming(&s, old), links);
    let item = {
        let q = s.query();
        let c = q
            .parts()
            .get(ew_ids::Id::from_uuid(old))
            .expect("the copy")
            .clone();
        let mut v = serde_json::to_value(&c.item).expect("its item");
        v["description"] = serde_json::json!("Forked");
        v
    };
    let fork_item = Uuid::from_u128(0x5eed_0000_0000_0000_0000_0000_0000_0003);
    run(
        &mut s,
        &AnyCommand {
            name: "part.edit_copy".into(),
            params: serde_json::json!({"copy": old, "part": item, "fork": fork_item}),
        },
    );
    let project_id = s.query().project().id.as_uuid();
    let fork: ew_ids::Id<ew_schema::Part> = ew_ids::v5::lib_copy_id(project_id, fork_item);
    let fork = fork.as_uuid();
    moved(&naming(&s, fork), "forked");
    assert!(naming(&s, old).is_empty(), "forked: no link names the old copy");
}

/// Nothing of a part is lost by changing another piece of it (doc 07 §5.6): a part only in this project whose first
/// piece names a symbol only the project holds (its library is not installed) keeps that symbol when its second
/// piece's pins change.
#[test]
fn changing_one_piece_keeps_another_piece_s_symbol_the_project_alone_holds() {
    let (project, records, mut lib) = common::sample_records();
    let gone = ew_ids::Uuid::from_u128(0x5eed_0000_0000_0000_0000_0000_0000_0002);
    let symbol: ew_schema::LibRef = serde_json::from_value(serde_json::json!({
        "library": gone, "item": ew_ids::Uuid::from_u128(0x5eed_0000_0000_0000_0000_0000_0000_0003), "version_req": "*"
    }))
    .expect("a reference");
    let mut copy_id = None;
    for doc in &mut lib {
        if let ew_schema::LibDocument::Part(c) = doc
            && copy_id.is_none()
            && c.item.function_template.len() >= 2
            && c.item.function_template.iter().all(|t| t.fndef.extra.get("key").is_some())
        {
            c.library_id = ew_ids::Id::from_uuid(gone);
            c.item.function_template[0].symbol = Some(symbol.clone());
            copy_id = Some(c.id.as_uuid());
        }
    }
    let s = common::session_of(project, records, lib);
    let mut sheet = s.part_draft(copy_id.expect("a part of two pieces")).expect("its sheet");
    assert!(sheet.pieces.len() >= 2, "{:?}", sheet.pieces);
    let pins: Vec<String> = sheet.pieces[1].pins.split(", ").map(str::to_owned).collect();
    let changed = pins[..pins.len() - 1].join(", ");
    s.set_draft_field(&mut sheet, "pieces[1].pins", &changed).expect("its second piece's pins");
    let cmd = s.part_edit_command(&sheet).expect("the change");
    let part: ew_schema::Part = serde_json::from_value(cmd.params["part"].clone()).expect("the part");
    assert_eq!(part.function_template[1].cps.len(), pins.len() - 1, "its second piece changed");
    assert_eq!(part.function_template[0].symbol, Some(symbol), "the first piece keeps its symbol");
}
