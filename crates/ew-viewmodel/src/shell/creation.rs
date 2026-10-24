// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Creating pages, structure nodes and external references from the user interface (FT-408 and FT-409, the named
//! scope items of WP-S2.06 built ahead of that package, decision log ID-619 and ID-623): `page.new`, `structure.new`
//! and `external_ref.new`.
//!
//! **One path.** Each action builds one existing command (`page.create`, `structure.create_node`,
//! `external_ref.create`) and commits it as the property grid does (WP-S2.05 §6.6): `Session::preview_with` once, the
//! prepared preview kept in the host, then the host's commit path (the window `Session::commit`, headless one command-log
//! line through `Session::run_script`, so `--record` and the exposure rules apply). A plain create is a single target
//! and no identifier change on several records, so the policy of FR-UI-044 commits it at once: no card.
//!
//! **Parameters.** A parameter that names a record takes the record's designation as the user types it (`J-E-206`,
//! the node's code), or its alias `@n` or UUID (the `ALIAS_PARAMS` rows); a code that two nodes of the aspect share is
//! refused with the instruction to give the alias. Nothing outside the project is read.
//!
//! **The dialog** (the pattern of Go to page and Save layout as: a small dialog, opened by the action when it is
//! dispatched in the window without its required parameters, never headless). One [`FormDialog`] holds the fields of
//! the action, Enter in a field creates, Escape cancels, Tab moves between the fields; a refusal of the command stays
//! in the dialog under the fields.
//!
//! **After a page is created** the focused tab shows it: a person who creates a page works on it next, and a page
//! change ends a modal tool like any other.

use ew_api::{AnyCommand, DynCommand};
use serde_json::{Map, Value, json};

use super::{Dialog, Mode, Outcome, ShellModel, failed};
use crate::strings;
use crate::tools::PreviewState;

/// The page types `page.new` creates: `PageType`'s snake_case spellings except `report`, whose pages are generated
/// (EW-INV-012). The signature of the action in `actions.rs` lists the same words (a test compares them).
pub const PAGE_TYPES: &[&str] = &[
    "title_page",
    "table_of_contents",
    "section_divider",
    "graphic",
    "single_line",
    "multi_line",
    "overview",
    "bus_topology",
    "fluid",
    "assembly_overview",
    "panel_layout",
    "model_view",
    "external_document",
];

/// The aspects of a structure tree `structure.new` creates a node in (the product aspect is the device tree).
pub const STRUCTURE_KINDS: &[&str] = &[
    "functional_assignment",
    "function",
    "installation_site",
    "location",
    "document_kind",
    "user_defined",
    "type",
];

/// The prefix sign of each structure aspect (doc 03 §5, `Aspect::default_sign`), the longer signs first so that `==` is
/// read before `=` and `++` before `+`.
const SIGNS: [(&str, &str); 7] = [
    ("==", "functional_assignment"),
    ("++", "installation_site"),
    ("=", "function"),
    ("+", "location"),
    ("&", "document_kind"),
    ("#", "user_defined"),
    ("%", "type"),
];

/// `text` as the code of a node of `aspect`: the aspect's own prefix sign (`+J-E-206`) is taken off, another aspect's sign
/// is refused with a text naming both, and any sign left in the code (`+K1+K2`) is refused: a designation names one
/// aspect, so it holds one sign.
fn strip_sign(aspect: &str, text: &str) -> Result<String, String> {
    let code = match SIGNS.iter().find(|e| text.starts_with(e.0)) {
        Some(&(sign, of)) if of == aspect => &text[sign.len()..],
        Some(&(sign, of)) => {
            return Err(strings::format(
                "error-wrong-sign",
                &[text, sign, &of.replace('_', " "), &aspect.replace('_', " ")],
            ));
        }
        None => text,
    };
    let code = code.trim();
    if let Some((second, _)) = SIGNS.iter().find(|(sign, _)| code.contains(sign)) {
        return Err(strings::format("error-second-sign", &[text, second]));
    }
    Ok(code.to_owned())
}

/// The codes from the root to node `id` in `nodes` (id, code, parent id).
fn node_path(nodes: &[(String, String, Option<String>)], id: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = Some(id.to_owned());
    while let Some(c) = cur.take() {
        let Some(n) = nodes.iter().find(|n| n.0 == c) else {
            break;
        };
        out.push(n.1.clone());
        if out.len() < 64 {
            cur = n.2.clone();
        }
    }
    out.reverse();
    out
}

/// One field of a creation action: its parameter, the key of its label, the values a choice field offers (none for
/// free text) and the value the dialog starts with.
#[derive(Clone, Copy)]
struct Field {
    name: &'static str,
    label_key: &'static str,
    choices: &'static [&'static str],
    default: &'static str,
}

/// A creation action's form: its action, the title key and the fields in order.
struct Spec {
    action: &'static str,
    title_key: &'static str,
    fields: &'static [Field],
}

const fn field(name: &'static str, label_key: &'static str) -> Field {
    Field {
        name,
        label_key,
        choices: &[],
        default: "",
    }
}

static PAGE_NEW: Spec = Spec {
    action: "page.new",
    title_key: "new-page-title",
    fields: &[
        field("folio", "form-folio"),
        Field {
            name: "type",
            label_key: "form-page-type",
            choices: PAGE_TYPES,
            default: "multi_line",
        },
        field("title", "form-title"),
        field("section", "form-section"),
    ],
};

static STRUCTURE_NEW: Spec = Spec {
    action: "structure.new",
    title_key: "new-structure-title",
    fields: &[
        Field {
            name: "kind",
            label_key: "form-kind",
            choices: STRUCTURE_KINDS,
            default: "location",
        },
        field("designation", "form-designation"),
        field("parent", "form-parent"),
    ],
};

static EXTERNAL_REF_NEW: Spec = Spec {
    action: "external_ref.new",
    title_key: "new-external-title",
    fields: &[
        field("location", "form-location"),
        field("device", "form-device"),
        field("connection", "form-connection"),
        field("project", "form-project"),
    ],
};

/// One field of an open creation dialog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FormField {
    /// The action parameter the field carries.
    pub name: &'static str,
    /// The key of the field's label.
    pub label_key: &'static str,
    /// The values a choice field offers, in order; empty for a free-text field.
    pub choices: &'static [&'static str],
    /// The text of the field.
    pub value: String,
}

/// A creation dialog: the action Enter dispatches with the fields, the title key, the fields, and the refusal of the
/// last attempt.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FormDialog {
    /// The action the dialog dispatches (`page.new`).
    pub action: &'static str,
    /// The key of the dialog's title.
    pub title_key: &'static str,
    /// The fields in order.
    pub fields: Vec<FormField>,
    /// What the last attempt was refused with, shown under the fields.
    pub error: Option<String>,
}

impl Spec {
    /// The dialog of this form, its fields taken from the parameters given (else their defaults).
    fn dialog(&self, p: &Map<String, Value>) -> FormDialog {
        FormDialog {
            action: self.action,
            title_key: self.title_key,
            fields: self
                .fields
                .iter()
                .map(|f| FormField {
                    name: f.name,
                    label_key: f.label_key,
                    choices: f.choices,
                    value: text_param(p, f.name)
                        .ok()
                        .filter(|v| !v.is_empty())
                        .unwrap_or_else(|| f.default.to_owned()),
                })
                .collect(),
            error: None,
        }
    }
}

/// A text parameter: a string (a number is its decimal text), `""` when it is absent or null; `Err` for any other type.
fn text_param(p: &Map<String, Value>, name: &str) -> Result<String, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => Ok(s.trim().to_owned()),
        Some(Value::Number(n)) => Ok(n.to_string()),
        Some(_) => Err(failed("error-bad-param", &[name])),
    }
}

/// The ID text of the section whose folio range contains `number`, else `None` (a page outside every section is front
/// matter).
fn section_of(shell: &ShellModel, number: u32) -> Option<String> {
    let q = shell.project.as_ref()?.session.query();
    let mut sections: Vec<(u16, u32, u32, String)> = q
        .sections()
        .iter()
        .map(|(id, s)| {
            (
                s.number,
                s.folio_range.0,
                s.folio_range.1,
                id.as_uuid().to_string(),
            )
        })
        .collect();
    sections.sort();
    sections
        .into_iter()
        .find(|(_, first, last, _)| (*first..=*last).contains(&number))
        .map(|(_, _, _, id)| id)
}

/// The folio of `text`: its number is the leading digits; any more text makes the whole text the folio's label
/// (`101a`, `101.1`), as the page order prints it.
fn parse_folio(text: &str) -> Result<(u32, Option<String>), String> {
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    let Ok(number) = digits.parse::<u32>() else {
        return Err(strings::format("error-folio-number", &[text]));
    };
    Ok((number, (text.len() > digits.len()).then(|| text.to_owned())))
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // The dialog

    /// Sets the text of field `name` of the open creation dialog (a keystroke or a choice) and clears the last refusal.
    pub fn set_form_field(&mut self, name: &str, value: &str) {
        if let Some(Dialog::Form(form)) = &mut self.dialog {
            if let Some(f) = form.fields.iter_mut().find(|f| f.name == name) {
                f.value = value.to_owned();
            }
            form.error = None;
        }
    }

    /// Enter in the creation dialog (or its Create button): the dialog's action with its non-empty fields as parameters.
    /// A refusal stays in the dialog; success closes it.
    pub fn submit_form(&mut self) -> Outcome {
        let Some(Dialog::Form(form)) = &self.dialog else {
            return Outcome::Done;
        };
        let action = form.action;
        let params: Map<String, Value> = form
            .fields
            .iter()
            .filter(|f| !f.value.trim().is_empty())
            .map(|f| (f.name.to_owned(), Value::String(f.value.trim().to_owned())))
            .collect();
        let out = self.dispatch(action, &Value::Object(params));
        // The project closed under the open dialog: nothing is left to create in.
        if matches!(out, Outcome::Refused { .. }) && !self.has_project() {
            self.dialog = None;
        }
        out
    }

    /// Whether the open dialog is the form of `spec`.
    fn in_form(&self, spec: &Spec) -> bool {
        matches!(&self.dialog, Some(Dialog::Form(f)) if f.action == spec.action)
    }

    /// A required parameter is missing: in the window the dialog opens (or says what is missing), headless the step
    /// fails.
    fn form_missing(&mut self, spec: &'static Spec, p: &Map<String, Value>, name: &str) -> Outcome {
        match self.mode {
            Mode::Headless => failed("error-missing-param", &[name]),
            Mode::Gui if self.in_form(spec) => {
                self.form_failed(spec, strings::format("error-missing-param", &[name]))
            }
            Mode::Gui => self.open_dialog(Dialog::Form(spec.dialog(p))),
        }
    }

    /// A refusal: it stays in the dialog when the form is open.
    fn form_failed(&mut self, spec: &Spec, message: String) -> Outcome {
        if self.in_form(spec)
            && let Some(Dialog::Form(f)) = &mut self.dialog
        {
            f.error = Some(message.clone());
        }
        Outcome::Failed { message }
    }

    /// The end of a creation: a failure stays in the dialog, a success closes it.
    fn form_done(&mut self, spec: &Spec, out: Outcome) -> Outcome {
        match out {
            Outcome::Failed { message } => self.form_failed(spec, message),
            Outcome::Done => {
                if self.in_form(spec) {
                    self.dialog = None;
                }
                Outcome::Done
            }
            other => other,
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // The commit path

    /// One command through the host with a preview (module documentation): `Session::preview_with` once, kept as the
    /// host keeps a tool's preview, then the host's commit. A refusal of the preview is the outcome's message.
    fn host_create(&mut self, name: &str, params: Value) -> Outcome {
        let cmd = AnyCommand {
            name: name.into(),
            params,
        };
        let json = match cmd.to_json() {
            Ok(v) => v,
            Err(e) => return failed("error-command", &[&e.to_string()]),
        };
        let Some(base_rev) = self.revision() else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let submit = self.tool_submit();
        let Some(project) = &mut self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let result = project.session.preview_with(&cmd, submit);
        self.count(|c| c.preview_with += 1);
        let preview = match result {
            Ok(p) => p,
            Err(e) => return failed("error-command", &[&e.to_string()]),
        };
        // A tool's own kept preview (the armed Place tool's ghost) is put back: it is stale after the commit and the
        // tool computes it again, as it would without this action.
        let tool_preview = self.tools.preview.replace(PreviewState {
            base_rev,
            command: json,
            name: name.to_owned(),
            result: Ok(preview),
            derived: Default::default(),
        });
        let out = self.run_commit(&cmd);
        self.tools.preview = tool_preview;
        out
    }

    /// The UUID text of the structure node `text` names in the aspect `aspect`: its alias or UUID (resolved by
    /// dispatch), or its code (with the aspect's own prefix sign, which is taken off) when exactly one node of the aspect
    /// has it. Several nodes with the code are refused listing them as aliases `@n` with their parent's code, ordered by
    /// their codes from the root (FT-439), because no navigator prints a node's alias yet.
    fn structure_node(&self, aspect: &str, text: &str) -> Result<String, String> {
        let Some(project) = &self.project else {
            return Err(strings::text("reason-no-project").to_owned());
        };
        let whole_uuid =
            crate::alias::find_uuid(text).is_some_and(|(at, u)| at == 0 && u.len() == text.len());
        let code = if whole_uuid {
            text.to_owned()
        } else {
            strip_sign(aspect, text)?
        };
        let q = project.session.query();
        let mut all: Vec<(String, String, Option<String>)> = Vec::new();
        for (id, node) in q.structure_nodes().iter() {
            let node_aspect = serde_json::to_value(&node.aspect).ok();
            if node_aspect.as_ref().and_then(Value::as_str) != Some(aspect) {
                continue;
            }
            all.push((
                id.as_uuid().to_string(),
                node.code.as_str().to_owned(),
                node.parent.map(|p| p.as_uuid().to_string()),
            ));
        }
        let hits: Vec<&(String, String, Option<String>)> = all
            .iter()
            .filter(|n| {
                if whole_uuid {
                    n.0.eq_ignore_ascii_case(&code)
                } else {
                    n.1 == code
                }
            })
            .collect();
        match hits.len() {
            0 => Err(strings::format(
                "error-no-node",
                &[&aspect.replace('_', " "), text],
            )),
            1 => Ok(hits[0].0.clone()),
            n => {
                let mut cands: Vec<(Vec<String>, String)> = hits
                    .iter()
                    .map(|h| (node_path(&all, &h.0), h.0.clone()))
                    .collect();
                cands.sort();
                let list: Vec<String> = cands
                    .iter()
                    .map(
                        |(path, id)| match path.len().checked_sub(2).and_then(|i| path.get(i)) {
                            Some(parent) => strings::format("node-candidate-under", &[id, parent]),
                            None => strings::format("node-candidate-top", &[id]),
                        },
                    )
                    .collect();
                Err(self.alias_text(&strings::format(
                    "error-ambiguous-node",
                    &[&n.to_string(), text, &list.join(", ")],
                )))
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // The actions

    /// `page.new {folio, type?, title?, section?}` (FT-408): one `page.create` with the explicit folio, then the new page
    /// is shown.
    pub(super) fn act_page_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in PAGE_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [folio, ty, title, section] = <[String; 4]>::try_from(v).expect("four fields");
        if folio.is_empty() {
            return self.form_missing(&PAGE_NEW, p, "folio");
        }
        let (number, label) = match parse_folio(&folio) {
            Ok(f) => f,
            Err(message) => return self.form_failed(&PAGE_NEW, message),
        };
        let mut params = json!({ "number": number });
        if let Some(label) = label {
            params["label"] = Value::String(label);
        }
        if !ty.is_empty() {
            if !PAGE_TYPES.contains(&ty.as_str()) {
                return self.form_failed(
                    &PAGE_NEW,
                    strings::format("error-page-type", &[&ty, &PAGE_TYPES.join(", ")]),
                );
            }
            params["kind"] = Value::String(ty);
        }
        if !title.is_empty() {
            params["title"] = Value::String(title);
        }
        if !section.is_empty() {
            let Some(project) = &self.project else {
                return Outcome::Refused {
                    reason_key: "reason-no-project",
                };
            };
            match project.session.resolve_reference(&format!("section:{section}")) {
                Ok(id) => params["section"] = Value::String(id.to_string()),
                Err(message) => return self.form_failed(&PAGE_NEW, message),
            }
        } else if let Some(id) = section_of(self, number) {
            // A new page belongs to the section whose folio range holds its number (window check, item 7): a sub-page
            // of 102 in the range 100–199 sits beside 102 in the navigator, not in the section-less group.
            params["section"] = Value::String(id);
        }
        let out = self.host_create("page.create", params);
        let out = self.form_done(&PAGE_NEW, out);
        if out == Outcome::Done
            && let Some(id) = self.tools.last_primary.clone()
        {
            self.show_page_by_id(&id);
        }
        out
    }

    /// `structure.new {kind, designation, parent?}` (FT-409): one `structure.create_node`.
    pub(super) fn act_structure_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in STRUCTURE_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [kind, designation, parent] = <[String; 3]>::try_from(v).expect("three fields");
        for (name, value) in [("kind", &kind), ("designation", &designation)] {
            if value.is_empty() {
                return self.form_missing(&STRUCTURE_NEW, p, name);
            }
        }
        if !STRUCTURE_KINDS.contains(&kind.as_str()) {
            return self.form_failed(
                &STRUCTURE_NEW,
                strings::format("error-structure-kind", &[&kind, &STRUCTURE_KINDS.join(", ")]),
            );
        }
        let designation = match strip_sign(&kind, &designation) {
            Ok(d) if !d.is_empty() => d,
            Ok(_) => return self.form_missing(&STRUCTURE_NEW, p, "designation"),
            Err(message) => return self.form_failed(&STRUCTURE_NEW, message),
        };
        let mut params = json!({ "aspect": kind, "code": designation });
        if !parent.is_empty() {
            match self.structure_node(&kind, &parent) {
                Ok(id) => params["parent"] = Value::String(id),
                Err(message) => return self.form_failed(&STRUCTURE_NEW, message),
            }
        }
        let out = self.host_create("structure.create_node", params);
        self.form_done(&STRUCTURE_NEW, out)
    }

    /// `external_ref.new {location, device, connection?, project?}` (FT-409): one `external_ref.create` at a location
    /// node.
    pub(super) fn act_external_ref_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in EXTERNAL_REF_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [location, device, connection, project] = <[String; 4]>::try_from(v).expect("four fields");
        for (name, value) in [("location", &location), ("device", &device)] {
            if value.is_empty() {
                return self.form_missing(&EXTERNAL_REF_NEW, p, name);
            }
        }
        let location = match self.structure_node("location", &location) {
            Ok(id) => id,
            Err(message) => return self.form_failed(&EXTERNAL_REF_NEW, message),
        };
        let mut params = json!({ "location": location, "device": device });
        if !connection.is_empty() {
            params["connection"] = Value::String(connection);
        }
        if !project.is_empty() {
            params["project"] = Value::String(project);
        }
        let out = self.host_create("external_ref.create", params);
        self.form_done(&EXTERNAL_REF_NEW, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{find, parse_signature};

    #[test]
    fn the_signatures_list_the_words_the_actions_accept() {
        let enum_of = |action: &str, name: &str| -> Vec<String> {
            let sig = parse_signature(find(action).unwrap().params).unwrap();
            let ty = &sig.iter().find(|p| p.name == name).unwrap().ty;
            ty.strip_prefix("enum(")
                .and_then(|t| t.strip_suffix(')'))
                .unwrap()
                .split('|')
                .map(str::to_owned)
                .collect()
        };
        assert_eq!(enum_of("page.new", "type"), PAGE_TYPES);
        assert_eq!(enum_of("structure.new", "kind"), STRUCTURE_KINDS);
        for spec in [&PAGE_NEW, &STRUCTURE_NEW, &EXTERNAL_REF_NEW] {
            let sig = parse_signature(find(spec.action).unwrap().params).unwrap();
            let names: Vec<&str> = sig.iter().map(|p| p.name.as_str()).collect();
            let fields: Vec<&str> = spec.fields.iter().map(|f| f.name).collect();
            assert_eq!(names, fields, "{}", spec.action);
        }
    }

    #[test]
    fn a_folio_is_its_number_and_a_label_when_it_has_more() {
        assert_eq!(parse_folio("101"), Ok((101, None)));
        assert_eq!(parse_folio("101a"), Ok((101, Some("101a".to_owned()))));
        assert_eq!(parse_folio("101.1"), Ok((101, Some("101.1".to_owned()))));
        assert!(parse_folio("A1").is_err());
        assert!(parse_folio("").is_err());
    }
}
