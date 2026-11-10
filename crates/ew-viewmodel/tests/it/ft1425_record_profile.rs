// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Choosing a library's address profile (`plc.set_profile` naming a profile only a library holds)
//! embeds the profile first; `--record` now writes that `library.embed` line before the command's own, so
//! the recorded steps replay on a copy of the project opened without the library.

use crate::common;

use std::path::Path;

use common::{GOLDEN_A, Tmp, copy_dir, done};
use ew_viewmodel::ew_api::library::{LibraryTiers, Registry};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::{Value, json};

fn tiers(root: &Path) -> LibraryTiers {
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: Vec::new(),
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// The ID of Golden A's configuration project.
fn config_project() -> String {
    let text = std::fs::read_to_string(Path::new(GOLDEN_A).join("project.jsonl")).unwrap();
    text.lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|v| v["$k"] == "plc_config_project")
        .expect("Golden A has a configuration project")["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// A shell over a writable copy of Golden A in `dir`, with `libraries`.
fn open(tmp: &Tmp, dir: &Path, libraries: Option<LibraryTiers>) -> ShellModel {
    copy_dir(Path::new(GOLDEN_A), dir);
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test").with_libraries(libraries),
    );
    done(
        &mut s,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    s
}

#[test]
fn ft1425_the_record_holds_the_embed_that_choosing_a_library_profile_runs() {
    let tmp = Tmp::new("ft1425");
    let t = tiers(&tmp.0);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(&t.user.join("plc.ewlib"), "plc", "PLC profiles", "CC0-1.0")
        .unwrap();
    reg.write(
        lib,
        &[ew_testkit::library::address_profile(lib, "fixture-absolute")],
    )
    .unwrap();
    let cfg = config_project();
    let mut s = open(&tmp, &tmp.0.join("a.ew"), Some(t));
    let choose = json!({"config_project": cfg, "profile": "fixture-absolute"});
    done(
        &mut s,
        "command.run",
        json!({"cmd": "plc.set_profile", "params": choose}),
    );
    let steps: Vec<(String, Value)> = s
        .recorded_commands()
        .iter()
        .map(|r| (r.cmd.clone(), r.params.clone()))
        .collect();
    let [(embed, copies), (set, params)] = steps.as_slice() else {
        panic!("two recorded steps: {steps:?}")
    };
    assert_eq!(embed, "library.embed");
    assert_eq!(copies["copies"][0]["profile"]["id"], "fixture-absolute");
    assert_eq!((set.as_str(), params), ("plc.set_profile", &choose));

    // The recorded steps replay on a fresh copy opened without the library.
    let mut r = open(&tmp, &tmp.0.join("b.ew"), None);
    for (cmd, params) in &steps {
        done(&mut r, "command.run", json!({"cmd": cmd, "params": params}));
    }
    assert_eq!(r.recorded_commands().len(), 2);
}
