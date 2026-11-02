//! A release leaves the site better than it found it (`project.a-release-leaves-the-site-better`).
//!
//! Two questions about one release window — the commits between the previous release's
//! recorded commit and the commit being released — and both are answered from git, so the
//! verdict can be taken again at any time from the two released trees and reads the same:
//!
//! ```text
//!   no regression   every measure of the site at the release is at least as good as at the
//!                   previous release: the readability of the design in each theme, and how
//!                   many of the site's pages are written by hand rather than generated
//!   an improvement  at least one change in the window has the site as its subject and
//!                   changes something that is not generated output
//! ```
//!
//! The measures are computed by this executable from the tree at each commit, never typed in:
//! the contrast of every pair the design declares (`crate::design::contrast`), and the files
//! under the site's content trees that `.gitattributes` does not mark `merge=derived`. Which
//! change has the site as its subject is the publication model's word
//! (`.ai/repo/ci/pages.yaml`, `release`): the gate classes whose paths are the site's sources,
//! and the conventional scopes and types that name the site. A change whose every file is
//! derived output counts for nothing, because a projection edited by hand is a defect, not an
//! improvement.
//!
//! ```
//! use majordomus_cli::release::site::{Better, SiteSubject};
//! // a count of pages written by hand that grew is a regression; one that shrank is not
//! assert!(Better::Lower.regressed(22.0, 23.0));
//! // a commit scoped `site,ui` names the site as its subject
//! let subject = SiteSubject { scopes: vec!["site".into()], ..Default::default() };
//! assert!(subject.names_scope(Some("site,ui")));
//! ```

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The rule this module enforces.
pub const RULE: &str = "project.a-release-leaves-the-site-better";

/// The publication model, which declares what has the site as its subject.
const PAGES_MODEL: &str = ".ai/repo/ci/pages.yaml";

/// The trees whose files are the site's pages: generated ones and the sources written by hand.
const CONTENT_TREES: &[&str] = &["site/content", "site/content-src"];

/// Two values closer than this are one value: the ratios are rounded to two places, the
/// counts are whole numbers, and a difference below either is noise rather than a movement.
const EPSILON: f64 = 1e-6;

/// Which way a measure improves, which is what decides whether a movement between two
/// releases is a regression that refuses the later one: a ratio must not fall, a count of
/// failures or of hand-written pages must not grow, and a difference below rounding is none.
///
/// ```
/// use majordomus_cli::release::site::Better;
/// assert!(Better::Higher.regressed(4.5, 4.4));
/// assert!(!Better::Higher.regressed(4.5, 4.6));
/// assert!(Better::Lower.regressed(3.0, 4.0));
/// assert!(!Better::Lower.regressed(3.0, 3.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ReleaseSiteBetter")]
pub enum Better {
    /// A larger value is better: a contrast ratio.
    Higher,
    /// A smaller value is better: a count of failures, of pages written by hand.
    Lower,
}

impl Better {
    /// Whether moving from `previous` to `current` made the site worse.
    ///
    /// ```
    /// use majordomus_cli::release::site::Better;
    /// assert!(Better::Lower.regressed(0.0, 1.0));
    /// ```
    pub fn regressed(self, previous: f64, current: f64) -> bool {
        match self {
            Better::Higher => current < previous - EPSILON,
            Better::Lower => current > previous + EPSILON,
        }
    }

    /// Whether moving from `previous` to `current` made the site better.
    ///
    /// ```
    /// use majordomus_cli::release::site::Better;
    /// assert!(Better::Higher.improved(4.5, 7.0));
    /// assert!(!Better::Lower.improved(2.0, 2.0));
    /// ```
    pub fn improved(self, previous: f64, current: f64) -> bool {
        match self {
            Better::Higher => current > previous + EPSILON,
            Better::Lower => current < previous - EPSILON,
        }
    }
}

/// One measure of the site in one tree, as this executable computed it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SiteMeasure {
    pub(crate) id: &'static str,
    pub(crate) title: &'static str,
    pub(crate) better: Better,
    /// The value, or why it could not be computed in that tree.
    pub(crate) value: Result<f64, String>,
}

/// How one measure moved between the two releases.
///
/// ```
/// use majordomus_cli::release::site::MeasureVerdict;
/// assert!(MeasureVerdict::Regressed.refuses());
/// assert!(MeasureVerdict::Unreadable.refuses());
/// assert!(!MeasureVerdict::Held.refuses());
/// assert!(!MeasureVerdict::Unmeasured.refuses(), "a baseline that cannot be read is said, not failed");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ReleaseSiteMeasureVerdict")]
pub enum MeasureVerdict {
    /// The same value at both releases.
    Held,
    /// Better at this release.
    Improved,
    /// Worse at this release: the release is refused.
    Regressed,
    /// The previous release's tree cannot be measured this way (it predates what the measure
    /// reads), so there is nothing to compare with; stated, and not a refusal.
    Unmeasured,
    /// The released tree itself cannot be measured: the release is refused, because a
    /// measure that could not be taken is never a pass.
    Unreadable,
}

impl MeasureVerdict {
    /// Whether this verdict refuses the release.
    ///
    /// ```
    /// use majordomus_cli::release::site::MeasureVerdict;
    /// assert!(!MeasureVerdict::Improved.refuses());
    /// ```
    pub fn refuses(self) -> bool {
        matches!(self, MeasureVerdict::Regressed | MeasureVerdict::Unreadable)
    }
}

/// One measure, at the previous release and at this one.
///
/// ```
/// use majordomus_cli::release::site::{Better, MeasureDelta, MeasureVerdict};
/// let d = MeasureDelta {
///     id: "contrast.dark.weakest".into(),
///     title: "the weakest text contrast in the dark theme".into(),
///     better: Better::Higher,
///     previous: Some(4.8),
///     current: Some(4.6),
///     delta: Some(-0.2),
///     verdict: MeasureVerdict::Regressed,
///     note: None,
/// };
/// assert!(d.verdict.refuses());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseSiteMeasure")]
pub struct MeasureDelta {
    /// The measure's identity: `contrast.light.failing`, `site.pages-by-hand`.
    pub id: String,
    /// What it measures, in a sentence fragment.
    pub title: String,
    /// Which way it improves.
    pub better: Better,
    /// Its value in the previous release's tree, when that tree could be measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<f64>,
    /// Its value in the released tree, when that tree could be measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<f64>,
    /// `current - previous`, when both were measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
    /// How it moved.
    pub verdict: MeasureVerdict,
    /// Why a side could not be measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One commit of the window that touched the site or named it as its subject.
///
/// ```
/// use majordomus_cli::release::site::SiteChange;
/// let c = SiteChange {
///     commit: "3024cbcc0".into(),
///     subject: "fix(site): a diagram's node text is readable in both themes".into(),
///     authored: vec!["lib/mermaid.awk".into()],
///     generated: vec![],
///     counts: true,
///     why: "names the site as its subject and changes a source".into(),
/// };
/// assert!(c.counts);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseSiteChange")]
pub struct SiteChange {
    /// The commit, abbreviated.
    pub commit: String,
    /// Its subject line, as written.
    pub subject: String,
    /// The files it changed that are not generated output and that make it a site change.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authored: Vec<String>,
    /// The site's generated files it changed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generated: Vec<String>,
    /// Whether it counts as the release's improvement of the site.
    pub counts: bool,
    /// Why it counts, or why it does not.
    pub why: String,
}

/// A reason the release is refused.
///
/// ```
/// use majordomus_cli::release::site::SiteFinding;
/// let f = SiteFinding { code: "site-not-improved".into(), message: "nothing".into() };
/// assert_eq!(f.code, "site-not-improved");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseSiteFinding")]
pub struct SiteFinding {
    /// `site-measure-regressed`, `site-measure-unreadable` or `site-not-improved`.
    pub code: String,
    /// What was found, naming the measure and the delta or the commits.
    pub message: String,
}

/// The release the verdict compares with.
///
/// ```
/// use majordomus_cli::release::site::SiteBaseline;
/// let b = SiteBaseline { tag: Some("v0.27.0".into()), commit: "c1a430dcf".into() };
/// assert_eq!(b.tag.as_deref(), Some("v0.27.0"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseSiteBaseline")]
pub struct SiteBaseline {
    /// The previous release's tag, when the baseline is a recorded release.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// The commit the previous release was built from.
    pub commit: String,
}

/// What has the site as its subject, as the publication model declares it.
///
/// ```
/// use majordomus_cli::release::site::SiteSubject;
/// let s = SiteSubject { classes: vec!["site".into()], scopes: vec!["site".into()], kinds: vec!["fix".into()] };
/// assert!(s.names_scope(Some("site,ui")));
/// assert!(!s.names_scope(Some("ci")));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseSiteSubject")]
pub struct SiteSubject {
    /// The gates.yaml classes whose paths are the site's own sources.
    #[serde(default)]
    pub classes: Vec<String>,
    /// The conventional scopes that name the site as a change's subject.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// The conventional types a change must carry to count.
    #[serde(default)]
    pub kinds: Vec<String>,
}

impl SiteSubject {
    /// Whether a commit's scope names the site: any of the words of a compound scope
    /// (`site,ui`, `site+design`, `site/kit`) is one of [`Self::scopes`].
    ///
    /// ```
    /// use majordomus_cli::release::site::SiteSubject;
    /// let s = SiteSubject { scopes: vec!["pages".into()], ..Default::default() };
    /// assert!(s.names_scope(Some("ci+pages")));
    /// assert!(!s.names_scope(None));
    /// ```
    pub fn names_scope(&self, scope: Option<&str>) -> bool {
        scope.is_some_and(|s| {
            s.split([',', '+', '/', ' '])
                .any(|w| self.scopes.iter().any(|x| x == w.trim()))
        })
    }

    /// Read the declaration from the publication model.
    pub(crate) fn load(root: &Path) -> Result<SiteSubject, String> {
        let text = std::fs::read_to_string(root.join(PAGES_MODEL))
            .map_err(|e| format!("{PAGES_MODEL} could not be read: {e}"))?;
        let map = crate::metadata::yaml::parse_mapping(&text)
            .map_err(|e| format!("{PAGES_MODEL} does not parse: {e}"))?;
        let value = map.get("release").cloned().ok_or_else(|| {
            format!("{PAGES_MODEL} declares no `release` section, so nothing says which change has the site as its subject")
        })?;
        let subject: SiteSubject = serde_json::from_value(value)
            .map_err(|e| format!("{PAGES_MODEL}: `release` is not a declaration: {e}"))?;
        if subject.classes.is_empty() && subject.scopes.is_empty() {
            return Err(format!(
                "{PAGES_MODEL}: `release` names neither a class nor a scope, so no change could ever count"
            ));
        }
        Ok(subject)
    }
}

/// The verdict on one release: its site measured against the previous release's, and the
/// changes in its window that have the site as their subject.
///
/// ```
/// use majordomus_cli::release::site::{judge, SiteVerdict};
/// let repo = majordomus_cli::synthetic::SyntheticRepository::small().expect("a synthetic repository");
/// // a repository that has released nothing has no baseline, and says so rather than passing
/// let refused: Result<SiteVerdict, String> = judge(repo.root(), None, None);
/// assert!(refused.is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseSiteVerdict")]
pub struct SiteVerdict {
    /// The rule the verdict is taken under.
    pub rule: String,
    /// The previous release.
    pub previous: SiteBaseline,
    /// The commit being released.
    pub head: String,
    /// Every measure, at both releases.
    pub measures: Vec<MeasureDelta>,
    /// The declaration that decided which change has the site as its subject.
    pub subject: SiteSubject,
    /// Every commit of the window that touched the site or named it, and whether it counts.
    pub changes: Vec<SiteChange>,
    /// How many of [`Self::changes`] count as an improvement of the site.
    pub improvements: usize,
    /// Every reason the release is refused; empty when it passes.
    pub findings: Vec<SiteFinding>,
    /// Whether the release leaves the site better than it found it.
    pub passes: bool,
}

// ---------------------------------------------------------------- git

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("git could not run: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The full name of the commit `rev` names.
fn commit_of(root: &Path, rev: &str) -> Result<String, String> {
    git(
        root,
        &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
    )
    .map(|s| s.trim().to_string())
    .map_err(|_| format!("`{rev}` names no commit this clone has"))
}

/// A file's text at a commit, or `None` when the commit has no such file.
fn show(root: &Path, commit: &str, path: &str) -> Option<String> {
    git(root, &["show", &format!("{commit}:{path}")]).ok()
}

// ---------------------------------------------------------------- the measures

/// The contrast measures of one tree, from its design declaration and the stylesheets that
/// consume it. `sources` carries `None` for a consumer the tree does not have, which makes
/// every contrast measure unreadable rather than a measurement of fewer pairs.
pub(crate) fn contrast_measures(
    tokens: Option<&str>,
    sources: &[(String, Option<String>)],
) -> Vec<SiteMeasure> {
    let reading: Result<crate::design::contrast::ContrastReport, String> = (|| {
        let tokens = tokens.ok_or_else(|| format!("no {}", crate::design::SOURCE))?;
        let design = crate::design::DesignSystem::parse(tokens)
            .map_err(|e| format!("{} does not parse: {e}", crate::design::SOURCE))?;
        let mut read = Vec::new();
        for (name, text) in sources {
            let text = text.as_ref().ok_or_else(|| format!("no {name}"))?;
            read.push((name.clone(), text.clone()));
        }
        Ok(crate::design::contrast::measure(&design, &read))
    })();
    let mut out = Vec::new();
    for (theme, failing_id, failing_title, weakest_id, weakest_title) in [
        (
            "light",
            "contrast.light.failing",
            "text pairs below WCAG AA in the light theme",
            "contrast.light.weakest",
            "the weakest text contrast in the light theme",
        ),
        (
            "dark",
            "contrast.dark.failing",
            "text pairs below WCAG AA in the dark theme",
            "contrast.dark.weakest",
            "the weakest text contrast in the dark theme",
        ),
    ] {
        let (failing, weakest) = match &reading {
            Err(e) => (Err(e.clone()), Err(e.clone())),
            Ok(report) => {
                let text: Vec<&crate::design::contrast::Measured> = report
                    .pairs
                    .iter()
                    .filter(|p| p.enforced && p.theme == theme)
                    .collect();
                let failing = text.iter().filter(|p| !p.passes).count() as f64;
                let weakest = text
                    .iter()
                    .map(|p| (p.ratio * 100.0).round() / 100.0)
                    .fold(None, |m: Option<f64>, r| Some(m.map_or(r, |m| m.min(r))))
                    .ok_or_else(|| {
                        format!("the design pairs no text with a ground in the {theme} theme")
                    });
                (Ok(failing), weakest)
            }
        };
        out.push(SiteMeasure {
            id: failing_id,
            title: failing_title,
            better: Better::Lower,
            value: failing,
        });
        out.push(SiteMeasure {
            id: weakest_id,
            title: weakest_title,
            better: Better::Higher,
            value: weakest,
        });
    }
    out
}

/// How many of the site's page files in a tree are written by hand: tracked under the
/// content trees and not marked `merge=derived` by that tree's own `.gitattributes`.
fn pages_by_hand(root: &Path, commit: &str) -> SiteMeasure {
    let value: Result<f64, String> = (|| {
        let mut args = vec!["ls-tree", "-r", "--name-only", commit, "--"];
        args.extend(CONTENT_TREES);
        let listed = git(root, &args)?;
        let files: Vec<String> = listed.lines().map(str::to_string).collect();
        if files.is_empty() {
            return Err("the tree has no site content".to_string());
        }
        let derived = crate::integration::relation::derived_paths(root, commit, &files)?;
        Ok(files.iter().filter(|f| !derived.contains(*f)).count() as f64)
    })();
    SiteMeasure {
        id: "site.pages-by-hand",
        title: "site page files written by hand rather than generated",
        better: Better::Lower,
        value,
    }
}

/// Every measure of the site in the tree at `commit`.
pub(crate) fn measure_at(root: &Path, commit: &str) -> Vec<SiteMeasure> {
    let tokens = show(root, commit, crate::design::SOURCE);
    let sources: Vec<(String, Option<String>)> = crate::design::contrast::CONSUMERS
        .iter()
        .map(|name| {
            let path = format!("share/{name}");
            let text = show(root, commit, &path);
            (path, text)
        })
        .collect();
    let mut out = contrast_measures(tokens.as_deref(), &sources);
    out.push(pages_by_hand(root, commit));
    out
}

/// Each measure at both releases, and how it moved.
pub(crate) fn compare(previous: &[SiteMeasure], current: &[SiteMeasure]) -> Vec<MeasureDelta> {
    current
        .iter()
        .map(|now| {
            let before = previous.iter().find(|p| p.id == now.id);
            let prev = before.and_then(|b| b.value.as_ref().ok().copied());
            let cur = now.value.as_ref().ok().copied();
            let (verdict, note) = match (&now.value, before.map(|b| &b.value)) {
                (Err(e), _) => (
                    MeasureVerdict::Unreadable,
                    Some(format!("the released tree: {e}")),
                ),
                (Ok(_), None) => (
                    MeasureVerdict::Unmeasured,
                    Some("the previous release was not measured this way".to_string()),
                ),
                (Ok(_), Some(Err(e))) => (
                    MeasureVerdict::Unmeasured,
                    Some(format!("the previous release's tree: {e}")),
                ),
                (Ok(c), Some(Ok(p))) => (
                    if now.better.regressed(*p, *c) {
                        MeasureVerdict::Regressed
                    } else if now.better.improved(*p, *c) {
                        MeasureVerdict::Improved
                    } else {
                        MeasureVerdict::Held
                    },
                    None,
                ),
            };
            MeasureDelta {
                id: now.id.to_string(),
                title: now.title.to_string(),
                better: now.better,
                previous: prev,
                current: cur,
                delta: prev
                    .zip(cur)
                    .map(|(p, c)| ((c - p) * 100.0).round() / 100.0),
                verdict,
                note,
            }
        })
        .collect()
}

// ---------------------------------------------------------------- the window

/// One commit of a release window: its name, its subject line and the files it changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowCommit {
    pub(crate) commit: String,
    pub(crate) subject: String,
    pub(crate) paths: Vec<String>,
}

/// Every commit in `from..to` but merges, newest first, with the files each changed.
fn window(root: &Path, from: &str, to: &str) -> Result<Vec<WindowCommit>, String> {
    let text = git(
        root,
        &[
            "log",
            "--no-merges",
            "--no-renames",
            "--name-only",
            "--format=%x02%H%x01%s",
            &format!("{from}..{to}"),
        ],
    )?;
    Ok(text
        .split('\u{2}')
        .filter_map(|record| {
            let (head, files) = record.split_once('\n').unwrap_or((record, ""));
            let (commit, subject) = head.split_once('\u{1}')?;
            Some(WindowCommit {
                commit: commit.chars().take(9).collect(),
                subject: subject.to_string(),
                paths: files
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_string)
                    .collect(),
            })
        })
        .collect())
}

/// Which commits of a window touched the site or named it, and which of them count as an
/// improvement of it. `derived` is every path of the window that `.gitattributes` marks
/// `merge=derived`; `in_classes` says whether a path is one of the site's sources.
pub(crate) fn classify(
    commits: &[WindowCommit],
    derived: &BTreeSet<String>,
    subject: &SiteSubject,
    in_classes: &dyn Fn(&str) -> bool,
) -> Vec<SiteChange> {
    let mut out = Vec::new();
    for c in commits {
        let header = crate::commit::CommitHeader::parse(&c.subject);
        let names_site = subject.names_scope(header.scope.as_deref());
        let site: Vec<&String> = c.paths.iter().filter(|p| in_classes(p)).collect();
        if site.is_empty() && !names_site {
            continue;
        }
        let authored_site: Vec<String> = site
            .iter()
            .filter(|p| !derived.contains(p.as_str()))
            .map(|p| p.to_string())
            .collect();
        let generated: Vec<String> = c
            .paths
            .iter()
            .filter(|p| derived.contains(p.as_str()) && (in_classes(p) || names_site))
            .cloned()
            .collect();
        let authored_any: Vec<String> = c
            .paths
            .iter()
            .filter(|p| !derived.contains(p.as_str()))
            .cloned()
            .collect();
        let kind_counts = subject.kinds.is_empty() || subject.kinds.contains(&header.word);
        let not_counted = || {
            format!(
                "its type `{}` is not one the publication model counts ({})",
                header.word,
                subject.kinds.join(", ")
            )
        };
        let (authored, counts, why) = if !authored_site.is_empty() {
            if kind_counts {
                (
                    authored_site,
                    true,
                    "changes the site's own sources".to_string(),
                )
            } else {
                (authored_site, false, not_counted())
            }
        } else if names_site && !authored_any.is_empty() {
            if kind_counts {
                (
                    authored_any,
                    true,
                    "names the site as its subject and changes a source the site is generated from"
                        .to_string(),
                )
            } else {
                (authored_any, false, not_counted())
            }
        } else if authored_any.is_empty() {
            (
                Vec::new(),
                false,
                "changes only generated output: a projection edited by hand is a defect, not an improvement"
                    .to_string(),
            )
        } else {
            (
                Vec::new(),
                false,
                "changes the site's generated output alongside sources that are not the site's, and does not name the site as its subject"
                    .to_string(),
            )
        };
        out.push(SiteChange {
            commit: c.commit.clone(),
            subject: c.subject.clone(),
            authored,
            generated,
            counts,
            why,
        });
    }
    out
}

// ---------------------------------------------------------------- the verdict

/// The previous release: the one named by `since`, or the newest stable record whose commit
/// is a strict ancestor of `head`.
fn baseline(root: &Path, since: Option<&str>, head: &str) -> Result<SiteBaseline, String> {
    let releases = crate::distribution::Releases::load(root).map_err(|e| e.to_string())?;
    if let Some(since) = since {
        let commit = match releases
            .releases
            .iter()
            .find(|r| r.tag == since || r.version == since)
        {
            Some(r) => commit_of(root, &r.commit)?,
            None => commit_of(root, since)?,
        };
        let tag = releases
            .releases
            .iter()
            .find(|r| r.tag == since || r.version == since)
            .map(|r| r.tag.clone());
        return Ok(SiteBaseline { tag, commit });
    }
    for r in releases.releases.iter().filter(|r| r.is_stable_candidate()) {
        let Ok(commit) = commit_of(root, &r.commit) else {
            continue;
        };
        if commit != head && git(root, &["merge-base", "--is-ancestor", &commit, head]).is_ok() {
            return Ok(SiteBaseline {
                tag: Some(r.tag.clone()),
                commit,
            });
        }
    }
    Err("no recorded stable release is an ancestor of the commit being released, so there is nothing to compare its site with".to_string())
}

/// Judge the release of `head` (`HEAD` when absent) against the previous release (`since`,
/// or the newest recorded stable release that is its ancestor).
///
/// An error is a verdict that could not be taken — no baseline, a ref this clone does not
/// have, a declaration that does not read — and never a pass.
///
/// ```
/// use majordomus_cli::release::site::judge;
/// let dir = tempfile::tempdir().expect("a directory");
/// // not a repository with a release: there is nothing to judge the site against
/// assert!(judge(dir.path(), None, None).is_err());
/// ```
pub fn judge(root: &Path, since: Option<&str>, head: Option<&str>) -> Result<SiteVerdict, String> {
    let head = commit_of(root, head.unwrap_or("HEAD"))?;
    let previous = baseline(root, since, &head)?;
    let subject = SiteSubject::load(root)?;
    let model = crate::gates::model::GateModel::load(root)?;
    let classes: Vec<&crate::gates::GateClass> = subject
        .classes
        .iter()
        .map(|id| {
            model.classes.iter().find(|c| &c.id == id).ok_or_else(|| {
                format!("{PAGES_MODEL} names the class `{id}`, which .ai/repo/ci/gates.yaml does not declare")
            })
        })
        .collect::<Result<_, _>>()?;

    let measures = compare(
        &measure_at(root, &previous.commit),
        &measure_at(root, &head),
    );

    let commits = window(root, &previous.commit, &head)?;
    let all: Vec<String> = commits
        .iter()
        .flat_map(|c| c.paths.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let derived = crate::integration::relation::derived_paths(root, &head, &all)?;
    let in_classes = |p: &str| classes.iter().any(|c| c.matches(p));
    let changes = classify(&commits, &derived, &subject, &in_classes);
    let improvements = changes.iter().filter(|c| c.counts).count();

    let mut findings = Vec::new();
    for m in &measures {
        match m.verdict {
            MeasureVerdict::Regressed => findings.push(SiteFinding {
                code: "site-measure-regressed".into(),
                message: format!(
                    "{} ({}) went from {} to {} ({:+}); a release never leaves the site worse than the one before it",
                    m.id,
                    m.title,
                    fmt(m.previous),
                    fmt(m.current),
                    m.delta.unwrap_or_default()
                ),
            }),
            MeasureVerdict::Unreadable => findings.push(SiteFinding {
                code: "site-measure-unreadable".into(),
                message: format!(
                    "{} ({}) could not be measured in the released tree: {}",
                    m.id,
                    m.title,
                    m.note.clone().unwrap_or_default()
                ),
            }),
            _ => {}
        }
    }
    if improvements == 0 {
        let seen: Vec<String> = changes
            .iter()
            .map(|c| format!("{} {} — {}", c.commit, c.subject, c.why))
            .collect();
        findings.push(SiteFinding {
            code: "site-not-improved".into(),
            message: if seen.is_empty() {
                format!(
                    "none of the {} commits since {} touches the site or names it; a release carries at least one improvement of the site",
                    commits.len(),
                    previous.tag.as_deref().unwrap_or(&previous.commit)
                )
            } else {
                format!(
                    "no change since {} improves the site through a source it is generated from; {} touched it and none counts: {}",
                    previous.tag.as_deref().unwrap_or(&previous.commit),
                    seen.len(),
                    seen.join("; ")
                )
            },
        });
    }
    Ok(SiteVerdict {
        rule: RULE.to_string(),
        previous,
        head,
        passes: findings.is_empty(),
        measures,
        subject,
        changes,
        improvements,
        findings,
    })
}

fn fmt(v: Option<f64>) -> String {
    v.map_or_else(|| "unmeasured".to_string(), |v| format!("{v}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &'static str, better: Better, value: Result<f64, String>) -> SiteMeasure {
        SiteMeasure {
            id,
            title: "t",
            better,
            value,
        }
    }

    #[test]
    fn a_measure_is_held_improved_or_regressed_by_its_direction() {
        let prev = vec![
            m("a", Better::Higher, Ok(4.5)),
            m("b", Better::Lower, Ok(2.0)),
            m("c", Better::Lower, Ok(1.0)),
        ];
        let cur = vec![
            m("a", Better::Higher, Ok(4.4)),
            m("b", Better::Lower, Ok(1.0)),
            m("c", Better::Lower, Ok(1.0)),
        ];
        let d = compare(&prev, &cur);
        assert_eq!(d[0].verdict, MeasureVerdict::Regressed);
        assert_eq!(d[0].delta, Some(-0.1));
        assert_eq!(d[1].verdict, MeasureVerdict::Improved);
        assert_eq!(d[2].verdict, MeasureVerdict::Held);
    }

    #[test]
    fn an_unreadable_release_refuses_and_an_unreadable_baseline_is_said() {
        let prev = vec![m("a", Better::Higher, Err("no tokens".into()))];
        let cur = vec![
            m("a", Better::Higher, Ok(5.0)),
            m("b", Better::Lower, Err("gone".into())),
        ];
        let d = compare(&prev, &cur);
        assert_eq!(d[0].verdict, MeasureVerdict::Unmeasured);
        assert!(d[0].note.as_deref().unwrap().contains("no tokens"));
        assert_eq!(d[1].verdict, MeasureVerdict::Unreadable);
        assert!(d[1].verdict.refuses());
        assert!(!d[0].verdict.refuses());
    }

    #[test]
    fn contrast_is_unreadable_without_a_declaration_or_a_consumer() {
        let none = contrast_measures(None, &[]);
        assert_eq!(none.len(), 4);
        assert!(none.iter().all(|x| x.value.is_err()));
        let missing = contrast_measures(
            Some(include_str!("../design/tokens.yaml")),
            &[("share/design/kit.css".into(), None)],
        );
        assert!(missing[0].value.as_ref().unwrap_err().contains("kit.css"));
        let bad = contrast_measures(Some("not: [a design"), &[]);
        assert!(bad[1]
            .value
            .as_ref()
            .unwrap_err()
            .contains("does not parse"));
    }

    #[test]
    fn the_compiled_design_measures_both_themes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let sources: Vec<(String, Option<String>)> = crate::design::contrast::CONSUMERS
            .iter()
            .map(|n| {
                let p = format!("share/{n}");
                let t = std::fs::read_to_string(root.join(&p)).ok();
                (p, t)
            })
            .collect();
        let got = contrast_measures(Some(include_str!("../design/tokens.yaml")), &sources);
        let ids: Vec<&str> = got.iter().map(|x| x.id).collect();
        assert_eq!(
            ids,
            [
                "contrast.light.failing",
                "contrast.light.weakest",
                "contrast.dark.failing",
                "contrast.dark.weakest"
            ]
        );
        assert_eq!(
            got[0].value,
            Ok(0.0),
            "the shipped design fails no text pair"
        );
        assert!(*got[1].value.as_ref().unwrap() >= 4.5);
    }

    fn commit(subject: &str, paths: &[&str]) -> WindowCommit {
        WindowCommit {
            commit: "abc".into(),
            subject: subject.into(),
            paths: paths.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn subject() -> SiteSubject {
        SiteSubject {
            classes: vec!["site".into()],
            scopes: vec!["site".into()],
            kinds: vec!["feat".into(), "fix".into()],
        }
    }

    #[test]
    fn only_an_authored_site_change_of_a_counted_type_counts() {
        let derived: BTreeSet<String> = ["site/content/x.md".to_string()].into();
        let in_site = |p: &str| p.starts_with("site/");
        let commits = vec![
            commit("fix(site): the template", &["site/templates/a.html"]),
            commit("docs: a page by hand", &["site/content/x.md"]),
            commit(
                "feat(site): a rule page",
                &[".ai/repo/rules/r.md", "site/content/x.md"],
            ),
            commit("feat(cli): unrelated", &["apps/x.rs"]),
            commit("test(site): a case", &["test/cases/1.sh"]),
            commit(
                "feat(cli): regenerated",
                &["apps/x.rs", "site/content/x.md"],
            ),
            commit("chore(site): tidy", &["site/templates/a.html"]),
        ];
        let got = classify(&commits, &derived, &subject(), &in_site);
        assert_eq!(got.len(), 6, "the unrelated commit is not listed");
        assert!(got[0].counts, "{}", got[0].why);
        assert!(!got[1].counts);
        assert!(got[1].why.contains("only generated output"));
        assert_eq!(got[1].generated, ["site/content/x.md"]);
        assert!(
            got[2].counts,
            "a canonical record named as a site change counts"
        );
        assert_eq!(got[2].authored, [".ai/repo/rules/r.md"]);
        assert!(!got[3].counts, "a test is not an improvement of the site");
        assert!(got[3].why.contains("`test`"));
        assert!(!got[4].counts);
        assert!(got[4].why.contains("does not name the site"));
        assert!(
            !got[5].counts,
            "a chore is not counted even on the site's own sources"
        );
        assert_eq!(got[5].authored, ["site/templates/a.html"]);
        assert!(got[5].why.contains("`chore`"));
    }

    #[test]
    fn a_window_is_read_from_git() {
        let dir = tempfile::tempdir().expect("a directory");
        let r = dir.path();
        let run = |args: &[&str]| {
            let ok = Command::new("git")
                .arg("-C")
                .arg(r)
                .args([
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@t",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(["-c", "core.hooksPath=/dev/null"])
                .args(args)
                .status()
                .expect("git")
                .success();
            assert!(ok, "git {args:?}");
        };
        run(&["init", "-q"]);
        std::fs::write(r.join("a"), "1").unwrap();
        run(&["add", "a"]);
        run(&["commit", "-q", "-m", "chore: first"]);
        std::fs::create_dir_all(r.join("site")).unwrap();
        std::fs::write(r.join("site/b"), "2").unwrap();
        run(&["add", "site/b"]);
        run(&["commit", "-q", "-m", "fix(site): second"]);
        let got = window(r, "HEAD~1", "HEAD").expect("a window");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].subject, "fix(site): second");
        assert_eq!(got[0].paths, ["site/b"]);
        assert_eq!(commit_of(r, "HEAD").unwrap().len(), 40);
        assert!(commit_of(r, "nope").is_err());
        assert_eq!(show(r, "HEAD", "site/b").as_deref(), Some("2"));
        assert!(show(r, "HEAD~1", "site/b").is_none());
        assert_eq!(fmt(None), "unmeasured");
    }

    #[test]
    fn the_declaration_is_read_and_a_missing_one_is_refused() {
        let dir = tempfile::tempdir().expect("a directory");
        assert!(SiteSubject::load(dir.path())
            .unwrap_err()
            .contains("could not be read"));
        std::fs::create_dir_all(dir.path().join(".ai/repo/ci")).unwrap();
        let p = dir.path().join(PAGES_MODEL);
        std::fs::write(&p, "version: 1\n").unwrap();
        assert!(SiteSubject::load(dir.path())
            .unwrap_err()
            .contains("no `release`"));
        std::fs::write(&p, "release:\n  kinds:\n    - fix\n").unwrap();
        assert!(SiteSubject::load(dir.path())
            .unwrap_err()
            .contains("neither a class nor a scope"));
        std::fs::write(
            &p,
            "release:\n  classes:\n    - site\n  scopes:\n    - site\n",
        )
        .unwrap();
        let s = SiteSubject::load(dir.path()).expect("a declaration");
        assert_eq!(s.classes, ["site"]);
        assert!(s.kinds.is_empty());
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ours = SiteSubject::load(&root).expect("this repository declares one");
        assert!(ours.classes.iter().any(|c| c == "site"));
    }
}
