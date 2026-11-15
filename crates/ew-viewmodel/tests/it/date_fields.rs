// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The owner's test of 0.3.0-rc.1: the project's Created date asked for YYYY-MM-DD while the project printed
//! dd-mm-yyyy. A date is shown and typed in the project's format, in the property grid and in the Title block fields
//! form; ISO is taken too; a wrong date is refused with an example in that format.

use crate::common;

use common::{Tmp, done, new_project};
use ew_viewmodel::properties::{RowValue, find_row};
use ew_viewmodel::shell::{Outcome, ShellModel};
use serde_json::json;

const CREATED: &str = "project/master.created";

/// The value text of the property row `key`.
fn shown(shell: &ShellModel, key: &str) -> String {
    let sheet = shell.properties().sheet().expect("a sheet");
    match &find_row(sheet, key).unwrap_or_else(|| panic!("no row {key}")).1.value {
        RowValue::One { text, .. } => text.clone(),
        other => format!("{other:?}"),
    }
}

#[test]
fn a_date_is_typed_and_shown_in_the_projects_format() {
    let tmp = Tmp::new("date-fields");
    let mut shell = new_project(&tmp);
    done(&mut shell, "props.project", json!({}));
    // Day first, with dots and a two-digit year, in a project that prints dd-mm-yyyy.
    done(&mut shell, "props.set", json!({"row": CREATED, "value": "15.11.26"}));
    assert_eq!(shown(&shell, CREATED), "15-11-2026");
    // ISO is taken too.
    done(&mut shell, "props.set", json!({"row": CREATED, "value": "2026-11-16"}));
    assert_eq!(shown(&shell, CREATED), "16-11-2026");
    // A wrong date is refused with an example in the project's format.
    let Outcome::Failed { message } =
        shell.dispatch("props.set", &json!({"row": CREATED, "value": "31-04-2026"}))
    else {
        panic!("31 April is refused");
    };
    assert!(message.contains("31-12-2026"), "{message}");
    shell.shutdown();
    // The Title block fields form takes the format too (with its parameters, as its Apply sends them), on a copy of
    // Golden A, whose title block prints the date and whose project prints dd-mm-yyyy.
    let mut shell = tmp.shell_in(ew_viewmodel::shell::Mode::Headless, true, "101");
    done(&mut shell, "page.title_block", json!({"project.created": "1/2/2027"}));
    done(&mut shell, "props.project", json!({}));
    assert_eq!(shown(&shell, CREATED), "01-02-2027");
    // Stored as one date: an ISO project shows the same day in ISO.
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "project.languages.set", "params": {"date": "iso8601"}}),
    );
    done(&mut shell, "props.project", json!({}));
    assert_eq!(shown(&shell, CREATED), "2027-02-01");
    shell.shutdown();
}
