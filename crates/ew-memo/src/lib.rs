// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Generic memo engine: revisions, dependency recording, early cut-off, durability levels, LRU, statistics,
//! oracle hooks and a reverse-dependency API per query family. Knows nothing about ECAD.
//!
//! `ew-memo` v1 is exactly what ADR-0002 decision 4 and amendment 1 list (ARCH §6.2):
//!
//! - **Revisions** ([`Rev`]): the host stamps its inputs and [advances](Store::advance) the store per commit.
//!   A memo holds `{value: Arc<V>, deps: SmallVec<(DepKey, observed)>, verified_at, changed_at}`.
//! - **Red-green verification with early cut-off** ([`Store`]): a read is a hit at the verified revision;
//!   otherwise the recorded dependencies are verified recursively, and a re-executed value that is
//!   [equivalent](Equivalence) to the previous one keeps its `changed_at`, so dependents stay green.
//! - **Durability** ([`Durability`]): library, settings, drawing; the skip rule is in [`Store::advance`].
//! - **A byte-budget LRU** for bulky families ([`Query::LRU`], [`Store::set_lru_budget`]).
//! - **Cycle detection** ([`Cycle`]): a panic in debug builds, an error in release builds.
//! - **Statistics** per family and per commit ([`Stats`], [`Counters`]), emitted through `tracing`.
//! - **Reverse dependencies** per family ([`Store::dependents`]).
//! - **Oracle mode** ([`Oracle`], [`Mismatch`]): memoised reads re-run cold and compared.
//!
//! The host implements [`Db`] for its snapshot and one [`Query`] per family; a query reads inputs through
//! [`Cx::read`] and other queries through [`Cx::get`].
//!
//! ```
//! use ew_memo::{Cx, Cycle, Db, Durability, Equivalence, Query, Rev, Store};
//!
//! /// Two cells with their revision stamps.
//! struct Cells([(i64, Rev); 2]);
//!
//! impl Db for Cells {
//!     type Input = usize;
//!     fn changed_at(&self, i: &usize) -> Rev {
//!         self.0[*i].1
//!     }
//!     fn durability(&self, _: &usize) -> Durability {
//!         Durability::Drawing
//!     }
//! }
//!
//! /// Whether cell `i` is positive.
//! struct Positive;
//! impl Query<Cells> for Positive {
//!     type Key = usize;
//!     type Value = bool;
//!     const NAME: &'static str = "positive";
//!     fn equivalence() -> Equivalence<bool> {
//!         Equivalence::by_eq()
//!     }
//!     fn execute(cx: &mut Cx<'_, Cells>, i: &usize) -> Result<bool, Cycle> {
//!         cx.read(i);
//!         Ok(cx.db().0[*i].0 > 0)
//!     }
//! }
//!
//! let mut db = Cells([(5, Rev(1)), (7, Rev(1))]);
//! let mut store = Store::new();
//! store.advance(Rev(1), Durability::Drawing);
//! assert!(*store.get::<Positive>(&db, &0)?);
//!
//! // Cell 0 changes but stays positive: the query re-executes and cuts off.
//! db.0[0] = (9, Rev(2));
//! store.advance(Rev(2), Durability::Drawing);
//! assert!(*store.get::<Positive>(&db, &0)?);
//! let c = store.commit_stats().family("positive");
//! assert_eq!((c.re_executions, c.cut_offs), (1, 1));
//! assert_eq!(store.memo_info::<Positive>(&0).unwrap().changed_at, Rev(1));
//! # Ok::<(), Cycle>(())
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod key;
mod lru;
mod query;
mod rdeps;
mod rev;
mod stats;
mod store;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_growth;
#[cfg(test)]
mod tests_push;

pub use key::{DepKey, FamilyId, SlotId};
pub use query::{Changes, Db, Equivalence, Query};
pub use rev::{Durability, Rev};
pub use stats::{Counters, Stats};
pub use store::{Cx, Cycle, Explain, FamilyMemory, MemoInfo, Mismatch, Oracle, Store, TrailEntry};
