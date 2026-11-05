// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-964, FT-949 and FT-850 (ID-1150): the hints, the HUD's prompts and the toasts take the keys they name from the
//! keymap, so a rebound key shows; a keymap file that is not TOML is refused in words.

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::json;

/// A headless shell with a writable copy of Golden A on 101, at actual size.
fn golden(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

/// Binds `keys` to `action` in the keymap editor (as the window's Keyboard shortcuts dialog does).
fn rebind(s: &mut ShellModel, action: &str, keys: &str) {
    done(s, "help.shortcuts", json!({}));
    done(s, "keymap.record", json!({"action": action, "keys": keys}));
    done(s, "ui.dismiss", json!({}));
}

/// Writes the user's keymap file of `tmp`'s shells with the `[[bind]]` tables `binds`.
fn user_keymap(tmp: &Tmp, binds: &str) {
    let config = tmp.0.join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("keymap.toml"),
        format!("[meta]\nname = \"Mine\"\nversion = 1\n\n{binds}"),
    )
    .unwrap();
}

fn hud(s: &ShellModel) -> String {
    ew_viewmodel::command_bar::hud(s).prompt
}

/// Puts the grid cursor on grid point `k` of the page on show.
fn cursor(s: &mut ShellModel, k: (i64, i64)) {
    let p = {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)))
    };
    done(s, "cursor.set", at(p, json!({})));
}

/// FT-964: with the default keymap the texts read as before; Enter rebound to F9 shows in the Measure and the Place
/// hints, Escape rebound to F10 in the Place hint and, in its short form, in the Place HUD.
#[test]
fn ft964_a_rebound_key_shows_in_the_hints_and_the_hud() {
    let tmp = Tmp::new("ft964-rebind");
    let mut s = golden(&tmp);
    done(&mut s, "tool.measure", json!({}));
    assert_eq!(s.tool_hint(), "Click or Enter at the first point");
    done(&mut s, "ui.dismiss", json!({}));
    let pick = json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"});
    done(&mut s, "place.pick", pick.clone());
    cursor(&mut s, (60, 38));
    assert_eq!(
        s.tool_hint(),
        "Click or Enter to place; R, X, Y and V turn it; Esc ends"
    );
    let prompt = hud(&s);
    assert!(
        prompt.ends_with(" \u{b7} R rotates \u{b7} V variant \u{b7} J Join device\u{2026} \u{b7} Esc done"),
        "{prompt}"
    );

    // Enter in the editor, Escape (hidden from the editor) in the user's keymap file.
    let tmp = Tmp::new("ft964-rebound");
    user_keymap(
        &tmp,
        "[[bind]]\nkeys = \"F10\"\naction = \"ui.dismiss\"\ncontext = \"global\"\n",
    );
    let mut s = golden(&tmp);
    assert_eq!(s.banner(), None, "the keymap file is taken");
    rebind(&mut s, "canvas.activate", "F9");
    done(&mut s, "tool.measure", json!({}));
    assert_eq!(s.tool_hint(), "Click or F9 at the first point");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "place.pick", pick);
    cursor(&mut s, (60, 38));
    assert_eq!(
        s.tool_hint(),
        "Click or F9 to place; R, X, Y and V turn it; F10 ends"
    );
    let rebound = hud(&s);
    assert_eq!(
        rebound,
        prompt.replace("Esc done", "F10 done"),
        "the HUD names the rebound key"
    );
    assert_eq!(
        s.status_fields(false)[0],
        "Click or F9 to place; R, X, Y and V turn it; F10 ends"
    );
}

/// FT-949: the toast of a placement that drew its proposed wires names the key of `canvas.activate_alt` from the
/// keymap, as the hint does; "Alt+Click" stays. The toast with the key rebound is the default's with that key only.
#[test]
fn ft949_the_autoconnect_toast_takes_ctrl_enter_from_the_keymap() {
    /// Places two contacts and a third between them that connects to both; the hint before the third's click and the
    /// toast after it.
    fn place_three(s: &mut ShellModel) -> (String, String) {
        done(
            s,
            "place.pick",
            json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
        );
        let span = 4;
        cursor(s, (60, 38));
        done(s, "canvas.activate", json!({}));
        cursor(s, (60, 38 + 3 * span + 2));
        done(s, "canvas.activate_alt", json!({}));
        cursor(s, (60, 38 + span + 1));
        let hint = s.tool_hint();
        done(s, "canvas.activate", json!({}));
        let toast = s.toast().expect("the placement's toast").text.clone();
        (hint, toast)
    }
    let tmp = Tmp::new("ft949-default");
    let mut s = golden(&tmp);
    let (hint, toast) = place_three(&mut s);
    assert_eq!(
        hint,
        "Click or Enter to place and connect the proposed wires; Alt+Click or Ctrl+Enter places without them; Tab \
         tries the next target; Esc ends"
    );
    assert!(
        toast.ends_with(". Alt+Click or Ctrl+Enter places without connecting"),
        "{toast}"
    );
    // `canvas.activate_alt` is hidden from the editor: the user's keymap file binds it.
    let tmp = Tmp::new("ft949-rebound");
    user_keymap(
        &tmp,
        "[[bind]]\nkeys = \"Ctrl+F9\"\naction = \"canvas.activate_alt\"\ncontext = \"canvas\"\n",
    );
    let mut s = golden(&tmp);
    assert_eq!(s.banner(), None, "the keymap file is taken");
    let key = ew_viewmodel::keymap::display("Ctrl+F9");
    let (rebound_hint, rebound_toast) = place_three(&mut s);
    assert_eq!(rebound_hint, hint.replace("Ctrl+Enter", &key));
    assert_eq!(rebound_toast, toast.replace("Ctrl+Enter", &key));
}

/// FT-850: a keymap file that is not TOML is refused whole, in words.
#[test]
fn ft850_a_keymap_file_that_is_not_toml_is_refused_in_words() {
    let tmp = Tmp::new("ft850-import");
    let mut s = golden(&tmp);
    let bad = tmp.0.join("not-toml.toml");
    std::fs::write(&bad, "[[bind]\nkeys = ").unwrap();
    done(&mut s, "help.shortcuts", json!({}));
    assert_eq!(
        s.dispatch("keymap.import", &json!({"path": bad.to_string_lossy()})),
        Outcome::Failed {
            message: "The keymap file could not be imported: it is not valid TOML.".to_owned()
        }
    );
}

/// ID-1231: the link preview's line without a key for `nav.follow` names the command by its menu path and label as the
/// menu table holds them (no keymap of this build unbinds an action, so the words are pinned to the table here).
#[test]
fn the_link_previews_line_without_a_key_names_the_menu_path() {
    use ew_viewmodel::strings::{Key, text};
    let meta = ew_viewmodel::actions::find("nav.follow").unwrap();
    let slot = meta.menu.as_ref().expect("in a menu");
    assert!(slot.submenu.is_none(), "no submenu between the menu and the item");
    let path = format!(
        "{} ▸ {}",
        text(slot.menu.label_key()),
        text(Key::run_time(&meta.label_key()))
    );
    assert_eq!(
        text(ew_viewmodel::key!("link-preview-go-none")),
        format!("Click the link or use {path} to go there")
    );
    assert_eq!(path, "Page ▸ Follow cross-reference");
    // FT-1189: the canvas link's hint without a key names the same path.
    assert_eq!(
        ew_viewmodel::strings::format(ew_viewmodel::key!("status-hint-follow-none"), &["103-A1"]),
        format!("Click or use {path}: go to 103-A1")
    );
}

/// FT-1042: a link's hint takes the follow key from the keymap, in Properties (`props.follow`) as on the canvas
/// (`nav.follow`), the canvas's also once rebound (the keymap editor does not rebind `props.follow`, a key of the
/// grid's own).
#[test]
fn ft1042_a_links_hint_takes_its_follow_key_from_the_keymap() {
    use ew_viewmodel::status::follow_hint;
    let tmp = Tmp::new("ft1042-hint");
    let mut s = golden(&tmp);
    assert_eq!(
        follow_hint(&s, "-201U7", "props.follow"),
        "Click or F: go to -201U7"
    );
    assert_eq!(
        follow_hint(&s, "103-A1", "nav.follow"),
        "Click or F: go to 103-A1"
    );
    done(&mut s, "help.shortcuts", json!({}));
    done(
        &mut s,
        "keymap.record",
        json!({"action": "nav.follow", "keys": "Ctrl+Shift+F7"}),
    );
    done(&mut s, "ui.dismiss", json!({}));
    let key = ew_viewmodel::keymap::short_display("Ctrl+Shift+F7");
    assert_eq!(
        follow_hint(&s, "103-A1", "nav.follow"),
        format!("Click or {key}: go to 103-A1")
    );
    assert_eq!(
        follow_hint(&s, "-201U7", "props.follow"),
        "Click or F: go to -201U7"
    );
    assert_eq!(
        ew_viewmodel::strings::format(ew_viewmodel::key!("status-hint-link"), &["-201U7"]),
        "Click to go to -201U7"
    );
}
