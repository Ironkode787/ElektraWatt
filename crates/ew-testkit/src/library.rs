// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A synthetic parts library (WP-S1.19 §7.4 IX-9, NFR-PERF-21): `n` parts and their manufacturers as library
//! documents of one library, deterministic for a given `n` and library ID. `ew-testkit` has no edge to `ew-library`;
//! the caller writes the documents with `Registry::write` (through `ew_api::library`).

use ew_ids::Uuid;
use ew_schema::LibDocument;
use serde_json::json;

/// The number of manufacturers of [`generated_parts`] (fewer when `n` is smaller).
pub const MANUFACTURERS: usize = 40;

const HASH: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

const WORDS: [&str; 24] = [
    "miniature",
    "circuit",
    "breaker",
    "contactor",
    "relay",
    "terminal",
    "block",
    "power",
    "supply",
    "fuse",
    "holder",
    "motor",
    "starter",
    "switch",
    "push",
    "button",
    "lamp",
    "sensor",
    "cable",
    "duct",
    "rail",
    "transformer",
    "meter",
    "socket",
];

const KINDS: [&str; 4] = ["device", "device", "device", "terminal"];

const CATEGORIES: [[&str; 3]; 4] = [
    ["ProtectionOvercurrent", "MCB", "1P+N"],
    ["ProtectionOvercurrent", "MCB", "1P"],
    ["Switching", "Contactor", "3P"],
    ["Supply", "PowerSupply", "DC"],
];

/// The item ID of the `i`-th generated part of `library`.
pub fn part_item(library: Uuid, i: usize) -> Uuid {
    Uuid::new_v5(&library, format!("ew-testkit-part-{i}").as_bytes())
}

/// The item ID of the `i`-th generated manufacturer of `library`.
pub fn manufacturer_item(library: Uuid, i: usize) -> Uuid {
    Uuid::new_v5(&library, format!("ew-testkit-manufacturer-{i}").as_bytes())
}

/// The generated part number of the `i`-th part: a two-letter series, seven digits and a rating suffix
/// (`AB0000123C16`), unique per `i`.
pub fn part_number(i: usize) -> String {
    let a = char::from(b'A' + u8::try_from(i % 26).unwrap_or(0));
    let b = char::from(b'A' + u8::try_from((i / 26) % 26).unwrap_or(0));
    let rating = [6, 10, 13, 16, 20, 25, 32][i % 7];
    format!("{a}{b}{i:07}C{rating}")
}

/// `n` parts and `min(n, MANUFACTURERS)` manufacturers of `library`, manufacturers first; the same output for the
/// same arguments on every platform.
pub fn generated_parts(n: usize, library: Uuid) -> Vec<LibDocument> {
    let lib = library.hyphenated().to_string();
    let m = n.clamp(1, MANUFACTURERS);
    let mut docs = Vec::with_capacity(n + m);
    for j in 0..m {
        let item = manufacturer_item(library, j).hyphenated().to_string();
        let value = json!({
            "$k": "manufacturer", "id": item, "library_id": lib, "item_id": item, "version": "1.0.0", "sha256": HASH,
            "key": format!("M{j:02}"), "name": format!("Manufacturer {j:02}"), "aliases": [format!("Maker{j:02}")],
        });
        docs.push(LibDocument::from_value(value).expect("a generated manufacturer is valid"));
    }
    for i in 0..n {
        let item = part_item(library, i).hyphenated().to_string();
        let mfr = manufacturer_item(library, i % m).hyphenated().to_string();
        let words: Vec<&str> = (0..4).map(|k| WORDS[(i * 7 + k * 5) % WORDS.len()]).collect();
        let value = json!({
            "$k": "part", "id": item, "library_id": lib, "item_id": item, "version": "1.0.0", "sha256": HASH,
            "meta": {"licence": "CC0-1.0", "status": "released"},
            "kind": KINDS[i % KINDS.len()],
            "manufacturer": {"library": lib, "item": mfr, "version_req": "^1"},
            "part_number": part_number(i),
            "description": format!("{} {i}", words.join(" ")),
            "category": CATEGORIES[i % CATEGORIES.len()],
        });
        docs.push(LibDocument::from_value(value).expect("a generated part is valid"));
    }
    docs
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_schema::Kind;

    #[test]
    fn the_generator_is_deterministic_and_valid() {
        let lib = Uuid::from_u128(0x1234);
        let a = generated_parts(100, lib);
        assert_eq!(a.len(), 100 + MANUFACTURERS);
        assert_eq!(a, generated_parts(100, lib));
        let parts: Vec<&LibDocument> = a.iter().filter(|d| d.kind() == Some(Kind::Part)).collect();
        assert_eq!(parts.len(), 100);
        let mut numbers: Vec<String> = (0..100).map(part_number).collect();
        numbers.sort();
        numbers.dedup();
        assert_eq!(numbers.len(), 100, "unique part numbers");
        assert_eq!(part_number(27), "BB0000027C32");
        assert_eq!(generated_parts(3, lib).len(), 6);
        assert!(
            generated_parts(0, lib)
                .iter()
                .all(|d| d.kind() == Some(Kind::Manufacturer))
        );
    }
}
