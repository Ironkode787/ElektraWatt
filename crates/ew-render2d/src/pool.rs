// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The raster worker pool (WP-S2.02 §6.3): tiles are rasterised off the UI thread by
//! `clamp(available_parallelism − 1, 1, 4)` workers.
//!
//! A [`Scene`] is one page content: its display list, sheet size and fonts, with the [`ItemBounds`] built once, on
//! the first worker that needs them. [`TilePool::submit`] replaces the queue with the newest plan's requests: a job
//! not in the newest plan is dropped before it starts (the generation counter counts the replacements), a running
//! job finishes, and its result comes back through [`TilePool::try_recv`] for the host to cache if its scene is
//! still current. Each finished tile calls the waker (the host's repaint request).

use std::collections::{BTreeSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread;

use ew_dl::{Color, DisplayList};
use ew_render_cpu::{
    FontResolver, ItemBounds, RegionOptions, RenderError, RgbaImage, SheetSize, render_region,
};

use crate::tiles::TileKey;

/// The fonts of a scene, shared by the workers.
pub type SharedFonts = Arc<dyn FontResolver + Send + Sync>;

/// One page content as the tile workers see it.
pub struct Scene {
    id: u64,
    list: Arc<DisplayList>,
    sheet: SheetSize,
    fonts: SharedFonts,
    bounds: OnceLock<ItemBounds>,
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("id", &self.id)
            .field("sheet", &self.sheet)
            .field("items", &self.list.items.len())
            .finish_non_exhaustive()
    }
}

impl Scene {
    /// The scene `id` (a number the host never reuses for other content) of `list` on `sheet`.
    pub fn new(id: u64, list: Arc<DisplayList>, sheet: SheetSize, fonts: SharedFonts) -> Arc<Scene> {
        Arc::new(Scene {
            id,
            list,
            sheet,
            fonts,
            bounds: OnceLock::new(),
        })
    }

    /// The scene's number.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The sheet size.
    pub fn sheet(&self) -> SheetSize {
        self.sheet
    }

    /// The display list.
    pub fn list(&self) -> &Arc<DisplayList> {
        &self.list
    }

    /// The fonts.
    pub fn fonts(&self) -> &SharedFonts {
        &self.fonts
    }

    /// The culling bounds of the list, built on first use.
    pub fn bounds(&self) -> &ItemBounds {
        self.bounds.get_or_init(|| ItemBounds::of(&self.list, None))
    }

    /// Rasterises tile `key` (whose scene must be this one): a region of the full-sheet raster at `key.dpi`, white
    /// paper, transparent outside the sheet (the host paints the surround).
    ///
    /// # Errors
    ///
    /// The renderer's [`RenderError`] (a tile outside the sheet has no pixels).
    pub fn render_tile(&self, key: TileKey) -> Result<RgbaImage, RenderError> {
        let (w, h) = key.size(self.sheet);
        let (x, y) = key.origin();
        let options = RegionOptions {
            dpi: key.dpi,
            x,
            y,
            width: w,
            height: h,
            background: Color::WHITE,
            surround: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
        };
        render_region(
            &self.list,
            Some(self.bounds()),
            self.sheet,
            &options,
            self.fonts.as_ref(),
            None,
        )
        .map(|(image, _)| image)
    }
}

/// A finished tile.
#[derive(Debug)]
pub struct TileResult {
    /// The tile.
    pub key: TileKey,
    /// Its pixels, or why it could not be rendered.
    pub image: Result<RgbaImage, RenderError>,
}

/// The number of tile workers of this machine: `clamp(available_parallelism − 1, 1, 4)`.
pub fn worker_count() -> usize {
    let n = thread::available_parallelism().map_or(1, std::num::NonZero::get);
    n.saturating_sub(1).clamp(1, 4)
}

/// One job of a [`WorkerPool`].
struct Queue<J> {
    jobs: VecDeque<J>,
    /// The keys of the jobs being worked on.
    running: BTreeSet<J>,
    generation: u64,
    dropped: u64,
    closed: bool,
}

type Shared<J> = Arc<(Mutex<Queue<J>>, Condvar)>;

fn lock<J>(shared: &Shared<J>) -> MutexGuard<'_, Queue<J>> {
    shared.0.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Worker threads over a queue that each submission replaces (latest plan wins). Generic over the job so that
/// the queue's rules are tested without rendering.
pub struct WorkerPool<J: Ord + Clone + Send + 'static> {
    shared: Shared<J>,
}

impl<J: Ord + Clone + Send + 'static> WorkerPool<J> {
    /// Starts `threads` workers (at least one) that run `handler` on one job at a time each.
    pub fn new(threads: usize, handler: impl Fn(J) + Send + Sync + 'static) -> WorkerPool<J> {
        let shared: Shared<J> = Arc::new((
            Mutex::new(Queue {
                jobs: VecDeque::new(),
                running: BTreeSet::new(),
                generation: 0,
                dropped: 0,
                closed: false,
            }),
            Condvar::new(),
        ));
        let handler = Arc::new(handler);
        for _ in 0..threads.max(1) {
            let theirs = Arc::clone(&shared);
            let handler = Arc::clone(&handler);
            // The threads are not joined: a render in progress ends by itself after the pool is dropped.
            thread::spawn(move || {
                loop {
                    let job = {
                        let mut q = lock(&theirs);
                        loop {
                            if q.closed {
                                return;
                            }
                            if let Some(job) = q.jobs.pop_front() {
                                q.running.insert(job.clone());
                                break job;
                            }
                            q = theirs.1.wait(q).unwrap_or_else(PoisonError::into_inner);
                        }
                    };
                    handler(job.clone());
                    lock(&theirs).running.remove(&job);
                }
            });
        }
        WorkerPool { shared }
    }

    /// Makes `jobs` (in priority order) the queue: waiting jobs not among them are dropped, jobs already running
    /// are not queued again, repeats are queued once.
    pub fn submit(&self, jobs: Vec<J>) {
        let mut q = lock(&self.shared);
        let wanted: BTreeSet<&J> = jobs.iter().collect();
        let dropped = q.jobs.iter().filter(|j| !wanted.contains(j)).count() as u64;
        q.dropped += dropped;
        q.generation += 1;
        let mut queued = BTreeSet::new();
        let fresh: VecDeque<J> = jobs
            .into_iter()
            .filter(|j| !q.running.contains(j) && queued.insert(j.clone()))
            .collect();
        q.jobs = fresh;
        drop(q);
        self.shared.1.notify_all();
    }

    /// The jobs waiting or running.
    pub fn pending(&self) -> usize {
        let q = lock(&self.shared);
        q.jobs.len() + q.running.len()
    }

    /// Whether `job` is waiting or running.
    pub fn is_pending(&self, job: &J) -> bool {
        let q = lock(&self.shared);
        q.running.contains(job) || q.jobs.contains(job)
    }

    /// The number of submissions so far.
    pub fn generation(&self) -> u64 {
        lock(&self.shared).generation
    }

    /// The number of waiting jobs a submission dropped before they started.
    pub fn dropped(&self) -> u64 {
        lock(&self.shared).dropped
    }
}

impl<J: Ord + Clone + Send + 'static> Drop for WorkerPool<J> {
    fn drop(&mut self) {
        lock(&self.shared).closed = true;
        self.shared.1.notify_all();
    }
}

/// A tile job: the key orders it, the scene comes along.
#[derive(Clone)]
struct TileJob {
    key: TileKey,
    scene: Arc<Scene>,
}

impl PartialEq for TileJob {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for TileJob {}

impl PartialOrd for TileJob {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TileJob {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}

/// The tile workers of the canvas and the channel their results come back on.
pub struct TilePool {
    pool: WorkerPool<TileJob>,
    results: Receiver<TileResult>,
    /// The tiles sent back and not yet received: a submission skips them as it skips running jobs, so a tile that
    /// finished between the host's receive and its next submission is never rasterised twice. A submission holds
    /// this lock while it queues, and a worker takes it before it sends, so nothing slips between the two checks.
    delivered: Arc<Mutex<BTreeSet<TileKey>>>,
}

impl TilePool {
    /// Starts `threads` workers; `waker` runs after every finished tile (the host asks for a repaint).
    pub fn new(threads: usize, waker: Arc<dyn Fn() + Send + Sync>) -> TilePool {
        let (tx, results): (Sender<TileResult>, Receiver<TileResult>) = channel();
        let delivered: Arc<Mutex<BTreeSet<TileKey>>> = Arc::new(Mutex::new(BTreeSet::new()));
        let theirs = Arc::clone(&delivered);
        let pool = WorkerPool::new(threads, move |job: TileJob| {
            let image = job.scene.render_tile(job.key);
            // Marked delivered (under the lock a submission holds) before it is sent and before the job stops
            // counting as running.
            let mut marked = theirs.lock().unwrap_or_else(PoisonError::into_inner);
            marked.insert(job.key);
            let sent = tx.send(TileResult { key: job.key, image });
            drop(marked);
            // The receiver is gone when the canvas was dropped; nothing to wake then.
            if sent.is_ok() {
                waker();
            }
        });
        TilePool {
            pool,
            results,
            delivered,
        }
    }

    /// Makes `keys` of `scene` (in priority order) the queue, dropping waiting jobs of earlier plans.
    pub fn submit(&self, scene: &Arc<Scene>, keys: &[TileKey]) {
        let jobs: Vec<(Arc<Scene>, TileKey)> = keys.iter().map(|&key| (Arc::clone(scene), key)).collect();
        self.submit_many(&jobs);
    }

    /// Makes `jobs` (each a scene and one of its tiles, in priority order, of one or several scenes) the queue,
    /// dropping waiting jobs of earlier plans: the merged plan of several visible canvases (WP-S2.03 §6.8).
    pub fn submit_many(&self, jobs: &[(Arc<Scene>, TileKey)]) {
        let delivered = self.delivered.lock().unwrap_or_else(PoisonError::into_inner);
        self.pool.submit(
            jobs.iter()
                .filter(|(_, key)| !delivered.contains(key))
                .map(|(scene, key)| TileJob {
                    key: *key,
                    scene: Arc::clone(scene),
                })
                .collect(),
        );
    }

    /// A finished tile, if one is waiting.
    pub fn try_recv(&self) -> Option<TileResult> {
        let done = self.results.try_recv().ok()?;
        self.received(done.key);
        Some(done)
    }

    /// A finished tile, waiting at most `timeout` for one.
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Option<TileResult> {
        let done = self.results.recv_timeout(timeout).ok()?;
        self.received(done.key);
        Some(done)
    }

    fn received(&self, key: TileKey) {
        self.delivered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&key);
    }

    /// The tiles waiting or being rasterised.
    pub fn pending(&self) -> usize {
        self.pool.pending()
    }

    /// The number of submissions so far.
    pub fn generation(&self) -> u64 {
        self.pool.generation()
    }

    /// The number of waiting tiles a newer plan dropped before they started.
    pub fn dropped(&self) -> u64 {
        self.pool.dropped()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use super::*;

    #[test]
    fn worker_count_is_between_one_and_four() {
        assert!((1..=4).contains(&worker_count()));
    }

    #[test]
    fn a_newer_plan_drops_the_waiting_jobs_of_an_older_one() {
        let (done_tx, done_rx) = channel();
        let (started_tx, started_rx) = channel();
        let (release_tx, release_rx) = channel::<()>();
        let release_rx = Mutex::new(release_rx);
        let pool = WorkerPool::new(1, move |n: u32| {
            // Job 0 holds the only worker until the test has submitted the newer plan: no timing.
            if n == 0 {
                started_tx.send(()).unwrap();
                release_rx.lock().unwrap().recv().unwrap();
            }
            done_tx.send(n).unwrap();
        });
        pool.submit(vec![0]);
        started_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("job 0 started");
        pool.submit((1..=10).collect());
        assert_eq!(pool.pending(), 11);
        // The newest plan keeps 5 and adds 20, 21; 0 is running and is not queued again.
        pool.submit(vec![20, 5, 0, 21, 5]);
        assert_eq!(pool.generation(), 3);
        assert_eq!(
            pool.dropped(),
            9,
            "1..=10 without 5 were dropped before they started"
        );
        assert!(pool.is_pending(&0) && pool.is_pending(&21) && !pool.is_pending(&3));
        release_tx.send(()).unwrap();
        let mut done = Vec::new();
        while let Ok(n) = done_rx.recv_timeout(Duration::from_secs(10)) {
            done.push(n);
            if done.len() == 4 {
                break;
            }
        }
        assert_eq!(
            done,
            [0, 20, 5, 21],
            "the running job finished, then the newest plan in its order"
        );
        // Nothing else comes.
        assert!(done_rx.recv_timeout(Duration::from_millis(100)).is_err());
    }

    struct NoFonts;

    impl FontResolver for NoFonts {
        fn font(&self, _: ew_dl::FontKey) -> Option<ew_render_cpu::FontBytes> {
            None
        }
    }

    #[test]
    fn a_tile_sent_back_and_not_yet_received_is_not_rasterised_again() {
        use ew_dl::ew_units::Nm;
        let sheet = SheetSize {
            width: Nm::from_mm(20),
            height: Nm::from_mm(20),
        };
        let scene = Scene::new(1, Arc::new(DisplayList::new()), sheet, Arc::new(NoFonts));
        let pool = TilePool::new(1, Arc::new(|| {}));
        let key = TileKey {
            scene: 1,
            dpi: 96,
            tx: 0,
            ty: 0,
        };
        pool.submit(&scene, &[key]);
        // Wait until the job has finished: it is sent back, not received.
        for _ in 0..2000 {
            if pool.pending() == 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(pool.pending(), 0);
        pool.submit(&scene, &[key]);
        assert_eq!(pool.pending(), 0, "the delivered tile is not queued again");
        let done = pool.recv_timeout(Duration::from_secs(10)).expect("the tile");
        assert_eq!(done.key, key);
        assert!(pool.try_recv().is_none(), "rasterised once");
        // Received: a new plan may ask for it again (the host's cache decides).
        pool.submit(&scene, &[key]);
        assert!(pool.recv_timeout(Duration::from_secs(10)).is_some());
    }

    #[test]
    fn one_submission_merges_the_tiles_of_two_scenes() {
        use ew_dl::ew_units::Nm;
        let sheet = SheetSize {
            width: Nm::from_mm(20),
            height: Nm::from_mm(20),
        };
        let a = Scene::new(1, Arc::new(DisplayList::new()), sheet, Arc::new(NoFonts));
        let b = Scene::new(2, Arc::new(DisplayList::new()), sheet, Arc::new(NoFonts));
        let key = |scene: u64| TileKey {
            scene,
            dpi: 96,
            tx: 0,
            ty: 0,
        };
        let pool = TilePool::new(2, Arc::new(|| {}));
        pool.submit_many(&[(Arc::clone(&a), key(1)), (Arc::clone(&b), key(2))]);
        let mut got = BTreeSet::new();
        for _ in 0..2 {
            let done = pool.recv_timeout(Duration::from_secs(10)).expect("a tile");
            assert!(done.image.is_ok());
            got.insert(done.key);
        }
        assert_eq!(got, BTreeSet::from([key(1), key(2)]));
    }

    #[test]
    fn a_dropped_pool_stops_its_workers() {
        let (tx, rx) = channel::<u32>();
        let pool = WorkerPool::new(2, move |n: u32| tx.send(n).unwrap());
        pool.submit(vec![1]);
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(1));
        drop(pool);
        // The workers have ended and dropped the handler (and its sender).
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(5)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}
