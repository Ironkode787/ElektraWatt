// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The registry of shipped Rust rules (ADR-0019 amendment 7: `ew-checks` owns the rule-implementation
//! registry). One list, in rule-ID order; the tests check IDs, codes and families against doc 15 §2.2.

use crate::rule::Rule;
use crate::rules as shipped;

/// Every shipped rule implementation, in rule-ID order.
static RULES: &[&dyn Rule] = &[
    &shipped::bus::BusAddress,
    &shipped::bus::Termination,
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
    &shipped::page::GraphicLineOnCp,
    &shipped::core::WaiverMatchesNothing,
    &shipped::system::RuleNotEvaluated,
    &shipped::system::PackNotEvaluated,
    &shipped::fluid::WorkingPortOpen,
    &shipped::fluid::CoilBindings,
    &shipped::fluid::SparePositions,
    &shipped::fluid::DrivesMismatch,
    &shipped::fluid::Occupancy,
    &shipped::imp::ImportEntries(&shipped::imp::IMP[0]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[1]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[2]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[3]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[4]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[5]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[6]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[7]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[8]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[9]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[10]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[11]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[12]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[13]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[14]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[15]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[16]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[17]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[18]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[19]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[20]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[21]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[22]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[23]),
    &shipped::imp::ImportEntries(&shipped::imp::IMP[24]),
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
    &shipped::plc::DuplicateSymbol,
    &shipped::plc::UsedWithoutAddress,
    &shipped::plc::DiffersFromMap,
    &shipped::plc::TbdAddress,
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
    &shipped::term::TerminalWithoutDesignation,
    &shipped::term::DuplicateDesignation,
    &shipped::term::SpareWiredOnFieldSide,
    &shipped::term::BridgeGroupInvalid,
    &shipped::term::RailBondingOnNonPe,
    &shipped::term::TerminalOnUnjoinedWire,
];

/// Every shipped rule implementation, in rule-ID order.
pub fn rules() -> &'static [&'static dyn Rule] {
    RULES
}

/// The shipped rule with this ID.
pub fn rule(id: &str) -> Option<&'static dyn Rule> {
    RULES.iter().copied().find(|r| r.meta().id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The family table of doc 15 §2.2 for the families with shipped rules: class and code offset.
    const FAMILIES: &[(&str, &str, u32)] = &[
        ("TERM", "EW-TERM", 0),
        ("POT", "EW-POT", 0),
        ("PLC", "EW-PLC", 0),
        ("BUS", "EW-BUS", 0),
        ("RDS", "EW-IEC81346", 0),
        ("NUM", "EW-IEC81346", 100),
        ("CORE", "EW-CORE", 0),
        ("REL", "EW-REL", 0),
        ("LAY", "EW-LAYOUT", 0),
        ("FL", "EW-FL", 0),
        ("IMP", "EW-IMP", 0),
    ];

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
            "PLC" => "110",
            "BUS" => "111",
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
        for r in RULES {
            let m = r.meta();
            let parts: Vec<&str> = m.id.split('-').collect();
            assert_eq!(parts.len(), 3, "{}", m.id);
            assert_eq!(parts[0], "EW", "{}", m.id);
            assert_eq!(parts[2].len(), 3, "{}", m.id);
            let n: u32 = parts[2].parse().expect("a three-digit number");
            let (_, pack, offset) = FAMILIES
                .iter()
                .find(|(f, _, _)| *f == parts[1])
                .unwrap_or_else(|| panic!("{}: family not in the table", m.id));
            assert_eq!(m.pack, *pack, "{}", m.id);
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

    /// T5 (WP-S7.05 D6): the refusal of a rule whose `docs` is not its page `reference/rules/<id in lower case>`.
    fn docs_refusal(m: &crate::rule::RuleMeta) -> Option<String> {
        let page = format!("reference/rules/{}", m.id.to_lowercase());
        (m.docs != page).then(|| {
            format!(
                "{}: docs \"{}\" is not the rule's page {page} (FR-CHK-015)",
                m.id, m.docs
            )
        })
    }

    /// FR-CHK-015: every shipped rule declares the reference of its page of the user guide, and a wrong one is
    /// refused in words (T5).
    #[test]
    fn every_rule_declares_its_documentation_page() {
        let refused: Vec<String> = RULES.iter().filter_map(|r| docs_refusal(r.meta())).collect();
        assert_eq!(refused, Vec::<String>::new());
        let wrong = crate::rule::RuleMeta {
            docs: "reference/rules/ew-core",
            ..crate::rules::core::CABLE_WITHOUT_LENGTH
        };
        assert_eq!(
            docs_refusal(&wrong).as_deref(),
            Some(
                "EW-CORE-033: docs \"reference/rules/ew-core\" is not the rule's page reference/rules/ew-core-033 \
                 (FR-CHK-015)"
            )
        );
    }
}
