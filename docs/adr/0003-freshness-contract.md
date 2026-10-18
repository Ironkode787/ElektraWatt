# ADR-0003: Freshness contract: classes I–III, frozen-on-purpose identifiers, committed computations, versioned derivation profiles

**Status:** Accepted · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

ADR-0001 says every printed value is derived. Taken literally, that conflicts with four realities of panel
manufacturing:

1. **Identifiers leave the building.** Device tags are printed on labels, wire numbers on ferrules, and PLC
   addresses go into the TIA Portal or Codesys project. A rule change or an upstream edit must not silently change
   -102F1, a wire number or Q0.1 after they have been used.
2. **Routed lengths feed machines.** A wire length cut on a Komax must not change behind the shop floor's back
   because someone moved a duct.
3. **Drawings must look identical years later.** Panel builders' customers expect a 2027 project to open in 2037.
   The Machinery Regulation (EU) 2023/1230 retention of technical documentation for at least 10 years is the driver
   (**unverified**, [gaps and corrections](../research/gaps-and-corrections.md) G2). A new EW release that changes
   the xref format or a numbering rule would otherwise rewrite archived drawings.
4. **Some derivations are slow.** Routing a 2,000-wire cabinet takes up to 5 s (NFR-PERF-16), a 20 MB STEP import
   up to 30 s (NFR-PERF-18), and a full SQL projection several seconds at XL. They cannot run synchronously.

## Decision

### Freshness classes

| Class | Examples | Contract |
|---|---|---|
| **I: synchronous** | Tags, xrefs, nets, potentials, wire properties, title blocks, TOC, BOM, display lists, online checks | Always fresh when read |
| **II: asynchronous** | Routing and wire lengths, full 3D collision, offline checks, PDF export, SQL projection, thumbnails | Computed on forks and tagged with their `Rev`; the UI shows a stale badge ("810 mm, re-routing…", "as of revision n"). **Release exports require fresh results** |
| **III: external** | STEP tessellation, feature edges, hulls | Content-addressed cache keyed by `sha256(file) + worker_version`; never model data |

### Frozen on purpose

Deliberate overrides are typed fields, not edits of derived values:
- `TagSpec.frozen: Option<FrozenTag>` for device tags;
- `NumberSpec::Frozen` for wire numbers;
- `AddrSpec::Manual` for PLC module starts and channel addresses.

`FreezeNumbers` and the PLC freeze command (FR-NUM-011, FR-PLC-015) freeze a scope in one previewed command,
typically at release. The numbering report lists every frozen value with its rule-derived value (FR-REP-022), and a
check reports each deviation at severity info (FR-CHK-037).

### Committed computations

Values that must stay stable but come from an expensive computation are **authored outputs of explicit commands**:

```rust
pub struct Committed<T> { value: T, input_hash: InputHash, rev: Rev }   // hash of every input the command read
```

- Producers: `RouteWires` (paths and lengths), `FreezeNumbers` (frozen wire numbers) and `ReleaseRevision`
  (released report snapshots).
- When the hash of the current inputs no longer equals `input_hash`, a check flags the value: **EW-L011** for
  routes, EW-MFG-01 for lengths, EW-THM-05 for thermal results, and **EW-REL-002** for any other committed value
  that a release output uses, such as a frozen report snapshot
  ([checks, revisions and collaboration](../design/15-checks-revisions-collaboration.md) §4.8). "Re-route" is an
  explicit, previewed command (FR-LAY-046).
- The release gate blocks on stale Class II results and stale committed computations (FR-REV-007), and release-mode
  manufacturing exports refuse them (FR-MFG-025).

### Versioned derivation profiles

Rules that affect output are **versioned data**, and each project pins one: `derivation_profile = "2027.1"` in
`project.jsonl`. A profile covers the xref format, numbering rules, title-block bindings, wire-property inheritance,
report layouts and the identifier set (ADR-0026).
- A new EW release may ship new profiles but never changes the profile of an existing project.
- Upgrading is an opt-in, previewed command that shows the tag, xref and wire-number diff (FR-PRJ-011).
- Golden outputs are tested per shipped profile.
- Released revisions record their profile and archive their PDF (ADR-0013).

## Consequences

### Positive

- A 2027 project opens in 2037 with identical tags, cross-references and wire numbers (NFR-REL-09).
- The shop floor sees explicit stale states instead of silently changing values.
- The golden project reproduces with S51, F55, T55, E55, S56, E56 and 1X1 as frozen tags listed in the numbering
  report, while every other tag stays derived ([architecture](../design/04-architecture.md) §14).
- Slow work never blocks editing, and its results never pose as current.

### Negative

- Every shipped profile is kept forever, and its derivation code paths with it. The golden test matrix grows with
  each profile.
- An output-changing bug fix ships as a new profile version. Projects pinned to the old profile keep the old output
  until they upgrade, which needs release notes (NFR-DOC-07).
- Three freshness classes add UI states (stale badges, "as of revision n") that users must learn (FR-UI-024).
- Frozen values can drift from their rule; the deviation report is the only safeguard.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Always recompute everything** | Changes archived drawings and manufactured identifiers after an upgrade |
| **Store derived values** | The D-01 class |
| **Hidden background re-routing** | Wire lengths would change behind manufacturing's back |
| **Rely on the archived PDF alone** | The PDF stays correct, but the first edit after reopening would re-derive every tag and xref with today's rules |
| **Freeze everything implicitly at each save** | Turns the derived model back into a stored one between saves |

## Open points

1. **Profile end-of-life.** Keeping every profile forever is cheap only while profiles differ in data, not code.
   *Recommendation:* express profiles as data wherever possible, and require an ADR before any profile needs its own
   code path.

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0002](0002-derived-data-engine-ew-memo.md),
  [ADR-0013](0013-revisions-and-as-built-archive.md), [ADR-0016](0016-3d-renderer-wgpu-and-out-of-process-step-worker.md), [ADR-0018](0018-reports-forms-engine-and-machine-profiles.md), [ADR-0019](0019-rule-engine-and-rule-packs.md), [ADR-0026](0026-default-device-tag-identifier-set.md).
- **Design:** [architecture](../design/04-architecture.md) §6.5, §6.6, §8.5;
  [checks, revisions and collaboration](../design/15-checks-revisions-collaboration.md) §5;
  [panel layout](../design/12-panel-layout.md); [manufacturing outputs](../design/13-manufacturing-outputs.md).
- **Requirements:** FR-PRJ-011, FR-NUM-010, FR-NUM-011, FR-NUM-022, FR-PLC-013, FR-PLC-015, FR-LAY-046, FR-REV-006,
  FR-REV-007, FR-REV-010, FR-REV-011, FR-MFG-025, FR-REP-022, FR-CHK-037, FR-UI-024, NFR-REL-09, NFR-DOC-07.
