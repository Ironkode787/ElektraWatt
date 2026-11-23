# Architecture: the derived-data engine

A chapter of the [architecture](04-architecture.md): its §6, under the numbers the code and the other
documents cite. The crates, their order, where a thing lives, how a text reaches a person and what the shell
and the window own are in the architecture itself; read this chapter by the section you need.

---

## 6. Derived-data engine

### 6.1 Cold first, memoise where measured

Every derived value is a **pure function over a tracked-read context**:

```rust
pub fn device_tag(q: &Q, dev: Id<Device>) -> Arc<Tag> { /* reads TagSpec, structure, main rep's folio, scheme */ }
// Q exposes only tracked accessors: q.devices().get(id), q.idx().reps_by_page(p), q.get::<PageOrder>(()).
// Queries receive &Q and nothing else, so an untracked model read inside a query does not compile.
```

- **Phase 1 (spine): cold.** Every query runs in **cold mode**: computed on demand, with a request-scoped cache
  that prevents repeated work within one top-level evaluation. No engine is needed, and contributors write plain
  functions.
- **As built.** `ew-derive` declares each family through the trait
  `Query { type Key; type Value; const FAMILY: &str; const POLICY: Policy /* Cold | Memo */; fn run(q: &Q, key: &Key) -> Arc<Value> }`.
  `Q` owns a model snapshot (an O(1) clone), its request-scoped cache (one typed map per family; a second query type
  with the same family name, or a re-entrant key, panics) and a `Tracker` that sees every read as a `DepKey`
  (project, row, table, index key, whole index, reserved records) with its observed stamp. Phase 1 uses the no-op
  `ColdTracker`; `RecordingTracker` serves tests and `--explain`. `POLICY` records the intended policy of §6.4
  but every family still runs cold; the spike switches `ew-memo` in behind the tracker
  without changing a query body. No accessor of `Q` returns the model, a table, an index or an `Op` (a
  `compile_fail` doctest guards it). What a derivation cannot express as a value (an unknown pinned profile, a
  numbering scheme not applied, the built-in default frame used, a malformed shim) goes to a request-scoped note
  channel, `DeriveNote`, which `ew-api`'s `Session` surfaces as findings.
- **Phase 2 (from the performance spike, G-SP): memoise where profiled.** A query is promoted to `Memo` when profiling on the
  synthetic L/XL projects shows it on an interactive path. Every promotion is a one-line policy change, and the
  oracle verifies it.
- **G-SP outcome: Recycle** (2026-10-20, ADR-0002 amendment 4). The spike
  ([report](../research/spike-g-sp-2026-10.md)) measured the cold global connectivity at 2.6–3× (S), 25–27× (M)
  and 66–97× (L) over NFR-PERF-05 (XL: 308× for a wire, 462× for a move), and an opaque memo of
  `potential_partition` worse than cold; the folio renumber, the open-time proxy and the affected-only profile
  change are within budget at S, M and L, while at XL the renumber (1.2×) needs the memos of `page_order` and
  `device_tag` and the affected-only profile change (2.1×) the cheaper wire chain as well. The memo list stays; the
  connectivity families are reshaped (§6.4) and become incremental (below), and the engine gains push
  invalidation (§6.2). The re-test after the recycle runs on a BM-class runner.
- **Fallback: incremental connectivity** (ADR-0002 amendment 4 replaces decision 7). The former
  fallback, page-scoped dirty flags with a coarse global recompute, fails as worded: the coarse recompute alone costs
  87 ms at M. A family that cannot meet its budget by memoisation is instead maintained incrementally, still under
  the oracle. For global connectivity: the local nets stay one memo per page; the stitch graph and the potential
  partition are kept per connected component; an edit that only adds connections unions components in
  near-constant time, and an edit that removes one or changes a page's nets recomputes only the components that
  held the touched page nets, from the per-page nets; potential names are derived afterwards by their own family.
  The cold family stays the reference that the oracle compares with in CI. The algorithm is WP-S1.07's work.

### 6.2 `ew-memo`: the cut-down engine

`ew-memo` applies salsa's red-green idea to persistent snapshots. **v1 contains only the following:**
1. **Revision stamps.** Every input row, index key and table carries `changed_at: Rev`. A memo stores
   `{value: Arc<V>, deps: SmallVec<(DepKey, observed)>, verified_at, changed_at}`.
2. **Verification and early cut-off.** A read at revision R is a hit if `verified_at == R`. Otherwise the
   dependencies are verified recursively: if none changed, the memo is re-stamped; else the query re-executes.
   - If the new value equals the old one (`Eq`; a 128-bit fingerprint for bulky values is specified; not built), `changed_at` is **backdated**
     and dependents stay green.
   - This keeps a wire edit on folio 102 from touching folio 203, and it lets unchanged display-list layers keep
     their GPU buffers.
3. **Three durability levels:** library, settings, drawing. A memo whose dependencies are all library-durable skips
   verification while the user draws. Symbol geometry and form layout are examples.
4. **LRU with a byte budget** for bulky memos (display lists, pagination). Specified for 1.0; not built: no memo value is
   measured or evicted, and the display-list and pagination families are plain `Memo`.
5. **Cycle detection.** A cycle panics in debug builds. A value computed under a caught cycle is volatile: it is
   returned but never kept, and its readers are volatile too (ADR-0002 amendment 5). The one legitimate fixpoint, a TOC whose page count depends
   on itself, is settled inside one query by a closed form and one confirming round (ADR-0018 amendment 4; [09](09-reports-forms-output.md) §3.1).
6. **Statistics** per query and per commit: hits, verifications, re-executions, cut-offs, kept as counters that the
   tests assert. Emitting them through `tracing` is specified for 1.0; not built.
7. **Reverse dependencies.** A generic API returning, per query family, the memos whose recorded reads include a given
   key is specified; not built (`Query::RDEPS`, `Store::dependents`). What exists is the push index of the additions
   below, keyed by page-level keys, which `advance` walks from the touched inputs. The online check engine would build its
   `rdeps` index from the generic form
   ([checks, revisions and collaboration](15-checks-revisions-collaboration.md) §3.3).

**Out of v1:**
- parallel evaluation of the main store;
- sharded locks;
- merging memos from forks (adopt-if-unchanged, the one exception of §2, is specified; not built);
- salsa-style cancellation of readers.

The estimated size is 1.5–2.5 kLOC. Two owners are required by the end of the spine (§15).

**Status.** `ew-memo` v1 exists with everything listed above except what items 4 and 7 and the statistics events say is not built, an oracle (`Every` or a deterministic
`OneIn(n)` sample, checked at every depth) and O(1) forks; ADR-0002 amendment 3 records the precise durability
contract and the cycle and cut-off readings. The synthetic S–XL generator and the benchmark harness are gone (no
performance runs during the alpha).

**Additions after G-SP** (2026-10-20, ADR-0002 amendment 4). The spike showed that
verification by walking every dependency cannot meet NFR-PERF-05 at L: 12 µs per page memo, so 18.8 ms for the
1,499 untouched page memos at L, and a hit that clones its memo with the dependency list first. v1 therefore also
contains:
1. **Push invalidation.** `advance` receives the commit's touched inputs from the change feed. The memos reachable
   from them through the reverse-dependency index (item 7) are marked possibly changed and verified at their next
   read; every other memo is re-stamped in O(1) at its next read. The index uses page-level keys (page, device
   shard, table, settings member): with row-level keys it would hold about 56 MB at XL, over its 40 MB budget
   (not measured).
2. **O(1) hits.** `verified_at` is checked before anything is cloned; dependency lists sit behind an `Arc`.
3. **Nested memos through `Q`.** `Q` gets a memo backend; a memoised family read inside another records one memo
   dependency instead of its flattened reads. The report-pages provider is bound to the store at creation.
4. **Value bytes and an incremental re-execution hook.** Families reporting their value bytes (`Query::bytes`) to a memory gate is specified for 1.0;
   not built. A family may receive its previous value and the touched inputs when it re-executes; its result
   is checked against the cold `run` by the oracle (§6.1, incremental connectivity).

**Status (ADR-0002 amendment 5).** All four additions are built, with part dependencies
(a reader of one part of a producer's value is invalidated only by that part) as the mechanism of the per-component
maintenance of §6.1. `ew-derive`'s `MemoStore` backs `Q`: in a `Session` memoisation is on by default, every family
of policy Memo is memoised (§6.4), a commit advances the store with the page-level keys of its change
feed, and a committed oracle test over the sample project compares the memoised session with cold evaluation.

**Status (ADR-0002 amendment 6).** The partition is an incremental producer of parts
and the connectivity inputs are incremental index and per-key families (§6.4). Value parts (`Q::get_value_part`)
make high fan-out values cut-off firewalls; the topology families are note-free; `Q::detached` shares the request cache
of its frame, so a cold `Q` evaluates every (family, key) once; `Store::memory` counts the memos and dependency entries of each family (no bytes); producers
forget dead keys and drop carried dependencies (`Store::forget`, `Store::drop_carried`), unread parts keep no stamp
and every advance collects orphan memos, since 2026-10-21 only those of dead keys, so an
unread memo of a live key is kept (ADR-0002 amendment 7); debug guards enforce `Query::PRODUCER` and `Query::NOTE_FREE`;
`MemoStore::fork` is public. The G-SP re-test on a BM-class runner is open (the benchmark harness is gone).

### 6.3 Granularity and memory rules

1. **No per-CP memos.** The finest memo key is a page, device, potential, strip, module, station, report block or
   layout space. A function is also allowed as a key where the value
   per function is small and an incremental reader needs it per function: `function_links(f)` (up to one memo per
   function with a CP in the graph, about 150,000 at XL-A) and `ip_info(f)` (one per interruption point). Their
   count and bytes are under the memory gate (rule 4) like every other family.
2. **Group small results.** Per-connection values are grouped per potential: `wire_props_of(potential) -> Arc<Map>`.
   The per-connection accessor `wire_props(conn)` is a cold lookup into that map.
3. **Promote only what pays.** A query is memoised only if its cold cost exceeds about 20 µs, or if it is read on
   every frame.
4. **Memory gate.** Specified for 1.0; not built: a check of memo count and bytes per query family on the XL fixture (§13). Memo values are not measured.
5. **No whole-table reads on interactive paths** (2026-10-20, ADR-0002 amendment 4). A query on the commit path
   reads through indexes and the settings members it needs; a read of a whole table or of the whole project row
   invalidates the query on every edit, which the spike found in `stitch_graph` and `potential_partition`.

### 6.4 Query catalogue

*Status (ADR-0002 amendments 5 and 6).* In a `Session` every built family below whose policy is Memo is memoised through `ew-memo`, those of `ew-reports` included (a Policy cell that says Memo + LRU is read as Memo: the LRU is specified for 1.0; not built); `query_over` and the tests' `Q::new` stay
cold. Each built family declares its policy in its `Query` impl (`POLICY`), and this table follows the code. Connectivity is incremental: `page_local_nets` per page, the index families
and the per-key memos re-execute only for what an edit touched, and `potential_partition` is a producer of parts
whose incremental execution recomputes the touched components only; it falls back to the cold algorithm, and
declares the difference, when it cannot attribute a change or `Changes::via` exceeds 4,096 entries (a bulk device
edit at L does today). **Still whole-project:** `stitch_graph` (the cold reference, read by tests only);
`potential_flow_bfs` (kept when the partition and the attributes it read are unchanged, otherwise one BFS); the
re-pairing of `ip_pairing` after a change; `bond_paths`, `bond_vias` and `rail_bonds` (small); and the report kinds,
which read the whole partition. *Note-free* below means that the family keeps what it must report in its value and
pushes no notes; its helper queries run in `Q::detached` (ADR-0002 amendment 6). *Value part* means that readers
take the whole value as a cut-off point (`Q::get_value_part`).

| Query (key) | Depends on | Initial policy | Notes |
|---|---|---|---|
| `page_order()` incl. generated pages | pages, sections, report blocks, the generated pages of the installed `ReportPagesProvider` (`report_block_pages`) | Memo | Source of Prev/Next, TOC, PDF outline and `/PageLabels` (REF-073). The TOC page count comes from a closed form with one confirming round, else EW-CORE-025; a `Q` without the report engine has no generated pages |
| `drawn_page_order()` | pages, sections | Memo | Drawn pages and dividers only, in the relative order of `page_order`. Every query that feeds pagination (`bom_rows`, the report row sources) reads this one, never `page_order` |
| `page_title(pageref)` | `Page.title`; for a divider the section, its title and `section_structure`; for a generated page `page_order` | Cold | The page description printed in the title block and the TOC (FR-PAG-010): the drawn page's title, the divider's "structure - Section n - Page a - b - title" text (FR-PAG-009), or the title the block's pagination supplies (FR-PAG-011) |
| `page_structure(page)` | the page's structure slots, structure nodes (via `structure_path`) | Cold | The page's own structure per aspect; pages do not inherit, so an absent, `none` or unknown slot leaves the aspect empty |
| `section_structure(section)` | `page_structure` of every drawn page of the section | Cold | The structure all drawn pages of a section share; empty when they disagree in any aspect or the section has no pages. Dividers and generated pages of the section print it (FR-PAG-009) |
| `page_ref_structure(pageref)` | `page_structure`, `section_structure`, report blocks | Cold | The structure of any page of the page order: a drawn page's own, the section's shared structure for a divider and for a generated page of a block in a section, and for a generated page of a block outside every section (the TOC) the structure all drawn pages share (empty when they disagree) |
| `function_cps(f)` | function definition, overrides | Cold | CP instantiation |
| `device_tag(dev)` → `{full, displayed(page)}` | `TagSpec`, identifier set, structure nodes, main representation's folio, assembly parent | Memo | EPLAN-style identifier suppression; child tags `-201U3-Q1` (D-27, D-41) |
| `main_function(dev)` | the device's functions | Cold | The device's function with role `Main` and the smallest ID |
| `main_rep(dev)` | `main_function`, the function's representations | Cold | The smallest-ID representation that draws the main function and is not a contact image ([03](03-domain-model.md) §5.1); none for an unplaced device |
| `displayed_tag(dev, page)` | `device_tag`, `page_structure` | Cold | The device's tag as printed on a page (FR-NUM-002); empty for a missing device |
| `numbering_settings()` | the project's `numbering` settings member | Cold | The numbering settings parsed once per `Q`; a member that is present but does not parse yields none (the profile's default applies) and a `MalformedShim` note |
| `structure_path(node)` | structure nodes, the parent's `structure_path` | Cold | The node's path from the root of its tree: the parent's path plus the node. A parent cycle on a corrupt snapshot is cut, noted as `QueryCycle`, and the paths stay finite |
| `structure_of(dev)` | the device's own slot, its terminal strip or parent device, `main_rep`'s page structure, `structure_path` | Cold | The resolved structure of a device per aspect: own slot, else the strip's or parent's, else the page of its main representation, else the parent's for an unplaced device (FR-PRJ-019). Inheritance cycles are cut and noted |
| `devices_by_structure()` | `structure_of` of every device | Memo | Every device under each structure node it resolves to (any aspect), devices in ID order; a request-scoped derived index |
| `rep_anchor_cell(rep)` | position, the symbol's anchor policy, form grid | Cold | Derived from the symbol definition, never from rendered geometry; fixes "3-B0" (D-14). The anchor point comes from one exact function in `ew-schema` over `SymbolDef` geometry (integer nm, ties toward zero), shared with `ew-scene` and the importers |
| `cell_of(point, page)` | the page's frame form and connection grid | Cold | Exact frame-cell lookup: a point less than 1 nm from a grid point (the rounding window) is evaluated at that grid point on the rational grid, any other point at its exact nm value ([06](06-schematic-editor.md) §5.1, FR-PAG-020). Used by `rep_anchor_cell` and the cursor readout; `ew-units` has only the integer `Grid` primitives |
| `frame_settings()` | the project's `frames` settings member | Cold | The `frames` settings member parsed once per `Q`, malformed parts dropped and noted once as `MalformedShim` |
| `frame_grid(page)` | the page, `frame_settings`, the frame form | Cold | The page's frame grid, evaluated once per page per `Q`: the page's own form, else the entry for its page type, else the default form, else the built-in default grid (noted); title, TOC and divider pages default to the no-grid variant (FR-PAG-018) |
| `xref_text(rep)` | `xref_relation`, partners' anchor cells, xref format | Memo | Star or chain interruption points; "(201-E1)", "∞ 203-B4". A text that lists several items carries one link per item |
| `xref_format()` | the `xref_format` settings member, else the pinned profile's `xref` table | Cold | A malformed member is noted and the profile's format applies |
| `xref_relation(rep)` | the representation's function, `ip_pairing` (the point's `Ip` part), `ip_notes` (value part), the device's representations | Cold | The cross-reference relation of a representation: partner points, or the main and auxiliary representations of its device |
| `device_boxes(device)` | the device's representations with a `box` | Cold | The black-box outlines of a device in page space, once per device, so a device with many auxiliaries stays linear |
| `page_local_nets(page)` | connections, junctions, reps on the page, `bond_vias` (value part) | Memo | Union-find over a few thousand edges, one memo per page; an edit re-executes only the pages it touched |
| `page_ids()` | the page table | Memo, incremental index | Every page in ID order; a renumber re-selects its pages and cuts off. Note-free |
| `pageless_links()` | the connection table, `bond_vias` | Memo, incremental index | Every `Bridge` connection and every page-less joining connection that no bond cuts, by ID, with an index by end; a change of the bond cut re-selects every row. Note-free |
| `bond_index()` | the bond table | Memo, incremental index | Every bond by ID with its kind and via connection. Note-free |
| `bond_vias()` | `bond_index` | Memo (value part) | The connections named as some `Bond.via` (the bond cut, [08](08-connections-terminals-cables.md) §3.7), in ID order; read by `page_local_nets` and `pageless_links` as a cut-off point. Small, recomputed whole |
| `ip_functions()` | the function table | Memo, incremental index | The specification of every interruption-point function, by ID. Note-free |
| `def_points()` | the potential-definition table | Memo, incremental index | Where each potential definition is placed. Note-free |
| `rail_terminals()` | the device table | Memo, incremental index | The terminal devices with `rail_bonded` set, in ID order. Note-free |
| `ip_info(f)` | `ip_functions`, the point's CP, first representation and page, `page_structure`, `drawn_page_order`, `rep_anchor_cell`, the `ip_pairing` settings member | Memo, per function (§6.3 rule 1) | One interruption point as the pairing and the names read it: specification, CP, page, pairing scope node and page-order position; a move or renumber re-derives this one value. Note-free (its EW-CORE-051 notes are in its value) |
| `ip_pairing()` | `ip_functions`, `ip_info` of every point, the `ip_pairing` settings member | Memo, incremental producer of `Ip` parts | Groups the points by name and scope and pairs them (star or chain, [03](03-domain-model.md) §10). Keeps its value when no point value changed, otherwise pairs again (whole) and declares the points whose pairing changed. Note-free. Split out of the stitch graph; `physical_wires_of` and `xref_relation` read it per point |
| `ip_notes()` | `ip_pairing` | Memo (value part) | Pushes the EW-CORE-051 notes of the pairing, in the order of the stitch graph; read whole by the cross-reference relation and the stitch graph |
| `function_links(f)` | the function, its FnDef, `function_cps`, `structure_of` for terminal groups | Memo, per function (§6.3 rule 1) | What a function contributes to the stitch graph whatever is drawn: declared FnDef links, duplicated-pin and terminal-level groups, and its flow data (protective device, N switched with L, terminal contact, source outputs). Note-free |
| `rail_terminal(d)` | `rail_terminals`, the terminal's functions and CPs, `structure_of` | Memo, per device | The PE CPs and location of one rail-bonded terminal; its EW-TERM-005, EW-POT-008 and cycle notes are in its value (note-free) |
| `rail_bonds()` | `rail_terminal` of every rail-bonded terminal | Memo, incremental | Per location the PE CPs that the rail joins ([08](08-connections-terminals-cables.md) §3.7); keeps its value when no terminal value it read changed. Note-free. Small |
| `bond_paths()` | `bond_index`, `ip_pairing` | Memo (value part) | The interruption points on the `via` path of every bond and their names ([03](03-domain-model.md) §9.6). Note-free. Small, recomputed whole |
| `stitch_graph()` | local nets, the index families, `ip_pairing`, `ip_notes`, `function_links`, `rail_bonds`, `bond_paths` | Memo; the cold reference only | Nodes are local nets, about 10–15 % of CPs. The partition no longer reads it: it is the oracle's and the tests' whole-graph reference and pushes the pairing's notes when read. Reads through indexes, never whole tables (§6.3 rule 5) |
| `potential_partition()` | `page_local_nets`, the index families, `ip_pairing`, `function_links`, `rail_bonds`, `bond_paths` | Memo, incremental producer of the parts `Potential`, `Endpoint`, `Bonds`, `BondsOf` | **Topology only:** the components and their `PotentialId`s, stored per component in persistent maps with the incidence the maintenance needs. `PotentialId` = the minimum `PotentialDef` ID when the potential has one or more definitions, else the minimum CP ID, so it is stable under unrelated edits. Cold = the global algorithm (the oracle's reference); incremental = the components through the touched endpoints, recomputed from the per-page nets (an added connection unions), with the changed potentials and endpoints declared; cold fallback as in the status line. Cut-off by structural equality with a 128-bit fingerprint per component. Note-free |
| `potential_name(p)` | the potential's `Potential` part, `potential_attrs`, `ip_info` of its points, `bond_paths` (value part), `device_tag`, `function_cps`, `rep_anchor_cell`, `drawn_page_order` | Memo, per potential | Replaces the `potential_names()` of ADR-0002 amendment 4. The canonical name (first definition in ID order, else the first non-external source point in page order, else the first external one, else `<tag>:<designation>` of the first member CP), the aliases (the other IP names, natural order; points on a bond's via path are named on the bond), the IP names and the name of each signal (the first IP name inside it; a signal outside a potential has none). Pushes EW-POT-001 for the attribute conflicts and EW-POT-017 for a second definition name, naming the potential. A folio renumber or a move re-derives names, not topology |
| `potential_attrs(p)` | the potential's `Potential` part, its definitions, the member CPs' `potential_role` | Memo, per potential | Type, voltage, environment and class, each from the first definition in ID order that sets it, the type else from the member roles; every disagreeing value is a conflict kept in the value. Note-free, so a rename does not reach the wire properties or the flow |
| `physical_wires_of(potential)` | the potential's `Potential` and `BondsOf` parts, `Bonds`, connections, junction wiring, the `Ip` parts of its points | Memo | One entry per physical CP-to-CP wire, keyed by the UUID of its target-side connection. Source of the wire list, routing and `WireRecord`. The causes of its super-linear chain are fixed; it would go under the LRU (1.0) if its values stay bulky |
| `potential_flow(potential)` | `potential_flow_bfs` (the potential's `Flow` part), `flow_notes` (value part) | Cold | Flow rooted at the source-output CPs; the protective device per load (a load is a consuming function on the potential); source of the potential overview. A lookup into `potential_flow_bfs` (< 20 µs), Cold since G-SP |
| `potential_flow_bfs()` | `potential_partition`, `potential_attrs`, the pass-through pairs of `function_links` | Memo, producer of `Flow` parts | The one BFS over the partition behind every `potential_flow(potential)` ([08](08-connections-terminals-cables.md) §3.6). Keeps its value when neither the partition nor an attribute it read changed; otherwise the BFS runs whole (incremental flow is follow-up work). Note-free: the pass-through conflicts are values |
| `flow_notes()` | `potential_flow_bfs`, `potential_name` | Memo (value part) | Pushes the EW-POT-001 notes of the pass-through conflicts, naming the two potentials |
| `settings_member(name)` | the project row | Memo (value part; a cut-off point, not a cost saving) | One key per settings member read by a derivation (`ip_pairing`, `colour_profile`, `size_profile`, `wire_default`, `numbering`, `frames`, the pinned profile), so a colour-profile edit no longer invalidates the pairing; no new stamp in `ew-model` (ADR-0002 amendment 4) |
| `wire_profiles()` | the `colour_profile`, `size_profile` and pinned-profile settings members | Memo | The three provisional members parsed, with their notes; never the project row. Readers take one row through `profile_row_for` |
| `wire_profile_notes()` | `wire_profiles` | Memo (value part) | The profile notes, sorted and deduplicated, pushed by `wire_props_of` |
| `wire_default_props()` | the `wire_default` settings member | Memo (value part) | The project default of the wire-property chain (level 1) |
| `profile_row_for(key)` | `wire_profiles` | Memo (value part) | The circuit class, colour row and size row of one attribute set (`ProfileKey`: type, voltage, AC/DC and the potential's own class, ordered by a canonical tuple); a profile edit re-executes the few keys and cuts off every unaffected one, so only the potentials whose row changed re-derive |
| `end_class(scope)` | `function_cps`, the potentials of the scope's CPs (`Endpoint` parts), `profile_row_for` | Memo, per device or strip | The highest circuit class among the potentials that hold a CP of a device's functions or of a strip's terminals; the PE rule's read (FR-CON-038), once per scope instead of once per wire end. A PE wire still reads the classes of its own end function's CPs directly, so a row change of another potential on that terminal re-executes it once (follow-up work) |
| `end_rating(scope)` | `function_cps`, the potentials of the scope's CPs, `potential_flow`, the protecting devices' technical data | Memo, per device or strip | The highest rated current, with its device, among the protecting devices of the potentials that hold a CP of a device's functions or of a strip's terminals: the PE wire's read of the rating at its ends (FT-2055, doc 08 §4.2) |
| `wire_props_of(potential)` | overrides, net defs, potential defs, `profile_row_for`, `end_class`, `end_rating`, `potential_flow` and the protecting device's technical data, `wire_default_props`, `wire_profile_notes` (value parts) | Memo | Precedence: connection override (CDP) > net definition > potential definition > profile rule (type × voltage × circuit class) > project default. Every value carries **provenance** (D-08). It reads the profile only through `profile_row_for` and never the project row, so a profile edit re-derives the affected potentials only; NFR-PERF-07 at L is feasible only with push invalidation (§6.2) |
| `wire_props(connection)` | `wire_props_of` of the connection's potentials | Cold | The physical wires a connection belongs to with their effective properties, in wire ID order; empty for bridges, bus and overview connections |
| `channel_addresses(module)` | module start, part address map, channel specs | Memo | One edit re-addresses a module (D-26) |
| `terminal_rows(strip)`, `cable_rows(cable)`, `plc_rows(station)` | terminals, levels, bridges, conductors, channels | Memo | Sources of terminal, cable and PLC reports (REF-045/049/060) |
| `device_part_lines(dev)` → `bom_rows(mode)` | parts on devices, assembly children, carriers, accessories | Memo | Assembly and component modes (D-04, D-36) |
| `report_rows(block)` (`ew-reports`) | the kind's row query, manual selection, sort, grouping, the form's layout | Memo | The block's `RowSet` in final order ([09](09-reports-forms-output.md) §2.3); embedded blocks read it too. A TOC's rows read `page_order`, so it is never read from inside pagination |
| `report_block_pages(block, toc_pages)` (`ew-reports`) | `report_rows`, form layouts, the block's numbering | Memo | Generated pages with IDs `v5(block, index)`; `toc_pages` matters only for a TOC, other blocks are keyed 0; feeds `page_order` through the provider ([09](09-reports-forms-output.md) §3.2) |
| `toc_rows()`, `title_block(pageref)` (struct `TitleBlockValues`), `layout_label(placement)` (not built yet) | page order, project data, overrides, revisions, tags | Memo | Bound fields only; overrides explicit and flagged (D-05, D-06, D-02) |
| `checks(scope)` (page, potential, strip, station) | rule inputs | Memo | Online message panel (§12). As built: `check_findings(rule, scope)` with the scope kinds project, page, strip, device, cable and potential; severity, supersession and waivers apply on top |
| `page_display_list(page, layer, lang)` | per-layer inputs | Memo | Schematic layers: frame, symbols, wires, labels, tables, revision, highlights. Panel-page layers: surface, carriers, placements, features, dims, plus frame, labels, tables and highlights. A wire edit never rebuilds the title block. As built it is a pure function over a caller-filled `PageView` (rule 7). As built: one Memo family per layer in `ew-api` (`page_display_list_frame`, `_symbols`, `_wires`, `_labels`, `_tables`; the other layers are empty). The layers read memoised view parts (`page_frame`, `page_grid`, `page_bindings`, `page_reps`, `page_slots`, `page_cp_targets` over the project-level `cp_targets_by_page`, `page_table`, `page_embedded`) and two cold ones, `page_connections` and `page_junctions`, whose row reads are recorded in the reading layer; neutral language, no `lang` key yet. Layers build serially on the session's thread; `PageTableOf` and `PageBindingsOf` are Memo, `cp_targets_by_page` is project-level; the cold parts' row reads are recorded in the reading layer, so a layer stays fresh |
| `panel_front_reps(dev)` | placements of the device, surfaces | Cold | Derived panel-front representations for navigation and "show all representations" (§4.2) |
| `layout_scene(space)` | placements, carriers, mesh refs, committed routes | Memo | Input to the 3D reconciler (§8.5) |
| `sql_table(api_table)` | the table's sources | Class II | SQL projection (§10.2) |

### 6.5 Freshness contract

| Class | Examples | Contract |
|---|---|---|
| **I: synchronous** | tags, xrefs, nets, wire properties, title blocks, TOC, BOM, display lists, online checks | Always fresh when read |
| **II: asynchronous** | routing and wire lengths, full 3D collision, offline checks, PDF export, SQL projection, thumbnails | Computed on forks and tagged with `Rev`; a stale badge is shown ("810 mm, re-routing…"). **Release exports require fresh results** |
| **III: external** | STEP tessellation, edges, hulls | Content-addressed cache, never model data |

**Two mechanisms keep shop-floor values stable.**

**Committed computations** (from B). Some computed values must not change behind the shop floor's back:
- routed paths and lengths;
- frozen wire numbers;
- report snapshots of a released revision.

They are **authored outputs of commands** (`RouteWires`, `FreezeNumbers`, `ReleaseRevision`), stored as
`Committed<T> { value, input_hash, rev }`. A check flags them when the input hash no longer matches: `EW-LAY-011`
for routes, `EW-MFG-001` for lengths, `EW-THM-005` for thermal results and `EW-REL-002` for any other committed value
a release output uses. "Re-route" is an explicit, previewed command.

**Frozen on purpose.** `TagSpec.frozen`, `NumberSpec::Frozen` and `AddrSpec::Manual` hold deliberate overrides. The
numbering report lists every frozen value and its deviation from the rule.

### 6.6 Versioned derivation profiles

Derived output depends on rules: xref format, numbering, title-block bindings, wire-property inheritance, report
layouts. These rules are therefore **versioned data**, and each project pins one (`derivation_profile = "2027.1"`).
- A new EW release ships new profiles but never silently changes the profile of an existing project. Upgrading a
  project's profile is an opt-in, previewed command.
- The shipped profiles are TOML data in `ew-derive` (`crates/ew-derive/profiles/2027.1.toml`), parsed once; they
  are part of the build, not library items. A project's own settings win over the profile's defaults (ADR-0003
  amendment 2).
- A pinned profile the build does not ship is kept, reported (EW-CORE-018) and used through the newest shipped
  profile for display only; profile-dependent commands are refused until the profile upgrade resolves it (ADR-0003
  amendment 1).
- Golden outputs are tested per shipped profile.
- Released revisions archive their PDF (§7.8).

This is how a 2027 project opens in 2037 with identical tags, cross-references and wire numbers, which the
Machinery Regulation's retention period demands (G2).

### 6.7 XL-A cost estimates

The estimate case XL-A is 2,000 pages, 50k devices, about 150k functions, 200k connections, about 500k instantiated
CPs, 20k terminals and 10k channels, on the G6 baseline machine. XL itself always means the class bound of
[non-functional requirements](02c-nonfunctional-requirements.md) §2.1 (5,000 pages, 250k connections).

| Operation | Work | Estimate |
|---|---|---|
| Open | Parallel parse of about 250 MB of JSONL; tables and base indexes | 1–3 s; the first page is interactive about 1 s later |
| Warm-up | Fork job; adopted if the user has not edited yet | 2–4 s in the background |
| Move a symbol within its cell / across cells | 1 op → page-local nets (cut off) → layers / plus partner xrefs | < 1 ms / < 2 ms, plus about 1 ms per re-tessellated layer |
| Draw one wire | page-local nets; global stitch graph; potential partition | 5–10 ms at commit. Live highlighting during the drag uses page-local nets only |
| Renumber folio 103 → 104 | page order, about 25 tags, xrefs into 103, TOC, BOM folio column, Prev/Next | < 5 ms plus the preview diff |
| Change the colour of "24 V DC" in the profile | wire properties of all 24 V potentials (about 60k wires) | ~30 ms |
| Full BOM / full online check set (cold) | aggregation / per scope in parallel on a fork | ~20 ms / 1–3 s |
| SQL projection, full build | about 2M rows into SQLite | 2–5 s job; then per-table refresh |
| Memory | model and indexes ~250 MB; memos ≤ 300 MB (target; not measured); display-list LRU ~100 MB (1.0 target; not built); undo ≤ 256 MB; projection ≤ 500 MB, file-backed in the local cache at XL size | ~1–1.5 GB peak, within the 4 GB budget |

The one honest hotspot is **global connectivity**. It costs 5–10 ms per committed wire edit, which is acceptable
per commit but not per drag frame. The design accounts for this.

**Measured at G-SP** (2026-10-20, [spike report](../research/spike-g-sp-2026-10.md)). Cold, the global part is far
above this estimate: a wire commit with the global potential update takes 124 ms (M), 656 ms (L) and 6.17 s (XL) at
p99, and the 24 V profile change over every potential 545 ms (M) and 10.9 s (L). The peak working set of the
benchmark process at XL, 5.78 GB, is over the 4 GB of the memory row although the two heaviest families were left
out (an upper bound with the generator's records included). The estimates above are therefore the targets of
the incremental connectivity and push invalidation of §6.1 and §6.2 (predicted: a few milliseconds per commit at L),
not cold figures; the re-test after the recycle replaces them with measurements.

### 6.8 Debuggability

- **`ew query <name> <key> --json`** prints any derived value. `--explain` adds its dependency trail, the memo state
  (hit, verified or re-executed) and the provenance of each field.
- **`ew verify-derived`** (also in the GUI under Help) recomputes everything cold and reports differences. On a fresh open it checks determinism only (memo and cold computed on one snapshot agree); staleness across edits needs a long-lived session.
- **Oracle mode.** In CI, debug builds and nightly runs, after each transaction sampled queries are recomputed cold
  and compared (all of them nightly).
- **Projection dump.** `ew sql dump <project> out.sqlite` writes the projection for inspection in any SQLite browser
  (§10.2).
