// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid 1.0 commands (doc 11 §4.3, WP-S1.25): `fluid.medium.edit`, `fluid.medium.define`, `fluid.tube.set`,
//! `fluid.length.set`, the `fluid` member of `project.settings.set`, normaliser N-FL-5 (`tube_tags`), `valve.add`'s
//! refusal of a PLC part (FT-1576) and invariant EW-INV-025 (FI-06).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AddValve, DefineMedium, EditMedium, MediumParam, SetLength, SetProjectSettings, SetTube, TubeField,
};
use ew_commands::{CmdError, Engine};
use ew_ids::Id;
use ew_mltext::MlText;
use ew_model::Model;
use ew_schema::{
    Connection, ConnectionKind, Device, Discipline, Endpoint, FluidLine, FnDef, FnRole, Function, KeyedList,
    LengthSource, LibDocument, Medium, MediumDef, NumberSpec, Part, PartUse, Record, TubeMaterial, TubeSpec,
};
use ew_units::{Dec3, Nm};
use serde_json::json;

const PNEU: Id<FnDef> = id(0x7a00);
const DEV_A: Id<Device> = id(0x7a10);
const DEV_B: Id<Device> = id(0x7a11);
const FN_A: Id<Function> = id(0x7a20);
const FN_B: Id<Function> = id(0x7a21);
const LINE: Id<Connection> = id(0x7a30);
const LINE_2: Id<Connection> = id(0x7a31);
const TUBE_PART: Id<Part> = id(0x7a40);

/// A model with two pneumatic devices -?Q91 and -?Q92 (CPs "1", "2") joined by two fluid lines on page 102, a wire of
/// the mini golden project, and a tube part.
fn model() -> Model {
    model_with(Vec::new())
}

/// [`model`] with the library documents `extra`.
fn model_with(extra: Vec<LibDocument>) -> Model {
    let (p, mut records, _) = mini_golden_parts();
    let mut lib = library();
    lib.extend(extra);
    let mut def = fndef(PNEU, &["1", "2"]);
    let cps = def
        .item
        .cps
        .iter()
        .cloned()
        .map(|mut c| {
            c.discipline = Discipline::Pneumatic;
            c
        })
        .collect();
    def.item.cps = KeyedList::from_vec(cps).expect("distinct CP indexes");
    lib.push(LibDocument::Fndef(def));
    lib.push(LibDocument::Part(part(TUBE_PART, "PUN-6X1")));
    for (dev, f, n) in [(DEV_A, FN_A, 91), (DEV_B, FN_B, 92)] {
        records.push(Record::Device(device(dev, "Q", n)));
        records.push(Record::Function(function(f, dev, PNEU, FnRole::Main)));
    }
    for (c, i) in [(LINE, 0), (LINE_2, 1)] {
        let mut line = connection(c, Some(PAGE_102), Endpoint::Cp(FN_A, i), Endpoint::Cp(FN_B, i));
        line.kind = ConnectionKind::Fluid(Box::default());
        records.push(Record::Connection(line));
    }
    Model::from_records(p, records, lib).expect("the fluid model")
}

/// A bulk command run through its preview.
fn bulk(e: &mut Engine, cmd: &dyn ew_commands::DynCommand) -> Result<ew_commands::Receipt, CmdError> {
    let p = e.preview(cmd)?;
    e.commit(p)
}

fn refusal(r: Result<ew_commands::Receipt, CmdError>) -> String {
    match r {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn line(e: &Engine, c: Id<Connection>) -> FluidLine {
    match &e.model().table::<Connection>().get(&c).expect("the line").kind {
        ConnectionKind::Fluid(l) => (**l).clone(),
        other => panic!("no fluid line: {other:?}"),
    }
}

fn air() -> MediumParam {
    MediumParam {
        name: MlText::neutral("compressed air 6 bar"),
        code: None,
        discipline: Discipline::Pneumatic,
        p_nom: Dec3::from_int(6).expect("6"),
        quality: None,
        t_range: None,
        colour: None,
        default_tube: TubeSpec::default(),
        line_kind_tubes: Vec::new(),
    }
}

/// A medium is made, changed and deleted with its definition points; an empty name and an electrical discipline are
/// refused in words.
#[test]
fn a_medium_is_made_changed_and_deleted_with_its_definitions() {
    let mut e = engine(model());
    let made = e
        .execute(&EditMedium {
            medium: None,
            set: Some(air()),
        })
        .expect("a medium");
    let m: Id<Medium> = made.outcome.primary.expect("the medium").1.cast();
    let mut renamed = air();
    renamed.name = MlText::neutral("air 4 bar");
    e.execute(&EditMedium {
        medium: Some(m),
        set: Some(renamed),
    })
    .expect("renamed");
    assert_eq!(
        e.model().table::<Medium>().get(&m).map(|x| x.name.clone()),
        Some(MlText::neutral("air 4 bar"))
    );
    e.execute(&DefineMedium {
        function: FN_A,
        idx: 0,
        medium: Some(m),
    })
    .expect("defined");
    let gone = e
        .execute(&EditMedium {
            medium: Some(m),
            set: None,
        })
        .expect("deleted");
    assert_eq!(
        gone.outcome.notices,
        ["deleted medium air 4 bar with 1 definition points"]
    );
    assert!(
        e.model().table::<MediumDef>().is_empty(),
        "the definition went with the medium"
    );
    let mut nameless = air();
    nameless.name = MlText::default();
    assert_eq!(
        refusal(e.execute(&EditMedium {
            medium: None,
            set: Some(nameless)
        })),
        "a medium needs a name"
    );
    let mut electrical = air();
    electrical.discipline = Discipline::Electrical;
    assert_eq!(
        refusal(e.execute(&EditMedium {
            medium: None,
            set: Some(electrical)
        })),
        "a medium is pneumatic, vacuum or hydraulic"
    );
}

/// FI-04: a definition point is a fluid port, at most one per port (a second replaces the first); without a medium the
/// definition goes.
#[test]
fn a_port_holds_one_medium_definition() {
    let mut e = engine(model());
    let m: Id<Medium> = e
        .execute(&EditMedium {
            medium: None,
            set: Some(air()),
        })
        .unwrap()
        .outcome
        .primary
        .unwrap()
        .1
        .cast();
    let n: Id<Medium> = e
        .execute(&EditMedium {
            medium: None,
            set: Some(air()),
        })
        .unwrap()
        .outcome
        .primary
        .unwrap()
        .1
        .cast();
    e.execute(&DefineMedium {
        function: FN_A,
        idx: 0,
        medium: Some(m),
    })
    .unwrap();
    e.execute(&DefineMedium {
        function: FN_A,
        idx: 0,
        medium: Some(n),
    })
    .unwrap();
    let defs: Vec<Id<Medium>> = e
        .model()
        .table::<MediumDef>()
        .iter()
        .map(|(_, d)| d.medium)
        .collect();
    assert_eq!(defs, [n]);
    e.execute(&DefineMedium {
        function: FN_A,
        idx: 0,
        medium: None,
    })
    .unwrap();
    assert!(e.model().table::<MediumDef>().is_empty());
    assert_eq!(
        refusal(e.execute(&DefineMedium {
            function: FN_102G1,
            idx: 0,
            medium: Some(m)
        })),
        "-102G1:L is not a fluid port"
    );
}

/// `fluid.tube.set` merges the values it sets into each line's overrides and resets the ones it names; FI-05 and a
/// wire are refused in words.
#[test]
fn tube_overrides_are_set_reset_and_checked() {
    let mut e = engine(model());
    let set = |spec: TubeSpec, reset: Vec<TubeField>| SetTube {
        lines: vec![LINE, LINE_2],
        set: spec,
        fitting_a: None,
        fitting_b: None,
        reset,
    };
    e.execute(&set(
        TubeSpec {
            od: Some(Nm(8_000_000)),
            material: Some(TubeMaterial::Pu),
            ..TubeSpec::default()
        },
        vec![],
    ))
    .expect("set");
    e.execute(&set(
        TubeSpec {
            id: Some(Nm(6_000_000)),
            ..TubeSpec::default()
        },
        vec![TubeField::Material],
    ))
    .expect("set and reset");
    let t = line(&e, LINE_2).tube;
    assert_eq!(
        (t.od, t.id, t.material),
        (Some(Nm(8_000_000)), Some(Nm(6_000_000)), None)
    );
    assert_eq!(
        refusal(e.execute(&set(
            TubeSpec {
                id: Some(Nm(9_000_000)),
                ..TubeSpec::default()
            },
            vec![]
        ))),
        "the outer diameter 8 mm is not larger than the inner diameter 9 mm"
    );
    let wire = e
        .model()
        .table::<Connection>()
        .iter()
        .find(|(_, c)| c.kind == ConnectionKind::Wire)
        .map(|(id, _)| *id)
        .expect("a wire");
    let r = e.execute(&SetTube {
        lines: vec![wire],
        set: TubeSpec::default(),
        fitting_a: None,
        fitting_b: None,
        reset: vec![],
    });
    assert!(refusal(r).ends_with("is no fluid line"));
}

/// `fluid.length.set` sets a manual length or clears it; zero is refused in words.
#[test]
fn a_tube_length_is_manual_or_missing() {
    let mut e = engine(model());
    e.execute(&SetLength {
        lines: vec![LINE],
        length: Some(Nm(1_200_000_000)),
    })
    .unwrap();
    assert_eq!(line(&e, LINE).length, LengthSource::Manual(Nm(1_200_000_000)));
    e.execute(&SetLength {
        lines: vec![LINE],
        length: None,
    })
    .unwrap();
    assert_eq!(line(&e, LINE).length, LengthSource::Missing);
    assert_eq!(
        refusal(e.execute(&SetLength {
            lines: vec![LINE],
            length: Some(Nm(0))
        })),
        "a tube length is longer than zero, not 0 mm"
    );
}

/// The `fluid` settings take a default tube checked against FI-05; under the line-tag scheme `page_counter` a new
/// fluid line gets the lowest counter free on its page (N-FL-5), and without it a new line has none.
#[test]
fn the_line_tag_scheme_numbers_new_fluid_lines_per_page() {
    let mut e = engine(model());
    let settings = |v: serde_json::Value| SetProjectSettings {
        member: "fluid".into(),
        value: v,
    };
    assert_eq!(
        refusal(bulk(
            &mut e,
            &settings(json!({"default_tube": {"od": 4000000, "id": 6000000}}))
        )),
        "the outer diameter 4 mm is not larger than the inner diameter 6 mm"
    );
    bulk(
        &mut e,
        &settings(json!({"default_tube": {"od": 6000000, "id": 4000000, "material": "pu"}, "line_tags": "page_counter"})),
    )
    .expect("the fluid settings");
    assert_eq!(
        e.model().project().settings.fluid.default_tube.material,
        Some(TubeMaterial::Pu)
    );
    let draw = |a: u16| ew_commands::commands::Connect {
        a: ew_commands::commands::EndpointParam::Cp {
            function: FN_A,
            idx: a,
        },
        b: ew_commands::commands::EndpointParam::Cp {
            function: FN_B,
            idx: a,
        },
        page: Some(PAGE_102),
        route: vec![],
        kind: ConnectionKind::Fluid(Box::default()),
    };
    let new: Id<Connection> = e
        .execute(&draw(0))
        .expect("a line")
        .outcome
        .primary
        .unwrap()
        .1
        .cast();
    let counter = |e: &Engine, c| e.model().table::<Connection>().get(&c).map(|c| c.number.clone());
    assert_eq!(
        counter(&e, new),
        Some(NumberSpec::Assigned(ew_schema::AssignedNumber::new(1)))
    );
    bulk(&mut e, &settings(serde_json::Value::Null)).unwrap();
    let plain: Id<Connection> = e
        .execute(&draw(1))
        .expect("a line")
        .outcome
        .primary
        .unwrap()
        .1
        .cast();
    assert_eq!(counter(&e, plain), Some(NumberSpec::Unassigned));
}

/// FT-1576: `valve.add` refuses a part with a PLC section, which normaliser N0 would attach to a station.
#[test]
fn valve_add_refuses_a_plc_module_part() {
    let mut module_part = part(id(0x7a50), "MODULE-4");
    module_part.item.sections.plc =
        Some(serde_json::from_value(json!({"role": "module", "positions": {"count": 4}})).unwrap());
    let mut e = engine(model_with(vec![LibDocument::Part(module_part)]));
    let r = e.execute(&AddValve {
        module: DEV_A,
        position: None,
        part: PartUse {
            part: ew_schema::PartRef::new(id(0x7a50)),
            qty: Dec3::ONE,
            extra: Default::default(),
        },
        letter: None,
        fndef: None,
        structure: Default::default(),
    });
    assert_eq!(
        refusal(r),
        "part MODULE-4 is a PLC station or module, not a valve; add it as a module of the valve terminal"
    );
}

/// EW-INV-025 (FI-06): a `drives` link to an ancestor of its device is refused on the edit path, here by a parent set
/// under an existing link.
#[test]
fn a_drives_link_to_an_ancestor_fails_ew_inv_025() {
    let mut e = engine(model());
    e.execute(&ew_commands::commands::LinkDevices {
        from: DEV_A,
        to: DEV_B,
        kind: ew_schema::DeviceLinkKind::Drives,
    })
    .expect("the link");
    let r = e.execute(&ew_commands::commands::AttachChildren {
        parent: DEV_B,
        children: vec![serde_json::from_value(json!({"device": DEV_A.to_string()})).expect("a child")],
    });
    match r {
        Err(CmdError::Model(ew_model::ModelError::Invariant { id, .. }, _)) => assert_eq!(id, "EW-INV-025"),
        other => panic!("expected EW-INV-025, got {other:?}"),
    }
}
