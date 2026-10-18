# ADR-0016: 3D renderer on wgpu; out-of-process OCCT STEP worker via occt-sys

**Status:** Accepted · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the judge panel)

## Context

- **Scope.** v1 must include a Pro Panel-class 3D panel layout: a 3D cabinet, rails and ducts, collision checks,
  wire routing and automatic wire lengths (vision, 1.0 scope, milestone M4). Folio 902 of the reference drawing set
  shows what the layout must at least replace: an NSYPLA1074G enclosure with an NSYPMM107 plate, 40 × 60 ducts, end
  brackets and device rectangles placed by tag. No DIN rails are drawn
  ([gaps-and-corrections](../research/gaps-and-corrections.md) §3.1). Two of its defects come from a layout that is
  not bound to the schematic: T51 versus T55 (D-02) and an incomplete layout without F55, E55, E56, S56 and -201U9
  (D-32).
- **Vendor geometry.** Rittal, Phoenix Contact, Siemens and Festo publish STEP AP214 or AP242 files with assemblies.
  Only OCCT reads them reliably today ([rust-ecosystem](../research/rust-ecosystem.md) §5.1). OCCT is C++
  (LGPL-2.1 plus the Open CASCADE exception, which does not relax relinking duties, C-6), takes a long C++ build,
  and can crash or hang on pathological files. The project's constraints allow C/C++ FFI only where justified, and
  name a B-rep/STEP kernel as the example ([vision](../design/00-vision-and-scope.md) §10.1, K-2).
- **Binding candidates** (C-5):
  - cadrum downloads prebuilt static OCCT 8.0.1 at build time and sits below the adoption threshold (about 7k
    downloads in 90 days, no MSRV);
  - occt-sys 7.8.1 plus opencascade 0.3.0 build OCCT from source. They are better adopted, but the wrapper
    describes itself as "a major work in progress".
- **No ready-made renderer fits.** The candidates own the app loop, run on OpenGL, or are archived (see
  Alternatives). kiss3d is "not designed to be feature-complete" and suits only debug views of routing graphs.
- **Load.** Scenes are modest: one enclosure, 100–500 devices and a few thousand wires. The XL bound is 10,000
  placements (NFR-PERF-17). Repeated parts dominate: the PT 2.5-TWIN terminals on +JE2416-X1031 are one part
  placed many times.
- **Fixed constraints.**
  - One wgpu device is shared by egui, the 2D canvas and 3D (ADR-0014, ADR-0015).
  - GPU-less RDP and VDI hosts must keep working (NFR-PLAT-03).
  - Routing takes seconds and is therefore Class II (ADR-0003).

## Decision

1. **Own renderer.** `ew-render3d` is our own wgpu 30 renderer, drawn on the shared device inside an `egui_wgpu`
   paint callback. It provides:
   - parametric prisms for enclosures, plates, rails and ducts;
   - instanced meshes, one mesh per (part, LOD), so the strip's twin terminals cost one mesh and N instances;
   - B-rep feature edges drawn as screen-space lines;
   - weighted blended transparency for the enclosure shell;
   - a CAD camera: orbit about the picked point, zoom to cursor, standard views, ortho and perspective;
   - frustum culling and LOD by projected size (LOD1 below 40 px, a box below 8 px);
   - transform-gizmo-egui 0.11.0 for placement.
2. **Scene as a query.** `layout_scene(space)` is a memoised query that returns an `Arc<Scene>` of instances. Each
   instance carries a placement ID, a mesh hash, a world pose and a state (selected, colliding, stale). Poses come
   from the model in i64 nm plus a quaternion (ADR-0005). f64 is used only for maths, and f32 only in GPU buffers,
   relative to a space-local origin.
3. **Reconciler.** Mutable structures (GPU instance buffers, parry3d 0.31.1 BVHs) are kept in step by a reconciler.
   It diffs successive `Arc<Scene>` values by placement ID and applies insert, remove and refit operations. In
   debug builds it also rebuilds from scratch and compares. A field-mask test fails when a new scene field is not
   reconciled.
4. **Collision and picking.**
   - Collision (EW-LAY-006) uses a BVH broad phase and a hull-then-mesh narrow phase.
   - Picking is a CPU ray cast against the same BVHs. It is deterministic and needs no GPU readback.
   - An ID buffer is optional, for hover feedback only.
5. **STEP worker.** STEP is read in a separate executable, `ew-step-worker`. It links occt-sys 7.8.1 and
   opencascade 0.3.0, and no other binary links OCCT.
   - **IPC contract.** The protocol lives in the permissive `ew-step-proto` crate. Frames are length-prefixed
     postcard over stdio.
     - Requests: `Hello{proto}`, `Ingest{job, source, sha256, opts{lin_defl, ang_defl, tri_budget,
       detect_features}}`, `ExportStep` (cabinet assembly as STEP AP242 named by device tag; FR-EXP-008, Could,
       1.0), `Hlr` (hidden-line views; FR-LAY-040, 1.x), `Cancel{job}`, `Shutdown`.
     - Replies: `Progress`, `IngestDone`, `Error{Timeout | Parse | Memory | Crash}`.
   - **Supervision.**
     - one or two pooled workers;
     - a per-job timeout (default 120 s), a memory cap (a job object on Windows, `RLIMIT_AS` on Unix, default
       4 GiB) and an input limit (default 200 MB);
     - restart with back-off;
     - quarantine of a file hash that crashed the worker twice.
   - **Fuzzing.** The IPC decoder has a fuzz target.
6. **Class III cache.** The worker's outputs are:
   - the assembly tree;
   - LOD0 (display, at most 200,000 triangles) and LOD1 (collision and far display, default 20,000, meshopt 0.7.0);
   - feature edges, the bounding box and convex hulls;
   - mounting frames, such as a detected 35 mm rail-clip groove;
   - holes and openings.

   They are stored under `<cache>/step/<sha256>/<worker_version>/` as glb and rkyv files, and a cached reopen is a
   memory map. Meshes are never model data: a `PartCopy` references the hash, never the vendor file. A decimated
   LOD1 mesh is embedded in the project, as the binary asset `assets/<sha256>.glb`, only when the part's
   `embeddable` flag allows it (FR-LAY-022). On a machine without the model the part renders as a box marked
   "model not available".
   - **Validation** (FR-LAY-021): the bounding box must match the part's W×H×D within ±5 %; a match at 1/25.4
     proposes an inch-file scale of 25.4; solids thinner than 0.3 mm are excluded from collision.
   - **No STEP model** (FR-LAY-023): a box macro is generated from W×H×D, with connection points spread over the
     top and bottom faces in pin-list order and `geometry_quality = generated` in every output that uses it.
7. **Routing.** Negotiated-congestion rip-up and re-route runs over the duct graph on a fork (Class II). Its paths
   and lengths become model data only through the explicit, previewed `RouteWires` command, as
   `Committed<T>{value, input_hash, rev}`. EW-LAY-011 flags a committed route whose input hash is stale (ADR-0003).
8. **Build isolation.** The worker builds in its own CI job with cached OCCT artefacts. The rest of the application
   builds with Rust and a C compiler only (NFR-MAINT-09). No build script downloads binaries (ADR-0023).
9. **GPU-less hosts.** 3D runs on DX12 WARP or a software Vulkan adapter where one exists, using LOD1. Otherwise the
   3D view is disabled with an explanation, and the 2D panel projection (the folio 902 type) stays available
   (NFR-PLAT-03).

## Consequences

**Positive**
- A crash, hang or memory blow-up on a vendor file never reaches the model or the session (NFR-REL-12). The worst
  case is a reported failed import.
- Project records stay text. Meshes live in the cache and are rebuilt from the hash, so merges and diffs are
  unaffected (ADR-0010). The only mesh a project can carry is the opt-in `assets/<sha256>.glb` of an embeddable
  part, a content-addressed binary that never conflicts on merge.
- The worker contract lets OCCT be swapped without touching the application: OCCT 8, a system OCCT, cadrum if it
  gains a source build, or a pure-Rust kernel later.
- LGPL obligations are visible and easy to meet: a source offer and a replaceable worker binary (NFR-LIC-08).
- One GPU device serves egui, 2D and 3D, and the egui, wgpu and winit upgrade train covers the 3D renderer too
  (ADR-0014).

**Negative**
- We own a 3D renderer and a CAD camera. That is estimated at a few thousand lines plus about 300 for the camera,
  and it needs owners in stream S4.
- The first import of a large file is slow (≤ 30 s for 20 MB, NFR-PERF-18), and IPC adds copying. Only the cached
  reopen (≤ 1 s) is interactive.
- occt-sys builds OCCT from C++ source, which takes a long time (likely tens of minutes, unverified). This needs the
  separate CI job and artefact caching.
- The opencascade wrapper's STEP coverage is partial. Its gaps must be closed by bindings inside the worker and
  proven on a vendor STEP corpus before M4.
- A second process lifetime must be supervised. An OS sandbox for the worker (no network, read its input, write
  only its cache) is a 1.x Could (NFR-SEC-21, [18](../design/18-quality-security-release.md) §11).

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **In-process OCCT** | A segfault in C++ kills the editor. The journal limits data loss, but the session is lost. Static LGPL linking adds relinking duties to the main binary, and the C++ build would sit on every developer's critical path |
| **cadrum** (prebuilt OCCT 8.0.1) | Downloading binaries at build time is a supply-chain and SBOM liability under the CRA (C-5, ADR-0023). Its adoption is below the watch-list threshold (C-9). It remains an evaluated option behind the same worker contract |
| **Pure-Rust kernels** (truck, monstertruck) | truck's last release dates from 2024-09, and its robustness on vendor STEP is undocumented. monstertruck has about 500 downloads. They are kept as an evaluation track behind `ew-step-proto`, not as a dependency |
| **Fornjot** | Shut down; repository archived 2026-06-19 |
| **rend3** | Archived 2025-06-07 |
| **Embedding Bevy** | It wants to own the window and app loop, which conflicts with the eframe shell. Its quarterly breaking releases would add a second upgrade train. Most of the engine would go unused |
| **three-d** | OpenGL only. eframe would have to leave its wgpu backend, so no device could be shared with the 2D canvas |
| **GPU ID-buffer picking only** | Readback stalls, and the results depend on the backend. Kept only as an optional hover aid |

## Related

- **ADRs:** [ADR-0003](0003-freshness-contract.md) (Class II/III, committed computations), [ADR-0005](0005-value-types-nm-dec3-mltext.md) (nm and quaternion poses), [ADR-0010](0010-project-storage-jsonl-directory.md) (no derived
  data on disk), [ADR-0014](0014-gui-toolkit-egui.md) (shared device, upgrade train), [ADR-0015](0015-display-list-renderers-pdf.md) (2D IR for panel pages),
  [ADR-0022](0022-platform-matrix-and-performance-budgets.md) (budgets),
  [ADR-0023](0023-supply-chain-and-release-integrity.md) (no build-time downloads),
  [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md) (`ew-step-proto` is permissive).
- **Design:** [04 architecture](../design/04-architecture.md) §2, §3, §8.5, §8.6;
  [12 panel layout](../design/12-panel-layout.md) §6; [18 quality](../design/18-quality-security-release.md) §11;
  [rust-ecosystem](../research/rust-ecosystem.md) §4–§5; [gaps-and-corrections](../research/gaps-and-corrections.md)
  C-5, C-6, C-9.
- **Requirements:** FR-LAY-019, FR-LAY-020, FR-LAY-021, FR-LAY-022, FR-LAY-023, FR-LAY-028, FR-LAY-040, FR-LAY-045,
  FR-LAY-046, FR-LAY-051; FR-EXP-008; NFR-PERF-16, NFR-PERF-17, NFR-PERF-18, NFR-REL-12, NFR-PLAT-03,
  NFR-MAINT-09, NFR-LIC-08, NFR-SEC-03, NFR-SEC-04, NFR-SEC-21.
