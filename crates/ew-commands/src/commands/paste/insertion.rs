// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The insertion engine of `edit.paste`, `macro.insert` and `page.copy` (effects 1 to 7 of `PasteFragment`).

use super::*;

/// The SHA-256 a held library copy row claims.
pub(super) fn held_sha(row: &ew_model::AnyRow) -> Option<ew_schema::Sha256> {
    use ew_model::AnyRow;
    match row {
        AnyRow::Part(c) => Some(c.sha256),
        AnyRow::Symbol(c) => Some(c.sha256),
        AnyRow::Fndef(c) => Some(c.sha256),
        AnyRow::Form(c) => Some(c.sha256),
        AnyRow::Manufacturer(c) => Some(c.sha256),
        AnyRow::PlcProfile(c) => Some(c.sha256),
        _ => None,
    }
}

/// How one reference of a fragment record resolves.
pub(super) enum Resolved {
    /// It names this ID now.
    To(Uuid),
    /// It stays as it is.
    Keep,
    /// It names nothing; the notice says why when it is cleared.
    Nothing(Option<Text>),
}

/// Library copies by (kind, copy ID).
pub(super) type CopySet = BTreeSet<(Kind, Uuid)>;

/// What an insertion learned about the fragment before it writes anything.
pub(super) struct Plan {
    /// The new ID of every fragment record, by (kind, source ID).
    ids: BTreeMap<(Kind, Uuid), Uuid>,
    /// The library copies the insertion may name: held ones and those it embeds, by (kind, copy ID).
    copies: BTreeSet<(Kind, Uuid)>,
    /// The rebound node of every fragment node (`None`: not in this project), with its printed path.
    nodes: BTreeMap<Uuid, (Option<Uuid>, String)>,
    /// Whether the fragment comes from the target project.
    same_project: bool,
}

/// A fragment record to insert: its index in [`Insertion::all`], its kind and its source ID.
pub(super) type RecordAt = (usize, Kind, Uuid);

/// How the insertion engine gives the records their new IDs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NewIds {
    /// `edit.paste` and `page.copy`: a minted UUIDv7 per record, in (kind rank, fragment order), and a PLC channel
    /// `plc_channel_id(new module, number)`.
    Mint,
    /// `macro.insert`: `v5(instance, local ID)`, and a PLC channel `plc_channel_id(new module,
    /// number)` (ADR-0004 amendment 2), so a replay reproduces every ID.
    Instance(Uuid),
}

impl NewIds {
    /// The ID of a non-channel record of local ID `local`.
    fn of(self, tx: &mut TxBuilder<'_>, local: Uuid) -> Uuid {
        match self {
            NewIds::Mint => tx.mint::<()>().as_uuid(),
            NewIds::Instance(instance) => ew_ids::v5::instance_member_id(instance, local),
        }
    }
}

/// The insertion engine of `edit.paste`, `macro.insert` and `page.copy`: the effects 1 to 7 of
/// [`PasteFragment`] on one fragment, with the differences a macro and a copy of pages need.
pub(crate) struct Insertion<'f> {
    /// The command, for its errors.
    pub(crate) name: &'static str,
    /// The JSON Pointer of the fragment in the command's parameters (`/fragment`), for the errors.
    pub(crate) prefix: &'static str,
    /// The target page, or for a fragment with page records the page after which the new pages go.
    pub(crate) page: Id<Page>,
    /// The new position of the fragment's anchor (ignored for a fragment with page records).
    pub(crate) at: Pt2,
    /// The orientation about the anchor, applied before the move to `at` (`macro.insert`); `R0` for a paste
    /// and for a fragment with page records.
    pub(crate) orient: Orient2,
    /// The fragment.
    pub(crate) fragment: &'f Fragment,
    /// The record kinds it may hold.
    pub(crate) kinds: &'static [Kind],
    /// Their refusal text: `is not pasted (only devices, …)`.
    pub(crate) kinds_refusal: Text,
    /// How the records get their IDs.
    pub(crate) ids: NewIds,
    /// The held copies of another version the insertion reuses instead of refusing (`edit.paste`'s `adapt`; empty
    /// for a macro).
    pub(crate) adapt: &'f [Id<()>],
    /// Whether frozen tags stay (a macro clears them when it is saved, unless marked keep).
    pub(crate) keep_frozen: bool,
    /// The devices (source IDs) whose tag counter stays (set by a macro variable).
    pub(crate) keep_counter: BTreeSet<Uuid>,
    /// Project records a reference to which stays when the project holds them (the devices a macro's `device_ref`
    /// variables name); any other reference outside the fragment resolves as effect 3 says.
    pub(crate) keep_refs: BTreeSet<Uuid>,
    /// Whether a device the project still holds comes back itself where the fragment's symbols drew it and none of
    /// them is drawn any more ([`Insertion::unplaced_devices`]: a cut left it so), its functions and a strip it
    /// belongs to with it (`edit.paste`); a macro and a copy of pages make every device anew.
    pub(crate) keep_unplaced: bool,
    /// The folio of each page record by its source ID (`page.copy` with an offset); `None` proposes the folios after
    /// [`Insertion::page`] (a page macro, `page.copy` with `after`).
    pub(crate) folios: Option<&'f BTreeMap<Uuid, ew_schema::Folio>>,
    /// Whether the fragment is bounded by [`PASTE_MAX_RECORDS`]: clipboard text and a library's macro are untrusted
    /// input; a copy of the project's own pages is as large as the pages are.
    pub(crate) bounded: bool,
}

/// What an insertion wrote.
#[derive(Default)]
pub(crate) struct Inserted {
    /// Every inserted record: its local ID and its new ID, in insertion order.
    pub(crate) members: Vec<(Uuid, Uuid)>,
    /// The first inserted device.
    pub(crate) first_device: Option<ew_model::AnyKey>,
    /// The first inserted representation.
    pub(crate) first_rep: Option<ew_model::AnyKey>,
    /// The new pages, in insertion order.
    pub(crate) pages: Vec<Id<Page>>,
    /// The notices.
    pub(crate) notices: Vec<Text>,
}

impl Insertion<'_> {
    fn invalid(&self, at: impl Into<String>, message: Text) -> CmdError {
        CmdError::invalid_params(self.name, at, message)
    }

    /// The fragment's records, then its page records.
    fn all(&self) -> impl Iterator<Item = &Value> {
        self.fragment.records.iter().chain(&self.fragment.pages)
    }

    /// The JSON Pointer of record `i` of [`Insertion::all`].
    fn pointer(&self, i: usize) -> String {
        let n = self.fragment.records.len();
        if i < n {
            format!("{}/records/{i}", self.prefix)
        } else {
            format!("{}/pages/{}", self.prefix, i - n)
        }
    }

    /// Record `i` of [`Insertion::all`].
    fn record_value(&self, i: usize) -> &Value {
        let n = self.fragment.records.len();
        if i < n {
            &self.fragment.records[i]
        } else {
            &self.fragment.pages[i - n]
        }
    }

    /// Whether the fragment brings its own pages (a page macro).
    fn has_pages(&self) -> bool {
        !self.fragment.pages.is_empty()
    }

    /// The target of one reference of a record (see [`Resolved`]).
    fn resolve(&self, tx: &TxBuilder<'_>, plan: &Plan, decl: &LinkDecl, old: Uuid) -> Resolved {
        if let Some(new) = plan.ids.get(&(decl.to, old)) {
            return Resolved::To(*new);
        }
        if self.keep_refs.contains(&old) && tx.row(&(decl.to, Id::from_uuid(old))).is_some() {
            return Resolved::Keep;
        }
        match decl.to {
            Kind::Page => Resolved::To(self.page.as_uuid()),
            Kind::Part | Kind::Symbol | Kind::Fndef | Kind::Form => {
                if plan.copies.contains(&(decl.to, old)) || tx.row(&(decl.to, Id::from_uuid(old))).is_some() {
                    Resolved::Keep
                } else {
                    Resolved::Nothing(None)
                }
            }
            Kind::StructureNode => match plan.nodes.get(&old) {
                Some((Some(node), _)) => Resolved::To(*node),
                Some((None, path)) => {
                    Resolved::Nothing(Some(text!("notice-paste-structure-gone", path.to_string())))
                }
                None => Resolved::Nothing(None),
            },
            Kind::ExternalRef
                if plan.same_project && tx.row(&(Kind::ExternalRef, Id::from_uuid(old))).is_some() =>
            {
                Resolved::Keep
            }
            _ => Resolved::Nothing(None),
        }
    }

    /// Effect 5 by grid steps (turned about the anchor first for a macro inserted with an orientation): the
    /// representations, junctions, page items and structure boxes move; routes follow their ends.
    fn move_by_steps(
        &self,
        tx: &TxBuilder<'_>,
        typed: &mut [(usize, Uuid, Record)],
        grid: Grid,
        steps: [i64; 2],
    ) -> Result<(), CmdError> {
        let too_far = |i: usize| self.invalid(self.pointer(i), text!("refusal-paste-out-of-range"));
        let motion = if self.has_pages() {
            Motion::steps(grid, steps)
        } else {
            Motion::turned(grid, steps, self.orient, self.fragment.anchor, self.at)
        };
        let mut moved_reps: Vec<(Representation, Representation)> = Vec::new();
        let mut moved_junctions: BTreeMap<Id<Junction>, (Pt2, Pt2)> = BTreeMap::new();
        for (i, _, record) in typed.iter_mut() {
            match record {
                Record::Representation(r) => {
                    let before = r.clone();
                    // A fitted symbol (its position between grid points) steps by its first pin.
                    let anchor = crate::geometry::grid_anchor(tx, r, grid)
                        .filter(|a| *a == r.pos || !motion.turns())
                        .ok_or_else(|| self.invalid(self.pointer(*i), text!("refusal-paste-rep-off-grid")))?;
                    let to = motion.point(anchor).map_err(|_| too_far(*i))?;
                    r.pos = sub(to, anchor)
                        .and_then(|d| crate::geometry::add(r.pos, d))
                        .map_err(|_| too_far(*i))?;
                    r.orient = motion.orient(r.orient);
                    if rep_cps_checked(tx, r).is_err() {
                        return Err(too_far(*i));
                    }
                    moved_reps.push((before, r.clone()));
                }
                Record::Junction(j) => {
                    let before = j.at;
                    j.at = motion.point(j.at).map_err(|_| too_far(*i))?;
                    moved_junctions.insert(j.id, (before, j.at));
                }
                Record::PageItem(item) => match &mut item.kind {
                    PageItemKind::Graphic(g) => {
                        motion.shape(&mut g.shape).map_err(|_| too_far(*i))?;
                    }
                    PageItemKind::FreeText(t) => {
                        t.at = motion.point(t.at).map_err(|_| too_far(*i))?;
                        t.angle = motion.angle(t.angle);
                    }
                    PageItemKind::WireMarker(m) => {
                        m.at = motion.point(m.at).map_err(|_| too_far(*i))?;
                        // a marker lies along its wire: 0 or 90°
                        m.angle = Angle2(motion.angle(m.angle).0 % 180_000);
                    }
                    PageItemKind::Image(_)
                    | PageItemKind::SurfaceView(_)
                    | PageItemKind::Underlay(_)
                    | PageItemKind::Unknown(_) => {
                        unreachable!(
                            "images, surface views, underlays and unknown page item kinds are left out"
                        )
                    }
                },
                Record::StructureBox(b) => {
                    for p in &mut b.outline {
                        *p = motion.point(*p).map_err(|_| too_far(*i))?;
                    }
                }
                _ => {}
            }
        }
        // The displacement and old point of a connection's end, drawn by a pasted representation or a pasted junction.
        let end_of = |tx: &TxBuilder<'_>, e: &Endpoint| -> Option<(Pt2, Pt2)> {
            match e {
                Endpoint::Cp(f, idx) => moved_reps.iter().find_map(|(old, new)| {
                    let find = |r: &Representation| {
                        crate::geometry::rep_cps(tx, r)
                            .into_iter()
                            .find(|c| c.function == *f && c.idx == *idx)
                            .map(|c| c.pos)
                    };
                    let (a, b) = (find(old)?, find(new)?);
                    Some((sub(b, a).ok()?, a))
                }),
                Endpoint::Junction(j) => {
                    let (a, b) = moved_junctions.get(j)?;
                    Some((sub(*b, *a).ok()?, *a))
                }
                Endpoint::Unknown(_) => None,
            }
        };
        for (i, _, record) in typed.iter_mut() {
            let Record::Connection(c) = record else { continue };
            if c.route.is_empty() {
                continue;
            }
            let first = c.route[0];
            let last = c.route[c.route.len() - 1];
            if motion.turns() {
                // Every vertex turns with the fragment; an end at its CP's or junction's old point then follows the
                // CP's new point (the two can part by the rounding of a fractional grid), keeping the axes.
                let turned: Result<Vec<Pt2>, OutOfRange> = c.route.iter().map(|p| motion.point(*p)).collect();
                let mut route = turned.map_err(|_| too_far(*i))?;
                let n = route.len();
                let follow = |e: &Endpoint, old: Pt2, now: Pt2| -> Result<Option<Pt2>, OutOfRange> {
                    match end_of(tx, e) {
                        Some((d, at)) if at == old => {
                            let d = sub(crate::geometry::add(at, d)?, now)?;
                            Ok((d != Pt2::ORIGIN).then_some(d))
                        }
                        _ => Ok(None),
                    }
                };
                let da = follow(&c.a, first, route[0]).map_err(|_| too_far(*i))?;
                let db = follow(&c.b, last, route[n - 1]).map_err(|_| too_far(*i))?;
                crate::geometry::follow_ends(&mut route, grid, da, db).map_err(|_| too_far(*i))?;
                c.route = route;
                continue;
            }
            let own = |p: Pt2| -> Result<(Pt2, Pt2), OutOfRange> { Ok((sub(motion.point(p)?, p)?, p)) };
            let a = match end_of(tx, &c.a) {
                Some(a) => a,
                None => own(first).map_err(|_| too_far(*i))?,
            };
            let b = match end_of(tx, &c.b) {
                Some(b) => b,
                None => own(last).map_err(|_| too_far(*i))?,
            };
            step_route(&mut c.route, grid, steps, a, b).map_err(|_| too_far(*i))?;
        }
        Ok(())
    }

    /// Checks the page, the grid, the anchor and the point; the page, the grid, the steps `Δk` and, for a fragment of
    /// another grid that is fitted to the page's (effect 5), the fragment's grid. A fragment with page records keeps
    /// its positions (no steps) on pages of its own grid after the page `page`.
    fn place(&self, tx: &TxBuilder<'_>) -> Result<Placed, CmdError> {
        let Some(page) = tx.get::<Page>(self.page) else {
            return Err(self.invalid("/page", text!("refusal-params-page-gone")));
        };
        if self.has_pages() {
            let [num, den] = self.fragment.grid;
            let grid = Grid::new(num, den).map_err(|_| {
                self.invalid(format!("{}/grid", self.prefix), text!("refusal-paste-grid-form"))
            })?;
            return Ok((page, grid, [0, 0], None));
        }
        match &page.kind {
            PageType::Report | PageType::TableOfContents | PageType::SectionDivider => {
                return Err(self.invalid("/page", text!("refusal-paste-generated-page")));
            }
            kind if allowed_rep_types(kind).is_some_and(<[_]>::is_empty) => {
                return Err(self.invalid("/page", text!("refusal-paste-page-type")));
            }
            _ => {}
        }
        let grid = page.grid;
        // A fragment of another grid is fitted (effect 5); a macro inserted turned is not.
        let fit = self.fragment.grid != [grid.num(), grid.den()];
        if fit && !self.orient.is_default() {
            let size = if grid.den() == 1 {
                format!("{} nm", grid.num())
            } else {
                format!("{}/{} nm", grid.num(), grid.den())
            };
            return Err(self.invalid(
                format!("{}/grid", self.prefix),
                text!(
                    "refusal-paste-grid",
                    self.fragment.grid[0].to_string(),
                    self.fragment.grid[1].to_string(),
                    size.to_string()
                ),
            ));
        }
        if !point_in_range(self.at) || !point_on_grid(grid, self.at) {
            return Err(self.invalid(
                "/at",
                text!(
                    "refusal-paste-off-grid",
                    crate::geometry::off_grid(tx, text!("refusal-paste-the-paste-point"), self.page, self.at)
                ),
            ));
        }
        let anchor = self.fragment.anchor;
        let source = if fit {
            let [num, den] = self.fragment.grid;
            Some(Grid::new(num, den).map_err(|_| {
                self.invalid(format!("{}/grid", self.prefix), text!("refusal-paste-grid-form"))
            })?)
        } else {
            None
        };
        if !point_in_range(anchor) || !point_on_grid(source.unwrap_or(grid), anchor) {
            return Err(self.invalid(
                format!("{}/anchor", self.prefix),
                text!("refusal-paste-anchor-off-grid"),
            ));
        }
        if source.is_some() {
            return Ok((page, grid, [0, 0], source));
        }
        let steps = [
            grid.snap(self.at.x).checked_sub(grid.snap(anchor.x)),
            grid.snap(self.at.y).checked_sub(grid.snap(anchor.y)),
        ];
        let [Some(kx), Some(ky)] = steps else {
            return Err(self.invalid("/at", text!("refusal-paste-far-from-anchor")));
        };
        Ok((page, grid, [kx, ky], None))
    }

    /// Reads the records: kind, schema version, distinct IDs. Returns each record's index in [`Insertion::all`],
    /// kind and source ID; a page item of a kind this version does not know is left out here (before any ID is
    /// minted), with its notice.
    fn records(&self) -> Result<(Vec<RecordAt>, Vec<Text>), CmdError> {
        let records = &self.fragment.records;
        if records.is_empty() && !self.has_pages() {
            return Err(self.invalid(format!("{}/records", self.prefix), text!("refusal-paste-empty")));
        }
        let total = records.len() + self.fragment.pages.len();
        if self.bounded && total > PASTE_MAX_RECORDS {
            return Err(self.invalid(
                format!("{}/records", self.prefix),
                text!(
                    "refusal-paste-too-many",
                    total.to_string(),
                    PASTE_MAX_RECORDS.to_string()
                ),
            ));
        }
        let n = records.len();
        let mut out = Vec::with_capacity(total);
        let mut notices = Vec::new();
        let mut seen = BTreeSet::new();
        for (i, v) in self.all().enumerate() {
            let at = self.pointer(i);
            let record = Record::from_value(v.clone()).map_err(|e| {
                self.invalid(
                    &at,
                    text!("refusal-params-detail", crate::error::schema_error_text(&e)),
                )
            })?;
            let kind = if i < n {
                record.kind().filter(|k| self.kinds.contains(k)).ok_or_else(|| {
                    self.invalid(
                        &at,
                        text!(
                            "refusal-paste-kind",
                            record.kind_name().unwrap_or("?"),
                            self.kinds_refusal.clone()
                        ),
                    )
                })?
            } else {
                match &record {
                    Record::Page(p) => {
                        if matches!(
                            p.kind,
                            PageType::Report | PageType::TableOfContents | PageType::SectionDivider
                        ) {
                            return Err(self.invalid(&at, text!("refusal-paste-generated-page-inserted")));
                        }
                        if p.grid.num() != self.fragment.grid[0] || p.grid.den() != self.fragment.grid[1] {
                            return Err(self.invalid(&at, text!("refusal-paste-grid-differs")));
                        }
                        Kind::Page
                    }
                    _ => return Err(self.invalid(&at, text!("refusal-paste-page-record-kind"))),
                }
            };
            let version = v.get("$v").and_then(Value::as_u64).unwrap_or(1);
            if version != u64::from(kind.schema_version()) {
                return Err(self.invalid(
                    &at,
                    text!(
                        "refusal-paste-schema-version",
                        version.to_string(),
                        kind.schema_version().to_string()
                    ),
                ));
            }
            let id = record.id().expect("an implemented kind has an ID");
            if !seen.insert(id) {
                return Err(self.invalid(&at, text!("refusal-params-record-twice")));
            }
            if let Record::PageItem(item) = &record {
                if matches!(item.kind, PageItemKind::Unknown(_)) {
                    notices.push(text!("notice-paste-unknown-item"));
                    continue;
                }
                if matches!(item.kind, PageItemKind::Image(_)) {
                    notices.push(text!("notice-paste-image"));
                    continue;
                }
                if matches!(item.kind, PageItemKind::SurfaceView(_)) {
                    notices.push(text!("notice-paste-surface-view"));
                    continue;
                }
                if matches!(item.kind, PageItemKind::Underlay(_)) {
                    notices.push(text!("notice-paste-underlay"));
                    continue;
                }
            }
            out.push((i, kind, id));
        }
        Ok((out, notices))
    }

    /// Checks the library copies (effect 1): the held set and the documents to embed.
    fn copies(&self, tx: &TxBuilder<'_>) -> Result<(CopySet, Vec<LibDocument>), CmdError> {
        let project = tx.project().id.as_uuid();
        let mut usable = BTreeSet::new();
        let mut embed = Vec::new();
        let mut adapted: BTreeSet<Uuid> = BTreeSet::new();
        for (i, a) in self.adapt.iter().enumerate() {
            if !adapted.insert(a.as_uuid()) {
                return Err(self.invalid(format!("/adapt/{i}"), text!("refusal-params-copy-twice")));
            }
        }
        let mut used: BTreeSet<Uuid> = BTreeSet::new();
        for (i, value) in self.fragment.copies.iter().enumerate() {
            let at = format!("{}/copies/{i}", self.prefix);
            let doc = LibDocument::from_value(value.clone()).map_err(|e| {
                self.invalid(
                    &at,
                    text!("refusal-params-detail", crate::error::schema_error_text(&e)),
                )
            })?;
            let (id, claimed): (Id<()>, ew_schema::Sha256) = match &doc {
                LibDocument::Part(c) => (c.id.cast(), c.sha256),
                LibDocument::Symbol(c) => (c.id.cast(), c.sha256),
                LibDocument::Fndef(c) => (c.id.cast(), c.sha256),
                LibDocument::Form(c) => (c.id.cast(), c.sha256),
                // A part's closure holds its manufacturer.
                LibDocument::Manufacturer(c) => (c.id.cast(), c.sha256),
                // a fragment carries the closure of its records (doc 07 §3.5), which names no macro and no
                // address profile
                LibDocument::Macro(_) | LibDocument::PlcProfile(_) | LibDocument::Unknown(_) => {
                    return Err(self.invalid(&at, text!("refusal-paste-library-kind")));
                }
            };
            let kind = doc.kind().expect("an implemented library kind");
            if !usable.insert((kind, id.as_uuid())) {
                return Err(self.invalid(&at, text!("refusal-params-copy-twice")));
            }
            match tx.row(&(kind, id)) {
                // A held copy of the same version is reused as it is (also a project-local item of this project).
                Some(row) => {
                    if held_sha(&row) != Some(claimed) && adapted.contains(&id.as_uuid()) {
                        // D22: the held version serves the pasted records unless a CP of theirs disappears.
                        let held = row_document(&row).expect("a held library copy");
                        let diff = classify_update(&doc, &held);
                        let index = self.adapt.iter().position(|a| *a == id).expect("listed in adapt");
                        if diff.class == ChangeClass::Major {
                            return Err(self.invalid(
                                format!("/adapt/{index}"),
                                text!(
                                    "refusal-paste-adapt-major",
                                    crate::words::document_item(tx, &doc),
                                    crate::words::comma_list(
                                        diff.major_reasons.into_iter().map(Into::into).collect()
                                    )
                                ),
                            ));
                        }
                        used.insert(id.as_uuid());
                    } else if held_sha(&row) != Some(claimed) {
                        return Err(self.invalid(
                            &at,
                            text!(
                                "refusal-paste-held-other-version",
                                crate::words::document_item(tx, &doc)
                            ),
                        ));
                    }
                }
                // Any other is embedded exactly as `library.embed` would embed it.
                None => embed.push(check_copy(self.name, &at, value, project)?.0),
            }
        }
        if let Some(i) = self.adapt.iter().position(|a| !used.contains(&a.as_uuid())) {
            return Err(self.invalid(format!("/adapt/{i}"), text!("refusal-paste-adapt-no-copy")));
        }
        Ok((usable, embed))
    }

    /// Rebinds the structure nodes (effect 2).
    fn nodes(&self, tx: &TxBuilder<'_>) -> Result<BTreeMap<Uuid, (Option<Uuid>, String)>, CmdError> {
        let all = tx.iter::<StructureNode>();
        let by_id: BTreeMap<Id<StructureNode>, &StructureNode> = all.iter().map(|n| (n.id, &**n)).collect();
        let aspect_of = |n: &StructureNode| {
            serde_json::to_value(&n.aspect)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        };
        let path_of = |n: &StructureNode| {
            let mut codes = vec![n.code.as_str().to_owned()];
            let mut cur = n.parent;
            while let Some(p) = cur.and_then(|p| by_id.get(&p)) {
                if codes.len() > by_id.len() {
                    break;
                }
                codes.push(p.code.as_str().to_owned());
                cur = p.parent;
            }
            codes.reverse();
            codes
        };
        let mut out = BTreeMap::new();
        for (i, n) in self.fragment.nodes.iter().enumerate() {
            let at = format!("{}/nodes/{i}", self.prefix);
            if n.path.is_empty() {
                return Err(self.invalid(at, text!("refusal-paste-node-no-path")));
            }
            let leaf = n.path.last().map(String::as_str).unwrap_or_default();
            let same: Vec<&StructureNode> = by_id
                .values()
                .copied()
                .filter(|t| aspect_of(t) == n.aspect)
                .collect();
            let target = same
                .iter()
                .find(|t| path_of(t) == n.path)
                .or_else(|| {
                    let mut by_alias = same
                        .iter()
                        .filter(|t| t.aliases.iter().any(|a| a.as_str() == leaf));
                    match (by_alias.next(), by_alias.next()) {
                        (Some(one), None) => Some(one),
                        _ => None,
                    }
                })
                .map(|t| t.id.as_uuid());
            let printed = format!("{} {}", n.aspect, n.path.join("."));
            if out.insert(n.id.as_uuid(), (target, printed)).is_some() {
                return Err(self.invalid(at, text!("refusal-params-node-twice")));
            }
        }
        Ok(out)
    }

    /// The devices of the fragment that a cut left unplaced, with their functions, by (kind, source ID) (doc 06 §3.6):
    /// a device the project holds with every function the fragment gives it, some of them drawn by the fragment's
    /// symbols and none of those drawn by a symbol of the project now. The strip of such a terminal comes along with
    /// its functions, so that a terminal comes back to its own strip.
    fn unplaced_devices(&self, tx: &TxBuilder<'_>, keys: &[RecordAt]) -> BTreeSet<(Kind, Uuid)> {
        let typed = |kind: Kind| {
            keys.iter()
                .filter(move |k| k.1 == kind)
                .filter_map(|k| Record::from_value(self.record_value(k.0).clone()).ok())
        };
        // The fragment's functions by their device, and the functions its symbols draw.
        let mut functions: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
        for r in typed(Kind::Function) {
            if let Record::Function(f) = r
                && let Some(d) = f.device
            {
                functions.entry(d.as_uuid()).or_default().push(f.id.as_uuid());
            }
        }
        let mut drawn: BTreeSet<Uuid> = BTreeSet::new();
        for r in typed(Kind::Representation) {
            if let Record::Representation(rep) = r {
                drawn.extend(rep.functions.iter().map(|f| f.as_uuid()));
                drawn.extend(rep.pin_map.as_slice().iter().map(|p| p.function.as_uuid()));
            }
        }
        let held = |d: Uuid| tx.get::<Device>(Id::from_uuid(d)).is_some();
        let mut kept = BTreeSet::new();
        for (d, fs) in &functions {
            let own = fs.iter().all(|f| {
                tx.get::<ew_schema::Function>(Id::from_uuid(*f))
                    .is_some_and(|x| x.device.map(|x| x.as_uuid()) == Some(*d))
            });
            let shown: Vec<Uuid> = fs.iter().copied().filter(|f| drawn.contains(f)).collect();
            if held(*d)
                && own
                && !shown.is_empty()
                && shown
                    .iter()
                    .all(|f| tx.reps_of_function(Id::from_uuid(*f)).is_empty())
            {
                kept.insert((Kind::Device, *d));
                kept.extend(fs.iter().map(|f| (Kind::Function, *f)));
            }
        }
        let strips: Vec<Uuid> = kept
            .iter()
            .filter(|k| k.0 == Kind::Device)
            .filter_map(|k| {
                let d = tx.get::<Device>(Id::from_uuid(k.1))?;
                d.aspects.terminal.as_ref().map(|t| t.strip.as_uuid())
            })
            .collect();
        for s in strips {
            if held(s) && keys.iter().any(|k| k.1 == Kind::Device && k.2 == s) {
                kept.insert((Kind::Device, s));
                kept.extend(
                    functions
                        .get(&s)
                        .into_iter()
                        .flatten()
                        .map(|f| (Kind::Function, *f)),
                );
            }
        }
        kept
    }

    /// The new IDs (effect 3): in (kind rank, fragment order); a PLC channel of [`NewIds::Instance`] takes
    /// `plc_channel_id(new module, number)` once its module's ID is known (modules rank before channels).
    fn mint_ids(
        &self,
        tx: &mut TxBuilder<'_>,
        keys: &[RecordAt],
        order: &[usize],
    ) -> Result<BTreeMap<(Kind, Uuid), Uuid>, CmdError> {
        let mut ids = BTreeMap::new();
        for &i in order {
            let (index, kind, old) = keys[i];
            // A PLC channel's ID is its new module's and number's in either way (a paste carries none; `page.copy`
            // mints the others).
            let new = match (self.ids, kind) {
                (_, Kind::Channel) => {
                    let v = self.record_value(index);
                    let module = v
                        .get("module")
                        .and_then(Value::as_str)
                        .and_then(|s| s.parse::<Uuid>().ok());
                    let number = v
                        .get("number")
                        .and_then(Value::as_u64)
                        .and_then(|n| u16::try_from(n).ok());
                    match (module.and_then(|m| ids.get(&(Kind::Device, m))), number) {
                        (Some(m), Some(n)) => {
                            let id: Id<()> = ew_ids::v5::plc_channel_id(Id::<()>::from_uuid(*m), n);
                            id.as_uuid()
                        }
                        _ => {
                            return Err(
                                self.invalid(self.pointer(index), text!("refusal-paste-channel-module"))
                            );
                        }
                    }
                }
                _ => self.ids.of(tx, old),
            };
            ids.insert((kind, old), new);
        }
        Ok(ids)
    }

    /// Runs the insertion (effects 1 to 7 of [`PasteFragment`]).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Inserted, CmdError> {
        if self.fragment.format != ew_schema::FORMAT {
            return Err(self.invalid(
                format!("{}/format", self.prefix),
                text!(
                    "refusal-paste-format",
                    self.fragment.format.to_string(),
                    ew_schema::FORMAT.to_string()
                ),
            ));
        }
        let (keys, skipped_items) = self.records()?;
        let (page, grid, steps, fit) = self.place(tx)?;
        let (copies, embed) = self.copies(tx)?;
        let nodes = self.nodes(tx)?;
        let mut notices: Vec<Text> = Vec::new();
        for text in skipped_items {
            notice(text, &mut notices);
        }
        let same_project = self.fragment.project.as_uuid() == tx.project().id.as_uuid();
        // Effect 8: a device a cut left unplaced comes back itself, keeping its ID, its functions and its tag; it is
        // not inserted again.
        let kept = if self.keep_unplaced && same_project {
            self.unplaced_devices(tx, &keys)
        } else {
            BTreeSet::new()
        };
        // Effect 3: new IDs in (kind rank, fragment order).
        let mut order: Vec<usize> = (0..keys.len()).collect();
        order.sort_by_key(|&i| (keys[i].1.rank(), keys[i].0));
        let minted: Vec<usize> = order
            .iter()
            .copied()
            .filter(|&i| !kept.contains(&(keys[i].1, keys[i].2)))
            .collect();
        let mut ids = self.mint_ids(tx, &keys, &minted)?;
        ids.extend(kept.iter().map(|k| (*k, k.1)));
        let plan = Plan {
            ids,
            copies,
            nodes,
            same_project,
        };
        // The new IDs of the pasted devices (effect 6).
        let pasted_devices: BTreeSet<Uuid> = plan
            .ids
            .iter()
            .filter(|((k, _), _)| *k == Kind::Device)
            .map(|(_, new)| *new)
            .collect();
        // The page kinds of the new pages, for their representations.
        let mut new_pages: BTreeMap<Uuid, PageType> = BTreeMap::new();
        // Effects 3 and 4 on each record's JSON, then the typed record.
        let mut typed: Vec<(usize, Uuid, Record)> = Vec::with_capacity(keys.len());
        for &i in &minted {
            let (record_index, kind, old) = keys[i];
            let at = self.pointer(record_index);
            let mut v = self.record_value(record_index).clone();
            drop_lineage(&mut v);
            if kind == Kind::Page {
                // the new page's section and folio are the target's (below), never the source's
                if let Some(folio) = v.get_mut("folio").and_then(Value::as_object_mut) {
                    folio.remove("section");
                }
            }
            if let Some(id) = v.get_mut("id") {
                *id = Value::from(plan.ids[&(kind, old)].hyphenated().to_string());
            }
            let mut unresolved: Vec<(Vec<Step>, &LinkDecl, Option<Text>)> = Vec::new();
            for decl in LINKS.iter().filter(|d| d.from == kind) {
                for (path, target) in occurrences(&v, decl.field) {
                    match self.resolve(tx, &plan, decl, target) {
                        Resolved::To(new) => {
                            if let Some(slot) = at_path(&mut v, &path) {
                                *slot = Value::from(new.hyphenated().to_string());
                            }
                        }
                        Resolved::Keep => {}
                        Resolved::Nothing(why) => unresolved.push((path, decl, why)),
                    }
                }
            }
            // Later list elements first, so the earlier paths stay valid.
            unresolved.sort_by(|a, b| b.0.cmp(&a.0));
            let mut parent_cleared = false;
            for (path, decl, why) in unresolved {
                let cleared = (decl.policy != Policy::Cascade)
                    .then(|| cleared(&v, &path))
                    .flatten();
                let Some(next) = cleared else {
                    return Err(self.invalid(
                        &at,
                        text!(
                            "refusal-paste-reference-missing",
                            kind.to_string(),
                            decl.field.to_string(),
                            decl.to.to_string()
                        ),
                    ));
                };
                v = next;
                if decl.from == Kind::Device && decl.field == "parent" {
                    parent_cleared = true;
                } else {
                    notice(
                        why.unwrap_or_else(|| {
                            text!(
                                "notice-paste-reference-cleared",
                                crate::words::kind_words(kind),
                                crate::words::kind_words(decl.to)
                            )
                        }),
                        &mut notices,
                    );
                }
            }
            let mut record = Record::from_value(v).map_err(|e| {
                self.invalid(
                    &at,
                    text!("refusal-params-detail", crate::error::schema_error_text(&e)),
                )
            })?;
            // Effect 7: a marker whose connections were all cleared is not inserted.
            if let Record::PageItem(item) = &record
                && item.kind.wire_marker().is_some_and(|m| m.connections.is_empty())
            {
                notice(text!("notice-paste-marker"), &mut notices);
                continue;
            }
            match &mut record {
                Record::Page(p) => {
                    new_pages.insert(p.id.as_uuid(), p.kind.clone());
                }
                Record::Device(d) => {
                    if parent_cleared {
                        notice(text!("notice-paste-child", old_tag(d)), &mut notices);
                        d.position = None;
                        if d.tag.scheme == TagScheme::Child {
                            d.tag.scheme = TagScheme::Default;
                        }
                    }
                    // Effect 6: the copies are numbered by the target's normalisers. A child tag counts under its
                    // parent, which online numbering does not number: a child whose parent comes along keeps its
                    // counter under the new parent, as `macro.insert` keeps it.
                    let parent_pasted = d.tag.scheme == TagScheme::Child
                        && d.parent.is_some_and(|p| pasted_devices.contains(&p.as_uuid()));
                    if !self.keep_counter.contains(&old) && !parent_pasted {
                        d.tag.counter = None;
                    }
                    if !self.keep_frozen {
                        d.tag.frozen = None;
                    }
                }
                Record::Connection(c) => c.number = NumberSpec::Unassigned,
                // A representation on a page record of the fragment stands on a page of its source's kind, as the
                // source holds it; one pasted onto the target page must suit that page.
                Record::Representation(r) if !new_pages.contains_key(&r.page.as_uuid()) => {
                    if let Some(allowed) = allowed_rep_types(&page.kind)
                        && !matches!(r.rep_type, ew_schema::RepType::Unknown(_))
                        && !allowed.contains(&r.rep_type)
                    {
                        return Err(self.invalid(
                            &at,
                            text!("refusal-paste-rep-type", crate::words::rep_type(&r.rep_type)),
                        ));
                    }
                }
                _ => {}
            }
            typed.push((record_index, old, record));
        }
        // Effect 1: the copies the project does not hold, in this transaction.
        for doc in embed {
            match doc {
                LibDocument::Part(c) => {
                    tx.insert(c)?;
                }
                LibDocument::Symbol(c) => {
                    tx.insert(c)?;
                }
                LibDocument::Fndef(c) => {
                    tx.insert(c)?;
                }
                LibDocument::Form(c) => {
                    tx.insert(c)?;
                }
                LibDocument::Manufacturer(c) => {
                    tx.insert(c)?;
                }
                LibDocument::Macro(_) | LibDocument::PlcProfile(_) | LibDocument::Unknown(_) => {
                    unreachable!("refused by `copies`")
                }
            }
        }
        let too_far = |i: usize| self.invalid(self.pointer(i), text!("refusal-paste-out-of-range"));
        // Effect 5 fitted: moved by `at − anchor`, then onto the page's grid together (`geometry::fit`); the
        // connections routed anew.
        if fit.is_some() {
            let d = sub(self.at, self.fragment.anchor)
                .map_err(|_| self.invalid("/at", text!("refusal-paste-far-from-anchor")))?;
            let mut items = Vec::new();
            for (i, _, record) in &mut typed {
                let i = *i;
                match record {
                    Record::Representation(r) => {
                        r.pos = crate::geometry::add(r.pos, d).map_err(|_| too_far(i))?;
                        items.push((i, crate::geometry::FitItem::rep(tx, r).map_err(|_| too_far(i))?));
                    }
                    Record::Junction(j) => {
                        j.at = crate::geometry::add(j.at, d).map_err(|_| too_far(i))?;
                        items.push((i, crate::geometry::FitItem::point(j.at)));
                    }
                    Record::PageItem(item) => {
                        *item = crate::commands::items::translate_item(item, d).ok_or_else(|| too_far(i))?;
                    }
                    Record::StructureBox(b) => {
                        for p in &mut b.outline {
                            let at = crate::geometry::add(*p, d).map_err(|_| too_far(i))?;
                            *p = crate::geometry::nearest_grid_point(grid, at).map_err(|_| too_far(i))?;
                        }
                    }
                    Record::Connection(c) => c.route.clear(),
                    _ => {}
                }
            }
            let fitted: Vec<crate::geometry::FitItem> = items.iter().map(|(_, item)| *item).collect();
            let moves: BTreeMap<usize, Pt2> = crate::geometry::fit(grid, &fitted, &Default::default())
                .map_err(|_| {
                    self.invalid(
                        format!("{}/records", self.prefix),
                        text!("refusal-paste-out-of-range"),
                    )
                })?
                .into_iter()
                .zip(&items)
                .map(|(m, (i, _))| (*i, m))
                .collect();
            for (i, _, record) in &mut typed {
                let (i, m) = (*i, moves.get(i).copied().unwrap_or(Pt2::ORIGIN));
                match record {
                    Record::Representation(r) => {
                        r.pos = crate::geometry::add(r.pos, m).map_err(|_| too_far(i))?;
                        if rep_cps_checked(tx, r).is_err() {
                            return Err(too_far(i));
                        }
                    }
                    Record::Junction(j) => j.at = crate::geometry::add(j.at, m).map_err(|_| too_far(i))?,
                    _ => {}
                }
            }
        } else if !self.has_pages() {
            self.move_by_steps(tx, &mut typed, grid, steps)?;
        }
        // A fragment with its pages keeps every position as its source holds it (a symbol off the grid too).
        // Effect 8: what the cut left of the devices that come back goes, so nothing is left behind: the wires of the
        // fragment and its junctions where the project still holds them (they are pasted anew), and a wire drawn on
        // another page that ends at such a device (left open there by the cut).
        if !kept.is_empty() {
            let mut gone: BTreeSet<ew_model::AnyKey> = BTreeSet::new();
            for &(_, kind, id) in &keys {
                if matches!(kind, Kind::Connection | Kind::Junction)
                    && tx.row(&(kind, Id::from_uuid(id))).is_some()
                {
                    gone.insert((kind, Id::from_uuid(id)));
                }
            }
            // The wires left open on another page, by page; the notice counts them.
            let mut open: BTreeMap<Id<Page>, BTreeSet<Id<Connection>>> = BTreeMap::new();
            for &(kind, f) in &kept {
                if kind != Kind::Function {
                    continue;
                }
                for c in tx.connections_of_function(Id::from_uuid(f)) {
                    if gone.contains(&(Kind::Connection, c.cast())) {
                        continue;
                    }
                    if let Some(p) = tx
                        .get::<Connection>(c)
                        .and_then(|c| c.page)
                        .filter(|p| *p != page.id)
                    {
                        open.entry(p).or_default().insert(c);
                    }
                }
            }
            for (p, wires) in &open {
                gone.extend(wires.iter().map(|c| (Kind::Connection, c.cast())));
                let on = crate::words::page(tx, *p);
                notice(
                    if wires.len() == 1 {
                        text!("notice-paste-moved-wire-removed", on)
                    } else {
                        text!("notice-paste-moved-wires-removed", wires.len().to_string(), on)
                    },
                    &mut notices,
                );
            }
            let gone: Vec<ew_model::AnyKey> = gone.into_iter().collect();
            tx.delete_many(&gone)?;
        }
        // The records, in (kind rank, fragment order).
        // Effect 6 is completed by online numbering (normaliser #4), which numbers the new devices in the order
        // of their anchor cells (FR-NUM-028), whatever IDs this transaction mints.
        let mut out = Inserted::default();
        let target = page.id;
        let mut after = page;
        for (_, old, record) in typed {
            let key = match record {
                Record::Page(mut p) => {
                    if let Some(folio) = self.folios.and_then(|f| f.get(&old)) {
                        p.folio.clone_from(folio);
                    } else {
                        // FR-MAC-002: the folio proposed after the target page, then after each new page in turn.
                        let (number, label) = crate::commands::page::propose_folio(tx, &after)?;
                        p.folio.section = after.folio.section;
                        p.folio.number = number;
                        p.folio.label = label;
                    }
                    let id = tx.insert(p)?;
                    out.pages.push(id.cast());
                    after = tx.require::<Page>(id.cast())?;
                    (Kind::Page, id.cast())
                }
                Record::Device(r) => (Kind::Device, tx.insert(r)?.cast()),
                Record::Function(r) => (Kind::Function, tx.insert(r)?.cast()),
                Record::Representation(r) => (Kind::Representation, tx.insert(r)?.cast()),
                Record::Connection(r) => (Kind::Connection, tx.insert::<Connection>(r)?.cast()),
                Record::Junction(r) => (Kind::Junction, tx.insert(r)?.cast()),
                Record::Channel(r) => (Kind::Channel, tx.insert(r)?.cast()),
                Record::PotentialDef(r) => (Kind::PotentialDef, tx.insert(r)?.cast()),
                Record::DeviceLink(r) => (Kind::DeviceLink, tx.insert(r)?.cast()),
                Record::PageItem(r) => (Kind::PageItem, tx.insert(r)?.cast()),
                Record::StructureBox(r) => (Kind::StructureBox, tx.insert(r)?.cast()),
                // `page.copy`'s cables, bonds and embedded report blocks.
                Record::Conductor(r) => (Kind::Conductor, tx.insert(r)?.cast()),
                Record::Bond(r) => (Kind::Bond, tx.insert(r)?.cast()),
                Record::ReportBlock(r) => (Kind::ReportBlock, tx.insert(r)?.cast()),
                _ => unreachable!("refused by the kind check"),
            };
            out.members.push((old, key.1.as_uuid()));
            match key.0 {
                Kind::Representation => {
                    out.first_rep.get_or_insert(key);
                }
                Kind::Device => {
                    out.first_device.get_or_insert(key);
                }
                _ => {}
            }
        }
        // Effect 8 on the target page: a wire there that ends at a device that came back (to a device the cut did not
        // take) follows its new place as a drag's wires do (`rep.move`, `geometry::follow_end`): its end moves from
        // where the route ended, the point the cut symbol drew, to the pin the pasted symbol draws.
        if !kept.is_empty() {
            self.follow_kept_ends(tx, &kept, target, grid)?;
        }
        out.notices = notices;
        Ok(out)
    }

    /// The wires on `page` that end at a function of `kept` (the devices a paste brought back), each end moved by
    /// [`crate::geometry::follow_end`] from its route's end point to the point the function's new symbol on `page`
    /// draws that pin at. A wire with no route, or whose end has no symbol on the page, keeps its route.
    fn follow_kept_ends(
        &self,
        tx: &mut TxBuilder<'_>,
        kept: &BTreeSet<(Kind, Uuid)>,
        page: Id<Page>,
        grid: Grid,
    ) -> Result<(), CmdError> {
        let pin_at = |tx: &TxBuilder<'_>, e: &Endpoint| -> Option<Pt2> {
            let Endpoint::Cp(f, idx) = e else { return None };
            if !kept.contains(&(Kind::Function, f.as_uuid())) {
                return None;
            }
            tx.reps_of_function(*f).into_iter().find_map(|r| {
                let rep = tx.get::<Representation>(r).filter(|rep| rep.page == page)?;
                crate::geometry::rep_cps(tx, &rep)
                    .into_iter()
                    .find(|c| c.function == *f && c.idx == *idx)
                    .map(|c| c.pos)
            })
        };
        let mut wires: BTreeSet<Id<Connection>> = BTreeSet::new();
        for &(kind, f) in kept {
            if kind == Kind::Function {
                wires.extend(tx.connections_of_function(Id::from_uuid(f)));
            }
        }
        for cid in wires {
            let Some(c) = tx.get::<Connection>(cid).filter(|c| c.page == Some(page)) else {
                continue;
            };
            let (Some(first), Some(last)) = (c.route.first().copied(), c.route.last().copied()) else {
                continue;
            };
            let delta = |to: Option<Pt2>, from: Pt2| {
                to.and_then(|to| sub(to, from).ok()).filter(|d| *d != Pt2::ORIGIN)
            };
            let (da, db) = (delta(pin_at(tx, &c.a), first), delta(pin_at(tx, &c.b), last));
            if da.is_none() && db.is_none() {
                continue;
            }
            let mut next = (*c).clone();
            if crate::geometry::follow_ends(&mut next.route, grid, da, db).is_err() {
                return tx.reject(text!(
                    "refusal-rep-move-route-out-of-range",
                    crate::words::wire(tx, cid)
                ));
            }
            tx.put(next);
        }
        Ok(())
    }
}

/// Drops a record's import lineage (`source_ref`): a pasted record is new, not imported, so its lineage is not a
/// reference to resolve (the copy drops it too; a fragment from elsewhere may carry one).
pub(super) fn drop_lineage(v: &mut Value) {
    if let Some(o) = v.as_object_mut() {
        o.remove("source_ref");
    }
}

/// Adds `text` to `notices` unless it is there already.
pub(super) fn notice(text: Text, notices: &mut Vec<Text>) {
    if !notices.contains(&text) {
        notices.push(text);
    }
}

/// The printed tag of a device record before the paste clears its counter (`-F1`).
pub(super) fn old_tag(d: &Device) -> String {
    match d.tag.counter {
        Some(c) => format!("-{}{c}", d.tag.letter.as_str()),
        None => format!("-{}", d.tag.letter.as_str()),
    }
}
