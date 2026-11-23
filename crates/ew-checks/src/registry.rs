// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The registry of shipped Rust rules (ADR-0019 amendment 7: `ew-checks` owns the rule-implementation
//! registry). One list, in rule-ID order; the tests check IDs, codes and families against doc 15 §2.2.

use std::sync::LazyLock;

use crate::rule::Rule;
use crate::rules as shipped;

/// The shipped rule implementations with a type of their own, in rule-ID order; the EW-IMP rules join them in
/// [`RULES`].
static IMPLEMENTED: &[&dyn Rule] = &[
    &shipped::bus::BusAddress,
    &shipped::bus_net::AddressRange,
    &shipped::bus_net::Profinet,
    &shipped::bus::Termination,
    &shipped::bus_net::Rate,
    &shipped::bus_net::SegmentLimits,
    &shipped::bus_net::NoMaster,
    &shipped::bus_net::WrongCable,
    &shipped::bus_net::NetMismatch,
    &shipped::device::DanglingReference,
    &shipped::core::ConflictRecords,
    &shipped::core::MergeCollisions,
    &shipped::page::FolioOutsideSection,
    &shipped::page::FolioCollision,
    &shipped::core::TitleBlockOverride,
    &shipped::page::TocFixpoint,
    &shipped::page::ReportCoverageGap,
    &shipped::cable::InvalidCoreColour,
    &shipped::cable::CoreDiffersFromPart,
    &shipped::core::CableWithoutLength,
    &shipped::cable::ExternalWithoutCable,
    &shipped::cable::CoreAssignedTwice,
    &shipped::cable::ShieldNotConnected,
    &shipped::device::DeviceWithoutPart,
    &shipped::device::IncompletePartData,
    &shipped::device::PlaceholderPart,
    &shipped::device::OpenTbdValue,
    &shipped::device::ContactsExceedPart,
    &shipped::core::DeviceNotInSchematic,
    &shipped::device::OrderCodesCover,
    &shipped::core::UnpairedInterruptionPoint,
    &shipped::core::AmbiguousInterruptionPoint,
    &shipped::core::XrefCellInvalid,
    &shipped::page::TextBelowMinimum,
    &shipped::page::UnstyledTextSize,
    &shipped::core::DescriptionLint,
    &shipped::page::LayoutRule(&shipped::page::WIRE_THROUGH_CP),
    &shipped::page::LayoutRule(&shipped::page::WIRE_ALONG_WIRE),
    &shipped::page::LayoutRule(&shipped::page::WIRE_THROUGH_SYMBOL),
    &shipped::page::LayoutRule(&shipped::page::WIRE_THROUGH_TEXT),
    &shipped::page::GraphicLineOnCp,
    &shipped::page::LayoutRule(&shipped::page::FOUR_WAY_JUNCTION),
    &shipped::page::LayoutRule(&shipped::page::UNROUTED_WIRE),
    &shipped::page::LayoutRule(&shipped::page::WIRES_MEET),
    &shipped::page::LayoutRule(&shipped::page::TEXT_ON_TEXT),
    &shipped::page::LayoutRule(&shipped::page::TEXT_ON_PART),
    &shipped::page::LayoutRule(&shipped::page::PART_ON_PART),
    &shipped::core::WaiverMatchesNothing,
    &shipped::system::RuleNotEvaluated,
    &shipped::system::PackNotEvaluated,
    &shipped::fluid::WorkingPortOpen,
    &shipped::fluid::CoilBindings,
    &shipped::fluid::SparePositions,
    &shipped::fluid_media::TwoMedia,
    &shipped::fluid_media::TubeMisfit,
    &shipped::fluid_media::NoTubeSpec,
    &shipped::fluid_media::ExhaustOpen,
    &shipped::fluid_media::CoilVoltage,
    &shipped::fluid_media::NoSupply,
    &shipped::fluid::DrivesMismatch,
    &shipped::fluid_media::PortRoleConflict,
    &shipped::fluid::Occupancy,
    &shipped::fluid_media::LocationCrossing,
    &shipped::iolink::PortClassRule,
    &shipped::iolink::Limits,
    &shipped::layout::UnplacedDevice,
    &shipped::layout::PartNotOnDevice,
    &shipped::layout::LocationMismatch,
    &shipped::layout::Collision,
    &shipped::layout::CarrierIncompatible,
    &shipped::layout::AssemblyOrder,
    &shipped::num::TagDeviates,
    &shipped::num::FrozenValueDeviates,
    &shipped::plc::AddressOverlap,
    &shipped::plc::OutsideModule,
    &shipped::plc::BitOutOfRange,
    &shipped::plc_wiring::KindMismatch,
    &shipped::plc_wiring::InvalidSymbol,
    &shipped::plc::DuplicateSymbol,
    &shipped::plc::UsedWithoutAddress,
    &shipped::plc_wiring::WithoutConnection,
    &shipped::plc_wiring::WidthMismatch,
    &shipped::plc_wiring::SlotRules,
    &shipped::plc::DiffersFromMap,
    &shipped::plc_wiring::CardSupply,
    &shipped::plc_wiring::OutputLoad,
    &shipped::plc::TbdAddress,
    &shipped::plc_exchange::ExchangeDrift,
    &shipped::plc::NotAModule,
    &shipped::plc::PinsUnbound,
    &shipped::pot::TypeOrVoltageConflict,
    &shipped::pot::PelvSelvBonding,
    &shipped::pot::FePinOnPe,
    &shipped::pot::GnyeOnNonPe,
    &shipped::pot::BuOnNonN,
    &shipped::pot::PeUndersized,
    &shipped::pot::PeIsland,
    &shipped::pot::UnconnectedCp,
    &shipped::pot::JunctionFeedAmbiguous,
    &shipped::pot::RingInDrawingGraph,
    &shipped::pot::NSwitching,
    &shipped::pot::PhysicalWireConflict,
    &shipped::pot::WireNumberConflict,
    &shipped::pot::RedundantOverride,
    &shipped::pot::TwoPotentialNames,
    &shipped::pot::LoadNotReturned,
    &shipped::pot::CsaBelowProtection,
    &shipped::process::ProcessTag,
    &shipped::plc_exchange::OpenInstrumentTag,
    &shipped::rds::DesignationDoesNotParse,
    &shipped::rds::SeparatorInCode,
    &shipped::rds::LetterAgainstFunction,
    &shipped::rds::DuplicateDesignation,
    &shipped::rds::ReservedLetters,
    &shipped::rds::ConfusableCharacter,
    &shipped::rds::StructureCodeVariants,
    &shipped::rel::FourEyes,
    &shipped::rel::StaleCommitted,
    &shipped::rel::LabelOrder,
    &shipped::safety::FAddress,
    &shipped::safety::Pair,
    &shipped::safety::TestPulse,
    &shipped::term::TerminalWithoutDesignation,
    &shipped::term::DuplicateDesignation,
    &shipped::term::SpareWiredOnFieldSide,
    &shipped::term::BridgeGroupInvalid,
    &shipped::term::RailBondingOnNonPe,
    &shipped::term::TerminalOnUnjoinedWire,
];

/// Every shipped rule implementation, in rule-ID order: [`IMPLEMENTED`] and one rule per code of
/// [`shipped::imp::IMP`].
static RULES: LazyLock<Vec<&'static dyn Rule>> = LazyLock::new(|| {
    let mut rules: Vec<&'static dyn Rule> = IMPLEMENTED.to_vec();
    rules.extend(shipped::imp::IMP_RULES.iter().map(|r| r as &dyn Rule));
    rules.sort_by_key(|r| r.meta().id);
    rules
});

/// Every shipped rule implementation, in rule-ID order.
pub fn rules() -> &'static [&'static dyn Rule] {
    RULES.as_slice()
}

/// The shipped rule with this ID.
pub fn rule(id: &str) -> Option<&'static dyn Rule> {
    RULES.iter().copied().find(|r| r.meta().id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The class of an EW-CORE number block (doc 15 §2.2).
    fn core_class(n: u32) -> &'static str {
        match n {
            1..=19 => "100",
            21..=29 => "101",
            31..=39 => "003",
            41..=49 => "102",
            51..=59 => "103",
            61..=79 => "104",
            _ => "105",
        }
    }

    fn class_of(family: &str, n: u32) -> &'static str {
        match family {
            "TERM" => "001",
            "POT" => "005",
            "PLC" | "SAF" | "PRC" => "110",
            "BUS" | "IOL" => "111",
            "RDS" | "NUM" => "017",
            "CORE" => core_class(n),
            "REL" => "190",
            "LAY" => "130",
            "FL" => "120",
            "IMP" => "160",
            _ => "???",
        }
    }

    #[test]
    fn ids_codes_and_packs_follow_the_registry_rules() {
        let mut ids: Vec<&str> = RULES.iter().map(|r| r.meta().id).collect();
        let sorted = {
            let mut s = ids.clone();
            s.sort_unstable();
            s
        };
        assert_eq!(ids, sorted, "the registry is in rule-ID order");
        ids.dedup();
        assert_eq!(ids.len(), RULES.len(), "rule IDs are unique");
        let mut codes: Vec<&str> = RULES.iter().map(|r| r.meta().code).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), RULES.len(), "codes are unique");
        for r in RULES.iter() {
            let m = r.meta();
            let parts: Vec<&str> = m.id.split('-').collect();
            assert_eq!(parts.len(), 3, "{}", m.id);
            assert_eq!(parts[0], "EW", "{}", m.id);
            assert_eq!(parts[2].len(), 3, "{}", m.id);
            let n: u32 = parts[2].parse().expect("a three-digit number");
            let (_, _, offset) = crate::rule::FAMILIES
                .iter()
                .find(|(f, _, _)| *f == parts[1])
                .unwrap_or_else(|| panic!("{}: family not in the table", m.id));
            assert_eq!(
                m.code,
                format!("{}{:03}", class_of(parts[1], n), n + offset),
                "{}",
                m.id
            );
            assert!(rule(m.id).is_some());
            for s in m.supersedes {
                assert_ne!(*s, m.id, "{} supersedes itself", m.id);
            }
        }
        assert!(rule("EW-NOPE-001").is_none());
    }
}
