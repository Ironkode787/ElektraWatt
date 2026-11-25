// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The store's open (doc 05 §6.1): the lock, an interrupted save rolled forward or waited for, the manifest
//! and the format decision, the files read, the model and its baseline, the journal.

use super::*;

impl Store {
    // -----------------------------------------------------------------------------------------------------
    // Open

    /// Opens a project directory (doc 05 §6.1): lock, roll-forward or temp clean-up, manifest, hashes,
    /// parse, integrity, model, leftover journal.
    ///
    /// # Errors
    ///
    /// [`OpenError`]; integrity findings are not errors: they are in the report, and those that force it
    /// make the session read-only.
    pub fn open(root: &Path, opts: StoreOptions) -> Result<Opened, OpenError> {
        if opts.cloud_synced {
            let clock = Arc::clone(&opts.clock);
            let first = Store::open_inner(root, opts.clone(), false);
            let missing = |findings: &[Finding]| {
                findings
                    .iter()
                    .any(|f| f.kind == ew_format::FindingKind::MissingFile)
            };
            let again = match &first {
                Ok(opened) => missing(&opened.report.findings),
                Err(OpenError::Model { findings, .. } | OpenError::NoProject(findings)) => missing(findings),
                Err(_) => false,
            };
            if !again {
                return first;
            }
            let opened = first;
            // D25: a sync client may still be writing the folder; the first open's lock goes with it.
            drop(opened);
            clock.sleep(crate::external::CLOUD_RECHECK);
        }
        Store::open_inner(root, opts, false)
    }

    /// Takes over a stale lock (doc 05 §7.4; the caller has confirmed) and opens the project for editing.
    ///
    /// # Errors
    ///
    /// [`OpenError::Takeover`] when the lock is not stale or another instance won; otherwise as
    /// [`Store::open`].
    pub fn take_over(root: &Path, opts: StoreOptions) -> Result<Opened, OpenError> {
        match opts.lock(root).take_over(TAKEOVER_SETTLE).map_err(io_err(root))? {
            Ok(_) => Store::open_inner(root, opts, true),
            Err(e) => Err(OpenError::Takeover(Box::new(e))),
        }
    }

    /// A store over `root` that has loaded nothing yet.
    pub(super) fn blank(root: &Path, opts: StoreOptions, access: Access, holds_lock: bool) -> Store {
        Store {
            root: root.to_path_buf(),
            opts,
            access,
            holds_lock,
            base: Model::new(placeholder_project()),
            generation: 0,
            manifest: None,
            manifest_hash: None,
            keep_format: false,
            disk: BTreeMap::new(),
            opaque: BTreeMap::new(),
            contents: BTreeMap::new(),
            kept: BTreeMap::new(),
            untouched: BTreeMap::new(),
            untouched_docs: BTreeMap::new(),
            rewrite: BTreeSet::new(),
            pending: None,
            pending_removes: Vec::new(),
            notes: Vec::new(),
            all_assets: false,
            quarantined: BTreeSet::new(),
            broken: BTreeMap::new(),
            journal: None,
            leftovers: Vec::new(),
            recovery: None,
            closed: false,
            manifest_bytes: None,
            migrated: None,
            watch: None,
        }
    }

    pub(super) fn open_inner(root: &Path, opts: StoreOptions, locked: bool) -> Result<Opened, OpenError> {
        let vfs = Arc::clone(&opts.vfs);
        match vfs.stat(root) {
            Ok(s) if s.kind == EntryKind::Dir => {}
            Ok(_) => return Err(OpenError::NotADirectory(root.to_path_buf())),
            Err(e) => return Err(io_err(root)(e)),
        }
        let mut access = Access::Edit;
        let mut holds_lock = locked;
        if opts.mode == OpenMode::ReadOnly {
            access = Access::ReadOnly(ReadOnlyReason::Requested);
        } else if !locked {
            match opts.lock(root).acquire().map_err(io_err(root))? {
                Acquire::Held => holds_lock = true,
                Acquire::HeldBy(info) => access = Access::ReadOnly(ReadOnlyReason::LockedBy(Box::new(info))),
                Acquire::NotWritable(why) => access = Access::ReadOnly(ReadOnlyReason::NotWritable(why)),
            }
        }
        let mut store = Store::blank(root, opts, access, holds_lock);
        // From here on, an error drops `store`, which releases the lock.
        let result = store.load();
        match result {
            Ok((model, report)) => Ok(Opened { store, model, report }),
            Err(e) => Err(e),
        }
    }

    pub(super) fn load(&mut self) -> Result<(Model, OpenReport), OpenError> {
        let mut o = Opening {
            findings: Vec::new(),
            removed: Vec::new(),
            rolled_forward: None,
            repairable: true,
            listing: BTreeMap::new(),
            abandoned: None,
        };
        // Intents and temps (doc 05 §5.3, §6.1).
        o.listing = self.walk().map_err(io_err(&self.root))?;
        if self.holds_lock {
            self.settle_interrupted_save(&mut o)?;
        } else {
            self.wait_for_a_save(&mut o)?;
        }
        let stamps = self.read_stamps(&mut o)?;
        let files = self.read_files(&mut o, &stamps);
        let model = self.load_model(&mut o, stamps.manifest, stamps.migration.as_ref(), &files)?;
        self.open_journal(&model);
        let report = OpenReport {
            access: self.access.clone(),
            generation: self.generation,
            format: stamps.format,
            findings: o.findings,
            rolled_forward: o.rolled_forward,
            removed: o.removed,
            recovery: self.recovery.clone(),
            journaled: self.journal.is_some(),
            migrated_from: self.migrated_from().map(str::to_owned),
        };
        Ok((model, report))
    }

    /// An editing open's first step (doc 05 §5.3): rolls the newest interrupted save forward, or, when it cannot,
    /// says why; then, when the project is repairable, deletes the temps, the files a crashed save left to remove and
    /// the old renamed locks, and lists the directory again.
    pub(super) fn settle_interrupted_save(&mut self, o: &mut Opening) -> Result<(), OpenError> {
        let vfs = Arc::clone(&self.opts.vfs);
        let current = self
            .read_manifest()
            .ok()
            .flatten()
            .map_or(0, |(m, _)| m.generation);
        if let Some(newest) = newest_intent(&o.listing, current) {
            match self.roll_forward(newest).map_err(io_err(&self.root))? {
                Roll::Completed(g, moved) => {
                    o.rolled_forward = Some(g);
                    o.findings.extend(moved);
                }
                Roll::Torn => {}
                Roll::NotRepairable(files) => {
                    o.repairable = false;
                    for f in files {
                        o.findings.push(Finding::new(
                            FindingKind::MixedGeneration,
                            Some(f),
                            None,
                            format!(
                                "an interrupted save of generation {newest} cannot roll forward: \
                                 neither the file nor its temp matches the intent"
                            ),
                        ));
                    }
                }
                Roll::BaseMoved(mismatch) => {
                    o.repairable = false;
                    o.findings.push(base_moved(newest, &mismatch));
                }
            }
        }
        if o.repairable {
            o.listing = self.walk().map_err(io_err(&self.root))?;
            for path in o.listing.keys() {
                if is_save_temp(path) {
                    let full = join(&self.root, path);
                    if vfs.remove_file(&full).is_ok() {
                        o.removed.push(path.clone());
                    }
                }
            }
            // Files a save removed after its manifest rename but before it could delete them (doc
            // 05 §5.3 row 3) are named by the crashed session's journal.
            if let Ok(Some((m, _))) = self.read_manifest() {
                self.prepare_journal_dir(Some(m.project));
                for seg in &self.leftovers {
                    for path in seg.removes_of(m.generation) {
                        let Ok(p) = FilePath::new(path.clone()) else {
                            continue;
                        };
                        // An asset file is in `removes` only when the purge removed it.
                        let owned = matches!(
                            classify(p.as_str()),
                            PathClass::Records(_)
                                | PathClass::LibDocument { .. }
                                | PathClass::Opaque(OpaqueKind::Asset { .. })
                        );
                        if owned
                            && !m.files.contains_key(&p)
                            && vfs.remove_file(&join(&self.root, p.as_str())).is_ok()
                        {
                            o.removed.push(path);
                        }
                    }
                }
            }
            self.opts.lock(&self.root).prune_stale(self.opts.clock.now());
            o.listing = self.walk().map_err(io_err(&self.root))?;
        }
        Ok(())
    }

    /// A read-only open's first step. Read-only openers never roll forward. Only an intent above the manifest's
    /// generation is a save in progress (an older one is a leftover the next editing open deletes). Behind a live
    /// writer they wait for its save to finish; behind a crashed one (a stale lock, or none) waiting is pointless.
    /// Either way, an intent still there after the wait is read in memory: the intent's generation, file by file from
    /// the target or its temp, and nothing is written. (On Windows a crashed writer's lock stays non-stale for
    /// `lock.stale_after`: the process probe cannot tell.)
    pub(super) fn wait_for_a_save(&mut self, o: &mut Opening) -> Result<(), OpenError> {
        let current = |store: &Store| {
            store
                .read_manifest()
                .ok()
                .flatten()
                .map_or(0, |(m, _)| m.generation)
        };
        let mut newest = newest_intent(&o.listing, current(self));
        if newest.is_some() && !self.writer_gone() {
            let mut waited = Duration::ZERO;
            while newest.is_some() && waited < self.opts.read_only_wait {
                self.opts.clock.sleep(READ_ONLY_POLL);
                waited += READ_ONLY_POLL;
                o.listing = self.walk().map_err(io_err(&self.root))?;
                newest = newest_intent(&o.listing, current(self));
            }
        }
        o.abandoned = newest;
        Ok(())
    }

    /// The manifest and the format decision (doc 05 §4, §10.1): the intent a read-only open reads in memory, the
    /// manifest (or the generation of one with git conflict markers), and the way an older major migrates.
    pub(super) fn read_stamps(&mut self, o: &mut Opening) -> Result<Stamps, OpenError> {
        let vfs = Arc::clone(&self.opts.vfs);
        let mut format = FormatAccess::ReadWrite;
        let mut pending_removal: BTreeSet<FilePath> = BTreeSet::new();
        let mut intent = o.abandoned.and_then(|g| {
            let bytes = vfs.read(&join(&self.root, &format!("{MANIFEST}.tmp-{g}"))).ok()?;
            // A torn intent never committed: generation g on disk is the truth.
            Manifest::parse(&bytes, &Limits::default())
                .ok()
                .filter(|m| m.generation == g && !m.merged)
        });
        if let Some(i) = &intent {
            // As for a roll-forward: an intent whose recorded base is not the manifest on disk, byte for
            // byte, is not applied.
            let on_disk = self.manifest_base().map_err(io_err(&self.root))?;
            if let Some(mismatch) = base_check(i, on_disk) {
                o.findings.push(base_moved(i.generation, &mismatch));
                intent = None;
            }
        }
        if let Some(intent) = &intent {
            if let Ok(Some((old, _))) = self.read_manifest() {
                pending_removal = old
                    .files
                    .keys()
                    .filter(|p| !intent.files.contains_key(*p))
                    .cloned()
                    .collect();
            }
            // A file the interrupted save creates exists only as its temp so far.
            for path in intent.files.keys() {
                if !o.listing.contains_key(path.as_str())
                    && o.listing.contains_key(&path.temp(intent.generation))
                {
                    o.listing.insert(path.as_str().to_owned(), EntryKind::File);
                }
            }
        }
        // The generation of an unusable manifest that came out of a git text merge.
        let mut conflicted = None;
        let manifest = match intent
            .clone()
            .map(|m| Ok(Some((m, [0u8; 32], None))))
            .unwrap_or_else(|| {
                self.read_manifest_bytes()
                    .map(|found| found.map(|(m, hash, bytes)| (m, hash, Some(bytes))))
            }) {
            Ok(Some((m, hash, bytes))) => {
                self.manifest_hash = Some(hash);
                self.manifest_bytes = bytes;
                Some(m)
            }
            Ok(None) => None,
            Err(e) => {
                if let Ok(bytes) = vfs.read(&join(&self.root, MANIFEST)) {
                    self.manifest_hash = Some(sha256(&bytes));
                    conflicted = Manifest::conflicted_generation(&bytes, &Limits::default());
                }
                o.findings.push(Finding::new(
                    FindingKind::ManifestUnreadable,
                    FilePath::new(MANIFEST).ok(),
                    None,
                    e,
                ));
                None
            }
        };
        if manifest.is_none()
            && self.manifest_hash.is_none()
            && o.listing
                .keys()
                .any(|p| matches!(classify(p), PathClass::Records(_)))
        {
            o.findings.push(Finding::new(
                FindingKind::ManifestUnreadable,
                FilePath::new(MANIFEST).ok(),
                None,
                "manifest.json is missing; it is rebuilt at the next save",
            ));
        }
        // The way from an older major's format to this one: the open migrates in memory.
        let mut migration = None;
        if let Some(m) = &manifest {
            format = ew_format::reader_access(m, FORMAT, self.opts.release_build);
            match &format {
                FormatAccess::Refuse(why) => return Err(OpenError::Format(why.clone())),
                FormatAccess::Migrate => {
                    let way = self
                        .opts
                        .migrations
                        .towards(&m.format, FORMAT)
                        .map_err(OpenError::Migration)?;
                    self.migrated = Some(Migrated {
                        from: m.format.clone(),
                        manifest: way
                            .manifest(m, &Limits::default())
                            .map_err(OpenError::Migration)?,
                        confirmed: false,
                    });
                    migration = Some(way);
                }
                FormatAccess::ReadOnly(why) => self.demote(ReadOnlyReason::Format(why)),
                FormatAccess::ReadWriteNewer => self.keep_format = true,
                FormatAccess::ReadWrite => {}
            }
            self.generation = m.generation;
        } else if let Some(g) = conflicted {
            // A manifest with git conflict markers (doc 05 §11.3): the greatest generation of its sides,
            // so that the next save writes one more than either side and never restarts at 1 under both. Without
            // such a line the generation stays 0, as for a missing manifest.
            self.generation = g;
        }
        Ok(Stamps {
            format,
            pending_removal,
            intent,
            manifest,
            migration,
        })
    }

    /// Plans and reads the files the manifest lists (doc 05 §6.1, §6.2): a file larger than the limit is not read,
    /// and behind an interrupted save each file is read from the target or its temp, whichever matches the intent.
    pub(super) fn read_files(&self, o: &mut Opening, stamps: &Stamps) -> BTreeMap<FilePath, Vec<u8>> {
        let vfs = &*self.opts.vfs;
        let limits = Limits::default();
        let plan = ew_format::plan(stamps.manifest.as_ref(), &o.listing, &limits);
        // The files the interrupted save removes are not orphans.
        o.findings.extend(plan.findings.into_iter().filter(|f| {
            !(f.kind == FindingKind::Orphan
                && f.file
                    .as_ref()
                    .is_some_and(|p| stamps.pending_removal.contains(p)))
        }));
        let mut files = BTreeMap::new();
        for path in plan.read {
            let full = join(&self.root, path.as_str());
            match vfs.stat(&full) {
                Ok(s) if s.len > limits.max_file_bytes => {
                    o.findings.push(Finding::new(
                        FindingKind::LimitExceeded,
                        Some(path),
                        None,
                        format!("{} bytes, more than the limit; not read", s.len),
                    ));
                    continue;
                }
                _ => {}
            }
            let read = match (&stamps.intent, vfs.read(&full)) {
                (Some(intent), target) => {
                    let wanted = intent.files.get(&path);
                    match target {
                        Ok(bytes) if wanted.is_none_or(|h| h.matches(&bytes)) => Ok(bytes),
                        target => {
                            let temp = vfs.read(&join(&self.root, &path.temp(intent.generation)));
                            // A live writer may rename the temp between the two reads: read the target
                            // once more before calling the file mixed.
                            let temp = match temp {
                                Ok(bytes) if wanted.is_some_and(|h| h.matches(&bytes)) => Ok(bytes),
                                _ => vfs.read(&full),
                            };
                            match temp {
                                Ok(bytes) if wanted.is_some_and(|h| h.matches(&bytes)) => Ok(bytes),
                                _ => {
                                    o.findings.push(Finding::new(
                                        FindingKind::MixedGeneration,
                                        Some(path.clone()),
                                        None,
                                        format!(
                                            "an interrupted save of generation {}: neither the file nor its \
                                             temp matches the intent",
                                            intent.generation
                                        ),
                                    ));
                                    target
                                }
                            }
                        }
                    }
                }
                (None, target) => target,
            };
            match read {
                Ok(bytes) => {
                    files.insert(path, bytes);
                }
                Err(e) => o.findings.push(Finding::new(
                    FindingKind::MissingFile,
                    Some(path),
                    None,
                    format!("cannot be read: {e}"),
                )),
            }
        }
        files
    }

    /// Parses the files read and builds the model (doc 05 §6.1, §6.3): the loader's findings, the read-only demotion,
    /// the assets, and the baseline a save compares with (the hashes, the kept lines, the rewrite set, the file of
    /// every key, the manifest and the base model).
    pub(super) fn load_model(
        &mut self,
        o: &mut Opening,
        manifest: Option<Manifest>,
        migration: Option<&Migration<'_>>,
        files: &BTreeMap<FilePath, Vec<u8>>,
    ) -> Result<Model, OpenError> {
        let limits = Limits::default();
        let loaded = match migration {
            Some(way) => ew_format::load_migrated(manifest.as_ref(), files, &limits, way)
                .map_err(OpenError::Migration)?,
            None => ew_format::load(manifest.as_ref(), files, &limits),
        };
        // The loader's list is capped at `max_findings`; its flag also counts the findings it dropped
        // (review, major 2: a thousand duplicates must not hide a line the writer would lose).
        // A repair open lets broken lines through, which the repair discards, and nothing
        // else: the loader's own flag for the other forcing findings counts those beyond its cap too.
        let repair = self.opts.repair;
        let loader_read_only = if repair {
            loaded.read_only_beyond_repair
        } else {
            loaded.read_only
        };
        o.findings.extend(loaded.findings);
        o.findings.sort();
        let Some(project) = loaded.project else {
            return Err(OpenError::NoProject(std::mem::take(&mut o.findings)));
        };
        // The library copies of the documents with a finding, which the loader may have dropped. A
        // record that names one is no EW-INV-001 at this open: the project opens read-only, the finding naming the
        // copy's file, and the user updates the copy from its library.
        let missing: BTreeSet<AnyKey> = o
            .findings
            .iter()
            .filter_map(|f| match classify(f.file.as_ref()?.as_str()) {
                PathClass::LibDocument { id, .. } => Some(id),
                _ => None,
            })
            .flat_map(|id| {
                ew_schema::Kind::ALL
                    .into_iter()
                    .filter(|k| k.is_library())
                    .map(move |k| (k, id))
            })
            .collect();
        let mut model = match Model::from_records_missing(project, loaded.records, loaded.lib, &missing) {
            Ok(m) => m,
            Err(error) => {
                return Err(OpenError::Model {
                    error,
                    findings: std::mem::take(&mut o.findings),
                });
            }
        };
        // The asset store (doc 05 §13.3): every asset file whose bytes match the hash in its name and the format
        // of its extension. Others stay opaque files only (EW-CORE-015 reports a wrong hash).
        for (path, hash) in &loaded.opaque {
            if let Some(asset) = asset_of(path, hash, files.get(path)) {
                model.put_asset(asset);
            }
        }
        let forced = o
            .findings
            .iter()
            .any(|f| f.kind.forces_read_only() && !(repair && f.is_broken_line()));
        // A record that names a copy the loader dropped: never an editable session, a repair open included.
        let dangling = !missing.is_empty() && model.integrity_check().is_err();
        if !o.repairable || loader_read_only || forced || dangling {
            self.demote(ReadOnlyReason::Integrity);
        }

        // The baseline.
        for (path, hash) in &loaded.hashes {
            if matches!(
                classify(path.as_str()),
                PathClass::Records(_) | PathClass::LibDocument { .. }
            ) && !loaded.opaque.contains_key(path)
            {
                // The raw hash as the reader found it (the manifest's), and the LF form's beside it.
                let bytes = files.get(path);
                let lf = bytes.and_then(|bytes| lf_hash(bytes, hash));
                self.disk.insert(path.clone(), Held { hash: *hash, lf });
            }
        }
        self.opaque = loaded
            .opaque
            .into_iter()
            .map(|(p, h)| (p, FileHash::Sha256(h)))
            .collect();
        let mut other_quarantined: BTreeSet<FilePath> = BTreeSet::new();
        for line in loaded.kept {
            if matches!(line.kept, Kept::Quarantined) {
                self.quarantined.insert(line.file.clone());
                if line.is_broken() {
                    self.broken
                        .entry(line.file.clone())
                        .or_default()
                        .lines
                        .insert(line.key.clone(), line.line);
                } else {
                    other_quarantined.insert(line.file.clone());
                }
            }
            match line.kept {
                Kept::UnknownKind | Kept::Quarantined => {
                    self.kept
                        .entry(line.file)
                        .or_default()
                        .push((line.key, line.text));
                }
                Kept::Untouched { original } => {
                    if let Some(row) = record_row(*original) {
                        self.untouched.insert(row.key(), (line.text, row));
                    }
                }
            }
        }
        for (file, broken) in &mut self.broken {
            broken.other_quarantined = other_quarantined.contains(file);
        }
        for doc in loaded.kept_docs {
            if let Some(row) = document_row(*doc.original) {
                self.untouched_docs.insert(row.key(), (doc.text, row));
            }
        }
        self.rewrite = loaded.rewrite;
        // Every file a step changed is written from its migrated values at the first save.
        self.rewrite.extend(loaded.migrated);
        for key in all_keys(&model) {
            if let Some(file) = file_of(&model, &key).map_err(|e| OpenError::Format(e.to_string()))? {
                self.contents.entry(file).or_default().insert(key);
            }
        }
        self.manifest = manifest;
        self.base = model.clone();

        Ok(model)
    }

    /// The journal (doc 05 §8): only an editing session journals; a session that is not editing gives the lock
    /// back.
    pub(super) fn open_journal(&mut self, model: &Model) {
        if self.access == Access::Edit {
            self.prepare_journal_dir(Some(model.project().id.cast()));
            // Leftovers without unsaved transactions (a session that crashed or was dropped right after a
            // save, or never committed) hold nothing to recover: delete them. Of the rest, the one with the
            // latest change is offered (doc 05 §8.2 step 1).
            let generation = self.generation;
            let (mut keep, empty): (Vec<Segment>, Vec<Segment>) = std::mem::take(&mut self.leftovers)
                .into_iter()
                .partition(|seg| !seg.unsaved(generation).is_empty());
            if let Some(j) = &self.journal {
                j.discard(&empty);
            }
            keep.sort_by_key(|seg| std::cmp::Reverse(seg.last_change(generation)));
            self.leftovers = keep;
            self.recovery = journal::summarise(&self.leftovers, self.generation, self.manifest_hash.as_ref());
            if let Some(j) = &mut self.journal {
                if j.start(self.generation, self.manifest_hash.as_ref()).is_err() {
                    self.journal = None;
                }
            }
        } else {
            // `ew fmt --repair` names a crashed session's journal before a finding that forces read-only: a
            // repair open that went read-only with the lock held read the leftovers, and summarises them as
            // an editing open does, deleting and starting nothing.
            if self.opts.repair && self.holds_lock {
                let generation = self.generation;
                let mut keep: Vec<Segment> = self
                    .leftovers
                    .iter()
                    .filter(|seg| !seg.unsaved(generation).is_empty())
                    .cloned()
                    .collect();
                keep.sort_by_key(|seg| std::cmp::Reverse(seg.last_change(generation)));
                self.recovery = journal::summarise(&keep, generation, self.manifest_hash.as_ref());
            }
            self.journal = None;
            self.leftovers.clear();
        }
        if self.access != Access::Edit && self.holds_lock {
            self.release_lock();
        }
    }

    /// Creates the journal handle for project `project` and reads the leftovers of other sessions (once).
    pub(super) fn prepare_journal_dir(&mut self, project: Option<AnyId>) {
        if self.journal.is_some() {
            return;
        }
        let (Some(state), Some(project)) = (self.opts.state_dir.clone(), project) else {
            return;
        };
        let j = Journal::new(
            Arc::clone(&self.opts.vfs),
            &state,
            project,
            &self.root,
            &self.opts.identity.session,
            &self.opts.identity.app_version,
        );
        self.leftovers = j.leftovers(self.opts.clock.now());
        self.journal = Some(j);
    }

    /// Whether no live session holds the lock: the lock is stale, absent or unreadable as a record. A
    /// read-only opener then treats an intent as abandoned by a crashed writer.
    pub(super) fn writer_gone(&self) -> bool {
        if let Access::ReadOnly(ReadOnlyReason::LockedBy(info)) = &self.access {
            return info.stale;
        }
        match self.opts.lock(&self.root).read(self.opts.clock.now()) {
            Ok(None) => true,
            Ok(Some(info)) => info.stale,
            Err(_) => false,
        }
    }

    /// The SHA-256 of the raw bytes of `manifest.json` as `sha256:<hex>`, `None` when absent: what an intent
    /// records as its [`BASE_MEMBER`].
    pub(super) fn manifest_base(&self) -> io::Result<Option<String>> {
        match self.opts.vfs.read(&join(&self.root, MANIFEST)) {
            Ok(bytes) => Ok(Some(hash_text(&sha256(&bytes)))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Reads and parses `manifest.json`: `Ok(None)` when absent, `Err(message)` when unusable.
    pub(super) fn read_manifest(&self) -> Result<Option<(Manifest, [u8; 32])>, String> {
        self.read_manifest_bytes()
            .map(|found| found.map(|(m, hash, _)| (m, hash)))
    }

    /// [`Store::read_manifest`] with the bytes read.
    pub(super) fn read_manifest_bytes(&self) -> Result<Option<FoundManifest>, String> {
        let path = join(&self.root, MANIFEST);
        match self.opts.vfs.read(&path) {
            Ok(bytes) => Manifest::parse(&bytes, &Limits::default())
                .map(|m| {
                    let hash = sha256(&bytes);
                    Some((m, hash, bytes))
                })
                .map_err(|e| e.to_string()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Every entry below the root by project-relative path (links not followed; `.ew/`, `.git/` and
    /// `.~ew.requests/` not entered), up to the file limit.
    pub(super) fn walk(&self) -> io::Result<BTreeMap<String, EntryKind>> {
        let mut out = BTreeMap::new();
        let mut stack: Vec<(String, usize)> = vec![(String::new(), 0)];
        while let Some((rel, depth)) = stack.pop() {
            let dir = if rel.is_empty() {
                self.root.clone()
            } else {
                join(&self.root, &rel)
            };
            let entries = match self.opts.vfs.list(&dir) {
                Ok(e) => e,
                Err(e) if e.kind() == io::ErrorKind::NotFound && !rel.is_empty() => continue,
                Err(e) => return Err(e),
            };
            for (name, kind) in entries {
                let path = if rel.is_empty() {
                    name.clone()
                } else {
                    format!("{rel}/{name}")
                };
                if kind == EntryKind::Dir
                    && depth < MAX_WALK_DEPTH
                    && !(rel.is_empty() && matches!(name.as_str(), ".ew" | ".git" | ".~ew.requests"))
                {
                    stack.push((path.clone(), depth + 1));
                }
                out.insert(path, kind);
                if out.len() > Limits::default().max_files + 64 {
                    return Ok(out);
                }
            }
        }
        Ok(out)
    }

    /// Completes the save of generation `g` from its intent (doc 05 §5.3), when the raw bytes of
    /// `manifest.json` are still those the intent records as its base ([`BASE_MEMBER`]). A file at a target
    /// path that the base does not list and that does not already hold the intent's bytes is moved aside
    /// first ([`Store::move_aside`]).
    pub(super) fn roll_forward(&mut self, g: u64) -> io::Result<Roll> {
        let vfs = Arc::clone(&self.opts.vfs);
        let intent_rel = format!("{MANIFEST}.tmp-{g}");
        let intent_path = join(&self.root, &intent_rel);
        let delete_temps = |store: &Store| -> io::Result<()> {
            for path in store.walk()?.keys() {
                if let PathClass::Temp { generation, .. } | PathClass::Intent { generation } = classify(path)
                {
                    if generation == g && is_save_temp(path) {
                        let _ = vfs.remove_file(&join(&store.root, path));
                    }
                }
            }
            Ok(())
        };
        let intent_bytes = vfs.read(&intent_path)?;
        let intent = match Manifest::parse(&intent_bytes, &Limits::default()) {
            Ok(m) if m.generation == g && !m.merged => m,
            _ => {
                delete_temps(self)?;
                return Ok(Roll::Torn);
            }
        };
        // The base: the manifest the save started from, byte for byte (its pre-check, doc 05 §5.2 step 2).
        if let Some(mismatch) = base_check(&intent, self.manifest_base()?) {
            return Ok(Roll::BaseMoved(mismatch));
        }
        // The base's file list, when it parses. A conflicted or otherwise unusable manifest (EW-CORE-001)
        // made the save read every writer-owned file present, so all of them were its baseline.
        let old = self.read_manifest().ok().flatten().map(|(m, _)| m);
        let listed = |p: &FilePath| old.as_ref().is_none_or(|m| m.files.contains_key(p));
        let mut renames = Vec::new();
        let mut missing = Vec::new();
        for (path, hash) in &intent.files {
            let FileHash::Sha256(h) = hash else {
                missing.push(path.clone());
                continue;
            };
            let target = join(&self.root, path.as_str());
            if vfs.read(&target).is_ok_and(|b| sha256(&b) == *h) {
                continue;
            }
            let temp = join(&self.root, &path.temp(g));
            if vfs.read(&temp).is_ok_and(|b| sha256(&b) == *h) {
                let occupied = !listed(path)
                    && vfs
                        .stat(&target)
                        .is_ok_and(|s| matches!(s.kind, EntryKind::File | EntryKind::Link));
                renames.push((path.clone(), temp, target, occupied));
            } else {
                missing.push(path.clone());
            }
        }
        if !missing.is_empty() {
            return Ok(Roll::NotRepairable(missing));
        }
        let mut dirs = BTreeSet::new();
        let mut moved = Vec::new();
        for (path, temp, target, occupied) in renames {
            if occupied {
                moved.push(self.move_aside(&path, g)?);
            }
            self.rename_retrying(&temp, &target)?;
            if let Some(parent) = target.parent() {
                dirs.insert(parent.to_path_buf());
            }
        }
        // Files of the old generation that the new one no longer lists: removed before the manifest
        // rename, while the old manifest still names them.
        if let Some(old) = &old {
            for path in old.files.keys() {
                // An asset file leaves a manifest only through the purge (a save never removes one).
                let owned = matches!(
                    classify(path.as_str()),
                    PathClass::Records(_)
                        | PathClass::LibDocument { .. }
                        | PathClass::Opaque(OpaqueKind::Asset { .. })
                );
                if owned && !intent.files.contains_key(path) {
                    let _ = vfs.remove_file(&join(&self.root, path.as_str()));
                }
            }
        }
        for dir in dirs.iter().filter(|d| **d != self.root) {
            vfs.sync_dir(dir)?;
        }
        self.rename_retrying(&intent_path, &join(&self.root, MANIFEST))?;
        vfs.sync_dir(&self.root)?;
        Ok(Roll::Completed(g, moved))
    }

    /// Moves the file at `path`, which the baseline does not list, to a free [`FilePath::unlisted`] name
    /// for a save of generation `g`, so that the save never renames over another user's file. The rename does
    /// not replace: a taken name is skipped. Returns the EW-CORE-004 finding naming the new path.
    ///
    /// A retried save or roll-forward of the same generation can find the file again, restored by a sync client after
    /// the first move: when the new stray's bytes equal those of an older stray of the same file and
    /// generation, the new one is removed and the finding names the older one, which holds the bytes.
    pub(super) fn move_aside(&self, path: &FilePath, g: u64) -> io::Result<Finding> {
        let vfs = &*self.opts.vfs;
        let from = join(&self.root, path.as_str());
        for n in 0..UNLISTED_ATTEMPTS {
            let aside = path.unlisted(g, n);
            let to = join(&self.root, aside.as_str());
            let moved = match vfs.rename_noreplace(&from, &to) {
                Ok(()) => true,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => false,
                // No rename without replacement here (doc 05 §7.4 step 3): check the name, then rename.
                Err(e) if e.kind() == io::ErrorKind::Unsupported => match vfs.stat(&to) {
                    Ok(_) => false,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                        self.rename_retrying(&from, &to)?;
                        true
                    }
                    Err(e) => return Err(e),
                },
                Err(e) => return Err(e),
            };
            if moved {
                if let Some(older) = self.equal_older_stray(path, g, n) {
                    // The bytes stay in the older stray; a duplicate that cannot be removed simply stays.
                    if vfs.remove_file(&to).is_ok() {
                        return Ok(Finding::new(
                            FindingKind::Orphan,
                            Some(older),
                            None,
                            format!(
                                "found again at {path}, which manifest.json does not list (another user's or a \
                                 sync client's file), with the bytes this file already holds; moved aside so that \
                                 the save of generation {g} does not overwrite it, and the duplicate removed"
                            ),
                        ));
                    }
                }
                return Ok(Finding::new(
                    FindingKind::Orphan,
                    Some(aside),
                    None,
                    format!(
                        "found at {path}, which manifest.json does not list (another user's or a sync \
                         client's file); moved here so that the save of generation {g} does not overwrite it"
                    ),
                ));
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{path}: no free name to move the unlisted file aside"),
        ))
    }

    /// An older stray of `path` and generation `g` ([`FilePath::unlisted`] with any other `n`, gaps skipped) whose
    /// bytes equal those of the stray `path.unlisted(g, new)` just moved, both read again just before the comparison.
    /// `None` when there is none, or when a listing or a read fails: the new stray then stays.
    pub(super) fn equal_older_stray(&self, path: &FilePath, g: u64, new: u32) -> Option<FilePath> {
        let vfs = &*self.opts.vfs;
        let dir = if path.dir().is_empty() {
            self.root.clone()
        } else {
            join(&self.root, path.dir())
        };
        let names: BTreeSet<String> = vfs
            .list(&dir)
            .ok()?
            .into_iter()
            .filter(|(_, kind)| *kind == EntryKind::File)
            .map(|(name, _)| name)
            .collect();
        let fresh = join(&self.root, path.unlisted(g, new).as_str());
        (0..UNLISTED_ATTEMPTS)
            .filter(|m| *m != new)
            .map(|m| path.unlisted(g, m))
            .filter(|older| names.contains(older.file_name()))
            .find(|older| same_bytes(vfs, &fresh, &join(&self.root, older.as_str())).unwrap_or(false))
    }

    pub(super) fn rename_retrying(&self, from: &Path, to: &Path) -> io::Result<()> {
        let mut attempt = 0;
        loop {
            match self.opts.vfs.rename(from, to) {
                Ok(()) => return Ok(()),
                Err(e) if is_transient(&e) && attempt < RENAME_BACKOFF.len() => {
                    self.opts.clock.sleep(RENAME_BACKOFF[attempt]);
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub(super) fn demote(&mut self, reason: ReadOnlyReason) {
        if self.access == Access::Edit {
            self.access = Access::ReadOnly(reason);
        }
    }

    pub(super) fn release_lock(&mut self) {
        if self.holds_lock {
            let _ = self.opts.lock(&self.root).release();
            self.holds_lock = false;
        }
    }
}
