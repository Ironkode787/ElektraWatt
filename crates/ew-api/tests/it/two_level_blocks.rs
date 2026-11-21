// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A two-level terminal block with a number on each level (doc 08 §6.3, FT-1878): a terminal joined into another
//! terminal of its strip is that one's second level, its symbol printing its number, its wires on the level's
//! connection points, a row of its own in the terminal diagram and no part of its own in the parts list; two terminals
//! of one number become one block of that number; a copy of the page carries the block.

use ew_api::output::{RowExport, page_view, row_export};
use ew_api::{BomMode, Session};
use ew_derive::{FunctionCps, PageOrder};
use ew_ids::{Id, Uuid};
use ew_schema::{Device, Endpoint, Record, Representation};
use serde_json::json;

use crate::common::{run_cmd, sample_records, session_of};

/// The sample's strip whose terminals 1 to 8 are drawn alone, each from the one-level part EW-GEN-TRM-1.
const STRIP: &str = "-X1021";

/// The sample as an editable session, its records first edited by `edit`.
fn sample_over(edit: impl FnOnce(&mut Vec<Record>)) -> Session {
    let (project, mut records, lib) = sample_records();
    edit(&mut records);
    session_of(project, records, lib)
}

/// The terminal `designation` of the strip.
fn terminal(s: &Session, designation: &str) -> Id<Device> {
    let q = s.query();
    let strip = Id::<Device>::from_uuid(s.resolve_reference(&format!("tag:{STRIP}")).expect("the strip"));
    let mut found: Vec<Id<Device>> = q
        .idx()
        .terminals_of_strip(strip)
        .filter(|t| {
            q.devices()
                .get(*t)
                .and_then(|d| d.aspects.terminal.as_ref())
                .is_some_and(|a| a.designation.as_str() == designation)
        })
        .collect();
    found.sort();
    *found.first().unwrap_or_else(|| panic!("terminal {designation} of {STRIP}"))
}

/// The one function of a device.
fn function_of(s: &Session, d: Id<Device>) -> Id<ew_schema::Function> {
    let q = s.query();
    let fs: Vec<_> = q.idx().functions_of_device(d).collect();
    let [f] = fs.as_slice() else {
        panic!("one function: {fs:?}")
    };
    *f
}

/// The representations of a device's function.
fn reps_of(s: &Session, d: Id<Device>) -> Vec<Id<Representation>> {
    let q = s.query();
    let mut reps: Vec<_> = q.idx().reps_of_function(function_of(s, d)).collect();
    reps.sort();
    reps
}

/// The CP indices of function `f` at which connections end, one per connection end.
fn ends_at(s: &Session, f: Id<ew_schema::Function>) -> Vec<u16> {
    let q = s.query();
    q.connections()
        .iter()
        .flat_map(|(_, c)| [c.a.clone(), c.b.clone()])
        .filter_map(|e| match e {
            Endpoint::Cp(g, i) if g == f => Some(i),
            _ => None,
        })
        .collect()
}

/// The texts the representation `rep` prints in its slots on its page.
fn printed(s: &Session, rep: Id<Representation>) -> Vec<String> {
    let q = s.query();
    for entry in &q.get::<PageOrder>(()).entries {
        if let Some(r) = page_view(&q, entry).reps.into_iter().find(|r| r.rep.id == rep) {
            return r.slots.iter().map(|s| s.text.clone()).collect();
        }
    }
    panic!("representation {rep} on no page")
}

/// The fields of one CSV line (a quoted field may hold commas).
fn fields(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// The data rows of an export as (column header, cell) lists.
fn rows(csv: &str) -> Vec<Vec<(String, String)>> {
    let mut lines = csv.lines();
    let header = fields(lines.next().expect("a header"));
    lines
        .map(fields)
        .filter(|f| f.first().is_some_and(|k| k == "data"))
        .map(|f| header.iter().cloned().zip(f).collect())
        .collect()
}

fn cell<'r>(row: &'r [(String, String)], column: &str) -> &'r str {
    row.iter()
        .find(|(c, _)| c == column)
        .map(|(_, v)| v.as_str())
        .unwrap_or_else(|| panic!("no column {column}: {row:?}"))
}

/// The strip's terminal-diagram rows as (terminal, side 1, side 2).
fn diagram(s: &Session) -> Vec<(String, String, String)> {
    let q = s.query();
    let out = row_export(&q, &RowExport::Terminals { strip: Some(STRIP.into()) }, None).expect("the terminal diagram");
    rows(&out.csv)
        .iter()
        .map(|r| {
            (
                cell(r, "Terminal").to_owned(),
                cell(r, "Side 1 (clamp a)").to_owned(),
                cell(r, "Side 2 (clamp b)").to_owned(),
            )
        })
        .collect()
}

/// The parts list's quantity of the part numbered `number`.
fn bom_qty(s: &Session, number: &str) -> String {
    let q = s.query();
    let out = row_export(&q, &RowExport::Bom(BomMode::Component), None).expect("the parts list");
    rows(&out.csv)
        .iter()
        .find(|r| cell(r, "Part number") == number)
        .map(|r| cell(r, "Qty").to_owned())
        .unwrap_or_else(|| panic!("{number} in the parts list: {}", out.csv))
}

/// What the terminal diagram lists for a terminal of the strip before a join: its row (`terminal` and its sides).
fn row_of(rows: &[(String, String, String)], terminal: &str) -> (String, String, String) {
    rows.iter()
        .find(|r| r.0 == terminal)
        .cloned()
        .unwrap_or_else(|| panic!("a row {terminal}: {rows:?}"))
}

/// Terminal 6 joined into terminal 5: one terminal with levels 1 and 2, level 2 numbered 6; 6's symbol draws the
/// second level and prints 6, its wires end there (a wire end reads `-X1021:6/…`), the terminal diagram keeps a row 6
/// with 6's sides beside row 5, the parts list counts one part fewer, and no designation is a duplicate.
#[test]
fn a_terminal_joined_into_another_is_its_second_level_with_its_number() {
    let mut s = sample_over(|_| {});
    let (five, six) = (terminal(&s, "5"), terminal(&s, "6"));
    let rep6 = reps_of(&s, six)[0];
    let before = diagram(&s);
    let (row5, row6) = (row_of(&before, "5"), row_of(&before, "6"));
    let wired = ends_at(&s, function_of(&s, six)).len();
    assert!(wired > 0, "terminal 6 of the sample is wired");
    let parts_before: u32 = bom_qty(&s, "EW-GEN-TRM-1").parse().expect("a count");

    run_cmd(
        &mut s,
        "rep.join_device",
        json!({"rep": rep6.to_string(), "device": five.to_string()}),
    );

    let q = s.query();
    assert!(q.devices().get(six).is_none(), "terminal 6 is gone into the block");
    let block = q.devices().get(five).and_then(|d| d.aspects.terminal.clone()).expect("the block");
    let levels: Vec<(u8, Option<&str>)> = block
        .levels
        .iter()
        .map(|l| (l.level, l.designation.as_ref().map(|d| d.as_str())))
        .collect();
    assert_eq!(levels, [(1, None), (2, Some("6"))]);
    // The block's function has the second level's CPs, and 6's symbol draws them.
    let f = function_of(&s, five);
    let cps = q.get::<FunctionCps>(f);
    let second: Vec<u16> = cps.iter().filter(|c| c.level == Some(2)).map(|c| c.idx).collect();
    assert!(!second.is_empty(), "{cps:?}");
    let rep = q.reps().get(rep6).expect("6's symbol stays");
    assert_eq!(rep.functions, [f]);
    assert!(rep.pin_map.iter().all(|p| p.function == f && second.contains(&p.idx)));
    assert_eq!(ew_derive::rep_level(&q, rep), Some(2));
    // Its wires now end at the second level.
    let ends: Vec<u16> = ends_at(&s, f).into_iter().filter(|i| second.contains(i)).collect();
    assert_eq!(ends.len(), wired);
    for i in &second {
        let label = ew_derive::cp_label(&q, ew_derive::CpRef::new(f, *i));
        assert!(label.starts_with(&format!("{STRIP}:6/")), "{label}");
    }
    // The symbols print their level's number.
    assert!(printed(&s, rep6).iter().any(|t| t == "6"), "{:?}", printed(&s, rep6));
    let rep5 = reps_of(&s, five).into_iter().find(|r| *r != rep6).expect("5's own symbol");
    assert!(printed(&s, rep5).iter().any(|t| t == "5"), "{:?}", printed(&s, rep5));
    assert!(!printed(&s, rep5).iter().any(|t| t == "6"));
    // The terminal diagram: rows 5 and 6 as before, now one block's levels.
    let after = diagram(&s);
    assert_eq!(row_of(&after, "5"), row5);
    assert_eq!(row_of(&after, "6"), row6);
    assert_eq!(after.len(), before.len());
    // The parts list: the block's one part.
    let parts_after: u32 = bom_qty(&s, "EW-GEN-TRM-1").parse().expect("a count");
    assert_eq!(parts_after + 1, parts_before);
    // No duplicate designation.
    let report = s.check(&ew_api::checks::PRE_DELIVERY, ew_api::checks::CheckMode::Online);
    assert!(
        report.findings.iter().all(|f| f.rule != "EW-TERM-002"),
        "{:?}",
        report.findings.iter().filter(|f| f.rule == "EW-TERM-002").collect::<Vec<_>>()
    );
    // One undo step takes it back.
    s.undo(&ew_api::Origin::Gui).expect("an undo step");
    assert!(s.query().devices().get(six).is_some());
    assert_eq!(diagram(&s), before);
}

/// Two terminals of one number, as an import of a two-level terminal leaves them: the second joined into the first is
/// the block's second level without a number of its own, so the duplicate is gone and both rows print the number.
#[test]
fn two_terminals_of_one_number_become_one_block_of_two_levels() {
    // Terminal 6 (the sample's only one) renumbered 5 in the records, as a lenient import leaves a duplicate.
    let mut s = sample_over(|records| {
        for r in records.iter_mut() {
            if let Record::Device(d) = r
                && let Some(t) = d.aspects.terminal.as_mut()
                && t.designation.as_str() == "6"
            {
                t.designation = "5".parse().expect("a designation");
            }
        }
    });
    let report = s.check(&ew_api::checks::PRE_DELIVERY, ew_api::checks::CheckMode::Online);
    let duplicates = |r: &ew_api::checks::CheckReport| r.findings.iter().filter(|f| f.rule == "EW-TERM-002").count();
    assert!(duplicates(&report) > 0, "the renumbered terminal is a duplicate");
    let q = s.query();
    let strip = Id::<Device>::from_uuid(s.resolve_reference(&format!("tag:{STRIP}")).expect("the strip"));
    let mut fives: Vec<Id<Device>> = q
        .idx()
        .terminals_of_strip(strip)
        .filter(|t| {
            q.devices()
                .get(*t)
                .and_then(|d| d.aspects.terminal.as_ref())
                .is_some_and(|a| a.designation.as_str() == "5")
        })
        .collect();
    fives.sort();
    let [first, second] = fives.as_slice() else {
        panic!("two terminals 5: {fives:?}")
    };
    let (first, second) = (*first, *second);
    let rep = reps_of(&s, second)[0];
    run_cmd(
        &mut s,
        "rep.join_device",
        json!({"rep": rep.to_string(), "device": first.to_string()}),
    );
    let q = s.query();
    let block = q.devices().get(first).and_then(|d| d.aspects.terminal.clone()).expect("the block");
    assert_eq!(block.levels.iter().map(|l| l.level).collect::<Vec<_>>(), [1, 2]);
    assert!(block.levels.iter().all(|l| l.designation.is_none()));
    assert!(printed(&s, rep).iter().any(|t| t == "5"));
    let fives = diagram(&s).into_iter().filter(|r| r.0 == "5").count();
    assert_eq!(fives, 2, "a row per level");
    let report = s.check(&ew_api::checks::PRE_DELIVERY, ew_api::checks::CheckMode::Online);
    assert_eq!(duplicates(&report), 0);
}

/// A level's number is a designation of the strip: a terminal renumbered to it is refused as a duplicate.
#[test]
fn a_level_number_is_taken_on_its_strip() {
    let mut s = sample_over(|_| {});
    let (five, six, seven) = (terminal(&s, "5"), terminal(&s, "6"), terminal(&s, "7"));
    let rep6 = reps_of(&s, six)[0];
    run_cmd(
        &mut s,
        "rep.join_device",
        json!({"rep": rep6.to_string(), "device": five.to_string()}),
    );
    let refused = s.execute(&ew_api::AnyCommand {
        name: "terminal.designate".into(),
        params: json!({"assignments": [{"terminal": seven.to_string(), "designation": "6"}]}),
    });
    let err = refused.expect_err("6 is the block's second level");
    assert!(err.to_string().contains("EW-TERM-002") || err.to_string().contains("taken"), "{err}");
}

/// A terminal of another strip is refused in words, at the device.
#[test]
fn a_terminal_of_another_strip_is_refused() {
    let mut s = sample_over(|_| {});
    let six = terminal(&s, "6");
    let other = Id::<Device>::from_uuid(s.resolve_reference("tag:-X1011:L1").expect("-X1011:L1"));
    let err = s
        .execute(&ew_api::AnyCommand {
            name: "rep.join_device".into(),
            params: json!({"rep": reps_of(&s, six)[0].to_string(), "device": other.to_string()}),
        })
        .expect_err("another strip");
    let text = err.to_string();
    assert!(text.contains("is no terminal of") && text.contains(STRIP), "{text}");
}

/// Copy and paste carry the block: the block's two symbols pasted beside them are a new terminal 5 (on a new strip, as
/// a pasted terminal brings its strip) with the levels of the original, its second symbol printing 6.
#[test]
fn a_paste_carries_the_block() {
    let mut s = sample_over(|_| {});
    let (five, six) = (terminal(&s, "5"), terminal(&s, "6"));
    let rep6 = reps_of(&s, six)[0];
    run_cmd(
        &mut s,
        "rep.join_device",
        json!({"rep": rep6.to_string(), "device": five.to_string()}),
    );
    let reps: Vec<Uuid> = reps_of(&s, five).into_iter().map(|r| r.as_uuid()).collect();
    let page = s.query().reps().get(rep6).expect("the symbol").page;
    let fragment = ew_api::window::parse_fragment(&s.copy_fragment(&reps).expect("copied").text)
        .expect("the copy reads")
        .params;
    // Ten connection-grid steps (QET 10 px) to the right of where it was copied.
    let step = ew_units::Grid::QET_10PX;
    let x = fragment["anchor"][0].as_i64().expect("an anchor");
    let y = fragment["anchor"][1].as_i64().expect("an anchor");
    let at = [
        step.at(step.snap(ew_units::Nm(x)) + 10).expect("on the page").0,
        step.at(step.snap(ew_units::Nm(y))).expect("on the page").0,
    ];
    let before: Vec<Uuid> = s.query().devices().iter().map(|(id, _)| id.as_uuid()).collect();
    run_cmd(
        &mut s,
        "edit.paste",
        json!({"page": page.to_string(), "at": at, "fragment": fragment}),
    );
    let q = s.query();
    let pasted: Vec<_> = q
        .devices()
        .iter()
        .filter(|(id, _)| !before.contains(&id.as_uuid()))
        .filter_map(|(id, d)| Some((id, d.aspects.terminal.clone()?)))
        .collect();
    let [(copy, t)] = pasted.as_slice() else {
        panic!("one pasted terminal: {pasted:?}")
    };
    assert_eq!(t.designation.as_str(), "5");
    let levels: Vec<(u8, Option<&str>)> = t
        .levels
        .iter()
        .map(|l| (l.level, l.designation.as_ref().map(|d| d.as_str())))
        .collect();
    assert_eq!(levels, [(1, None), (2, Some("6"))]);
    let prints: Vec<Vec<String>> = reps_of(&s, *copy).into_iter().map(|r| printed(&s, r)).collect();
    assert!(prints.iter().any(|p| p.iter().any(|t| t == "6")), "{prints:?}");
    assert!(prints.iter().any(|p| p.iter().any(|t| t == "5")), "{prints:?}");
}

/// The copy of the page that holds the block carries it: the copy's terminal 5 is a block of two levels, level 2
/// numbered 6, and the copy's symbol of the second level prints 6.
#[test]
fn a_copy_of_the_page_carries_the_block() {
    let mut s = sample_over(|_| {});
    let (five, six) = (terminal(&s, "5"), terminal(&s, "6"));
    let rep6 = reps_of(&s, six)[0];
    run_cmd(
        &mut s,
        "rep.join_device",
        json!({"rep": rep6.to_string(), "device": five.to_string()}),
    );
    let before: Vec<Uuid> = s.query().devices().iter().map(|(id, _)| id.as_uuid()).collect();
    // The supply, control and motor pages, their interruption points renamed so that the copy pairs within itself.
    let pages = ["101", "102", "103"]
        .iter()
        .map(|f| s.page_tables(f).expect("a drawn page").page)
        .collect();
    let rename = [("1L", "2L"), ("24V", "24V.2"), ("0V", "0V.2"), ("PE", "PE.2")]
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect();
    let plan = s
        .copy_pages(&ew_api::PageCopySpec {
            pages,
            folios: ew_api::CopyFolios::Offset(10),
            structure: Vec::new(),
            rename,
        })
        .expect("the copy of the pages");
    run_cmd(&mut s, &plan.command.name, plan.command.params.clone());
    let q = s.query();
    let blocks: Vec<_> = q
        .devices()
        .iter()
        .filter(|(id, _)| !before.contains(&id.as_uuid()))
        .filter_map(|(id, d)| Some((id, d.aspects.terminal.clone()?)))
        .filter(|(_, t)| t.designation.as_str() == "5")
        .collect();
    let [(copy, t)] = blocks.as_slice() else {
        panic!("one copied terminal 5: {blocks:?}")
    };
    let levels: Vec<(u8, Option<&str>)> = t
        .levels
        .iter()
        .map(|l| (l.level, l.designation.as_ref().map(|d| d.as_str())))
        .collect();
    assert_eq!(levels, [(1, None), (2, Some("6"))]);
    let reps = reps_of(&s, *copy);
    assert!(
        reps.iter().any(|r| printed(&s, *r).iter().any(|t| t == "6")),
        "the copy's second level prints 6"
    );
}
