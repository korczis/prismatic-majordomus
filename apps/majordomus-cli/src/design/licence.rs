//! Licence: whether a repository may carry what a licensed kit is made of, and whether the
//! machine judging it holds the evidence to say.
//!
//! A kit the owner licenses comes with sources that the licence lets the owner adapt and
//! never lets anyone publish. Nothing proved that a public tree held none of them: a rule
//! asked, and a reviewer looked. This module compares bytes (I4336 (a)).
//!
//! # The evidence
//!
//! kit-evidence/v1 is a list of fingerprints, one line per distinct licensed file content:
//! the sha256 of its exact bytes and the sha256 of its bytes after every maximal run of
//! 0x20, 0x09, 0x0D and 0x0A is replaced by one 0x20 (no trimming, no decoding). A line is
//! either the owner's full form, `<exact64>\t<norm64>\t<source path>`, or the CI form,
//! `<exact16>\t<norm16>`; both are compared on their first 16 hex digits, so one list
//! answers for either. The full form names the licensed source a match came from; the CI
//! form names none, by construction: a secret carries no path.
//!
//! The evidence is never in a repository. It is found through `--evidence`, which the CI
//! runner points at the file it decoded `MAJORDOMUS_KIT_EVIDENCE_<KIT>` into, or through the
//! policy's `ui.evidence` entries, each `~`-relative and expanded against `$HOME` on the
//! owner's machine. This module reads a file and nothing else, so it needs neither a
//! network nor a decoder, and an error about the file names the file and the line, never
//! what the line holds.
//!
//! # What is judged
//!
//! The raw bytes of every file git tracks, committed bundles included: what is staged,
//! what `HEAD` holds where that differs (a licensed blob committed and then overwritten is
//! still pushed), and the working copy where it differs from what is staged. Then, under
//! each `ui.publish` directory, every untracked file, ignored ones included: the place a
//! build copies a vendor file into before a deploy uploads it. That directory is read as
//! the deploy's archive reads it, through every symbolic link and into a nested repository
//! git does not descend, `.git` aside. Each content is compared whole, exactly and
//! normalised. Elsewhere a tracked symbolic link or a submodule is not a file, and is not
//! read through.
//!
//! # The answer
//!
//! - **refused**: the tree is public and a judged file holds a licensed file's bytes,
//!   exactly or reformatted. Each match names the file and, where the evidence says, the
//!   licensed source.
//! - **not judged**: the tree is public, declares the kit, has a user interface, and the
//!   machine holds no evidence. That is a failure, never a pass: the place a public tree is
//!   published from is exactly the place without the owner's files. A `ui:` block that
//!   cannot be used as written (an absolute `ui.publish` or `ui.evidence` entry) is not
//!   judged either, whatever the visibility, and the reason names the entry.
//! - **inactive**: no kit is declared, or there is no evidence and nothing can leak (a
//!   private tree) or nothing is held to the kit (no UI file). One line says which.
//! - **active**: the evidence is held and the tree is clean, or private (a private tree may
//!   be the source store itself).
//!
//! Near-copies are out of scope: a licensed file with one class changed, or inlined inside
//! a larger bundle, is a different file to this module. The evidence holds whole-file
//! fingerprints only, and the capability's description says so.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::metadata::yaml;
use crate::order::{self, OrderKey, Ordered};
use crate::policy::{is_safe_relative, POLICY_SECTION};
use crate::repository::Repository;

/// The evidence format this module reads.
pub const EVIDENCE_SCHEMA: &str = "kit-evidence/v1";

/// The variable that may tighten the declared visibility to `public`, and never loosen it.
pub const VISIBILITY_ENV: &str = "MAJORDOMUS_VISIBILITY";

/// The extensions that make a tracked file a user interface, so a declared kit owes
/// evidence: markup a browser renders, or a component that renders it.
pub const UI_EXTENSIONS: &[&str] = &["html", "htm", "heex", "jsx", "tsx", "vue", "svelte"];

/// Whether a tree may be published. Declared by the policy (`ui.visibility`), never read
/// from a forge; `MAJORDOMUS_VISIBILITY` may tighten it and never loosen it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Anyone may read it: no licensed byte may be in it.
    #[default]
    Public,
    /// Only the owner reads it: it may hold the licensed sources.
    Private,
}

impl Visibility {
    /// The declared visibility, tightened by the environment's word when it says public.
    pub fn resolve(declared: Visibility, env: Option<&str>) -> Visibility {
        match env.map(str::trim) {
            Some("public") => Visibility::Public,
            _ => declared,
        }
    }
}

/// How a file matched a licensed one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MatchKind {
    /// The same bytes.
    Exact,
    /// The same bytes once every run of whitespace is one space: a reformatted copy.
    Normalised,
}

/// One judged file that holds a licensed file's bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Match {
    /// Repository-relative path of the file.
    pub path: String,
    /// The licensed source it is a copy of, when the evidence names one; the CI form never
    /// does.
    pub source: Option<String>,
    /// Whether the copy is exact or reformatted.
    pub kind: MatchKind,
}

/// The answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Licence {
    /// The evidence is held and the tree is clean, or private.
    Active,
    /// Nothing is judged, and nothing needs to be.
    Inactive {
        /// Why, in one line.
        why: String,
    },
    /// A public tree that owes a judgement this machine cannot give.
    NotJudged {
        /// Why, in one line.
        why: String,
    },
    /// A public tree that holds licensed bytes.
    Refused {
        /// Every file that does.
        matches: Vec<Match>,
    },
}

impl Licence {
    /// Whether the answer fails the gate.
    pub fn fails(&self) -> bool {
        matches!(self, Licence::Refused { .. } | Licence::NotJudged { .. })
    }

    /// The answer's word, as the report carries it.
    pub fn verdict(&self) -> Verdict {
        match self {
            Licence::Active => Verdict::Active,
            Licence::Inactive { .. } => Verdict::Inactive,
            Licence::NotJudged { .. } => Verdict::NotJudged,
            Licence::Refused { .. } => Verdict::Refused,
        }
    }
}

/// The answer's word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Judged, and nothing licensed is where it may not be.
    Active,
    /// Not judged, and nothing needs to be.
    Inactive,
    /// Owed a judgement and not given one: a failure.
    NotJudged,
    /// A licensed file is in a public tree: a failure.
    Refused,
}

/// The fingerprints of a licensed set, keyed by the first 64 bits of each digest.
#[derive(Debug, Default)]
pub struct Evidence {
    exact: HashMap<u64, Option<String>>,
    normalised: HashMap<u64, Option<String>>,
}

/// Why an evidence text could not be read: the line and the reason, never the value. A
/// malformed secret is named by its file or its variable, not shown.
#[derive(Debug, PartialEq, Eq)]
pub struct EvidenceError {
    /// The 1-based line, or 0 for the text as a whole.
    pub line: usize,
    /// What is wrong with it.
    pub reason: &'static str,
}

impl Evidence {
    /// Parse kit-evidence/v1, either form; `#` lines and blank lines are skipped.
    pub fn parse(text: &str) -> Result<Evidence, EvidenceError> {
        let mut ev = Evidence::default();
        for (i, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut cols = line.split('\t');
            let (Some(e), Some(n)) = (cols.next(), cols.next()) else {
                return Err(EvidenceError {
                    line: i + 1,
                    reason: "fewer than two tab-separated columns",
                });
            };
            let source = cols.next().filter(|s| !s.is_empty()).map(str::to_owned);
            let (Some(e), Some(n)) = (prefix(e), prefix(n)) else {
                return Err(EvidenceError {
                    line: i + 1,
                    reason: "a digest is not 16 or 64 hex digits",
                });
            };
            ev.exact.insert(e, source.clone());
            ev.normalised.insert(n, source);
        }
        if ev.exact.is_empty() {
            return Err(EvidenceError {
                line: 0,
                reason: "no fingerprint",
            });
        }
        Ok(ev)
    }

    /// Fold another evidence file into this one: two `ui.evidence` entries answer together.
    pub fn merge(&mut self, other: Evidence) {
        self.exact.extend(other.exact);
        self.normalised.extend(other.normalised);
    }

    /// The number of distinct licensed contents the evidence names.
    pub fn len(&self) -> usize {
        self.exact.len()
    }

    /// Whether `bytes` are a licensed file's, exactly or reformatted: the judgement of one
    /// file already in memory, which [`scan`] makes in chunks from the disk.
    #[cfg(test)]
    pub fn find(&self, bytes: &[u8]) -> Option<(MatchKind, Option<String>)> {
        let mut f = Fingerprint::default();
        f.update(bytes);
        self.find_fingerprint(&f.finish())
    }

    /// Whether a file with these two digests is a licensed file's.
    fn find_fingerprint(&self, (exact, norm): &(u64, u64)) -> Option<(MatchKind, Option<String>)> {
        if let Some(s) = self.exact.get(exact) {
            return Some((MatchKind::Exact, s.clone()));
        }
        self.normalised
            .get(norm)
            .map(|s| (MatchKind::Normalised, s.clone()))
    }
}

/// Every maximal run of space, tab, carriage return and line feed becomes one space: what
/// the normalised digest is taken over.
#[cfg(test)]
pub fn normalise(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut in_run = false;
    push_normalised(bytes, &mut in_run, &mut out);
    out
}

fn push_normalised(bytes: &[u8], in_run: &mut bool, out: &mut Vec<u8>) {
    for &b in bytes {
        if matches!(b, b' ' | b'\t' | b'\r' | b'\n') {
            if !*in_run {
                out.push(b' ');
            }
            *in_run = true;
        } else {
            out.push(b);
            *in_run = false;
        }
    }
}

/// Both digests of one file, fed in chunks: a run of whitespace that spans two chunks is
/// still one space, so a file of any size is judged without being held in memory.
#[derive(Default)]
struct Fingerprint {
    exact: Sha256,
    norm: Sha256,
    in_run: bool,
    buf: Vec<u8>,
}

impl Fingerprint {
    fn update(&mut self, chunk: &[u8]) {
        self.exact.update(chunk);
        self.buf.clear();
        push_normalised(chunk, &mut self.in_run, &mut self.buf);
        self.norm.update(&self.buf);
    }

    fn finish(self) -> (u64, u64) {
        (
            first64(&self.exact.finalize()),
            first64(&self.norm.finalize()),
        )
    }
}

impl Write for Fingerprint {
    fn write(&mut self, chunk: &[u8]) -> std::io::Result<usize> {
        self.update(chunk);
        Ok(chunk.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn first64(digest: &[u8]) -> u64 {
    u64::from_be_bytes(digest[..8].try_into().expect("a sha256 digest is 32 bytes"))
}

fn prefix(hex: &str) -> Option<u64> {
    if !matches!(hex.len(), 16 | 64) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(&hex[..16], 16).ok()
}

/// What a tree offers the judgement besides its files.
pub struct Tree {
    /// The declared visibility, after the environment tightened it.
    pub visibility: Visibility,
    /// Whether the policy declares a kit (`ui.kit` not empty).
    pub declares_kit: bool,
    /// Whether the tree tracks a UI file ([`UI_EXTENSIONS`]).
    pub has_ui: bool,
}

/// The answer for one tree. `evidence` is `None` when the machine holds none; `scan` finds
/// the matches, and is called only when a public tree is judged against held evidence, so
/// a tree that owes nothing is never read.
/// Why a tree that declares no kit is not judged.
const NO_KIT: &str = "no kit declared (ui.kit is empty)";

pub fn judge(
    tree: &Tree,
    evidence: Option<&Evidence>,
    scan: impl FnOnce(&Evidence) -> Vec<Match>,
) -> Licence {
    if !tree.declares_kit {
        return Licence::Inactive {
            why: NO_KIT.into(),
        };
    }
    let Some(ev) = evidence else {
        return match (tree.visibility, tree.has_ui) {
            (Visibility::Public, true) => Licence::NotJudged {
                why: "a public tree declares a licensed kit and this machine holds no licence \
                      evidence (ui.evidence, or MAJORDOMUS_KIT_EVIDENCE_<KIT> in CI)"
                    .into(),
            },
            (Visibility::Public, false) => Licence::Inactive {
                why: "no UI file is tracked, so the kit is not held here".into(),
            },
            (Visibility::Private, _) => Licence::Inactive {
                why: "a private tree, and no licence evidence on this machine".into(),
            },
        };
    };
    if tree.visibility == Visibility::Private {
        return Licence::Active;
    }
    let matches = scan(ev);
    if matches.is_empty() {
        Licence::Active
    } else {
        Licence::Refused { matches }
    }
}

/// The matches among files already in memory: the judgement without a disk.
#[cfg(test)]
pub fn matches_in<'a>(
    ev: &Evidence,
    files: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Vec<Match> {
    files
        .into_iter()
        .filter_map(|(path, bytes)| {
            ev.find(bytes).map(|(kind, source)| Match {
                path: path.to_string(),
                source,
                kind,
            })
        })
        .collect()
}

// ------------------------------------------------------------------ the policy block

/// `ui:` in `.ai/repo/policy.yaml`, as this module reads it. Read leniently from the
/// policy text, beside the typed [`crate::policy::Policy`]: an absent block declares no
/// kit, which is the one documented optional reading.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct UiPolicy {
    /// `ui.kit:` — the declared kit id; empty means none.
    #[serde(default)]
    pub kit: String,
    /// `ui.visibility:` — public or private.
    #[serde(default)]
    pub visibility: Visibility,
    /// `ui.publish:` — the directories a deploy publishes, repository-relative.
    #[serde(default)]
    pub publish: Vec<String>,
    /// `ui.evidence:` — `~`-relative evidence files on the owner's machine.
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Deserialize, Default)]
struct PolicyUi {
    #[serde(default)]
    ui: UiPolicy,
}

impl UiPolicy {
    /// The `ui:` block of a policy text.
    pub fn parse(text: &str) -> Result<UiPolicy, String> {
        yaml::parse_into::<PolicyUi>(text).map(|p| p.ui)
    }

    /// The `ui:` block of a repository's policy.
    pub fn load(repository: &Repository) -> Result<UiPolicy, LicenceError> {
        UiPolicy::load_from(repository.root(), repository.section_path(POLICY_SECTION))
    }

    /// The `ui:` block of the policy at `rel` under `root`; `None` when the manifest names
    /// no policy section.
    fn load_from(root: &Path, rel: Option<String>) -> Result<UiPolicy, LicenceError> {
        let Some(rel) = rel else {
            return Err(LicenceError::Policy("the manifest names no policy section".into()));
        };
        std::fs::read_to_string(root.join(&rel))
            .map_err(|e| format!("{rel}: {e}"))
            .and_then(|text| UiPolicy::parse(&text).map_err(|e| format!("{rel}: ui: {e}")))
            .map_err(LicenceError::Policy)
    }
}

// ------------------------------------------------------------------ the whole judgement

/// Why a judgement could not be made. Neither variant ever carries a line of evidence.
#[derive(Debug, PartialEq, Eq)]
pub enum LicenceError {
    /// The evidence file is missing, unreadable or not kit-evidence/v1.
    Evidence(String),
    /// The policy's `ui:` block cannot be used as written.
    Policy(String),
    /// The tree could not be listed.
    Tree(String),
}

impl std::fmt::Display for LicenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LicenceError::Evidence(m) | LicenceError::Policy(m) | LicenceError::Tree(m) => {
                f.write_str(m)
            }
        }
    }
}

/// The answer as every projection carries it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LicenceReport {
    /// `active`, `inactive`, `not_judged` or `refused`.
    pub verdict: Verdict,
    /// Whether the verdict fails the gate (`refused`, `not_judged`).
    pub fails: bool,
    /// The declared kit id; empty when none is.
    pub kit: String,
    /// The visibility judged, after `MAJORDOMUS_VISIBILITY` tightened it.
    pub visibility: Visibility,
    /// Whether the tree tracks a UI file.
    pub has_ui: bool,
    /// Why the verdict is inactive or not judged.
    pub reason: Option<String>,
    /// Where the evidence was read from, as it was named: the `--evidence` file, or each
    /// `ui.evidence` entry found on this machine. Empty when none was.
    pub evidence: Vec<String>,
    /// How many distinct licensed contents the evidence names.
    pub fingerprints: usize,
    /// How many contents were compared: staged, committed, working and published bytes,
    /// each distinct one of a path once.
    pub files: usize,
    /// Every file that holds a licensed file's bytes.
    pub matches: Vec<Match>,
}

/// Judge a repository: its policy's `ui:` block, its files, and the evidence named by
/// `evidence` (a path as the caller gave it) or else by `ui.evidence` under `home`.
///
/// A `ui:` block that cannot be used as written (an absolute `ui.publish` or `ui.evidence`
/// entry, a value of the wrong kind) is answered, not raised: `not_judged`, naming the
/// entry, so every consumer of the report fails closed for the real reason. Evidence that
/// cannot be read and a tree git cannot list are errors.
pub fn assess(
    repository: &Repository,
    evidence: Option<&str>,
    home: Option<&Path>,
    visibility_env: Option<&str>,
) -> Result<LicenceReport, LicenceError> {
    // every failure of load is the block's own, and so is answered, never raised
    let ui = match UiPolicy::load(repository) {
        Ok(ui) => ui,
        Err(e) => return Ok(unusable(&UiPolicy::default(), visibility_env, e.to_string())),
    };
    match judge_tree(repository.root(), &ui, evidence, home, visibility_env) {
        Err(LicenceError::Policy(why)) => Ok(unusable(&ui, visibility_env, why)),
        answered => answered,
    }
}

/// The report of a tree whose `ui:` block cannot be used: not judged, a failure whatever
/// the visibility, since the block that says what may leak is the thing in doubt.
fn unusable(ui: &UiPolicy, visibility_env: Option<&str>, why: String) -> LicenceReport {
    let mut report = report_for(ui, Visibility::resolve(ui.visibility, visibility_env));
    report.verdict = Verdict::NotJudged;
    report.fails = true;
    report.reason = Some(format!("the policy's ui: block cannot be used: {why}"));
    report
}

fn report_for(ui: &UiPolicy, visibility: Visibility) -> LicenceReport {
    LicenceReport {
        verdict: Verdict::Inactive,
        fails: false,
        kit: ui.kit.clone(),
        visibility,
        has_ui: false,
        reason: None,
        evidence: Vec::new(),
        fingerprints: 0,
        files: 0,
        matches: Vec::new(),
    }
}

fn judge_tree(
    root: &Path,
    ui: &UiPolicy,
    evidence: Option<&str>,
    home: Option<&Path>,
    visibility_env: Option<&str>,
) -> Result<LicenceReport, LicenceError> {
    let visibility = Visibility::resolve(ui.visibility, visibility_env);
    let declares_kit = !ui.kit.trim().is_empty();
    let mut report = report_for(ui, visibility);
    if !declares_kit {
        return Ok(finish(report, Licence::Inactive { why: NO_KIT.into() }));
    }
    for dir in &ui.publish {
        if !is_safe_relative(dir) {
            return Err(LicenceError::Policy(format!(
                "ui.publish entry `{dir}` is not a repository-relative directory"
            )));
        }
    }
    let index = entries(&git_out(root, &["ls-files", "-z", "-s"])?, 1);
    report.has_ui = index.iter().any(|e| is_ui(&e.path));
    let held = match evidence {
        Some(given) => Some((
            read_evidence(Path::new(given), given)?,
            vec![given.to_string()],
        )),
        None => policy_evidence(&ui.evidence, home)?,
    };
    if let Some((ev, named)) = &held {
        report.fingerprints = ev.len();
        report.evidence = named.clone();
    }
    let tree = Tree {
        visibility,
        declares_kit,
        has_ui: report.has_ui,
    };
    let mut scan_error = None;
    let mut judged = 0;
    let answer =
        judge(
            &tree,
            held.as_ref().map(|(ev, _)| ev),
            |ev| match contents(root, &ui.publish, &index).and_then(|c| scan(root, &c, ev)) {
                Ok((n, found)) => {
                    judged = n;
                    found
                }
                Err(e) => {
                    scan_error = Some(e);
                    Vec::new()
                }
            },
        );
    if let Some(e) = scan_error {
        return Err(e);
    }
    report.files = judged;
    Ok(finish(report, answer))
}

fn finish(mut report: LicenceReport, answer: Licence) -> LicenceReport {
    report.verdict = answer.verdict();
    report.fails = answer.fails();
    match answer {
        Licence::Inactive { why } | Licence::NotJudged { why } => report.reason = Some(why),
        Licence::Refused { matches } => report.matches = matches,
        Licence::Active => {}
    }
    report
}

/// Whether a path is a user interface file.
fn is_ui(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| UI_EXTENSIONS.iter().any(|u| e.eq_ignore_ascii_case(u)))
}

/// The evidence a file holds. `named` is how the caller named it, and is what an error
/// says: the file and the line, never what the line holds.
fn read_evidence(path: &Path, named: &str) -> Result<Evidence, LicenceError> {
    let bytes = std::fs::read(path).map_err(|e| LicenceError::Evidence(format!("{named}: {e}")))?;
    let text = std::str::from_utf8(&bytes).map_err(|_| {
        LicenceError::Evidence(format!(
            "{named}: not {EVIDENCE_SCHEMA}: the file is not text"
        ))
    })?;
    Evidence::parse(text).map_err(|e| {
        let at = if e.line == 0 {
            String::new()
        } else {
            format!(" line {}:", e.line)
        };
        LicenceError::Evidence(format!(
            "{named}:{at} not {EVIDENCE_SCHEMA}: {}; each line is `<exact sha256>\\t<normalised \
             sha256>[\\t<source>]`, 64 hex digits or their first 16",
            e.reason
        ))
    })
}

/// The evidence the policy's `ui.evidence` entries name and this machine holds. An entry
/// is `~/`-relative: an absolute path would put one machine's layout in a tracked file,
/// and any other would be read from the repository, where the evidence never is. An entry
/// whose file is not here is not evidence; one that is here and cannot be read is an error.
fn policy_evidence(
    entries: &[String],
    home: Option<&Path>,
) -> Result<Option<(Evidence, Vec<String>)>, LicenceError> {
    let mut held: Option<(Evidence, Vec<String>)> = None;
    for entry in entries {
        let Some(rest) = entry.strip_prefix("~/").filter(|r| is_safe_relative(r)) else {
            return Err(LicenceError::Policy(format!(
                "ui.evidence entry `{entry}` is not `~/`-relative: the evidence lives on the \
                 owner's machine, never in the repository and never at an absolute path"
            )));
        };
        let Some(home) = home.filter(|h| h.is_absolute()) else {
            continue;
        };
        let path = home.join(rest);
        if !path.is_file() {
            continue;
        }
        let ev = read_evidence(&path, entry)?;
        match &mut held {
            Some((all, named)) => {
                all.merge(ev);
                named.push(entry.clone());
            }
            None => held = Some((ev, vec![entry.clone()])),
        }
    }
    Ok(held)
}

// ------------------------------------------------------------------ what is read

/// Where the bytes of one judged content are read from.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Bytes {
    /// A git object: a tracked path's staged or committed content.
    Object(String),
    /// A file on the disk, read through any symbolic link.
    Disk(PathBuf),
}

/// One content to judge: the repository-relative path it is reported as, and its bytes.
type Content = (String, Bytes);

/// One line of `git ls-files -s` or `git ls-tree -r`: the mode, the object and the path.
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    mode: String,
    oid: String,
    path: String,
}

impl Entry {
    /// A regular file (100644, 100755); a symbolic link's object is its target's name and
    /// a submodule's is a commit, and neither is a file's bytes.
    fn is_file(&self) -> bool {
        self.mode.starts_with("100")
    }
}

/// The NUL-separated records of `ls-files -z -s` (`oid_col` 1) or `ls-tree -r -z`
/// (`oid_col` 2): space-separated columns, a tab, the path.
fn entries(out: &[u8], oid_col: usize) -> Vec<Entry> {
    out.split(|b| *b == 0)
        .filter(|r| !r.is_empty())
        .filter_map(|record| {
            let record = String::from_utf8_lossy(record);
            let (meta, path) = record.split_once('\t')?;
            let cols: Vec<&str> = meta.split(' ').collect();
            Some(Entry {
                mode: cols[0].to_string(),
                oid: cols.get(oid_col)?.to_string(),
                path: path.to_string(),
            })
        })
        .collect()
}

/// Everything judged, each once:
///
/// - every tracked file's staged bytes, and its committed bytes where those differ: a
///   licensed blob that is committed is pushed though the working copy no longer holds it;
/// - every tracked file's working copy where it differs from what is staged, so a copy
///   is caught before it is staged;
/// - under each `ui.publish` directory, every untracked file, ignored ones included, and
///   every symbolic link, read through as the deploy's archive reads it; a directory git
///   does not descend (a nested repository, a linked one) is walked, `.git` aside.
fn contents(
    root: &Path,
    publish: &[String],
    index: &[Entry],
) -> Result<Vec<Content>, LicenceError> {
    let mut all: BTreeSet<Content> = BTreeSet::new();
    let staged = index.iter().filter(|e| e.is_file());
    all.extend(staged.map(|e| (e.path.clone(), Bytes::Object(e.oid.clone()))));
    if git_ok(root, &["rev-parse", "--verify", "-q", "HEAD^{tree}"]) {
        let head = git_out(root, &["ls-tree", "-r", "-z", "--full-tree", "HEAD"])?;
        let committed = entries(&head, 2).into_iter().filter(Entry::is_file);
        all.extend(committed.map(|e| (e.path, Bytes::Object(e.oid))));
    }
    for path in git_paths(root, &["diff", "-z", "--name-only", "--no-ext-diff"])? {
        let full = root.join(&path);
        if std::fs::symlink_metadata(&full).is_ok_and(|m| m.file_type().is_file()) {
            all.insert((path, Bytes::Disk(full)));
        }
    }
    let mut seen = HashSet::new();
    for dir in publish {
        for found in git_paths(root, &["ls-files", "-z", "--others", "--", dir])? {
            let rel = found.trim_end_matches('/').to_string();
            walk(root.join(&rel), rel, &mut all, &mut seen);
        }
        let linked = index
            .iter()
            .filter(|e| e.mode == "120000" && under(&e.path, dir));
        for e in linked {
            walk(root.join(&e.path), e.path.clone(), &mut all, &mut seen);
        }
    }
    Ok(all.into_iter().collect())
}

/// Whether a repository-relative path is at or under a `ui.publish` directory.
fn under(path: &str, dir: &str) -> bool {
    let parts = |p: &str| -> Vec<String> {
        Path::new(p)
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect()
    };
    parts(path).starts_with(&parts(dir))
}

/// A published path on the disk, read as `upload-pages-artifact` archives it: through
/// every symbolic link, a directory walked in canonical name order, `.git` aside. A
/// directory reached twice is walked once; a dangling link publishes nothing.
fn walk(full: PathBuf, rel: String, all: &mut BTreeSet<Content>, seen: &mut HashSet<PathBuf>) {
    let Ok(meta) = std::fs::metadata(&full) else {
        return;
    };
    if meta.is_file() {
        all.insert((rel, Bytes::Disk(full)));
        return;
    }
    if !meta.is_dir() || !full.canonicalize().is_ok_and(|real| seen.insert(real)) {
        return;
    }
    let Ok(dir) = std::fs::read_dir(&full) else {
        return;
    };
    let mut names: Vec<DirName> =
        dir.filter_map(|e| e.ok().map(|e| DirName::new(e.file_name()))).collect();
    order::canonical(&mut names);
    for DirName { shown, name } in names.into_iter().filter(|n| n.name != ".git") {
        walk(full.join(&name), format!("{rel}/{shown}"), all, seen);
    }
}

/// One entry of a published directory: its name as the disk holds it, which is what the walk
/// opens, and as a person reads it, which is what the walk records and orders by.
struct DirName {
    shown: String,
    name: std::ffi::OsString,
}

impl DirName {
    fn new(name: std::ffi::OsString) -> Self {
        DirName { shown: name.to_string_lossy().into_owned(), name }
    }
}

impl Ordered for DirName {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::plain(&self.shown, &self.shown)
    }
}

/// The matches among `contents`, and how many were compared. A content that cannot be
/// read now (a file deleted since it was listed) is not compared.
fn scan(
    root: &Path,
    contents: &[Content],
    ev: &Evidence,
) -> Result<(usize, Vec<Match>), LicenceError> {
    let oids: BTreeSet<&str> = contents
        .iter()
        .filter_map(|(_, b)| match b {
            Bytes::Object(oid) => Some(oid.as_str()),
            Bytes::Disk(_) => None,
        })
        .collect();
    let objects = fingerprint_objects(root, oids.into_iter().map(str::to_owned).collect())?;
    let mut n = 0;
    let mut found: BTreeMap<String, Match> = BTreeMap::new();
    for (path, bytes) in contents {
        let fp = match bytes {
            Bytes::Object(oid) => objects.get(oid).copied().flatten(),
            Bytes::Disk(full) => fingerprint_file(full),
        };
        let Some(fp) = fp else {
            continue;
        };
        n += 1;
        if let Some((kind, source)) = ev.find_fingerprint(&fp) {
            let m = Match {
                path: path.clone(),
                source,
                kind,
            };
            found.entry(path.clone()).or_insert(m);
        }
    }
    Ok((n, found.into_values().collect()))
}

fn fingerprint_file(path: &Path) -> Option<(u64, u64)> {
    let mut f = Fingerprint::default();
    std::fs::File::open(path)
        .and_then(|mut file| std::io::copy(&mut file, &mut f))
        .ok()
        .map(|_| f.finish())
}

/// The fingerprint of each object, streamed through one `git cat-file --batch`; `None`
/// for an object that is missing or not a blob.
fn fingerprint_objects(
    root: &Path,
    oids: Vec<String>,
) -> Result<HashMap<String, Option<(u64, u64)>>, LicenceError> {
    if oids.is_empty() {
        return Ok(HashMap::new());
    }
    let tree = |e: std::io::Error| LicenceError::Tree(format!("git cat-file --batch: {e}"));
    let mut child = git(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(tree)?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let stdout = child.stdout.take().expect("stdout is piped");
    // written beside the reading, so neither pipe fills while the other waits
    let asked = oids.clone();
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        for oid in &asked {
            writeln!(stdin, "{oid}")?;
        }
        Ok(())
    });
    let read = read_batch(&mut BufReader::new(stdout), oids.len());
    let wrote = writer.join().expect("the writer writes and never panics");
    let exited = child.wait();
    wrote.map_err(tree)?;
    let prints = read.map_err(tree)?;
    if !exited.is_ok_and(|s| s.success()) {
        return Err(LicenceError::Tree("git cat-file --batch failed".into()));
    }
    Ok(oids.into_iter().zip(prints).collect())
}

/// `n` answers of `git cat-file --batch`: `<oid> <type> <size>`, the bytes and a line
/// feed, or `<oid> missing`. Each object is fingerprinted in chunks, never held whole.
fn read_batch(r: &mut impl BufRead, n: usize) -> std::io::Result<Vec<Option<(u64, u64)>>> {
    let mut out = Vec::with_capacity(n);
    let mut header = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    for _ in 0..n {
        header.clear();
        if r.read_until(b'\n', &mut header)? == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        let line = String::from_utf8_lossy(&header);
        let mut cols = line.trim_end().split(' ').skip(1);
        let (kind, size) = (
            cols.next(),
            cols.next().and_then(|s| s.parse::<usize>().ok()),
        );
        let Some(mut left) = size else {
            out.push(None);
            continue;
        };
        let mut f = Fingerprint::default();
        while left > 0 {
            let take = left.min(chunk.len());
            r.read_exact(&mut chunk[..take])?;
            f.update(&chunk[..take]);
            left -= take;
        }
        r.read_exact(&mut [0u8; 1])?;
        out.push((kind == Some("blob")).then(|| f.finish()));
    }
    Ok(out)
}

thread_local! {
    /// The program the tree is listed and read with: `git`. A test puts a stand-in here that
    /// fails the way a broken or missing git does; nothing else sets it.
    static GIT: std::cell::RefCell<std::ffi::OsString> = std::cell::RefCell::new("git".into());
}

/// A command for the tree's git, run at `root`.
fn git(root: &Path) -> Command {
    let mut cmd = Command::new(GIT.with(|g| g.borrow().clone()));
    cmd.arg("-C").arg(root);
    cmd
}

/// What git prints for `args` at `root`.
fn git_out(root: &Path, args: &[&str]) -> Result<Vec<u8>, LicenceError> {
    let out = git(root)
        .args(args)
        .output()
        .map_err(|e| LicenceError::Tree(format!("git could not start: {e}")))?;
    if !out.status.success() {
        return Err(LicenceError::Tree(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(out.stdout)
}

/// Whether git succeeds at `args`, saying nothing.
fn git_ok(root: &Path, args: &[&str]) -> bool {
    git(root)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The NUL-separated paths git prints for `args` at `root`.
fn git_paths(root: &Path, args: &[&str]) -> Result<Vec<String>, LicenceError> {
    Ok(git_out(root, args)?
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LICENSED: &[u8] = b"<div class=\"examplekit-shell\">\n  <nav>pro</nav>\n</div>\n";

    fn hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn full_evidence() -> Evidence {
        Evidence::parse(&format!(
            "# kit-evidence/v1\n{}\t{}\tblocks/shell.html\n",
            hex(LICENSED),
            hex(&normalise(LICENSED))
        ))
        .unwrap()
    }

    fn tree(visibility: Visibility) -> Tree {
        Tree {
            visibility,
            declares_kit: true,
            has_ui: true,
        }
    }

    fn judged(t: &Tree, ev: Option<&Evidence>, files: &[(&str, &[u8])]) -> Licence {
        judge(t, ev, |ev| matches_in(ev, files.iter().copied()))
    }

    #[test]
    fn a_byte_identical_licensed_file_in_a_public_tree_is_refused_naming_its_source() {
        let files: &[(&str, &[u8])] = &[("web/shell.html", LICENSED)];
        let got = judged(&tree(Visibility::Public), Some(&full_evidence()), files);
        assert_eq!(
            got,
            Licence::Refused {
                matches: vec![Match {
                    path: "web/shell.html".into(),
                    source: Some("blocks/shell.html".into()),
                    kind: MatchKind::Exact,
                }]
            }
        );
    }

    #[test]
    fn a_reformatted_copy_is_refused_as_normalised() {
        let reformatted: &[u8] =
            b"<div class=\"examplekit-shell\">\r\n\t<nav>pro</nav>\r\n</div>\r\n";
        let files: &[(&str, &[u8])] = &[("web/shell.html", reformatted)];
        let Licence::Refused { matches } =
            judged(&tree(Visibility::Public), Some(&full_evidence()), files)
        else {
            panic!("a reformatted copy must be refused");
        };
        assert_eq!(matches[0].kind, MatchKind::Normalised);
    }

    #[test]
    fn the_same_tree_declared_private_is_active() {
        let files: &[(&str, &[u8])] = &[("web/shell.html", LICENSED)];
        let got = judge(&tree(Visibility::Private), Some(&full_evidence()), |_| {
            panic!("a private tree is never read")
        });
        assert_eq!(got, Licence::Active);
        assert!(!judged(&tree(Visibility::Private), Some(&full_evidence()), files).fails());
    }

    #[test]
    fn a_public_tree_without_evidence_fails_as_not_judged() {
        let got = judged(&tree(Visibility::Public), None, &[]);
        assert!(matches!(got, Licence::NotJudged { .. }) && got.fails());
        assert_eq!(got.verdict(), Verdict::NotJudged);
    }

    #[test]
    fn a_private_tree_without_evidence_is_inactive_and_passes() {
        let got = judged(&tree(Visibility::Private), None, &[]);
        assert!(matches!(got, Licence::Inactive { .. }) && !got.fails());
    }

    #[test]
    fn a_public_tree_with_no_ui_is_not_held_to_the_kit() {
        let t = Tree {
            visibility: Visibility::Public,
            declares_kit: true,
            has_ui: false,
        };
        assert!(!judged(&t, None, &[]).fails());
    }

    #[test]
    fn a_tree_that_declares_no_kit_is_skipped_whatever_it_holds() {
        let t = Tree {
            visibility: Visibility::Public,
            declares_kit: false,
            has_ui: true,
        };
        let got = judge(&t, Some(&full_evidence()), |_| panic!("nothing is read"));
        assert_eq!(
            got,
            Licence::Inactive {
                why: "no kit declared (ui.kit is empty)".into()
            }
        );
    }

    #[test]
    fn the_ci_form_matches_without_naming_a_source() {
        let ev = Evidence::parse(&format!(
            "{}\t{}\n",
            &hex(LICENSED)[..16],
            &hex(&normalise(LICENSED))[..16]
        ))
        .unwrap();
        let files: &[(&str, &[u8])] = &[("dist/app.css", LICENSED)];
        let Licence::Refused { matches } = judged(&tree(Visibility::Public), Some(&ev), files)
        else {
            panic!("the CI form must refuse the same file");
        };
        assert_eq!(matches[0].source, None);
    }

    #[test]
    fn malformed_evidence_names_the_line_never_the_value() {
        assert_eq!(
            Evidence::parse("zz\tyy\n").unwrap_err(),
            EvidenceError {
                line: 1,
                reason: "a digest is not 16 or 64 hex digits"
            }
        );
        // a 20-digit column is neither form
        let odd = format!("{}\t{}\n", "a".repeat(20), "b".repeat(16));
        assert_eq!(Evidence::parse(&odd).unwrap_err().line, 1);
        assert_eq!(
            Evidence::parse("# only a comment\n").unwrap_err().reason,
            "no fingerprint"
        );
        assert_eq!(Evidence::parse("deadbeef\n").unwrap_err().line, 1);
    }

    #[test]
    fn normalisation_matches_the_owner_generator() {
        // the owner's generator: LC_ALL=C tr -s ' \t\r\n' ' '
        assert_eq!(normalise(b"a \t\r\n b\n"), b"a b ".to_vec());
        assert_eq!(normalise(b"  x"), b" x".to_vec());
    }

    #[test]
    fn a_whitespace_run_split_across_chunks_is_still_one_space() {
        let mut f = Fingerprint::default();
        f.update(b"<div>\r");
        f.update(b"\n  <nav>");
        let mut whole = Fingerprint::default();
        whole.update(b"<div>\r\n  <nav>");
        assert_eq!(f.finish(), whole.finish());
    }

    #[test]
    fn the_environment_tightens_visibility_and_never_loosens_it() {
        assert_eq!(
            Visibility::resolve(Visibility::Private, Some("public")),
            Visibility::Public
        );
        assert_eq!(
            Visibility::resolve(Visibility::Public, Some("private")),
            Visibility::Public
        );
    }

    #[test]
    fn the_ui_block_is_read_from_the_policy_and_absent_declares_no_kit() {
        assert_eq!(
            UiPolicy::parse("version: 1\n").unwrap(),
            UiPolicy::default()
        );
        let ui = UiPolicy::parse(
            "version: 1\nui:\n  kit: examplekit\n  visibility: private\n  publish: [dist]\n  \
             evidence: [\"~/.config/examplekit/evidence.tsv\"]\n",
        )
        .unwrap();
        assert_eq!(ui.kit, "examplekit");
        assert_eq!(ui.visibility, Visibility::Private);
        assert_eq!(ui.publish, ["dist"]);
        assert!(UiPolicy::parse("ui:\n  visibility: internal\n").is_err());
    }

    #[test]
    fn a_policy_evidence_entry_is_home_relative_or_refused() {
        for bad in ["/etc/evidence.tsv", "evidence.tsv", "~/../x"] {
            let got = policy_evidence(&[bad.to_string()], Some(Path::new("/nonexistent")));
            assert!(matches!(got, Err(LicenceError::Policy(_))), "{bad}");
        }
        // a ~/ entry this machine does not hold is no evidence, not an error
        let none = policy_evidence(&["~/absent.tsv".into()], Some(Path::new("/nonexistent")));
        assert!(matches!(none, Ok(None)));
        // and without an absolute HOME nothing is expanded
        assert!(matches!(
            policy_evidence(&["~/a.tsv".into()], None),
            Ok(None)
        ));
    }

    #[test]
    fn a_batch_answer_is_fingerprinted_object_by_object_and_a_missing_one_is_none() {
        let mut stream = Vec::new();
        stream.extend_from_slice(format!("aa blob {}\n", LICENSED.len()).as_bytes());
        stream.extend_from_slice(LICENSED);
        stream.extend_from_slice(b"\nbb missing\ncc tree 3\nxyz\n");
        let got = read_batch(&mut std::io::Cursor::new(stream), 3).unwrap();
        let mut whole = Fingerprint::default();
        whole.update(LICENSED);
        assert_eq!(got, vec![Some(whole.finish()), None, None]);
        // an answer cut short is an error, never a clean tree
        let cut = format!("aa blob {}\n<div", LICENSED.len());
        assert!(read_batch(&mut std::io::Cursor::new(cut.into_bytes()), 1).is_err());
    }

    #[test]
    fn the_index_and_the_tree_listings_name_mode_object_and_path() {
        let staged = entries(b"100644 aa 0\tweb/a b.html\x00120000 bb 0\tdist/link\0", 1);
        assert_eq!(staged[0].path, "web/a b.html");
        assert_eq!(staged[0].oid, "aa");
        assert!(staged[0].is_file() && !staged[1].is_file());
        let head = entries(b"100755 blob cc\tbin/x\x00160000 commit dd\tgarden/m\0", 2);
        assert_eq!((head[0].oid.as_str(), head[1].is_file()), ("cc", false));
    }

    #[test]
    fn a_path_is_under_a_publish_directory_by_component() {
        assert!(under("dist/vendor/a.html", "dist"));
        assert!(under("dist/vendor/a.html", "./dist/"));
        assert!(under("dist", "dist"));
        assert!(under("web/a.html", "."));
        assert!(!under("distro/a.html", "dist"));
    }

    #[test]
    fn an_unreadable_evidence_error_never_carries_its_content() {
        let dir = std::env::temp_dir().join(format!("mj-licence-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ev.tsv");
        std::fs::write(&path, "SECRET-LINE-VALUE\n").unwrap();
        let err = read_evidence(&path, "the-evidence-file")
            .unwrap_err()
            .to_string();
        std::fs::remove_dir_all(&dir).ok();
        assert!(err.starts_with("the-evidence-file: line 1:"), "{err}");
        assert!(!err.contains("SECRET"), "{err}");
    }

    // ------------------------------------------------------------------ a whole tree

    /// A git repository and, beside it, a directory the owner's evidence lives in; both are
    /// removed when dropped. The judgement runs against the real git listing of the tree.
    struct Fixture {
        repo: tempfile::TempDir,
        side: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Fixture {
            let f = Fixture {
                repo: tempfile::tempdir().unwrap(),
                side: tempfile::tempdir().unwrap(),
            };
            f.git(&["init", "-q"]);
            std::fs::write(
                f.side.path().join("ev.tsv"),
                format!(
                    "# kit-evidence/v1\n{}\t{}\tblocks/shell.html\n",
                    hex(LICENSED),
                    hex(&normalise(LICENSED))
                ),
            )
            .unwrap();
            f
        }

        fn root(&self) -> &Path {
            self.repo.path()
        }

        fn evidence(&self) -> String {
            self.side.path().join("ev.tsv").to_string_lossy().into_owned()
        }

        fn git(&self, args: &[&str]) {
            let ok = Command::new("git")
                .arg("-C")
                .arg(self.root())
                .args(["-c", "user.email=t@example.invalid", "-c", "user.name=t"])
                .args(["-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null"])
                .args(args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {}", args.join(" "));
        }

        fn write(&self, rel: &str, bytes: &[u8]) {
            let p = self.root().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, bytes).unwrap();
        }

        fn commit(&self) {
            self.git(&["add", "-A"]);
            self.git(&["commit", "-qm", "c", "--allow-empty"]);
        }

        fn judged(&self, ui: &UiPolicy, evidence: Option<&str>) -> LicenceReport {
            judge_tree(self.root(), ui, evidence, Some(self.side.path()), None).unwrap()
        }
    }

    fn ui(kit: &str, visibility: Visibility, publish: &[&str]) -> UiPolicy {
        UiPolicy {
            kit: kit.into(),
            visibility,
            publish: publish.iter().map(|s| s.to_string()).collect(),
            evidence: Vec::new(),
        }
    }

    fn only_match(r: &LicenceReport) -> (&str, MatchKind) {
        assert_eq!(r.verdict, Verdict::Refused, "{r:?}");
        assert!(r.fails);
        assert_eq!(r.matches.len(), 1, "{r:?}");
        (r.matches[0].path.as_str(), r.matches[0].kind)
    }

    #[test]
    fn a_tracked_licensed_file_is_refused_and_every_content_is_counted() {
        let f = Fixture::new();
        f.write("web/shell.html", LICENSED);
        f.write("web/index.html", b"<p>own page</p>\n");
        f.commit();
        let r = f.judged(&ui("examplekit", Visibility::Public, &[]), Some(&f.evidence()));
        assert_eq!(only_match(&r), ("web/shell.html", MatchKind::Exact));
        assert_eq!(r.matches[0].source.as_deref(), Some("blocks/shell.html"));
        assert!(r.has_ui);
        assert_eq!((r.fingerprints, r.evidence.len()), (1, 1));
        assert!(r.files >= 2, "{r:?}");
    }

    #[test]
    fn a_reformatted_copy_staged_but_never_committed_is_refused() {
        let f = Fixture::new();
        f.write("web/index.html", b"<p>own</p>\n");
        f.commit();
        f.write("web/shell.html", b"<div class=\"examplekit-shell\">\r\n\t<nav>pro</nav>\r\n</div>\r\n");
        f.git(&["add", "web/shell.html"]);
        let r = f.judged(&ui("examplekit", Visibility::Public, &[]), Some(&f.evidence()));
        assert_eq!(only_match(&r), ("web/shell.html", MatchKind::Normalised));
    }

    #[test]
    fn a_licensed_file_committed_then_overwritten_is_still_refused() {
        let f = Fixture::new();
        f.write("web/shell.html", LICENSED);
        f.commit();
        f.write("web/shell.html", b"<p>rewritten in the working tree</p>\n");
        let ev = f.evidence();
        let unstaged = f.judged(&ui("examplekit", Visibility::Public, &[]), Some(&ev));
        assert_eq!(only_match(&unstaged).0, "web/shell.html");
        f.git(&["add", "web/shell.html"]);
        let staged = f.judged(&ui("examplekit", Visibility::Public, &[]), Some(&ev));
        assert_eq!(only_match(&staged).0, "web/shell.html");
    }

    #[cfg(unix)]
    #[test]
    fn a_published_directory_is_read_as_the_deploy_archives_it() {
        let f = Fixture::new();
        f.write(".gitignore", b"dist/\nnode_modules/\n");
        f.write("web/index.html", b"<p>own</p>\n");
        f.commit();
        let ev = f.evidence();
        let publish = ui("examplekit", Visibility::Public, &["dist"]);

        // an ignored, untracked file under the publish directory
        f.write("dist/vendor/shell.html", LICENSED);
        assert_eq!(only_match(&f.judged(&publish, Some(&ev))).0, "dist/vendor/shell.html");
        std::fs::remove_dir_all(f.root().join("dist")).unwrap();

        // a symbolic link from it into an ignored node_modules, and a dangling one
        f.write("node_modules/kit/shell.html", LICENSED);
        std::fs::create_dir_all(f.root().join("dist")).unwrap();
        std::os::unix::fs::symlink("../node_modules/kit", f.root().join("dist/link")).unwrap();
        std::os::unix::fs::symlink("../absent", f.root().join("dist/dangling")).unwrap();
        assert_eq!(only_match(&f.judged(&publish, Some(&ev))).0, "dist/link/shell.html");
        std::fs::remove_dir_all(f.root().join("dist")).unwrap();

        // a nested repository the directory is, as a gh-pages checkout is
        f.write("dist/page/shell.html", LICENSED);
        let nested = Command::new("git")
            .arg("-C")
            .arg(f.root().join("dist"))
            .args(["init", "-q"])
            .status()
            .unwrap();
        assert!(nested.success());
        assert_eq!(only_match(&f.judged(&publish, Some(&ev))).0, "dist/page/shell.html");
    }

    #[test]
    fn a_clean_public_tree_is_active_and_a_private_one_is_never_scanned() {
        let f = Fixture::new();
        f.write("web/index.html", b"<p>own</p>\n");
        f.commit();
        let ev = f.evidence();
        let clean = f.judged(&ui("examplekit", Visibility::Public, &[]), Some(&ev));
        assert_eq!((clean.verdict, clean.fails), (Verdict::Active, false));
        assert!(clean.files >= 1);

        f.write("web/shell.html", LICENSED);
        f.commit();
        let private = f.judged(&ui("examplekit", Visibility::Private, &[]), Some(&ev));
        assert_eq!((private.verdict, private.files), (Verdict::Active, 0));
        // the environment tightens a private declaration back to public
        let tightened = judge_tree(
            f.root(),
            &ui("examplekit", Visibility::Private, &[]),
            Some(&ev),
            None,
            Some("public"),
        )
        .unwrap();
        assert_eq!(only_match(&tightened).0, "web/shell.html");
    }

    #[test]
    fn without_evidence_a_public_ui_is_not_judged_and_the_rest_is_inactive() {
        let f = Fixture::new();
        f.write("README.md", b"# no interface here\n");
        f.commit();
        let no_ui = f.judged(&ui("examplekit", Visibility::Public, &[]), None);
        assert_eq!((no_ui.verdict, no_ui.has_ui), (Verdict::Inactive, false));

        f.write("web/PAGE.HTML", b"<p>upper-case extension</p>\n");
        f.commit();
        let owed = f.judged(&ui("examplekit", Visibility::Public, &[]), None);
        assert_eq!((owed.verdict, owed.fails), (Verdict::NotJudged, true));
        assert!(owed.reason.unwrap().contains("no licence evidence"));

        let private = f.judged(&ui("examplekit", Visibility::Private, &[]), None);
        assert_eq!(private.verdict, Verdict::Inactive);
        let no_kit = f.judged(&ui("", Visibility::Public, &[]), None);
        assert_eq!(no_kit.verdict, Verdict::Inactive);
        assert!(no_kit.reason.unwrap().contains("no kit declared"));
    }

    #[test]
    fn the_policy_evidence_under_home_judges_as_a_given_file_does() {
        let f = Fixture::new();
        f.write("web/shell.html", LICENSED);
        f.commit();
        std::fs::copy(f.side.path().join("ev.tsv"), f.side.path().join("ev2.tsv")).unwrap();
        let mut declared = ui("examplekit", Visibility::Public, &[]);
        declared.evidence = vec!["~/ev.tsv".into(), "~/ev2.tsv".into(), "~/absent.tsv".into()];
        let r = f.judged(&declared, None);
        assert_eq!(only_match(&r).0, "web/shell.html");
        assert_eq!(r.evidence, ["~/ev.tsv", "~/ev2.tsv"]);
    }

    #[test]
    fn an_unusable_block_a_bad_file_and_an_unlistable_tree_are_answered_by_name() {
        let f = Fixture::new();
        f.write("web/index.html", b"<p>own</p>\n");
        f.commit();
        // an absolute publish entry is a policy error, which assess answers not judged
        let bad = judge_tree(
            f.root(),
            &ui("examplekit", Visibility::Public, &["/srv/www"]),
            Some(&f.evidence()),
            None,
            None,
        );
        let Err(LicenceError::Policy(why)) = bad else {
            panic!("an absolute ui.publish entry must be a policy error: {bad:?}");
        };
        let answered = unusable(&ui("examplekit", Visibility::Private, &[]), None, why);
        assert_eq!((answered.verdict, answered.fails), (Verdict::NotJudged, true));
        assert!(answered.reason.unwrap().starts_with("the policy's ui: block cannot be used"));

        // evidence that is not there, or not text, is an evidence error naming the file
        let missing = f.judged_err(Some("/nonexistent/ev.tsv"));
        assert!(matches!(missing, LicenceError::Evidence(m) if m.starts_with("/nonexistent/ev.tsv")));
        std::fs::write(f.side.path().join("bin.tsv"), [0xff, 0xfe, 0x00]).unwrap();
        let binary = f.side.path().join("bin.tsv").to_string_lossy().into_owned();
        let not_text = f.judged_err(Some(&binary)).to_string();
        assert!(not_text.contains("the file is not text"), "{not_text}");

        // a directory git cannot list is no answer at all
        let plain = tempfile::tempdir().unwrap();
        let unlisted = judge_tree(
            plain.path(),
            &ui("examplekit", Visibility::Public, &[]),
            Some(&f.evidence()),
            None,
            None,
        );
        assert!(matches!(unlisted, Err(LicenceError::Tree(m)) if m.starts_with("git ")));
    }

    #[test]
    fn evidence_with_no_fingerprint_and_a_malformed_policy_file_are_named() {
        let f = Fixture::new();
        f.write("web/index.html", b"<p>own</p>\n");
        f.commit();
        std::fs::write(f.side.path().join("empty.tsv"), "# kit-evidence/v1\n").unwrap();
        let empty = f.side.path().join("empty.tsv").to_string_lossy().into_owned();
        let msg = f.judged_err(Some(&empty)).to_string();
        // a file with no fingerprint names no line: there is none to point at
        assert!(msg.contains("no fingerprint") && !msg.contains(": line "), "{msg}");

        std::fs::write(f.side.path().join("bad.tsv"), "zz\tyy\n").unwrap();
        let mut declared = ui("examplekit", Visibility::Public, &[]);
        declared.evidence = vec!["~/bad.tsv".into()];
        let got = judge_tree(f.root(), &declared, None, Some(f.side.path()), None).unwrap_err();
        assert!(matches!(got, LicenceError::Evidence(m) if m.starts_with("~/bad.tsv: line 1:")));

        declared.evidence = vec!["/abs/ev.tsv".into()];
        let got = judge_tree(f.root(), &declared, None, Some(f.side.path()), None).unwrap_err();
        assert!(matches!(got, LicenceError::Policy(_)));
    }

    #[cfg(unix)]
    #[test]
    fn a_tracked_link_under_a_publish_directory_is_followed_and_an_unborn_head_is_read() {
        let f = Fixture::new();
        f.write(".gitignore", b"node_modules/\n");
        f.write("node_modules/kit/shell.html", LICENSED);
        f.write("site/index.html", b"<p>own</p>\n");
        std::os::unix::fs::symlink("../node_modules/kit", f.root().join("site/kit")).unwrap();
        // staged, never committed: HEAD is unborn and only the index is listed
        f.git(&["add", "-A"]);
        let publish = ui("examplekit", Visibility::Public, &["site"]);
        let r = f.judged(&publish, Some(&f.evidence()));
        assert_eq!(only_match(&r).0, "site/kit/shell.html");
    }

    #[test]
    fn a_public_tree_that_tracks_nothing_is_active_with_nothing_compared() {
        let f = Fixture::new();
        f.commit();
        let r = f.judged(&ui("examplekit", Visibility::Public, &[]), Some(&f.evidence()));
        assert_eq!((r.verdict, r.files, r.has_ui), (Verdict::Active, 0, false));
    }

    #[test]
    fn a_listing_record_without_its_columns_is_skipped() {
        let got = entries(b"no-tab-here\x00100644 aa\tshort\x00100644 bb 0\tok.html\0", 1);
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!(got.last().unwrap().path, "ok.html");
        assert!(entries(b"100644\tpath\0", 2).is_empty());
    }

    // ------------------------------------------------------------------ a git that fails

    /// Runs `body` with the tree read by `program` instead of git, on this thread only.
    fn with_git<T>(program: &Path, body: impl FnOnce() -> T) -> T {
        GIT.with(|g| *g.borrow_mut() = program.as_os_str().to_owned());
        let out = body();
        GIT.with(|g| *g.borrow_mut() = "git".into());
        out
    }

    /// A stand-in for git: the real git for every subcommand except `broken`, which runs
    /// `instead` (a shell fragment) — the way a git that fails half-way behaves.
    fn fake_git(dir: &Path, broken: &str, instead: &str) -> PathBuf {
        let real = String::from_utf8(
            Command::new("sh").args(["-c", "command -v git"]).output().unwrap().stdout,
        )
        .unwrap();
        let path = dir.join(format!("git-{}", broken.replace(' ', "-")));
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\n# $1 $2 is -C <root>\nREAL={}\ncase \"$*\" in\n  *\"{broken}\"*) {instead} ;;\nesac\nexec \"$REAL\" \"$@\"\n",
                real.trim()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    fn tree_error(r: Result<LicenceReport, LicenceError>, says: &str) {
        match r {
            Err(LicenceError::Tree(m)) => assert!(m.contains(says), "{m}"),
            other => panic!("expected a tree error saying {says}: {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_git_that_fails_half_way_is_no_answer_never_a_clean_tree() {
        let f = Fixture::new();
        f.write(".gitignore", b"dist/\n");
        f.write("web/shell.html", LICENSED);
        f.commit();
        f.write("web/shell.html", b"<p>changed</p>\n");
        let ev = f.evidence();
        let publish = ui("examplekit", Visibility::Public, &["dist"]);
        let bin = tempfile::tempdir().unwrap();
        let judge_with = |program: PathBuf| {
            with_git(&program, || judge_tree(f.root(), &publish, Some(&ev), None, None))
        };
        // git missing altogether
        tree_error(judge_with(bin.path().join("absent")), "git could not start");
        // each listing fails
        for step in ["ls-tree -r", "diff -z", "ls-files -z --others"] {
            let program = fake_git(bin.path(), step, "echo broken >&2; exit 1");
            tree_error(judge_with(program), "failed");
        }
        // cat-file says nothing and exits: the read ends early
        let silent = fake_git(bin.path(), "cat-file --batch", "cat >/dev/null; exit 0");
        tree_error(judge_with(silent), "git cat-file --batch");
        // cat-file answers every object and then fails
        let failing = fake_git(bin.path(), "cat-file --batch", "\"$REAL\" \"$@\"; exit 1");
        tree_error(judge_with(failing), "git cat-file --batch failed");
        // cat-file answers that every object is missing: nothing is compared, nothing leaks
        let missing = fake_git(
            bin.path(),
            "cat-file --batch",
            "while read oid; do echo \"$oid missing\"; done; exit 0",
        );
        let none = judge_with(missing).unwrap();
        assert_eq!(none.verdict, Verdict::Active, "{none:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_cat_file_that_stops_reading_fails_the_write() {
        let f = Fixture::new();
        // more object names than a pipe holds, so the writer meets the closed pipe
        for i in 0..2500 {
            f.write(&format!("web/p{i}.html"), format!("<p>{i}</p>\n").as_bytes());
        }
        f.commit();
        let bin = tempfile::tempdir().unwrap();
        let deaf = fake_git(bin.path(), "cat-file --batch", "exec 0<&-; exit 0");
        let oids: Vec<String> = entries(&git_out(f.root(), &["ls-files", "-z", "-s"]).unwrap(), 1)
            .into_iter()
            .map(|e| e.oid)
            .collect();
        let got = with_git(&deaf, || fingerprint_objects(f.root(), oids));
        assert!(matches!(got, Err(LicenceError::Tree(m)) if m.starts_with("git cat-file --batch")));
        // and a git that cannot start is named too
        let gone = bin.path().join("gone");
        let got = with_git(&gone, || fingerprint_objects(f.root(), vec!["aa".into()]));
        assert!(matches!(got, Err(LicenceError::Tree(_))));
    }

    #[test]
    fn a_batch_stream_that_breaks_is_an_error() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("the pipe broke"))
            }
        }
        assert!(read_batch(&mut BufReader::new(Broken), 1).is_err());
        assert!(read_batch(&mut std::io::Cursor::new(Vec::new()), 1).is_err());
        let no_feed = format!("aa blob {}\n", LICENSED.len()).into_bytes();
        let mut no_feed = [no_feed, LICENSED.to_vec()].concat();
        assert!(read_batch(&mut std::io::Cursor::new(no_feed.clone()), 1).is_err());
        no_feed.push(b'\n');
        assert!(read_batch(&mut std::io::Cursor::new(no_feed), 1).unwrap()[0].is_some());
    }

    #[test]
    fn what_cannot_be_read_now_is_not_compared() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(fingerprint_file(&dir.path().join("deleted")), None);
        assert_eq!(fingerprint_file(dir.path()), None);
        let gone = vec![("gone.html".to_string(), Bytes::Disk(dir.path().join("gone.html")))];
        let (n, found) = scan(dir.path(), &gone, &full_evidence()).unwrap();
        assert_eq!((n, found.len()), (0, 0));
        // a walk of nothing publishes nothing, and a directory reached twice is walked once
        let mut all = BTreeSet::new();
        let mut seen = HashSet::new();
        walk(dir.path().join("absent"), "absent".into(), &mut all, &mut seen);
        std::fs::write(dir.path().join("a.html"), "<p/>").unwrap();
        walk(dir.path().to_path_buf(), "d".into(), &mut all, &mut seen);
        walk(dir.path().to_path_buf(), "again".into(), &mut all, &mut seen);
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn a_policy_that_names_no_section_or_cannot_be_read_is_the_blocks_failure() {
        let dir = tempfile::tempdir().unwrap();
        let none = UiPolicy::load_from(dir.path(), None).unwrap_err();
        assert_eq!(none.to_string(), "the manifest names no policy section");
        let unread = UiPolicy::load_from(dir.path(), Some("absent.yaml".into())).unwrap_err();
        assert!(matches!(unread, LicenceError::Policy(m) if m.starts_with("absent.yaml: ")));
        std::fs::write(dir.path().join("p.yaml"), "ui:\n  visibility: internal\n").unwrap();
        let bad = UiPolicy::load_from(dir.path(), Some("p.yaml".into())).unwrap_err();
        assert!(bad.to_string().starts_with("p.yaml: ui: "), "{bad}");
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_directory_and_a_deleted_file_are_not_compared() {
        use std::os::unix::fs::PermissionsExt;
        let f = Fixture::new();
        f.write("web/shell.html", LICENSED);
        f.write("web/gone.html", b"<p>tracked, then deleted in the working tree</p>\n");
        f.commit();
        std::fs::remove_file(f.root().join("web/gone.html")).unwrap();
        let r = f.judged(&ui("examplekit", Visibility::Public, &[]), Some(&f.evidence()));
        assert_eq!(only_match(&r).0, "web/shell.html");
        // a published directory the walk may enter but not list publishes nothing
        let locked = f.side.path().join("locked");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let (mut all, mut seen) = (BTreeSet::new(), HashSet::new());
        walk(locked.clone(), "locked".into(), &mut all, &mut seen);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(all.is_empty());
    }

    #[test]
    fn a_fingerprint_is_a_writer_and_every_error_reads_as_its_message() {
        let mut f = Fingerprint::default();
        f.write_all(LICENSED).unwrap();
        f.flush().unwrap();
        let mut whole = Fingerprint::default();
        whole.update(LICENSED);
        assert_eq!(f.finish(), whole.finish());
        for e in [
            LicenceError::Evidence("e".into()),
            LicenceError::Policy("p".into()),
            LicenceError::Tree("t".into()),
        ] {
            assert_eq!(e.to_string().len(), 1);
        }
    }

    impl Fixture {
        fn judged_err(&self, evidence: Option<&str>) -> LicenceError {
            judge_tree(
                self.root(),
                &ui("examplekit", Visibility::Public, &[]),
                evidence,
                None,
                None,
            )
            .unwrap_err()
        }
    }
}
