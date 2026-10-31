// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The entity kinds of the model: the sealed [`Entity`] trait that ties each `ew-schema` record type to its
//! [`Kind`], its table and its ID type, the kind-erased row [`AnyRow`] of the op layer and its key
//! [`AnyKey`].

use std::fmt;
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{
    AnyId, Bond, Carrier, Channel, Conductor, Connection, Device, DeviceLink, Enclosure, ExternalRef, FnDef,
    FormDef, Function, ImportLineage, Junction, Kind, LayoutSpace, LibCopy, LogicalNet, MacroDef,
    MacroInstance, Manufacturer, MountingSurface, Page, PageItem, Part, PhysicalNet, Placement,
    PlcConfigProject, PotentialDef, Project, ReportBlock, Representation, Revision, Section, StructureBox,
    StructureNode, SymbolDef, Waiver,
};

use crate::diff::Presence;
use crate::model::Model;
use crate::rev::{Rev, RowId, Table};

/// The key of one row in the op layer: its kind and its ID (ADR-0008).
pub type AnyKey = (Kind, AnyId);

mod sealed {
    use crate::model::Model;
    use crate::rev::Table;

    /// Seals [`Entity`](super::Entity) and holds its table accessor: a method of a trait that other crates
    /// cannot name is not callable from them, so the accessor stays behind the `raw-read` gate
    /// ([`Model::table`](crate::Model) is the gated way in).
    pub trait Sealed {
        /// This kind's table in a model.
        fn table(model: &Model) -> &Table<Self>
        where
            Self: crate::Entity;
    }
}

/// A record type stored in its own [`Table`] of the [`Model`]: every implemented record kind of
/// `ew-schema` except the project row, and the six library-copy kinds.
///
/// The trait is sealed: the set of entities is the persisted schema (ADR-0001 decision 1), not an
/// extension point.
pub trait Entity: sealed::Sealed + Clone + PartialEq + fmt::Debug + Send + Sync + 'static {
    /// The kind of this entity.
    const KIND: Kind;

    /// The marker type of its IDs: the record type itself, or the item type of a library copy (a
    /// `LibCopy<Part>` is named by an `Id<Part>`).
    type Marker: 'static;

    /// The row's ID.
    fn id(&self) -> RowId<Self>;

    /// Wraps a record into a kind-erased row.
    fn wrap(row: Arc<Self>) -> AnyRow;

    /// The typed record of a kind-erased row of this kind.
    fn unwrap(row: &AnyRow) -> Option<&Arc<Self>>;

    /// The op-layer key of the row with this ID.
    fn key(id: RowId<Self>) -> AnyKey {
        (Self::KIND, id.cast())
    }
}

/// The embedded library copies of a project (ARCH §4.1 `LibraryCopies`), one table per implemented library
/// kind.
#[derive(Clone, Debug, Default)]
pub struct LibraryCopies {
    pub(crate) parts: Table<LibCopy<Part>>,
    pub(crate) symbols: Table<LibCopy<SymbolDef>>,
    pub(crate) fndefs: Table<LibCopy<FnDef>>,
    pub(crate) forms: Table<LibCopy<FormDef>>,
    pub(crate) macros: Table<LibCopy<MacroDef>>,
    pub(crate) manufacturers: Table<LibCopy<Manufacturer>>,
}

impl LibraryCopies {
    raw_read! {
        /// Part copies.
        fn parts(&self) -> &Table<LibCopy<Part>> {
            &self.parts
        }
    }

    raw_read! {
        /// Symbol copies.
        fn symbols(&self) -> &Table<LibCopy<SymbolDef>> {
            &self.symbols
        }
    }

    raw_read! {
        /// Function-definition copies.
        fn fndefs(&self) -> &Table<LibCopy<FnDef>> {
            &self.fndefs
        }
    }

    raw_read! {
        /// Form copies.
        fn forms(&self) -> &Table<LibCopy<FormDef>> {
            &self.forms
        }
    }

    raw_read! {
        /// Macro copies (WP-S1.19 part 1).
        fn macros(&self) -> &Table<LibCopy<MacroDef>> {
            &self.macros
        }
    }

    raw_read! {
        /// Manufacturer copies (WP-S1.19 part 1).
        fn manufacturers(&self) -> &Table<LibCopy<Manufacturer>> {
            &self.manufacturers
        }
    }
}

macro_rules! entities {
    ($( $variant:ident : $ty:ty, $marker:ty, $($field:ident).+ ; )*) => {
        $(
            impl sealed::Sealed for $ty {
                fn table(model: &Model) -> &Table<Self> {
                    &model.$($field).+
                }
            }

            impl Entity for $ty {
                const KIND: Kind = Kind::$variant;
                type Marker = $marker;

                fn id(&self) -> RowId<Self> {
                    self.id
                }

                fn wrap(row: Arc<Self>) -> AnyRow {
                    AnyRow::$variant(row)
                }

                fn unwrap(row: &AnyRow) -> Option<&Arc<Self>> {
                    match row {
                        AnyRow::$variant(r) => Some(r),
                        _ => None,
                    }
                }
            }
        )*

        /// A kind-erased row of the op layer: one `Arc` variant per entity kind plus the project row
        /// (ADR-0008 `AnyRow`).
        #[derive(Clone, Debug)]
        pub enum AnyRow {
            /// The project row.
            Project(Arc<Project>),
            $(
                #[doc = concat!("A `", stringify!($ty), "` row.")]
                $variant(Arc<$ty>),
            )*
        }

        impl AnyRow {
            /// The kind of the row.
            pub fn kind(&self) -> Kind {
                match self {
                    AnyRow::Project(_) => Kind::Project,
                    $( AnyRow::$variant(_) => Kind::$variant, )*
                }
            }

            /// The row's ID, kind-erased.
            pub fn id(&self) -> AnyId {
                match self {
                    AnyRow::Project(r) => r.id.cast(),
                    $( AnyRow::$variant(r) => r.id.cast(), )*
                }
            }

            /// The row's key.
            pub fn key(&self) -> AnyKey {
                (self.kind(), self.id())
            }

            /// Whether both rows are the same record: pointer equality, else value equality.
            pub fn same(&self, other: &AnyRow) -> bool {
                match (self, other) {
                    (AnyRow::Project(a), AnyRow::Project(b)) => Arc::ptr_eq(a, b) || a == b,
                    $( (AnyRow::$variant(a), AnyRow::$variant(b)) => Arc::ptr_eq(a, b) || a == b, )*
                    _ => false,
                }
            }
        }

        /// The kinds stored in tables, in [`Kind`] order of declaration (project excluded).
        pub(crate) const TABLE_KINDS: &[Kind] = &[ $( Kind::$variant, )* ];

        /// The current row of a key.
        pub(crate) fn get_row(model: &Model, key: &AnyKey) -> Option<AnyRow> {
            let (kind, id) = *key;
            match kind {
                Kind::Project => (model.project.value.id.cast::<()>() == id)
                    .then(|| AnyRow::Project(Arc::clone(&model.project.value))),
                $( Kind::$variant => model.$($field).+.get(&id.cast()).map(|r| AnyRow::$variant(Arc::clone(r))), )*
                _ => None,
            }
        }

        /// Stores `after` (or removes the row) under its key and stamps row and table with `rev`. The
        /// caller has checked that the key's kind matches the row and that a project row is never removed.
        pub(crate) fn put_row(model: &mut Model, key: &AnyKey, after: Option<&AnyRow>, rev: Rev) {
            let (kind, id) = *key;
            match (kind, after) {
                (Kind::Project, Some(AnyRow::Project(r))) => {
                    model.project = crate::rev::Row::new(Arc::clone(r), rev);
                }
                $(
                    (Kind::$variant, Some(AnyRow::$variant(r))) => {
                        model.$($field).+.put(id.cast(), Some(Arc::clone(r)), rev)
                    }
                    (Kind::$variant, None) => model.$($field).+.put(id.cast(), None, rev),
                )*
                _ => unreachable!("put_row: kind and row checked by the caller"),
            }
        }

        /// Every key of every table row, in kind order, then ID order (project excluded).
        pub(crate) fn table_keys(model: &Model) -> Vec<AnyKey> {
            let mut out = Vec::new();
            $( out.extend(model.$($field).+.ids().map(|id| (Kind::$variant, id.cast::<()>()))); )*
            out
        }

        /// Whether two models hold the same rows in every table (`changed_at` ignored; project excluded).
        pub(crate) fn same_tables(a: &Model, b: &Model) -> bool {
            true $( && a.$($field).+.same_rows(&b.$($field).+) )*
        }

        /// The first table row that differs between two models or that one of them lacks, in kind order (the
        /// order of this list), then ID order (project excluded); `None` exactly when `same_tables` holds.
        pub(crate) fn first_row_difference(a: &Model, b: &Model) -> Option<(AnyKey, Presence)> {
            $( if let Some((id, presence)) = a.$($field).+.first_difference(&b.$($field).+) {
                return Some(((Kind::$variant, id.cast::<()>()), presence));
            } )*
            None
        }

    };
}

entities! {
    PlcConfigProject: PlcConfigProject, PlcConfigProject, plc_config_projects;
    LogicalNet: LogicalNet, LogicalNet, logical_nets;
    PhysicalNet: PhysicalNet, PhysicalNet, physical_nets;
    ImportLineage: ImportLineage, ImportLineage, lineages;
    StructureNode: StructureNode, StructureNode, structure;
    Section: Section, Section, sections;
    ExternalRef: ExternalRef, ExternalRef, ext_refs;
    Page: Page, Page, pages;
    Device: Device, Device, devices;
    Function: Function, Function, functions;
    Representation: Representation, Representation, reps;
    Connection: Connection, Connection, connections;
    Junction: Junction, Junction, junctions;
    Conductor: Conductor, Conductor, conductors;
    Channel: Channel, Channel, channels;
    PotentialDef: PotentialDef, PotentialDef, potential_defs;
    Bond: Bond, Bond, bonds;
    DeviceLink: DeviceLink, DeviceLink, device_links;
    PageItem: PageItem, PageItem, page_items;
    StructureBox: StructureBox, StructureBox, structure_boxes;
    ReportBlock: ReportBlock, ReportBlock, report_blocks;
    MacroInstance: MacroInstance, MacroInstance, macro_instances;
    LayoutSpace: LayoutSpace, LayoutSpace, layout_spaces;
    Enclosure: Enclosure, Enclosure, enclosures;
    MountingSurface: MountingSurface, MountingSurface, mounting_surfaces;
    Carrier: Carrier, Carrier, carriers;
    Placement: Placement, Placement, placements;
    Revision: Revision, Revision, revisions;
    Waiver: Waiver, Waiver, waivers;
    Part: LibCopy<Part>, Part, lib.parts;
    Symbol: LibCopy<SymbolDef>, SymbolDef, lib.symbols;
    Fndef: LibCopy<FnDef>, FnDef, lib.fndefs;
    Form: LibCopy<FormDef>, FormDef, lib.forms;
    Macro: LibCopy<MacroDef>, MacroDef, lib.macros;
    Manufacturer: LibCopy<Manufacturer>, Manufacturer, lib.manufacturers;
}

impl AnyRow {
    /// Wraps a typed record.
    pub fn of<T: Entity>(record: T) -> AnyRow {
        T::wrap(Arc::new(record))
    }

    /// The typed record, if the row is of kind `T`.
    pub fn get<T: Entity>(&self) -> Option<&Arc<T>> {
        T::unwrap(self)
    }
}

/// The ID of a key as a typed ID (no kind check).
pub(crate) fn typed<T>(key: &AnyKey) -> Id<T> {
    key.1.cast()
}
