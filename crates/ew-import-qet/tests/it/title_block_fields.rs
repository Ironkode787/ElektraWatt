// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! QET's own title-block variables become the project's and the pages' fields, and the title block prints
//! the fields, not the values as fixed texts (doc 14 §4.9): a page's `=` and `+` its structure, and the values every
//! folio shares the open revision's label, the designer and the drawing number. A value the folios do not share, and
//! a variable of the person's own, stay text. A small hand-written project with made-up content.

use ew_import_qet::QetProjectImporter;
use ew_import_qet::ew_interop_api::{ImportBatch, ImportInput};
use ew_import_qet::project::new_project_context;
use ew_schema::{LibDocument, Page, PartyRole, Record, Slot, StructureNode};

const TEMPLATE: &str = r#"<titleblocktemplates>
 <titleblocktemplate name="tb">
  <grid cols="t50%;t50%;" rows="10;10;10;">
   <field name="" col="0" row="0" displaylabel="false"><value><translation lang="en">%plant</translation></value></field>
   <field name="" col="1" row="0" displaylabel="false"><value><translation lang="en">%locmach</translation></value></field>
   <field name="" col="0" row="1" displaylabel="false"><value><translation lang="en">%indexrev</translation></value></field>
   <field name="" col="1" row="1" displaylabel="false"><value><translation lang="en">%author</translation></value></field>
   <field name="" col="0" row="2" displaylabel="false"><value><translation lang="en">%filename</translation></value></field>
   <field name="" col="1" row="2" displaylabel="false"><value><translation lang="en">%{customer}</translation></value></field>
  </grid>
 </titleblocktemplate>
</titleblocktemplates>"#;

fn diagram(folio: u32, locmach: &str, indexrev: &str) -> String {
    format!(
        r#"<diagram folio="{folio}" order="{folio}" title="F{folio}" plant="=P01" locmach="{locmach}" indexrev="{indexrev}" author="JD" filename="DWG-1" titleblocktemplate="tb" titleblocktemplateCollection="embedded"><properties><property name="customer">ACME</property></properties></diagram>"#
    )
}

fn import(revisions: [&str; 2]) -> ImportBatch {
    let xml = format!(
        "<project title=\"Fields\">{TEMPLATE}{}{}</project>",
        diagram(1, "+A1", revisions[0]),
        diagram(2, "+A2", revisions[1])
    );
    let input = ImportInput::new("x.qet", xml.into_bytes()).unwrap();
    QetProjectImporter::new()
        .import(&input, &mut new_project_context())
        .unwrap()
}

/// The cell values of the title block of each page, in page order.
fn cells(b: &ImportBatch) -> Vec<Vec<String>> {
    let form = |id| {
        b.lib.iter().find_map(|d| match d {
            LibDocument::Form(c) if c.id == id => Some(c.item.clone()),
            _ => None,
        })
    };
    let mut pages: Vec<&Page> = b
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Page(p) => Some(p),
            _ => None,
        })
        .collect();
    pages.sort_by_key(|p| p.folio.number);
    pages
        .iter()
        .map(|p| {
            let frame = form(p.form.expect("a frame")).expect("the frame");
            let tb = form(frame.title_block.expect("a title block")).expect("the title block");
            tb.cells.iter().filter_map(|c| c.value.clone()).collect()
        })
        .collect()
}

#[test]
fn the_title_block_prints_the_fields_the_import_writes() {
    let b = import(["B", "B"]);
    let fields = [
        " {page.function}",
        " {page.location}",
        " {revision.current.label}",
        " {revision.current.author.name}",
        " {project.drawing_id}",
        " ACME",
    ];
    let c = cells(&b);
    assert_eq!(c[0], fields);
    assert_eq!(c[0], c[1], "both pages print one title block");
    // The fields hold the values.
    let nodes: Vec<(String, String)> = b
        .records
        .iter()
        .filter_map(|r| match r {
            Record::StructureNode(n) => Some((
                serde_json::to_value(&n.aspect).unwrap().to_string(),
                n.code.to_string(),
            )),
            _ => None,
        })
        .collect();
    for want in [
        ("\"function\"", "P01"),
        ("\"location\"", "A1"),
        ("\"location\"", "A2"),
    ] {
        assert!(
            nodes.iter().any(|(a, c)| (a.as_str(), c.as_str()) == want),
            "{want:?} in {nodes:?}"
        );
    }
    let node = |r: &Record| match r {
        Record::StructureNode(n) => Some(n.clone()),
        _ => None,
    };
    let all: Vec<StructureNode> = b.records.iter().filter_map(node).collect();
    let page_one = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Page(p) if p.folio.number == 1 => Some(p.clone()),
            _ => None,
        })
        .unwrap();
    let Some(Slot::Node(f)) = page_one.structure.function else {
        panic!("the page's function")
    };
    assert_eq!(
        all.iter().find(|n| n.id == f).map(|n| n.code.to_string()),
        Some("P01".into())
    );
    let revision = b.records.iter().find_map(|r| match r {
        Record::Revision(r) => Some(r.clone()),
        _ => None,
    });
    assert_eq!(
        revision.map(|r| (r.label.to_string(), r.is_open())),
        Some(("B".into(), true))
    );
    let project = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Project(p) => Some(p.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        project.master.drawing_id.map(|d| d.to_string()).as_deref(),
        Some("DWG-1")
    );
    assert_eq!(
        project
            .parties
            .iter()
            .map(|p| (p.role.clone(), p.name.clone()))
            .collect::<Vec<_>>(),
        [(PartyRole::Author, ew_mltext::MlText::neutral("JD"))]
    );
}

#[test]
fn a_value_the_folios_do_not_share_stays_text() {
    let b = import(["A", "B"]);
    let c = cells(&b);
    assert_eq!(c[0][2], " A");
    assert_eq!(c[1][2], " B");
    assert!(!b.records.iter().any(|r| matches!(r, Record::Revision(_))));
    // The page's own fields print on each page all the same.
    assert_eq!(c[0][0], " {page.function}");
}
