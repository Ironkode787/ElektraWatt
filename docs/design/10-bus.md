# PLC, I/O and fieldbus: the bus

A chapter of [PLC, I/O and fieldbus](10-plc-and-bus.md), whose table says where each section lives.

---

### 2.7 Bus model

```rust
pub struct BusPort {                     // aspect on a Function (03 §14)
    pub interface: Ident,                // "DP", "X1"
    pub port_label: Option<Ident>,       // "P1R"
    pub bus: BusSystem,                  // ProfibusDp | Profinet | EtherCat | EthernetIp | IoLink | Asi | CanOpen | ModbusTcp
    pub role: PortRole,                  // Master | Slave | Controller | Device | Passive
    pub address: AddrValue<NetAddress>,  // Value | Tbd | NotApplicable
    pub termination: Option<OnOff>,      // connectors and devices with a termination switch (FR-BUS-004)
    pub logical_net: Option<Id<LogicalNet>>,
    pub physical_net: Option<Id<PhysicalNet>>,
}
pub enum NetAddress {
    Profibus(u8), Profinet { name: Ident, ip: Option<Ipv4Addr>, mask: Option<Ipv4Addr>, gateway: Option<Ipv4Addr> },
    EtherCat { position: Option<u16>, alias: Option<u16> }, EthernetIp { ip: Ipv4Addr, mask: Ipv4Addr },
    Asi { addr: u8, ab: Option<AsiAb> }, IoLinkPort(u8), CanOpen(u8), ModbusTcp { ip: Ipv4Addr, unit: u8 },
}
pub struct LogicalNet  { pub bus: BusSystem, pub name: Ident, pub master: Option<Endpoint>, pub rate: Option<BusRate>,
                         pub space: AddressSpace }                      // address space as in 03 §14
pub struct PhysicalNet { pub name: Ident, pub medium: Medium,           // added: medium (copper, fibre, …)
                         pub order_hint: Vec<Id<Function>> }            // added; segments are derived (§3.8)
```

- **Logical networks** carry addressing, rate and master (FR-BUS-002). Ports read the rate; they never copy it.
- **Physical networks** are named records. Their **segments are derived** from `Bus` connections, connector
  through-links and plug-on links (`bus_segments`, §3.8). `order_hint` orders members that are not drawn on any
  topology page. [03](03-domain-model.md) §14 derives them the same way: a stored member order
  would be a second copy of what the topology page already states.
- **Bus connectors** are devices of kind `BusConnector` with a port function (role Passive). That function has an
  In entry (A1, B1), an Out entry (A2, B2, optional CPs), a device side and `termination`. -3X1 mates with -201U1's
  DP socket through a `Bus` connection without a cable.
- **Bus cables** are ordinary cables ([08](08-connections-terminals-cables.md) §7.5) drawn single-line as one
  `Connection{kind: Bus(BusLine{cable})}` (FR-CBL-008).
- **A bus line's cable and plugs from the window.** Cable… on selected bus lines (the Draw tab's
  button, the line's menu, the line's Cable and Plugs rows in Properties) opens the bus line's form: the cable's tag,
  its part (the cable parts whose `cable_for` names the line's bus first, the others marked "not made for"), its
  length, and for one line the plug at each end with its termination (a script names the cable part and the plugs
  by part number, as every part parameter). `bus.cable.set` links the lines to the cable,
  or to a new one made as `cable.create` makes it (its part's cores), in one step; several selected lines take one
  cable. A plug drawn as its own connector device stays the model above; a plug **typed on a line** is the smallest
  model: a part on the line's end, `BusLine.plug_a` / `plug_b` = `BusPlug {part, termination?}` (optional, additive;
  an older build keeps them in `extra`), not a device. It is a parts-list line of the device at that end
  (`LineSource::Plug`), the bus cable list names it at that end, and EW-BUS-004 reads its termination as the
  terminating element of the port it sits on when the port has no switch of its own. A plug part is an accessory
  with a passive bus interface of its bus (starter EW-GEN-PLG-1 … -10). The cable is numbered on
  the page of its first bus line, as a wiring page's cable on its first core's page, and `bus.cable.set` gives each
  line's page the cable's mark (a wire marker listing the line, across the middle of its longest run: the tag in the
  tag style, the type under it, movable); a line no mark lists (an older file) prints the tag beside it in the tag
  style. The form also offers the project's cables made for the bus (a further line runs on in one, the person's
  choice), and its lists end in New bus cable… and New plug…: the part editor's short form of a cable for the bus
  (its cores as the bus has them, `part_cable::bus_cores`, no cross-section needed; its `cable_for`) or of a plug (its
  bus, straight or angled, programming socket and switchable resistor, worded into its description), chosen in the
  form at once. The cable's cores are used by its line: the cable overview and `api_v1_conductor` give each core the
  line's ends, and Connect cores… offers none of them; a cable with a length entered counts it in the parts list.
  A new cable chosen for lines whose cable then carries nothing (no other line, no core, no drawing) removes that
  cable in the same step, and the form proposes the tag the numbering then gives. A plug at an end without a device
  (an interruption point) counts with the line's cable, else with the device at the other end.
- `NetAddress` types `profinet {name, ip?, mask?, gateway?}`, `ethercat {position?, alias?}`
  and `io_link_port` besides `profibus` (new values that an older reader keeps as `Unknown`); `BusPort` has the
  optional `mrp` (`manager`, `client`). The P1/P2 neighbours of FR-BUS-010 are the ports' `Bus` connections, read from
  `bus_segments`. An EtherCAT terminal's position after its coupler is its module's positional slot (FR-BUS-011).
- **Nets from drawn lines.** A person who draws a bus between ports means one
  network. Normaliser N6 (`bus_nets`, doc 06 §3.2 row 14b) takes a new or changed `Bus` line's segment (every port
  of its bus system reached over bus lines from its ends) and gives the segment's ports one logical net: the one net
  they name, else, with two ports or more, a new `LogicalNet` named for the bus system (`PROFIBUS DP`, then
  `PROFIBUS DP 2` …) whose master is the CP of the one `master` or `controller` port. A passive port (a bus plug)
  passes the net through and takes none, as -3X1 does: two stations joined plug to plug are one net.
  It never changes a net a port names (two nets in one segment stay EW-BUS-010's). No member is added:
  `logical_net` was there, and an older 0.1 build reads the net as any other. The physical net goes the same way:
  the one physical net the segment's ports name, a
  plug's included, else a new `PhysicalNet` of copper named as a new logical net is, which every port without one
  takes. EW-BUS-004 checks the termination of a terminated line bus only (PROFIBUS DP, CANopen): an Ethernet link,
  IO-Link and AS-Interface have none, so a drawn PROFINET star stays silent.
- **One interface, many ports; switches and devices with ports.** A bus interface is one port
  function carrying one `BusPort`, so one address: a PROFINET device name and IP address belong to the interface, not
  to a socket. Its sockets are the function's connection points. `PartBusPort.ports` (optional, additive) gives their
  number n; a port function whose `bus_port` names such an interface of a part of its device derives the CPs `P1` …
  `Pn` (`function_cps`; the FnDef's own CPs for the indices it has, a copy of its first CP beyond them), and absent,
  the FnDef's own CPs stay (the PROFINET port's P1 and the optional P2). A `port_label` still names one port of an
  interface listed port by port (an import's "X1 P1R"). The neighbours of FR-BUS-010 are the bus lines at those CPs,
  read from `bus_segments`. What the device is follows from the interface's role:
  - an **unmanaged switch** is an interface of role `passive` with its sockets (EW-GEN-ESW-5: five): no address, it
    passes the net through and names none (N6), and `net_members` lists it under the net its bus lines join it to
    (`through`; the Bus navigator's row carries the badge "passes the net through");
  - a **managed switch** (EW-GEN-ESW-8: eight sockets) and a **device with ports** (a drive, an HMI, a gateway;
    EW-GEN-PND-1: two) are interfaces of role `device`: a node with its one address, which N6 also walks through;
  - a **head station** is a part with a PLC section of role station and its interfaces (N0 makes it `head_station`).
  - A device given to a PLC (Add to PLC: its menu, its Bus panel row, the PLC row of
    its Properties, or "Use <tag>" in the new-station dialog) is a station of that PLC. A **network switch**, a device
    no part of which has a PLC section and one of whose interfaces has more than two sockets, belongs to the PLC with
    no rack: no station view opens, a toast with Undo names the PLC, and its menu offers no Rack…. Put it on a bus page
    places every interface of the station not drawn there, a box each, and makes a bus-topology page when the project
    has none; a station whose part has no bus interface is refused in words. The starter valve terminals have their bus
    node as an interface: EW-GEN-VTM-1 PROFIBUS DP, EW-GEN-VTM-2 PROFINET.
  A part without pins whose first interface has a starter port function names that function as its main template
  entry (`box.black` draws such an entry as a box with a socket per CP, the function staying a normal one). A later
  device-description import fills the same members: the interfaces, their roles and their socket counts. The PLC
  exchange exports the stations of a configuration project, so a switch or a device without a PLC section is left out
  of the AML file (no refusal).
- **A device on a bus-topology page is drawn as its interfaces.** A box of format 0.1
  draws the CPs of one function (`BoxCp.idx` is a CP of the representation's first function, and an older build reads
  it so), and an interface is one port function with one address; so the smallest model that shows every port of a
  device with several interfaces is **one box per interface**, side by side: the CPU EW-GEN-CPU-2 stands as a box DP
  and a box X1, a head station with one DP interface as one box, a switch as its X1. Each box is the black-box frame
  of type `topology` on the interface's port function, with a socket per port (`P1` … `Pn` for an interface that
  counts n, else one socket, the port function's first CP, named by the interface); a box of several sockets names
  its interface in its middle, every box prints the device's tag and the interface's address, and the first box alone
  its part. Its supply pins are not on the bus page: they are its black box on a schematic page, which the device's
  unplaced function places there (the black box tool draws an unplaced black-box function). `bus.place_ports` draws
  them in one step: for a new device of a part (from the device browser, the template functions made as
  `device.place` makes them, none drawn there) or an existing one (Place port `<interface>` on a station's row while
  a bus-topology page is shown, and the Bus panel's Ports not drawn yet); each interface takes the function that
  carries it, else a port-less bus-port function of the device, else a new port function of its bus's starter FnDef
  (the PROFINET port's for a bus without one), whose port it gets as N5 gives one. Nothing is added to the format: an
  older 0.1 build reads every box as any black box.
  The boxes of one device behave as one box where a
  person would expect one: a box dragged, or moved by the keyboard, takes the other boxes of its device's interfaces on
  that page with it, one `rep.move`; the row of boxes being drawn stands inside the frame's drawing area, the whole
  row moved by whole grid steps where its second corner would let a box run past the edge, and it is refused in words
  ("Another device stands here: draw the boxes of the bus interfaces on a free place") where a box of it would cover a
  device drawn on the page, and with its size where it is larger than the area. Each box is still selected, reshaped
  and deleted on its own. A device placed first on a bus page takes its tag from that page (no other page draws
  it); when its own box is drawn later on a schematic page (Place here, the device browser's unplaced function, Draw on
  a page…), its tag becomes that page's, by the numbering rule that a device's tag names the page of its main function
  (FR-NUM-007, doc 03 §5.1). Decided for the toast, not for keeping the tag: the tag that names the page with the device's supply
  and channels is the one cross-references, lists and the cabinet take, and keeping the bus page's would need an
  exception in the rule `ew_commands::numbering` and `ew_derive::MainRep` share. The toast names both tags and why
  ("-105A1 is now -104A1: a device takes its tag from the page that draws its own box, not from a bus page, which shows
  its interfaces").
- **External partners** are placeholder devices with `PlcStation{role: ExternalPartner}`, `BusNode`, a `BusPort`
  and an `ExternalRef` (project, location, device), but no parts. They are excluded from the BOM (FR-BUS-005). On
  folio 3, the external interruption point "DP" inside the External Connection box is the physical segment end, and
  its `ExternalRef` names JE-N1000-03 in +J-E-206.

---

### 3.8 Bus topology, addresses and termination

**A5. Segment derivation.**
1. Build a graph whose nodes are bus-discipline CPs: port functions and connector In, Out and device-side entries.
2. Its edges are `Bus` connections (with or without a cable), each connector's internal through-link and plug-on
   links.
3. Each connected component within one `PhysicalNet` is a segment. For line buses (PROFIBUS, RS-485) the component
   must be a path. A branch is reported by EW-BUS-006 as a stub.
4. Ends are degree-1 nodes. An external interruption point counts as an end.
5. Member order walks from the master end: the end whose port is the logical net's master, or the external end
   whose `ExternalRef` names the master; failing that, from the end with the smallest (`order_hint` position, ID).

**Termination (EW-BUS-004).** At each end, the termination state of the nearest terminating element (connector port
or device port with a switch) must be On, and every other terminating element on the segment must be Off. An
external end whose state is unknown gives one info (FR-BUS-016). In the running example the segment is external end
(JE-N1000-03, master) · -3W1 · -3X1 (On) · -201U1:DP, the order of FR-BUS-003 and [03](03-domain-model.md) §14. It
gives no error and one info. Setting -3X1 Off gives an error, and its rendered
text changes from "Switch to "ON"" to "Switch to "OFF"" because the text is a property arrangement over
`termination` (FR-BUS-004).

**Addresses.**
- `AddrValue::Tbd` renders "TBD", appears in the open-items list and blocks release (EW-BUS-001, FR-CHK-034).
- `NotApplicable` is for ports without addresses (IO-Link device side).
- Ranges, segment lengths and PROFINET name rules are data with a `verified` flag. Unverified constants report at
  most a warning (FR-CHK-046; NFR-COMP-05).

| Bus | Release | Address data | Main checks |
|---|---|---|---|
| PROFIBUS DP | MVP (model, address, termination); 1.0 (rates from GSD) | station address 0–125 (unverified), rate from the logical net (9.6 kbit/s … 12 Mbit/s) | EW-BUS-001, -002, -004, -005, -006 |
| PROFINET IO | 1.0 | device name, IPv4, mask, gateway, P1/P2 neighbours, MRP role | EW-BUS-001, -003, -007 |
| IO-Link | 1.0 | port number; mode IO-Link / DI / DQ / deactivated; class A or B | EW-IOL-001, -002 |
| EtherCAT | 1.0 structure; 1.x E-bus budget | position after the coupler, alias; rail order from layout at 1.0 ([12](12-panel-layout.md)) | EW-BUS-008 (1.x) |
| EtherNet/IP, AS-Interface | 1.x | IP, mask, RPI, DLR; 1–31, 1A–31B | EW-BUS-001, -002 |
| CANopen, Modbus | Later | node ID 1–127; IP + unit ID | EW-BUS-001 |

**IO-Link ports (1.0, FR-BUS-012).** A master port is a channel of kind `MultiFunction{mode}` with modes IO-Link,
DI, DQ and deactivated. The master's part declares the port window (fixed bytes per port, or a configurable maximum
such as 32 bytes ▲). In IO-Link mode the connected device's IODD supplies the process-data lengths and a struct
data type: the Balluff sample's 88 input bits give an 11-byte input window and a struct-typed symbol. The module's
I/O width is the sum of its port windows in their current modes, so a mode change re-runs A1 for the following
Auto modules (§8, item 10). EW-IOL-002 compares the IODD lengths with the window.

@@@ END

IO-Link: a master's part declares its `iolink` window; each `multi_function` channel is a
port whose window in IO-Link mode is its connected device's process data in whole bytes (the traced target's part's
IO-Link port; the part's window when no device is connected), one input byte in DI mode, one output byte in DQ mode
and none deactivated. A1 allocates the module's sum per direction, so a mode change moves the following automatic
modules, and a port prints the first byte of its window (`IB0`), a DI or DQ port bit 0 of its byte. The port takes
the device's struct data type. EW-IOL-001 and -002 check class, mode, window and cable length.

The pages of a bus potential come from its `Bus` connections, because a bus line joins nets but is no wire. Its
colour and size come from the cable (`BusLine.cable`), so the potential list's colour/size cell prints "(cable)", and
the cable is an edge of the segment graph (A5), not a member of the potential.

