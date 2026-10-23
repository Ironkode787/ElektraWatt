// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The starter library (WP-S2.04 D8, §4.3): the shipped symbols and function definitions of `content/symbols`,
//! `content/symbols/qet10` and `content/fndefs`, compiled in, so that a new project without symbol copies can place
//! symbols. Each item is one library document in the `lib/` JSON form that `library.embed` takes (doc 05 §13.2);
//! [`items`] lists them in key order, the key being the file stem (`ew.iec.mcb-1p@qet10`, `ew.fn.mcb-1p`).
//!
//! The picker of the place tool lists the starter symbols a project has not embedded; choosing one embeds the symbol
//! and its function definition through `library.embed` (the host's commit, an undo step of its own).

/// What a starter item is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StarterKind {
    /// A symbol (`$k` `symbol`).
    Symbol,
    /// A function definition (`$k` `fndef`).
    Fndef,
}

/// One starter item: its key (the file stem), its kind and its library document as JSON text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StarterItem {
    /// The key, the file stem (`ew.iec.mcb-1p@qet10`).
    pub key: &'static str,
    /// Symbol or function definition.
    pub kind: StarterKind,
    /// The library document (`lib/` JSON form).
    pub json: &'static str,
}

macro_rules! item {
    ($key:literal, $kind:ident, $path:literal) => {
        StarterItem {
            key: $key,
            kind: StarterKind::$kind,
            json: include_str!(concat!("../../../content/", $path)),
        }
    };
}

/// The items in key order (byte order of the keys).
const ITEMS: &[StarterItem] = &[
    item!("ew.bus.dp-connector", Symbol, "symbols/ew.bus.dp-connector.json"),
    item!(
        "ew.bus.dp-connector@qet10",
        Symbol,
        "symbols/qet10/ew.bus.dp-connector@qet10.json"
    ),
    item!("ew.bus.dp-port", Symbol, "symbols/ew.bus.dp-port.json"),
    item!(
        "ew.bus.dp-port@qet10",
        Symbol,
        "symbols/qet10/ew.bus.dp-port@qet10.json"
    ),
    item!("ew.fn.blackbox", Fndef, "fndefs/ew.fn.blackbox.json"),
    item!("ew.fn.coil", Fndef, "fndefs/ew.fn.coil.json"),
    item!("ew.fn.contact-co", Fndef, "fndefs/ew.fn.contact-co.json"),
    item!("ew.fn.contact-nc", Fndef, "fndefs/ew.fn.contact-nc.json"),
    item!("ew.fn.contact-no", Fndef, "fndefs/ew.fn.contact-no.json"),
    item!(
        "ew.fn.cylinder-double-acting",
        Fndef,
        "fndefs/ew.fn.cylinder-double-acting.json"
    ),
    item!("ew.fn.dp-connector", Fndef, "fndefs/ew.fn.dp-connector.json"),
    item!("ew.fn.dp-port", Fndef, "fndefs/ew.fn.dp-port.json"),
    item!("ew.fn.external-arrow", Fndef, "fndefs/ew.fn.external-arrow.json"),
    item!("ew.fn.fe", Fndef, "fndefs/ew.fn.fe.json"),
    item!("ew.fn.fuse", Fndef, "fndefs/ew.fn.fuse.json"),
    item!("ew.fn.graphic", Fndef, "fndefs/ew.fn.graphic.json"),
    item!("ew.fn.heater", Fndef, "fndefs/ew.fn.heater.json"),
    item!("ew.fn.ip", Fndef, "fndefs/ew.fn.ip.json"),
    item!("ew.fn.lamp", Fndef, "fndefs/ew.fn.lamp.json"),
    item!("ew.fn.mcb-1p", Fndef, "fndefs/ew.fn.mcb-1p.json"),
    item!("ew.fn.mcb-1pn", Fndef, "fndefs/ew.fn.mcb-1pn.json"),
    item!("ew.fn.motor-3ph", Fndef, "fndefs/ew.fn.motor-3ph.json"),
    item!("ew.fn.overview", Fndef, "fndefs/ew.fn.overview.json"),
    item!("ew.fn.pe", Fndef, "fndefs/ew.fn.pe.json"),
    item!("ew.fn.plug-pin", Fndef, "fndefs/ew.fn.plug-pin.json"),
    item!(
        "ew.fn.position-switch-no",
        Fndef,
        "fndefs/ew.fn.position-switch-no.json"
    ),
    item!(
        "ew.fn.process-valve-2way",
        Fndef,
        "fndefs/ew.fn.process-valve-2way.json"
    ),
    item!("ew.fn.pushbutton-no", Fndef, "fndefs/ew.fn.pushbutton-no.json"),
    item!("ew.fn.resistor", Fndef, "fndefs/ew.fn.resistor.json"),
    item!("ew.fn.strip-segment", Fndef, "fndefs/ew.fn.strip-segment.json"),
    item!(
        "ew.fn.switch-2p-manual",
        Fndef,
        "fndefs/ew.fn.switch-2p-manual.json"
    ),
    item!("ew.fn.terminal", Fndef, "fndefs/ew.fn.terminal.json"),
    item!("ew.fn.terminal-pe", Fndef, "fndefs/ew.fn.terminal-pe.json"),
    item!("ew.fn.thermostat-nc", Fndef, "fndefs/ew.fn.thermostat-nc.json"),
    item!(
        "ew.fn.transformer-1ph",
        Fndef,
        "fndefs/ew.fn.transformer-1ph.json"
    ),
    item!(
        "ew.fn.valve-5-2-sol-spring",
        Fndef,
        "fndefs/ew.fn.valve-5-2-sol-spring.json"
    ),
    item!("ew.gen.blackbox", Symbol, "symbols/ew.gen.blackbox.json"),
    item!(
        "ew.gen.blackbox@qet10",
        Symbol,
        "symbols/qet10/ew.gen.blackbox@qet10.json"
    ),
    item!("ew.gen.cable-marker", Symbol, "symbols/ew.gen.cable-marker.json"),
    item!(
        "ew.gen.cable-marker@qet10",
        Symbol,
        "symbols/qet10/ew.gen.cable-marker@qet10.json"
    ),
    item!(
        "ew.gen.external-arrow",
        Symbol,
        "symbols/ew.gen.external-arrow.json"
    ),
    item!(
        "ew.gen.external-arrow@qet10",
        Symbol,
        "symbols/qet10/ew.gen.external-arrow@qet10.json"
    ),
    item!("ew.gen.ip", Symbol, "symbols/ew.gen.ip.json"),
    item!("ew.gen.ip@qet10", Symbol, "symbols/qet10/ew.gen.ip@qet10.json"),
    item!(
        "ew.gen.junction-bevel",
        Symbol,
        "symbols/ew.gen.junction-bevel.json"
    ),
    item!(
        "ew.gen.junction-bevel@qet10",
        Symbol,
        "symbols/qet10/ew.gen.junction-bevel@qet10.json"
    ),
    item!("ew.gen.junction-dot", Symbol, "symbols/ew.gen.junction-dot.json"),
    item!(
        "ew.gen.junction-dot@qet10",
        Symbol,
        "symbols/qet10/ew.gen.junction-dot@qet10.json"
    ),
    item!(
        "ew.gen.strip-segment",
        Symbol,
        "symbols/ew.gen.strip-segment.json"
    ),
    item!(
        "ew.gen.strip-segment@qet10",
        Symbol,
        "symbols/qet10/ew.gen.strip-segment@qet10.json"
    ),
    item!("ew.gen.wire-marker", Symbol, "symbols/ew.gen.wire-marker.json"),
    item!(
        "ew.gen.wire-marker@qet10",
        Symbol,
        "symbols/qet10/ew.gen.wire-marker@qet10.json"
    ),
    item!("ew.iec.coil", Symbol, "symbols/ew.iec.coil.json"),
    item!(
        "ew.iec.coil@qet10",
        Symbol,
        "symbols/qet10/ew.iec.coil@qet10.json"
    ),
    item!("ew.iec.contact-co", Symbol, "symbols/ew.iec.contact-co.json"),
    item!(
        "ew.iec.contact-co@qet10",
        Symbol,
        "symbols/qet10/ew.iec.contact-co@qet10.json"
    ),
    item!("ew.iec.contact-nc", Symbol, "symbols/ew.iec.contact-nc.json"),
    item!(
        "ew.iec.contact-nc@qet10",
        Symbol,
        "symbols/qet10/ew.iec.contact-nc@qet10.json"
    ),
    item!("ew.iec.contact-no", Symbol, "symbols/ew.iec.contact-no.json"),
    item!(
        "ew.iec.contact-no@qet10",
        Symbol,
        "symbols/qet10/ew.iec.contact-no@qet10.json"
    ),
    item!("ew.iec.fe", Symbol, "symbols/ew.iec.fe.json"),
    item!("ew.iec.fe@qet10", Symbol, "symbols/qet10/ew.iec.fe@qet10.json"),
    item!("ew.iec.fuse", Symbol, "symbols/ew.iec.fuse.json"),
    item!(
        "ew.iec.fuse@qet10",
        Symbol,
        "symbols/qet10/ew.iec.fuse@qet10.json"
    ),
    item!("ew.iec.heater", Symbol, "symbols/ew.iec.heater.json"),
    item!(
        "ew.iec.heater@qet10",
        Symbol,
        "symbols/qet10/ew.iec.heater@qet10.json"
    ),
    item!("ew.iec.lamp", Symbol, "symbols/ew.iec.lamp.json"),
    item!(
        "ew.iec.lamp@qet10",
        Symbol,
        "symbols/qet10/ew.iec.lamp@qet10.json"
    ),
    item!("ew.iec.mcb-1p", Symbol, "symbols/ew.iec.mcb-1p.json"),
    item!(
        "ew.iec.mcb-1p@qet10",
        Symbol,
        "symbols/qet10/ew.iec.mcb-1p@qet10.json"
    ),
    item!("ew.iec.mcb-1pn", Symbol, "symbols/ew.iec.mcb-1pn.json"),
    item!(
        "ew.iec.mcb-1pn@qet10",
        Symbol,
        "symbols/qet10/ew.iec.mcb-1pn@qet10.json"
    ),
    item!("ew.iec.motor-3ph", Symbol, "symbols/ew.iec.motor-3ph.json"),
    item!(
        "ew.iec.motor-3ph@qet10",
        Symbol,
        "symbols/qet10/ew.iec.motor-3ph@qet10.json"
    ),
    item!("ew.iec.pe", Symbol, "symbols/ew.iec.pe.json"),
    item!("ew.iec.pe@qet10", Symbol, "symbols/qet10/ew.iec.pe@qet10.json"),
    item!("ew.iec.plug-pin", Symbol, "symbols/ew.iec.plug-pin.json"),
    item!(
        "ew.iec.plug-pin@qet10",
        Symbol,
        "symbols/qet10/ew.iec.plug-pin@qet10.json"
    ),
    item!(
        "ew.iec.position-switch-no",
        Symbol,
        "symbols/ew.iec.position-switch-no.json"
    ),
    item!(
        "ew.iec.position-switch-no@qet10",
        Symbol,
        "symbols/qet10/ew.iec.position-switch-no@qet10.json"
    ),
    item!(
        "ew.iec.pushbutton-no",
        Symbol,
        "symbols/ew.iec.pushbutton-no.json"
    ),
    item!(
        "ew.iec.pushbutton-no@qet10",
        Symbol,
        "symbols/qet10/ew.iec.pushbutton-no@qet10.json"
    ),
    item!("ew.iec.resistor", Symbol, "symbols/ew.iec.resistor.json"),
    item!(
        "ew.iec.resistor@qet10",
        Symbol,
        "symbols/qet10/ew.iec.resistor@qet10.json"
    ),
    item!(
        "ew.iec.switch-2p-manual",
        Symbol,
        "symbols/ew.iec.switch-2p-manual.json"
    ),
    item!(
        "ew.iec.switch-2p-manual@qet10",
        Symbol,
        "symbols/qet10/ew.iec.switch-2p-manual@qet10.json"
    ),
    item!("ew.iec.terminal", Symbol, "symbols/ew.iec.terminal.json"),
    item!("ew.iec.terminal-pe", Symbol, "symbols/ew.iec.terminal-pe.json"),
    item!(
        "ew.iec.terminal-pe@qet10",
        Symbol,
        "symbols/qet10/ew.iec.terminal-pe@qet10.json"
    ),
    item!(
        "ew.iec.terminal@qet10",
        Symbol,
        "symbols/qet10/ew.iec.terminal@qet10.json"
    ),
    item!(
        "ew.iec.thermostat-nc",
        Symbol,
        "symbols/ew.iec.thermostat-nc.json"
    ),
    item!(
        "ew.iec.thermostat-nc@qet10",
        Symbol,
        "symbols/qet10/ew.iec.thermostat-nc@qet10.json"
    ),
    item!(
        "ew.iec.transformer-1ph",
        Symbol,
        "symbols/ew.iec.transformer-1ph.json"
    ),
    item!(
        "ew.iec.transformer-1ph@qet10",
        Symbol,
        "symbols/qet10/ew.iec.transformer-1ph@qet10.json"
    ),
    item!(
        "ew.iso.cylinder-double-acting",
        Symbol,
        "symbols/ew.iso.cylinder-double-acting.json"
    ),
    item!(
        "ew.iso.cylinder-double-acting@qet10",
        Symbol,
        "symbols/qet10/ew.iso.cylinder-double-acting@qet10.json"
    ),
    item!(
        "ew.iso.process-valve-2way",
        Symbol,
        "symbols/ew.iso.process-valve-2way.json"
    ),
    item!(
        "ew.iso.process-valve-2way@qet10",
        Symbol,
        "symbols/qet10/ew.iso.process-valve-2way@qet10.json"
    ),
    item!(
        "ew.iso.valve-5-2-sol-spring",
        Symbol,
        "symbols/ew.iso.valve-5-2-sol-spring.json"
    ),
    item!(
        "ew.iso.valve-5-2-sol-spring@qet10",
        Symbol,
        "symbols/qet10/ew.iso.valve-5-2-sol-spring@qet10.json"
    ),
    item!("ew.ovw.module", Symbol, "symbols/ew.ovw.module.json"),
    item!(
        "ew.ovw.module@qet10",
        Symbol,
        "symbols/qet10/ew.ovw.module@qet10.json"
    ),
    item!("ew.ovw.valve-top", Symbol, "symbols/ew.ovw.valve-top.json"),
    item!(
        "ew.ovw.valve-top@qet10",
        Symbol,
        "symbols/qet10/ew.ovw.valve-top@qet10.json"
    ),
];

/// Every starter item, in key order.
pub fn items() -> &'static [StarterItem] {
    ITEMS
}

/// The starter item with this key.
pub fn item(key: &str) -> Option<&'static StarterItem> {
    ITEMS.binary_search_by(|i| i.key.cmp(key)).ok().map(|i| &ITEMS[i])
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use ew_schema::LibDocument;

    use super::*;

    fn stems(dir: &str) -> Vec<String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../content")
            .join(dir);
        let mut out: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| {
                let name = e.unwrap().file_name().to_string_lossy().into_owned();
                name.strip_suffix(".json").map(str::to_owned)
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn every_content_file_is_an_item_in_key_order() {
        let mut expected: Vec<String> = stems("symbols");
        expected.extend(stems("symbols/qet10"));
        expected.extend(stems("fndefs"));
        expected.sort();
        let keys: Vec<&str> = items().iter().map(|i| i.key).collect();
        assert_eq!(keys, expected);
        assert_eq!(items().len(), 104);
        assert!(keys.windows(2).all(|w| w[0] < w[1]), "key order");
    }

    #[test]
    fn every_item_parses_as_the_library_document_library_embed_takes() {
        for i in items() {
            let doc = LibDocument::from_json(i.json).unwrap_or_else(|e| panic!("{}: {e}", i.key));
            let key = match (i.kind, &doc) {
                (StarterKind::Symbol, LibDocument::Symbol(c)) => c.item.meta.key.clone(),
                (StarterKind::Fndef, LibDocument::Fndef(c)) => c.item.meta.key.clone(),
                _ => panic!("{}: kind", i.key),
            };
            assert_eq!(key.as_ref().map(|k| k.as_str()), Some(i.key));
        }
        assert_eq!(
            item("ew.iec.mcb-1p@qet10").map(|i| i.kind),
            Some(StarterKind::Symbol)
        );
        assert_eq!(item("ew.fn.mcb-1p").map(|i| i.kind), Some(StarterKind::Fndef));
        assert!(item("no.such.key").is_none());
    }
}
