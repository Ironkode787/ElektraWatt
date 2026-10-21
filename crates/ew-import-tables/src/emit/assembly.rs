// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The assembly table (doc 20 §14.2, doc 03 §7; D-36, D-41): `assembly_configs.csv` → the `assembly` aspect of
//! the device named by `assembly`, one `AssemblyConfig` per row, in `n` order.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_schema::{Assembly, AssemblyConfig, Device, PartRef, Record};

use super::{Cx, fresh};
use crate::table::Table;

/// The pre-order interval of every emitted device in the parent forest: `x` is `a` or a descendant of `a`
/// exactly when `x`'s `enter` lies in `[a.enter, a.exit)`. One iterative walk, O(N) (doc 14 §11: no recursion,
/// no per-row chain walks on hostile input).
struct Forest {
    span: BTreeMap<Id<Device>, (usize, usize)>,
}

impl Forest {
    fn new(devices: &[(Id<Device>, Option<Id<Device>>)]) -> Forest {
        let mut children: BTreeMap<Id<Device>, Vec<Id<Device>>> = BTreeMap::new();
        let mut roots = Vec::new();
        let known: BTreeSet<Id<Device>> = devices.iter().map(|(id, _)| *id).collect();
        for (id, parent) in devices {
            match parent.filter(|p| known.contains(p)) {
                Some(p) => children.entry(p).or_default().push(*id),
                None => roots.push(*id),
            }
        }
        let mut span = BTreeMap::new();
        let mut clock = 0usize;
        for root in roots {
            // (device, index of its next child)
            let mut stack = vec![(root, 0usize)];
            span.insert(root, (clock, clock));
            clock += 1;
            while let Some((id, next)) = stack.last().copied() {
                let kids = children.get(&id).map_or(&[][..], Vec::as_slice);
                if let Some(&kid) = kids.get(next) {
                    if let Some(top) = stack.last_mut() {
                        top.1 += 1;
                    }
                    span.insert(kid, (clock, clock));
                    clock += 1;
                    stack.push((kid, 0));
                } else {
                    if let Some(s) = span.get_mut(&id) {
                        s.1 = clock;
                    }
                    stack.pop();
                }
            }
        }
        Forest { span }
    }

    /// Whether `x` is `a` or a descendant of `a`.
    fn within(&self, a: Id<Device>, x: Id<Device>) -> bool {
        match (self.span.get(&a), self.span.get(&x)) {
            (Some(a), Some(x)) => a.0 <= x.0 && x.0 < a.1,
            _ => false,
        }
    }
}

/// `assembly_configs.csv` → the `assembly` aspects of the devices. Run after the devices and the parts.
///
/// Errors never repair: an unknown assembly, part or covered device is EW-IMP-003; a repeated `n` of one
/// assembly, a repeated covered device, or a covered device that is neither the assembly nor one of its
/// descendants (`parent` of `devices.csv`) is EW-IMP-002. The row is dropped in each case.
pub(crate) fn emit_assembly_configs(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    // The emitted devices: their record position and parent.
    let mut at: BTreeMap<Id<Device>, usize> = BTreeMap::new();
    let mut devices = Vec::new();
    for (i, r) in cx.records.iter().enumerate() {
        if let Record::Device(d) = r {
            at.insert(d.id, i);
            devices.push((d.id, d.parent));
        }
    }
    let forest = Forest::new(&devices);
    let mut seen = BTreeMap::<String, ()>::new();
    let mut configs: BTreeMap<Id<Device>, BTreeMap<u32, AssemblyConfig>> = BTreeMap::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        seen.insert(row.key().to_owned(), ());
        let mut r = row.reader();
        let assembly = r.req("assembly");
        let n = r.req_uint::<u32>("n");
        let order_code = r.req_ident("order_code");
        let label = r.mltext("label");
        let part = r.text("part");
        let covers = r.list("covers");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(assembly), Some(n), Some(order_code)) = (assembly, n, order_code) else {
            continue;
        };
        let Some(assembly_id) = cx
            .devices
            .get(assembly)
            .map(|d| d.id)
            .filter(|id| at.contains_key(id))
        else {
            cx.unresolved(row, &format!("unknown device `{assembly}` in column `assembly`"));
            continue;
        };
        let part = match part {
            None => None,
            Some(key) => match cx.parts.get(key) {
                Some(p) => Some(PartRef::new(p.id)),
                None => {
                    cx.unresolved(row, &format!("unknown part `{key}` in column `part`"));
                    continue;
                }
            },
        };
        let mut covered = Vec::with_capacity(covers.len());
        let mut unknown = None;
        for key in &covers {
            match cx.devices.get(*key) {
                Some(d) => covered.push((*key, d.id)),
                None => {
                    unknown = Some(*key);
                    break;
                }
            }
        }
        if let Some(key) = unknown {
            cx.unresolved(row, &format!("unknown device `{key}` in column `covers`"));
            continue;
        }
        if configs.get(&assembly_id).is_some_and(|c| c.contains_key(&n)) {
            cx.malformed(
                row,
                &format!("`{assembly}` already has an order code with n = {n}"),
            );
            continue;
        }
        let mut distinct = BTreeSet::new();
        let mut problem = None;
        for (key, id) in &covered {
            if !distinct.insert(*id) {
                problem = Some(format!("column `covers`: `{key}` is listed twice"));
                break;
            }
            if !forest.within(assembly_id, *id) {
                problem = Some(format!(
                    "column `covers`: `{key}` is neither `{assembly}` nor one of its descendants"
                ));
                break;
            }
        }
        if let Some(message) = problem {
            cx.malformed(row, &message);
            continue;
        }
        configs.entry(assembly_id).or_default().insert(
            n,
            AssemblyConfig {
                label,
                order_code,
                part,
                covers: distinct.into_iter().collect(),
                extra: Default::default(),
            },
        );
    }
    for (id, by_n) in configs {
        if let Some(&i) = at.get(&id)
            && let Some(Record::Device(d)) = cx.records.get_mut(i)
        {
            d.aspects.assembly = Some(Assembly {
                configs: by_n.into_values().collect(),
                bom_basis: Default::default(),
                extra: Default::default(),
            });
        }
    }
}
