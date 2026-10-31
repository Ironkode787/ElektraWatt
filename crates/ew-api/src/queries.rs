// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The public queries of `ew query` (doc 17 §3.3 and §3.4, FR-EXT-005; WP-S1.22 part 4, D11 and D12).
//!
//! [`REGISTRY`] is the closed MVP set of twelve named queries. Each entry has a name, the grammar of its key, an
//! output type (`serde` and `schemars`, as D5 rules for the CLI's results) with `output_version` 1, and a golden
//! example key on Golden A. A key is a reference of the `.ewcmd` grammar (`tag:`, `id:`, `folio:`, a bare tag),
//! resolved by [`Session::resolve_reference`], or `-` for a query without a key. Every adapter checks the kind of
//! the record its key resolves to and maps the user's key onto the memo family's key (project owner (delegated),
//! cycle 41): `cell_of`, `xref_text_of` and `plc_address_text` go through the device's main representation or main
//! function, `potential_name` and `wire_props_of` through a wire's potential, `wire_numbers` filters the project's
//! numbers by the folio.
//!
//! **`--explain`** ([`Session::run_query`] with `explain`) adds the policy line, the memo's dependencies in words and,
//! for a query that backs a property-grid row (`device_tag`, `cell_of`), that row's provenance. The memo state is read
//! through `ew_memo::Store::explain` (read-only) twice: on an O(1) fork of the memo store taken before the key is
//! resolved, for whether the read was a hit (a fresh open prints a miss, every run alike), and after the value is
//! computed, for the revisions and the trail. A composed adapter explains its outermost memoised family; a family with
//! the policy `Cold` has no memo and prints its policy line only. No wall-clock time is printed: one fresh open prints
//! one text.

use std::collections::BTreeMap;
use std::fmt;

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{
    BomMode, BomRows, ChannelAddr, ChannelAddresses, DepKey, DeviceTag, DisplayedTag, IxKey, MainFunction,
    MainRep, MemoInput, PageOrder, PageRef, PlcAddressText, Policy, PotentialId, PotentialNameOf,
    PotentialPartition, Q, Query, RepAnchorCell, TbValue, TitleBlockValues, WireNumbers, WirePropsOf,
    XrefTextOf,
};
use ew_model::EndpointKey;
use ew_schema::{AnyId, Connection, Device, Function, Junction, Kind, Page, Representation};
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Session;
use crate::cli_results::{QueryEntry, QueryExplain};
use crate::output::Provenance;

/// The version of every output type of the MVP set.
pub const OUTPUT_VERSION: u32 = 1;

/// The most memo dependencies `--explain` lists (`ew_memo::Store::explain` keeps as many, then an ellipsis).
const EXPLAIN_LINES: usize = 64;

/// One public query of `ew query`.
#[derive(Clone, Copy)]
pub struct QueryDef {
    /// The name (`device_tag`).
    pub name: &'static str,
    /// The grammar of its key.
    pub key: &'static str,
    /// What it answers.
    pub description: &'static str,
    /// Its golden example: a key on Golden A (`golden/J-E-2416.ew`).
    pub example: &'static str,
    /// The version of its output type.
    pub output_version: u32,
    /// What its key names, in words, for the refusal of a key of another kind.
    takes: &'static str,
    output: fn() -> Value,
    run: fn(&Cx<'_>, &str) -> Result<Answer, String>,
}

impl fmt::Debug for QueryDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueryDef")
            .field("name", &self.name)
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl QueryDef {
    /// The JSON Schema (draft 2020-12) of its output object.
    pub fn output_schema(&self) -> Value {
        (self.output)()
    }

    /// Its entry of `ew queries` and of the snapshot `crates/ew-api/api/queries.json`.
    pub fn entry(&self) -> QueryEntry {
        QueryEntry {
            name: self.name.to_owned(),
            key: self.key.to_owned(),
            description: self.description.to_owned(),
            example: self.example.to_owned(),
            output_version: self.output_version,
            output: self.output_schema(),
        }
    }
}

fn schema_of<T: JsonSchema>() -> Value {
    SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<T>()
        .to_value()
}

const DEVICE: &str = "a device (`tag:-101F1` or `id:<uuid>`)";

/// The closed MVP set of D11, in name order.
pub const REGISTRY: &[QueryDef] = &[
    QueryDef {
        name: "bom_rows",
        key: "-",
        description: "The bill of materials in the parts mode that folio 901 prints: position, quantity, part, \
                      manufacturer, description, tags and folios of each row.",
        example: "-",
        output_version: OUTPUT_VERSION,
        takes: "no key (`-`)",
        output: schema_of::<BomRowsOutput>,
        run: bom_rows,
    },
    QueryDef {
        name: "cell_of",
        key: "tag:<tag> | id:<uuid> (a device)",
        description: "The folio and frame cell of a device's main representation.",
        example: "tag:-101F1",
        output_version: OUTPUT_VERSION,
        takes: DEVICE,
        output: schema_of::<CellOfOutput>,
        run: cell_of,
    },
    QueryDef {
        name: "channel_addresses",
        key: "tag:<tag> | id:<uuid> (a PLC module)",
        description: "The address of each channel of a PLC module, in channel-number order.",
        example: "tag:-201U5",
        output_version: OUTPUT_VERSION,
        takes: "a PLC module (`tag:-201U5`)",
        output: schema_of::<ChannelAddressesOutput>,
        run: channel_addresses,
    },
    QueryDef {
        name: "device_tag",
        key: "tag:<tag> | id:<uuid> (a device)",
        description: "A device's full designation and its tag as displayed on its main page.",
        example: "tag:-101F1",
        output_version: OUTPUT_VERSION,
        takes: DEVICE,
        output: schema_of::<DeviceTagOutput>,
        run: device_tag,
    },
    QueryDef {
        name: "displayed_tag",
        key: "<device>,folio:<label> (a device and a page)",
        description: "A device's tag as a page prints it: the structure the page shares is left out.",
        example: "tag:-101F1,folio:101",
        output_version: OUTPUT_VERSION,
        takes: "a device and a folio (`tag:-101F1,folio:101`)",
        output: schema_of::<DisplayedTagOutput>,
        run: displayed_tag,
    },
    QueryDef {
        name: "page_order",
        key: "-",
        description: "The folio labels of every page in page order: drawn pages, section dividers and generated \
                      report pages.",
        example: "-",
        output_version: OUTPUT_VERSION,
        takes: "no key (`-`)",
        output: schema_of::<PageOrderOutput>,
        run: page_order,
    },
    QueryDef {
        name: "plc_address_text",
        key: "id:<uuid> (a function) | tag:<tag> (a device: its main function)",
        description: "The PLC addresses a function's representation prints beside it.",
        example: "tag:-201U3-Q1",
        output_version: OUTPUT_VERSION,
        takes: "a function (`id:<uuid>`) or a device (`tag:-201U3-Q1`, its main function)",
        output: schema_of::<PlcAddressTextOutput>,
        run: plc_address_text,
    },
    QueryDef {
        name: "potential_name",
        key: "id:<uuid> (a wire or a junction) | tag:<tag> (a terminal)",
        description: "The name of the potential a wire, a junction or a terminal belongs to, with its aliases.",
        example: "id:5945bf6c-b3b1-5964-8651-59a4b48396d3",
        output_version: OUTPUT_VERSION,
        takes: "a wire or a junction (`id:<uuid>`) or a terminal (`tag:-X1031:1`)",
        output: schema_of::<PotentialNameOutput>,
        run: potential_name,
    },
    QueryDef {
        name: "title_block",
        key: "folio:<label> (a drawn page)",
        description: "The values a drawn page's title block prints, by binding path.",
        example: "folio:101",
        output_version: OUTPUT_VERSION,
        takes: "a drawn page (`folio:101`)",
        output: schema_of::<TitleBlockOutput>,
        run: title_block,
    },
    QueryDef {
        name: "wire_numbers",
        key: "folio:<label> (a drawn page)",
        description: "The numbers of the wires numbered on a drawn page (their From end lies on it), in numbering \
                      order.",
        example: "folio:102",
        output_version: OUTPUT_VERSION,
        takes: "a drawn page (`folio:102`)",
        output: schema_of::<WireNumbersOutput>,
        run: wire_numbers,
    },
    QueryDef {
        name: "wire_props_of",
        key: "id:<uuid> (a wire)",
        description: "The cross-section and colour of each physical wire of a connection, each with where it comes \
                      from.",
        example: "id:13c75a2c-b8f2-5b51-8e22-4cb6522139b0",
        output_version: OUTPUT_VERSION,
        takes: "a wire (`id:<uuid>`)",
        output: schema_of::<WirePropsOutput>,
        run: wire_props_of,
    },
    QueryDef {
        name: "xref_text_of",
        key: "tag:<tag> | id:<uuid> (a device)",
        description: "The cross-reference text printed at a device's main representation.",
        example: "tag:-201U1",
        output_version: OUTPUT_VERSION,
        takes: DEVICE,
        output: schema_of::<XrefTextOutput>,
        run: xref_text_of,
    },
];

/// The registry entry named `name`.
pub fn query(name: &str) -> Option<&'static QueryDef> {
    REGISTRY.iter().find(|q| q.name == name)
}

// ---------------------------------------------------------------------------------------------------------
// The output types

/// `device_tag`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeviceTagOutput {
    /// The full designation (`=P01+J-E-2416-101F1`).
    pub full: String,
    /// The tag as displayed on the device's main page (`-101F1`), else its short tag.
    pub displayed: String,
}

/// `displayed_tag`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DisplayedTagOutput {
    /// The tag as the page prints it.
    pub displayed: String,
}

/// `page_order`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PageOrderOutput {
    /// The folio labels in page order.
    pub folios: Vec<String>,
}

/// `cell_of`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CellOfOutput {
    /// The folio of the main representation's page; `null` for a device that is not placed.
    pub folio: Option<String>,
    /// The frame cell of its anchor, row then column (`B8`); `null` outside the frame grid or unplaced.
    pub cell: Option<String>,
}

/// `potential_name`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PotentialNameOutput {
    /// The name (`?` for a potential without one).
    pub name: String,
    /// The other names its definitions give it.
    pub aliases: Vec<String>,
}

/// One effective property of a wire with where it comes from.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WireProp {
    /// The value as text (`1.5`, `BU`).
    pub value: String,
    /// Where it comes from, as the property grid's chip names it.
    pub via: String,
}

/// One physical wire of [`WirePropsOutput`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WireOut {
    /// The printed wire number (`?` when unnumbered).
    pub number: String,
    /// The cross-section; `null` when no level gives one.
    pub cross_section: Option<WireProp>,
    /// The colour; `null` when no level gives one.
    pub colour: Option<WireProp>,
}

/// `wire_props_of`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WirePropsOutput {
    /// The potential's name.
    pub potential: String,
    /// The physical wires the connection belongs to, in the order of their numbers.
    pub wires: Vec<WireOut>,
}

/// `xref_text_of`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XrefTextOutput {
    /// The printed cross-reference; empty when the representation has none.
    pub text: String,
}

/// One channel of [`ChannelAddressesOutput`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChannelAddress {
    /// The channel number.
    pub number: u16,
    /// The address as drawings print it (`Q0.1`), `TBD`, `excluded` or `unresolved`.
    pub address: String,
}

/// `channel_addresses`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChannelAddressesOutput {
    /// The channels in number order.
    pub channels: Vec<ChannelAddress>,
}

/// `plc_address_text`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlcAddressTextOutput {
    /// The text a `PlcAddress` slot prints (`Q0.1`, `I0.0 / I0.1`); `null` when no address resolves.
    pub text: Option<String>,
}

/// `wire_numbers`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WireNumbersOutput {
    /// The printed numbers in numbering order.
    pub numbers: Vec<String>,
}

/// `title_block`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TitleBlockOutput {
    /// Binding path (`page.title`) → printed text; a path without a value is absent, an image prints `(image)`.
    pub values: BTreeMap<String, String>,
}

/// One row of [`BomRowsOutput`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BomRowOut {
    /// The position (`1`; `A1` for a configured product).
    pub position: String,
    /// The quantity.
    pub quantity: String,
    /// The part number.
    pub part_number: String,
    /// The manufacturer's name.
    pub manufacturer: String,
    /// The description.
    pub description: String,
    /// The devices' tags.
    pub tags: Vec<String>,
    /// The folios.
    pub folios: Vec<String>,
}

/// `bom_rows`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BomRowsOutput {
    /// The rows in order.
    pub rows: Vec<BomRowOut>,
}

// ---------------------------------------------------------------------------------------------------------
// Running a query

/// The answer of one query.
#[derive(Clone, PartialEq, Debug)]
pub struct QueryAnswer {
    /// The query's name.
    pub query: &'static str,
    /// The key as given.
    pub key: String,
    /// The session's revision the answer was computed at.
    pub rev: u64,
    /// The output object (the query's output type).
    pub output: Value,
    /// The output as text lines (`label  value`), as `ew query` prints it.
    pub lines: Vec<String>,
    /// The explanation, when asked for.
    pub explain: Option<QueryExplain>,
}

impl QueryAnswer {
    /// The text `ew query` prints: with an explanation the header line, the value, `policy`, `depends on` and
    /// `provenance`; without one the value lines.
    pub fn text(&self) -> String {
        let mut out = String::new();
        let Some(e) = &self.explain else {
            for l in &self.lines {
                out.push_str(l);
                out.push('\n');
            }
            return out;
        };
        out.push_str(&format!("{} {} @ rev {}\n", self.query, self.key, self.rev));
        for l in &self.lines {
            out.push_str("  ");
            out.push_str(l);
            out.push('\n');
        }
        out.push_str(&format!("  {:<11}{}\n", "policy", e.policy));
        if !e.depends_on.is_empty() {
            out.push_str("  depends on\n");
            for d in &e.depends_on {
                out.push_str("    ");
                out.push_str(d);
                out.push('\n');
            }
        }
        if let Some(p) = &e.provenance {
            out.push_str(&format!("  {:<11}{p}\n", "provenance"));
        }
        out
    }
}

/// Why a query was not answered (exit 2 in `ew query`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum QueryError {
    /// No query of the registry has the name.
    Unknown(String),
    /// The key is malformed, names no record, or a record of another kind.
    Key(String),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryError::Unknown(name) => write!(f, "unknown query `{name}`; `ew queries` lists the queries"),
            QueryError::Key(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for QueryError {}

/// What an adapter computed: its output, its text lines and its trail (when asked).
struct Answer {
    output: Value,
    lines: Vec<String>,
    explain: Option<QueryExplain>,
}

/// The context of one query: the session, a memoising `Q`, the memo store's fork from before the key was resolved
/// (with `--explain`), and the query's entry.
struct Cx<'s> {
    session: &'s Session,
    q: Q,
    before: Option<crate::MemoStore>,
    explain: bool,
    def: &'static QueryDef,
}

impl Session {
    /// Answers the public query `name` for `key` (doc 17 §3.4, WP-S1.22 D11), with the explanation of D12 when
    /// `explain` is set. Reads only: no command runs and no memo is written by the explanation (the value itself is
    /// computed, and memoised, as any read of the session's queries is).
    ///
    /// # Errors
    ///
    /// [`QueryError::Unknown`] for a name the registry does not hold; [`QueryError::Key`] for a key that is
    /// malformed, that names no record (the message of a command-log replay that cannot resolve it) or a record of
    /// another kind than the query takes.
    pub fn run_query(&self, name: &str, key: &str, explain: bool) -> Result<QueryAnswer, QueryError> {
        let def = query(name).ok_or_else(|| QueryError::Unknown(name.to_owned()))?;
        // The fork is taken before the key is resolved: resolving a tag computes every device's tag.
        let before = if explain {
            self.memo().map(crate::MemoStore::fork)
        } else {
            None
        };
        let cx = Cx {
            session: self,
            q: self.query(),
            before,
            explain,
            def,
        };
        let answer = (def.run)(&cx, key).map_err(QueryError::Key)?;
        Ok(QueryAnswer {
            query: def.name,
            key: key.to_owned(),
            rev: self.world().rev.0,
            output: answer.output,
            lines: answer.lines,
            explain: answer.explain,
        })
    }
}

/// `value` as the output object.
fn to_value<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// A text line `label  value` with the label padded to 11 columns, and at least two spaces after a longer one.
fn line(label: &str, value: &str) -> String {
    let width = 11.max(label.chars().count() + 2);
    format!("{label:<width$}{value}").trim_end().to_owned()
}

impl Cx<'_> {
    /// The explanation of the memoised family `F` at `key`, with `provenance`; `None` without `--explain`. Reads the
    /// hit flag from the fork taken before the key was resolved and the rest from the store after the value was
    /// computed (the caller computes it first).
    fn explain<F: Query>(&self, key: &F::Key, provenance: Option<String>) -> Option<QueryExplain> {
        if !self.explain {
            return None;
        }
        let policy_word = match F::POLICY {
            Policy::Cold => "Cold",
            Policy::Memo => "Memo",
            Policy::MemoLru => "MemoLru",
        };
        let Some(memo) = self.session.memo().filter(|_| F::POLICY != Policy::Cold) else {
            let policy = if F::POLICY == Policy::Cold {
                format!("Cold · {} is computed on demand and has no memo", F::FAMILY)
            } else {
                format!("{policy_word} · the session keeps no memo store")
            };
            return Some(QueryExplain {
                family: F::FAMILY.to_owned(),
                policy,
                hit: None,
                verified_at: None,
                changed_at: None,
                depends_on: Vec::new(),
                provenance,
            });
        };
        let hit = self.before.as_ref().is_some_and(|b| b.explain::<F>(key).hit);
        let after = memo.explain::<F>(key);
        // A record read twice (the trail keeps read order, collapsing consecutive repeats only) is listed once.
        let mut depends_on: Vec<String> = Vec::new();
        for d in after.deps.iter().take(EXPLAIN_LINES) {
            let what = match (d.input(), d.memo()) {
                (Some(input), _) => input_words(&self.q, input),
                (None, Some((family, key))) => memo_words(&self.q, family, key),
                (None, None) => String::new(),
            };
            let text = format!("{what} (rev {})", d.observed().0);
            if !depends_on.contains(&text) {
                depends_on.push(text);
            }
        }
        if after.truncated {
            depends_on.push("…".to_owned());
        }
        Some(QueryExplain {
            family: F::FAMILY.to_owned(),
            policy: format!(
                "{policy_word} · {} · verified at rev {} ({}) · changed at rev {}",
                F::FAMILY,
                after.verified_at.0,
                if hit { "hit" } else { "miss" },
                after.changed_at.0
            ),
            hit: Some(hit),
            verified_at: Some(after.verified_at.0),
            changed_at: Some(after.changed_at.0),
            depends_on,
            provenance,
        })
    }

    /// The record the key names, checked to be of a kind `accept` takes.
    fn record(&self, key: &str) -> Result<(Uuid, Rec), String> {
        let uuid = self.session.resolve_reference(key)?;
        Ok((uuid, rec(&self.q, uuid)))
    }

    /// The refusal of a key that names a record of another kind.
    fn wrong(&self, key: &str, uuid: Uuid, rec: Rec, why: &str) -> String {
        format!(
            "`{key}` names {}{why}; `{}` takes {}",
            rec_words(&self.q, uuid, rec),
            self.def.name,
            self.def.takes
        )
    }

    /// The device a key names.
    fn device(&self, key: &str) -> Result<Id<Device>, String> {
        match self.record(key)? {
            (_, Rec::Device(d)) => Ok(d),
            (uuid, other) => Err(self.wrong(key, uuid, other, "")),
        }
    }

    /// The drawn page a `folio:` key names.
    fn page(&self, key: &str) -> Result<Id<Page>, String> {
        match self.record(key)? {
            (_, Rec::Page(p)) => Ok(p),
            (uuid, other) => Err(self.wrong(key, uuid, other, "")),
        }
    }

    /// The `-` of a query without a key.
    fn no_key(&self, key: &str) -> Result<(), String> {
        if key == "-" {
            Ok(())
        } else {
            Err(format!("`{}` takes no key: give `-`", self.def.name))
        }
    }
}

/// The kind of record a key resolved to, as the adapters tell them apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Rec {
    Device(Id<Device>),
    Function(Id<Function>),
    Connection(Id<Connection>),
    Junction(Id<Junction>),
    Page(Id<Page>),
    Representation(Id<Representation>),
    Other(Option<Kind>),
}

/// The kind of the record `uuid`.
fn rec(q: &Q, uuid: Uuid) -> Rec {
    let id = AnyId::from_uuid(uuid);
    if q.devices().contains(id.cast()) {
        return Rec::Device(id.cast());
    }
    if q.functions().contains(id.cast()) {
        return Rec::Function(id.cast());
    }
    if q.connections().contains(id.cast()) {
        return Rec::Connection(id.cast());
    }
    if q.junctions().contains(id.cast()) {
        return Rec::Junction(id.cast());
    }
    if q.pages().contains(id.cast()) {
        return Rec::Page(id.cast());
    }
    if q.reps().contains(id.cast()) {
        return Rec::Representation(id.cast());
    }
    let kind = if q.project().id.as_uuid() == uuid {
        Some(Kind::Project)
    } else if q.sections().contains(id.cast()) {
        Some(Kind::Section)
    } else if q.structure_nodes().contains(id.cast()) {
        Some(Kind::StructureNode)
    } else if q.potential_defs().contains(id.cast()) {
        Some(Kind::PotentialDef)
    } else if q.report_blocks().contains(id.cast()) {
        Some(Kind::ReportBlock)
    } else if q.conductors().contains(id.cast()) {
        Some(Kind::Conductor)
    } else if q.page_items().contains(id.cast()) {
        Some(Kind::PageItem)
    } else {
        None
    };
    Rec::Other(kind)
}

/// A record kind in words (`structure node`).
fn kind_words(kind: Kind) -> String {
    kind.name().replace('_', " ")
}

/// A record named by its kind and its title (`device -101F1`, `page 101 Main supply`, `wire 102.1`, `function of
/// -101F1`, `main representation of -101F1 on folio 101`).
fn rec_words(q: &Q, uuid: Uuid, rec: Rec) -> String {
    let tag_of_function = |f: Id<Function>| {
        q.functions()
            .get(f)
            .and_then(|x| x.device)
            .map(|d| Session::displayed_tag_text(q, d))
    };
    let kind = match rec {
        Rec::Connection(_) => {
            return format!("wire {}", Session::record_words(q, uuid).unwrap_or_default());
        }
        Rec::Function(f) => {
            return match tag_of_function(f) {
                Some(tag) => format!("function of {tag}"),
                None => "function without a device".to_owned(),
            };
        }
        Rec::Representation(r) => {
            let Some(rep) = q.reps().get(r) else {
                return "representation".to_owned();
            };
            let function = rep.functions.first().copied();
            let device = function.and_then(|f| q.functions().get(f).and_then(|x| x.device));
            let main = device.is_some_and(|d| *q.get::<MainRep>(d) == Some(r));
            let of = function
                .and_then(tag_of_function)
                .map(|t| format!(" of {t}"))
                .unwrap_or_default();
            let on = folio_of(q, rep.page)
                .map(|f| format!(" on folio {f}"))
                .unwrap_or_default();
            return format!("{}representation{of}{on}", if main { "main " } else { "" });
        }
        Rec::Device(_) => Some(Kind::Device),
        Rec::Junction(_) => Some(Kind::Junction),
        Rec::Page(_) => Some(Kind::Page),
        Rec::Other(kind) => kind,
    };
    match (kind, Session::record_words(q, uuid)) {
        (Some(k), Some(t)) if !t.is_empty() && t != k.name() => format!("{} {t}", kind_words(k)),
        (Some(k), _) => kind_words(k),
        (None, _) => "a record".to_owned(),
    }
}

/// A record of a dependency named in words: its kind with its tag, folio or title, else its ID (D12). A device's line
/// names its tag specification (letter and counter, or the frozen tag).
fn id_words(q: &Q, kind: Kind, id: AnyId) -> String {
    let uuid = id.as_uuid();
    let r = rec(q, uuid);
    if r != Rec::Other(None) {
        let words = rec_words(q, uuid, r);
        let spec = match r {
            // A terminal's tag comes from its strip: its own specification says nothing.
            Rec::Device(d) => q
                .devices()
                .get(d)
                .filter(|dev| dev.aspects.terminal.is_none())
                .map(|dev| match &dev.tag.frozen {
                    Some(f) => format!(" · tag specification: frozen {}", f.text.as_str()),
                    None => format!(
                        " · tag specification: letter {}, counter {}",
                        dev.tag.letter.as_str(),
                        dev.tag.counter.map_or_else(|| "?".to_owned(), |c| c.to_string())
                    ),
                }),
            _ => None,
        };
        return format!("{words}{}", spec.unwrap_or_default());
    }
    // A library copy by its part number, key or name; a channel by its number and module.
    let named = |item: Option<Value>| {
        item.and_then(|v| {
            ["part_number", "key", "name"]
                .iter()
                .find_map(|m| v.get(*m).and_then(Value::as_str).map(str::to_owned))
        })
    };
    let words = match kind {
        Kind::Part => named(
            q.parts()
                .get(id.cast())
                .and_then(|c| serde_json::to_value(&c.item).ok()),
        ),
        Kind::Symbol => named(
            q.symbols()
                .get(id.cast())
                .and_then(|c| serde_json::to_value(&c.item).ok()),
        ),
        Kind::Fndef => named(
            q.fndefs()
                .get(id.cast())
                .and_then(|c| serde_json::to_value(&c.item).ok()),
        ),
        Kind::Form => named(
            q.forms()
                .get(id.cast())
                .and_then(|c| serde_json::to_value(&c.item).ok()),
        ),
        Kind::Channel => q
            .table::<ew_schema::Channel>()
            .get(id.cast())
            .map(|c| format!("{} of {}", c.number, Session::displayed_tag_text(q, c.module))),
        _ => None,
    };
    if let Some(w) = words {
        return format!("{} {w}", kind_words(kind));
    }
    match Session::record_words(q, uuid) {
        Some(t) if !t.is_empty() && t != kind.name() => format!("{} {t}", kind_words(kind)),
        _ => format!("{} {}", kind_words(kind), uuid.hyphenated()),
    }
}

/// A camel-case index name in words (`FunctionsByDevice` → `functions by device`).
fn camel_words(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push(' ');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A connection point or junction in words.
fn endpoint_words(q: &Q, at: EndpointKey) -> String {
    match at {
        EndpointKey::Cp(function, idx) => {
            let label = ew_derive::cp_label(q, ew_derive::CpRef { function, idx });
            if label.is_empty() {
                format!("connection point {idx} of a function not in the project")
            } else {
                format!("connection point {label}")
            }
        }
        EndpointKey::Junction(j) => id_words(q, Kind::Junction, j.cast()),
    }
}

/// An input of the memo engine in words: the record by kind and tag, folio or ID; a table; an index bucket.
fn input_words(q: &Q, input: &MemoInput) -> String {
    match input {
        MemoInput::Read(dep) => match dep {
            DepKey::Project => {
                "project settings · identifier set, numbering rules, master data and derivation profile"
                    .to_owned()
            }
            DepKey::Assets => "asset store".to_owned(),
            DepKey::Row((kind, id)) => id_words(q, *kind, *id),
            DepKey::Table(kind) => format!("every {}", kind_words(*kind)),
            DepKey::IndexKey(name, IxKey::Id(id)) => {
                let target = match rec(q, id.as_uuid()) {
                    Rec::Other(None) => format!("record {}", id.as_uuid().hyphenated()),
                    r => rec_words(q, id.as_uuid(), r),
                };
                format!("index {} at {target}", camel_words(&format!("{name:?}")))
            }
            DepKey::IndexKey(name, IxKey::Endpoint(at)) => {
                format!(
                    "index {} at {}",
                    camel_words(&format!("{name:?}")),
                    endpoint_words(q, *at)
                )
            }
            DepKey::Index(name) => format!("index {}", camel_words(&format!("{name:?}"))),
            DepKey::Reserved => "revisions".to_owned(),
        },
        MemoInput::Page(page) => format!("rows drawn on {}", id_words(q, Kind::Page, page.cast())),
        MemoInput::Shard(kind, n) => format!("{} shard {n}", kind_words(*kind)),
        MemoInput::IndexShard(name, n) => format!("index {} shard {n}", camel_words(&format!("{name:?}"))),
        MemoInput::Part(part) => format!("part {}", camel_words(&format!("{part:?}"))),
    }
}

/// A memo dependency in words: the family and its key, every ID in the key's text replaced by the record's words.
fn memo_words(q: &Q, family: &str, key: &str) -> String {
    if key == "()" {
        return family.to_owned();
    }
    format!("{family}({})", ids_to_words(q, key))
}

/// `text` with every `Id(<uuid>)` and bare hyphenated UUID of a record replaced by its words; an ID that names no
/// record stays.
fn ids_to_words(q: &Q, text: &str) -> String {
    const LEN: usize = 36;
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < text.len() {
        let parsed = text
            .get(i..i + LEN)
            .filter(|w| w.as_bytes().get(8) == Some(&b'-'))
            .and_then(|w| Uuid::parse_str(w).ok());
        if let Some(uuid) = parsed {
            let r = rec(q, uuid);
            if r != Rec::Other(None) {
                let words = rec_words(q, uuid, r);
                // `Id(<uuid>)` becomes the words alone.
                if out.ends_with("Id(") && bytes.get(i + LEN) == Some(&b')') {
                    out.truncate(out.len() - 3);
                    out.push_str(&words);
                    i += LEN + 1;
                } else {
                    out.push_str(&words);
                    i += LEN;
                }
                continue;
            }
        }
        let c = text[i..].chars().next().unwrap_or(' ');
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// The provenance's text: a derived row's `via`, a frozen or overridden row's note.
fn provenance_text(p: &Provenance) -> Option<String> {
    match p {
        Provenance::Derived { via } => Some(via.clone()),
        Provenance::Frozen { note } | Provenance::Overridden { note } => Some(note.clone()),
        Provenance::Inherited { from } => Some(format!("inherited from {from}")),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------------------
// The adapters

fn device_tag(cx: &Cx<'_>, key: &str) -> Result<Answer, String> {
    let d = cx.device(key)?;
    let q = &cx.q;
    let full = q
        .get::<DeviceTag>(d)
        .as_ref()
        .as_ref()
        .map(|t| t.full.clone())
        .unwrap_or_default();
    let out = DeviceTagOutput {
        full,
        displayed: Session::displayed_tag_text(q, d),
    };
    let provenance = cx
        .explain
        .then(|| provenance_text(&Session::tag_provenance(q, d)))
        .flatten();
    Ok(Answer {
        lines: vec![line("full", &out.full), line("displayed", &out.displayed)],
        output: to_value(&out),
        explain: cx.explain::<DeviceTag>(&d, provenance),
    })
}

fn displayed_tag(cx: &Cx<'_>, key: &str) -> Result<Answer, String> {
    let Some((device_key, folio_key)) = key.rsplit_once(',') else {
        return Err(format!("`{key}`: `{}` takes {}", cx.def.name, cx.def.takes));
    };
    let d = cx.device(device_key)?;
    let page = cx.page(folio_key)?;
    let out = DisplayedTagOutput {
        displayed: cx.q.get::<DisplayedTag>((d, page)).as_ref().clone(),
    };
    Ok(Answer {
        lines: vec![line("displayed", &out.displayed)],
        output: to_value(&out),
        explain: cx.explain::<DisplayedTag>(&(d, page), None),
    })
}

fn page_order(cx: &Cx<'_>, key: &str) -> Result<Answer, String> {
    cx.no_key(key)?;
    let out = PageOrderOutput {
        folios: cx
            .q
            .get::<PageOrder>(())
            .entries
            .iter()
            .map(|e| e.folio.clone())
            .collect(),
    };
    Ok(Answer {
        lines: out.folios.clone(),
        output: to_value(&out),
        explain: cx.explain::<PageOrder>(&(), None),
    })
}

/// The folio label of a drawn page in the page order.
fn folio_of(q: &Q, page: Id<Page>) -> Option<String> {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.page == PageRef::Drawn(page))
        .map(|e| e.folio.clone())
}

fn cell_of(cx: &Cx<'_>, key: &str) -> Result<Answer, String> {
    let d = cx.device(key)?;
    let q = &cx.q;
    let rep = *q.get::<MainRep>(d);
    let folio = rep
        .and_then(|r| q.reps().get(r).map(|x| x.page))
        .and_then(|p| folio_of(q, p));
    let cell = rep.and_then(|r| {
        q.get::<RepAnchorCell>(r)
            .as_ref()
            .as_ref()
            .map(ToString::to_string)
    });
    let out = CellOfOutput { folio, cell };
    let provenance = cx
        .explain
        .then(|| provenance_text(&Session::main_location_provenance()))
        .flatten();
    let explain = match rep {
        Some(r) => cx.explain::<RepAnchorCell>(&r, provenance),
        None => cx.explain::<MainRep>(&d, provenance),
    };
    Ok(Answer {
        lines: vec![
            line("folio", out.folio.as_deref().unwrap_or("-")),
            line("cell", out.cell.as_deref().unwrap_or("-")),
        ],
        output: to_value(&out),
        explain,
    })
}

/// The potential at the first endpoint of `points` that has one.
fn potential_at(q: &Q, points: impl IntoIterator<Item = EndpointKey>) -> Option<PotentialId> {
    let partition = q.get::<PotentialPartition>(());
    points
        .into_iter()
        .find_map(|at| partition.potential_at(at).map(|p| p.id))
}

/// The potential of a connection's first end that has one (the property sheet's rule).
fn potential_of_connection(q: &Q, c: Id<Connection>) -> Option<PotentialId> {
    let conn = q.connections().get(c)?;
    potential_at(q, [&conn.a, &conn.b].into_iter().filter_map(EndpointKey::of))
}

/// The potential of a terminal: of its functions' connection points, in function and index order.
fn potential_of_terminal(q: &Q, d: Id<Device>) -> Option<PotentialId> {
    let mut points: Vec<EndpointKey> = Vec::new();
    for f in q.idx().functions_of_device(d) {
        for cp in q.get::<ew_derive::FunctionCps>(f).iter() {
            points.push(EndpointKey::Cp(f, cp.idx));
        }
    }
    potential_at(q, points)
}

fn potential_name(cx: &Cx<'_>, key: &str) -> Result<Answer, String> {
    let q = &cx.q;
    let (uuid, r) = cx.record(key)?;
    let potential = match r {
        Rec::Connection(c) => potential_of_connection(q, c),
        Rec::Junction(j) => potential_at(q, [EndpointKey::Junction(j)]),
        Rec::Device(d) if q.devices().get(d).is_some_and(|x| x.aspects.terminal.is_some()) => {
            potential_of_terminal(q, d)
        }
        Rec::Device(_) => return Err(cx.wrong(key, uuid, r, ", which is not a terminal")),
        other => return Err(cx.wrong(key, uuid, other, "")),
    };
    let Some(p) = potential else {
        return Err(format!(
            "`{key}`: {} belongs to no potential",
            rec_words(q, uuid, r)
        ));
    };
    let names = q.get::<PotentialNameOf>(p);
    let out = PotentialNameOutput {
        name: names.name.text.clone(),
        aliases: names.aliases.clone(),
    };
    Ok(Answer {
        lines: vec![line("name", &out.name), line("aliases", &out.aliases.join(", "))],
        output: to_value(&out),
        explain: cx.explain::<PotentialNameOf>(&p, None),
    })
}

/// A wire property's value as text: a JSON string as is, a list of codes joined (`GNYE`), else its JSON.
fn value_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(items) if items.iter().all(Value::is_string) => {
            items.iter().filter_map(Value::as_str).collect::<String>()
        }
        other => other.to_string(),
    }
}

fn wire_props_of(cx: &Cx<'_>, key: &str) -> Result<Answer, String> {
    let q = &cx.q;
    let c = match cx.record(key)? {
        (_, Rec::Connection(c)) => c,
        (uuid, other) => return Err(cx.wrong(key, uuid, other, "")),
    };
    let Some(p) = potential_of_connection(q, c) else {
        return Err(format!(
            "`{key}`: {} belongs to no potential",
            rec_words(q, c.as_uuid(), Rec::Connection(c))
        ));
    };
    let potential = q.get::<PotentialNameOf>(p).name.text.clone();
    let map = q.get::<WirePropsOf>(p);
    let numbers = q.get::<WireNumbers>(());
    let mut wires: Vec<(Option<String>, Id<Connection>)> = map
        .by_connection
        .get(&c)
        .map(|ws| {
            ws.iter()
                .map(|w| (numbers.number(*w).map(|n| n.label.clone()), *w))
                .collect()
        })
        .unwrap_or_default();
    wires.sort_by(|(a, wa), (b, wb)| match (a, b) {
        (Some(a), Some(b)) => ew_derive::ew_mltext::natural_cmp(a, b)
            .then_with(|| a.cmp(b))
            .then_with(|| wa.cmp(wb)),
        _ => a.is_none().cmp(&b.is_none()).then_with(|| wa.cmp(wb)),
    });
    let prop = |value: Option<(Value, &ew_derive::PropSource)>, class| {
        value.map(|(v, source)| WireProp {
            value: value_text(&v),
            via: Session::wire_prop_via(q, source, &potential, class),
        })
    };
    let out = WirePropsOutput {
        potential: potential.clone(),
        wires: wires
            .iter()
            .filter_map(|(number, w)| {
                let e = map.wires.get(w)?;
                let class = e.class.as_ref();
                Some(WireOut {
                    number: number.clone().unwrap_or_else(|| "?".to_owned()),
                    cross_section: prop(
                        e.csa
                            .as_ref()
                            .and_then(|x| serde_json::to_value(&x.value).ok().map(|v| (v, &x.source))),
                        class,
                    ),
                    colour: prop(
                        e.colour
                            .as_ref()
                            .and_then(|x| serde_json::to_value(&x.value).ok().map(|v| (v, &x.source))),
                        class,
                    ),
                })
            })
            .collect(),
    };
    let mut lines = vec![line("potential", &out.potential)];
    for w in &out.wires {
        let text = |p: &Option<WireProp>| {
            p.as_ref()
                .map_or_else(|| "-".to_owned(), |p| format!("{} ({})", p.value, p.via))
        };
        lines.push(line(
            "wire",
            &format!(
                "{}: cross-section {}, colour {}",
                w.number,
                text(&w.cross_section),
                text(&w.colour)
            ),
        ));
    }
    Ok(Answer {
        lines,
        output: to_value(&out),
        explain: cx.explain::<WirePropsOf>(&p, None),
    })
}

fn xref_text_of(cx: &Cx<'_>, key: &str) -> Result<Answer, String> {
    let d = cx.device(key)?;
    let q = &cx.q;
    let rep = *q.get::<MainRep>(d);
    let text = rep
        .map(|r| q.get::<XrefTextOf>(r).text.clone())
        .unwrap_or_default();
    let out = XrefTextOutput { text };
    let explain = match rep {
        Some(r) => cx.explain::<XrefTextOf>(&r, None),
        None => cx.explain::<MainRep>(&d, None),
    };
    Ok(Answer {
        lines: vec![line("xref", &out.text)],
        output: to_value(&out),
        explain,
    })
}

fn channel_addresses(cx: &Cx<'_>, key: &str) -> Result<Answer, String> {
    let q = &cx.q;
    let (uuid, r) = cx.record(key)?;
    let module = match r {
        Rec::Device(d) if q.devices().get(d).is_some_and(|x| x.aspects.plc_module.is_some()) => d,
        Rec::Device(_) => return Err(cx.wrong(key, uuid, r, ", which is not a PLC module")),
        other => return Err(cx.wrong(key, uuid, other, "")),
    };
    let out = ChannelAddressesOutput {
        channels: q
            .get::<ChannelAddresses>(module)
            .iter()
            .map(|e| ChannelAddress {
                number: e.number,
                address: match &e.address {
                    ChannelAddr::At { text, .. } => text.clone(),
                    ChannelAddr::Tbd => "TBD".to_owned(),
                    ChannelAddr::Excluded => "excluded".to_owned(),
                    ChannelAddr::Unresolved => "unresolved".to_owned(),
                },
            })
            .collect(),
    };
    Ok(Answer {
        lines: out
            .channels
            .iter()
            .map(|c| line(&format!("channel {}", c.number), &c.address))
            .collect(),
        output: to_value(&out),
        explain: cx.explain::<ChannelAddresses>(&module, None),
    })
}

fn plc_address_text(cx: &Cx<'_>, key: &str) -> Result<Answer, String> {
    let q = &cx.q;
    let function = match cx.record(key)? {
        (_, Rec::Function(f)) => f,
        (uuid, Rec::Device(d)) => match *q.get::<MainFunction>(d) {
            Some(f) => f,
            None => return Err(cx.wrong(key, uuid, Rec::Device(d), ", which has no main function")),
        },
        (uuid, other) => return Err(cx.wrong(key, uuid, other, "")),
    };
    let out = PlcAddressTextOutput {
        text: q.get::<PlcAddressText>(function).text.clone(),
    };
    Ok(Answer {
        lines: vec![line("address", out.text.as_deref().unwrap_or("-"))],
        output: to_value(&out),
        explain: cx.explain::<PlcAddressText>(&function, None),
    })
}

fn wire_numbers(cx: &Cx<'_>, key: &str) -> Result<Answer, String> {
    let page = cx.page(key)?;
    let out = WireNumbersOutput {
        numbers: cx
            .q
            .get::<WireNumbers>(())
            .wires
            .iter()
            .filter_map(|w| w.number.as_ref())
            .filter(|n| n.page == page)
            .map(|n| n.label.clone())
            .collect(),
    };
    Ok(Answer {
        lines: out.numbers.clone(),
        output: to_value(&out),
        explain: cx.explain::<WireNumbers>(&(), None),
    })
}

/// A multi-language text in the project's source language, else its neutral text, else its first language.
fn ml_text(q: &Q, text: &ew_derive::ew_mltext::MlText) -> String {
    let source = q.project().languages.as_ref().map(|l| l.source.to_string());
    match serde_json::to_value(text).unwrap_or(Value::Null) {
        Value::String(s) => s,
        Value::Object(m) => source
            .as_deref()
            .and_then(|s| m.get(s))
            .or_else(|| m.values().next())
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        _ => String::new(),
    }
}

/// A title-block value as printed text.
fn tb_text(q: &Q, value: &TbValue) -> String {
    match value {
        TbValue::Text(t) => t.clone(),
        TbValue::Ml(t) => ml_text(q, t),
        TbValue::Date(d) => value_text(&serde_json::to_value(d).unwrap_or(Value::Null)),
        TbValue::Tag { displayed, .. } => displayed.clone(),
        TbValue::Asset(_) => "(image)".to_owned(),
    }
}

fn title_block(cx: &Cx<'_>, key: &str) -> Result<Answer, String> {
    let page = cx.page(key)?;
    let r = PageRef::Drawn(page);
    let tb = cx.q.get::<TitleBlockValues>(r);
    let out = TitleBlockOutput {
        values: tb
            .values
            .iter()
            .map(|(path, v)| (path.clone(), tb_text(&cx.q, v)))
            .collect(),
    };
    Ok(Answer {
        lines: out.values.iter().map(|(p, v)| line(p, v)).collect(),
        output: to_value(&out),
        explain: cx.explain::<TitleBlockValues>(&r, None),
    })
}

fn bom_rows(cx: &Cx<'_>, key: &str) -> Result<Answer, String> {
    cx.no_key(key)?;
    let q = &cx.q;
    let order = q.get::<PageOrder>(());
    let folio = |p: &PageRef| {
        order
            .entries
            .iter()
            .find(|e| e.page == *p)
            .map(|e| e.folio.clone())
    };
    let rows = q.get::<BomRows>(BomMode::Component);
    let out = BomRowsOutput {
        rows: rows
            .iter()
            .map(|r| BomRowOut {
                position: if r.configured {
                    format!("A{}", r.pos)
                } else {
                    r.pos.to_string()
                },
                quantity: r.qty.to_string(),
                part_number: r.part_number.clone(),
                manufacturer: ew_derive::manufacturer_name(q, &r.manufacturer),
                description: ml_text(q, &r.description),
                tags: r
                    .devices
                    .iter()
                    .map(|d| Session::displayed_tag_text(q, *d))
                    .collect(),
                folios: r.pages.iter().filter_map(folio).collect(),
            })
            .collect(),
    };
    Ok(Answer {
        lines: out
            .rows
            .iter()
            .map(|r| {
                line(
                    &r.position,
                    &format!(
                        "{} × {} {} {}",
                        r.quantity,
                        r.part_number,
                        r.manufacturer,
                        r.tags.join(" ")
                    ),
                )
            })
            .collect(),
        output: to_value(&out),
        explain: cx.explain::<BomRows>(&BomMode::Component, None),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_is_the_closed_set_of_d11_in_name_order() {
        let names: Vec<&str> = REGISTRY.iter().map(|q| q.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
        // D11 is closed (WP-S1.22 §10.2 item 6): the twelve names are the oracle.
        assert_eq!(
            names,
            [
                "bom_rows",
                "cell_of",
                "channel_addresses",
                "device_tag",
                "displayed_tag",
                "page_order",
                "plc_address_text",
                "potential_name",
                "title_block",
                "wire_numbers",
                "wire_props_of",
                "xref_text_of",
            ]
        );
        for q in REGISTRY {
            assert_eq!(q.output_version, OUTPUT_VERSION, "{}", q.name);
            let s = q.output_schema();
            assert_eq!(s["type"], "object", "{}", q.name);
        }
    }

    #[test]
    fn camel_case_names_read_as_words() {
        assert_eq!(camel_words("FunctionsByDevice"), "functions by device");
        assert_eq!(camel_words("Notes"), "notes");
    }
}
