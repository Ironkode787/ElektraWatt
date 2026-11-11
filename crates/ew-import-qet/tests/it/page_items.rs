// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A diagram's free texts, shapes and images become page items where QET drew them, and placed elements print
//! their own texts and nothing else (doc 14 §4.4.2, §4.13). The project is written by hand with made-up content.

use ew_import_qet::QetProjectImporter;
use ew_import_qet::decimal::Decimal;
use ew_import_qet::ew_interop_api::ew_schema::{
    FreeText, Graphic, GraphicShape, ImagePrim, LibDocument, LineDash, PageItemKind, Primitive, Record,
    Representation, SlotKind, TextAlign, base64_decode,
};
use ew_import_qet::ew_interop_api::{Code, ImportBatch, ImportInput};
use ew_import_qet::project::new_project_context;

/// A 1 × 1 PNG.
const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";

const FONT9: &str = "Sans Serif,9,-1,5,50,0,0,0,0,0";

/// The em height of a 9 pt QET font (the `text.normal` of doc 20 §17.5), nm.
const EM9: i64 = 3_175_000;

/// The drop from a text box's top-left corner to its first line's middle, after the 4 px margin: 0.547 em.
const MIDDLE9: i64 = 1_736_725;

fn px(v: &str) -> i64 {
    Decimal::parse(v).unwrap().px_to_nm().unwrap().0
}

/// One placed element of the definition `a.elmt` (one ElementInfo label slot in 9 pt, two terminals).
fn element(uuid: &str, x: u32, infos: &str, texts: &str) -> String {
    format!(
        r#"<element type="embed://import/a.elmt" uuid="{{{uuid}}}" x="{x}" y="100" orientation="0">
 <terminals/>
 <elementInformations>{infos}</elementInformations>
 <dynamic_texts>{texts}</dynamic_texts>
</element>"#
    )
}

fn project() -> String {
    // Qt rich text, escaped into the attribute as QET writes it.
    let html = "&lt;!DOCTYPE HTML PUBLIC &quot;-//W3C//DTD HTML 4.0//EN&quot;&gt;&lt;html&gt;&lt;body style=&quot; font-family:'Sans Serif'; font-size:12pt; font-weight:400;&quot;&gt;\n&lt;p style=&quot; margin-top:0px;&quot;&gt;First &amp;amp; one&lt;/p&gt;\n&lt;p style=&quot; margin-top:0px;&quot;&gt;Second&lt;/p&gt;&lt;/body&gt;&lt;/html&gt;";
    let label_text = |x: u32| {
        format!(
            r#"<dynamic_elmt_text text_from="ElementInfo" x="{x}" y="-30" rotation="0" font="{FONT9}"><text/><info_name>label</info_name></dynamic_elmt_text>"#
        )
    };
    let elements = [
        // Unlabelled: QET prints nothing for its label text.
        element("10000000-0000-4000-8000-000000000001", 100, "", &label_text(5)),
        // Labelled: the tag slot shows at the element's own text.
        element(
            "10000000-0000-4000-8000-000000000002",
            300,
            r#"<elementInformation name="label">-K7</elementInformation>"#,
            &label_text(5),
        ),
        // An information text: printed on the page at its own place.
        element(
            "10000000-0000-4000-8000-000000000003",
            500,
            r#"<elementInformation name="comment">Pump left</elementInformation>"#,
            &format!(
                r#"<dynamic_elmt_text text_from="ElementInfo" x="20" y="30" rotation="0" font="{FONT9}"><text/><info_name>comment</info_name></dynamic_elmt_text>"#
            ),
        ),
        // An information text aligned right in a box 90 px wide.
        element(
            "10000000-0000-4000-8000-000000000004",
            700,
            r#"<elementInformation name="comment">Valve right</elementInformation>"#,
            &format!(
                r#"<dynamic_elmt_text text_from="ElementInfo" x="20" y="30" rotation="0" font="{FONT9}" text_width="90" Halignment="AlignRight"><text/><info_name>comment</info_name></dynamic_elmt_text>"#
            ),
        ),
    ]
    .join("\n");
    format!(
        r##"<project version="0.100.0" title="Items">
<diagram folio="1" order="1" cols="17" colsize="60" rows="8" rowsize="80" height="660">
<elements>
{elements}
</elements>
<inputs>
 <input x="10" y="200" rotation="0" font="Sans Serif,12,-1,5,50,0,0,0,0,0" text="Note"/>
 <input x="10" y="300" rotation="0" font="{FONT9}" text="{html}"/>
</inputs>
<shapes>
 <shape type="Line" x1="10" y1="20" x2="110" y2="20" closed="0"><pen style="DashLine" widthF="2" color="#000000"/><brush style="NoBrush" color="#000000"/></shape>
 <shape type="Rectangle" x1="50" y1="60" x2="10" y2="30" rx="0" ry="0" closed="0"><pen style="SolidLine" widthF="1" color="#000000"/><brush style="SolidPattern" color="#000000"/></shape>
 <shape type="Rectangle" x1="200" y1="30" x2="300" y2="90" rx="10" ry="10" closed="0"><pen style="SolidLine" widthF="1" color="#000000"/><brush style="NoBrush"/></shape>
 <shape type="Ellipse" x1="60" y1="30" x2="80" y2="50" closed="0"><pen style="DotLine" widthF="1" color="#000000"/><brush style="NoBrush"/></shape>
 <shape type="Polygon" closed="1"><pen style="SolidLine" widthF="1" color="#000000"/><brush style="NoBrush"/><points><point x="0" y="0"/><point x="10" y="0"/><point x="10" y="10"/></points></shape>
</shapes>
<images>
 <image x="100" y="50" size="3" rotation="0" z="0" is_movable="1">
{PNG}
 </image>
</images>
</diagram>
<collection>
 <category name="import">
  <element name="a.elmt">
   <definition type="element" link_type="simple" version="0.100" width="20" height="20" hotspot_x="10" hotspot_y="10">
    <uuid uuid="{{20000000-0000-4000-8000-000000000001}}"/>
    <names><name lang="en">Part</name></names>
    <description>
     <line x1="-10" y1="0" x2="10" y2="0"/>
     <dynamic_text x="5" y="-20" text_from="ElementInfo" uuid="{{20000000-0000-4000-8000-000000000002}}" font="{FONT9}"><text/><info_name>label</info_name></dynamic_text>
     <dynamic_text x="5" y="10" text_from="UserText" uuid="{{20000000-0000-4000-8000-000000000003}}"><text>Template</text></dynamic_text>
     <terminal x="-10" y="0" orientation="w" uuid="{{20000000-0000-4000-8000-000000000004}}"/>
     <terminal x="10" y="0" orientation="e" uuid="{{20000000-0000-4000-8000-000000000005}}"/>
    </description>
   </definition>
  </element>
 </category>
</collection>
</project>"##
    )
}

fn import() -> ImportBatch {
    let input = ImportInput::new("items.qet", project().into_bytes()).unwrap();
    QetProjectImporter::new()
        .import(&input, &mut new_project_context())
        .unwrap()
}

fn kinds(batch: &ImportBatch) -> Vec<&PageItemKind> {
    batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::PageItem(i) => Some(&i.kind),
            _ => None,
        })
        .collect()
}

fn free_texts(batch: &ImportBatch) -> Vec<&FreeText> {
    kinds(batch)
        .into_iter()
        .filter_map(|k| match k {
            PageItemKind::FreeText(t) => Some(t),
            _ => None,
        })
        .collect()
}

fn graphics(batch: &ImportBatch) -> Vec<&Graphic> {
    kinds(batch)
        .into_iter()
        .filter_map(|k| match k {
            PageItemKind::Graphic(g) => Some(g),
            _ => None,
        })
        .collect()
}

fn text<'b>(batch: &'b ImportBatch, s: &str) -> &'b FreeText {
    free_texts(batch)
        .into_iter()
        .find(|t| t.text.as_neutral() == Some(s))
        .unwrap_or_else(|| panic!("no free text {s:?}"))
}

#[test]
fn free_texts_stand_where_qet_drew_them_in_their_size() {
    let b = import();
    // The box's top-left corner (10, 200) px, the 4 px margin, then down to the middle of the first line.
    let note = text(&b, "Note");
    let em12 = 4_233_333;
    assert_eq!(note.at.x.0, px("10") + px("4"));
    assert_eq!(note.at.y.0, px("200") + px("4") + em12 * 547 / 1000);
    assert_eq!(note.style.as_ref().map(|s| s.as_str()), Some("qet.12"));
    // Qt rich text: one free text per paragraph, the body's size, entities decoded.
    let first = text(&b, "First & one");
    let second = text(&b, "Second");
    assert_eq!(first.style.as_ref().map(|s| s.as_str()), Some("qet.12"));
    assert_eq!(first.at.x, second.at.x);
    assert_eq!(second.at.y.0 - first.at.y.0, em12 * 1150 / 1000 + 1);
    // The styles the drawing names are the project's.
    let project = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Project(p) => Some(p),
            _ => None,
        })
        .unwrap();
    let styles = &project.settings.text_styles;
    assert_eq!(styles.get("qet.12").map(|s| s.size.0), Some(em12));
    assert_eq!(styles.get("qet.9").map(|s| s.size.0), Some(EM9));
    // Nothing says these were not imported.
    assert!(
        !b.report
            .entries
            .iter()
            .any(|e| e.code == Code::DROPPED_BY_DESIGN && e.message.contains("not imported")),
        "{:?}",
        b.report.entries
    );
}

#[test]
fn shapes_become_graphics_on_their_points() {
    let b = import();
    let g = graphics(&b);
    let line = g
        .iter()
        .find_map(|g| match &g.shape {
            GraphicShape::Line(l) => Some((l, g)),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        (line.0.a.x.0, line.0.a.y.0, line.0.b.x.0),
        (px("10"), px("20"), px("110"))
    );
    assert_eq!(line.1.dash, LineDash::Dashed);
    assert_eq!(line.1.stroke.as_ref().map(|s| s.width.0), Some(px("2")));
    let rect = g
        .iter()
        .find_map(|g| match &g.shape {
            GraphicShape::Rect(r) => Some((r, g)),
            _ => None,
        })
        .unwrap();
    // Corners normalised, the brush fills it.
    assert_eq!((rect.0.min.x.0, rect.0.min.y.0), (px("10"), px("30")));
    assert_eq!((rect.0.max.x.0, rect.0.max.y.0), (px("50"), px("60")));
    assert!(rect.1.fill);
    let circle = g
        .iter()
        .find_map(|g| match &g.shape {
            GraphicShape::Circle(c) => Some((c, g)),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        (circle.0.centre.x.0, circle.0.centre.y.0, circle.0.r.0),
        (px("70"), px("40"), px("10"))
    );
    assert_eq!(circle.1.dash, LineDash::Dotted);
    let polygons: Vec<&Vec<_>> = g
        .iter()
        .filter_map(|g| match &g.shape {
            GraphicShape::Polygon(p) => Some(&p.points),
            _ => None,
        })
        .collect();
    // The triangle as drawn, and the rounded rectangle through its corners within its box.
    assert!(
        polygons
            .iter()
            .any(|p| p.len() == 3 && p[1].x.0 == px("10") && p[1].y.0 == 0)
    );
    let rounded = polygons.iter().find(|p| p.len() > 3).unwrap();
    assert_eq!(rounded.iter().map(|p| p.x.0).min(), Some(px("200")));
    assert_eq!(rounded.iter().map(|p| p.y.0).max(), Some(px("90")));
    assert!(
        !rounded.iter().any(|p| p.x.0 == px("200") && p.y.0 == px("30")),
        "the corner is rounded"
    );
}

#[test]
fn an_image_is_drawn_scaled_about_its_middle_and_its_bytes_travel_with_the_batch() {
    let b = import();
    let image = kinds(&b)
        .into_iter()
        .find_map(|k| match k {
            PageItemKind::Image(i) => Some(i),
            _ => None,
        })
        .unwrap();
    // 1 × 1 px at (100, 50), three times as large about its middle (100.5, 50.5).
    let r = image.rect.0;
    assert_eq!((r.min().x.0, r.min().y.0), (px("99"), px("49")));
    assert_eq!((r.max().x.0, r.max().y.0), (px("102"), px("52")));
    assert_eq!(b.assets, vec![base64_decode(PNG).unwrap()]);
}

fn reps(b: &ImportBatch) -> Vec<&Representation> {
    b.records
        .iter()
        .filter_map(|r| match r {
            Record::Representation(r) => Some(r),
            _ => None,
        })
        .collect()
}

#[test]
fn an_element_prints_its_own_texts_and_no_tag_where_qet_prints_none() {
    let b = import();
    let symbol = b
        .lib
        .iter()
        .find_map(|d| match d {
            LibDocument::Symbol(s) => Some(&s.item),
            _ => None,
        })
        .unwrap();
    // The definition's texts are templates: its label slot is hidden, its user text is not drawn.
    let tag = symbol
        .text_slots
        .iter()
        .position(|s| s.kind == SlotKind::Tag)
        .unwrap();
    assert!(!symbol.text_slots[tag].visible);
    assert!(
        free_texts(&b)
            .iter()
            .all(|t| t.text.as_neutral() != Some("Template"))
    );
    let by_x = |x: &str| reps(&b).into_iter().find(|r| r.pos.x.0 == px(x)).unwrap();
    // The unlabelled element shows no slot and no text of its own.
    assert!(by_x("100").text_slots.is_empty());
    // The labelled one shows its tag slot at its own text: (5, -30) px, the margin, the first line's middle.
    let shown = by_x("300").text_slots.iter().next().unwrap();
    assert_eq!(usize::from(shown.slot), tag);
    assert_eq!(shown.visible, Some(true));
    let pos = shown.pos.unwrap();
    assert_eq!(
        (pos.x.0, pos.y.0),
        (px("5") + px("4"), px("-30") + px("4") + MIDDLE9)
    );
    // The information text prints its value at (500 + 20, 100 + 30) px, the margin, the line's middle.
    let comment = text(&b, "Pump left");
    assert_eq!(
        (comment.at.x.0, comment.at.y.0),
        (
            px("500") + px("20") + px("4"),
            px("100") + px("30") + px("4") + MIDDLE9
        )
    );
    assert_eq!(comment.style.as_ref().map(|s| s.as_str()), Some("qet.9"));
    assert_eq!(comment.align, None);
    // In a box of fixed width QET 0.100 centres the text whatever its alignment: on 700 + 20 + 90 / 2.
    let centred = text(&b, "Valve right");
    assert_eq!(
        (centred.at.x.0, centred.at.y.0),
        (
            px("700") + px("20") + px("45"),
            px("100") + px("30") + px("4") + MIDDLE9
        )
    );
    assert_eq!(centred.align, Some(TextAlign::Centre));
}

/// The import of a project whose title block draws the logo `logo` (a `<logo>` resource named `mark`) in a
/// cell of 40 × 20 px.
fn logo_project(logo: &str) -> ImportBatch {
    let xml = format!(
        r#"<project version="0.100.0">
<titleblocktemplates>
 <titleblocktemplate name="tb">
  <logos>{logo}</logos>
  <grid cols="40px;r100%;" rows="20;">
   <logo name="" col="0" row="0" resource="mark"/>
  </grid>
 </titleblocktemplate>
</titleblocktemplates>
<diagram folio="1" titleblocktemplate="tb" titleblocktemplateCollection="embedded"/>
</project>"#
    );
    let input = ImportInput::new("logo.qet", xml.into_bytes()).unwrap();
    QetProjectImporter::new()
        .import(&input, &mut new_project_context())
        .unwrap()
}

/// The image a title-block form of `b` draws.
fn logo_image(b: &ImportBatch) -> &ImagePrim {
    b.lib
        .iter()
        .filter_map(|d| match d {
            LibDocument::Form(f) => Some(&f.item.graphics),
            _ => None,
        })
        .flatten()
        .find_map(|g| match g {
            Primitive::Image(i) => Some(i),
            _ => None,
        })
        .expect("the logo is an image of the title block")
}

#[test]
fn a_title_block_logo_in_svg_is_drawn_in_its_cell() {
    // QET writes an SVG logo as the SVG element itself (`storage="xml"`): it is the asset, as written.
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10"/></svg>"#;
    let b = logo_project(&format!(
        r#"<logo name="mark" storage="xml" type="svg">{svg}</logo>"#
    ));
    // Twice as wide as high in a 40 × 20 px cell: the whole cell (its height half its width, in whole nm).
    let r = logo_image(&b).rect.0;
    assert_eq!((r.min().x.0, r.min().y.0), (0, 0));
    assert_eq!((r.max().x.0, r.max().y.0), (px("40"), px("40") / 2));
    assert_eq!(b.assets, vec![svg.as_bytes().to_vec()]);
}

#[test]
fn a_title_block_logo_is_drawn_in_its_cell_from_the_same_asset_store() {
    let b = logo_project(&format!(
        r#"<logo name="mark" storage="base64" type="png">
{PNG}
  </logo>"#
    ));
    // A square picture in a 40 × 20 px cell: 20 × 20 px, centred.
    let r = logo_image(&b).rect.0;
    assert_eq!((r.min().x.0, r.min().y.0), (px("10"), 0));
    assert_eq!((r.max().x.0, r.max().y.0), (px("10") + px("20"), px("20")));
    assert_eq!(b.assets, vec![base64_decode(PNG).unwrap()]);
}
