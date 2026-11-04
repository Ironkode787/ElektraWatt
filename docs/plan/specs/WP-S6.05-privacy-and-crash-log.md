# WP-S6.05: Privacy controls and the local crash log (closed specification)

**Status:** complete in cycle 50, built by one lane and its fix round (decision log ID-1338, ID-1339); PV-2 and PV-8 wait for the deep nightly's first `network-sandbox` run, and PV-13 is WP-S6.08's manual check (ID-1319); specified in cycle 50 (documents only), an M2 package at the size of ID-1187's re-estimate (PERT 0.6 pw,
which stays), built whole by one lane (ID-1281) in the six steps of §6 · **Date:** 2026-11-04 · **Work package:**
[WP-S6.05](../02-work-packages.md#wp-s605-privacy-controls-and-the-local-crash-log) · **Decision log:** ID-1187,
ID-1203, ID-1247, ID-1311 · **Part of:** ElektraWatt plan, specifications

**Purpose.** 0.1.0 holds no network code (ADR-0029 as amended on 2026-11-02, ID-1187), but nothing proves it: the
network-sandbox job CI-N-06 is not built, and `PRIVACY.md` says so. A panic today leaves no trace: no panic hook is
installed, no log is written, and the journal of a dirty project is found only when that project is next opened.
This document closes ADR-0029's interim rule for 0.1.0, CI-N-06, NFR-REL-13's 0.1.0 part (the panic's message and
backtrace in a local log, the journal kept, the recovery offer at the next start), the release-binary audit for debug
endpoints and the anonymisation guard to decisions, a build order and acceptance, against the code at `c97e3a15`
(§2.1). What ID-1187 and ID-1203 moved to WP-S6.10 stays there. Its size is an estimate, not a commitment (ID-1210);
every time is not measured (owner's rule). Where this document and a design document differ, this document is the
narrower and wins for the lane; the design documents stay the target and are updated at the design pass (§13). A
choice the lane meets that this document does not settle is escalated, never guessed.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [The texts](#5-the-texts)
6. [Build order](#6-build-order)
7. [Acceptance](#7-acceptance)
8. [Neighbours, files owned and what a lane may not decide](#8-neighbours-files-owned-and-what-a-lane-may-not-decide)
9. [Risks](#9-risks)
10. [Size and routing](#10-size-and-routing)
11. [Follow-ups, proposed](#11-follow-ups-proposed)
12. [Open questions](#12-open-questions)
13. [Documents to update](#13-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package every night a Linux job runs the release builds of `elektrawatt` and `ew` through
the smoke, headless, kittest and command-line flows in a network namespace with only loopback up, under `strace`, and
fails on any network socket, DNS attempt or listening port, and on a debug-endpoint string in a release binary; the
shipped binaries' graph holds no network-client crate; a panic of `elektrawatt` writes a local, user-only crash log
with the panic's message, location and backtrace, keeps the journal, and the next start offers to recover the
project; the state and configuration directories are user-only on Linux; the About dialog and the first-run step open
the bundled `PRIVACY.md`, which states all of this truly; and the anonymisation guard requires its list.

**Non-goals** (each with its package):
- No update check, no signed manifest, no policy keys, no crash upload, no endpoint, no payload preview, no scrubber,
  no minidump, no `report.json`, no crash monitor, no daily log with `Sensitive<T>`, no Help ▸ Diagnostics, no Copy
  summary (WP-S6.10, from 1.0; NFR-PRIV-03, NFR-PRIV-04, NFR-SEC-15, NFR-PLAT-08; §12 O2).
- No abort strategy for panics on threads other than the main thread (doc 18 §12.3's 1.0 form, FP-1).
- No crash log for the `ew` CLI (FP-2); no GPU device-loss recovery (FT-310, WP-S2.01 part 2).
- No `--omit-personal-data` (NFR-PRIV-06, FT-255: WP-S3.04 by WP-S1.22's specification); no usability-test consent
  (NFR-PRIV-10, WP-S2.12).
- No change to the network flows of `PRIVACY.md` (any change needs an ADR, NFR-PRIV-07); no change to ADR-0029.
- No `cargo-auditable` data check and no SBOM (WP-S6.07 D12, D9); no installer (WP-S6.06).
- No new Rust dependency; no change to `deny.toml`, `supply-chain/` or `Cargo.lock`.

## 2. Inputs and the code as it is

| Input | What the lane takes from it |
|---|---|
| ADR-0029 (Proposed; the Decision as amended on 2026-11-02) | The interim rule 0.1.0 follows (D2) |
| [Quality](../../design/18-quality-security-release.md) §12.1 to §12.3, the CI-N-06 row, TM-10 | The flows, the sandbox, the crash strategy, the debug-endpoint threat |
| [Project format](../../design/05-project-format-and-persistence.md) §8.1 to §8.3 | The journal, its durability and lifecycle, the recovery and its start-up half |
| [Non-functional requirements](../../design/02c-nonfunctional-requirements.md) NFR-PRIV-01 to -10, NFR-REL-13, NFR-SEC-09 | §3 |
| [Extended requirements](../../design/02b-requirements-extended.md) FR-UI-050, FR-UI-038 | The dialog's 0.1.0 part, the About dialog |
| [Golden project](../../design/20-golden-project.md) §3.5, AC-39 | The guard |
| [Gates](../03-gates.md) G-M1-07, G-M2-09, G-M2-15 | §3 |
| Decisions ID-1187, ID-1188, ID-1203, ID-1229, ID-1247 | The 0.1.0 scope, NFR-PRIV-03 and -04 to WP-S6.10, FT-495's rescope |
| Follow-ups FT-255, FT-342 (closed), FT-386, FT-495 | D16, D11 |
| Specifications [WP-S6.06](WP-S6.06-pilot-installers-and-platform-matrix.md) D3, D5 and §8; [WP-S6.07](WP-S6.07-release-pipeline.md) D6, D12 | The installed tree with `PRIVACY.md` and the `.pdb` files; the release's need for a green deep nightly; the release-artefact audit |

### 2.1 What exists at `c97e3a15` (the base contract)

The lane relies on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used for |
|---|---|---|
| No `set_hook`, no log subscriber, no `catch_unwind` in `elektrawatt`; "the crash handler come[s] with later work packages" | `bin/elektrawatt/src/main.rs:4-5`, `:645` | D6, D7 |
| At exit the shell shuts down: the lock released, the journal kept while dirty, removed only after a clean close | `bin/elektrawatt/src/main.rs:500-503` | D8 |
| Every journal entry is appended and synced at once; leftovers older than 30 days are deleted only when their project is opened | `crates/ew-store/src/journal.rs:16-18`, `:44`, `:589-611` | D8, D10 |
| The state directory (`%LOCALAPPDATA%\ElektraWatt\state`, `$XDG_STATE_HOME/elektrawatt`), under `EW_CONFIG_DIR/state` when that is set | `crates/ew-store/src/store.rs:133-147`; `crates/ew-viewmodel/src/shell.rs:160-176` | D10 |
| Only `settings.toml` is written user-only (0600, Unix); no directory gets a mode | `crates/ew-viewmodel/src/settings.rs:747-763` | D10 |
| Panics caught in place: job threads, rule runs (EW-CORE-083), printing, image decoding, CPU rendering, tessellation, the merge driver | `crates/ew-jobs/src/lib.rs:301`; `crates/ew-checks/src/rules/system.rs:241`, `:250`; `crates/ew-print/src/lib.rs:261`; `crates/ew-render-cpu/src/image.rs:45`, `render.rs:939`; `crates/ew-render2d/src/gpu/tess.rs:206`, `:242`; `bin/ew/src/cli/merge_driver.rs:66` | D7 |
| `tracing` events in `ew-memo`, `ew-render2d` and `ew-ui`, with no subscriber anywhere | `crates/ew-memo/src/stats.rs:191`; `crates/ew-render2d/src/gpu/render.rs:303`, `:310`; `crates/ew-ui/src/app.rs:735` | D13 |
| The journal recovery dialog at open (WP-S2.09 part 1) and its headless answers `journal.recover`, `journal.keep`, `file.discard_journal` | `crates/ew-viewmodel/locales/en-GB/shell.ftl:1195-1210`; `crates/ew-viewmodel/src/actions.rs:1252-1258` | D11 |
| The About dialog's privacy line ("This version has no update check and sends no crash reports.", FT-1117) and its and the first-run step's privacy button, which open the repository's web copy | `crates/ew-viewmodel/src/about.rs:26-29`, `:47`; `crates/ew-ui/src/app.rs:1754`; `crates/ew-ui/src/first_run.rs:67` | D16 |
| `Resources` holds the sample, the content, the guide and the notices, found beside the executable by `locate_resources` | `crates/ew-viewmodel/src/shell.rs:132-142`; `bin/elektrawatt/src/main.rs:406` | D16 |
| `PRIVACY.md`: "In short", the stored-data table (no crash-log row), the flows table (unchanged here), "The network sandbox" (being built; "Windows and Linux are observed by hand") | `PRIVACY.md:12-63` | D15 |
| `check-features` fails on a banned crate outside dev edges in the shipped binaries' graph; `banned_crates = ["egui_mcp", "ew-testids"]`; no HTTP, TLS or DNS client crate in `Cargo.lock` | `ci/dep-rules.toml:20-32`; `xtask/src/features.rs` | D3, D5 |
| The anonymisation guard runs per pull request without `--require-entries`; `ci/denylist.sha256` holds entries | `.github/workflows/docs.yml:74-75`; `xtask/src/anonymise.rs:13-14`; `ci/README.md:853-860` | D17 |
| `deep-nightly.yml` (06:07 UTC and on dispatch): `full-oracle`, `proptest`, `fuzz`, `fuzz-deny`, `asan-command-fuzz`, `gpu-parity`; no network job | `.github/workflows/deep-nightly.yml:53-61` | D4 |

## 3. Requirements traced

| Requirement | State and what this package delivers | Acceptance |
|---|---|---|
| NFR-PRIV-01 | The network sandbox CI-N-06 (D4, D5) | PV-1, PV-2 |
| NFR-PRIV-02 | No analytics or identifier: the sandbox, D3's ban, a release-checklist line (D18) | PV-2, PV-3 |
| NFR-PRIV-05 | User-only state and configuration directories on Linux, the inherited per-user ACL on Windows; leftovers pruned at start; no log but the crash log (D10, D13) | PV-7, PV-8 |
| NFR-PRIV-07 | `PRIVACY.md` true for 0.1.0 and opened from the bundle (D15, D16) | PV-10, PV-11 |
| NFR-PRIV-08 | No AI or translation service: the sandbox and D3's ban (D18) | PV-2, PV-3 |
| NFR-PRIV-09 | The guard is built; it requires its list (D17) | PV-12 |
| NFR-REL-13 (0.1.0 part) | The panic hook, the log with message and backtrace, the journal kept, the recovery offer (D6 to D11) | PV-4 to PV-7 |
| NFR-SEC-09 | No listening socket (D4) and no debug-endpoint string in a release binary (D5), beside `check-features` | PV-2, PV-9 |
| FR-UI-050 (0.1.0 part, ID-1247) | The start-up offer: Recover, Open log folder, Delete log (D11, D12) | PV-6, PV-13 |
| AC-39 | Stays G-M1-07's, CI-PR-10 (built); this package turns `--require-entries` on (§12 O3) | PV-12 |
| G-M2-09, G-M2-15 | NFR-REL-13's 0.1.0 part; SC-10 through CI-N-06 | PV-2, PV-4 to PV-6 |

## 4. Decisions

Each is a lead ruling that the owner may reverse.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **The size.** 0.6 pw (O 0.3 · L 0.5 · P 1), ID-1187's re-estimate: the PERT stays | §10 |
| D2 | **What 0.1.0 needs of ADR-0029: its Decision as amended on 2026-11-02 alone** (no update check, no crash upload, a local crash log, no connection without a user action). The Recommendation is not needed, so the ADR stays Proposed through 0.1.0 and is accepted before 1.0, as its amendment says (§12 O1). Its sentence "the network-sandbox CI job already enforces this" becomes true with D4 (§13) | The interim rule binds every build until acceptance; accepting the Recommendation would bind 1.0's flows now |
| D3 | **No network code, by the graph.** `banned_crates` in `ci/dep-rules.toml` gains `attohttpc, curl, curl-sys, h2, hickory-resolver, hyper, isahc, minreq, native-tls, openssl, reqwest, rustls, sentry, trust-dns-resolver, ureq`; `check-features` then fails on any of them in the shipped binaries' normal or build graph, and allows them on dev edges (1.0's mock server) | None is in `Cargo.lock`, so the rule costs nothing today and turns "0.1.0 holds no network code" into a per-PR check |
| D4 | **CI-N-06, the job `network-sandbox` in `deep-nightly.yml`** (ubuntu-24.04). Before the namespace it builds `elektrawatt` and `ew` with `cargo build --release --locked` and the `ew-ui` kittest test binary, and copies Golden A. Then each flow runs as the runner user in a new network namespace whose only interface is loopback, up (`unshare --net` through `sudo`, then dropping back to the user), under `strace -f -e trace=socket,connect,bind,listen,sendto,sendmsg,sendmmsg,openat -o <flow>.strace`. Flows: `elektrawatt --smoke <GP>`; `elektrawatt --headless --script ci/sandbox/flows.ewui --config-dir <tmp>` (first run skipped, open GP, an edit, check, export PDF, print to file, save, quit); the kittest flows that `ci/sandbox/kittest-filter.txt` names (first run, open GP, edit, check, export PDF, the ones that exist); `ew` `check`, `export pdf`, `export svg`, `export bom`, `render`, `sql`, `import qet` on a committed public fixture, `print --to-file`, `apply` (the subcommands that exist at the lane's base, listed in the job). Red on any finding of D5 or a failed flow. `strace` is the hosted image's (installed by `apt-get` before the namespace when absent) | Doc 18 §12.2 at 0.1.0's size. The deep nightly gates every release candidate (WP-S6.07 D6), so the sandbox gates 0.1.0; per pull request it would cost a release build each time |
| D5 | **The verdict, `ci/sandbox-check.py`** (standard library; `--test` its offline unit test). `scan <flow>.strace…` fails on: a `socket` of `AF_INET` or `AF_INET6`; a `connect` or `sendto` to an `AF_INET`/`AF_INET6` address; a `connect` to `/run/systemd/resolve/*`, `/run/nscd/*` or `/var/run/nscd/*`; any `listen`; a `bind` of `AF_INET`/`AF_INET6`; an `openat` of `/etc/resolv.conf`, `/etc/hosts` or `/etc/gai.conf` (the files a name lookup reads, in place of doc 18's `LD_PRELOAD` shim). It passes other `AF_UNIX` traffic and `AF_NETLINK`. No allowlist: 0.1.0 has no `ew mcp`. `canary <binary>…` fails when a release binary contains the bytes `EW_DEBUG_PANIC` (D9) or `egui_mcp`. Each failure prints the flow or file, the syscall line and the rule | The release-binary audit for debug endpoints is then three checks: the graph (`check-features`, per PR, built), the strings (here, nightly on release builds) and the socket scan (here); WP-S6.07 D12 adds the embedded package list of the release artefacts. A text scan of `strace` output needs no C shim |
| D6 | **The panic hook** (`bin/elektrawatt/src/crash.rs`), installed first in `main` for every mode (window, headless, smoke), before the previous hook, which it then calls (stderr keeps the default message). Per panic it appends one entry to the process's crash log `<state>/crashes/<yyyymmddThhmmssZ>-<pid>.log` (the process start in UTC; created at the first panic): the lines of §5 L1 to L7, then the frames of `std::backtrace::Backtrace::force_capture()`. It takes no lock of the session, store or journal; a panic inside the hook is not logged again (a thread-local guard); a write error is ignored. Without a state directory it writes nothing. The start time is the one wall-clock read, allowed at its site with its justification (FT-317's list) | NFR-REL-13's log; the hook must not deadlock on a lock the panicking thread holds. The location is compiled in, so it is there even when a release backtrace has no names (Linux keeps the symbol table; Windows names come from the `.pdb` WP-S6.06 D5 ships) |
| D7 | **What a crash is.** `main` runs its mode inside `catch_unwind`; a panic that reaches it appends L8 to the log and the process exits with code 101. A panic caught on its way (the sites of §2.1: job threads, rule runs, printing, decoders, tessellation) is logged without L8 and the session goes on, which tells FT-310's caught tessellator panic from a crash. A panic that ends a thread other than the main thread is logged without L8 (FP-1) | The hook cannot know whether a panic will be caught; the outermost frame can. Doc 18 §12.3's abort after a journal flush would end a session that the catch sites keep alive |
| D8 | **The journal: kept, not flushed.** Every entry is synced as it is appended (`journal.rs:16-18`), so a panic finds every committed transaction on disk and the hook does nothing to the journal; the unwind reaches no clean close (`main.rs:500-503` runs only at a normal exit), which the forced-panic test proves | Doc 18's 500 ms flush serves a group commit that the code does not have; a flush from the hook would take the store's lock |
| D9 | **The forced panic.** `elektrawatt` built with `debug_assertions` reads `EW_DEBUG_PANIC`: set to `1`, a headless run panics on the main thread after its script's last step, with the message L6's value "forced panic (EW_DEBUG_PANIC)". Release builds compile none of it, which D5's canary checks. The environment read is allowed at its site (FT-317's list) | The test of NFR-REL-13 needs a real panic through the real binary; a hidden script action would enter the action registry and its snapshots, and an environment variable leaves no trace in a release build |
| D10 | **Storage, retention, permissions.** On Unix the state directory, its `journal/` and `crashes/` and the configuration directory are created with mode 0700 and tightened to it at start when wider; files created in them get 0600. On Windows they inherit the per-user ACL of `%LOCALAPPDATA%` and `%APPDATA%` (checked by hand with `icacls` at RC-05). At start: crash logs older than 30 days or beyond the 20 newest are deleted; leftover journals older than 30 days of every project are deleted (doc 05 §8.1's start-up pruning, today only at the project's next open) | NFR-PRIV-05; one rule for every file EW keeps outside projects |
| D11 | **The start-up offer** (NFR-REL-13's "offer recovery at the next start"; FR-UI-050's 0.1.0 part as ID-1247 reads it; doc 05 §8.3's start-up half). At start (window and headless, not smoke) the shell lists (a) the leftover journal segments with unsaved transactions under `<state>/journal/*/`, skipping a project whose lock is live (`lock.rs`'s stale rule), each with the project's directory name, path, unsaved count and last change; (b) the newest crash log that ends with L8 and was not offered. When either is non-empty it shows the prompt of §5 S1 to S7: Recover per project (opens it as Open does; WP-S2.09's recovery dialog follows), Open log folder and Delete log (with a log), Not now. When the prompt closes, an offered log is renamed `<stem>.offered.log` and never offered again; the journals stay until recovered, discarded or pruned. Headless, the prompt is pending and blocks no other action; `startup.recover {project}` (1-based), `startup.open_log_folder`, `startup.delete_log` and `startup.dismiss` answer it, and `ui.describe` lists it | A crash without a dirty project still shows its log once; a dirty project left by any unclean end is offered at the next start, whichever project the user opens; a non-blocking headless prompt leaves every existing script unchanged |
| D12 | **What stays 1.0's** (WP-S6.10): Copy summary and Help ▸ Diagnostics (both need the scrubber of doc 18 §12.3 to be "without project data"), `report.json`, the minidump and its monitor, the payload preview and the upload, the abort strategy (FP-1) | ID-1247: 0.1.0 has the log, the kept journal and the offer; §12 O2 |
| D13 | **No log but the crash log.** 0.1.0 installs no `tracing` subscriber, so the events of §2.1 go nowhere and EW writes no other log file. The crash log keeps the panic's message as NFR-REL-13 asks, although a message can quote project text; it is user-only (D10), local and named so in `PRIVACY.md` (D15). The daily log with `Sensitive<T>` is 1.0's | NFR-PRIV-05's "the default log level records no project text" holds because no leveled log exists; the one record that can hold project text is the one NFR-REL-13 requires |
| D14 | **`ew` gets no hook:** a CLI panic prints Rust's message to stderr and exits 101, as today | A CLI user sees the panic where it happened; its sessions keep their journal behaviour |
| D15 | **`PRIVACY.md` kept true**, in the file's style, with these facts: "In short": a crash writes a local log that this version never sends; the stored-data table gains the row of §5 P1; the journal row's retention: offered at the next start and pruned after 30 days at a start; "The network sandbox": it runs every night on Linux with only loopback up and fails on any socket, name lookup or listening port, and Windows is observed by hand for each minor release (the claim that Linux is observed by hand goes); the permissions of D10. The flows table is unchanged (NFR-PRIV-07) | Every sentence the package makes true or false is corrected in the same change |
| D16 | **`PRIVACY.md` from the bundle** (FT-386's window half; WP-S6.06 D3 places it in the tree). `Resources` gains `privacy_file`, found as the notices are (beside the executable; the repository root in a development build); the About and first-run privacy buttons open it locally through the opener, and fall back to `PRIVACY_URL` with the banner of `app.rs:1754` only when no file is found | NFR-PRIV-07 offline; doc 18 §12.1's web-link exception ends |
| D17 | **The anonymisation guard requires its list:** `docs.yml` runs `cargo xtask check-anonymisation --require-entries` (its README's switch-on condition, a committed list, is met; §12 O4) | AC-39 stays green with an emptied list now failing |
| D18 | **NFR-PRIV-02 and NFR-PRIV-08's audit half** is D3's ban plus a line of WP-S6.07's release checklist (RC-01's review): "no analytics, install identifier, feature counter, AI or translation call" | The sandbox proves the flows; the review covers what a flow-less build could still carry |
| D19 | **Every item closed;** no decision changes an ADR or an architecture edge | — |

## 5. The texts

Every text is asserted whole (en-GB); the nl-NL texts are the lane's translation. `<…>` are filled as named.

| # | Where | Text |
|---|---|---|
| L1 | Crash log, entry line 1 | `ElektraWatt <version> crash log` |
| L2 | line 2 | `OS: <std::env::consts::OS> <std::env::consts::ARCH>` |
| L3 | line 3 | `Time: <yyyy-mm-ddThh:mm:ssZ>` |
| L4 | line 4 | `Thread: <name, else "unnamed">` |
| L5 | line 5 | `Location: <file>:<line>:<column>` |
| L6 | line 6 | `Message: <the payload's text, else "(no message)">` |
| L7 | line 7 | `Backtrace:`, then the frames; an entry ends with an empty line |
| L8 | after the entry of a panic that reached `main` | `Ended: this panic ended the session` |
| S1 | Prompt title, with a project | `Unsaved work from an earlier session` |
| S2 | Prompt title, a log alone | `ElektraWatt stopped unexpectedly` |
| S3 | Crash line, with a project | `ElektraWatt stopped unexpectedly on <yyyy-mm-dd hh:mm> UTC. Its log is in <folder>.` |
| S4 | Crash line, a log alone | `ElektraWatt stopped unexpectedly on <yyyy-mm-dd hh:mm> UTC; no project had unsaved changes. Its log is in <folder>.` |
| S5 | Project row | `<project>: <n> unsaved changes, last change <yyyy-mm-dd hh:mm> UTC` (one: `1 unsaved change`) |
| S6 | Buttons | `Recover`, `Open log folder`, `Delete log`, `Not now` |
| S7 | Refusals | `The project is no longer at <path>.`; `The log folder could not be opened; it is at <folder>.` |
| P1 | `PRIVACY.md`, stored-data row | What: crash logs (`crashes/`). Purpose: help find the cause of a crash. Location: the state directory, beside the journal. Retention: the 20 newest, each at most 30 days; offered once at the next start after a crash. How to delete: delete the file, or Delete log in the start-up dialog. A log holds the error's message, which can quote project text such as a tag or a file name, and the program's call stack; it is never sent anywhere |

## 6. Build order

One lane builds the six steps in this order and commits at the end of each.

1. **The hook and the log** (D6, D7, D9, D13): `crash.rs`, the install in `main`, the outer `catch_unwind` and exit
   code 101, the debug trigger; PV-4, PV-5.
2. **The directories** (D10, D8): modes, start-up pruning of logs and journals; PV-7, PV-8.
3. **The start-up offer** (D11): the scan across projects in `ew-store`, the prompt in `ew-viewmodel` with its
   headless actions and strings, the dialog in `ew-ui`; PV-6, PV-13 with frames.
4. **The bundle** (D16) and **`PRIVACY.md`** (D15); PV-10, PV-11.
5. **The guards** (D3, D17, D18): the banned crates with an `xtask` unit test, `--require-entries`, the checklist line
   handed to WP-S6.07's template (or its draft, if that package is not built yet); PV-3, PV-12.
6. **The sandbox** (D4, D5): `ci/sandbox-check.py` with `--test`, `ci/sandbox/flows.ewui`, the kittest filter, the
   job; `ci/README.md`'s CI-N-06 row; PV-1, PV-9, then PV-2 at the delivery's CI run.

## 7. Acceptance

- **PV-1** `python -I ci/sandbox-check.py --test` passes: fixture `strace` lines of an `AF_INET` socket, a `connect` to
  the resolved socket, an `openat` of `/etc/resolv.conf`, a `listen` and an `AF_INET` `bind` each fail with the flow,
  the line and the rule; `AF_UNIX` D-Bus traffic and `AF_NETLINK` pass; a fixture binary holding `EW_DEBUG_PANIC` fails
  the canary and one without it passes.
- **PV-2** (CI run, the delivery's) The `network-sandbox` job is green: every flow of D4 ran, no finding.
- **PV-3** An `xtask` unit test: a fixture graph with `reqwest` on a normal edge of a shipped binary fails
  `check-features`, the same on a dev edge passes; the workspace passes.
- **PV-4** (NFR-REL-13) A test runs the debug `elektrawatt --headless` with `EW_DEBUG_PANIC=1`, a fresh
  `--config-dir` and a script that opens a copy of Golden A and makes one edit: the exit code is 101; one log in
  `<config>/state/crashes/` whose entry has L1 to L7 with the thread `main`, a location in `bin/elektrawatt/src/`, the
  message "forced panic (EW_DEBUG_PANIC)", at least one backtrace frame, then L8; the journal replays to the model
  after the edit (equal digests).
- **PV-5** With the hook installed, a panic caught by `catch_unwind` (a rule run's EW-CORE-083 path) appends an entry
  without L8 and the session goes on; such a log is never offered.
- **PV-6** The next headless start after PV-4 holds the pending prompt with S1, S3 and one S5 row; `startup.recover 1`
  opens the project and WP-S2.09's recovery prompt follows; a start after `startup.dismiss` shows the project row and no
  crash line; `startup.delete_log` deletes the log; a missing project gives S7's first text. Every text whole.
- **PV-7** A leftover journal whose project holds a live lock is not listed; a journal and a log older than 30 days and
  a 21st log are deleted at start.
- **PV-8** (Unix) After a start the state directory, `journal/`, `crashes/` and the configuration directory have mode
  0700 and a crash log and a journal segment 0600, also when the directories existed with 0755.
- **PV-9** `ci/sandbox-check.py canary` passes on the job's release binaries (CI run), and the debug `elektrawatt`
  contains `EW_DEBUG_PANIC` (the canary's positive control, a local test).
- **PV-10** The About and first-run privacy buttons open `PRIVACY.md` from `Resources::privacy_file` when present and
  `PRIVACY_URL` when absent (two tests through the platform seam).
- **PV-11** `PRIVACY.md` carries D15's facts and P1; `cargo xtask check-governance-files` passes.
- **PV-12** `cargo xtask check-anonymisation --require-entries` passes on the tree, and its unit test with an empty
  list fails.
- **PV-13** (window) The start-up dialog with a project and a crash line, at 1280 × 800 and 1920 × 1080; by hand on
  Windows: a window ended in Task Manager with a dirty project offers it at the next start, and `icacls` shows the
  state directory's per-user ACL (RC-05).

## 8. Neighbours, files owned and what a lane may not decide

### 8.1 Neighbours

| Neighbour | It owns | This package |
|---|---|---|
| WP-S2.09 | The recovery dialog at open, FR-UI-050's traced row | Recover hands over to it |
| WP-S2.01 part 2 | FT-310 (native menus, GPU device loss) | D7 tells its caught panic from a crash |
| WP-S6.06 | The installed tree with `PRIVACY.md` and the `.pdb` files, `package.yml` | Reads `PRIVACY.md` from the tree (D16) |
| WP-S6.07 | The release workflow, CI-R-09's artefact audit (D12), the checklist | Gives the deep-nightly job that the release requires (WP-S6.07 D6) and one checklist line (D18) |
| WP-S6.10 | Everything of §1's first non-goal | Leaves the crash directory ready for `report.json` and the minidump |

### 8.2 Files owned

`bin/elektrawatt/src/main.rs` (the install, the outer catch, the trigger, `locate_resources`), `bin/elektrawatt/src/crash.rs`
(new); `crates/ew-store/src/journal.rs` and `store.rs` (the scan across projects, pruning, modes);
`crates/ew-viewmodel/src/{shell,about,actions,settings}.rs`, the start-up prompt's module, `strings/`, `locales/`;
`crates/ew-ui/src/{app,first_run}.rs` and the dialog; `ci/sandbox-check.py`, `ci/sandbox/` (new);
`.github/workflows/deep-nightly.yml` (one job), `.github/workflows/docs.yml` (one flag); `ci/dep-rules.toml`
(`banned_crates`); `xtask/src/features.rs` (its test), the FT-317 review list of `check-disallowed-methods`;
`ci/README.md` (rows); `PRIVACY.md`. Read-only: every other crate, `deny.toml`, `supply-chain/`, `Cargo.lock`,
`golden/**`, `docs/**`.

### 8.3 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a flow opens a network socket, reads a name-lookup file or listens: it is a finding, reported with its flow and
   stack, never allowlisted;
2. a needed change reaches a network flow of `PRIVACY.md`, an ADR, a new dependency, or a GitHub setting;
3. the hook would need a lock, an allocation-free path or `unsafe`;
4. a text would differ from §5, or the journal would be touched by the hook;
5. the namespace cannot be made on the hosted runner without a privilege other than `sudo`.

The lane decides freely: module and function names, the script's step names, the job's step names, the strace output
layout, the dialog's layout under the design language, and the nl-NL wording.

## 9. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 A dependency reads `/etc/hosts` or opens a socket at start (a font or portal library) | Low · high | It is a finding (§8.3 item 1): the probe names the crate; a ruling follows |
| R2 Ubuntu's restriction of unprivileged user namespaces blocks `unshare` | Medium · low | The namespace is made through `sudo` (§8.3 item 5) |
| R3 The window's event loop does not carry a panic back to `main` on Windows, so L8 is missing there | Medium · medium | PV-13's manual check; the leftover journal is offered either way, the crash line alone is lost (FP-1) |
| R4 A test leaves a crash log or journal in a shared configuration directory and a later test meets the prompt | Low · low | The headless prompt does not block (D11); tests use fresh directories |
| R5 A release backtrace has no names | Medium · low | The location is always present (D6); the `.pdb` beside the executable on Windows (WP-S6.06 D5) |

## 10. Size and routing

**Size.** 0.6 pw: step 1 0.12, step 2 0.06, step 3 0.17, step 4 0.05, step 5 0.04, step 6 0.16. The PERT of 0.6 pw
stays.

**Routing** (`.claude/orchestration.md` rule 0): one lane, **Opus at `medium`** (decided work with tests to run; the
hook sits on every panic path and the prompt on every start). It depends on WP-S2.01 (built) and on WP-S2.09 part 1
(built); D16 reads the tree of WP-S6.06 part 1 when built and falls back otherwise.

## 11. Follow-ups, proposed

The design pass opens the rows. **FT-386** closes with step 4 (its bundling half closed with WP-S6.06); **FT-495**'s
0.1.0 part (ID-1247) with step 3; its 1.0 part stays in WP-S6.10.
- **FP-1** Panics on threads other than the main thread: doc 18 §12.3's catch-scope marking and abort, with the
  crash line for them (WP-S6.10).
- **FP-2** A crash log for the `ew` CLI.
- **FP-3** The Windows ACL check of D10 automated in the Windows CI job.
- **FP-4** A page of the user guide, "After a crash".
- **FP-5** `PRIVACY.md` and `default_state_dir` still name macOS paths, though macOS is not supported (ID-880).

## 12. Open questions

- **O1** (the owner's) ADR-0029's acceptance. Options: (1) 0.1.0 ships under the interim rule and the ADR stays
  Proposed until before 1.0, as its amendment of 2026-11-02 says; (2) accept it before 0.1.0. Recommendation: (1).
- **O2** FR-UI-050's dialog at 0.1.0. Options: (1) Recover, Open log folder, Delete log and Not now, with Copy summary
  and Help ▸ Diagnostics at 1.0 beside the scrubber (D12); (2) add Copy summary now with the header lines L1 to L5 only
  (about 0.05 pw). Recommendation: (1).
- **O3** AC-39: the WP row lists it, while G-M1-07 already holds it through CI-PR-10. Options: (1) it stays G-M1-07's,
  and this package's part is D17; the design pass rewords the WP's acceptance cell to NFR-PRIV-01 and NFR-REL-13;
  (2) keep it in both. Recommendation: (1).
- **O4** (the owner's) `--require-entries` makes an emptied token list fail CI; the owner keeps the list for the
  pilot's corpus. Options: (1) switch it on now (D17); (2) wait for the owner's word that the list is complete.
  Recommendation: (1).
- **O5** The size: 0.6 pw, the PERT unchanged. Recommendation: keep it.

## 13. Documents to update

At the design pass after the lane: ADR-0029's Decision ("the network-sandbox CI job already enforces this": CI-N-06,
built by WP-S6.05); doc 18 §12.2 (as built: `unshare --net`, the name-lookup files instead of the `LD_PRELOAD` shim,
the flows, the canary), §12.3 (0.1.0's form: every panic logged, the end mark at `main`, no abort, no flush because
every entry is synced), the CI-N-06 row and TM-10; doc 05 §8.1 (pruning at start, the panics paragraph) and §8.3 (the
start-up half as built); doc 16 (the start-up dialog); 02c NFR-PRIV-05 (0.1.0 writes no log but the crash log); 02b
FR-UI-050 ("built in part" note); doc 02's WP-S6.05 acceptance cell (§12 O3); `ci/README.md` (the lane's).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S6.05, WP-S2.01, WP-S2.09, WP-S6.06, WP-S6.07, WP-S6.10; FT-255,
  FT-310, FT-342, FT-386, FT-495.
- [Decisions](../decisions.md): ID-1187, ID-1188, ID-1203, ID-1210, ID-1229, ID-1247, ID-1281, ID-1311.
- [ADR-0029](../../adr/0029-network-telemetry-crash-reporting-policy.md).
- [Quality](../../design/18-quality-security-release.md) §12, the CI-N table, TM-10;
  [project format](../../design/05-project-format-and-persistence.md) §8;
  [non-functional requirements](../../design/02c-nonfunctional-requirements.md) §6, NFR-REL-13, NFR-SEC-09;
  [extended requirements](../../design/02b-requirements-extended.md) FR-UI-038, FR-UI-050;
  [golden project](../../design/20-golden-project.md) §3.5, AC-39; [gates](../03-gates.md) G-M1-07, G-M2-09, G-M2-15.
- `PRIVACY.md`; the specifications of [WP-S6.06](WP-S6.06-pilot-installers-and-platform-matrix.md) and
  [WP-S6.07](WP-S6.07-release-pipeline.md).
