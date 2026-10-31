// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 3: the library reference form of the commands (D7). The session resolves a [`LibRef`] with its
//! closure from [`Session::libraries`] and composes `library.embed`, `part.assign` and `library.update` with the
//! documents; the commands never read a library, so a command log replays on a machine with no library installed.
//! Also the library status of the held copies (FT-487's data) and the update plan the card shows (D9).

use std::collections::BTreeSet;

use ew_commands::commands::{
    AssignModeParam, AssignPart, ChangeClass, EmbedLibraryCopies, FieldChange, UpdateLibraryCopies,
    classify_update,
};
use ew_derive::ew_ids::{Id, Uuid};
use ew_schema::{
    Device, ItemStatus, Kind, LibCopy, LibDocument, LibRef, MacroDef, MacroInstance, Part, Representation,
    Sha256,
};
use ew_units::Dec3;

use ew_derive::Q;

use crate::{AnyCommand, BomMode, CmdError, DerivedDiff, Export, Session, library};

/// Why a library operation of the session could not compose its command.
#[derive(Clone, PartialEq, Debug)]
pub enum ApiError {
    /// A reference that no installed library resolves (with its closure).
    Library {
        /// The reference, as `library/item@version_req`.
        reference: String,
        /// The registry's error.
        error: library::LibraryError,
    },
    /// The project holds the item at another version (another hash): update from library first.
    OtherVersionHeld {
        /// The held copy ID.
        copy: Uuid,
        /// The held version.
        held: String,
        /// The library's version.
        library: String,
    },
    /// The project holds no library copy with this ID.
    NotHeld(Uuid),
    /// The resolved reference is not a part.
    NotAPart(Uuid),
    /// The resolved reference is not a macro.
    NotAMacro(Uuid),
    /// Nothing to do: the project holds every document already, or every copy is current.
    NothingToDo,
    /// The command was refused (the update plan's preview).
    Cmd(CmdError),
    /// The strip cell's compile refused the chosen part (WP-S2.07 D16, [`Session::part_pick_command`]).
    Strip(crate::output::StripEditError),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Library { reference, error } => write!(f, "{reference}: {error}"),
            ApiError::OtherVersionHeld { copy, held, library } => write!(
                f,
                "the project holds copy {copy} at version {held}, the library has {library}: update from library first"
            ),
            ApiError::NotHeld(c) => write!(f, "the project holds no library copy {c}"),
            ApiError::NotAPart(c) => write!(f, "{c} is not a part"),
            ApiError::NotAMacro(c) => write!(f, "{c} is not a macro"),
            ApiError::NothingToDo => {
                f.write_str("nothing to do: the project holds the documents as they are")
            }
            ApiError::Cmd(e) => write!(f, "{e}"),
            ApiError::Strip(crate::output::StripEditError::Refused(key)) => f.write_str(key),
            ApiError::Strip(crate::output::StripEditError::Invalid(m)) => f.write_str(m),
            ApiError::Strip(crate::output::StripEditError::Ambiguous(c)) => {
                write!(f, "{} parts fit; choose one", c.len())
            }
        }
    }
}

impl std::error::Error for ApiError {}

/// How [`Session::assign_part`] changes the devices' part uses (FR-PRT-023).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AssignMode {
    /// Adds a part use.
    Append,
    /// Replaces every part use by the one part use.
    ReplaceAll,
    /// Replaces each part use of the copy `from`.
    Replace {
        /// The copy ID of the part to replace.
        from: Uuid,
    },
}

/// A CP mapping of one copy for [`Session::update_from_library`]: pairs `(old_idx, new_idx)`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CpMap {
    /// The held copy ID.
    pub copy: Uuid,
    /// The pairs `(old_idx, new_idx)`.
    pub map: Vec<(u16, u16)>,
}

/// The library state of one held copy (WP-S1.19 §8, FT-487's data).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CopyState {
    /// The library's document has the held copy's hash (or a lower version).
    Current,
    /// The library has another version of the item; `class` is the classified change (doc 07 §4.5).
    UpdateAvailable {
        /// The library's version.
        version: String,
        /// The change class.
        class: ChangeClass,
    },
    /// The library's document is deprecated, with its successor if it names one (FR-PRT-034).
    Deprecated {
        /// The successor.
        successor: Option<LibRef>,
    },
    /// No installed library holds the item.
    SourceMissing,
    /// The copy is a local fork (`meta.source` names the item it was forked from).
    LocalFork,
}

/// One held library copy and its state ([`Session::library_status`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CopyStatus {
    /// The kind (part, symbol, fndef, form or manufacturer).
    pub kind: Kind,
    /// The copy ID.
    pub copy: Uuid,
    /// The library ID.
    pub library: Uuid,
    /// The item ID.
    pub item: Uuid,
    /// The held version.
    pub version: String,
    /// The state.
    pub state: CopyState,
    /// The devices that use the copy (where-used, in ID order).
    pub devices: Vec<Id<Device>>,
}

/// One copy of an update plan: the versions, the class and the field diff.
#[derive(Clone, PartialEq, Debug)]
pub struct CopyUpdate {
    /// The kind.
    pub kind: Kind,
    /// The copy ID.
    pub copy: Uuid,
    /// The held version.
    pub from: String,
    /// The library's version.
    pub to: String,
    /// The class (doc 07 §4.5), whatever the declared versions say.
    pub class: ChangeClass,
    /// The members that differ.
    pub fields: Vec<FieldChange>,
    /// Why the change is major.
    pub major_reasons: Vec<String>,
}

/// What [`Session::update_from_library`] would change (D9's card): the command to preview and commit, the copies with
/// their class and field diff, the devices and representations touched, the BOM rows that change and the derived
/// texts (device tags and cross-reference texts) that change.
#[derive(Clone, Debug)]
pub struct UpdatePlan {
    /// The `library.update` command with the library documents.
    pub command: AnyCommand,
    /// The copies that change, in the order asked.
    pub copies: Vec<CopyUpdate>,
    /// The devices that use a changed copy (in ID order).
    pub devices: Vec<Id<Device>>,
    /// The representations of those devices' functions and of a changed symbol (in ID order).
    pub representations: Vec<Id<Representation>>,
    /// The component BOM's CSV rows that go (in BOM order).
    pub bom_removed: Vec<String>,
    /// The component BOM's CSV rows that come (in BOM order).
    pub bom_added: Vec<String>,
    /// The printed tags and cross-reference texts that change.
    pub derived: DerivedDiff,
}

// WP-S2.07 part 4b
/// One library copy a pasted fragment carries in another version than the project holds (WP-S2.07 D22, FT-396), as
/// the paste choice card lists it: "FX-MCB-1PN (Fixture): this project 1.1.0, pasted 1.0.0, patch".
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VersionConflict {
    /// The kind of the copy.
    pub kind: Kind,
    /// The copy ID (equal across versions).
    pub copy: Uuid,
    /// The copy's printed name: a part's `<part number> (<manufacturer>)`, else its key or name.
    pub title: String,
    /// The project's version.
    pub held: String,
    /// The fragment's version.
    pub pasted: String,
    /// The class of the change from the pasted version to the project's (doc 07 §4.5): what "Use the project's copy"
    /// does to the pasted records; `Major` (a CP they may name disappears) disables it.
    pub class: ChangeClass,
    /// Whether the pasted version is newer than the project's (semantic version core); "Update the project's copy" is
    /// offered only when every listed copy's is.
    pub newer: bool,
}

/// The FnDefs and the manufacturer a part names, the FnDef a symbol names (FT-640's closure items), by (kind, copy ID).
fn named_items(doc: &LibDocument) -> Vec<(Kind, Uuid)> {
    let copy = |r: &LibRef| -> Uuid {
        let id: Id<()> = ew_derive::ew_ids::v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid());
        id.as_uuid()
    };
    match doc {
        LibDocument::Part(c) => c
            .item
            .function_template
            .iter()
            .map(|t| (Kind::Fndef, copy(&t.fndef)))
            .chain(std::iter::once((Kind::Manufacturer, copy(&c.item.manufacturer))))
            .collect(),
        LibDocument::Symbol(c) => vec![(Kind::Fndef, copy(&c.item.fndef))],
        _ => Vec::new(),
    }
}

/// A library document's printed name for the paste choice card: a part's `<part number> (<manufacturer>)` (the
/// manufacturer as the project holds it, else as `docs` carry it, else left out), else the item's key or name, else
/// its kind.
fn copy_title(q: &Q, doc: &LibDocument, docs: &[LibDocument]) -> String {
    if let LibDocument::Part(c) = doc {
        let mut name = ew_derive::manufacturer_name(q, &c.item.manufacturer);
        if name.is_empty() {
            let id: Id<()> = ew_derive::ew_ids::v5::lib_copy_id(
                c.item.manufacturer.library.as_uuid(),
                c.item.manufacturer.item.as_uuid(),
            );
            name = docs
                .iter()
                .find_map(|d| match d {
                    LibDocument::Manufacturer(m) if m.id.as_uuid() == id.as_uuid() => {
                        Some(m.item.key.as_str().to_owned())
                    }
                    _ => None,
                })
                .unwrap_or_default();
        }
        let number = c.item.part_number.as_str();
        return if name.is_empty() {
            number.to_owned()
        } else {
            format!("{number} ({name})")
        };
    }
    let v = to_value(doc);
    ["key", "name"]
        .into_iter()
        .find_map(|m| match v.get(m) {
            Some(serde_json::Value::String(s)) => Some(s.clone()),
            Some(serde_json::Value::Object(o)) => o.values().find_map(|t| t.as_str().map(str::to_owned)),
            _ => None,
        })
        .unwrap_or_else(|| identity(doc).map(|i| i.0.to_string()).unwrap_or_default())
}

fn ref_text(r: &LibRef) -> String {
    format!("{}/{}@{}", r.library, r.item, r.version_req)
}

/// Kind, copy ID, library, item, version and hash of a library document.
fn identity(doc: &LibDocument) -> Option<(Kind, Uuid, Uuid, Uuid, String, Sha256)> {
    fn of<T>(k: Kind, c: &LibCopy<T>) -> (Kind, Uuid, Uuid, Uuid, String, Sha256) {
        (
            k,
            c.id.as_uuid(),
            c.library_id.as_uuid(),
            c.item_id.as_uuid(),
            c.version.clone(),
            c.sha256,
        )
    }
    Some(match doc {
        LibDocument::Part(c) => of(Kind::Part, c),
        LibDocument::Symbol(c) => of(Kind::Symbol, c),
        LibDocument::Fndef(c) => of(Kind::Fndef, c),
        LibDocument::Form(c) => of(Kind::Form, c),
        LibDocument::Macro(c) => of(Kind::Macro, c),
        LibDocument::Manufacturer(c) => of(Kind::Manufacturer, c),
        LibDocument::Unknown(_) => return None,
    })
}

/// The held copy of a kind and ID, as a document.
fn held(q: &Q, kind: Kind, copy: Uuid) -> Option<LibDocument> {
    match kind {
        Kind::Part => q
            .parts()
            .get(Id::from_uuid(copy))
            .map(|c| LibDocument::Part((**c).clone())),
        Kind::Symbol => q
            .symbols()
            .get(Id::from_uuid(copy))
            .map(|c| LibDocument::Symbol((**c).clone())),
        Kind::Fndef => q
            .fndefs()
            .get(Id::from_uuid(copy))
            .map(|c| LibDocument::Fndef((**c).clone())),
        Kind::Form => q
            .forms()
            .get(Id::from_uuid(copy))
            .map(|c| LibDocument::Form((**c).clone())),
        Kind::Manufacturer => q
            .manufacturers()
            .get(Id::from_uuid(copy))
            .map(|c| LibDocument::Manufacturer((**c).clone())),
        Kind::Macro => q
            .table::<LibCopy<MacroDef>>()
            .get(Id::from_uuid(copy))
            .map(|c| LibDocument::Macro((**c).clone())),
        _ => None,
    }
}

/// The held copy with this ID, of any kind the session reads.
fn held_any(q: &Q, copy: Uuid) -> Option<LibDocument> {
    [
        Kind::Part,
        Kind::Symbol,
        Kind::Fndef,
        Kind::Form,
        Kind::Manufacturer,
        Kind::Macro,
    ]
    .into_iter()
    .find_map(|k| held(q, k, copy))
}

/// Every held library copy the session reads (parts, symbols, FnDefs, forms, manufacturers, macros), in kind then ID
/// order.
fn held_copies(q: &Q) -> Vec<LibDocument> {
    let mut out: Vec<LibDocument> = Vec::new();
    out.extend(q.parts().iter().map(|(_, c)| LibDocument::Part((**c).clone())));
    out.extend(
        q.symbols()
            .iter()
            .map(|(_, c)| LibDocument::Symbol((**c).clone())),
    );
    out.extend(q.fndefs().iter().map(|(_, c)| LibDocument::Fndef((**c).clone())));
    out.extend(q.forms().iter().map(|(_, c)| LibDocument::Form((**c).clone())));
    out.extend(
        q.manufacturers()
            .iter()
            .map(|(_, c)| LibDocument::Manufacturer((**c).clone())),
    );
    out.extend(
        q.table::<LibCopy<MacroDef>>()
            .iter()
            .map(|(_, c)| LibDocument::Macro((**c).clone())),
    );
    out
}

fn to_value(doc: &LibDocument) -> serde_json::Value {
    serde_json::to_value(doc).expect("a library document has a JSON form")
}

/// The devices that use a copy: a part's devices, a FnDef's functions' devices, a symbol's representations' devices,
/// a manufacturer's parts' devices, a macro's devices that its placed instances created; in ID order.
fn where_used(q: &Q, doc: &LibDocument) -> (Vec<Id<Device>>, Vec<Id<Representation>>) {
    let mut devices = BTreeSet::new();
    let mut reps = BTreeSet::new();
    match doc {
        LibDocument::Part(c) => {
            for (id, d) in q.devices().iter() {
                if d.parts.iter().any(|u| u.part.copy == c.id) {
                    devices.insert(id);
                }
            }
        }
        LibDocument::Manufacturer(c) => {
            let parts: BTreeSet<Id<Part>> = q
                .parts()
                .iter()
                .filter(|(_, p)| {
                    p.item.manufacturer.item.as_uuid() == c.item_id.as_uuid()
                        && p.item.manufacturer.library.as_uuid() == c.library_id.as_uuid()
                })
                .map(|(id, _)| id)
                .collect();
            for (id, d) in q.devices().iter() {
                if d.parts.iter().any(|u| parts.contains(&u.part.copy)) {
                    devices.insert(id);
                }
            }
        }
        LibDocument::Fndef(c) => {
            for (_, f) in q.functions().iter() {
                if f.def == c.id {
                    devices.extend(f.device);
                }
            }
        }
        LibDocument::Symbol(c) => {
            for (id, r) in q.reps().iter() {
                if r.symbol == c.id {
                    reps.insert(id);
                    for f in &r.functions {
                        if let Some(func) = q.functions().get(*f) {
                            devices.extend(func.device);
                        }
                    }
                }
            }
        }
        LibDocument::Macro(c) => {
            for (_, inst) in q.table::<MacroInstance>().iter() {
                if inst.macro_copy == c.id {
                    for (_, made) in &inst.members {
                        let d: Id<Device> = Id::from_uuid(made.as_uuid());
                        if q.devices().get(d).is_some() {
                            devices.insert(d);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    for (id, r) in q.reps().iter() {
        let drawn = r.functions.iter().any(|f| {
            q.functions()
                .get(*f)
                .and_then(|func| func.device)
                .is_some_and(|d| devices.contains(&d))
        });
        if drawn {
            reps.insert(id);
        }
    }
    (devices.into_iter().collect(), reps.into_iter().collect())
}

/// Whether a copy is a local fork: its `meta.source` (an untyped member until doc 07's `ForkOf` is typed) names the
/// item it was forked from (`{"fork_of": …}`).
fn is_fork(doc: &LibDocument) -> bool {
    fn meta<T>(c: &LibCopy<T>, m: impl Fn(&T) -> &ew_schema::ItemMeta) -> bool {
        m(&c.item)
            .extra
            .get("source")
            .and_then(|s| s.get("fork_of"))
            .is_some()
    }
    match doc {
        LibDocument::Part(c) => meta(c, |i| &i.meta),
        LibDocument::Symbol(c) => meta(c, |i| &i.meta),
        LibDocument::Fndef(c) => meta(c, |i| &i.meta),
        LibDocument::Form(c) => meta(c, |i| &i.meta),
        LibDocument::Manufacturer(c) => meta(c, |i| &i.meta),
        LibDocument::Macro(c) => meta(c, |i| &i.meta),
        _ => false,
    }
}

fn status_of(doc: &LibDocument) -> ItemStatus {
    match doc {
        LibDocument::Part(c) => c.item.meta.status.clone(),
        LibDocument::Symbol(c) => c.item.meta.status.clone(),
        LibDocument::Fndef(c) => c.item.meta.status.clone(),
        LibDocument::Form(c) => c.item.meta.status.clone(),
        LibDocument::Manufacturer(c) => c.item.meta.status.clone(),
        LibDocument::Macro(c) => c.item.meta.status.clone(),
        LibDocument::Unknown(_) => ItemStatus::default(),
    }
}

fn version_core(v: &str) -> Option<[u64; 3]> {
    let core = v.trim().split(['-', '+']).next()?;
    let parts: Vec<u64> = core.split('.').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    match parts[..] {
        [a, b, c] => Some([a, b, c]),
        _ => None,
    }
}

impl Session {
    /// The documents of `refs` with their closures (D7, FT-274) that the project does not hold yet: a held copy with
    /// the same hash is left out; one with another hash is [`ApiError::OtherVersionHeld`].
    pub(crate) fn closure_docs(&self, refs: &[LibRef]) -> Result<Vec<LibDocument>, ApiError> {
        let q = self.query();
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for r in refs {
            let docs = self.libraries().closure(r).map_err(|error| ApiError::Library {
                reference: ref_text(r),
                error,
            })?;
            for doc in docs {
                let Some((kind, copy, _, _, version, sha)) = identity(&doc) else {
                    continue;
                };
                if !seen.insert((kind, copy)) {
                    continue;
                }
                // a macro in a closure (a part's macro links) is carried as before: the command reuses a held copy
                if let Some(h) = held(&q, kind, copy).filter(|_| kind != Kind::Macro) {
                    let (_, _, _, _, held_version, held_sha) = identity(&h).expect("a held copy");
                    if held_sha != sha {
                        return Err(ApiError::OtherVersionHeld {
                            copy,
                            held: held_version,
                            library: version,
                        });
                    }
                    continue;
                }
                out.push(doc);
            }
        }
        Ok(out)
    }

    /// The item ID of the next fork of the held copy `copy` (WP-S2.07 D20): `v5(copy, "fork")`, else
    /// `v5(copy, "fork\u{1f}<n>")` for the first `n` from 1 whose copy the project does not hold, as
    /// [`Session::next_macro_instance`] mints instance IDs. The command carries the ID, so a replay reproduces it.
    pub fn next_fork_item(&self, copy: Uuid) -> Uuid {
        let q = self.query();
        let project = q.project().id.as_uuid();
        let held = |item: Uuid| {
            q.parts()
                .contains(ew_derive::ew_ids::v5::lib_copy_id(project, item))
        };
        std::iter::once(Uuid::new_v5(&copy, b"fork"))
            .chain((1u64..).map(|n| Uuid::new_v5(&copy, format!("fork\u{1f}{n}").as_bytes())))
            .find(|item| !held(*item))
            .expect("an unbounded sequence has a free item")
    }

    /// The parts of the system libraries (WP-S2.18 D6: the compiled-in generic starter parts, and any installed system
    /// library), as library documents with copy metadata, in library then item ID order. The picker and the Parts row
    /// list those the project does not hold after its embedded copies.
    pub fn library_parts(&self) -> Vec<LibCopy<Part>> {
        let reg = self.libraries();
        reg.libraries()
            .into_iter()
            .filter(|l| l.tier == library::Tier::System)
            .flat_map(|l| reg.documents(l.id, Kind::Part))
            .filter_map(|d| match d {
                LibDocument::Part(c) => Some(c),
                _ => None,
            })
            .collect()
    }

    /// The manufacturer a library part names, as the picker shows it: the held copy's name, else the library
    /// record's key; empty when neither resolves.
    pub fn library_manufacturer(&self, r: &LibRef) -> String {
        let held = ew_derive::manufacturer_name(&self.query(), r);
        if !held.is_empty() {
            return held;
        }
        match self.libraries().resolve(r) {
            Ok(LibDocument::Manufacturer(c)) => c.item.key.as_str().to_owned(),
            _ => String::new(),
        }
    }

    /// The reference to the library part whose copy would have ID `copy`, if a system library holds one
    /// ([`Session::library_parts`]).
    pub fn library_part_ref(&self, copy: Uuid) -> Option<LibRef> {
        self.library_parts()
            .into_iter()
            .find(|c| c.id.as_uuid() == copy)
            .map(|c| LibRef {
                library: Id::from_uuid(c.library_id.as_uuid()),
                item: Id::from_uuid(c.item_id.as_uuid()),
                version_req: "*".to_owned(),
                extra: ew_schema::Extra::new(),
            })
    }

    /// The Parts row's edit as `part.assign` (WP-S2.18 D6): when the new part uses `value` (`[{part: {copy}, qty}]`, the
    /// row's value) are exactly one use whose copy the project does not hold and a system library holds
    /// ([`Session::library_part_ref`]), the command that embeds the part with its closure and makes it the devices'
    /// only part use (`replace_all`, the use's quantity) in one transaction; `None` otherwise (the row's own
    /// `fields.set`). A list that names a library part beside other uses stays `fields.set`, which refuses the copy the
    /// project does not hold.
    pub fn part_uses_assign(
        &self,
        devices: &[Uuid],
        value: &serde_json::Value,
    ) -> Option<Result<AnyCommand, ApiError>> {
        let [one] = value.as_array()?.as_slice() else {
            return None;
        };
        let copy = one.get("part")?.get("copy")?.as_str()?.parse::<Uuid>().ok()?;
        if self.query().parts().contains(Id::from_uuid(copy)) {
            return None;
        }
        let r = self.library_part_ref(copy)?;
        let qty = match one.get("qty") {
            None => Dec3::ONE,
            Some(q) => q
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| q.to_string())
                .parse::<Dec3>()
                .ok()?,
        };
        Some(self.assign_part(devices, &r, AssignMode::ReplaceAll, qty))
    }

    /// `library.embed` by reference (FT-274, WP-S1.19 §8): resolves each reference with its closure from
    /// [`Session::libraries`] and builds the raw-document command; documents the project holds with the same hash are
    /// left out.
    ///
    /// # Errors
    ///
    /// [`ApiError::Library`] for a reference the libraries do not resolve; [`ApiError::OtherVersionHeld`] for an item
    /// held at another version; [`ApiError::NothingToDo`] when the project holds everything already.
    pub fn embed_command(&self, refs: &[LibRef]) -> Result<AnyCommand, ApiError> {
        let docs = self.closure_docs(refs)?;
        if docs.is_empty() {
            return Err(ApiError::NothingToDo);
        }
        let cmd = EmbedLibraryCopies {
            copies: docs.iter().map(to_value).collect(),
        };
        AnyCommand::of(&cmd).map_err(ApiError::Cmd)
    }

    /// `part.assign` by reference (D7, D8, FT-168): resolves the part with its closure (template FnDefs, manufacturer)
    /// and builds the command that embeds what the project lacks and sets the part uses of `devices` by `mode`.
    ///
    /// # Errors
    ///
    /// [`ApiError::Library`], [`ApiError::OtherVersionHeld`] as [`Session::embed_command`]; [`ApiError::NotAPart`]
    /// when the reference resolves to another kind.
    pub fn assign_part(
        &self,
        devices: &[Uuid],
        part: &LibRef,
        mode: AssignMode,
        qty: Dec3,
    ) -> Result<AnyCommand, ApiError> {
        let copy: Id<Part> = ew_derive::ew_ids::v5::lib_copy_id(part.library.as_uuid(), part.item.as_uuid());
        let item = self
            .libraries()
            .resolve(part)
            .map_err(|error| ApiError::Library {
                reference: ref_text(part),
                error,
            })?;
        if !matches!(item, LibDocument::Part(_)) {
            return Err(ApiError::NotAPart(copy.as_uuid()));
        }
        let docs = self.closure_docs(std::slice::from_ref(part))?;
        let (mode, from) = match mode {
            AssignMode::Append => (AssignModeParam::Append, None),
            AssignMode::ReplaceAll => (AssignModeParam::ReplaceAll, None),
            AssignMode::Replace { from } => (AssignModeParam::Replace, Some(Id::from_uuid(from))),
        };
        let cmd = AssignPart {
            devices: devices.iter().map(|d| Id::from_uuid(*d)).collect(),
            part: copy,
            copies: docs.iter().map(to_value).collect(),
            mode,
            from,
            qty: (qty != Dec3::ONE).then_some(qty),
        };
        AnyCommand::of(&cmd).map_err(ApiError::Cmd)
    }

    /// The library state of every held copy that came from a library (WP-S1.19 §8, FT-487's data), in kind then copy
    /// ID order: a local fork; else the item as [`Session::libraries`] holds it now: missing, deprecated (with its
    /// successor), the held hash (current), or another version (update available, with the class of the change; a
    /// lower library version counts as current). Project-local copies (the project's own items, `library_id` = the
    /// project ID) have no source library and are not listed.
    pub fn library_status(&self) -> Vec<CopyStatus> {
        let q = self.query();
        let project = q.project().id.as_uuid();
        let mut out = Vec::new();
        for doc in held_copies(&q) {
            let Some((kind, copy, library, item, version, sha)) = identity(&doc) else {
                continue;
            };
            let fork = is_fork(&doc);
            if library == project && !fork {
                continue;
            }
            let state = if fork {
                CopyState::LocalFork
            } else {
                let r = LibRef {
                    library: Id::from_uuid(library),
                    item: Id::from_uuid(item),
                    version_req: "*".to_owned(),
                    extra: ew_schema::Extra::new(),
                };
                match self.libraries().resolve(&r) {
                    Err(_) => CopyState::SourceMissing,
                    Ok(lib) => {
                        let (_, _, _, _, lib_version, lib_sha) = identity(&lib).expect("a typed document");
                        if let ItemStatus::Deprecated(d) = status_of(&lib) {
                            CopyState::Deprecated {
                                successor: d.successor,
                            }
                        } else if lib_sha == sha
                            || matches!((version_core(&lib_version), version_core(&version)), (Some(l), Some(h)) if l < h)
                        {
                            CopyState::Current
                        } else {
                            CopyState::UpdateAvailable {
                                class: classify_update(&doc, &lib).class,
                                version: lib_version,
                            }
                        }
                    }
                }
            };
            let (devices, _) = where_used(&q, &doc);
            out.push(CopyStatus {
                kind,
                copy,
                library,
                item,
                version,
                state,
                devices,
            });
        }
        out
    }

    /// The library copies of a fragment that the project holds in another version (WP-S2.07 D22, FT-396), in
    /// fragment order: what makes `edit.paste` refuse without `adapt`. `fragment` is `edit.paste`'s `fragment`
    /// member (`ParsedFragment::params`); a copy that does not read as a library document is left out (the paste
    /// refuses it). Empty when the paste needs no choice.
    pub fn paste_versions(&self, fragment: &serde_json::Value) -> Vec<VersionConflict> {
        let q = self.query();
        let docs: Vec<LibDocument> = fragment
            .get("copies")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| LibDocument::from_value(v.clone()).ok())
            .collect();
        let mut out = Vec::new();
        for doc in &docs {
            let Some((kind, copy, _, _, pasted, sha)) = identity(doc) else {
                continue;
            };
            let Some(h) = held(&q, kind, copy) else {
                continue;
            };
            let (_, _, _, _, held_version, held_sha) = identity(&h).expect("a held copy");
            if held_sha == sha {
                continue;
            }
            let newer = matches!(
                (version_core(&pasted), version_core(&held_version)),
                (Some(p), Some(h)) if p > h
            );
            out.push(VersionConflict {
                kind,
                copy,
                title: copy_title(&q, &h, &docs),
                held: held_version,
                pasted,
                class: classify_update(doc, &h).class,
                newer,
            });
        }
        out
    }

    /// The plan of "update from library" (D9, doc 07 §4.5, FR-PRT-014): for each held copy, the item as
    /// [`Session::libraries`] holds it now; copies whose hash did not change are left out. The plan carries the
    /// `library.update` command (Bulk: preview and commit it), each copy's class and field diff, the devices and
    /// representations touched, the component BOM rows that change and the printed tags and cross-reference texts
    /// that change, read from a preview of the command on a fork of the current snapshot.
    ///
    /// # Errors
    ///
    /// [`ApiError::NotHeld`] for a copy ID the project does not hold; [`ApiError::Library`] when no library holds the
    /// item; [`ApiError::NothingToDo`] when every copy is current; [`ApiError::Cmd`] when the command is refused (a
    /// downgrade, a connected CP left unmapped).
    pub fn update_from_library(&self, copies: &[Uuid], cp_maps: &[CpMap]) -> Result<UpdatePlan, ApiError> {
        let q = self.query();
        let mut pairs: Vec<(LibDocument, LibDocument)> = Vec::new();
        let mut embed: Vec<LibDocument> = Vec::new();
        let mut embed_seen: BTreeSet<Uuid> = BTreeSet::new();
        for copy in copies {
            let held_doc = held_any(&q, *copy).ok_or(ApiError::NotHeld(*copy))?;
            let (_, _, library, item, _, sha) = identity(&held_doc).expect("a held copy");
            let r = LibRef {
                library: Id::from_uuid(library),
                item: Id::from_uuid(item),
                version_req: "*".to_owned(),
                extra: ew_schema::Extra::new(),
            };
            let lib = self.libraries().resolve(&r).map_err(|error| ApiError::Library {
                reference: ref_text(&r),
                error,
            })?;
            let (_, _, _, _, _, lib_sha) = identity(&lib).expect("a typed document");
            if lib_sha == sha {
                continue;
            }
            // FT-640: the new version's closure that the project does not hold travels with the update.
            for d in self
                .libraries()
                .closure(&r)
                .unwrap_or_default()
                .into_iter()
                .skip(1)
            {
                let Some((kind, id, ..)) = identity(&d) else {
                    continue;
                };
                // a part's macro links are not what its update needs
                if kind != Kind::Macro && held_any(&q, id).is_none() && embed_seen.insert(id) {
                    embed.push(d);
                }
            }
            pairs.push((held_doc, lib));
        }
        self.update_plan(&q, pairs, &embed, cp_maps)
    }

    /// The plan of "update the project's copy" from documents (WP-S2.07 D22, FT-396): as
    /// [`Session::update_from_library`], with the new versions taken from `docs` (a pasted fragment's copies) instead
    /// of the installed libraries. A document the project holds with another hash is updated to it; one it holds with
    /// the same hash is left out; one it does not hold travels in the command's `embed` when an updated document names
    /// it (its manufacturer or template FnDefs, a symbol's FnDef; FT-640), else it is left for the paste to embed.
    ///
    /// # Errors
    ///
    /// [`ApiError::NothingToDo`] when the project holds no document of `docs` in another version; [`ApiError::Cmd`]
    /// when the command is refused (a downgrade, a connected CP left unmapped).
    pub fn update_from_documents(
        &self,
        docs: &[LibDocument],
        cp_maps: &[CpMap],
    ) -> Result<UpdatePlan, ApiError> {
        let q = self.query();
        let mut pairs: Vec<(LibDocument, LibDocument)> = Vec::new();
        let mut lacking: Vec<LibDocument> = Vec::new();
        for doc in docs {
            let Some((kind, copy, _, _, _, sha)) = identity(doc) else {
                continue;
            };
            match held(&q, kind, copy) {
                Some(h) if identity(&h).is_some_and(|i| i.5 != sha) => pairs.push((h, doc.clone())),
                Some(_) => {}
                None if kind != Kind::Macro => lacking.push(doc.clone()),
                None => {}
            }
        }
        let named: BTreeSet<(Kind, Uuid)> = pairs.iter().flat_map(|(_, new)| named_items(new)).collect();
        let embed: Vec<LibDocument> = lacking
            .into_iter()
            .filter(|d| identity(d).is_some_and(|(k, c, ..)| named.contains(&(k, c))))
            .collect();
        self.update_plan(&q, pairs, &embed, cp_maps)
    }

    /// [`Session::update_from_documents`] with the copies of a pasted fragment (`edit.paste`'s `fragment` member, as
    /// [`Session::paste_versions`] reads it): FT-396's "Update the project's copy" for a front end that holds the
    /// fragment as JSON. A copy that does not read as a library document is left out.
    ///
    /// # Errors
    ///
    /// As [`Session::update_from_documents`].
    pub fn paste_update(&self, fragment: &serde_json::Value) -> Result<UpdatePlan, ApiError> {
        let docs: Vec<LibDocument> = fragment
            .get("copies")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| LibDocument::from_value(v.clone()).ok())
            .collect();
        self.update_from_documents(&docs, &[])
    }

    /// The update plan of the pairs (held copy, new version) with the closure documents `embed` (FT-640): the
    /// `library.update` command, each copy's class and field diff, and what a preview on a throwaway engine changes.
    fn update_plan(
        &self,
        q: &Q,
        pairs: Vec<(LibDocument, LibDocument)>,
        embed: &[LibDocument],
        cp_maps: &[CpMap],
    ) -> Result<UpdatePlan, ApiError> {
        let mut docs = Vec::new();
        let mut plan_copies = Vec::new();
        let mut devices = BTreeSet::new();
        let mut reps = BTreeSet::new();
        for (held_doc, lib) in pairs {
            let (kind, copy, _, _, from, _) = identity(&held_doc).expect("a held copy");
            let (_, _, _, _, to, _) = identity(&lib).expect("a typed document");
            let diff = classify_update(&held_doc, &lib);
            let (d, rp) = where_used(q, &held_doc);
            devices.extend(d);
            reps.extend(rp);
            plan_copies.push(CopyUpdate {
                kind,
                copy,
                from,
                to,
                class: diff.class,
                fields: diff.fields,
                major_reasons: diff.major_reasons,
            });
            docs.push(lib);
        }
        if docs.is_empty() {
            return Err(ApiError::NothingToDo);
        }
        let cmd = UpdateLibraryCopies {
            copies: docs.iter().map(to_value).collect(),
            cp_maps: cp_maps
                .iter()
                .map(|m| ew_commands::commands::CpMapParam {
                    copy: Id::from_uuid(m.copy),
                    map: m.map.iter().map(|(a, b)| [*a, *b]).collect(),
                })
                .collect(),
            embed: embed.iter().map(to_value).collect(),
        };
        let command = AnyCommand::of(&cmd).map_err(ApiError::Cmd)?;
        // the preview on a throwaway engine over the current snapshot (the session's engine is not touched)
        let model = self.world().model.clone();
        let mut engine = ew_commands::Engine::new(
            model,
            ew_commands::EngineOptions {
                prevent: Some(Box::new(crate::ShippedPrevent)),
                ..ew_commands::EngineOptions::default()
            },
        );
        let preview = engine.preview(&command).map_err(ApiError::Cmd)?;
        let after = self.query_over(preview.candidate());
        let derived = ew_commands::derived_diff(q, &after);
        let bom = |q: &Q| -> Vec<String> {
            ew_reports::export::export_csv(q, Export::Bom(BomMode::Component))
                .map(|t| t.lines().skip(1).map(str::to_owned).collect())
                .unwrap_or_default()
        };
        let (before_rows, after_rows) = (bom(q), bom(&after));
        let bom_removed = before_rows
            .iter()
            .filter(|r| !after_rows.contains(r))
            .cloned()
            .collect();
        let bom_added = after_rows
            .iter()
            .filter(|r| !before_rows.contains(r))
            .cloned()
            .collect();
        Ok(UpdatePlan {
            command,
            copies: plan_copies,
            devices: devices.into_iter().collect(),
            representations: reps.into_iter().collect(),
            bom_removed,
            bom_added,
            derived,
        })
    }
}
