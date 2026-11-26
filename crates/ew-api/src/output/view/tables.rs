// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tables of a page's view (see the module documentation of `view`): the report table of a generated page and
//! the embedded report blocks of a drawn one, the cover's tables, a row's links.

#[allow(clippy::wildcard_imports)]
use super::*;

/// `page_table(page)`: the printed table of a generated page; `None` for other pages. Its own memo: it reads
/// `page_order` and the block's rows, so a page-order or row change that keeps this page's printed table is cut
/// off here and does not rebuild its tables layer.
#[derive(Debug)]
pub struct PageTableOf;

impl Query for PageTableOf {
    type Key = PageRef;
    type Value = Option<TableView>;
    const FAMILY: &'static str = "page_table";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &PageRef) -> Arc<Option<TableView>> {
        // A drawn page or a divider has no table. Its record is still read, so that deleting it marks this memo
        // and the store collects it (memo keys of deleted records must not accumulate).
        match *key {
            PageRef::Drawn(p) => {
                let _ = q.pages().get(p);
                return Arc::new(None);
            }
            PageRef::Divider(s) => {
                let _ = q.sections().get(s);
                return Arc::new(None);
            }
            PageRef::Generated { .. } => {}
        }
        let order = q.get::<PageOrder>(());
        Arc::new(order.generated(*key).map(|g| table_view(q, g, *key)))
    }
}

/// `page_embedded(page)`: the embedded blocks of a drawn page, each drawn by the scene into its rectangle (see the
/// module documentation). Its own memo: it reads every report block and the rows of the page's blocks.
#[derive(Debug)]
pub struct PageEmbeddedOf;

impl Query for PageEmbeddedOf {
    type Key = Id<Page>;
    type Value = Vec<EmbeddedView>;
    const FAMILY: &'static str = "page_embedded";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Vec<EmbeddedView>> {
        let page = *key;
        let out = q
            .report_blocks()
            .iter()
            .filter_map(|(_, b)| {
                let embed = b.embedded.as_ref().filter(|e| e.page == page)?;
                let content = if matches!(b.source, ReportSource::Kind(ReportKind::ProfileTables)) {
                    EmbeddedContent::Cover(Box::new(cover_tables(q, b)))
                } else {
                    EmbeddedContent::Table(boxed_table(q, b, page))
                };
                Some(EmbeddedView {
                    rect: embed.rect.0,
                    content,
                })
            })
            .collect();
        Arc::new(out)
    }
}

/// The three tables of a `profile_tables` block as printed texts.
fn cover_tables(q: &Q, block: &ReportBlock) -> CoverTables {
    let set = q.get::<BlockRows>(block.id);
    let opts = RenderOptions {
        lang: ew_derive::print_language(q),
        ..RenderOptions::default()
    };
    let cell = |row: &Row, key: &str| -> String {
        set.column(key)
            .and_then(|i| row.cells.get(i))
            .map(|c| render(q, c, &opts))
            .unwrap_or_default()
    };
    let label = |key: &str| -> String {
        set.column(key)
            .map(|i| ew_derive::text_in(&set.columns[i].label, opts.lang.as_ref()).to_owned())
            .unwrap_or_default()
    };
    let colour_keys = ["voltage_class", "function", "potential_type", "colour"];
    let mut cover = CoverTables::default();
    cover.colours.header = colour_keys.map(label).to_vec();
    for row in &set.rows {
        let text = cell(row, "voltage_class");
        match (cell(row, "table").as_str(), row.kind) {
            ("colours", RowKind::GroupHeader) => cover.colours.title = text,
            ("control", RowKind::GroupHeader) => cover.control.title = text,
            ("mains", RowKind::GroupHeader) => cover.mains.title = text,
            ("colours", RowKind::GroupFooter) => cover.colours.caption = Some(text),
            ("note", RowKind::GroupFooter) => cover.note = Some(text),
            ("colours", RowKind::Data) => cover
                .colours
                .rows
                .push(colour_keys.map(|k| cell(row, k)).to_vec()),
            ("control", RowKind::Data) => cover
                .control
                .rows
                .push(["colour", "size"].map(|k| cell(row, k)).to_vec()),
            ("mains", RowKind::Data) => cover
                .mains
                .rows
                .push(["colour", "size"].map(|k| cell(row, k)).to_vec()),
            _ => {}
        }
    }
    cover
}

/// The boxed table of an embedded block other than the cover, as printed texts on `page` (see the module
/// documentation).
pub(crate) fn boxed_table(q: &Q, block: &ReportBlock, page: Id<Page>) -> EmbeddedTable {
    boxed_table_of(q, block, &q.get::<BlockRows>(block.id), page)
}

/// [`boxed_table`] of `block` with the rows `set` (a block the project does not hold yet: the legend Add legend would
/// make, whose fit it tries).
pub(crate) fn boxed_table_of(q: &Q, block: &ReportBlock, set: &RowSet, page: Id<Page>) -> EmbeddedTable {
    let layout = match &block.source {
        ReportSource::Kind(kind) => Registry::builtin().get(kind).map(|k| k.embedded_layout()),
        _ => None,
    }
    .unwrap_or_else(|| EmbeddedLayout::every_column(&set.columns));
    // A block's own printed columns replace the kind's (Add legend's description, doc 09 §2.2).
    let layout = if block.columns.is_empty() {
        layout
    } else {
        EmbeddedLayout {
            columns: block.columns.iter().map(|c| (c.key.clone(), c.weight)).collect(),
            ..layout
        }
    };
    let opts = RenderOptions {
        page: Some(PageRef::Drawn(page)),
        lang: ew_derive::print_language(q),
        ..RenderOptions::default()
    };
    let in_lang = |m: &MlText| ew_derive::text_in(m, opts.lang.as_ref()).to_owned();
    // The printed columns: the layout's keys that the row set has, in the layout's order.
    let printed: Vec<(usize, u32)> = layout
        .columns
        .iter()
        .filter_map(|(key, weight)| set.column(key).map(|i| (i, *weight)))
        .collect();
    let columns = printed
        .iter()
        .map(|&(i, weight)| TableColumn {
            header: in_lang(&set.columns[i].label),
            weight,
        })
        .collect();
    let title = if block.title.is_empty() {
        in_lang(&layout.title)
    } else {
        in_lang(&block.title)
    };
    let note = match &set.status {
        RowSetStatus::NotDerived(reason) => Some(in_lang(&reason.words())),
        RowSetStatus::Derived => None,
    };
    let rows = if note.is_some() {
        Vec::new()
    } else {
        set.rows
            .iter()
            .map(|r| {
                let kind = match r.kind {
                    RowKind::Data => TableRowKind::Data,
                    RowKind::Heading | RowKind::GroupHeader | RowKind::GroupFooter => TableRowKind::Heading,
                };
                let cells = match kind {
                    // A heading prints its first cell across the row.
                    TableRowKind::Heading => {
                        r.cells.first().map(|c| render(q, c, &opts)).into_iter().collect()
                    }
                    TableRowKind::Data => printed
                        .iter()
                        .map(|&(i, _)| r.cells.get(i).map(|c| render(q, c, &opts)).unwrap_or_default())
                        .collect(),
                };
                // An embedded block draws no back-links: only the connection-point links
                // of a PLC overview box as built (FR-PLC-052), whole-cell, renumbered to their printed position.
                let links = row_links(q, r)
                    .into_iter()
                    .filter(|l| matches!(l.dest, Destination::Cp(_)) && l.span.is_none())
                    .filter_map(|l| {
                        let p = printed.iter().position(|&(i, _)| i == l.column)?;
                        Some(TableLink { column: p, ..l })
                    })
                    .collect();
                TableRow { kind, cells, links }
            })
            .collect()
    };
    // A block with its own printed columns (Add legend's, doc 09 §2.2) is a lookup table (doc 12 §5.4): a column of
    // text (the description) is prose and wraps; of the identifiers the position and the tag give way last, every
    // other first.
    let lookup = (!block.columns.is_empty()).then(|| {
        let rank = |i: usize| match set.columns[i].key.as_str() {
            "pos" => 2,
            "tag" => 1,
            _ => 0,
        };
        let mut order: Vec<usize> = (0..printed.len()).collect();
        order.sort_by_key(|&c| rank(printed[c].0));
        let prose = (0..printed.len())
            .filter(|&c| set.columns[printed[c].0].ty == ew_reports::CellType::Text)
            .collect();
        Lookup { order, prose }
    });
    EmbeddedTable {
        title,
        columns,
        headers: layout.headers,
        flow_columns: layout.flow_columns,
        rows,
        note,
        lookup,
    }
}

/// The drawn row links of a report row on a generated page (FR-REP-042), in the
/// row's link order:
///
/// - `Cp`: the connection point, which the PLC overview sets to the coil of each bound channel (doc 10 §5);
/// - `Page(p)`: the page, when the page order holds it;
/// - `MainRep(d)`: the device's main representation, when it has one;
/// - `Rep(r)`: the representation, when it exists;
/// - `Placement(p)`: the device's footprint on the first panel page that shows its surface; the row is then also where
///   Follow reference from the device lands (`Destination::LegendRow`), on its first printed cell without a link of its own
///   (doc 12 §5.4).
///
/// A link with an `item` covers that page's printed span of a `Folios` cell ([`folios_printed`], the function the
/// printer uses); a page inside a printed run has no span and no link. A link that would not resolve is not drawn:
/// never a dead link.
fn row_links(q: &Q, r: &Row) -> Vec<TableLink> {
    let order = q.get::<PageOrder>(());
    let reps = q.reps();
    let rep = |rid: Id<Representation>| {
        reps.get(rid)
            .filter(|x| order.position(PageRef::Drawn(x.page)).is_some())
            .map(|_| Destination::Representation(rid.as_uuid()))
    };
    let mut spans: BTreeMap<usize, Vec<Option<std::ops::Range<usize>>>> = BTreeMap::new();
    r.links
        .iter()
        .filter_map(|l| {
            let column = usize::from(l.column);
            let dest = match l.target {
                LinkTarget::Cp { function, idx } => {
                    let cp: Id<()> = ew_ids::v5::cp_id(function, u32::from(idx));
                    Some(Destination::Cp(cp.as_uuid()))
                }
                LinkTarget::Page(p) => order.position(p).map(|_| page_destination(p)),
                LinkTarget::MainRep(d) => (*q.get::<MainRep>(d)).and_then(rep),
                LinkTarget::Rep(rid) => rep(rid),
                LinkTarget::Placement(p) => crate::output::panel::placement_page(q, p)
                    .filter(|page| order.position(PageRef::Drawn(*page)).is_some())
                    .map(|_| Destination::Placement(p.as_uuid())),
            }?;
            let span = match l.item {
                None => None,
                Some(n) => {
                    let items = spans.entry(column).or_insert_with(|| match r.cells.get(column) {
                        Some(Cell::Folios(ps)) => folios_printed(q, ps).1,
                        _ => Vec::new(),
                    });
                    Some(items.get(usize::from(n))?.clone()?)
                }
            };
            Some(TableLink {
                column,
                span,
                dest,
                target: false,
            })
        })
        .collect()
}

/// The printed table of a generated page (see the module documentation).
fn table_view(q: &Q, g: &GeneratedPage, page: PageRef) -> TableView {
    let set = q.get::<BlockRows>(g.block);
    let lang = ew_derive::print_language(q);
    let in_lang = |m: &MlText| ew_derive::text_in(m, lang.as_ref()).to_owned();
    let opts = RenderOptions {
        page: Some(page),
        lang: lang.clone(),
        ..RenderOptions::default()
    };
    let block = q.report_blocks().get(g.block);
    // The printed columns (doc 09 §3.8): the block's own, those its rows have, in its order, each with its width and
    // its heading or the kind's; else the columns its kind prints ([`ew_reports::ReportKindDef::printed`]), else every
    // column of the rows.
    let kind_printed = block
        .and_then(|b| match &b.source {
            ReportSource::Kind(kind) => Registry::builtin().get(kind),
            _ => None,
        })
        .and_then(|k| k.printed());
    let printed: Vec<(usize, u32, &MlText)> = match block.filter(|b| !b.columns.is_empty()) {
        Some(b) => b
            .columns
            .iter()
            .filter_map(|c| {
                let i = set.column(&c.key)?;
                let heading = if c.heading.is_empty() {
                    &set.columns[i].label
                } else {
                    &c.heading
                };
                Some((i, c.weight, heading))
            })
            .collect(),
        None => match kind_printed {
            Some(keys) => keys
                .iter()
                .filter_map(|key| {
                    let i = set.column(key)?;
                    Some((i, set.columns[i].weight, &set.columns[i].label))
                })
                .collect(),
            None => set
                .columns
                .iter()
                .enumerate()
                .map(|(i, c)| (i, c.weight, &c.label))
                .collect(),
        },
    };
    let columns = printed
        .iter()
        .map(|&(_, weight, heading)| TableColumn {
            header: in_lang(heading),
            weight,
        })
        .collect();
    let line = |m: Option<&MlText>| m.filter(|m| !m.is_empty()).map(in_lang);
    let caption = line(block.map(|b| &b.caption));
    let legend = line(block.map(|b| &b.legend));
    if let Some(note) = &g.note {
        return TableView {
            columns,
            rows: Vec::new(),
            note: Some(in_lang(note)),
            caption,
            legend,
        };
    }
    let end = (g.rows.end as usize).min(set.rows.len());
    let start = (g.rows.start as usize).min(end);
    let overview =
        block.is_some_and(|b| matches!(b.source, ReportSource::Kind(ReportKind::RevisionOverview)));
    let pages_column = set.column("pages").filter(|_| overview);
    let cell = |r: &Row, i: usize| match pages_column
        .filter(|p| *p == i)
        .and_then(|_| revision_pages(q, r))
    {
        Some(pages) => render(q, &pages, &opts),
        None => r.cells.get(i).map(|c| render(q, c, &opts)).unwrap_or_default(),
    };
    let rows = set.rows[start..end]
        .iter()
        .map(|r| {
            let kind = match r.kind {
                RowKind::Data => TableRowKind::Data,
                RowKind::Heading | RowKind::GroupHeader | RowKind::GroupFooter => TableRowKind::Heading,
            };
            let cells = match kind {
                // A heading prints its first cell across the row.
                TableRowKind::Heading => vec![cell(r, 0)],
                TableRowKind::Data => printed.iter().map(|&(i, _, _)| cell(r, i)).collect(),
            };
            // The links of the printed cells, at their printed column.
            let mut links: Vec<TableLink> = row_links(q, r)
                .into_iter()
                .filter_map(|l| {
                    let column = printed.iter().position(|&(i, _, _)| i == l.column)?;
                    Some(TableLink { column, ..l })
                })
                .collect();
            // A row that links to a placement is where Follow reference from that device lands (doc 12 §5.4): its first
            // printed cell without a link of its own.
            let placement = r.links.iter().find_map(|l| match l.target {
                LinkTarget::Placement(p) => Some(p),
                _ => None,
            });
            let free = (0..printed.len()).find(|c| !links.iter().any(|l| l.column == *c && l.span.is_none()));
            if let (Some(p), Some(column)) = (placement, free) {
                links.push(TableLink {
                    column,
                    span: None,
                    dest: Destination::LegendRow(p.as_uuid()),
                    target: true,
                });
            }
            TableRow { kind, cells, links }
        })
        .collect();
    TableView {
        columns,
        rows,
        note: None,
        caption,
        legend,
    }
}

/// The `pages` cell of a revision overview row at draw time: the revision's `changed_pages`, which
/// sees the page digests and the generated pages the row source inside the pagination cannot
/// (`ew_reports::kinds::revision_rows`); `None` for a row that names no revision.
pub(crate) fn revision_pages(q: &Q, row: &Row) -> Option<Cell> {
    let id = row.id.entities().next()?;
    let rev = q.table::<ew_schema::Revision>().get(Id::from_uuid(id))?;
    Some(Cell::Folios(crate::output::revisions::changed_pages(q, rev)))
}
