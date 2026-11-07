// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The closed sets of choices the window shows through `strings::choice_text` (FT-1521, WP-S2.11 §12's design note,
//! decision log ID-1434, ID-1465): a form field's `choices` is a `&'static` [`ClosedChoices`], which only this module
//! can build, and the one `closed_choices!` invocation below defines every set and [`ALL`] from the same lines, so a
//! set a form names is in [`ALL`] by construction and the choice-word test reads it. A run-time list (the project's
//! locations, the enclosure parts) is an open set in a field's `options`, outside the type.

/// One closed set of choices, in order. Its value list is private to this module, so a list built elsewhere does not
/// compile:
///
/// ```compile_fail
/// static OTHER: ew_viewmodel::shell::choices::ClosedChoices = ew_viewmodel::shell::choices::ClosedChoices(&["x"]);
/// ```
#[derive(PartialEq, Eq, Debug)]
pub struct ClosedChoices(&'static [&'static str]);

impl ClosedChoices {
    /// The values, in order.
    pub fn values(&self) -> &'static [&'static str] {
        self.0
    }
}

impl std::ops::Deref for ClosedChoices {
    type Target = [&'static str];

    fn deref(&self) -> &[&'static str] {
        self.0
    }
}

impl<'a> IntoIterator for &'a ClosedChoices {
    type Item = &'a &'static str;
    type IntoIter = std::slice::Iter<'a, &'static str>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// A free-text field's choices: none. Not a closed set, so not in [`ALL`].
pub static NONE: ClosedChoices = ClosedChoices(&[]);

/// Defines each closed set as a `pub static` and [`ALL`] from the same lines.
macro_rules! closed_choices {
    ($($(#[$doc:meta])* $name:ident = $values:expr;)*) => {
        $($(#[$doc])* pub static $name: ClosedChoices = ClosedChoices($values);)*

        /// Every closed set of this module (ID-1278 ruling 2: the choice-word test reads it).
        pub static ALL: &[&ClosedChoices] = &[$(&$name),*];
    };
}

closed_choices! {
    /// New page's page types.
    PAGE_TYPES = super::creation::PAGE_TYPES;
    /// New structure node's aspects.
    STRUCTURE_KINDS = super::creation::STRUCTURE_KINDS;
    /// A yes-or-no field (drawn as a check box).
    BOOL = super::bulk::BOOL_CHOICES;
    /// Renumber devices…' scopes.
    DEVICE_SCOPES = super::bulk::DEVICE_SCOPES;
    /// Renumber devices…' schemes.
    SCHEMES = super::bulk::SCHEMES;
    /// Save as macro's kinds.
    MACRO_KINDS = &super::macros::MACRO_KINDS;
    /// The revision forms' roles.
    ROLES = super::revision::ROLES;
    /// The part editor's kinds (ew-api's list; not a form field).
    PART_KINDS = &super::part_dialog::PART_KINDS;
    /// A fluid line's kinds (WP-S1.20 T25; not a form field).
    LINE_KINDS = crate::strings::LINE_KINDS;
}
