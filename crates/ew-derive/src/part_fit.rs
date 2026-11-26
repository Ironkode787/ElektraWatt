// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What fits a device (doc 07 §4.3 "What fits", FR-PRT-015, FR-DEV-033): [`part_fit`] judges a part for a device by the
//! pins and clamps the device has wired; the part dialog (`ew-api`'s `part_offers`) asks it for every offer and words
//! the first reason found. Whether the part's function is the device's is the caller's (it ranks the offers by it
//! already) and comes in as `function_fits`. Nothing is stored.

use ew_ids::Id;
use ew_mltext::natural_key;
use ew_model::EndpointKey;
use ew_schema::{Connection, Csa, Device, Part};
use ew_units::Dec3;

use crate::Q;
use crate::cps::FunctionCps;
use crate::potentials::PotentialPartition;
use crate::wire_props::WirePropsOf;

/// Why a part does not fit a device: the first reason found, in this order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Misfit {
    /// A part of another kind: no template function the device's main function can become.
    Function,
    /// The part has no pin of the designation `pin`, which the device has wired.
    MissingPin {
        /// The designation as the device prints it.
        pin: String,
    },
    /// The clamp `pin` takes `takes` (mm²); the wire `wire` on it is `csa` mm².
    ClampTooSmall {
        /// The clamp's designation.
        pin: String,
        /// The cross-sections it takes, smallest and largest, in mm².
        takes: (Dec3, Dec3),
        /// The wire that does not fit (its carrier connection).
        wire: Id<Connection>,
        /// The wire's effective cross-section in mm².
        csa: Dec3,
    },
}

/// `part_fit(device, part)`: `None` when `part` fits `device`, else the first reason it does not ([`Misfit`]): its
/// function (`function_fits` false), a pin the device has wired that the part's pattern lacks (designations compared
/// natural-normalised), a clamp of the pattern with a cross-section range that a wire on it lies outside of (its
/// effective cross-section, doc 08 §4). A part without a CP pattern has its pins from its functions and is not judged
/// by pins; a clamp without a range takes any wire.
pub fn part_fit(q: &Q, device: Id<Device>, part: &Part, function_fits: bool) -> Option<Misfit> {
    if !function_fits {
        return Some(Misfit::Function);
    }
    if part.cp_pattern.is_empty() {
        return None;
    }
    let partition = q.get::<PotentialPartition>(());
    let mut functions: Vec<_> = q.idx().functions_of_device(device).collect();
    functions.sort();
    for f in functions {
        let cps = q.get::<FunctionCps>(f);
        for (idx, mut conns) in q.idx().connected_cps_of_function(f) {
            let Some(cp) = cps.iter().find(|c| c.idx == idx) else {
                continue;
            };
            let key = natural_key(&cp.designation);
            let Some(data) = part
                .cp_pattern
                .iter()
                .find(|d| natural_key(d.designation.as_str()) == key)
            else {
                return Some(Misfit::MissingPin {
                    pin: cp.designation.clone(),
                });
            };
            let (Some(min), Some(max)) = (data.csa_min, data.csa_max) else {
                continue;
            };
            let Some(p) = partition.potential_at(EndpointKey::Cp(f, idx)) else {
                continue;
            };
            let props = q.get::<WirePropsOf>(p.id);
            conns.sort();
            for c in conns {
                for wire in props.by_connection.get(&c).into_iter().flatten() {
                    let Some(Csa::Mm2(csa)) = props
                        .wires
                        .get(wire)
                        .and_then(|e| e.csa.as_ref())
                        .map(|p| &p.value)
                    else {
                        continue;
                    };
                    if *csa < min || *csa > max {
                        return Some(Misfit::ClampTooSmall {
                            pin: cp.designation.clone(),
                            takes: (min, max),
                            wire: *wire,
                            csa: *csa,
                        });
                    }
                }
            }
        }
    }
    None
}
