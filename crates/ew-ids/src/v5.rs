// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! UUIDv5 identities of instantiated, imported and generated records (ADR-0004 with amendments 1–2,
//! domain model §4.1, doc 05 §2.2).
//!
//! These IDs are computed, so two branches, two imports or two regenerations produce the same ID for the
//! same thing. The formulas name a namespace and components; **this module fixes their byte encoding,
//! which is part of the file format and never changes**:
//!
//! - **Namespace:** the parent UUID the formula names (the function, device, report block, module,
//!   connection or import lineage), or the fixed [`NS_LIBCOPY`] for embedded library copies. Import-lineage
//!   namespaces are themselves derived under the fixed [`NS_IMPORT`] by [`lineage_namespace`].
//! - **Name:** the UTF-8 bytes of the components' text forms joined by U+001F (UNIT SEPARATOR):
//!   - UUIDs as canonical lower-case hyphenated strings;
//!   - integers in decimal, without sign, padding or separators;
//!   - literals (`channel`, `route`) verbatim;
//!   - source keys and version strings exactly as given (no normalisation here).
//! - **Hash:** `Uuid::new_v5(namespace, name)` (SHA-1, RFC 9562 §5.5).
//!
//! | Function | Formula | Name bytes |
//! |---|---|---|
//! | [`cp_id`] | `v5(function, idx)` | `"2"` |
//! | [`report_page_id`] | `v5(report_block, page_index)` | `"7"` |
//! | [`imported_id`] | `v5(lineage namespace, source_key)` | `"element:3f2a"` |
//! | [`lineage_namespace`] | `v5(NS_IMPORT, importer ‖ anchor kind ‖ anchor value)` | `"dxf\u{1F}sha256\u{1F}e3b0…"` |
//! | [`template_function_id`] | `v5(device, template_index)` | `"3"` |
//! | [`plc_channel_id`] | `v5(module, "channel", number)` | `"channel\u{1F}3"` |
//! | [`lib_copy_id`] | `v5(NS_LIBCOPY, library ‖ item)` | `"<library>\u{1F}<item>"` |
//! | [`lib_copy_keep_both_id`] | `v5(NS_LIBCOPY, library ‖ item ‖ version)` | `"<library>\u{1F}<item>\u{1F}<version>"` |
//! | [`plc_config_project_id`] | `v5(station, "plc_config_project")` | `"plc_config_project"` |
//! | [`revision_next_id`] | `v5(latest released revision, "next")` | `"next"` |
//! | [`revision_withdrawal_id`] | `v5(revision, "withdrawal")` | `"withdrawal"` |
//! | [`conflict_file_id`] | `v5(NS_CONFLICT, path)` | `"pages/<id>.jsonl"` |
//! | [`library_item_id`] | `v5(library, key)` | `"ew.fn.coil"` |
//! | [`kind_item_id`] | `v5(library, kind ‖ key)` | `"macro\u{1F}<key>"` |
//! | [`manufacturer_item_id`] | `v5(library, "manufacturer:" name)` | `"manufacturer:siemens ag"` |
//! | [`fork_item_id`] | `v5(copy, "fork")`, `v5(copy, "fork", n)` | `"fork"`, `"fork\u{1F}2"` |
//! | [`macro_instance_id`] | `v5(copy, "instance")`, `v5(copy, "instance", n)` | `"instance"`, `"instance\u{1F}2"` |
//! | [`macro_thumbnail_instance_id`] | `v5(copy, "thumbnail")` | `"thumbnail"` |
//! | [`instance_member_id`] | `v5(instance, local)` | `"<local>"` |
//! | [`drives_link_id`] | `v5(valve, "drives", device)` | `"drives\u{1F}<device>"` |
//! | [`valve_glyph_entity`] | `v5(representation, device)` | the device UUID's 16 bytes |
//! | [`valve_position_entity`] | `v5(representation, "position", n)` | `"position\u{1F}1"` |
//!
//! U+001F cannot occur in a UUID or a decimal integer, so the joined name is unambiguous. Two schemes keep a
//! form of their own: [`manufacturer_item_id`] lower-cases the name and joins with a colon, and
//! [`valve_glyph_entity`] hashes the device UUID's bytes, not its text. The module's
//! constants are [`NS_LIBCOPY`], [`NS_IMPORT`] and [`NS_CONFLICT`]; [`LineageAnchor`] and [`LineageNameError`]
//! belong to [`lineage_namespace`].
//!
//! ```
//! use ew_ids::{Id, Uuid, v5};
//!
//! struct Module;
//! struct Channel;
//!
//! let module: Id<Module> = "0192f3a4-5b6c-7d8e-8f01-23456789abd0".parse().unwrap();
//! let ch: Id<Channel> = v5::plc_channel_id(module, 3);
//! // The same as hashing the documented name by hand:
//! assert_eq!(ch.as_uuid(), Uuid::new_v5(&module.as_uuid(), "channel\u{1F}3".as_bytes()));
//! assert_eq!(ch.to_string(), "c3cb5781-8135-54b1-8948-9f118e22553d");
//! ```

use uuid::Uuid;

use crate::Id;

/// The namespace of embedded library copies (ADR-0004 amendment 2). A random v4 UUID generated once for
/// this purpose; **fixed forever**, since every embedded copy's file name and references derive from it.
pub const NS_LIBCOPY: Uuid = Uuid::from_u128(0x04c1640f_8857_4dfc_9bff_e9baad47fb47);

/// The namespace of import-lineage namespaces (ADR-0004 amendment 4, doc 05 §3.2a). A random v4 UUID
/// minted once for this purpose; **fixed forever**, since every imported record's ID derives from it.
pub const NS_IMPORT: Uuid = Uuid::from_u128(0x7b75186c_f28e_49ce_b7f7_26b137095a65);

/// The namespace of the file-level conflict records of the merge core. A random v4 UUID minted once for this
/// purpose; **fixed forever**, since a file's conflict record is found by its ID.
pub const NS_CONFLICT: Uuid = Uuid::from_u128(0x62f02f37_42d2_48ec_8f0d_692fb85254a0);

/// The component separator, U+001F UNIT SEPARATOR.
const SEP: char = '\u{1F}';

/// `v5(namespace, name)` typed as `Id<T>`.
fn v5<T>(namespace: Uuid, name: &str) -> Id<T> {
    Id::from_uuid(Uuid::new_v5(&namespace, name.as_bytes()))
}

/// The ID of the connection point with index `idx` of `function`: `v5(function, idx)`.
///
/// Computed, never stored; only `CpOverride` rows persist. `idx` is the CP's index in the function
/// definition, which keeps duplicate printed designations distinct (FR-DEV-006).
pub fn cp_id<F, C>(function: Id<F>, idx: u32) -> Id<C> {
    v5(function.as_uuid(), &idx.to_string())
}

/// The ID of generated report page `page_index` of `block`: `v5(report_block, page_index)`, so PDF named
/// destinations stay stable across regenerations.
pub fn report_page_id<B, P>(block: Id<B>, page_index: u32) -> Id<P> {
    v5(block.as_uuid(), &page_index.to_string())
}

/// The ID of an imported record: `v5(namespace, source_key)` (ADR-0004 amendment 1).
///
/// `namespace` is the import lineage's namespace, **derived from** the lineage's anchor by
/// [`lineage_namespace`] (ADR-0004 amendment 4) and stored in `ImportLineage.namespace`; it is not the
/// anchor itself. `source_key` is the importer's key for the source item, used byte for byte as given;
/// importers emit keys in NFC.
pub fn imported_id<T>(namespace: Uuid, source_key: &str) -> Id<T> {
    v5(namespace, source_key)
}

/// What an import lineage is anchored on (doc 14 §2.3). Together with the importer's name it determines the
/// lineage's namespace through [`lineage_namespace`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineageAnchor<'a> {
    /// The `<project uuid>` of a QET 0.200+ project file.
    QetProjectUuid(Uuid),
    /// The SHA-256 of the lineage's first imported file, for sources without a stable identity of their own.
    FirstImportSha256([u8; 32]),
    /// "Import as copy": names the original lineage by its **namespace** UUID (not a record ID) and a copy
    /// number, so two branches importing the same file as a copy mint the same IDs.
    Copy {
        /// The namespace of the lineage this one is a copy of.
        of: Uuid,
        /// The copy number.
        n: u32,
    },
    /// A fixed label, such as `golden` for the golden CSV bootstrap.
    Fixed(&'a str),
}

/// Why [`lineage_namespace`] refused an importer name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineageNameError {
    /// The importer name is empty or contains anything but ASCII lower-case letters, digits and `-`.
    InvalidImporter,
}

impl std::fmt::Display for LineageNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidImporter => {
                f.write_str("importer name must be non-empty ASCII lower-case letters, digits or '-'")
            }
        }
    }
}

impl std::error::Error for LineageNameError {}

/// The namespace of an import lineage: `v5(NS_IMPORT, importer ‖ anchor kind ‖ anchor value)` (ADR-0004
/// amendment 4, doc 14 §2.3). The name is the U+001F-joined text
/// `<importer>␟project␟<uuid>`, `<importer>␟sha256␟<64 hex digits>`, `<importer>␟copy␟<uuid>␟<n>` or
/// `<importer>␟fixed␟<label>`, with UUIDs and hex digits in lower case.
///
/// The result is stored in `ImportLineage.namespace`; compute it once, when the lineage is created.
///
/// # Errors
///
/// [`LineageNameError::InvalidImporter`] unless `importer` is non-empty and made of ASCII lower-case
/// letters, digits and `-`.
///
/// ```
/// use ew_ids::v5::{LineageAnchor, lineage_namespace};
///
/// // The SHA-256 of the empty input.
/// let digest: [u8; 32] = [
///     0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24,
///     0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
/// ];
/// let ns = lineage_namespace("dxf", &LineageAnchor::FirstImportSha256(digest)).unwrap();
/// assert_eq!(ns.to_string(), "a1ba512b-c515-50c5-af0b-26fa3b5599f5");
/// assert!(lineage_namespace("DXF", &LineageAnchor::Fixed("golden")).is_err());
/// ```
pub fn lineage_namespace(importer: &str, anchor: &LineageAnchor<'_>) -> Result<Uuid, LineageNameError> {
    let valid = !importer.is_empty()
        && importer
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !valid {
        return Err(LineageNameError::InvalidImporter);
    }
    let name = match anchor {
        LineageAnchor::QetProjectUuid(u) => format!("{importer}{SEP}project{SEP}{}", u.hyphenated()),
        LineageAnchor::FirstImportSha256(d) => {
            let hex: String = d.iter().map(|b| format!("{b:02x}")).collect();
            format!("{importer}{SEP}sha256{SEP}{hex}")
        }
        LineageAnchor::Copy { of, n } => {
            format!("{importer}{SEP}copy{SEP}{}{SEP}{n}", of.hyphenated())
        }
        LineageAnchor::Fixed(label) => format!("{importer}{SEP}fixed{SEP}{label}"),
    };
    Ok(Uuid::new_v5(&NS_IMPORT, name.as_bytes()))
}

/// The ID of the function created from template `template_index` of a part's function template on
/// `device`: `v5(device, template_index)` (ADR-0004 amendment 2).
pub fn template_function_id<D, F>(device: Id<D>, template_index: u32) -> Id<F> {
    v5(device.as_uuid(), &template_index.to_string())
}

/// The ID of PLC channel `number` of `module`: `v5(module, "channel", number)` (ADR-0004 amendment 2).
/// Stored, because a channel carries authored data; two branches that add the same module mint the same
/// channel IDs.
pub fn plc_channel_id<M, C>(module: Id<M>, number: u16) -> Id<C> {
    v5(module.as_uuid(), &format!("channel{SEP}{number}"))
}

/// The ID of the PLC tag of `channel` in an exchange file: `v5(channel, "tag")` (doc 10 §4.2, FR-PLC-056). Never
/// stored: a tag is the symbol of its channel.
pub fn plc_tag_id<C, T>(channel: Id<C>) -> Id<T> {
    v5(channel.as_uuid(), "tag")
}

/// The ID of a record of a configuration project's exchange snapshot: `v5(config_project, what, object, target)`,
/// `what` being `exchange` for the exchanged values of `target` and `foreign` for its foreign ID, `object` the
/// object's kind name (`station`, `tag`). Computed, so that recording the same exchange twice writes the same
/// records.
pub fn exchange_id<P, T>(config_project: Id<P>, what: &str, object: &str, target: Uuid) -> Id<T> {
    v5(
        config_project.as_uuid(),
        &format!("{what}{SEP}{object}{SEP}{}", target.hyphenated()),
    )
}

/// The ID of the embedded copy of catalogue `item` from `library`:
/// `v5(NS_LIBCOPY, library ‖ item)` (ADR-0004 amendment 2). One copy per item and project, so "update from
/// library" rewrites one file and leaves every reference untouched.
pub fn lib_copy_id<T>(library: Uuid, item: Uuid) -> Id<T> {
    v5(
        NS_LIBCOPY,
        &format!("{}{SEP}{}", library.hyphenated(), item.hyphenated()),
    )
}

/// The ID of a second embedded copy minted only by an explicit "keep both" after a cross-project paste
/// of another version: `v5(NS_LIBCOPY, library ‖ item ‖ version)` (ADR-0004 amendment 2).
pub fn lib_copy_keep_both_id<T>(library: Uuid, item: Uuid, version: &str) -> Id<T> {
    v5(
        NS_LIBCOPY,
        &format!("{}{SEP}{}{SEP}{version}", library.hyphenated(), item.hyphenated()),
    )
}

/// The ID of the configuration project that the first PLC station of a project creates on demand:
/// `v5(station, "plc_config_project")` (normaliser N0). Stored; computed so that a replay or a redo
/// of the placement mints the same record.
pub fn plc_config_project_id<S, P>(station: Id<S>) -> Id<P> {
    v5(station.as_uuid(), "plc_config_project")
}

/// The ID of the open revision that the first content edit after a release adds (`revision.ensure_open`):
/// `v5(latest, "next")`, where `latest` is the latest released revision. Computed, so a replay of
/// the edit opens the same record.
pub fn revision_next_id<R>(latest: Id<R>) -> Id<R> {
    v5(latest.as_uuid(), "next")
}

/// The ID of the withdrawal record of `revision` (`revision.withdraw`): `v5(revision, "withdrawal")`.
/// One withdrawal per revision, so a second one is the same record.
pub fn revision_withdrawal_id<R>(revision: Id<R>) -> Id<R> {
    v5(revision.as_uuid(), "withdrawal")
}

/// The ID of the file-level conflict record of the project file at `path` (project-relative, `/`
/// separators): `v5(NS_CONFLICT, path)`. One per file, so two merges that conflict on
/// the same file name the same record.
pub fn conflict_file_id<T>(path: &str) -> Id<T> {
    v5(NS_CONFLICT, path)
}

/// The item ID of the item of `library` named by `key` (the system library's starter content):
/// `v5(library, key)`.
pub fn library_item_id(library: Uuid, key: &str) -> Uuid {
    Uuid::new_v5(&library, key.as_bytes())
}

/// The item ID of the item of kind `kind` saved under `key` in `library`: `v5(library, kind ‖ key)`. A macro is of
/// kind `macro`, a symbol drawn in the symbol editor of kind `symbol` and its function definition of kind `fndef`,
/// so that a later save of the key is the same item.
pub fn kind_item_id(library: Uuid, kind: &str, key: &str) -> Uuid {
    Uuid::new_v5(&library, format!("{kind}{SEP}{key}").as_bytes())
}

/// The item ID of the manufacturer `name` made in `library`: `v5(library, "manufacturer:" name)`, the name in
/// lower case (Unicode's mapping) after a colon.
pub fn manufacturer_item_id(library: Uuid, name: &str) -> Uuid {
    Uuid::new_v5(
        &library,
        format!("manufacturer:{}", name.to_lowercase()).as_bytes(),
    )
}

/// The item ID of a fork of the held copy `copy`: `v5(copy, "fork")` for `None`, `v5(copy, "fork", n)` for
/// `Some(n)`.
pub fn fork_item_id(copy: Uuid, n: Option<u64>) -> Uuid {
    match n {
        None => Uuid::new_v5(&copy, b"fork"),
        Some(n) => Uuid::new_v5(&copy, format!("fork{SEP}{n}").as_bytes()),
    }
}

/// The ID of an instance of the macro copy `copy`: `v5(copy, "instance")` for `None`, `v5(copy, "instance", n)` for
/// `Some(n)`.
pub fn macro_instance_id(copy: Uuid, n: Option<usize>) -> Uuid {
    match n {
        None => Uuid::new_v5(&copy, b"instance"),
        Some(n) => Uuid::new_v5(&copy, format!("instance{SEP}{n}").as_bytes()),
    }
}

/// The ID of the instance of the macro copy `copy` that its thumbnail draws: `v5(copy, "thumbnail")`.
pub fn macro_thumbnail_instance_id(copy: Uuid) -> Uuid {
    Uuid::new_v5(&copy, b"thumbnail")
}

/// The ID that `macro.insert` gives the record of local ID `local` in `instance`: `v5(instance, local)`.
pub fn instance_member_id(instance: Uuid, local: Uuid) -> Uuid {
    Uuid::new_v5(&instance, local.hyphenated().to_string().as_bytes())
}

/// The ID of the `drives` link a fluid line adds from `valve` to `device`: `v5(valve, "drives", device)`.
pub fn drives_link_id(valve: Uuid, device: Uuid) -> Uuid {
    Uuid::new_v5(&valve, format!("drives{SEP}{}", device.hyphenated()).as_bytes())
}

/// The entity of the glyph of the valve `device` on the overview representation `rep`: `v5(rep, device)` over
/// the device UUID's 16 bytes. Never stored.
pub fn valve_glyph_entity(rep: Uuid, device: Uuid) -> Uuid {
    Uuid::new_v5(&rep, device.as_bytes())
}

/// The entity of valve position `position` (from 1) on the overview representation `rep`:
/// `v5(rep, "position", position)`. Never stored.
pub fn valve_position_entity(rep: Uuid, position: usize) -> Uuid {
    Uuid::new_v5(&rep, format!("position{SEP}{position}").as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct K;

    fn id(s: &str) -> Id<K> {
        s.parse().unwrap()
    }

    fn uuid(s: &str) -> Uuid {
        id(s).as_uuid()
    }

    const FUNCTION: &str = "0192f3a4-5b6c-7d8e-9f01-23456789abcd";
    const DEVICE: &str = "0192f3a4-5b6c-7d8e-af01-23456789abce";
    const BLOCK: &str = "0192f3a4-5b6c-7d8e-bf01-23456789abcf";
    const MODULE: &str = "0192f3a4-5b6c-7d8e-8f01-23456789abd0";
    const LIBRARY: &str = "0192f3a4-5b6c-7d8e-af01-23456789abd2";
    const ITEM: &str = "0192f3a4-5b6c-7d8e-bf01-23456789abd3";
    const REVISION: &str = "0192f3a4-5b6c-7d8e-af01-23456789abd4";
    const LINEAGE: &str = "6f1c9a0e-3b7d-4e2f-8a5c-1d0e9b8a7c6f";

    /// Fixed vectors, computed independently (Python's `uuid.uuid5` over the documented name bytes).
    /// They are part of the file format: a change here breaks every existing project.
    #[test]
    fn fixed_vectors() {
        let s = |i: Id<K>| i.to_string();
        assert_eq!(NS_LIBCOPY.to_string(), "04c1640f-8857-4dfc-9bff-e9baad47fb47");
        assert_eq!(s(cp_id(id(FUNCTION), 0)), "661d07a7-1d8a-5b19-9305-97e62b1db2b9");
        assert_eq!(s(cp_id(id(FUNCTION), 1)), "2913bb81-10ef-50c3-9946-f02fd643f882");
        assert_eq!(s(cp_id(id(FUNCTION), 2)), "15a583e3-755d-5f84-b46a-b2317c7b9b33");
        assert_eq!(
            s(report_page_id(id(BLOCK), 0)),
            "ff9efc71-1afc-58a6-a30a-354e83408911"
        );
        assert_eq!(
            s(report_page_id(id(BLOCK), 7)),
            "993a0206-94c5-5c04-84e8-526f150e6e32"
        );
        assert_eq!(
            s(imported_id(uuid(LINEAGE), "element:3f2a")),
            "9ba319d6-db7e-5984-8aa3-41a97b10c324"
        );
        assert_eq!(
            s(imported_id(uuid(LINEAGE), "Gr\u{f6}\u{df}e/-101F1")),
            "81eeaa9a-3951-585e-9e54-362777a1ecb6"
        );
        assert_eq!(
            s(template_function_id(id(DEVICE), 3)),
            "3518e93d-59fa-57e9-8a81-ffb5b80d272f"
        );
        assert_eq!(
            s(plc_channel_id(id(MODULE), 0)),
            "7afbba6f-9ea6-5715-bcfb-064d1853a0e6"
        );
        assert_eq!(
            s(plc_channel_id(id(MODULE), 3)),
            "c3cb5781-8135-54b1-8948-9f118e22553d"
        );
        assert_eq!(
            s(lib_copy_id(uuid(LIBRARY), uuid(ITEM))),
            "8f8d6839-368b-5d83-9f7e-f281852eed18"
        );
        assert_eq!(
            s(lib_copy_keep_both_id(uuid(LIBRARY), uuid(ITEM), "2.1.0")),
            "b89c20b8-52f5-5f8b-bd16-3de9f1385a47"
        );
        assert_eq!(
            s(plc_config_project_id(id(DEVICE))),
            "e854e68b-948c-5290-a7d7-9db5bf1f9833"
        );
        assert_eq!(
            s(revision_next_id(id(REVISION))),
            "247e2e70-0e5b-5a39-bcef-3356e6ef9577"
        );
        assert_eq!(
            s(revision_withdrawal_id(id(REVISION))),
            "c5147bfd-2854-507d-92aa-c18d958f88a0"
        );
        assert_eq!(NS_CONFLICT.to_string(), "62f02f37-42d2-48ec-8f0d-692fb85254a0");
        assert_eq!(
            s(conflict_file_id(
                "pages/4aa43308-d0d3-57ee-be0c-d9383f429f50.jsonl"
            )),
            "c801da2a-0f8c-5452-8203-791712d85dcd"
        );
    }

    /// The schemes that were minted by hand outside this crate, against the identifiers the hand-written code
    /// gave for these inputs (captured before the move): moving them here changed no byte.
    #[test]
    fn moved_schemes_keep_their_identifiers() {
        let s = |u: Uuid| u.to_string();
        let system = Uuid::from_u128(0x7202fd8b_1024_4bf1_9e99_670772eaa589);
        let (library, copy) = (uuid(LIBRARY), uuid(ITEM));
        let (instance, local, device, rep) = (uuid(REVISION), uuid(FUNCTION), uuid(DEVICE), uuid(MODULE));
        assert_eq!(
            s(library_item_id(system, "ew.fn.coil")),
            "aebc618c-20bb-54eb-bda5-a82355802f37"
        );
        assert_eq!(
            s(library_item_id(system, "ew.fn.ip")),
            "71f84e74-578d-5835-9a77-cfdaf8057ecd"
        );
        assert_eq!(
            s(kind_item_id(library, "macro", "Motor starter")),
            "b571fb07-4d27-5a9b-867f-ace2a65f7e09"
        );
        assert_eq!(
            s(kind_item_id(library, "symbol", "my.sym.x1")),
            "4e4c9530-5c93-5c2d-8020-ea9815fb787e"
        );
        assert_eq!(
            s(manufacturer_item_id(library, "Siemens AG")),
            "a6e479ee-8c67-5878-8058-60cfa6ff117b"
        );
        assert_eq!(
            s(fork_item_id(copy, None)),
            "bbbb9b53-aea7-5043-ba46-d4e6fecf55d0"
        );
        assert_eq!(
            s(fork_item_id(copy, Some(2))),
            "a1c4ee27-c31d-592d-8401-68f3da6539ea"
        );
        assert_eq!(
            s(macro_instance_id(copy, Some(2))),
            "0f4f9bff-2584-5c50-a720-b156d4d9ffb5"
        );
        assert_eq!(
            s(macro_instance_id(copy, None)),
            "87c22f7e-c47b-5028-917f-85bdc9c8936c"
        );
        assert_eq!(
            s(macro_thumbnail_instance_id(copy)),
            "32b55d0f-4cc1-541b-a87b-e39e199de499"
        );
        assert_eq!(
            s(instance_member_id(instance, local)),
            "5a85ea0d-3b58-5cfb-b2d5-e782df5f8d9a"
        );
        assert_eq!(
            s(drives_link_id(local, device)),
            "56fa696c-e19a-57c1-90f1-f9f9864872e4"
        );
        assert_eq!(
            s(valve_glyph_entity(rep, device)),
            "80c70eac-a07f-5b3f-84b2-89073027d45a"
        );
        assert_eq!(
            s(valve_position_entity(rep, 3)),
            "ddb4ac79-b485-5ecf-821f-e6c17535e103"
        );
        assert_eq!(
            s(lib_copy_id::<K>(system, copy).as_uuid()),
            "890a0a06-cd31-5a6e-a401-c85336509394"
        );
    }

    #[test]
    fn names_follow_the_documented_layout() {
        let f = id(FUNCTION);
        assert_eq!(cp_id::<_, K>(f, 12).as_uuid(), Uuid::new_v5(&f.as_uuid(), b"12"));
        let m = id(MODULE);
        assert_eq!(
            plc_channel_id::<_, K>(m, 65_535).as_uuid(),
            Uuid::new_v5(&m.as_uuid(), b"channel\x1f65535")
        );
        let name = format!("{LIBRARY}\u{1F}{ITEM}");
        assert_eq!(
            lib_copy_id::<K>(uuid(LIBRARY), uuid(ITEM)).as_uuid(),
            Uuid::new_v5(&NS_LIBCOPY, name.as_bytes())
        );
    }

    #[test]
    fn deterministic_and_version_5() {
        let f = id(FUNCTION);
        let a: Id<K> = cp_id(f, 4);
        assert_eq!(a, cp_id(f, 4));
        assert_eq!(a.as_uuid().get_version_num(), 5);
        assert_eq!(a.as_uuid().get_variant(), uuid::Variant::RFC4122);
    }

    #[test]
    fn neighbours_and_kinds_are_distinct() {
        let f = id(FUNCTION);
        assert_ne!(cp_id::<_, K>(f, 1), cp_id::<_, K>(f, 2));
        assert_ne!(
            report_page_id::<_, K>(id(BLOCK), 1),
            report_page_id::<_, K>(id(BLOCK), 2)
        );
        assert_ne!(
            plc_channel_id::<_, K>(id(MODULE), 1),
            plc_channel_id::<_, K>(id(MODULE), 2)
        );
        assert_ne!(
            template_function_id::<_, K>(id(DEVICE), 1),
            template_function_id::<_, K>(id(DEVICE), 2)
        );
        // Same parent, different formula: "3" vs "channel␟3".
        assert_ne!(
            cp_id::<_, K>(id(MODULE), 3),
            plc_channel_id::<_, K>(id(MODULE), 3)
        );
        // Different parents, same index.
        assert_ne!(cp_id::<_, K>(id(FUNCTION), 0), cp_id::<_, K>(id(DEVICE), 0));
        // The next revision and the withdrawal record of one revision.
        assert_ne!(
            revision_next_id(id(REVISION)),
            revision_withdrawal_id(id(REVISION))
        );
        // Keep-both differs from the plain copy and between versions.
        let (l, i) = (uuid(LIBRARY), uuid(ITEM));
        assert_ne!(lib_copy_id::<K>(l, i), lib_copy_keep_both_id::<K>(l, i, "2.1.0"));
        assert_ne!(
            lib_copy_keep_both_id::<K>(l, i, "2.1.0"),
            lib_copy_keep_both_id::<K>(l, i, "2.1.1")
        );
        assert_ne!(lib_copy_id::<K>(l, i), lib_copy_id::<K>(i, l));
        assert_ne!(
            imported_id::<K>(uuid(LINEAGE), "a"),
            imported_id::<K>(uuid(LINEAGE), "b")
        );
    }

    /// SHA-256 of the empty input.
    const EMPTY_SHA256: [u8; 32] = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24, 0x27,
        0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
    ];
    const EMPTY_SHA256_HEX: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    /// Doc 05 §3.2a's worked vector, computed independently with Python's `uuid.uuid5`.
    #[test]
    fn lineage_namespace_fixed_vector() {
        assert_eq!(NS_IMPORT.to_string(), "7b75186c-f28e-49ce-b7f7-26b137095a65");
        let ns = lineage_namespace("dxf", &LineageAnchor::FirstImportSha256(EMPTY_SHA256)).unwrap();
        assert_eq!(ns.to_string(), "a1ba512b-c515-50c5-af0b-26fa3b5599f5");
    }

    #[test]
    fn lineage_names_follow_the_documented_layout() {
        let u = uuid(LINEAGE);
        assert_eq!(
            lineage_namespace("qet", &LineageAnchor::QetProjectUuid(u)).unwrap(),
            Uuid::new_v5(&NS_IMPORT, format!("qet\u{1F}project\u{1F}{LINEAGE}").as_bytes())
        );
        assert_eq!(
            lineage_namespace("dxf", &LineageAnchor::FirstImportSha256(EMPTY_SHA256)).unwrap(),
            Uuid::new_v5(
                &NS_IMPORT,
                format!("dxf\u{1F}sha256\u{1F}{EMPTY_SHA256_HEX}").as_bytes()
            )
        );
        assert_eq!(
            lineage_namespace("qet", &LineageAnchor::Copy { of: u, n: 2 }).unwrap(),
            Uuid::new_v5(
                &NS_IMPORT,
                format!("qet\u{1F}copy\u{1F}{LINEAGE}\u{1F}2").as_bytes()
            )
        );
        assert_eq!(
            lineage_namespace("csv", &LineageAnchor::Fixed("golden")).unwrap(),
            Uuid::new_v5(&NS_IMPORT, b"csv\x1ffixed\x1fgolden")
        );
    }

    #[test]
    fn lineage_importer_is_validated() {
        let a = LineageAnchor::Fixed("golden");
        for bad in ["", "QET", "dxf importer", "qet\u{1F}x", "d\u{e9}", "a_b"] {
            assert_eq!(
                lineage_namespace(bad, &a),
                Err(LineageNameError::InvalidImporter),
                "{bad:?}"
            );
        }
        assert!(lineage_namespace("qet-2", &a).is_ok());
        assert!(!LineageNameError::InvalidImporter.to_string().is_empty());
    }

    #[test]
    fn lineage_copy_differs_from_original() {
        let original = lineage_namespace("qet", &LineageAnchor::QetProjectUuid(uuid(LINEAGE))).unwrap();
        let copy1 = lineage_namespace("qet", &LineageAnchor::Copy { of: original, n: 1 }).unwrap();
        let copy2 = lineage_namespace("qet", &LineageAnchor::Copy { of: original, n: 2 }).unwrap();
        assert_ne!(copy1, original);
        assert_ne!(copy1, copy2);
        assert_eq!(copy1.get_version_num(), 5);
    }
}
