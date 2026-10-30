// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project, parties, structure, external references, sections, pages and report blocks (doc 20 §4, §5, §16).

use std::collections::BTreeMap;

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_rds::Aspect;
use ew_schema::{
    AnyId, Bounds, ContinuationPolicy, DateFormat, DecimalSeparator, EarthingSystem, EmbedSpec, ExternalRef,
    Folio, Formatting, MasterData, Open, Page, PageNote, PageType, Party, PartyRole, Project, ProjectKind,
    ProjectSettings, Quantity, Record, ReportBlock, ReportKind, ReportSource, Revision, RevisionState, Scale,
    Section, Set, Slot, StructureAssignment, StructureNode, SupplyOrigin, SupplySpec, Unit,
};
use ew_units::{Nm, Pt2};

use super::{Cx, Fate, PageInfo, fresh, grid_named, parent_fates};
use crate::ids::{record_id, source_ref};
use crate::table::{Rd, Row, Table};

/// `aspect` names of `structure.csv` (the product aspect is never a structure node, doc 03 §5).
const ASPECTS: &[(&str, Aspect)] = &[
    ("functional_assignment", Aspect::FunctionalAssignment),
    ("function", Aspect::Function),
    ("installation_site", Aspect::InstallationSite),
    ("location", Aspect::Location),
    ("document_kind", Aspect::DocumentKind),
    ("user_defined", Aspect::UserDefined),
    ("type", Aspect::Type),
];

const PAGE_KINDS: &[(&str, PageType)] = &[
    ("title_page", PageType::TitlePage),
    ("table_of_contents", PageType::TableOfContents),
    ("section_divider", PageType::SectionDivider),
    ("graphic", PageType::Graphic),
    ("single_line", PageType::SingleLine),
    ("multi_line", PageType::MultiLine),
    ("overview", PageType::Overview),
    ("bus_topology", PageType::BusTopology),
    ("fluid", PageType::Fluid),
    ("assembly_overview", PageType::AssemblyOverview),
    ("panel_layout", PageType::PanelLayout),
    ("model_view", PageType::ModelView),
    ("external_document", PageType::ExternalDocument),
];

const REPORT_KINDS: &[(&str, ReportKind)] = &[
    ("profile_tables", ReportKind::ProfileTables),
    ("part_data", ReportKind::PartData),
    ("toc", ReportKind::Toc),
    ("structure_overview", ReportKind::StructureOverview),
    ("parts_summary", ReportKind::PartsSummary),
    ("parts_list", ReportKind::PartsList),
    ("device_list", ReportKind::DeviceList),
    ("terminal_diagram", ReportKind::TerminalDiagram),
    ("terminal_overview", ReportKind::TerminalOverview),
    ("cable_overview_diagrams", ReportKind::CableOverviewDiagrams),
    ("wire_list", ReportKind::WireList),
    ("plc_overview", ReportKind::PlcOverview),
    ("io_list", ReportKind::IoList),
    ("potential_overview", ReportKind::PotentialOverview),
    ("revision_overview", ReportKind::RevisionOverview),
    ("open_items", ReportKind::OpenItems),
    ("numbering_report", ReportKind::NumberingReport),
    ("enclosure_legend", ReportKind::EnclosureLegend),
    ("check_report", ReportKind::CheckReport),
];

const ROLES: &[(&str, PartyRole)] = &[
    ("customer", PartyRole::Customer),
    ("end_user", PartyRole::EndUser),
    ("panel_builder", PartyRole::PanelBuilder),
    ("author", PartyRole::Author),
    ("checker", PartyRole::Checker),
    ("engineer", PartyRole::Engineer),
];

/// The structure slot of `col` (a `structure` key).
fn slot(cx: &Cx<'_>, r: &mut Rd<'_>, col: &str) -> Option<Slot> {
    let key = r.text(col)?;
    match cx.structure.get(key) {
        Some(id) => Some(Slot::Node(*id)),
        None => {
            r.fail(format!("column `{col}`: unknown structure node `{key}`"));
            None
        }
    }
}

/// The `function` (=) and `location` (+) slots of a page or device row.
pub(crate) fn structure_assignment(cx: &Cx<'_>, r: &mut Rd<'_>) -> StructureAssignment {
    StructureAssignment {
        function: slot(cx, r, "function"),
        location: slot(cx, r, "location"),
        ..StructureAssignment::default()
    }
}

/// `structure.csv` → `structure_node`. Parents are resolved after all rows are known.
pub(crate) fn emit_structure(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    let mut pending = Vec::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &cx.structure) {
            continue;
        }
        let mut r = row.reader();
        let aspect = r.req_choice("aspect", ASPECTS);
        let code = r.req_ident("code");
        let aliases: Vec<_> = r
            .list("aliases")
            .into_iter()
            .filter_map(|a| a.parse().ok())
            .collect();
        let description = r.mltext("description");
        let order = r.uint::<u32>("order").unwrap_or(0);
        let parent = r.text("parent").map(str::to_owned);
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(aspect), Some(code)) = (aspect, code) else {
            continue;
        };
        let id = record_id::<StructureNode>("structure", row.key());
        cx.structure.insert(row.key().to_owned(), id);
        pending.push((
            row,
            parent,
            StructureNode {
                id,
                v: 1,
                aspect: Open::Known(aspect),
                code,
                aliases: Set::from(aliases),
                parent: None,
                description,
                order,
                source_ref: Some(source_ref("structure", row.key())),
                extra: Default::default(),
            },
        ));
    }
    // Parents: resolve, then refuse cycles and drop the children of dropped nodes, in one linear walk
    // (the model rejects a parent cycle, so the batch must not hold one).
    let index: BTreeMap<&str, usize> = pending
        .iter()
        .enumerate()
        .map(|(i, (row, _, _))| (row.key(), i))
        .collect();
    let mut parents: Vec<Option<usize>> = vec![None; pending.len()];
    let mut pre = vec![false; pending.len()];
    for (i, (row, parent, _)) in pending.iter().enumerate() {
        if let Some(p) = parent {
            match index.get(p.as_str()) {
                Some(j) => parents[i] = Some(*j),
                None => {
                    cx.unresolved(*row, &format!("unknown parent `{p}`"));
                    pre[i] = true;
                }
            }
        }
    }
    let fates = parent_fates(&parents, &pre);
    let rows: Vec<Row<'_>> = pending.iter().map(|(row, _, _)| *row).collect();
    cx.report_fates(&rows, &fates, "structure");
    let ids: Vec<Id<StructureNode>> = pending.iter().map(|(_, _, n)| n.id).collect();
    for (i, (row, _, mut node)) in pending.into_iter().enumerate() {
        if fates[i] != Fate::Kept {
            cx.structure.remove(row.key());
            continue;
        }
        node.parent = parents[i].map(|j| ids[j]);
        cx.records.push(Record::StructureNode(node));
    }
}

/// `external_refs.csv` → `external_ref`.
pub(crate) fn emit_external_refs(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &cx.external_refs) {
            continue;
        }
        let mut r = row.reader();
        let location = r.req("location").map(str::to_owned);
        let device = r.req_ident("device");
        let connection = r.ident("connection");
        let project = r.ident("project");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(location), Some(device)) = (location, device) else {
            continue;
        };
        let Some(location) = cx.structure.get(&location).copied() else {
            cx.unresolved(row, &format!("unknown location `{location}`"));
            continue;
        };
        let id = record_id::<ExternalRef>("external_refs", row.key());
        cx.external_refs.insert(row.key().to_owned(), id);
        cx.records.push(Record::ExternalRef(ExternalRef {
            id,
            v: 1,
            project,
            location,
            device,
            connection,
            source_ref: Some(source_ref("external_refs", row.key())),
            extra: Default::default(),
        }));
    }
}

/// `sections.csv` → `section`.
pub(crate) fn emit_sections(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &cx.sections) {
            continue;
        }
        let mut r = row.reader();
        let number = r.req_uint::<u16>("number");
        let from = r.req_uint::<u32>("folio_from");
        let to = r.req_uint::<u32>("folio_to");
        let title = r.mltext("title");
        let divider = r.flag("divider");
        if let (Some(f), Some(t)) = (from, to)
            && f > t
        {
            r.fail("folio_from is above folio_to");
        }
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(number), Some(from), Some(to)) = (number, from, to) else {
            continue;
        };
        let id = record_id::<Section>("sections", row.key());
        cx.sections.insert(row.key().to_owned(), id);
        cx.records.push(Record::Section(Section {
            id,
            v: 1,
            number,
            folio_range: (from, to),
            title,
            divider,
            source_ref: Some(source_ref("sections", row.key())),
            extra: Default::default(),
        }));
    }
}

/// `pages.csv` → `page`.
pub(crate) fn emit_pages(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &cx.pages) {
            continue;
        }
        let mut r = row.reader();
        let number = r.req_uint::<u32>("number");
        let label = r.ident("label");
        let kind = r.req_choice("kind", PAGE_KINDS);
        let title = r.mltext("title");
        let structure = structure_assignment(cx, &mut r);
        let scale = r.uint::<u32>("scale");
        let grid = r.req("grid").and_then(|g| {
            let grid = grid_named(g);
            if grid.is_none() {
                r.fail(format!("column `grid`: `{g}` is not qet10 or mm4"));
            }
            grid
        });
        let section = r.text("section").map(str::to_owned);
        let form = r.req("form").map(str::to_owned);
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(number), Some(kind), Some(grid), Some(form)) = (number, kind, grid, form) else {
            continue;
        };
        let section = match section {
            None => None,
            Some(s) => match cx.sections.get(&s) {
                Some(id) => Some(*id),
                None => {
                    cx.unresolved(row, &format!("unknown section `{s}`"));
                    continue;
                }
            },
        };
        let Some(form) = cx.form_id(&form) else {
            cx.unresolved(row, &format!("unknown form `{form}`"));
            continue;
        };
        let id = record_id::<Page>("pages", row.key());
        cx.pages.insert(row.key().to_owned(), PageInfo { id, grid });
        cx.records.push(Record::Page(Page {
            id,
            v: 1,
            folio: Folio {
                section,
                number,
                label,
                extra: Default::default(),
            },
            kind,
            structure,
            form: Some(form),
            title,
            description_override: None,
            scale: match scale {
                Some(n) if n > 0 => Scale::OneTo(n),
                _ => Scale::NotToScale,
            },
            grid,
            source_ref: Some(source_ref("pages", row.key())),
            extra: Default::default(),
        }));
    }
}

/// `report_blocks.csv` → `report_block`.
pub(crate) fn emit_report_blocks(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    let mut seen = std::collections::BTreeMap::<String, ()>::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let kind = r.req_choice("kind", REPORT_KINDS);
        let form = r.req("form").map(str::to_owned);
        let section = r.text("section").map(str::to_owned);
        let first_folio = r.uint::<u32>("first_folio");
        let title = r.mltext("title");
        let continuation = r
            .choice(
                "continuation",
                &[
                    ("sub_page", ContinuationPolicy::SubPage),
                    ("next_folio", ContinuationPolicy::NextFolio),
                ],
            )
            .unwrap_or_default();
        let include: Vec<String> = r.list("include").into_iter().map(str::to_owned).collect();
        let embedded_page = r.text("embedded_page").map(str::to_owned);
        let rect = r.text("embedded_rect").map(str::to_owned);
        if embedded_page.is_some() != rect.is_some() {
            r.fail("embedded_page and embedded_rect go together");
        }
        if embedded_page.is_none() && first_folio.is_none() {
            r.fail("a page-oriented block needs first_folio");
        }
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(kind), Some(form)) = (kind, form) else {
            continue;
        };
        let Some(form) = cx.form_id(&form) else {
            cx.unresolved(row, &format!("unknown form `{form}`"));
            continue;
        };
        let section = match section {
            None => None,
            Some(s) => match cx.sections.get(&s) {
                Some(id) => Some(*id),
                None => {
                    cx.unresolved(row, &format!("unknown section `{s}`"));
                    continue;
                }
            },
        };
        let mut ids: Vec<AnyId> = Vec::new();
        let mut missing = None;
        for k in &include {
            match cx.devices.get(k) {
                Some(d) => ids.push(Id::from_uuid(d.id.as_uuid())),
                None => missing = Some(k.clone()),
            }
        }
        if let Some(k) = missing {
            cx.unresolved(row, &format!("unknown device `{k}` in include"));
            continue;
        }
        let embedded = match (embedded_page, rect) {
            (Some(p), Some(rect)) => {
                let Some(page) = cx.pages.get(&p).map(|p| p.id) else {
                    cx.unresolved(row, &format!("unknown page `{p}`"));
                    continue;
                };
                match parse_rect(&rect) {
                    Ok(rect) => Some(EmbedSpec {
                        page,
                        rect,
                        extra: Default::default(),
                    }),
                    Err(e) => {
                        cx.malformed(row, &format!("column `embedded_rect`: {e}"));
                        continue;
                    }
                }
            }
            _ => None,
        };
        seen.insert(row.key().to_owned(), ());
        cx.records.push(Record::ReportBlock(ReportBlock {
            id: record_id("report_blocks", row.key()),
            v: 1,
            source: ReportSource::Kind(kind),
            manual: ew_schema::ManualSelection {
                include: Set::from(ids),
                ..Default::default()
            },
            sort: Vec::new(),
            group: None,
            form,
            section,
            first_folio: if embedded.is_some() { None } else { first_folio },
            continuation,
            title,
            empty: Default::default(),
            embedded,
            source_ref: Some(source_ref("report_blocks", row.key())),
            extra: Default::default(),
        }));
    }
}

/// `x0;y0;x1;y1` in nm.
fn parse_rect(s: &str) -> Result<Bounds, String> {
    let v: Vec<i64> = s
        .split(';')
        .map(|p| p.parse::<i64>().map_err(|_| format!("`{p}` is not an integer")))
        .collect::<Result<_, _>>()?;
    let [x0, y0, x1, y1] = v[..] else {
        return Err(format!("`{s}` is not x0;y0;x1;y1"));
    };
    Bounds::new(Pt2::new(Nm(x0), Nm(y0)), Pt2::new(Nm(x1), Nm(y1))).map_err(|e| e.to_string())
}

/// The tokens of a device-numbering format (doc 03 §8.2 step 3).
const FORMAT_TOKENS: &[&str] = &["letter", "counter", "subcounter", "page", "column", "row"];

/// Checks a numbering format: literal text and the tokens of [`FORMAT_TOKENS`] in braces, no product sign
/// (the formatter adds it, doc 03 §8.2).
fn check_format(col: &str, format: &str) -> Result<(), String> {
    if format.starts_with('-') {
        return Err(format!(
            "column `{col}`: a format never contains the product sign `-`"
        ));
    }
    let mut rest = format;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let close = after
            .find('}')
            .ok_or_else(|| format!("column `{col}`: unclosed `{{` in `{format}`"))?;
        let token = &after[..close];
        if !FORMAT_TOKENS.contains(&token) {
            return Err(format!(
                "column `{col}`: unknown token `{{{token}}}` in `{format}`"
            ));
        }
        rest = &after[close + 1..];
    }
    if rest.contains('}') {
        return Err(format!("column `{col}`: stray `}}` in `{format}`"));
    }
    Ok(())
}

/// The `numbering_*` columns → `ProjectSettings.extra["numbering"]` in the doc 03 §5.1 shape (`{default,
/// schemes: [DeviceNumberingScheme]}`, doc 20 §4.4): one scheme `numbering_scheme` (default `page-based`) with
/// format `numbering_format`, the strip rule `{select: {aspect: "strip"}, format: numbering_strip_format}` and
/// `child_separator` when given. `None` when no numbering column is set (the profile's scheme applies).
fn numbering(r: &mut Rd<'_>) -> Option<serde_json::Value> {
    let key = r.ident("numbering_scheme");
    let format = r.text("numbering_format");
    let strip = r.text("numbering_strip_format");
    let separator = r.text("numbering_child_separator");
    if key.is_none() && format.is_none() && strip.is_none() && separator.is_none() {
        return None;
    }
    let Some(format) = format else {
        r.fail("column `numbering_format` is required with the other numbering columns");
        return None;
    };
    for (col, f) in [
        ("numbering_format", Some(format)),
        ("numbering_strip_format", strip),
    ] {
        if let Some(f) = f
            && let Err(e) = check_format(col, f)
        {
            r.fail(e);
        }
    }
    let key = key.map_or_else(|| "page-based".to_owned(), |k| k.to_string());
    let mut scheme = serde_json::Map::new();
    scheme.insert("key".into(), key.clone().into());
    scheme.insert("format".into(), format.into());
    if let Some(strip) = strip {
        scheme.insert(
            "rules".into(),
            serde_json::json!([{"select": {"aspect": "strip"}, "format": strip}]),
        );
    }
    if let Some(sep) = separator {
        scheme.insert("child_separator".into(), sep.into());
    }
    Some(serde_json::json!({"default": key, "schemes": [scheme]}))
}

/// `project.csv` (first row) and `parties.csv` → the `project` record, pushed into the records.
pub(crate) fn emit_project(
    cx: &mut Cx<'_>,
    table: Option<&Table>,
    parties: Option<&Table>,
    profiles: Vec<(String, serde_json::Value)>,
) -> Option<()> {
    let table = table?;
    let mut rows = table.rows();
    let row = rows.next()?;
    for extra in rows {
        cx.malformed(extra, "project.csv holds exactly one row");
    }
    let mut r = row.reader();
    let master = MasterData {
        description: r.mltext("description"),
        long_description: r.mltext("long_description"),
        project_ref: r.ident("project_ref"),
        drawing_id: r.ident("drawing_id"),
        object_id: r.req_ident("key"),
        manufacturing_year: r.uint::<u16>("manufacturing_year"),
        created: r.date("created"),
        edited: r.date("edited"),
        document_title: r.mltext("document_title"),
        extra: Default::default(),
    };
    let supply = match r.uint::<u8>("supply_phases") {
        None => None,
        Some(phases) => {
            let neutral = r.flag("supply_neutral");
            let pe = r.flag("supply_pe");
            let earthing = r.choice(
                "supply_earthing",
                &[
                    ("tn_s", EarthingSystem::TnS),
                    ("tn_c_s", EarthingSystem::TnCS),
                    ("tt", EarthingSystem::Tt),
                    ("it", EarthingSystem::It),
                ],
            );
            let u_nom = r.dec3("supply_u_nom");
            let frequency = r.dec3("supply_frequency");
            if u_nom.is_none() || frequency.is_none() {
                r.fail("a supply needs supply_u_nom and supply_frequency");
            }
            let origin = match r.text("supply_origin") {
                None => None,
                Some(k) => match cx.external_refs.get(k) {
                    Some(id) => Some(SupplyOrigin {
                        source: Some(*id),
                        ..Default::default()
                    }),
                    None => {
                        r.fail(format!(
                            "column `supply_origin`: unknown external reference `{k}`"
                        ));
                        None
                    }
                },
            };
            Some(SupplySpec {
                phases,
                neutral,
                pe,
                earthing,
                u_nom: u_nom.unwrap_or_default(),
                frequency: frequency.unwrap_or_default(),
                origin,
                extra: Default::default(),
            })
        }
    };
    let control_voltage = r.dec3("control_voltage").map(|v| Quantity::new(v, Unit::V));
    let rated_consumption = r.dec3("rated_consumption").map(|v| Quantity::new(v, Unit::A));
    let settings = ProjectSettings {
        identifier_set: r.ident("identifier_set"),
        ip_pairing: None,
        formatting: Formatting {
            date: r.choice(
                "date_format",
                &[
                    ("dd_mm_yyyy", DateFormat::DdMmYyyy),
                    ("iso8601", DateFormat::Iso8601),
                ],
            ),
            decimal_separator: r.choice(
                "decimal_separator",
                &[
                    ("point", DecimalSeparator::Point),
                    ("comma", DecimalSeparator::Comma),
                ],
            ),
            tag_case: None,
            extra: Default::default(),
        },
        // Importers write no text styles: the golden project prints with the built-in table (ADR-0032 decision 3).
        text_styles: Default::default(),
        extra: numbering(&mut r)
            .map(|n| ("numbering".to_owned(), n))
            .into_iter()
            .chain(profiles)
            .collect(),
    };
    let derivation_profile = r.req_ident("derivation_profile");
    if let Err(e) = r.finish() {
        cx.malformed(row, &e);
        return None;
    }
    let derivation_profile = derivation_profile?;

    let mut party_list = Vec::new();
    if let Some(parties) = parties {
        let mut seen = std::collections::BTreeMap::<String, ()>::new();
        for row in parties.rows() {
            if !fresh(&mut cx.findings, row, &seen) {
                continue;
            }
            let mut r = row.reader();
            let role = r.req_choice("role", ROLES);
            let name = r.mltext("name");
            let street = r.text("street").map(str::to_owned);
            let postcode = r.text("postcode").map(str::to_owned);
            let city = r.text("city").map(str::to_owned);
            let country = r.text("country").map(str::to_owned);
            let personal_data = r.flag("personal_data");
            if let Err(e) = r.finish() {
                cx.malformed(row, &e);
                continue;
            }
            let Some(role) = role else { continue };
            seen.insert(row.key().to_owned(), ());
            let address = (street.is_some() || postcode.is_some() || city.is_some() || country.is_some())
                .then(|| ew_schema::Address {
                    street,
                    postcode,
                    city,
                    country,
                    extra: Default::default(),
                });
            party_list.push(Party {
                role,
                name,
                address,
                personal_data,
                extra: Default::default(),
            });
        }
    }

    cx.records.push(Record::Project(Project {
        id: cx.project,
        v: 1,
        master,
        parties: party_list,
        supply,
        control_voltage,
        rated_consumption,
        settings,
        languages: None,
        derivation_profile,
        kind: ProjectKind::Schematic,
        source_ref: Some(source_ref("project", row.key())),
        basic_project: None,
        extra: Default::default(),
    }));
    Some(())
}

/// `revisions.csv` → `revision` records (doc 03 §18, typed since FT-165).
///
/// `key` is the label and `state` one of `open`, `released`, `withdrawn`. `author` and `checker` name a party
/// of `parties.csv` by its key or its name; the record keeps the party's role, the only party reference form
/// doc 03 §18 allows so far. Every page of `changed_pages` becomes a page note.
pub(crate) fn emit_revisions(cx: &mut Cx<'_>, table: Option<&Table>, parties: Option<&Table>) {
    let Some(table) = table else { return };
    // Party reference (key or name) → role; the first party of a name wins.
    let mut roles: BTreeMap<String, &'static str> = BTreeMap::new();
    for row in parties.into_iter().flat_map(Table::rows) {
        let Some(role) = row
            .get("role")
            .and_then(|r| ROLES.iter().find(|(n, _)| *n == r))
            .map(|(n, _)| *n)
        else {
            continue;
        };
        roles.entry(row.key().to_owned()).or_insert(role);
        if let Some(name) = row.get("name") {
            roles.entry(name.to_owned()).or_insert(role);
        }
    }
    let mut seen = BTreeMap::<String, ()>::new();
    let mut open_seen = false;
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let date = r.date("date");
        let description = r.mltext("description");
        let state = r.req_choice(
            "state",
            &[
                ("open", "open"),
                ("released", "released"),
                ("withdrawn", "withdrawn"),
            ],
        );
        let party = |r: &mut Rd<'_>, col: &str| match r.text(col) {
            None => None,
            Some(p) => match roles.get(p) {
                Some(role) => Some(*role),
                None => {
                    r.fail(format!("column `{col}`: unknown party `{p}`"));
                    None
                }
            },
        };
        let author = party(&mut r, "author");
        let checker = party(&mut r, "checker");
        if author.is_some() && author == checker {
            r.fail(
                "columns `author` and `checker`: the same party; the checker is another party (EW-INV-018)",
            );
        }
        if state == Some("open") && open_seen {
            r.fail("column `state`: a second open revision; at most one is open (EW-INV-016)");
        }
        let mut notes: Vec<PageNote> = Vec::new();
        for page in r.list("changed_pages") {
            match cx.pages.get(page) {
                Some(info) if notes.iter().any(|n| n.page == info.id) => {
                    r.fail(format!(
                        "column `changed_pages`: page `{page}` listed twice (EW-INV-019)"
                    ));
                }
                Some(info) => notes.push(PageNote {
                    page: info.id,
                    description: MlText::default(),
                    extra: Default::default(),
                }),
                None => r.fail(format!("column `changed_pages`: unknown page `{page}`")),
            }
        }
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let Some(state) = state else { continue };
        let Ok(label) = Ident::from_input(row.key()) else {
            cx.malformed(row, "column `key`: not a revision label");
            continue;
        };
        seen.insert(row.key().to_owned(), ());
        open_seen |= state == "open";
        let role = |name: Option<&str>| name.and_then(|n| serde_json::from_value::<PartyRole>(n.into()).ok());
        cx.records.push(Record::Revision(Revision {
            id: record_id("revisions", row.key()),
            v: 1,
            label,
            date,
            author: role(author),
            checker: role(checker),
            description,
            state: match state {
                "open" => RevisionState::Open,
                "released" => RevisionState::Released,
                _ => RevisionState::Withdrawn,
            },
            page_notes: notes,
            withdraws: None,
            source_ref: Some(source_ref("revisions", row.key())),
            extra: Default::default(),
        }));
    }
}
