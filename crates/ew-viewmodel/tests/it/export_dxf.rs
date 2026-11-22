// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Output ▸ Export DXF… (`file.export_dxf`, doc 09 §4.9, FT-1960): the DXF set of the open project as a job, written
//! into a folder as a whole, one file per page of the page order and the pictures they name, the toast saying so in
//! words; a folder that holds other files is refused before the job starts and left as it was.

use std::collections::BTreeSet;
use std::path::Path;

use crate::common::{Tmp, done, refused};
use ew_api::library::file::FILE_SET_MARK;
use ew_api::output::PageSelection;
use ew_viewmodel::strings;
use serde_json::json;

fn path_text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn names(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn the_dxf_set_goes_into_its_folder_and_a_folder_of_other_files_is_refused() {
    let tmp = Tmp::new("export-dxf");
    let mut s = tmp.sample_shell("101");
    let dir = tmp.0.join("set");
    done(&mut s, "file.export_dxf", json!({"dir": path_text(&dir)}));
    // One file per page of the page order, by the name the export gives a page, and the set's mark.
    let order: BTreeSet<String> = {
        let session = ew_api::Session::open_read_only(Path::new(crate::common::SAMPLE), "test")
            .expect("the sample opens")
            .0;
        let options = ew_api::output::ExportOptions {
            pages: PageSelection::All,
            ..Default::default()
        };
        session
            .export_pages(&options)
            .expect("the pages")
            .pages
            .iter()
            .map(|p| ew_api::output::dxf_file_name(&p.label))
            .chain([FILE_SET_MARK.to_owned()])
            .collect()
    };
    assert_eq!(names(&dir), order);
    let toast = s.toast().expect("a toast").text.clone();
    assert!(toast.starts_with("Exported the DXF set to set: "), "{toast}");
    assert!(
        s.describe()
            .contains("item file.export_dxf \"Export DXF…\" enabled"),
        "in the Output menu beside Export PDF"
    );
    // Again into the same folder: the set replaces itself, and the toast says "again".
    done(&mut s, "file.export_dxf", json!({"dir": path_text(&dir)}));
    assert_eq!(names(&dir), order);
    assert!(s.toast().expect("a toast").text.contains(" again: "));

    // A folder of the person's own pictures (a file type a DXF set holds too): refused in words, nothing in it changes.
    let mine = tmp.0.join("mine");
    std::fs::create_dir_all(&mine).unwrap();
    std::fs::write(mine.join("shot.png"), b"mine").unwrap();
    let reason = refused(&mut s, "file.export_dxf", json!({"dir": path_text(&mine)}));
    assert_eq!(names(&mine), BTreeSet::from(["shot.png".to_owned()]));
    assert_eq!(std::fs::read(mine.join("shot.png")).unwrap(), b"mine");
    assert_eq!(
        strings::lookup(reason),
        Some(
            "The folder holds files that no earlier DXF export wrote there; choose an empty folder or one an earlier \
             DXF export wrote"
        )
    );
    assert!(strings::set_locale("nl"));
    let dutch = strings::lookup(reason);
    assert!(strings::set_locale("en"));
    assert_eq!(
        dutch,
        Some(
            "De map bevat bestanden die geen eerdere DXF-export daar schreef; kies een lege map of een map die een \
             eerdere DXF-export schreef"
        )
    );
    s.shutdown();
}
