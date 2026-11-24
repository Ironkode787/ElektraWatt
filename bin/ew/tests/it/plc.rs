// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Through the `ew` binary: the PLC exchange (doc 10 §7). An export writes its file and records the exchange in the
//! saved project; the file comes back as a preview and, with `--accept`, as one applied import; `ew plc status`
//! tells the state; an instrument list is previewed and applied.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Output;

use crate::support::{Tmp, stderr, stdout};

fn ok(o: &Output) -> String {
    assert_eq!(o.status.code(), Some(0), "{}", stderr(o));
    stdout(o)
}

#[test]
fn an_export_is_recorded_and_its_file_comes_back() {
    let t = Tmp::new("ew-plc-test");
    let project = t.sample("sample.ew");
    let aml = t.path("PLC1.aml");

    let o = t.ew(
        &[
            "export".as_ref(),
            "plc-aml".as_ref(),
            project.as_os_str(),
            "--out".as_ref(),
            aml.as_os_str(),
        ],
    );
    let text = ok(&o);
    // The sample's PLC: the CPU's station with its input and output card, and the valve terminal's station on the
    // PROFIBUS with its one module; 8 channels each.
    assert!(text.contains("2 stations, 3 modules, 24 channels"), "{text}");
    let written = std::fs::read_to_string(&aml).unwrap();
    assert!(written.contains("SchemaVersion=\"2.15\""));
    // the snapshot is in the saved project, in the configuration project's own file
    let snapshots: Vec<_> = std::fs::read_dir(project.join("exchange"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(snapshots.len(), 1, "one configuration project, one exchange file");
    let snapshot = std::fs::read_to_string(snapshots[0].path()).unwrap();
    assert!(snapshot.contains("\"$k\":\"exchange_record\""));

    let status = |file: &Path| {
        ok(&t.ew(
            &[
                "plc".as_ref(),
                "status".as_ref(),
                file.as_os_str(),
                "--project".as_ref(),
                project.as_os_str(),
            ],
        ))
    };
    let text = status(&aml);
    assert!(
        text.contains("PLC1: last exchange: AML 1.4.0 export of "),
        "{text}"
    );
    assert!(text.contains("the project is as exchanged"), "{text}");
    assert!(text.contains("the file is the one last exchanged"), "{text}");

    // the preview of the file that comes back unchanged, and nothing is written
    let import = |extra: &[&OsStr]| {
        let mut args: Vec<&OsStr> = vec![
            "import".as_ref(),
            "plc-aml".as_ref(),
            aml.as_os_str(),
            project.as_os_str(),
        ];
        args.extend(extra);
        t.ew(&args)
    };
    let text = ok(&import(&[]));
    assert!(
        text.contains("0 change(s), 0 parked, 0 removed; nothing is written"),
        "{text}"
    );
    assert!(
        status(&aml).contains("last exchange: AML 1.4.0 export of "),
        "a preview leaves the project as it was"
    );

    let choices = t.path("choices.json");
    std::fs::write(&choices, "{}").unwrap();
    let text = ok(&import(&["--accept".as_ref(), choices.as_os_str()]));
    assert!(text.contains("applied and saved"), "{text}");
    assert!(status(&aml).contains("last exchange: AML import of "));

    // a choice that names nothing is refused in words, and so is an export without a file
    std::fs::write(&choices, "{\"tag/none/name\": \"theirs\"}").unwrap();
    let o = import(&["--accept".as_ref(), choices.as_os_str()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("is no change and no parked object of this import"),
        "{}",
        stderr(&o)
    );
    let o = t.ew(&["export".as_ref(), "plcopen".as_ref(), project.as_os_str()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("give `--out <file>`"), "{}", stderr(&o));
}

#[test]
fn the_tag_table_and_the_gvl_are_written_under_both_names() {
    let t = Tmp::new("ew-plc-test");
    let project = t.sample("sample.ew");
    for (word, file) in [
        ("plc-xlsx", "tags.xlsx"),
        ("plc-tags", "tags2.xlsx"),
        ("plcopen", "io.xml"),
    ] {
        let out = t.path(file);
        let o = t.ew(
            &[
                "export".as_ref(),
                word.as_ref(),
                project.as_os_str(),
                "--out".as_ref(),
                out.as_os_str(),
            ],
        );
        ok(&o);
        assert!(out.is_file(), "{word}");
    }
    assert_eq!(
        std::fs::read(t.path("tags.xlsx")).unwrap(),
        std::fs::read(t.path("tags2.xlsx")).unwrap(),
        "`plc-tags` is `plc-xlsx`"
    );
    assert!(
        std::fs::read_to_string(t.path("io.xml"))
            .unwrap()
            .contains("IO_ElektraWatt")
    );
    let o = t.ew(
        &[
            "export".as_ref(),
            "plcopen".as_ref(),
            project.as_os_str(),
            "--apc".as_ref(),
            "1.2.0".as_ref(),
        ],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("`--apc` is an option of `export plc-aml`"),
        "{}",
        stderr(&o)
    );
}

#[test]
fn an_instrument_list_is_previewed_and_applied() {
    let t = Tmp::new("ew-plc-test");
    let project = t.sample("sample.ew");
    let list = t.path("instruments.csv");
    std::fs::write(
        &list,
        "tag,signal type,description\nTA05-V-40,DO,Valve 40\nTA05-T-01,AI,Temperature\n",
    )
    .unwrap();
    let run = |extra: &[&OsStr]| {
        let mut args: Vec<&OsStr> = vec![
            "import".as_ref(),
            "instrument-list".as_ref(),
            list.as_os_str(),
            project.as_os_str(),
        ];
        args.extend(extra);
        t.ew(&args)
    };
    let text = ok(&run(&[]));
    // The first spare output of the output card; the sample has no analogue input.
    assert!(
        text.contains("TA05-V-40: channel 0 of -104A1-A2 reserved"),
        "{text}"
    );
    assert!(text.contains("TA05-T-01: no spare AI channel"), "{text}");
    assert!(text.contains("nothing is written: apply with --apply"), "{text}");
    let text = ok(&run(&["--apply".as_ref()]));
    assert!(text.contains("applied and saved"), "{text}");
    // applied: the row has its channel, so the same list reserves no other
    let text = ok(&run(&[]));
    assert!(
        text.contains("TA05-V-40: channel 0 of -104A1-A2 reserved"),
        "{text}"
    );
}
