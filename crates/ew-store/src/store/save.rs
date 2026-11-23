// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The store's create and save (doc 05 §5): a new project's first generation, the save protocol with its
//! commit point, a pending save completed, and the plan and the bytes a save writes.

use super::*;

impl Store {
    // -----------------------------------------------------------------------------------------------------
    // Create

    /// Writes `model` as generation 1 into `root` (a new project, Save As), which must not hold a
    /// manifest, and keeps it open for editing. It deletes the leftover journals of its project ID at `root`: they
    /// are of a project that was there before (doc 05 §8.1). Inside a git worktree it also writes the managed `.gitattributes` and
    /// `.gitignore` blocks ([`crate::git::write_blocks`]).
    ///
    /// # Errors
    ///
    /// [`OpenError::Exists`] when `root` already holds a project; I/O and lock errors; a save error as
    /// [`OpenError::Format`]; [`OpenError::Model`] for a model that fails its integrity check (a read-only project
    /// whose damaged library copy a record names), before anything is written.
    pub fn create(root: &Path, model: &Model, opts: StoreOptions) -> Result<Store, OpenError> {
        model.integrity_check().map_err(|error| OpenError::Model {
            error,
            findings: Vec::new(),
        })?;
        let vfs = Arc::clone(&opts.vfs);
        vfs.create_dir_all(root).map_err(io_err(root))?;
        if vfs.stat(&join(root, MANIFEST)).is_ok() {
            return Err(OpenError::Exists(root.to_path_buf()));
        }
        let Opened { mut store, .. } = Store::open_inner_empty(root, opts)?;
        // Records of reserved or unknown kinds without a source file: placed by their rule.
        for record in model.unknown_records() {
            let Some(file) = unknown_record_file(match record {
                ew_format::ew_schema::Record::Unknown(v) => v,
                _ => continue,
            }) else {
                return Err(OpenError::Format(format!(
                    "no file for a record of kind {:?}",
                    record.kind_name()
                )));
            };
            let line = ew_format::record_line(record).map_err(|e| OpenError::Format(e.to_string()))?;
            let key = LineKey::of(record).unwrap_or_else(LineKey::keyless);
            store.kept.entry(file).or_default().push((key, line.into_bytes()));
        }
        store.all_assets = true;
        let saved = store.save(model, Changes::All);
        store.all_assets = false;
        saved.map_err(|e| OpenError::Format(e.to_string()))?;
        // Inside a git worktree the project gets the managed `.gitattributes` and `.gitignore` blocks (doc 05
        // §11.1); they are not project data, so a failure to write them leaves the project as created.
        if crate::git::worktree_root(&*vfs, root).is_ok_and(|w| w.is_some()) {
            let _ = crate::git::write_blocks(&*vfs, root);
        }
        store.prepare_journal_dir(Some(model.project().id.cast()));
        if let Some(j) = &mut store.journal {
            // A leftover journal of this project ID at this path is of a project that was here before (`root` held
            // no project): nothing of it applies to the new one, so it is not offered to it (doc 05 §8.1).
            j.discard(&std::mem::take(&mut store.leftovers));
            if j.start(store.generation, store.manifest_hash.as_ref()).is_err() {
                store.journal = None;
            }
        }
        Ok(store)
    }

    /// An editing store over a directory without a project.
    pub(super) fn open_inner_empty(root: &Path, opts: StoreOptions) -> Result<Opened, OpenError> {
        let held = opts.lock(root).acquire().map_err(io_err(root))?;
        let access = match held {
            Acquire::Held => Access::Edit,
            Acquire::HeldBy(info) => Access::ReadOnly(ReadOnlyReason::LockedBy(Box::new(info))),
            Acquire::NotWritable(why) => Access::ReadOnly(ReadOnlyReason::NotWritable(why)),
        };
        let store = Store::blank(root, opts, access.clone(), access == Access::Edit);
        if let Access::ReadOnly(reason) = access {
            return Err(OpenError::Format(format!("cannot create: {reason:?}")));
        }
        let model = store.base.clone();
        Ok(Opened {
            store,
            model,
            report: OpenReport {
                access: Access::Edit,
                generation: 0,
                format: FormatAccess::ReadWrite,
                findings: Vec::new(),
                rolled_forward: None,
                removed: Vec::new(),
                recovery: None,
                journaled: false,
                migrated_from: None,
            },
        })
    }

    // -----------------------------------------------------------------------------------------------------
    // Save

    /// Saves `model` (doc 05 §5.2): serialise the files of the dirty set, drop those equal to their
    /// hashes, check that every file it replaces or removes is still as loaded or written, then temps with
    /// `sync_all`, the intent manifest, directory syncs, `save-begin` in the journal (the commit point),
    /// renames, the manifest rename, removals, `save-done`. An unchanged project writes nothing.
    ///
    /// # Errors
    ///
    /// [`SaveError`]: before the commit point nothing on disk changed; after it the generation is decided
    /// and completes by roll-forward.
    pub fn save(&mut self, model: &Model, changes: Changes<'_>) -> Result<SaveReport, SaveError> {
        self.save_with(model, changes, &BTreeSet::new())
    }

    /// [`Store::save`] that also removes the opaque files `drop` (the asset purge, doc 05 §13.3): they leave the
    /// manifest of the new generation, are journalled in `save-begin{removes}` and deleted in step 8 with the
    /// save's other removals, so a crash completes their removal by roll-forward or leaves the old generation.
    /// A path of `drop` that is not an opaque file of the baseline is ignored.
    pub(super) fn save_with(
        &mut self,
        model: &Model,
        changes: Changes<'_>,
        drop: &BTreeSet<FilePath>,
    ) -> Result<SaveReport, SaveError> {
        self.save_inner(model, changes, drop, false)
    }

    /// [`Store::save_with`] that also discards the broken lines when `discard` is set ([`Store::discard_broken_lines`]):
    /// their files join the plan's rewrite set and are rendered without them, and they leave the kept lines in
    /// [`Store::adopt`], never before.
    pub(super) fn save_inner(
        &mut self,
        model: &Model,
        changes: Changes<'_>,
        drop: &BTreeSet<FilePath>,
        discard: bool,
    ) -> Result<SaveReport, SaveError> {
        let on_disk = self.pre_check()?;

        // Step 3: serialise the dirty files.
        let mut plan = self.plan(model, changes, discard)?;
        let drop: BTreeSet<FilePath> = drop
            .iter()
            .filter(|p| self.opaque.contains_key(*p) && !plan.writes.contains_key(*p))
            .cloned()
            .collect();
        plan.removes.extend(drop.iter().cloned());
        // The rest of step 2: every file this save replaces or removes still holds the bytes we loaded or
        // wrote, else it changed outside the session and nothing is written. Before any temp and any move-aside.
        let changed = self.changed_targets(&plan)?;
        if !changed.is_empty() {
            return Err(SaveError::ExternalChange { files: changed });
        }
        let g = self.generation;
        let g1 = g.saturating_add(1);
        let (manifest, manifest_same) = self.next_manifest(model, &plan, &drop, g1, on_disk.as_ref());
        if plan.writes.is_empty() && plan.removes.is_empty() && manifest_same {
            self.adopt(model, plan.contents, plan.disk, discard);
            return Ok(SaveReport {
                generation: g,
                findings: std::mem::take(&mut self.notes),
                ..SaveReport::default()
            });
        }
        // The manifest's ceiling: a generation above it is one the next open refuses, so a save that would
        // write one is refused here, beside D5's pre-check and before any temp or move-aside. A save with nothing to
        // write (above) stays allowed.
        if g1 > ew_format::MAX_GENERATION {
            return Err(SaveError::GenerationLimit);
        }
        let manifest_text = manifest.to_text();
        let vfs = Arc::clone(&self.opts.vfs);

        let dirs = self.write_temps(&plan, g1, &manifest_text)?;
        let mut report = SaveReport {
            generation: g1,
            written: plan.writes.keys().cloned().collect(),
            removed: Vec::new(),
            manifest_written: true,
            leftovers: Vec::new(),
            journal_error: None,
            findings: Vec::new(),
            unstable: manifest.unstable,
        };
        let removes: Vec<FilePath> = plan.removes.iter().cloned().collect();
        if let Some(j) = &mut self.journal {
            if let Err(e) = j.save_begin(g1, plan.writes.len(), &removes) {
                report.journal_error = Some(e.to_string());
            }
        }
        // Commit point: generation g1 is decided.
        let manifest_hash = sha256(manifest_text.as_bytes());
        self.opaque.retain(|p, _| !drop.contains(p));
        self.opaque.extend(plan.assets);
        self.adopt(model, plan.contents, plan.disk, discard);
        self.manifest = Some(manifest);
        self.manifest_hash = Some(manifest_hash);
        self.manifest_bytes = Some(manifest_text.into_bytes());
        self.migrated = None;
        self.generation = g1;
        self.pending = Some(g1);
        self.pending_removes.clone_from(&removes);

        self.rename_into_place(&plan.writes, &dirs, g1)?;
        self.pending = None;
        self.pending_removes.clear();
        // Step 8: removals.
        for path in removes {
            match vfs.remove_file(&join(&self.root, path.as_str())) {
                Ok(()) => report.removed.push(path),
                Err(e) if e.kind() == io::ErrorKind::NotFound => report.removed.push(path),
                Err(_) => report.leftovers.push(path),
            }
        }
        // Step 9: bookkeeping.
        if let Some(j) = &mut self.journal {
            if let Err(e) = j.save_done(g1, &manifest_hash) {
                report.journal_error = Some(e.to_string());
            }
        }
        report.findings = std::mem::take(&mut self.notes);
        Ok(report)
    }

    /// What a save checks before it writes anything (doc 05 §5.2 step 2): the session may write and has no save
    /// still pending, the lock holds our session, and the manifest is what we last loaded or wrote. Gives the SHA-256
    /// of the manifest on disk (`None`: absent).
    pub(super) fn pre_check(&mut self) -> Result<Option<[u8; 32]>, SaveError> {
        if let Access::ReadOnly(reason) = &self.access {
            return Err(SaveError::ReadOnly(reason.clone()));
        }
        // The first save of a migrated project converts it: only with the user's consent, whoever saves.
        if let Some(migrated) = self.migrated.as_ref().filter(|m| !m.confirmed) {
            return Err(SaveError::UpgradeConsent {
                from: migrated.from.clone(),
            });
        }
        // An earlier save that is still incomplete must complete first; when it cannot, this save writes
        // nothing and says so with its own error, never with the earlier save's `Incomplete` (the caller
        // would count this model as saved).
        if self.pending.is_some() {
            if let Err(e) = self.retry_pending() {
                return Err(match e {
                    SaveError::Incomplete {
                        generation, error, ..
                    } => SaveError::PendingIncomplete { generation, error },
                    other => other,
                });
            }
        }
        // Step 2: the lock still holds our session, and the manifest is what we last loaded or wrote.
        match self.opts.lock(&self.root).holder() {
            Ok(Holder::Us) => {}
            Ok(Holder::Other(rec)) => return Err(self.lose_lock(rec)),
            Ok(Holder::Nobody) => return Err(self.lose_lock(None)),
            Err(error) => {
                return Err(SaveError::Failed {
                    step: SaveStep::PreCheck,
                    path: join(&self.root, ew_format::LOCK),
                    error,
                });
            }
        }
        let manifest_path = join(&self.root, MANIFEST);
        let on_disk = match self.opts.vfs.read(&manifest_path) {
            Ok(b) => Some(sha256(&b)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(SaveError::Failed {
                    step: SaveStep::PreCheck,
                    path: manifest_path,
                    error,
                });
            }
        };
        if on_disk != self.manifest_hash {
            return Err(SaveError::ExternalChange {
                files: FilePath::new(MANIFEST).into_iter().collect(),
            });
        }
        Ok(on_disk)
    }

    /// The manifest of generation `g1` for the files of `plan` (doc 05 §4): the stamps (a newer minor keeps its own),
    /// the members a migration or the previous manifest carries, every file without `drop`, and the base the save
    /// replaces. Also whether it equals the manifest on disk but for the generation, the generator and the base.
    pub(super) fn next_manifest(
        &self,
        model: &Model,
        plan: &SavePlan,
        drop: &BTreeSet<FilePath>,
        g1: u64,
        on_disk: Option<&[u8; 32]>,
    ) -> (Manifest, bool) {
        let mut manifest = Manifest::new(
            FORMAT,
            &self.opts.identity.app_version,
            model.project().id.cast(),
            g1,
        );
        manifest.unstable = unstable_flag(
            self.manifest.as_ref().filter(|_| self.keep_format),
            self.opts.released,
        );
        if self.keep_format {
            if let Some(old) = &self.manifest {
                // A newer minor keeps its stamps.
                manifest.format.clone_from(&old.format);
                manifest.min_reader.clone_from(&old.min_reader);
            }
        }
        // A migrated project's members other than the stamps and the files are the manifest steps'.
        let old_manifest = match &self.migrated {
            Some(migrated) => Some(&migrated.manifest),
            None => self.manifest.as_ref(),
        };
        if let Some(old) = old_manifest {
            manifest.extra.clone_from(&old.extra);
            // The base of the previous save; this save records its own below.
            manifest.extra.remove(BASE_MEMBER);
        }
        manifest.files = plan
            .disk
            .iter()
            .map(|(p, h)| (p.clone(), FileHash::Sha256(h.hash)))
            .chain(
                self.opaque
                    .iter()
                    .filter(|(p, _)| !drop.contains(*p))
                    .map(|(p, h)| (p.clone(), *h)),
            )
            .chain(plan.assets.iter().map(|(p, h)| (p.clone(), *h)))
            .collect();
        let manifest_same = self.manifest.as_ref().is_some_and(|old| {
            let mut a = old.clone();
            a.generation = g1;
            a.generator.clone_from(&manifest.generator);
            a.extra.remove(BASE_MEMBER);
            a == manifest
        });
        // The raw bytes this save is based on, for the base check of a roll-forward:
        // absent when there is no manifest.
        if let Some(h) = on_disk {
            manifest
                .extra
                .insert(BASE_MEMBER.to_owned(), serde_json::Value::from(hash_text(h)));
        }
        (manifest, manifest_same)
    }

    /// Steps 4 and 5 of the save (doc 05 §5.2): the temps of `plan`'s files and the intent `manifest_text` of
    /// generation `g1`, synced with their directories; a file the baseline does not list is moved aside first. Any
    /// failure deletes our temps; the generation on disk stays intact. Gives the directories the save writes in.
    pub(super) fn write_temps(
        &mut self,
        plan: &SavePlan,
        g1: u64,
        manifest_text: &str,
    ) -> Result<BTreeSet<PathBuf>, SaveError> {
        let vfs = Arc::clone(&self.opts.vfs);
        // Steps 4 and 5: temps, intent, directory syncs. Any failure deletes our temps; g stays intact.
        let mut created: Vec<PathBuf> = Vec::new();
        let mut moved: Vec<Finding> = Vec::new();
        let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
        dirs.insert(self.root.clone());
        let pre = (|| -> Result<(), (SaveStep, PathBuf, io::Error)> {
            for (path, bytes) in &plan.writes {
                let target = join(&self.root, path.as_str());
                let mut dir = target.parent().map(Path::to_path_buf);
                if let Some(d) = &dir {
                    vfs.create_dir_all(d)
                        .map_err(|e| (SaveStep::Temps, d.clone(), e))?;
                }
                while let Some(d) = dir {
                    if d == self.root || !d.starts_with(&self.root) {
                        break;
                    }
                    dir = d.parent().map(Path::to_path_buf);
                    dirs.insert(d);
                }
                // Never rename over a file the baseline does not list (another user's file, a sync
                // client's copy): move it aside first. Its directory is
                // synced with the temps below, so the move is durable before the commit point.
                if !self.disk.contains_key(path) {
                    match vfs.stat(&target) {
                        Ok(s) if matches!(s.kind, EntryKind::File | EntryKind::Link) => moved.push(
                            self.move_aside(path, g1)
                                .map_err(|e| (SaveStep::Temps, target.clone(), e))?,
                        ),
                        // A directory or a device: the rename of step 6 fails and reports it.
                        Ok(_) => {}
                        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                        Err(e) => return Err((SaveStep::Temps, target.clone(), e)),
                    }
                }
                let temp = join(&self.root, &path.temp(g1));
                let _ = vfs.remove_file(&temp);
                vfs.create_new(&temp, bytes)
                    .map_err(|e| (SaveStep::Temps, temp.clone(), e))?;
                created.push(temp.clone());
                vfs.sync(&temp).map_err(|e| (SaveStep::Temps, temp.clone(), e))?;
            }
            let intent = join(&self.root, &format!("{MANIFEST}.tmp-{g1}"));
            let _ = vfs.remove_file(&intent);
            vfs.create_new(&intent, manifest_text.as_bytes())
                .map_err(|e| (SaveStep::Intent, intent.clone(), e))?;
            created.push(intent.clone());
            vfs.sync(&intent)
                .map_err(|e| (SaveStep::Intent, intent.clone(), e))?;
            // Subdirectories first, the root (which holds the intent) last: a durable intent name implies
            // durable temp names, so a power loss never leaves an intent whose temps are gone.
            for d in dirs
                .iter()
                .filter(|d| **d != self.root)
                .chain(std::iter::once(&self.root))
            {
                vfs.sync_dir(d).map_err(|e| (SaveStep::Intent, d.clone(), e))?;
            }
            Ok(())
        })();
        // Files moved aside stay moved whatever happens next; the next successful save reports them.
        self.notes.append(&mut moved);
        if let Err((step, path, error)) = pre {
            for temp in created.iter().rev() {
                let _ = vfs.remove_file(temp);
            }
            // Make the removal of a possibly durable intent durable too, so that a later power loss cannot
            // bring back a generation this session reported as not saved.
            let _ = vfs.sync_dir(&self.root);
            return Err(SaveError::Failed { step, path, error });
        }
        Ok(dirs)
    }

    /// Steps 6 and 7 of the save (doc 05 §5.2), after its commit point: the temps of `writes` renamed over their
    /// files, the directories `dirs` synced, then the intent renamed over the manifest and the root synced.
    pub(super) fn rename_into_place(
        &self,
        writes: &BTreeMap<FilePath, Vec<u8>>,
        dirs: &BTreeSet<PathBuf>,
        g1: u64,
    ) -> Result<(), SaveError> {
        let vfs = &*self.opts.vfs;
        let manifest_path = join(&self.root, MANIFEST);
        // Step 6: renames.
        for path in writes.keys() {
            let temp = join(&self.root, &path.temp(g1));
            let target = join(&self.root, path.as_str());
            if let Err(error) = self.rename_retrying(&temp, &target) {
                return Err(SaveError::Incomplete {
                    generation: g1,
                    step: SaveStep::Renames,
                    error,
                });
            }
        }
        // The renamed names are made durable before the manifest names the new generation: with
        // per-directory sync semantics (ext4, APFS) a durable manifest rename must never precede a durable
        // data-file rename, or a power loss could leave manifest g+1 over files of g with no intent left.
        for d in dirs.iter().filter(|d| **d != self.root) {
            if let Err(error) = vfs.sync_dir(d) {
                return Err(SaveError::Incomplete {
                    generation: g1,
                    step: SaveStep::Renames,
                    error,
                });
            }
        }
        // Step 7: the manifest, then the root directory (which also holds the renamed root files).
        let intent = join(&self.root, &format!("{MANIFEST}.tmp-{g1}"));
        if let Err(error) = self.rename_retrying(&intent, &manifest_path) {
            return Err(SaveError::Incomplete {
                generation: g1,
                step: SaveStep::Manifest,
                error,
            });
        }
        if let Err(error) = vfs.sync_dir(&self.root) {
            return Err(SaveError::Incomplete {
                generation: g1,
                step: SaveStep::Manifest,
                error,
            });
        }
        Ok(())
    }

    /// Completes a save that passed its commit point (doc 05 §5.4: retried every 5 s by the caller).
    ///
    /// # Errors
    ///
    /// [`SaveError::Incomplete`] while it still fails.
    pub fn retry_pending(&mut self) -> Result<Option<u64>, SaveError> {
        let Some(g) = self.pending else {
            return Ok(None);
        };
        let incomplete = |error| SaveError::Incomplete {
            generation: g,
            step: SaveStep::Renames,
            error,
        };
        let done = |store: &mut Store| {
            store.pending = None;
            for path in std::mem::take(&mut store.pending_removes) {
                let _ = store.opts.vfs.remove_file(&join(&store.root, path.as_str()));
            }
            if let (Some(j), Some(h)) = (&mut store.journal, store.manifest_hash) {
                let _ = j.save_done(g, &h);
            }
            Ok(Some(g))
        };
        // The manifest rename happened (`manifest.json` holds the bytes of our intent); only a directory
        // sync failed.
        let decided = self.manifest_hash.as_ref().map(hash_text);
        if decided.is_some() && self.manifest_base().ok().flatten() == decided {
            if let Err(e) = self.opts.vfs.sync_dir(&self.root) {
                return Err(incomplete(e));
            }
            return done(self);
        }
        // Like the pre-check of a save, the roll-forward requires the raw manifest the save started from
        // (its intent's recorded base); otherwise an external change happened and
        // completing the save would overwrite it.
        match self.roll_forward(g) {
            Ok(Roll::Completed(_, moved)) => {
                self.notes.extend(moved);
                done(self)
            }
            Ok(Roll::Torn) => Err(incomplete(io::Error::other("the intent manifest is unreadable"))),
            Ok(Roll::NotRepairable(files)) => Err(incomplete(io::Error::other(format!(
                "{} files match neither the intent nor a temp (EW-CORE-002)",
                files.len()
            )))),
            Ok(Roll::BaseMoved(mismatch)) => {
                Err(incomplete(io::Error::other(base_moved(g, &mismatch).message)))
            }
            Err(e) => Err(incomplete(e)),
        }
    }

    pub(super) fn lose_lock(&mut self, rec: Option<LockRecord>) -> SaveError {
        self.holds_lock = false;
        let rec = rec.map(Box::new);
        self.access = Access::ReadOnly(ReadOnlyReason::LockLost(rec.clone()));
        SaveError::LockLost(rec)
    }

    /// The files of `plan` that the save replaces or removes and that changed on disk since we loaded or wrote them,
    /// in path order: each is compared with the hashes we hold ([`unchanged_on_disk`]), for record files
    /// and library documents (`disk`) the raw hash and the LF form's, so that their text is compared, for
    /// opaque files and assets (`opaque`) the one hash. A new target (one we hold no hash for) is not compared: the save
    /// moves it aside when it is there.
    ///
    /// # Errors
    ///
    /// [`SaveError::Failed`] at [`SaveStep::PreCheck`] when a file cannot be read.
    pub(super) fn changed_targets(&self, plan: &SavePlan) -> Result<Vec<FilePath>, SaveError> {
        let targets: BTreeSet<&FilePath> = plan.writes.keys().chain(&plan.removes).collect();
        let mut changed = Vec::new();
        for path in targets {
            let held = match (self.disk.get(path), self.opaque.get(path)) {
                // A record file or library document: its raw hash or its LF form's.
                (Some(held), _) => held.accepted(),
                (None, Some(FileHash::Sha256(hash))) => vec![*hash],
                // A hash we do not know cannot show the file unchanged.
                (None, Some(FileHash::Pending)) => {
                    changed.push(path.clone());
                    continue;
                }
                (None, None) => continue,
            };
            let full = join(&self.root, path.as_str());
            match unchanged_on_disk(&*self.opts.vfs, &full, &held) {
                Ok(true) => {}
                Ok(false) => changed.push(path.clone()),
                Err(error) => {
                    return Err(SaveError::Failed {
                        step: SaveStep::PreCheck,
                        path: full,
                        error,
                    });
                }
            }
        }
        Ok(changed)
    }

    /// Makes `model` the baseline after a successful plan: at the commit point of a save, or for a save with nothing to
    /// write. With `discard` (a repair's save, [`Store::discard_broken_lines`]) the broken lines leave the kept lines
    /// here, and only here, so that a save that fails before its commit point keeps every one of them; a file left with
    /// no other quarantined line leaves the quarantined files.
    pub(super) fn adopt(
        &mut self,
        model: &Model,
        contents: BTreeMap<FilePath, BTreeSet<AnyKey>>,
        disk: BTreeMap<FilePath, Held>,
        discard: bool,
    ) {
        for (path, keys) in contents {
            if keys.is_empty() {
                self.contents.remove(&path);
            } else {
                self.contents.insert(path, keys);
            }
        }
        self.disk = disk;
        self.rewrite.clear();
        self.base = model.clone();
        if discard {
            for (file, broken) in std::mem::take(&mut self.broken) {
                if let Some(lines) = self.kept.get_mut(&file) {
                    lines.retain(|(key, _)| !broken.lines.contains_key(key));
                    if lines.is_empty() {
                        self.kept.remove(&file);
                    }
                }
                if !broken.other_quarantined {
                    self.quarantined.remove(&file);
                }
            }
        }
    }

    /// Step 3 of doc 05 §5.2: the bytes of every dirty file and the files to remove. With `discard` (a repair's save)
    /// the files that hold broken lines join the dirty files, rendered without those lines.
    pub(super) fn plan(&self, model: &Model, changes: Changes<'_>, discard: bool) -> Result<SavePlan, SaveError> {
        let closure: BTreeSet<AnyKey> = match changes {
            Changes::Keys(keys) => follow_closure(keys, &self.base, model),
            Changes::All => {
                let mut all = all_keys(model);
                all.extend(self.contents.values().flatten().copied());
                all
            }
        };
        let mut dirty: BTreeSet<FilePath> = self.rewrite.clone();
        // A repair: the files that hold broken lines are rewritten as `self.rewrite`'s are,
        // since under `Changes::Keys` a file whose only change is a dropped kept line is not otherwise dirty.
        if discard {
            dirty.extend(self.broken.keys().cloned());
        }
        if matches!(changes, Changes::All) {
            dirty.extend(self.disk.keys().cloned());
            dirty.extend(self.kept.keys().cloned());
            dirty.extend(self.contents.keys().cloned());
        }
        let mut contents: BTreeMap<FilePath, BTreeSet<AnyKey>> = BTreeMap::new();
        let entry = |path: &FilePath, contents: &mut BTreeMap<FilePath, BTreeSet<AnyKey>>| {
            if !contents.contains_key(path) {
                contents.insert(path.clone(), self.contents.get(path).cloned().unwrap_or_default());
            }
        };
        for key in &closure {
            let old = file_of(&self.base, key)?;
            let new = file_of(model, key)?;
            if let Some(o) = &old {
                entry(o, &mut contents);
                debug_assert!(
                    contents[o].contains(key) || self.base.row(key).is_none(),
                    "{key:?} is not in its old file {o}"
                );
                if let Some(set) = contents.get_mut(o) {
                    set.remove(key);
                }
                dirty.insert(o.clone());
            }
            if let Some(n) = &new {
                entry(n, &mut contents);
                if let Some(set) = contents.get_mut(n) {
                    set.insert(*key);
                }
                dirty.insert(n.clone());
            }
        }
        for path in &dirty {
            entry(path, &mut contents);
        }
        let mut writes = BTreeMap::new();
        let mut removes = BTreeSet::new();
        let mut disk = self.disk.clone();
        for path in &dirty {
            if self.opaque.contains_key(path) {
                continue;
            }
            let keys = &contents[path];
            let bytes = self.render(model, path, keys, discard)?;
            if bytes.is_empty() {
                if disk.remove(path).is_some() {
                    removes.insert(path.clone());
                }
                continue;
            }
            let hash = sha256(&bytes);
            if disk.get(path).map(|h| h.hash) != Some(hash) {
                disk.insert(path.clone(), Held::of(&bytes));
                writes.insert(path.clone(), bytes);
            }
        }
        // The assets the records name that have no file yet (doc 05 §13.3); in Save As every asset the model
        // holds. A save never removes an asset file.
        let mut assets = BTreeMap::new();
        let wanted: Vec<Sha256> = if self.all_assets {
            model.assets().map(|a| *a.sha256()).collect()
        } else {
            model.referenced_assets().into_keys().collect()
        };
        for sha in &wanted {
            let Some(asset) = model.asset(sha) else {
                continue; // named but not held: nothing to write (the check reports it)
            };
            // `assets/<64 hex>.<ext>` is always a valid path.
            let Ok(path) = FilePath::new(asset.reference().path()) else {
                continue;
            };
            // On disk with its bytes: nothing to do. A file under that name whose bytes are not the asset's
            // (EW-CORE-015) is replaced by the asset the model holds.
            if self.opaque.get(&path) == Some(&FileHash::Sha256(sha.0)) {
                continue;
            }
            writes.insert(path.clone(), asset.bytes().to_vec());
            assets.insert(path, FileHash::Sha256(sha.0));
        }
        Ok(SavePlan {
            writes,
            removes,
            disk,
            contents,
            assets,
        })
    }

    /// The bytes of one writer-owned file from the model and the kept lines; with `discard`, without its broken lines.
    pub(super) fn render(
        &self,
        model: &Model,
        path: &FilePath,
        keys: &BTreeSet<AnyKey>,
        discard: bool,
    ) -> Result<Vec<u8>, SaveError> {
        if let PathClass::LibDocument { .. } = classify(path.as_str()) {
            let Some(key) = keys.iter().find(|k| is_library(k)) else {
                return Ok(Vec::new());
            };
            let Some(row) = model.row(key) else {
                return Ok(Vec::new());
            };
            if let Some((text, original)) = self.untouched_docs.get(key) {
                if row.same(original) {
                    return Ok(text.clone());
                }
            }
            return Ok(row_text(&row)?.into_bytes());
        }
        let mut lines: Vec<(LineKey, Vec<u8>)> = Vec::with_capacity(keys.len());
        for key in keys {
            let Some(row) = model.row(key) else { continue };
            let text = match self.untouched.get(key) {
                Some((text, original)) if row.same(original) => text.clone(),
                _ => row_text(&row)?.into_bytes(),
            };
            lines.push((row_line_key(key), text));
        }
        if let Some(kept) = self.kept.get(path) {
            let broken = self.broken.get(path).filter(|_| discard);
            lines.extend(
                kept.iter()
                    .filter(|(key, _)| broken.is_none_or(|b| !b.lines.contains_key(key)))
                    .cloned(),
            );
        }
        Ok(jsonl_file(lines))
    }

    /// Every writer-owned file as a full save of `model` would write it from scratch (the oracle of
    /// NFR-REL-16), plus the opaque files' hashes. For tests and `ew fmt --check`.
    ///
    /// # Errors
    ///
    /// [`SaveError::Format`] for a record without a file or a JSON form.
    pub fn render_all(&self, model: &Model) -> Result<BTreeMap<FilePath, Vec<u8>>, SaveError> {
        let mut contents: BTreeMap<FilePath, BTreeSet<AnyKey>> = BTreeMap::new();
        for key in all_keys(model) {
            if let Some(file) = file_of(model, &key)? {
                contents.entry(file).or_default().insert(key);
            }
        }
        for path in self.kept.keys() {
            contents.entry(path.clone()).or_default();
        }
        let mut out = BTreeMap::new();
        for (path, keys) in &contents {
            let bytes = self.render(model, path, keys, false)?;
            if !bytes.is_empty() {
                out.insert(path.clone(), bytes);
            }
        }
        Ok(out)
    }

    /// The rewrite set of the generation on disk in canonical form (`ew fmt`): each file the loader
    /// found not in canonical form or holding a record of another file (`Loaded::rewrite`), with the bytes the next
    /// save writes for it (`None`: the save removes it, nothing is left in it). Files already canonical are not in
    /// it: this is the save plan with no changed record, never a full rewrite. Rendered from the generation on disk
    /// (unsaved edits are not in it); kept lines (quarantined, unknown kinds, untouched malformed values) are in the
    /// bytes verbatim, as in every save. Nothing is written.
    ///
    /// # Errors
    ///
    /// [`SaveError::Format`] for a record without a file or a JSON form.
    pub fn canonical_rewrite(&self) -> Result<BTreeMap<FilePath, Option<Vec<u8>>>, SaveError> {
        self.rewrite_plan(false)
    }

    /// The files `ew fmt --repair`'s save writes ([`Store::discard_broken_lines`]), as [`Store::canonical_rewrite`]
    /// gives `ew fmt`'s: the files that hold broken lines, with the bytes the save writes for them (every other line as
    /// the next save writes it, kept lines verbatim), and the files the loader found not in canonical form; `None` for
    /// a file the save removes (nothing is left in it). Nothing is written.
    ///
    /// # Errors
    ///
    /// [`SaveError::Format`] for a record without a file or a JSON form.
    pub fn repair_rewrite(&self) -> Result<BTreeMap<FilePath, Option<Vec<u8>>>, SaveError> {
        self.rewrite_plan(true)
    }

    /// The record files and library documents of the plan with no changed record, with or without the broken lines.
    pub(super) fn rewrite_plan(&self, discard: bool) -> Result<BTreeMap<FilePath, Option<Vec<u8>>>, SaveError> {
        let plan = self.plan(&self.base, Changes::Keys(&BTreeSet::new()), discard)?;
        let mut out: BTreeMap<FilePath, Option<Vec<u8>>> = plan
            .writes
            .into_iter()
            .filter(|(path, _)| {
                matches!(
                    classify(path.as_str()),
                    PathClass::Records(_) | PathClass::LibDocument { .. }
                )
            })
            .map(|(path, bytes)| (path, Some(bytes)))
            .collect();
        out.extend(plan.removes.into_iter().map(|path| (path, None)));
        Ok(out)
    }
}
