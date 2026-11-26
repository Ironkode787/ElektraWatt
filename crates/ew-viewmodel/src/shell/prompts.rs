// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The table of the file dialogs' prompts: for each [`Prompt`] its title, the answer it asks for, the file name a
//! save starts with and its filters. The desktop program builds its native dialog from it; the window says nothing of
//! its own.

use super::{Prompt, ShellModel};
use crate::key;
use crate::strings;

/// The answer a file dialog asks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickKind {
    /// A folder.
    Folder,
    /// An existing file.
    File,
    /// The name of a file to write.
    Save,
}

/// What the file dialog of a [`Prompt`] shows.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PromptSpec {
    /// The dialog's title.
    pub title: &'static str,
    /// The answer it asks for.
    pub kind: PickKind,
    /// The file name a save starts with.
    pub file_name: Option<String>,
    /// The filters in the order offered, each a name with its extensions.
    pub filters: Vec<(String, Vec<&'static str>)>,
}

impl Prompt {
    /// The file dialog of this prompt in the window's language.
    pub fn spec(self) -> PromptSpec {
        let title = |k| strings::text(k);
        let name = |k| Some(strings::text(k).to_owned());
        let filter = |name: &str, extensions: &[&'static str]| (name.to_owned(), extensions.to_vec());
        let (title, kind, file_name, filters) = match self {
            Prompt::OpenFolder => (title(key!("prompt-open-title")), PickKind::Folder, None, vec![]),
            Prompt::NewFolder => (
                title(key!("prompt-save-as-title")),
                PickKind::Folder,
                None,
                vec![],
            ),
            Prompt::OpenQet => (
                title(key!("prompt-open-qet-title")),
                PickKind::File,
                None,
                vec![filter("QElectroTech", &["qet"])],
            ),
            Prompt::OpenPartsTable => (
                title(key!("prompt-open-parts-title")),
                PickKind::File,
                None,
                vec![filter("CSV", &["csv"])],
            ),
            Prompt::OpenImage => (
                title(key!("prompt-open-image-title")),
                PickKind::File,
                None,
                vec![filter(
                    strings::text(key!("prompt-image-filter")),
                    &["svg", "png", "jpg", "jpeg"],
                )],
            ),
            Prompt::OpenFont => (
                title(key!("prompt-open-font-title")),
                PickKind::File,
                None,
                vec![filter("TrueType, OpenType", &["ttf", "otf"])],
            ),
            Prompt::SavePdf => (
                title(key!("prompt-export-title")),
                PickKind::Save,
                name(key!("prompt-pdf-name")),
                vec![filter("PDF", &["pdf"])],
            ),
            // Output ▸ Export DXF…: the folder the set goes into.
            Prompt::SaveDxf => (
                title(key!("prompt-export-dxf-title")),
                PickKind::Folder,
                None,
                vec![],
            ),
            // The keymap editor's Import… and Export….
            Prompt::OpenKeymap => (
                title(key!("prompt-open-keymap-title")),
                PickKind::File,
                None,
                vec![filter("TOML", &["toml"])],
            ),
            Prompt::SaveKeymap => (
                title(key!("prompt-save-keymap-title")),
                PickKind::Save,
                name(key!("prompt-keymap-name")),
                vec![filter("TOML", &["toml"])],
            ),
            // A navigator table's Export table… and Import table….
            Prompt::SaveTable => (
                title(key!("prompt-export-table-title")),
                PickKind::Save,
                name(key!("prompt-table-name")),
                vec![filter("XLSX", &["xlsx"]), filter("CSV", &["csv"])],
            ),
            Prompt::OpenTable => (
                title(key!("prompt-open-table-title")),
                PickKind::File,
                None,
                vec![filter("XLSX", &["xlsx"]), filter("CSV", &["csv"])],
            ),
            // File ▸ Export .ewz… and Open .ewz….
            Prompt::SaveEwz => (
                title(key!("prompt-export-ewz-title")),
                PickKind::Save,
                name(key!("prompt-ewz-name")),
                vec![filter("EWZ", &["ewz"])],
            ),
            Prompt::OpenEwz => (
                title(key!("prompt-open-ewz-title")),
                PickKind::File,
                None,
                vec![filter("EWZ", &["ewz"])],
            ),
            // The chosen format's file: its extension names the file and its one filter.
            Prompt::SavePlcExchange(extension) => (
                title(key!("prompt-plc-export-title")),
                PickKind::Save,
                Some(format!("plc.{extension}")),
                vec![filter(&extension.to_uppercase(), &[extension])],
            ),
            // Rework list…'s Archive file… and Export….
            Prompt::OpenArchive => (
                title(key!("prompt-open-archive-title")),
                PickKind::File,
                None,
                vec![filter("EWARCHIVE", &["ewarchive"])],
            ),
            Prompt::LabelFolder => (
                title(key!("prompt-label-folder-title")),
                PickKind::Folder,
                None,
                vec![],
            ),
            Prompt::SaveRework => (
                title(key!("prompt-save-rework-title")),
                PickKind::Save,
                name(key!("prompt-rework-name")),
                vec![filter("XLSX", &["xlsx"]), filter("CSV", &["csv"])],
            ),
            Prompt::OpenPlcExchange => (
                title(key!("prompt-plc-import-title")),
                PickKind::File,
                None,
                vec![filter("AutomationML", &["aml"]), filter("XLSX", &["xlsx"])],
            ),
            // File ▸ Import…: a QElectroTech project or a project archive, both offered at once.
            Prompt::OpenImport => (
                title(key!("prompt-import-title")),
                PickKind::File,
                None,
                vec![filter(
                    strings::text(key!("prompt-import-filter")),
                    &ShellModel::import_extensions(),
                )],
            ),
            // A DWG import's converter: any program file.
            Prompt::OpenConverter => (
                title(key!("prompt-converter-title")),
                PickKind::File,
                None,
                vec![],
            ),
            // The symbol editor's file and library folder.
            Prompt::OpenSymbol => (
                title(key!("prompt-open-symbol-title")),
                PickKind::File,
                None,
                vec![filter(
                    strings::text(key!("prompt-symbol-filter")),
                    &super::symbol_editor::FILE_EXTENSIONS,
                )],
            ),
            Prompt::SaveSymbol => (
                title(key!("prompt-save-symbol-title")),
                PickKind::Save,
                name(key!("prompt-symbol-name")),
                vec![filter("JSON", &["json"])],
            ),
            Prompt::Folder => (title(key!("prompt-folder-title")), PickKind::Folder, None, vec![]),
            Prompt::LibraryFolder => (
                title(key!("prompt-library-folder-title")),
                PickKind::Folder,
                None,
                vec![],
            ),
        };
        PromptSpec {
            title,
            kind,
            file_name,
            filters,
        }
    }
}
