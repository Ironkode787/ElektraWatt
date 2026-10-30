// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The builder DSL (doc 20 §3.2 path (b), WP-S6.01): typed methods that create one record each through the
//! commands of doc 06 §3.1 and return the IDs the commands minted.
//!
//! Each method takes the record to create, with every reference already naming a record of the built
//! project, and issues the creating command with the record's members as parameters, then `fields.set`
//! (or the member's own command) for each further member the record carries. A member that no command
//! writes yet is not written: it is counted in [`Builder::unwritten`] under `<kind>.<member>`.
//!
//! ```text
//! let mut b = Builder::new(&mut sink);
//! let loc = b.structure_node(&node)?;                       // structure.create_node
//! let page = b.page(&page)?;                                // page.create (+ fields.set scale)
//! let (dev, main) = b.device(&device, Some(&main_fn))?;     // device.create (+ fields.set …)
//! let (rep, f) = b.place(&rep, Placement::Join { device: dev, def, role, kind })?;
//! let w = b.connect(&End::Cp(f, 0), &End::Cp(g, 1), Some(page), &kind)?.connection;
//! ```

use std::collections::BTreeMap;

use ew_ids::Id;
use ew_model::AnyKey;
use ew_schema::{
    AnyId, Bond, Channel, Conductor, Connection, ConnectionKind, Device, DeviceLink, Endpoint, ExternalRef,
    FnDef, FnRole, Function, FunctionKind, IpSpec, Junction, Kind, LibDocument, LogicalNet, Page, PageItem,
    Part, PartUse, PhysicalNet, PlcConfigProject, PotentialDef, Record, ReportBlock, Representation,
    Revision, RevisionState, Section, StructureBox, StructureNode, SymbolDef,
};
use ew_units::Pt2;
use serde_json::{Map, Value, json};

/// What a command created: its primary record and every key its transaction inserted, in insertion order
/// (the `Receipt` of `ew-commands`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Done {
    /// The record the command is about.
    pub primary: Option<AnyKey>,
    /// Every key the transaction inserted.
    pub created: Vec<AnyKey>,
}

/// Runs one command against the project being built: `name` is a command name of the registry, `params`
/// its JSON parameters. A bulk command runs as preview and commit. The error is the command's, as text.
pub trait CommandSink {
    /// Runs the command and returns what it created.
    ///
    /// # Errors
    ///
    /// The command's error, as text.
    fn run(&mut self, name: &str, params: Value) -> std::result::Result<Done, String>;
}

impl<F: FnMut(&str, Value) -> std::result::Result<Done, String>> CommandSink for F {
    fn run(&mut self, name: &str, params: Value) -> std::result::Result<Done, String> {
        self(name, params)
    }
}

/// A command that failed while building, with its parameters.
#[derive(Clone, PartialEq, Debug)]
pub struct BuildError {
    /// The command name.
    pub command: String,
    /// Its parameters.
    pub params: Value,
    /// What the command reported (or what the builder expected and did not get).
    pub message: String,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {} (params {})", self.command, self.message, self.params)
    }
}

impl std::error::Error for BuildError {}

/// The result of a builder step.
pub type Result<T> = std::result::Result<T, BuildError>;

/// One end of [`Builder::connect`]: a CP, a junction, or a point on a connection (which splits it and puts
/// a junction there).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum End {
    /// CP `idx` of a function.
    Cp(Id<Function>, u16),
    /// A junction.
    Junction(Id<Junction>),
    /// A point on a connection.
    On(Id<Connection>, Pt2),
}

impl End {
    /// The `EndpointParam` JSON of the commands.
    pub fn param(&self) -> Value {
        match self {
            End::Cp(f, idx) => json!({"cp": {"function": f, "idx": idx}}),
            End::Junction(j) => json!({"junction": {"junction": j}}),
            End::On(c, at) => json!({"on_connection": {"connection": c, "at": at}}),
        }
    }

    /// The end of a record endpoint (`Endpoint` JSON: `{"cp": [f, idx]}` or `{"junction": j}`).
    pub fn of_endpoint(endpoint: &Value) -> Option<End> {
        if let Some([f, idx]) = endpoint.get("cp").and_then(Value::as_array).map(Vec::as_slice) {
            let f: Id<Function> = serde_json::from_value(f.clone()).ok()?;
            return Some(End::Cp(f, u16::try_from(idx.as_u64()?).ok()?));
        }
        let j: Id<Junction> = serde_json::from_value(endpoint.get("junction")?.clone()).ok()?;
        Some(End::Junction(j))
    }
}

/// Which function a representation draws (see [`Builder::place`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Placement {
    /// Another representation of an existing function.
    Existing {
        /// The function.
        function: Id<Function>,
        /// Its kind (a black box is placed with `box.black`).
        kind: FunctionKind,
    },
    /// A new function of an existing device.
    Join {
        /// The device.
        device: Id<Device>,
        /// The FnDef copy.
        def: Id<FnDef>,
        /// The role.
        role: FnRole,
        /// The kind: normal (`device.place`) or black box (`box.black`).
        kind: FunctionKind,
    },
    /// A new interruption point (`ip.place`).
    Ip {
        /// The FnDef copy.
        def: Id<FnDef>,
        /// Name, direction, star source, sort key and external target.
        spec: IpSpec,
    },
}

/// The interruption point at the end of a wire (see [`Builder::draw_to_ip`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IpEnd {
    /// The IP symbol copy.
    pub symbol: Id<SymbolDef>,
    /// The FnDef copy; `None` takes the symbol's.
    pub def: Option<Id<FnDef>>,
    /// The insertion point.
    pub pos: Pt2,
    /// Name, direction, star source and sort key (`external` is ignored).
    pub spec: IpSpec,
}

/// What [`Builder::connect`] drew.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Drawn {
    /// The new connection.
    pub connection: Id<Connection>,
    /// The junction an `On` end created.
    pub junction: Option<Id<Junction>>,
    /// The second half of the connection an `On` end split (the first half keeps its ID).
    pub split: Option<Id<Connection>>,
}

/// The macro [`Builder::insert_macro`] inserts, and the values it fills in.
#[derive(Clone, PartialEq, Debug)]
pub struct MacroInsertion {
    /// The macro's copy ID.
    pub macro_copy: AnyId,
    /// The macro's document and the closure documents the project does not hold (`lib/` JSON form).
    pub copies: Vec<Value>,
    /// The instance, minted by the caller.
    pub instance: AnyId,
    /// The value set, by name.
    pub value_set: Option<String>,
    /// Explicit values, by variable name.
    pub values: Map<String, Value>,
}

/// The builder: runs commands through a [`CommandSink`] and records them.
pub struct Builder<'s, S: CommandSink + ?Sized> {
    sink: &'s mut S,
    script: Vec<(String, Value)>,
    unwritten: BTreeMap<String, usize>,
}

fn typed<T>(key: AnyKey) -> Id<T> {
    key.1.cast()
}

/// The record's members as a JSON object, without `$k`, `$v`, `id` and `source_ref`.
fn members(record: Record) -> Map<String, Value> {
    match serde_json::to_value(record) {
        Ok(Value::Object(mut m)) => {
            for k in ["$k", "$v", "id", "source_ref"] {
                m.remove(k);
            }
            m
        }
        _ => Map::new(),
    }
}

fn take(m: &mut Map<String, Value>, key: &str) -> Option<Value> {
    m.remove(key)
}

impl<'s, S: CommandSink + ?Sized> Builder<'s, S> {
    /// A builder over `sink`.
    pub fn new(sink: &'s mut S) -> Builder<'s, S> {
        Builder {
            sink,
            script: Vec::new(),
            unwritten: BTreeMap::new(),
        }
    }

    /// Runs one command and records it.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn run(&mut self, name: &str, params: Value) -> Result<Done> {
        match self.sink.run(name, params.clone()) {
            Ok(done) => {
                self.script.push((name.to_owned(), params));
                Ok(done)
            }
            Err(message) => Err(BuildError {
                command: name.to_owned(),
                params,
                message,
            }),
        }
    }

    /// Notes that `count` values of `what` (`<kind>.<member>` or a record kind) were not written because no
    /// command writes them yet.
    pub fn not_written(&mut self, what: &str, count: usize) {
        if count > 0 {
            *self.unwritten.entry(what.to_owned()).or_default() += count;
        }
    }

    /// What was not written, by `<kind>.<member>` or record kind.
    pub fn unwritten(&self) -> &BTreeMap<String, usize> {
        &self.unwritten
    }

    /// The commands that committed so far, in order, as `(name, params)` (a refused command is not listed).
    pub fn script(&self) -> &[(String, Value)] {
        &self.script
    }

    /// Consumes the builder: the commands issued and what was not written.
    pub fn finish(self) -> (Vec<(String, Value)>, BTreeMap<String, usize>) {
        (self.script, self.unwritten)
    }

    fn primary<T>(&mut self, name: &str, params: Value) -> Result<(Id<T>, Done)> {
        let done = self.run(name, params.clone())?;
        match done.primary {
            Some(key) => Ok((typed(key), done)),
            None => Err(BuildError {
                command: name.to_owned(),
                params,
                message: "the command named no primary record".into(),
            }),
        }
    }

    fn created<T>(done: &Done, kind: Kind) -> Vec<Id<T>> {
        done.created
            .iter()
            .filter(|k| k.0 == kind)
            .map(|k| typed(*k))
            .collect()
    }

    /// `fields.set` of one member path on records of one kind.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn set(&mut self, kind: &str, targets: &[AnyId], path: &str, value: Value) -> Result<()> {
        self.run(
            "fields.set",
            json!({"kind": kind, "targets": targets, "path": path, "value": value}),
        )
        .map(|_| ())
    }

    // WP-S2.05 part 2
    /// `fields.set_many` (WP-S2.05 part 2): one transaction of `fields.set` entries, each `(kind, targets, path,
    /// value)`, in the given order (one value per target set: find and replace, freezing several tags).
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn set_many(&mut self, edits: &[(&str, &[AnyId], &str, Value)]) -> Result<()> {
        let edits: Vec<Value> = edits
            .iter()
            .map(|(kind, targets, path, value)| {
                json!({"kind": kind, "targets": targets, "path": path, "value": value})
            })
            .collect();
        self.run("fields.set_many", json!({ "edits": edits })).map(|_| ())
    }

    // WP-S2.05 part 3
    /// `edit.paste` (WP-S2.05 part 3): pastes `fragment` (the command's `fragment` member, what
    /// `ew_api::output::parse_fragment` reads from clipboard text) onto `page` with its anchor at `at` (nm, a grid point
    /// of the page).
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn paste(&mut self, page: AnyId, at: [i64; 2], fragment: Value) -> Result<()> {
        self.run(
            "edit.paste",
            json!({"page": page, "at": at, "fragment": fragment}),
        )
        .map(|_| ())
    }

    /// `macro.insert` (FT-672): inserts `m` on `page` (a window macro: its anchor at `at`, nm, a grid point of the
    /// page, turned by `orient` in the command's JSON form, `r0` … `m270`; a page macro: its pages after `page`, with
    /// `at` `None` and `orient` `r0`). `r0` is left out of the parameters, as a log written before FT-672 has it.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn insert_macro(
        &mut self,
        page: AnyId,
        at: Option<[i64; 2]>,
        orient: &str,
        m: &MacroInsertion,
    ) -> Result<()> {
        let mut params = json!({
            "page": page,
            "macro": m.macro_copy,
            "copies": m.copies,
            "instance": m.instance,
            "at": at,
            "value_set": m.value_set,
            "values": m.values,
        });
        if orient != "r0" {
            params["orient"] = json!(orient);
        }
        self.run("macro.insert", params).map(|_| ())
    }

    /// `fields.set` of each remaining member of `rest` that is in `settable`; the others are noted as not
    /// written under `<kind>.<member>`.
    fn set_rest(&mut self, kind: &str, id: AnyId, rest: Map<String, Value>, settable: &[&str]) -> Result<()> {
        for (member, value) in rest {
            if settable.contains(&member.as_str()) {
                self.set(kind, &[id], &member, value)?;
            } else {
                self.not_written(&format!("{kind}.{member}"), 1);
            }
        }
        Ok(())
    }

    /// `library.embed` of `copies`, the non-local library documents of the project, in one command (the
    /// caller owns the closure, so the whole set goes in together). No command when `copies` is empty.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn embed_library(&mut self, copies: &[LibDocument]) -> Result<()> {
        if copies.is_empty() {
            return Ok(());
        }
        let docs = copies
            .iter()
            .map(|d| {
                serde_json::to_value(d).map_err(|e| BuildError {
                    command: "library.embed".into(),
                    params: Value::Null,
                    message: format!("a library document does not serialise: {e}"),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.run("library.embed", json!({"copies": docs})).map(|_| ())
    }

    /// `project.settings.set` of one member.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn setting(&mut self, member: &str, value: Value) -> Result<()> {
        self.run("project.settings.set", json!({"member": member, "value": value}))
            .map(|_| ())
    }

    /// `structure.create_node`, then `fields.set` of `order`.
    ///
    /// # Errors
    ///
    /// A command's error.
    pub fn structure_node(&mut self, node: &StructureNode) -> Result<Id<StructureNode>> {
        let mut m = members(Record::StructureNode(node.clone()));
        let mut p = Map::new();
        for k in ["aspect", "code", "parent", "aliases", "description"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        let (id, _) = self.primary::<StructureNode>("structure.create_node", Value::Object(p))?;
        self.set_rest("structure_node", id.cast(), m, &["order"])?;
        Ok(id)
    }

    /// `section.create`.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn section(&mut self, section: &Section) -> Result<Id<Section>> {
        let mut m = members(Record::Section(section.clone()));
        let mut p = Map::new();
        for k in ["number", "folio_range", "title", "divider"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        let (id, _) = self.primary::<Section>("section.create", Value::Object(p))?;
        self.set_rest("section", id.cast(), m, &[])?;
        Ok(id)
    }

    /// `external_ref.create`.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn external_ref(&mut self, r: &ExternalRef) -> Result<Id<ExternalRef>> {
        let mut m = members(Record::ExternalRef(r.clone()));
        let mut p = Map::new();
        for k in ["location", "device", "connection", "project"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        let (id, _) = self.primary::<ExternalRef>("external_ref.create", Value::Object(p))?;
        self.set_rest("external_ref", id.cast(), m, &[])?;
        Ok(id)
    }

    /// `page.create` with an explicit folio, then `fields.set` of `scale` and `description_override`.
    ///
    /// # Errors
    ///
    /// A command's error.
    pub fn page(&mut self, page: &Page) -> Result<Id<Page>> {
        let mut m = members(Record::Page(page.clone()));
        let mut p = Map::new();
        if let Some(Value::Object(folio)) = take(&mut m, "folio") {
            for (k, v) in folio {
                p.insert(k, v);
            }
        }
        for k in ["kind", "title", "structure", "form", "grid"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        let (id, _) = self.primary::<Page>("page.create", Value::Object(p))?;
        self.set_rest("page", id.cast(), m, &["scale", "description_override"])?;
        Ok(id)
    }

    /// `page.reorder`: moves `pages` before `before`, or to the end of `section` (`None`: front matter) when `before` is
    /// `None`.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn reorder_pages(
        &mut self,
        pages: &[Id<Page>],
        before: Option<Id<Page>>,
        section: Option<Id<Section>>,
    ) -> Result<()> {
        let mut p = Map::new();
        p.insert("pages".into(), json!(pages));
        match (before, section) {
            (Some(before), _) => {
                p.insert("before".into(), json!(before));
            }
            (None, Some(section)) => {
                p.insert("section".into(), json!(section));
            }
            (None, None) => {}
        }
        self.run("page.reorder", Value::Object(p)).map(|_| ())
    }

    /// `device.create` (with the main function `main`, a normal function, when given), then `fields.set`
    /// of `tag.frozen`, `process_tag`, `tech_data` and `state`. A terminal or cable is created with
    /// [`Builder::terminals`] or [`Builder::cable`] instead.
    ///
    /// # Errors
    ///
    /// A command's error.
    pub fn device(
        &mut self,
        device: &Device,
        main: Option<&Function>,
    ) -> Result<(Id<Device>, Option<Id<Function>>)> {
        let mut m = members(Record::Device(device.clone()));
        let mut p = Map::new();
        let tag = take(&mut m, "tag");
        if let Some(Value::Object(tag)) = &tag {
            if let Some(v) = tag.get("letter") {
                p.insert("letter".into(), v.clone());
            }
            if let Some(v) = tag.get("counter") {
                p.insert("counter".into(), v.clone());
            }
        }
        for k in ["kind", "parts", "function_text", "structure", "strip"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        if let Some(f) = main {
            p.insert("main_fndef".into(), json!(f.def));
        }
        let (id, done) = self.primary::<Device>("device.create", Value::Object(p))?;
        let function = Self::created::<Function>(&done, Kind::Function).first().copied();
        self.device_tag_rest(id, tag, &["letter", "counter"])?;
        self.set_rest("device", id.cast(), m, &["process_tag", "tech_data", "state"])?;
        if let (Some(f), Some(fid)) = (main, function) {
            self.function_rest(fid, f)?;
        }
        Ok((id, function))
    }

    /// The tag members not written by the creating command (those not in `covered`): `frozen` and `scheme`
    /// by `fields.set`; any other member is noted as not written.
    fn device_tag_rest(&mut self, id: Id<Device>, tag: Option<Value>, covered: &[&str]) -> Result<()> {
        let Some(Value::Object(tag)) = tag else {
            return Ok(());
        };
        for (member, value) in tag {
            if covered.contains(&member.as_str()) {
                continue;
            }
            if member == "frozen" || member == "scheme" {
                self.set("device", &[id.cast()], &format!("tag.{member}"), value)?;
            } else {
                self.not_written(&format!("device.tag.{member}"), 1);
            }
        }
        Ok(())
    }

    /// `fields.set` of a function's `cp_overrides`; its other members are written by the creating command.
    fn function_rest(&mut self, id: Id<Function>, f: &Function) -> Result<()> {
        let mut m = members(Record::Function(f.clone()));
        for k in ["device", "def", "role", "kind"] {
            m.remove(k);
        }
        self.set_rest("function", id.cast(), m, &["cp_overrides", "bus_port"])
    }

    /// `terminal.add` of `terminals` (devices with the terminal aspect, all of the strip `strip` and with
    /// main functions of `fndef`), then `fields.set` of the members `terminal.add` does not take: `parts`,
    /// `state`, `process_tag`, `tech_data`, and the tag and structure where they differ from what
    /// `terminal.add` gives (the strip's letter with the sequence number as counter, the strip's structure).
    ///
    /// # Errors
    ///
    /// A command's error, or a count mismatch.
    pub fn terminals(
        &mut self,
        strip: &Device,
        strip_id: Id<Device>,
        fndef: Id<FnDef>,
        terminals: &[&Device],
    ) -> Result<Vec<(Id<Device>, Id<Function>)>> {
        let mut specs = Vec::new();
        let mut rests = Vec::new();
        for t in terminals {
            let mut m = members(Record::Device((*t).clone()));
            let mut spec = Map::new();
            if let Some(Value::Object(aspect)) = take(&mut m, "terminal") {
                for (k, v) in aspect {
                    match k.as_str() {
                        "designation" | "seq" | "levels" | "rail_bonded" => {
                            spec.insert(k, v);
                        }
                        "strip" => {}
                        _ => self.not_written(&format!("device.terminal.{k}"), 1),
                    }
                }
            }
            if let Some(v) = take(&mut m, "function_text") {
                spec.insert("function_text".into(), v);
            }
            specs.push(Value::Object(spec));
            rests.push((m, t.aspects.terminal.as_ref().map_or(0, |a| a.seq)));
        }
        let params = json!({"strip": strip_id, "fndef": fndef, "terminals": specs});
        let done = self.run("terminal.add", params.clone())?;
        let devices = Self::created::<Device>(&done, Kind::Device);
        let functions = Self::created::<Function>(&done, Kind::Function);
        if devices.len() != terminals.len() || functions.len() != terminals.len() {
            return Err(BuildError {
                command: "terminal.add".into(),
                params,
                message: format!(
                    "{} terminals asked, {} devices and {} functions created",
                    terminals.len(),
                    devices.len(),
                    functions.len()
                ),
            });
        }
        let strip_members = members(Record::Device(strip.clone()));
        let strip_letter = strip.tag.letter.clone();
        for ((mut m, seq), id) in rests.into_iter().zip(devices.iter().copied()) {
            // the tag: terminal.add gives {letter: the strip's, counter: seq}
            if let Some(Value::Object(tag)) = take(&mut m, "tag") {
                let letter = tag.get("letter").cloned();
                if letter != Some(json!(strip_letter)) {
                    self.set(
                        "device",
                        &[id.cast()],
                        "tag.letter",
                        letter.unwrap_or(Value::Null),
                    )?;
                }
                let counter = tag.get("counter").cloned();
                if counter != Some(json!(seq)) {
                    self.set(
                        "device",
                        &[id.cast()],
                        "tag.counter",
                        counter.unwrap_or(Value::Null),
                    )?;
                }
                let rest: Map<String, Value> = tag
                    .into_iter()
                    .filter(|(k, _)| k != "letter" && k != "counter")
                    .collect();
                self.device_tag_rest(id, Some(Value::Object(rest)), &[])?;
            }
            // terminal.add copies the strip's structure and sets the kind `terminal`
            let structure = take(&mut m, "structure");
            if structure != strip_members.get("structure").cloned() {
                self.set(
                    "device",
                    &[id.cast()],
                    "structure",
                    structure.unwrap_or(Value::Null),
                )?;
            }
            if take(&mut m, "kind") != Some(json!("terminal")) {
                self.not_written("device.kind", 1);
            }
            self.set_rest(
                "device",
                id.cast(),
                m,
                &["parts", "state", "process_tag", "tech_data"],
            )?;
        }
        Ok(devices.into_iter().zip(functions).collect())
    }

    // WP-S2.07 part 1
    /// `terminal.insert` (WP-S2.07 §5.1): the `terminals` (`TerminalSpec` JSON objects without `seq`) of the strip
    /// `strip`, with main functions of `fndef` and the part uses `parts` on each, put before the terminal `before`, or
    /// after the strip's last terminal when `None`; the following terminals, group bounds and accessories move up.
    /// Returns the first new terminal device (the command's primary record).
    ///
    /// # Errors
    ///
    /// The command's error, or no primary record.
    pub fn terminal_insert(
        &mut self,
        strip: Id<Device>,
        before: Option<Id<Device>>,
        fndef: Id<FnDef>,
        terminals: &[Value],
        parts: &[PartUse],
    ) -> Result<Id<Device>> {
        let mut p = Map::new();
        p.insert("strip".into(), json!(strip));
        if let Some(before) = before {
            p.insert("before".into(), json!(before));
        }
        p.insert("fndef".into(), json!(fndef));
        p.insert("terminals".into(), json!(terminals));
        if !parts.is_empty() {
            p.insert("parts".into(), json!(parts));
        }
        self.primary::<Device>("terminal.insert", Value::Object(p))
            .map(|(id, _)| id)
    }

    /// `terminal.move` (WP-S2.07 §5.2): the terminals `terminals` of one strip put before the terminal `before`, or
    /// after the strip's last terminal when `None`; the sequence numbers of the changed range are permuted.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn terminal_move(&mut self, terminals: &[Id<Device>], before: Option<Id<Device>>) -> Result<()> {
        let mut p = Map::new();
        p.insert("terminals".into(), json!(terminals));
        if let Some(before) = before {
            p.insert("before".into(), json!(before));
        }
        self.run("terminal.move", Value::Object(p)).map(|_| ())
    }

    // WP-S1.19 part 3
    /// `part.assign` (WP-S1.19 D8): the part copy `part` assigned to `devices`, embedding the library documents
    /// `copies` the project lacks (the part's closure; empty when the project holds it). `mode` is `"append"`,
    /// `"replace_all"` or `"replace"` (with `from`, the part copy it replaces); `qty` defaults to 1.
    ///
    /// # Errors
    ///
    /// The command's error, or a document that does not serialise.
    pub fn assign_part(
        &mut self,
        devices: &[Id<Device>],
        part: Id<ew_schema::Part>,
        copies: &[LibDocument],
        mode: &str,
        from: Option<Id<ew_schema::Part>>,
        qty: Option<&str>,
    ) -> Result<()> {
        let mut p = Map::new();
        p.insert("devices".into(), json!(devices));
        p.insert("part".into(), json!(part));
        if !copies.is_empty() {
            p.insert(
                "copies".into(),
                Value::Array(self.lib_values("part.assign", copies)?),
            );
        }
        p.insert("mode".into(), json!(mode));
        if let Some(from) = from {
            p.insert("from".into(), json!(from));
        }
        if let Some(qty) = qty {
            p.insert("qty".into(), json!(qty));
        }
        self.run("part.assign", Value::Object(p)).map(|_| ())
    }

    /// `library.update` (WP-S1.19 D9): the held copies replaced by the documents `copies` (same copy IDs), with the
    /// CP maps `cp_maps` (`(copy, [(old_idx, new_idx)])`). The command is Bulk: the sink previews and commits it.
    ///
    /// # Errors
    ///
    /// The command's error, or a document that does not serialise.
    pub fn update_library(
        &mut self,
        copies: &[LibDocument],
        cp_maps: &[(ew_ids::Uuid, Vec<(u16, u16)>)],
    ) -> Result<()> {
        let mut p = Map::new();
        p.insert(
            "copies".into(),
            Value::Array(self.lib_values("library.update", copies)?),
        );
        if !cp_maps.is_empty() {
            let maps: Vec<Value> = cp_maps
                .iter()
                .map(|(copy, map)| {
                    let pairs: Vec<[u16; 2]> = map.iter().map(|(a, b)| [*a, *b]).collect();
                    json!({"copy": copy.to_string(), "map": pairs})
                })
                .collect();
            p.insert("cp_maps".into(), Value::Array(maps));
        }
        self.run("library.update", Value::Object(p)).map(|_| ())
    }

    // WP-S2.07 part 4a
    /// `part.edit_copy` (WP-S2.07 D20): the part copy `copy` edited to the part item `part` (its members, without the
    /// copy metadata), in place for a project-local copy, else forked to the item `fork`; `copies` are library
    /// documents the edit names that the project lacks (a manufacturer). The command is Bulk: the sink previews and
    /// commits it. Returns the edited copy (the fork's for a fork).
    ///
    /// # Errors
    ///
    /// The command's error, or a document that does not serialise.
    pub fn part_edit_copy(
        &mut self,
        copy: Id<ew_schema::Part>,
        part: Value,
        fork: Option<ew_ids::Uuid>,
        copies: &[LibDocument],
    ) -> Result<Id<ew_schema::Part>> {
        let mut p = Map::new();
        p.insert("copy".into(), json!(copy));
        p.insert("part".into(), part);
        if let Some(fork) = fork {
            p.insert("fork".into(), json!(fork.to_string()));
        }
        if !copies.is_empty() {
            p.insert(
                "copies".into(),
                Value::Array(self.lib_values("part.edit_copy", copies)?),
            );
        }
        self.primary::<ew_schema::Part>("part.edit_copy", Value::Object(p))
            .map(|(id, _)| id)
    }

    /// Library documents as command parameter values.
    fn lib_values(&self, command: &str, docs: &[LibDocument]) -> Result<Vec<Value>> {
        docs.iter()
            .map(|d| {
                serde_json::to_value(d).map_err(|e| BuildError {
                    command: command.into(),
                    params: Value::Null,
                    message: format!("a library document does not serialise: {e}"),
                })
            })
            .collect()
    }

    /// `cable.create` with its conductors, then `fields.set` of the remaining device members.
    ///
    /// # Errors
    ///
    /// A command's error, or a count mismatch.
    pub fn cable(
        &mut self,
        cable: &Device,
        conductors: &[&Conductor],
    ) -> Result<(Id<Device>, Vec<Id<Conductor>>)> {
        let mut m = members(Record::Device(cable.clone()));
        let mut p = Map::new();
        let tag = take(&mut m, "tag");
        if let Some(Value::Object(tag)) = &tag {
            if let Some(v) = tag.get("letter") {
                p.insert("letter".into(), v.clone());
            }
            if let Some(v) = tag.get("counter") {
                p.insert("counter".into(), v.clone());
            }
        }
        for k in ["parts", "function_text", "structure", "cable"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        let mut specs = Vec::new();
        for c in conductors {
            let mut cm = members(Record::Conductor((*c).clone()));
            cm.remove("cable");
            let mut spec = Map::new();
            for k in ["core", "colour", "csa", "shield", "pair"] {
                if let Some(v) = take(&mut cm, k) {
                    spec.insert(k.into(), v);
                }
            }
            for k in cm.keys() {
                self.not_written(&format!("conductor.{k}"), 1);
            }
            specs.push(Value::Object(spec));
        }
        p.insert("conductors".into(), Value::Array(specs));
        let params = Value::Object(p);
        let (id, done) = self.primary::<Device>("cable.create", params.clone())?;
        let made = Self::created::<Conductor>(&done, Kind::Conductor);
        if made.len() != conductors.len() {
            return Err(BuildError {
                command: "cable.create".into(),
                params,
                message: format!("{} conductors asked, {} created", conductors.len(), made.len()),
            });
        }
        self.device_tag_rest(id, tag, &["letter", "counter"])?;
        self.set_rest(
            "device",
            id.cast(),
            m,
            &["kind", "process_tag", "tech_data", "state"],
        )?;
        Ok((id, made))
    }

    /// Places `rep` (its `page`, `symbol`, `pos`, `orient`, `rep_type`, `variant` and `box`) drawing the
    /// function of `placement`: `ip.place` for a new interruption point, `box.black` for a black-box
    /// function, `device.place` otherwise. Then `fields.set` of `text_slots` and `arrangement`, and
    /// `rep.set_anchor_policy` of `anchor`. Returns the representation and the function it draws.
    ///
    /// # Errors
    ///
    /// A command's error.
    pub fn place(
        &mut self,
        rep: &Representation,
        placement: &Placement,
    ) -> Result<(Id<Representation>, Id<Function>)> {
        let mut m = members(Record::Representation(rep.clone()));
        m.remove("functions");
        let mut p = Map::new();
        for k in ["page", "symbol", "pos", "orient"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        let rep_type = take(&mut m, "rep_type");
        let variant = take(&mut m, "variant");
        let bx = take(&mut m, "box");
        let black = match placement {
            Placement::Existing { kind, .. } | Placement::Join { kind, .. } => {
                *kind == FunctionKind::BlackBox
            }
            Placement::Ip { .. } => false,
        };
        let target = match placement {
            Placement::Existing { function, .. } => json!({"existing_function": {"function": function}}),
            Placement::Join {
                device, def, role, ..
            } => {
                json!({"join_device": {"device": device, "fndef": def, "role": role}})
            }
            Placement::Ip { .. } => Value::Null,
        };
        let (rep_id, function) = match placement {
            Placement::Ip { def, spec } => {
                p.insert("fndef".into(), json!(def));
                if let Ok(Value::Object(s)) = serde_json::to_value(spec) {
                    for (k, v) in s {
                        p.insert(k, v);
                    }
                }
                if rep_type.as_ref().is_some_and(|t| t != "multi_line") {
                    self.not_written("representation.rep_type", 1);
                }
                if variant.is_some() {
                    self.not_written("representation.variant", 1);
                }
                if bx.is_some() {
                    self.not_written("representation.box", 1);
                }
                let (f, done) = self.primary::<Function>("ip.place", Value::Object(p))?;
                let rep = Self::created::<Representation>(&done, Kind::Representation)
                    .first()
                    .copied()
                    .ok_or_else(|| BuildError {
                        command: "ip.place".into(),
                        params: Value::Null,
                        message: "no representation created".into(),
                    })?;
                (rep, f)
            }
            _ => {
                if let Some(v) = rep_type {
                    p.insert("rep_type".into(), v);
                }
                p.insert("target".into(), target);
                let name = if black {
                    if let Some(b) = bx {
                        p.insert("box".into(), b);
                    }
                    if variant.is_some() {
                        self.not_written("representation.variant", 1);
                    }
                    "box.black"
                } else {
                    if let Some(v) = variant {
                        p.insert("variant".into(), v);
                    }
                    if bx.is_some() {
                        self.not_written("representation.box", 1);
                    }
                    "device.place"
                };
                let (rep, done) = self.primary::<Representation>(name, Value::Object(p))?;
                let f = match placement {
                    Placement::Existing { function, .. } => *function,
                    _ => Self::created::<Function>(&done, Kind::Function)
                        .first()
                        .copied()
                        .ok_or_else(|| BuildError {
                            command: name.into(),
                            params: Value::Null,
                            message: "no function created".into(),
                        })?,
                };
                (rep, f)
            }
        };
        if let Some(anchor) = take(&mut m, "anchor") {
            self.run(
                "rep.set_anchor_policy",
                json!({"reps": [rep_id], "anchor": anchor}),
            )?;
        }
        self.set_rest("representation", rep_id.cast(), m, &["text_slots", "arrangement"])?;
        Ok((rep_id, function))
    }

    /// `fields.set` of the function members its creating command did not write (`cp_overrides`).
    ///
    /// # Errors
    ///
    /// A command's error.
    pub fn function_details(&mut self, id: Id<Function>, f: &Function) -> Result<()> {
        self.function_rest(id, f)
    }

    /// `connection.draw` of one connection.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn connect(
        &mut self,
        a: &End,
        b: &End,
        page: Option<Id<Page>>,
        kind: &ConnectionKind,
    ) -> Result<Drawn> {
        let params = json!({"a": a.param(), "b": b.param(), "page": page, "kind": kind});
        let (connection, done) = self.primary::<Connection>("connection.draw", params)?;
        let junction = Self::created::<Junction>(&done, Kind::Junction).first().copied();
        let split = Self::created::<Connection>(&done, Kind::Connection)
            .into_iter()
            .find(|c| *c != connection);
        Ok(Drawn {
            connection,
            junction,
            split,
        })
    }

    /// `connection.draw_many` (cycle 33): page-less connections of one `kind` between each pair of `ends`, in one
    /// transaction (a chain of bridges).
    ///
    /// # Errors
    ///
    /// A command's error.
    pub fn connect_many(&mut self, ends: &[(End, End)], kind: &ConnectionKind) -> Result<()> {
        let connections: Vec<Value> = ends
            .iter()
            .map(|(a, b)| json!({"a": a.param(), "b": b.param(), "kind": kind}))
            .collect();
        self.run("connection.draw_many", json!({"connections": connections}))
            .map(|_| ())
    }

    /// `fields.set` of a connection's `overrides` members and `number`; its ends, page and kind are written
    /// by `connection.draw`; a stored `route` is noted as not written.
    ///
    /// # Errors
    ///
    /// A command's error.
    pub fn connection_details(&mut self, id: Id<Connection>, c: &Connection) -> Result<()> {
        let mut m = members(Record::Connection(c.clone()));
        for k in ["a", "b", "page", "kind"] {
            m.remove(k);
        }
        // The builder draws without a route (a junction's connections come from a split); a stored route
        // is not written, for every connection alike (`connection.route` would need the drawn geometry).
        if take(&mut m, "route").is_some() {
            self.not_written("connection.route", 1);
        }
        if let Some(Value::Object(o)) = take(&mut m, "overrides") {
            for (k, v) in o {
                self.set("connection", &[id.cast()], &format!("overrides.{k}"), v)?;
            }
        }
        self.set_rest("connection", id.cast(), m, &["number"])
    }

    /// `junction.edit` of a junction's style, wiring and sequence (the junction itself is created by the
    /// `connection.draw` that splits a connection at its point).
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn junction(&mut self, id: Id<Junction>, j: &Junction) -> Result<()> {
        let mut p = Map::new();
        p.insert("junction".into(), json!(id));
        let m = members(Record::Junction(j.clone()));
        if let Some(style) = m.get("style") {
            p.insert("style".into(), style.clone());
        }
        p.insert(
            "wiring".into(),
            m.get("wiring").cloned().unwrap_or(json!("style_default")),
        );
        if let Some(seq) = m.get("sequence") {
            p.insert("sequence".into(), seq.clone());
        }
        self.run("junction.edit", Value::Object(p)).map(|_| ())
    }

    /// `potential.define`.
    ///
    /// # Errors
    ///
    /// The command's error, or an `at` that is not a CP or junction.
    pub fn potential(&mut self, d: &PotentialDef) -> Result<Id<PotentialDef>> {
        let mut m = members(Record::PotentialDef(d.clone()));
        let at = take(&mut m, "at").and_then(|v| End::of_endpoint(&v));
        let Some(at) = at else {
            return Err(BuildError {
                command: "potential.define".into(),
                params: Value::Null,
                message: "the definition point is not a CP or junction".into(),
            });
        };
        let mut p = Map::new();
        p.insert("at".into(), at.param());
        for k in ["name", "ptype", "voltage", "env", "circuit_class", "props"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        let (id, _) = self.primary::<PotentialDef>("potential.define", Value::Object(p))?;
        self.set_rest("potential_def", id.cast(), m, &[])?;
        Ok(id)
    }

    /// `bond.create`.
    ///
    /// # Errors
    ///
    /// The command's error, or an end that is not a CP or junction.
    pub fn bond(&mut self, bond: &Bond) -> Result<Id<Bond>> {
        let mut m = members(Record::Bond(bond.clone()));
        let a = take(&mut m, "a").and_then(|v| End::of_endpoint(&v));
        let b = take(&mut m, "b").and_then(|v| End::of_endpoint(&v));
        let (Some(a), Some(b)) = (a, b) else {
            return Err(BuildError {
                command: "bond.create".into(),
                params: Value::Null,
                message: "an end is not a CP or junction".into(),
            });
        };
        let mut p = Map::new();
        p.insert("a".into(), a.param());
        p.insert("b".into(), b.param());
        for k in ["kind", "via"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        let (id, _) = self.primary::<Bond>("bond.create", Value::Object(p))?;
        self.set_rest("bond", id.cast(), m, &[])?;
        Ok(id)
    }

    /// `report_block.create`.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn report_block(&mut self, block: &ReportBlock) -> Result<Id<ReportBlock>> {
        let mut m = members(Record::ReportBlock(block.clone()));
        let mut p = Map::new();
        for k in [
            "source",
            "form",
            "section",
            "first_folio",
            "title",
            "manual",
            "sort",
            "group",
            "continuation",
            "empty",
            "embedded",
        ] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        let (id, _) = self.primary::<ReportBlock>("report_block.create", Value::Object(p))?;
        self.set_rest("report_block", id.cast(), m, &[])?;
        Ok(id)
    }

    /// `assembly.attach` of `children` (device and position) under `parent`.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn attach(&mut self, parent: Id<Device>, children: &[(Id<Device>, Option<u32>)]) -> Result<()> {
        let children: Vec<Value> = children
            .iter()
            .map(|(device, position)| match position {
                Some(p) => json!({"device": device, "position": p}),
                None => json!({"device": device}),
            })
            .collect();
        self.run("assembly.attach", json!({"parent": parent, "children": children}))
            .map(|_| ())
    }

    /// `valve.add` of a valve child `device` (child tag scheme, its counter equal to its position) on the
    /// module `module`, with the main function `main`; then `fields.set` of the members `valve.add` does
    /// not take (`function_text`, `state`, `process_tag`, `tech_data`, `kind`, further part uses).
    ///
    /// # Errors
    ///
    /// A command's error, or a device that is not a valve child (no position, no part).
    pub fn valve(
        &mut self,
        device: &Device,
        module: Id<Device>,
        main: Option<&Function>,
    ) -> Result<(Id<Device>, Option<Id<Function>>)> {
        let mut m = members(Record::Device(device.clone()));
        for k in ["parent", "position"] {
            m.remove(k);
        }
        let (Some(position), Some(first)) = (device.position, device.parts.first()) else {
            return Err(BuildError {
                command: "valve.add".into(),
                params: Value::Null,
                message: "a valve child has a position and a part".into(),
            });
        };
        let mut p = Map::new();
        p.insert("module".into(), json!(module));
        p.insert("position".into(), json!(position));
        p.insert("part".into(), json!(first));
        p.insert("letter".into(), json!(device.tag.letter));
        if let Some(f) = main {
            p.insert("fndef".into(), json!(f.def));
        }
        if let Some(v) = take(&mut m, "structure") {
            p.insert("structure".into(), v);
        }
        let parts = take(&mut m, "parts");
        let tag = take(&mut m, "tag");
        let (id, done) = self.primary::<Device>("valve.add", Value::Object(p))?;
        let function = Self::created::<Function>(&done, Kind::Function).first().copied();
        if device.parts.len() > 1 {
            self.set("device", &[id.cast()], "parts", parts.unwrap_or(Value::Null))?;
        }
        self.device_tag_rest(id, tag, &["letter", "counter", "scheme"])?;
        self.set_rest(
            "device",
            id.cast(),
            m,
            &["function_text", "state", "process_tag", "tech_data", "kind"],
        )?;
        if let (Some(f), Some(fid)) = (main, function) {
            self.function_rest(fid, f)?;
        }
        Ok((id, function))
    }

    /// `fields.set` of one device aspect (`assembly`, `plc_station`, `plc_module`) as a whole; returns what
    /// the command created (the channels a new `plc_module` instantiates).
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn aspect(&mut self, device: Id<Device>, aspect: &str, value: Value) -> Result<Done> {
        self.run(
            "fields.set",
            json!({"kind": "device", "targets": [device], "path": aspect, "value": value}),
        )
    }

    /// `plc.config_project.create`.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn config_project(&mut self, cfg: &PlcConfigProject) -> Result<Id<PlcConfigProject>> {
        let mut m = members(Record::PlcConfigProject(cfg.clone()));
        let mut p = Map::new();
        for k in ["name", "profile", "cpu"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        let (id, _) = self.primary::<PlcConfigProject>("plc.config_project.create", Value::Object(p))?;
        self.set_rest("plc_config_project", id.cast(), m, &[])?;
        Ok(id)
    }

    /// `bus.logical_net.create` of `net`'s bus, name and rate (WP-S1.21 D11); its master names a function, so the
    /// caller sets it with [`Builder::logical_net_master`] once the functions are built.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn logical_net(&mut self, net: &LogicalNet) -> Result<Id<LogicalNet>> {
        let mut m = members(Record::LogicalNet(net.clone()));
        let mut p = Map::new();
        for k in ["bus", "name", "rate"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        take(&mut m, "master");
        let (id, _) = self.primary::<LogicalNet>("bus.logical_net.create", Value::Object(p))?;
        self.set_rest("logical_net", id.cast(), m, &[])?;
        Ok(id)
    }

    /// `fields.set` of a logical net's `master` (an endpoint of the built functions).
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn logical_net_master(&mut self, id: Id<LogicalNet>, master: &Endpoint) -> Result<()> {
        let value = serde_json::to_value(master).map_err(|e| BuildError {
            command: "fields.set".into(),
            params: Value::Null,
            message: format!("an endpoint does not serialise: {e}"),
        })?;
        self.set("logical_net", &[id.cast()], "master", value)
    }

    /// `bus.physical_net.create` of `net`'s name and medium (WP-S1.21 D11); its `order_hint` names functions, so the
    /// caller sets it with `fields.set` once they are built.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn physical_net(&mut self, net: &PhysicalNet) -> Result<Id<PhysicalNet>> {
        let mut m = members(Record::PhysicalNet(net.clone()));
        let mut p = Map::new();
        for k in ["name", "medium"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        take(&mut m, "order_hint");
        let (id, _) = self.primary::<PhysicalNet>("bus.physical_net.create", Value::Object(p))?;
        self.set_rest("physical_net", id.cast(), m, &[])?;
        Ok(id)
    }

    /// `plc.bind_channel` of `channel` to `endpoint` (a CP).
    ///
    /// # Errors
    ///
    /// The command's error, or an endpoint that is not a CP.
    pub fn bind_channel(&mut self, channel: Id<Channel>, endpoint: &Endpoint) -> Result<()> {
        let Endpoint::Cp(f, idx) = endpoint else {
            return Err(BuildError {
                command: "plc.bind_channel".into(),
                params: Value::Null,
                message: "a channel binding is a CP".into(),
            });
        };
        self.run(
            "plc.bind_channel",
            json!({"channel": channel, "endpoint": End::Cp(*f, *idx).param()}),
        )
        .map(|_| ())
    }

    /// `device.link`.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn device_link(&mut self, link: &DeviceLink) -> Result<Id<DeviceLink>> {
        let mut m = members(Record::DeviceLink(link.clone()));
        let mut p = Map::new();
        for k in ["from", "to", "kind"] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        let (id, _) = self.primary::<DeviceLink>("device.link", Value::Object(p))?;
        self.set_rest("device_link", id.cast(), m, &[])?;
        Ok(id)
    }

    /// `revision.create` (FT-165): the open revision, or a historic released one (no release record, no
    /// archive). A withdrawal record, or a state of a newer format, is not written (`revision.withdraw` does
    /// not exist yet): `None`, counted under `revision (withdrawn or newer state)`.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn revision(&mut self, rev: &Revision) -> Result<Option<Id<Revision>>> {
        if !matches!(rev.state, RevisionState::Open | RevisionState::Released) {
            self.not_written("revision (withdrawn or newer state)", 1);
            return Ok(None);
        }
        let mut m = members(Record::Revision(rev.clone()));
        let mut p = Map::new();
        for k in [
            "label",
            "state",
            "date",
            "author",
            "checker",
            "description",
            "page_notes",
        ] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        let (id, _) = self.primary::<Revision>("revision.create", Value::Object(p))?;
        self.set_rest("revision", id.cast(), m, &[])?;
        Ok(Some(id))
    }

    /// `project.supply.set` of one supply member.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn supply(&mut self, path: &str, value: Value) -> Result<()> {
        self.run("project.supply.set", json!({"path": path, "value": value}))
            .map(|_| ())
    }

    /// `project.company.set`: the panel builder's name, the title-block notice and the personal-data flag; an
    /// absent (`None`) member is left as it is.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn company(
        &mut self,
        name: Option<&str>,
        notice: Option<&str>,
        personal_data: Option<bool>,
    ) -> Result<()> {
        let mut p = Map::new();
        if let Some(name) = name {
            p.insert("name".into(), json!(name));
        }
        if let Some(notice) = notice {
            p.insert("notice".into(), json!(notice));
        }
        if let Some(flag) = personal_data {
            p.insert("personal_data".into(), json!(flag));
        }
        self.run("project.company.set", Value::Object(p)).map(|_| ())
    }

    /// `project.company.set` with `logo`: the company logo from the bytes of an image file (SVG, PNG or JPEG),
    /// sent in base64 (FT-337); empty bytes remove the logo.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn company_logo(&mut self, bytes: &[u8]) -> Result<()> {
        self.run(
            "project.company.set",
            json!({"logo": ew_schema::base64_encode(bytes)}),
        )
        .map(|_| ())
    }

    /// `report_block.create` of a block whose form comes from the library: `copies` are the library documents of
    /// the form and its closure (`lib/` JSON form), embedded with the block where the project does not hold them
    /// (FT-341).
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn report_block_from_library(
        &mut self,
        block: &ReportBlock,
        copies: &[Value],
    ) -> Result<Id<ReportBlock>> {
        let mut m = members(Record::ReportBlock(block.clone()));
        let mut p = Map::new();
        for k in [
            "source",
            "form",
            "section",
            "first_folio",
            "title",
            "manual",
            "sort",
            "group",
            "continuation",
            "empty",
            "embedded",
        ] {
            if let Some(v) = take(&mut m, k) {
                p.insert(k.into(), v);
            }
        }
        p.insert("form_copies".into(), Value::Array(copies.to_vec()));
        let (id, _) = self.primary::<ReportBlock>("report_block.create", Value::Object(p))?;
        self.set_rest("report_block", id.cast(), m, &[])?;
        Ok(id)
    }

    /// `project.frames.set`: the whole frame defaults `{default?, per_page_type}` (doc 03 §5.1).
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn frame_defaults(&mut self, frames: Value) -> Result<()> {
        self.run("project.frames.set", frames).map(|_| ())
    }

    /// `project.languages.set`: the project's languages and printed formats as one value `{source?,
    /// translations?, date?, decimal_separator?}`; an absent member is removed.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn languages(&mut self, languages: Value) -> Result<()> {
        self.run("project.languages.set", languages).map(|_| ())
    }

    /// `project.checks.set`: the check schemes `{active_scheme?, release_scheme?}`. The command is `HumanOnly`
    /// (ADR-0008 amendment 3): a sink that submits as a command log or a script is refused, so a builder script
    /// that uses this verb does not replay with `ew apply`.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn check_schemes(&mut self, schemes: Value) -> Result<()> {
        self.run("project.checks.set", schemes).map(|_| ())
    }

    /// `device.place` (or `box.black` with `black_box`) with a `from_part` target (FR-UI-017; WP-S2.04 D9): a new
    /// device of the part copy `part` with the letter `letter` and one function per entry of its function template;
    /// the representation draws entry `template`, the others stay unplaced. Returns the representation and the
    /// functions the command created, in template order.
    ///
    /// # Errors
    ///
    /// The command's error.
    #[allow(clippy::too_many_arguments)]
    pub fn place_from_part(
        &mut self,
        page: Id<Page>,
        symbol: Id<SymbolDef>,
        pos: Pt2,
        part: Id<Part>,
        template: u16,
        letter: &str,
        black_box: bool,
    ) -> Result<(Id<Representation>, Vec<Id<Function>>)> {
        let target = json!({"from_part": {"part": part, "template": template, "letter": letter}});
        let name = if black_box { "box.black" } else { "device.place" };
        let (rep, done) = self.primary::<Representation>(
            name,
            json!({"page": page, "symbol": symbol, "pos": pos, "target": target}),
        )?;
        Ok((rep, Self::created::<Function>(&done, Kind::Function)))
    }

    /// `device.place` with a `new_terminal` target (AC-28; WP-S2.04 D9): one new terminal of the strip `strip`
    /// with the designation `designation` (the sequence number `seq`, or one more than the highest when absent),
    /// created as `terminal.add` creates it, with the symbol's function definition, and placed. Returns the
    /// representation and the terminal device.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn place_new_terminal(
        &mut self,
        page: Id<Page>,
        symbol: Id<SymbolDef>,
        pos: Pt2,
        strip: Id<Device>,
        designation: &str,
        seq: Option<u32>,
    ) -> Result<(Id<Representation>, Option<Id<Device>>)> {
        let mut terminal = json!({"strip": strip, "designation": designation});
        if let Some(seq) = seq {
            terminal["seq"] = json!(seq);
        }
        let (rep, done) = self.primary::<Representation>(
            "device.place",
            json!({"page": page, "symbol": symbol, "pos": pos, "target": {"new_terminal": terminal}}),
        )?;
        Ok((rep, Self::created::<Device>(&done, Kind::Device).first().copied()))
    }

    /// `rep.move` by grid steps `[kx, ky]` on the page's connection grid (WP-S2.04 D13): each representation lands on
    /// the grid point at its grid index plus the steps, and the routes follow.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn move_steps(&mut self, reps: &[Id<Representation>], steps: [i64; 2]) -> Result<()> {
        self.run("rep.move", json!({"reps": reps, "steps": steps}))
            .map(|_| ())
    }

    /// `rep.exchange_symbol` (FR-SYM-022): the representation `rep` is drawn by the symbol copy `symbol` from now
    /// on. `pin_map` lists `(symbol CP, function CP)` pairs; `None` derives the map (by index for the same
    /// function definition, else by designation).
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn exchange_symbol(
        &mut self,
        rep: Id<Representation>,
        symbol: Id<SymbolDef>,
        pin_map: Option<&[(u16, u16)]>,
    ) -> Result<()> {
        let mut p = json!({"rep": rep, "symbol": symbol});
        if let Some(map) = pin_map {
            p["pin_map"] = Value::Array(
                map.iter()
                    .map(|(symbol_cp, function_cp)| json!({"symbol_cp": symbol_cp, "function_cp": function_cp}))
                    .collect(),
            );
        }
        self.run("rep.exchange_symbol", p).map(|_| ())
    }

    /// `project.drawing.set` (FR-CON-031): how crossing wires are drawn. `crossings`: `None` leaves the setting as
    /// it is, `Some(Some("plain" | "hops"))` sets it, `Some(None)` removes it (plain).
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn drawing(&mut self, crossings: Option<Option<&str>>) -> Result<()> {
        let mut p = Map::new();
        if let Some(value) = crossings {
            p.insert("crossings".into(), value.map_or(Value::Null, |c| json!(c)));
        }
        self.run("project.drawing.set", Value::Object(p)).map(|_| ())
    }

    // WP-S2.04 part 3
    /// `connection.draw_to_ip` (doc 06 §4.4 "P"): a wire from `a` that ends in a new interruption point, in one
    /// transaction. `route` (empty for none) must end at the IP's one connection point; `kind` `None` draws a wire.
    /// `ip.spec.external` is not part of the command and is ignored. Returns the new connection and the new IP
    /// function.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn draw_to_ip(
        &mut self,
        a: &End,
        page: Id<Page>,
        route: &[Pt2],
        kind: Option<&ConnectionKind>,
        ip: &IpEnd,
    ) -> Result<(Id<Connection>, Id<Function>)> {
        let mut at_end = Map::new();
        at_end.insert("symbol".into(), json!(ip.symbol));
        if let Some(def) = ip.def {
            at_end.insert("fndef".into(), json!(def));
        }
        at_end.insert("pos".into(), json!(ip.pos));
        if let Ok(Value::Object(s)) = serde_json::to_value(&ip.spec) {
            for (k, v) in s {
                if k != "external" {
                    at_end.insert(k, v);
                }
            }
        }
        let mut p = json!({"a": a.param(), "page": page, "ip": Value::Object(at_end)});
        if !route.is_empty() {
            p["route"] = json!(route);
        }
        if let Some(kind) = kind {
            p["kind"] = json!(kind);
        }
        let (connection, done) = self.primary::<Connection>("connection.draw_to_ip", p.clone())?;
        let function = Self::created::<Function>(&done, Kind::Function)
            .first()
            .copied()
            .ok_or_else(|| BuildError {
                command: "connection.draw_to_ip".into(),
                params: p,
                message: "no function created".into(),
            })?;
        Ok((connection, function))
    }

    /// `ip.arrange` (FR-XRF-013): the interruption points `ips` of one page exchanged into the order given;
    /// `set_sort` also numbers their chain order from 1.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn arrange_ips(&mut self, ips: &[Id<Function>], set_sort: bool) -> Result<()> {
        let mut p = json!({"ips": ips});
        if set_sort {
            p["set_sort"] = json!(true);
        }
        self.run("ip.arrange", p).map(|_| ())
    }

    /// `box.reshape` (FR-DEV-039): the black box `rep` gets `outline` (relative to its position); `cps` `None`
    /// re-projects the box's connection points onto the new outline, `Some` lists the new ones as `(function CP,
    /// position relative to the representation, direction)`, the direction as the command writes it (`"up"`,
    /// `"right"`, `"down"`, `"left"`).
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn reshape_box(
        &mut self,
        rep: Id<Representation>,
        outline: &[Pt2],
        cps: Option<&[(u16, Pt2, &str)]>,
    ) -> Result<()> {
        let mut p = json!({"rep": rep, "outline": outline});
        if let Some(cps) = cps {
            p["cps"] = Value::Array(
                cps.iter()
                    .map(|(idx, pos, dir)| json!({"idx": idx, "pos": pos, "dir": dir}))
                    .collect(),
            );
        }
        self.run("box.reshape", p).map(|_| ())
    }

    // WP-S2.04 part 4a ---------------------------------------------------------------------------------------

    /// `project.drawing.set` with `marker_mode` (FR-CON-024): `Some("all" | "deviations" | "explicit" | "none")`
    /// sets which wires carry a marker, `None` removes the member (the as-built `show_marker` rule).
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn marker_mode(&mut self, mode: Option<&str>) -> Result<()> {
        self.run(
            "project.drawing.set",
            json!({"marker_mode": mode.map_or(Value::Null, |m| json!(m))}),
        )
        .map(|_| ())
    }

    /// `item.place` (FR-SYM-023, FR-SYM-024): a page item on `page`; `kind` is the command's `kind` member,
    /// `{"graphic": {"shape": …}}` or `{"free_text": {"text": …, "at": …}}`. Returns the item.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn place_item(&mut self, page: Id<Page>, kind: Value) -> Result<Id<PageItem>> {
        let (id, _) = self.primary::<PageItem>("item.place", json!({"page": page, "kind": kind}))?;
        Ok(id)
    }

    /// `marker.place` (FR-CON-023, FR-CON-039): the group wire marker of `connections` (printed order) on `page`
    /// at `at`, turned 90° with `vertical`; `csa` and `colours` (empty, or one per connection) become the
    /// connections' level-7 overrides. Returns the marker.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn place_marker(
        &mut self,
        page: Id<Page>,
        connections: &[Id<Connection>],
        at: Pt2,
        vertical: bool,
        csa: Option<&str>,
        colours: &[Value],
    ) -> Result<Id<PageItem>> {
        let mut p = json!({"page": page, "connections": connections, "at": at});
        if vertical {
            p["angle"] = json!(90_000);
        }
        if let Some(csa) = csa {
            p["csa"] = json!(csa);
        }
        if !colours.is_empty() {
            p["colours"] = Value::Array(colours.to_vec());
        }
        let (id, _) = self.primary::<PageItem>("marker.place", p)?;
        Ok(id)
    }

    /// `box.structure` (FR-PRJ-020): a structure box on `page` with its absolute `outline`, `style` (`dotted` or
    /// `dash_dot`), `caption` (empty: none), the `assigns` member (`{"location": node}` …) and the external point
    /// it stands for. Returns the box.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn structure_box(
        &mut self,
        page: Id<Page>,
        outline: &[Pt2],
        style: &str,
        caption: &str,
        assigns: Value,
        external: Option<Id<ExternalRef>>,
    ) -> Result<Id<StructureBox>> {
        let mut p = json!({"page": page, "outline": outline, "style": style, "assigns": assigns});
        if !caption.is_empty() {
            p["caption"] = json!(caption);
        }
        if let Some(e) = external {
            p["external"] = json!(e);
        }
        let (id, _) = self.primary::<StructureBox>("box.structure", p)?;
        Ok(id)
    }

    /// `rep.move` with `items` (WP-S2.04 part 4a): the representations, page items and structure boxes named move
    /// by `delta` together.
    ///
    /// # Errors
    ///
    /// The command's error.
    pub fn move_with_items(
        &mut self,
        reps: &[Id<Representation>],
        items: &[AnyId],
        delta: Pt2,
    ) -> Result<()> {
        self.run("rep.move", json!({"reps": reps, "items": items, "delta": delta}))
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // WP-S2.05 part 2
    /// `set_many` issues one `fields.set_many` with the entries in order.
    #[test]
    fn set_many_issues_one_command_with_the_entries_in_order() {
        let mut seen: Vec<(String, Value)> = Vec::new();
        let mut sink = |name: &str, params: Value| {
            seen.push((name.to_owned(), params));
            Ok(Done::default())
        };
        let a = AnyId::from_uuid(ew_ids::Uuid::from_u128(1));
        let b = AnyId::from_uuid(ew_ids::Uuid::from_u128(2));
        let mut builder = Builder::new(&mut sink);
        builder
            .set_many(&[
                ("device", &[a], "function_text", json!("Spare")),
                ("page", &[b], "title", json!("Main")),
            ])
            .unwrap();
        assert_eq!(builder.script().len(), 1);
        drop(builder);
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "fields.set_many");
        assert_eq!(
            seen[0].1,
            json!({"edits": [
                {"kind": "device", "targets": [a], "path": "function_text", "value": "Spare"},
                {"kind": "page", "targets": [b], "path": "title", "value": "Main"}]})
        );
    }

    /// `reorder_pages` issues one `page.reorder`, with `before` or `section` and never both.
    #[test]
    fn reorder_pages_issues_one_command_with_before_or_section() {
        let mut seen: Vec<(String, Value)> = Vec::new();
        let mut sink = |name: &str, params: Value| {
            seen.push((name.to_owned(), params));
            Ok(Done::default())
        };
        let a: Id<Page> = Id::from_uuid(ew_ids::Uuid::from_u128(1));
        let b: Id<Page> = Id::from_uuid(ew_ids::Uuid::from_u128(2));
        let s: Id<Section> = Id::from_uuid(ew_ids::Uuid::from_u128(3));
        let mut builder = Builder::new(&mut sink);
        builder.reorder_pages(&[a], Some(b), Some(s)).unwrap();
        builder.reorder_pages(&[a, b], None, Some(s)).unwrap();
        builder.reorder_pages(&[a], None, None).unwrap();
        drop(builder);
        assert!(seen.iter().all(|(name, _)| name == "page.reorder"));
        assert_eq!(seen[0].1, json!({"pages": [a], "before": b}));
        assert_eq!(seen[1].1, json!({"pages": [a, b], "section": s}));
        assert_eq!(seen[2].1, json!({"pages": [a]}));
    }

    // WP-S2.07 part 1
    /// `terminal_insert` and `terminal_move` issue one `terminal.insert` / `terminal.move` each, with `before` only
    /// when given and `parts` only when not empty; `terminal_insert` returns the command's primary record.
    #[test]
    fn terminal_insert_and_move_issue_one_command_with_or_without_before() {
        let strip: Id<Device> = Id::from_uuid(ew_ids::Uuid::from_u128(1));
        let t1: Id<Device> = Id::from_uuid(ew_ids::Uuid::from_u128(2));
        let t2: Id<Device> = Id::from_uuid(ew_ids::Uuid::from_u128(3));
        let fndef: Id<FnDef> = Id::from_uuid(ew_ids::Uuid::from_u128(4));
        let part: Id<Part> = Id::from_uuid(ew_ids::Uuid::from_u128(5));
        let new: Id<Device> = Id::from_uuid(ew_ids::Uuid::from_u128(6));
        let mut seen: Vec<(String, Value)> = Vec::new();
        let mut sink = |name: &str, params: Value| {
            seen.push((name.to_owned(), params));
            Ok(Done {
                primary: (name == "terminal.insert").then_some((Kind::Device, new.cast())),
                created: Vec::new(),
            })
        };
        let parts: Vec<PartUse> =
            serde_json::from_value(json!([{"part": {"copy": part}, "qty": "1"}])).unwrap();
        let mut builder = Builder::new(&mut sink);
        let first = builder
            .terminal_insert(strip, Some(t1), fndef, &[json!({"designation": "21"})], &parts)
            .unwrap();
        assert_eq!(first, new);
        builder
            .terminal_insert(strip, None, fndef, &[json!({"designation": "22"})], &[])
            .unwrap();
        builder.terminal_move(&[t1, t2], Some(strip)).unwrap();
        builder.terminal_move(&[t2], None).unwrap();
        assert_eq!(builder.script().len(), 4);
        drop(builder);
        let names: Vec<&str> = seen.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "terminal.insert",
                "terminal.insert",
                "terminal.move",
                "terminal.move"
            ]
        );
        assert_eq!(
            seen[0].1,
            json!({"strip": strip, "before": t1, "fndef": fndef, "terminals": [{"designation": "21"}],
                "parts": [{"part": {"copy": part}, "qty": "1"}]})
        );
        assert_eq!(
            seen[1].1,
            json!({"strip": strip, "fndef": fndef, "terminals": [{"designation": "22"}]})
        );
        assert_eq!(seen[2].1, json!({"terminals": [t1, t2], "before": strip}));
        assert_eq!(seen[3].1, json!({"terminals": [t2]}));
    }

    // WP-S2.05 part 3
    /// `paste` issues one `edit.paste` with the fragment as given.
    #[test]
    fn paste_issues_one_command_with_the_fragment() {
        let mut seen: Vec<(String, Value)> = Vec::new();
        let mut sink = |name: &str, params: Value| {
            seen.push((name.to_owned(), params));
            Ok(Done::default())
        };
        let page = AnyId::from_uuid(ew_ids::Uuid::from_u128(3));
        let fragment = json!({"format": "0.1", "records": []});
        let mut builder = Builder::new(&mut sink);
        builder.paste(page, [10, 20], fragment.clone()).unwrap();
        drop(builder);
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "edit.paste");
        assert_eq!(
            seen[0].1,
            json!({"page": page, "at": [10, 20], "fragment": fragment})
        );
    }

    // FT-672
    /// `insert_macro` issues one `macro.insert` with the orientation, and leaves `r0` out as an old log has it.
    #[test]
    fn insert_macro_issues_one_command_with_its_orientation() {
        let mut seen: Vec<(String, Value)> = Vec::new();
        let mut sink = |name: &str, params: Value| {
            seen.push((name.to_owned(), params));
            Ok(Done::default())
        };
        let page = AnyId::from_uuid(ew_ids::Uuid::from_u128(3));
        let m = MacroInsertion {
            macro_copy: AnyId::from_uuid(ew_ids::Uuid::from_u128(4)),
            copies: vec![json!({"format": "0.1"})],
            instance: AnyId::from_uuid(ew_ids::Uuid::from_u128(5)),
            value_set: Some("spare".into()),
            values: Map::new(),
        };
        let mut builder = Builder::new(&mut sink);
        builder.insert_macro(page, Some([10, 20]), "r90", &m).unwrap();
        builder.insert_macro(page, None, "r0", &m).unwrap();
        drop(builder);
        assert_eq!(seen.len(), 2);
        assert!(seen.iter().all(|(name, _)| name == "macro.insert"));
        let base = json!({"page": page, "macro": m.macro_copy, "copies": m.copies, "instance": m.instance,
            "value_set": "spare", "values": {}});
        let mut turned = base.clone();
        turned["at"] = json!([10, 20]);
        turned["orient"] = json!("r90");
        assert_eq!(seen[0].1, turned);
        let mut page_macro = base;
        page_macro["at"] = Value::Null;
        assert_eq!(seen[1].1, page_macro);
    }

    // WP-S2.04 part 4a
    /// The page-item verbs issue their commands with the documented parameters.
    #[test]
    fn page_item_verbs_issue_their_commands() {
        let mut seen: Vec<(String, Value)> = Vec::new();
        let mut sink = |name: &str, params: Value| {
            seen.push((name.to_owned(), params));
            Ok(Done::default())
        };
        let page: Id<Page> = Id::from_uuid(ew_ids::Uuid::from_u128(7));
        let c: Id<Connection> = Id::from_uuid(ew_ids::Uuid::from_u128(8));
        let at = Pt2::new(ew_units::Nm(1), ew_units::Nm(2));
        let mut builder = Builder::new(&mut sink);
        builder.marker_mode(Some("explicit")).unwrap();
        // No primary key comes back from this sink: the placing verbs report that as an error, after the run.
        assert!(
            builder
                .place_item(page, json!({"free_text": {"text": "x", "at": [0, 0]}}))
                .is_err()
        );
        assert!(
            builder
                .place_marker(page, &[c], at, true, Some("2.5"), &[json!(["BK"])])
                .is_err()
        );
        assert!(
            builder
                .structure_box(page, &[at, at, at], "dotted", "Box", json!({}), None)
                .is_err()
        );
        builder.move_with_items(&[], &[c.cast()], at).unwrap();
        drop(builder);
        let names: Vec<&str> = seen.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "project.drawing.set",
                "item.place",
                "marker.place",
                "box.structure",
                "rep.move"
            ]
        );
        assert_eq!(seen[0].1, json!({"marker_mode": "explicit"}));
        assert_eq!(
            seen[2].1,
            json!({"page": page, "connections": [c], "at": [1, 2], "angle": 90_000, "csa": "2.5",
                "colours": [["BK"]]})
        );
        assert_eq!(seen[3].1["caption"], json!("Box"));
        assert_eq!(seen[4].1, json!({"reps": [], "items": [c], "delta": [1, 2]}));
    }
}
