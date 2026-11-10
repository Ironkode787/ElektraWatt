// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arms for the PLC configuration and the bus nets.

use super::*;

pub(super) fn plc_config_project_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let profile = "siemens-tia-intl@2027.1";
    match rng.below(8) {
        // The command refuses: a name another configuration project has.
        0 => {
            let name = rng
                .pick(&live.cfg_names.iter().cloned().collect::<Vec<_>>())?
                .to_owned();
            Some(Step::cmd(
                "plc.config_project.create",
                json!({"name": name, "profile": profile}),
            ))
        }
        // The command refuses: not `id@version`.
        1 => Some(Step::cmd(
            "plc.config_project.create",
            json!({"name": format!("FZ{}", rng.below(10_000)), "profile": "no-version"}),
        )),
        // Refused: the CPU device does not exist.
        2 => Some(
            Step::cmd(
                "plc.config_project.create",
                json!({"name": format!("FZ{}", rng.below(10_000)), "profile": profile, "cpu": rng.uuid()}),
            )
            .refuse(),
        ),
        _ => {
            let mut name = format!("FZ{}", rng.below(10_000));
            while live.cfg_names.contains(&name) {
                name.push('x');
            }
            let mut p = json!({"name": name, "profile": profile});
            if rng.chance(2) {
                p["cpu"] = rng.pick(&live.devices)?.clone();
            }
            Some(Step::cmd("plc.config_project.create", p).counts(&[(Table::PlcConfigProjects, 1)]))
        }
    }
}

/// A name no net of `taken` has.
pub(super) fn fresh_net_name(taken: &std::collections::BTreeSet<String>, rng: &mut SplitMix64) -> String {
    let mut name = format!("NET{}", rng.below(10_000));
    while taken.contains(&name) {
        name.push('x');
    }
    name
}

pub(super) fn bus_logical_net_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    const NAME: &str = "bus.logical_net.create";
    match rng.below(8) {
        // The command refuses: a name another logical net has.
        0 => {
            let name = rng
                .pick(&live.lnet_names.iter().cloned().collect::<Vec<_>>())?
                .to_owned();
            Some(Step::cmd(NAME, json!({"bus": "profibus_dp", "name": name})))
        }
        // The command refuses: a rate of 0 bit/s.
        1 => Some(Step::cmd(
            NAME,
            json!({"bus": "profibus_dp", "name": fresh_net_name(&live.lnet_names, rng), "rate": 0}),
        )),
        // Refused: a bus system this build does not know.
        2 => Some(
            Step::cmd(
                NAME,
                json!({"bus": "sercos", "name": fresh_net_name(&live.lnet_names, rng)}),
            )
            .refuse(),
        ),
        _ => {
            let bus = *rng.pick(&["profibus_dp", "profinet", "ethercat", "io_link"])?;
            let mut p = json!({"bus": bus, "name": fresh_net_name(&live.lnet_names, rng)});
            if rng.chance(2) {
                p["rate"] = json!(1_500_000);
            }
            Some(Step::cmd(NAME, p).counts(&[(Table::LogicalNets, 1)]))
        }
    }
}

pub(super) fn bus_physical_net_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    const NAME: &str = "bus.physical_net.create";
    match rng.below(6) {
        // The command refuses: a name another physical net has.
        0 => {
            let name = rng
                .pick(&live.pnet_names.iter().cloned().collect::<Vec<_>>())?
                .to_owned();
            Some(Step::cmd(NAME, json!({"name": name, "medium": "copper"})))
        }
        // Refused: a medium this build does not know.
        1 => Some(
            Step::cmd(
                NAME,
                json!({"name": fresh_net_name(&live.pnet_names, rng), "medium": "radio"}),
            )
            .refuse(),
        ),
        _ => {
            let medium = *rng.pick(&["copper", "fibre"])?;
            Some(
                Step::cmd(
                    NAME,
                    json!({"name": fresh_net_name(&live.pnet_names, rng), "medium": medium}),
                )
                .counts(&[(Table::PhysicalNets, 1)]),
            )
        }
    }
}

pub(super) fn plc_bind_channel(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let ch = rng.pick(&live.channels)?;
    match rng.below(10) {
        // The command refuses: a CP of a function outside the module's subtree.
        0 => {
            let (f, idx) = ch.outside.as_ref()?;
            Some(Step::cmd(
                "plc.bind_channel",
                json!({"channel": ch.id, "endpoint": cp_end(f, *idx)}),
            ))
        }
        // The command refuses: the function has no such CP.
        1 => {
            let (f, _) = rng.pick(&ch.targets)?;
            Some(Step::cmd(
                "plc.bind_channel",
                json!({"channel": ch.id, "endpoint": cp_end(f, 99)}),
            ))
        }
        // The command refuses: a junction is not a signal CP.
        2 => {
            let j = rng.pick(&live.junctions)?;
            Some(Step::cmd(
                "plc.bind_channel",
                json!({"channel": ch.id, "endpoint": {"junction": {"junction": j}}}),
            ))
        }
        // Refused: the channel does not exist.
        3 => {
            let (f, idx) = rng.pick(&ch.targets)?;
            Some(
                Step::cmd(
                    "plc.bind_channel",
                    json!({"channel": rng.uuid(), "endpoint": cp_end(f, *idx)}),
                )
                .refuse(),
            )
        }
        // Unbinding is always valid.
        4 => {
            let channel = ch.id.clone();
            Some(
                Step::cmd("plc.bind_channel", json!({"channel": ch.id, "endpoint": null})).check(
                    move |_, after| {
                        let c =
                            typed(&channel).and_then(|i| after.table::<ew_schema::Channel>().get(i).cloned());
                        c.is_some_and(|c| c.binding.is_none())
                            .then_some(())
                            .ok_or_else(|| "the channel is still bound".to_owned())
                    },
                ),
            )
        }
        _ => {
            let (f, idx) = rng.pick(&ch.targets)?.clone();
            let (channel, function) = (ch.id.clone(), f.clone());
            Some(
                Step::cmd(
                    "plc.bind_channel",
                    json!({"channel": ch.id, "endpoint": cp_end(&f, idx)}),
                )
                .check(move |_, after| {
                    let want = ew_schema::Endpoint::Cp(typed(&function).ok_or("a function ID")?, idx);
                    let id = typed::<ew_schema::Channel>(&channel).ok_or("a channel ID")?;
                    let c = after
                        .table::<ew_schema::Channel>()
                        .get(id)
                        .cloned()
                        .ok_or("channel gone")?;
                    if c.binding.as_ref() != Some(&want) {
                        return Err(format!("the channel is bound to {:?}, not the CP", c.binding));
                    }
                    // A CP is in at most one channel (FR-PLC-005).
                    let n = after
                        .table::<ew_schema::Channel>()
                        .iter()
                        .filter(|(_, o)| o.binding.as_ref() == Some(&want))
                        .count();
                    (n == 1)
                        .then_some(())
                        .ok_or_else(|| format!("{n} channels are bound to the same CP"))
                }),
            )
        }
    }
}

/// `plc.set_profile` (WP-S1.21 D8): a shipped profile by its reference or its ID pins that reference; an unknown
/// profile and an unknown configuration project are refused.
pub(super) fn plc_set_profile(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(8) {
        // The command refuses: no such shipped profile.
        0 => {
            let cfg = rng.pick(&live.cfgs)?.clone();
            Some(Step::cmd(
                "plc.set_profile",
                json!({"config_project": cfg, "profile": "nope@1"}),
            ))
        }
        // Refused: the configuration project does not exist.
        1 => Some(
            Step::cmd(
                "plc.set_profile",
                json!({"config_project": rng.uuid(), "profile": "iec-61131-3"}),
            )
            .refuse(),
        ),
        _ => {
            let cfg = rng.pick(&live.cfgs)?.clone();
            let (text, reference) = *rng.pick(&[
                ("iec-61131-3", "iec-61131-3@2027.1"),
                ("IEC-61131-3@2027.1", "iec-61131-3@2027.1"),
                ("siemens-tia-intl@2027.1", "siemens-tia-intl@2027.1"),
            ])?;
            let id = cfg.clone();
            Some(
                Step::cmd("plc.set_profile", json!({"config_project": cfg, "profile": text}))
                    .counts(&[(Table::PlcConfigProjects, 0)])
                    .check(move |_, after| {
                        let id =
                            typed::<ew_schema::PlcConfigProject>(&id).ok_or("a configuration project ID")?;
                        let c = after
                            .table::<ew_schema::PlcConfigProject>()
                            .get(id)
                            .cloned()
                            .ok_or("the configuration project is gone")?;
                        if c.profile.as_str() == reference {
                            Ok(())
                        } else {
                            Err(format!("profile {} instead of {reference}", c.profile))
                        }
                    }),
            )
        }
    }
}

/// `plc.set_start` (WP-S1.21 D8): a keyword or a start of the direction's area sets the module's start; an unknown
/// module and a start in the other area are refused.
pub(super) fn plc_set_start(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let m = rng.pick(&live.modules)?.clone();
    match rng.below(8) {
        // Refused: the module does not exist.
        0 => Some(
            Step::cmd(
                "plc.set_start",
                json!({"modules": [rng.uuid()], "dir": "output", "spec": "auto"}),
            )
            .refuse(),
        ),
        // The command refuses: an input address as an output start.
        1 => Some(Step::cmd(
            "plc.set_start",
            json!({"modules": [m], "dir": "output", "spec": "I3"}),
        )),
        k => {
            let output = k % 2 == 0;
            let byte = rng.below(64);
            let spec = match rng.below(4) {
                0 => "auto".to_owned(),
                1 => "excluded".to_owned(),
                2 => "TBD".to_owned(),
                _ => format!("{}{byte}", if output { "Q" } else { "I" }),
            };
            let dir = if output { "output" } else { "input" };
            let module = m.clone();
            let want = spec.to_ascii_lowercase();
            Some(
                Step::cmd("plc.set_start", json!({"modules": [m], "dir": dir, "spec": spec}))
                    .counts(&[(Table::Devices, 0)])
                    .check(move |_, after| {
                        let id = typed::<ew_schema::Device>(&module).ok_or("a device ID")?;
                        let start = after
                            .devices()
                            .get(id)
                            .and_then(|d| d.aspects.plc_module.clone())
                            .ok_or("the module lost its aspect")?
                            .start;
                        let got = if output { start.output } else { start.input };
                        let ok = match (want.as_str(), &got) {
                            ("auto", ew_schema::AddrSpec::Auto)
                            | ("excluded", ew_schema::AddrSpec::Excluded)
                            | ("tbd", ew_schema::AddrSpec::Tbd) => true,
                            (_, ew_schema::AddrSpec::Manual(a)) => usize::try_from(a.byte).ok() == Some(byte),
                            _ => false,
                        };
                        if ok {
                            Ok(())
                        } else {
                            Err(format!("start {got:?} for spec {want}"))
                        }
                    }),
            )
        }
    }
}

/// `plc.set_module_part` (Bulk: the runner previews and commits it). A well-formed step sets a module's
/// part to any part copy (or keeps it) and leaves no channel of the module at or above the new part's
/// channel count: a part without channels removes all eight channels of a golden module with their bindings.
pub(super) fn plc_set_module_part(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let m = rng.pick(&live.modules)?.clone();
    match rng.below(8) {
        // Refused: the module does not exist.
        0 => Some(Step::cmd("plc.set_module_part", json!({"module": rng.uuid()})).refuse()),
        // Refused: the part copy does not exist.
        1 => Some(
            Step::cmd(
                "plc.set_module_part",
                json!({"module": m, "parts": [{"part": {"copy": rng.uuid()}, "qty": "1"}]}),
            )
            .refuse(),
        ),
        k => {
            let mut p = json!({"module": m});
            if k > 2 {
                p["parts"] = json!([part_use(&live.parts, rng)?]);
            }
            let module = m.clone();
            Some(
                Step::cmd("plc.set_module_part", p)
                    .counts(&[(Table::Devices, 0)])
                    .check(move |before, after| {
                        let id = typed::<ew_schema::Device>(&module).ok_or("a device ID")?;
                        let has_aspect = after
                            .devices()
                            .get(id)
                            .is_some_and(|d| d.aspects.plc_module.is_some());
                        let count = if has_aspect {
                            ew_derive::module_part(after, id)
                                .and_then(|(_, s)| s.channels.map(|c| c.count))
                                .unwrap_or(0)
                        } else {
                            0
                        };
                        let surplus = after
                            .table::<ew_schema::Channel>()
                            .iter()
                            .filter(|(_, c)| c.module == id && c.number >= count)
                            .count();
                        if surplus != 0 {
                            return Err(format!("{surplus} surplus channels remain"));
                        }
                        // no over-deletion: every channel below the count survives with its binding
                        for (cid, c) in before.table::<ew_schema::Channel>().iter() {
                            if c.module != id || c.number >= count {
                                continue;
                            }
                            match after.table::<ew_schema::Channel>().get(cid) {
                                Some(a) if a.binding == c.binding => {}
                                Some(_) => return Err(format!("channel {} lost its binding", c.number)),
                                None => return Err(format!("channel {} below the count is gone", c.number)),
                            }
                        }
                        Ok(())
                    }),
            )
        }
    }
}
