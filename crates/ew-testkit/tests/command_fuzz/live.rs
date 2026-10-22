// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The live records of the current snapshot that the generator picks its targets from (read through
//! `session.query()`, once per generated step). A generator that needs a record kind the snapshot no longer
//! has returns `None` for its arm (the no-op rule of doc 18 §3.3: a step whose fixture row is gone is not
//! generated).

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::Q;
use ew_schema::{Endpoint, Function, FunctionKind};
use serde_json::{Value, json};

pub fn id_json<T: serde::Serialize>(id: T) -> Value {
    serde_json::to_value(id).expect("an ID serialises")
}

/// The text of a JSON ID (the ID of a record is a string).
pub fn id_text(v: &Value) -> String {
    v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string())
}

pub struct PageInfo {
    pub id: Value,
    /// The page type in its JSON form (`multi_line`, `fluid`, …).
    pub kind: String,
    pub section: Option<Value>,
    pub number: u32,
    pub label: Option<String>,
    /// The page grid, `(num, den)` in nm: `t * num` is on the grid for every integer `t`.
    pub grid: (u64, u64),
}

pub struct SectionInfo {
    pub id: Value,
    pub number: u16,
    pub range: (u32, u32),
}

pub struct NodeInfo {
    pub id: Value,
    /// The aspect in its JSON form (`location`, `function`, …).
    pub aspect: String,
    pub parent: Option<Value>,
    pub code: String,
}

pub struct SymInfo {
    pub id: Value,
    /// The representation type in its JSON form.
    pub rep_type: String,
    pub cps: Vec<u16>,
    /// The embedded copy of the symbol's function definition, when the project has it.
    pub fndef: Option<Value>,
}

pub struct FnDefInfo {
    pub id: Value,
}

pub struct FuncInfo {
    pub id: Value,
    pub ip: bool,
}

pub struct StripInfo {
    pub id: Value,
    /// (terminal device, designation)
    pub terminals: Vec<(Value, String)>,
    pub seqs: BTreeSet<u32>,
}

pub struct CableInfo {
    pub id: Value,
    pub cores: BTreeSet<String>,
}

pub struct TreeDev {
    pub id: Value,
    pub key: String,
    pub parent: Option<String>,
    pub position: Option<u32>,
}

pub struct ChannelInfo {
    pub id: Value,
    /// Signal CPs the channel may bind: a CP of a function on the module or one of its descendants.
    pub targets: Vec<(Value, u16)>,
    /// A CP of a function that is not on the module's subtree.
    pub outside: Option<(Value, u16)>,
}

pub struct IpInfo {
    pub function: Value,
    pub page: Value,
    pub grid: (u64, u64),
    pub name: String,
}

pub struct RouteInfo {
    pub connection: Value,
    /// Where the CPs at its two ends are drawn on the connection's page.
    pub a: (i64, i64),
    pub b: (i64, i64),
    pub grid_num: i64,
}

pub struct AsmInfo {
    pub device: Value,
    /// The assembly device and its descendants.
    pub subtree: Vec<Value>,
    /// The devices its configs cover now.
    pub covers: BTreeSet<String>,
}

/// What the generator reads of the initial project, once per run: the settings the settings command can
/// restore, the form copies that `library.embed` re-embeds with a new lineage and the project ID.
pub struct Fixture {
    pub initial_settings: BTreeMap<String, Value>,
    /// (copy ID, the copy in its library-document JSON form with `$k`).
    pub form_json: Vec<(Value, Value)>,
    pub project_id: Value,
}

impl Fixture {
    pub fn of(q: &Q) -> Fixture {
        let initial_settings = ["numbering", "colour_profile", "size_profile", "xref_format"]
            .iter()
            .filter_map(|m| {
                q.project()
                    .settings
                    .extra
                    .get(*m)
                    .map(|v| ((*m).to_owned(), v.clone()))
            })
            .collect();
        let form_json = q
            .forms()
            .iter()
            .filter_map(|(id, copy)| {
                let mut v = serde_json::to_value(&**copy).ok()?;
                v["$k"] = json!("form");
                Some((id_json(id), v))
            })
            .collect();
        Fixture {
            initial_settings,
            form_json,
            project_id: id_json(q.project().id),
        }
    }
}

/// The snapshot's live records.
pub struct Live {
    pub fixture: std::rc::Rc<Fixture>,
    pub reps: Vec<Value>,
    pub devices: Vec<Value>,
    /// Devices that are not terminals or strips (a tag, a function text, a state make sense).
    pub plain_devices: Vec<Value>,
    pub connections: Vec<Value>,
    pub junctions: Vec<Value>,
    /// (section ID, first folio number) of the sections that hold pages.
    pub sections: Vec<(Value, u32)>,
    pub pages: Vec<Value>,
    /// The strips with at least two terminals: (terminal ID, designation) per strip.
    pub strips: Vec<Vec<(Value, Value)>>,
    /// The functions drawn on each page that has at least two: function IDs.
    pub page_functions: Vec<(Value, Vec<Value>)>,
    pub page_info: Vec<PageInfo>,
    pub section_info: Vec<SectionInfo>,
    pub nodes: Vec<NodeInfo>,
    pub ext_refs: Vec<Value>,
    pub symbols: Vec<SymInfo>,
    /// (symbol, function definition) pairs of the interruption points drawn now.
    pub ip_syms: Vec<(Value, Value)>,
    /// (symbol, function definition) pairs of the black boxes drawn now.
    pub bb_syms: Vec<(Value, Value)>,
    pub fndefs: Vec<FnDefInfo>,
    pub forms: Vec<Value>,
    pub parts: Vec<Value>,
    pub functions: Vec<FuncInfo>,
    pub strip_infos: Vec<StripInfo>,
    /// The function definition the terminals use.
    pub term_fndef: Option<Value>,
    pub cables: Vec<CableInfo>,
    pub tree: Vec<TreeDev>,
    /// (from, to, kind) of the device links.
    pub links: BTreeSet<(String, String, String)>,
    pub channels: Vec<ChannelInfo>,
    pub cfg_names: BTreeSet<String>,
    pub ip_infos: Vec<IpInfo>,
    pub routes: Vec<RouteInfo>,
    /// The members of `ProjectSettings.extra` the settings command edits, as stored now.
    pub settings: BTreeMap<String, Value>,
    pub has_supply: bool,
    pub assemblies: Vec<AsmInfo>,
    /// Station devices (a `plc_station` aspect) and module devices (a `plc_module` aspect).
    pub stations: Vec<Value>,
    pub modules: Vec<Value>,
    /// The `plc_config_project` IDs.
    pub cfgs: Vec<Value>,
    /// Every report block.
    pub blocks: Vec<Value>,
    /// Every conductor and potential definition.
    pub conductors: Vec<Value>,
    pub potential_defs: Vec<Value>,
    /// The location-aspect structure nodes.
    pub locations: Vec<Value>,
}

fn grid_of(g: ew_units::Grid) -> (u64, u64) {
    (g.num(), g.den())
}

fn kind_text<T: serde::Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

impl Live {
    pub fn read(q: &Q, fixture: &std::rc::Rc<Fixture>) -> Live {
        let reps: Vec<Value> = q.reps().ids().map(id_json).collect();
        let mut devices = Vec::new();
        let mut plain_devices = Vec::new();
        let mut strips: BTreeMap<String, Vec<(Value, Value)>> = BTreeMap::new();
        let mut strip_infos: Vec<StripInfo> = Vec::new();
        let mut cables = Vec::new();
        let mut tree = Vec::new();
        let (mut stations, mut modules) = (Vec::new(), Vec::new());
        let mut asm_devices = Vec::new();
        for (id, d) in q.devices().iter() {
            devices.push(id_json(id));
            tree.push(TreeDev {
                id: id_json(id),
                key: id.to_string(),
                parent: d.parent.map(|p| p.to_string()),
                position: d.position,
            });
            match &d.aspects.terminal {
                Some(t) => strips
                    .entry(id_json(t.strip).to_string())
                    .or_default()
                    .push((id_json(id), id_json(&t.designation))),
                None if d.aspects.strip.is_none() => plain_devices.push(id_json(id)),
                None => {}
            }
            if d.aspects.strip.is_some() {
                let mut terminals = Vec::new();
                let mut seqs = BTreeSet::new();
                for t in q.idx().terminals_of_strip(id) {
                    if let Some(td) = q.devices().get(t).and_then(|x| x.aspects.terminal.clone()) {
                        seqs.insert(td.seq);
                        terminals.push((id_json(t), td.designation.as_str().to_owned()));
                    }
                }
                strip_infos.push(StripInfo {
                    id: id_json(id),
                    terminals,
                    seqs,
                });
            }
            if d.aspects.cable.is_some() {
                let cores = q
                    .idx()
                    .conductors_of_cable(id)
                    .filter_map(|c| q.conductors().get(c).map(|c| c.core.as_str().to_owned()))
                    .collect();
                cables.push(CableInfo {
                    id: id_json(id),
                    cores,
                });
            }
            if d.aspects.plc_station.is_some() {
                stations.push(id_json(id));
            }
            if d.aspects.plc_module.is_some() {
                modules.push(id_json(id));
            }
            if d.aspects.assembly.is_some() {
                asm_devices.push(id);
            }
        }
        // Descendants per device.
        let mut children: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, t) in tree.iter().enumerate() {
            if let Some(p) = &t.parent {
                children.entry(p.clone()).or_default().push(i);
            }
        }
        let subtree_of = |root: usize| -> Vec<usize> {
            let mut out = vec![root];
            let mut at = 0;
            while at < out.len() && out.len() <= tree.len() {
                if let Some(cs) = children.get(&tree[out[at]].key) {
                    for c in cs {
                        if !out.contains(c) {
                            out.push(*c);
                        }
                    }
                }
                at += 1;
            }
            out
        };
        let index_of: BTreeMap<String, usize> =
            tree.iter().enumerate().map(|(i, t)| (t.key.clone(), i)).collect();
        let assemblies = asm_devices
            .iter()
            .filter_map(|id| {
                let i = *index_of.get(&id.to_string())?;
                let d = q.devices().get(*id)?;
                let asm = d.aspects.assembly.as_ref()?;
                Some(AsmInfo {
                    device: id_json(id),
                    subtree: subtree_of(i).into_iter().map(|j| tree[j].id.clone()).collect(),
                    covers: asm
                        .configs
                        .iter()
                        .flat_map(|c| c.covers.iter().map(ToString::to_string))
                        .collect(),
                })
            })
            .collect();

        let mut by_section: BTreeMap<String, u32> = BTreeMap::new();
        let mut pages = Vec::new();
        let mut page_info = Vec::new();
        for (id, p) in q.pages().iter() {
            pages.push(id_json(id));
            if let Some(s) = p.folio.section {
                let n = by_section.entry(id_json(s).to_string()).or_insert(p.folio.number);
                *n = (*n).min(p.folio.number);
            }
            page_info.push(PageInfo {
                id: id_json(id),
                kind: kind_text(&p.kind),
                section: p.folio.section.map(id_json),
                number: p.folio.number,
                label: p.folio.label.as_ref().map(|l| l.as_str().to_owned()),
                grid: grid_of(p.grid),
            });
        }
        let mut page_functions = Vec::new();
        for page in q.pages().ids() {
            let mut functions: Vec<Value> = Vec::new();
            for rep in q.idx().reps_by_page(page) {
                if let Some(r) = q.reps().get(rep) {
                    functions.extend(r.functions.iter().map(id_json));
                }
            }
            functions.sort_by_key(Value::to_string);
            functions.dedup();
            if functions.len() >= 2 {
                page_functions.push((id_json(page), functions));
            }
        }

        let section_info = q
            .sections()
            .iter()
            .map(|(id, s)| SectionInfo {
                id: id_json(id),
                number: s.number,
                range: s.folio_range,
            })
            .collect();
        let nodes: Vec<NodeInfo> = q
            .structure_nodes()
            .iter()
            .map(|(id, n)| NodeInfo {
                id: id_json(id),
                aspect: kind_text(&n.aspect),
                parent: n.parent.map(id_json),
                code: n.code.as_str().to_owned(),
            })
            .collect();
        let locations = nodes
            .iter()
            .filter(|n| n.aspect == "location")
            .map(|n| n.id.clone())
            .collect();

        let fndefs: Vec<FnDefInfo> = q
            .fndefs()
            .iter()
            .map(|(id, _)| FnDefInfo { id: id_json(id) })
            .collect();
        let symbols: Vec<SymInfo> = q
            .symbols()
            .iter()
            .map(|(id, s)| {
                let copy: ew_ids::Id<ew_schema::FnDef> =
                    ew_ids::v5::lib_copy_id(s.item.fndef.library.as_uuid(), s.item.fndef.item.as_uuid());
                SymInfo {
                    id: id_json(id),
                    rep_type: kind_text(&s.item.rep_type),
                    cps: s.item.cps.as_slice().iter().map(|c| c.idx).collect(),
                    fndef: q.fndefs().contains(copy).then(|| id_json(copy)),
                }
            })
            .collect();

        let mut functions = Vec::new();
        for (id, f) in q.functions().iter() {
            functions.push(FuncInfo {
                id: id_json(id),
                ip: matches!(f.kind, FunctionKind::Ip(_)),
            });
        }
        let mut ip_syms = Vec::new();
        let mut bb_syms = Vec::new();
        let mut ip_infos = Vec::new();
        let mut term_fndef = None;
        for (id, f) in q.functions().iter() {
            let first_rep = q.idx().reps_of_function(id).next().and_then(|r| q.reps().get(r));
            match (&f.kind, first_rep) {
                (FunctionKind::Ip(spec), Some(r)) => {
                    ip_syms.push((id_json(r.symbol), id_json(f.def)));
                    if spec.external.is_none() {
                        let grid = q.pages().get(r.page).map(|p| grid_of(p.grid)).unwrap_or((0, 1));
                        ip_infos.push(IpInfo {
                            function: id_json(id),
                            page: id_json(r.page),
                            grid,
                            name: spec.name.as_str().to_owned(),
                        });
                    }
                }
                (FunctionKind::BlackBox, Some(r)) => bb_syms.push((id_json(r.symbol), id_json(f.def))),
                _ => {}
            }
            if term_fndef.is_none()
                && let Some(d) = f.device.and_then(|d| q.devices().get(d))
                && d.aspects.terminal.is_some()
            {
                term_fndef = Some(id_json(f.def));
            }
        }
        ip_syms.sort_by_key(|(a, b)| (a.to_string(), b.to_string()));
        ip_syms.dedup();
        bb_syms.sort_by_key(|(a, b)| (a.to_string(), b.to_string()));
        bb_syms.dedup();

        let links = q
            .table::<ew_schema::DeviceLink>()
            .iter()
            .map(|(_, l)| (l.from.to_string(), l.to.to_string(), kind_text(&l.kind)))
            .collect();

        // The channels: the CPs they may bind lie on the module's subtree.
        let mut channels = Vec::new();
        let fn_all: Vec<(ew_ids::Id<Function>, std::sync::Arc<Function>)> =
            q.functions().iter().map(|(i, f)| (i, f.clone())).collect();
        for (id, ch) in q.table::<ew_schema::Channel>().iter() {
            let Some(&mi) = index_of.get(&ch.module.to_string()) else {
                continue;
            };
            let inside: BTreeSet<String> = subtree_of(mi).into_iter().map(|j| tree[j].key.clone()).collect();
            let (mut targets, mut outside) = (Vec::new(), None);
            for (fid, f) in &fn_all {
                let Some(def) = q.fndefs().get(f.def) else {
                    continue;
                };
                let Some(cp) = def.item.cps.as_slice().first() else {
                    continue;
                };
                let is_inside = f.device.is_some_and(|d| inside.contains(&d.to_string()));
                if is_inside {
                    for cp in def.item.cps.as_slice() {
                        targets.push((id_json(fid), cp.idx));
                    }
                } else if outside.is_none() {
                    outside = Some((id_json(fid), cp.idx));
                }
            }
            channels.push(ChannelInfo {
                id: id_json(id),
                targets,
                outside,
            });
        }

        // The connections between two CPs that are both drawn on the connection's page.
        let routes = q
            .connections()
            .iter()
            .filter_map(|(id, c)| {
                let page = c.page?;
                let grid = q.pages().get(page)?.grid;
                let (Endpoint::Cp(fa, ia), Endpoint::Cp(fb, ib)) = (&c.a, &c.b) else {
                    return None;
                };
                let a = ew_derive::cp_point(q, *fa, *ia, page)?;
                let b = ew_derive::cp_point(q, *fb, *ib, page)?;
                Some(RouteInfo {
                    connection: id_json(id),
                    a: (a.x.0, a.y.0),
                    b: (b.x.0, b.y.0),
                    grid_num: i64::try_from(grid.num()).unwrap_or(1),
                })
            })
            .collect();

        let settings = ["numbering", "colour_profile", "size_profile", "xref_format"]
            .iter()
            .filter_map(|m| {
                q.project()
                    .settings
                    .extra
                    .get(*m)
                    .map(|v| ((*m).to_owned(), v.clone()))
            })
            .collect::<BTreeMap<_, _>>();

        Live {
            fixture: fixture.clone(),
            reps,
            devices,
            plain_devices,
            connections: q.connections().ids().map(id_json).collect(),
            junctions: q.junctions().ids().map(id_json).collect(),
            sections: by_section
                .into_iter()
                .map(|(s, n)| (Value::String(s.trim_matches('"').to_owned()), n))
                .collect(),
            pages,
            strips: strips.into_values().filter(|t| t.len() >= 2).collect(),
            page_functions,
            page_info,
            section_info,
            nodes,
            ext_refs: q.external_refs().ids().map(id_json).collect(),
            symbols,
            ip_syms,
            bb_syms,
            fndefs,
            forms: q.forms().ids().map(id_json).collect(),
            parts: q.parts().ids().map(id_json).collect(),
            functions,
            strip_infos,
            term_fndef,
            cables,
            tree,
            links,
            channels,
            cfg_names: q
                .table::<ew_schema::PlcConfigProject>()
                .iter()
                .map(|(_, c)| c.name.as_str().to_owned())
                .collect(),
            ip_infos,
            routes,
            settings,
            has_supply: q.project().supply.is_some(),
            assemblies,
            stations,
            modules,
            cfgs: q
                .table::<ew_schema::PlcConfigProject>()
                .ids()
                .map(id_json)
                .collect(),
            blocks: q.report_blocks().ids().map(id_json).collect(),
            conductors: q.conductors().ids().map(id_json).collect(),
            potential_defs: q.potential_defs().ids().map(id_json).collect(),
            locations,
        }
    }

    /// The unit `json!` of a page position: `(t1 * num, t2 * num)` on the page grid.
    pub fn grid_pos(page: &PageInfo, t1: i64, t2: i64) -> Value {
        let n = i64::try_from(page.grid.0).unwrap_or(1);
        json!([t1 * n, t2 * n])
    }
}
