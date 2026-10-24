// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Run folders, the run's files and the formats `ew.blindrun/1` and `ew.blindturns/1` (WP-S6.13 §5.1).
//!
//! A run is a folder outside the repository. The harness writes no time stamp, host name, user name or process ID
//! into any file of it, and every list it writes has a named order, so two runs of equal steps leave equal files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The `$schema` of `run.json`.
pub const RUN_SCHEMA: &str = "ew.blindrun/1";
/// The `$schema` of the header of `turns.jsonl`.
pub const TURNS_SCHEMA: &str = "ew.blindturns/1";
/// The `$schema` of a UI script.
pub const SCRIPT_SCHEMA: &str = "ew.uiscript/1";

/// The version of the harness recorded in `run.json`.
pub const HARNESS_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Lower-case hexadecimal of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// The SHA-256 of `bytes` in hexadecimal.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// The digest over every file under `dir`, in path order: each file's relative path (with `/`), a NUL, its SHA-256 and
/// a newline, hashed again. A text file's line ends do not matter (the fixture is read the same on every checkout).
pub fn tree_sha(dir: &Path) -> Result<String> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .with_context(|| format!("reading {}", dir.display()))?
            .collect::<std::io::Result<_>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            let rel = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            if e.file_type()?.is_dir() {
                walk(&path, &rel, out)?;
            } else {
                out.push((rel, path));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, "", &mut files)?;
    let mut all = String::new();
    for (rel, path) in files {
        let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        // A line-end difference of a text checkout must not change the digest: hash text files line-end free.
        let digest = match std::str::from_utf8(&bytes) {
            Ok(text) => sha256_hex(text.replace("\r\n", "\n").as_bytes()),
            Err(_) => sha256_hex(&bytes),
        };
        all.push_str(&format!("{rel}\0{digest}\n"));
    }
    Ok(sha256_hex(all.as_bytes()))
}

/// `value` as compact JSON with the members of every object in key order, whatever the map implementation is.
pub fn canonical(value: &Value) -> String {
    match value {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let members: Vec<String> = keys
                .into_iter()
                .map(|k| format!("{}:{}", Value::from(k.as_str()), canonical(&m[k])))
                .collect();
            format!("{{{}}}", members.join(","))
        }
        Value::Array(a) => {
            let items: Vec<String> = a.iter().map(canonical).collect();
            format!("[{}]", items.join(","))
        }
        other => other.to_string(),
    }
}

/// Writes `text` to `path` through a temporary file in the same folder, so a reader never sees half a file.
pub fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let tmp = path.with_extension(match path.extension() {
        Some(e) => format!("{}.tmp", e.to_string_lossy()),
        None => "tmp".to_owned(),
    });
    std::fs::write(&tmp, text).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

/// Appends `text` to the file `path` (created when absent).
pub fn append(path: &Path, text: &str) -> Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    f.write_all(text.as_bytes())
        .with_context(|| format!("writing {}", path.display()))
}

/// Copies the folder `from` to `to`, files and sub-folders.
pub fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to).with_context(|| format!("creating {}", to.display()))?;
    for entry in std::fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)
                .with_context(|| format!("copying {}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// The kinds of run (D3 and the check run of §5.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// The agent chooses its own task.
    Free,
    /// A fixed task sheet with an oracle.
    Gate,
    /// A second agent audits a stopped gate run.
    Check,
}

impl Kind {
    /// The kind's word in `run.json` and on the command line.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Free => "free",
            Kind::Gate => "gate",
            Kind::Check => "check",
        }
    }

    /// The kind of a word.
    pub fn parse(word: &str) -> Result<Kind> {
        match word {
            "free" => Ok(Kind::Free),
            "gate" => Ok(Kind::Gate),
            "check" => Ok(Kind::Check),
            other => bail!("unknown kind `{other}` (free, gate or check)"),
        }
    }

    /// The caps of the kind (§5.2.2 item 7).
    pub fn caps(self) -> Caps {
        match self {
            Kind::Gate => Caps {
                turns: 900,
                steps: 3000,
                looks: 1500,
                refusals: 100,
            },
            Kind::Free => Caps {
                turns: 300,
                steps: 800,
                looks: 400,
                refusals: 50,
            },
            Kind::Check => Caps {
                turns: 200,
                steps: 600,
                looks: 600,
                refusals: 50,
            },
        }
    }
}

/// The counted caps of a run: turns, agent steps, looks and harness refusals (never times).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Caps {
    /// `do` calls.
    pub turns: u32,
    /// Steps the agent sent to the product.
    pub steps: u32,
    /// `look` calls.
    pub looks: u32,
    /// Steps the harness refused.
    pub refusals: u32,
}

/// What a run has used so far.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Counts {
    /// `do` calls.
    pub turns: u32,
    /// Steps the agent sent to the product.
    pub steps: u32,
    /// `look` calls.
    pub looks: u32,
    /// `do` and `look` calls the harness refused.
    pub refusals: u32,
    /// Steps the product refused.
    pub refused: u32,
    /// Steps the product failed.
    pub failed: u32,
    /// Steps marked broken.
    pub broken: u32,
}

/// `run.json`, format `ew.blindrun/1` (additive within `/1`).
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct RunJson {
    /// `ew.blindrun/1`.
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The run's ID.
    pub run: String,
    /// `free`, `gate` or `check`.
    pub kind: String,
    /// The task.
    pub task: String,
    /// The persona, for a drafter.
    #[serde(default)]
    pub persona: Option<String>,
    /// For a check run: the drafter run's ID.
    #[serde(default)]
    pub of: Option<String>,
    /// The folios the epilogue describes.
    #[serde(default)]
    pub folios: Vec<String>,
    /// The caps.
    pub caps: Caps,
    /// Path (relative to the repository root, with `/`) to SHA-256 of every composed prompt file, the task files and
    /// the reference files.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    /// The SHA-256 over every file under `golden/` at `start`.
    pub golden: String,
    /// `elektrawatt --version`.
    pub app_version: String,
    /// The harness's version.
    pub harness_version: String,
    /// Whether the run has no keeper (`--stateless`).
    #[serde(default)]
    pub stateless: bool,
    /// What the run has used.
    #[serde(default)]
    pub counts: Counts,
    /// How the run ended: `handback`, `cap`, `quit`, `orchestrator`, `diverged` or `lost`; `None` while it goes on.
    #[serde(default)]
    pub end: Option<String>,
    /// Whether the hand-back is filed (`end` keeps the first cause).
    #[serde(default)]
    pub handed_back: bool,
    /// The first step (1-based, over the prelude and the steps) where a replay differed, for `end: diverged`.
    #[serde(default)]
    pub diverged_at: Option<usize>,
    /// `ci`: `none`, `pr` or `nightly`; set by `bless`.
    pub ci: String,
    /// `null`, or why the recording no longer replays; never set on the self-test recording.
    #[serde(default)]
    pub stale: Option<String>,
    /// After `stop`: whether the project was clean after the agent's last step.
    #[serde(default)]
    pub clean_before_epilogue: Option<bool>,
    /// After `stop`: the exit code of the product.
    #[serde(default)]
    pub child_exit: Option<i32>,
}

/// What a step the product did not do looked like (the broken rule of §5.2.2 item 4).
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct NotDone {
    /// The action.
    pub action: String,
    /// The parameters as canonical JSON.
    pub params: String,
    /// `refused` or `failed`.
    pub outcome: String,
    /// The reason key of a refusal or the message of a failure.
    #[serde(default)]
    pub reason: Option<String>,
}

impl NotDone {
    /// The key of the exact step (action and parameters) the broken rule refuses.
    pub fn step_key(&self) -> String {
        format!("{} {}", self.action, self.params)
    }
}

/// One line of `turns.jsonl` (format `ew.blindturns/1`).
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct Turn {
    /// The line's number among the turn lines, from 1.
    pub n: u32,
    /// `do`, `look`, `note`, `handback` or `epilogue`.
    pub kind: String,
    /// For `do`: the turn's number; for `note`: the turn it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<u32>,
    /// The inclusive range of `steps.uiscript` lines the call sent, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<[u32; 2]>,
    /// The folio shown after the call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folio: Option<String>,
    /// The text of the harness refusal that ended the call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    /// For `do`: the first step that was not done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_not_done: Option<NotDone>,
    /// For `do`: the step this call marked broken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broken: Option<NotDone>,
    /// For `note`: the patience score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patience: Option<u8>,
    /// For `note`: the flow score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<u8>,
}

/// The directory of a run and the paths inside it.
#[derive(Clone, Debug)]
pub struct Run {
    /// The run's folder.
    pub dir: PathBuf,
}

impl Run {
    /// The default folder of run `id`: `<temp>/ew-blind/<id>`.
    pub fn default_dir(id: &str) -> PathBuf {
        std::env::temp_dir().join("ew-blind").join(id)
    }

    /// A path inside the run folder.
    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    /// `run.json`.
    pub fn json_path(&self) -> PathBuf {
        self.path("run.json")
    }

    /// The copied harness: the agent's only executable.
    pub fn harness_exe(&self) -> PathBuf {
        self.path(&format!("bin/xtask{}", std::env::consts::EXE_SUFFIX))
    }

    /// A copied product binary.
    pub fn product_exe(&self, name: &str) -> PathBuf {
        self.path(&format!(".harness/bin/{name}{}", std::env::consts::EXE_SUFFIX))
    }

    /// The command the composed prompt calls `{blind}`.
    pub fn blind_command(&self) -> String {
        format!("{} blind", self.harness_exe().display())
    }

    /// Reads `run.json`.
    pub fn load(&self) -> Result<RunJson> {
        let text = std::fs::read_to_string(self.json_path())
            .with_context(|| format!("reading {}", self.json_path().display()))?;
        serde_json::from_str(&text).with_context(|| format!("parsing {}", self.json_path().display()))
    }

    /// Writes `run.json` (pretty, with a final newline).
    pub fn save(&self, json: &RunJson) -> Result<()> {
        let mut text = serde_json::to_string_pretty(json)?;
        text.push('\n');
        write_atomic(&self.json_path(), &text)
    }

    /// Changes `run.json` through `f` and writes it back.
    pub fn update(&self, f: impl FnOnce(&mut RunJson)) -> Result<RunJson> {
        let mut json = self.load()?;
        f(&mut json);
        self.save(&json)?;
        Ok(json)
    }

    /// The lines of `turns.jsonl` after its header.
    pub fn turns(&self) -> Result<Vec<Turn>> {
        let path = self.path("turns.jsonl");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        let mut out = Vec::new();
        for (i, line) in text.lines().enumerate().skip(1) {
            if line.trim().is_empty() {
                continue;
            }
            out.push(serde_json::from_str(line).with_context(|| format!("turns.jsonl line {}", i + 1))?);
        }
        Ok(out)
    }

    /// Appends a turn line, numbering it.
    pub fn push_turn(&self, mut turn: Turn) -> Result<Turn> {
        turn.n = u32::try_from(self.turns()?.len())
            .unwrap_or(u32::MAX)
            .saturating_add(1);
        append(
            &self.path("turns.jsonl"),
            &format!("{}\n", serde_json::to_string(&turn)?),
        )?;
        Ok(turn)
    }

    /// The steps of `steps.uiscript` after its header: `(action, params)` of each line.
    pub fn steps(&self) -> Result<Vec<(String, Value)>> {
        read_script(&self.path("steps.uiscript"))
    }

    /// The lines of `transcript.jsonl` after its header, as written.
    pub fn transcript(&self) -> Result<Vec<String>> {
        let text = match std::fs::read_to_string(self.path("transcript.jsonl")) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        Ok(text.lines().skip(1).map(str::to_owned).collect())
    }
}

/// The steps of a UI script file after its header.
pub fn read_script(path: &Path) -> Result<Vec<(String, Value)>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_script(&text).with_context(|| path.display().to_string())
}

/// The steps of a UI script text after its header.
pub fn parse_script(text: &str) -> Result<Vec<(String, Value)>> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(line).with_context(|| format!("line {}", i + 1))?;
        if v.get("$schema").is_some() {
            continue;
        }
        let action = v
            .get("action")
            .and_then(Value::as_str)
            .with_context(|| format!("line {}: no action", i + 1))?
            .to_owned();
        let params = v
            .get("params")
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));
        out.push((action, params));
    }
    Ok(out)
}

/// A UI script line for a step: `{"action":…,"params":{…}}` with the members of `params` in key order.
pub fn step_line(action: &str, params: &Value) -> String {
    format!(
        "{{\"action\":{},\"params\":{}}}",
        Value::from(action),
        canonical(params)
    )
}

/// The header line of a UI script.
pub fn script_header(label: &str) -> String {
    format!(
        "{{\"$schema\":\"{SCRIPT_SCHEMA}\",\"label\":{}}}",
        Value::from(label)
    )
}

/// The header line of `turns.jsonl`.
pub fn turns_header() -> String {
    format!("{{\"$schema\":\"{TURNS_SCHEMA}\"}}")
}

/// Whether `id` is a legal run ID: ASCII letters, digits, `.`, `_` and `-`, at most 64 characters, not starting
/// with a dot.
pub fn valid_run_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Finds the run named by `arg` (its ID or its folder). The harness copied into a run folder always works on that
/// folder; from the repository the folder is `<out>/<id>` or `<temp>/ew-blind/<id>`.
pub fn locate(arg: &str, out: Option<&Path>, exe: Option<&Path>) -> Result<Run> {
    if let Some(exe) = exe
        && let Some(bin) = exe.parent()
        && bin.file_name().is_some_and(|n| n == "bin")
        && let Some(dir) = bin.parent()
        && dir.join("run.json").is_file()
    {
        let run = Run {
            dir: dir.to_path_buf(),
        };
        let id = run.load()?.run;
        if arg != id && Path::new(arg) != dir {
            bail!("this harness belongs to run `{id}`, not `{arg}`");
        }
        return Ok(run);
    }
    let dir = if Path::new(arg).join("run.json").is_file() {
        PathBuf::from(arg)
    } else if valid_run_id(arg) {
        out.map_or_else(|| Run::default_dir(arg), |o| o.join(arg))
    } else {
        bail!("`{arg}` is neither a run ID nor a run folder");
    };
    if !dir.join("run.json").is_file() {
        bail!("no run `{arg}` (looked for {})", dir.display());
    }
    Ok(Run { dir })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blind::testkit::Tmp;

    #[test]
    fn canonical_json_sorts_the_members_at_every_depth() {
        let v: Value = serde_json::from_str(r#"{"b":1,"a":{"d":[{"z":1,"y":2}],"c":"x"}}"#).unwrap();
        assert_eq!(canonical(&v), r#"{"a":{"c":"x","d":[{"y":2,"z":1}]},"b":1}"#);
        assert_eq!(
            step_line("canvas.press", &v),
            format!("{{\"action\":\"canvas.press\",\"params\":{}}}", canonical(&v))
        );
    }

    #[test]
    fn run_ids_are_plain_names() {
        for ok in ["gpp-r1", "free-2026-11-a", "selftest", "a.b_c"] {
            assert!(valid_run_id(ok), "{ok}");
        }
        for bad in ["", ".hidden", "a b", "a/b", "a\\b", "..", &"x".repeat(65)] {
            assert!(!valid_run_id(bad), "{bad}");
        }
    }

    #[test]
    fn the_tree_digest_ignores_line_ends_and_sees_every_change() {
        let tmp = Tmp::new("tree");
        std::fs::create_dir_all(tmp.0.join("a/b")).unwrap();
        std::fs::write(tmp.0.join("a/x.txt"), "one\ntwo\n").unwrap();
        std::fs::write(tmp.0.join("a/b/y.bin"), [0u8, 159, 146, 150]).unwrap();
        let first = tree_sha(&tmp.0).unwrap();
        std::fs::write(tmp.0.join("a/x.txt"), "one\r\ntwo\r\n").unwrap();
        assert_eq!(tree_sha(&tmp.0).unwrap(), first);
        std::fs::write(tmp.0.join("a/b/y.bin"), [0u8, 159, 146, 151]).unwrap();
        assert_ne!(tree_sha(&tmp.0).unwrap(), first);
        std::fs::write(tmp.0.join("new.txt"), "").unwrap();
        let with_new = tree_sha(&tmp.0).unwrap();
        std::fs::remove_file(tmp.0.join("new.txt")).unwrap();
        assert_ne!(tree_sha(&tmp.0).unwrap(), with_new);
    }

    #[test]
    fn run_json_and_turns_round_trip() {
        let tmp = Tmp::new("roundtrip");
        let run = Run { dir: tmp.0.clone() };
        let json = RunJson {
            schema: RUN_SCHEMA.to_owned(),
            run: "r1".to_owned(),
            kind: "free".to_owned(),
            task: "selftest".to_owned(),
            persona: None,
            of: None,
            folios: vec!["101".to_owned()],
            caps: Kind::Free.caps(),
            files: BTreeMap::from([("tools/blind/HELP.md".to_owned(), "ab".to_owned())]),
            golden: "cd".to_owned(),
            app_version: "0.0.0".to_owned(),
            harness_version: HARNESS_VERSION.to_owned(),
            stateless: false,
            counts: Counts::default(),
            end: None,
            handed_back: false,
            diverged_at: None,
            ci: "none".to_owned(),
            stale: None,
            clean_before_epilogue: None,
            child_exit: None,
        };
        run.save(&json).unwrap();
        assert_eq!(run.load().unwrap(), json);
        let text = std::fs::read_to_string(run.json_path()).unwrap();
        assert!(text.contains("\"$schema\": \"ew.blindrun/1\""), "{text}");
        // No time, host, user or process ID: the keys are the documented ones.
        for key in ["time", "date", "host", "user", "pid"] {
            assert!(!text.contains(&format!("\"{key}\"")), "{key}");
        }
        std::fs::write(run.path("turns.jsonl"), format!("{}\n", turns_header())).unwrap();
        let t = run
            .push_turn(Turn {
                kind: "do".to_owned(),
                turn: Some(1),
                steps: Some([1, 2]),
                first_not_done: Some(NotDone {
                    action: "edit.redo".to_owned(),
                    params: "{}".to_owned(),
                    outcome: "refused".to_owned(),
                    reason: Some("reason-nothing-to-redo".to_owned()),
                }),
                ..Turn::default()
            })
            .unwrap();
        assert_eq!(t.n, 1);
        run.push_turn(Turn {
            kind: "note".to_owned(),
            turn: Some(1),
            patience: Some(9),
            flow: Some(8),
            ..Turn::default()
        })
        .unwrap();
        let turns = run.turns().unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[1].n, 2);
        assert_eq!(
            turns[0].first_not_done.as_ref().unwrap().step_key(),
            "edit.redo {}"
        );
    }

    #[test]
    fn a_script_text_parses_after_its_header() {
        let text = format!(
            "{}\n{}\n{}\n",
            script_header("t"),
            step_line("file.new", &serde_json::json!({"dir": "project"})),
            step_line("edit.undo", &serde_json::json!({}))
        );
        let steps = parse_script(&text).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].0, "file.new");
        assert_eq!(steps[0].1["dir"], "project");
    }
}
