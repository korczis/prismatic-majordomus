//! The repository's own automation, measured against the inventory that tracks its
//! migration: which shell units the tree carries, and whether each one is declared.
//!
//! ADR 0069 replaces shell automation gradually with Rhai scripts over typed capabilities.
//! A migration of that shape converges only if new shell stops arriving while the old
//! shell is being moved, so this module answers two questions about the tree and nothing
//! else:
//!
//! 1. **which files are shell units** — every file under a governed directory whose first
//!    line is a shell interpreter line (`#!/bin/sh`, `#!/usr/bin/env bash`, `zsh`, ...) or
//!    whose name ends in `.sh`;
//! 2. **whether the inventory accounts for them** — every shell unit must have a record in
//!    [`INVENTORY`] carrying an exemption (why shell is required, and the condition under
//!    which it stops being), and every record must name something the tree still has.
//!
//! The second half is what makes the list a ratchet. A record whose file is gone is a
//! finding rather than a harmless leftover, so the exemptions can only shrink as units
//! are migrated, and a new shell unit cannot be added without a line in a reviewed diff
//! that says why.
//!
//! The inventory is JSON Lines, one record per unit, in canonical order
//! ([`crate::order`]). Mechanical facts that drift on every edit — line counts, the
//! interpreter a file declares — are deliberately not recorded: the check derives them,
//! so the file cannot go stale silently. What is recorded is what no reader of the tree
//! can derive: the disposition, the reason for it, the exemption, and the audit's
//! descriptive facts.
//!
//! `test/` is not governed. Every behavioural case is a shell script by the runner's
//! contract (`test/run.sh` runs `test/cases/*.sh`), so refusing them would refuse every new
//! test until the harness itself moves; the harness is one inventory record instead.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::order::{canonical, is_canonical, OrderKey, Ordered};

/// The tracked inventory, relative to the repository root.
pub(crate) const INVENTORY: &str = ".ai/repo/automation/inventory.jsonl";

/// The directories whose shell units must be declared, each with its trailing separator.
pub(crate) const GOVERNED: &[&str] = &[
    ".claude/hooks/",
    ".githooks/",
    "bin/",
    "lib/",
    "scripts/",
    "share/",
];

/// The dispositions a record may carry, as the migration audit defined them.
pub(crate) const DISPOSITIONS: &[(&str, &str)] = &[
    (
        "A",
        "core capability: moves into a typed capability of the executable",
    ),
    (
        "B",
        "orchestration: becomes a scripted capability (ADR 0069)",
    ),
    ("C", "bootstrap glue: runs before or around the executable"),
    ("D", "dead: no caller, awaiting deletion"),
    ("E", "generated: written by a generator or a provider"),
    (
        "F",
        "platform adaptor: drives a tool reached through a shell",
    ),
];

/// How much of a file is read to find its interpreter line. An interpreter line longer
/// than this is not one any kernel honours.
const HEAD_BYTES: u64 = 256;

/// The shell interpreters a first line may name.
const SHELLS: &[&str] = &["ash", "bash", "dash", "ksh", "sh", "zsh"];

/// The remedy every undeclared unit is given.
const REMEDY_UNDECLARED: &str = "new repository automation is a scripted capability \
(ADR 0069) or a typed capability of the executable, not shell; if shell is genuinely \
required, add a record to .ai/repo/automation/inventory.jsonl with a disposition, a reason \
and an exemption {why, removal}";

/// One shell unit the tree carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ShellUnit {
    /// The path, relative to the repository root.
    pub path: String,
    /// The shell the unit is written for: the interpreter its first line names, or
    /// `sourced` for a `*.sh` file with no interpreter line.
    pub dialect: String,
    /// The disposition its inventory record carries, when it has one.
    pub disposition: Option<String>,
}

impl Ordered for ShellUnit {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::plain(&self.path, &self.path)
    }
}

/// One thing the check refuses, with what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ShellFinding {
    /// The finding's code: `shell.undeclared`, `shell.stale`, `shell.missing_field`,
    /// `shell.invalid_disposition`, `shell.invalid_unit`, `shell.exemption_not_shell`,
    /// `shell.duplicate`, `shell.out_of_order` or `shell.malformed`.
    pub code: String,
    /// The file the finding is about: the shell unit, or the inventory itself.
    pub path: String,
    /// The line of the inventory the finding is about, when it is about one.
    pub line: Option<usize>,
    /// What is wrong, in a sentence.
    pub message: String,
    /// What to do about it.
    pub remedy: String,
}

impl Ordered for ShellFinding {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::grouped(&self.path, &self.code, &self.message)
    }
}

/// The verdict of one check of the tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ShellReport {
    /// Whether the tree could be measured at all. `false` is not a pass: the reason says
    /// what stopped the measurement, and a caller that gates on this report must refuse.
    pub measured: bool,
    /// Why the tree could not be measured, when it could not.
    pub reason: Option<String>,
    /// The inventory read, relative to the repository root.
    pub inventory: String,
    /// The directories whose shell units must be declared.
    pub governed: Vec<String>,
    /// Every shell unit under the governed directories, in canonical order.
    pub units: Vec<ShellUnit>,
    /// How many records the inventory holds, shell or not.
    pub records: usize,
    /// How many exemptions the inventory declares, by disposition.
    pub exemptions: BTreeMap<String, usize>,
    /// Every finding, in canonical order. Empty is the passing answer.
    pub findings: Vec<ShellFinding>,
    /// `true` when the tree was measured and nothing was found.
    pub passes: bool,
}

/// The exemption a shell unit's record carries.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Exemption {
    /// Why this unit has to be shell today.
    #[serde(default)]
    pub why: Option<String>,
    /// The condition under which the exemption is removed, and the unit with it.
    #[serde(default)]
    pub removal: Option<String>,
}

/// One record of the inventory, as written.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    /// The file or directory the record is about, relative to the repository root.
    pub unit: String,
    /// A place inside the unit — a workflow step, a recipe, a function — for a record about
    /// shell embedded in a file rather than about the file.
    #[serde(default)]
    pub anchor: Option<String>,
    /// One of [`DISPOSITIONS`].
    #[serde(default)]
    pub disposition: Option<String>,
    /// Why the unit has that disposition.
    #[serde(default)]
    pub reason: Option<String>,
    /// Present on the record of a shell unit: what allows it to be shell.
    #[serde(default)]
    pub exemption: Option<Exemption>,
    /// The audit's descriptive facts. Carried, never judged.
    #[serde(default)]
    #[allow(dead_code)]
    pub facts: Option<serde_json::Map<String, serde_json::Value>>,
}

/// A record with the inventory line it was read from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Line {
    /// The 1-based line of the inventory.
    pub number: usize,
    /// The record on it.
    pub record: Record,
    /// `unit` and `anchor` joined, which is the record's identity.
    identity: String,
}

impl Ordered for Line {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::grouped(
            &self.record.unit,
            self.record.anchor.as_deref().unwrap_or(""),
            &self.identity,
        )
    }
}

/// The shell an interpreter line names, if it names one.
///
/// `#!/usr/bin/env bash` names `bash`, and so does `#!/usr/bin/env -S bash -eu`; `#! /bin/sh`
/// with a space names `sh`; `#!/usr/bin/env node` and `#!/bin/bashful` name no shell.
pub(crate) fn interpreter(first_line: &str) -> Option<String> {
    let rest = first_line.strip_prefix("#!")?;
    let mut words = rest.split_whitespace();
    let program = words.next()?;
    let base = |w: &str| w.rsplit('/').next().unwrap_or(w).to_string();
    let name = if base(program) == "env" {
        words
            .find(|w| !w.starts_with('-') && !w.contains('='))?
            .to_string()
    } else {
        program.to_string()
    };
    let name = base(&name);
    SHELLS.contains(&name.as_str()).then_some(name)
}

/// The dialect of a file, given its path and the first bytes of its content; `None` when
/// the file is not a shell unit.
pub(crate) fn dialect(path: &str, head: &[u8]) -> Option<String> {
    let first = head.split(|b| *b == b'\n').next().unwrap_or_default();
    let first = String::from_utf8_lossy(first);
    if let Some(shell) = interpreter(first.trim_end_matches('\r')) {
        return Some(shell);
    }
    path.ends_with(".sh").then(|| "sourced".to_string())
}

/// Is `path` under one of the governed directories?
pub(crate) fn governed(path: &str) -> bool {
    GOVERNED.iter().any(|dir| path.starts_with(dir))
}

/// Every shell unit under the governed directories of `root`.
///
/// The files are the ones git would commit — tracked, plus untracked files no ignore rule
/// excludes — so a unit is refused before it is added rather than after. A tracked file
/// deleted from the working tree is gone. A symbolic link is not a unit: what it points at
/// is, where it is in the tree, and following one could read outside the repository.
pub(crate) fn enumerate(root: &Path) -> Result<Vec<ShellUnit>, String> {
    let out = crate::git::read_only(root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
        ])
        .args(GOVERNED)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git ls-files: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let paths: BTreeSet<String> = out
        .stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect();
    let mut units = Vec::new();
    // git matched the pathspecs; the prefix test is what a caller reading the contract checks
    for path in paths.into_iter().filter(|p| governed(p)) {
        let full = root.join(&path);
        let Ok(meta) = std::fs::symlink_metadata(&full) else {
            continue;
        };
        if !meta.file_type().is_file() {
            continue;
        }
        let mut head = Vec::new();
        if let Ok(file) = std::fs::File::open(&full) {
            // a file that cannot be read has no interpreter line to find; its name still
            // decides whether it is a `*.sh` unit
            let _ = file.take(HEAD_BYTES).read_to_end(&mut head);
        }
        if let Some(dialect) = dialect(&path, &head) {
            units.push(ShellUnit {
                path,
                dialect,
                disposition: None,
            });
        }
    }
    canonical(&mut units);
    Ok(units)
}

fn finding(
    code: &str,
    path: &str,
    line: Option<usize>,
    message: String,
    remedy: &str,
) -> ShellFinding {
    ShellFinding {
        code: code.into(),
        path: path.into(),
        line,
        message,
        remedy: remedy.into(),
    }
}

/// The records of an inventory's text, and a finding for every line that is not one.
pub(crate) fn parse(text: &str) -> (Vec<Line>, Vec<ShellFinding>) {
    let mut lines = Vec::new();
    let mut findings = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        if raw.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Record>(raw) {
            Ok(record) => {
                let identity = match &record.anchor {
                    Some(anchor) => format!("{}#{anchor}", record.unit),
                    None => record.unit.clone(),
                };
                lines.push(Line {
                    number,
                    record,
                    identity,
                });
            }
            Err(e) => findings.push(finding(
                "shell.malformed",
                INVENTORY,
                Some(number),
                format!("line {number} is not an inventory record: {e}"),
                "write one JSON object per line with `unit`, `disposition`, `reason`, and \
                 for a shell unit an `exemption` {why, removal}; `anchor` and `facts` are optional",
            )),
        }
    }
    (lines, findings)
}

fn blank(value: &Option<String>) -> bool {
    value.as_deref().map(str::trim).unwrap_or("").is_empty()
}

/// Judge the enumerated units against the inventory's records.
///
/// `exists` answers whether a unit is still in the tree; it is a parameter so that the
/// judgement can be tested without a filesystem. The units come back with the disposition
/// their record carries, and every finding in canonical order.
pub(crate) fn judge(
    units: &mut [ShellUnit],
    lines: &[Line],
    exists: &dyn Fn(&str) -> bool,
) -> (Vec<ShellFinding>, BTreeMap<String, usize>) {
    let mut findings = Vec::new();
    let mut exemptions = BTreeMap::new();
    let shell: BTreeSet<&str> = units.iter().map(|u| u.path.as_str()).collect();

    if !is_canonical(lines) {
        for pair in lines.windows(2) {
            if pair[0].order_key() > pair[1].order_key() {
                findings.push(finding(
                    "shell.out_of_order",
                    INVENTORY,
                    Some(pair[1].number),
                    format!(
                        "line {} ({}) belongs before line {} ({})",
                        pair[1].number, pair[1].identity, pair[0].number, pair[0].identity
                    ),
                    "keep the records in canonical order (`majordomus shell check --canonical` \
                     rewrites the file in it)",
                ));
            }
        }
    }

    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut files: BTreeMap<&str, &Line> = BTreeMap::new();
    for line in lines {
        let r = &line.record;
        let at = Some(line.number);
        if let Some(first) = seen.insert(&line.identity, line.number) {
            findings.push(finding(
                "shell.duplicate",
                &r.unit,
                at,
                format!(
                    "{} is recorded on line {first} and again on line {}",
                    line.identity, line.number
                ),
                "keep one record per unit and anchor",
            ));
            continue;
        }
        let unit = r.unit.as_str();
        if unit.is_empty()
            || unit.starts_with('/')
            || unit.contains('\\')
            || unit.split('/').any(|c| c == ".." || c == ".")
            || unit.starts_with(".ai/local/")
        {
            findings.push(finding(
                "shell.invalid_unit",
                INVENTORY,
                at,
                format!("line {} names `{unit}`, which is not a path inside the tracked tree", line.number),
                "name the unit by its path relative to the repository root, never a machine-local one",
            ));
            continue;
        }
        if !exists(unit) {
            findings.push(finding(
                "shell.stale",
                unit,
                at,
                format!("{} is recorded on line {} and is no longer in the tree", line.identity, line.number),
                "delete the record: the unit it described was migrated or removed, and the list only shrinks",
            ));
        }
        match r.disposition.as_deref() {
            Some(d) if DISPOSITIONS.iter().any(|(code, _)| *code == d) => {}
            Some(d) => findings.push(finding(
                "shell.invalid_disposition",
                unit,
                at,
                format!(
                    "{} carries disposition `{d}`, which is not one of A-F",
                    line.identity
                ),
                "use A (core capability), B (orchestration), C (bootstrap glue), D (dead), \
                 E (generated) or F (platform adaptor)",
            )),
            None => findings.push(missing(line, "disposition")),
        }
        if blank(&r.reason) {
            findings.push(missing(line, "reason"));
        }
        if let Some(exemption) = &r.exemption {
            if r.anchor.is_some() || !shell.contains(unit) {
                findings.push(finding(
                    "shell.exemption_not_shell",
                    unit,
                    at,
                    format!(
                        "{} carries an exemption and is not a shell unit under {}",
                        line.identity,
                        GOVERNED.join(", ")
                    ),
                    "remove the exemption: only a whole shell file under a governed directory is exempted",
                ));
            } else {
                files.insert(unit, line);
                if let Some(d) = &r.disposition {
                    *exemptions.entry(d.clone()).or_insert(0) += 1;
                }
            }
            if blank(&exemption.why) {
                findings.push(missing(line, "exemption.why"));
            }
            if blank(&exemption.removal) {
                findings.push(missing(line, "exemption.removal"));
            }
        }
    }

    for unit in units.iter_mut() {
        match files.get(unit.path.as_str()) {
            Some(line) => unit.disposition = line.record.disposition.clone(),
            None => {
                let declared = lines
                    .iter()
                    .any(|l| l.record.unit == unit.path && l.record.anchor.is_none());
                let message = if declared {
                    format!(
                        "{} is a {} unit whose record carries no exemption",
                        unit.path, unit.dialect
                    )
                } else {
                    format!(
                        "{} is a {} unit the inventory does not declare",
                        unit.path, unit.dialect
                    )
                };
                findings.push(finding(
                    "shell.undeclared",
                    &unit.path,
                    None,
                    message,
                    REMEDY_UNDECLARED,
                ));
            }
        }
    }

    canonical(&mut findings);
    (findings, exemptions)
}

fn missing(line: &Line, field: &str) -> ShellFinding {
    finding(
        "shell.missing_field",
        &line.record.unit,
        Some(line.number),
        format!("{} has no `{field}`", line.identity),
        "every record states its disposition and reason; every exemption states why shell is \
         required and the condition under which it is removed",
    )
}

/// Measure the tree at `root` against its inventory.
///
/// A repository with no inventory has no declarations, which is not an error: every shell
/// unit it carries is undeclared, and one that carries none passes.
pub(crate) fn check(root: &Path) -> ShellReport {
    let mut report = ShellReport {
        measured: false,
        reason: None,
        inventory: INVENTORY.into(),
        governed: GOVERNED.iter().map(|g| g.to_string()).collect(),
        units: Vec::new(),
        records: 0,
        exemptions: BTreeMap::new(),
        findings: Vec::new(),
        passes: false,
    };
    let mut units = match enumerate(root) {
        Ok(units) => units,
        Err(reason) => {
            report.reason = Some(reason);
            return report;
        }
    };
    let text = match std::fs::read_to_string(root.join(INVENTORY)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            report.reason = Some(format!("cannot read {INVENTORY}: {e}"));
            return report;
        }
    };
    let (lines, mut findings) = parse(&text);
    let exists = |unit: &str| std::fs::symlink_metadata(root.join(unit)).is_ok();
    let (judged, exemptions) = judge(&mut units, &lines, &exists);
    findings.extend(judged);
    canonical(&mut findings);
    report.measured = true;
    report.records = lines.len();
    report.exemptions = exemptions;
    report.passes = findings.is_empty();
    report.findings = findings;
    report.units = units;
    report
}

/// The inventory's text with its records in canonical order, every line otherwise as it
/// was; `None` when a line is not a record, because reordering a file that cannot be read
/// would move the damage somewhere else.
pub(crate) fn canonical_text(text: &str) -> Option<String> {
    let (lines, findings) = parse(text);
    if !findings.is_empty() {
        return None;
    }
    /// A record and the text it was read from, ordered as the record.
    struct Keyed<'a>(Line, &'a str);
    impl Ordered for Keyed<'_> {
        fn order_key(&self) -> OrderKey<'_> {
            self.0.order_key()
        }
    }
    let raw = text.lines().filter(|l| !l.trim().is_empty());
    let mut items: Vec<Keyed<'_>> = lines
        .into_iter()
        .zip(raw)
        .map(|(l, r)| Keyed(l, r))
        .collect();
    canonical(&mut items);
    let mut out = String::new();
    for item in items {
        out.push_str(item.1);
        out.push('\n');
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(unit: &str, extra: &str) -> String {
        format!(r#"{{"unit":"{unit}","disposition":"A","reason":"not migrated"{extra}}}"#)
    }

    fn exempt(unit: &str) -> String {
        record(
            unit,
            r#","exemption":{"why":"no capability yet","removal":"the capability lands"}"#,
        )
    }

    fn unit(path: &str) -> ShellUnit {
        ShellUnit {
            path: path.into(),
            dialect: "bash".into(),
            disposition: None,
        }
    }

    fn codes(findings: &[ShellFinding]) -> Vec<&str> {
        findings.iter().map(|f| f.code.as_str()).collect()
    }

    fn run(units: &[&str], text: &str, present: &[&str]) -> (Vec<ShellUnit>, Vec<ShellFinding>) {
        let mut units: Vec<ShellUnit> = units.iter().map(|u| unit(u)).collect();
        let (lines, mut findings) = parse(text);
        let exists = |u: &str| present.contains(&u);
        let (judged, _) = judge(&mut units, &lines, &exists);
        findings.extend(judged);
        canonical(&mut findings);
        (units, findings)
    }

    #[test]
    fn every_interpreter_line_that_names_a_shell_is_recognised() {
        for (line, shell) in [
            ("#!/bin/sh", "sh"),
            ("#!/bin/bash", "bash"),
            ("#!/usr/bin/env bash", "bash"),
            ("#!/usr/bin/env sh", "sh"),
            ("#!/usr/bin/env zsh", "zsh"),
            ("#! /bin/dash", "dash"),
            ("#!/usr/bin/env -S bash -eu", "bash"),
            ("#!/usr/bin/env LC_ALL=C bash", "bash"),
        ] {
            assert_eq!(interpreter(line).as_deref(), Some(shell), "{line}");
        }
        for line in [
            "#!/usr/bin/env node",
            "#!/usr/bin/env python3",
            "#!/bin/bashful",
            "# bash",
            "bash",
            "#!",
            "#!/usr/bin/env",
        ] {
            assert_eq!(interpreter(line), None, "{line}");
        }
    }

    #[test]
    fn a_file_is_a_unit_by_its_interpreter_line_or_its_name_and_nothing_else() {
        assert_eq!(
            dialect("scripts/x", b"#!/usr/bin/env bash\nset -eu\n").as_deref(),
            Some("bash")
        );
        assert_eq!(
            dialect("scripts/x", b"#!/bin/sh\r\n").as_deref(),
            Some("sh")
        );
        assert_eq!(
            dialect("lib/a.sh", b"# sourced\n").as_deref(),
            Some("sourced")
        );
        assert_eq!(
            dialect("lib/a.sh", b"#!/bin/bash\n").as_deref(),
            Some("bash")
        );
        assert_eq!(dialect("scripts/x.mjs", b"#!/usr/bin/env node\n"), None);
        assert_eq!(dialect("scripts/lib/a.awk", b"# awk\n"), None);
        assert_eq!(
            dialect("share/x.png", &[0x89, b'P', b'N', b'G', 0, 0xff]),
            None
        );
        assert_eq!(dialect("scripts/empty", b""), None);
    }

    #[test]
    fn only_the_governed_directories_are_governed() {
        assert!(governed("scripts/ci/x"));
        assert!(governed(".claude/hooks/x"));
        assert!(
            !governed("test/cases/1_x.sh"),
            "the harness is one record, not every case"
        );
        assert!(!governed("examples/minimal/generate.sh"));
        assert!(!governed("scriptsx/y"));
    }

    #[test]
    fn a_declared_unit_passes_and_carries_its_disposition() {
        let (units, findings) = run(
            &["scripts/a"],
            &format!("{}\n", exempt("scripts/a")),
            &["scripts/a"],
        );
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(units[0].disposition.as_deref(), Some("A"));
    }

    #[test]
    fn an_undeclared_unit_is_refused_by_name_with_the_remedy() {
        let (_, findings) = run(&["scripts/foo"], "", &["scripts/foo"]);
        assert_eq!(codes(&findings), ["shell.undeclared"]);
        assert_eq!(findings[0].path, "scripts/foo");
        assert!(findings[0].remedy.contains("scripted capability"));

        let (_, findings) = run(
            &["scripts/foo"],
            &record("scripts/foo", ""),
            &["scripts/foo"],
        );
        assert_eq!(
            codes(&findings),
            ["shell.undeclared"],
            "a record without an exemption"
        );
        assert!(findings[0].message.contains("no exemption"));
    }

    #[test]
    fn a_record_whose_unit_is_gone_is_stale() {
        let (_, findings) = run(&[], &exempt("scripts/gone"), &[]);
        assert!(codes(&findings).contains(&"shell.stale"), "{findings:?}");
        assert!(codes(&findings).contains(&"shell.exemption_not_shell"));
        let (_, findings) = run(
            &[],
            &record(".github/workflows/x.yml", r#","anchor":"job :: step""#),
            &[],
        );
        assert_eq!(codes(&findings), ["shell.stale"]);
    }

    #[test]
    fn every_missing_field_is_named() {
        let text = r#"{"unit":"scripts/a","exemption":{"why":" "}}"#;
        let (_, findings) = run(&["scripts/a"], text, &["scripts/a"]);
        let messages: Vec<&str> = findings.iter().map(|f| f.message.as_str()).collect();
        for field in [
            "`disposition`",
            "`reason`",
            "`exemption.why`",
            "`exemption.removal`",
        ] {
            assert!(
                messages.iter().any(|m| m.contains(field)),
                "{field}: {messages:?}"
            );
        }
        let text = r#"{"unit":"scripts/a","disposition":"Z","reason":"r","exemption":{"why":"w","removal":"r"}}"#;
        let (_, findings) = run(&["scripts/a"], text, &["scripts/a"]);
        assert_eq!(codes(&findings), ["shell.invalid_disposition"]);
    }

    #[test]
    fn a_duplicate_a_malformed_line_and_a_machine_path_are_refused() {
        let text = format!("{}\n{}\n", exempt("scripts/a"), exempt("scripts/a"));
        let (_, findings) = run(&["scripts/a"], &text, &["scripts/a"]);
        assert_eq!(codes(&findings), ["shell.duplicate"]);
        assert_eq!(findings[0].line, Some(2));

        let (_, findings) = run(&[], "{not json\n", &[]);
        assert_eq!(codes(&findings), ["shell.malformed"]);
        let (_, findings) = run(
            &[],
            r#"{"unit":"x","disposition":"A","reason":"r","lines":3}"#,
            &["x"],
        );
        assert_eq!(
            codes(&findings),
            ["shell.malformed"],
            "an unknown field is refused"
        );

        for unit in ["/abs/scripts/a", "../a", ".ai/local/cache/x", r"a\\b", ""] {
            let (_, findings) = run(&[], &record(unit, ""), &[unit]);
            assert_eq!(codes(&findings), ["shell.invalid_unit"], "{unit}");
        }
    }

    #[test]
    fn an_exemption_on_a_file_that_is_not_shell_or_on_an_anchor_is_refused() {
        let (_, findings) = run(&[], &exempt("scripts/lib/a.awk"), &["scripts/lib/a.awk"]);
        assert_eq!(codes(&findings), ["shell.exemption_not_shell"]);
        let text = record(
            "scripts/a",
            r#","anchor":"f","exemption":{"why":"w","removal":"r"}"#,
        );
        let (_, findings) = run(&["scripts/a"], &text, &["scripts/a"]);
        assert!(
            codes(&findings).contains(&"shell.exemption_not_shell"),
            "{findings:?}"
        );
    }

    #[test]
    fn the_records_are_kept_in_canonical_order_and_can_be_put_in_it() {
        let text = format!("{}\n{}\n", exempt("scripts/b-10"), exempt("scripts/b-2"));
        let (_, findings) = run(
            &["scripts/b-10", "scripts/b-2"],
            &text,
            &["scripts/b-10", "scripts/b-2"],
        );
        assert_eq!(codes(&findings), ["shell.out_of_order"]);
        assert_eq!(findings[0].line, Some(2));

        let fixed = canonical_text(&text).expect("every line is a record");
        assert!(
            fixed.find("b-2").unwrap() < fixed.find("b-10").unwrap(),
            "{fixed}"
        );
        let (_, findings) = run(
            &["scripts/b-10", "scripts/b-2"],
            &fixed,
            &["scripts/b-10", "scripts/b-2"],
        );
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(
            canonical_text(&fixed).as_deref(),
            Some(fixed.as_str()),
            "idempotent"
        );
        assert_eq!(canonical_text("{broken\n"), None);

        // a whole file before an anchor inside it, and anchors naturally among themselves
        let text = format!(
            "{}\n{}\n{}\n",
            record("justfile", r#","anchor":"recipe 10""#),
            record("justfile", ""),
            record("justfile", r#","anchor":"recipe 9""#),
        );
        let fixed = canonical_text(&text).unwrap();
        let order: Vec<&str> = fixed.lines().collect();
        assert!(!order[0].contains("anchor"));
        assert!(
            order[1].contains("recipe 9") && order[2].contains("recipe 10"),
            "{fixed}"
        );
    }

    #[test]
    fn the_findings_come_back_in_one_order_whatever_the_input_order() {
        let (_, one) = run(&["scripts/z", "bin/a", "lib/m.sh"], "", &[]);
        let (_, two) = run(&["lib/m.sh", "scripts/z", "bin/a"], "", &[]);
        assert_eq!(one, two);
        let paths: Vec<&str> = one.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["bin/a", "lib/m.sh", "scripts/z"]);
    }

    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git runs")
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    #[test]
    fn the_tree_is_enumerated_from_what_git_would_commit_without_following_links() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        let write = |path: &str, body: &str| {
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, body).unwrap();
        };
        write("scripts/a", "#!/usr/bin/env bash\n");
        write("scripts/node", "#!/usr/bin/env node\n");
        write("lib/b.sh", "# sourced\n");
        write(".githooks/pre-commit", "#!/bin/sh\n");
        write("test/cases/1_x.sh", "#!/usr/bin/env bash\n");
        write("scripts/ignored", "#!/bin/sh\n");
        write(".gitignore", "scripts/ignored\n");
        std::fs::create_dir_all(root.join("bin")).unwrap();
        // a link to a shell unit is not a second unit, and a link out of the tree is not read
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("scripts/a"), root.join("bin/link")).unwrap();
            std::os::unix::fs::symlink("/bin/sh", root.join("bin/outside.sh")).unwrap();
        }

        let units = enumerate(root).expect("a work tree is enumerable");
        let paths: Vec<&str> = units.iter().map(|u| u.path.as_str()).collect();
        assert_eq!(paths, [".githooks/pre-commit", "lib/b.sh", "scripts/a"]);
        assert_eq!(units[0].dialect, "sh");
        assert_eq!(units[1].dialect, "sourced");

        let report = check(root);
        assert!(report.measured);
        assert!(!report.passes);
        assert_eq!(report.findings.len(), 3, "{:?}", report.findings);
        assert!(report.findings.iter().all(|f| f.code == "shell.undeclared"));

        std::fs::create_dir_all(root.join(".ai/repo/automation")).unwrap();
        let text = format!(
            "{}\n{}\n{}\n",
            exempt(".githooks/pre-commit"),
            exempt("lib/b.sh"),
            exempt("scripts/a")
        );
        std::fs::write(root.join(INVENTORY), &text).unwrap();
        let report = check(root);
        assert!(report.passes, "{:?}", report.findings);
        assert_eq!(report.exemptions.get("A"), Some(&3));
        assert_eq!(report.records, 3);

        std::fs::remove_file(root.join("scripts/a")).unwrap();
        let report = check(root);
        assert_eq!(
            codes(&report.findings),
            ["shell.exemption_not_shell", "shell.stale"]
        );
    }

    #[test]
    fn a_directory_that_is_not_a_work_tree_is_not_measured_and_does_not_pass() {
        let dir = tempfile::tempdir().unwrap();
        let report = check(dir.path());
        assert!(!report.measured);
        assert!(!report.passes);
        assert!(report.reason.is_some());
    }
}
