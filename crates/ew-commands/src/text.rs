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
    // assembly.attach, valve.add and device.link.
    (
        "refusal-assembly-position-twice",
        "position {0} is given to two children ({1} and {2})",
    ),
    (
        "refusal-assembly-cycle",
        "device {0} is an ancestor of the parent {1}; the assembly graph is acyclic (EW-INV-009)",
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
        "device {0} already links to {1} with this kind (link {2})",
    ),
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
    // bus.logical_net.create and bus.physical_net.create.
    (
        "refusal-bus-logical-net-exists",
        "logical net {0} exists already; names are unique",
    ),
    (
        "refusal-bus-physical-net-exists",
        "physical net {0} exists already; names are unique",
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
    // check.waive.
    (
        "refusal-check-waive-twice",
        "waiver {0} already covers the finding {1} of {2}",
    ),
    // connection.draw, connection.draw_many and connection.delete.
    (
        "refusal-connection-no-page",
        "connection {0} has no page, so a junction cannot be placed on it; give it a page first",
    ),
    (
        "refusal-connection-same-end",
        "both ends are the same endpoint (CI-01); pick two different ends",
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
        "representation type {0} is not allowed on a {1} page ({2}); allowed: {3}",
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
    // fields.set.
    ("refusal-fields-ip-star", "{0}: {1}"),
    ("refusal-fields-not-fluid-line", "{0} is not a fluid line"),
    (
        "refusal-fields-counter-used",
        "counter {0} with letter {1} is already used on {2} by another device; use {3}, the next free counter",
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
        "report pages are generated from report blocks (EW-INV-012); create a report block",
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
        "the update leaves connected CPs without a CP; map them in `cp_maps` (doc 07 §4.5): {0}",
    ),
    (
        "refusal-library-update-missing",
        "the new versions name items the project does not hold; give them in `embed` (FT-640): {0}",
    ),
    (
        "refusal-library-update-overrides",
        "the mapping moves two CP overrides of function {0} onto CP {1}; change the overrides first",
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
    // part.edit.
    ("refusal-part-edit-unchanged", "the part is unchanged"),
    // The PLC configuration: plc.config_project.create, plc.bind_channel, plc.set_start, plc.set_module_part.
    (
        "refusal-plc-project-exists",
        "configuration project {0} exists ({1}); names are unique",
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
    // revision.create and revision.withdraw.
    (
        "refusal-revision-label-used",
        "revision {0} already has the label {1}; a label names one revision",
    ),
    (
        "refusal-revision-open",
        "revision {0} ({1}) is open; at most one revision collects the current edits",
    ),
    (
        "refusal-revision-released",
        "revision {0} ({1}) has a release record; a historic released revision cannot follow a release by ElektraWatt: a release goes through revision.release and its gate",
    ),
    (
        "refusal-revision-withdrawn",
        "revision {0} ({1}) is withdrawn already by the withdrawal record {2}",
    ),
    // The structure: structure.create_node, section.create, report_block.create and external_ref.create.
    (
        "refusal-structure-product-aspect",
        "the product aspect is the device tree, not a structure node; pick another aspect",
    ),
    (
        "refusal-structure-parent-aspect",
        "the parent belongs to another aspect (EW-INV-011); pick a parent of the same aspect",
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
        "a page-oriented report block needs `first_folio` (EW-INV-014); give one or embed it",
    ),
    (
        "refusal-external-ref-not-location",
        "node {0} is not a location; an external reference sits at a location node",
    ),
    // project.supply.set.
    (
        "refusal-supply-none",
        "the project has no supply (a basic or macro project); there is nothing to set",
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
    // The normalisers: a split connection, a dissolving junction and a valve's functions.
    (
        "refusal-split-not-on-route",
        "the split point is not on the route of connection {0}; pick a point on the wire",
    ),
    (
        "refusal-split-second-not-on-route",
        "the second split point is not on the route of connection {0} after the first",
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
        "a normaliser removed the imported {0} {1}",
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
    /// text (`text: None`), and every key of [`TEXTS`] is named by the code.
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
            for (i, line) in text.lines().enumerate() {
                let line = line.trim_start();
                if !line.starts_with("//")
                    && (line.contains(".reject(format!") || line.contains(".reject(\""))
                {
                    unkeyed.push(format!("{name}:{}", i + 1));
                }
            }
        }
        assert_eq!(unkeyed, Vec::<String>::new());
        let unused: Vec<&str> = TEXTS
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| !code.contains(&format!("\"{k}\"")))
            .collect();
        assert_eq!(unused, Vec::<&str>::new());
    }
}
