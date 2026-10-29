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
//! A file may say whose evidence it is with an optional header, `# kit: <id>`, as its first
//! non-empty line ([`EVIDENCE_KIT_HEADER`], read by [`Evidence::kit`]). A file without one
//! belongs to any kit. The header is a comment like any other to the fingerprints: it is
//! read back only to be compared with the kit a caller judges for.
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

/// The optional header of a kit-evidence/v1 file: its first non-empty line, when that line
/// starts with this, names the kit the evidence is of.
pub const EVIDENCE_KIT_HEADER: &str = "# kit:";

/// The fingerprints of a licensed set, keyed by the first 64 bits of each digest.
#[derive(Debug, Default)]
pub struct Evidence {
    exact: HashMap<u64, Option<String>>,
    normalised: HashMap<u64, Option<String>>,
    kit: Option<String>,
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
    /// Parse kit-evidence/v1, either form; `#` lines and blank lines are skipped, and the
    /// first non-empty line, when it is the header `# kit: <id>`, sets [`Evidence::kit`].
    pub fn parse(text: &str) -> Result<Evidence, EvidenceError> {
        let mut ev = Evidence {
            kit: kit_header(text),
            ..Evidence::default()
        };
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

    /// The kit the evidence names in its header, trimmed; `None` when it has no header and
    /// so belongs to any kit. An empty id names no kit, and so is no kit's.
    pub fn kit(&self) -> Option<&str> {
        self.kit.as_deref()
    }

    /// Fold another evidence file into this one: two `ui.evidence` entries answer together.
    /// The fingerprints are folded; the kit stays this evidence's.
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

/// The kit the header of an evidence text names: its first non-empty line, whitespace
/// trimmed, when that starts with [`EVIDENCE_KIT_HEADER`]; the id after it, trimmed.
fn kit_header(text: &str) -> Option<String> {
    let first = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    first
        .strip_prefix(EVIDENCE_KIT_HEADER)
        .map(|kit| kit.trim().to_owned())
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
pub fn judge(
    tree: &Tree,
    evidence: Option<&Evidence>,
    scan: impl FnOnce(&Evidence) -> Vec<Match>,
) -> Licence {
    if !tree.declares_kit {
        return Licence::Inactive {
            why: "no kit declared (ui.kit is empty)".into(),
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
        let rel = repository
            .section_path(POLICY_SECTION)
            .ok_or_else(|| LicenceError::Policy("the manifest names no policy section".into()))?;
        let text = std::fs::read_to_string(repository.root().join(&rel))
            .map_err(|e| LicenceError::Policy(format!("{rel}: {e}")))?;
        UiPolicy::parse(&text).map_err(|e| LicenceError::Policy(format!("{rel}: ui: {e}")))
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
    let ui = match UiPolicy::load(repository) {
        Ok(ui) => ui,
        Err(LicenceError::Policy(why)) => {
            return Ok(unusable(&UiPolicy::default(), visibility_env, why))
        }
        Err(e) => return Err(e),
    };
    match judge_tree(repository.root(), &ui, evidence, home, visibility_env) {
        Err(LicenceError::Policy(why)) => Ok(unusable(&ui, visibility_env, why)),
        answered => answered,
    }
}

/// Judge the tree at `root` under the `ui:` block of `policy`, the text of its policy file
/// (`None` when it has none): [`assess`] for a caller that holds a root rather than a loaded
/// repository, such as the design kit judge (I4334), which judges repositories Majordomus
/// does not supervise too. The answer is [`assess`]'s for the same block and tree.
pub fn assess_root(
    root: &Path,
    policy: Option<&str>,
    evidence: Option<&str>,
    home: Option<&Path>,
    visibility_env: Option<&str>,
) -> Result<LicenceReport, LicenceError> {
    let ui = match policy.map(UiPolicy::parse).transpose() {
        Ok(ui) => ui.unwrap_or_default(),
        Err(why) => {
            return Ok(unusable(
                &UiPolicy::default(),
                visibility_env,
                format!("ui: {why}"),
            ))
        }
    };
    match judge_tree(root, &ui, evidence, home, visibility_env) {
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
        let tree = Tree {
            visibility,
            declares_kit,
            has_ui: false,
        };
        return Ok(finish(report, judge(&tree, None, |_| Vec::new())));
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
                mode: cols.first()?.to_string(),
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
    let mut names: Vec<DirName> = dir
        .filter_map(|e| e.ok().map(|e| DirName::new(e.file_name())))
        .collect();
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
        DirName {
            shown: name.to_string_lossy().into_owned(),
            name,
        }
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
    let mut file = std::fs::File::open(path).ok()?;
    let mut f = Fingerprint::default();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => f.update(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
    Some(f.finish())
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
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| LicenceError::Tree(format!("git could not start: {e}")))?;
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return Err(LicenceError::Tree(
            "git cat-file --batch has no pipes".into(),
        ));
    };
    // written beside the reading, so neither pipe fills while the other waits
    let asked = oids.clone();
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        for oid in &asked {
            writeln!(stdin, "{oid}")?;
        }
        Ok(())
    });
    let read = read_batch(&mut BufReader::new(stdout), oids.len());
    let wrote = writer
        .join()
        .unwrap_or_else(|_| Err(std::io::Error::other("the writer stopped")));
    let status = child.wait().map_err(tree)?;
    let prints = read.map_err(tree)?;
    wrote.map_err(tree)?;
    if !status.success() {
        return Err(LicenceError::Tree(format!(
            "git cat-file --batch failed: {status}"
        )));
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

/// What git prints for `args` at `root`.
fn git_out(root: &Path, args: &[&str]) -> Result<Vec<u8>, LicenceError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
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
    Command::new("git")
        .arg("-C")
        .arg(root)
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
    fn the_kit_is_the_header_on_the_first_non_empty_line() {
        const FP: &str = "aaaaaaaaaaaaaaaa\tbbbbbbbbbbbbbbbb";
        let kit = |text: &str| {
            let text = text.replace("{FP}", FP);
            Evidence::parse(&text).unwrap().kit().map(str::to_owned)
        };
        // present, with its whitespace trimmed and a blank line and a CR before it
        assert_eq!(
            kit("# kit: flowbite-pro\n{FP}\n").as_deref(),
            Some("flowbite-pro")
        );
        assert_eq!(
            kit("\n\r\n# kit:  examplekit \r\n{FP}\n").as_deref(),
            Some("examplekit")
        );
        // absent: any kit's, as a secret written before the header was is
        assert_eq!(kit("{FP}\n"), None);
        assert_eq!(kit("# kit-evidence/v1\n{FP}\n"), None);
        // a header that is not the first non-empty line is a comment, and names no kit
        assert_eq!(kit("# kit-evidence/v1\n# kit: flowbite-pro\n{FP}\n"), None);
        assert_eq!(kit("{FP}\n# kit: other\n"), None);
        // the header changes no fingerprint
        let ev = Evidence::parse(&format!("# kit: x\n{FP}\n")).unwrap();
        assert_eq!(ev.len(), 1);
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
}
