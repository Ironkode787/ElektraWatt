// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page digests and the page change status (doc 15 §5.6; WP-S1.18 part 2, D6 and D7; FR-REV-004, FR-REV-012,
//! FR-REV-022).
//!
//! **The digest** ([`page_digest`], [`page_digests`]). The first 128 bits of the SHA-256 of
//!
//! 1. the prefix `ew-page-digest/1\n` and the `ew_dl` encoding version ([`ew_dl::canonical::CANONICAL_VERSION`]);
//! 2. per layer of the page's kind except `frame`, `highlights` and the derived `revision` layer, in paint order: the
//!    layer's name, its display list and its symbol geometry in key order ([`ew_dl::Canonical`]), so that a changed
//!    symbol definition changes every page that places it;
//! 3. the title-block values (`title_block(page)`) except every `revision.*` value (label, state, date, author and
//!    checker of the current revision), `page.last_changed_in` and every date (`project.created`, `project.edited`):
//!    their path and their value in path order.
//!
//! The frame layer is left out because it prints the title block, whose values step 3 takes without the ones a new
//! revision changes on every page. A page of a `revision_overview` block leaves out its `tables` layer too: its rows
//! list the revisions, so it changes with every new revision record and is changed in every digest-based revision
//! (doc 15 §5.6, "Generated pages"); taking its table into its digest would also make the digests depend on
//! themselves. Only the neutral language is digested: the pages print neutral texts until multi-language output
//! (FR-I18N-005); the loop over the documentation languages joins when they render.
//!
//! The digest reads the memoised layers and the memoised title block, so it is a function of `Q` (Class I), whatever
//! the thread that encodes it ([`page_digests`] encodes on [`ew_export::par_map`]).
//!
//! **The change status.** `r_last` is the latest released revision that is not withdrawn
//! ([`ew_derive::last_released_revision`]), `o` the open revision ([`ew_derive::open_revision`]):
//!
//! - [`changed_now`]: without an open revision, no page; when `r_last` carries page digests (the digest branch), a
//!   page whose digest differs from `r_last`'s or that `r_last` does not list, every page of a `revision_overview`
//!   block, and a page `o`'s page notes name (the release's `changed_pages(r)` unions `r.page_notes` into the
//!   digest-changed pages, so the open revision shows what its release will record); otherwise (the no-digest
//!   branch: Golden A's historic V1.0) a page `o`'s page notes name.
//! - [`last_changed_in`]: `o`'s label if the page is changed now, else the latest released revision that recorded it,
//!   else the first revision ([`ew_derive::last_changed_in`] with this module's `changed_now` in the digest branch).
//! - [`changed_pages`]: of `o`, the pages changed now, in page order; of any other revision the pages it records
//!   ([`ew_derive::recorded_changed_pages`]) that are in the page order, in page order.
//!
//! **How the status reaches the outputs** (the mechanism of WP-S1.18 part 2): `ew-derive` and `ew-reports` cannot
//! see the page scene, so this crate computes the status and hands it down as a parameter, as ADR-0032 hands the
//! text-style table to the scene: `ew_derive::last_changed_in` takes `changed_now`, the page bindings
//! (`PageBindingsOf`) replace the title block's `page.last_changed_in` with [`last_changed_in`] in the digest branch,
//! and the printed table of a `revision_overview` page (`PageTableOf`) takes each row's pages from [`changed_pages`].
//! The digests never read a page's bindings or a revision overview's table, so the hand-down has no cycle.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::{PageEntry, PageOrder, PageRef, Q, TbValue, TitleBlock, TitleBlockValues};
use ew_dl::canonical::CANONICAL_VERSION;
use ew_dl::{Canonical, Layer};
use ew_export::{Parallelism, par_map};
use ew_mltext::MlTextView;
use ew_schema::{AssetFormat, Digest128, Page, ReportKind, ReportSource, Revision};
use sha2::{Digest, Sha256};

use super::layers::{PageLayer, page_kind, page_layer};
use super::view::page_uuid;

/// The prefix of every page digest's input (version 1 of this module's layout).
const DIGEST_PREFIX: &[u8] = b"ew-page-digest/1\n";

/// What one page digest reads: its digested layers and its title-block values.
struct DigestInput {
    layers: Vec<(Layer, Arc<PageLayer>)>,
    title: Arc<TitleBlock>,
}

/// Whether `page` is a page of a `revision_overview` block.
fn is_revision_overview(q: &Q, page: PageRef) -> bool {
    let PageRef::Generated { block, .. } = page else {
        return false;
    };
    q.report_blocks()
        .get(block)
        .is_some_and(|b| matches!(b.source, ReportSource::Kind(ReportKind::RevisionOverview)))
}

/// Whether the digest leaves out the title-block value at `path`: the current revision's values, the page's own
/// change status and every date (D6).
fn excluded(path: &str, value: &TbValue) -> bool {
    path.starts_with("revision.") || path == "page.last_changed_in" || matches!(value, TbValue::Date(_))
}

fn digest_input(q: &Q, entry: &PageEntry) -> DigestInput {
    let overview = is_revision_overview(q, entry.page);
    let layers = page_kind(entry)
        .layers()
        .iter()
        .copied()
        .filter(|l| !matches!(l, Layer::Frame | Layer::Highlights | Layer::Revision))
        .filter(|l| !(overview && *l == Layer::Tables))
        .map(|l| (l, page_layer(q, entry.page, l)))
        .collect();
    DigestInput {
        layers,
        title: q.get::<TitleBlockValues>(entry.page),
    }
}

/// The digest of one input (see the module documentation).
fn digest(input: &DigestInput) -> Digest128 {
    let mut c = Canonical::new();
    c.fixed(DIGEST_PREFIX);
    c.u32(CANONICAL_VERSION);
    c.count(input.layers.len());
    for (layer, l) in &input.layers {
        c.str(layer.name());
        c.display_list(&l.list);
        c.geometry(&l.geometry);
    }
    let values: Vec<(&String, &TbValue)> = input
        .title
        .values
        .iter()
        .filter(|(p, v)| !excluded(p, v))
        .collect();
    c.count(values.len());
    for (path, value) in values {
        c.str(path);
        title_value(&mut c, value);
    }
    let sha = Sha256::digest(c.as_bytes());
    let mut first = [0u8; 16];
    first.copy_from_slice(&sha[..16]);
    Digest128(u128::from_be_bytes(first))
}

/// A title-block value: a tag byte and its content.
fn title_value(c: &mut Canonical, value: &TbValue) {
    match value {
        TbValue::Text(t) => {
            c.u8(0);
            c.str(t);
        }
        TbValue::Ml(m) => {
            c.u8(1);
            match m.view() {
                MlTextView::Neutral(t) => {
                    c.u8(0);
                    c.str(t);
                }
                MlTextView::Langs(langs) => {
                    c.u8(1);
                    c.count(langs.len());
                    for (lang, text) in langs {
                        c.str(lang.as_str());
                        c.str(text);
                    }
                }
            }
        }
        TbValue::Date(d) => {
            c.u8(2);
            c.str(&d.to_string());
        }
        TbValue::Tag { full, displayed } => {
            c.u8(3);
            c.str(full);
            c.str(displayed);
        }
        TbValue::Asset(r) => {
            c.u8(4);
            c.fixed(&r.sha256.0);
            c.u8(match r.format {
                AssetFormat::Svg => 0,
                AssetFormat::Png => 1,
                AssetFormat::Jpeg => 2,
            });
            c.flag(r.size.is_some());
            if let Some((w, h)) = r.size {
                c.u32(w);
                c.u32(h);
            }
        }
    }
}

/// The digest of `page` (see the module documentation); `None` for a page that is not in the page order.
pub fn page_digest(q: &Q, page: PageRef) -> Option<Digest128> {
    let order = q.get::<PageOrder>(());
    order.entry(page).map(|e| digest(&digest_input(q, e)))
}

/// The digest of every page of the page order by page ID ([`page_uuid`]: a drawn page's ID, a divider's section ID,
/// a generated page's v5 ID), the map `ReleaseRecord.page_digests` holds. The layers are built on the calling thread
/// through `q`; the encoding and hashing run on `parallelism` threads and give the same digests on any number.
pub fn page_digests(q: &Q, parallelism: Parallelism) -> BTreeMap<Id<Page>, Digest128> {
    let order = q.get::<PageOrder>(());
    let inputs: Vec<(Id<Page>, DigestInput)> = order
        .entries
        .iter()
        .map(|e| (Id::from_uuid(page_uuid(e.page)), digest_input(q, e)))
        .collect();
    par_map(&inputs, parallelism, |_, (id, input)| (*id, digest(input)))
        .into_iter()
        .collect()
}

/// The page digests of `r_last`, when it carries them: the digest branch of doc 15 §5.6.
fn last_digests(q: &Q) -> Option<BTreeMap<Id<Page>, Digest128>> {
    ew_derive::last_released_revision(q)
        .and_then(|r| r.release.as_ref().and_then(|rel| rel.page_digests.clone()))
}

/// `changed_now(p)` of doc 15 §5.6 (see the module documentation).
pub fn changed_now(q: &Q, page: PageRef) -> bool {
    let Some(open) = ew_derive::open_revision(q) else {
        return false;
    };
    let id: Id<Page> = Id::from_uuid(page_uuid(page));
    match last_digests(q) {
        // The digest branch takes `o`'s noted pages too, as the release's `changed_pages(r)` will (FX37a).
        Some(digests) => {
            open.page_notes.iter().any(|n| n.page == id)
                || is_revision_overview(q, page)
                || digests.get(&id).copied() != page_digest(q, page)
        }
        None => open.page_notes.iter().any(|n| n.page == id),
    }
}

/// `last_changed_in(p)` of doc 15 §5.6 (see the module documentation); `None` without revisions.
pub fn last_changed_in(q: &Q, page: PageRef) -> Option<String> {
    let id: Id<Page> = Id::from_uuid(page_uuid(page));
    let now = last_digests(q).is_some().then(|| changed_now(q, page));
    ew_derive::last_changed_in(q, id, now)
}

/// The digest branch's `page.last_changed_in` of a page's bindings: `Some` only when `r_last` carries page digests
/// (the no-digest value is the title block's own).
pub(crate) fn digest_last_changed_in(q: &Q, page: PageRef) -> Option<String> {
    last_digests(q)?;
    last_changed_in(q, page)
}

/// `changed_pages(r)` of doc 15 §5.6 for the revision `rev` (see the module documentation), in page order.
pub fn changed_pages(q: &Q, rev: &Revision) -> Vec<PageRef> {
    let order = q.get::<PageOrder>(());
    let open = ew_derive::open_revision(q).is_some_and(|o| o.id == rev.id);
    if open && last_digests(q).is_some() {
        return order
            .entries
            .iter()
            .map(|e| e.page)
            .filter(|p| changed_now(q, *p))
            .collect();
    }
    let by_id: BTreeMap<Id<Page>, (usize, PageRef)> = order
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| (Id::from_uuid(page_uuid(e.page)), (i, e.page)))
        .collect();
    let mut pages: Vec<(usize, PageRef)> = ew_derive::recorded_changed_pages(rev)
        .into_iter()
        .filter_map(|p| by_id.get(&p).copied())
        .collect();
    pages.sort();
    pages.dedup();
    pages.into_iter().map(|(_, p)| p).collect()
}
