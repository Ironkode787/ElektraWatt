// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLCopen XML writer (doc 10 §4.6, FR-PLC-037): one global variable list [`GVL`] per configuration project,
//! in the namespace of TC6 v2.01, with one variable per channel with a determined symbol and an address.
//!
//! A variable's address is always the IEC rendering (`%QX0.1`), whatever profile the project draws with; its type
//! is the IEC elementary type of the channel's data type, a type the standard does not name a derived type; its
//! documentation is the channel's function text and its signal pin (`TA05-V-15 · -201U3-Q1:14`). The list sits
//! under `instances/configurations/configuration/resource`, and the file header takes the export's document date,
//! never the clock (NFR-REL-07). The bytes depend on the projection and the options only.

use std::fmt::Write as _;

use ew_derive::ew_plcaddr::Purpose;

use crate::image::Projection;

/// The name of the global variable list.
pub const GVL: &str = "IO_ElektraWatt";

/// The namespace of PLCopen XML TC6 v2.01.
pub const NAMESPACE: &str = "http://www.plcopen.org/xml/tc6_0201";

/// What a written file says beyond its variables.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct GvlOptions {
    /// The export's document date, `YYYY-MM-DD`.
    pub date: String,
    /// The version of the writing program.
    pub writer_version: String,
    /// The language of the documentation; `None`: the text without a language.
    pub lang: Option<String>,
}

/// The IEC 61131-3 elementary types, as PLCopen XML names their elements.
const ELEMENTARY: [&str; 21] = [
    "BOOL", "BYTE", "WORD", "DWORD", "LWORD", "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT",
    "ULINT", "REAL", "LREAL", "TIME", "DATE", "DT", "TOD", "string", "wstring",
];

/// The PLCopen XML file of `projection`'s tags.
pub fn write_gvl(projection: &Projection, options: &GvlOptions) -> String {
    let iec = ew_derive::builtin_profiles()
        .iter()
        .find(|p| p.reference().starts_with("iec-61131-3@"));
    let name = escape(&projection.image.project.name);
    let stamp = format!("{}T00:00:00", options.date);
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    let _ = writeln!(s, "<project xmlns=\"{NAMESPACE}\">");
    let _ = writeln!(
        s,
        "  <fileHeader companyName=\"ElektraWatt\" productName=\"ElektraWatt\" productVersion=\"{}\" \
         creationDateTime=\"{stamp}\"/>",
        escape(&options.writer_version)
    );
    let _ = writeln!(
        s,
        "  <contentHeader name=\"{name}\" modificationDateTime=\"{stamp}\">"
    );
    s.push_str("    <coordinateInfo>\n");
    for language in ["fbd", "ld", "sfc"] {
        let _ = writeln!(s, "      <{language}><scaling x=\"1\" y=\"1\"/></{language}>");
    }
    s.push_str("    </coordinateInfo>\n  </contentHeader>\n");
    s.push_str("  <types>\n    <dataTypes/>\n    <pous/>\n  </types>\n");
    s.push_str("  <instances>\n    <configurations>\n");
    let _ = writeln!(s, "      <configuration name=\"{name}\">");
    let _ = writeln!(s, "        <resource name=\"{name}\">");
    let _ = writeln!(s, "          <globalVars name=\"{GVL}\">");
    for (tag, info) in projection.image.tags.iter().zip(&projection.tags) {
        let address = iec
            .map(|p| p.render(&info.address, Purpose::Drawing))
            .unwrap_or_default();
        let _ = writeln!(
            s,
            "            <variable name=\"{}\" address=\"{}\">",
            escape(&tag.name),
            escape(&address)
        );
        let data_type = tag.data_type.as_deref().unwrap_or("Bool");
        let elementary = ELEMENTARY.iter().find(|e| e.eq_ignore_ascii_case(data_type));
        match elementary {
            Some(e) => {
                let _ = writeln!(s, "              <type><{e}/></type>");
            }
            None => {
                let _ = writeln!(
                    s,
                    "              <type><derived name=\"{}\"/></type>",
                    escape(data_type)
                );
            }
        }
        let text = info.function_text.as_ref().and_then(|t| {
            options
                .lang
                .as_deref()
                .and_then(|l| l.parse().ok())
                .and_then(|l| t.text_for(&l))
                .or_else(|| t.as_neutral())
        });
        let documentation: Vec<&str> = text.into_iter().chain(info.pin.as_deref()).collect();
        if !documentation.is_empty() {
            let _ = writeln!(
                s,
                "              <documentation><xhtml xmlns=\"http://www.w3.org/1999/xhtml\">{}</xhtml></documentation>",
                escape(&documentation.join(" · "))
            );
        }
        s.push_str("            </variable>\n");
    }
    s.push_str("          </globalVars>\n        </resource>\n      </configuration>\n");
    s.push_str("    </configurations>\n  </instances>\n</project>\n");
    s
}

/// `text` for an attribute value or character data; a character XML 1.0 cannot hold is left out.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if (c as u32) < 0x20 || c == '\u{fffe}' || c == '\u{ffff}' => {}
            c => out.push(c),
        }
    }
    out
}
