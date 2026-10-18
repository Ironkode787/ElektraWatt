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
//!   connection or import lineage), or the fixed [`NS_LIBCOPY`] for embedded library copies.
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
//! | [`template_function_id`] | `v5(device, template_index)` | `"3"` |
//! | [`plc_channel_id`] | `v5(module, "channel", number)` | `"channel\u{1F}3"` |
//! | [`lib_copy_id`] | `v5(NS_LIBCOPY, library ‖ item)` | `"<library>\u{1F}<item>"` |
//! | [`lib_copy_keep_both_id`] | `v5(NS_LIBCOPY, library ‖ item ‖ version)` | `"<library>\u{1F}<item>\u{1F}<version>"` |
//! | [`route_id`] | `v5(connection, "route")` | `"route"` |
//!
//! U+001F cannot occur in a UUID or a decimal integer, so the joined name is unambiguous.
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
/// `namespace` is the import lineage's anchor (the QET project UUID, or the namespace derived from the
/// lineage's first imported file); `source_key` is the importer's key for the source item, used byte for
/// byte as given.
pub fn imported_id<T>(namespace: Uuid, source_key: &str) -> Id<T> {
    v5(namespace, source_key)
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

/// The ID of the committed route of `connection`: `v5(connection, "route")` (doc 05 §2.2).
pub fn route_id<C, R>(connection: Id<C>) -> Id<R> {
    v5(connection.as_uuid(), "route")
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
    const CONNECTION: &str = "0192f3a4-5b6c-7d8e-9f01-23456789abd1";
    const LIBRARY: &str = "0192f3a4-5b6c-7d8e-af01-23456789abd2";
    const ITEM: &str = "0192f3a4-5b6c-7d8e-bf01-23456789abd3";
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
            s(route_id(id(CONNECTION))),
            "aee2006f-142b-5569-a476-c57094452ca9"
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
}
