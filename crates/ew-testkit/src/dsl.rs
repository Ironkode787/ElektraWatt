// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The builder of the tests: typed methods that issue one command each through a [`CommandSink`] and return the
//! IDs the commands minted. A method that creates a record issues the creating command with the record's members as
//! parameters, then `fields.set` for each further member a command writes.
//!
//! ```text
//! let mut b = Builder::new(&mut sink);
//! let (dev, main) = b.device(&device, Some(&main_fn))?;     // device.create (+ fields.set …)
//! b.assign_part(&[f1], copy, &[], "append", None, None)?;   // part.assign
//! ```

use ew_ids::Id;
use ew_model::AnyKey;
use ew_schema::{
    AnyId, Connection, ConnectionKind, Device, FnDef, Function, IpSpec, Junction, Kind, LibDocument, Page,
    Part, PartUse, Record, Representation, SymbolDef,
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

/// The start of a wire [`Builder::draw_to_ip`] draws: a CP, a junction, or a point on a connection (which splits
/// it and puts a junction there).
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

/// The builder: runs commands through a [`CommandSink`] and records them.
pub struct Builder<'s, S: CommandSink + ?Sized> {
    sink: &'s mut S,
    script: Vec<(String, Value)>,
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

    /// The commands that committed so far, in order, as `(name, params)` (a refused command is not listed).
    pub fn script(&self) -> &[(String, Value)] {
        &self.script
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

    /// `fields.set` of each remaining member of `rest` that is in `settable`; the others are not written.
    fn set_rest(&mut self, kind: &str, id: AnyId, rest: Map<String, Value>, settable: &[&str]) -> Result<()> {
        for (member, value) in rest {
            if settable.contains(&member.as_str()) {
                self.set(kind, &[id], &member, value)?;
            }
        }
        Ok(())
    }

    /// `device.create` (with the main function `main`, a normal function, when given), then `fields.set`
    /// of `tag.frozen`, `process_tag`, `tech_data` and `state`.
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
    /// by `fields.set`; any other member is not written.
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
            }
        }
        Ok(())
    }

    fn function_rest(&mut self, id: Id<Function>, f: &Function) -> Result<()> {
        let mut m = members(Record::Function(f.clone()));
        // `template_entry`: written by the command that made the function from its part's template (N-FL-1)
        for k in ["device", "def", "role", "kind", "template_entry"] {
            m.remove(k);
        }
        self.set_rest("function", id.cast(), m, &["cp_overrides", "bus_port"])
    }

    /// `terminal.insert`: the `terminals` (`TerminalSpec` JSON objects without `seq`) of the strip
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

    /// `terminal.move`: the terminals `terminals` of one strip put before the terminal `before`, or
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

    /// `part.assign`: the part copy `part` assigned to `devices`, embedding the library documents
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

    /// `library.update`: the held copies replaced by the documents `copies` (same copy IDs), with the
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

    /// `part.edit_copy`: the part copy `copy` edited to the part item `part` (its members, without the
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

    /// `device.place` (or `box.black` with `black_box`) with a `from_part` target (FR-UI-017): a new
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

    /// `device.place` with a `new_terminal` target: one new terminal of the strip `strip`
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

    /// `rep.move` by grid steps `[kx, ky]` on the page's connection grid: each representation lands on
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

    /// The drawing setting `crossings` of `project.settings.set` (FR-CON-031): how crossing wires are drawn. `crossings`: `None` leaves the setting as
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
        self.run(
            "project.settings.set",
            json!({ "member": "drawing", "value": Value::Object(p) }),
        )
        .map(|_| ())
    }

    /// `connection.draw_to_ip` (doc 06 §4.4 "P"): a wire from `a` that ends in a new interruption point, in one
    /// transaction. `route` (empty for none) must end at the IP's one connection point; `kind` `None` draws a wire.
    /// `ip.spec.external`, when set, is the point's external target. Returns the new connection and the new IP
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
            at_end.extend(s);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The commands `f` issues through a sink that answers `done(name)` to each, with their parameters, in order.
    fn issued(
        done: impl Fn(&str) -> Done,
        f: impl FnOnce(&mut Builder<'_, dyn CommandSink + '_>),
    ) -> Vec<(String, Value)> {
        let mut seen = Vec::new();
        let mut sink = |name: &str, params: Value| {
            seen.push((name.to_owned(), params));
            Ok(done(name))
        };
        f(&mut Builder::new(&mut sink as &mut dyn CommandSink));
        seen
    }

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
        let parts: Vec<PartUse> =
            serde_json::from_value(json!([{"part": {"copy": part}, "qty": "1"}])).unwrap();
        let seen = issued(
            |name| Done {
                primary: (name == "terminal.insert").then_some((Kind::Device, new.cast())),
                created: Vec::new(),
            },
            |builder| {
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
            },
        );
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
}
