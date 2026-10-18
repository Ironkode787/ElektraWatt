// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// @generated from data/identifier-sets.toml by tests/identifier_sets.rs. Do not edit: change the
// TOML file and run `EW_RDS_BLESS=1 cargo test -p ew-rds --test identifier_sets`.

//! The shipped identifier sets.

use super::{IdentifierSet, Mapping};
use crate::LetterCode;

/// A letter code of the tables; an invalid one fails compilation.
const fn l(code: &'static str) -> LetterCode {
    LetterCode::from_static(code)
}

static ROWS_EN81346_2_2009: [Mapping; 37] = [
    Mapping::from_static("ProtectionOvercurrent", None, l("F"), &[], false, "Protects a circuit against overcurrent: miniature circuit breakers, fuses, electronic circuit breakers"),
    Mapping::from_static("ProtectionResidualCurrent", None, l("F"), &[], false, "Disconnects a circuit on residual current to earth: RCDs and RCBOs"),
    Mapping::from_static("ProtectionOvervoltage", None, l("F"), &[], false, "Limits transient overvoltages: surge protective devices"),
    Mapping::from_static("PowerSupplyAcDc", None, l("G"), &[], false, "Converts mains AC into a regulated DC control voltage: 24 V power supply units"),
    Mapping::from_static("TransformerAcAc", None, l("T"), &[], false, "Transforms one AC voltage into another: control and isolating transformers"),
    Mapping::from_static("FrequencyConverter", None, l("T"), &[], false, "Feeds a motor with variable voltage and frequency: frequency converters and drives"),
    Mapping::from_static("SwitchingMain", None, l("Q"), &[l("S")], false, "Isolates or switches the main supply by hand: main switches, disconnectors, load-break switches"),
    Mapping::from_static("SwitchingPower", None, l("Q"), &[], false, "Switches a load circuit under remote control: contactors and power circuit breakers"),
    Mapping::from_static("Relay", None, l("K"), &[], false, "Relays a control signal: auxiliary, coupling and time relays"),
    Mapping::from_static("Controller", None, l("K"), &[], false, "Processes signals and data: PLC CPUs, I/O modules, bus couplers and gateways"),
    Mapping::from_static("NetworkSwitch", None, l("K"), &[], false, "Forwards data between network segments: industrial Ethernet switches"),
    Mapping::from_static("ValveTerminalNode", None, l("K"), &[l("U")], false, "Connects a valve terminal to its fieldbus and controls its valves: valve-terminal bus nodes"),
    Mapping::from_static("ValveTerminalModule", None, l("U"), &[l("K")], false, "Carries and drives the valves of a valve terminal: electrical sub-bases and valve modules"),
    Mapping::from_static("SensingTemperature", None, l("B"), &[], false, "Senses a temperature and signals it: thermostats, temperature switches and sensors"),
    Mapping::from_static("SensingPosition", None, l("B"), &[l("S")], false, "Senses a position or presence and signals it: door switches, limit switches, proximity sensors"),
    Mapping::from_static("SensingPressure", None, l("B"), &[], false, "Senses a pressure and signals it: pressure switches and transmitters"),
    Mapping::from_static("ManualCommand", None, l("S"), &[], false, "Turns a manual action into a control signal: push-buttons, selector switches, emergency-stop units"),
    Mapping::from_static("IndicatorLight", None, l("P"), &[], false, "Shows a state by light: pilot lights, signal lamps, signal towers"),
    Mapping::from_static("Meter", None, l("P"), &[], false, "Measures and displays a value: meters, panel instruments and scalar displays"),
    Mapping::from_static("Lighting", None, l("E"), &[], false, "Gives light for working: cabinet lamps and luminaires"),
    Mapping::from_static("Heating", None, l("E"), &[], false, "Heats electrically: enclosure heaters and fan heaters"),
    Mapping::from_static("Fan", None, l("G"), &[l("E")], false, "Moves air through an enclosure: filter fans and enclosure fans"),
    Mapping::from_static("MotorRotary", None, l("M"), &[], false, "Turns electrical energy into rotation: electric motors"),
    Mapping::from_static("SolenoidActuator", None, l("M"), &[], false, "Moves a part by electromagnetic force: solenoids, magnetic brakes and clutches"),
    Mapping::from_static("CylinderFluid", None, l("M"), &[], false, "Moves a part by fluid power: pneumatic and hydraulic cylinders"),
    Mapping::from_static("ValveDirectional", None, l("Q"), &[l("QM")], false, "Opens, closes or redirects a fluid flow on command: directional and shut-off valves"),
    Mapping::from_static("ValvePilot", None, l("Q"), &[l("K")], false, "Controls another valve with a small fluid signal: pilot valves"),
    Mapping::from_static("Terminal", None, l("X"), &[], false, "Connects conductors at a fixed point: terminals and terminal strips"),
    Mapping::from_static("TerminalEarth", None, l("X"), &[], false, "Connects protective-earth and functional-earth conductors: PE and FE terminals"),
    Mapping::from_static("ConnectorSignal", None, l("X"), &[], false, "Connects signal and bus lines detachably: plug connectors, sockets and bus connectors"),
    Mapping::from_static("CablePower", None, l("W"), &[], false, "Carries supply and load current: power cables and conductors"),
    Mapping::from_static("CableSignal", None, l("W"), &[], false, "Carries control signals or data: control, fieldbus and network cables"),
    Mapping::from_static("EarthingBar", None, l("W"), &[l("X")], false, "Collects earthing and equipotential-bonding conductors: PE and earthing bars"),
    Mapping::from_static("Enclosure", None, l("U"), &[], false, "Encloses and carries equipment: cabinets, enclosures and housings"),
    Mapping::from_static("MountingAccessory", None, l("U"), &[], false, "Holds equipment or cables in position: mounting rails, cable glands, holders"),
    Mapping::from_static("Resistor", None, l("R"), &[], false, "Limits a current or dissipates energy: resistors and braking resistors"),
    Mapping::from_static("Assembly", None, l("A"), &[], false, "Combines several main functions in one unit that no single category describes"),
];

static ROWS_IEC81346_2_2019: [Mapping; 36] = [
    Mapping::from_static("ProtectionOvercurrent", None, l("FC"), &[], false, "Protects a circuit against overcurrent: miniature circuit breakers, fuses, electronic circuit breakers"),
    Mapping::from_static("ProtectionResidualCurrent", None, l("FB"), &[], false, "Disconnects a circuit on residual current to earth: RCDs and RCBOs"),
    Mapping::from_static("ProtectionOvervoltage", None, l("FA"), &[], false, "Limits transient overvoltages: surge protective devices"),
    Mapping::from_static("PowerSupplyAcDc", None, l("TC"), &[l("TB")], false, "Converts mains AC into a regulated DC control voltage: 24 V power supply units"),
    Mapping::from_static("TransformerAcAc", None, l("TA"), &[], false, "Transforms one AC voltage into another: control and isolating transformers"),
    Mapping::from_static("FrequencyConverter", None, l("TA"), &[], false, "Feeds a motor with variable voltage and frequency: frequency converters and drives"),
    Mapping::from_static("SwitchingMain", None, l("QB"), &[], false, "Isolates or switches the main supply by hand: main switches, disconnectors, load-break switches"),
    Mapping::from_static("SwitchingPower", None, l("QA"), &[], false, "Switches a load circuit under remote control: contactors and power circuit breakers"),
    Mapping::from_static("Relay", None, l("KF"), &[], false, "Relays a control signal: auxiliary, coupling and time relays"),
    Mapping::from_static("Controller", None, l("KE"), &[], false, "Processes signals and data: PLC CPUs, I/O modules, bus couplers and gateways"),
    Mapping::from_static("NetworkSwitch", None, l("KF"), &[], false, "Forwards data between network segments: industrial Ethernet switches"),
    Mapping::from_static("ValveTerminalNode", None, l("KE"), &[], false, "Connects a valve terminal to its fieldbus and controls its valves: valve-terminal bus nodes"),
    Mapping::from_static("ValveTerminalModule", None, l("UA"), &[l("KF")], false, "Carries and drives the valves of a valve terminal: electrical sub-bases and valve modules"),
    Mapping::from_static("SensingTemperature", None, l("BT"), &[], false, "Senses a temperature and signals it: thermostats, temperature switches and sensors"),
    Mapping::from_static("SensingPosition", None, l("BG"), &[], false, "Senses a position or presence and signals it: door switches, limit switches, proximity sensors"),
    Mapping::from_static("SensingPressure", None, l("BP"), &[], false, "Senses a pressure and signals it: pressure switches and transmitters"),
    Mapping::from_static("ManualCommand", None, l("SJ"), &[l("SF"), l("SK")], false, "Turns a manual action into a control signal: push-buttons, selector switches, emergency-stop units"),
    Mapping::from_static("IndicatorLight", None, l("PF"), &[], false, "Shows a state by light: pilot lights, signal lamps, signal towers"),
    Mapping::from_static("Meter", None, l("PG"), &[], false, "Measures and displays a value: meters, panel instruments and scalar displays"),
    Mapping::from_static("Lighting", None, l("EA"), &[], false, "Gives light for working: cabinet lamps and luminaires"),
    Mapping::from_static("Heating", None, l("EB"), &[], false, "Heats electrically: enclosure heaters and fan heaters"),
    Mapping::from_static("Fan", None, l("GQ"), &[], false, "Moves air through an enclosure: filter fans and enclosure fans"),
    Mapping::from_static("MotorRotary", None, l("MA"), &[], false, "Turns electrical energy into rotation: electric motors"),
    Mapping::from_static("SolenoidActuator", None, l("MB"), &[], false, "Moves a part by electromagnetic force: solenoids, magnetic brakes and clutches"),
    Mapping::from_static("CylinderFluid", None, l("MM"), &[], false, "Moves a part by fluid power: pneumatic and hydraulic cylinders"),
    Mapping::from_static("ValveDirectional", None, l("QM"), &[], false, "Opens, closes or redirects a fluid flow on command: directional and shut-off valves"),
    Mapping::from_static("ValvePilot", None, l("KH"), &[], false, "Controls another valve with a small fluid signal: pilot valves"),
    Mapping::from_static("Terminal", None, l("XD"), &[], false, "Connects conductors at a fixed point: terminals and terminal strips"),
    Mapping::from_static("TerminalEarth", None, l("XE"), &[], false, "Connects protective-earth and functional-earth conductors: PE and FE terminals"),
    Mapping::from_static("ConnectorSignal", None, l("XG"), &[], false, "Connects signal and bus lines detachably: plug connectors, sockets and bus connectors"),
    Mapping::from_static("CablePower", None, l("WD"), &[], false, "Carries supply and load current: power cables and conductors"),
    Mapping::from_static("CableSignal", None, l("WG"), &[], false, "Carries control signals or data: control, fieldbus and network cables"),
    Mapping::from_static("EarthingBar", None, l("WE"), &[], false, "Collects earthing and equipotential-bonding conductors: PE and earthing bars"),
    Mapping::from_static("Enclosure", None, l("UC"), &[], false, "Encloses and carries equipment: cabinets, enclosures and housings"),
    Mapping::from_static("MountingAccessory", None, l("UA"), &[], false, "Holds equipment or cables in position: mounting rails, cable glands, holders"),
    Mapping::from_static("Resistor", None, l("RA"), &[], false, "Limits a current or dissipates energy: resistors and braking resistors"),
];

static ROWS_IEC60750_LEGACY: [Mapping; 35] = [
    Mapping::from_static("ProtectionOvercurrent", None, l("F"), &[], false, "Protects a circuit against overcurrent: miniature circuit breakers, fuses, electronic circuit breakers"),
    Mapping::from_static("ProtectionResidualCurrent", None, l("F"), &[], false, "Disconnects a circuit on residual current to earth: RCDs and RCBOs"),
    Mapping::from_static("ProtectionOvervoltage", None, l("F"), &[], false, "Limits transient overvoltages: surge protective devices"),
    Mapping::from_static("PowerSupplyAcDc", None, l("G"), &[], false, "Converts mains AC into a regulated DC control voltage: 24 V power supply units"),
    Mapping::from_static("TransformerAcAc", None, l("T"), &[], false, "Transforms one AC voltage into another: control and isolating transformers"),
    Mapping::from_static("FrequencyConverter", None, l("U"), &[], false, "Feeds a motor with variable voltage and frequency: frequency converters and drives"),
    Mapping::from_static("SwitchingMain", None, l("Q"), &[], false, "Isolates or switches the main supply by hand: main switches, disconnectors, load-break switches"),
    Mapping::from_static("SwitchingPower", None, l("K"), &[], false, "Switches a load circuit under remote control: contactors and power circuit breakers"),
    Mapping::from_static("Relay", None, l("K"), &[], false, "Relays a control signal: auxiliary, coupling and time relays"),
    Mapping::from_static("Controller", None, l("A"), &[l("K")], false, "Processes signals and data: PLC CPUs, I/O modules, bus couplers and gateways"),
    Mapping::from_static("NetworkSwitch", None, l("A"), &[l("K")], false, "Forwards data between network segments: industrial Ethernet switches"),
    Mapping::from_static("ValveTerminalNode", None, l("A"), &[l("K")], false, "Connects a valve terminal to its fieldbus and controls its valves: valve-terminal bus nodes"),
    Mapping::from_static("ValveTerminalModule", None, l("A"), &[], false, "Carries and drives the valves of a valve terminal: electrical sub-bases and valve modules"),
    Mapping::from_static("SensingTemperature", None, l("B"), &[], false, "Senses a temperature and signals it: thermostats, temperature switches and sensors"),
    Mapping::from_static("SensingPosition", None, l("S"), &[l("B")], false, "Senses a position or presence and signals it: door switches, limit switches, proximity sensors"),
    Mapping::from_static("SensingPressure", None, l("B"), &[], false, "Senses a pressure and signals it: pressure switches and transmitters"),
    Mapping::from_static("ManualCommand", None, l("S"), &[], false, "Turns a manual action into a control signal: push-buttons, selector switches, emergency-stop units"),
    Mapping::from_static("IndicatorLight", None, l("H"), &[], false, "Shows a state by light: pilot lights, signal lamps, signal towers"),
    Mapping::from_static("Meter", None, l("P"), &[], false, "Measures and displays a value: meters, panel instruments and scalar displays"),
    Mapping::from_static("Lighting", None, l("E"), &[], false, "Gives light for working: cabinet lamps and luminaires"),
    Mapping::from_static("Heating", None, l("E"), &[], false, "Heats electrically: enclosure heaters and fan heaters"),
    Mapping::from_static("Fan", None, l("E"), &[], false, "Moves air through an enclosure: filter fans and enclosure fans"),
    Mapping::from_static("MotorRotary", None, l("M"), &[], false, "Turns electrical energy into rotation: electric motors"),
    Mapping::from_static("SolenoidActuator", None, l("Y"), &[], false, "Moves a part by electromagnetic force: solenoids, magnetic brakes and clutches"),
    Mapping::from_static("CylinderFluid", None, l("M"), &[], false, "Moves a part by fluid power: pneumatic and hydraulic cylinders"),
    Mapping::from_static("ValveDirectional", None, l("Y"), &[], false, "Opens, closes or redirects a fluid flow on command: directional and shut-off valves"),
    Mapping::from_static("ValvePilot", None, l("Y"), &[], false, "Controls another valve with a small fluid signal: pilot valves"),
    Mapping::from_static("Terminal", None, l("X"), &[], false, "Connects conductors at a fixed point: terminals and terminal strips"),
    Mapping::from_static("TerminalEarth", None, l("X"), &[], false, "Connects protective-earth and functional-earth conductors: PE and FE terminals"),
    Mapping::from_static("ConnectorSignal", None, l("X"), &[], false, "Connects signal and bus lines detachably: plug connectors, sockets and bus connectors"),
    Mapping::from_static("CablePower", None, l("W"), &[], false, "Carries supply and load current: power cables and conductors"),
    Mapping::from_static("CableSignal", None, l("W"), &[], false, "Carries control signals or data: control, fieldbus and network cables"),
    Mapping::from_static("EarthingBar", None, l("X"), &[], false, "Collects earthing and equipotential-bonding conductors: PE and earthing bars"),
    Mapping::from_static("Resistor", None, l("R"), &[], false, "Limits a current or dissipates energy: resistors and braking resistors"),
    Mapping::from_static("Assembly", None, l("A"), &[], false, "Combines several main functions in one unit that no single category describes"),
];

pub(super) static SHIPPED: [IdentifierSet; 3] = [
    IdentifierSet::from_static("EN81346-2:2009", "EN 81346-2:2009 single-letter classes", &['D'], &ROWS_EN81346_2_2009),
    IdentifierSet::from_static("IEC81346-2:2019", "IEC 81346-2:2019 classes and subclasses", &['A', 'D', 'J', 'L', 'V', 'Y', 'Z'], &ROWS_IEC81346_2_2019),
    IdentifierSet::from_static("IEC60750-legacy", "IEC 60750 / DIN 40719-2 legacy letters", &[], &ROWS_IEC60750_LEGACY),
];
