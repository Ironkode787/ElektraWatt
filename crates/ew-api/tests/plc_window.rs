// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.21 part 1 in the Session (acceptance PW-1 to PW-6, PW-8): the generic CPU and the two generic cards of the
//! starter parts placed from the picker's command (`box.black` from the part, with the part's closure), normaliser N0
//! giving them their PLC records, and the channel members edited through `fields.set` with the property sheet of a
//! channel (selected through the API, `Session::property_sheet`).

use ew_api::output::{EditRoute, RowValue};
use ew_api::{AnyCommand, NewProjectKind, ProjectTemplate, Session, SessionOptions};
use ew_derive::{ChannelAddr, ChannelAddresses, PlcRows};
use ew_ids::{Id, Uuid};
use ew_schema::{Channel, Device, PlcConfigProject, StationRole};
use serde_json::{Value, json};

fn fresh(seed: u64) -> Session {
    let options = SessionOptions {
        ids: Some(ew_commands::testkit::seeded_ids(seed)),
        ..SessionOptions::default()
    };
    Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options).unwrap()
}

/// The starter black box symbol.
const BLACKBOX: &str = "ew.gen.blackbox@qet10";

/// The copy ID of the starter part `number` among the library parts.
fn library_part(s: &Session, number: &str) -> Uuid {
    s.library_parts()
        .iter()
        .find(|c| c.item.part_number.as_str() == number)
        .unwrap_or_else(|| panic!("{number} among the library parts"))
        .id
        .as_uuid()
}

/// The first page of the project.
fn first_page(s: &Session) -> Uuid {
    let q = s.query();
    let mut pages: Vec<Uuid> = q.pages().iter().map(|(id, _)| id.as_uuid()).collect();
    pages.sort();
    pages.first().copied().expect("the new project has a page")
}

/// The copy ID of the starter black box symbol (the ID it has once embedded).
fn symbol_id(s: &Session) -> Uuid {
    s.starter_symbol_copy(BLACKBOX).expect("a starter symbol")
}

/// Places the starter part `number` as the picker does: one `box.black` from the part with its closure. The new
/// device's ID.
fn place(s: &mut Session, number: &str, slot: i64) -> Uuid {
    let part = library_part(s, number);
    let copies = s
        .starter_embed_part(part, BLACKBOX)
        .expect("the closure resolves")
        .unwrap_or_else(|| json!({"copies": []}));
    let symbol = symbol_id(s);
    let page = first_page(s);
    let before: Vec<Uuid> = s.query().devices().iter().map(|(id, _)| id.as_uuid()).collect();
    // 7.9375 mm is three steps of the page's connection grid (7937500/3 nm)
    let step = 7_937_500i64;
    let r = s
        .execute(&AnyCommand {
            name: "box.black".into(),
            params: json!({
                "page": page.to_string(),
                "symbol": symbol.to_string(),
                "pos": [12 * step * slot, 12 * step],
                "box": {"outline": [[0, 0], [5 * step, 0], [5 * step, 8 * step], [0, 8 * step]]},
                "target": {"from_part": {"part": part.to_string(), "template": 0, "letter": "A"}},
                "copies": copies["copies"],
            }),
        })
        .unwrap_or_else(|e| panic!("box.black {number}: {e:?}"));
    let _ = r;
    let q = s.query();
    let new: Vec<Uuid> = q
        .devices()
        .iter()
        .map(|(id, _)| id.as_uuid())
        .filter(|id| !before.contains(id))
        .collect();
    let [id] = new.as_slice() else {
        panic!("one new device: {new:?}")
    };
    *id
}

fn device(s: &Session, id: Uuid) -> std::sync::Arc<Device> {
    s.query()
        .devices()
        .get(Id::from_uuid(id))
        .expect("the device")
        .clone()
}

/// The drawing texts of a module's channel addresses, in channel order (`-` where none is derived).
fn addresses(s: &Session, module: Uuid) -> Vec<String> {
    s.query()
        .get::<ChannelAddresses>(Id::from_uuid(module))
        .iter()
        .map(|e| match &e.address {
            ChannelAddr::At { text, .. } => text.clone(),
            ChannelAddr::Excluded => "excluded".into(),
            ChannelAddr::Tbd => "TBD".into(),
            ChannelAddr::Unresolved => "-".into(),
        })
        .collect()
}

fn texts(prefix: &str) -> Vec<String> {
    (0..8).map(|n| format!("{prefix}0.{n}")).collect()
}

/// The CPU, the input card and the output card placed in that order.
fn three(seed: u64) -> (Session, Uuid, Uuid, Uuid) {
    let mut s = fresh(seed);
    let cpu = place(&mut s, "EW-GEN-CPU-1", 1);
    let di = place(&mut s, "EW-GEN-DI-1", 2);
    let dout = place(&mut s, "EW-GEN-DO-1", 3);
    (s, cpu, di, dout)
}

/// PW-1: one station (role `cpu`), two modules at positions 1 and 2, sixteen channels with v5 IDs, each bound to
/// the card pin n + 1, and the addresses I0.0 … I0.7 and Q0.0 … Q0.7. PW-2: the first placement created `PLC1` with
/// the TIA profile and the CPU as its `cpu`.
#[test]
fn pw1_pw2_a_cpu_and_two_cards_from_the_picker_give_a_station_modules_channels_and_addresses() {
    let (s, cpu, di, dout) = three(11);
    let station = device(&s, cpu).aspects.plc_station.clone().expect("a station");
    assert_eq!(station.role, StationRole::Cpu);
    let q = s.query();
    let cfgs: Vec<_> = q
        .table::<PlcConfigProject>()
        .iter()
        .map(|(_, c)| c.clone())
        .collect();
    assert_eq!(cfgs.len(), 1);
    assert_eq!(cfgs[0].name.as_str(), "PLC1");
    assert_eq!(cfgs[0].profile.as_str(), "siemens-tia-intl@2027.1");
    assert_eq!(cfgs[0].cpu.map(|c| c.as_uuid()), Some(cpu));
    assert_eq!(station.config_project, cfgs[0].id);
    drop(q);
    for (m, pos) in [(di, 1), (dout, 2)] {
        let d = device(&s, m);
        assert_eq!(d.parent.map(|p| p.as_uuid()), Some(cpu));
        assert_eq!(d.position, Some(pos));
        assert!(d.aspects.plc_module.is_some());
    }
    let q = s.query();
    let main_of = |m: Uuid| {
        q.functions()
            .iter()
            .find(|(_, f)| f.device.map(|d| d.as_uuid()) == Some(m))
            .map(|(id, _)| id)
            .expect("the card's black box function")
    };
    let mut count = 0;
    for m in [di, dout] {
        let f = main_of(m);
        for (_, c) in q
            .table::<Channel>()
            .iter()
            .filter(|(_, c)| c.module.as_uuid() == m)
        {
            count += 1;
            assert_eq!(
                c.id,
                ew_ids::v5::plc_channel_id(Id::<Device>::from_uuid(m), c.number)
            );
            // the pin designated n + 1 is CP index n of the card's pattern
            assert_eq!(
                c.binding,
                Some(ew_schema::Endpoint::Cp(f, c.number)),
                "{}",
                c.number
            );
        }
    }
    assert_eq!(count, 16);
    drop(q);
    assert_eq!(addresses(&s, di), texts("I"));
    assert_eq!(addresses(&s, dout), texts("Q"));
}

/// PW-2: a second CPU joins `PLC1` with the notice of D3.
#[test]
fn pw2_a_second_cpu_joins_plc1() {
    let (mut s, ..) = three(12);
    let part = library_part(&s, "EW-GEN-CPU-1");
    let copies = s.starter_embed_part(part, BLACKBOX).unwrap();
    assert!(copies.is_none(), "the project holds the CPU's closure");
    let second = place(&mut s, "EW-GEN-CPU-1", 4);
    let q = s.query();
    assert_eq!(q.table::<PlcConfigProject>().iter().count(), 1);
    let cfg = q.table::<PlcConfigProject>().iter().next().map(|(id, _)| id);
    drop(q);
    assert_eq!(
        device(&s, second)
            .aspects
            .plc_station
            .as_ref()
            .map(|st| st.config_project),
        cfg
    );
}

/// PW-3: a card placed without a station has no parent, its channels exist and its addresses are not derived; the
/// notice reads "no PLC station: place a CPU first". With two stations a card stays unattached.
#[test]
fn pw3_a_card_without_one_station_stays_unattached() {
    let mut s = fresh(13);
    let part = library_part(&s, "EW-GEN-DI-1");
    let copies = s.starter_embed_part(part, BLACKBOX).unwrap().unwrap();
    let symbol = symbol_id(&s);
    let page = first_page(&s);
    let r = s
        .execute(&AnyCommand {
            name: "box.black".into(),
            params: json!({
                "page": page.to_string(), "symbol": symbol.to_string(), "pos": [7_937_500, 7_937_500],
                "box": {"outline": [[0, 0], [39_687_500, 0], [39_687_500, 63_500_000], [0, 63_500_000]]},
                "target": {"from_part": {"part": part.to_string(), "template": 0, "letter": "A"}},
                "copies": copies["copies"],
            }),
        })
        .expect("the card");
    assert!(
        r.outcome
            .notices
            .iter()
            .any(|n| n == "no PLC station: place a CPU first"),
        "{:?}",
        r.outcome.notices
    );
    let q = s.query();
    let (card, d) = q
        .devices()
        .iter()
        .find(|(_, d)| d.aspects.plc_module.is_some())
        .map(|(id, d)| (id.as_uuid(), d.clone()))
        .expect("the card");
    drop(q);
    assert_eq!(d.parent, None);
    assert_eq!(addresses(&s, card), vec!["-".to_owned(); 8], "no address");

    let mut s = fresh(14);
    place(&mut s, "EW-GEN-CPU-1", 1);
    place(&mut s, "EW-GEN-CPU-1", 2);
    let card = place(&mut s, "EW-GEN-DI-1", 3);
    assert_eq!(device(&s, card).parent, None);
}

/// PW-4: each placement is one undo step; undo of the first removes the station and `PLC1` and nothing else; redo
/// restores identical IDs.
#[test]
fn pw4_each_placement_is_one_undo_step() {
    let (mut s, cpu, di, dout) = three(15);
    let full = s.world().model;
    for _ in 0..2 {
        s.undo(&ew_api::Origin::Gui).expect("undo a card");
    }
    assert!(s.query().devices().get(Id::from_uuid(di)).is_none());
    assert!(s.query().devices().get(Id::from_uuid(dout)).is_none());
    let with_cpu = s.world().model;
    s.undo(&ew_api::Origin::Gui).expect("undo the CPU");
    assert!(s.query().devices().get(Id::from_uuid(cpu)).is_none());
    assert_eq!(s.query().table::<PlcConfigProject>().iter().count(), 0);
    assert!(
        s.world().model.content_eq(&fresh(15).world().model),
        "nothing else"
    );
    s.redo(&ew_api::Origin::Gui).expect("redo the CPU");
    assert!(s.world().model.content_eq(&with_cpu), "identical IDs");
    for _ in 0..2 {
        s.redo(&ew_api::Origin::Gui).expect("redo a card");
    }
    assert!(s.world().model.content_eq(&full));
}

fn set_channel(s: &mut Session, channel: Id<Channel>, path: &str, value: Value) -> Result<(), String> {
    s.execute(&AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "channel", "targets": [channel], "path": path, "value": value}),
    })
    .map(|_| ())
    .map_err(|e| format!("{e}"))
}

/// The PLC row of a channel of the station.
fn plc_row(s: &Session, station: Uuid, channel: Id<Channel>) -> ew_derive::PlcRow {
    s.query()
        .get::<PlcRows>(Id::from_uuid(station))
        .iter()
        .find(|r| r.channel == channel)
        .cloned()
        .expect("the channel's row")
}

/// PW-5: `fields.set` on a channel: symbol, comment, state, the address forms (a manual address refused for I4.9
/// with the profile's example), each one undo step, and the rows of `PlcRows` follow. The property sheet of the
/// selected channel shows the members with their `fields.set` routes.
#[test]
fn pw5_the_channel_members_through_fields_set_and_the_property_sheet() {
    let (mut s, cpu, di, _) = three(16);
    let c0: Id<Channel> = ew_ids::v5::plc_channel_id(Id::<Device>::from_uuid(di), 0);
    set_channel(&mut s, c0, "symbol", json!("B1_Start")).unwrap();
    assert_eq!(plc_row(&s, cpu, c0).text.symbol.as_deref(), Some("B1_Start"));
    set_channel(&mut s, c0, "comment", json!("Start")).unwrap();
    set_channel(&mut s, c0, "state", json!("spare")).unwrap();
    set_channel(&mut s, c0, "address", json!("I4.0")).unwrap();
    assert_eq!(addresses(&s, di)[0], "I4.0");
    let refused = set_channel(&mut s, c0, "address", json!("I4.9")).unwrap_err();
    assert!(refused.contains("expected an address like Q0.1"), "{refused}");
    set_channel(&mut s, c0, "address", json!("excluded")).unwrap();
    assert!(matches!(plc_row(&s, cpu, c0).address, ChannelAddr::Excluded));
    set_channel(&mut s, c0, "address", json!("tbd")).unwrap();
    assert!(matches!(plc_row(&s, cpu, c0).address, ChannelAddr::Tbd));
    set_channel(&mut s, c0, "address", json!("auto")).unwrap();
    assert_eq!(addresses(&s, di)[0], "I0.0");
    // each one undo step: undo the auto, then the tbd
    s.undo(&ew_api::Origin::Gui).unwrap();
    assert!(matches!(plc_row(&s, cpu, c0).address, ChannelAddr::Tbd));
    s.undo(&ew_api::Origin::Gui).unwrap();
    assert!(matches!(plc_row(&s, cpu, c0).address, ChannelAddr::Excluded));

    // the property sheet of the selected channel: its members route to `fields.set`
    let sheet = s.property_sheet(&[c0.as_uuid()]);
    assert_eq!(sheet.sections.len(), 1, "{:?}", sheet.sections);
    let section = &sheet.sections[0];
    assert_eq!(section.kind, "channel");
    for member in ["designation", "address", "state", "symbol", "comment", "target"] {
        let row = section
            .rows
            .iter()
            .find(|r| r.key == format!("channel/{member}"))
            .unwrap_or_else(|| panic!("the row {member}"));
        assert_eq!(
            row.route,
            EditRoute::FieldsSet {
                kind: "channel".into(),
                path: member.into()
            },
            "{member}"
        );
    }
    let symbol = section.rows.iter().find(|r| r.key == "channel/symbol").unwrap();
    assert!(
        matches!(&symbol.value, RowValue::One { text, .. } if text == "B1_Start"),
        "{:?}",
        symbol.value
    );
}

/// PW-6: renumbering the devices after PW-1 changes no address (FR-PLC-055).
#[test]
fn pw6_renumbering_changes_no_address() {
    let (mut s, _, di, dout) = three(17);
    let before = (addresses(&s, di), addresses(&s, dout));
    s.execute(&AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "device", "targets": [di.to_string()], "path": "tag.counter", "value": 9}),
    })
    .expect("a new counter");
    assert_eq!(device(&s, di).tag.counter, Some(9));
    assert_eq!((addresses(&s, di), addresses(&s, dout)), before);
    let cmd = AnyCommand {
        name: "numbering.renumber".into(),
        params: json!({"scope": "project"}),
    };
    let preview = s.preview(&cmd).expect("renumber preview");
    s.commit(preview).expect("renumber");
    assert_ne!(device(&s, di).tag.counter, Some(9), "the gap is closed");
    assert_eq!((addresses(&s, di), addresses(&s, dout)), before);
}

/// PW-8: two runs with one seed give the same model.
#[test]
fn pw8_twice_run_determinism() {
    let (a, ..) = three(18);
    let (b, ..) = three(18);
    assert!(a.world().model.content_eq(&b.world().model));
}

/// The binding of a channel.
fn binding(s: &Session, channel: Id<Channel>) -> Option<ew_schema::Endpoint> {
    s.query().table::<Channel>().get(channel).expect("the channel").binding.clone()
}

/// FX38a (found by lane K38): `plc.bind_channel` checks the CP against the CPs the function has as the pages draw
/// them. A black-box card's CPs come from its part's pattern (the generic black-box FnDef defines none), so binding
/// channel 2 of `EW-GEN-DO-1` to the pin of channel 5 moves the binding and unbinds channel 5 with the FR-PLC-005
/// notice; an index the card does not have is refused, naming the part pattern as where its CPs come from.
#[test]
fn bind_channel_takes_a_pin_of_a_black_box_card() {
    let (mut s, _, _, dout) = three(23);
    let ids = s.query().get::<ew_derive::ModuleChannelIds>(Id::from_uuid(dout));
    let ch = |n: u16| ids.iter().find(|(m, _)| *m == n).map(|(_, id)| *id).expect("the channel");
    let (ch2, ch5) = (ch(2), ch(5));
    let Some(ew_schema::Endpoint::Cp(function, idx)) = binding(&s, ch5) else {
        panic!("channel 5 is bound to its card pin")
    };
    let receipt = s
        .execute(&AnyCommand {
            name: "plc.bind_channel".into(),
            params: json!({"channel": ch2.to_string(),
                "endpoint": {"cp": {"function": function.to_string(), "idx": idx}}}),
        })
        .expect("a black-box pin binds");
    assert_eq!(binding(&s, ch2), Some(ew_schema::Endpoint::Cp(function, idx)));
    assert_eq!(binding(&s, ch5), None);
    let notice = format!("channel {ch5} was bound to the same CP and is unbound (FR-PLC-005)");
    assert!(
        receipt.outcome.notices.contains(&notice),
        "{:?}",
        receipt.outcome.notices
    );
    let err = s
        .execute(&AnyCommand {
            name: "plc.bind_channel".into(),
            params: json!({"channel": ch2.to_string(),
                "endpoint": {"cp": {"function": function.to_string(), "idx": 999}}}),
        })
        .expect_err("no CP 999 on the card");
    let err = format!("{err}");
    assert!(err.contains("has no CP 999"), "{err}");
    assert!(err.contains("part pattern"), "{err}");
}
