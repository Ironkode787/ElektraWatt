// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Entity tags and cross-reference links.

use uuid::Uuid;

/// The id of the model record an item represents (a representation, a connection, a page item …), for
/// picking and highlighting. Never a session-dense index, so a display list means the same thing in every
/// session.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct EntityRef(pub Uuid);

/// A named destination of the exported PDF, derived from a stable identity (ADR-0004, FR-OUT-007), so links
/// survive regeneration.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Destination {
    /// A page; generated report pages use their v5 id `v5(block, index)`.
    Page(Uuid),
    /// A representation (a symbol placement on a page).
    Representation(Uuid),
    /// A connection point, by its v5 id `v5(function, idx)`.
    Cp(Uuid),
}

impl Destination {
    /// The PDF destination name of doc 09 §4.2: `ew:p:<uuid>`, `ew:r:<uuid>` or `ew:cp:<uuid>`, with the
    /// UUID in lower-case hyphenated form.
    ///
    /// ```
    /// use ew_dl::Destination;
    /// use uuid::Uuid;
    ///
    /// let id = Uuid::from_u128(0xc7a90e35_42d8_5f61_8b03_9d2e6f1a47c2);
    /// assert_eq!(Destination::Cp(id).pdf_name(), "ew:cp:c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2");
    /// ```
    pub fn pdf_name(&self) -> String {
        let (prefix, id) = match self {
            Destination::Page(id) => ("p", id),
            Destination::Representation(id) => ("r", id),
            Destination::Cp(id) => ("cp", id),
        };
        format!("ew:{prefix}:{}", id.hyphenated())
    }
}

/// The link role of an item: the source of a cross-reference (a link annotation over the item's box,
/// FR-OUT-005) or its target (a named destination at the item's anchor, FR-OUT-007).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Link {
    /// The item links to the destination.
    Source(Destination),
    /// The item is where the destination points.
    Target(Destination),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_names() {
        let id = Uuid::from_u128(0x0192_6f3a_7b1c_7d2e_8f40_1234_5678_9abc);
        assert_eq!(
            Destination::Page(id).pdf_name(),
            "ew:p:01926f3a-7b1c-7d2e-8f40-123456789abc"
        );
        assert_eq!(
            Destination::Representation(id).pdf_name(),
            "ew:r:01926f3a-7b1c-7d2e-8f40-123456789abc"
        );
        assert_eq!(
            Destination::Cp(id).pdf_name(),
            "ew:cp:01926f3a-7b1c-7d2e-8f40-123456789abc"
        );
        let upper = Uuid::from_u128(u128::MAX);
        assert_eq!(
            Destination::Page(upper).pdf_name(),
            "ew:p:ffffffff-ffff-ffff-ffff-ffffffffffff"
        );
        assert_eq!(
            Destination::Page(Uuid::nil()).pdf_name(),
            "ew:p:00000000-0000-0000-0000-000000000000"
        );
    }
}
