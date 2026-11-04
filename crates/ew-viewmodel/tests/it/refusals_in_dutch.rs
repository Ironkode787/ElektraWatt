// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1337 (decision log ID-1359 ruling 5): a command's own refusal reaches the Dutch window in Dutch, whole, a record
//! named in words in Dutch too; the same refusal in en-GB is the English text the CLI prints.

use crate::common::{Tmp, done};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

/// The message of `cmd` refused through `command.run`.
fn refused(shell: &mut ShellModel, cmd: &str, params: Value) -> String {
    match shell.dispatch("command.run", &json!({"cmd": cmd, "params": params})) {
        Outcome::Failed { message } => message,
        other => panic!("{cmd} {params}: {other:?}"),
    }
}

/// FT-1241's refusal (a point that defines a potential already), a refusal with a folio, and one that names a position
/// in words: each in Dutch in the Dutch window and in English in the English one.
#[test]
fn a_refused_command_shows_its_reason_in_the_window_language() {
    let tmp = Tmp::new("refusals-nl");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let tables = shell.canvas().unwrap().page().tables.clone();
    let junction = tables.junctions.first().expect("a junction on folio 102").id;
    let at = json!({"junction": {"junction": junction.to_string()}});
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "potential.define", "params": {"at": at, "name": "FT1241"}}),
    );
    let rep = tables
        .reps
        .iter()
        .find(|r| r.device.is_some())
        .expect("a symbol of a device")
        .id;
    let twice = json!({"at": at, "name": "FT1241-B"});
    let view = json!({"page": {"ref": "folio:101"}, "surface": "00000000-0000-0000-0000-0000000000ab",
        "datum_at": [0, 0]});
    let far = json!({"reps": [rep.to_string()], "steps": [1_000_000_000_000_i64, 0]});

    assert!(strings::set_locale("nl"));
    assert_eq!(
        refused(&mut shell, "potential.define", twice.clone()),
        "dit punt definieert de potentiaal FT1241 al; wijzig in plaats daarvan die definitie"
    );
    assert_eq!(
        refused(&mut shell, "page.surface_view.set", view.clone()),
        "folio 101 is geen paneellayoutpagina; een paneelaanzicht hoort op een paneellayoutpagina"
    );
    assert_eq!(
        refused(&mut shell, "rep.move", far.clone()),
        "de positie op x 2645833333441.8, y 29.1 mm ligt buiten het tekenbereik; kies een punt dichter bij de \
         oorsprong van de pagina"
    );

    assert!(strings::set_locale("en"));
    assert_eq!(
        refused(&mut shell, "potential.define", twice),
        "this point already defines the potential FT1241; change that definition instead"
    );
    assert_eq!(
        refused(&mut shell, "page.surface_view.set", view),
        "folio 101 is not a panel-layout page; a surface view goes on a panel-layout page"
    );
    assert_eq!(
        refused(&mut shell, "rep.move", far),
        "the position at x 2645833333441.8, y 29.1 mm lies outside the drawable range; move it nearer the page origin"
    );
    shell.shutdown();
}
