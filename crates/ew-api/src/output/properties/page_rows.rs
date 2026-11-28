// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The derived rows of a page (its description, inherited or overridden) and of a representation (its
//! cross-reference text).
//!
//! Not here: a device's rows (`device_rows.rs`); a connection's rows (`connection_rows.rs`).

use ew_derive::{PageRef, Q, TbValue, TitleBlockValues, XrefTextOf};
use ew_ids::{Id, Uuid};
use ew_schema::Kind;
use serde_json::Value;

use super::DerivedRow;
use super::context::SheetCx;
use super::sections::{Built, derived, merge};
use super::{Provenance, RowAction, RowValue, SourceLink, device_tag_text, ml_text};
use crate::Session;
use crate::output::fields::{Category, FieldType};

impl Session {
    /// A page's derived row: the description, inherited from the project's or overridden.
    pub(super) fn derived_rows_of_page(
        q: &Q,
        cx: &SheetCx<'_>,
        targets: &[Uuid],
        jsons: &[Value],
    ) -> Vec<Built> {
        let source = cx.source.as_deref();
        let editable = |path: &str| Self::editable(Kind::Page, path);
        let mut out = Vec::new();
        let project_description = ml_text(
            &serde_json::to_value(&q.project().master.description).unwrap_or(Value::Null),
            source,
        );
        let mut per = Vec::new();
        let mut any_override = false;
        for (t, j) in targets.iter().zip(jsons) {
            let tb = q.get::<TitleBlockValues>(PageRef::Drawn(Id::from_uuid(*t)));
            let text = match tb.get("page.description") {
                Some(TbValue::Ml(m)) => ml_text(&serde_json::to_value(m).unwrap_or(Value::Null), source),
                Some(TbValue::Text(s)) => s.clone(),
                _ => String::new(),
            };
            let overridden = j
                .get("description_override")
                .is_some_and(|o| !o.is_null() && !ml_text(o, source).is_empty());
            let provenance = if overridden {
                any_override = true;
                Provenance::Overridden {
                    note: text!(
                        "via-joined",
                        text!("via-override"),
                        text!("via-project-description-is", project_description.clone())
                    ),
                }
            } else {
                Provenance::Inherited {
                    from: text!("via-project-description"),
                }
            };
            per.push((Some(Value::String(text)), provenance));
        }
        let (value, provenance) = merge(cx, &FieldType::MlText, per);
        let mut actions = vec![RowAction::Override];
        if any_override {
            actions.push(RowAction::Reset);
        }
        let inherited = matches!(provenance, Provenance::Inherited { .. });
        let mut built = derived(DerivedRow {
            kind: "page",
            name: "description",
            partner: Some(("description_override", editable("description_override"), false)),
            category: Category::Texts,
            ty: FieldType::MlText,
            value,
            provenance,
            actions,
        });
        if inherited {
            built.row.source = Some(SourceLink::Project {
                row: "project/master.description".into(),
            });
        }
        out.push(built);
        out
    }

    /// A representation's derived row: the cross-reference text.
    pub(super) fn derived_rows_of_representation(q: &Q, cx: &SheetCx<'_>, targets: &[Uuid]) -> Vec<Built> {
        let mut out = Vec::new();
        // One bus symbol selected: with what it is drawn, and the way to another look.
        if let [one] = targets
            && let Some(rep) = q.reps().get(Id::from_uuid(*one))
            && q.symbols().get(rep.symbol).is_some_and(|s| ew_derive::is_bus_copy(&s.item))
            && let Some(device) = rep.functions.first().and_then(|f| q.functions().get(*f)?.device)
        {
            out.extend(super::device_rows::bus_symbol_row(q, device));
        }
        let per: Vec<(Option<Value>, Provenance)> = targets
            .iter()
            .map(|t| {
                let x = q.get::<XrefTextOf>(Id::from_uuid(*t));
                (
                    (!x.text.is_empty()).then(|| Value::String(x.text.clone())),
                    Provenance::Derived {
                        via: text!("via-cross-reference-format"),
                    },
                )
            })
            .collect();
        let (value, provenance) = merge(cx, &FieldType::Text, per);
        // A cross-reference is an electrical device's (a contact's to its coil); an empty one of a symbol
        // with no electrical point is not listed.
        let electrical = targets.iter().any(|t| {
            q.reps().get(Id::from_uuid(*t)).is_some_and(|r| {
                r.functions.iter().any(|f| {
                    q.get::<ew_derive::FunctionCps>(*f)
                        .iter()
                        .any(|c| c.discipline == ew_schema::Discipline::Electrical)
                })
            })
        });
        if value != RowValue::Absent || electrical {
            out.push(derived(DerivedRow {
                kind: "representation",
                name: "xref",
                partner: None,
                category: Category::Texts,
                ty: FieldType::Text,
                value,
                provenance,
                actions: Vec::new(),
            }));
        }
        // The coil symbol of a valve's solenoid says which valve it belongs to, a link to the valve.
        if let [one] = targets
            && let Some(rep) = q.reps().get(Id::from_uuid(*one))
            && let [f] = rep.functions.as_slice()
            && let Some(device) = q.functions().get(*f).and_then(|x| x.device)
            && let Some(s) = crate::output::rack::solenoids(q, device)
                .into_iter()
                .find(|s| s.function == f.as_uuid())
        {
            let tag = device_tag_text(q, device);
            let value = RowValue::One {
                json: serde_json::json!({"device": device.as_uuid().to_string(), "name": s.name, "tag": tag}),
                text: format!("Solenoid {} of {tag}", s.name),
            };
            out.push(derived(DerivedRow {
                kind: "representation",
                name: "solenoid_of",
                partner: None,
                category: Category::Structure,
                ty: FieldType::Link { kind: "valve".into() },
                value,
                provenance: Provenance::Derived {
                    via: text!("via-valve-part"),
                },
                actions: Vec::new(),
            }));
        }
        out
    }
}
