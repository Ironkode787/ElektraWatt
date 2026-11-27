// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The symbol a page draws a piece of a part with (doc 07 §2.5, §5.6): the piece's own symbol (`TemplateFn.symbol`)
//! where it is among the symbols the page can draw the piece's function with, else the first of them. Placing a part,
//! its picture in Parts and the Symbol cell of its pieces all ask [`placed_symbol`].

use ew_ids::Uuid;
use ew_schema::TemplateFn;

/// The copy ID the piece's own symbol has in a project (`v5(NS_LIBCOPY, library ‖ item)`); `None` without one.
pub fn piece_symbol_copy(entry: &TemplateFn) -> Option<Uuid> {
    let s = entry.symbol.as_ref()?;
    Some(ew_ids::v5::lib_copy_id::<()>(s.library.as_uuid(), s.item.as_uuid()).as_uuid())
}

/// Which of `candidates` draws template entry `entry`: the symbols of its function that the page can draw, in the
/// order the page offers them, each by its copy ID; the entry's own symbol where it is among them, else the first.
pub fn placed_symbol<T>(entry: &TemplateFn, candidates: impl IntoIterator<Item = (Uuid, T)>) -> Option<T> {
    let own = piece_symbol_copy(entry);
    let mut first = None;
    for (copy, c) in candidates {
        if Some(copy) == own {
            return Some(c);
        }
        if first.is_none() {
            first = Some(c);
        }
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(symbol: Option<(u128, u128)>) -> TemplateFn {
        let r = |(l, i): (u128, u128)| ew_schema::LibRef {
            library: ew_ids::Id::from_uuid(Uuid::from_u128(l)),
            item: ew_ids::Id::from_uuid(Uuid::from_u128(i)),
            version_req: "^0.1".into(),
            extra: ew_schema::Extra::new(),
        };
        TemplateFn {
            fndef: r((1, 2)),
            main: true,
            cps: Vec::new(),
            symbol: symbol.map(r),
            extra: ew_schema::Extra::new(),
        }
    }

    /// A piece's own symbol draws it where the page has it; a piece without one, or whose symbol the page cannot draw,
    /// takes the first symbol of its function.
    #[test]
    fn a_piece_takes_its_own_symbol_where_the_page_has_it_else_the_first() {
        let own = entry(Some((7, 8)));
        let own_copy = piece_symbol_copy(&own).expect("a copy ID");
        let first = Uuid::from_u128(100);
        let candidates = || [(first, "first"), (own_copy, "own")];
        assert_eq!(placed_symbol(&own, candidates()), Some("own"));
        assert_eq!(placed_symbol(&entry(None), candidates()), Some("first"));
        assert_eq!(
            placed_symbol(&own, [(first, "first")]),
            Some("first"),
            "a page that has not its symbol"
        );
        assert_eq!(placed_symbol::<&str>(&own, []), None);
    }
}
