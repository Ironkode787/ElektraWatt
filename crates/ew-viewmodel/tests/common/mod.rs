// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers of the tool-framework tests (WP-S2.03 part 2): temporary copies of Golden A, dispatch with checks,
//! points of a page.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use ew_viewmodel::canvas::{CanvasModel, HOVER_SLOP_PT, pick, rect_centre};
use ew_viewmodel::script;
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

pub const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

pub use ew_testdir as fresh_dir;

/// A fresh temporary directory, removed on drop.
pub struct Tmp(pub PathBuf);

impl Tmp {
    pub fn new(tag: &str) -> Tmp {
        Tmp(fresh_dir::fresh_dir(&format!("ew-vm-{tag}")))
    }

    /// A shell in `mode` with a copy of Golden A open (read-only unless `writable`), showing `folio`.
    pub fn shell_in(&self, mode: Mode, writable: bool, folio: &str) -> ShellModel {
        let dir = self.golden_copy();
        if !dir.exists() {
            copy_dir(Path::new(GOLDEN_A), &dir);
        }
        let mut shell = ShellModel::new(mode, ShellConfig::in_dir(&self.0.join("config"), "0.0.0-test"));
        done(
            &mut shell,
            "file.open",
            json!({"dir": dir.to_string_lossy(), "read_only": !writable}),
        );
        done(&mut shell, "page.goto", json!({ "folio": folio }));
        shell
    }

    /// The directory of the copy of Golden A that [`Tmp::shell_in`] opens (for a test that saves and reopens it).
    pub fn golden_copy(&self) -> PathBuf {
        self.0.join("J-E-2416.ew")
    }

    /// A headless shell with a read-only copy of Golden A on `folio`.
    pub fn shell(&self, folio: &str) -> ShellModel {
        self.shell_in(Mode::Headless, false, folio)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Whether `EW_BLESS=1` asks the golden-text tests to rewrite their expected files (WP-S6.13).
// Allowed: the program boundary of a test, read once; the value never reaches a result.
#[allow(clippy::disallowed_methods)]
pub fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

#[track_caller]
pub fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

#[track_caller]
pub fn refused(shell: &mut ShellModel, action: &str, params: Value) -> &'static str {
    match shell.dispatch(action, &params) {
        Outcome::Refused { reason_key } => reason_key,
        other => panic!("{action}: {other:?}"),
    }
}

/// `{"x_mm", "y_mm"}` of a sheet point, with more members.
pub fn at(p: (i64, i64), more: Value) -> Value {
    let mut v = json!({"x_mm": p.0 as f64 / 1e6, "y_mm": p.1 as f64 / 1e6});
    if let (Some(o), Value::Object(m)) = (v.as_object_mut(), more) {
        o.extend(m);
    }
    v
}

pub fn state(shell: &ShellModel) -> Value {
    serde_json::from_str(&script::state_json(shell)).unwrap()
}

/// A click (press and release) at `p` with `mods` (`{"shift": true}` …).
pub fn click(shell: &mut ShellModel, p: (i64, i64), mods: Value) {
    done(shell, "canvas.press", at(p, mods.clone()));
    done(shell, "canvas.release", at(p, mods));
}

/// The centres of the entity rectangles that are no link and whose pick is that entity, in paint order.
pub fn entity_points(c: &CanvasModel) -> Vec<(ew_viewmodel::ew_api::output::ew_dl::EntityRef, (i64, i64))> {
    let slop = HOVER_SLOP_PT / c.points_per_nm();
    let mut out = Vec::new();
    for (e, r) in &c.page().entities {
        let p = rect_centre(*r);
        if c.link_at(p).is_none()
            && pick(&c.page().entities, Some(p), slop).map(|(x, _)| x) == Some(*e)
            && !out.iter().any(|(x, _)| x == e)
        {
            out.push((*e, p));
        }
    }
    out
}

/// A point of the sheet with no entity and no link under it.
pub fn empty_point(c: &CanvasModel) -> (i64, i64) {
    let slop = HOVER_SLOP_PT / c.points_per_nm();
    let (w, h) = c.sheet();
    for y in (5_000_000..h).step_by(3_000_000) {
        for x in (5_000_000..w).step_by(3_000_000) {
            if pick(&c.page().entities, Some((x, y)), slop).is_none() && c.link_at((x, y)).is_none() {
                return (x, y);
            }
        }
    }
    panic!("no empty point on {}", c.page().folio)
}

/// The structural tokens of a describe line: every token outside quotes, numbers, aliases and punctuation left out.
pub fn structural_tokens(line: &str) -> Vec<String> {
    let mut outside = String::new();
    let mut quoted = false;
    for c in line.chars() {
        if c == '"' {
            quoted = !quoted;
            outside.push(' ');
        } else if !quoted {
            outside.push(c);
        }
    }
    outside
        .split_whitespace()
        .filter(|t| {
            t.parse::<f64>().is_err()
                && !ew_viewmodel::alias::is_alias(t)
                && !t.chars().all(|c| !c.is_alphanumeric())
        })
        .map(str::to_owned)
        .collect()
}
