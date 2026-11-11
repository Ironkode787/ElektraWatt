// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A command's own refusal reaches the Dutch window in Dutch, whole, a record
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

/// A refusal (a point that defines a potential already), a refusal with a folio, and one that names a position
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
        "de positie op x 2645833333447.1, y 37 mm ligt buiten het tekenbereik; kies een punt dichter bij de \
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
        "the position at x 2645833333447.1, y 37 mm lies outside the drawable range; move it nearer the page origin"
    );
    shell.shutdown();
}

/// `ew-api`'s own refusals carry their keyed text: a purge of a project with unsaved
/// changes reaches the Dutch window in Dutch, and the English one in English.
#[test]
fn the_api_s_own_refusals_show_in_the_window_language() {
    let tmp = Tmp::new("api-refusals-nl");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    let tables = shell.canvas().unwrap().page().tables.clone();
    let junction = tables.junctions.first().expect("a junction on folio 102").id;
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "potential.define", "params": {"at": {"junction": {"junction": junction.to_string()}},
            "name": "FT1421"}}),
    );
    let purge = |shell: &mut ShellModel| match shell.dispatch("project.purge_assets", &json!({})) {
        Outcome::Failed { message } => message,
        other => panic!("project.purge_assets: {other:?}"),
    };
    assert!(strings::set_locale("nl"));
    assert_eq!(
        purge(&mut shell),
        "Het opruimen is geweigerd: het project heeft niet-opgeslagen wijzigingen; sla op vóór het opruimen"
    );
    assert!(strings::set_locale("en"));
    assert_eq!(
        purge(&mut shell),
        "The purge was refused: the project has unsaved changes; save before a purge"
    );
    shell.shutdown();
}

/// A route whose end misses its junction is refused with the junction in the window's own
/// word for it, "aftakking" in Dutch.
#[test]
fn a_route_off_its_junction_is_refused_with_the_aftakking() {
    let tmp = Tmp::new("junction-nl");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let tables = shell.canvas().unwrap().page().tables.clone();
    let c = tables
        .connections
        .iter()
        .find(|c| {
            matches!(c.a, ew_viewmodel::ew_api::output::TableEnd::Cp { .. })
                && matches!(c.b, ew_viewmodel::ew_api::output::TableEnd::Junction { .. })
                && c.polyline.len() == 2
        })
        .expect("a straight wire from a CP to a junction on folio 102")
        .clone();
    let (a, b) = (c.polyline[0], c.polyline[1]);
    // The same straight line, one grid step short of the junction.
    let step = if a.0 == b.0 {
        (0, (a.1 - b.1).signum() * 2_645_833)
    } else {
        ((a.0 - b.0).signum() * 2_645_833, 0)
    };
    let route = json!({"connection": c.id.to_string(), "route": [[a.0, a.1], [b.0 + step.0, b.1 + step.1]]});
    assert!(strings::set_locale("nl"));
    assert_eq!(
        refused(&mut shell, "connection.route", route.clone()),
        "het einde van de draad op x 116.4, y 52.9 mm op pagina 102 sluit niet aan op de aftakking op x 116.4, y \
         55.6 mm; laat de draad op de aftakking eindigen"
    );
    assert!(strings::set_locale("en"));
    assert_eq!(
        refused(&mut shell, "connection.route", route),
        "the wire's end at x 116.4, y 52.9 mm on page 102 does not meet the junction at x 116.4, y 55.6 mm; end the \
         wire at its junction"
    );
    shell.shutdown();
}
