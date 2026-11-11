// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A diagram's border, its title-block template and its tables through the public project importer (doc 14
//! §4.9, §4.13): small hand-written projects with made-up content.

use ew_import_qet::QetProjectImporter;
use ew_import_qet::ew_interop_api::{ImportBatch, ImportInput};
use ew_import_qet::project::new_project_context;
use ew_schema::{
    ContinuationPolicy, FormDef, FormKind, LibCopy, LibDocument, Page, Record, ReportBlock, ReportKind,
    ReportSource,
};
use ew_units::{Grid, Nm};

fn import(xml: &str) -> ImportBatch {
    let input = ImportInput::new("x.qet", xml.as_bytes().to_vec()).unwrap();
    QetProjectImporter::new()
        .import(&input, &mut new_project_context())
        .unwrap()
}

fn pages(b: &ImportBatch) -> Vec<&Page> {
    let mut v: Vec<&Page> = b
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Page(p) => Some(p),
            _ => None,
        })
        .collect();
    v.sort_by_key(|p| p.folio.number);
    v
}

fn blocks(b: &ImportBatch) -> Vec<&ReportBlock> {
    b.records
        .iter()
        .filter_map(|r| match r {
            Record::ReportBlock(r) => Some(r),
            _ => None,
        })
        .collect()
}

fn form(b: &ImportBatch, id: ew_ids::Id<FormDef>) -> &LibCopy<FormDef> {
    b.lib
        .iter()
        .find_map(|d| match d {
            LibDocument::Form(c) if c.id == id => Some(c),
            _ => None,
        })
        .expect("the form is in the batch's library")
}

fn px(n: u64) -> Grid {
    Grid::new(n * 793_750, 3).unwrap()
}

/// One diagram with `attrs` and `body`.
fn diagram(attrs: &str, body: &str) -> String {
    format!(
        r#"<diagram titleblocktemplate="tb" titleblocktemplateCollection="embedded" {attrs}>{body}</diagram>"#
    )
}

const TEMPLATE: &str = r#"<titleblocktemplates>
 <titleblocktemplate name="tb">
  <grid cols="20px;r100%;t25%;" rows="10;10;">
   <logo name="" col="0" row="0" rowspan="1" resource="logo.png"/>
   <field name="" col="1" row="0" colspan="1" fontsize="7" align="center" valign="center" hadjust="true" displaylabel="false">
    <value><translation lang="en">%title</translation></value>
   </field>
   <field name="" col="1" row="1" fontsize="6" align="left" displaylabel="true">
    <value><translation lang="de">X</translation><translation lang="EN">%author {%{customer}} %projectpath</translation></value>
    <label><translation lang="en">By</translation></label>
   </field>
   <field name="" col="2" row="1" displaylabel="true">
    <value><translation lang="en">%{folio-id}/%folio-total %unknown</translation></value>
    <label><translation lang="en">Sheet</translation></label>
   </field>
  </grid>
 </titleblocktemplate>
</titleblocktemplates>"#;

#[test]
fn the_border_becomes_a_frame_numbered_from_one_at_qets_margin() {
    let xml = format!(
        "<project>{}{}</project>",
        diagram(r#"folio="1" cols="12" colsize="50" rows="5" rowsize="70""#, ""),
        diagram(
            r#"folio="2" cols="12" colsize="50" rows="5" rowsize="70" displaycols="false" displayrows="false""#,
            ""
        ),
    );
    let b = import(&xml);
    let p = pages(&b);
    let shown = form(&b, p[0].form.expect("a frame"));
    let grid = shown.item.grid.as_ref().expect("a grid");
    assert_eq!(shown.item.kind, FormKind::Frame);
    assert_eq!((grid.columns.count, grid.columns.start_index), (12, 1));
    assert_eq!(
        (grid.columns.width, grid.rows.count, grid.rows.height),
        (px(50), 5, px(70))
    );
    // QET's 20 px header band starts at its 5 px margin.
    assert_eq!(grid.header_band, Some(px(25)));
    assert_eq!(grid.extra["ew.frame.margin"], px(1).at(5).unwrap().0);
    assert!(!grid.extra.contains_key("ew.frame.show_grid"));
    // A4 landscape; the border's corner on the sheet's corner.
    let t = shown.item.page_to_sheet.as_ref().expect("a sheet transform");
    assert_eq!(
        (shown.item.sheet.width, shown.item.sheet.height),
        (Nm(297_000_000), Nm(210_000_000))
    );
    let corner = i128::from(px(1).at(5).unwrap().0) * i128::from(t.scale_ppb) / 1_000_000_000;
    assert!((i128::from(t.offset.x.0) + corner).abs() <= 1, "{:?}", t.offset);
    // The drawing (620 px wide with its header) fills the sheet's width.
    let drawn = i128::from(px(1).at(621).unwrap().0) * i128::from(t.scale_ppb) / 1_000_000_000;
    assert!((297_000_000 - drawn).abs() < 1_000, "{drawn}");

    let hidden = form(&b, p[1].form.expect("a frame"));
    let grid = hidden.item.grid.as_ref().expect("a grid");
    assert_eq!(grid.extra["ew.frame.show_grid"], false);
}

#[test]
fn the_template_becomes_a_title_block_with_the_folios_values() {
    let props = r#"<properties><property name="customer">ACME</property></properties>"#;
    let xml = format!(
        "<project>{TEMPLATE}{}{}{}</project>",
        diagram(r#"folio="1" title="One" author="A""#, props),
        diagram(r#"folio="2" title="Two" author="A""#, props),
        diagram(r#"folio="3" title="Three" author="B""#, props),
    );
    let b = import(&xml);
    let p = pages(&b);
    let tb = |i: usize| {
        let frame = form(&b, p[i].form.expect("a frame"));
        form(&b, frame.item.title_block.expect("a title block"))
    };
    let first = tb(0);
    assert_eq!(first.item.kind, FormKind::TitleBlock);
    // As wide as the border (20 + 17 × 60 px), as high as the rows.
    assert_eq!(
        (first.item.sheet.width, first.item.sheet.height),
        (px(1).at(1040).unwrap(), px(1).at(20).unwrap())
    );
    let values: Vec<&str> = first
        .item
        .cells
        .iter()
        .filter_map(|c| c.value.as_deref())
        .collect();
    assert_eq!(
        values,
        [
            "{page.title}",
            "By: A {{ACME}} ",
            "Sheet: {doc.page_index}/{doc.page_count} %unknown"
        ]
    );
    // `colspan` counts the columns beyond the first: the title spans columns 1 and 2, to the right edge.
    assert_eq!(first.item.cells[0].rect.0.max().x, px(1).at(1040).unwrap());
    assert_eq!(first.item.cells[0].style.as_str(), "qet.7");
    assert!(!first.item.graphics.is_empty());
    // Equal values share one form; another author makes another.
    assert_eq!(tb(0).id, tb(1).id);
    assert_ne!(tb(0).id, tb(2).id);
    // The sizes the cells use are project text styles; the logo and the path are reported.
    let project = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Project(p) => Some(p),
            _ => None,
        })
        .unwrap();
    assert!(project.settings.text_styles.keys().any(|k| k.as_str() == "qet.6"));
    let said = |needle: &str| b.report.entries.iter().any(|e| e.message.contains(needle));
    assert!(said("logo"), "{:?}", b.report.entries);
    assert!(said("local file path"));
}

#[test]
fn a_table_filling_its_folios_becomes_a_page_oriented_block() {
    let head = r#"<tables><graphics_table uuid="{11111111-1111-4111-8111-111111111111}" x="50" y="70" width="900" height="500">
      <model><project_data_base_model><query>SELECT folio, title FROM project_summary_view</query></project_data_base_model></model>
    </graphics_table></tables>"#;
    let next = r#"<tables><graphics_table uuid="{22222222-2222-4222-8222-222222222222}" x="50" y="70" width="900" height="200">
      <previous_table uuid="{11111111-1111-4111-8111-111111111111}"/>
    </graphics_table></tables>"#;
    let nomenclature = r#"<inputs><input x="10" y="10" text="note"/></inputs>
    <tables><graphics_table uuid="{33333333-3333-4333-8333-333333333333}" x="100" y="200" width="300" height="150">
      <model><project_data_base_model><query>SELECT * FROM element_nomenclature_view</query></project_data_base_model></model>
    </graphics_table></tables>"#;
    let xml = format!(
        "<project>{}{}{}{}</project>",
        diagram(r#"folio="1" title="Cover""#, ""),
        diagram(r#"folio="2" title="Contents 1""#, head),
        diagram(r#"folio="3" title="Contents 2""#, next),
        diagram(r#"folio="4" title="Parts""#, nomenclature),
    );
    let b = import(&xml);
    let folios: Vec<u32> = pages(&b).iter().map(|p| p.folio.number).collect();
    assert_eq!(folios, [1, 4]);
    let bl = blocks(&b);
    let toc = bl
        .iter()
        .find(|r| r.source == ReportSource::Kind(ReportKind::Toc))
        .expect("a TOC block");
    assert_eq!(toc.first_folio, Some(2));
    assert_eq!(toc.continuation, ContinuationPolicy::NextFolio);
    assert_eq!(toc.title.as_neutral(), Some("Contents {n}"));
    assert!(toc.embedded.is_none());
    let parts = bl
        .iter()
        .find(|r| r.source == ReportSource::Kind(ReportKind::PartsList))
        .expect("a parts block");
    let embed = parts.embedded.as_ref().expect("embedded in its folio");
    assert_eq!(Some(embed.page), pages(&b).last().map(|p| p.id));
    assert_eq!(embed.rect.0.min().x, px(1).at(100).unwrap());
    assert_eq!(embed.rect.0.max().y, px(1).at(350).unwrap());
}
