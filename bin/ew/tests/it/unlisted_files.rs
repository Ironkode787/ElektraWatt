// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A file added to a project by hand that `manifest.json` does not list is not read, and the commands that open the
//! project say so on stderr instead of the file being silently not there (FT-1879).

use crate::support::{EwTmp, Tmp, args, stderr};

#[test]
fn a_reader_and_a_writer_name_a_library_file_the_manifest_does_not_list() {
    let t = Tmp::new("ew-unlisted");
    let p = t.tiny("p.ew");
    std::fs::create_dir_all(p.join("lib").join("symbols")).unwrap();
    std::fs::write(
        p.join("lib")
            .join("symbols")
            .join("00000000-0000-4000-8000-000000000001.json"),
        "{}\n",
    )
    .unwrap();
    let warning = "warning: lib/symbols/00000000-0000-4000-8000-000000000001.json: manifest.json does not list it, \
                   so it is not read";
    for run in [args([&"layout", &p]), args([&"fmt", &"--check", &p]), args([&"fmt", &p])] {
        let o = t.ew(&run);
        assert!(stderr(&o).contains(warning), "{run:?}: {}", stderr(&o));
    }
}
