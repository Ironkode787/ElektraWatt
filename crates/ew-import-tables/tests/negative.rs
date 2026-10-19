// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Defective tables on small in-memory inputs: what is refused, what is dropped and what is kept with a finding.

mod common;

use ew_ids::Uuid;
use ew_import_tables::TablesImporter;
use ew_import_tables::ids;
use ew_interop_api::ReportEntry;
use ew_interop_api::{Code, ImportBatch, ImportError, LineageInfo, Severity};
use ew_schema::{Device, LibDocument, LibItem, Page, Record, Representation};

const PAGES: &str = "key,number,kind,form,grid\n101,101,multi_line,ew.frame.qet-a4-landscape-grid,qet10\n";
const DEVICES: &str = "key,letter,counter\n-1F1,F,1\n";
const FUNCTIONS: &str = "key,device,fndef\n-1F1,-1F1,ew.fn.mcb-1pn\n";
const REPS: &str = "key,functions,page,symbol,cell,dx,dy\n-1F1,-1F1,101,ew.iec.mcb-1pn@qet10,B2,,\n";

/// The base tables with the files of `replace` substituted by name and the rest of `extra` appended.
fn run_with(replace: &[(&str, &str)]) -> Result<ImportBatch, ImportError> {
    let mut files: Vec<(&str, &str)> = vec![
        ("project.csv", common::PROJECT_CSV),
        ("pages.csv", PAGES),
        ("devices.csv", DEVICES),
        ("functions.csv", FUNCTIONS),
        ("representations.csv", REPS),
    ];
    for (name, text) in replace {
        match files.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 = text,
            None => files.push((name, text)),
        }
    }
    common::import(&common::inputs(&files))
}

fn run(replace: &[(&str, &str)]) -> ImportBatch {
    run_with(replace).expect("the import succeeds")
}

fn count(b: &ImportBatch, code: u16, severity: Severity) -> usize {
    b.report
        .with_code(Code::imp(code))
        .filter(|e| e.severity == severity)
        .count()
}

fn devices(b: &ImportBatch) -> usize {
    b.records
        .iter()
        .filter(|r| matches!(r, Record::Device(_)))
        .count()
}

fn representations(b: &ImportBatch) -> Vec<&Representation> {
    b.records
        .iter()
        .filter_map(|r| match r {
            Record::Representation(x) => Some(x),
            _ => None,
        })
        .collect()
}

/// The base tables import without a single finding.
#[test]
fn the_base_tables_are_clean() {
    let b = run(&[]);
    assert!(b.report.entries.is_empty(), "{:#?}", b.report.entries);
    assert_eq!(devices(&b), 1);
    assert_eq!(representations(&b).len(), 1);
}

/// A table without a required column is refused, and rows that name its keys do not resolve.
#[test]
fn a_missing_required_column_refuses_the_table() {
    let b = run(&[("devices.csv", "key,counter\n-1F1,1\n")]);
    assert_eq!(
        b.report.with_code(Code::imp(2)).count(),
        1,
        "{:#?}",
        b.report.entries
    );
    let e = b.report.with_code(Code::imp(2)).next().unwrap();
    assert_eq!(e.severity, Severity::Error);
    assert_eq!(e.location.as_ref().unwrap().file, "devices.csv");
    assert_eq!(devices(&b), 0);
    assert!(
        count(&b, 3, Severity::Error) >= 1,
        "the function names a device that does not exist"
    );
}

/// A representation on an unknown page is dropped as an unresolved reference.
#[test]
fn a_representation_on_an_unknown_page_is_dropped() {
    let reps = "key,functions,page,symbol,cell\n-1F1,-1F1,999,ew.iec.mcb-1pn@qet10,B2\n";
    let b = run(&[("representations.csv", reps)]);
    assert_eq!(count(&b, 3, Severity::Error), 1, "{:#?}", b.report.entries);
    assert!(representations(&b).is_empty());
}

/// A repeated key in devices.csv drops the second row.
#[test]
fn a_duplicate_key_is_dropped() {
    let b = run(&[("devices.csv", "key,letter,counter\n-1F1,F,1\n-1F1,F,2\n")]);
    assert_eq!(count(&b, 2, Severity::Error), 1, "{:#?}", b.report.entries);
    assert_eq!(devices(&b), 1);
}

/// A position off the page grid drops the representation with one error; nothing is snapped.
#[test]
fn an_off_grid_position_drops_the_row() {
    let reps = "key,functions,page,symbol,cell,dx,dy\n-1F1,-1F1,101,ew.iec.mcb-1pn@qet10,B2,5,\n";
    let b = run(&[("representations.csv", reps)]);
    assert_eq!(count(&b, 2, Severity::Error), 1, "{:#?}", b.report.entries);
    assert_eq!(count(&b, 2, Severity::Warning), 0, "{:#?}", b.report.entries);
    let e = errors(&b, 2).remove(0);
    assert!(e.message.contains("off the page grid"), "{}", e.message);
    assert_eq!(e.location.as_ref().unwrap().file, "representations.csv");
    assert!(representations(&b).is_empty());
}

/// A junction off the page grid is dropped with the connections that end at it.
#[test]
fn an_off_grid_junction_is_dropped_with_its_connections() {
    let devices = "key,letter,counter\n-1F1,F,1\n-2F1,F,2\n-3F1,F,3\n";
    let functions =
        "key,device,fndef\n-1F1,-1F1,ew.fn.mcb-1pn\n-2F1,-2F1,ew.fn.mcb-1pn\n-3F1,-3F1,ew.fn.mcb-1pn\n";
    let connections = "key,page,a,b\nJ1.feed,101,-1F1:2,@J1\nW2,101,@J1,-2F1:1\nW3,101,@J1,-3F1:1\n";
    let junctions = "key,page,cell,dx,dy,style,sequence\nJ1,101,C11,5,,dot,J1.feed;W2;W3\n";
    let b = run(&[
        ("devices.csv", devices),
        ("functions.csv", functions),
        ("connections.csv", connections),
        ("junctions.csv", junctions),
    ]);
    assert_eq!(count(&b, 2, Severity::Warning), 0, "{:#?}", b.report.entries);
    let e2 = errors(&b, 2);
    assert_eq!(e2.len(), 1, "{:#?}", b.report.entries);
    assert!(e2[0].message.contains("off the page grid"), "{}", e2[0].message);
    assert_eq!(e2[0].location.as_ref().unwrap().file, "junctions.csv");
    assert_eq!(count(&b, 3, Severity::Error), 3, "{:#?}", b.report.entries);
    assert!(
        !b.records
            .iter()
            .any(|r| matches!(r, Record::Junction(_) | Record::Connection(_)))
    );
}

/// An arrangement the symbol does not define drops the representation with one error naming symbol and name.
#[test]
fn an_unknown_arrangement_drops_the_row() {
    let reps = "key,functions,page,symbol,cell,arrangement
-1F1,-1F1,101,ew.iec.mcb-1pn@qet10,B2,nonexistent
";
    let b = run(&[("representations.csv", reps)]);
    assert_eq!(count(&b, 2, Severity::Error), 1, "{:#?}", b.report.entries);
    expect_error(
        &b,
        2,
        "representations.csv",
        "-1F1",
        "symbol `ew.iec.mcb-1pn@qet10` has no arrangement `nonexistent`",
    );
    assert!(representations(&b).is_empty());
}

/// An arrangement cell that is not an identifier drops the representation with one error.
#[test]
fn an_invalid_arrangement_identifier_drops_the_row() {
    let reps = "key,functions,page,symbol,cell,arrangement
-1F1,-1F1,101,ew.iec.mcb-1pn@qet10,B2,not an ident!
";
    let b = run(&[("representations.csv", reps)]);
    assert_eq!(count(&b, 2, Severity::Error), 1, "{:#?}", b.report.entries);
    expect_error(&b, 2, "representations.csv", "-1F1", "arrangement");
    assert!(representations(&b).is_empty());
}

/// A bad cell reference drops the representation.
#[test]
fn a_bad_cell_drops_the_row() {
    let reps = "key,functions,page,symbol,cell\n-1F1,-1F1,101,ew.iec.mcb-1pn@qet10,Z9\n";
    let b = run(&[("representations.csv", reps)]);
    assert_eq!(count(&b, 2, Severity::Error), 1, "{:#?}", b.report.entries);
    assert!(representations(&b).is_empty());
}

/// An unknown enumeration value drops the page row; the valid page stays.
#[test]
fn a_bad_enum_value_drops_the_row() {
    let pages = "key,number,kind,form,grid\n\
        101,101,multi_line,ew.frame.qet-a4-landscape-grid,qet10\n\
        102,102,nonsense,ew.frame.qet-a4-landscape-grid,qet10\n";
    let b = run(&[("pages.csv", pages)]);
    assert_eq!(count(&b, 2, Severity::Error), 1, "{:#?}", b.report.entries);
    let n = b.records.iter().filter(|r| matches!(r, Record::Page(_))).count();
    assert_eq!(n, 1);
    let gone: ew_ids::Id<Page> = ids::record_id("pages", "102");
    assert!(
        !b.records
            .iter()
            .any(|r| matches!(r, Record::Page(p) if p.id == gone))
    );
}

/// An unknown table file is a warning, a non-CSV file an info; both are ignored.
#[test]
fn unknown_and_non_csv_files_are_reported_and_ignored() {
    let b = run(&[("mystery.csv", "key\nx\n"), ("notes.txt", "hello")]);
    let warn: Vec<_> = b
        .report
        .with_code(Code::imp(4))
        .filter(|e| e.severity == Severity::Warning)
        .collect();
    let info: Vec<_> = b
        .report
        .with_code(Code::imp(4))
        .filter(|e| e.severity == Severity::Info)
        .collect();
    assert_eq!(warn.len(), 1, "{:#?}", b.report.entries);
    assert_eq!(warn[0].location.as_ref().unwrap().file, "mystery.csv");
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].location.as_ref().unwrap().file, "notes.txt");
    assert_eq!(devices(&b), 1);
}

/// A stray quote refuses the table with an error, but the import still succeeds.
#[test]
fn a_malformed_csv_refuses_the_table_only() {
    let b = run(&[("devices.csv", "key,letter\n-1F1,F\"x\n")]);
    let e: Vec<_> = b.report.with_code(Code::imp(2)).collect();
    assert_eq!(e.len(), 1, "{:#?}", b.report.entries);
    assert_eq!(e[0].severity, Severity::Error);
    assert_eq!(e[0].location.as_ref().unwrap().file, "devices.csv");
    assert!(e[0].message.contains("table refused"), "{}", e[0].message);
    assert_eq!(devices(&b), 0);
}

/// Without project.csv the import is refused.
#[test]
fn a_missing_project_table_is_refused() {
    let inputs = common::inputs(&[("pages.csv", PAGES)]);
    let r = common::import(&inputs);
    assert!(matches!(r, Err(ImportError::Malformed { .. })), "{r:?}");
}

/// A foreign lineage is unsupported.
#[test]
fn a_foreign_lineage_is_unsupported() {
    let inputs = common::inputs(&[("project.csv", common::PROJECT_CSV)]);
    let mut ctx = common::context();
    ctx.lineage = Some(LineageInfo {
        id: ids::lineage_id(),
        namespace: Uuid::from_u128(0x1234_5678_9abc_def0),
    });
    let r = TablesImporter::new(common::library()).import(&inputs, &ctx);
    assert!(matches!(r, Err(ImportError::Unsupported { .. })), "{r:?}");
}

/// A terminal whose key is not `<strip>:<designation>` is dropped.
#[test]
fn a_terminal_key_must_name_strip_and_designation() {
    let devices_csv = "key,letter,counter,strip_label_mode\n-1F1,F,1,\n-1X1,X,1,designation\n";
    let terminals = "key,strip,seq,designation\nbogus,-1X1,1,1\n";
    let b = run(&[("devices.csv", devices_csv), ("terminals.csv", terminals)]);
    assert_eq!(count(&b, 2, Severity::Error), 1, "{:#?}", b.report.entries);
    let n = b
        .records
        .iter()
        .filter(|r| matches!(r, Record::Device(d) if d.aspects.terminal.is_some()))
        .count();
    assert_eq!(n, 0);
    let _: Option<Device> = None;
}

// ---- finding helpers --------------------------------------------------------------------------------------

/// The findings of `code` with severity error.
fn errors(b: &ImportBatch, code: u16) -> Vec<&ReportEntry> {
    b.report
        .with_code(Code::imp(code))
        .filter(|e| e.severity == Severity::Error)
        .collect()
}

/// Asserts exactly one error of `code` at `file`/`item` whose message contains `needle`.
fn expect_error(b: &ImportBatch, code: u16, file: &str, item: &str, needle: &str) {
    let hits: Vec<_> = errors(b, code)
        .into_iter()
        .filter(|e| {
            let l = e.location.as_ref().unwrap();
            l.file == file && l.item.as_deref() == Some(item)
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "{file}:{item} EW-IMP-00{code}: {:#?}",
        b.report.entries
    );
    assert!(hits[0].message.contains(needle), "{}", hits[0].message);
}

type PartCopy = ew_schema::LibCopy<ew_schema::Part>;

/// The project-local part copies of the batch.
fn part_copies(b: &ImportBatch) -> Vec<&PartCopy> {
    b.lib
        .iter()
        .filter_map(|d| match d {
            LibDocument::Part(c) => Some(c),
            _ => None,
        })
        .collect()
}

fn part<'a>(b: &'a ImportBatch, key: &str) -> Option<&'a PartCopy> {
    let id: ew_ids::Id<LibItem> = ids::record_id("parts", key);
    part_copies(b).into_iter().find(|c| c.item_id == id)
}

const PARTS: &str = "key,manufacturer,part_number,kind\nP1,ACME,N1,device\n";

fn with_parts(extra: &[(&str, &str)]) -> ImportBatch {
    let mut files = vec![("parts.csv", PARTS)];
    files.extend_from_slice(extra);
    run(&files)
}

// ---- parts.csv --------------------------------------------------------------------------------------------

#[test]
fn a_part_without_manufacturer_is_dropped() {
    let parts = "key,manufacturer,part_number\nP1,,N1\nP2,ACME,N2\n";
    let b = run(&[("parts.csv", parts)]);
    expect_error(&b, 2, "parts.csv", "P1", "manufacturer");
    assert_eq!(errors(&b, 2).len(), 1);
    assert!(part(&b, "P1").is_none());
    assert!(part(&b, "P2").is_some());
    assert_eq!(part_copies(&b).len(), 1);
}

#[test]
fn a_duplicate_part_key_is_dropped() {
    let parts = "key,manufacturer,part_number\nP1,ACME,N1\nP1,ACME,N9\n";
    let b = run(&[("parts.csv", parts)]);
    expect_error(&b, 2, "parts.csv", "P1", "duplicate key");
    let copies = part_copies(&b);
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0].item.part_number.as_str(), "N1", "the first row is kept");
}

#[test]
fn a_part_with_a_bad_kind_is_dropped() {
    let parts = "key,manufacturer,part_number,kind\nP1,ACME,N1,nonsense\nP2,ACME,N2,cable\n";
    let b = run(&[("parts.csv", parts)]);
    expect_error(&b, 2, "parts.csv", "P1", "kind");
    assert!(part(&b, "P1").is_none());
    assert!(part(&b, "P2").is_some());
}

// ---- part_cps.csv -----------------------------------------------------------------------------------------

#[test]
fn a_cp_of_an_unknown_part_is_dropped() {
    let cps = "key,part,idx,designation\nP1:0,P1,0,1\nX:0,NOPE,0,1\n";
    let b = with_parts(&[("part_cps.csv", cps)]);
    expect_error(&b, 3, "part_cps.csv", "X:0", "unknown part");
    assert_eq!(errors(&b, 3).len(), 1);
    assert_eq!(part(&b, "P1").unwrap().item.cp_pattern.len(), 1);
}

#[test]
fn a_repeated_pattern_index_drops_the_second_cp() {
    let cps = "key,part,idx,designation\nP1:0,P1,0,1\nP1:0b,P1,0,2\nP1:1,P1,1,3\n";
    let b = with_parts(&[("part_cps.csv", cps)]);
    expect_error(&b, 2, "part_cps.csv", "P1:0b", "repeated");
    assert_eq!(errors(&b, 2).len(), 1);
    assert_eq!(part(&b, "P1").unwrap().item.cp_pattern.len(), 2);
}

#[test]
fn a_cp_with_a_bad_clamp_is_dropped() {
    let cps = "key,part,idx,designation,clamp\nP1:0,P1,0,1,ZZ\nP1:1,P1,1,2,b\n";
    let b = with_parts(&[("part_cps.csv", cps)]);
    expect_error(&b, 2, "part_cps.csv", "P1:0", "clamp");
    assert_eq!(part(&b, "P1").unwrap().item.cp_pattern.len(), 1);
}

// ---- part_templates.csv -----------------------------------------------------------------------------------

const ONE_CP: &str = "key,part,idx,designation\nP1:0,P1,0,1\n";

#[test]
fn a_template_index_outside_the_pattern_is_dropped() {
    let tpl = "key,part,n,fndef,main,cps\nP1:t0,P1,0,ew.fn.mcb-1pn,true,0\nP1:t1,P1,1,ew.fn.mcb-1pn,,5\n";
    let b = with_parts(&[("part_cps.csv", ONE_CP), ("part_templates.csv", tpl)]);
    expect_error(&b, 2, "part_templates.csv", "P1:t1", "CP pattern");
    assert_eq!(part(&b, "P1").unwrap().item.function_template.len(), 1);
}

#[test]
fn a_template_with_an_unknown_fndef_is_dropped() {
    let tpl = "key,part,n,fndef,main,cps\nP1:t0,P1,0,ew.fn.nope,true,0\nP1:t1,P1,1,ew.fn.mcb-1pn,,0\n";
    let b = with_parts(&[("part_cps.csv", ONE_CP), ("part_templates.csv", tpl)]);
    expect_error(&b, 3, "part_templates.csv", "P1:t0", "unknown FnDef");
    assert_eq!(part(&b, "P1").unwrap().item.function_template.len(), 1);
}

// ---- cable_cores.csv --------------------------------------------------------------------------------------

#[test]
fn a_core_of_an_unknown_part_is_dropped() {
    let cores = "key,part,n,core\nP1:A,P1,0,A\nX:A,NOPE,0,A\n";
    let b = with_parts(&[("cable_cores.csv", cores)]);
    expect_error(&b, 3, "cable_cores.csv", "X:A", "unknown part");
    assert_eq!(part(&b, "P1").unwrap().item.sections.cable_cores.len(), 1);
}

#[test]
fn a_core_without_a_name_is_dropped() {
    let cores = "key,part,n,core\nP1:A,P1,0,\nP1:B,P1,1,B\n";
    let b = with_parts(&[("cable_cores.csv", cores)]);
    expect_error(&b, 2, "cable_cores.csv", "P1:A", "core");
    assert_eq!(part(&b, "P1").unwrap().item.sections.cable_cores.len(), 1);
}

// ---- potentials.csv ---------------------------------------------------------------------------------------

fn potentials(b: &ImportBatch) -> usize {
    b.records
        .iter()
        .filter(|r| matches!(r, Record::PotentialDef(_)))
        .count()
}

#[test]
fn a_potential_at_an_unknown_endpoint_is_dropped() {
    let csv = "key,at,ptype\n+24V,-1F1:2,l_plus\n+12V,-9X9:1,l_plus\n";
    let b = run(&[("potentials.csv", csv)]);
    expect_error(&b, 3, "potentials.csv", "+12V", "unknown device");
    assert_eq!(errors(&b, 3).len(), 1);
    assert_eq!(potentials(&b), 1);
}

#[test]
fn a_potential_with_a_bad_ptype_is_dropped() {
    let csv = "key,at,ptype\n+24V,-1F1:2,nonsense\n+12V,-1F1:1,l_plus\n";
    let b = run(&[("potentials.csv", csv)]);
    expect_error(&b, 2, "potentials.csv", "+24V", "ptype");
    assert_eq!(potentials(&b), 1);
}

#[test]
fn a_potential_key_must_be_an_identifier() {
    let csv = "key,at\nbad\u{7}key,-1F1:2\n+12V,-1F1:1\n";
    let b = run(&[("potentials.csv", csv)]);
    expect_error(&b, 2, "potentials.csv", "bad\u{7}key", "not an identifier");
    assert_eq!(potentials(&b), 1);
}

// ---- bonds.csv --------------------------------------------------------------------------------------------

fn bonds(b: &ImportBatch) -> usize {
    b.records.iter().filter(|r| matches!(r, Record::Bond(_))).count()
}

#[test]
fn a_bond_with_equal_endpoints_is_dropped() {
    let csv = "key,a,b,kind\nB1,-1F1:1,-1F1:1,fe_pe\nB2,-1F1:1,-1F1:2,fe_pe\n";
    let b = run(&[("bonds.csv", csv)]);
    expect_error(&b, 2, "bonds.csv", "B1", "same");
    assert_eq!(bonds(&b), 1);
}

#[test]
fn a_bond_with_an_unknown_via_is_dropped() {
    let csv = "key,a,b,kind,via\nB1,-1F1:1,-1F1:2,fe_pe,W99\nB2,-1F1:1,-1F1:2,fe_pe,\n";
    let b = run(&[("bonds.csv", csv)]);
    expect_error(&b, 3, "bonds.csv", "B1", "unknown connection");
    assert_eq!(bonds(&b), 1);
}

#[test]
fn a_bond_with_a_bad_kind_is_dropped() {
    let csv = "key,a,b,kind\nB1,-1F1:1,-1F1:2,nonsense\nB2,-1F1:1,-1F1:2,shield\n";
    let b = run(&[("bonds.csv", csv)]);
    expect_error(&b, 2, "bonds.csv", "B1", "kind");
    assert_eq!(bonds(&b), 1);
}

// ---- report_blocks.csv ------------------------------------------------------------------------------------

const FORM: &str = "ew.frame.qet-a4-landscape-nogrid";
const BLOCK_HEAD: &str = "key,kind,form,first_folio,include,embedded_page,embedded_rect\n";

fn blocks(b: &ImportBatch) -> usize {
    b.records
        .iter()
        .filter(|r| matches!(r, Record::ReportBlock(_)))
        .count()
}

fn block_row(key: &str, form: &str, first_folio: &str, include: &str, page: &str, rect: &str) -> String {
    format!("{key},toc,{form},{first_folio},{include},{page},{rect}\n")
}

#[test]
fn a_report_block_with_an_unknown_form_is_dropped() {
    let csv = format!(
        "{BLOCK_HEAD}{}{}",
        block_row("R1", "ew.frame.nope", "2", "", "", ""),
        block_row("R2", FORM, "3", "", "", "")
    );
    let b = run(&[("report_blocks.csv", &csv)]);
    expect_error(&b, 3, "report_blocks.csv", "R1", "unknown form");
    assert_eq!(blocks(&b), 1);
}

#[test]
fn a_report_block_including_an_unknown_device_is_dropped() {
    let csv = format!(
        "{BLOCK_HEAD}{}{}",
        block_row("R1", FORM, "2", "-1F1;-9Z9", "", ""),
        block_row("R2", FORM, "3", "-1F1", "", "")
    );
    let b = run(&[("report_blocks.csv", &csv)]);
    expect_error(&b, 3, "report_blocks.csv", "R1", "unknown device");
    assert_eq!(blocks(&b), 1);
}

#[test]
fn an_embedded_page_needs_an_embedded_rect() {
    let csv = format!(
        "{BLOCK_HEAD}{}{}",
        block_row("R1", FORM, "", "", "101", ""),
        block_row("R2", FORM, "3", "", "", "")
    );
    let b = run(&[("report_blocks.csv", &csv)]);
    expect_error(&b, 2, "report_blocks.csv", "R1", "go together");
    assert_eq!(blocks(&b), 1);
}

#[test]
fn a_page_oriented_block_needs_a_first_folio() {
    let csv = format!(
        "{BLOCK_HEAD}{}{}",
        block_row("R1", FORM, "", "", "", ""),
        block_row("R2", FORM, "3", "", "", "")
    );
    let b = run(&[("report_blocks.csv", &csv)]);
    expect_error(&b, 2, "report_blocks.csv", "R1", "first_folio");
    assert_eq!(blocks(&b), 1);
}

// ---- structure.csv ----------------------------------------------------------------------------------------

fn structure_nodes(b: &ImportBatch) -> Vec<ew_ids::Id<ew_schema::StructureNode>> {
    b.records
        .iter()
        .filter_map(|r| match r {
            Record::StructureNode(n) => Some(n.id),
            _ => None,
        })
        .collect()
}

fn node(key: &str) -> ew_ids::Id<ew_schema::StructureNode> {
    ids::record_id("structure", key)
}

#[test]
fn a_structure_node_with_an_unknown_parent_is_dropped() {
    let csv = "key,aspect,code,parent\n+A,location,A,\n+B,location,B,+NOPE\n+C,location,C,+A\n";
    let b = run(&[("structure.csv", csv)]);
    expect_error(&b, 3, "structure.csv", "+B", "unknown parent");
    assert_eq!(errors(&b, 3).len(), 1);
    let nodes = structure_nodes(&b);
    assert_eq!(nodes.len(), 2);
    assert!(nodes.contains(&node("+A")) && nodes.contains(&node("+C")));
}

/// Today a node naming itself reports EW-IMP-003 `unknown parent`; the cycle detection (F8) may reclassify it
/// as EW-IMP-002. The test pins only "dropped with exactly one error".
#[test]
fn a_structure_node_that_is_its_own_parent_is_dropped() {
    let csv = "key,aspect,code,parent\n+A,location,A,+A\n+B,location,B,\n";
    let b = run(&[("structure.csv", csv)]);
    let n = errors(&b, 2).len() + errors(&b, 3).len();
    assert_eq!(n, 1, "{:#?}", b.report.entries);
    assert_eq!(structure_nodes(&b), vec![node("+B")]);
}

/// F8: parent cycles are refused; a child of a cycle member is dropped with it.
#[test]
fn a_structure_parent_cycle_is_dropped_with_its_children() {
    let csv = "key,aspect,code,parent\n\
        +A,location,A,+B\n+B,location,B,+A\n+C,location,C,+A\n+D,location,D,\n";
    let b = run(&[("structure.csv", csv)]);
    expect_error(&b, 2, "structure.csv", "+A", "parent chain is a cycle");
    expect_error(&b, 2, "structure.csv", "+B", "parent chain is a cycle");
    expect_error(&b, 3, "structure.csv", "+C", "was dropped");
    assert_eq!(structure_nodes(&b), vec![node("+D")]);
}

/// F8: the same for `devices.csv` parents (EW-INV-009).
#[test]
fn a_device_parent_cycle_is_dropped_with_its_children() {
    let csv = "key,letter,counter,parent\n\
        -1F1,F,1,-1F2\n-1F2,F,2,-1F1\n-1F3,F,3,-1F1\n-1F4,F,4,\n";
    let b = run(&[("devices.csv", csv)]);
    expect_error(&b, 2, "devices.csv", "-1F1", "parent chain is a cycle");
    expect_error(&b, 2, "devices.csv", "-1F2", "parent chain is a cycle");
    expect_error(&b, 3, "devices.csv", "-1F3", "was dropped");
    assert_eq!(devices(&b), 1);
}
