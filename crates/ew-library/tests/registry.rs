// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 2's acceptance (§7.4, IX-1 to IX-8): tiers, the `.ewlib` reader and writer, the lock, the index and
//! search, over fixture libraries in a fresh temporary directory.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ew_format::ew_schema::{ContactKind, ItemStatus, Kind, LibDocument, LibRef, PartKind};
use ew_ids::Uuid;
use ew_library::lock::{Identity, LOCK};
use ew_library::{
    FnCompat, INDEX_FILE, IndexState, ItemQuery, LibManifest, LibraryError, LibraryTiers, LockPolicy,
    PartHit, PartQuery, Registry, Tier, WriteFault, starter,
};
use serde_json::{Value, json};

const ZERO: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

/// A fresh root with the four tiers and the cache under it.
struct Fixture {
    root: PathBuf,
    tiers: LibraryTiers,
}

impl Fixture {
    fn new(stem: &str) -> Fixture {
        let root = ew_testdir::fresh_dir(stem);
        let tiers = LibraryTiers {
            system: vec![root.join("system")],
            community: root.join("data").join("packs"),
            company: vec![root.join("company")],
            user: root.join("data").join("libraries"),
            cache: root.join("cache"),
        };
        Fixture { root, tiers }
    }

    fn open(&self) -> Registry {
        let mut r = Registry::open(&self.tiers).unwrap();
        r.set_lock_policy(LockPolicy {
            stale_after: Duration::from_secs(60),
            settle: Duration::from_millis(10),
        });
        r
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn starter_item(key: &str) -> Uuid {
    let doc = LibDocument::from_json(starter::item(key).unwrap().json).unwrap();
    match doc {
        LibDocument::Fndef(c) => c.item_id.as_uuid(),
        LibDocument::Symbol(c) => c.item_id.as_uuid(),
        _ => unreachable!(),
    }
}

fn libref(lib: Uuid, item: Uuid, req: &str) -> Value {
    json!({"library": lib.to_string(), "item": item.to_string(), "version_req": req})
}

fn manufacturer(lib: Uuid, item: Uuid, key: &str, name: &str, aliases: &[&str]) -> LibDocument {
    LibDocument::from_value(json!({
        "$k": "manufacturer", "id": item.to_string(), "library_id": lib.to_string(),
        "item_id": item.to_string(), "version": "1.0.0", "sha256": ZERO,
        "key": key, "name": name, "aliases": aliases,
    }))
    .unwrap()
}

struct P<'a> {
    item: Uuid,
    version: &'a str,
    pn: &'a str,
    mfr: Uuid,
    descr: &'a str,
    kind: &'a str,
    category: Option<[&'a str; 3]>,
    fndef: Option<&'a str>,
    status: Value,
}

impl<'a> P<'a> {
    fn new(item: Uuid, pn: &'a str, mfr: Uuid, descr: &'a str) -> P<'a> {
        P {
            item,
            version: "1.0.0",
            pn,
            mfr,
            descr,
            kind: "device",
            category: None,
            fndef: None,
            status: json!("released"),
        }
    }

    fn doc(&self, lib: Uuid) -> LibDocument {
        let mut v = json!({
            "$k": "part", "id": self.item.to_string(), "library_id": lib.to_string(),
            "item_id": self.item.to_string(), "version": self.version, "sha256": ZERO,
            "meta": {"licence": "CC0-1.0", "status": self.status},
            "kind": self.kind,
            "manufacturer": libref(lib, self.mfr, "^1"),
            "part_number": self.pn,
            "description": self.descr,
        });
        if let Some(c) = self.category {
            v["category"] = json!(c);
        }
        if let Some(f) = self.fndef {
            v["function_template"] =
                json!([{"fndef": libref(starter::LIBRARY, starter_item(f), "^0.1"), "main": true}]);
        }
        LibDocument::from_value(v).unwrap()
    }
}

const SE: u128 = 0x5e;
const PXC: u128 = 0x9c;
const A9P52610: u128 = 0x0001_0010;
const A9P52616: u128 = 0x0002_0010;
const A9F78116: u128 = 0x0003_0011;
const PSU: u128 = 0x0004_0012;
const TERMINAL: u128 = 0x0005_0013;
const CONTACTOR: u128 = 0x0006_0014;
const OLD: u128 = 0x0007_0015;
const GONE: u128 = 0x0008_0016;
const TIE_X10: u128 = 0x0009_0017;
const TIE_X2: u128 = 0x000a_0018;

const MCB: [&str; 3] = ["ProtectionOvercurrent", "MCB", "1P+N"];

fn parts(lib: Uuid) -> Vec<LibDocument> {
    let mut v = vec![
        manufacturer(lib, id(SE), "SE", "Schneider Electric", &["Schneider"]),
        manufacturer(lib, id(PXC), "PXC", "Phoenix Contact", &[]),
    ];
    let mut add = |p: P<'_>| v.push(p.doc(lib));
    add(P {
        category: Some(MCB),
        fndef: Some("ew.fn.mcb-1pn"),
        ..P::new(
            id(A9P52610),
            "A9P52610",
            id(SE),
            "Miniature circuit breaker 1P+N C10",
        )
    });
    add(P {
        category: Some(MCB),
        fndef: Some("ew.fn.mcb-1pn"),
        ..P::new(
            id(A9P52616),
            "A9P52616",
            id(SE),
            "Miniature circuit breaker 1P+N C16",
        )
    });
    add(P {
        category: Some(["ProtectionOvercurrent", "MCB", "1P"]),
        fndef: Some("ew.fn.mcb-1p"),
        ..P::new(
            id(A9F78116),
            "A9F78116",
            id(SE),
            "Miniature circuit breaker 1P C16",
        )
    });
    add(P::new(id(PSU), "2904602", id(PXC), "Power supply 24 V DC"));
    add(P {
        kind: "terminal",
        ..P::new(id(TERMINAL), "3044102", id(PXC), "Feed-through terminal block")
    });
    add(P {
        fndef: Some("ew.fn.coil"),
        ..P::new(id(CONTACTOR), "LC1D09", id(SE), "Contactor 9 A")
    });
    add(P {
        status: json!({"deprecated": {"successor": libref(lib, id(A9P52610), "^1")}}),
        ..P::new(id(OLD), "A9N00001", id(SE), "Old circuit breaker")
    });
    add(P {
        status: json!("withdrawn"),
        ..P::new(id(GONE), "A9N00002", id(SE), "Withdrawn circuit breaker")
    });
    add(P::new(id(TIE_X10), "X10", id(PXC), "Equal tie part"));
    add(P::new(id(TIE_X2), "X2", id(PXC), "Equal tie part"));
    v
}

/// A company library with the fixture parts; returns its ID.
fn company(f: &Fixture, reg: &mut Registry) -> Uuid {
    let dir = f.root.join("company").join("company-std.ewlib");
    let lib = reg
        .create(&dir, "company-std", "Company standard", "CC0-1.0")
        .unwrap();
    let report = reg.write(lib, &parts(lib)).unwrap();
    assert_eq!(report.generation, 2);
    assert_eq!(report.added.len(), 12);
    lib
}

fn search(reg: &Registry, text: &str) -> Vec<PartHit> {
    reg.search(&PartQuery {
        text: text.into(),
        ..PartQuery::default()
    })
}

fn numbers(hits: &[PartHit]) -> Vec<&str> {
    hits.iter().map(|h| h.part_number.as_str()).collect()
}

/// IX-8, the first check: FTS5 and the trigram tokenizer are in the bundled SQLite.
#[test]
fn ix8_fts5_and_the_trigram_tokenizer_are_available() {
    let c = rusqlite::Connection::open_in_memory().unwrap();
    let v: String = c.query_row("SELECT sqlite_version()", [], |r| r.get(0)).unwrap();
    assert!(
        v.as_str() >= "3.34",
        "the trigram tokenizer needs SQLite 3.34: {v}"
    );
    c.execute_batch(
        "CREATE VIRTUAL TABLE t USING fts5(a, tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3');
         CREATE VIRTUAL TABLE g USING fts5(a, tokenize = 'trigram');
         INSERT INTO t VALUES ('miniature circuit breaker');
         INSERT INTO g VALUES ('A9P52610');",
    )
    .unwrap();
    let n: i64 = c
        .query_row("SELECT count(*) FROM g WHERE g MATCH '\"9P526\"'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(n, 1);
    let s: f64 = c
        .query_row("SELECT bm25(t) FROM t WHERE t MATCH 'circuit'", [], |r| r.get(0))
        .unwrap();
    assert!(s <= 0.0);
}

/// IX-1: the part number in any spelling, a substring and words of the description find A9P52610.
#[test]
fn ix1_a_part_is_found_by_number_substring_and_description() {
    let f = Fixture::new("ew-library-ix1");
    let mut reg = f.open();
    let lib = company(&f, &mut reg);
    let hits = search(&reg, "A9P52610");
    assert_eq!(hits[0].part_number, "A9P52610");
    assert_eq!(hits[0].rank_tier, 1);
    assert_eq!(hits[0].library, lib);
    assert_eq!(hits[0].item, id(A9P52610));
    assert_eq!(hits[0].manufacturer, "Schneider Electric");
    assert_eq!(hits[0].tier, Tier::Company);
    assert_eq!(hits[0].kind, PartKind::Device);
    assert_eq!(hits[0].version, "1.0.0");
    let hits = search(&reg, "a9p 52610");
    assert_eq!((hits[0].part_number.as_str(), hits[0].rank_tier), ("A9P52610", 1));
    let hits = search(&reg, "9P526");
    let pos = numbers(&hits).iter().position(|n| *n == "A9P52610").unwrap();
    assert_eq!(hits[pos].rank_tier, 3, "trigram");
    assert!(numbers(&hits).contains(&"A9P52616"));
    let hits = search(&reg, "miniature circuit");
    assert!(numbers(&hits).contains(&"A9P52610"), "{:?}", numbers(&hits));
    assert!(hits.iter().all(|h| h.rank_tier == 4));
    // A prefix of the number: tier 2, before the description hits.
    let hits = search(&reg, "A9P5261");
    assert_eq!(numbers(&hits)[..2], ["A9P52610", "A9P52616"]);
    assert!(hits[..2].iter().all(|h| h.rank_tier == 2));
    // The manufacturer's name and alias are searchable.
    assert!(numbers(&search(&reg, "schneider")).contains(&"LC1D09"));
    assert!(numbers(&search(&reg, "phoenix")).contains(&"2904602"));
    // The limit.
    let hits = reg.search(&PartQuery {
        text: "circuit".into(),
        limit: 1,
        ..PartQuery::default()
    });
    assert_eq!(hits.len(), 1);
}

/// IX-2: canonical JSONL, one changed line per changed part between two generations, the manifest last, and a writer
/// killed between shard and manifest leaves the old generation readable.
#[test]
fn ix2_writes_are_canonical_and_a_killed_writer_leaves_the_old_generation() {
    let f = Fixture::new("ew-library-ix2");
    let mut reg = f.open();
    let lib = company(&f, &mut reg);
    let dir = f.root.join("company").join("company-std.ewlib");
    let manifest = LibManifest::parse(&std::fs::read(dir.join("library.json")).unwrap()).unwrap();
    assert_eq!(manifest.generation, 2);
    // A9P52610 and A9P52616 share shard 10.
    let shard = "parts/10.2.jsonl";
    let before = std::fs::read_to_string(dir.join(shard)).unwrap();
    assert_eq!(before.lines().count(), 2);
    assert!(before.ends_with('\n') && !before.contains('\r'));
    for line in before.lines() {
        assert!(line.starts_with("{\"$k\":\"part\",\"id\":\""), "{line}");
        assert!(!line.contains("library_id") && !line.contains("sha256"), "{line}");
    }
    // A new version of one part.
    let changed = P {
        version: "1.0.1",
        category: Some(MCB),
        fndef: Some("ew.fn.mcb-1pn"),
        ..P::new(
            id(A9P52610),
            "A9P52610",
            id(SE),
            "Miniature circuit breaker 1P+N C10, 6 kA",
        )
    };
    let report = reg.write(lib, &[changed.doc(lib)]).unwrap();
    assert_eq!(report.generation, 3);
    assert_eq!(report.updated, [id(A9P52610)]);
    assert_eq!(report.written, ["parts/10.3.jsonl", "library.json"]);
    assert_eq!(report.removed, [shard]);
    let after = std::fs::read_to_string(dir.join("parts/10.3.jsonl")).unwrap();
    let diff: Vec<(&str, &str)> = before
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(diff.len(), 1, "one line per changed part");
    assert!(diff[0].1.contains("\"version\":\"1.0.1\""));
    // Every other shard is untouched.
    let m3 = LibManifest::parse(&std::fs::read(dir.join("library.json")).unwrap()).unwrap();
    assert_eq!(m3.files.len(), manifest.files.len());
    // Killed after the shards: the old generation stays readable, the new shard is a leftover.
    let newer = P {
        version: "1.0.2",
        ..changed
    };
    for fault in [WriteFault::AfterShards, WriteFault::BeforeManifestRename] {
        let mut crashing = f.open();
        crashing.set_write_fault(Some(fault));
        let e = crashing.write(lib, &[newer.doc(lib)]).unwrap_err();
        assert!(matches!(e, LibraryError::Io(_)), "{e:?}");
        assert!(dir.join("parts/10.4.jsonl").is_file(), "the new shard is on disk");
        drop(crashing);
        let reader = f.open();
        let doc = reader
            .resolve(&serde_json::from_value::<LibRef>(libref(lib, id(A9P52610), "*")).unwrap())
            .unwrap();
        let LibDocument::Part(c) = doc else { panic!() };
        assert_eq!(c.version, "1.0.1", "{fault:?}: the old generation");
        assert_eq!(reader.libraries()[1].generation, 3);
    }
    // The next write removes the leftovers.
    let report = reg.write(lib, &[newer.doc(lib)]).unwrap();
    assert_eq!(report.generation, 4);
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.contains(".tmp-")), "{names:?}");
    let m4 = LibManifest::parse(&std::fs::read(dir.join("library.json")).unwrap()).unwrap();
    for e in std::fs::read_dir(dir.join("parts")).unwrap() {
        let name = format!("parts/{}", e.unwrap().file_name().to_string_lossy());
        assert!(m4.files.contains_key(&name), "{name} is listed");
    }
    assert!(!dir.join(LOCK).exists(), "the lock is released");
}

/// Published versions are immutable; lines this reader cannot type survive a rewrite of their shard.
#[test]
fn versions_are_immutable_and_untyped_lines_survive_a_rewrite() {
    let f = Fixture::new("ew-library-immutable");
    let mut reg = f.open();
    let lib = company(&f, &mut reg);
    let edited = P {
        category: Some(MCB),
        fndef: Some("ew.fn.mcb-1pn"),
        ..P::new(id(A9P52610), "A9P52610", id(SE), "Edited without a new version")
    };
    let e = reg.write(lib, &[edited.doc(lib)]).unwrap_err();
    assert_eq!(
        e,
        LibraryError::Immutable {
            item: id(A9P52610),
            version: "1.0.0".into()
        }
    );
    assert_eq!(reg.libraries()[1].generation, 2, "nothing written");
    // The same content again changes nothing.
    let same = parts(lib);
    let report = reg.write(lib, &same).unwrap();
    assert_eq!(report.generation, 2);
    assert_eq!(report.unchanged.len(), 12);
    assert!(report.written.is_empty());
    // A line of a newer format in shard 10, listed with its hash.
    let dir = f.root.join("company").join("company-std.ewlib");
    let future = format!(
        "{{\"$k\":\"part\",\"id\":\"{}\",\"version\":\"1.0.0\",\"future\":1}}",
        id(0x0b_0010)
    );
    let shard = dir.join("parts/10.2.jsonl");
    let mut text = std::fs::read_to_string(&shard).unwrap();
    text.push_str(&future);
    text.push('\n');
    std::fs::write(&shard, &text).unwrap();
    let mut m = LibManifest::parse(&std::fs::read(dir.join("library.json")).unwrap()).unwrap();
    m.files
        .insert("parts/10.2.jsonl".into(), ew_format::sha256(text.as_bytes()));
    m.generation += 1;
    std::fs::write(dir.join("library.json"), m.to_text()).unwrap();
    let mut reg = f.open();
    let info = &reg.libraries()[1];
    assert!(
        info.findings
            .iter()
            .any(|n| n.contains("parts/10.2.jsonl: line 3")),
        "{:?}",
        info.findings
    );
    let newer = P {
        version: "2.0.0",
        category: Some(MCB),
        fndef: Some("ew.fn.mcb-1pn"),
        ..P::new(
            id(A9P52616),
            "A9P52616",
            id(SE),
            "Miniature circuit breaker 1P+N C16",
        )
    };
    reg.write(lib, &[newer.doc(lib)]).unwrap();
    let rewritten = std::fs::read_to_string(dir.join("parts/10.4.jsonl")).unwrap();
    assert!(rewritten.contains(&future), "kept verbatim: {rewritten}");
    // A document of another library is refused.
    let foreign = P::new(id(0x77), "F1", id(SE), "Foreign").doc(id(0x1234));
    assert!(matches!(reg.write(lib, &[foreign]), Err(LibraryError::Format(_))));
}

/// IX-3: a deleted index is rebuilt with the same results; a truncated one is replaced; a changed library reindexes
/// only itself.
#[test]
fn ix3_the_index_is_rebuilt_and_only_what_changed_is_reindexed() {
    let f = Fixture::new("ew-library-ix3");
    let mut reg = f.open();
    company(&f, &mut reg);
    let user_dir = f.tiers.user.join("mine.ewlib");
    let user = reg.create(&user_dir, "mine", "Mine", "CC0-1.0").unwrap();
    reg.write(
        user,
        &[
            manufacturer(user, id(0x1), "WAGO", "WAGO", &[]),
            P::new(id(0x2_0010), "2002-1201", id(0x1), "Feed-through terminal block").doc(user),
        ],
    )
    .unwrap();
    let queries = [
        "A9P52610",
        "a9p 52610",
        "9P526",
        "miniature circuit",
        "A9P5261",
        "schneider",
        "phoenix",
        "terminal",
        "2002-1201",
        "20021201",
        "tie",
        "equal tie part",
        "contactor",
        "LC1",
        "power",
        "24",
        "breaker",
        "C16",
        "wago",
        "",
    ];
    let run = |reg: &Registry| -> Vec<Vec<(Uuid, u8)>> {
        queries
            .iter()
            .map(|q| search(reg, q).iter().map(|h| (h.item, h.rank_tier)).collect())
            .collect()
    };
    let expected = run(&reg);
    assert!(
        expected.iter().filter(|r| !r.is_empty()).count() >= 18,
        "{expected:?}"
    );
    drop(reg);
    // Reopened: everything fresh.
    let reg = f.open();
    assert!(
        reg.libraries().iter().all(|l| l.index == IndexState::Fresh),
        "{:?}",
        reg.libraries()
    );
    assert_eq!(run(&reg), expected);
    drop(reg);
    // Deleted: rebuilt with identical results.
    let index = f.tiers.cache.join(INDEX_FILE);
    std::fs::remove_file(&index).unwrap();
    let reg = f.open();
    assert!(reg.libraries().iter().all(|l| l.index == IndexState::Rebuilt));
    assert_eq!(run(&reg), expected);
    drop(reg);
    // Truncated: replaced.
    let bytes = std::fs::read(&index).unwrap();
    std::fs::write(&index, &bytes[..bytes.len().min(1000)]).unwrap();
    let reg = f.open();
    assert_eq!(run(&reg), expected);
    drop(reg);
    // Foreign: replaced.
    std::fs::write(&index, b"not a database at all").unwrap();
    let reg = f.open();
    assert_eq!(run(&reg), expected);
    drop(reg);
    // Another user's registry (its own cache) changes the user library: the next open reindexes only it.
    let mut other = Registry::open(&LibraryTiers {
        cache: f.root.join("cache-of-another-user"),
        ..f.tiers.clone()
    })
    .unwrap();
    other
        .write(
            user,
            &[P::new(id(0x3_0011), "2002-1301", id(0x1), "Ground terminal").doc(user)],
        )
        .unwrap();
    drop(other);
    let reg = f.open();
    let states: Vec<(String, IndexState)> = reg.libraries().into_iter().map(|l| (l.key, l.index)).collect();
    assert_eq!(
        states,
        [
            ("ew-starter".to_owned(), IndexState::Fresh),
            ("company-std".to_owned(), IndexState::Fresh),
            ("mine".to_owned(), IndexState::Rebuilt),
        ]
    );
    assert!(numbers(&search(&reg, "2002-1301")).contains(&"2002-1301"));
    // A full rebuild gives the same results again.
    let mut reg = reg;
    let before = run(&reg);
    reg.rebuild_index().unwrap();
    assert_eq!(run(&reg), before);
}

/// IX-4: system and community libraries refuse writes; the index lives in the cache directory only.
#[test]
fn ix4_read_only_tiers_and_no_sqlite_outside_the_cache() {
    let f = Fixture::new("ew-library-ix4");
    // A community pack and a shipped system library, written by hand.
    for (dir, key) in [
        (f.tiers.community.join("pack.ewlib"), "pack"),
        (
            f.root.join("system").join("ew-starter-parts.ewlib"),
            "ew-starter-parts",
        ),
    ] {
        std::fs::create_dir_all(&dir).unwrap();
        let lib = if key == "ew-starter-parts" {
            ew_library::starter_parts_library()
        } else {
            id(0xabc)
        };
        std::fs::write(
            dir.join("library.json"),
            LibManifest::new(lib, key, key, "CC0-1.0").to_text(),
        )
        .unwrap();
    }
    let mut reg = f.open();
    let libs = reg.libraries();
    let tiers: Vec<(&str, Tier, bool)> = libs
        .iter()
        .map(|l| (l.key.as_str(), l.tier, l.writable))
        .collect();
    assert_eq!(
        tiers,
        [
            ("ew-starter", Tier::System, false),
            ("ew-starter-parts", Tier::System, false),
            ("pack", Tier::Community, false),
        ]
    );
    let doc = P::new(id(0x5_0001), "P1", id(SE), "x").doc(starter::LIBRARY);
    assert_eq!(reg.write(starter::LIBRARY, &[doc]), Err(LibraryError::ReadOnly));
    let doc = P::new(id(0x5_0001), "P1", id(SE), "x").doc(id(0xabc));
    assert_eq!(reg.write(id(0xabc), &[doc]), Err(LibraryError::ReadOnly));
    assert_eq!(reg.take_over_stale(id(0xabc)), Err(LibraryError::ReadOnly));
    assert_eq!(
        reg.create(&f.tiers.community.join("new.ewlib"), "new", "New", "CC0-1.0"),
        Err(LibraryError::ReadOnly)
    );
    assert_eq!(
        reg.create(&f.root.join("system").join("new.ewlib"), "new", "New", "CC0-1.0"),
        Err(LibraryError::ReadOnly)
    );
    // A company library on a "share": written and searched, never opened as SQLite.
    let lib = company(&f, &mut reg);
    assert!(!search(&reg, "A9P52610").is_empty());
    let index = reg.index_path().unwrap();
    assert!(index.starts_with(&f.tiers.cache), "{}", index.display());
    assert_eq!(index, f.tiers.cache.join(INDEX_FILE));
    fn sqlite_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                sqlite_files(&p, out);
            } else if p.to_string_lossy().contains("sqlite") {
                out.push(p);
            }
        }
    }
    let mut found = Vec::new();
    sqlite_files(&f.root, &mut found);
    assert!(found.iter().all(|p| p.starts_with(&f.tiers.cache)), "{found:?}");
    assert!(!found.is_empty());
    // The writable tiers.
    assert!(
        reg.libraries()
            .iter()
            .any(|l| l.id == lib && l.writable && l.tier == Tier::Company)
    );
}

/// IX-5: for a 1P+N MCB function the MCB parts rank first, the others come back `compatible = false`.
#[test]
fn ix5_the_compatibility_filter_ranks_fitting_parts_first() {
    let f = Fixture::new("ew-library-ix5");
    let mut reg = f.open();
    company(&f, &mut reg);
    let hits = reg.search(&PartQuery {
        compatible_with: Some(FnCompat {
            category: ["ProtectionOvercurrent".into(), "MCB".into()],
            contact_kind: ContactKind::Pole,
        }),
        ..PartQuery::default()
    });
    let fits: Vec<(&str, Option<bool>)> = hits
        .iter()
        .map(|h| (h.part_number.as_str(), h.compatible))
        .collect();
    // The two levels of D6: the 1P MCB fits a 1P+N function too (category's first two levels and contact kind).
    assert_eq!(
        fits[..3],
        [
            ("A9F78116", Some(true)),
            ("A9P52610", Some(true)),
            ("A9P52616", Some(true))
        ]
    );
    assert!(fits[3..].iter().all(|(_, c)| *c == Some(false)), "{fits:?}");
    assert!(fits.iter().any(|(n, _)| *n == "LC1D09"), "the others are listed");
    // Without the filter, `compatible` is not reported.
    assert!(search(&reg, "").iter().all(|h| h.compatible.is_none()));
    // The starter's coil function finds the contactor; the same categories with another contact kind find nothing.
    let LibDocument::Fndef(c) = LibDocument::from_json(starter::item("ew.fn.coil").unwrap().json).unwrap()
    else {
        panic!()
    };
    let (a, b, _) = c.item.category.clone();
    let category = [a.as_str().to_owned(), b.as_str().to_owned()];
    let coil = reg.search(&PartQuery {
        compatible_with: Some(FnCompat {
            category: category.clone(),
            contact_kind: c.item.contact_kind.clone(),
        }),
        ..PartQuery::default()
    });
    assert_eq!(
        (coil[0].part_number.as_str(), coil[0].compatible),
        ("LC1D09", Some(true))
    );
    assert!(coil[1..].iter().all(|h| h.compatible == Some(false)));
    let other_kind = if c.item.contact_kind == ContactKind::Pole {
        ContactKind::Coil
    } else {
        ContactKind::Pole
    };
    let none = reg.search(&PartQuery {
        compatible_with: Some(FnCompat {
            category,
            contact_kind: other_kind,
        }),
        ..PartQuery::default()
    });
    assert!(none.iter().all(|h| h.compatible == Some(false)));
}

/// IX-6: deprecated and withdrawn parts are hidden unless asked for; the successor is reported.
#[test]
fn ix6_inactive_parts_are_hidden_and_the_successor_is_reported() {
    let f = Fixture::new("ew-library-ix6");
    let mut reg = f.open();
    let lib = company(&f, &mut reg);
    let visible = numbers(&search(&reg, "circuit breaker")).join(",");
    assert!(!visible.contains("A9N0000"), "{visible}");
    let all = reg.search(&PartQuery {
        text: "circuit breaker".into(),
        include_inactive: true,
        ..PartQuery::default()
    });
    let old = all.iter().find(|h| h.part_number == "A9N00001").unwrap();
    let ItemStatus::Deprecated(d) = &old.status else {
        panic!("{:?}", old.status)
    };
    let successor = d.successor.as_ref().unwrap();
    assert_eq!(
        (successor.library.as_uuid(), successor.item.as_uuid()),
        (lib, id(A9P52610))
    );
    let gone = all.iter().find(|h| h.part_number == "A9N00002").unwrap();
    assert_eq!(gone.status, ItemStatus::Withdrawn);
}

/// IX-7: equal scores order by natural part number, manufacturer, library ID, item ID (X2 before X10).
#[test]
fn ix7_ties_order_by_natural_part_number() {
    let f = Fixture::new("ew-library-ix7");
    let mut reg = f.open();
    company(&f, &mut reg);
    let hits = search(&reg, "equal tie part");
    assert_eq!(numbers(&hits), ["X2", "X10"]);
    // Held parts come first inside their tier, then the boost list.
    let lib = hits[0].library;
    let held = reg.search(&PartQuery {
        text: "equal tie part".into(),
        held: vec![(lib, id(TIE_X10))],
        ..PartQuery::default()
    });
    assert_eq!(numbers(&held), ["X10", "X2"]);
    let boosted = reg.search(&PartQuery {
        text: "tie".into(),
        boost: vec![(lib, id(TIE_X10))],
        ..PartQuery::default()
    });
    assert_eq!(numbers(&boosted), ["X10", "X2"]);
    // Filters: kinds, category prefix, tiers.
    let terminals = reg.search(&PartQuery {
        kinds: vec![PartKind::Terminal],
        ..PartQuery::default()
    });
    assert_eq!(numbers(&terminals), ["3044102"]);
    let mcbs = reg.search(&PartQuery {
        category: vec!["ProtectionOvercurrent".into(), "MCB".into()],
        ..PartQuery::default()
    });
    assert_eq!(numbers(&mcbs), ["A9F78116", "A9P52610", "A9P52616"]);
    let user_only = reg.search(&PartQuery {
        tiers: vec![Tier::User],
        ..PartQuery::default()
    });
    assert!(user_only.is_empty());
}

/// The resolver and the closure: the item first, then FnDefs, …, manufacturers; copy metadata added.
#[test]
fn resolve_and_closure() {
    let f = Fixture::new("ew-library-resolve");
    let mut reg = f.open();
    let lib = company(&f, &mut reg);
    let r: LibRef = serde_json::from_value(libref(lib, id(A9P52610), "^1")).unwrap();
    let doc = reg.resolve(&r).unwrap();
    let LibDocument::Part(c) = &doc else { panic!() };
    assert_eq!(c.library_id.as_uuid(), lib);
    assert_eq!(
        c.id.as_uuid(),
        ew_ids::v5::lib_copy_id::<()>(lib, id(A9P52610)).as_uuid()
    );
    assert_eq!(
        Some(c.sha256.0),
        ew_format::library_copy_hash(&doc),
        "EW-CORE-014"
    );
    let closure = reg.closure(&r).unwrap();
    let kinds: Vec<Option<Kind>> = closure.iter().map(LibDocument::kind).collect();
    assert_eq!(
        kinds,
        [Some(Kind::Part), Some(Kind::Fndef), Some(Kind::Manufacturer)]
    );
    // A version the reference does not accept, an unknown item, an unknown library.
    let r2: LibRef = serde_json::from_value(libref(lib, id(A9P52610), "^2")).unwrap();
    assert_eq!(reg.resolve(&r2), Err(LibraryError::NotFound));
    let r3: LibRef = serde_json::from_value(libref(lib, id(0xdead), "*")).unwrap();
    assert_eq!(reg.resolve(&r3), Err(LibraryError::NotFound));
    let r4: LibRef = serde_json::from_value(libref(id(0xbeef), id(A9P52610), "*")).unwrap();
    assert_eq!(reg.resolve(&r4), Err(LibraryError::NotFound));
    // A starter symbol's closure is the symbol and its FnDef.
    let sym = starter_item("ew.iec.mcb-1pn@qet10");
    let r5: LibRef = serde_json::from_value(libref(starter::LIBRARY, sym, "^0.1")).unwrap();
    let kinds: Vec<Option<Kind>> = reg.closure(&r5).unwrap().iter().map(LibDocument::kind).collect();
    assert_eq!(kinds, [Some(Kind::Symbol), Some(Kind::Fndef)]);
    // A part whose manufacturer no library holds has no closure.
    let orphan = P::new(id(0x6_0020), "ORPHAN", id(0x999), "Orphan").doc(lib);
    reg.write(lib, &[orphan]).unwrap();
    let r6: LibRef = serde_json::from_value(libref(lib, id(0x6_0020), "*")).unwrap();
    assert!(reg.resolve(&r6).is_ok());
    assert_eq!(reg.closure(&r6), Err(LibraryError::NotFound));
}

/// The item search over the starter and the library's manufacturers.
#[test]
fn items_are_searchable() {
    let f = Fixture::new("ew-library-items");
    let mut reg = f.open();
    company(&f, &mut reg);
    let hits = reg.search_items(&ItemQuery {
        text: "ew.fn.mcb-1pn".into(),
        ..ItemQuery::default()
    });
    assert_eq!(
        (hits[0].key.as_str(), hits[0].kind, hits[0].rank_tier),
        ("ew.fn.mcb-1pn", Kind::Fndef, 1)
    );
    assert_eq!(hits[0].tier, Tier::System);
    let mfrs = reg.search_items(&ItemQuery {
        text: "schneider".into(),
        kinds: vec![Kind::Manufacturer],
        ..ItemQuery::default()
    });
    assert_eq!(mfrs.len(), 1);
    assert_eq!(
        (mfrs[0].key.as_str(), mfrs[0].name.as_str()),
        ("SE", "Schneider Electric")
    );
    let symbols = reg.search_items(&ItemQuery {
        kinds: vec![Kind::Symbol],
        ..ItemQuery::default()
    });
    assert!(symbols.len() > 30, "{}", symbols.len());
    // The starter-only registry has the starter items in memory.
    let only = Registry::starter_only();
    assert_eq!(only.libraries().len(), 1);
    assert!(only.index_path().is_none());
    assert!(!only.search_items(&ItemQuery::default()).is_empty());
    assert!(
        only.search(&PartQuery::default()).is_empty(),
        "the starter has no parts"
    );
}

/// The library lock: a live lock refuses with the holder, a stale one asks for a takeover, which then writes.
#[test]
fn a_locked_library_refuses_and_a_stale_lock_needs_a_confirmed_takeover() {
    let f = Fixture::new("ew-library-locked");
    let mut reg = f.open();
    let lib = company(&f, &mut reg);
    let dir = f.root.join("company").join("company-std.ewlib");
    let other = Identity {
        host: "another-host".into(),
        ..Identity::current("other")
    };
    std::fs::write(
        dir.join(LOCK),
        // The record's time is informative; the stale rule reads the file's modification time.
        other
            .record(std::time::UNIX_EPOCH + Duration::from_secs(1_800_000_000))
            .to_text(),
    )
    .unwrap();
    let doc = P::new(id(0x7_0030), "NEW1", id(SE), "New").doc(lib);
    match reg.write(lib, std::slice::from_ref(&doc)) {
        Err(LibraryError::Locked { host, .. }) => assert_eq!(host, "another-host"),
        r => panic!("{r:?}"),
    }
    assert_eq!(
        reg.take_over_stale(lib)
            .map_err(|e| matches!(e, LibraryError::Locked { .. })),
        Err(true),
        "a live lock is never taken over"
    );
    let file = std::fs::File::options().write(true).open(dir.join(LOCK)).unwrap();
    let written = file.metadata().unwrap().modified().unwrap();
    file.set_modified(written - Duration::from_secs(7200)).unwrap();
    drop(file);
    match reg.write(lib, std::slice::from_ref(&doc)) {
        Err(LibraryError::Stale { host, last_seen, .. }) => {
            assert_eq!(host, "another-host");
            assert!(last_seen.is_some());
        }
        r => panic!("{r:?}"),
    }
    reg.take_over_stale(lib).unwrap();
    let report = reg.write(lib, &[doc]).unwrap();
    assert_eq!(report.added, [id(0x7_0030)]);
    assert!(!dir.join(LOCK).exists(), "released after the write");
}

/// The shipped, still empty system library `ew-starter-parts` (§7.1): its manifest names the derived ID; WP-S2.18
/// part 2 fills it.
#[test]
fn the_shipped_starter_parts_library_has_the_derived_id() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let libraries = repo.join("content").join("libraries");
    let expected = LibManifest::new(
        ew_library::starter_parts_library(),
        ew_library::STARTER_PARTS_KEY,
        "ElektraWatt starter parts",
        "CC-BY-SA-4.0",
    )
    .to_text();
    let shipped = std::fs::read_to_string(libraries.join("ew-starter-parts.ewlib").join("library.json"));
    assert_eq!(
        shipped.map(|s| s.replace("\r\n", "\n")).ok().as_deref(),
        Some(expected.as_str())
    );
    // Read as a system library next to the compiled-in starter.
    let f = Fixture::new("ew-library-shipped");
    let reg = Registry::open(&LibraryTiers {
        system: vec![libraries],
        ..f.tiers.clone()
    })
    .unwrap();
    let keys: Vec<(String, Tier, bool)> = reg
        .libraries()
        .into_iter()
        .map(|l| (l.key, l.tier, l.writable))
        .collect();
    assert_eq!(
        keys,
        [
            ("ew-starter".to_owned(), Tier::System, false),
            ("ew-starter-parts".to_owned(), Tier::System, false)
        ]
    );
}
