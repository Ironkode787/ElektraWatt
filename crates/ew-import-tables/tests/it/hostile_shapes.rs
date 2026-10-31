// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hostile shapes (doc 14 §11: hostile input is rejected within 1 s / 256 MB): deep device parent chains and
//! cycles, junction drop cascades and many orphaned interruption points.
//!
//! The correctness tests run moderate sizes in the normal suite and assert output and findings only. The
//! timing tests are performance tests (`CLAUDE.md` testing policy): `#[ignore]`d, run at milestone deliveries
//! with `cargo nextest run -p ew-import-tables --release --run-ignored only --test hostile_shapes`, and assert
//! ≤ 1 s each at the review sizes (20,000-row chain, 1,000-junction cascade, 20,000 orphans).

// A wall-clock timer of this test, asserted only with EW_PERF_ASSERT (the timing rule of doc 18 §3.16); no timestamp reaches a compared result.
#![allow(clippy::disallowed_methods)]

use crate::common;

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use ew_import_tables::ids::record_id;
use ew_interop_api::{Code, ImportBatch, Severity};
use ew_schema::{Device, Record};

const PAGES: &str = "key,number,kind,form,grid\n101,101,multi_line,ew.frame.qet-a4-landscape-grid,qet10\n";

fn run(files: &[(&str, &str)]) -> ImportBatch {
    let mut all = vec![("project.csv", common::PROJECT_CSV), ("pages.csv", PAGES)];
    all.extend_from_slice(files);
    common::import(&common::inputs(&all)).expect("the import succeeds")
}

fn kinds(b: &ImportBatch, f: fn(&Record) -> bool) -> usize {
    b.records.iter().filter(|r| f(r)).count()
}

/// `n` devices `d0 … d{n-1}`, each the child of the next; with `cycle`, the last one's parent is `d0`.
fn chain(n: usize, cycle: bool) -> String {
    let mut s = String::from("key,letter,counter,parent\n");
    for i in 0..n {
        let parent = if i + 1 < n {
            format!("d{:06}", i + 1)
        } else if cycle {
            "d000000".to_owned()
        } else {
            String::new()
        };
        let _ = writeln!(s, "d{i:06},F,{},{parent}", i + 1);
    }
    s
}

/// A junction cascade: `j0 … j{n-1}` chained by connections `c{i}` from `@j{i}` to `@j{i+1}`, plus one CP
/// connection at each end; `j0`'s sequence names an unknown connection, so every junction falls in turn.
fn cascade(n: usize) -> (String, String) {
    let mut conns = String::from("key,page,a,b\nhead,101,-1F1:1,@j000000\n");
    let mut juncs = String::from("key,page,cell,style,sequence\n");
    for i in 0..n {
        if i + 1 < n {
            let _ = writeln!(conns, "c{i:06},101,@j{i:06},@j{:06}", i + 1);
        }
        let prev = if i == 0 {
            "nope".to_owned()
        } else {
            format!("c{:06}", i - 1)
        };
        let next = if i + 1 < n {
            format!("c{i:06}")
        } else {
            "tail".to_owned()
        };
        let first = if i == 0 { "head;nope".to_owned() } else { prev };
        let _ = writeln!(juncs, "j{i:06},101,C6,dot,{first};{next}");
    }
    let _ = writeln!(conns, "tail,101,@j{:06},-1F1:2", n - 1);
    (conns, juncs)
}

/// `n` interruption points without a representation (EW-INV-013).
fn orphans(n: usize) -> String {
    let mut s = String::from("key,fndef,kind,ip_name,ip_direction\n");
    for i in 0..n {
        let _ = writeln!(s, "ip{i:06},ew.fn.ip,ip,P{i},source");
    }
    s
}

const ONE_DEVICE: (&str, &str) = ("devices.csv", "key,letter,counter\n-1F1,F,1\n");
const ONE_FUNCTION: (&str, &str) = ("functions.csv", "key,device,fndef\n-1F1,-1F1,ew.fn.mcb-1pn\n");

#[test]
fn a_deep_parent_chain_is_kept_with_its_parents() {
    let n = 500;
    let b = run(&[("devices.csv", &chain(n, false))]);
    assert!(
        b.report.entries.is_empty(),
        "{:?}",
        &b.report.entries[..3.min(b.report.entries.len())]
    );
    assert_eq!(kinds(&b, |r| matches!(r, Record::Device(_))), n);
    let id = |i: usize| record_id::<Device>("devices", &format!("d{i:06}"));
    for r in &b.records {
        if let Record::Device(d) = r {
            let i: usize = d.source_ref.as_ref().unwrap().key["devices|d".len()..]
                .parse()
                .unwrap();
            assert_eq!(d.parent, (i + 1 < n).then(|| id(i + 1)), "d{i}");
        }
    }
}

#[test]
fn a_parent_cycle_drops_the_cycle_and_everything_below_it() {
    // d0 → d1 → … → d{n-1} → d0: every device is on the cycle; one more than the report lists.
    let n = ew_import_tables::MAX_REPORT_ENTRIES + 1;
    let b = run(&[("devices.csv", &chain(n, true))]);
    assert_eq!(kinds(&b, |r| matches!(r, Record::Device(_))), 0);
    let cyc = b
        .report
        .with_code(Code::imp(2))
        .filter(|e| e.message.contains("parent chain is a cycle"))
        .count();
    // The report lists at most 1,000 findings; the rest are counted in one EW-IMP-001 entry.
    assert_eq!(cyc, ew_import_tables::MAX_REPORT_ENTRIES);

    // A three-cycle with a 500-long tail hanging below it: 3 cycle findings, 500 orphans.
    let mut s = String::from("key,letter,counter,parent\na,F,1,b\nb,F,2,c\nc,F,3,a\n");
    for i in 0..500 {
        let parent = if i == 0 {
            "a".to_owned()
        } else {
            format!("t{:03}", i - 1)
        };
        let _ = writeln!(s, "t{i:03},F,{},{parent}", i + 10);
    }
    let _ = writeln!(s, "free,F,999,");
    let b = run(&[("devices.csv", &s)]);
    assert_eq!(
        kinds(&b, |r| matches!(r, Record::Device(_))),
        1,
        "only `free` is kept"
    );
    let errors = |code: u16, text: &str| {
        b.report
            .with_code(Code::imp(code))
            .filter(|e| e.severity == Severity::Error && e.message.contains(text))
            .count()
    };
    assert_eq!(errors(2, "parent chain is a cycle"), 3);
    assert_eq!(errors(3, "was dropped"), 500);
}

#[test]
fn a_junction_cascade_drops_every_junction_and_connection() {
    let n = 200;
    let (conns, juncs) = cascade(n);
    let b = run(&[
        ONE_DEVICE,
        ONE_FUNCTION,
        ("connections.csv", &conns),
        ("junctions.csv", &juncs),
    ]);
    assert_eq!(kinds(&b, |r| matches!(r, Record::Junction(_))), 0);
    assert_eq!(kinds(&b, |r| matches!(r, Record::Connection(_))), 0);
    // One finding per junction and per connection (n − 1 chain links plus head and tail).
    let junction_findings = b
        .report
        .entries
        .iter()
        .filter(|e| e.location.as_ref().is_some_and(|l| l.file == "junctions.csv"))
        .count();
    let connection_findings = b
        .report
        .entries
        .iter()
        .filter(|e| e.location.as_ref().is_some_and(|l| l.file == "connections.csv"))
        .count();
    assert_eq!(junction_findings, n);
    assert_eq!(connection_findings, n + 1);
}

#[test]
fn a_consistent_junction_chain_is_kept() {
    // The same shape with a valid head: nothing falls.
    let n = 200;
    let (conns, juncs) = cascade(n);
    let juncs = juncs.replace("head;nope", "head");
    let b = run(&[
        ONE_DEVICE,
        ONE_FUNCTION,
        ("connections.csv", &conns),
        ("junctions.csv", &juncs),
    ]);
    assert!(
        b.report.entries.is_empty(),
        "{:?}",
        &b.report.entries[..3.min(b.report.entries.len())]
    );
    assert_eq!(kinds(&b, |r| matches!(r, Record::Junction(_))), n);
    assert_eq!(kinds(&b, |r| matches!(r, Record::Connection(_))), n + 1);
}

#[test]
fn orphaned_interruption_points_are_all_dropped() {
    let n = 300;
    let b = run(&[("functions.csv", &orphans(n))]);
    assert_eq!(kinds(&b, |r| matches!(r, Record::Function(_))), 0);
    let dropped = b
        .report
        .with_code(Code::imp(3))
        .filter(|e| e.message.contains("without a representation"))
        .count();
    assert_eq!(dropped, n);
}

fn timed(what: &str, f: impl FnOnce() -> ImportBatch) -> ImportBatch {
    let t = Instant::now();
    let b = f();
    let dt = t.elapsed();
    println!("{what}: {dt:?}");
    assert!(
        dt <= Duration::from_secs(1),
        "{what} took {dt:?} (> 1 s, doc 14 §11); run with --release"
    );
    b
}

#[test]
#[ignore = "perf: release-profile timing, milestone deliveries only"]
fn timing_a_20000_row_parent_chain() {
    let text = chain(20_000, true);
    let b = timed("20,000-row cyclic chain", || run(&[("devices.csv", &text)]));
    assert_eq!(kinds(&b, |r| matches!(r, Record::Device(_))), 0);
    let text = chain(20_000, false);
    let b = timed("20,000-row chain", || run(&[("devices.csv", &text)]));
    assert_eq!(kinds(&b, |r| matches!(r, Record::Device(_))), 20_000);
}

#[test]
#[ignore = "perf: release-profile timing, milestone deliveries only"]
fn timing_a_1000_junction_cascade() {
    let (conns, juncs) = cascade(1_000);
    let b = timed("1,000-junction cascade", || {
        run(&[
            ONE_DEVICE,
            ONE_FUNCTION,
            ("connections.csv", &conns),
            ("junctions.csv", &juncs),
        ])
    });
    assert_eq!(kinds(&b, |r| matches!(r, Record::Junction(_))), 0);
}

#[test]
#[ignore = "perf: release-profile timing, milestone deliveries only"]
fn timing_20000_orphaned_interruption_points() {
    let text = orphans(20_000);
    let b = timed("20,000 orphaned interruption points", || {
        run(&[("functions.csv", &text)])
    });
    assert_eq!(kinds(&b, |r| matches!(r, Record::Function(_))), 0);
}
