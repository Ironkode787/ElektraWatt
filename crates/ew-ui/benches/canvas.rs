// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `canvas` bench (WP-S2.02 part 1, spec section 9): the frame cost of the page canvas under a gesture script,
//! on the CPU tile path, without egui.
//!
//! Run with `cargo bench -p ew-ui --bench canvas` (`-- --quick` takes 100 frames per scene instead of 1,000: a
//! smoke run, not evidence). `cargo xtask perf-gate` runs it once per gate run; its values are report-only.
//!
//! **Scenes.** Golden A folio 103 (a temporary copy of the sample project, removed at the end) and a dense A3
//! landscape page made by translating copies of folio 103's display list until at least 5,000 paths and glyph runs
//! are drawn. The latter's family names end in `.dense_a3`.
//!
//! **Gesture script** per scene, 1,000 frames (quick: 100): 400 pan steps of 40 points, 400 zoom steps about
//! anchors that move across the sheet and cross tile levels (ladder runs in and out between 25 % and 800 %, four
//! frames per ladder step), 200 pointer moves over entity centres, in a 1200 × 800 point pane at 1 pixel per point.
//! The planner's clock is a fake one that advances 1/60 s per frame, so a moving view never settles.
//!
//! **One frame** is what `ew_ui::canvas::CanvasHost::show` does minus egui, on the same planner, tile cache, worker
//! pool and overlay: `input` (the model update), `scene` (the scene is made on the first frame and reused after),
//! `render` (the plan, the finished tiles taken from the pool, at most 8 uploads and 2 MiB per frame, the requests
//! for the missing ones) and `paint` (the cache lookups of the placed tiles, the overlay and the eviction). The
//! frame families are the sum of the four phases; the per-phase families split it. `render.pan.cpu` and
//! `render.zoom.cpu` are the raster latency: from the frame's plan until every visible tile of the frame's wanted
//! resolution has been rastered by the pool (uploads are not capped there). That wait is outside the frame's
//! phases. Pointer frames count into `frame.gesture.point` only.
//!
//! **Output**: the title line, the header `class=GP generator_version=0 mode=…`, then per scene one `scene` line
//! and the gate's `bench <family> median=…ms p99=…ms` lines (nearest-rank percentiles). Nothing else carries a
//! time, and nothing is written into the repository.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::hint::black_box;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ew_render_cpu::{FontBytes, FontResolver, SheetSize};
use ew_render2d::overlay::{self, GridSpec, OverlayInput};
use ew_render2d::pool::{Scene, TilePool, TileResult, worker_count};
use ew_render2d::stats::{Phase, nearest_rank, now};
use ew_render2d::tiles::{DEFAULT_CACHE_BYTES, Planner, TileCache, TileKey};
use ew_render2d::view::View;
use ew_ui::canvas::{MAX_UPLOAD_BYTES_PER_FRAME, MAX_UPLOADS_PER_FRAME};
use ew_ui::ew_viewmodel::canvas::{CanvasModel, CanvasPage, rect_centre, zoom_in_step, zoom_out_step};
use ew_ui::ew_viewmodel::ew_api::output::ew_dl::ew_units::{Nm, Pt2};
use ew_ui::ew_viewmodel::ew_api::output::ew_dl::{DisplayList, FontKey, Item, ItemKind, Transform};
use ew_ui::ew_viewmodel::ew_api::output::font_bytes;
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

/// The pane in logical points and the pixel density of the script.
const PANE: (f64, f64) = (1200.0, 800.0);
const PPP: f64 = 1.0;

/// The fake time between two frames (seconds).
const FRAME_SECONDS: f64 = 1.0 / 60.0;

/// The distance of one pan step in points.
const PAN_STEP_POINTS: f64 = 40.0;

/// The frames one pan direction is held for.
const PAN_RUN: usize = 10;

/// The zoom range of the zoom gesture and the frames spent on one ladder step.
const ZOOM_LOW: f64 = 0.25;
const ZOOM_HIGH: f64 = 8.0;
const FRAMES_PER_LADDER_STEP: usize = 4;

/// The zoom the pan gesture runs at.
const PAN_ZOOM: f64 = 2.0;

/// The dense page draws at least this many paths and glyph runs.
const DENSE_MIN_ITEMS: usize = 5_000;

/// A3 landscape in nm.
const A3_LANDSCAPE: (i64, i64) = (420_000_000, 297_000_000);

/// A frame that waits longer than this for its tiles is a bench failure.
const TILE_WAIT_LIMIT: Duration = Duration::from_secs(120);

/// The frame counts of a mode: pan, zoom and pointer frames.
#[derive(Clone, Copy)]
struct Counts {
    pan: usize,
    zoom: usize,
    point: usize,
}

impl Counts {
    const FULL: Counts = Counts {
        pan: 400,
        zoom: 400,
        point: 200,
    };
    const QUICK: Counts = Counts {
        pan: 40,
        zoom: 40,
        point: 20,
    };
}

/// The bundled fonts as the tile workers read them (as `ew_ui::canvas` does).
struct BundledFonts(&'static BTreeMap<FontKey, FontBytes>);

impl FontResolver for BundledFonts {
    fn font(&self, key: FontKey) -> Option<FontBytes> {
        self.0.font(key)
    }
}

/// A temporary directory with the Golden A sample project, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        let dir = std::env::temp_dir().join(format!("ew-canvas-bench-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a directory of the copy");
    for entry in std::fs::read_dir(from).expect("the golden directory") {
        let entry = entry.expect("a directory entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("a file of the copy");
        }
    }
}

/// Folio 103 of a copy of Golden A, as the canvas shows it.
fn golden_page(scratch: &Scratch) -> Arc<CanvasPage> {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
    let dir = scratch.0.join("J-E-2416.ew");
    copy_dir(&golden, &dir);
    let config = ShellConfig::in_dir(&scratch.0.join("config"), "bench");
    let mut shell = ShellModel::new(Mode::Headless, config);
    let opened = shell.dispatch(
        "file.open",
        &json!({"dir": dir.to_string_lossy(), "read_only": true}),
    );
    assert_eq!(opened, Outcome::Done, "opening the golden copy");
    let went = shell.dispatch("page.goto", &json!({"folio": "103"}));
    assert_eq!(went, Outcome::Done, "going to folio 103");
    let page = Arc::clone(shell.canvas().expect("folio 103 has a canvas").page());
    shell.shutdown();
    page
}

/// The paths and glyph runs `items` draw, groups followed.
fn drawn_items(items: &[Item]) -> usize {
    items
        .iter()
        .map(|item| match &item.kind {
            ItemKind::Path(_) | ItemKind::Glyphs(_) => 1,
            ItemKind::Group(g) => drawn_items(&g.items),
            ItemKind::Image(_) | ItemKind::Instance(_) => 0,
        })
        .sum()
}

/// An A3 landscape page of translated copies of `base`'s list, spread over the sheet, until at least
/// [`DENSE_MIN_ITEMS`] paths and glyph runs are drawn.
fn dense_page(base: &CanvasPage) -> CanvasPage {
    let per_copy = drawn_items(&base.list.items);
    assert!(per_copy > 0, "folio 103 draws something");
    let copies = DENSE_MIN_ITEMS.div_ceil(per_copy).max(1);
    let cols = (1..=copies).find(|c| c * c >= copies).unwrap_or(1);
    let rows = copies.div_ceil(cols);
    // The copies are spread over the room the A3 sheet has beyond folio 103's sheet.
    let slack = (
        (A3_LANDSCAPE.0 - base.sheet.0).max(0),
        (A3_LANDSCAPE.1 - base.sheet.1).max(0),
    );
    let spread = |slack: i64, i: usize, n: usize| slack * i as i64 / (n.max(2) - 1) as i64;
    let mut list = DisplayList::new();
    for i in 0..copies {
        let at = Pt2::new(
            Nm(spread(slack.0, i % cols, cols)),
            Nm(spread(slack.1, i / cols, rows)),
        );
        list.push(Item::group(
            Some(Transform::from_translation(at)),
            None,
            base.list.items.clone(),
        ));
    }
    CanvasPage::new(base.rev, "A3", "dense", A3_LANDSCAPE, Arc::new(list), base.grid)
}

/// One step of a gesture script.
#[derive(Clone, Copy)]
enum Step {
    /// `view.pan` by points.
    Pan(f64, f64),
    /// `view.zoom_to` a factor about a sheet point.
    ZoomTo(f64, (i64, i64)),
    /// `canvas.pointer` at a sheet point.
    Pointer((i64, i64)),
}

/// The pan script: `n` steps of [`PAN_STEP_POINTS`], the direction turning every [`PAN_RUN`] steps.
fn pan_script(n: usize) -> Vec<Step> {
    let d = std::f64::consts::FRAC_1_SQRT_2 * PAN_STEP_POINTS;
    let s = PAN_STEP_POINTS;
    let directions = [
        (s, 0.0),
        (d, d),
        (0.0, s),
        (-d, d),
        (-s, 0.0),
        (-d, -d),
        (0.0, -s),
        (d, -d),
    ];
    (0..n)
        .map(|i| {
            let (dx, dy) = directions[(i / PAN_RUN) % directions.len()];
            Step::Pan(dx, dy)
        })
        .collect()
}

/// The zoom factors of `n` frames: ladder runs in from [`ZOOM_LOW`] to [`ZOOM_HIGH`] and out again, each ladder
/// step eased over [`FRAMES_PER_LADDER_STEP`] frames.
fn zoom_path(n: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(n);
    let mut zoom = ZOOM_LOW;
    let mut up = true;
    while out.len() < n {
        let next = if up {
            zoom_in_step(zoom)
        } else {
            zoom_out_step(zoom)
        };
        for k in 1..=FRAMES_PER_LADDER_STEP {
            let t = k as f64 / FRAMES_PER_LADDER_STEP as f64;
            out.push(zoom * (next / zoom).powf(t));
        }
        zoom = next;
        if up && zoom >= ZOOM_HIGH * (1.0 - 1e-9) {
            up = false;
        } else if !up && zoom <= ZOOM_LOW * (1.0 + 1e-9) {
            up = true;
        }
    }
    out.truncate(n);
    out
}

/// The zoom script: [`zoom_path`] about an anchor that wanders over the sheet.
fn zoom_script(n: usize, sheet: (i64, i64)) -> Vec<Step> {
    zoom_path(n)
        .into_iter()
        .enumerate()
        .map(|(i, z)| {
            let at = |frac: f64| 0.5 + 0.4 * frac;
            let ax = at((i as f64 * 0.05).sin());
            let ay = at((i as f64 * 0.037).cos());
            Step::ZoomTo(z, ((sheet.0 as f64 * ax) as i64, (sheet.1 as f64 * ay) as i64))
        })
        .collect()
}

/// The pointer script: the centres of the page's entities, in a fixed scattered order (points over the sheet
/// when the page has none).
fn point_script(n: usize, page: &CanvasPage) -> Vec<Step> {
    (0..n)
        .map(|i| {
            let p = if page.entities.is_empty() {
                let f = (i % 20) as i64 + 1;
                (page.sheet.0 * f / 21, page.sheet.1 * f / 21)
            } else {
                rect_centre(page.entities[i * 7919 % page.entities.len()].1)
            };
            Step::Pointer(p)
        })
        .collect()
}

/// The UI-thread time of the four phases of one frame, in milliseconds.
#[derive(Clone, Copy, Default)]
struct FrameTimes {
    input: f64,
    scene: f64,
    render: f64,
    paint: f64,
}

impl FrameTimes {
    fn phase(&self, phase: Phase) -> f64 {
        match phase {
            Phase::Input => self.input,
            Phase::Scene => self.scene,
            Phase::Render => self.render,
            Phase::Paint => self.paint,
            Phase::Interval => 0.0,
        }
    }

    fn total(&self) -> f64 {
        self.input + self.scene + self.render + self.paint
    }
}

/// What one gesture measured.
struct Run {
    frames: Vec<FrameTimes>,
    /// The raster latency per frame (empty for the pointer gesture).
    latency: Vec<f64>,
}

fn millis(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// The canvas host of one gesture: the pieces of `ew_ui::canvas::CanvasHost` without egui.
struct Host {
    pool: TilePool,
    planner: Planner,
    cache: TileCache<()>,
    scene: Option<Arc<Scene>>,
    /// Finished tiles the wait took from the pool and the frame has not looked at yet.
    inbox: VecDeque<TileResult>,
    /// Converted tiles waiting for their upload (key, bytes).
    arrived: VecDeque<(TileKey, u64)>,
    /// Tiles that failed to render: they stay missing.
    failed: BTreeSet<TileKey>,
    submitted: Vec<TileKey>,
    frame: u64,
}

impl Host {
    fn new() -> Host {
        Host {
            pool: TilePool::new(worker_count(), Arc::new(|| {})),
            planner: Planner::new(),
            cache: TileCache::new(DEFAULT_CACHE_BYTES),
            scene: None,
            inbox: VecDeque::new(),
            arrived: VecDeque::new(),
            failed: BTreeSet::new(),
            submitted: Vec::new(),
            frame: 0,
        }
    }

    /// Takes the finished tiles of the current scene from the pool and uploads at most
    /// [`MAX_UPLOADS_PER_FRAME`] ([`MAX_UPLOAD_BYTES_PER_FRAME`]) of the waiting ones.
    fn take_arrivals(&mut self, scene: &Scene) {
        while let Some(done) = self.inbox.pop_front().or_else(|| self.pool.try_recv()) {
            if done.key.scene != scene.id() {
                continue;
            }
            match done.image {
                Ok(image) => {
                    let bytes = image.pixels.len() as u64;
                    // Stands in for the host's conversion to a texture image, a copy of the pixels.
                    black_box(image.pixels.clone());
                    self.arrived.push_back((done.key, bytes));
                }
                Err(_) => {
                    self.failed.insert(done.key);
                }
            }
        }
        let mut uploaded = 0usize;
        let mut bytes_up = 0u64;
        while uploaded < MAX_UPLOADS_PER_FRAME {
            let Some(&(_, bytes)) = self.arrived.front() else {
                break;
            };
            if uploaded > 0 && bytes_up + bytes > MAX_UPLOAD_BYTES_PER_FRAME as u64 {
                break;
            }
            let Some((key, bytes)) = self.arrived.pop_front() else {
                break;
            };
            self.cache.insert(key, (), bytes, self.frame);
            uploaded += 1;
            bytes_up += bytes;
        }
    }

    /// Submits the plan's missing tiles (in its priority order) when they differ from the last submission.
    fn request(&mut self, scene: &Arc<Scene>, requests: &[TileKey]) {
        let waiting: BTreeSet<TileKey> = self.arrived.iter().map(|(k, _)| *k).collect();
        let wanted: Vec<TileKey> = requests
            .iter()
            .filter(|k| !self.cache.contains(k) && !waiting.contains(k))
            .copied()
            .collect();
        if wanted == self.submitted {
            return;
        }
        self.pool.submit(scene, &wanted);
        self.submitted = wanted;
    }

    /// Whether the raster of `key` is done: uploaded, waiting for its upload or failed.
    fn rastered(&self, key: &TileKey) -> bool {
        self.cache.contains(key)
            || self.failed.contains(key)
            || self.arrived.iter().any(|(k, _)| k == key)
            || self.inbox.iter().any(|t| t.key == *key)
    }

    /// Blocks until the pool has rastered every one of `required` (no upload cap: this is raster latency).
    fn wait_for(&mut self, required: &[TileKey]) {
        let started = now();
        while !required.iter().all(|k| self.rastered(k)) {
            match self.pool.try_recv() {
                Some(done) => self.inbox.push_back(done),
                None => {
                    assert!(
                        now() - started < TILE_WAIT_LIMIT,
                        "the tiles of a frame did not arrive in {TILE_WAIT_LIMIT:?}"
                    );
                    std::thread::yield_now();
                }
            }
        }
    }
}

/// The paper size of a page.
fn sheet_of(page: &CanvasPage) -> SheetSize {
    SheetSize {
        width: Nm(page.sheet.0),
        height: Nm(page.sheet.1),
    }
}

/// Plays `steps` on a fresh canvas of `page` (made ready by `setup`) and times every frame; `wait` also measures
/// the raster latency of each frame.
fn run_gesture(
    page: &Arc<CanvasPage>,
    setup: impl FnOnce(&mut CanvasModel),
    steps: &[Step],
    wait: bool,
) -> Run {
    let mut model = CanvasModel::new(Arc::clone(page), PANE, PPP);
    setup(&mut model);
    let fonts: Arc<dyn FontResolver + Send + Sync> = Arc::new(BundledFonts(font_bytes()));
    let sheet = sheet_of(page);
    let grid = page.grid.map(|g| GridSpec {
        num_nm: g.num_nm,
        den: g.den,
        sheet_scale_ppb: g.sheet_scale_ppb,
        sheet_offset_nm: g.sheet_offset_nm,
    });
    let mut host = Host::new();
    let mut run = Run {
        frames: Vec::with_capacity(steps.len()),
        latency: Vec::new(),
    };
    for (i, step) in steps.iter().enumerate() {
        host.frame += 1;
        let t0 = now();

        // Phase 1: input (the registry actions the window maps its input to).
        match *step {
            Step::Pan(dx, dy) => model.pan_points(dx, dy),
            Step::ZoomTo(zoom, anchor) => model.zoom_about(zoom, anchor),
            Step::Pointer(p) => model.set_pointer(Some(p)),
        }
        let t1 = now();

        // Phase 2: scene (made once, reused after).
        let scene = Arc::clone(
            host.scene
                .get_or_insert_with(|| Scene::new(1, Arc::clone(&page.list), sheet, Arc::clone(&fonts))),
        );
        let t2 = now();

        // Phase 3: render.
        let view = View {
            centre: model.centre(),
            zoom: model.zoom(),
            pane: model.pane(),
            ppp: model.ppp(),
        };
        let plan = host
            .planner
            .plan(&view, sheet, scene.id(), i as f64 * FRAME_SECONDS);
        host.take_arrivals(&scene);
        host.request(&scene, &plan.requests);
        let t3 = now();

        // Phase 4: paint (the lookups of the placed tiles, the overlay, the eviction).
        let frame = host.frame;
        for layer in &plan.layers {
            for placed in layer {
                black_box(host.cache.get(&placed.key, frame));
            }
        }
        let overlay = overlay::build(&OverlayInput {
            transform: plan.transform,
            pane_px: view.pane_px_whole(),
            ppp: view.ppp,
            sheet,
            grid,
            hover: model.hover().map(|(_, r)| r),
            selected: &[],
        });
        black_box(&overlay);
        host.cache.evict(frame);
        let t4 = now();

        run.frames.push(FrameTimes {
            input: millis(t1 - t0),
            scene: millis(t2 - t1),
            render: millis(t3 - t2),
            paint: millis(t4 - t3),
        });
        if wait {
            let mut required: Vec<TileKey> = plan
                .layers
                .iter()
                .flatten()
                .map(|p| p.key)
                .filter(|k| k.dpi == plan.wanted_dpi)
                .collect();
            required.sort_unstable();
            required.dedup();
            host.wait_for(&required);
            run.latency.push(millis(now() - t2));
        }
    }
    run
}

/// Prints one `bench` line of `values` (sorted here): the nearest-rank median and p99.
fn bench_line(family: &str, mut values: Vec<f64>) {
    values.sort_by(f64::total_cmp);
    println!(
        "bench {family} median={:.3}ms p99={:.3}ms",
        nearest_rank(&values, 50.0),
        nearest_rank(&values, 99.0)
    );
    let _ = std::io::stdout().flush();
}

/// The frame family and its four phase families of one gesture.
fn frame_lines(name: &str, suffix: &str, frames: &[FrameTimes]) {
    bench_line(
        &format!("frame.gesture.{name}{suffix}"),
        frames.iter().map(FrameTimes::total).collect(),
    );
    for phase in [Phase::Input, Phase::Scene, Phase::Render, Phase::Paint] {
        bench_line(
            &format!("frame.gesture.{name}.{}{suffix}", phase.name()),
            frames.iter().map(|f| f.phase(phase)).collect(),
        );
    }
}

/// Measures the three gestures on `page` and prints its lines; `suffix` ends every family name.
fn measure_scene(label: &str, suffix: &str, page: &Arc<CanvasPage>, counts: Counts) {
    println!("scene {label} items={}", drawn_items(&page.list.items));
    let _ = std::io::stdout().flush();
    let centre = (page.sheet.0 / 2, page.sheet.1 / 2);

    let pan = run_gesture(
        page,
        |m| m.zoom_about(PAN_ZOOM, centre),
        &pan_script(counts.pan),
        true,
    );
    frame_lines("pan", suffix, &pan.frames);
    bench_line(&format!("render.pan.cpu{suffix}"), pan.latency);

    let zoom = run_gesture(
        page,
        |m| m.zoom_about(ZOOM_LOW, centre),
        &zoom_script(counts.zoom, page.sheet),
        true,
    );
    frame_lines("zoom", suffix, &zoom.frames);
    bench_line(&format!("render.zoom.cpu{suffix}"), zoom.latency);

    let point = run_gesture(page, |_| {}, &point_script(counts.point, page), false);
    frame_lines("point", suffix, &point.frames);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // `cargo nextest` lists the tests of a bench target with `--list`: there are none.
    if args.iter().any(|a| a == "--list") {
        return;
    }
    let quick = args.iter().any(|a| a == "--quick" || a == "--test");
    let counts = if quick { Counts::QUICK } else { Counts::FULL };
    println!("canvas benchmarks");
    println!(
        "class=GP generator_version=0 mode={}",
        if quick { "quick" } else { "full" }
    );
    let _ = std::io::stdout().flush();

    let scratch = Scratch::new();
    let golden = golden_page(&scratch);
    measure_scene("golden_a_103", "", &golden, counts);
    let dense = Arc::new(dense_page(&golden));
    measure_scene("dense_a3", ".dense_a3", &dense, counts);
}
