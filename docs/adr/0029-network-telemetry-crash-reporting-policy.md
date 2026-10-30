# ADR-0029: Network, telemetry and crash-reporting policy

**Status:** Proposed · amended 2026-10-30 (macOS not supported, decision log ID-880) · **Date:** 2026-10-17 · **Deciders:** project owner

## Context

- **Projects are confidential.** Engineering projects are confidential customer work. Every title block of the
  reference drawing set carries a confidentiality notice. Projects also contain personal data: the engineer's name,
  checker fields, and the customer's and end user's addresses on the cover page (G12).
- **Legal frame** (G11, G12, partly **unverified**):
  - The GDPR applies to install identifiers and IP addresses.
  - ePrivacy Art. 5(3) requires consent for reading or storing information on terminal equipment unless strictly
    necessary. EDPB Guidelines 2/2023 read this to cover application telemetry.
  - Under the CRA, making personal-data processing a condition of use is a monetisation indicator. That would pull a
    non-monetised open-source project towards manufacturer obligations (ADR-0030).
- **Community precedent.**
  - Audacity's 2021 telemetry pull request drew about 3,300 negative against 250 positive reactions and was closed
    within a week, even after maintainers promised it would be optional and off by default.
  - KiCad compiles Sentry support optionally, with an explicit opt-in, a first-run prompt and a resettable anonymous
    ID.
- **Deployment reality.** Panel builders deploy through MSI with policy keys, work behind proxies with TLS
  inspection, and sometimes run air-gapped shop-floor machines.
- **What the architecture already fixes.** No network traffic without a user action, no telemetry, no font, symbol
  or part downloads (ARCH §16), and no listening socket unless the user starts `ew mcp` (NFR-SEC-09). Still open:
  - the update-check default and its exact behaviour;
  - whether a crash-report endpoint exists, and who runs it;
  - the written policy that binds all of it.

## Decision

**Pending.** Until this ADR is accepted, every build follows the binding parts above:
- development builds contain no update check and no crash upload;
- crash reports are local only;
- the network-sandbox CI job already enforces this.

## Recommendation

**"Offline-first, zero by default":**

1. **No network traffic without an explicit user action.** That includes DNS lookups and WPAD proxy discovery
   (NFR-PRIV-01). Start-up opens no connection (NFR-PERF-02). The complete list of permitted flows is the `PRIVACY.md`
   table:

   | Flow | Trigger | Content | Default |
   |---|---|---|---|
   | Update check | opt-in; at most once a day | `GET` of a signed static manifest; only `User-Agent: ElektraWatt/<version> (<os>; <arch>)` | off; off in MSI deployments |
   | Crash upload | per report, after previewing the payload | scrubbed stack frames and allowlisted metadata | off; no button while no endpoint exists |
   | Help link | click | URL opened in the system browser | offline docs bundled |
   | Plugin network (1.x) | capability granted at install | plugin-defined | denied |
   | `ew mcp` | user starts it | stdio, or loopback with a token | off |

2. **Update check.**
   - It is opt-in at first run, and off by default in MSI installs (policy key). A policy key can point it at an
     internal mirror.
   - The manifest signature is verified with the embedded release key before anything is shown.
   - At MVP it shows a notice only. From 1.0 a click downloads the installer, which is verified (manifest SHA-256 and
     OS signature) and never installed silently (NFR-SEC-15).
3. **Crash reports.**
   - **Local by default.** A local minidump plus a log tail is written to the per-user state directory with
     user-only permissions. The user can open, inspect and copy it.
   - **Opt-in upload.** Upload is opt-in per report, to a project-run endpoint (self-hosted GlitchTip or Sentry;
     the Sentry server licence is unverified).
   - **What is uploaded.** The payload is a client-side stack walk (module, build ID, offset), allowlisted metadata
     and the panic message, scrubbed of paths, user and host names, and every identifier and text of open projects.
     **The raw minidump is never uploaded**, because its stack memory can hold project text. The user sees the exact
     JSON before sending.
4. **No usage analytics in v1.** That means no install or user identifier and no feature counters (NFR-PRIV-02).
   Analytics could only ever be added by a new ADR: opt-in, aggregated, with a documented schema and a public
   dashboard.
5. **Data minimisation elsewhere.**
   - Exports offer to omit person fields from page text, reports and XMP (`--omit-personal-data`; NFR-PRIV-06,
     FR-OUT-025).
   - No project data goes to any AI or translation service. Agents connect through MCP by the user's choice
     (NFR-PRIV-08).
6. **Documented and tested.**
   - `PRIVACY.md` lists every flow and is shown in the About dialog, and any change to it requires an ADR
     (NFR-PRIV-07).
   - A CI job runs the GUI smoke flows and the CLI in a network namespace with only loopback up, under `strace`. It
     fails on any `AF_INET`/`AF_INET6` socket or DNS attempt outside the allowlisted `ew mcp --listen 127.0.0.1:0`
     test (loopback only; any non-loopback address is refused).
   - A second run with the update check opted in asserts the exact request against a mock server.
   - Windows is observed manually for each minor (macOS is not a supported platform, decision log ID-880).

**The owner decides:**
- whether "zero by default" is acceptable, given that it limits product analytics;
- whether a crash endpoint will exist, and if so, who operates it and holds the GDPR record of processing.

The recommended answer: local-only at MVP. At 1.0, the steward runs a self-hosted GlitchTip, but only if a named
maintainer owns its operation and retention. Otherwise the upload button stays absent.

## Consequences

If the recommendation is accepted:

**Positive**
- **Trust.** Panel builders and their customers get strong trust: no data leaves a workstation unless a person sends
  it, and that is provable through `PRIVACY.md` and the CI sandbox.
- **CRA position.** The CRA position is clean, because no personal-data processing is a condition of use.
- **GDPR exposure is minimal**, and only for the optional, consented upload.
- **Offline installations work unchanged.** Air-gapped and proxy-restricted installations need no configuration.

**Negative**
- **Little field data.** There is no product analytics. Prioritisation relies on the usability tests (NFR-UX-01,
  NFR-UX-02), pilot feedback and the issue tracker.
- **Field crashes surface late.** Many users never opt in, so field crashes surface more slowly.
- **Update notices.** Users who decline the update check learn of security releases only through release notes and
  announcements, which matters for the support policy (ADR-0030).
- **Endpoint duties.** Operating an endpoint, even a self-hosted one, brings GDPR duties: a record of processing,
  retention rules, access control.

## Alternatives considered

| Alternative | Assessment |
|---|---|
| **Opt-out telemetry** | Rejected on legal grounds (consent under ePrivacy and GDPR), on community grounds (the Audacity precedent) and on CRA grounds (a monetisation indicator) |
| **No crash reporting at all** | Loses the only field signal for native crashes. Local reports cost nothing and help support even without upload |
| **A third-party SaaS crash service** | Adds an external data processor and a data-processing agreement, and the confidential context depends on perfect scrubbing in someone else's pipeline |
| **KiCad-style opt-in with an anonymous install ID** | An ID adds personal-data processing that crash triage does not need. Build ID and stack frames are enough |
| **Update check on by default** | Every start would contact a server, which violates NFR-PRIV-01 and NFR-PERF-02 and surprises offline and enterprise users |

**Decision needed from:** the project owner (PQ-33).
- The policy must be accepted before the first binary release (MVP), because the first-run prompt and `PRIVACY.md`
  ship with it.
- The crash-endpoint question is due by 1.0.

## Related

- **ADRs:** [ADR-0020](0020-extensibility-tiers-and-security-model.md) (MCP off by default),
  [ADR-0023](0023-supply-chain-and-release-integrity.md) (network-sandbox job, signed manifests),
  [ADR-0030](0030-legal-steward-cra-role-support-signing.md) (CRA role, who operates endpoints).
- **Design:** [04 architecture](../design/04-architecture.md) §9, §13, §16;
  [18 quality, security and release](../design/18-quality-security-release.md) §9.3, §12, OQ5;
  [16 UX/UI](../design/16-ux-ui.md) (first run); [gaps-and-corrections](../research/gaps-and-corrections.md) G11,
  G12.
- **Requirements:** NFR-PRIV-01 to NFR-PRIV-08; NFR-SEC-09, NFR-SEC-15; NFR-PERF-02; NFR-REL-13; NFR-PLAT-06;
  FR-OUT-025, FR-EXT-026.
