// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The names of the starter content that code refers to: the system library `ew-starter` that ships under `content/`
//! (its symbols, function definitions and forms), the version requirement of a reference to one of its items, and
//! every key a program names. ew-library's content audit finds each of them in the shipped content.

use ew_ids::Uuid;

/// The ID of the system library `ew-starter`: the `library_id` of every starter document under `content/symbols`,
/// `content/symbols/qet10`, `content/fndefs` and `content/forms`. A starter item's ID is `v5(LIBRARY, meta.key)`
/// ([`ew_ids::v5::library_item_id`]) and its copy ID `ew_ids::v5::lib_copy_id(LIBRARY, item)` (doc 05 §13.2).
pub const LIBRARY: Uuid = Uuid::from_u128(0x7202fd8b_1024_4bf1_9e99_670772eaa589);

/// The `version_req` of a reference to a starter function definition, which are version `0.1`.
pub const VERSION_REQ: &str = "^0.1";

/// Declares each key as a constant and [`KEYS`] as the list of them all.
macro_rules! keys {
    ($($(#[doc = $doc:literal])* $name:ident = $key:literal;)*) => {
        $($(#[doc = $doc])* pub const $name: &str = $key;)*

        /// Every key of this module, which the content audit finds in the shipped content.
        pub const KEYS: &[&str] = &[$($name),*];
    };
}

keys! {
    /// The function definition of a terminal.
    FN_TERMINAL = "ew.fn.terminal";
    /// The function definition of a PE terminal.
    FN_TERMINAL_PE = "ew.fn.terminal-pe";
    /// EW's generic black box: the function of a part's pins that no other definition maps.
    FN_BLACKBOX = "ew.fn.blackbox";
    /// The interruption point's function definition, which QElectroTech's `next_report` and `previous_report`
    /// elements reference instead of a generated one (doc 07 §2.3, doc 14 §4.3). `ew.gen.ip` is the key of the
    /// interruption point's *symbol*, never of a function definition.
    FN_IP = "ew.fn.ip";
    /// The black box's symbol.
    GEN_BLACKBOX = "ew.gen.blackbox";
    /// The black box's symbol, QElectroTech edition.
    GEN_BLACKBOX_QET10 = "ew.gen.blackbox@qet10";
    /// The wide module of an overview.
    OVW_MODULE_WIDE = "ew.ovw.module-wide";
    /// The wide module of an overview, QElectroTech edition.
    OVW_MODULE_WIDE_QET10 = "ew.ovw.module-wide@qet10";
    /// The wire marker's symbol, which the marker tool places.
    GEN_WIRE_MARKER = "ew.gen.wire-marker";
    /// The interruption point's source form: the outgoing arrow.
    GEN_IP = "ew.gen.ip";
    /// The interruption point's target form: the incoming arrow.
    GEN_IP_TARGET = "ew.gen.ip-target";
    /// [`GEN_IP`], QElectroTech edition.
    GEN_IP_QET10 = "ew.gen.ip@qet10";
    /// [`GEN_IP_TARGET`], QElectroTech edition.
    GEN_IP_TARGET_QET10 = "ew.gen.ip-target@qet10";
    /// The fluid line's interruption point, source form (FR-FLU-030).
    ISO_IP = "ew.iso.ip";
    /// The fluid line's interruption point, target form.
    ISO_IP_TARGET = "ew.iso.ip-target";
    /// [`ISO_IP`], QElectroTech edition.
    ISO_IP_QET10 = "ew.iso.ip@qet10";
    /// [`ISO_IP_TARGET`], QElectroTech edition.
    ISO_IP_TARGET_QET10 = "ew.iso.ip-target@qet10";
    /// The frame form of dividers, and of pages without a form.
    NOGRID_FRAME = "ew.frame.qet-a4-landscape-nogrid";
    /// The shipped frame with the column and row grid.
    GRID_FRAME = "ew.frame.qet-a4-landscape-grid";
    /// The shipped cover frame.
    COVER_FRAME = "ew.frame.qet-a4-landscape-cover";
    /// The shipped title block, which the shipped A4 frames name and which reproduces QElectroTech's default
    /// template.
    TITLE_BLOCK = "ew.titleblock.qet-a4";
    /// The shipped A3 frame with the grid (FT-1826), which names the A3 title block.
    A3_GRID_FRAME = "ew.frame.qet-a3-landscape-grid";
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L1-5: the library ID is the decided value. That every starter document carries it, and that every key names
    /// one, is checked by ew-library's content audit, outside this permissive crate.
    #[test]
    fn the_library_is_the_decided_value() {
        assert_eq!(LIBRARY.to_string(), "7202fd8b-1024-4bf1-9e99-670772eaa589");
    }
}
