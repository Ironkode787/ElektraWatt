// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Effective wire properties with provenance: `wire_props_of(potential)`, `wire_props(conn)` (ARCH §6.4,
//! doc 08 §4, D-08).
//!
//! # The chain (doc 08 §4.1, doc 03 §9.5)
//!
//! Each property takes the value of the highest level that sets it, with its provenance:
//!
//! | Level | Source | [`PropSource`] |
//! |---|---|---|
//! | 7 | CDP: the wire's merged `Connection.overrides` | `Override` |
//! | 6 | intrinsic: the `Conductor` of a core | `Conductor` |
//! | 5, 4 | `NetDef` (1.0; no record in this format) | — |
//! | 3 | `PotentialDef.props`, first definition in ID order that sets the member | `PotentialDef` |
//! | 2 | profile rule: colour row × size row, PE-class and bond rules | `ColourRow`, `SizeRow` |
//! | 1 | project default `ProjectSettings.extra["wire_default"]` | `ProjectDefault` |
//! | 0 | the built-in defaults of the derivation profile, when the project has no profile | `BuiltIn` |
//!
//! [`pick`] is the precedence engine: the first level, from the top, that has a value.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_model::EndpointKey;
use ew_schema::{
    CircuitClass, ColourSpec, Conductor, Connection, Csa, Function, PartRef, PotentialDef, PotentialType,
    WireProps,
};

use crate::note::DeriveNote;
use crate::potentials::{Partition, PotentialId, PotentialPartition};
use crate::query::{Policy, Query};
use crate::stitch::{StitchGraph, StitchGraphQ};
use crate::wire_profile::{
    Attrs, WireProfiles, builtin_class, builtin_colour_for, builtin_csa, load as load_profiles,
};
use crate::wires::{EndKind, PhysicalWire, PhysicalWiresOf, WireEnd};
use crate::{FunctionCps, PotentialFlow, Q};

/// Which size rule gave a level-2 cross-section (doc 08 §4.2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SizeRule {
    /// The potential's circuit class.
    Class,
    /// A PE wire: the highest class at its two ends (FR-CON-038).
    PeHighestAtEnds,
    /// A bond wire: the class of its non-PE potential.
    Bond,
}

/// Where an effective value comes from (D-08).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PropSource {
    /// Level 7: the override on this constituent connection (the one that carries the value, which need not
    /// be the wire's carrier: a shared star feed arm, the other leg of an interruption-point pair).
    Override(Id<Connection>),
    /// Level 6: the conductor of a core.
    Conductor(Id<Conductor>),
    /// Level 3: a potential definition.
    PotentialDef(Id<PotentialDef>),
    /// Level 2: row `row` (0-based, cover-table order) of the colour profile `profile` (its key).
    ColourRow {
        /// The profile key.
        profile: String,
        /// The row index.
        row: usize,
    },
    /// Level 2: the size row of `class` of the size profile `profile` (its key), chosen by `rule`.
    SizeRow {
        /// The profile key.
        profile: String,
        /// The circuit class of the row.
        class: CircuitClass,
        /// The rule that chose the class.
        rule: SizeRule,
    },
    /// Level 1: the project default.
    ProjectDefault,
    /// Level 0: the built-in defaults of the derivation profile `profile` (its version).
    BuiltIn {
        /// The derivation profile version.
        profile: String,
    },
}

impl PropSource {
    /// The level of the chain (doc 08 §4.1); 0 for the built-in defaults.
    pub fn level(&self) -> u8 {
        match self {
            PropSource::Override(_) => 7,
            PropSource::Conductor(_) => 6,
            PropSource::PotentialDef(_) => 3,
            PropSource::ColourRow { .. } | PropSource::SizeRow { .. } => 2,
            PropSource::ProjectDefault => 1,
            PropSource::BuiltIn { .. } => 0,
        }
    }
}

/// A value with its provenance.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Prov<T> {
    /// The value.
    pub value: T,
    /// Where it comes from.
    pub source: PropSource,
}

impl<T> Prov<T> {
    /// `value` from `source`.
    pub fn new(value: T, source: PropSource) -> Prov<T> {
        Prov { value, source }
    }

    /// The level of its source.
    pub fn level(&self) -> u8 {
        self.source.level()
    }
}

/// The precedence engine: the candidates of one property, one per level, highest level first; the first
/// that is set wins.
///
/// ```
/// use ew_derive::{Prov, PropSource, pick};
///
/// let csa = pick([None, Some(Prov::new(15, PropSource::ProjectDefault))]);
/// assert_eq!(csa.map(|p| p.value), Some(15));
/// ```
pub fn pick<T>(levels: impl IntoIterator<Item = Option<Prov<T>>>) -> Option<Prov<T>> {
    levels.into_iter().flatten().next()
}

/// The effective properties of one physical wire (doc 08 §4.1 "Fields").
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct EffectiveProps {
    /// Cross-section.
    pub csa: Option<Prov<Csa>>,
    /// Colour.
    pub colour: Option<Prov<ColourSpec>>,
    /// Wire part.
    pub wire_part: Option<Prov<PartRef>>,
    /// Insulation type.
    pub insulation: Option<Prov<Ident>>,
    /// Wire function.
    pub function: Option<Prov<Ident>>,
    /// Shielded.
    pub shielded: Option<Prov<bool>>,
    /// Twisted.
    pub twisted: Option<Prov<bool>>,
    /// The circuit class the level-2 size rule used, if it ran.
    pub class: Option<CircuitClass>,
}

/// The value of [`WirePropsOf`]: the effective properties of every physical wire of a potential.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WirePropsMap {
    /// By wire ID (the carrier connection).
    pub wires: BTreeMap<Id<Connection>, EffectiveProps>,
    /// For each constituent connection, the wires it belongs to (two for a shared star feed arm).
    pub by_connection: BTreeMap<Id<Connection>, Vec<Id<Connection>>>,
    /// The notes of this derivation (also pushed to the `Q`).
    pub notes: Vec<DeriveNote>,
}

/// `wire_props_of(potential)` (doc 08 §4). Initial policy Memo (ARCH §6.4).
///
/// # Readings
///
/// - **Potential of a wire.** Levels 3 and 2 read the key potential. A bond wire (carried by a bond's `via`)
///   belongs to both potentials of the bond, so its result must not depend on the key: level 3 reads the
///   definitions of both potentials in `PhysicalWire::potentials` order, and level 2 follows the bond rule
///   (the PE colour row, the class of the non-PE potential, [`SizeRule::Bond`]).
/// - **PE rule.** A wire whose potential has class `Pe` takes the highest class at its two ends
///   (`Mains > Control > Analogue > Bus`, `Pe` and unknown classes ignored; no class found: `Control`):
///   at a device end the potentials of every CP of every function of the device, at a terminal end the
///   potentials of every CP of every terminal of its strip, an external or open end nothing.
/// - **Class.** [`EffectiveProps::class`] is the class the size rule resolved for the wire, set whenever
///   the wire has a potential, whichever level then supplied the cross-section.
/// - **Cores.** The profile never recolours a core; a core without a conductor colour takes the level-1
///   or built-in colour. Its cross-section comes from the conductor first, then the rules.
/// - **No row.** A colour row that no potential attribute matches, or a class without a size row, leaves
///   level 2 unset and the value falls to level 1 or the built-in default.
#[derive(Debug)]
pub struct WirePropsOf;

impl Query for WirePropsOf {
    type Key = PotentialId;
    type Value = WirePropsMap;
    const FAMILY: &'static str = "wire_props_of";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &PotentialId) -> Arc<WirePropsMap> {
        let physical = q.get::<PhysicalWiresOf>(*key);
        let profiles = load_profiles(q.project());
        let ctx = Ctx {
            q,
            profiles: &profiles,
            partition: q.get::<PotentialPartition>(()),
            graph: q.get::<StitchGraphQ>(()),
            version: q.profile().version.clone(),
        };
        let mut map = WirePropsMap::default();
        for wire in &physical.wires {
            for c in &wire.constituents {
                let wires = map.by_connection.entry(*c).or_default();
                wires.push(wire.id);
            }
            map.wires.insert(wire.id, ctx.props_of(wire, *key));
        }
        for wires in map.by_connection.values_mut() {
            wires.sort();
            wires.dedup();
        }
        let mut notes = profiles.notes.clone();
        notes.sort();
        notes.dedup();
        for note in &notes {
            q.note(note.clone());
        }
        map.notes = notes;
        Arc::new(map)
    }
}

/// `wire_props(conn)`: a cold lookup into [`WirePropsOf`] of the connection's potential(s) (ARCH §6.3):
/// the wires the connection belongs to with their effective properties, in wire ID order. Empty for a
/// connection that forms no physical wire (bridges, bus lines, overview connections).
#[derive(Debug)]
pub struct ConnWireProps;

impl Query for ConnWireProps {
    type Key = Id<Connection>;
    type Value = Vec<(Id<Connection>, EffectiveProps)>;
    const FAMILY: &'static str = "wire_props";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Id<Connection>) -> Arc<Vec<(Id<Connection>, EffectiveProps)>> {
        let Some(conn) = q.connections().get(*key) else {
            return Arc::new(Vec::new());
        };
        let graph = q.get::<StitchGraphQ>(());
        let partition = q.get::<PotentialPartition>(());
        let mut potentials: Vec<PotentialId> = [&conn.a, &conn.b]
            .into_iter()
            .filter_map(EndpointKey::of)
            .filter_map(|at| partition.of(&graph, at))
            .map(|p| p.id)
            .collect();
        potentials.sort();
        potentials.dedup();
        let mut out: BTreeMap<Id<Connection>, EffectiveProps> = BTreeMap::new();
        for p in potentials {
            let props = q.get::<WirePropsOf>(p);
            for w in props.by_connection.get(key).into_iter().flatten() {
                if let Some(e) = props.wires.get(w) {
                    out.entry(*w).or_insert_with(|| e.clone());
                }
            }
        }
        Arc::new(out.into_iter().collect())
    }
}

/// The reads shared by the wires of one query.
struct Ctx<'a> {
    q: &'a Q,
    profiles: &'a WireProfiles,
    partition: Arc<Partition>,
    graph: Arc<StitchGraph>,
    version: String,
}

/// The rank of a class in the PE rule: `Mains > Control > Analogue > Bus`; `Pe` and unknown classes have
/// none.
fn rank(class: &CircuitClass) -> Option<u8> {
    match class {
        CircuitClass::Mains => Some(3),
        CircuitClass::Control => Some(2),
        CircuitClass::Analogue => Some(1),
        CircuitClass::Bus => Some(0),
        _ => None,
    }
}

/// The highest class of `classes` by [`rank`]; `Control` when none ranks (FR-CON-038).
fn highest(classes: impl IntoIterator<Item = CircuitClass>) -> CircuitClass {
    classes
        .into_iter()
        .filter_map(|c| rank(&c).map(|r| (r, c)))
        .max_by_key(|(r, _)| *r)
        .map_or(CircuitClass::Control, |(_, c)| c)
}

/// `Some(Prov)` of a set value.
fn set<T: Clone>(value: &Option<T>, source: impl FnOnce() -> PropSource) -> Option<Prov<T>> {
    value.as_ref().map(|v| Prov::new(v.clone(), source()))
}

/// `Some(Prov(true))` of a flag that is on (an unset flag is not an override).
fn flag(on: bool, source: impl FnOnce() -> PropSource) -> Option<Prov<bool>> {
    on.then(|| Prov::new(true, source()))
}

/// Level 3: the first definition, in the order given, whose props set the member `get` reads.
fn from_defs<T>(
    defs: &[(Id<PotentialDef>, Arc<PotentialDef>)],
    get: impl Fn(&WireProps) -> Option<T>,
) -> Option<Prov<T>> {
    defs.iter()
        .find_map(|(d, r)| get(&r.props).map(|v| Prov::new(v, PropSource::PotentialDef(*d))))
}

/// The members one level sets, as candidates for [`pick`].
struct Level {
    csa: Option<Prov<Csa>>,
    colour: Option<Prov<ColourSpec>>,
    wire_part: Option<Prov<PartRef>>,
    insulation: Option<Prov<Ident>>,
    function: Option<Prov<Ident>>,
    shielded: Option<Prov<bool>>,
    twisted: Option<Prov<bool>>,
}

impl Level {
    /// The members of a [`WireProps`] value (levels 3 and 1), all with `source`.
    fn of_props(p: &WireProps, source: &PropSource) -> Level {
        let src = || source.clone();
        Level {
            csa: set(&p.csa, src),
            colour: set(&p.colour, src),
            wire_part: set(&p.wire_part, src),
            insulation: set(&p.insulation, src),
            function: set(&p.function, src),
            shielded: flag(p.flags.shielded, src),
            twisted: flag(p.flags.twisted, src),
        }
    }
}

impl Ctx<'_> {
    /// The final attributes of a potential: the flow values, else the partition's own.
    fn attrs(&self, id: PotentialId) -> Attrs {
        if let Some(e) = self.q.get::<PotentialFlow>(id).as_ref() {
            return Attrs {
                ptype: e.ptype.as_ref().map(|a| a.value.clone()),
                voltage: e.voltage.as_ref().map(|a| a.value),
                ac_dc: e.env.as_ref().and_then(|a| a.value.ac_dc.clone()),
            };
        }
        match self.partition.get(id) {
            Some(p) => Attrs {
                ptype: p.ptype.as_ref().map(|a| a.value.clone()),
                voltage: p.voltage.as_ref().map(|a| a.value),
                ac_dc: p.env.as_ref().and_then(|a| a.value.ac_dc.clone()),
            },
            None => Attrs::default(),
        }
    }

    /// The circuit class of a potential: its definition's, else the size profile's class rules (the
    /// built-in rules without a profile).
    fn class_of(&self, id: PotentialId, attrs: &Attrs) -> CircuitClass {
        if let Some(c) = self.partition.get(id).and_then(|p| p.circuit_class.as_ref()) {
            return c.value.clone();
        }
        match &self.profiles.size {
            Some(size) => size.class_of(attrs),
            None => builtin_class(attrs),
        }
    }

    /// The classes of the potentials holding a CP of `functions`.
    fn classes_of_functions(&self, functions: Vec<Id<Function>>) -> Vec<CircuitClass> {
        let mut potentials: Vec<PotentialId> = Vec::new();
        for f in functions {
            for cp in self.q.get::<FunctionCps>(f).iter() {
                if let Some(p) = self.partition.of(&self.graph, EndpointKey::Cp(f, cp.idx)) {
                    potentials.push(p.id);
                }
            }
        }
        potentials.sort();
        potentials.dedup();
        potentials
            .into_iter()
            .map(|p| self.class_of(p, &self.attrs(p)))
            .collect()
    }

    /// The classes at one wire end (the PE rule).
    fn end_classes(&self, end: &WireEnd) -> Vec<CircuitClass> {
        if end.kind != EndKind::Device {
            return Vec::new();
        }
        let f = end.cp.function;
        let Some(function) = self.q.functions().get(f) else {
            return Vec::new();
        };
        let device = function
            .device
            .and_then(|d| self.q.devices().get(d).map(|dev| (d, dev)));
        let mut functions: Vec<Id<Function>> = vec![f];
        match device {
            None => {}
            Some((_, dev)) if dev.aspects.terminal.is_some() => {
                if let Some(t) = &dev.aspects.terminal {
                    for terminal in self.q.idx().terminals_of_strip(t.strip) {
                        functions.extend(self.q.idx().functions_of_device(terminal));
                    }
                }
            }
            Some((d, _)) => functions.extend(self.q.idx().functions_of_device(d)),
        }
        functions.sort();
        functions.dedup();
        self.classes_of_functions(functions)
    }

    /// The class of a PE wire: the highest class at its two ends.
    fn highest_at_ends(&self, wire: &PhysicalWire) -> CircuitClass {
        highest(wire.ends.iter().flat_map(|e| self.end_classes(e)))
    }

    /// The effective properties of one wire of the potential `key`.
    fn props_of(&self, wire: &PhysicalWire, key: PotentialId) -> EffectiveProps {
        let q = self.q;
        let bond = wire.bond.is_some();
        // The potentials levels 3 and 2 read: the key potential, or both of a bond wire.
        let pots: Vec<PotentialId> = if bond { wire.potentials.clone() } else { vec![key] };

        // Level 7: the merged overrides, each naming the constituent that carries it.
        let o = &wire.overrides;
        let s = &wire.override_sources;
        let src7 = |from: Option<Id<Connection>>| move || PropSource::Override(from.unwrap_or(wire.id));
        let l7 = Level {
            csa: set(&o.csa, src7(s.csa)),
            colour: set(&o.colour, src7(s.colour)),
            wire_part: set(&o.wire_part, src7(s.wire_part)),
            insulation: set(&o.insulation, src7(s.insulation)),
            function: set(&o.function, src7(s.function)),
            shielded: flag(o.flags.shielded, src7(s.shielded)),
            twisted: flag(o.flags.twisted, src7(s.twisted)),
        };

        // Level 6: the conductor of a core.
        let conductor = wire.core.and_then(|c| q.conductors().get(c).map(|r| (c, r)));
        let l6_csa = conductor.and_then(|(c, r)| set(&r.csa, || PropSource::Conductor(c)));
        let l6_colour = conductor.and_then(|(c, r)| set(&r.colour, || PropSource::Conductor(c)));

        // Level 3: per member, the first definition in ID order that sets it.
        let defs: Vec<(Id<PotentialDef>, Arc<PotentialDef>)> = pots
            .iter()
            .filter_map(|p| self.partition.get(*p))
            .flat_map(|p| p.defs.iter())
            .filter_map(|d| q.potential_defs().get(*d).map(|r| (*d, r.clone())))
            .collect();
        let l3 = Level {
            csa: from_defs(&defs, |p| p.csa.clone()),
            colour: from_defs(&defs, |p| p.colour.clone()),
            wire_part: from_defs(&defs, |p| p.wire_part.clone()),
            insulation: from_defs(&defs, |p| p.insulation.clone()),
            function: from_defs(&defs, |p| p.function.clone()),
            shielded: from_defs(&defs, |p| p.flags.shielded.then_some(true)),
            twisted: from_defs(&defs, |p| p.flags.twisted.then_some(true)),
        };

        // Level 2: the colour row and the size row, and level 0 from the same attributes.
        let colour_attrs = if bond {
            Attrs {
                ptype: Some(PotentialType::Pe),
                ..Attrs::default()
            }
        } else {
            self.attrs(key)
        };
        let (class, rule) = if bond {
            let non_pe = wire
                .potentials
                .iter()
                .map(|p| (*p, self.attrs(*p)))
                .find(|(_, a)| a.ptype != Some(PotentialType::Pe));
            let class = non_pe.map_or(CircuitClass::Control, |(p, a)| self.class_of(p, &a));
            (class, SizeRule::Bond)
        } else {
            let class = self.class_of(key, &colour_attrs);
            if class == CircuitClass::Pe {
                (self.highest_at_ends(wire), SizeRule::PeHighestAtEnds)
            } else {
                (class, SizeRule::Class)
            }
        };
        let colour2 = match (&self.profiles.colour, wire.core) {
            (Some(profile), None) => profile.select(&colour_attrs).map(|(row, c)| {
                Prov::new(
                    c.clone(),
                    PropSource::ColourRow {
                        profile: profile.key().to_owned(),
                        row,
                    },
                )
            }),
            _ => None,
        };
        let csa2 = self.profiles.size.as_ref().and_then(|size| {
            size.csa_of(&class).map(|csa| {
                Prov::new(
                    csa.clone(),
                    PropSource::SizeRow {
                        profile: size.key().to_owned(),
                        class: class.clone(),
                        rule,
                    },
                )
            })
        });

        // Level 1: the project default.
        let l1 = Level::of_props(&self.profiles.default, &PropSource::ProjectDefault);

        // Level 0: the built-ins of the derivation profile.
        let built_in = || PropSource::BuiltIn {
            profile: self.version.clone(),
        };
        let csa0 = Some(Prov::new(builtin_csa(&class), built_in()));
        let colour0 = Some(Prov::new(builtin_colour_for(&colour_attrs, &class), built_in()));

        EffectiveProps {
            csa: pick([l7.csa, l6_csa, l3.csa, csa2, l1.csa, csa0]),
            colour: pick([l7.colour, l6_colour, l3.colour, colour2, l1.colour, colour0]),
            wire_part: pick([l7.wire_part, l3.wire_part, l1.wire_part]),
            insulation: pick([l7.insulation, l3.insulation, l1.insulation]),
            function: pick([l7.function, l3.function, l1.function]),
            shielded: pick([l7.shielded, l3.shielded, l1.shielded]),
            twisted: pick([l7.twisted, l3.twisted, l1.twisted]),
            class: Some(class),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highest_class_ranks_mains_over_control_over_analogue_over_bus() {
        use CircuitClass::*;
        assert_eq!(highest([Bus, Analogue]), Analogue);
        assert_eq!(highest([Analogue, Control, Bus]), Control);
        assert_eq!(highest([Control, Mains, Pe]), Mains);
        // Pe is ignored, and no class at all is Control.
        assert_eq!(highest([Pe]), Control);
        assert_eq!(highest([]), Control);
    }

    #[test]
    fn levels_are_ordered_by_precedence() {
        let levels = [
            PropSource::ColourRow {
                profile: "p".into(),
                row: 0,
            },
            PropSource::ProjectDefault,
            PropSource::BuiltIn { profile: "v".into() },
        ];
        assert_eq!(levels.clone().map(|s| s.level()), [2, 1, 0]);
        let top = pick([
            None,
            Some(Prov::new(1, levels[0].clone())),
            Some(Prov::new(2, levels[1].clone())),
        ]);
        assert_eq!(top.map(|p| p.value), Some(1));
    }
}
