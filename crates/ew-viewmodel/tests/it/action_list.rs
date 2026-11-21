// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ui.actions` prints the action list into the transcript, as `ui.describe` prints the window (FT-1868): a script
//! finds every action, its parameters and whether it runs now without the source at hand.

use crate::common::Tmp;
use ew_viewmodel::script::{self, NoExtension};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};

#[test]
fn the_action_list_names_each_action_with_its_parameters_and_state() {
    let tmp = Tmp::new("action-list");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    let text = "{\"$schema\":\"ew.uiscript/1\",\"label\":\"actions\"}\n{\"action\":\"ui.actions\"}\n";
    let mut lines = Vec::new();
    script::run(&mut shell, &mut NoExtension, &script::parse(text).unwrap(), false, &mut |l| {
        lines.push(l.to_owned());
        Ok(())
    })
    .unwrap();
    let step: serde_json::Value = serde_json::from_str(lines.last().unwrap()).unwrap();
    assert_eq!(step["outcome"], "done");
    let list = step["describe"].as_str().expect("the action list");
    let line = |id: &str| {
        list.lines()
            .find(|l| l.starts_with(&format!("item {id} ")))
            .unwrap_or_else(|| panic!("no line of {id}:\n{list}"))
            .to_owned()
    };
    // Every action of the registry has its line.
    for meta in ew_viewmodel::actions::ACTIONS {
        line(meta.id);
    }
    // With its parameters, and why it cannot run now (no project is open).
    let goto = line("page.goto");
    assert!(goto.contains(" disabled: ") && goto.ends_with("params: folio:text"), "{goto}");
    assert!(line("file.open").contains(" enabled"), "{}", line("file.open"));
}
