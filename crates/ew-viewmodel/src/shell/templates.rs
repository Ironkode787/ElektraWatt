// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! New project, the templates and the check level (FR-PRJ-009, FR-PRJ-010, FR-MAC-011; doc 16 §3.7, §9.2).
//!
//! **New project** (`file.new` in the window) is one dialog that says what the project starts with: its name, the
//! folder it is made in, and under Start from the empty project, each template of the list and Another project…, each
//! line with what it brings in words. The pages of a template or a project, and the devices on them, come along only
//! when Also copy its pages is ticked; otherwise the project starts with one empty page. The template marked for new
//! projects is the line chosen when the dialog opens, else the empty project.
//!
//! **Templates** (`templates.open`): the template folders the user keeps (`Settings::templates`) with what each holds,
//! and for the selected one Use for new projects or Do not use, Edit, Rename and Remove from the list (the folder
//! stays on the disk); New template… (the wizard) and Add a folder…. **Save as template…** (`project.save_as_template`)
//! makes a template of the open project: its settings, frames, text styles, sections and structure always, its pages
//! and their devices only when asked. An open template says so in a band across the window
//! ([`ShellModel::editing_template`]) and in the title bar, and it is no recent project.
//!
//! **Check level…** (`project.check_level`) sets how strictly the drawings are checked before they go out, in the
//! words of the wizard's step. Its command is `HumanOnly`: the window sets it, a script is refused.

use std::path::{Path, PathBuf};

use ew_api::{AnyCommand, TemplateSummary, template_summary};
use serde_json::{Map, Value, json};

use super::creation::{FormDialog, FormField};
use super::{Dialog, Mode, Outcome, Prompt, ShellModel, dir_name, failed, open_error_key, string_param};
use crate::key;
use crate::settings::TemplateEntry;
use crate::strings;

/// The check levels a person chooses (the wizard's step and Check level…), from the least strict: only levels that
/// check differently. `drafting`, the message panel's scheme, switches nothing off and so checks as `full` does; a
/// project that names it keeps it, and Check level… shows it as Full.
pub const CHECK_LEVELS: [&str; 2] = ["pre-delivery", "full"];

/// The check level drawings are checked at before they go out when the project names none.
pub const DEFAULT_CHECK_LEVEL: &str = "pre-delivery";

/// A check level's name in words (`choice-check_scheme-<level>`); a level this build does not know by its name.
pub fn level_words(level: &str) -> String {
    strings::lookup(&format!("choice-check_scheme-{level}"))
        .unwrap_or(level)
        .to_owned()
}

/// The offered level ([`CHECK_LEVELS`]) a project's level shows as: itself, else the offered level whose scheme
/// switches off the same rules (an older project's `drafting` shows as `full`), else the default.
fn offered_level(level: &str) -> &'static str {
    let off = |name: &str| ew_api::checks::scheme(name).map(|s| s.off);
    CHECK_LEVELS
        .iter()
        .find(|l| **l == level || off(level).is_some_and(|o| off(l) == Some(o)))
        .copied()
        .unwrap_or(DEFAULT_CHECK_LEVEL)
}

/// What a check level stops, in one sentence (`check-level-<level>`); `None` for a level this build does not know.
pub fn level_sentence(level: &str) -> Option<&'static str> {
    strings::lookup(&format!("check-level-{level}"))
}

/// What a template or a project brings to a new project, in words: its frames, its check level, its text styles, its
/// sections and its pages.
pub fn brings(s: &TemplateSummary) -> String {
    let mut parts = vec![
        strings::text(if s.own_frames {
            key!("template-own-frames")
        } else {
            key!("template-program-frames")
        })
        .to_owned(),
        strings::format(
            key!("template-check-level"),
            &[&level_words(
                s.check_level.as_deref().unwrap_or(DEFAULT_CHECK_LEVEL),
            )],
        ),
    ];
    if s.text_styles {
        parts.push(strings::text(key!("template-text-styles")).to_owned());
    }
    if s.sections > 0 {
        parts.push(strings::format_count(
            crate::count_key!("template-sections"),
            s.sections,
        ));
    }
    parts.push(if s.pages == 0 {
        strings::text(key!("template-no-pages")).to_owned()
    } else {
        strings::format_count(crate::count_key!("template-pages"), s.pages)
    });
    // The line starts as a sentence does.
    let line = parts.join(" · ");
    let mut chars = line.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

/// One line of New project's Start from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StartLine {
    /// The template or project folder; `None` for the empty project, and for Another project… before one is chosen.
    pub dir: Option<PathBuf>,
    /// Its name: Empty project, the template's name, Another project… or the chosen project's folder name.
    pub name: String,
    /// What it brings, in words, or why it cannot be read.
    pub what: String,
    /// The pages with something on them: Also copy its pages is offered for a line that has some.
    pub pages: usize,
    /// Whether it is the template new projects start from.
    pub star: bool,
    /// Whether it is the line Another project….
    pub other: bool,
    /// Whether it can be read (a template folder that is gone cannot).
    pub ok: bool,
}

/// The New project dialog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NewProjectDialog {
    /// The project's name, its folder's name.
    pub name: String,
    /// The folder the project's folder is made in.
    pub folder: String,
    /// The lines of Start from: the empty project, the templates, Another project….
    pub starts: Vec<StartLine>,
    /// The chosen line.
    pub chosen: usize,
    /// Whether Also copy its pages is ticked.
    pub pages: bool,
    /// The unsaved changes of the open project were answered with Don't save.
    discard: bool,
    /// Why the last Create failed.
    pub error: Option<String>,
}

/// Whether `name` can be a folder's name on every system the program runs on: not empty, none of `\ / : * ? " < > |`
/// or a control character, not `.` or `..`, and no point or space at its end.
fn folder_name_ok(name: &str) -> bool {
    !name.is_empty()
        && !name.chars().any(|c| c.is_control() || r#"\/:*?"<>|"#.contains(c))
        && !name.ends_with(['.', ' '])
}

/// Whether `dir` holds anything (a folder of that name is there and not empty, or a file is).
fn taken(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(mut entries) => entries.next().is_some(),
        Err(_) => dir.exists(),
    }
}

impl NewProjectDialog {
    /// The chosen line.
    pub fn start(&self) -> &StartLine {
        &self.starts[self.chosen.min(self.starts.len() - 1)]
    }

    /// The folder the project is made in: the folder and the name.
    pub fn target(&self) -> PathBuf {
        Path::new(self.folder.trim()).join(self.name.trim())
    }

    /// Whether the chosen line has pages to copy and Also copy its pages is ticked.
    pub fn copies_pages(&self) -> bool {
        self.pages && self.start().pages > 0
    }

    /// Why Create cannot run now, in words; `None` when it can.
    pub fn problem(&self) -> Option<String> {
        let name = self.name.trim();
        let folder = self.folder.trim();
        let key = if name.is_empty() {
            key!("new-project-need-name")
        } else if !folder_name_ok(name) {
            key!("new-project-bad-name")
        } else if folder.is_empty() {
            key!("new-project-need-folder")
        } else if !Path::new(folder).is_absolute() {
            key!("new-project-folder-whole")
        } else if self.start().other && self.start().dir.is_none() {
            key!("new-project-need-other")
        } else if !self.start().ok {
            key!("new-project-start-unreadable")
        } else if taken(&self.target()) {
            key!("new-project-taken")
        } else {
            return None;
        };
        Some(strings::text(key).to_owned())
    }

    /// The parameters of `file.new` that Create dispatches.
    pub fn create_params(&self) -> Value {
        let template = self
            .start()
            .dir
            .as_ref()
            .map_or_else(|| "builtin".to_owned(), |d| d.to_string_lossy().into_owned());
        json!({
            "dir": self.target().to_string_lossy(),
            "template": template,
            "pages": self.copies_pages(),
            "discard": self.discard,
        })
    }
}

/// One template of the Templates dialog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TemplateRow {
    /// Its name.
    pub name: String,
    /// Its folder.
    pub dir: String,
    /// What it holds, in words, or why it cannot be read.
    pub what: String,
    /// Whether new projects start from it.
    pub star: bool,
}

/// The Templates dialog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TemplatesDialog {
    /// The templates of the list, in its order.
    pub rows: Vec<TemplateRow>,
    /// The selected row (0-based); its actions are under the list.
    pub selected: usize,
    /// The row whose name is being changed.
    pub renaming: Option<usize>,
    /// The line under the list after an action: what changed, or why it did not.
    pub note: Option<String>,
}

impl TemplatesDialog {
    /// The selected row, when the list has one.
    pub fn selected_row(&self) -> Option<&TemplateRow> {
        self.rows.get(self.selected)
    }
}

/// What a folder holds as a template, in words, or why it cannot be read; with its pages.
fn summary_words(dir: &Path) -> (String, usize, bool) {
    match template_summary(dir) {
        Ok(s) => (brings(&s), s.pages, true),
        Err(e) => (
            strings::format(key!("template-unreadable"), &[strings::text(open_error_key(&e))]),
            0,
            false,
        ),
    }
}

/// The 1-based parameter `name` of a list of `len` rows as a 0-based index.
fn index_param(p: &Map<String, Value>, name: &str, len: usize) -> Result<usize, Outcome> {
    match p.get(name).map(Value::as_u64) {
        None => Err(failed(key!("error-missing-param"), &[name])),
        Some(Some(i)) => match usize::try_from(i) {
            Ok(i) if (1..=len).contains(&i) => Ok(i - 1),
            _ => Err(failed(key!("error-bad-param"), &[name])),
        },
        Some(None) => Err(failed(key!("error-bad-param"), &[name])),
    }
}

/// A yes-or-no parameter that a form sends as its text (`"true"`) or a script as a boolean; absent is `default`.
fn yes_no(p: &Map<String, Value>, name: &str, default: bool) -> Result<bool, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(Value::String(s)) if s == "true" => Ok(true),
        Some(Value::String(s)) if s == "false" => Ok(false),
        Some(_) => Err(failed(key!("error-bad-param"), &[name])),
    }
}

/// The action of Save as template…'s form.
const SAVE_AS_TEMPLATE: &str = "project.save_as_template";

/// The action of Check level…'s form.
const CHECK_LEVEL: &str = "project.check_level";

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // New project

    /// The lines of New project's Start from: the empty project, each template of the list, Another project….
    fn start_lines(&self) -> Vec<StartLine> {
        let mut lines = vec![StartLine {
            dir: None,
            name: strings::text(key!("new-project-empty")).to_owned(),
            what: strings::text(key!("new-project-empty-what")).to_owned(),
            pages: 0,
            star: false,
            other: false,
            ok: true,
        }];
        for t in &self.settings.templates {
            let dir = PathBuf::from(&t.dir);
            let (what, pages, ok) = summary_words(&dir);
            lines.push(StartLine {
                star: self.settings.basic_project.as_deref() == Some(t.dir.as_str()),
                dir: Some(dir),
                name: t.name.clone(),
                what,
                pages,
                other: false,
                ok,
            });
        }
        lines.push(StartLine {
            dir: None,
            name: strings::text(key!("new-project-other")).to_owned(),
            what: strings::text(key!("new-project-other-what")).to_owned(),
            pages: 0,
            star: false,
            other: true,
            ok: true,
        });
        lines
    }

    /// Opens the New project dialog: the template for new projects chosen (else the empty project), the folder of the
    /// last project (else the projects folder) and a name no folder there has.
    pub(super) fn open_new_project(&mut self, discard: bool) -> Outcome {
        let starts = self.start_lines();
        let chosen = starts.iter().position(|l| l.star && l.ok).unwrap_or(0);
        let folder = self
            .settings
            .recent_projects
            .first()
            .and_then(|r| Path::new(r).parent().map(Path::to_path_buf))
            .or_else(|| self.config.projects_dir.clone())
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default();
        let base = strings::text(key!("new-project-default-name"));
        let name = (1..100)
            .map(|n| {
                if n == 1 {
                    base.to_owned()
                } else {
                    format!("{base} {n}")
                }
            })
            .find(|n| folder.is_empty() || !taken(&Path::new(&folder).join(n)))
            .unwrap_or_else(|| base.to_owned());
        self.open_dialog(Dialog::NewProject(Box::new(NewProjectDialog {
            name,
            folder,
            starts,
            chosen,
            pages: false,
            discard,
            error: None,
        })))
    }

    /// The open New project dialog.
    fn new_project_dialog(&mut self) -> Result<&mut NewProjectDialog, Outcome> {
        match &mut self.dialog {
            Some(Dialog::NewProject(d)) => Ok(d),
            _ => Err(Outcome::refused(key!("reason-no-new-project"))),
        }
    }

    /// `new_project.set {name?, folder?, start?, pages?}`: a field of the New project dialog. `start` is the line of
    /// Start from (1-based); Another project… without a project asks for one.
    pub(crate) fn act_new_project_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let name = or_return!(string_param(p, "name")).map(str::to_owned);
        let folder = or_return!(string_param(p, "folder")).map(str::to_owned);
        let pages = match p.get("pages") {
            None => None,
            Some(_) => Some(or_return!(yes_no(p, "pages", false))),
        };
        let d = or_return!(self.new_project_dialog());
        let start = match p.contains_key("start") {
            true => Some(or_return!(index_param(p, "start", d.starts.len()))),
            false => None,
        };
        if let Some(n) = name {
            d.name = n;
        }
        if let Some(f) = folder {
            d.folder = f;
        }
        if let Some(on) = pages {
            d.pages = on;
        }
        d.error = None;
        if let Some(i) = start {
            d.chosen = i;
            if d.starts[i].other && d.starts[i].dir.is_none() {
                return self.need_path("new_project.other", Prompt::OpenFolder, "dir", &Map::new());
            }
        }
        Outcome::Done
    }

    /// `new_project.browse {folder?}`: Browse… beside the folder; without `folder` the window asks for one.
    pub(crate) fn act_new_project_browse(&mut self, p: &Map<String, Value>) -> Outcome {
        or_return!(self.new_project_dialog());
        match or_return!(string_param(p, "folder")) {
            None => self.need_path("new_project.browse", Prompt::Folder, "folder", p),
            Some(folder) => {
                let folder = folder.to_owned();
                let d = or_return!(self.new_project_dialog());
                d.folder = folder;
                d.error = None;
                Outcome::Done
            }
        }
    }

    /// `new_project.other {dir?}`: Another project…, the project to start from; without `dir` the window asks for
    /// one. A folder that holds no readable project is refused in words.
    pub(crate) fn act_new_project_other(&mut self, p: &Map<String, Value>) -> Outcome {
        or_return!(self.new_project_dialog());
        let Some(dir) = or_return!(string_param(p, "dir")).map(PathBuf::from) else {
            return self.need_path("new_project.other", Prompt::OpenFolder, "dir", p);
        };
        let summary = template_summary(&dir);
        let d = or_return!(self.new_project_dialog());
        let Some(i) = d.starts.iter().position(|l| l.other) else {
            return Outcome::Done;
        };
        match summary {
            Ok(s) => {
                d.starts[i] = StartLine {
                    name: dir_name(&dir),
                    what: brings(&s),
                    pages: s.pages,
                    dir: Some(dir),
                    star: false,
                    other: true,
                    ok: true,
                };
                d.chosen = i;
                d.error = None;
                Outcome::Done
            }
            Err(e) => {
                let message = strings::format(
                    key!("new-project-other-unreadable"),
                    &[strings::text(open_error_key(&e))],
                );
                d.error = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Templates

    /// The Templates dialog of the settings' list, row `selected` selected, with `note` under the list.
    fn templates_dialog(&self, selected: usize, note: Option<String>) -> TemplatesDialog {
        let rows: Vec<TemplateRow> = self
            .settings
            .templates
            .iter()
            .map(|t| TemplateRow {
                what: summary_words(Path::new(&t.dir)).0,
                star: self.settings.basic_project.as_deref() == Some(t.dir.as_str()),
                name: t.name.clone(),
                dir: t.dir.clone(),
            })
            .collect();
        TemplatesDialog {
            selected: selected.min(rows.len().saturating_sub(1)),
            rows,
            renaming: None,
            note,
        }
    }

    /// Shows the Templates dialog again after a change of the list: row `selected`, `note` under the list.
    fn show_templates(&mut self, selected: usize, note: Option<String>) -> Outcome {
        let dialog = self.templates_dialog(selected, note);
        if let Some(Dialog::Templates(d)) = &mut self.dialog {
            **d = dialog;
            Outcome::Done
        } else {
            self.open_dialog(Dialog::Templates(Box::new(dialog)))
        }
    }

    /// `templates.open` (Templates… on the start page and the Project tab).
    pub(crate) fn act_templates_open(&mut self) -> Outcome {
        let selected = self
            .settings
            .templates
            .iter()
            .position(|t| self.settings.basic_project.as_deref() == Some(t.dir.as_str()))
            .unwrap_or(0);
        self.open_dialog(Dialog::Templates(Box::new(self.templates_dialog(selected, None))))
    }

    /// The template of a 1-based `index` parameter: its 0-based index and entry.
    fn template_param(&self, p: &Map<String, Value>) -> Result<(usize, TemplateEntry), Outcome> {
        let i = index_param(p, "index", self.settings.templates.len())?;
        Ok((i, self.settings.templates[i].clone()))
    }

    /// `templates.select {index}`: the row whose actions the dialog shows; a rename in progress ends.
    pub(crate) fn act_templates_select(&mut self, p: &Map<String, Value>) -> Outcome {
        let (i, _) = or_return!(self.template_param(p));
        if let Some(Dialog::Templates(d)) = &mut self.dialog {
            d.selected = i;
            d.renaming = None;
        }
        Outcome::Done
    }

    /// `templates.use {index, on?}`: new projects start from the template (`on`, the default), or no longer do.
    pub(crate) fn act_templates_use(&mut self, p: &Map<String, Value>) -> Outcome {
        let (i, t) = or_return!(self.template_param(p));
        let on = or_return!(yes_no(p, "on", true));
        let note = if on {
            self.settings.basic_project = Some(t.dir.clone());
            strings::format(key!("templates-note-used"), &[&t.name])
        } else {
            if self.settings.basic_project.as_deref() == Some(t.dir.as_str()) {
                self.settings.basic_project = None;
            }
            strings::format(key!("templates-note-not-used"), &[&t.name])
        };
        self.save_settings();
        self.show_templates(i, Some(note))
    }

    /// `templates.edit {index}`: opens the template to change it; the band across the window says it is a template.
    pub(crate) fn act_templates_edit(&mut self, p: &Map<String, Value>) -> Outcome {
        let (_, t) = or_return!(self.template_param(p));
        if self.has_project()
            && let Some(outcome) = self.guard_unsaved("templates.edit", p)
        {
            return outcome;
        }
        self.dialog = None;
        self.open_dir(Path::new(&t.dir), false)
    }

    /// `templates.rename {index, name?}`: without `name` the row's name becomes a field; with it, the list shows the
    /// template by that name (its folder keeps its own).
    pub(crate) fn act_templates_rename(&mut self, p: &Map<String, Value>) -> Outcome {
        let (i, t) = or_return!(self.template_param(p));
        let Some(name) = or_return!(string_param(p, "name")).map(str::trim) else {
            if let Some(Dialog::Templates(d)) = &mut self.dialog {
                d.selected = i;
                d.renaming = Some(i);
                d.note = None;
            }
            return Outcome::Done;
        };
        if name.is_empty() {
            let message = strings::text(key!("templates-need-name")).to_owned();
            if let Some(Dialog::Templates(d)) = &mut self.dialog {
                d.note = Some(message.clone());
            }
            return Outcome::Failed { message };
        }
        let name = name.to_owned();
        self.settings.templates[i].name.clone_from(&name);
        self.save_settings();
        self.show_templates(
            i,
            Some(strings::format(key!("templates-note-renamed"), &[&t.name, &name])),
        )
    }

    /// `templates.remove {index}`: takes the template out of the list; its folder stays on the disk, and the dialog
    /// says so.
    pub(crate) fn act_templates_remove(&mut self, p: &Map<String, Value>) -> Outcome {
        let (i, t) = or_return!(self.template_param(p));
        self.settings.remove_template(i);
        self.save_settings();
        self.show_templates(
            i,
            Some(strings::format(key!("templates-note-removed"), &[&t.name])),
        )
    }

    /// `templates.add {dir?}`: Add a folder…, a template somebody gave; without `dir` the window asks for it. A folder
    /// that holds no readable project is refused in words.
    pub(crate) fn act_templates_add(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(dir) = or_return!(string_param(p, "dir")).map(str::to_owned) else {
            return self.need_path("templates.add", Prompt::OpenFolder, "dir", p);
        };
        if let Err(e) = template_summary(Path::new(&dir)) {
            let message = strings::format(
                key!("templates-add-unreadable"),
                &[strings::text(open_error_key(&e))],
            );
            if let Some(Dialog::Templates(d)) = &mut self.dialog {
                d.note = Some(message.clone());
            }
            return Outcome::Failed { message };
        }
        let i = self.settings.add_template(&dir, None);
        self.save_settings();
        let name = self.settings.templates[i].name.clone();
        self.show_templates(i, Some(strings::format(key!("templates-note-added"), &[&name])))
    }

    /// The name of the open project when it is a template (a folder of the list, or a project of kind template), for
    /// the band across the window and the title bar.
    pub fn editing_template(&self) -> Option<String> {
        let p = self.project.as_ref()?;
        let dir = p.dir.to_string_lossy();
        if let Some(t) = self.settings.templates.iter().find(|t| t.dir == dir) {
            return Some(t.name.clone());
        }
        (p.session.query().project().kind == ew_api::ProjectKind::Basic).then(|| p.name.clone())
    }

    // -----------------------------------------------------------------------------------------------------------
    // Save as template

    /// `project.save_as_template {name?, folder?, pages?, use?}` (Project ▸ Save as template…): writes the open project
    /// as a template into `<folder>/<name>`: its settings, frames, text styles, sections and structure always, its pages
    /// and their devices only with `pages`; the template is added to the list, and new projects start from it with
    /// `use` (on unless it is off). Without `name` the window opens its form; `folder` absent is `Templates` in the
    /// projects folder.
    pub(crate) fn act_save_as_template(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(project_name) = self.project_name().map(str::to_owned) else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let default_folder = self
            .config
            .projects_dir
            .as_ref()
            .map(|d| d.join("Templates").to_string_lossy().into_owned())
            .unwrap_or_default();
        let in_form = self.in_form_of(SAVE_AS_TEMPLATE);
        let name = or_return!(string_param(p, "name")).map(str::trim).unwrap_or("");
        if name.is_empty() && !in_form {
            if self.mode == Mode::Headless {
                return failed(key!("error-missing-param"), &["name"]);
            }
            let field = |name: &'static str, label_key, choices, value: String| FormField {
                name,
                label_key,
                choices,
                options: Vec::new(),
                value,
            };
            let none = &super::choices::NONE;
            let yes_no_field = &super::choices::BOOL;
            return self.open_dialog(Dialog::Form(FormDialog {
                action: SAVE_AS_TEMPLATE,
                title_key: key!("save-template-title"),
                ok_key: key!("save-template-ok"),
                fields: vec![
                    field(
                        "name",
                        key!("form-template-name"),
                        none,
                        strings::format(key!("save-template-default-name"), &[&project_name]),
                    ),
                    field("folder", key!("form-template-folder"), none, default_folder),
                    field(
                        "pages",
                        key!("form-template-pages"),
                        yes_no_field,
                        "false".to_owned(),
                    ),
                    field("use", key!("form-template-use"), yes_no_field, "true".to_owned()),
                ],
                error: None,
                table: None,
            }));
        }
        let name = name.to_owned();
        let pages = or_return!(yes_no(p, "pages", false));
        let use_it = or_return!(yes_no(p, "use", true));
        let folder = or_return!(string_param(p, "folder"))
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .map_or(default_folder, str::to_owned);
        let why = if name.is_empty() {
            Some(key!("new-project-need-name"))
        } else if !folder_name_ok(&name) {
            Some(key!("new-project-bad-name"))
        } else if folder.is_empty() {
            Some(key!("new-project-need-folder"))
        } else if !Path::new(&folder).is_absolute() {
            Some(key!("new-project-folder-whole"))
        } else {
            None
        };
        if let Some(why) = why {
            return self.form_failed(SAVE_AS_TEMPLATE, strings::text(why).to_owned());
        }
        let dir = Path::new(&folder).join(&name);
        if taken(&dir) {
            return self.form_failed(
                SAVE_AS_TEMPLATE,
                strings::text(key!("new-project-taken")).to_owned(),
            );
        }
        let store = self.store_options();
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let written = project
            .session
            .save_as_basic(&dir, !pages, !pages, store)
            .map_err(|e| match e {
                ew_api::SaveAsBasicError::Omit(e) => super::tools::command_error_words(&e),
                ew_api::SaveAsBasicError::Write(e) => strings::text(open_error_key(&e)).to_owned(),
            });
        if let Err(why) = written {
            return self.form_failed(
                SAVE_AS_TEMPLATE,
                strings::format(key!("save-template-failed"), &[&why]),
            );
        }
        let dir_text = dir.to_string_lossy().into_owned();
        self.settings.add_template(&dir_text, Some(name.clone()));
        if use_it {
            self.settings.basic_project = Some(dir_text);
        }
        self.save_settings();
        self.banner = Some(strings::format(
            if use_it {
                key!("banner-template-saved-used")
            } else {
                key!("banner-template-saved")
            },
            &[&name],
        ));
        self.form_done(SAVE_AS_TEMPLATE, Outcome::Done)
    }

    // -----------------------------------------------------------------------------------------------------------
    // Check level

    /// The open project's check level before drawings go out, when it names one, and its online scheme.
    fn project_levels(&self) -> (Option<String>, Option<String>) {
        let Some(p) = &self.project else {
            return (None, None);
        };
        let q = p.session.query();
        let checks = q.project().settings.extra.get(ew_api::checks::CHECKS_MEMBER);
        let member = |m: &str| {
            checks
                .and_then(|c| c.get(m))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        (member("release_scheme"), member("active_scheme"))
    }

    /// `project.check_level {level?}` (Project ▸ Check level…): how strictly the drawings are checked before they go
    /// out, one of [`CHECK_LEVELS`]; without `level` the window opens its form, in the words of the wizard's step. Its
    /// command `project.checks.set` is `HumanOnly`: a script is refused, as the command refuses it.
    pub(crate) fn act_check_level(&mut self, p: &Map<String, Value>) -> Outcome {
        if !self.has_project() {
            return Outcome::refused(key!("reason-no-project"));
        }
        let (release, active) = self.project_levels();
        let level = match or_return!(string_param(p, "level")) {
            Some(l) if CHECK_LEVELS.contains(&l) => l.to_owned(),
            Some(_) => return failed(key!("error-bad-param"), &["level"]),
            None if self.mode == Mode::Gui && !self.in_form_of(CHECK_LEVEL) => {
                return self.open_dialog(Dialog::Form(FormDialog {
                    action: CHECK_LEVEL,
                    title_key: key!("check-level-title"),
                    ok_key: key!("dialog-apply"),
                    fields: vec![FormField {
                        name: "level",
                        label_key: key!("wizard-field-check-scheme"),
                        choices: &super::choices::NONE,
                        options: CHECK_LEVELS
                            .iter()
                            .map(|l| ((*l).to_owned(), level_words(l)))
                            .collect(),
                        value: offered_level(release.as_deref().unwrap_or(DEFAULT_CHECK_LEVEL)).to_owned(),
                    }],
                    error: None,
                    table: None,
                }));
            }
            None => return failed(key!("error-missing-param"), &["level"]),
        };
        let mut params = json!({ "release_scheme": level });
        if let Some(active) = active {
            params["active_scheme"] = Value::String(active);
        }
        let cmd = AnyCommand {
            name: "project.checks.set".to_owned(),
            params,
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            self.banner = Some(strings::format(
                key!("banner-check-level-set"),
                &[&level_words(&level)],
            ));
        }
        self.form_done(CHECK_LEVEL, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every level the window offers checks differently from the others (Drafting and Full checked the same), and a
    /// project that names a level no longer offered shows as the one that checks as it does.
    #[test]
    fn the_offered_check_levels_check_differently() {
        let off: Vec<_> = CHECK_LEVELS
            .iter()
            .map(|l| ew_api::checks::scheme(l).expect("a shipped scheme").off)
            .collect();
        for (i, a) in off.iter().enumerate() {
            for (b, level) in off.iter().zip(CHECK_LEVELS).skip(i + 1) {
                assert_ne!(a, b, "{} and {level} check the same", CHECK_LEVELS[i]);
            }
        }
        assert_eq!(offered_level("drafting"), "full");
        assert_eq!(offered_level("pre-delivery"), "pre-delivery");
        assert_eq!(offered_level("strict-2031"), DEFAULT_CHECK_LEVEL);
    }
}
