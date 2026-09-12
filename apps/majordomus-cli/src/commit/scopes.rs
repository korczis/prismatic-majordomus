//! The scope vocabulary, derived from the repository rather than declared beside it.
//!
//! # The table this replaces
//!
//! The usual way to answer *"what scope does this change belong to?"* is a table: a list of
//! path patterns and the scope each one maps to, written once, kept in a document a worker
//! is told to read. It works on the day it is written. Then a directory is added, or
//! renamed, or a subsystem is split in two, and the table says a scope that the history
//! stopped using months ago — with nothing to notice, because a table has no way to be
//! wrong.
//!
//! The repository already knows the answer. Every commit in the history is a worked example
//! of which scope a set of paths belongs to, decided by whoever made the change and kept by
//! whoever reviewed it. [`fn@derive`] reads those examples: the scopes the history uses, how
//! often, and which directories each one is written about. A scope is not configured. It is
//! observed, with the observation attached, so that the answer can be checked rather than
//! believed.
//!
//! The consequence a person feels is that the vocabulary is never stale and never has to be
//! maintained: a subsystem that gets its first commit today is in the vocabulary tomorrow,
//! and one that nobody has touched for a year falls to the bottom of the list on its own.
//!
//! # What it costs and where it is allowed to
//!
//! Reading names out of the log is not free, and this is deliberately not on any hot path:
//! nothing about entering the repository, completing a word or answering `--help` reads it.
//! It is read when a commit is being planned or judged, which is a moment that already
//! involves a person waiting for git.

//! ```
//! use majordomus_cli::commit::ScopeVocabulary;
//!
//! // A vocabulary is normally learned with `derive`; stated directly it reads the same.
//! let mut v = ScopeVocabulary::default();
//! v.learn("commit", "apps/majordomus-cli/src/commit", 40);
//! v.learn("site", "site/content", 90);
//!
//! let s = v.suggest(&["apps/majordomus-cli/src/commit/plan.rs".into()]).expect("an association");
//! assert_eq!(s.scope, "commit");
//! assert_eq!(s.commits, 40, "the association, not the vote weight");
//! assert!(!s.ambiguous);
//!
//! // and a path nothing in the history scopes is answered with nothing, never with the
//! // repository's most popular scope
//! assert!(v.suggest(&["brand/new/thing.rs".into()]).is_none());
//! ```

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::CommitHeader;

/// How many commits back the vocabulary is learned from.
///
/// Bounded because the cost is linear in it and the value is not: a scope used ten times
/// last month is in the vocabulary either way, and one last used two thousand commits ago
/// is history rather than vocabulary.
pub const SAMPLE: usize = 1500;

/// How many leading path segments an association is recorded at.
///
/// Four, measured rather than chosen: this repository's source lives at
/// `apps/majordomus-cli/src/<subsystem>/`, and three segments stop one level above the
/// subsystem — where a dozen scopes all have commits and no path can be told from another.
/// Four reaches the directory that actually determines the scope, and a path with fewer
/// directories than that simply associates at every depth it has.
const PREFIX_DEPTH: usize = 4;

/// One scope the history uses, and how much.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// ```
/// use majordomus_cli::commit::scopes::ScopeUse;
/// let u = ScopeUse { scope: "commit".into(), commits: 40, directories: vec!["src/commit".into()] };
/// assert_eq!(u.commits, 40);
/// ```
pub struct ScopeUse {
    /// The scope word, as commits spell it.
    pub scope: String,
    /// How many of the sampled commits used it.
    pub commits: usize,
    /// The directory prefixes those commits touched, most often first, at most five.
    pub directories: Vec<String>,
}

/// The vocabulary as observed, with what was observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
/// ```
/// use majordomus_cli::commit::ScopeVocabulary;
/// // an empty vocabulary knows nothing and says so, rather than guessing
/// let v = ScopeVocabulary::default();
/// assert!(!v.knows("anything"));
/// assert!(v.words().is_empty());
/// ```
pub struct ScopeVocabulary {
    /// Every scope, by how often it is used, then by name. The order is the answer to
    /// "which scopes does this repository use", so it is the order a person reads.
    pub scopes: Vec<ScopeUse>,
    /// How many commits were read to learn it.
    pub sampled: usize,
    /// Set when git could not be asked; the vocabulary is then empty rather than wrong.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<String>,
    /// scope × directory prefix, counted. Not rendered for a person; this is what
    /// [`ScopeVocabulary::suggest`] reads.
    #[serde(skip)]
    association: BTreeMap<(String, String), usize>,
}

/// Which scope a set of paths belongs to, and the evidence for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// ```
/// use majordomus_cli::commit::{ScopeSuggestion, ScopeVocabulary};
/// let mut v = ScopeVocabulary::default();
/// // two scopes with equal claim to one directory: reported, never broken silently
/// v.learn("a", "src", 7);
/// v.learn("b", "src", 7);
/// let s: ScopeSuggestion = v.suggest(&["src/x.rs".into()]).expect("a tie");
/// assert!(s.ambiguous);
/// ```
pub struct ScopeSuggestion {
    /// The scope.
    pub scope: String,
    /// How many prior commits associate this scope with directories these paths are in.
    pub commits: usize,
    /// The directory prefix the association was strongest at.
    pub directory: String,
    /// Whether another scope was as strongly associated. A tie is reported rather than
    /// broken silently: a planner that guessed would be inventing the one field of a commit
    /// message that a reader uses to navigate.
    pub ambiguous: bool,
}

/// The directory prefixes a path is in, deepest first, at most [`PREFIX_DEPTH`] segments.
fn prefixes(path: &str) -> Vec<String> {
    let segments: Vec<&str> = path.split('/').collect();
    // The file name itself is never a prefix; a directory is.
    let dirs = segments.len().saturating_sub(1);
    let mut out = Vec::new();
    for depth in (1..=dirs.min(PREFIX_DEPTH)).rev() {
        out.push(segments[..depth].join("/"));
    }
    out
}

/// Learn the vocabulary from the repository's own history.
///
/// Never fails: a directory that is not a work tree, a repository with no commits and a
/// missing `git` all yield an empty vocabulary that says why.
/// ```
/// use majordomus_cli::commit::scopes::derive;
/// // a directory that is not a work tree yields an empty vocabulary that says why,
/// // never an empty one that looks like a repository with no conventions
/// let plain = tempfile::tempdir().expect("a temporary directory");
/// let v = derive(plain.path());
/// assert!(v.scopes.is_empty());
/// assert!(v.unavailable.is_some());
/// ```
pub fn derive(root: &Path) -> ScopeVocabulary {
    // One `git log`, formatted so that a subject and the names it touched arrive together.
    // `%x00` separates the two, and a record separator git will not produce delimits the
    // commits, because a file name may contain anything a byte can be.
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "log",
            "--no-merges",
            "-n",
            &SAMPLE.to_string(),
            "--name-only",
            "--format=%x02%s",
        ])
        .output();
    let out = match out {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            return ScopeVocabulary {
                unavailable: Some(String::from_utf8_lossy(&o.stderr).trim().to_string()),
                ..ScopeVocabulary::default()
            }
        }
        Err(e) => {
            return ScopeVocabulary {
                unavailable: Some(e.to_string()),
                ..ScopeVocabulary::default()
            }
        }
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut association: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut sampled = 0usize;
    for record in text.split('\u{2}') {
        let mut lines = record.lines();
        let Some(subject) = lines.next() else {
            continue;
        };
        if subject.trim().is_empty() {
            continue;
        }
        sampled += 1;
        let header = CommitHeader::parse(subject);
        if !header.is_conventional() {
            continue;
        }
        let Some(scope) = header.scope.filter(|s| !s.trim().is_empty()) else {
            continue;
        };
        *counts.entry(scope.clone()).or_default() += 1;
        let mut seen: Vec<String> = Vec::new();
        for path in lines.filter(|l| !l.trim().is_empty()) {
            for prefix in prefixes(path) {
                if seen.contains(&prefix) {
                    continue;
                }
                seen.push(prefix.clone());
                *association.entry((scope.clone(), prefix)).or_default() += 1;
            }
        }
    }
    let mut scopes: Vec<ScopeUse> = counts
        .into_iter()
        .map(|(scope, commits)| {
            let mut dirs: Vec<(String, usize)> = association
                .iter()
                .filter(|((s, _), _)| *s == scope)
                .map(|((_, d), n)| (d.clone(), *n))
                .collect();
            dirs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            dirs.truncate(5);
            ScopeUse {
                scope,
                commits,
                directories: dirs.into_iter().map(|(d, _)| d).collect(),
            }
        })
        .collect();
    scopes.sort_by(|a, b| {
        b.commits
            .cmp(&a.commits)
            .then_with(|| a.scope.cmp(&b.scope))
    });
    ScopeVocabulary {
        scopes,
        sampled,
        unavailable: None,
        association,
    }
}

impl ScopeVocabulary {
    /// Record that `commits` commits scoped changes under `prefix` as `scope`.
    ///
    /// [`fn@derive`] is how a vocabulary is normally built, from the history. This is the same
    /// association stated directly, for a caller that has the evidence by another route —
    /// and for tests, which must be able to state a history rather than construct one.
    /// ```
    /// use majordomus_cli::commit::ScopeVocabulary;
    /// let mut v = ScopeVocabulary::default();
    /// v.learn("commit", "src/commit", 12);
    /// v.learn("commit", "src/commit", 5);
    /// assert_eq!(v.suggest(&["src/commit/a.rs".into()]).expect("known").commits, 17);
    /// ```
    pub fn learn(&mut self, scope: &str, prefix: &str, commits: usize) {
        *self
            .association
            .entry((scope.to_string(), prefix.to_string()))
            .or_default() += commits;
    }

    /// Whether the history has used this scope.
    /// ```
    /// use majordomus_cli::commit::{scopes::ScopeUse, ScopeVocabulary};
    /// let mut v = ScopeVocabulary::default();
    /// v.scopes.push(ScopeUse { scope: "commit".into(), commits: 1, directories: vec![] });
    /// assert!(v.knows("commit"));
    /// assert!(!v.knows("nonesuch"));
    /// ```
    pub fn knows(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s.scope == scope)
    }

    /// The scope words alone, in vocabulary order.
    /// ```
    /// use majordomus_cli::commit::{scopes::ScopeUse, ScopeVocabulary};
    /// let mut v = ScopeVocabulary::default();
    /// v.scopes.push(ScopeUse { scope: "site".into(), commits: 9, directories: vec![] });
    /// assert_eq!(v.words(), vec!["site".to_string()]);
    /// ```
    pub fn words(&self) -> Vec<String> {
        self.scopes.iter().map(|s| s.scope.clone()).collect()
    }

    /// Which scope these paths belong to, judged by what prior commits did with the same
    /// directories.
    ///
    /// `None` when nothing in the history associates any scope with any of these paths —
    /// which is the honest answer for the first commit of a new subsystem, and is better
    /// than the most popular scope in the repository, which is what a tie-break to
    /// frequency would produce.
    /// ```
    /// use majordomus_cli::commit::ScopeVocabulary;
    /// let mut v = ScopeVocabulary::default();
    /// // the deepest association wins over the busiest directory two levels up
    /// v.learn("commit", "apps/cli/src/commit", 12);
    /// v.learn("everything", "apps", 900);
    /// let s = v.suggest(&["apps/cli/src/commit/a.rs".into()]).expect("an association");
    /// assert_eq!(s.scope, "commit");
    /// ```
    pub fn suggest(&self, paths: &[String]) -> Option<ScopeSuggestion> {
        // Each path votes once, at the deepest prefix that anything is associated with, so
        // that a change inside one subsystem is not outvoted by the repository's busiest
        // directory two levels up.
        let mut votes: BTreeMap<String, (usize, String)> = BTreeMap::new();
        for path in paths {
            for prefix in prefixes(path) {
                let here: Vec<(&String, usize)> = self
                    .association
                    .iter()
                    .filter(|((_, d), _)| *d == prefix)
                    .map(|((s, _), n)| (s, *n))
                    .collect();
                if here.is_empty() {
                    continue;
                }
                for (scope, n) in here {
                    let entry = votes.entry(scope.clone()).or_insert((0, prefix.clone()));
                    entry.0 += n;
                    entry.1 = prefix.clone();
                }
                break;
            }
        }
        let mut ranked: Vec<(String, usize, String)> =
            votes.into_iter().map(|(s, (n, d))| (s, n, d)).collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let (scope, weight, directory) = ranked.first()?.clone();
        let ambiguous = ranked.get(1).is_some_and(|(_, n, _)| *n == weight);
        // The weight is what ranked the candidates — every path votes, so it grows with the
        // number of paths. What a person is told is the association itself: how many commits
        // scoped *that directory* that way. Reporting the weight would say "80 prior commits"
        // for two files under a directory forty commits have touched, which is a number
        // nobody could reconcile with the history.
        let commits = self
            .association
            .get(&(scope.clone(), directory.clone()))
            .copied()
            .unwrap_or(weight);
        Some(ScopeSuggestion {
            scope,
            commits,
            directory,
            ambiguous,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prefix_is_a_directory_and_never_the_file() {
        assert_eq!(
            prefixes("apps/majordomus-cli/src/commit/plan.rs"),
            vec![
                "apps/majordomus-cli/src/commit",
                "apps/majordomus-cli/src",
                "apps/majordomus-cli",
                "apps"
            ],
            "deepest first, at most PREFIX_DEPTH segments"
        );
        assert_eq!(prefixes("docs/COMMIT.md"), vec!["docs"]);
        // a file at the root is in no directory, so it associates with nothing
        assert!(prefixes("README.md").is_empty());
    }

    fn vocabulary(rows: &[(&str, &str, usize)]) -> ScopeVocabulary {
        let mut v = ScopeVocabulary::default();
        for (scope, prefix, n) in rows {
            v.learn(scope, prefix, *n);
            if !v.knows(scope) {
                v.scopes.push(ScopeUse {
                    scope: scope.to_string(),
                    commits: *n,
                    directories: vec![prefix.to_string()],
                });
            }
        }
        v
    }

    #[test]
    fn a_path_takes_the_scope_the_history_gave_its_directory() {
        let v = vocabulary(&[
            ("commit", "apps/majordomus-cli/src", 40),
            ("site", "site", 90),
        ]);
        let s = v
            .suggest(&["apps/majordomus-cli/src/commit/plan.rs".into()])
            .expect("an association");
        assert_eq!(s.scope, "commit");
        assert_eq!(s.directory, "apps/majordomus-cli/src");
        assert!(!s.ambiguous);
    }

    #[test]
    fn the_deepest_association_wins_over_the_busiest_directory() {
        // `apps` is where everything is; `apps/majordomus-cli/src` is where this is. A vote
        // at the shallow prefix would make every change in the crate take the scope of
        // whatever the crate's busiest subsystem happens to be.
        let v = vocabulary(&[
            ("commit", "apps/majordomus-cli/src", 12),
            ("everything", "apps", 900),
        ]);
        let s = v
            .suggest(&["apps/majordomus-cli/src/commit/plan.rs".into()])
            .expect("an association");
        assert_eq!(s.scope, "commit");
    }

    #[test]
    fn a_tie_is_reported_rather_than_broken() {
        let v = vocabulary(&[("a", "src", 7), ("b", "src", 7)]);
        let s = v.suggest(&["src/x.rs".into()]).expect("an association");
        assert!(s.ambiguous, "two scopes are equally associated");
        // and it is still deterministic: the same input yields the same answer every time
        assert_eq!(s.scope, v.suggest(&["src/x.rs".into()]).unwrap().scope);
    }

    #[test]
    fn nothing_known_is_answered_with_nothing_and_not_with_the_most_popular_scope() {
        let v = vocabulary(&[("site", "site", 900)]);
        assert!(v.suggest(&["brand/new/thing.rs".into()]).is_none());
        assert!(v.suggest(&[]).is_none());
    }

    #[test]
    fn a_directory_that_is_not_a_work_tree_yields_an_empty_vocabulary_that_says_why() {
        let plain = tempfile::tempdir().expect("a temporary directory");
        let v = derive(plain.path());
        assert!(v.scopes.is_empty());
        assert!(
            v.unavailable.is_some(),
            "it says why rather than pretending"
        );
        assert!(!v.knows("anything"));
    }
}
