// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `terminal`: a connection point.
//!
//! | Attribute | Mapping |
//! |---|---|
//! | `x`, `y` (required) | [`Terminal::pos`], relative to the hotspot |
//! | `orientation` (required) | `n` → [`Dir4::Up`], `e` → [`Dir4::Right`], `s` → [`Dir4::Down`], `w` → [`Dir4::Left`]; anything else is an [`ItemError`] |
//! | `name` (optional, may be empty) | [`Terminal::name`] verbatim; absent = empty |
//! | `type` (optional) | [`Terminal::kind`]: `Generic` (default), `Inner`, `Outer`; another value is kept verbatim and reported (EW-IMP-004, info) |
//! | `uuid` (optional) | [`Terminal::uuid`] verbatim |
//!
//! Terminals are appended to [`Parsed::terminals`] in document order; the definition order is the CP index.

use ew_schema::Dir4;
use roxmltree::Node;

use super::{ItemError, Parsed, PrimCtx, Terminal};

/// `terminal`.
///
/// # Errors
///
/// [`ItemError`] for a missing or unparsable coordinate or orientation.
pub fn terminal(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>, out: &mut Parsed) -> Result<(), ItemError> {
    let pos = ctx.point(node, "x", "y")?;
    let dir = match node.attribute("orientation") {
        Some("n") => Dir4::Up,
        Some("e") => Dir4::Right,
        Some("s") => Dir4::Down,
        Some("w") => Dir4::Left,
        Some(other) => {
            return Err(ItemError(format!(
                "terminal orientation {other:?} not understood"
            )));
        }
        None => return Err(ItemError("missing attribute orientation".to_owned())),
    };
    let kind = node.attribute("type").unwrap_or("Generic");
    if !matches!(kind, "Generic" | "Inner" | "Outer") {
        ctx.unsupported(node, format!("terminal type {kind} not supported"));
    }
    out.terminals.push(Terminal {
        pos,
        dir,
        name: node.attribute("name").unwrap_or("").to_owned(),
        kind: kind.to_owned(),
        uuid: node.attribute("uuid").map(str::to_owned),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use ew_interop_api::Code;
    use ew_units::{Nm, Pt2};

    use super::*;
    use crate::decimal::Decimal;

    fn run(xml: &str) -> (Result<(), ItemError>, Parsed, PrimCtx<'static>) {
        let doc = roxmltree::Document::parse(xml).unwrap();
        let mut ctx = PrimCtx::new("t.elmt", None, Decimal::ZERO, Decimal::ZERO);
        let mut out = Parsed::default();
        let r = terminal(doc.root_element(), &mut ctx, &mut out);
        (r, out, ctx)
    }

    #[test]
    fn four_orientations() {
        for (o, d) in [
            ("n", Dir4::Up),
            ("e", Dir4::Right),
            ("s", Dir4::Down),
            ("w", Dir4::Left),
        ] {
            let (r, out, ctx) = run(&format!(r#"<terminal x="0" y="-20" orientation="{o}"/>"#));
            r.unwrap();
            assert!(ctx.entries.is_empty());
            assert_eq!(out.terminals.len(), 1);
            let t = &out.terminals[0];
            assert_eq!(t.dir, d);
            assert_eq!(t.pos, Pt2::new(Nm(0), ew_units::Grid::QET_10PX.at(-2).unwrap()));
        }
    }

    #[test]
    fn bad_or_missing_orientation_is_item_error() {
        let (r, out, _) = run(r#"<terminal x="0" y="0" orientation="x"/>"#);
        assert!(r.is_err());
        assert!(out.terminals.is_empty());
        let (r, ..) = run(r#"<terminal x="0" y="0"/>"#);
        assert!(r.is_err());
        let (r, ..) = run(r#"<terminal y="0" orientation="n"/>"#);
        assert!(r.is_err());
    }

    #[test]
    fn name_empty_or_absent_and_defaults() {
        let (r, out, _) = run(r#"<terminal x="0" y="0" orientation="n" name=""/>"#);
        r.unwrap();
        assert_eq!(out.terminals[0].name, "");
        assert_eq!(out.terminals[0].kind, "Generic");
        assert_eq!(out.terminals[0].uuid, None);
        let (r, out, _) = run(r#"<terminal x="0" y="0" orientation="n" name="A1"/>"#);
        r.unwrap();
        assert_eq!(out.terminals[0].name, "A1");
    }

    #[test]
    fn type_outer_kept_unknown_reported() {
        let (r, out, ctx) = run(r#"<terminal x="0" y="0" orientation="n" type="Outer"/>"#);
        r.unwrap();
        assert_eq!(out.terminals[0].kind, "Outer");
        assert!(ctx.entries.is_empty());
        let (r, out, ctx) = run(r#"<terminal x="0" y="0" orientation="n" type="Weird"/>"#);
        r.unwrap();
        assert_eq!(out.terminals[0].kind, "Weird");
        assert_eq!(ctx.entries.len(), 1);
        assert_eq!(ctx.entries[0].code, Code::DROPPED_BY_DESIGN);
        assert_eq!(ctx.entries[0].message, "terminal type Weird not supported");
    }

    #[test]
    fn uuid_kept_verbatim() {
        let (r, out, _) = run(r#"<terminal x="0" y="0" orientation="s" uuid="{1b2c-3d}"/>"#);
        r.unwrap();
        assert_eq!(out.terminals[0].uuid.as_deref(), Some("{1b2c-3d}"));
    }
}
