// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The package for the workshop (doc 13-package §8): one ZIP with the drawings as one PDF, the workshop lists, the label
//! files and the drawings for drilling, each written by the export that exists for it, a `Contents.txt` that says what
//! is in it and a `manifest.json` for a program. [`Session::package_plan`] says what a package of the project would
//! hold and what stands in its way (the release check, doc 13-package §8.6); the dialog and `ew export package` read
//! only it. [`PackageJob`] makes the package on a copy of the snapshot, on another thread; nothing of the session
//! changes, and the copy, Without personal data, is never saved.
//!
//! **The same bytes.** The entries stand in byte order of their paths, each deflated at `ew-zip`'s fixed level with its
//! fixed date; every member is dated by the package's date (the release's for a release package, else the caller's
//! `SOURCE_DATE_EPOCH`, else the model's edited date), never the clock; nothing names a path of the user's disk.

use std::path::Path;

use ew_derive::{LabelSet, Q, WireLength, WireRecordSet, print_language};
use ew_l10n::Text;
use ew_mltext::LangTag;
use serde::Serialize;

use crate::output::print::{PrintOptions, in_print_language, print_set_hooked};
use crate::output::{DrillingAsk, PdfDate, RowExport, RowsFormat, RowsOutput, row_output};
use crate::{LabelScope, Session, checks};

/// One member of the package (doc 13-package §8.2), declared once: the dialog's rows, the manifest's kinds, the lines
/// of Contents and `ew export package --leave-out` read [`PackageMember::all`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PackageMember {
    /// `Contents.txt`: what the package is for and what is in it. Always written.
    Contents,
    /// The drawings: every page in page order as one PDF, as Export PDF writes them.
    Pages,
    /// A workshop list (`ReportKindDef::workshop`) as its Excel export, by its kind's name.
    List(&'static str),
    /// The label files, in the form of the label file's profile.
    Labels,
    /// The drawings for drilling: per surface with holes its template and its DXF.
    Drilling,
}

impl PackageMember {
    /// Every member, in the order the dialog and Contents list them: Contents, the drawings, the workshop lists in the
    /// order their kinds are declared, the label files, the drawings for drilling.
    pub fn all() -> Vec<PackageMember> {
        let lists = ew_reports::Registry::builtin()
            .workshop()
            .into_iter()
            .map(PackageMember::List);
        std::iter::once(PackageMember::Contents)
            .chain(std::iter::once(PackageMember::Pages))
            .chain(lists)
            .chain([PackageMember::Labels, PackageMember::Drilling])
            .collect()
    }

    /// Its kind: the manifest's `kind` and the word `--leave-out` takes (`pages`, `wire_list`, `labels`).
    pub fn key(self) -> &'static str {
        match self {
            PackageMember::Contents => "contents",
            PackageMember::Pages => "pages",
            PackageMember::List(kind) => kind,
            PackageMember::Labels => "labels",
            PackageMember::Drilling => "drilling",
        }
    }

    /// The member of kind `key`, if there is one.
    pub fn parse(key: &str) -> Option<PackageMember> {
        PackageMember::all().into_iter().find(|m| m.key() == key)
    }

    /// Whether a person may leave it out: every member but Contents.
    pub fn optional(self) -> bool {
        self != PackageMember::Contents
    }

    /// Its words ("Wire list", "Label files") in the language `tag` (`nl-NL`; en-GB where it has none): a list by its
    /// kind's name, as Report block… names it.
    pub fn words(self, tag: &str) -> String {
        let t = match self {
            PackageMember::Contents => text!("package-member-contents"),
            PackageMember::Pages => text!("package-member-pages"),
            PackageMember::List(kind) => {
                return ew_reports::words::kind_name(kind, tag).unwrap_or_else(|| kind.to_owned());
            }
            PackageMember::Labels => text!("package-member-labels"),
            PackageMember::Drilling => text!("package-member-drilling"),
        };
        crate::catalogue().text_or_english(&t, tag)
    }

    /// Its form ("PDF", "Excel", "PDF, DXF"); the label files' is the profile's ([`crate::LabelProfile::summary`]).
    fn form(self, q: &Q) -> Text {
        match self {
            PackageMember::Contents => text!("package-form-text"),
            PackageMember::Pages => text!("package-form-pdf"),
            PackageMember::List(_) => text!("label-file-xlsx"),
            PackageMember::Labels => crate::labels::LabelProfile::of(q).summary(),
            PackageMember::Drilling => text!("package-form-drilling"),
        }
    }

    /// The folder its files stand in, in the project's language (`Lists`); `None` for a file at the top.
    fn folder(self, lang: Option<&LangTag>) -> Option<String> {
        let t = match self {
            PackageMember::Contents | PackageMember::Pages => return None,
            PackageMember::List(_) => text!("package-folder-lists"),
            PackageMember::Labels => text!("package-folder-labels"),
            PackageMember::Drilling => text!("package-folder-drilling"),
        };
        Some(in_print_language(&t, lang))
    }
}

/// What a package is asked to be.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct PackageAsk {
    /// The members left out (the dialog's unticked rows, `--leave-out`); Contents never is.
    pub leave_out: Vec<PackageMember>,
    /// Without personal data (doc 13-package §8.7): the name and the address of each party marked as personal data
    /// are left out of every member.
    pub omit_personal: bool,
    /// The date of a draft's members (`SOURCE_DATE_EPOCH`); `None`: the model's edited date. A release package is
    /// dated by its release.
    pub date: Option<PdfDate>,
    /// The integrity findings of opening the project, which the release check joins as the release gate does.
    pub integrity: Vec<checks::IntegrityItem>,
}

/// Why wires have no length, as Contents and the dialog say it (doc 13-package §8.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoLengthWhy {
    /// An end of each is made on site: the fitter measures it there.
    Site,
    /// Route has laid no way for them yet, and none is typed: wires of the cabinet [`NoLength::cabinet`] names, or of
    /// no cabinet (an end in another location) without one.
    NotLaid,
    /// The project has no panel layout, so Route lays none of them, and none is typed.
    NoLayout,
    /// The way Route laid no longer holds: their length is printed after "≈".
    Stale,
}

/// Wires without a length that holds, by why.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NoLength {
    /// Why.
    pub why: NoLengthWhy,
    /// The cabinet whose Route lays them ("+CAB"), for wires not laid yet; `None` for every other reason.
    pub cabinet: Option<String>,
    /// Their numbers, in the wire list's order (a wire drawn on no page has none and is counted only).
    pub numbers: Vec<String>,
    /// How many.
    pub count: usize,
}

impl NoLength {
    /// The reason in a sentence without its wires ("3 wires have no length: one end of each is made on site"): the one
    /// home of the words Contents writes, after which it lists the wires, and the dialog shows.
    pub fn sentence(&self) -> Text {
        let key = match (self.why, &self.cabinet) {
            (NoLengthWhy::Site, _) => key!("package-no-length-site"),
            (NoLengthWhy::NotLaid, Some(_)) => key!("package-no-length-not-laid-in"),
            (NoLengthWhy::NotLaid, None) => key!("package-no-length-not-laid"),
            (NoLengthWhy::NoLayout, _) => key!("package-no-length-no-layout"),
            (NoLengthWhy::Stale, _) => key!("package-no-length-stale"),
        };
        counted(key, self.count).with("cabinet", self.cabinet.clone().unwrap_or_default())
    }

    /// The reason as the dialog says it under the check's findings, without its wires: the sentence with its full stop.
    pub fn line(&self) -> Text {
        text!("package-no-length-line", self.sentence())
    }

    /// Whether a finding of the release check names these wires already: EW-MFG-001 a cabinet's wires not laid yet,
    /// EW-LAY-011 those whose way no longer holds. The dialog says the other reasons in a line of their own.
    pub fn in_a_finding(&self) -> bool {
        matches!(
            (self.why, &self.cabinet),
            (NoLengthWhy::NotLaid, Some(_)) | (NoLengthWhy::Stale, _)
        )
    }
}

/// One member as the package of the project would hold it.
#[derive(Clone, PartialEq, Debug)]
pub struct PlannedMember {
    /// The member.
    pub member: PackageMember,
    /// What it holds ("54 wires, 2 bridges"); `None` for a member without content, which is not written.
    pub holds: Option<Text>,
    /// Why it is not written, for a member without content ("No surface of this project has holes yet").
    pub why_not: Option<Text>,
    /// Its form ("Excel"; the label files' profile in a line).
    pub form: Text,
    /// What it lacks of part data, each with the way to it: a label set without a marker part in the stock.
    pub lacks: Vec<Text>,
}

/// What the package of the project would hold and what stands in its way (doc 13-package §8.5).
#[derive(Clone, PartialEq, Debug)]
pub struct PackagePlan {
    /// The revision's label: the open one's for a draft, the released one's for a release package; `None` without a
    /// revision.
    pub revision: Option<String>,
    /// For a release package (the last revision released and none open), its release date as the project writes a
    /// date ("28-11-2026"); `None` for a draft.
    pub released_on: Option<String>,
    /// The release check (doc 13-package §8.6): what stood, errors first.
    pub check: checks::CheckReport,
    /// Every member that may go in, in [`PackageMember::all`]'s order, Contents not among them.
    pub members: Vec<PlannedMember>,
    /// The wires without a length that holds, by why.
    pub without_length: Vec<NoLength>,
    /// The names of the parties marked as personal data, in the project's language.
    pub personal: Vec<String>,
    /// The file's name ("SAMPLE-E V1.0 2026-11-27 draft package.zip").
    pub file_name: String,
    /// The members' date.
    pub date: PdfDate,
}

impl PackagePlan {
    /// Whether the package is a draft: a revision is open, or there is none.
    pub fn draft(&self) -> bool {
        self.released_on.is_none()
    }

    /// The open errors of the release check: each stops a release package.
    pub fn errors(&self) -> Vec<&checks::Finding> {
        checks::blockers(&self.check)
    }

    /// Whether the package is refused: a release package with an open error of the release check (a draft is written
    /// whatever stood).
    pub fn refused(&self) -> bool {
        !self.draft() && !self.errors().is_empty()
    }

    /// The open findings of the release check, errors first.
    pub fn findings(&self) -> Vec<&checks::Finding> {
        let mut open: Vec<&checks::Finding> = self.check.findings.iter().filter(|f| f.is_open()).collect();
        open.sort_by_key(|f| std::cmp::Reverse(f.severity));
        open
    }
}

/// The package written: its bytes and what it holds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Package {
    /// The ZIP's bytes.
    pub bytes: Vec<u8>,
    /// The paths of its entries, in the ZIP's order.
    pub paths: Vec<String>,
}

/// Why no package was made.
#[derive(Debug)]
pub enum PackageError {
    /// A release package with open errors of the release check: their messages.
    Refused(Vec<String>),
    /// A member's export failed, in words.
    Member(String),
    /// The job was cancelled.
    Cancelled,
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageError::Refused(errors) => {
                let n = errors.len();
                let words = in_print_language(&counted(key!("package-refused"), n), None);
                write!(f, "{words}: {}", errors.join("; "))
            }
            PackageError::Member(why) => f.write_str(why),
            PackageError::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for PackageError {}

/// The text of the key `key` with the count `n`: its `$n`, and `$count` `one` or `many`, which its pattern selects on.
fn counted(key: &'static str, n: usize) -> Text {
    Text::checked_by_macro(key, crate::text_english(key).unwrap_or(key), Vec::new())
        .with("count", if n == 1 { "one" } else { "many" })
        .with("n", n.to_string())
}

/// The text of the key `key` with two counts: `$n` and `$count` of the first, `$m` and `$count2` of the second.
fn counted2(key: &'static str, n: usize, m: usize) -> Text {
    counted(key, n)
        .with("count2", if m == 1 { "one" } else { "many" })
        .with("m", m.to_string())
}

/// `d` as ISO `2026-11-27`.
fn iso(d: PdfDate) -> String {
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

/// The revision a package is made of: its label and, for a release package, its release date.
fn revision_of(q: &Q) -> (Option<String>, Option<ew_schema::Date>) {
    if let Some(open) = ew_derive::open_revision(q) {
        return (Some(open.label.as_str().to_owned()), None);
    }
    match ew_derive::current_revision(q) {
        Some(r) if r.state == ew_schema::RevisionState::Released => (
            Some(r.label.as_str().to_owned()),
            Some(r.date.unwrap_or_else(|| model_day(q))),
        ),
        Some(r) => (Some(r.label.as_str().to_owned()), None),
        None => (None, None),
    }
}

/// The model's date as a day.
fn model_day(q: &Q) -> ew_schema::Date {
    let d = crate::output::model_date(q);
    ew_schema::Date::new(d.year, d.month, d.day)
        .unwrap_or_else(|_| ew_schema::Date::new(1980, 1, 1).expect("a date"))
}

/// The wires without a length that holds, by why: an end made on site; not laid yet, per cabinet as EW-MFG-001 counts
/// them (the cabinet's wires that Route lays), then those of no cabinet; every one in a project without a panel layout;
/// a way that no longer holds.
fn without_length(q: &Q) -> Vec<NoLength> {
    use ew_derive::layout::{CabinetWire, cabinet_name, cabinet_wires};
    let records = q.get::<WireRecordSet>(());
    let missing = |r: &&ew_derive::WireRecord| r.length == WireLength::Missing && !r.flags.field;
    let group = |why, cabinet, wires: Vec<&ew_derive::WireRecord>| {
        (!wires.is_empty()).then(|| NoLength {
            why,
            cabinet,
            numbers: wires.iter().filter_map(|r| r.number.clone()).collect(),
            count: wires.len(),
        })
    };
    let mut out: Vec<NoLength> = Vec::new();
    let site = records
        .iter()
        .filter(|r| r.length == WireLength::Missing && r.flags.field)
        .collect();
    out.extend(group(NoLengthWhy::Site, None, site));
    let spaces: Vec<_> = q.table::<ew_schema::LayoutSpace>().ids().collect();
    if spaces.is_empty() {
        out.extend(group(
            NoLengthWhy::NoLayout,
            None,
            records.iter().filter(missing).collect(),
        ));
    } else {
        let mut laid_by_route = std::collections::BTreeSet::new();
        for space in spaces {
            let wires: Vec<_> = cabinet_wires(q, space)
                .into_iter()
                .filter(|(r, k)| *k == CabinetWire::Routed && r.length == WireLength::Missing)
                .map(|(r, _)| r.wire.id)
                .collect();
            laid_by_route.extend(wires.iter().copied());
            let of_cabinet = records.iter().filter(|r| wires.contains(&r.wire.id)).collect();
            out.extend(group(
                NoLengthWhy::NotLaid,
                Some(cabinet_name(q, space)),
                of_cabinet,
            ));
        }
        let rest = records
            .iter()
            .filter(missing)
            .filter(|r| !laid_by_route.contains(&r.wire.id))
            .collect();
        out.extend(group(NoLengthWhy::NotLaid, None, rest));
    }
    let stale = records
        .iter()
        .filter(|r| matches!(r.length, WireLength::Stale { .. }))
        .collect();
    out.extend(group(NoLengthWhy::Stale, None, stale));
    out
}

/// The drawing's name a file of the package starts with: the drawing number, else the object's, else `project`, as
/// the label files are named.
fn drawing_stem(q: &Q) -> String {
    crate::labels::drawing_stem(q)
}

/// `name` as a file's name: each of its words made safe for a file system ([`ew_store::archive::sanitise`]), the
/// spaces between them kept, as the label files are named.
fn file_words(name: &str) -> String {
    name.split(' ')
        .map(ew_store::archive::sanitise)
        .collect::<Vec<_>>()
        .join(" ")
}

/// `words` as a sentence: its first letter upper-case, a full stop at its end.
fn sentence(words: &str) -> String {
    let mut chars = words.chars();
    let first = chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_default();
    format!("{first}{}.", chars.as_str())
}

/// The project as the package names it: the drawing's name with the revision's label (`SAMPLE-E V1.0`).
fn drawing_and_revision(q: &Q, revision: Option<&str>) -> String {
    match revision {
        Some(label) => format!("{} {label}", drawing_stem(q)),
        None => drawing_stem(q),
    }
}

impl Session {
    /// What the package of the project would hold and what stands in its way (doc 13-package §8.5): the revision, the
    /// release check's findings, per member what it holds or why it is not written and what it lacks, the wires without
    /// a length by why, the parties marked as personal data and the file's name. The dialog and `ew export package`
    /// read only this.
    pub fn package_plan(&self, ask: &PackageAsk) -> PackagePlan {
        let q = self.query();
        let lang = print_language(&q);
        let (revision, released) = revision_of(&q);
        let date = released.map_or_else(
            || ask.date.unwrap_or_else(|| crate::output::model_date(&q)),
            |d| PdfDate::ymd(d.year(), d.month(), d.day()),
        );
        let check = self.release_check(&q, &ask.integrity);
        let members = PackageMember::all()
            .into_iter()
            .filter(|m| m.optional())
            .map(|m| self.planned(&q, m))
            .collect();
        let personal = q
            .project()
            .parties
            .iter()
            .filter(|p| p.personal_data)
            .map(|p| ew_derive::text_in(&p.name, lang.as_ref()).to_owned())
            .collect();
        let name = drawing_and_revision(&q, revision.as_deref());
        let file = if released.is_some() {
            text!("package-file-name", name, iso(date))
        } else {
            text!("package-file-name-draft", name, iso(date))
        };
        PackagePlan {
            released_on: released.map(|d| q.project().settings.formatting.date_format().format(d)),
            revision,
            check,
            members,
            without_length: without_length(&q),
            personal,
            file_name: file_words(&in_print_language(&file, lang.as_ref())),
            date,
        }
    }

    /// The release check of the snapshot `q` (doc 13-package §8.6): the project's release scheme in release mode with
    /// the evidence the release gate hands it, `integrity` among it.
    pub(crate) fn release_check(&self, q: &Q, integrity: &[checks::IntegrityItem]) -> checks::CheckReport {
        let scheme = checks::release_scheme_of(q);
        let evidence = checks::Evidence {
            integrity: integrity.to_vec(),
            merged: self.merge_evidence(),
            layout: self.layout_evidence(q, scheme, checks::CheckMode::Release),
        };
        checks::release_check(q, scheme, evidence)
    }

    /// The member `m` as the package would hold it.
    fn planned(&self, q: &Q, m: PackageMember) -> PlannedMember {
        let mut lacks = Vec::new();
        let (holds, why_not) = match m {
            PackageMember::Contents | PackageMember::Pages => (Some(text!("package-holds-pages")), None),
            PackageMember::List(kind) => {
                let rows = ew_schema::ReportKind::from_name(kind)
                    .and_then(|k| ew_reports::project_rows(q, &k, None))
                    .map_or(0, |set| set.rows.len());
                if rows == 0 {
                    let why = if kind == "hole_table" {
                        text!("package-no-holes")
                    } else {
                        text!("package-no-rows")
                    };
                    (None, Some(why))
                } else {
                    (Some(self.list_holds(q, kind, rows)), None)
                }
            }
            PackageMember::Labels => match self.label_files(&LabelScope::All, None, None) {
                Ok(files) if !files.is_empty() => {
                    let labels: usize = files.iter().map(|f| f.count).sum();
                    for f in files.iter().filter(|f| f.marker_part.is_none()) {
                        lacks.push(text!(
                            "package-lacks-marker",
                            crate::label_set_text(f.set),
                            marks_words(f.set)
                        ));
                    }
                    (
                        Some(counted2(key!("package-holds-labels"), files.len(), labels)),
                        None,
                    )
                }
                Ok(_) => (None, Some(text!("package-no-labels"))),
                Err(e) => (None, Some(Text::bare(e.to_string()))),
            },
            PackageMember::Drilling => {
                let surfaces = self.drilling_surfaces(ew_derive::layout::Paper::A4);
                if surfaces.is_empty() {
                    (None, Some(text!("package-no-holes")))
                } else {
                    (
                        Some(counted(key!("package-holds-drilling"), surfaces.len())),
                        None,
                    )
                }
            }
        };
        PlannedMember {
            member: m,
            holds,
            why_not,
            form: m.form(q),
            lacks,
        }
    }

    /// What the workshop list `kind` holds, in words ("54 wires, 2 bridges"; "3 strips"; "3 surfaces, 20 holes"), else
    /// its rows.
    fn list_holds(&self, q: &Q, kind: &str, rows: usize) -> Text {
        match kind {
            "wire_list" => counted2(
                key!("package-holds-wire-list"),
                q.get::<WireRecordSet>(()).len(),
                ew_reports::kinds::bridge_count(q),
            ),
            "assembly_list" => counted(
                key!("package-holds-assembly-list"),
                q.devices()
                    .iter()
                    .filter(|(_, d)| d.aspects.strip.is_some())
                    .count(),
            ),
            "hole_table" => {
                let surfaces = self.drilling_surfaces(ew_derive::layout::Paper::A4);
                let holes: usize = surfaces.iter().map(|s| s.holes).sum();
                counted2(key!("package-holds-hole-table"), surfaces.len(), holes)
            }
            "cut_list" => text!("package-holds-cut-list"),
            "consumables" => text!("package-holds-consumables"),
            _ => counted(key!("package-holds-rows"), rows),
        }
    }

    /// The package of the current snapshot as a job (doc 13-package §8.4): the snapshot and its assets cross to the
    /// job's thread, where [`PackageJob::run`] makes it. A release package with an open error of the release check is
    /// refused before anything is made.
    ///
    /// # Errors
    ///
    /// [`PackageError::Refused`] with the errors' messages.
    pub fn package_job(&self, ask: &PackageAsk) -> Result<PackageJob, PackageError> {
        let plan = self.package_plan(ask);
        if plan.refused() {
            let q = self.query();
            let lang = print_language(&q).map(|l| l.to_string());
            let lang = lang.as_deref().unwrap_or(checks::messages::DEFAULT_LANGUAGE);
            return Err(PackageError::Refused(
                plan.errors().iter().map(|f| checks::render(f, lang)).collect(),
            ));
        }
        Ok(PackageJob {
            model: self.world().model,
            generation: self.store().map(ew_store::Store::generation),
            ask: ask.clone(),
            plan,
        })
    }
}

/// What a label set marks, in words ("cables"), for the line that says its stock has no marker part for it.
fn marks_words(set: LabelSet) -> Text {
    match set {
        LabelSet::WireMarkers => text!("package-marks-wires"),
        LabelSet::TerminalMarkers => text!("package-marks-terminals"),
        LabelSet::StripMarkers => text!("package-marks-strips"),
        LabelSet::CableMarkers => text!("package-marks-cables"),
        LabelSet::DeviceLabels => text!("package-marks-devices"),
        LabelSet::TubeMarkers => text!("package-marks-tubes"),
        LabelSet::Plates => text!("package-marks-plates"),
    }
}

/// The package of a snapshot, made on another thread ([`Session::package_job`]): the model with its assets, what was
/// asked and the plan it was asked on. It is `Send`; the `Session` is not.
pub struct PackageJob {
    model: ew_model::Model,
    /// The session's store generation, which a workbook's metadata names as `ew export report` does.
    generation: Option<u64>,
    ask: PackageAsk,
    plan: PackagePlan,
}

/// The package job crosses to its thread: checked at compile time.
const _: () = {
    const fn send<T: Send>() {}
    send::<PackageJob>();
};

impl std::fmt::Debug for PackageJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackageJob")
            .field("ask", &self.ask)
            .field("file_name", &self.plan.file_name)
            .finish_non_exhaustive()
    }
}

/// One entry of the package before it is written.
struct Entry {
    path: String,
    kind: &'static str,
    bytes: Vec<u8>,
    /// What it is for, as Contents says it.
    about: String,
}

impl PackageJob {
    /// The plan the job was made on.
    pub fn plan(&self) -> &PackagePlan {
        &self.plan
    }

    /// Makes the package on the calling thread (the job's): a scratch session over the snapshot (Without personal data:
    /// with the personal parties' names and addresses emptied, never saved), then each member from the export that
    /// exists for it, Contents and the manifest from the one list of written members, and the ZIP of them in byte order
    /// of their paths. Writes no file.
    ///
    /// # Errors
    ///
    /// [`PackageError::Member`] when an export fails; [`PackageError::Cancelled`] at a checkpoint after a cancel.
    pub fn run(self, ctl: Option<&ew_jobs::JobCtl>) -> Result<Package, PackageError> {
        // The snapshot's revision, which a workbook's metadata names whatever the copy below is.
        let rev = self.model.rev().0;
        let model = if self.ask.omit_personal {
            without_personal(self.model)?
        } else {
            self.model
        };
        let s = Session::new(model, crate::SessionOptions::default());
        let checkpoint = |what: &'static str| -> Result<(), PackageError> {
            match ctl {
                Some(ctl) => ctl.checkpoint(what).map_err(|_| PackageError::Cancelled),
                None => Ok(()),
            }
        };
        let plan = &self.plan;
        let q = s.query();
        let lang = print_language(&q);
        let words = |t: &Text| in_print_language(t, lang.as_ref());
        // The project's language as a tag of the text files, which names a list's file.
        let tag = ew_l10n::render::language_for(lang.as_ref(), &crate::catalogue().tags()).to_owned();
        let left_out = |m: PackageMember| self.ask.leave_out.contains(&m);
        let mut entries: Vec<Entry> = Vec::new();
        // The labels the label files hold, which Contents counts.
        let mut labels = 0;
        let stem = drawing_and_revision(&q, plan.revision.as_deref());
        // The drawings: the print set of every page at 100 % on its own sheet, a draft's pages with the mark.
        if !left_out(PackageMember::Pages) {
            let mark = plan.draft().then(|| {
                words(&match &plan.revision {
                    Some(label) => text!("package-draft-mark", label.clone()),
                    None => text!("package-draft-mark-none"),
                })
            });
            let options = PrintOptions {
                date: Some(plan.date),
                ..PrintOptions::default()
            };
            let set = print_set_hooked(&q, &options, &s.export_assets(), ctl, mark.as_deref()).map_err(
                |h| match h {
                    crate::output::Halt::Cancelled => PackageError::Cancelled,
                    crate::output::Halt::Output(e) => PackageError::Member(e.to_string()),
                },
            )?;
            entries.push(Entry {
                path: file_words(&words(&text!("package-drawings-file", stem.clone()))),
                kind: PackageMember::Pages.key(),
                bytes: set.pdf,
                about: words(&text!("package-about-pages")),
            });
        }
        // The workshop lists, each as `ew export report --kind <kind> --format xlsx` writes it: the session's revision
        // and store generation in the hidden sheet `_ew`, not the copy's.
        let mut meta = s.workbook_meta(Some(plan.date));
        meta.rev = rev;
        meta.generation = self.generation;
        for m in &plan.members {
            let PackageMember::List(kind) = m.member else {
                continue;
            };
            if left_out(m.member) || m.holds.is_none() {
                continue;
            }
            checkpoint("list")?;
            let file = row_output(
                &q,
                &RowExport::Kind(kind.to_owned()),
                None,
                &RowsFormat::Xlsx(meta.clone()),
            )
            .map_err(|e| PackageError::Member(e.to_string()))?;
            let RowsOutput::Xlsx(bytes) = file.output else {
                continue;
            };
            let folder = m.member.folder(lang.as_ref()).unwrap_or_default();
            let name = format!("{} {}", drawing_stem(&q), m.member.words(&tag));
            entries.push(Entry {
                path: format!("{folder}/{}.xlsx", file_words(&name)),
                kind: m.member.key(),
                bytes,
                about: sentence(&m.holds.as_ref().map_or_else(|| m.member.words(&tag), words)),
            });
        }
        // The label files, in the form of the label file's profile.
        if !left_out(PackageMember::Labels) {
            checkpoint("labels")?;
            let files = s
                .label_files(&LabelScope::All, None, Some(plan.date))
                .map_err(|e| PackageError::Member(e.to_string()))?;
            let folder = PackageMember::Labels.folder(lang.as_ref()).unwrap_or_default();
            labels = files.iter().map(|f| f.count).sum();
            for f in files {
                let about = match &f.marker_part {
                    Some(_) => counted(key!("package-about-labels"), f.count),
                    None => counted(key!("package-about-labels-no-part"), f.count),
                };
                entries.push(Entry {
                    path: format!("{folder}/{}", f.name),
                    kind: PackageMember::Labels.key(),
                    bytes: f.bytes,
                    about: words(&about),
                });
            }
        }
        // The drawings for drilling: per surface with holes its template on A4 and its DXF.
        if !left_out(PackageMember::Drilling) {
            checkpoint("drilling")?;
            let paper = ew_derive::layout::Paper::A4;
            let surfaces = s.drilling_surfaces(paper);
            let ask = DrillingAsk {
                surfaces: surfaces.iter().map(|d| d.surface).collect(),
                paper,
                template: true,
                dxf: true,
                date: Some(plan.date),
            };
            let files = s.drilling_files(&ask).map_err(PackageError::Member)?;
            let folder = PackageMember::Drilling.folder(lang.as_ref()).unwrap_or_default();
            // Per surface its DXF, then its template, in the surfaces' order.
            for (f, d) in files.into_iter().zip(surfaces.iter().flat_map(|d| [d, d])) {
                let about = if f.name.ends_with(".dxf") {
                    counted(key!("package-about-dxf"), d.holes)
                } else {
                    counted2(key!("package-about-template"), d.holes, d.sheets)
                };
                entries.push(Entry {
                    path: format!("{folder}/{}", f.name),
                    kind: PackageMember::Drilling.key(),
                    bytes: f.bytes,
                    about: words(&about),
                });
            }
        }
        checkpoint("written")?;
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let contents = Entry {
            path: words(&text!("package-contents-file")),
            kind: PackageMember::Contents.key(),
            bytes: contents_txt(&q, plan, &entries, labels, lang.as_ref()).into_bytes(),
            about: String::new(),
        };
        let mut members: Vec<&Entry> = entries.iter().chain(std::iter::once(&contents)).collect();
        members.sort_by(|a, b| a.path.cmp(&b.path));
        let manifest = manifest_json(&q, plan, &members, self.ask.omit_personal, lang.as_ref());
        let mut zip: Vec<(&str, ew_zip::Method, &[u8])> = members
            .iter()
            .map(|e| (e.path.as_str(), ew_zip::Method::Deflated, e.bytes.as_slice()))
            .collect();
        zip.push(("manifest.json", ew_zip::Method::Deflated, manifest.as_bytes()));
        zip.sort_by(|a, b| a.0.cmp(b.0));
        let bytes = ew_zip::write(&zip).map_err(|e| PackageError::Member(e.to_string()))?;
        Ok(Package {
            paths: zip.iter().map(|(p, _, _)| (*p).to_owned()).collect(),
            bytes,
        })
    }
}

/// The model `m` with the personal data of each party marked as personal data left out (doc 13-package §8.7): every
/// member of the project's record that [`ew_schema::PERSONAL_DATA_FIELDS`] names, removed where the array element on
/// its path is marked (`personal_data`). It is no edit of the project: the project's row is replaced in the copy alone,
/// with no command, so no normaliser runs (no revision opens, no date is stamped) and nothing is saved.
///
/// # Errors
///
/// [`PackageError::Member`] when the record without them does not read back or the model refuses the row.
fn without_personal(m: ew_model::Model) -> Result<ew_model::Model, PackageError> {
    let failed = |e: &dyn std::fmt::Display| PackageError::Member(e.to_string());
    let before = std::sync::Arc::clone(Q::new(&m).project());
    let mut value = serde_json::to_value(&*before).map_err(|e| failed(&e))?;
    let mut changed = false;
    for (kind, path) in ew_schema::PERSONAL_DATA_FIELDS {
        if *kind == ew_schema::Kind::Project {
            changed |= leave_out(&mut value, &path.split('.').collect::<Vec<_>>());
        }
    }
    if !changed {
        return Ok(m);
    }
    let after: ew_schema::Project = serde_json::from_value(value).map_err(|e| failed(&e))?;
    let op = ew_model::Op::update(
        ew_model::AnyRow::Project(before),
        ew_model::AnyRow::Project(std::sync::Arc::new(after)),
    );
    let rev = ew_model::Rev(m.rev().0 + 1);
    m.apply_with(&[op], rev, &ew_model::ApplyOptions::default())
        .map_err(|e| failed(&e))
}

/// Removes the member at `path` (`parties[]`, `name`) of `value`, walking into the elements of an array (`a[]`) that are
/// marked as personal data; whether it removed one.
fn leave_out(value: &mut serde_json::Value, path: &[&str]) -> bool {
    let Some((first, rest)) = path.split_first() else {
        return false;
    };
    let Some(o) = value.as_object_mut() else {
        return false;
    };
    match (first.strip_suffix("[]"), rest.is_empty()) {
        (Some(array), _) => o
            .get_mut(array)
            .and_then(serde_json::Value::as_array_mut)
            .is_some_and(|items| {
                items
                    .iter_mut()
                    .filter(|e| e.get("personal_data").and_then(serde_json::Value::as_bool) == Some(true))
                    .fold(false, |done, e| leave_out(e, rest) | done)
            }),
        (None, true) => o.remove(*first).is_some(),
        (None, false) => o.get_mut(*first).is_some_and(|v| leave_out(v, rest)),
    }
}

/// Contents.txt (doc 13-package §8.2): what the package is for, the revision, whether it is a draft and why, what stood
/// in the release check, how many wires have no length and why, and each file with what it is for; UTF-8 with a
/// byte-order mark and CR LF, in the project's language.
fn contents_txt(
    q: &Q,
    plan: &PackagePlan,
    entries: &[Entry],
    labels: usize,
    lang: Option<&LangTag>,
) -> String {
    let words = |t: &Text| in_print_language(t, lang);
    let m = &q.project().master;
    let head: Vec<String> = [
        Some(drawing_stem(q)),
        Some(ew_derive::text_in(&m.document_title, lang).to_owned()),
        Some(ew_derive::text_in(&m.description, lang).to_owned()),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.trim().is_empty())
    .collect();
    let mut lines: Vec<String> = vec![head.join(" · ")];
    let date = iso(plan.date);
    lines.push(words(&match (&plan.revision, plan.draft()) {
        (Some(label), false) => text!("package-contents-released-on", label.clone(), date),
        (Some(label), true) => text!("package-contents-made-on", label.clone(), date),
        (None, _) => text!("package-contents-made-on-none", date),
    }));
    lines.push(String::new());
    lines.push(words(&match (&plan.revision, plan.draft()) {
        (Some(label), true) => text!(
            "package-contents-draft",
            label.clone(),
            words(&text!("package-draft-mark", label.clone()))
        ),
        (None, _) => text!(
            "package-contents-draft-none",
            words(&text!("package-draft-mark-none"))
        ),
        (Some(_), false) => text!("package-contents-release"),
    }));
    lines.push(String::new());
    let findings = plan.findings();
    if findings.is_empty() {
        lines.push(words(&text!("package-contents-ready")));
    } else {
        lines.push(words(&counted(key!("package-contents-stood"), findings.len())));
        let tag = lang.map(ToString::to_string);
        let tag = tag.as_deref().unwrap_or(checks::messages::DEFAULT_LANGUAGE);
        for f in findings {
            lines.push(format!(
                "  {} {}: {}",
                f.rule,
                ew_reports::words::text(&ew_reports::words::severity(f.severity), lang),
                checks::render(f, tag)
            ));
        }
    }
    lines.push(String::new());
    for n in &plan.without_length {
        lines.push(words(&text!(
            "package-contents-no-length",
            n.sentence(),
            n.numbers.join(", ")
        )));
    }
    lines.push(words(&if plan.without_length.is_empty() {
        text!("package-contents-every-length")
    } else {
        text!("package-contents-other-lengths")
    }));
    lines.push(String::new());
    lines.push(words(&text!("package-contents-what")));
    lines.push(String::new());
    // Each file with what it is for, in the members' order (the drawings, the lists, the label files, the drawings for
    // drilling), the files of a folder under its line, the manifest last.
    let order = PackageMember::all();
    let at = |e: &Entry| order.iter().position(|m| m.key() == e.kind).unwrap_or(usize::MAX);
    let mut listed: Vec<&Entry> = entries.iter().collect();
    listed.sort_by(|a, b| (at(a), &a.path).cmp(&(at(b), &b.path)));
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut folder: Option<&str> = None;
    for e in listed {
        match e.path.split_once('/') {
            Some((dir, name)) => {
                if folder != Some(dir) {
                    let about = match PackageMember::parse(e.kind) {
                        Some(PackageMember::Labels) => {
                            words(&counted(key!("package-about-labels-folder"), labels))
                        }
                        Some(PackageMember::Drilling) => words(&text!("package-about-drilling-folder")),
                        _ => words(&text!("package-about-lists-folder")),
                    };
                    rows.push((format!("  {dir}/"), about));
                    folder = Some(dir);
                }
                rows.push((format!("    {name}"), e.about.clone()));
            }
            None => {
                folder = None;
                rows.push((format!("  {}", e.path), e.about.clone()));
            }
        }
    }
    rows.push((
        "  manifest.json".to_owned(),
        words(&text!("package-about-manifest")),
    ));
    let width = rows.iter().map(|(p, _)| p.chars().count()).max().unwrap_or(0) + 2;
    for (path, about) in rows {
        let pad = width - path.chars().count();
        lines.push(format!("{path}{}{about}", " ".repeat(pad)));
    }
    lines.push(String::new());
    lines.push(words(&text!(
        "package-contents-made-by",
        env!("CARGO_PKG_VERSION")
    )));
    let mut out = String::from("\u{feff}");
    for l in lines {
        out.push_str(l.trim_end());
        out.push_str("\r\n");
    }
    out
}

/// The manifest's members in the order doc 13-package §8.3 gives them.
#[derive(Serialize)]
struct Manifest<'a> {
    format: &'static str,
    made_by: String,
    date: String,
    project: ManifestProject,
    revision: ManifestRevision,
    draft: bool,
    personal_data: &'static str,
    findings: Vec<ManifestFinding>,
    members: Vec<ManifestMember<'a>>,
}

#[derive(Serialize)]
struct ManifestProject {
    id: String,
    drawing: String,
    description: String,
}

#[derive(Serialize)]
struct ManifestRevision {
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    released_on: Option<String>,
}

#[derive(Serialize)]
struct ManifestFinding {
    rule: &'static str,
    severity: &'static str,
    message: String,
}

#[derive(Serialize)]
struct ManifestMember<'a> {
    path: &'a str,
    kind: &'static str,
    bytes: usize,
    sha256: String,
}

/// manifest.json (doc 13-package §8.3): UTF-8 JSON, two spaces deep, every other entry with its kind, size and SHA-256.
fn manifest_json(
    q: &Q,
    plan: &PackagePlan,
    members: &[&Entry],
    omit_personal: bool,
    lang: Option<&LangTag>,
) -> String {
    let tag = lang.map(ToString::to_string);
    let tag = tag.as_deref().unwrap_or(checks::messages::DEFAULT_LANGUAGE);
    let m = Manifest {
        format: "elektrawatt-package/1",
        made_by: format!("ElektraWatt {}", env!("CARGO_PKG_VERSION")),
        date: iso(plan.date),
        project: ManifestProject {
            id: q.project().id.to_string(),
            drawing: drawing_stem(q),
            description: ew_derive::text_in(&q.project().master.description, lang).to_owned(),
        },
        revision: ManifestRevision {
            label: plan.revision.clone(),
            state: if plan.draft() { "open" } else { "released" },
            released_on: (!plan.draft()).then(|| iso(plan.date)),
        },
        draft: plan.draft(),
        personal_data: if omit_personal { "left_out" } else { "kept" },
        findings: plan
            .findings()
            .into_iter()
            .map(|f| ManifestFinding {
                rule: f.rule,
                severity: f.severity.name(),
                message: checks::render(f, tag),
            })
            .collect(),
        members: members
            .iter()
            .map(|e| ManifestMember {
                path: &e.path,
                kind: e.kind,
                bytes: e.bytes.len(),
                sha256: crate::output::rows::sha256_hex(&e.bytes),
            })
            .collect(),
    };
    let mut out = serde_json::to_string_pretty(&m).unwrap_or_default();
    out.push('\n');
    out
}

/// Writes `package` to `path` whole or not at all.
///
/// # Errors
///
/// The I/O error of the write.
pub fn write_package(path: &Path, package: &Package) -> std::io::Result<()> {
    ew_store::file::write_replacing(path, &package.bytes)
}
