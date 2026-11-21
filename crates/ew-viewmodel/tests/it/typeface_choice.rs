// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Standard typeface of Text styles (doc 09 §4.8): a font file chosen by its path prints the open page's texts,
//! the line names it, and Use Noto Sans takes it back; a file that is no font, or none at all, is refused in words that
//! name the file by its name. The user's file here is a bundled font file of the repository (Routed Gothic).

use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::json;

use crate::common::{Tmp, done};

const FONT: &[u8] = include_bytes!("../../../../assets/fonts/RoutedGothic-Regular.ttf");

fn page(shell: &ShellModel) -> String {
    format!("{:?}", shell.page_content("101").unwrap().pages[0].content)
}

fn line(shell: &ShellModel) -> String {
    shell.typeface_line().expect("a project").words
}

#[test]
fn a_font_file_prints_the_page_and_use_noto_sans_takes_it_back() {
    let tmp = Tmp::new("typeface-choice");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    assert_eq!(line(&shell), "Noto Sans, which comes with the program");
    let before = page(&shell);
    let font = tmp.0.join("RoutedGothic-Regular.ttf");
    std::fs::write(&font, FONT).unwrap();
    done(&mut shell, "text_styles.open", json!({}));
    done(
        &mut shell,
        "text_styles.typeface",
        json!({"path": font.display().to_string()}),
    );
    assert_eq!(
        line(&shell),
        "Routed Gothic (RoutedGothic-Regular.ttf), a font file the project holds"
    );
    assert!(shell.typeface_line().unwrap().own);
    assert_ne!(page(&shell), before, "the open page prints in it");
    done(&mut shell, "text_styles.typeface_clear", json!({}));
    assert_eq!(line(&shell), "Noto Sans, which comes with the program");
    assert_eq!(page(&shell), before);
}

#[test]
fn a_file_that_is_no_font_is_refused_in_words() {
    let tmp = Tmp::new("typeface-refused");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    let not_a_font = tmp.0.join("notes.ttf");
    std::fs::write(&not_a_font, b"just some text").unwrap();
    let gone = tmp.0.join("gone.ttf");
    let cases = [
        (not_a_font, "no font of one typeface"),
        (gone, "Cannot read the font file gone.ttf"),
    ];
    for (path, words) in cases {
        let out = shell.dispatch("text_styles.typeface", &json!({"path": path.display().to_string()}));
        let Outcome::Failed { message } = out else {
            panic!("{path:?}: {out:?}");
        };
        assert!(message.contains(words), "{message}");
        assert!(!message.contains(&tmp.0.display().to_string()), "no folder in {message}");
    }
    assert_eq!(line(&shell), "Noto Sans, which comes with the program");
}
