// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What only the window shows and edits through the session: the clipboard, find and replace, the history panel's
//! rows, the edit-cable table's core swap and the PLC navigator's drop. The command line does not use it, and
//! [`crate::output`] never names it.

// The edit-cable table's gestures (§7).
mod cable_edit;
pub(crate) mod clipboard;
mod find;
// The history panel's rows (§4.2).
mod history;
// What a PLC channel dropped on a field device can do.
mod plc_bind;

pub use clipboard::{
    CopiedFragment, CopyError, FRAGMENT_HEADER, FragmentError, ParsedFragment, parse_fragment,
    parse_fragment_with,
};
pub use find::{FindError, FindField, FindHit, FindQuery, FindResults};
pub use history::{HistoryRow, HistoryView};
pub use plc_bind::OffCardBind;
