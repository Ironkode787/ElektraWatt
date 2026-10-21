// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `terminal.designate`: designations of several terminals set in one transaction (swap included).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{DesignateTerminals, TerminalDesignation};
use ew_commands::{AnyCommand, CmdError, CommandRegistry, Engine};
use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::Device;
use serde_json::json;

fn designation(e: &Engine, d: Id<Device>) -> String {
    let device = e.model().table::<Device>().get(&d).expect("row exists").clone();
    device
        .aspects
        .terminal
        .as_ref()
        .expect("terminal")
        .designation
        .as_str()
        .to_owned()
}

fn assign(terminal: Id<Device>, name: &str) -> TerminalDesignation {
    TerminalDesignation {
        terminal,
        designation: Ident::from_input(name).expect("valid designation"),
    }
}

fn cmd(assignments: Vec<TerminalDesignation>) -> DesignateTerminals {
    DesignateTerminals { assignments }
}

fn invalid_path<T: std::fmt::Debug>(r: Result<T, CmdError>) -> String {
    match r {
        Err(CmdError::InvalidParams { path, .. }) => path,
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

#[test]
fn designate_sets_one_terminal() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    e.execute(&cmd(vec![assign(DEV_T1, "9")])).unwrap();
    assert_eq!(designation(&e, DEV_T1), "9");
    assert_eq!(designation(&e, DEV_T2), "2");
    assert_eq!(e.history().entries().len(), 1);
    e.undo().unwrap();
    assert_eq!(designation(&e, DEV_T1), "1");
    assert_eq!(lines(e.model()), before);
}

#[test]
fn designate_swaps_two_terminals_in_one_transaction() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let r = e
        .execute(&cmd(vec![assign(DEV_T3, "2"), assign(DEV_T2, "3")]))
        .unwrap();
    assert_eq!(designation(&e, DEV_T2), "3");
    assert_eq!(designation(&e, DEV_T3), "2");
    assert_eq!(designation(&e, DEV_T1), "1");
    assert_eq!(e.history().entries().len(), 1);
    assert_eq!(r.outcome.primary, None);
    e.undo().unwrap();
    assert_eq!(designation(&e, DEV_T2), "2");
    assert_eq!(designation(&e, DEV_T3), "3");
    assert_eq!(lines(e.model()), before);
}

#[test]
fn designate_rejects_empty_list() {
    let mut e = engine(golden_model());
    assert_eq!(invalid_path(e.execute(&cmd(vec![]))), "/assignments");
    assert_eq!(e.model().rev().0, 0);
}

#[test]
fn designate_rejects_duplicate_terminal() {
    let mut e = engine(golden_model());
    let r = e.execute(&cmd(vec![assign(DEV_T1, "8"), assign(DEV_T1, "9")]));
    assert_eq!(invalid_path(r), "/assignments/1/terminal");
    assert_eq!(designation(&e, DEV_T1), "1");
}

#[test]
fn designate_rejects_non_terminal() {
    let mut e = engine(golden_model());
    let r = e.execute(&cmd(vec![assign(DEV_T1, "8"), assign(DEV_X1031, "9")]));
    assert_eq!(invalid_path(r), "/assignments/1/terminal");
    assert_eq!(designation(&e, DEV_T1), "1");
    assert_eq!(e.model().rev().0, 0);
}

#[test]
fn designate_missing_terminal_is_not_found() {
    let mut e = engine(golden_model());
    match e.execute(&cmd(vec![assign(id(0x996), "9")])) {
        Err(CmdError::NotFound { .. }) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn designate_rejects_empty_designation() {
    let e = engine(golden_model());
    let any = AnyCommand {
        name: "terminal.designate".into(),
        params: json!({"assignments": [{"terminal": DEV_T1.to_string(), "designation": ""}]}),
    };
    match CommandRegistry::builtin().resolve(&any) {
        Err(CmdError::InvalidParams { command, .. }) => assert_eq!(command, "terminal.designate"),
        Err(other) => panic!("expected InvalidParams, got {other:?}"),
        Ok(_) => panic!("expected the empty designation to be rejected"),
    }
    assert!(e.resolve(&any).is_err());
}
