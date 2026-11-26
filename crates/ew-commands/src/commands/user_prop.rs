// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `user_prop.define`, `user_prop.rename` and `user_prop.remove`: the fields of the project's own, values a title block
//! prints that the program does not know (doc 09 §2.10, FR-PRJ-007).

use ew_ids::Id;
use ew_l10n::{List, Text, TextArg};
use ew_mltext::{Ident, MlText};
use ew_schema::{Page, UserPropDef, UserPropKind, UserPropScope, UserPropValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{IdentS, MlTextS, UuidS};
use crate::tx::TxBuilder;

/// The placeholder path a cell prints a field by: `project.prop.<key>` or `page.prop.<key>` (doc 09 §2.6).
pub fn user_prop_path(scope: &UserPropScope, key: &str) -> Option<String> {
    match scope {
        UserPropScope::Project => Some(format!("project.prop.{key}")),
        UserPropScope::Page => Some(format!("page.prop.{key}")),
        UserPropScope::Unknown(_) => None,
    }
}

/// The value list `values` with `key` set to `value` (an empty value removes it).
pub(crate) fn with_value(values: &[UserPropValue], key: &Ident, value: &MlText) -> Vec<UserPropValue> {
    let mut out: Vec<UserPropValue> = values.iter().filter(|v| v.key != *key).cloned().collect();
    if !value.is_empty() {
        out.push(UserPropValue {
            key: key.clone(),
            value: value.clone(),
            extra: Default::default(),
        });
    }
    out
}

/// `user_prop.define` (`DefineUserProp`): makes a field of the project's own (doc 09 §2.10): its key minted from the
/// label (`UserPropDef::mint_key`), its definition of the kind text appended to
/// `Project.user_prop_defs`, and its value now when given: the project's for a field of the whole project, the page's
/// (`page`) for a field of each page.
///
/// Rejected: an empty label (`/label`); a scope of a newer format (`/scope`); a value for a field of each page without
/// its page (`/page`), and a page that does not exist. One undo step. Outcome: none (the new definition is the
/// project's last).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DefineUserProp {
    /// The field's name.
    #[schemars(with = "MlTextS")]
    pub label: MlText,
    /// Whether it has one value for the whole project (`project`) or one for each page (`page`).
    pub scope: UserPropScope,
    /// Its value now; absent or empty: none yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<MlTextS>")]
    pub value: Option<MlText>,
    /// The page whose value `value` is, for a field of each page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub page: Option<Id<Page>>,
}

impl DefineUserProp {
    /// The definition this command appends to `tx`'s project, with its minted key, and the page value it writes; the
    /// refusals of [`DefineUserProp`]. Shared with `form.edit`'s field made from a cell.
    pub(crate) fn define(&self, tx: &mut TxBuilder<'_>, command: &str) -> Result<Ident, CmdError> {
        if ew_derive::text_in(&self.label, None).trim().is_empty() {
            return Err(CmdError::invalid_params(
                command,
                "/label",
                text!("refusal-user-prop-no-name"),
            ));
        }
        if matches!(self.scope, UserPropScope::Unknown(_)) {
            return Err(CmdError::invalid_params(
                command,
                "/scope",
                text!("refusal-user-prop-scope"),
            ));
        }
        let mut project = (*tx.project()).clone();
        let key = UserPropDef::mint_key(ew_derive::text_in(&self.label, None), |k| {
            project.user_prop_defs.iter().any(|d| d.key.as_str() == k)
        });
        let key: Ident = key
            .parse()
            .map_err(|_| CmdError::internal("user_prop.define: a minted key is an identifier"))?;
        project.user_prop_defs.push(UserPropDef {
            key: key.clone(),
            label: self.label.clone(),
            value_kind: UserPropKind::Text,
            scope: vec![self.scope.clone()],
            extra: Default::default(),
        });
        let value = self.value.clone().unwrap_or_default();
        match (&self.scope, self.page) {
            (UserPropScope::Project, _) => {
                project.user_props = with_value(&project.user_props, &key, &value);
            }
            (UserPropScope::Page, Some(page)) => {
                let values = tx.require::<Page>(page)?.user_props.clone();
                let values = with_value(&values, &key, &value);
                tx.update(page, |p: &mut Page| p.user_props = values)?;
            }
            (UserPropScope::Page, None) if !value.is_empty() => {
                return Err(CmdError::invalid_params(
                    command,
                    "/page",
                    text!("refusal-user-prop-value-page"),
                ));
            }
            _ => {}
        }
        tx.set_project(project)?;
        Ok(key)
    }
}

impl Command for DefineUserProp {
    const NAME: &'static str = "user_prop.define";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        self.define(tx, Self::NAME)?;
        Ok(Outcome::default())
    }
}

/// The definition of `key` in `tx`'s project, else refused at `/key`.
fn defined(tx: &TxBuilder<'_>, command: &str, key: &Ident) -> Result<usize, CmdError> {
    tx.project()
        .user_prop_defs
        .iter()
        .position(|d| d.key == *key)
        .ok_or_else(|| CmdError::invalid_params(command, "/key", text!("refusal-user-prop-unknown")))
}

/// `user_prop.rename` (`RenameUserProp`): gives a field of the project's own another name; its key stays, so every cell
/// that prints it goes on printing it (doc 09 §2.10). Rejected: a key the project does not define (`/key`); an empty
/// name (`/label`). One undo step. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameUserProp {
    /// The field's key.
    #[schemars(with = "IdentS")]
    pub key: Ident,
    /// Its new name.
    #[schemars(with = "MlTextS")]
    pub label: MlText,
}

impl Command for RenameUserProp {
    const NAME: &'static str = "user_prop.rename";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let at = defined(tx, Self::NAME, &self.key)?;
        if ew_derive::text_in(&self.label, None).trim().is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/label",
                text!("refusal-user-prop-no-name"),
            ));
        }
        let mut project = (*tx.project()).clone();
        project.user_prop_defs[at].label = self.label.clone();
        tx.set_project(project)?;
        Ok(Outcome::default())
    }
}

/// `user_prop.remove` (`RemoveUserProp`): deletes a field of the project's own, its definition and every value, the
/// project's and each page's (doc 09 §2.10).
///
/// Rejected: a key the project does not define (`/key`); a field a form the project holds prints (a cell whose value or
/// label names `{project.prop.<key>}` or `{page.prop.<key>}`), in words that name the pages that print the form ("The
/// title block of pages 101 to 105 and 201 shows Order; give that cell another value first"; a form no page prints is
/// named without pages). One undo step. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveUserProp {
    /// The field's key.
    #[schemars(with = "IdentS")]
    pub key: Ident,
}

impl Command for RemoveUserProp {
    const NAME: &'static str = "user_prop.remove";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let at = defined(tx, Self::NAME, &self.key)?;
        let mut project = (*tx.project()).clone();
        let def = project.user_prop_defs.remove(at);
        if let Some(text) = shown_by_a_form(tx, &def) {
            return tx.reject(text);
        }
        project.user_props.retain(|v| v.key != self.key);
        tx.set_project(project)?;
        for page in tx.iter::<Page>() {
            if page.user_props.iter().any(|v| v.key == self.key) {
                tx.update(page.id, |p: &mut Page| p.user_props.retain(|v| v.key != self.key))?;
            }
        }
        Ok(Outcome::default())
    }
}

/// The refusal of a field a held form prints: the first such form, named as a title block or a frame with the pages
/// that print it (page-order runs of two or more as "101 to 105"); `None` when no held form prints it.
fn shown_by_a_form(tx: &TxBuilder<'_>, def: &UserPropDef) -> Option<Text> {
    let paths: Vec<String> = def
        .scope
        .iter()
        .filter_map(|s| user_prop_path(s, def.key.as_str()))
        .collect();
    let shows = |form: &ew_schema::FormDef| {
        form.cells.iter().any(|c| {
            c.value
                .as_deref()
                .is_some_and(|v| ew_derive::placeholder_paths(v).iter().any(|p| paths.contains(p)))
        })
    };
    let q = ew_derive::Q::new(tx.base_model());
    let (form_id, form) = q.forms().iter().find(|(_, f)| shows(&f.item))?;
    let order = q.get::<ew_derive::PageOrder>(());
    let prints: Vec<bool> = order
        .entries
        .iter()
        .map(|e| {
            ew_derive::page_frame_form(&q, &e.page).is_some_and(|frame| {
                frame == form_id
                    || q.forms()
                        .get(frame)
                        .is_some_and(|f| f.item.title_block == Some(form_id))
            })
        })
        .collect();
    let mut runs: Vec<TextArg> = Vec::new();
    let mut i = 0;
    while i < prints.len() {
        if !prints[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < prints.len() && prints[i] {
            i += 1;
        }
        let (first, last) = (&order.entries[start].folio, &order.entries[i - 1].folio);
        runs.push(if i - start == 1 {
            first.clone().into()
        } else {
            text!("refusal-user-prop-pages-run", first.clone(), last.clone()).into()
        });
    }
    let name = TextArg::Ml(def.label.clone());
    let block = form.item.kind == ew_schema::FormKind::TitleBlock;
    Some(match (runs.is_empty(), block) {
        (true, true) => text!("refusal-user-prop-shown-block", name),
        (true, false) => text!("refusal-user-prop-shown-frame", name),
        (false, block) => {
            let pages = TextArg::List(List::new(
                runs,
                text!("refusal-join-comma"),
                text!("refusal-join-and"),
            ));
            if block {
                text!("refusal-user-prop-shown-block-pages", pages, name)
            } else {
                text!("refusal-user-prop-shown-frame-pages", pages, name)
            }
        }
    })
}
