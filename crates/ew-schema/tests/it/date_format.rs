// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A date is shown and typed in the project's date format (a project set to
//! dd-mm-yyyy asked for YYYY-MM-DD): ISO is always taken; a day-first project takes a day, month and year with `-`,
//! `.` or `/` and a two- or four-digit year; what is not a date is refused.

use ew_schema::{Date, DateFormat, Formatting};

#[test]
fn a_date_reads_and_writes_in_the_projects_format() {
    let day_first = Formatting::default().date_format();
    assert_eq!(day_first, DateFormat::DdMmYyyy, "a project that names none");
    let d = Date::new(2025, 4, 14).unwrap();
    assert_eq!(day_first.format(d), "14-04-2025");
    assert_eq!(DateFormat::Iso8601.format(d), "2025-04-14");
    for typed in ["14-04-2025", "14.4.2025", "14/04/25", " 2025-04-14 "] {
        assert_eq!(day_first.parse(typed), Ok(d), "{typed}");
    }
    assert_eq!(DateFormat::Iso8601.parse("2025-04-14"), Ok(d));
    for wrong in [
        "14-04-2025",
        "31-04-2025",
        "2025-4-14",
        "14-04",
        "14-04-202",
        "",
        "aa-bb-cccc",
    ] {
        let format = if wrong == "14-04-2025" {
            DateFormat::Iso8601
        } else {
            DateFormat::DdMmYyyy
        };
        assert!(format.parse(wrong).is_err(), "{wrong} in {format:?}");
    }
    assert_eq!(day_first.example(), "31-12-2026");
    assert_eq!(DateFormat::Iso8601.example(), "2026-12-31");
}
