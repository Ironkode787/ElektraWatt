// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1378 as ruled by ID-1417: the pixel limit of a raster image applies at the gates that bring an image
//! in, never at open. A project holding an image over the limit, which an earlier build took in, opens with the image
//! kept, not drawn, and named in words; inserting such an image is refused.

use crate::common;

use std::path::Path;

use common::{Tmp, done};
use ew_viewmodel::ew_api::{Sha256, base64_encode};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

/// The header of a PNG file of `w` × `h` pixels: what the asset gate reads of it.
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
    b.extend_from_slice(&13u32.to_be_bytes());
    b.extend_from_slice(b"IHDR");
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    b
}

/// The SHA-256 of `bytes` in hex.
fn hex(bytes: &[u8]) -> String {
    let sums = ew_viewmodel::ew_api::archive::sha256sums([("f", bytes)]);
    sums.split_once("  ").unwrap().0.to_owned()
}

/// Every file under `dir`, recursively.
fn files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files(&path));
        } else {
            out.push(path);
        }
    }
    out
}

/// Turns the saved image `small` of the project at `dir` into `big`, as an earlier build without the pixel limit would
/// have saved it: the asset file, every record naming it and the manifest's hashes.
fn swap_asset(dir: &Path, small: &[u8], big: &[u8]) {
    let (from, to) = (hex(small), hex(big));
    let mut manifest = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
    for path in files(dir) {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name == "manifest.json" || name.ends_with(".png") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if text.contains(&from) {
            let changed = text.replace(&from, &to);
            manifest = manifest.replace(&hex(text.as_bytes()), &hex(changed.as_bytes()));
            std::fs::write(&path, changed).unwrap();
        }
    }
    let assets = dir.join("assets");
    std::fs::remove_file(assets.join(format!("{from}.png"))).unwrap();
    std::fs::write(assets.join(format!("{to}.png")), big).unwrap();
    std::fs::write(dir.join("manifest.json"), manifest.replace(&from, &to)).unwrap();
}

/// `item.place` of an image given as data on the page shown.
fn place(s: &mut ShellModel, bytes: &[u8]) -> Outcome {
    let page = s.canvas().unwrap().page().tables.page.to_string();
    s.dispatch(
        "command.run",
        &json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"image": {"data": base64_encode(bytes), "rect": [[0, 0], [2_000_000, 1_000_000]]}}}}),
    )
}

#[test]
fn an_image_over_the_pixel_limit_is_kept_at_open_and_refused_at_insertion() {
    let tmp = Tmp::new("ft1378-undrawn");
    let mut s = tmp.shell_in(Mode::Gui, true, "101");
    // The gate: an image over the limit is refused, in words.
    let big = png(65536, 32768);
    assert_eq!(
        place(&mut s, &big),
        Outcome::Failed {
            message: "Place item: Data: the png image is 65536 × 32768 pixels, more than a drawing can show (at most \
                      16777216 pixels and 65535 on a side); scale it down and add it again"
                .to_owned()
        }
    );
    // An earlier build took it in: a small image is placed and saved, then turned into the big one on disk.
    let small = png(2, 1);
    assert_eq!(place(&mut s, &small), Outcome::Done);
    done(&mut s, "file.save", json!({}));
    s.shutdown();
    swap_asset(&tmp.golden_copy(), &small, &big);

    for (locale, words) in [
        (
            "en",
            "The image on folio 101 is 65536 × 32768 pixels, more than a drawing can show, so it is kept but not \
             drawn; scale it down and add it again.",
        ),
        (
            "nl",
            "De afbeelding op folio 101 is 65536 × 32768 pixels, meer dan een tekening kan tonen, en wordt daarom \
             bewaard maar niet getekend; verklein haar en voeg haar opnieuw toe.",
        ),
    ] {
        let config = ShellConfig::in_dir(&tmp.0.join(format!("config-{locale}")), "0.0.0-test");
        let mut s = ShellModel::new(Mode::Gui, config);
        assert!(strings::set_locale(locale));
        let dir = tmp.golden_copy();
        done(&mut s, "file.open", json!({"dir": dir.to_string_lossy()}));
        let banner = s.banner().map(str::to_owned);
        let world = s.world_for_tests().expect("the project");
        let sha: Sha256 = format!("sha256:{}", hex(&big)).parse().unwrap();
        let kept = world.model.asset(&sha).is_some();
        s.shutdown();
        assert!(strings::set_locale("en"));
        assert_eq!(banner.as_deref(), Some(words), "{locale}");
        assert!(kept, "the open keeps the image ({locale})");
    }
}

/// An image over the pixel limit that the project already holds is refused when it is placed again
/// by its hash (`item.place {asset}`, `image.pick`), as its data and its file are; a project made from a template that
/// holds it names it as an open does.
#[test]
fn a_held_image_over_the_limit_is_refused_by_its_hash_and_named_after_a_template_copy() {
    let tmp = Tmp::new("ft1523-held");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    let (small, big) = (png(2, 1), png(65536, 32768));
    assert_eq!(place(&mut s, &small), Outcome::Done);
    done(&mut s, "file.save", json!({}));
    s.shutdown();
    swap_asset(&tmp.golden_copy(), &small, &big);
    let dir = tmp.golden_copy();
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(&mut s, "file.open", json!({"dir": dir.to_string_lossy()}));
    done(&mut s, "page.goto", json!({"folio": "101"}));
    let hash = format!("sha256:{}", hex(&big));
    let page = s.canvas().unwrap().page().tables.page.to_string();
    assert_eq!(
        s.dispatch(
            "command.run",
            &json!({"cmd": "item.place", "params": {"page": page,
                "kind": {"image": {"asset": hash, "rect": [[0, 0], [2_000_000, 1_000_000]]}}}}),
        ),
        Outcome::Failed {
            message:
                "Place item: Asset: the png image is 65536 × 32768 pixels, more than a drawing can show (at \
                      most 16777216 pixels and 65535 on a side); scale it down and add it again"
                    .to_owned()
        }
    );
    done(&mut s, "tool.image", json!({}));
    assert_eq!(
        s.dispatch("image.pick", &json!({ "asset": hash })),
        Outcome::Failed {
            message: format!(
                "The image {hash} is not accepted: the png image is 65536 × 32768 pixels, more than a drawing can \
                 show (at most 16777216 pixels and 65535 on a side); scale it down and add it again."
            )
        }
    );
    assert_eq!(s.tool().state(), "picking", "the tool is unchanged");
    // Both refusals in a Dutch window's language.
    done(&mut s, "view.language", json!({ "value": "nl" }));
    assert_eq!(
        s.dispatch(
            "command.run",
            &json!({"cmd": "item.place", "params": {"page": page,
                "kind": {"image": {"asset": hash, "rect": [[0, 0], [2_000_000, 1_000_000]]}}}}),
        ),
        Outcome::Failed {
            message: "Item plaatsen: Bestand: ongeldige invoer: de png-afbeelding is 65536 × 32768 pixels, meer dan \
                      een tekening kan tonen (ten hoogste 16777216 pixels en 65535 per zijde); verklein haar en voeg \
                      haar opnieuw toe"
                .to_owned()
        }
    );
    done(&mut s, "tool.image", json!({}));
    assert_eq!(
        s.dispatch("image.pick", &json!({ "asset": hash })),
        Outcome::Failed {
            message: format!(
                "De afbeelding {hash} wordt niet aanvaard: de png-afbeelding is 65536 × 32768 pixels, meer dan een \
                 tekening kan tonen (ten hoogste 16777216 pixels en 65535 per zijde); verklein haar en voeg haar \
                 opnieuw toe."
            )
        }
    );
    done(&mut s, "view.language", json!({ "value": "en" }));
    done(&mut s, "file.close", json!({}));
    let made = tmp.0.join("made");
    done(
        &mut s,
        "file.new",
        json!({"template": dir.to_string_lossy(), "dir": made.to_string_lossy()}),
    );
    assert_eq!(
        s.banner(),
        Some(
            "The image on folio 101 is 65536 × 32768 pixels, more than a drawing can show, so it is kept but not \
             drawn; scale it down and add it again."
        )
    );
    s.shutdown();
}
