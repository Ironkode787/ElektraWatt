# ADR-0030: Legal steward, CRA role, support period and signing identity

**Status:** Proposed · **Date:** 2026-10-17 · **Deciders:** project owner, with legal advice

## Context

- **CRA roles** (Regulation (EU) 2024/2847; G11, verified through the ORC WG CRA FAQ unless marked):
  - **Outside the manufacturer obligations:** free and open-source software that is not monetised.
  - **Steward** (Art. 3(14), Art. 24): a legal person that systematically supports FOSS intended for commercial use.
    It has light-touch duties: a cybersecurity policy, vulnerability handling, cooperation with market-surveillance
    authorities, and notification of actively exploited vulnerabilities.
  - **Both at once:** one organisation can be a manufacturer for a commercial edition and a steward for the
    community edition.
  - **Manufacturer:** selling support beyond actual costs, or paid LTS builds, under the ElektraWatt name makes the
    seller a manufacturer of that product from **2027-12-11**. The obligations are the Annex I requirements, an
    SBOM, vulnerability handling, a support period normally of at least five years, technical documentation, an EU
    declaration of conformity and CE marking (self-assessment for the default category; details **verify**).
  - **Dates:** reporting of actively exploited vulnerabilities and severe incidents (Art. 14) has applied since
    2026-09-11, and all other obligations apply from 2027-12-11. With MVP in month 6–7 and 1.0 in months 15–18,
    **every 1.0 release ships under full application**.
- **Product Liability Directive (EU) 2024/2853** (**unverified**). Software is a product, for products placed on
  the market after 2026-12-09. FOSS supplied outside a commercial activity is excluded. EW's PE-sizing,
  N-switching and load checks are safety-relevant engineering aids.
- **Signing identity.**
  - Artifact Signing issues public-trust Authenticode certificates to organisations in the EU, but not to individual
    EU developers (gaps §3.3).
  - Apple's Developer Program enrolment as an organisation requires a D-U-N-S number for the legal entity.
  - Without an organisation, no Windows or macOS release can be signed (ADR-0023). The organisation's name appears on
    every signed binary and as publisher in the About dialog.
- **Backport cost.** egui, wgpu and winit break every 6–10 weeks and are upgraded together once a quarter (ADR-0014,
  NFR-MAINT-04). Long backport windows on old UI stacks are therefore expensive for a team of 2–4 developers.
- **Neutrality.** The pilot panel builder employs the main contributors. The trademark, the signing role and any
  donations need a neutral home in the long run.

## Decision

**Pending.** Until it is decided:
- no tagged or signed binary release is published, only nightlies labelled "unsigned development build";
- nothing is sold under the ElektraWatt name (NFR-COMP-02);
- `SECURITY.md` with private reporting is set up anyway.

## Recommendation

1. **Steward and identity holder, in two phases** ([19](../design/19-open-source-governance.md) §3.10):
   - **Phase 1 (M0 to 1.0).** The pilot panel builder's company is the interim **open-source steward** of the
     community edition. It holds the signing identity (Artifact Signing, Apple Developer ID) and the trademark, on two
     conditions:
     - it sells no EW services or builds;
     - it signs a commitment to transfer the trademark, the signing role and any donated funds to a stichting.
   - **Phase 2.** A Dutch **stichting** (foundation, notarial deed, no members) becomes steward. Its board of three
     has one member from the pilot panel builder, one maintainer not employed by it, and one independent member from
     the user community. The board decides money, trademark, legal matters and steward duties; maintainers decide
     technical matters.
   - **Trigger.** Phase 2 starts at governance gate LG-6: more than 3 regular external contributors, more than €50k
     income a year, or 1.0, whichever comes first.
   - **Anonymity.** If the pilot panel builder does not want to appear publicly as publisher, found the stichting
     before the first signed release instead.
2. **No paid offering yet.** Nothing paid (support, LTS builds, hosted services) is sold under the ElektraWatt name
   until the manufacturer obligations are budgeted and in place (NFR-COMP-02).
3. **Support policy** (NFR-COMP-03):
   - The latest minor receives all fixes.
   - When a minor's successor ships, that minor receives security and data-loss fixes for 6 more months. With
     quarterly minors, up to two older lines can be in that window at once.
   - After N.0, the last minor of N−1 gets the same 6-month window. Before 1.0, only the latest 0.x is supported.
   - **No LTS line** exists until a commercial decision funds the five-year support period it would imply.
   - Each build embeds its planned support end, which the About dialog shows without network access.
4. **Vulnerability handling.**
   - `SECURITY.md` offers private reporting through GitHub security advisories, acknowledgement within 7 days,
     coordinated disclosure, CVE requests and a supported-versions table (NFR-SEC-14).
   - A runbook covers notification through the single reporting platform (Art. 14, Art. 16): for an actively
     exploited vulnerability an early warning within 24 h, a notification within 72 h and a final report within
     14 days after a fix or mitigation is available; for a severe incident 24 h, 72 h and a final report within one
     month. It operates from the first public release whether or not counsel confirms that the steward's Art. 24
     duties include Art. 14 reporting.
   - A tabletop exercise is held before 1.0.
5. **Liability statement.** The licence texts, documentation, About dialog, check panel, release dialog and every
   release report state that EW's checks do not replace engineering verification (NFR-COMP-04).
6. **Evidence.** The steward's CRA evidence is the ADR-0023 control set: SBOMs, signed and attested artefacts,
   advisories policy, threat model.

## Consequences

If the recommendation is accepted:

**Positive**
- **Timely signing.** A signing identity is available in time for the MVP, without waiting for a foundation to be
  set up.
- **Steward duties are met** with controls that already exist (ADR-0023). The CRA position is documented before the
  first public release.
- **Known cost of going commercial.** Commercial options stay open with a known cost: becoming a manufacturer for a
  commercial edition while remaining steward of the community edition.
- **Affordable backports.** The short support window matches the quarterly UI-stack upgrade train, so backports
  stay affordable.

**Negative**
- **Single-employer phase.** Phase 1 concentrates trademark, identity and steward role in one employer, which may
  deter other companies until the stichting exists.
- **Visibility.** The phase-1 steward's name appears publicly on every binary.
- **Stichting cost.** Founding the stichting costs notary and bookkeeping fees and board effort (**verify** the
  amounts).
- **No LTS yet.** Enterprises that want multi-year LTS cannot get it until a commercial decision is made.
- **Counsel needed** on the steward's reporting start date, the PLD exposure of a commercial edition, and the
  transfer commitment.

## Alternatives considered

| Alternative | Assessment |
|---|---|
| **An individual maintainer as holder** | Cannot be a CRA steward, which must be a legal person. Cannot obtain Artifact Signing as an individual in the EU. Rejected |
| **An immediate commercial LTS** | Manufacturer duties from day one, including a support period of normally at least five years on a UI stack that breaks quarterly. Unaffordable for 2–4 developers before 1.0 |
| **An umbrella foundation** (Linux Foundation, Software Freedom Conservancy, Eclipse) | Neutral and experienced, but slower to join (months). Eclipse's contributor agreement would conflict with the DCO-only rule of ADR-0024 (unverified). Worth re-evaluating at LG-6 |
| **A fiscal host only** (Open Collective Europe) | Fine for donations alongside a steward, but it offers no signing identity, and whether a host acts as steward is unclear |
| **A vereniging** (association with members) | A valid steward, but the general-assembly overhead does not suit a 2–4 person team |
| **A stichting from day one** | The cleanest neutrality, but its set-up delay would likely push the first signed release past the MVP. Recommended instead if publisher anonymity matters |

**Decision needed from:** the project owner, with legal advice (PQ-31). Recommended by M1
([18](../design/18-quality-security-release.md) OQ4), and in any case before the first signed release (governance
gate LG-4) and before the first public release (CRA role).

## Related

- **ADRs:** [ADR-0014](0014-gui-toolkit-egui.md) (upgrade cadence), [ADR-0023](0023-supply-chain-and-release-integrity.md) (signing, SBOM, provenance),
  [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md) (DCO, no CLA),
  [ADR-0029](0029-network-telemetry-crash-reporting-policy.md) (who operates endpoints).
- **Design:** [19 open-source governance](../design/19-open-source-governance.md) §3.3, §3.10, OQ7;
  [18 quality, security and release](../design/18-quality-security-release.md) §7.5, §9.1, §11.3, §13;
  [00 vision and scope](../design/00-vision-and-scope.md) OQ9; [gaps-and-corrections](../research/gaps-and-corrections.md)
  G11, §3.3.
- **Requirements:** NFR-COMP-01, NFR-COMP-02, NFR-COMP-03, NFR-COMP-04; NFR-SEC-12, NFR-SEC-14; NFR-MAINT-04.
