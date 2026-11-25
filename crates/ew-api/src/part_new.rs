// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A new part made in the window (`part.new`): [`Session::new_part_draft`] opens the part editor on an empty
//! draft; [`Session::new_part_command`] turns the draft into the part and the one command that brings it into the
//! project.
//!
//! **Where it is saved.** With [`PartDraft::to_library`] and a user library folder, the part goes into the person's
//! own parts library ("My parts", [`crate::eplan::USER_PARTS_KEY`], where the EPLAN import adds parts too) and the
//! project embeds that library copy (`library.embed`): the next project finds it there, and an edit in a project forks
//! it as any library part. Otherwise it is a project-local part (`parts.import`). A new part never overwrites another:
//! its item ID is new, and a part of the same number and manufacturer in the project or in My parts refuses the save
//! at the part number.
//!
//! **What it holds.** The identity, the technical values and the bus section the form gives. Its function template
//! is the one a placement needs: a part without pins whose first interface has a starter port function is drawn by
//! that port (a switch or a device with ports, placed as a box with every socket); any other part with bus interfaces
//! keeps no template, so its ports are placed from the device's row (Place port).

use ew_commands::commands::{EmbedLibraryCopies, ImportParts};
use ew_format::library_copy_hash;
use ew_ids::{Id, Uuid, v5};
use ew_library::starter;
use ew_mltext::Ident;
use ew_schema::{Kind, LibCopy, LibDocument, LibRef, Manufacturer, Part, Sha256, TemplateFn};
use serde_json::{Value, json};

use crate::part_editor::{BusRow, PartDraft, PartDraftError, bus_section, message};
use crate::{AnyCommand, Session};

/// What saving a new part does: the command that brings it into the project and the library it was added to.
#[derive(Clone, Debug)]
pub struct NewPart {
    /// `library.embed` of the library copy, or `parts.import` of a project-local part.
    pub command: AnyCommand,
    /// The name of the library the part was added to; `None` for a project-local part.
    pub library: Option<String>,
}

/// What taking a part out of My parts did ([`Session::remove_own_part`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RemovedPart {
    /// Its part number.
    pub number: String,
    /// The tags of the project's devices that keep using the project's copy of it, sorted.
    pub devices: Vec<String>,
}

/// The starter document of `key`.
pub(crate) fn starter_doc(key: &str) -> Option<LibDocument> {
    LibDocument::from_json(starter::item(key)?.json).ok()
}

/// The reference to starter FnDef copy `c`, with its key for the reader and the caret requirement of its version.
pub(crate) fn fndef_ref(c: &LibCopy<ew_schema::FnDef>) -> LibRef {
    let minor = c.version.split('.').take(2).collect::<Vec<_>>().join(".");
    let mut r = LibRef {
        library: Id::from_uuid(c.library_id.as_uuid()),
        item: Id::from_uuid(c.item_id.as_uuid()),
        version_req: format!("^{minor}"),
        extra: Default::default(),
    };
    if let Some(key) = &c.item.meta.key {
        r.extra
            .insert("key".into(), Value::String(key.as_str().to_owned()));
    }
    r
}

/// The starter port function that draws a port of `bus` (the FnDef of its starter port symbol), with its category.
fn port_fndef(bus: &ew_schema::BusSystem) -> Option<LibCopy<ew_schema::FnDef>> {
    let symbol = crate::output::bus_port_symbol_key(bus)?;
    let LibDocument::Symbol(s) = starter_doc(&symbol)? else {
        return None;
    };
    let key = s.item.fndef.extra.get("key")?.as_str()?.to_owned();
    match starter_doc(&key)? {
        LibDocument::Fndef(f) => Some(f),
        _ => None,
    }
}

impl Session {
    /// The draft of a new part (`part.new`): no manufacturer or number yet, kind device, unit piece, no bus interface,
    /// saved to My parts too.
    ///
    /// # Errors
    ///
    /// [`PartDraftError::Refused`] (`error-part-new-id`) when no new item ID can be drawn (no entropy).
    pub fn new_part_draft(&self) -> Result<PartDraft, PartDraftError> {
        let item = Id::<()>::try_new_v7()
            .map_err(|_| PartDraftError::Refused(key!("error-part-new-id")))?
            .as_uuid();
        let original: Part = serde_json::from_value(json!({
            "manufacturer": {"library": Uuid::nil(), "item": Uuid::nil(), "version_req": "*"},
            "part_number": "-",
        }))
        .expect("a minimal part");
        let fields = crate::part_editor::DRAFT_FIELDS
            .iter()
            .map(|f| {
                let text = match *f {
                    "kind" => "device",
                    "unit" => "piece",
                    _ => "",
                };
                (*f, text.to_owned())
            })
            .collect();
        Ok(PartDraft {
            copy: item,
            fields,
            tech: Vec::new(),
            size: crate::part_editor::size_rows(None),
            messages: Vec::new(),
            fork: false,
            library: None,
            version: "0.1.0".to_owned(),
            functions: Vec::new(),
            pins: Vec::new(),
            box_: None,
            outputs: Vec::new(),
            box_part: false,
            new: true,
            bus: Vec::<BusRow>::new(),
            to_library: true,
            own: false,
            users: 0,
            plc: crate::part_plc::PlcRows::new_part(),
            cable: crate::part_cable::CableRows::new_cable(),
            drawn_with: None,
            fndef: None,
            manufacturer: (original.manufacturer.clone(), Vec::new()),
            original,
        })
    }

    /// The draft of a new part drawn with the symbol at `address` (`library:<library>:<item>`, the symbol editor's
    /// saved symbol): the symbol's function definition becomes the part's main function, so a device placed from the
    /// part is drawn with that symbol.
    ///
    /// # Errors
    ///
    /// [`PartDraftError::Refused`] (`reason-part-symbol-unsaved`) when no installed library holds that symbol;
    /// as [`Session::new_part_draft`].
    pub fn new_part_draft_for_symbol(&self, address: &str) -> Result<PartDraft, PartDraftError> {
        let symbol = self
            .installed_symbol(address)
            .ok_or(PartDraftError::Refused(key!("reason-part-symbol-unsaved")))?;
        let mut draft = self.new_part_draft()?;
        let name = symbol
            .item
            .name
            .as_neutral()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                symbol
                    .item
                    .name
                    .languages()
                    .next()
                    .and_then(|l| symbol.item.name.get(l))
                    .unwrap_or_default()
                    .to_owned()
            });
        draft.drawn_with = Some(name);
        draft.fndef = Some(symbol.item.fndef.clone());
        Ok(draft)
    }

    /// The new part's item from a draft: the fields, the bus section and the function template a placement needs
    /// (see the module documentation); the manufacturer is the caller's.
    fn new_part_item(&self, draft: &PartDraft) -> Result<Part, PartDraftError> {
        let mut p = self.sectioned_item(draft)?;
        p.meta.key = Some(p.part_number.clone());
        Ok(p)
    }

    /// The item of a new part or of a part of the person's own (`PartDraft::own`): the identity fields over the held
    /// item, and the bus and PLC sections of the forms over the held sections, keeping every member the forms do not
    /// show (a port's connector and signal map, a station's I/O lengths, a valve terminal's positions). The pins and
    /// the function template are made again only when the PLC form's channels, pin pattern or supply pins changed;
    /// a template is given only to a part that has none.
    pub(crate) fn sectioned_item(&self, draft: &PartDraft) -> Result<Part, PartDraftError> {
        let (mut p, _) = self.draft_part(draft)?;
        let original = &draft.original;
        // A form left as it was leaves its section as it was, byte for byte.
        let held_bus: Vec<crate::part_editor::BusRow> = original
            .sections
            .bus
            .as_ref()
            .map(|b| b.ports.iter().map(crate::part_editor::BusRow::of).collect())
            .unwrap_or_default();
        if draft.new || draft.bus != held_bus {
            p.sections.bus =
                crate::part_editor::merge_bus(original.sections.bus.as_ref(), bus_section(&draft.bus)?);
        }
        if let Some(m) = crate::part_plc::check_plc(&draft.plc).into_iter().next() {
            return Err(PartDraftError::Field(m));
        }
        let held_plc = original.sections.plc.as_ref().map(crate::part_plc::rows_of);
        let untouched = !draft.new
            && draft.plc
                == held_plc
                    .clone()
                    .unwrap_or_else(crate::part_plc::PlcRows::new_part);
        let before = held_plc.unwrap_or_default();
        let pins_changed = !crate::part_plc::same_pins(&before, &draft.plc);
        let made = (!untouched)
            .then(|| crate::part_plc::plc_section(&draft.plc))
            .flatten();
        if !untouched && made.is_none() && draft.plc.role.trim().is_empty() {
            p.sections.plc = None;
        }
        // The PLC section and its pins: a black-box main function over the pins, as the starter cards carry.
        if let Some((mut section, cps)) = made {
            crate::part_plc::keep_unshown(&mut section, original.sections.plc.as_ref(), &draft.plc);
            p.sections.plc = Some(section);
            if pins_changed
                && !cps.is_empty()
                && let Some(LibDocument::Fndef(def)) = starter_doc(ew_schema::starter::FN_BLACKBOX)
            {
                p.cp_pattern = ew_schema::KeyedList::from_vec(cps).map_err(|e| {
                    PartDraftError::Field(message("plc.pin_of", key!("error-part-value"), &[&e.to_string()]))
                })?;
                p.function_template = vec![TemplateFn {
                    fndef: fndef_ref(&def),
                    main: true,
                    cps: p.cp_pattern.iter().map(|c| c.idx).collect(),
                    extra: Default::default(),
                }];
                if p.category.is_none() {
                    p.category = Some(def.item.category.clone());
                }
            }
        }
        // A part made for a symbol is drawn with it: its function definition is the main template entry.
        if p.function_template.is_empty()
            && let Some(r) = &draft.fndef
        {
            p.function_template = vec![TemplateFn {
                fndef: r.clone(),
                main: true,
                cps: Vec::new(),
                extra: Default::default(),
            }];
            if p.category.is_none()
                && let Ok(LibDocument::Fndef(def)) = self.libraries().resolve(r)
            {
                p.category = Some(def.item.category.clone());
            }
        }
        // A cable part's cores from its section, a section left as it was byte for byte; a new one without a
        // description reads as the cable does ("3G1.5 · BN BU GNYE").
        let held_cable =
            crate::part_cable::rows_of(&original.sections.cable_cores, original.sections.bus.as_ref());
        if p.kind == ew_schema::PartKind::Cable && (draft.new || draft.cable != held_cable) {
            p.sections.cable_cores = self.cable_cores(&draft.cable).map_err(PartDraftError::Field)?;
            // The bus systems the cable is made for (New bus cable…), beside the ports the section keeps.
            let cable_for: Vec<ew_schema::BusSystem> = draft
                .cable
                .bus
                .split_whitespace()
                .filter_map(|w| serde_json::from_value(Value::String(w.to_owned())).ok())
                .collect();
            let mut section = p.sections.bus.take().unwrap_or_default();
            section.cable_for = cable_for;
            p.sections.bus = (section != ew_schema::BusSection::default()).then_some(section);
            if draft.new
                && draft.text("description").trim().is_empty()
                && let Some(reads) = draft.cable.preview()
            {
                let series = draft.text("type_designation").trim();
                p.description.set_neutral(format!("{series} {reads}").trim());
            }
        }
        // A plug (an accessory with a bus interface) is no device drawn by its port: it has no template.
        let first_port = p.sections.bus.as_ref().and_then(|b| b.ports.first());
        if p.function_template.is_empty()
            && p.kind != ew_schema::PartKind::Accessory
            && let Some(def) = first_port.and_then(|port| port_fndef(&port.bus))
        {
            p.function_template = vec![TemplateFn {
                fndef: fndef_ref(&def),
                main: true,
                cps: Vec::new(),
                extra: Default::default(),
            }];
            if p.category.is_none() {
                p.category = Some(def.item.category.clone());
            }
        }
        self.apply_pins(draft, &mut p)?;
        Ok(p)
    }

    /// The part `number` of manufacturer `maker` among `parts` (part number and name compared without regard to case).
    fn same_part<'a>(&self, parts: impl IntoIterator<Item = &'a Part>, number: &str, maker: &str) -> bool {
        let q = self.query();
        parts.into_iter().any(|p| {
            p.part_number.as_str().eq_ignore_ascii_case(number)
                && ew_derive::manufacturer_name(&q, &p.manufacturer).eq_ignore_ascii_case(maker)
        })
    }

    /// Saves a new part (see the module documentation): into My parts and the command that embeds it, or the
    /// `parts.import` of a project-local part when the draft keeps it to the project or the window has no user
    /// library folder.
    ///
    /// # Errors
    ///
    /// [`PartDraftError::Field`] for the first field that does not parse; on `part_number` (`error-part-exists`) when
    /// the project or My parts holds a part of that number and manufacturer; on `part` when My parts cannot be written.
    pub fn new_part_command(&mut self, draft: &PartDraft) -> Result<NewPart, PartDraftError> {
        let mut part = self.new_part_item(draft)?;
        let maker = draft.text("manufacturer").trim().to_owned();
        let number = part.part_number.as_str().to_owned();
        let exists = || {
            PartDraftError::Field(message(
                "part_number",
                key!("error-part-exists"),
                &[&number, &maker],
            ))
        };
        {
            let q = self.query();
            let project = q.project().id.as_uuid();
            let held: Vec<Part> = q.parts().iter().map(|(_, c)| c.item.clone()).collect();
            let key_taken = q
                .parts()
                .iter()
                .any(|(_, c)| c.library_id.as_uuid() == project && c.item.meta.key == part.meta.key);
            if key_taken || self.same_part(&held, &number, &maker) {
                return Err(exists());
            }
        }
        let library = draft
            .to_library
            .then(|| crate::eplan::user_parts_library(self.libraries_mut()).ok())
            .flatten();
        let item = draft.copy;
        let Some(lib) = library else {
            return self.project_part(part, &maker, item);
        };
        let docs = self.libraries().documents(lib, Kind::Part);
        let mine: Vec<Part> = docs
            .iter()
            .filter_map(|d| match d {
                LibDocument::Part(c) => Some(c.item.clone()),
                _ => None,
            })
            .collect();
        if self.same_part(&mine, &number, &maker) {
            return Err(exists());
        }
        let mut written = Vec::new();
        part.manufacturer = match self
            .libraries()
            .documents(lib, Kind::Manufacturer)
            .iter()
            .find_map(|d| match d {
                LibDocument::Manufacturer(c) if maker_named(c, &maker) => {
                    Some(lib_ref(c.library_id.as_uuid(), c.item_id.as_uuid()))
                }
                _ => None,
            }) {
            Some(r) => r,
            None => {
                let copy = new_maker(lib, &maker)?;
                let r = lib_ref(lib, copy.item_id.as_uuid());
                written.push(LibDocument::Manufacturer(copy));
                r
            }
        };
        let copy = finish(
            LibCopy {
                id: v5::lib_copy_id(lib, item),
                v: 1,
                library_id: Id::from_uuid(lib),
                item_id: Id::from_uuid(item),
                version: draft.version.clone(),
                sha256: Sha256([0; 32]),
                item: part,
            },
            LibDocument::Part,
        );
        written.push(LibDocument::Part(copy.clone()));
        let name = self
            .libraries()
            .libraries()
            .into_iter()
            .find(|l| l.id == lib)
            .map(|l| l.name)
            .unwrap_or_default();
        self.libraries_mut().write(lib, &written).map_err(|e| {
            PartDraftError::Field(message(
                "part",
                key!("error-part-library"),
                &[&name, &e.to_string()],
            ))
        })?;
        let docs = self.closure_docs(&[lib_ref(lib, item)]).map_err(|e| {
            PartDraftError::Field(message(
                "part",
                key!("error-part-library"),
                &[&name, &e.to_string()],
            ))
        })?;
        let copies = docs.iter().filter_map(|d| serde_json::to_value(d).ok()).collect();
        let command = AnyCommand::of(&EmbedLibraryCopies { copies }).map_err(|e| {
            PartDraftError::Field(message("part", key!("error-part-value"), &[&e.to_string()]))
        })?;
        Ok(NewPart {
            command,
            library: Some(name),
        })
    }

    /// The `parts.import` of a project-local part: the manufacturer a part copy of the project names under that name,
    /// else a new project-local manufacturer record.
    /// Saves a corrected part of My parts (a held copy of the person's own library): the part with the forms'
    /// identity and sections as the library item's next patch version, written into My parts, and the
    /// `library.update` that brings the project's copy to it (Bulk: its card lists the changed fields and the devices,
    /// and the devices that use the copy take the new connection points; a bus port a function carries keeps its
    /// value, N5).
    ///
    /// # Errors
    ///
    /// [`PartDraftError::Field`] for the first field that does not parse, or on `part` when My parts cannot be
    /// written; [`PartDraftError::Refused`] (`reason-part-unchanged`) when nothing changed.
    pub fn own_part_update(
        &mut self,
        draft: &PartDraft,
    ) -> Result<crate::library_ops::UpdatePlan, PartDraftError> {
        let item = self.sectioned_item(draft)?;
        if item == draft.original {
            return Err(PartDraftError::Refused(key!("reason-part-unchanged")));
        }
        let held = {
            let q = self.query();
            q.parts()
                .get(Id::from_uuid(draft.copy))
                .map(|c| (**c).clone())
                .ok_or(PartDraftError::Refused(key!("reason-part-edit-target")))?
        };
        let lib = held.library_id.as_uuid();
        let name = draft.library.clone().unwrap_or_default();
        let failed =
            |e: String| PartDraftError::Field(message("part", key!("error-part-library"), &[&name, &e]));
        // The library's own version of the item: a later one written meanwhile is the base of the next.
        let current = self
            .libraries()
            .documents(lib, Kind::Part)
            .into_iter()
            .find_map(|d| match d {
                LibDocument::Part(c) if c.item_id == held.item_id => Some(c.version),
                _ => None,
            })
            .unwrap_or_else(|| held.version.clone());
        let version = ew_schema::next_patch(&current).ok_or_else(|| failed(current.clone()))?;
        let copy = finish(
            LibCopy {
                version,
                item,
                ..held
            },
            LibDocument::Part,
        );
        let doc = LibDocument::Part(copy);
        self.libraries_mut()
            .write(lib, std::slice::from_ref(&doc))
            .map_err(|e| failed(e.to_string()))?;
        let plan = self
            .update_from_documents(std::slice::from_ref(&doc), &[])
            .map_err(|e| failed(e.to_string()))?;
        Ok(plan)
    }

    /// The part number of part `item` when My parts, the person's own writable library, is `library` and holds it:
    /// what [`Session::remove_own_part`] can take out; `None` for any other.
    pub fn own_part_number(&self, library: Uuid, item: Uuid) -> Option<String> {
        let registry = self.libraries();
        registry
            .libraries()
            .iter()
            .any(|l| l.id == library && l.key == crate::eplan::USER_PARTS_KEY && l.writable)
            .then(|| {
                registry
                    .documents(library, Kind::Part)
                    .into_iter()
                    .find_map(|d| match d {
                        LibDocument::Part(c) if c.item_id.as_uuid() == item => {
                            Some(c.item.part_number.as_str().to_owned())
                        }
                        _ => None,
                    })
            })
            .flatten()
    }

    /// Takes part `item` out of My parts, the person's own library `library` (`part.remove_own`, doc 07
    /// §2.5): the next projects no longer offer it. A project that holds a copy of it keeps the copy, and its devices
    /// keep their part: each project embeds the parts it uses, so no project loses a part this way. The devices of
    /// this project that use its copy are named in the answer.
    ///
    /// # Errors
    ///
    /// [`PartDraftError::Refused`] (`reason-part-not-own`) when `library` is not My parts or holds no such part;
    /// [`PartDraftError::Field`] on `part` when My parts cannot be written (nothing is taken out then).
    pub fn remove_own_part(&mut self, library: Uuid, item: Uuid) -> Result<RemovedPart, PartDraftError> {
        let mine = self
            .libraries()
            .libraries()
            .into_iter()
            .find(|l| l.id == library && l.key == crate::eplan::USER_PARTS_KEY && l.writable);
        let (Some(mine), Some(number)) = (mine, self.own_part_number(library, item)) else {
            return Err(PartDraftError::Refused(key!("reason-part-not-own")));
        };
        self.libraries_mut()
            .remove(library, Kind::Part, &[item])
            .map_err(|e| {
                PartDraftError::Field(message(
                    "part",
                    key!("error-part-library"),
                    &[&mine.name, &e.to_string()],
                ))
            })?;
        let q = self.query();
        let held: Id<Part> = v5::lib_copy_id(library, item);
        let mut devices: Vec<String> = q
            .devices()
            .iter()
            .filter(|(_, d)| d.parts.iter().any(|u| u.part.copy == held))
            .map(|(id, _)| ew_derive::short_tag(&q, id))
            .collect();
        devices.sort();
        Ok(RemovedPart { number, devices })
    }

    /// A new part's draft with the identity, the technical data and the sections of the held copy `copy` (Copy as a
    /// part of your own: a starter part or another library's part, which the person cannot change): Save makes it a
    /// part of his own under a part number of its own.
    ///
    /// # Errors
    ///
    /// As [`Session::part_draft`] and [`Session::new_part_draft`].
    pub fn new_part_draft_from(&self, copy: Uuid) -> Result<PartDraft, PartDraftError> {
        let held = self
            .part_draft(copy)
            .map_err(|_| PartDraftError::Refused(key!("reason-part-edit-target")))?;
        self.new_part_draft_copying(&held)
    }

    /// [`Session::new_part_draft_from`] over the draft `held` of a part (a held copy's, or a library part's that the
    /// project does not hold, [`Session::library_part_draft`]).
    ///
    /// # Errors
    ///
    /// As [`Session::new_part_draft`].
    pub fn new_part_draft_copying(&self, held: &PartDraft) -> Result<PartDraft, PartDraftError> {
        let mut draft = self.new_part_draft()?;
        draft.fields = held.fields.clone();
        draft.tech = held.tech.clone();
        draft.bus = held.bus.clone();
        draft.plc = held.plc.clone();
        draft.cable = held.cable.clone();
        draft.pins = held.pins.clone();
        draft.box_ = held.box_.clone();
        draft.outputs = held.outputs.clone();
        draft.box_part = held.box_part;
        // The held item keeps what the forms do not show (pins, template, other sections); a new item ID and key.
        let mut original = held.original.clone();
        original.meta = Default::default();
        draft.original = original;
        draft.manufacturer = held.manufacturer.clone();
        Ok(draft)
    }

    fn project_part(&self, mut part: Part, maker: &str, item: Uuid) -> Result<NewPart, PartDraftError> {
        let q = self.query();
        let project = q.project().id.as_uuid();
        let mut copies = Vec::new();
        let held = q.manufacturers().iter().find_map(|(_, c)| {
            let r = lib_ref(c.library_id.as_uuid(), c.item_id.as_uuid());
            ew_derive::manufacturer_name(&q, &r)
                .eq_ignore_ascii_case(maker)
                .then_some(r)
        });
        part.manufacturer = match held {
            Some(r) => r,
            None => {
                let copy = new_maker(project, maker)?;
                let r = lib_ref(project, copy.item_id.as_uuid());
                copies.push(
                    serde_json::to_value(LibDocument::Manufacturer(copy))
                        .expect("a manufacturer has a JSON form"),
                );
                r
            }
        };
        let copy = finish(
            LibCopy {
                id: v5::lib_copy_id(project, item),
                v: 1,
                library_id: Id::from_uuid(project),
                item_id: Id::from_uuid(item),
                version: "0.1.0".to_owned(),
                sha256: Sha256([0; 32]),
                item: part,
            },
            LibDocument::Part,
        );
        copies.insert(
            0,
            serde_json::to_value(LibDocument::Part(copy)).expect("a part has a JSON form"),
        );
        let command = AnyCommand::of(&ImportParts { copies }).map_err(|e| {
            PartDraftError::Field(message("part", key!("error-part-value"), &[&e.to_string()]))
        })?;
        Ok(NewPart {
            command,
            library: None,
        })
    }
}

/// A reference to item `item` of library `library`.
fn lib_ref(library: Uuid, item: Uuid) -> LibRef {
    LibRef {
        library: Id::from_uuid(library),
        item: Id::from_uuid(item),
        version_req: "^0.1".to_owned(),
        extra: Default::default(),
    }
}

/// Whether manufacturer copy `c` is named `name` (its key or a name, without regard to case).
fn maker_named(c: &LibCopy<Manufacturer>, name: &str) -> bool {
    c.item.key.as_str().eq_ignore_ascii_case(name)
        || c.item
            .name
            .languages()
            .any(|l| c.item.name.get(l).is_some_and(|n| n.eq_ignore_ascii_case(name)))
        || c.item
            .name
            .as_neutral()
            .is_some_and(|n| n.eq_ignore_ascii_case(name))
}

/// A new manufacturer record `name` in library `library`: item `v5(library, "manufacturer:<name>")`.
fn new_maker(library: Uuid, name: &str) -> Result<LibCopy<Manufacturer>, PartDraftError> {
    let key = Ident::from_input(name).map_err(|e| {
        PartDraftError::Field(message(
            "manufacturer",
            key!("error-part-value"),
            &[&e.to_string()],
        ))
    })?;
    let item = v5::manufacturer_item_id(library, name);
    Ok(finish(
        LibCopy {
            id: v5::lib_copy_id(library, item),
            v: 1,
            library_id: Id::from_uuid(library),
            item_id: Id::from_uuid(item),
            version: "0.1.0".to_owned(),
            sha256: Sha256([0; 32]),
            item: Manufacturer {
                meta: Default::default(),
                key,
                name: ew_mltext::MlText::neutral(name),
                aliases: Vec::new(),
                brands: Vec::new(),
                extra: Default::default(),
            },
        },
        LibDocument::Manufacturer,
    ))
}

/// A copy with its hash (EW-CORE-014); `doc` makes the library document the hash is taken of.
fn finish<T: Clone>(mut copy: LibCopy<T>, doc: fn(LibCopy<T>) -> LibDocument) -> LibCopy<T> {
    copy.sha256 = Sha256(library_copy_hash(&doc(copy.clone())).unwrap_or([0; 32]));
    copy
}
