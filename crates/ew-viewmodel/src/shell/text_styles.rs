// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text styles (Project ▸ Text styles…, `text_styles.open`; ADR-0032, ADR-0028 amendment 2; doc 16 §3.7).
//!
//! One table of the project's text styles, grouped by what a person sees on a drawing (Devices, Wires and cables,
//! Terminals, Page, Cover and sections), each style by its name and where it prints, never by its key. Per style:
//! the **height** of a capital letter in millimetres, as a person measures it on paper and as ISO 3098 names a text's
//! size (a list of the standard heights 1.8, 2.5, 3.5, 5 and 7 mm, or any typed value); the **typeface**, one of the
//! bundled faces (system fonts are not offered: a drawing looks the same on every machine); **bold**; a sample; and
//! **Reset**, back to what a new project has. A height under the readable 1.2 mm is allowed and noted at the row in the
//! words of EW-CORE-061. **Standard sizes** sets every height to a new project's; **Reset all** every style.
//!
//! Every change is one `project.settings.set {member: "text_styles"}`: one undo step, and the open page shows it at
//! once. The styles are the project's; a template carries them to the projects made from it.
//!
//! Above the styles, the *Standard typeface* line (doc 09 §4.8): Noto Sans, or a font file the project holds, which then
//! prints every style of the Standard typeface ([`ShellModel::typeface_line`]); **Choose a font file…**
//! (`text_styles.typeface {path}`) and **Use Noto Sans** (`text_styles.typeface_clear`) are one `project.typeface.set`
//! each.

use ew_api::{
    AnyCommand, EffectiveStyle, MIN_CAP_HEIGHT, TEXT_STYLE_MAX_SIZE, TEXT_STYLE_MIN_SIZE, TYPEFACES,
    TextStyleGroup, TextWeight, Typeface, cap_height, em_for_cap, standard_text_style, text_style_entry,
    text_style_group,
};
use std::io::Read as _;

use ew_api::output::TypefaceState;
use ew_units::Nm;
use serde_json::{Map, Value};

use super::{Dialog, Outcome, Prompt, ShellModel, failed, string_param};
use crate::key;
use crate::strings::{self, Key};

/// The heights a person picks from, in mm of a capital letter (ISO 3098's nominal sizes).
pub const STANDARD_HEIGHTS: [&str; 5] = ["1.8", "2.5", "3.5", "5", "7"];

/// The groups of the page, in its order.
pub const GROUPS: [TextStyleGroup; 5] = [
    TextStyleGroup::Devices,
    TextStyleGroup::Wires,
    TextStyleGroup::Terminals,
    TextStyleGroup::Page,
    TextStyleGroup::Cover,
];

/// A group's heading.
pub fn group_words(group: TextStyleGroup) -> &'static str {
    strings::text(match group {
        TextStyleGroup::Devices => key!("text-styles-group-devices"),
        TextStyleGroup::Wires => key!("text-styles-group-wires"),
        TextStyleGroup::Terminals => key!("text-styles-group-terminals"),
        TextStyleGroup::Page => key!("text-styles-group-page"),
        TextStyleGroup::Cover => key!("text-styles-group-cover"),
    })
}

/// A typeface's name: the choice word of its member's word (`choice-narrow`), the standard face's for a face of a
/// newer format.
pub fn face_words(face: &Typeface) -> String {
    strings::choice_text(face.word().unwrap_or("standard"))
}

/// A cap height in mm with at most two decimals, trailing zeros left out, as the UI language writes it (`1,8`).
pub fn height_text(cap: Nm) -> String {
    let hundredths = (cap.0 + 5_000) / 10_000;
    let mut text = format!("{}.{:02}", hundredths / 100, hundredths % 100);
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    strings::decimal(&text)
}

/// A typed height (`2.5`, `2,5`, `2.5 mm`) as a cap height in nm; `None` when it is no positive number of mm.
pub fn parse_height(text: &str) -> Option<Nm> {
    let t = text.trim();
    let t = t.strip_suffix("mm").unwrap_or(t).trim().replace(',', ".");
    let (whole, frac) = t.split_once('.').unwrap_or((&t, ""));
    if whole.is_empty() && frac.is_empty()
        || !whole.chars().all(|c| c.is_ascii_digit())
        || !frac.chars().all(|c| c.is_ascii_digit())
        || whole.len() > 4
    {
        return None;
    }
    let whole: i64 = if whole.is_empty() { 0 } else { whole.parse().ok()? };
    let mut micro = String::from(frac);
    micro.truncate(6);
    while micro.len() < 6 {
        micro.push('0');
    }
    let nm = whole * 1_000_000 + micro.parse::<i64>().ok()?;
    (nm > 0).then_some(Nm(nm))
}

/// One style of the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TextStyleRow {
    /// The style's key: the parameter of the page's actions, never shown.
    pub key: String,
    /// Its group.
    pub group: TextStyleGroup,
    /// Its name.
    pub name: String,
    /// Where it prints, in words.
    pub prints: String,
    /// Its height: the cap height in mm as the UI language writes it.
    pub height: String,
    /// The em height it prints with (the sample's size).
    pub em: Nm,
    /// Its typeface.
    pub face: Typeface,
    /// Whether it prints bold: a bold style in a face without a bold cut prints regular (it stays bold in the file, so
    /// it is bold again in a face that has one).
    pub bold: bool,
    /// The quiet note of a height under the readable minimum.
    pub small: Option<String>,
    /// Whether it differs from what a new project has (Reset can be pressed).
    pub changed: bool,
}

impl TextStyleRow {
    /// Why its Bold toggle is greyed: its face has no bold cut; `None` when it has one.
    pub fn bold_reason(&self) -> Option<Key<'static>> {
        (!self.face.has_bold()).then_some(key!("text-styles-bold-none"))
    }
}

/// The page's *Standard typeface* line (doc 09 §4.8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TypefaceLine {
    /// What prints the Standard typeface, in words: Noto Sans, or the family and the file of the project's, or why the
    /// project's file does not print.
    pub words: String,
    /// Whether the project names a font file of its own (Use Noto Sans can be pressed).
    pub own: bool,
    /// The face of the file when it prints: the samples of the Standard styles are drawn in it.
    pub face: Option<ew_dl::FontKey>,
}

/// The Text styles dialog.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TextStylesDialog {
    /// The style the page opened at (a finding's style): it is marked and scrolled to.
    pub focus: Option<String>,
    /// The line after the last change: what changed, or why it did not.
    pub note: Option<String>,
}

/// Where a style prints, in words: a built-in style's `text-styles-prints-<key>`, an imported or a project's own style
/// the texts that name it.
fn prints_words(key: &str) -> &'static str {
    strings::lookup(&format!("text-styles-prints-{}", key.replace('.', "-")))
        .unwrap_or_else(|| strings::text(key!("text-styles-prints-own")))
}

/// The page's rows of `styles` (the project's effective table), in the order of [`GROUPS`] and, inside a group, the
/// table's.
pub fn rows_of(styles: &[EffectiveStyle]) -> Vec<TextStyleRow> {
    let lang = strings::rule_language();
    let mut rows: Vec<TextStyleRow> = styles
        .iter()
        .map(|s| {
            let key = s.key.as_str();
            let cap = cap_height(s.em);
            TextStyleRow {
                key: key.to_owned(),
                group: text_style_group(key),
                name: ew_api::checks::messages::text_style_name(key, lang),
                prints: prints_words(key).to_owned(),
                height: height_text(cap),
                em: s.em,
                face: s.face.clone(),
                bold: s.is_bold() && s.face.has_bold(),
                small: (cap < MIN_CAP_HEIGHT).then(|| {
                    strings::format(
                        key!("text-styles-small"),
                        &[&height_text(cap), &height_text(MIN_CAP_HEIGHT)],
                    )
                }),
                changed: standard_text_style(key).as_ref() != Some(s),
            }
        })
        .collect();
    rows.sort_by_key(|r| GROUPS.iter().position(|g| *g == r.group));
    rows
}

impl ShellModel {
    /// The open project's text styles as the page lists them; empty without a project.
    pub fn text_style_rows(&self) -> Vec<TextStyleRow> {
        let Some(p) = &self.project else {
            return Vec::new();
        };
        rows_of(&ew_api::effective_text_styles(
            &p.session.query().project().settings,
        ))
    }

    /// The line of a text's Properties that names its style and opens Text styles at it ("Style: Device tag · Change
    /// styles…"): a free text's own style (the default one without), a device's tag style; `None` for any other sheet.
    pub fn text_style_link(&self) -> Option<crate::actions::ActionItem> {
        let sheet = self.props.sheet()?;
        let key = match sheet.header.kinds.as_slice() {
            [(kind, 1)] if kind == "page_item" => {
                let row = sheet
                    .sections
                    .iter()
                    .flat_map(|s| &s.rows)
                    .find(|r| r.key == crate::properties::STYLE_ROW)?;
                match &row.value {
                    ew_api::output::RowValue::One { json, .. } => json
                        .as_str()
                        .unwrap_or(ew_api::output::DEFAULT_STYLE_KEY)
                        .to_owned(),
                    ew_api::output::RowValue::Absent => ew_api::output::DEFAULT_STYLE_KEY.to_owned(),
                    ew_api::output::RowValue::Varies { .. } => return None,
                }
            }
            [(kind, 1)] if kind == "device" => "tag.main".to_owned(),
            _ => return None,
        };
        let mut item = self.menu_item("text_styles.open")?;
        let name = ew_api::checks::messages::text_style_name(&key, strings::rule_language());
        item.label = strings::format(key!("text-styles-link"), &[&name]);
        item.params = serde_json::json!({ "style": key });
        Some(item)
    }

    /// `text_styles.open {style?}` (Project ▸ Text styles…, a text's Change styles…, a finding of EW-CORE-061): opens
    /// the page, at `style` when given.
    pub(crate) fn act_text_styles_open(&mut self, p: &Map<String, Value>) -> Outcome {
        if !self.has_project() {
            return Outcome::refused(key!("reason-no-project"));
        }
        let focus = or_return!(string_param(p, "style")).map(str::to_owned);
        if let Some(Dialog::TextStyles(d)) = &mut self.dialog {
            d.focus = focus;
            d.note = None;
            return Outcome::Done;
        }
        self.open_dialog(Dialog::TextStyles(Box::new(TextStylesDialog {
            focus,
            note: None,
        })))
    }

    /// The project's effective style `key`, or the refusal in words.
    fn text_style_of(&self, key: &str) -> Result<EffectiveStyle, Outcome> {
        let Some(p) = &self.project else {
            return Err(Outcome::refused(key!("reason-no-project")));
        };
        ew_api::effective_text_styles(&p.session.query().project().settings)
            .into_iter()
            .find(|s| s.key.as_str() == key)
            .ok_or_else(|| failed(key!("error-text-style-unknown"), &[key]))
    }

    /// Writes `styles` (style key → the style it prints as) into the project's text styles in one step, every other
    /// entry kept; `done` is the note the page shows after it.
    fn commit_text_styles(&mut self, styles: &[EffectiveStyle], done: String) -> Outcome {
        let Some(p) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let mut value = p.session.query().project().settings.text_styles.clone();
        for s in styles {
            let key = s.key.as_str();
            match text_style_entry(key, s, value.get(key)) {
                Some(entry) => {
                    value.insert(s.key.clone(), entry);
                }
                None => {
                    value.remove(key);
                }
            }
        }
        let value = serde_json::to_value(&value).unwrap_or(Value::Null);
        let cmd = AnyCommand {
            name: "project.settings.set".to_owned(),
            params: serde_json::json!({ "member": "text_styles", "value": value }),
        };
        self.commit_noted(&cmd, Some(done))
    }

    /// Commits `cmd` in one step; the page's note is `done`, or why it did not commit.
    fn commit_noted(&mut self, cmd: &AnyCommand, done: Option<String>) -> Outcome {
        let out = self.run_commit(cmd);
        let note = match &out {
            Outcome::Done => done,
            Outcome::Failed { message } => Some(message.clone()),
            _ => None,
        };
        if let Some(Dialog::TextStyles(d)) = &mut self.dialog {
            d.note = note;
        }
        out
    }

    /// The page's *Standard typeface* line (doc 09 §4.8); `None` without a project.
    pub fn typeface_line(&self) -> Option<TypefaceLine> {
        let p = self.project.as_ref()?;
        let (words, own, face) = match p.session.typeface() {
            TypefaceState::Bundled => (
                strings::text(key!("text-styles-typeface-bundled")).to_owned(),
                false,
                None,
            ),
            TypefaceState::Loaded { name, family, face } => (
                strings::format(key!("text-styles-typeface-loaded"), &[&family, &name]),
                true,
                Some(face),
            ),
            TypefaceState::Missing { name } => (
                strings::format(key!("text-styles-typeface-missing"), &[&name]),
                true,
                None,
            ),
            TypefaceState::Unusable { name } => (
                strings::format(key!("text-styles-typeface-unusable"), &[&name]),
                true,
                None,
            ),
        };
        Some(TypefaceLine { words, own, face })
    }

    /// `text_styles.typeface {path}` (Text styles ▸ Choose a font file…): the project's typeface from a font file, in
    /// one step (`project.typeface.set` with the file and its name; doc 09 §4.8). The window asks for the file with its
    /// dialog (`Prompt::OpenFont`). A file that cannot be read or is larger than the 2.5 MiB a project takes is refused
    /// here, naming the file by its name; one that is no usable font, by the command, in its words. The file goes to the
    /// command in parts of one parameter string each ([`ew_api::MAX_PARAM_STRING`]), so a large font is one step as a
    /// small one is, in the window and in a script's command log alike. The page's line names the new typeface; no note
    /// repeats it.
    pub(crate) fn act_text_styles_typeface(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => return self.need_path("text_styles.typeface", Prompt::OpenFont, "path", p),
        };
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let limit = ew_api::MAX_TYPEFACE_BYTES;
        let too_large = |me: &mut ShellModel| {
            me.text_styles_refused(strings::format(
                key!("error-typeface-too-large"),
                &[&name, &(limit / 1024).to_string()],
            ))
        };
        // Only a regular file is read, and no more than the limit and one byte (it may grow after the metadata).
        match std::fs::metadata(&path) {
            Ok(m) if !m.is_file() => {
                let why = strings::text(key!("error-typeface-not-a-file"));
                return self.text_styles_refused(strings::format(key!("error-typeface-file"), &[&name, why]));
            }
            Ok(m) if m.len() > limit as u64 => return too_large(self),
            Ok(_) => {}
            Err(e) => {
                return self.text_styles_refused(strings::format(
                    key!("error-typeface-file"),
                    &[&name, &e.to_string()],
                ));
            }
        }
        let mut bytes = Vec::new();
        if let Err(e) =
            std::fs::File::open(&path).and_then(|f| f.take(limit as u64 + 1).read_to_end(&mut bytes))
        {
            return self.text_styles_refused(strings::format(
                key!("error-typeface-file"),
                &[&name, &e.to_string()],
            ));
        }
        if bytes.len() > limit {
            return too_large(self);
        }
        let text = ew_api::base64_encode(&bytes);
        // The text is ASCII, so every cut is on a character boundary.
        let part = ew_api::MAX_PARAM_STRING;
        let parts: Vec<&str> = (0..text.len())
            .step_by(part)
            .map(|at| &text[at..(at + part).min(text.len())])
            .collect();
        let cmd = AnyCommand {
            name: "project.typeface.set".to_owned(),
            params: serde_json::json!({ "file": parts, "name": name }),
        };
        self.commit_noted(&cmd, None)
    }

    /// `text_styles.typeface_clear` (Text styles ▸ Use Noto Sans): the Standard typeface back to the bundled Noto Sans,
    /// in one step; the page's line says so.
    pub(crate) fn act_text_styles_typeface_clear(&mut self) -> Outcome {
        let cmd = AnyCommand {
            name: "project.typeface.set".to_owned(),
            params: serde_json::json!({ "file": [] }),
        };
        self.commit_noted(&cmd, None)
    }

    /// `text_styles.set {style, height?, face?, bold?}`: one style's height (a cap height in mm, `2.5` or `2,5`), its
    /// typeface (`standard`, `narrow`, `mono`, `technical_drawing`) and whether it is bold, in one step. A height that is no
    /// number, or one outside what prints (0.36 mm to 35.7 mm), is refused in words.
    pub(crate) fn act_text_styles_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(key) = or_return!(string_param(p, "style")).map(str::to_owned) else {
            return failed(key!("error-missing-param"), &["style"]);
        };
        let mut style = or_return!(self.text_style_of(&key));
        if let Some(text) = or_return!(string_param(p, "height")) {
            let Some(cap) = parse_height(text) else {
                return self.text_styles_refused(strings::format(key!("error-text-style-height"), &[text]));
            };
            let em = em_for_cap(cap);
            if em < TEXT_STYLE_MIN_SIZE || em > TEXT_STYLE_MAX_SIZE {
                return self.text_styles_refused(strings::format(
                    key!("error-text-style-height-range"),
                    &[
                        &height_text(cap_height(TEXT_STYLE_MIN_SIZE)),
                        &height_text(cap_height(TEXT_STYLE_MAX_SIZE)),
                    ],
                ));
            }
            style.em = em;
        }
        if let Some(word) = or_return!(string_param(p, "face")) {
            let Some(face) = Typeface::from_word(word) else {
                return failed(key!("error-bad-param"), &["face"]);
            };
            style.face = face;
        }
        match p.get("bold") {
            None => {}
            Some(Value::Bool(b)) => {
                style.weight = if *b { TextWeight::Bold } else { TextWeight::Regular };
            }
            Some(_) => return failed(key!("error-bad-param"), &["bold"]),
        }
        let lang = strings::rule_language();
        let name = ew_api::checks::messages::text_style_name(&key, lang);
        let note = strings::format(
            key!("text-styles-note-set"),
            &[
                &name,
                &height_text(cap_height(style.em)),
                &face_words(&style.face),
            ],
        );
        self.commit_text_styles(&[style], note)
    }

    /// The page's note `message`, as the refusal of the change.
    fn text_styles_refused(&mut self, message: String) -> Outcome {
        if let Some(Dialog::TextStyles(d)) = &mut self.dialog {
            d.note = Some(message.clone());
        }
        Outcome::Failed { message }
    }

    /// `text_styles.reset {style?}`: one style, or without `style` every style, back to what a new project has
    /// (height, typeface and bold), in one step. A style a project added itself keeps what it has.
    pub(crate) fn act_text_styles_reset(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = or_return!(string_param(p, "style")).map(str::to_owned);
        let styles: Vec<EffectiveStyle> = match &key {
            Some(k) => {
                or_return!(self.text_style_of(k));
                standard_text_style(k).into_iter().collect()
            }
            None => self
                .text_style_rows()
                .iter()
                .filter_map(|r| standard_text_style(&r.key))
                .collect(),
        };
        let note = match &key {
            Some(k) => strings::format(
                key!("text-styles-note-reset"),
                &[&ew_api::checks::messages::text_style_name(
                    k,
                    strings::rule_language(),
                )],
            ),
            None => strings::text(key!("text-styles-note-reset-all")).to_owned(),
        };
        self.commit_text_styles(&styles, note)
    }

    /// `text_styles.standard`: every style's height to what a new project has; typefaces and bold stay.
    pub(crate) fn act_text_styles_standard(&mut self) -> Outcome {
        let p = or_return!(crate::shell::gated(&self.project));
        let styles: Vec<EffectiveStyle> =
            ew_api::effective_text_styles(&p.session.query().project().settings)
                .into_iter()
                .filter_map(|mut s| {
                    s.em = standard_text_style(s.key.as_str())?.em;
                    Some(s)
                })
                .collect();
        self.commit_text_styles(
            &styles,
            strings::text(key!("text-styles-note-standard")).to_owned(),
        )
    }
}

/// The faces of the page's typeface list, in order, with their names.
pub fn faces() -> Vec<(Typeface, String)> {
    TYPEFACES.iter().map(|f| (f.clone(), face_words(f))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_read_as_a_person_types_them() {
        assert_eq!(parse_height("2.5"), Some(Nm(2_500_000)));
        assert_eq!(parse_height(" 2,5 mm "), Some(Nm(2_500_000)));
        assert_eq!(parse_height("3"), Some(Nm(3_000_000)));
        assert_eq!(parse_height(".9"), Some(Nm(900_000)));
        assert_eq!(parse_height("1.2345678"), Some(Nm(1_234_567)));
        for bad in ["", "mm", "-1", "0", "abc", "1.2.3", "99999", "1e3"] {
            assert_eq!(parse_height(bad), None, "{bad}");
        }
    }
}
