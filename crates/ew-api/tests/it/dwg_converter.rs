// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ODA File Converter's run (doc 14 §6.5, §11): a converter that does not finish is stopped with what it started,
//! and the working folder of its copy is removed whichever way the run ends.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ew_api::dxf::{DwgError, convert_dwg};

/// A converter that records the folder it was given and then runs for half a minute, in a child of its own where the
/// platform has one to record (`pids`).
fn hanging_converter(dir: &Path, folder: &Path, pids: &Path) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let script = dir.join("converter.sh");
        let text = format!(
            "#!/bin/sh\necho \"$1\" > \"{}\"\nsleep 30 &\necho $! > \"{}\"\nwait\n",
            folder.display(),
            pids.display()
        );
        std::fs::write(&script, text).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }
    #[cfg(windows)]
    {
        let _ = pids;
        let script = dir.join("converter.cmd");
        let text = format!(
            "@echo off\r\necho %1> \"{}\"\r\nping -n 30 127.0.0.1 > nul\r\n",
            folder.display()
        );
        std::fs::write(&script, text).unwrap();
        script
    }
}

#[test]
fn a_converter_that_does_not_finish_is_stopped_and_its_folder_removed() {
    let tmp = ew_testdir::Tmp::new("dwg-converter");
    let (folder, pids) = (tmp.path("folder.txt"), tmp.path("pids.txt"));
    let converter = hanging_converter(tmp.dir(), &folder, &pids);
    let e = convert_dwg(
        &converter,
        "plan.dwg",
        b"not a drawing",
        Duration::from_millis(1500),
    )
    .unwrap_err();
    assert_eq!(e, DwgError::TimedOut);

    // The folder the converter was given (`<work>/in`) is gone with its work folder.
    let given = std::fs::read_to_string(&folder).expect("the converter ran");
    let work = Path::new(given.trim()).parent().unwrap().to_path_buf();
    assert!(!work.exists(), "the work folder is removed after a timeout");

    // What the converter started is stopped with it (a process group on Unix).
    #[cfg(unix)]
    {
        let pid = std::fs::read_to_string(&pids).unwrap().trim().to_owned();
        let alive = || {
            std::process::Command::new("kill")
                .args(["-0", &pid])
                .output()
                .is_ok_and(|o| o.status.success())
        };
        let mut waited = 0;
        while alive() && waited < 50 {
            std::thread::sleep(Duration::from_millis(100));
            waited += 1;
        }
        assert!(!alive(), "the converter's own child was stopped");
    }
}
