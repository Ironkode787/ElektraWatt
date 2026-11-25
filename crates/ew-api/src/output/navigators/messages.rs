// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The message panel's table and the migration report: the findings grouped by severity and rule, their
//! target drawables and pages.
//!
//! Not here: the checks themselves (`crate::checks`); the table types (`navigators.rs`).

use std::collections::BTreeMap;

use ew_derive::Q;
use ew_ids::{Id, Uuid};
use ew_mltext::natural_key;
use ew_schema::{FnRole, Function, Page};

use super::{
    Builder, Drawable, FIX_ALL, NavKind, NavOpen, NavRole, NavTable, connection_drawables, device_drawables,
    drawable, drawn_as_members, folios, group, sorted,
};
use crate::Session;

impl Session {
    /// The Messages table of the migration report (d, T11): the tree of [`Session::navigator_table`]'s
    /// Messages from a run of the `migration` scheme at the release mode, the import runs' findings among the live
    /// ones.
    pub fn migration_table(&self) -> NavTable {
        let rev = self.world().rev.0;
        let mut b = Builder { nodes: Vec::new() };
        let q = self.query();
        self.messages_table(
            &q,
            &mut b,
            &crate::checks::MIGRATION,
            crate::checks::CheckMode::Release,
        );
        b.finish(NavKind::Messages, rev)
    }

    /// Whether the project holds an import lineage, so that its migration report can be shown (d).
    pub fn has_lineage(&self) -> bool {
        self.query()
            .table::<ew_schema::ImportLineage>()
            .iter()
            .next()
            .is_some()
    }
}

/// The columns of the message panel's table rows, in order; `rule` is the key column.
pub(super) const MESSAGE_COLUMNS: [(&str, &str); 7] = [
    ("severity", key!("column-severity")),
    ("rule", key!("column-rule")),
    ("message", key!("column-message")),
    ("object", key!("column-object")),
    ("page_cell", key!("column-page-cell")),
    ("fix", key!("column-fix")),
    ("state", key!("column-state")),
];

/// The severities in the panel's group order, with the group labels.
const SEVERITY_GROUPS: [(crate::checks::Severity, &str); 3] = [
    (crate::checks::Severity::Error, "Errors"),
    (crate::checks::Severity::Warning, "Warnings"),
    (crate::checks::Severity::Info, "Infos"),
];

/// The word of a rule kind (`safety`, `conflict`, `completeness`, `hygiene`, `integrity`): the `kind` facet of a
/// finding row and the detail line's kind.
fn rule_kind_word(kind: crate::checks::RuleKind) -> &'static str {
    use crate::checks::RuleKind;
    match kind {
        RuleKind::Safety => "safety",
        RuleKind::Conflict => "conflict",
        RuleKind::Completeness => "completeness",
        RuleKind::Hygiene => "hygiene",
        RuleKind::Integrity => "integrity",
    }
}

/// The drawables of one finding target: a device the representations of its functions, else where it is drawn as
/// its members (a cable its cores, a strip its terminals: a cable's finding opened no page); a function its
/// representations; a representation and a connection themselves; a conductor its core connections; every other
/// record none.
fn target_drawables(
    q: &Q,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    target: &crate::checks::Target,
) -> Vec<Drawable> {
    use ew_schema::{Conductor, ConnectionKind, Kind};
    let id = target.id.as_uuid();
    match target.kind {
        Kind::Device => {
            let device = Id::from_uuid(id);
            let own = device_drawables(q, folios, device);
            match q.devices().get(device) {
                Some(d) if own.is_empty() => drawn_as_members(q, folios, d),
                _ => own,
            }
        }
        Kind::Function => {
            let f: Id<Function> = Id::from_uuid(id);
            let main = q.functions().get(f).is_none_or(|x| x.role == FnRole::Main);
            sorted(
                q.idx()
                    .reps_of_function(f)
                    .filter_map(|r| drawable(q, folios, r, main))
                    .collect(),
            )
        }
        Kind::Representation => sorted(drawable(q, folios, Id::from_uuid(id), true).into_iter().collect()),
        Kind::Connection => connection_drawables(q, folios, &[Id::from_uuid(id)]),
        Kind::Conductor => {
            let cid: Id<Conductor> = Id::from_uuid(id);
            let mut cons: Vec<Id<ew_schema::Connection>> = q
                .connections()
                .iter()
                .filter(|(_, c)| matches!(c.kind, ConnectionKind::Core(x) if x == cid))
                .map(|(_, c)| c.id)
                .collect();
            cons.sort();
            connection_drawables(q, folios, &cons)
        }
        _ => Vec::new(),
    }
}

/// Sets a node's first natural key to its rank (its index), so that the default label sort keeps the provider's order.
fn rank(b: &mut Builder, at: u32) {
    let r = natural_key(&at.to_string());
    if let Some(first) = b.nodes[at as usize].natural.first_mut() {
        *first = r;
    }
}

impl Session {
    /// The check scheme of the online checks: the project's `active_scheme`
    /// ([`Session::project_check_scheme`] in [`crate::checks::CheckMode::Online`]), else `drafting`. The message panel
    /// and the property grid's header findings run it.
    pub fn online_scheme(&self) -> &'static crate::checks::Scheme {
        self.project_check_scheme(crate::checks::CheckMode::Online)
            .unwrap_or(&crate::checks::DRAFTING)
    }

    /// The message panel's tree: severity groups (Error, Warning, Info; an empty one left out) → rule
    /// groups in rule-ID order (label `<rule ID> <title>`, badge the number of its findings) → the findings in the
    /// report's order (label `<subject>: <message>`, key `finding:<fingerprint>`), from the online scheme's run. A
    /// node's first natural key is its rank in this order, so that the default label sort shows the provider's order.
    pub(super) fn messages_table(
        &self,
        q: &Q,
        b: &mut Builder,
        scheme: &'static crate::checks::Scheme,
        mode: crate::checks::CheckMode,
    ) {
        use crate::checks;
        // With merge evidence, EW-CORE-013's groups.
        let report = self.check_over(q, scheme, mode, Vec::new());
        let folios = folios(q);
        for (severity, label) in SEVERITY_GROUPS {
            let of_severity: Vec<&checks::Finding> = report
                .findings
                .iter()
                .filter(|f| f.severity == severity)
                .collect();
            if of_severity.is_empty() {
                continue;
            }
            let word = severity.name();
            let snode = group(
                b,
                None,
                NavRole::Severity,
                &format!("severity:{word}"),
                label.to_owned(),
            );
            rank(b, snode);
            let rules: std::collections::BTreeSet<&'static str> =
                of_severity.iter().map(|f| f.rule).collect();
            for rule in rules {
                let findings: Vec<&checks::Finding> =
                    of_severity.iter().copied().filter(|f| f.rule == rule).collect();
                let meta = checks::rule(rule).map(|r| r.meta());
                // Every finding of the group has a quick fix of one kind (its label's key, else, for a fix an importer
                // worded, its command): Fix all (one step for what would take one fix after another).
                let kinds: std::collections::BTreeSet<Option<String>> = findings
                    .iter()
                    .map(|f| {
                        f.fix.as_ref().map(|x| match x.label.key() {
                            k if k == ew_checks::key!("words-as-written") => x.command.clone(),
                            k => k.to_owned(),
                        })
                    })
                    .collect();
                let mut badges = vec![findings.len().to_string()];
                if findings.len() > 1 && kinds.len() == 1 && kinds.iter().all(Option::is_some) {
                    badges.push(FIX_ALL.to_owned());
                }
                let rnode = b.push(
                    Some(snode),
                    NavRole::Rule,
                    &format!("rule:{rule}"),
                    match meta.map(|m| m.title()) {
                        Some(title) if !title.is_empty() => format!("{rule} {title}"),
                        _ => rule.to_owned(),
                    },
                    badges,
                    Vec::new(),
                    Vec::new(),
                    None,
                    vec![("kind".to_owned(), NavRole::Rule.word().to_owned())],
                );
                rank(b, rnode);
                let kind = meta.map(|m| rule_kind_word(m.kind));
                for f in findings {
                    finding_node(q, b, &folios, rnode, f, kind);
                }
            }
        }
    }
}

/// The drawables of a finding's primary target among `candidates` (the place key of each target's first drawable,
/// its printed label, its drawables, in target order): the smallest place key, a tie broken on the label in natural
/// order and then by bytes; the target order decides only between equal labels.
pub(super) fn primary_drawables<K: Ord>(
    candidates: impl Iterator<Item = (K, String, Vec<Drawable>)>,
) -> Vec<Drawable> {
    candidates
        .map(|(key, label, ds)| (key, natural_key(&label), label, ds))
        .min_by(|a, b| (&a.0, &a.1, &a.2).cmp(&(&b.0, &b.1, &b.2)))
        .map(|(_, _, _, ds)| ds)
        .unwrap_or_default()
}

/// Whether a finding's `message` already names its `subject`, so that the Messages row shows the subject once: the
/// message leads with it (then a colon, a space or nothing), or names it after a space, followed by the end, a space,
/// a colon, a comma or a full stop ("Wire -G1:+ → -S1:1 has no route", the layout rules).
pub fn names_subject(subject: &str, message: &str) -> bool {
    let ends = |rest: &str| rest.is_empty() || rest.starts_with([':', ' ', ',', '.']);
    message
        .strip_prefix(subject)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(':') || rest.starts_with(' '))
        || (!subject.is_empty()
            && message
                .match_indices(subject)
                .any(|(i, _)| message[..i].ends_with(' ') && ends(&message[i + subject.len()..])))
}

/// A finding's page and its primary target's drawables: the finding's page, else the page of the primary
/// target's first drawable; the folio as the page prints it.
pub(crate) fn finding_page(
    q: &Q,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    f: &crate::checks::Finding,
) -> (Option<(String, Uuid)>, Vec<Drawable>) {
    // The primary target is the one drawn first (main drawables first, then page order and cell), not the first in
    // target order: targets sort by ID, which is random in a new project, so a finding over several members (a
    // potential's) would open another page in another copy of the project.
    let positions: BTreeMap<Uuid, usize> = folios.values().map(|(pos, _, u)| (*u, *pos)).collect();
    let first_key = |ds: &[Drawable]| {
        ds.first().map(|d| {
            (
                !d.main,
                positions.get(&d.page).copied().unwrap_or(usize::MAX),
                natural_key(&d.cell),
            )
        })
    };
    // A tie (two targets drawn first in the same cell) breaks on the targets' printed labels in natural order,
    // never on the target (ID) order.
    let label =
        |t: &crate::checks::Target| crate::output::properties::order_key(q, t.kind, t.id.as_uuid(), None).0;
    let primary = primary_drawables(
        f.targets
            .iter()
            .map(|t| (target_drawables(q, folios, t), t))
            .filter(|(ds, _)| !ds.is_empty())
            .map(|(ds, t)| (first_key(&ds), label(t), ds)),
    );
    let page: Option<(String, Uuid)> = match f.page {
        Some(p) => folios.get(&p).map(|(_, folio, u)| (folio.clone(), *u)),
        None => primary.first().map(|d| (d.folio.clone(), d.page)),
    };
    (page, primary)
}

/// One finding row: its records are the targets, its drawables theirs; its page is the finding's page, else the
/// page of the primary target's first drawable; Enter opens that page with the targets' drawables on it selected.
fn finding_node(
    q: &Q,
    b: &mut Builder,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    parent: u32,
    f: &crate::checks::Finding,
    kind: Option<&'static str>,
) {
    let mut drawables = Vec::new();
    for t in &f.targets {
        drawables.extend(target_drawables(q, folios, t));
    }
    let mut seen = std::collections::BTreeSet::new();
    drawables.retain(|d| seen.insert(d.record));
    let (page, primary) = finding_page(q, folios, f);
    let page_cell = match &page {
        Some((folio, u)) => primary
            .iter()
            .find(|d| d.page == *u)
            .map_or_else(|| folio.clone(), Drawable::place),
        None => String::new(),
    };
    let waived = matches!(f.state, crate::checks::FindingState::Waived { .. });
    let state = if waived { "waived" } else { "open" };
    let word = f.severity.name();
    let cells = vec![
        word.to_owned(),
        f.rule.to_owned(),
        f.message().clone(),
        f.subject.clone(),
        page_cell,
        f.fix
            .as_ref()
            .map(|x| crate::checks::messages::fix_label(x, crate::checks::messages::DEFAULT_LANGUAGE))
            .unwrap_or_default(),
        state.to_owned(),
    ];
    let mut badges = vec![word.to_owned()];
    if waived {
        badges.push("waived".to_owned());
    }
    if f.fix.is_some() {
        badges.push("fix".to_owned());
    }
    let mut facets = Vec::new();
    if let Some(k) = kind {
        facets.push(("kind".to_owned(), k.to_owned()));
    }
    if let Some((folio, _)) = &page {
        facets.push(("page".to_owned(), folio.to_lowercase()));
    }
    facets.push(("state".to_owned(), state.to_owned()));
    // A rule whose message already leads with its subject (EW-BUS-001, EW-CORE-033: "X: X has
    // no ...") shows the subject once; the findings themselves, the CSV and the reports are unchanged.
    let label = if f.subject.is_empty() || names_subject(&f.subject, &f.message()) {
        f.message().clone()
    } else {
        format!("{}: {}", f.subject, f.message())
    };
    let open_page = page
        .as_ref()
        .map(|(_, u)| *u)
        .or_else(|| drawables.first().map(|d| d.page));
    let records: Vec<Uuid> = f.targets.iter().map(|t| t.id.as_uuid()).collect();
    let select = |p: Uuid| -> Vec<Uuid> {
        drawables
            .iter()
            .filter(|d| d.page == p)
            .map(|d| d.record)
            .collect()
    };
    let open = open_page.map(|p| NavOpen {
        page: p,
        select: select(p),
    });
    let at = b.push(
        Some(parent),
        NavRole::Finding,
        &format!("finding:{}", f.fingerprint),
        label,
        badges,
        records,
        drawables,
        Some(cells),
        facets,
    );
    rank(b, at);
    b.nodes[at as usize].open = open;
}
