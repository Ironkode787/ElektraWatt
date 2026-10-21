// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `Importer` trait, its context and its result.

use std::fmt;

use ew_ids::{Id, Uuid};
use ew_schema::{Date, ImportLineage, LibDocument, Project, Record};

use crate::{ImportInput, ImportReport};

/// How sure an importer is that it can read a file (doc 14 §2.1 `sniff`): content first, the file name
/// extension last.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Detection {
    /// Not this format.
    No,
    /// Possibly this format, with a score 0–100 (higher is surer); used to rank importers.
    Maybe(u8),
    /// Certainly this format.
    Certain,
}

/// An existing import lineage to import into (doc 14 §2.3): its record ID and its namespace, which the host
/// reads from the target's `ImportLineage` record.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LineageInfo {
    /// The `ImportLineage` record ID.
    pub id: Id<ImportLineage>,
    /// `ImportLineage.namespace`, the namespace of every imported ID.
    pub namespace: Uuid,
}

/// Input limits beyond the file size (doc 14 §11, NFR-SEC-02, FR-IMP-005). A violation refuses the file
/// ([`ImportError::LimitExceeded`]) or skips the item with EW-IMP-001.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    /// Largest accepted input, in bytes.
    pub max_input_bytes: u64,
    /// Deepest accepted XML element nesting.
    pub max_xml_depth: u32,
    /// Most XML nodes in one document: the document root, elements, texts, comments and processing
    /// instructions. Attributes are not nodes; they are bounded by [`Limits::max_xml_attributes`],
    /// [`Limits::max_xml_attributes_per_element`] and [`Limits::max_xml_attribute_work`].
    ///
    /// The default, 1,000,000, keeps a hostile document within the 1 s and 256 MB of doc 14 §11 (measured in
    /// release: a million empty elements parse in about 0.2 s into about 125 MB; ten million take about
    /// 2.1 s and 1.2 GB). Element definitions and typical projects stay far below it. A project importer that
    /// must read larger `.qet` files may raise it explicitly, under its own time and memory budget.
    ///
    /// An XML importer may derive further bounds from it and [`Limits::max_xml_attributes`]: the QET importer
    /// refuses an input with more than `2 × max_xml_nodes + max_xml_attributes` `<` and `=` bytes, because
    /// its parser reserves memory for each of them before parsing.
    pub max_xml_nodes: u32,
    /// Longest accepted XML attribute value, in bytes.
    pub max_attribute_bytes: usize,
    /// Most attributes on one XML element. Enforced before the XML is parsed, by a flat scan.
    ///
    /// The default, 8,192, is set by the parser rather than by the formats: `roxmltree` 0.21.1 checks each
    /// attribute of an element against every earlier one for duplicates, so parsing is quadratic in the
    /// attributes of one element (measured in release: 16,384 take about 0.3 s, 65,536 about 3.8 s, 8,192
    /// about 75 ms), and a hostile element must still be read or refused within the 1 s of doc 14 §11.
    pub max_xml_attributes_per_element: u32,
    /// Most XML attributes in one document. Enforced before the XML is parsed, by a flat scan.
    pub max_xml_attributes: u32,
    /// Most attribute work in one document: the sum, over its elements, of the square of each element's
    /// attribute count. Enforced before the XML is parsed, by a flat scan.
    ///
    /// The parser's duplicate-attribute check is quadratic per element (see
    /// [`Limits::max_xml_attributes_per_element`]), so the parse time of a document grows with this sum,
    /// which the per-element and per-document counts alone do not bound: 128 elements of 8,100 attributes
    /// (10 MB, within both counts) took 13.4 s. The default, 2^26 = 67,108,864, admits one element at the
    /// 8,192 cap, or about 1,000 elements of 256 attributes, in about 90 ms.
    pub max_xml_attribute_work: u64,
    /// Longest accepted XML text node or CDATA section, in bytes. Enforced before the XML is parsed, by a
    /// flat scan: the bytes between a `>` and the next `<` outside comments and CDATA, and the payload of a
    /// CDATA section, each at most this long.
    ///
    /// The default, 1 MiB (1,048,576), closes a gap in the other bounds, which count markup and not text: a
    /// 200 MiB plain-text run in one `<name>` imported in 2.55 s with 1.6 GB extra, because an importer
    /// copies a name several times (white-space collapse, multilingual text, canonical XML, symbol name,
    /// serialisation). Names, descriptions and kind information are a few hundred bytes in practice.
    pub max_text_bytes: u32,
}

impl Limits {
    /// The limits for a standalone element definition: the defaults with `max_input_bytes` of 16 MiB
    /// (16,777,216), `max_xml_attributes` of 131,072 and `max_xml_nodes` of 262,144.
    ///
    /// A standalone QET element definition is kilobytes and has at most hundreds of primitives. The 512 MiB
    /// input, 1,048,576 attributes and 1,000,000 nodes stay those of project files, whose importer reads
    /// them under its own time and memory budget. The two counts are lowered because, under the general
    /// defaults, a 16 MiB element file of about 349,000 three-attribute `<text>` primitives imported in
    /// 0.7 s but allocated about 0.5 GB extra, twice the 256 MB budget of doc 14 §11 (review N7). Under the
    /// preset about 44,000 such primitives fit, well under 100 MiB extra.
    pub fn element_definition() -> Limits {
        Limits {
            max_input_bytes: 16 * 1024 * 1024,
            max_xml_attributes: 131_072,
            max_xml_nodes: 262_144,
            ..Limits::default()
        }
    }

    /// The limits of the golden CSV bootstrap: the defaults with `max_input_bytes` of 64 MiB (67,108,864).
    ///
    /// The total of all table files of one bootstrap is at most 64 MiB (doc 14 §11); each table file is bound
    /// by its own, lower, per-table limits.
    pub fn bootstrap_tables() -> Limits {
        Limits {
            max_input_bytes: 64 * 1024 * 1024,
            ..Limits::default()
        }
    }

    /// The field-wise minimum of `self` and `other`: the stricter of each limit.
    pub fn stricter(self, other: Limits) -> Limits {
        Limits {
            max_input_bytes: self.max_input_bytes.min(other.max_input_bytes),
            max_xml_depth: self.max_xml_depth.min(other.max_xml_depth),
            max_xml_nodes: self.max_xml_nodes.min(other.max_xml_nodes),
            max_attribute_bytes: self.max_attribute_bytes.min(other.max_attribute_bytes),
            max_xml_attributes_per_element: self
                .max_xml_attributes_per_element
                .min(other.max_xml_attributes_per_element),
            max_xml_attributes: self.max_xml_attributes.min(other.max_xml_attributes),
            max_xml_attribute_work: self.max_xml_attribute_work.min(other.max_xml_attribute_work),
            max_text_bytes: self.max_text_bytes.min(other.max_text_bytes),
        }
    }
}

impl Default for Limits {
    /// The values of doc 14 §11: XML depth ≤ 256, attribute value ≤ 16 MiB; inputs up to 512 MiB (the DXF
    /// file limit); 1,000,000 XML nodes (see [`Limits::max_xml_nodes`]); 8,192 attributes per element (see
    /// [`Limits::max_xml_attributes_per_element`]), 1,048,576 per document and an attribute work of 2^26
    /// (see [`Limits::max_xml_attribute_work`]); text nodes and CDATA sections up to 1 MiB (see
    /// [`Limits::max_text_bytes`]).
    fn default() -> Limits {
        Limits {
            max_input_bytes: 512 * 1024 * 1024,
            max_xml_depth: 256,
            max_xml_nodes: 1_000_000,
            max_attribute_bytes: 16 * 1024 * 1024,
            max_xml_attributes_per_element: 8_192,
            max_xml_attributes: 1_048_576,
            max_xml_attribute_work: 1 << 26,
            max_text_bytes: 1024 * 1024,
        }
    }
}

/// How an import treats a value that a prevent rule forbids in a strict project, such as the foreign core
/// colour "BR" (EW-CORE-031; doc 15 §3.5, doc 14 §8.4, FR-CHK-011, D-17).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ImportMode {
    /// The value is refused: the item that carries it is dropped with an error finding (the default; the
    /// golden CSV bootstrap runs strict, doc 14 §8.4).
    #[default]
    Strict,
    /// The value is kept as foreign data and reported, with a quick fix where one exists.
    Lenient,
}

/// What an import runs against (doc 14 §2.1 `ImportCx`, minimal form): plain data, never the model.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportContext {
    /// The project imported into. Project-local library items, such as the per-project "QET import"
    /// library, use it as their `library_id` (doc 05 §13.2).
    pub project: Id<Project>,
    /// The lineage to import into; `None` creates a new one, whose `ImportLineage` record the importer
    /// emits in the batch with the importer's natural anchor (doc 14 §2.3).
    pub lineage: Option<LineageInfo>,
    /// The command's `as_of` date; importers never read the wall clock (doc 07 §3.6).
    pub as_of: Option<Date>,
    /// Input limits.
    pub limits: Limits,
    /// Strict or lenient (an importer that has no strict form, such as the lenient QET project import,
    /// ignores it).
    pub mode: ImportMode,
}

impl ImportContext {
    /// A context for importing into `project`: a new lineage, no `as_of` date, the default [`Limits`] and
    /// [`ImportMode::Strict`].
    pub fn new(project: Id<Project>) -> ImportContext {
        ImportContext {
            project,
            lineage: None,
            as_of: None,
            limits: Limits::default(),
            mode: ImportMode::Strict,
        }
    }
}

/// The result of one import (doc 14 §2.1): format-level records with final IDs, embedded library
/// documents, and the report.
#[derive(Clone, PartialEq, Debug)]
pub struct ImportBatch {
    /// Records, sorted by kind rank, then ID.
    pub records: Vec<Record>,
    /// Library documents (symbols, function definitions, forms) for the project's import library, sorted
    /// by kind, then copy ID.
    pub lib: Vec<LibDocument>,
    /// The report.
    pub report: ImportReport,
}

/// Why an importer refused a whole file. Problems with single items are `ReportEntry`s
/// ([`crate::ReportEntry`]), never errors.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ImportError {
    /// The input exceeds a limit (EW-IMP-001 for the file).
    LimitExceeded {
        /// What was exceeded, such as `"input size"` or `"XML depth"`.
        what: String,
        /// The limit.
        limit: u64,
    },
    /// The input is not well-formed in its format (malformed XML, wrong root element).
    Malformed {
        /// What is wrong.
        message: String,
        /// 1-based line of the problem, if known.
        line: Option<u32>,
    },
    /// The input is well-formed but of a kind or version this importer does not read.
    Unsupported {
        /// What is not supported.
        message: String,
    },
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::LimitExceeded { what, limit } => {
                write!(f, "input exceeds the {what} limit of {limit}")
            }
            ImportError::Malformed {
                message,
                line: Some(n),
            } => write!(f, "malformed input at line {n}: {message}"),
            ImportError::Malformed { message, line: None } => write!(f, "malformed input: {message}"),
            ImportError::Unsupported { message } => write!(f, "unsupported input: {message}"),
        }
    }
}

impl std::error::Error for ImportError {}

/// An importer of one source format (doc 14 §2.1).
///
/// Implementations are pure: `import` reads only its input, never another file, the model, the clock or the
/// network, and is deterministic (NFR-SEC-18). Everything that cannot be mapped becomes a report entry;
/// only a file that cannot be read at all is an [`ImportError`].
pub trait Importer: Send + Sync {
    /// The importer's lower-case ID, also the `ImportLineage.importer`: `"qet"`, `"dxf"`, `"csv"`.
    fn id(&self) -> &'static str;

    /// The importer's version, recorded in the report and the import run; bumped on every mapping change.
    fn version(&self) -> &'static str;

    /// Whether this importer reads a file starting with `head` (the first bytes, at most the whole file)
    /// and named `name` (a file name, never a path). Content decides first; the extension only breaks ties.
    fn detect(&self, head: &[u8], name: &str) -> Detection;

    /// Imports one file.
    ///
    /// # Errors
    ///
    /// [`ImportError`] when the file as a whole cannot be read; item-level problems are report entries.
    fn import(&self, input: &ImportInput, ctx: &mut ImportContext) -> Result<ImportBatch, ImportError>;
}
