// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading the `.qet` DOM into [`QetProject`] (doc 14 §4.1): plain owned data, no mapping decision. Item
//! problems are findings ([`Findings`]), never errors; only a file that is not a readable `<project>` is an
//! [`ImportError`]. Every pass is linear in the input: the only unbounded nesting, the embedded collection,
//! is walked with an explicit stack.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use ew_interop_api::{Code, ImportError, Limits, Severity};
use roxmltree::Node;

use super::cx::Findings;
use super::model::{
    Border, DynText, EmbeddedDef, End, InstTerminal, QetConductor, QetDiagram, QetElement, QetProject,
    Segment, TerminalRef, norm_uuid,
};
use crate::decimal::Decimal;
use crate::element::{child, parse_xml};
use crate::xml::LineIndex;

/// The title-block attributes of a `<diagram>`, in the order [`QetDiagram::titleblock`] lists them.
const TITLEBLOCK: [&str; 9] = [
    "author",
    "date",
    "indexrev",
    "filename",
    "plant",
    "locmach",
    "titleblocktemplate",
    "auto_page_num",
    "displayAt",
];

/// The longest collection path (bytes) the reader keeps; a category or element below it is dropped with a
/// warning, so that a hostile nesting of long names cannot multiply the input size.
const MAX_PATH_BYTES: usize = 4096;

/// The longest piece of source text quoted in a finding message.
const QUOTE_CHARS: usize = 48;

/// Reads the project in `xml` into owned data. Runs on the parser thread, after [`crate::xml::prescan`].
///
/// # Errors
///
/// [`ImportError::Malformed`] for malformed XML or a DOCTYPE, [`ImportError::LimitExceeded`] for an
/// exceeded limit, [`ImportError::Unsupported`] if the root is not `<project>`.
pub fn read_project(xml: &str, findings: &mut Findings, limits: &Limits) -> Result<QetProject, ImportError> {
    let doc = parse_xml(xml, limits)?;
    let root = doc.root_element();
    if root.tag_name().name() != "project" {
        return Err(ImportError::Unsupported {
            message: format!(
                "the root element is <{}>, not <project>",
                clip(root.tag_name().name())
            ),
        });
    }
    let lines = LineIndex::new(xml);
    let rd = Reader { xml, lines: &lines };

    let mut project = QetProject {
        version: root.attribute("version").map(str::to_owned),
        uuid: root.attribute("uuid").and_then(norm_uuid),
        title: root.attribute("title").map(str::to_owned),
        properties: child(root, "properties").map(properties).unwrap_or_default(),
        report_label: child(root, "newdiagrams")
            .and_then(|n| child(n, "report"))
            .and_then(|n| n.attribute("label"))
            .map(str::to_owned),
        ..QetProject::default()
    };

    let mut unmapped_at: BTreeMap<&str, usize> = BTreeMap::new();
    for node in root.children().filter(Node::is_element) {
        match node.tag_name().name() {
            "properties" | "newdiagrams" => {}
            "diagram" => {
                let index = project.diagrams.len();
                project.diagrams.push(rd.diagram(node, index, findings));
            }
            "collection" => rd.collection(node, &mut project.definitions, findings),
            other => match unmapped_at.get(other) {
                Some(&i) => project.unmapped_top_level[i].1 += 1,
                None => {
                    unmapped_at.insert(other, project.unmapped_top_level.len());
                    project.unmapped_top_level.push((other.to_owned(), 1));
                }
            },
        }
    }
    Ok(project)
}

/// What the reading functions share.
struct Reader<'a> {
    xml: &'a str,
    lines: &'a LineIndex,
}

impl Reader<'_> {
    /// The 1-based line of the start tag of `node`.
    fn line(&self, node: Node<'_, '_>) -> u32 {
        self.lines.line(self.xml, node.range().start)
    }

    /// One `<diagram>`.
    fn diagram(&self, node: Node<'_, '_>, index: usize, findings: &mut Findings) -> QetDiagram {
        let folio_attr = node.attribute("folio").unwrap_or("");
        let folio = (!folio_attr.is_empty()).then_some(folio_attr);
        let mut d = QetDiagram {
            index,
            order: node.attribute("order").and_then(|v| v.trim().parse().ok()),
            title: node.attribute("title").unwrap_or("").to_owned(),
            folio: folio_attr.to_owned(),
            titleblock: TITLEBLOCK
                .iter()
                .filter_map(|n| node.attribute(*n).map(|v| ((*n).to_owned(), v.to_owned())))
                .collect(),
            border: Border {
                cols: unsigned(node.attribute("cols")),
                colsize: decimal(node.attribute("colsize")),
                rows: unsigned(node.attribute("rows")),
                rowsize: decimal(node.attribute("rowsize")),
                height: decimal(node.attribute("height")),
            },
            line: self.line(node),
            ..QetDiagram::default()
        };
        let mut unknown: BTreeSet<&str> = BTreeSet::new();
        let (mut n_elements, mut n_conductors) = (0usize, 0usize);
        for c in node.children().filter(Node::is_element) {
            match c.tag_name().name() {
                "properties" => d.properties.extend(properties(c)),
                "defaultconductor" => {}
                "elements" => {
                    for e in c
                        .children()
                        .filter(|e| e.is_element() && e.tag_name().name() == "element")
                    {
                        if let Some(el) = self.element(e, n_elements, folio, findings) {
                            d.elements.push(el);
                        }
                        n_elements += 1;
                    }
                }
                "conductors" => {
                    for e in c
                        .children()
                        .filter(|e| e.is_element() && e.tag_name().name() == "conductor")
                    {
                        d.conductors
                            .push(self.conductor(e, n_conductors, folio, findings));
                        n_conductors += 1;
                    }
                }
                "inputs" => d.free_texts = d.free_texts.saturating_add(count(c, "input")),
                "shapes" => d.shapes = d.shapes.saturating_add(count(c, "shape")),
                "images" => d.images = d.images.saturating_add(count(c, "image")),
                "tables" => d.tables = d.tables.saturating_add(count(c, "graphics_table")),
                other => {
                    if unknown.insert(other) {
                        let loc = findings.loc(folio, None, Some(self.line(c)));
                        findings.push(
                            Code::DROPPED_BY_DESIGN,
                            Severity::Info,
                            loc,
                            format!("unsupported {} dropped", clip(other)),
                        );
                    }
                }
            }
        }
        d
    }

    /// One placed `<element>`; `None` (reported) when its position is unusable.
    fn element(
        &self,
        node: Node<'_, '_>,
        n: usize,
        folio: Option<&str>,
        findings: &mut Findings,
    ) -> Option<QetElement> {
        let line = self.line(node);
        let uuid_attr = node.attribute("uuid");
        let item = || match uuid_attr {
            Some(u) => clip(u),
            None => format!("element {n}"),
        };
        let (Some(x), Some(y)) = (decimal(node.attribute("x")), decimal(node.attribute("y"))) else {
            let loc = findings.loc(folio, Some(item()), Some(line));
            findings.push(
                Code::MALFORMED_ITEM,
                Severity::Error,
                loc,
                "element skipped: bad x/y",
            );
            return None;
        };
        let orientation = match node.attribute("orientation") {
            None => 0,
            Some(v) => match v.trim().parse::<u8>() {
                Ok(o @ 0..=3) => o,
                _ => {
                    let loc = findings.loc(folio, Some(item()), Some(line));
                    findings.push(
                        Code::MALFORMED_ITEM,
                        Severity::Warning,
                        loc,
                        format!("invalid orientation {}; 0 used", clip(v)),
                    );
                    0
                }
            },
        };
        let terminals = child(node, "terminals")
            .map(|t| {
                t.children()
                    .filter(|c| c.is_element() && c.tag_name().name() == "terminal")
                    .map(|c| InstTerminal {
                        id: c.attribute("id").map(str::to_owned),
                        x: decimal(c.attribute("x")),
                        y: decimal(c.attribute("y")),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let infos = child(node, "elementInformations")
            .map(|t| {
                t.children()
                    .filter(|c| c.is_element() && c.tag_name().name() == "elementInformation")
                    .map(|c| (c.attribute("name").unwrap_or("").to_owned(), text(c)))
                    .collect()
            })
            .unwrap_or_default();
        let dyn_texts = child(node, "dynamic_texts")
            .map(|t| {
                t.children()
                    .filter(|c| c.is_element() && c.tag_name().name() == "dynamic_elmt_text")
                    .map(|c| DynText {
                        text_from: c.attribute("text_from").unwrap_or("").to_owned(),
                        text: child(c, "text").map(text).unwrap_or_default(),
                        info_name: child(c, "info_name").map(text),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let links = child(node, "links_uuids")
            .map(|t| {
                t.children()
                    .filter(|c| c.is_element() && c.tag_name().name() == "link_uuid")
                    .filter_map(|c| c.attribute("uuid").and_then(norm_uuid))
                    .collect()
            })
            .unwrap_or_default();
        Some(QetElement {
            n,
            type_path: node.attribute("type").unwrap_or("").to_owned(),
            uuid: uuid_attr.and_then(norm_uuid),
            x,
            y,
            orientation,
            prefix: node.attribute("prefix").unwrap_or("").to_owned(),
            terminals,
            infos,
            dyn_texts,
            links,
            line,
        })
    }

    /// One `<conductor>`.
    fn conductor(
        &self,
        node: Node<'_, '_>,
        n: usize,
        folio: Option<&str>,
        findings: &mut Findings,
    ) -> QetConductor {
        let line = self.line(node);
        let mut segments = Vec::new();
        if let Some(s) = child(node, "segments") {
            for seg in s
                .children()
                .filter(|c| c.is_element() && c.tag_name().name() == "segment")
            {
                match decimal(seg.attribute("length")) {
                    Some(length) => segments.push(Segment {
                        horizontal: seg.attribute("orientation") == Some("horizontal"),
                        length,
                    }),
                    None => {
                        let loc = findings.loc(folio, Some(format!("conductor {n}")), Some(self.line(seg)));
                        findings.push(
                            Code::MALFORMED_ITEM,
                            Severity::Warning,
                            loc,
                            "segment dropped: bad length",
                        );
                    }
                }
            }
        }
        let attr = |name: &str| node.attribute(name).unwrap_or("").to_owned();
        QetConductor {
            end1: End {
                element: node.attribute("element1").and_then(norm_uuid),
                terminal: terminal_ref(node.attribute("terminal1")),
            },
            end2: End {
                element: node.attribute("element2").and_then(norm_uuid),
                terminal: terminal_ref(node.attribute("terminal2")),
            },
            num: attr("num"),
            function: attr("function"),
            tension_protocol: attr("tension_protocol"),
            color: attr("conductor_color"),
            section: attr("conductor_section"),
            segments,
            line,
        }
    }

    /// The embedded definitions below `<collection>` `root`, appended to `out` in document order. Iterative:
    /// the nesting of categories is only bounded by the depth limit.
    fn collection(&self, root: Node<'_, '_>, out: &mut Vec<EmbeddedDef>, findings: &mut Findings) {
        // Each entry: a node to visit and the category path (names joined by `/`) it sits in.
        let mut stack: Vec<(Node<'_, '_>, Rc<str>)> = Vec::new();
        push_children(&mut stack, root, &Rc::from(""));
        while let Some((node, prefix)) = stack.pop() {
            let name = node.attribute("name").unwrap_or("");
            let path = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}/{name}")
            };
            match node.tag_name().name() {
                "category" => {
                    if path.len() > MAX_PATH_BYTES {
                        let loc = findings.loc(None, Some(clip(&path)), Some(self.line(node)));
                        findings.push(
                            Code::MALFORMED_ITEM,
                            Severity::Warning,
                            loc,
                            "collection category skipped: path too long",
                        );
                    } else {
                        push_children(&mut stack, node, &Rc::from(path));
                    }
                }
                "element" => {
                    if path.len() > MAX_PATH_BYTES {
                        let loc = findings.loc(None, Some(clip(&path)), Some(self.line(node)));
                        findings.push(
                            Code::MALFORMED_ITEM,
                            Severity::Warning,
                            loc,
                            "collection element skipped: path too long",
                        );
                    } else if let Some(def) = child(node, "definition") {
                        out.push(EmbeddedDef {
                            path: format!("embed://{path}"),
                            range: def.range(),
                            line: self.line(def),
                        });
                    } else {
                        let loc = findings.loc(None, Some(format!("embed://{path}")), Some(self.line(node)));
                        findings.push(
                            Code::MALFORMED_ITEM,
                            Severity::Warning,
                            loc,
                            "collection element without definition",
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

/// Pushes the `category` and `element` children of `node` so that they pop in document order.
fn push_children<'a, 'i>(stack: &mut Vec<(Node<'a, 'i>, Rc<str>)>, node: Node<'a, 'i>, prefix: &Rc<str>) {
    stack.extend(
        node.children()
            .rev()
            .filter(|c| c.is_element() && matches!(c.tag_name().name(), "category" | "element"))
            .map(|c| (c, Rc::clone(prefix))),
    );
}

/// `(name, text)` of the `<property>` children of `node`.
fn properties(node: Node<'_, '_>) -> Vec<(String, String)> {
    node.children()
        .filter(|c| c.is_element() && c.tag_name().name() == "property")
        .map(|c| (c.attribute("name").unwrap_or("").to_owned(), text(c)))
        .collect()
}

/// The number of child elements of `node` named `name`.
fn count(node: Node<'_, '_>, name: &str) -> u32 {
    let n = node
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == name)
        .count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The trimmed text of `node` ("" when it has none).
fn text(node: Node<'_, '_>) -> String {
    node.text().unwrap_or("").trim().to_owned()
}

/// An attribute as [`Decimal`]; `None` when absent or unparsable.
fn decimal(v: Option<&str>) -> Option<Decimal> {
    v.and_then(|v| Decimal::parse(v).ok())
}

/// An attribute as `u32`; `None` when absent or unparsable.
fn unsigned(v: Option<&str>) -> Option<u32> {
    v.and_then(|v| v.trim().parse().ok())
}

/// The `terminal{i}` attribute of a conductor.
fn terminal_ref(v: Option<&str>) -> TerminalRef {
    let Some(raw) = v else {
        return TerminalRef::Missing;
    };
    let t = raw.trim();
    if t.is_empty() {
        TerminalRef::Missing
    } else if let Some(u) = norm_uuid(t) {
        TerminalRef::Uuid(u)
    } else if is_integer(t) {
        TerminalRef::Id(t.to_owned())
    } else {
        TerminalRef::Other(raw.to_owned())
    }
}

/// Whether `t` is ASCII digits with an optional leading `-`.
fn is_integer(t: &str) -> bool {
    let digits = t.strip_prefix('-').unwrap_or(t);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// `s` shortened to [`QUOTE_CHARS`] characters, for finding messages and items.
fn clip(s: &str) -> String {
    match s.char_indices().nth(QUOTE_CHARS) {
        Some((at, _)) => format!("{}...", &s[..at]),
        None => s.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use ew_interop_api::{Code, ImportError, Limits, ReportEntry, Severity};

    use super::super::cx::Findings;
    use super::super::model::{End, QetProject, Segment, TerminalRef};
    use super::read_project;
    use crate::decimal::Decimal;

    const U1: &str = "{6F1C9A0E-3B7D-4E2F-8A5C-1D0E9B8A7C6F}";
    const U1N: &str = "6f1c9a0e-3b7d-4e2f-8a5c-1d0e9b8a7c6f";
    const U2: &str = "11111111-2222-4333-8444-555555555555";
    const TU: &str = "{AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE}";

    fn dec(s: &str) -> Decimal {
        Decimal::parse(s).unwrap()
    }

    fn read(xml: &str) -> (Result<QetProject, ImportError>, Vec<ReportEntry>) {
        let mut f = Findings::new("t.qet", 100);
        let r = read_project(xml, &mut f, &Limits::default());
        (r, f.finish().0)
    }

    /// The 1-based line of the first occurrence of `needle` in `xml`.
    fn line_of(xml: &str, needle: &str) -> u32 {
        let at = xml.find(needle).unwrap();
        xml[..at].matches('\n').count() as u32 + 1
    }

    fn sample() -> String {
        format!(
            r#"<project version="0.100.0" title="Demo">
 <properties>
  <property name="k1">v1</property>
  <property name="k2"> v 2 </property>
  <property>noname</property>
 </properties>
 <newdiagrams>
  <report label="%f-%l%c"/>
 </newdiagrams>
 <titleblocktemplates><template name="a"/></titleblocktemplates>
 <diagram order="2" title="First" folio="1" author="A" date="2026-01-01" plant="P" displayAt="bottom" cols="17" colsize="60.5" rows="8" rowsize="80" height="640">
  <properties>
   <property name="x">y</property>
  </properties>
  <defaultconductor num="_" function=""/>
  <elements>
   <element type="embed://import/10_electric/a.elmt" uuid="{U1}" x="100.5" y="-20" orientation="2" prefix="K">
    <terminals>
     <terminal id="7" x="0" y="-10"/>
     <terminal x="bad"/>
    </terminals>
    <elementInformations>
     <elementInformation name="label"> K1 </elementInformation>
     <elementInformation name="comment"/>
    </elementInformations>
    <dynamic_texts>
     <dynamic_elmt_text text_from="UserText"><text>hello</text></dynamic_elmt_text>
     <dynamic_elmt_text text_from="ElementInfo"><text/><info_name>label</info_name></dynamic_elmt_text>
    </dynamic_texts>
    <links_uuids>
     <link_uuid uuid="{TU}"/>
     <link_uuid uuid="not-a-uuid"/>
    </links_uuids>
   </element>
  </elements>
  <conductors>
   <conductor element1="{U1}" terminal1="{TU}" element2="{U2}" terminal2="7" num="W1" function="f" tension_protocol="tp" conductor_color="RD" conductor_section="1.5">
    <segments>
     <segment orientation="horizontal" length="12.5"/>
     <segment orientation="vertical" length="-30"/>
    </segments>
   </conductor>
   <conductor element1="x" terminal1="-4" element2="" terminal2="abc"/>
   <conductor terminal1="  "/>
  </conductors>
  <inputs><input/><input/></inputs>
  <shapes><shape/></shapes>
  <images><image/><image/><image/></images>
  <tables><graphics_table/></tables>
 </diagram>
 <diagram title="Second">
  <elements/>
 </diagram>
 <collection>
  <category name="import">
   <category name="10_electric">
    <element name="a.elmt">
     <definition type="element" version="0.100.0"><uuid uuid="{{1}}"/></definition>
    </element>
    <element name="empty.elmt"/>
   </category>
   <element name="b.elmt"><definition/></element>
  </category>
  <element name="top.elmt"><definition x="1"></definition></element>
 </collection>
 <terminal_strips/>
 <terminal_strips/>
</project>
"#
        )
    }

    #[test]
    fn reads_the_sample_exactly() {
        let xml = sample();
        let (r, entries) = read(&xml);
        let p = r.unwrap();
        assert_eq!(p.version.as_deref(), Some("0.100.0"));
        assert_eq!(p.title.as_deref(), Some("Demo"));
        assert_eq!(
            p.properties,
            vec![
                ("k1".to_owned(), "v1".to_owned()),
                ("k2".to_owned(), "v 2".to_owned()),
                (String::new(), "noname".to_owned()),
            ]
        );
        assert_eq!(p.report_label.as_deref(), Some("%f-%l%c"));
        assert_eq!(
            p.unmapped_top_level,
            vec![
                ("titleblocktemplates".to_owned(), 1),
                ("terminal_strips".to_owned(), 2)
            ]
        );
        assert_eq!(p.diagrams.len(), 2);

        let d = &p.diagrams[0];
        assert_eq!(d.index, 0);
        assert_eq!(d.order, Some(2));
        assert_eq!(d.title, "First");
        assert_eq!(d.folio, "1");
        assert_eq!(d.line, line_of(&xml, "<diagram order"));
        assert_eq!(
            d.titleblock,
            vec![
                ("author".to_owned(), "A".to_owned()),
                ("date".to_owned(), "2026-01-01".to_owned()),
                ("plant".to_owned(), "P".to_owned()),
                ("displayAt".to_owned(), "bottom".to_owned()),
            ]
        );
        assert_eq!(d.properties, vec![("x".to_owned(), "y".to_owned())]);
        assert_eq!(d.border.cols, Some(17));
        assert_eq!(d.border.colsize, Some(dec("60.5")));
        assert_eq!(d.border.rows, Some(8));
        assert_eq!(d.border.rowsize, Some(dec("80")));
        assert_eq!(d.border.height, Some(dec("640")));
        assert_eq!((d.free_texts, d.shapes, d.images, d.tables), (2, 1, 3, 1));

        assert_eq!(d.elements.len(), 1);
        let e = &d.elements[0];
        assert_eq!(e.type_path, "embed://import/10_electric/a.elmt");
        assert_eq!(e.uuid.as_deref(), Some(U1N));
        assert_eq!((e.x, e.y), (dec("100.5"), dec("-20")));
        assert_eq!(e.orientation, 2);
        assert_eq!(e.prefix, "K");
        assert_eq!(e.line, line_of(&xml, "<element type"));
        assert_eq!(e.terminals.len(), 2);
        assert_eq!(e.terminals[0].id.as_deref(), Some("7"));
        assert_eq!(e.terminals[0].x, Some(dec("0")));
        assert_eq!(e.terminals[0].y, Some(dec("-10")));
        assert_eq!(e.terminals[1].id, None);
        assert_eq!(e.terminals[1].x, None);
        assert_eq!(
            e.infos,
            vec![
                ("label".to_owned(), "K1".to_owned()),
                ("comment".to_owned(), String::new())
            ]
        );
        assert_eq!(e.info_nonempty("label"), Some("K1"));
        assert_eq!(e.dyn_texts.len(), 2);
        assert_eq!(e.dyn_texts[0].text_from, "UserText");
        assert_eq!(e.dyn_texts[0].text, "hello");
        assert_eq!(e.dyn_texts[0].info_name, None);
        assert_eq!(e.dyn_texts[1].text_from, "ElementInfo");
        assert_eq!(e.dyn_texts[1].text, "");
        assert_eq!(e.dyn_texts[1].info_name.as_deref(), Some("label"));
        assert_eq!(e.links, vec!["aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee".to_owned()]);

        assert_eq!(d.conductors.len(), 3);
        let c = &d.conductors[0];
        assert_eq!(
            c.end1,
            End {
                element: Some(U1N.to_owned()),
                terminal: TerminalRef::Uuid("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee".to_owned()),
            }
        );
        assert_eq!(
            c.end2,
            End {
                element: Some(U2.to_owned()),
                terminal: TerminalRef::Id("7".to_owned()),
            }
        );
        assert_eq!(
            (
                c.num.as_str(),
                c.function.as_str(),
                c.tension_protocol.as_str(),
                c.color.as_str(),
                c.section.as_str()
            ),
            ("W1", "f", "tp", "RD", "1.5")
        );
        assert_eq!(
            c.segments,
            vec![
                Segment {
                    horizontal: true,
                    length: dec("12.5")
                },
                Segment {
                    horizontal: false,
                    length: dec("-30")
                },
            ]
        );
        assert_eq!(c.line, line_of(&xml, "<conductor element1=\"{"));
        let c1 = &d.conductors[1];
        assert_eq!(c1.end1.element, None);
        assert_eq!(c1.end1.terminal, TerminalRef::Id("-4".to_owned()));
        assert_eq!(c1.end2.terminal, TerminalRef::Other("abc".to_owned()));
        assert_eq!(c1.section, "");
        assert_eq!(d.conductors[2].end1.terminal, TerminalRef::Missing);
        assert_eq!(d.conductors[2].end2.terminal, TerminalRef::Missing);

        let d2 = &p.diagrams[1];
        assert_eq!(
            (d2.index, d2.order, d2.title.as_str(), d2.folio.as_str()),
            (1, None, "Second", "")
        );
        assert!(d2.elements.is_empty() && d2.titleblock.is_empty());
        assert_eq!(d2.border.cols, None);

        // Definitions: nested categories, document order, ranges slice back to the definition.
        let paths: Vec<&str> = p.definitions.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "embed://import/10_electric/a.elmt",
                "embed://import/b.elmt",
                "embed://top.elmt"
            ]
        );
        let first = &p.definitions[0];
        let slice = &xml[first.range.clone()];
        assert!(
            slice.starts_with("<definition") && slice.ends_with("</definition>"),
            "{slice}"
        );
        assert_eq!(first.line, line_of(&xml, "<definition type"));
        let b = &p.definitions[1];
        assert_eq!(&xml[b.range.clone()], "<definition/>");
        assert_eq!(b.line, line_of(&xml, "<definition/>"));
        assert_eq!(
            &xml[p.definitions[2].range.clone()],
            "<definition x=\"1\"></definition>"
        );

        // Findings: the element without definition only.
        assert_eq!(entries.len(), 1, "{entries:?}");
        let w = &entries[0];
        assert_eq!(w.code, Code::MALFORMED_ITEM);
        assert_eq!(w.severity, Severity::Warning);
        assert_eq!(w.message, "collection element without definition");
        let loc = w.location.as_ref().unwrap();
        assert_eq!(loc.item.as_deref(), Some("embed://import/10_electric/empty.elmt"));
        assert_eq!(loc.line, Some(line_of(&xml, "<element name=\"empty.elmt\"")));
    }

    #[test]
    fn bad_position_skips_the_element() {
        let xml = r#"<project>
<diagram folio="3">
<elements>
<element type="t" uuid="{6F1C9A0E-3B7D-4E2F-8A5C-1D0E9B8A7C6F}" x="NaN" y="1"/>
<element type="t" y="1"/>
<element type="t" x="1" y="2"/>
</elements>
</diagram>
</project>"#;
        let (r, entries) = read(xml);
        let d = &r.unwrap().diagrams[0];
        assert_eq!(d.elements.len(), 1);
        assert_eq!(d.elements[0].line, 6);
        assert_eq!(entries.len(), 2);
        for e in &entries {
            assert_eq!(e.code, Code::MALFORMED_ITEM);
            assert_eq!(e.severity, Severity::Error);
            assert_eq!(e.message, "element skipped: bad x/y");
            assert_eq!(e.location.as_ref().unwrap().folio.as_deref(), Some("3"));
        }
        let l0 = entries[0].location.as_ref().unwrap();
        assert_eq!(l0.item.as_deref(), Some(U1));
        assert_eq!(l0.line, Some(4));
        let l1 = entries[1].location.as_ref().unwrap();
        assert_eq!(l1.item.as_deref(), Some("element 1"));
        assert_eq!(l1.line, Some(5));
    }

    #[test]
    fn bad_orientation_is_zero_with_a_warning() {
        let xml = r#"<project><diagram>
<elements>
<element x="1" y="1" orientation="9"/>
<element x="1" y="1" orientation="3"/>
<element x="1" y="1" orientation="-1"/>
<element x="1" y="1" orientation="north"/>
<element x="1" y="1"/>
</elements>
</diagram></project>"#;
        let (r, entries) = read(xml);
        let d = &r.unwrap().diagrams[0];
        let o: Vec<u8> = d.elements.iter().map(|e| e.orientation).collect();
        assert_eq!(o, vec![0, 3, 0, 0, 0]);
        assert_eq!(entries.len(), 3);
        assert!(
            entries
                .iter()
                .all(|e| e.code == Code::MALFORMED_ITEM && e.severity == Severity::Warning)
        );
        assert_eq!(entries[0].message, "invalid orientation 9; 0 used");
        let loc = entries[0].location.as_ref().unwrap();
        assert_eq!(loc.folio, None);
        assert_eq!(loc.item.as_deref(), Some("element 0"));
        assert_eq!(loc.line, Some(3));
    }

    #[test]
    fn unknown_diagram_children_are_reported_once_per_tag() {
        let xml = r#"<project>
<diagram folio="F1">
<wibble/>
<wibble/>
<wobble/>
</diagram>
<diagram>
<wibble/>
</diagram>
</project>"#;
        let (r, entries) = read(xml);
        assert_eq!(r.unwrap().diagrams.len(), 2);
        assert_eq!(entries.len(), 3);
        assert!(
            entries
                .iter()
                .all(|e| e.code == Code::DROPPED_BY_DESIGN && e.severity == Severity::Info)
        );
        assert_eq!(entries[0].message, "unsupported wibble dropped");
        let l0 = entries[0].location.as_ref().unwrap();
        assert_eq!(
            (l0.folio.as_deref(), l0.item.as_deref(), l0.line),
            (Some("F1"), None, Some(3))
        );
        assert_eq!(entries[1].message, "unsupported wobble dropped");
        assert_eq!(entries[1].location.as_ref().unwrap().line, Some(5));
        let l2 = entries[2].location.as_ref().unwrap();
        assert_eq!((l2.folio.as_deref(), l2.line), (None, Some(8)));
    }

    #[test]
    fn bad_segment_length_is_dropped_with_a_warning() {
        let xml = r#"<project><diagram folio="2"><conductors>
<conductor>
<segments>
<segment orientation="horizontal" length="abc"/>
<segment orientation="horizontal" length="5"/>
</segments>
</conductor>
</conductors></diagram></project>"#;
        let (r, entries) = read(xml);
        let c = &r.unwrap().diagrams[0].conductors[0];
        assert_eq!(c.segments.len(), 1);
        assert_eq!(c.segments[0].length, dec("5"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, Code::MALFORMED_ITEM);
        assert_eq!(entries[0].severity, Severity::Warning);
        assert_eq!(entries[0].message, "segment dropped: bad length");
        assert_eq!(entries[0].location.as_ref().unwrap().line, Some(4));
    }

    #[test]
    fn a_definition_root_is_unsupported() {
        let (r, _) = read("<definition type=\"element\"/>");
        assert!(matches!(r, Err(ImportError::Unsupported { .. })), "{r:?}");
    }

    #[test]
    fn a_doctype_is_malformed() {
        let (r, _) = read("<!DOCTYPE project [<!ENTITY a \"b\">]><project/>");
        assert!(matches!(r, Err(ImportError::Malformed { .. })), "{r:?}");
        let (r, _) = read("<project>");
        assert!(matches!(r, Err(ImportError::Malformed { .. })), "{r:?}");
    }

    #[test]
    fn deep_category_nesting_is_walked_without_recursion() {
        let mut xml = String::from("<project><collection>");
        for _ in 0..200 {
            xml.push_str("<category name=\"c\">");
        }
        xml.push_str("<element name=\"e\"><definition/></element>");
        for _ in 0..200 {
            xml.push_str("</category>");
        }
        xml.push_str("</collection></project>");
        let limits = Limits {
            max_xml_depth: 1000,
            ..Limits::default()
        };
        // The parser is recursive: run on the importer's parser stack, as the importer does.
        let depth = crate::xml::prescan(&xml, &limits).unwrap();
        let p = crate::xml::with_parser_stack(depth, || {
            let mut f = Findings::new("t.qet", 100);
            read_project(&xml, &mut f, &limits)
        })
        .unwrap();
        assert_eq!(p.definitions.len(), 1);
        assert!(p.definitions[0].path.starts_with("embed://c/c/"));
        assert!(p.definitions[0].path.ends_with("/c/e"));
    }

    #[test]
    fn overlong_collection_paths_are_skipped() {
        let long = "n".repeat(3000);
        let xml = format!(
            "<project><collection><category name=\"{long}\"><category name=\"{long}\"><element name=\"e\"><definition/></element></category></category></collection></project>"
        );
        let (r, entries) = read(&xml);
        assert!(r.unwrap().definitions.is_empty());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, Code::MALFORMED_ITEM);
    }
}
