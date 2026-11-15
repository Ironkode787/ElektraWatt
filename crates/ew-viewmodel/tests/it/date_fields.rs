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

const EDITED: &str = "project/master.edited";

/// The owner's test of 0.3.0-rc.1: the edited date is the program's. A new project is created on the session's date;
/// a save with changes takes the session's date as its edited date, a save without a change keeps it; the row is
/// read-only and says why.
#[test]
fn the_edited_date_is_set_by_a_save_with_changes() {
    let tmp = Tmp::new("date-edited");
    let mut shell = tmp.empty_shell(ew_viewmodel::shell::Mode::Gui);
    shell.set_today("2026-03-04".to_owned());
    let dir = tmp.0.join("p.ew");
    done(
        &mut shell,
        "file.new",
        json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
    );
    done(&mut shell, "props.project", json!({}));
    assert_eq!(shown(&shell, CREATED), "04-03-2026", "created on the session's date");
    let Outcome::Failed { message } =
        shell.dispatch("props.set", &json!({"row": EDITED, "value": "01-01-2026"}))
    else {
        panic!("the edited date is read-only");
    };
    assert!(message.contains("set when the project is saved"), "{message}");
    // A change, then a save on another day: the edited date is that day.
    done(&mut shell, "page.new", json!({"folio": "2"}));
    shell.set_today("2026-03-05".to_owned());
    done(&mut shell, "file.save", json!({}));
    assert!(!shell.is_dirty(), "the date goes with the save");
    done(&mut shell, "props.project", json!({}));
    assert_eq!(shown(&shell, EDITED), "05-03-2026");
    // A save without a change keeps it.
    shell.set_today("2026-03-06".to_owned());
    let rev = shell.revision();
    done(&mut shell, "file.save", json!({}));
    assert_eq!(shell.revision(), rev, "nothing written");
    done(&mut shell, "props.project", json!({}));
    assert_eq!(shown(&shell, EDITED), "05-03-2026");
    shell.shutdown();
}
