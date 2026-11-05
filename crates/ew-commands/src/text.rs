// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A command's refusal by key (FT-1337, decision log ID-1359): the key of the window's text files and its arguments,
//! beside the English text that the CLI and `ew-api` print (FT-680). The window renders the key in its language, an
//! argument that is itself a [`Text`] (a record in words, [`crate::words`]) in its language too.
//!
//! [`TEXTS`] holds each key's English text, its arguments written `{0}`, `{1}`, … as the window's
//! `strings::text` shows them; `ew-viewmodel` checks that each equals the en-GB file's message. Code names a key with
//! [`text!`], which checks at compile time that the key is in [`TEXTS`] and that it is given as many arguments as its
//! text takes.

use std::fmt;

/// A text by key, with its arguments: [`fmt::Display`] writes it in English.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Text {
    key: &'static str,
    args: Vec<TextArg>,
}

/// An argument of a [`Text`]: a value written as it is (a tag, a number, a name), or a text by key.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TextArg {
    /// A value that no language changes.
    Plain(String),
    /// Words by key, in the reader's language.
    Text(Text),
}

impl Text {
    /// The constructor behind the crate's `text!` macro, which checks the key and the number of arguments; call the
    /// macro instead.
    #[doc(hidden)]
    pub fn checked_by_macro(key: &'static str, args: Vec<TextArg>) -> Text {
        Text { key, args }
    }

    /// The key: a key of [`TEXTS`] and of the window's text files.
    pub fn key(&self) -> &'static str {
        self.key
    }

    /// The arguments, in the order of `{0}`, `{1}`, ….
    pub fn args(&self) -> &[TextArg] {
        &self.args
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut rest = english(self.key).unwrap_or("?");
        while let Some(open) = rest.find('{') {
            f.write_str(&rest[..open])?;
            let after = &rest[open + 1..];
            let digits = after.bytes().take_while(u8::is_ascii_digit).count();
            match after[digits..].strip_prefix('}') {
                Some(tail) if digits > 0 => {
                    let i: usize = after[..digits].parse().unwrap_or(usize::MAX);
                    match self.args.get(i) {
                        Some(arg) => write!(f, "{arg}")?,
                        None => f.write_str("?")?,
                    }
                    rest = tail;
                }
                _ => {
                    f.write_str("{")?;
                    rest = after;
                }
            }
        }
        f.write_str(rest)
    }
}

impl fmt::Display for TextArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextArg::Plain(s) => f.write_str(s),
            TextArg::Text(t) => write!(f, "{t}"),
        }
    }
}

impl From<String> for TextArg {
    fn from(s: String) -> TextArg {
        TextArg::Plain(s)
    }
}

impl From<&String> for TextArg {
    fn from(s: &String) -> TextArg {
        TextArg::Plain(s.clone())
    }
}

impl From<&str> for TextArg {
    fn from(s: &str) -> TextArg {
        TextArg::Plain(s.to_owned())
    }
}

impl From<Text> for TextArg {
    fn from(t: Text) -> TextArg {
        TextArg::Text(t)
    }
}

/// Whether two strings are equal, in a const context.
const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The English text of `key`, or `None` for a key that [`TEXTS`] does not hold.
pub const fn english(key: &str) -> Option<&'static str> {
    let mut i = 0;
    while i < TEXTS.len() {
        if same(TEXTS[i].0, key) {
            return Some(TEXTS[i].1);
        }
        i += 1;
    }
    None
}

/// The number of arguments `template` takes: one more than its highest `{n}`.
pub const fn arity(template: &str) -> usize {
    let b = template.as_bytes();
    let mut most = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'{' {
            let mut j = i + 1;
            let mut n = 0;
            while j < b.len() && b[j].is_ascii_digit() {
                n = n * 10 + (b[j] - b'0') as usize;
                j += 1;
            }
            if j > i + 1 && j < b.len() && b[j] == b'}' && n + 1 > most {
                most = n + 1;
            }
        }
        i += 1;
    }
    most
}

/// The [`Text`] of the key `$k`, a string literal, with the arguments that follow it (each a `String`, a `&str` or a
/// [`Text`]): a key that [`TEXTS`] lacks, or a wrong number of arguments, fails the build.
macro_rules! text {
    ($k:literal $(, $a:expr)* $(,)?) => {
        $crate::text::Text::checked_by_macro(
            const {
                let Some(template) = $crate::text::english($k) else {
                    panic!(concat!("unknown text key: ", $k));
                };
                assert!(
                    $crate::text::arity(template) == <[&str]>::len(&[$(stringify!($a)),*]),
                    concat!("the wrong number of arguments for ", $k)
                );
                $k
            },
            vec![$($crate::text::TextArg::from($a)),*],
        )
    };
}
pub(crate) use text;

// map: refusal-texts: a new command refusal goes in `TEXTS` under its command, and with the same key in both `shell.ftl` files beside the other refusals of its command.
/// Every key of a command's refusal with its English text, grouped by command; each is a message of both `shell.ftl`
/// files of `ew-viewmodel`, with the same text in en-GB.
pub const TEXTS: &[(&str, &str)] = &[
    // The records in words (`words.rs`) and the geometry's sentences (`geometry.rs`).
    ("refusal-words-device-gone", "a device that is not in the project"),
    ("refusal-words-function-no-device", "a function without a device"),
    ("refusal-words-cp", "{0}:{1}"),
    ("refusal-words-channel", "channel {0} of {1}"),
    (
        "refusal-words-channel-gone",
        "a channel that is not in the project",
    ),
    ("refusal-words-page", "page {0}"),
    ("refusal-words-page-gone", "a page that is not in the project"),
    ("refusal-words-symbol", "the symbol of {0}"),
    ("refusal-words-symbol-any", "a symbol"),
    ("refusal-words-point", "x {0}, y {1} mm on {2}"),
    ("refusal-words-junction", "the junction at {0}"),
    (
        "refusal-words-junction-gone",
        "a junction that is not in the project",
    ),
    ("refusal-words-ip", "interruption point {0}"),
    ("refusal-words-position", "x {0}, y {1} mm"),
    ("refusal-words-wire", "wire {0}"),
    ("refusal-words-wire-on", "a wire on {0}"),
    ("refusal-words-wire-any", "a wire"),
    ("refusal-words-cp-index", "{0} CP {1}"),
    ("refusal-words-kind-project", "a project"),
    (
        "refusal-words-kind-plc-config-project",
        "a PLC configuration project",
    ),
    ("refusal-words-kind-logical-net", "a logical net"),
    ("refusal-words-kind-physical-net", "a physical net"),
    ("refusal-words-kind-medium", "a medium"),
    ("refusal-words-kind-import-lineage", "an import lineage"),
    ("refusal-words-kind-structure-node", "a structure node"),
    ("refusal-words-kind-section", "a section"),
    ("refusal-words-kind-external-ref", "an external reference"),
    ("refusal-words-kind-conductor", "a conductor"),
    ("refusal-words-kind-channel", "a channel"),
    ("refusal-words-kind-potential-def", "a potential definition"),
    ("refusal-words-kind-net-def", "a net definition"),
    ("refusal-words-kind-medium-def", "a medium definition"),
    ("refusal-words-kind-bond", "a bond"),
    ("refusal-words-kind-device-link", "a device link"),
    ("refusal-words-kind-page-item", "a page item"),
    ("refusal-words-kind-structure-box", "a structure box"),
    ("refusal-words-kind-report-block", "a report block"),
    ("refusal-words-kind-macro-instance", "a macro instance"),
    ("refusal-words-kind-placeholder-group", "a placeholder group"),
    ("refusal-words-kind-layout-space", "a layout space"),
    ("refusal-words-kind-enclosure", "an enclosure"),
    ("refusal-words-kind-mounting-surface", "a mounting surface"),
    ("refusal-words-kind-carrier", "a carrier"),
    ("refusal-words-kind-placement", "a placement"),
    ("refusal-words-kind-route-override", "a route override"),
    ("refusal-words-kind-committed-route", "a committed route"),
    ("refusal-words-kind-routing-path", "a routing path"),
    ("refusal-words-kind-surface-feature", "a surface feature"),
    ("refusal-words-kind-revision", "a revision"),
    ("refusal-words-kind-waiver", "a waiver"),
    ("refusal-words-kind-comment", "a comment"),
    ("refusal-words-kind-transmittal", "a transmittal"),
    ("refusal-words-kind-exchange-record", "an exchange record"),
    ("refusal-words-kind-foreign-id", "a foreign ID"),
    ("refusal-words-kind-import-run", "an import run"),
    ("refusal-words-kind-conflict", "a conflict"),
    ("refusal-words-kind-part", "a part"),
    ("refusal-words-kind-symbol", "a symbol"),
    ("refusal-words-kind-fndef", "a function definition"),
    ("refusal-words-kind-form", "a form"),
    ("refusal-words-kind-macro", "a macro"),
    ("refusal-words-kind-manufacturer", "a manufacturer"),
    ("refusal-words-kind-plc-profile", "a PLC profile"),
    ("refusal-words-kind-machine-profile", "a machine profile"),
    ("refusal-words-kind-label-profile", "a label profile"),
    ("refusal-words-kind-other", "an item"),
    ("refusal-words-copy", "{0} at version {1}"),
    (
        "refusal-geometry-off-grid",
        "{0} at {1} is not on the drawing grid",
    ),
    ("refusal-geometry-branch-point", "the branch point"),
    ("refusal-geometry-outline-corner", "the outline's corner"),
    ("refusal-geometry-box-cp", "the box's connection point"),
    ("refusal-geometry-position", "the position"),
    (
        "refusal-geometry-out-of-range",
        "{0} at {1} lies outside the drawable range; move it nearer the page origin",
    ),
    ("refusal-geometry-onto-grid", "{0}; move it onto the grid"),
    ("refusal-geometry-wire-start", "the wire's start"),
    ("refusal-geometry-wire-end", "the wire's end"),
    ("refusal-geometry-wire-corner", "the wire's corner"),
    (
        "refusal-geometry-route-out-of-range",
        "{0} at {1} lies outside the drawable range; draw the wire nearer the page origin",
    ),
    ("refusal-geometry-ip-pin", "the connection point of {0}"),
    ("refusal-geometry-pin", "connection point {0}"),
    ("refusal-geometry-misses", "misses {0} by less than 0.1 mm"),
    ("refusal-geometry-does-not-meet", "does not meet {0} at {1}"),
    (
        "refusal-geometry-end-off-cp",
        "{0} at {1} {2}; end the wire at its connection point",
    ),
    (
        "refusal-geometry-corner-onto-grid",
        "{0}; move the corner onto the grid",
    ),
    (
        "refusal-geometry-duplicate-corner",
        "{0} at {1} repeats the corner before it; remove the duplicate corner",
    ),
    (
        "refusal-geometry-overlap",
        "the wire runs back over itself on {0}: its stretch from {1} to {2} lies on its stretch from {3} to {4}; remove the stretch it runs back over",
    ),
    ("refusal-words-list", "{0}; {1}"),
    ("refusal-words-list-comma", "{0}, {1}"),
    ("refusal-words-rep-type-multi-line", "multi-line"),
    ("refusal-words-rep-type-single-line", "single-line"),
    ("refusal-words-rep-type-overview", "overview"),
    ("refusal-words-rep-type-fluid", "fluid"),
    ("refusal-words-rep-type-topology", "topology"),
    ("refusal-words-rep-type-contact-image", "contact image"),
    ("refusal-words-rep-type-panel-front", "panel front"),
    ("refusal-words-rep-type-strip-segment", "strip segment"),
    ("refusal-words-rep-type-graphic", "graphic"),
    ("refusal-words-page-kind-title-page", "a title page"),
    ("refusal-words-page-kind-table-of-contents", "a table of contents"),
    ("refusal-words-page-kind-section-divider", "a section divider"),
    ("refusal-words-page-kind-graphic", "a graphic page"),
    ("refusal-words-page-kind-single-line", "a single-line page"),
    ("refusal-words-page-kind-multi-line", "a multi-line page"),
    ("refusal-words-page-kind-overview", "an overview page"),
    ("refusal-words-page-kind-bus-topology", "a bus topology page"),
    ("refusal-words-page-kind-fluid", "a fluid page"),
    (
        "refusal-words-page-kind-assembly-overview",
        "an assembly overview page",
    ),
    ("refusal-words-page-kind-panel-layout", "a panel layout page"),
    ("refusal-words-page-kind-model-view", "a model view page"),
    (
        "refusal-words-page-kind-external-document",
        "an external document page",
    ),
    ("refusal-words-page-kind-report", "a report page"),
    // The parameters of any command: a record not in the project, one listed twice, none given, the schema's detail.
    ("refusal-params-detail", "{0}"),
    ("refusal-params-device-gone", "the device is not in the project"),
    ("refusal-params-surface-gone", "the surface is not in the project"),
    (
        "refusal-params-mounting-surface-gone",
        "the mounting surface is not in the project",
    ),
    ("refusal-params-page-gone", "the page is not in the project"),
    (
        "refusal-params-placement-gone",
        "the placement is not in the project",
    ),
    (
        "refusal-params-space-gone",
        "the layout space is not in the project",
    ),
    ("refusal-params-carrier-gone", "the carrier is not in the project"),
    (
        "refusal-params-node-gone",
        "the structure node is not in the project",
    ),
    (
        "refusal-params-enclosure-gone",
        "the enclosure is not in the project",
    ),
    (
        "refusal-params-revision-gone",
        "the revision is not in the project",
    ),
    ("refusal-params-waiver-gone", "the waiver is not in the project"),
    (
        "refusal-params-external-ref-gone",
        "the external reference is not in the project",
    ),
    ("refusal-params-copy-twice", "the copy is listed twice"),
    ("refusal-params-item-twice", "the item is listed twice"),
    ("refusal-params-device-twice", "the device is listed twice"),
    ("refusal-params-rep-twice", "the representation is listed twice"),
    ("refusal-params-record-twice", "the record is listed twice"),
    ("refusal-params-target-twice", "the target is listed twice"),
    ("refusal-params-node-twice", "the structure node is listed twice"),
    ("refusal-params-part-twice", "the part is listed twice"),
    (
        "refusal-params-ip-twice",
        "the interruption point is listed twice",
    ),
    (
        "refusal-params-connection-twice",
        "the connection is listed twice",
    ),
    ("refusal-params-child-twice", "the child is listed twice"),
    ("refusal-params-terminal-twice", "the terminal is listed twice"),
    ("refusal-params-asset-twice", "the asset is listed twice"),
    ("refusal-params-no-target", "give at least one target"),
    (
        "refusal-params-no-library-document",
        "give at least one library document",
    ),
    ("refusal-params-no-connection", "give at least one connection"),
    ("refusal-params-no-child", "give at least one child"),
    ("refusal-params-no-device", "give at least one device"),
    ("refusal-params-no-entry", "give at least one entry"),
    ("refusal-params-no-rep", "give at least one representation"),
    ("refusal-params-no-part-copy", "give at least one part copy"),
    ("refusal-params-no-assignment", "give at least one assignment"),
    (
        "refusal-params-no-terminal-to-insert",
        "give at least one terminal to insert",
    ),
    ("refusal-params-no-terminal", "name at least one terminal"),
    ("refusal-params-no-page", "name at least one page"),
    ("refusal-params-no-module", "name at least one module"),
    ("refusal-params-positions-one-based", "positions are 1-based"),
    // assembly.attach, valve.add and device.link.
    (
        "refusal-assembly-position-twice",
        "position {0} is given to two children ({1} and {2})",
    ),
    (
        "refusal-assembly-cycle",
        "device {0} is an ancestor of the parent {1}; an assembly cannot hold itself",
    ),
    (
        "refusal-assembly-position-held",
        "position {0} of {1} is held by device {2}; move it first (no push insert), then attach {3}",
    ),
    (
        "refusal-valve-position-held",
        "position {0} of {1} is held by {2}; choose a free position (no push insert)",
    ),
    (
        "refusal-device-link-holder",
        "{0} cannot drive {1}, which holds it",
    ),
    (
        "refusal-device-link-twice",
        "device {0} already links to {1} with this kind",
    ),
    ("refusal-assembly-own-child", "a device cannot be its own child"),
    ("refusal-device-link-itself", "a device cannot link to itself"),
    // box.reshape.
    (
        "refusal-box-reshape-no-box",
        "the representation has no box; only a black box drawn with an outline can be reshaped",
    ),
    (
        "refusal-box-reshape-out-of-range",
        "the box lies outside the drawable range; reshape it nearer the page origin",
    ),
    (
        "refusal-box-reshape-cp-connected",
        "box connection point {0} is connected on this page and the new connection points leave it out; keep it in `cps`",
    ),
    (
        "refusal-box-reshape-few-vertices",
        "an outline has at least three vertices",
    ),
    (
        "refusal-box-reshape-cp-off-outline",
        "the connection point does not lie on the outline; put it on an edge",
    ),
    (
        "refusal-box-cp-index-twice",
        "two box CPs have the same function CP index",
    ),
    // box.structure.
    (
        "refusal-box-structure-few-vertices",
        "a structure box's outline needs at least three vertices, not {0}",
    ),
    (
        "refusal-box-structure-node-gone",
        "structure node {0} does not exist",
    ),
    (
        "refusal-box-structure-node-aspect",
        "structure node {0} ({1}) is not of the aspect {2}",
    ),
    (
        "refusal-box-structure-style",
        "a box style this build does not know; use dotted or dash_dot",
    ),
    (
        "refusal-box-structure-no-area",
        "a structure box's outline has no area: its vertices lie on one line",
    ),
    // bus.logical_net.create and bus.physical_net.create.
    (
        "refusal-bus-logical-net-exists",
        "logical net {0} exists already; names are unique",
    ),
    (
        "refusal-bus-physical-net-exists",
        "physical net {0} exists already; names are unique",
    ),
    (
        "refusal-bus-medium-unknown",
        "{0} is not a medium; give copper or fibre",
    ),
    (
        "refusal-bus-system-unknown",
        "{0} is not a bus system; give profibus_dp, profinet, ethercat, ethernet_ip, io_link, asi, canopen or modbus_tcp",
    ),
    (
        "refusal-bus-zero-rate",
        "a data rate of 0 bit/s; give the rate in bit/s (1500000) or leave it unset",
    ),
    // The project settings: project.checks.set, project.frames.set, project.drawing.set and project.company.set.
    (
        "refusal-settings-member-not-object",
        "the settings member `{0}` is not an object (a newer or damaged format); it is kept as it is",
    ),
    (
        "refusal-drawing-member-not-object",
        "the project settings' `{0}` member is not an object (a newer or damaged format); it is kept as it is",
    ),
    (
        "refusal-company-no-party",
        "the project has no panel-builder party to mark; give the company `name` to create it",
    ),
    (
        "refusal-company-member-not-object",
        "the project row's `{0}` member is not an object (a newer or damaged format); it is kept as it is",
    ),
    (
        "refusal-checks-scheme-unknown",
        "`{0}` is not a shipped check scheme; the schemes are: {1}",
    ),
    (
        "refusal-frames-page-type-twice",
        "the page type `{0}` has an entry already; give one entry per page type",
    ),
    (
        "refusal-company-no-member",
        "give at least one of `name`, `notice`, `personal_data` and `logo`",
    ),
    (
        "refusal-drawing-no-member",
        "give at least one member (`crossings`, `marker_mode`)",
    ),
    (
        "refusal-frames-unknown-page-type",
        "not a page type of this version",
    ),
    (
        "refusal-frames-report",
        "report pages take the form of their report block; give no entry for `report`",
    ),
    (
        "refusal-frames-not-frame",
        "the form is not a frame (its kind must be `frame`, `cover` or `divider`)",
    ),
    (
        "refusal-company-image-too-large",
        "{0} is larger than {1} KiB (the limit of {2})",
    ),
    ("refusal-company-the-logo", "the logo"),
    ("refusal-company-a-logo", "a company logo"),
    // project.languages.set and project.settings.set.
    (
        "refusal-languages-source-as-translation",
        "`{0}` is the source language, not a translation",
    ),
    (
        "refusal-settings-member-unknown",
        "`{0}` is not a settable project setting; settable: {1}",
    ),
    ("refusal-settings-value", "not a valid `{0}` setting: {1}"),
    (
        "refusal-settings-text-styles",
        "not a valid `text_styles` setting: {0}",
    ),
    ("refusal-languages-twice", "a translation language is named twice"),
    (
        "refusal-languages-date-format",
        "not a date format of this version",
    ),
    (
        "refusal-languages-decimal-separator",
        "not a decimal separator of this version",
    ),
    (
        "refusal-languages-no-source",
        "translation languages need a `source` language",
    ),
    // check.waive.
    (
        "refusal-check-waive-twice",
        "a waiver of {0} covers this finding already",
    ),
    (
        "refusal-check-waive-rule-id",
        "`{0}` is not a rule ID (EW-<PACK>-<three digits>, for example EW-CORE-041)",
    ),
    (
        "refusal-check-waive-fingerprint",
        "`{0}` is not a finding fingerprint (32 hex digits)",
    ),
    (
        "refusal-check-waive-reason",
        "a waiver needs a reason of at least {0} characters",
    ),
    ("refusal-check-unwaive-gone", "waiver {0} does not exist"),
    // connection.draw, connection.draw_many and connection.delete.
    (
        "refusal-connection-no-page",
        "{0} has no page, so a junction cannot be placed on it; give it a page first",
    ),
    (
        "refusal-connection-same-end",
        "both ends are the same endpoint; pick two different ends",
    ),
    (
        "refusal-connection-same-connection",
        "both ends are on the same connection; pick two different connections",
    ),
    ("refusal-connection-fluid-line-on", "the fluid line on {0}"),
    ("refusal-connection-fluid-line-any", "a fluid line"),
    ("refusal-connection-not-fluid-port", "{0} is not a fluid port"),
    ("refusal-connection-junction-joins-wires", "{0} joins wires"),
    ("refusal-connection-no-fluid-line", "{0} is no fluid line"),
    (
        "refusal-connection-fluid-ports-only",
        "a fluid line joins fluid ports only; {0}",
    ),
    (
        "refusal-connection-fluid-media",
        "a fluid line does not join a hydraulic port to a pneumatic or vacuum port: {0}, {1}",
    ),
    (
        "refusal-connection-fluid-port",
        "{0} is a fluid port; only a fluid line joins it",
    ),
    (
        "refusal-connection-fluid-junction",
        "{0} joins fluid lines; only a fluid line joins it",
    ),
    (
        "refusal-connection-fluid-branch",
        "only a fluid line branches from {0}",
    ),
    ("refusal-connection-draw-many-entry", "entry {0}: {1}"),
    (
        "refusal-connection-delete-none",
        "no connection given; name at least one connection to delete",
    ),
    (
        "refusal-connection-draw-many-too-many",
        "{0} connections are more than the {1} one command takes",
    ),
    // junction.edit.
    (
        "refusal-junction-sequence",
        "`sequence` is not a permutation of the connections ending at the junction; list each of them exactly once",
    ),
    // ip.place, ip.continue and the star of an interruption point.
    (
        "refusal-ip-star-external",
        "an external point never pairs, so it takes no star; clear the star",
    ),
    (
        "refusal-ip-star-not-source",
        "a star source is a source point; set the direction to source, or clear the star",
    ),
    (
        "refusal-ip-continue-not-ip",
        "the function is not an interruption point; pick an IP function",
    ),
    (
        "refusal-ip-continue-external",
        "an external connection point never pairs; it cannot be continued",
    ),
    (
        "refusal-ip-continue-same-page",
        "the target page is the page of the interruption point; continue it on another page",
    ),
    (
        "refusal-ip-continue-no-rep",
        "the interruption point has no representation to continue",
    ),
    // potential.define and bond.create.
    (
        "refusal-potential-define-on-connection",
        "a potential is defined at a CP or a junction, not on a connection; name one of them",
    ),
    (
        "refusal-potential-define-twice",
        "this point already defines the potential {0}; change that definition instead",
    ),
    (
        "refusal-bond-on-connection",
        "a bond ends at a CP or a junction, not on a connection; name one of them",
    ),
    (
        "refusal-bond-same-end",
        "both ends of the bond are the same point; pick two different ends",
    ),
    ("refusal-bond-unknown-kind", "unknown bond kind"),
    // connection.draw_to_ip.
    (
        "refusal-draw-to-ip-cp-count",
        "the interruption point's function definition has {0} connection points; a wire ends in an interruption point with exactly one",
    ),
    (
        "refusal-draw-to-ip-fluid",
        "a fluid line joins fluid ports only; an interruption point is not a fluid port",
    ),
    (
        "refusal-connection-start-on-other-page",
        "the wire's start is drawn on {0}, not on {1}; start the wire on {1}",
    ),
    (
        "refusal-draw-to-ip-start-on-no-page",
        "the wire's start is drawn on no page, not on {0}; start the wire there",
    ),
    (
        "refusal-draw-to-ip-symbol-cp",
        "the interruption-point symbol does not draw its connection point; pick an interruption-point symbol that draws it",
    ),
    // ip.arrange, and a move that takes a route along.
    (
        "refusal-ip-arrange-not-ip",
        "{0} is not an interruption point; arrange interruption points only",
    ),
    (
        "refusal-ip-arrange-drawn-times",
        "{0} is drawn {1} times; an arranged interruption point is drawn exactly once",
    ),
    (
        "refusal-ip-arrange-pages",
        "the interruption points lie on different pages; arrange the interruption points of one page",
    ),
    (
        "refusal-ip-arrange-out-of-range",
        "the arrangement puts a connection point outside the drawable range; arrange interruption points nearer the page origin",
    ),
    (
        "refusal-ip-arrange-too-far",
        "the arrangement moves a connection point too far",
    ),
    ("refusal-move-what-arrangement", "the arrangement"),
    ("refusal-move-what-reshape", "the reshape"),
    (
        "refusal-move-route-out-of-range",
        "{0} moves the route of {1} outside the drawable range; move it less far",
    ),
    (
        "refusal-arrange-few-ips",
        "arrange at least two interruption points",
    ),
    // device.place, box.black and a device from a part.
    (
        "refusal-device-place-no-cp",
        "the symbol has no connection point {0}; pick one of its CPs",
    ),
    (
        "refusal-device-place-ip",
        "an interruption point is placed with `ip.place`; pick another function",
    ),
    (
        "refusal-device-place-out-of-range",
        "the position puts a connection point of the symbol outside the drawable range; place it nearer the page origin",
    ),
    (
        "refusal-device-place-lost-cp",
        "the symbol lost a connection point",
    ),
    (
        "refusal-device-place-fndef-missing",
        "the function definition of the symbol is not embedded in the project; pass `fndef` or embed it",
    ),
    (
        "refusal-rep-type",
        "representation type {0} is not allowed on {1} ({2}); allowed: {3}",
    ),
    (
        "refusal-rep-type-none",
        "none (this page type takes no representations)",
    ),
    (
        "refusal-box-black-cp-out-of-range",
        "box CP {0} lies outside the drawable range; place the box nearer the page origin",
    ),
    (
        "refusal-box-black-not-black-box",
        "the function is not a black box; place another representation of it with `device.place`",
    ),
    (
        "refusal-box-black-out-of-range",
        "the position puts a connection point of the black box outside the drawable range; place it nearer the page origin",
    ),
    (
        "refusal-from-part-no-template",
        "part {0} has no function template; place a symbol with a `new_device` target",
    ),
    (
        "refusal-from-part-no-entry",
        "part {0} has no template entry {1}; it has {2} (0 to {3})",
    ),
    (
        "refusal-part-entry-fndef-missing",
        "the function definition of template entry {0} of part {1} is not embedded in the project; embed it first (`library.embed`)",
    ),
    (
        "refusal-from-part-mains",
        "the function template of part {0} has {1} main entries; a device made from it needs exactly one",
    ),
    (
        "refusal-from-part-black-box",
        "template entry {0} of part {1} is a black box; place it with `box.black`",
    ),
    (
        "refusal-from-part-other-fndef",
        "the symbol draws another function definition than template entry {0} of part {1}; pick a symbol of that entry's function definition",
    ),
    (
        "refusal-from-part-not-black-box",
        "template entry {0} of part {1} is not a black box; place it with `device.place`",
    ),
    (
        "refusal-new-terminal-part",
        "part {0} is no terminal of the symbol's function definition; pick a symbol of the part's main function or another part",
    ),
    (
        "refusal-box-black-terminal",
        "a black box is no terminal; place a terminal with `device.place`",
    ),
    (
        "refusal-box-black-few-points",
        "an outline has at least three points",
    ),
    // part.assign.
    (
        "refusal-assign-held-other-hash",
        "the project holds {0} at version {1} with another hash; update from library first (library.update)",
    ),
    (
        "refusal-assign-part-unused",
        "{0} does not use the part to replace",
    ),
    (
        "refusal-assign-part-missing",
        "part copy {0} is neither held by the project nor in `copies`",
    ),
    (
        "refusal-assign-from-without-replace",
        "`from` goes with mode `replace` only",
    ),
    (
        "refusal-assign-replace-needs-from",
        "mode `replace` needs `from`, the part to replace",
    ),
    ("refusal-assign-quantity", "the quantity must be greater than 0"),
    // rep.move and rep.transform.
    (
        "refusal-rep-move-surface-view",
        "the surface view on {0} does not move with a selection; leave it out of the selection, or change it with Set panel view",
    ),
    (
        "refusal-rep-move-unknown-item",
        "a page item on {0} is of a kind this build keeps as it is, so it cannot be moved; leave it out of the selection",
    ),
    (
        "refusal-rep-move-item-out-of-range",
        "the move takes the {0} on {1} outside the drawable range; move it less far",
    ),
    ("refusal-rep-move-item-graphic", "graphic"),
    ("refusal-rep-move-item-text", "text"),
    ("refusal-rep-move-item-wire-marker", "wire marker"),
    ("refusal-rep-move-item-image", "image"),
    (
        "refusal-rep-move-box-out-of-range",
        "the move takes a structure box on {0} outside the drawable range; move it less far",
    ),
    (
        "refusal-rep-move-steps-off-grid",
        "{0}, so it cannot be moved by grid steps; move it onto the grid first",
    ),
    (
        "refusal-rep-move-steps-out-of-range",
        "the move takes {0} outside the drawable range; move it fewer grid steps",
    ),
    (
        "refusal-rep-move-steps-cp-out-of-range",
        "the move takes a connection point of {0} outside the drawable range; move it fewer grid steps",
    ),
    (
        "refusal-rep-move-route-out-of-range",
        "the move takes the route of {0} outside the drawable range; move it less far",
    ),
    (
        "refusal-rep-move-nothing",
        "give at least one representation or item to move",
    ),
    (
        "refusal-rep-move-out-of-range",
        "the move takes {0} outside the drawable range; move it less far",
    ),
    (
        "refusal-rep-move-cp-out-of-range",
        "the move takes a connection point of {0} outside the drawable range; move it less far",
    ),
    (
        "refusal-rep-move-end-behind",
        "the move would keep one end of {0} behind on the old page; move both ends together",
    ),
    (
        "refusal-rep-transform-nothing",
        "give at least one representation to transform",
    ),
    (
        "refusal-rep-transform-rotate",
        "`rotate` counts quarter turns and must be 0 to 3",
    ),
    (
        "refusal-rep-transform-cp-out-of-range",
        "the transform puts a connection point of {0} outside the drawable range",
    ),
    (
        "refusal-rep-transform-route-out-of-range",
        "the transform moves the route of {0} outside the drawable range; turn it nearer the page origin",
    ),
    ("refusal-rep-move-one-page", "every item lies on one page"),
    (
        "refusal-rep-move-other-page",
        "{0} is on {1}, the items on {2}; move one page at a time",
    ),
    ("refusal-rep-anchor-policy-unknown", "unknown anchor policy"),
    (
        "refusal-rep-move-steps-other-page",
        "a move to another page takes `delta`; `steps` moves on the representations' own page",
    ),
    (
        "refusal-rep-move-steps-and-delta",
        "give either `steps` or a non-zero `delta`, not both",
    ),
    (
        "refusal-rep-move-items-other-page",
        "page items and structure boxes move on their own page; a move to another page takes no `items`",
    ),
    // The terminals of a strip (device.place, terminal.add).
    (
        "refusal-terminal-not-strip",
        "the device is not a terminal strip; pick a device with the strip aspect",
    ),
    (
        "refusal-terminal-designation-used",
        "designation {0} is already used in the strip; choose another",
    ),
    (
        "refusal-terminal-seq-used",
        "sequence number {0} is already used in the strip; choose another",
    ),
    // edit.delete.
    (
        "refusal-edit-delete-plate",
        "The plate goes with its enclosure; delete the enclosure of {0}.",
    ),
    ("refusal-edit-delete-the-plate", "the plate"),
    ("refusal-edit-delete-the-enclosure", "the enclosure"),
    ("refusal-edit-delete-holds-one", "holds 1 placement"),
    ("refusal-edit-delete-holds", "holds {0} placements"),
    (
        "refusal-edit-delete-carrier-holds",
        "{0} {1}; use Delete carrier… to remove or rehome them.",
    ),
    (
        "refusal-edit-delete-plate-holds",
        "The plate of {0} {1}; remove them first.",
    ),
    (
        "refusal-edit-delete-carries-enclosure",
        "{0} carries an enclosure; delete the enclosure first.",
    ),
    ("refusal-edit-delete-project", "the project cannot be deleted"),
    // edit.paste.
    (
        "refusal-paste-grid",
        "the copied content uses another grid ({0}/{1} nm); paste on a page with that grid (this page has {2})",
    ),
    ("refusal-paste-too-many", "{0} records; a paste takes at most {1}"),
    (
        "refusal-paste-schema-version",
        "the record is of schema version {0}, this version reads {1}",
    ),
    (
        "refusal-paste-format",
        "the fragment is of project format {0}, this version writes {1}",
    ),
    (
        "refusal-paste-channel-module",
        "a channel's module is not in the fragment; copy them together",
    ),
    (
        "refusal-paste-generated-page-inserted",
        "a generated page is not inserted",
    ),
    ("refusal-paste-generated-page", "a generated page takes no paste"),
    (
        "refusal-paste-page-macro",
        "a page macro's pages are inserted with macro.insert, not pasted",
    ),
    ("refusal-paste-page-record-kind", "a page record is of kind page"),
    ("refusal-paste-node-no-path", "a structure node without a path"),
    (
        "refusal-paste-adapt-no-copy",
        "names no copy of the fragment that the project holds in another version",
    ),
    (
        "refusal-paste-library-kind",
        "not a library document of kind part, symbol, fndef, form or manufacturer",
    ),
    (
        "refusal-paste-anchor-off-grid",
        "the anchor is not a point of the fragment's grid",
    ),
    ("refusal-paste-empty", "the fragment holds no record"),
    (
        "refusal-paste-grid-form",
        "the grid is [num, den] of positive integers",
    ),
    (
        "refusal-paste-grid-differs",
        "the page's grid differs from the fragment's",
    ),
    (
        "refusal-paste-page-type",
        "the page's type takes no symbols or wires: paste on a schematic page",
    ),
    (
        "refusal-paste-out-of-range",
        "the paste moves the record outside the drawable range",
    ),
    (
        "refusal-paste-far-from-anchor",
        "the point is too far from the anchor",
    ),
    (
        "refusal-paste-rep-off-grid",
        "the representation is not on the connection grid, so it cannot be pasted by grid steps",
    ),
    (
        "refusal-paste-rep-type",
        "the page's type does not take a representation of type {0}",
    ),
    ("refusal-paste-off-grid", "{0}; paste at a point on the grid"),
    ("refusal-paste-the-paste-point", "the paste point"),
    ("refusal-paste-kind", "a record of kind {0} {1}"),
    (
        "refusal-paste-kinds",
        "is not pasted (only devices, functions, representations, connections, junctions, potential definitions, page items and structure boxes)",
    ),
    (
        "refusal-paste-adapt-major",
        "the project's version of {0} {1} differs from the pasted one in a major way ({2}); update instead",
    ),
    (
        "refusal-paste-held-other-version",
        "the project holds another version of {0} {1}; update from library first",
    ),
    (
        "refusal-paste-reference-missing",
        "the {0}'s {1} names a {2} that is neither in the fragment nor in this project; copy them together",
    ),
    // item.place and marker.place, and a page item's members.
    (
        "refusal-items-image-asset-missing",
        "the project holds no asset {0}; an image places an asset the project already holds, or gives its file as `data`",
    ),
    (
        "refusal-items-marker-wire-other-page",
        "the wire is not drawn on {0}",
    ),
    (
        "refusal-items-marker-angle",
        "a wire marker's angle is 0 or 90000 (10⁻³ degree), not {0}",
    ),
    (
        "refusal-items-marker-colours",
        "give no colour or one per connection ({0} connections, {1} colours)",
    ),
    (
        "refusal-items-csa-unknown",
        "a cross-section this build does not know",
    ),
    (
        "refusal-items-image-asset-or-data",
        "give exactly one of `asset` (a hash the project holds) and `data` (the image file in base64), not both and not neither",
    ),
    ("refusal-items-line-zero", "a line of zero length"),
    (
        "refusal-items-polyline-points",
        "a polyline needs at least two distinct points",
    ),
    (
        "refusal-items-polygon-points",
        "a polygon needs at least three distinct points",
    ),
    (
        "refusal-items-polygon-no-area",
        "a polygon without area: its points lie on one line",
    ),
    ("refusal-items-rect-no-area", "a rectangle without area"),
    (
        "refusal-items-circle-radius",
        "a circle needs a radius greater than 0",
    ),
    ("refusal-items-arc-radius", "an arc needs a radius greater than 0"),
    ("refusal-items-arc-sweep", "an arc needs a sweep other than 0"),
    ("refusal-items-shape-unknown", "a shape this build does not know"),
    (
        "refusal-items-stroke-width",
        "a stroke width must be greater than 0",
    ),
    (
        "refusal-items-dash-unknown",
        "a dash pattern this build does not know",
    ),
    (
        "refusal-items-bind-unknown",
        "`{0}` is not a bindable name; bindable: {1}",
    ),
    (
        "refusal-items-text-empty",
        "a free text without `bind` needs a text",
    ),
    (
        "refusal-items-text-angle",
        "a free text's angle is 0, 90000, 180000 or 270000 (10⁻³ degree), not {0}",
    ),
    (
        "refusal-items-align-unknown",
        "an alignment this build does not know",
    ),
    ("refusal-items-image-no-area", "an image rectangle without area"),
    (
        "refusal-items-colour-not-company",
        "`{0}` is neither an IEC 60757 colour code nor a company code of the project's colour profile",
    ),
    (
        "refusal-items-colour-foreign",
        "`{0}` is a foreign colour code; use an IEC 60757 code or a company code of the project's colour profile",
    ),
    (
        "refusal-items-colour-unknown",
        "a colour code this build does not know",
    ),
    ("refusal-items-the-image", "the image"),
    ("refusal-items-image-as-data", "an image given as data"),
    // rep.exchange_symbol.
    (
        "refusal-exchange-no-function",
        "the representation draws no function; there is nothing to keep",
    ),
    (
        "refusal-exchange-ip",
        "an interruption point keeps its interruption-point symbol; its symbol cannot be exchanged",
    ),
    (
        "refusal-exchange-no-counterpart",
        "connection point {0} of the new symbol has no counterpart by designation and would draw function CP {1}; give `pin_map`",
    ),
    (
        "refusal-exchange-out-of-range",
        "the new symbol puts a connection point outside the drawable range; place the representation nearer the page origin",
    ),
    (
        "refusal-exchange-cp-connected",
        "function CP {0} is connected on this page and the new symbol does not draw it; pick a symbol that draws it or give `pin_map`",
    ),
    (
        "refusal-exchange-too-far",
        "the exchange moves a connection point too far",
    ),
    (
        "refusal-exchange-route-out-of-range",
        "the exchange moves the route of {0} outside the drawable range; move the representation nearer the page origin first",
    ),
    (
        "refusal-exchange-no-symbol-cp",
        "the new symbol has no connection point {0}",
    ),
    (
        "refusal-exchange-no-function-cp",
        "the function has no connection point {0}",
    ),
    (
        "refusal-exchange-cp-unmapped",
        "connection point {0} of the new symbol is not in the map and would draw function CP {1}; map it",
    ),
    (
        "refusal-exchange-symbol-cp-twice",
        "the symbol CP is mapped twice",
    ),
    // rep.join_device.
    (
        "refusal-join-device-link",
        "the representation's device takes part in a device link with {0}, which the join would lose; remove the link first",
    ),
    (
        "refusal-join-same-device",
        "the representation belongs to device {0} already; name another device",
    ),
    (
        "refusal-join-cp-connected",
        "on device {0} the function has no connection point {1}, which is connected now; join another device or disconnect it first",
    ),
    (
        "refusal-join-not-eligible",
        "{0}; a representation joins another device only when its device has exactly one function, no part and no other representation",
    ),
    (
        "refusal-join-not-one-function",
        "the representation draws not exactly one function",
    ),
    (
        "refusal-join-ip-or-box",
        "the representation draws an interruption point or a black box",
    ),
    (
        "refusal-join-no-device",
        "the representation's function has no device",
    ),
    (
        "refusal-join-terminal",
        "the representation's device is a terminal or a strip",
    ),
    (
        "refusal-join-other-function",
        "the representation's device has another function",
    ),
    ("refusal-join-has-part", "the representation's device has a part"),
    (
        "refusal-join-other-rep",
        "the representation's function has another representation",
    ),
    (
        "refusal-join-no-function",
        "device {0} cannot take {1}: it has no unplaced function of it, and {2}",
    ),
    (
        "refusal-join-no-auxiliary",
        "its family takes no auxiliary contacts (no coil, no part naming it)",
    ),
    (
        "refusal-join-contact-only",
        "only a contact joins as an auxiliary function",
    ),
    // fields.set.
    ("refusal-fields-ip-star", "{0}: {1}"),
    ("refusal-fields-not-fluid-line", "{0} is not a fluid line"),
    (
        "refusal-fields-counter-used",
        "counter {0} with letter {1} is already used on {2} by another device; use {3}, the next free counter",
    ),
    (
        "refusal-fields-revision-party",
        "`{0}` of a revision is not set here: set by Check ▸ Revision ▸ Set revision author… or by approval (four eyes need one confirmed path per role)",
    ),
    (
        "refusal-fields-link-kind-unknown",
        "{0} is not a device link kind; give drives or linkage",
    ),
    (
        "refusal-fields-below-non-object",
        "`{0}` lies below a value that is not an object",
    ),
    (
        "refusal-fields-channel-state-unknown",
        "{0} is not a channel state; give spare, reserved or deactivated, or null for the derived state",
    ),
    (
        "refusal-fields-set-many-too-many",
        "{0} entries are more than the {1} one command takes",
    ),
    (
        "refusal-fields-set-many-pair-twice",
        "an earlier entry already sets `{0}` on this target; give each (target, path) once",
    ),
    (
        "refusal-fields-config-covers-other",
        "the configuration {0} covers {1}, which is neither the assembly device {2} nor one of its descendants",
    ),
    (
        "refusal-fields-item-variant",
        "the page item is a {0}, not a {1}; `{2}` is not one of its fields",
    ),
    (
        "refusal-fields-not-ip",
        "{0} is not an interruption point; `{1}` is not one of its fields",
    ),
    (
        "refusal-fields-master-cp",
        "a master is a CP: {\"cp\": [function, index]}",
    ),
    (
        "refusal-fields-direction-unknown",
        "{0} is not a direction; give source, target or undirected",
    ),
    (
        "refusal-fields-no-aspect",
        "the device has no `{0}` aspect; `{1}` cannot be set",
    ),
    (
        "refusal-fields-zero-rate",
        "a data rate of 0 bit/s; give the rate in bit/s (1500000) or null",
    ),
    (
        "refusal-fields-line-kind",
        "a line kind is one of working, pilot, exhaust, drain and flexible",
    ),
    (
        "refusal-fields-copies-in-entry",
        "give the library documents in the command's `copies`, not in an entry",
    ),
    (
        "refusal-fields-address-unknown",
        "not an address this build knows; give an address text, auto, excluded or tbd",
    ),
    (
        "refusal-fields-not-settable-kind",
        "`{0}` is not settable: {1} has no settable fields",
    ),
    (
        "refusal-fields-not-settable",
        "`{0}` is not a settable field of {1}; settable: {2}",
    ),
    ("refusal-fields-function-named", "function {0}"),
    (
        "refusal-fields-no-port",
        "{0} has no bus port; set `bus_port` first",
    ),
    (
        "refusal-fields-port-role-unknown",
        "{0} is not a port role; give master, slave, controller, device or passive",
    ),
    (
        "refusal-fields-bus-address-unknown",
        "{0} is not a bus address; give tbd, not_applicable or {\"value\": {\"profibus\": n}}",
    ),
    (
        "refusal-fields-termination-unknown",
        "{0} is not a termination; give on or off",
    ),
    (
        "refusal-fields-counter-used-project",
        "counter {0} with letter {1} is already used by another cable without a page; use {2}, the next free counter",
    ),
    // valve.set_occupancy and fluid.port.close.
    ("refusal-valve-no-positions", "{0} has no valve positions"),
    (
        "refusal-valve-no-position",
        "{0} has {1} valve positions; position {2} does not exist",
    ),
    (
        "refusal-valve-position-holds",
        "position {0} of {1} holds {2}; assign the new part to {2} instead",
    ),
    (
        "refusal-valve-position-empty",
        "position {0} of {1} holds no valve; name the valve part",
    ),
    (
        "refusal-fluid-port-has-line",
        "{0} has a fluid line; delete the line before closing the port",
    ),
    // The panel layout: layout.place, layout.move, layout.place_assembly, layout.auto_fill, the carriers, spaces and enclosures.
    ("refusal-words-rail", "rail {0}"),
    ("refusal-words-duct", "duct {0}"),
    ("refusal-words-rail-label", "Rail {0}"),
    ("refusal-words-duct-label", "Duct {0}"),
    (
        "refusal-words-device-gone-start",
        "A device that is not in the project",
    ),
    ("refusal-layout-the-rail", "the rail"),
    ("refusal-layout-the-assembly", "the assembly {0}"),
    ("refusal-layout-the-assembly-start", "The assembly {0}"),
    ("refusal-layout-a-placement", "a placement"),
    ("refusal-layout-mount-screw", "screw-mounted"),
    ("refusal-layout-mount-busbar-adapter", "busbar adapter"),
    ("refusal-layout-mount-door-cutout", "door cut-out"),
    ("refusal-layout-mount-on-parent", "mounted on its parent"),
    ("refusal-layout-mount-none", "not mounted"),
    (
        "refusal-layout-rail-path",
        "the rail's path is not two points on one horizontal or vertical line; edit the rail first",
    ),
    (
        "refusal-layout-carrier-unknown",
        "the carrier is of a kind this build does not know; it carries no placements",
    ),
    (
        "refusal-layout-part-no-surface",
        "Part {0} does not mount on a surface; choose a part that mounts on it.",
    ),
    ("refusal-layout-does-not-fit", "{0} ({1}) does not fit a {2} rail"),
    (
        "refusal-layout-pose-rail-axis",
        "a pose on a rail lies on its axis: t = (s, 0, 0)",
    ),
    (
        "refusal-layout-pose-rail-turn",
        "a pose on a rail turns by 0° or 180° about the surface normal only",
    ),
    (
        "refusal-layout-pose-off-rail",
        "the pose is off the rail: s = {0} mm is outside [0, {1} mm]",
    ),
    (
        "refusal-layout-pose-surface-plane",
        "a pose on a surface lies in its plane: t = (u, v, 0)",
    ),
    (
        "refusal-layout-pose-surface-turn",
        "a pose on a surface turns in steps of 90° about the surface normal only",
    ),
    ("refusal-layout-pose-duct", "a duct carries no placements"),
    (
        "refusal-layout-space-named-exists",
        "location {0} already has layout space \"{1}\"; a location has one layout space",
    ),
    (
        "refusal-layout-space-exists",
        "the layout space of location {0} already exists; a location has one layout space",
    ),
    (
        "refusal-layout-space-not-location",
        "node {0} is not a location; a layout space lays out a location",
    ),
    (
        "refusal-layout-space-has-enclosure",
        "the layout space already has enclosure {0}; a layout space has one enclosure. Choose another layout space, or edit {0}",
    ),
    (
        "refusal-layout-no-enclosure-part",
        "{0}: the device has no enclosure part",
    ),
    (
        "refusal-layout-carrier-holds",
        "the carrier holds {0} placement(s); give `then`: `remove_placements` or `rehome_to_surface`",
    ),
    (
        "refusal-layout-rehome-rail-only",
        "only the placements of a rail (two points on one horizontal or vertical line) rehome to its surface",
    ),
    (
        "refusal-layout-rehome-off-rail",
        "the placement of {0} has a pose off the rail; move or remove it first",
    ),
    (
        "refusal-layout-snap-no-gap",
        "No gap on {0} is wide enough for {1} ({2} mm).",
    ),
    ("refusal-layout-snap-unlock", "Unlock {0} to push or swap it."),
    (
        "refusal-layout-snap-swap-locked",
        "Swapping would move {0}, which is locked.",
    ),
    (
        "refusal-layout-snap-push-locked",
        "Pushing would move {0}, which is locked.",
    ),
    (
        "refusal-layout-snap-swap-past-end",
        "Swapping would move {0} past the rail end by {1} mm.",
    ),
    (
        "refusal-layout-snap-push-past-end",
        "Pushing would move {0} past the rail end by {1} mm.",
    ),
    (
        "refusal-layout-snap-item-past-end",
        "{0} would pass the rail end by {1} mm.",
    ),
    (
        "refusal-layout-snap-swap-across",
        "Swap works within one rail; use push or free to place it there.",
    ),
    (
        "refusal-layout-snap-swap-no-target",
        "Drop onto a placement to swap with it.",
    ),
    ("refusal-layout-modes-on-rail", "Push and swap work on a rail."),
    (
        "refusal-layout-already-placed",
        "{0} is already placed with this part and function; move its placement instead",
    ),
    (
        "refusal-layout-assembly-on-rail",
        "An assembly is placed on a rail.",
    ),
    (
        "refusal-layout-assembly-swap",
        "Swap works with one placement; place an assembly in free or push mode.",
    ),
    (
        "refusal-layout-not-assembly",
        "{0} is not part of an assembly; place it on its own.",
    ),
    (
        "refusal-layout-assembly-placed",
        "Every member of {0} that mounts is already placed.",
    ),
    ("refusal-layout-auto-fill-rail", "Auto-fill works on a rail."),
    ("refusal-layout-auto-fill-no-device", "no device to place."),
    (
        "refusal-layout-auto-fill-nothing",
        "Nothing was placed on {0}: {1}",
    ),
    (
        "refusal-layout-no-enclosure-section",
        "{0} has no enclosure section; choose an enclosure part.",
    ),
    (
        "refusal-layout-location-has-enclosure",
        "location +{0} already has enclosure {1}; a location has one enclosure. Choose another location, or edit {1}",
    ),
    (
        "refusal-layout-plate-part-missing",
        "the enclosure part names a plate part that the project does not hold; add it to copies",
    ),
    (
        "refusal-layout-enclosure-turn",
        "an enclosure's pose is a translation at this version; give it without a turn",
    ),
    (
        "refusal-layout-part-copy-missing",
        "part copy {0} is not held by the project",
    ),
    ("refusal-layout-part-unused", "{0} does not use this part"),
    (
        "refusal-layout-no-member-on-rail",
        "no member of {0} is on {1} yet; give the position s",
    ),
    ("refusal-layout-node-not-function", "node {0} is not a function"),
    (
        "refusal-layout-placement-on",
        "a placement sits on a carrier or on a surface",
    ),
    (
        "refusal-layout-rail-profile",
        "a rail profile is ts35x7_5, ts35x15, ts32 or ts15",
    ),
    (
        "refusal-layout-carrier-edit-no-member",
        "give at least one member (`surface`, `part`, `path`, `params`, `pe_bond`)",
    ),
    (
        "refusal-layout-space-edit-no-name",
        "give the name, or null to clear it",
    ),
    (
        "refusal-layout-carrier-kind-unknown",
        "the carrier is of a kind this build does not know; its parameters are kept as they are",
    ),
    (
        "refusal-layout-function-not-device",
        "the function is not one of the device's",
    ),
    ("refusal-layout-duct-length", "the length must be greater than 0"),
    (
        "refusal-layout-duct-params",
        "the parameters of a duct are `duct`",
    ),
    (
        "refusal-layout-rail-params",
        "the parameters of a rail are `rail`",
    ),
    (
        "refusal-layout-rail-part-profile",
        "the rail's part has profile {0}; the profile comes from its rail section",
    ),
    ("refusal-layout-fill-notice", "{0}: {1}"),
    ("refusal-layout-fill-no-room", "{0}: no room left on {1}"),
    ("refusal-layout-fill-placed", "already placed"),
    (
        "refusal-layout-fill-other-space",
        "not in this rail's layout space",
    ),
    ("refusal-layout-fill-not-mountable", "not mountable"),
    ("refusal-layout-fill-no-fit", "{0} does not fit a {1} rail"),
    ("refusal-layout-fill-not-rail", "{0}, not on a rail"),
    // page.create, page.delete, page.renumber and page.surface_view.set.
    (
        "refusal-page-create-outside-range",
        "folio {0} lies outside section {1}'s folio range {2} to {3}; choose a folio in the range",
    ),
    (
        "refusal-page-create-divider",
        "folio {0} is the divider page of section {1}; choose another folio",
    ),
    (
        "refusal-page-create-generated",
        "folio {0} is a page the report block at folio {1} generates; choose another folio",
    ),
    (
        "refusal-page-create-report",
        "report pages are generated from report blocks; create a report block",
    ),
    (
        "refusal-page-create-section",
        "`section` differs from the section of `after`; omit it or give an explicit `number`",
    ),
    (
        "refusal-page-create-no-folio",
        "give an explicit folio `number` or the page to insert `after`",
    ),
    (
        "refusal-page-create-exists",
        "folio {0} already exists in this section; choose a free folio",
    ),
    (
        "refusal-page-delete-none",
        "no pages given; name at least one page",
    ),
    ("refusal-page-renumber-step", "`step` must be at least 1"),
    (
        "refusal-page-renumber-no-pages",
        "no pages in scope; name pages, a section or the project with pages",
    ),
    (
        "refusal-page-renumber-out-of-range",
        "new folio number {0} is out of range; choose another `start` or `step`",
    ),
    (
        "refusal-page-renumber-used",
        "folio {0} is already used by a page outside the renumbered pages; choose another `start` or include that page",
    ),
    (
        "refusal-page-renumber-label-used",
        "sub-page label {0} is already used by another page of the section; choose another `start`",
    ),
    (
        "refusal-page-surface-view-not-layout",
        "folio {0} is not a panel-layout page; a surface view goes on a panel-layout page",
    ),
    ("refusal-page-scale", "a scale is 1:n with n at least 1"),
    // page.reorder.
    ("refusal-page-reorder-twice", "{0} is listed twice"),
    ("refusal-page-reorder-no-section", "no section {0}"),
    (
        "refusal-page-reorder-other-section",
        "page {0} is in another section; move pages between sections by changing their section first",
    ),
    (
        "refusal-page-reorder-too-many",
        "at most 1000 pages may be moved at once",
    ),
    (
        "refusal-page-reorder-before-and-section",
        "give either before or section, not both",
    ),
    (
        "refusal-page-reorder-before-moved",
        "the page to put the pages before is one of the moved pages; name a page that stays",
    ),
    (
        "refusal-page-reorder-unchanged",
        "the pages are already in this order",
    ),
    // library.embed and library.update.
    (
        "refusal-library-embed-held",
        "the project holds this {0} already; take another version of it with update from library",
    ),
    (
        "refusal-library-update-cp-twice",
        "two updated copies map CP {0} of function {1} differently ({2} and {3})",
    ),
    (
        "refusal-library-update-blocked",
        "the update leaves connected CPs without a CP; map them in `cp_maps`: {0}",
    ),
    (
        "refusal-library-update-missing",
        "the new versions name items the project does not hold; give them in `embed`: {0}",
    ),
    (
        "refusal-library-update-overrides",
        "the mapping moves two CP overrides of function {0} onto CP {1}; change the overrides first",
    ),
    (
        "refusal-library-copy-id",
        "the copy ID is {0}, not the copy identity {1} = v5(NS_LIBCOPY, library_id ‖ item_id)",
    ),
    (
        "refusal-library-copy-hash",
        "{0} is not the hash of the item's members (EW-CORE-014); the copy is not consistent with the hash it claims",
    ),
    (
        "refusal-library-update-not-semver",
        "`{0}` is not a semantic version",
    ),
    (
        "refusal-library-update-downgrade",
        "version {0} is lower than the held {1}: a downgrade is not an update",
    ),
    (
        "refusal-library-update-map-copy",
        "copy {0} is not one of `copies`",
    ),
    (
        "refusal-library-embed-local",
        "a project-local item (library_id = the project ID) is not embedded",
    ),
    (
        "refusal-library-embed-kind",
        "not a library document of kind part, symbol, fndef, form, macro or manufacturer",
    ),
    (
        "refusal-library-update-map-old-index",
        "{0} is no CP index of the held copy",
    ),
    (
        "refusal-library-update-map-new-index",
        "{0} is no CP index of the new copy",
    ),
    (
        "refusal-library-update-map-old-twice",
        "CP {0} of the held copy is mapped twice",
    ),
    (
        "refusal-library-update-map-new-twice",
        "CP {0} of the new copy is the target of two CPs of the held copy",
    ),
    (
        "refusal-library-update-attached-potential",
        "potential definition {0}",
    ),
    ("refusal-library-update-attached-bond", "a bond"),
    (
        "refusal-library-update-not-held",
        "the project holds no copy of {0}; embed it first (library.embed or part.assign)",
    ),
    ("refusal-library-update-blocked-pin", "{0} pin {1}: {2}"),
    (
        "refusal-library-update-embed-held",
        "the project holds a copy of {0} already; leave it out",
    ),
    ("refusal-library-update-missing-copy", "{0}, named by {1}"),
    // macro.insert.
    (
        "refusal-macro-held-version",
        "the project holds version {0} of this macro, not {1}; update from library first",
    ),
    (
        "refusal-macro-copy-missing",
        "no macro copy {0}: give its document in `copies`",
    ),
    (
        "refusal-macro-page-orientation",
        "a page macro keeps the layout of its pages and takes no orientation; insert it with `r0`",
    ),
    (
        "refusal-macro-window-no-at",
        "a window macro needs the point `at`",
    ),
    (
        "refusal-macro-window-pages",
        "a window macro's fragment holds no pages",
    ),
    (
        "refusal-macro-only-own-document",
        "only the inserted macro's document is given",
    ),
    (
        "refusal-macro-kind",
        "only window and page macros are inserted at this version",
    ),
    (
        "refusal-macro-no-variant",
        "the macro has no variant for this page's representation types",
    ),
    (
        "refusal-macro-page-no-page",
        "the page macro's fragment holds no page",
    ),
    ("refusal-macro-value-text", "{0} is not a text"),
    ("refusal-macro-value-non-empty", "{0} is not a non-empty text"),
    ("refusal-macro-value-number", "{0} is not a number"),
    ("refusal-macro-value-bool", "{0} is not true or false"),
    (
        "refusal-macro-value-tag-counter",
        "{0} is not a tag counter (an integer from 1)",
    ),
    (
        "refusal-macro-value-channel",
        "{0} is not a channel number (an integer from 0 to 65535)",
    ),
    (
        "refusal-macro-value-part",
        "{0} is not a part reference ({\"copy\": <copy ID>})",
    ),
    ("refusal-macro-value-device", "{0} is not a device ID"),
    (
        "refusal-macro-value-text-slots",
        "{0} is not a list of text slots",
    ),
    (
        "refusal-macro-fragment-unreadable",
        "the macro's fragment does not read: {0}",
    ),
    ("refusal-macro-no-value-set", "the macro has no value set `{0}`"),
    ("refusal-macro-no-variable", "the macro has no variable `{0}`"),
    ("refusal-macro-variable", "variable `{0}`: {1}"),
    (
        "refusal-macro-binds-missing",
        "variable `{0}` binds record {1}, which is not in the fragment",
    ),
    ("refusal-macro-record-no-kind", "record {0} has no kind"),
    (
        "refusal-macro-binds-field",
        "variable `{0}` binds `{1}`, which a {2} does not take",
    ),
    ("refusal-macro-field-misfit", "`{0}` does not fit record {1}"),
    (
        "refusal-macro-field-value",
        "`{0}` of record {1} does not take {2}: {3}",
    ),
    (
        "refusal-macro-group-no-variable",
        "group `{0}` is included by `{1}`, which is no variable",
    ),
    (
        "refusal-macro-group-not-bool",
        "group `{0}` is included by `{1}`, which is not a boolean",
    ),
    (
        "refusal-macro-value-set-none",
        "no value set `{0}`: the macro has none",
    ),
    (
        "refusal-macro-value-set-unknown",
        "no value set `{0}`: the macro has {1}",
    ),
    (
        "refusal-macro-kinds",
        "is not inserted (only devices, functions, representations, connections, junctions, channels, potential definitions, device links, page items and structure boxes)",
    ),
    // merge.resolve.
    (
        "refusal-merge-project-record",
        "the project record cannot be deleted; take the other side",
    ),
    (
        "refusal-merge-kind-kept",
        "theirs cannot be written: this version keeps records of the kind \"{0}\" unchanged; take ours",
    ),
    (
        "refusal-merge-invalid-record",
        "theirs does not make a valid record ({0}); take ours, or edit the record",
    ),
    (
        "refusal-merge-record-kept",
        "the record is of a kind this version keeps unchanged",
    ),
    (
        "refusal-merge-file-side",
        "the conflict of a file holds a side this version does not know; open the project in a newer version",
    ),
    (
        "refusal-merge-file-line",
        "line {0} of the file the merge could not read is not a record this version reads; take the other side, or fix the file",
    ),
    (
        "refusal-merge-chosen-twice",
        "an earlier choice names this conflict already",
    ),
    (
        "refusal-merge-no-conflict-record",
        "names no conflict record of the project",
    ),
    (
        "refusal-merge-none-chosen",
        "no conflict chosen; name at least one",
    ),
    // part.edit.
    ("refusal-part-edit-unchanged", "the part is unchanged"),
    ("refusal-part-edit-tech-value", "not a technical value: {0}"),
    ("refusal-part-edit-tech-twice", "{0} is given twice"),
    ("refusal-part-edit-not-part", "not a part item: {0}"),
    (
        "refusal-part-edit-copy-missing",
        "the project holds no part copy {0}",
    ),
    (
        "refusal-part-edit-fork-held",
        "the project holds item {0} already; mint another fork item",
    ),
    (
        "refusal-part-edit-manufacturer-missing",
        "the project holds no manufacturer {0}/{1} and `copies` does not carry it",
    ),
    (
        "refusal-part-edit-needs-fork",
        "a copy from a library is forked: give `fork`, the item ID of the project's copy",
    ),
    ("refusal-part-edit-no-manufacturer", "a part needs a manufacturer"),
    ("refusal-part-edit-no-number", "a part needs a part number"),
    (
        "refusal-part-edit-local-fork",
        "a project-local copy is edited in place; give no `fork`",
    ),
    (
        "refusal-part-edit-not-object",
        "not a part item: give the part's members as an object",
    ),
    (
        "refusal-part-edit-fixed-members",
        "only the identity, the description, the category, the unit and the technical data can be edited",
    ),
    (
        "refusal-part-edit-tech-data-list",
        "the technical data are a list",
    ),
    ("refusal-part-edit-unit", "the unit is piece, m or set"),
    // parts.import.
    (
        "refusal-parts-import-no-key",
        "a part copy without a key (the key is what a re-import matches on)",
    ),
    ("refusal-parts-import-kind", "not a part or manufacturer copy"),
    (
        "refusal-parts-import-not-local",
        "not a project-local copy (library_id must be the project ID); a copy of another library is embedded with library.embed",
    ),
    // The PLC configuration: plc.config_project.create, plc.bind_channel, plc.set_start, plc.set_module_part.
    (
        "refusal-plc-project-exists",
        "configuration project {0} exists already; names are unique",
    ),
    (
        "refusal-plc-channel-not-on-module",
        "{0} is not on module {1} or one of its children; a channel binds a pin of its module or the coil of a valve on it",
    ),
    (
        "refusal-plc-channel-no-cp",
        "{0} has no connection point number {1} ({2})",
    ),
    (
        "refusal-plc-address-not-module",
        "{0} is not a PLC module, and a start address belongs to a module; make it a module first",
    ),
    (
        "refusal-plc-not-module",
        "{0} is not a PLC module; make it a module first",
    ),
    (
        "refusal-channel-identity-taken",
        "channel {0} of module {1} cannot be made: channel {2} of module {3} already has its identity; delete that channel first",
    ),
    (
        "refusal-plc-profile-reference",
        "`{0}` is not a profile reference `id@version` (\"siemens-tia-intl@2027.1\")",
    ),
    (
        "refusal-plc-bind-endpoint",
        "a channel binds a CP (`{\"cp\": {\"function\", \"idx\"}}`), not a junction or a point on a connection",
    ),
    (
        "refusal-plc-profile-unknown",
        "`{0}` is not an address profile of this build or of the project; the profiles at hand are {1}",
    ),
    (
        "refusal-plc-module-part-use",
        "the module's `plc_module.part_use` is {0}, which names no entry of the {1} new part use(s); change `plc_module.part_use` first",
    ),
    ("refusal-plc-start-input", "an input"),
    ("refusal-plc-start-output", "an output"),
    (
        "refusal-plc-start-area",
        "`{0}` is not {1} start: {1} start is in area {2}, like {3}",
    ),
    // revision.create and revision.withdraw.
    (
        "refusal-revision-label-used",
        "another revision already has the label {0}; a label names one revision",
    ),
    (
        "refusal-revision-open",
        "revision {0} is open; at most one revision collects the current edits",
    ),
    (
        "refusal-revision-released",
        "revision {0} has a release record; a historic released revision cannot follow a release by ElektraWatt: a release goes through revision.release and its gate",
    ),
    ("refusal-revision-withdrawn", "revision {0} is withdrawn already"),
    (
        "refusal-revision-approve-no-author",
        "revision {0} has no author; four eyes compare two parties, so set the author first (Revision ▸ Set author)",
    ),
    (
        "refusal-revision-waiver-twice",
        "the waiver of {0} is listed twice",
    ),
    (
        "refusal-revision-release-no-sign-off",
        "revision {0} has no sign-off; the checker approves it first (Revision ▸ Approve, ew revision approve)",
    ),
    (
        "refusal-revision-archive-name",
        "`{0}` is not an archive file name (`<label>.ewarchive`, no directory)",
    ),
    ("refusal-revision-git-tag", "the tag of revision {0} is `{1}`"),
    ("refusal-revision-note-page-gone", "page {0} does not exist"),
    (
        "refusal-revision-note-twice",
        "{0} has a note already; one note per page",
    ),
    (
        "refusal-revision-closed",
        "revision {0} is not open; a released or withdrawn revision is immutable: make the change in the open revision",
    ),
    (
        "refusal-revision-withdraw-no-reason",
        "a withdrawal needs a reason",
    ),
    (
        "refusal-revision-author-is-checker",
        "the author is the checker's party; four eyes need another party: choose another author",
    ),
    (
        "refusal-revision-checker-is-author",
        "the checker is the author's party; four eyes need another party: choose another checker",
    ),
    (
        "refusal-revision-create-checker",
        "the checker of an open revision is written by approval (revision.approve), never when it is created: four eyes need one confirmed path per role; only a historic released revision takes a checker here",
    ),
    ("refusal-revision-state-open", "open"),
    ("refusal-revision-state-withdrawn", "a withdrawal record"),
    ("refusal-revision-state-other", "in the state {0}"),
    (
        "refusal-revision-withdraw-not-released",
        "revision {0} is {1}; only a released revision is withdrawn: choose a released one",
    ),
    // revision.approve, revision.release and project.assets.purge: the refusals of `ew-api`'s own (FT-1421).
    (
        "refusal-revision-approve-direct",
        "the approval is recorded only by Approve revision… on the saved project, which runs the release gate first",
    ),
    (
        "refusal-revision-release-direct",
        "the release is written only by Release revision…, after the release gate, the as-built archive and the git tag",
    ),
    (
        "refusal-purge-read-only",
        "the project is open read-only; a purge needs a project open for editing",
    ),
    (
        "refusal-purge-quarantined",
        "the project has quarantined lines in {0}; repair them before a purge",
    ),
    (
        "refusal-purge-unsaved",
        "the project has unsaved changes; save before a purge",
    ),
    (
        "refusal-purge-recovery",
        "a crashed session left unsaved changes in its journal; recover or discard them before a purge",
    ),
    (
        "refusal-purge-incomplete",
        "the last save is incomplete; it completes first",
    ),
    (
        "refusal-purge-external",
        "the project changed on disk; merge the change first",
    ),
    (
        "refusal-purge-stale",
        "{0} of the listed images can no longer be purged; preview again; nothing was removed",
    ),
    (
        "refusal-purge-stale-one",
        "1 of the listed images can no longer be purged; preview again; nothing was removed",
    ),
    (
        "refusal-purge-unreadable",
        "a file of the project could not be read; nothing was removed",
    ),
    (
        "refusal-purge-save",
        "the save that removes the images failed: {0}",
    ),
    (
        "refusal-purge-human-only",
        "the purge is human-only: confirm it in ElektraWatt or with ew project assets purge on a terminal; nothing was removed",
    ),
    // project.assets.purge's own parameters.
    (
        "refusal-purge-asset-hash",
        "an asset hash is `sha256:` and 64 lower-case hex digits",
    ),
    // The structure: structure.create_node, section.create, report_block.create and external_ref.create.
    (
        "refusal-structure-product-aspect",
        "the product aspect is the device tree, not a structure node; pick another aspect",
    ),
    (
        "refusal-structure-parent-aspect",
        "the parent belongs to another aspect; pick a parent of the same aspect",
    ),
    (
        "refusal-structure-code-used",
        "a sibling with code `{0}` already exists in this aspect; choose another code",
    ),
    (
        "refusal-section-range-order",
        "`folio_range` starts after it ends; give first <= last",
    ),
    (
        "refusal-section-number-used",
        "section number {0} already exists; choose another number",
    ),
    (
        "refusal-section-range-overlap",
        "folio range {0}-{1} overlaps section {2} ({3}-{4}); choose a free range",
    ),
    (
        "refusal-report-block-form-version",
        "the project holds another version of this form; take the new one with update from library",
    ),
    (
        "refusal-report-block-first-folio",
        "a page-oriented report block needs `first_folio`; give one or embed it",
    ),
    (
        "refusal-external-ref-not-location",
        "node {0} is not a location; an external reference sits at a location node",
    ),
    (
        "refusal-report-block-form-not-given",
        "`form` is {0}, which none of `form_copies` is",
    ),
    (
        "refusal-report-block-not-form",
        "not a library document of kind form",
    ),
    // project.supply.set.
    (
        "refusal-supply-none",
        "the project has no supply (a basic or macro project); there is nothing to set",
    ),
    (
        "refusal-supply-member-unknown",
        "`{0}` is not a settable supply member; settable: {1}",
    ),
    // terminal.add, terminal.insert, terminal.move and conductor.add.
    ("refusal-terminal-add-none", "give at least one terminal to add"),
    (
        "refusal-terminal-insert-splits-group",
        "inserting before terminal {0} would split bridge group {1} (terminals {2}); insert before terminal {3} or after terminal {4}, or delete the group's bridge there first",
    ),
    (
        "refusal-terminal-move-remedy-outside",
        "put the terminals before terminal {0} or after terminal {1}, or delete the group's bridge there first",
    ),
    (
        "refusal-terminal-move-remedy-remove",
        "remove terminal {0} from the group first (delete its bridges), or move the whole group",
    ),
    (
        "refusal-terminal-move-splits-group",
        "the move would split bridge group {0} (terminals {1}); {2}",
    ),
    (
        "refusal-conductor-not-cable",
        "the device is not a cable; pick a device with the cable aspect",
    ),
    ("refusal-conductor-add-none", "give at least one conductor to add"),
    (
        "refusal-conductor-core-used",
        "core {0} is already used in the cable; choose another",
    ),
    (
        "refusal-terminal-seq-overflow",
        "the sequence numbers would pass 4294967295; insert at a lower position",
    ),
    (
        "refusal-terminal-designation-twice",
        "designation {0} is given twice; choose another",
    ),
    ("refusal-terminal-move-twice", "terminal {0} is listed twice"),
    (
        "refusal-terminal-not-same-strip",
        "the device is not a terminal of the same strip",
    ),
    (
        "refusal-terminal-not-of-strip",
        "the device is not a terminal of the strip",
    ),
    ("refusal-terminal-not-terminal", "the device is not a terminal"),
    (
        "refusal-terminal-insert-seq",
        "the position follows from `before`; leave out `seq`",
    ),
    (
        "refusal-terminal-move-other-strip",
        "the terminal is of another strip; move the terminals of one strip at a time",
    ),
    (
        "refusal-terminal-move-before-moved",
        "the terminal to put the terminals before is one of the moved terminals; name a terminal that stays",
    ),
    (
        "refusal-terminal-move-unchanged",
        "the terminals are already there",
    ),
    // The normalisers: a split connection, a dissolving junction and a valve's functions.
    (
        "refusal-split-not-on-route",
        "the split point is not on the route of {0}; pick a point on the wire",
    ),
    (
        "refusal-split-second-not-on-route",
        "the second split point is not on the route of {0} after the first",
    ),
    (
        "refusal-junction-merge-numbers",
        "{0} would merge a wire {1} and a wire {2}, which carry different numbers; clear one of the numbers or delete the wires separately",
    ),
    ("refusal-junction-numbered", "numbered \"{0}\""),
    (
        "refusal-junction-numbered-assigned",
        "numbered \"{0}\" with assigned number {1}",
    ),
    ("refusal-junction-assigned", "with assigned number {0}"),
    ("refusal-junction-unnumbered", "without a number"),
    ("refusal-junction-number-newer", "with a number of a newer format"),
    (
        "refusal-valve-functions-no-id",
        "{0} cannot take the functions of part {1}: no free function ID is left; replace the device",
    ),
    // The imports and the external changes: import.apply and external.apply.
    ("refusal-import-unknown-kind", "a record of an unknown kind ({0})"),
    (
        "refusal-import-removed",
        "a normaliser removed a record the import brought: {0}",
    ),
    (
        "refusal-external-image",
        "an image of the change does not read ({0})",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A text writes its arguments in place, a text argument in English, and a key is in the table once.
    #[test]
    fn a_text_writes_its_arguments_in_english() {
        let t = text!("refusal-words-page", "101");
        assert_eq!(t.to_string(), "page 101");
        let t = text!(
            "refusal-connection-start-on-other-page",
            text!("refusal-words-page", "2"),
            text!("refusal-words-page", "3")
        );
        assert_eq!(
            t.to_string(),
            "the wire's start is drawn on page 2, not on page 3; start the wire on page 3"
        );
        let mut seen = std::collections::BTreeSet::new();
        let twice: Vec<&str> = TEXTS
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !seen.insert(*k))
            .collect();
        assert_eq!(twice, Vec::<&str>::new());
        assert_eq!(arity("a {0} b {2} {1}"), 3);
        assert_eq!(arity("no arguments"), 0);
    }

    /// C51-p5#7: a hand-built text (the macro refuses both at compile time) writes `?` for a missing argument and for
    /// a key that the table does not hold.
    #[test]
    fn a_hand_built_text_writes_a_question_mark_for_what_is_missing() {
        let short = Text::checked_by_macro("refusal-connection-start-on-other-page", vec!["page 2".into()]);
        assert_eq!(
            short.to_string(),
            "the wire's start is drawn on page 2, not on ?; start the wire on ?"
        );
        assert_eq!(Text::checked_by_macro("no-such-key", vec![]).to_string(), "?");
    }

    /// The `.rs` files under `dir`, recursively.
    fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// FT-1337 (ID-1359 ruling 1): no refusal of the crate goes without its key. `tx.reject` and `CmdError::rejected`
    /// take a [`Text`]; besides them, no source builds a `CmdError::Rejected` itself (with `source:`) or one without a
    /// text (`text: None`), and every key of [`TEXTS`] is named by the code. FT-1395 (ID-1415):
    /// `CmdError::invalid_params` and the helpers around it take a [`Text`] too, so no source builds a
    /// `CmdError::InvalidParams` itself (with `message:`), and none builds a [`Text`] but through `text!`.
    #[test]
    fn every_refusal_of_the_crate_is_keyed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        sources(&root, &mut files);
        files.sort();
        let mut unkeyed = Vec::new();
        let mut code = String::new();
        for file in files {
            let name = file
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&file).unwrap();
            if name == "text.rs" {
                continue;
            }
            code.push_str(&text);
            if name == "error.rs" {
                continue;
            }
            for (at, _) in text.match_indices("Rejected {") {
                let fields = &text[at..at + text[at..].find('}').unwrap_or(text.len() - at)];
                if fields.contains("source:") || fields.contains("text: None") {
                    let line = text[..at].lines().count();
                    unkeyed.push(format!("{name}:{line}"));
                }
            }
            for (at, _) in text.match_indices("InvalidParams {") {
                let fields = &text[at..at + text[at..].find('}').unwrap_or(text.len() - at)];
                if fields.contains("message:") {
                    let line = text[..at].lines().count();
                    unkeyed.push(format!("{name}:{line}"));
                }
            }
            for (i, line) in text.lines().enumerate() {
                let line = line.trim_start();
                if !line.starts_with("//")
                    && (line.contains(".reject(format!")
                        || line.contains(".reject(\"")
                        || line.contains("checked_by_macro"))
                {
                    unkeyed.push(format!("{name}:{}", i + 1));
                }
            }
        }
        assert_eq!(unkeyed, Vec::<String>::new());
        // FT-1421: the keys of `ew-api`'s own refusals are named there.
        let mut api = Vec::new();
        sources(&root.join("../../ew-api/src"), &mut api);
        for file in api {
            code.push_str(&std::fs::read_to_string(&file).unwrap());
        }
        let unused: Vec<&str> = TEXTS
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !code.contains(&format!("\"{k}\"")))
            .collect();
        assert_eq!(unused, Vec::<&str>::new());
    }
}
