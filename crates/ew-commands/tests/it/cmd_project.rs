// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The company-standard commands of FT-336: `project.company.set`, `project.frames.set`,
//! `project.languages.set` and `project.checks.set` (doc 16 §9.2 steps 1, 2, 8 and 9).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::{FORM, form, id, mini_golden_parts};
use common::*;
use ew_commands::commands::{COMPANY_MEMBER, SHIPPED_CHECK_SCHEMES, SetCheckSchemes, SetCompany};
use ew_commands::{
    AnyCommand, CmdError, CommandRegistry, Cost, DynCommand, Engine, Exposure, Origin, Submit,
};
use ew_ids::Id;
use ew_mltext::MlText;
use ew_model::Model;
use ew_schema::{FormDef, FormKind, LibDocument, PartyRole};
use serde_json::{Value, json};

fn any(name: &str, params: Value) -> AnyCommand {
    AnyCommand {
        name: name.to_owned(),
        params,
    }
}

fn commit(e: &mut Engine, cmd: &dyn DynCommand) {
    let p = e
        .preview(cmd)
        .unwrap_or_else(|err| panic!("{}: {err:?}", cmd.name()));
    e.commit(p).unwrap();
}

fn invalid(e: &mut Engine, cmd: &dyn DynCommand) -> (String, String) {
    match e.preview(cmd).unwrap_err() {
        CmdError::InvalidParams { path, message, .. } => (path, message),
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

fn rejected(e: &mut Engine, cmd: &dyn DynCommand) -> String {
    match e.preview(cmd).unwrap_err() {
        CmdError::Rejected { reason, .. } => reason,
        other => panic!("expected Rejected, got {other:?}"),
    }
}

fn settings_member(e: &Engine, member: &str) -> Option<Value> {
    e.model().project().settings.extra.get(member).cloned()
}

/// The registered metadata: three Bulk `Edit` commands and one interactive `HumanOnly` command, all undoable.
#[test]
fn the_company_standard_commands_are_registered_with_their_cost_and_exposure() {
    let registry = CommandRegistry::builtin();
    for (name, cost, exposure) in [
        ("project.company.set", Cost::Bulk, Exposure::Edit),
        ("project.frames.set", Cost::Bulk, Exposure::Edit),
        ("project.languages.set", Cost::Bulk, Exposure::Edit),
        ("project.checks.set", Cost::Interactive, Exposure::HumanOnly),
    ] {
        let meta = registry
            .get(name)
            .unwrap_or_else(|| panic!("{name} is registered"))
            .meta();
        assert_eq!((meta.cost, meta.exposure), (cost, exposure), "{name}");
        assert!(meta.undoable, "{name}");
    }
    let mut e = engine(golden_model());
    let err = e
        .execute(&any("project.company.set", json!({"name": "Example Panels"})))
        .unwrap_err();
    assert!(matches!(err, CmdError::NeedsPreview { .. }), "{err:?}");
}

// ---------------------------------------------------------------------------------------------------------
// project.company.set

#[test]
fn company_set_creates_the_panel_builder_party_and_the_notice_and_undoes() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let cmd = SetCompany {
        name: Some(MlText::neutral("Example Panels")),
        notice: Some(MlText::neutral("Property of Example Panels.")),
        personal_data: None,
        logo: None,
    };
    let p = e.preview(&cmd).unwrap();
    assert_eq!(p.diff().len(), 1, "only the project record changes");
    e.commit(p).unwrap();
    let project = e.model().project().clone();
    let builders: Vec<_> = project
        .parties
        .iter()
        .filter(|p| p.role == PartyRole::PanelBuilder)
        .collect();
    assert_eq!(builders.len(), 1);
    assert_eq!(builders[0].name.as_neutral(), Some("Example Panels"));
    assert!(
        builders[0].personal_data,
        "a new party is personal data unless told otherwise"
    );
    assert_eq!(
        project.extra.get(COMPANY_MEMBER),
        Some(&json!({"notice": "Property of Example Panels."}))
    );
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
    e.redo(&Origin::Gui).unwrap();
    assert_eq!(e.model().project().parties.len(), 1);
}

#[test]
fn company_set_edits_the_existing_party_and_keeps_the_logo() {
    let (mut project, records, _) = mini_golden_parts();
    project.parties.push(ew_schema::Party {
        role: PartyRole::PanelBuilder,
        name: Default::default(),
        address: Some(ew_schema::Address {
            city: Some("Example City".into()),
            ..Default::default()
        }),
        personal_data: true,
        extra: Default::default(),
    });
    let logo = format!("sha256:{}", "ab".repeat(32));
    project.extra.insert(
        COMPANY_MEMBER.into(),
        json!({"logo": logo, "notice": "Old notice"}),
    );
    let mut e = engine(Model::from_records(project, records, library()).unwrap());
    commit(
        &mut e,
        &SetCompany {
            name: Some(MlText::neutral("Example Panels")),
            notice: Some(Default::default()),
            personal_data: Some(false),
            logo: None,
        },
    );
    let project = e.model().project().clone();
    assert_eq!(project.parties.len(), 1, "the party is edited, not added");
    let party = &project.parties[0];
    assert_eq!(party.name.as_neutral(), Some("Example Panels"));
    assert!(!party.personal_data);
    assert!(party.address.is_some(), "the address stays");
    assert_eq!(
        project.extra.get(COMPANY_MEMBER),
        Some(&json!({"logo": logo})),
        "an empty notice removes the notice and keeps the logo"
    );
    // An empty name keeps the party.
    commit(
        &mut e,
        &SetCompany {
            name: Some(Default::default()),
            notice: None,
            personal_data: None,
            logo: None,
        },
    );
    assert_eq!(e.model().project().parties.len(), 1);
    assert!(e.model().project().parties[0].name.is_empty());
}

#[test]
fn company_set_refuses_what_it_cannot_do() {
    let mut e = engine(golden_model());
    let (path, _) = invalid(&mut e, &any("project.company.set", json!({})));
    assert_eq!(path, "");
    let reason = rejected(
        &mut e,
        &any("project.company.set", json!({"personal_data": false})),
    );
    assert!(reason.contains("no panel-builder party"), "{reason}");
    // An empty name without a party changes nothing (an empty transaction).
    let rev = e.model().rev();
    commit(&mut e, &any("project.company.set", json!({"name": ""})));
    assert_eq!(e.model().rev(), rev);
    // A company member of another shape is never overwritten.
    let (mut project, records, _) = mini_golden_parts();
    project.extra.insert(COMPANY_MEMBER.into(), json!("newer"));
    let mut e = engine(Model::from_records(project, records, library()).unwrap());
    let reason = rejected(&mut e, &any("project.company.set", json!({"notice": "x"})));
    assert!(reason.contains("not an object"), "{reason}");
    // The name alone does not touch that member.
    commit(&mut e, &any("project.company.set", json!({"name": "Example"})));
    assert_eq!(
        e.model().project().extra.get(COMPANY_MEMBER),
        Some(&json!("newer"))
    );
}

/// A placeholder logo: an SVG of 80 × 20 user units (no company's artwork).
const LOGO_SVG: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 20"><rect width="80" height="20"/></svg>"#;

/// A PNG signature and `IHDR` of 4 × 2 pixels (enough for the asset store's sniffer).
fn logo_png() -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
    b.extend_from_slice(&13u32.to_be_bytes());
    b.extend_from_slice(b"IHDR");
    b.extend_from_slice(&4u32.to_be_bytes());
    b.extend_from_slice(&2u32.to_be_bytes());
    b.extend_from_slice(&[8, 6, 0, 0, 0]);
    b
}

/// FT-337: the logo joins the asset store and the company member names it by its reference; undo restores the
/// reference and keeps the bytes; an empty text removes the logo.
#[test]
fn company_set_stores_the_logo_as_an_asset_and_undoes() {
    use sha2::Digest;
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let cmd = any(
        "project.company.set",
        json!({"logo": ew_schema::base64_encode(LOGO_SVG)}),
    );
    let p = e.preview(&cmd).unwrap();
    assert!(p.outcome().notices.is_empty(), "an SVG logo has no notice");
    e.commit(p).unwrap();
    let logo = ew_schema::company_logo(&e.model().project().extra).expect("a logo");
    assert_eq!(logo.format, ew_schema::AssetFormat::Svg);
    assert_eq!(logo.size, Some((80, 20)));
    assert_eq!(logo.sha256.0, <[u8; 32]>::from(sha2::Sha256::digest(LOGO_SVG)));
    assert_eq!(
        e.model().project().extra[COMPANY_MEMBER]["logo"],
        serde_json::to_value(logo).unwrap()
    );
    assert_eq!(&**e.model().asset(&logo.sha256).unwrap().bytes(), LOGO_SVG);
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
    assert!(e.model().asset(&logo.sha256).is_some(), "undo keeps the bytes");
    e.redo(&Origin::Gui).unwrap();
    assert_eq!(ew_schema::company_logo(&e.model().project().extra), Some(logo));
    // A raster logo says that it prints rasterised.
    let p = e
        .preview(&any(
            "project.company.set",
            json!({"logo": ew_schema::base64_encode(&logo_png())}),
        ))
        .unwrap();
    assert_eq!(p.outcome().notices.len(), 1);
    assert!(p.outcome().notices[0].contains("rasterised"));
    e.commit(p).unwrap();
    assert_eq!(
        ew_schema::company_logo(&e.model().project().extra).map(|r| (r.format, r.size)),
        Some((ew_schema::AssetFormat::Png, Some((4, 2))))
    );
    // Empty removes the logo, and the member with it when nothing else is left.
    commit(&mut e, &any("project.company.set", json!({"logo": ""})));
    assert_eq!(e.model().project().extra.get(COMPANY_MEMBER), None);
}

/// FT-337 fix round, item 3: picking again a logo whose bytes the model lost changes no row, but brings the asset
/// and leaves the project unsaved.
#[test]
fn a_logo_picked_again_without_row_changes_leaves_the_project_unsaved() {
    use sha2::Digest;
    let (mut project, records, _) = mini_golden_parts();
    let (format, size) = ew_schema::sniff(LOGO_SVG).unwrap();
    let reference = ew_schema::AssetRef {
        sha256: ew_schema::Sha256(sha2::Sha256::digest(LOGO_SVG).into()),
        format,
        size,
    };
    project.extra.insert(
        COMPANY_MEMBER.into(),
        json!({"logo": serde_json::to_value(reference).unwrap()}),
    );
    let mut e = engine(Model::from_records(project, records, library()).unwrap());
    assert!(e.model().asset(&reference.sha256).is_none() && !e.is_dirty());
    let rev = e.model().rev();
    commit(
        &mut e,
        &any(
            "project.company.set",
            json!({"logo": ew_schema::base64_encode(LOGO_SVG)}),
        ),
    );
    assert_eq!(e.model().rev(), rev, "no row changed");
    assert!(e.model().asset(&reference.sha256).is_some());
    assert!(e.is_dirty(), "the asset arrived: the project is unsaved");
    e.mark_saved();
    assert!(!e.is_dirty());
}

/// FT-337 fix round: a logo above 512 KiB is refused; one of exactly the limit passes.
#[test]
fn company_set_refuses_a_logo_above_the_limit() {
    let mut e = engine(golden_model());
    let mut bytes = logo_png();
    bytes.resize(ew_schema::MAX_LOGO_BYTES + 1, 0);
    let (path, message) = invalid(
        &mut e,
        &any(
            "project.company.set",
            json!({"logo": ew_schema::base64_encode(&bytes)}),
        ),
    );
    assert_eq!(path, "/logo");
    assert!(message.contains("512 KiB"), "{message}");
    bytes.truncate(ew_schema::MAX_LOGO_BYTES);
    commit(
        &mut e,
        &any(
            "project.company.set",
            json!({"logo": ew_schema::base64_encode(&bytes)}),
        ),
    );
    assert!(ew_schema::company_logo(&e.model().project().extra).is_some());
}

#[test]
fn company_set_refuses_a_logo_that_is_no_image() {
    let mut e = engine(golden_model());
    for logo in ["not base64!", "R0lGODlh", "PGh0bWw+PC9odG1sPg=="] {
        let (path, _) = invalid(&mut e, &any("project.company.set", json!({"logo": logo})));
        assert_eq!(path, "/logo", "{logo}");
    }
}

// ---------------------------------------------------------------------------------------------------------
// project.frames.set

/// A model whose library also holds a title-block form copy.
fn with_title_block() -> (Model, Id<FormDef>) {
    let tb: Id<FormDef> = id(0x4f);
    let mut copy = form(tb);
    copy.item.kind = FormKind::TitleBlock;
    let (p, r, _) = mini_golden_parts();
    let mut lib = library();
    lib.push(LibDocument::Form(copy));
    (Model::from_records(p, r, lib).unwrap(), tb)
}

#[test]
fn frames_set_stores_the_frame_defaults_in_page_type_order_and_undoes() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let form = FORM.to_string();
    commit(
        &mut e,
        &any(
            "project.frames.set",
            json!({"default": form, "per_page_type": [
                {"page_type": "title_page", "form": form},
                {"page_type": "section_divider", "form": form},
            ]}),
        ),
    );
    assert_eq!(
        settings_member(&e, "frames"),
        Some(json!({"default": form, "per_page_type": [
            {"form": form, "page_type": "section_divider"},
            {"form": form, "page_type": "title_page"},
        ]}))
    );
    // The derivation reads the member as written: no malformed-shim note.
    let mut q = ew_derive::Q::new(e.model());
    let frames = q.get::<ew_derive::FrameSettings>(());
    assert_eq!(frames.default, Some(FORM));
    assert_eq!(frames.per_page_type.len(), 2);
    let notes = q.diagnostics().notes();
    assert!(notes.is_empty(), "{notes:?}");
    // A default alone still writes the (empty) entry list, a required member of doc 03 §5.1.
    commit(&mut e, &any("project.frames.set", json!({"default": form})));
    assert_eq!(
        settings_member(&e, "frames"),
        Some(json!({"default": form, "per_page_type": []}))
    );
    // No default and no entry removes the member.
    commit(&mut e, &any("project.frames.set", json!({})));
    assert_eq!(settings_member(&e, "frames"), None);
    e.undo(&Origin::Gui).unwrap();
    e.undo(&Origin::Gui).unwrap();
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

/// Doc 05 §10.3: members of a newer format inside `frames` survive the command; a `frames` value that is not an
/// object is never overwritten.
#[test]
fn frames_set_keeps_unknown_members_and_refuses_a_foreign_value() {
    let form = FORM.to_string();
    let (mut project, records, _) = mini_golden_parts();
    project.settings.extra.insert(
        "frames".into(),
        json!({"default": form, "per_page_type": [], "ew.newer": {"x": 1}}),
    );
    let mut e = engine(Model::from_records(project, records, library()).unwrap());
    commit(
        &mut e,
        &any(
            "project.frames.set",
            json!({"per_page_type": [{"page_type": "fluid", "form": form}]}),
        ),
    );
    assert_eq!(
        settings_member(&e, "frames"),
        Some(json!({"ew.newer": {"x": 1}, "per_page_type": [{"form": form, "page_type": "fluid"}]})),
        "the default is the new value's (none), the newer member stays"
    );
    // With a newer member, an empty value keeps the member with an empty entry list.
    commit(&mut e, &any("project.frames.set", json!({})));
    assert_eq!(
        settings_member(&e, "frames"),
        Some(json!({"ew.newer": {"x": 1}, "per_page_type": []}))
    );
    let (mut project, records, _) = mini_golden_parts();
    project.settings.extra.insert("frames".into(), json!("newer"));
    let mut e = engine(Model::from_records(project, records, library()).unwrap());
    let reason = rejected(&mut e, &any("project.frames.set", json!({"default": form})));
    assert!(reason.contains("not an object"), "{reason}");
    assert_eq!(settings_member(&e, "frames"), Some(json!("newer")));
}

#[test]
fn frames_set_refuses_bad_forms_and_page_types() {
    let (model, tb) = with_title_block();
    let mut e = engine(model);
    let form = FORM.to_string();
    let err = e
        .preview(&any(
            "project.frames.set",
            json!({"default": id::<FormDef>(0x999).to_string()}),
        ))
        .unwrap_err();
    assert!(matches!(err, CmdError::NotFound { .. }), "{err:?}");
    let (path, message) = invalid(
        &mut e,
        &any("project.frames.set", json!({"default": tb.to_string()})),
    );
    assert_eq!(path, "/default");
    assert!(message.contains("not a frame"), "{message}");
    let (path, _) = invalid(
        &mut e,
        &any(
            "project.frames.set",
            json!({"per_page_type": [{"page_type": "report", "form": form}]}),
        ),
    );
    assert_eq!(path, "/per_page_type/0/page_type");
    let (path, message) = invalid(
        &mut e,
        &any(
            "project.frames.set",
            json!({"per_page_type": [
                {"page_type": "multi_line", "form": form},
                {"page_type": "multi_line", "form": form},
            ]}),
        ),
    );
    assert_eq!(path, "/per_page_type/1/page_type");
    assert!(message.contains("one entry per page type"), "{message}");
    let (path, _) = invalid(
        &mut e,
        &any(
            "project.frames.set",
            json!({"per_page_type": [{"page_type": "multi_line", "form": tb.to_string()}]}),
        ),
    );
    assert_eq!(path, "/per_page_type/0/form");
    // The parameter schema refuses a page type it does not know.
    let (path, _) = invalid(
        &mut e,
        &any(
            "project.frames.set",
            json!({"per_page_type": [{"page_type": "hologram", "form": form}]}),
        ),
    );
    assert!(path.starts_with("/per_page_type/0"), "{path}");
}

// ---------------------------------------------------------------------------------------------------------
// project.languages.set

#[test]
fn languages_set_writes_the_languages_and_the_formats_as_one_value() {
    let (mut project, records, _) = mini_golden_parts();
    project.settings.formatting.tag_case = Some(ew_schema::TagCase::Insensitive);
    let mut e = engine(Model::from_records(project, records, library()).unwrap());
    let before = lines(e.model());
    commit(
        &mut e,
        &any(
            "project.languages.set",
            json!({"source": "en", "translations": ["nl", "de"], "date": "iso8601", "decimal_separator": "comma"}),
        ),
    );
    let project = e.model().project().clone();
    let languages = project.languages.as_ref().expect("languages");
    assert_eq!(languages.source.as_str(), "en");
    let translations: Vec<&str> = languages.translations.iter().map(|t| t.as_str()).collect();
    assert_eq!(translations, ["de", "nl"], "a set, sorted");
    assert_eq!(
        project.settings.formatting.date,
        Some(ew_schema::DateFormat::Iso8601)
    );
    assert_eq!(
        project.settings.formatting.decimal_separator,
        Some(ew_schema::DecimalSeparator::Comma)
    );
    assert_eq!(
        project.settings.formatting.tag_case,
        Some(ew_schema::TagCase::Insensitive),
        "the tag case is not this command's"
    );
    // Absent members are removed: the value is the whole step.
    commit(
        &mut e,
        &any("project.languages.set", json!({"date": "dd_mm_yyyy"})),
    );
    let project = e.model().project().clone();
    assert_eq!(project.languages, None);
    assert_eq!(
        project.settings.formatting.date,
        Some(ew_schema::DateFormat::DdMmYyyy)
    );
    assert_eq!(project.settings.formatting.decimal_separator, None);
    e.undo(&Origin::Gui).unwrap();
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn languages_set_refuses_translations_that_do_not_fit() {
    let mut e = engine(golden_model());
    let (path, _) = invalid(
        &mut e,
        &any("project.languages.set", json!({"translations": ["de"]})),
    );
    assert_eq!(path, "/translations");
    let (path, message) = invalid(
        &mut e,
        &any(
            "project.languages.set",
            json!({"source": "en", "translations": ["en"]}),
        ),
    );
    assert_eq!(path, "/translations");
    assert!(message.contains("source language"), "{message}");
    let (path, _) = invalid(
        &mut e,
        &any(
            "project.languages.set",
            json!({"source": "en", "translations": ["de", "de"]}),
        ),
    );
    assert_eq!(path, "/translations");
    let (path, _) = invalid(&mut e, &any("project.languages.set", json!({"date": "mm/dd/yy"})));
    assert!(path.starts_with("/date") || path.is_empty(), "{path}");
}

// ---------------------------------------------------------------------------------------------------------
// project.checks.set

#[test]
fn checks_set_names_the_schemes_and_keeps_the_rest_of_the_member() {
    let (mut project, records, _) = mini_golden_parts();
    project
        .settings
        .extra
        .insert("checks".into(), json!({"strictness": "strict"}));
    let mut e = engine(Model::from_records(project, records, library()).unwrap());
    let before = lines(e.model());
    let cmd = SetCheckSchemes {
        active_scheme: Some("drafting".parse().unwrap()),
        release_scheme: Some("full".parse().unwrap()),
    };
    e.execute(&cmd).unwrap();
    assert_eq!(
        settings_member(&e, "checks"),
        Some(json!({"active_scheme": "drafting", "release_scheme": "full", "strictness": "strict"}))
    );
    e.execute(&any(
        "project.checks.set",
        json!({"release_scheme": "pre-delivery"}),
    ))
    .unwrap();
    assert_eq!(
        settings_member(&e, "checks"),
        Some(json!({"release_scheme": "pre-delivery", "strictness": "strict"}))
    );
    e.undo(&Origin::Gui).unwrap();
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
    // Without anything else in the member, removing both schemes removes the member.
    let mut e = engine(golden_model());
    e.execute(&any("project.checks.set", json!({"release_scheme": "full"})))
        .unwrap();
    e.execute(&any("project.checks.set", json!({}))).unwrap();
    assert_eq!(settings_member(&e, "checks"), None);
}

#[test]
fn checks_set_takes_shipped_schemes_only_and_only_from_a_human() {
    let mut e = engine(golden_model());
    let err = e
        .execute(&any("project.checks.set", json!({"release_scheme": "lenient"})))
        .unwrap_err();
    let CmdError::InvalidParams { path, message, .. } = err else {
        panic!("{err:?}");
    };
    assert_eq!(path, "/release_scheme");
    assert!(message.contains(&SHIPPED_CHECK_SCHEMES.join(", ")), "{message}");
    for origin in [
        Origin::CmdLog {
            file: "wizard.ewcmd".into(),
        },
        Origin::Mcp {
            client: "agent".into(),
        },
        Origin::Cli {
            subcommand: "apply".into(),
            confirmed: false,
        },
    ] {
        let err = e
            .execute_with(
                &any("project.checks.set", json!({"release_scheme": "full"})),
                Submit::from(origin.clone()),
            )
            .unwrap_err();
        assert!(matches!(err, CmdError::Exposure { .. }), "{origin:?}: {err:?}");
    }
    assert_eq!(settings_member(&e, "checks"), None);
}

/// FT-404 (ADR-0008 amendment 3's note): a `project.checks.set` step records `HumanOnly`, and undo and redo refuse it
/// from every origin but the GUI and a confirmed CLI subcommand, with the session unchanged.
#[test]
fn checks_set_is_undone_and_redone_only_by_a_human() {
    let mut e = engine(golden_model());
    e.execute(&any("project.checks.set", json!({"release_scheme": "full"})))
        .unwrap();
    let step = e.history().entries().last().unwrap();
    assert_eq!(step.exposure(), Exposure::HumanOnly);
    let refused = |e: &mut Engine, undo: bool, origin: Origin| {
        let (rev, cursor) = (e.model().rev(), e.history().cursor());
        let err = if undo { e.undo(&origin) } else { e.redo(&origin) }.unwrap_err();
        assert_eq!(
            err,
            CmdError::Exposure {
                command: "project.checks.set".into(),
                exposure: Exposure::HumanOnly,
                origin: origin.clone(),
            },
            "{origin:?}"
        );
        assert_eq!(
            (e.model().rev(), e.history().cursor()),
            (rev, cursor),
            "{origin:?}"
        );
    };
    let scripted = [
        Origin::CmdLog {
            file: "wizard.ewcmd".into(),
        },
        Origin::Script {
            name: "renumber".into(),
        },
        Origin::Mcp {
            client: "agent".into(),
        },
        Origin::Plugin { id: "acme".into() },
        Origin::Python {
            script: "fix.py".into(),
        },
        Origin::Cli {
            subcommand: "apply".into(),
            confirmed: false,
        },
    ];
    for origin in scripted.clone() {
        refused(&mut e, true, origin);
    }
    assert_eq!(
        settings_member(&e, "checks"),
        Some(json!({"release_scheme": "full"}))
    );
    // The GUI undoes it; scripted origins may not redo it; a confirmed CLI subcommand redoes it.
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(settings_member(&e, "checks"), None);
    for origin in scripted {
        refused(&mut e, false, origin);
    }
    let confirmed = Origin::Cli {
        subcommand: "project checks".into(),
        confirmed: true,
    };
    e.redo(&confirmed).unwrap();
    assert_eq!(
        settings_member(&e, "checks"),
        Some(json!({"release_scheme": "full"}))
    );
    e.undo(&confirmed).unwrap();
    e.redo(&Origin::Gui).unwrap();
    assert_eq!(
        settings_member(&e, "checks"),
        Some(json!({"release_scheme": "full"}))
    );
}

/// FT-404: an `Edit` step is undone and redone from a scripted origin as before.
#[test]
fn an_edit_step_is_undone_from_a_script() {
    let mut e = engine(golden_model());
    commit(
        &mut e,
        &any("project.company.set", json!({"name": "Example Panels"})),
    );
    assert_eq!(e.history().entries().last().unwrap().exposure(), Exposure::Edit);
    let script = Origin::Script { name: "batch".into() };
    e.undo(&script).unwrap();
    e.redo(&script).unwrap();
    e.undo(&Origin::CmdLog {
        file: "a.ewcmd".into(),
    })
    .unwrap();
}

/// FT-380: `project.assets.purge` is a Bulk `HumanOnly` command with a destructive confirmation and no undo; in the engine
/// (which owns no project directory) it commits a transparent marker line: no revision, no change of the model, and
/// an undo passes over it to the step below.
#[test]
fn assets_purge_is_a_transparent_non_undoable_marker() {
    let registry = CommandRegistry::builtin();
    let meta = registry.get("project.assets.purge").expect("registered").meta();
    assert_eq!((meta.cost, meta.exposure), (Cost::Bulk, Exposure::HumanOnly));
    assert!(!meta.undoable);
    assert_eq!(meta.confirmation, ew_commands::Confirmation::Destructive);

    let mut e = engine(golden_model());
    commit(
        &mut e,
        &any("project.company.set", json!({"name": "Example Panels"})),
    );
    let rev = e.model().rev();
    let listed = format!("sha256:{}", "ab".repeat(32));
    let p = e
        .preview(&any("project.assets.purge", json!({ "assets": [listed] })))
        .unwrap();
    assert_eq!(p.op_count(), 0);
    assert_eq!(
        p.outcome().notices,
        [format!("removes the asset file of {listed}")]
    );
    e.commit(p).unwrap();
    assert_eq!(e.model().rev(), rev, "a marker makes no revision");
    let top = e.history().entries().last().expect("the marker line");
    assert_eq!(top.label(), "project.assets.purge");
    assert!(!top.is_step() && !top.is_barrier(), "a transparent marker");
    // Undo passes over the marker and reverts the company name.
    e.undo(&Origin::Gui).unwrap();
    assert!(
        !e.model()
            .project()
            .parties
            .iter()
            .any(|p| p.role == PartyRole::PanelBuilder)
    );
}

#[test]
fn assets_purge_refuses_malformed_and_repeated_hashes() {
    let mut e = engine(golden_model());
    let (path, _) = invalid(
        &mut e,
        &any("project.assets.purge", json!({"assets": ["sha256:AB"]})),
    );
    assert_eq!(path, "/assets/0");
    let h = format!("sha256:{}", "cd".repeat(32));
    let (path, message) = invalid(&mut e, &any("project.assets.purge", json!({ "assets": [h, h] })));
    assert_eq!(path, "/assets/1");
    assert!(message.contains("twice"), "{message}");
    let (path, _) = invalid(&mut e, &any("project.assets.purge", json!({"all": true})));
    assert_eq!(path, "/all", "unknown members are refused");
}

/// FT-380: the history names every asset its entries could bring back, the redo branch included.
#[test]
fn the_history_names_the_assets_of_its_entries() {
    let mut e = engine(golden_model());
    assert!(e.history().asset_mentions().unwrap().is_empty());
    commit(
        &mut e,
        &any(
            "project.company.set",
            json!({"logo": ew_schema::base64_encode(LOGO_SVG)}),
        ),
    );
    let first = ew_schema::company_logo(&e.model().project().extra)
        .unwrap()
        .sha256;
    commit(
        &mut e,
        &any(
            "project.company.set",
            json!({"logo": ew_schema::base64_encode(&logo_png())}),
        ),
    );
    let second = ew_schema::company_logo(&e.model().project().extra)
        .unwrap()
        .sha256;
    let named = e.history().asset_mentions().unwrap();
    assert!(named.contains(&first) && named.contains(&second), "{named:?}");
    e.undo(&Origin::Gui).unwrap();
    let named = e.history().asset_mentions().unwrap();
    assert!(named.contains(&second), "the redo branch is named too");
}

/// FT-383: a hostile SVG logo (synthetic) never reaches the asset store: the command refuses it at `/logo` with the
/// rejecter's reason, and the model holds no new asset.
#[test]
fn company_set_refuses_a_hostile_svg_logo() {
    let mut e = engine(golden_model());
    let before = e.model().assets().count();
    for svg in [
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4" onload="x()"/>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><script>x()</script></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><image href="file:///etc/hostname"/></svg>"#,
    ] {
        let (path, message) = invalid(
            &mut e,
            &any(
                "project.company.set",
                json!({"logo": ew_schema::base64_encode(svg.as_bytes())}),
            ),
        );
        assert_eq!(path, "/logo");
        assert!(message.contains("the SVG file is refused"), "{message}");
    }
    assert_eq!(e.model().assets().count(), before);
}

// ---------------------------------------------------------------------------------------------------------
// FT-447: the project description through `fields.set`

/// `fields.set` with kind `project` and the project's ID sets `master.description`; undo restores it; a neutral
/// text and a language object both read back.
#[test]
fn fields_set_sets_the_project_description_and_undoes() {
    let mut e = engine(golden_model());
    let project = common::fixture::PROJECT.to_string();
    let before = e.model().project().master.description.clone();
    e.execute(&any(
        "fields.set",
        json!({"kind": "project", "targets": [project], "path": "master.description", "value": "Feed panel"}),
    ))
    .expect("the project description");
    assert_eq!(
        e.model().project().master.description,
        MlText::neutral("Feed panel")
    );
    e.execute(&any(
        "fields.set",
        json!({"kind": "project", "targets": [project], "path": "master.description",
            "value": {"en": "Feed panel", "nl": "Voedingskast"}}),
    ))
    .expect("a language object");
    assert_eq!(
        e.model().project().master.description.get(&"nl".parse().unwrap()),
        Some("Voedingskast")
    );
    e.undo(&Origin::Gui).unwrap();
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(e.model().project().master.description, before);
}

/// The refusals: another project member (at `/path`, naming the one settable path), a target that is not the
/// project (not found), a value that is not a text (at `/value`), and `edit.delete` of the project (at
/// `/targets/0`).
#[test]
fn fields_set_on_the_project_refuses_other_members_targets_and_values() {
    let mut e = engine(golden_model());
    let project = common::fixture::PROJECT.to_string();
    let (path, message) = invalid(
        &mut e,
        &any(
            "fields.set",
            json!({"kind": "project", "targets": [project], "path": "master.object_id", "value": "X"}),
        ),
    );
    assert_eq!(path, "/path");
    assert!(message.contains("settable: master.description"), "{message}");
    let other = id::<ew_schema::Project>(0x4fe).to_string();
    assert!(matches!(
        e.preview(&any(
            "fields.set",
            json!({"kind": "project", "targets": [other], "path": "master.description", "value": "X"}),
        )),
        Err(CmdError::NotFound { .. })
    ));
    let (path, _) = invalid(
        &mut e,
        &any(
            "fields.set",
            json!({"kind": "project", "targets": [project], "path": "master.description", "value": 7}),
        ),
    );
    assert_eq!(path, "/value");
    let (path, message) = invalid(
        &mut e,
        &any(
            "edit.delete",
            json!({"targets": [{"kind": "project", "id": project}]}),
        ),
    );
    assert_eq!(
        (path.as_str(), message.as_str()),
        ("/targets/0", "the project cannot be deleted")
    );
}
