//! Evidence subjects: every declaration a verdict can be asked about, what it is made of,
//! and the tests it reaches.
//!
//! A *subject* is a declaration a person or a page asks "is this proven?" about: a claim of
//! `docs/CLAIMS.yaml`, a rule of the layer, a feature, a public command, a builtin capability
//! of the executable, an MCP tool (an alias of its capability) or a use case. Its key is
//! `<kind>:<id>` — `claim:scope-integrity`, `command:commit plan`, `mcp:majordomus_get`.
//!
//! # Derived only
//!
//! Nothing lists a subject, a member or a route by hand. [`index`] reads the declarations
//! that already exist and nothing else:
//!
//! * the claims, through the evidence join ([`crate::evidence::report`]) with an empty
//!   ledger, so a claim's test identity is the one grammar [`TestId::of`] owns;
//! * the rules, through [`crate::rules::definitions`];
//! * the features, through the product model;
//! * the use cases and the public shell commands, from the index;
//! * the documented command-line examples ([`crate::cli::EXAMPLES`]);
//! * the builtin capabilities of the registry, with the file each was composed in;
//! * the first `# majordomus-covers:` and the first `# majordomus-negative:` line of every
//!   case under `test/cases/`, and the integration test binaries of the crate.
//!
//! Adding a declaration adds its subject, and removing it removes it. The committed index,
//! [`PATH`], carries structure only — no ledger row, no commit — so recording a run never
//! makes it stale, and `majordomus generate --check` refuses a copy that differs from this
//! derivation.
//!
//! # The shell derivations it replaces or holds equal
//!
//! Before this index, six shell programs derived "which tests prove this subject":
//!
//! * the command pages (`scripts/generate-site-data`, `commands.json`) read the first
//!   covers and negative header of every case — the index reads the same first line, and
//!   case 507 holds the two equal;
//! * the command-coverage doctrine (`lib/commands.sh`) reads the same first line;
//! * the `command-furnished` gate and the use-case impact trace match a command anywhere in
//!   *any* header line, between word boundaries, so they would read a second header of a
//!   kind, and `commit` inside a word such as `capability:commit.plan`, where the index
//!   reads neither — case 507 holds every case to one header of each kind, and every header
//!   to naming the same public commands under both readings, so they cannot yet disagree;
//! * the capability pages (`scripts/lib/executable-site.jq`) give every capability the
//!   claims implemented in its module's file — the index gives `capability:<id>` the same
//!   claim members, and case 507 holds the two equal;
//! * the doctrine pages (`doctrines.json`) name the first test of each rule — case 507
//!   holds every one of them to a route or a mechanism of `rule:<id>`.
//!
//! The command words are the command graph's ([`crate::command_graph`]) as well: the public
//! commands of the shell tool and the runnable commands of the executable, which the
//! documented examples name one for one. Case 507 holds the command subjects, and the
//! programs each names, equal to `majordomus commands graph`.
//!
//! # Example
//!
//! ```
//! use majordomus_cli::evidence::subject::{self, SubjectKind};
//! use majordomus_cli::synthetic::SyntheticRepository;
//!
//! let repo = SyntheticRepository::small().unwrap();
//! let ctx = repo.context().unwrap();
//! let s = subject::index(&ctx.index, &ctx.product, &ctx.registry);
//!
//! // an MCP tool is an alias of the capability that declares it, never a subject of its own
//! let tool = s.get("mcp:majordomus_capabilities").unwrap();
//! let capability = tool.alias_of.as_deref().unwrap();
//! assert_eq!(SubjectKind::parse(capability).unwrap().0, SubjectKind::Capability);
//! assert_eq!(tool.members, vec![capability.to_string()]);
//! assert!(tool.page.is_none(), "an alias has no page of its own");
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::capability::{CapabilityRegistry, Context, Provenance};
use crate::evidence::{Ledger, TestId};
use crate::index::Index;
use crate::product::ProductModel;
use crate::Severity;

/// The schema the committed index declares, validated against its published contract.
pub const SCHEMA: &str = "majordomus-site-evidence-subjects/v1";

/// Where the committed index lives: the Rust executable's site directory, never the shell
/// site generator's, which that generator replaces wholesale.
pub const PATH: &str = "site/data/registry/evidence-subjects.json";

/// The manifest's document id of the committed index.
pub const DOCUMENT: &str = "site-evidence-subjects";

/// What the committed index is derived from, as its banner says.
pub const SOURCE: &str = "the claims, rules, features, use cases, commands, documented \
command-line examples and builtin capabilities this repository declares, and the first \
coverage and negative headers of its test cases";

/// The site route every subject page lives under.
pub const ROUTE: &str = "/evidence/";

/// A feature that reaches no test a runner drives: structural, advisory until the gate.
pub const FEATURE_WITHOUT_EVIDENCE: &str = "feature_without_evidence";

/// A command word both the shell tool and the executable answer to: structural, advisory.
pub const COMMAND_IN_TWO_PROGRAMS: &str = "command_in_two_programs";

/// Git could not compare an evidence commit with the presented revision.
pub const FRESHNESS_UNKNOWN: &str = "freshness_unknown";

/// A rule a subject is made of names a path that is not in the tree.
pub const DANGLING_MEMBER: &str = "dangling_member";

/// The header a case declares the commands it exercises with.
const COVERS: &str = "# majordomus-covers:";
/// The header a case declares the commands it refutes with.
const NEGATIVE: &str = "# majordomus-negative:";
/// The integration test binary that runs every documented command-line example.
const CLI_EXAMPLES: &str = "apps/majordomus-cli/tests/cli_examples.rs";
/// Where the suite's cases live.
const CASES_DIR: &str = "test/cases";
/// Where the crate's integration test binaries live.
const CRATE_TESTS_DIR: &str = "apps/majordomus-cli/tests";

// ---------------------------------------------------------------- the vocabulary

/// What kind of declaration a subject is. The declaration order is the order the committed
/// index lists its kinds in.
///
/// ```
/// use majordomus_cli::evidence::subject::SubjectKind;
///
/// let key = SubjectKind::Command.key("commit plan");
/// assert_eq!(key, "command:commit plan");
/// assert_eq!(SubjectKind::parse(&key), Some((SubjectKind::Command, "commit plan")));
/// assert_eq!(serde_json::to_value(SubjectKind::UseCase).unwrap(), "use_case");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidenceSubjectKind")]
pub enum SubjectKind {
    /// A claim of `docs/CLAIMS.yaml`.
    Claim,
    /// A rule of the layer, by id without its version.
    Rule,
    /// A feature under `.ai/repo/features/`.
    Feature,
    /// A public shell command or a native command path, its words joined by one space.
    Command,
    /// A builtin capability of the executable.
    Capability,
    /// An MCP tool name: an alias of its capability.
    Mcp,
    /// A use case under `.ai/repo/use-cases/`.
    UseCase,
}

impl SubjectKind {
    /// Every kind, in declaration order.
    pub const ALL: [SubjectKind; 7] = [
        SubjectKind::Claim,
        SubjectKind::Rule,
        SubjectKind::Feature,
        SubjectKind::Command,
        SubjectKind::Capability,
        SubjectKind::Mcp,
        SubjectKind::UseCase,
    ];

    /// The word a key starts with: the kind's serialised name.
    ///
    /// ```
    /// use majordomus_cli::evidence::subject::SubjectKind;
    /// assert_eq!(SubjectKind::Claim.prefix(), "claim");
    /// assert_eq!(SubjectKind::UseCase.prefix(), "use_case");
    /// ```
    pub fn prefix(self) -> &'static str {
        match self {
            SubjectKind::Claim => "claim",
            SubjectKind::Rule => "rule",
            SubjectKind::Feature => "feature",
            SubjectKind::Command => "command",
            SubjectKind::Capability => "capability",
            SubjectKind::Mcp => "mcp",
            SubjectKind::UseCase => "use_case",
        }
    }

    /// The key of the subject of this kind with this id: `<prefix>:<id>`.
    ///
    /// ```
    /// use majordomus_cli::evidence::subject::SubjectKind;
    /// assert_eq!(SubjectKind::Rule.key("project.scope"), "rule:project.scope");
    /// ```
    pub fn key(self, id: &str) -> String {
        format!("{}:{id}", self.prefix())
    }

    /// Read a key back into its kind and its id, or `None` when it is not a subject key.
    ///
    /// The key splits at its **first** `:`, so an id may carry colons and spaces of its own;
    /// the prefix must be a kind's, and the id must not be empty.
    ///
    /// ```
    /// use majordomus_cli::evidence::subject::SubjectKind;
    /// assert_eq!(
    ///     SubjectKind::parse("command:commit plan"),
    ///     Some((SubjectKind::Command, "commit plan"))
    /// );
    /// assert_eq!(SubjectKind::parse("gate:x"), None, "not a kind");
    /// assert_eq!(SubjectKind::parse("claim:"), None, "no id");
    /// assert_eq!(SubjectKind::parse("nonsense"), None);
    /// ```
    pub fn parse(key: &str) -> Option<(SubjectKind, &str)> {
        let (prefix, id) = key.split_once(':')?;
        if id.is_empty() {
            return None;
        }
        let kind = SubjectKind::ALL
            .into_iter()
            .find(|k| k.prefix() == prefix)?;
        Some((kind, id))
    }

    /// The site section a subject of this kind has its page in, or `None` when the kind has
    /// no page of its own (a rule, an MCP alias).
    ///
    /// ```
    /// use majordomus_cli::evidence::subject::SubjectKind;
    /// assert_eq!(SubjectKind::UseCase.section(), Some("use-cases"));
    /// assert_eq!(SubjectKind::Mcp.section(), None);
    /// ```
    pub fn section(self) -> Option<&'static str> {
        match self {
            SubjectKind::Claim => Some("claims"),
            SubjectKind::Rule => None,
            SubjectKind::Feature => Some("features"),
            SubjectKind::Command => Some("commands"),
            SubjectKind::Capability => Some("capabilities"),
            SubjectKind::Mcp => None,
            SubjectKind::UseCase => Some("use-cases"),
        }
    }
}

/// How a subject reaches a test. The declaration order is the order routes are listed in,
/// which puts the behavioural routes before the scenario.
///
/// ```
/// use majordomus_cli::evidence::subject::Via;
/// assert!(Via::Behaviour < Via::Scenario);
/// assert_eq!(serde_json::to_value(Via::Negative).unwrap(), "negative");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidenceVia")]
pub enum Via {
    /// The test a claim names.
    Claim,
    /// A test a rule's enforcement block names.
    Rule,
    /// A case whose first covers header names the subject.
    Behaviour,
    /// A case whose first negative header names the subject.
    Negative,
    /// The binary that runs every documented command-line example.
    Example,
    /// A use case's own scenario, which the ledger does not record.
    Scenario,
}

/// What kind of test a route reaches, read off the test's identity and never declared.
///
/// ```
/// use majordomus_cli::evidence::subject::Category;
/// assert_eq!(Category::of("suite:07_scope"), Some(Category::E2e));
/// assert_eq!(Category::of("crate:why"), Some(Category::Integration));
/// assert_eq!(Category::of("lib/x.sh"), None);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidenceCategory")]
pub enum Category {
    /// A behavioural case of the suite, driving the tools end to end.
    E2e,
    /// An integration test binary of the crate.
    Integration,
    /// A unit test inside the library (no runner records one yet).
    Unit,
    /// A documentation example (no runner records one yet).
    Doctest,
    /// A use case's scenario.
    Scenario,
}

impl Category {
    /// Every category, in declaration order.
    pub const ALL: [Category; 5] = [
        Category::E2e,
        Category::Integration,
        Category::Unit,
        Category::Doctest,
        Category::Scenario,
    ];

    /// The category of a test identity, or `None` when no runner's prefix is on it. The
    /// first matching prefix wins, so a library unit test is never read as an integration
    /// binary.
    ///
    /// ```
    /// use majordomus_cli::evidence::subject::Category;
    /// assert_eq!(Category::of("crate:lib::evidence::x"), Some(Category::Unit));
    /// assert_eq!(Category::of("crate:cli_examples"), Some(Category::Integration));
    /// assert_eq!(Category::of("doc:x"), Some(Category::Doctest));
    /// ```
    pub fn of(test: &str) -> Option<Category> {
        if test.starts_with("suite:") {
            Some(Category::E2e)
        } else if test.starts_with("crate:lib::") {
            Some(Category::Unit)
        } else if test.starts_with("crate:") {
            Some(Category::Integration)
        } else if test.starts_with("doc:") {
            Some(Category::Doctest)
        } else {
            None
        }
    }
}

/// One way a subject reaches a test, or the path it names when no runner drives it.
///
/// ```
/// use majordomus_cli::evidence::subject::{Category, Route, Via};
///
/// let route = Route {
///     via: Via::Behaviour,
///     test: Some("suite:07_scope".into()),
///     path: Some("test/cases/07_scope.sh".into()),
///     category: Category::of("suite:07_scope"),
///     inputs: None,
/// };
/// let json = serde_json::to_value(&route).unwrap();
/// assert_eq!(json["category"], "e2e");
/// // a route that declares no inputs says so by omission, never with an empty list
/// assert!(json.get("inputs").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceSubjectRoute")]
pub struct Route {
    /// How the subject reaches this test.
    pub via: Via,
    /// `TestId::as_string()` when a runner drives the path; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    /// The path the declaration names, verbatim, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// What kind of test it is, read off the test's identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
    /// The paths whose change since the run makes it stale. Absent = the route declares
    /// none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<String>>,
}

/// A structural finding about a subject: something its declarations say, not something a
/// run said. Advisory until the gate slice holds it.
///
/// ```
/// use majordomus_cli::evidence::subject::{SubjectFinding, FEATURE_WITHOUT_EVIDENCE};
/// use majordomus_cli::Severity;
///
/// let finding = SubjectFinding {
///     code: FEATURE_WITHOUT_EVIDENCE.into(),
///     severity: Severity::Error,
///     subject: "feature:lonely".into(),
///     message: "the feature `lonely` reaches no test".into(),
/// };
/// assert_eq!(serde_json::to_value(&finding).unwrap()["severity"], "error");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceSubjectFinding")]
pub struct SubjectFinding {
    /// One of [`FEATURE_WITHOUT_EVIDENCE`], [`COMMAND_IN_TWO_PROGRAMS`],
    /// [`FRESHNESS_UNKNOWN`] and [`DANGLING_MEMBER`].
    pub code: String,
    /// How much it matters.
    pub severity: Severity,
    /// The subject key it is about.
    pub subject: String,
    /// What it means, in one sentence.
    pub message: String,
}

/// Which program answers to a command word: the shell tool, or the Rust executable. A word
/// both answer to names both, in this order.
///
/// These are two of the command graph's [`crate::command_graph::Origin`]s, read from the
/// same declarations: `Native` is a runnable command of the executable
/// (`Origin::Executable`), and `Shell` a command of the shell tool (`Origin::Tool`), whose
/// registry `share/commands.yaml` both read. A group of the executable that only holds
/// other commands, such as `majordomus evidence`, is not a command the executable answers
/// to. Case 507 holds the command subjects and their programs equal to the command graph's.
///
/// ```
/// use majordomus_cli::evidence::subject::Program;
/// assert!(Program::Native < Program::Shell);
/// assert_eq!(serde_json::to_value(Program::Shell).unwrap(), "shell");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Program {
    /// The Rust executable: a documented example path or a capability's command-line path.
    /// `cli::validate` gives every runnable command an example and refuses one on a group,
    /// so these are exactly the executable's runnable commands.
    Native,
    /// The shell tool: a public command of its registry.
    Shell,
}

// ---------------------------------------------------------------- the index

/// One subject of the index: what it is, what it is made of and the tests it reaches.
///
/// ```
/// use majordomus_cli::evidence::subject::{self, SubjectEntry, SubjectKind};
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let s = subject::index(&ctx.index, &ctx.product, &ctx.registry);
/// let entry: &SubjectEntry = s.get("capability:capabilities.list").unwrap();
/// assert_eq!(entry.kind, SubjectKind::Capability);
/// assert_eq!(entry.page.as_deref(), Some("/evidence/capabilities/capabilities-list/"));
/// assert!(entry.members.contains(&"command:capabilities list".to_string()));
/// assert!(entry.programs.is_empty(), "only a command names its programs");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubjectEntry {
    /// What kind of declaration it is.
    pub kind: SubjectKind,
    /// Its id within the kind.
    pub id: String,
    /// The status its declaration states: a claim, a rule, a feature or a use case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Its page on the site: `/evidence/<section>/<slug>/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// The capability an MCP tool is an alias of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias_of: Option<String>,
    /// The programs a command word belongs to, in declaration order; empty for every other
    /// kind.
    pub programs: Vec<Program>,
    /// The subjects it is made of, by key, unique, in byte order.
    pub members: Vec<String>,
    /// Its own routes, in (via, test, path) order.
    pub routes: Vec<Route>,
    /// A rule's named paths that no runner drives.
    pub mechanisms: Vec<String>,
    /// Its structural findings.
    pub findings: Vec<SubjectFinding>,
}

/// Every subject of a repository, and every test nothing reaches.
///
/// ```
/// use majordomus_cli::evidence::subject::{self, SubjectIndex};
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let s: SubjectIndex = subject::index(&ctx.index, &ctx.product, &ctx.registry);
/// assert!(s.subjects.keys().any(|k| k.starts_with("capability:")));
/// assert!(s.excluded.is_empty(), "a synthetic tree carries no tests");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubjectIndex {
    /// Every subject, by key.
    pub subjects: BTreeMap<String, SubjectEntry>,
    /// Every test a runner owns in this tree that no route reaches, as test ids, byte order.
    pub excluded: Vec<String>,
}

/// One page the site renders for a subject.
///
/// ```
/// use majordomus_cli::evidence::subject::{self, Page, SubjectKind};
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let pages: Vec<Page> = subject::index(&ctx.index, &ctx.product, &ctx.registry).pages();
/// assert!(pages.iter().all(|p| p.path.starts_with("/evidence/")));
/// assert!(pages.iter().all(|p| p.kind != SubjectKind::Mcp), "an alias has no page");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    /// The subject's kind.
    pub kind: SubjectKind,
    /// The subject's key.
    pub subject: String,
    /// The page's path on the site.
    pub path: String,
}

/// A key that names no subject of this repository.
///
/// ```
/// use majordomus_cli::evidence::subject::{self, UnknownSubject};
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let s = subject::index(&ctx.index, &ctx.product, &ctx.registry);
/// let refused: UnknownSubject = s.get("feature:no-such").unwrap_err();
/// assert_eq!(refused.key, "feature:no-such");
/// assert!(refused.to_string().contains("`feature:no-such` is not a subject"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSubject {
    /// The key that was asked for.
    pub key: String,
}

impl fmt::Display for UnknownSubject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not a subject of this repository: a subject is <kind>:<id> for a declared \
             claim, rule, feature, command, capability, MCP tool or use case",
            self.key
        )
    }
}

impl std::error::Error for UnknownSubject {}

impl SubjectIndex {
    /// The subject a key names, or the refusal that names the key.
    ///
    /// ```
    /// use majordomus_cli::evidence::subject;
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let s = subject::index(&ctx.index, &ctx.product, &ctx.registry);
    /// assert!(s.get("command:capabilities list").is_ok());
    /// assert!(s.get("gate:x").is_err());
    /// ```
    pub fn get(&self, key: &str) -> Result<&SubjectEntry, UnknownSubject> {
        self.subjects.get(key).ok_or_else(|| UnknownSubject {
            key: key.to_string(),
        })
    }

    /// Every page the site renders, ordered by path. An MCP alias has no page of its own.
    ///
    /// ```
    /// use majordomus_cli::evidence::subject;
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let pages = subject::index(&ctx.index, &ctx.product, &ctx.registry).pages();
    /// let paths: Vec<&str> = pages.iter().map(|p| p.path.as_str()).collect();
    /// assert!(paths.windows(2).all(|w| w[0] < w[1]), "ordered by path, each once");
    /// ```
    pub fn pages(&self) -> Vec<Page> {
        let mut by_path: BTreeMap<String, Page> = BTreeMap::new();
        for (key, e) in &self.subjects {
            if let Some(path) = &e.page {
                by_path.entry(path.clone()).or_insert_with(|| Page {
                    kind: e.kind,
                    subject: key.clone(),
                    path: path.clone(),
                });
            }
        }
        by_path.into_values().collect()
    }

    /// Every page path more than one subject claims, with those subjects' keys. Empty on a
    /// sound index; [`artifact`] refuses to write one that is not.
    ///
    /// ```
    /// use majordomus_cli::evidence::subject;
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let s = subject::index(&ctx.index, &ctx.product, &ctx.registry);
    /// assert!(s.duplicate_pages().is_empty());
    /// ```
    pub fn duplicate_pages(&self) -> Vec<(String, Vec<String>)> {
        let mut by_path: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for (key, e) in &self.subjects {
            if let Some(path) = &e.page {
                by_path.entry(path.as_str()).or_default().push(key.clone());
            }
        }
        by_path
            .into_iter()
            .filter(|(_, keys)| keys.len() > 1)
            .map(|(path, keys)| (path.to_string(), keys))
            .collect()
    }

    /// Every route a subject reaches: its own routes, then each member's, depth first in
    /// member order, each subject walked once and each route listed once.
    ///
    /// This is the one walk of the member graph; the structural finding of a feature is
    /// read off it, and a surface that needs what a subject reaches calls it rather than
    /// walking the members again.
    ///
    /// ```
    /// use majordomus_cli::evidence::subject::{self, Via};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let s = subject::index(&ctx.index, &ctx.product, &ctx.registry);
    /// // an alias reaches what its capability reaches, and nothing the tree does not carry
    /// let reached = s.reach("mcp:majordomus_capabilities").unwrap();
    /// assert!(reached.iter().all(|r| r.via != Via::Scenario));
    /// assert!(s.reach("feature:no-such").is_err());
    /// ```
    pub fn reach(&self, key: &str) -> Result<Vec<&Route>, UnknownSubject> {
        let (key, entry) = self
            .subjects
            .get_key_value(key)
            .ok_or_else(|| UnknownSubject {
                key: key.to_string(),
            })?;
        let mut walked: BTreeSet<&str> = BTreeSet::new();
        let mut listed: BTreeSet<&Route> = BTreeSet::new();
        let mut out = Vec::new();
        self.walk(key, entry, &mut walked, &mut listed, &mut out);
        Ok(out)
    }

    /// One step of [`SubjectIndex::reach`]. A member is always a key of the index, because
    /// `derive` adds only members that exist; one that did not would be passed over.
    fn walk<'s>(
        &'s self,
        key: &'s str,
        entry: &'s SubjectEntry,
        walked: &mut BTreeSet<&'s str>,
        listed: &mut BTreeSet<&'s Route>,
        out: &mut Vec<&'s Route>,
    ) {
        if !walked.insert(key) {
            return;
        }
        for route in &entry.routes {
            if listed.insert(route) {
                out.push(route);
            }
        }
        for (member, e) in entry
            .members
            .iter()
            .filter_map(|m| self.subjects.get_key_value(m))
        {
            self.walk(member, e, walked, listed, out);
        }
    }

    /// The committed index as a document: the header, the kinds, the categories, the pages,
    /// every subject by key and every test nothing reaches, in that order. Structure only:
    /// no commit, no ledger row and no time.
    ///
    /// ```
    /// use majordomus_cli::evidence::subject::{self, SCHEMA};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let doc = subject::index(&ctx.index, &ctx.product, &ctx.registry).document("0.0.0");
    /// assert_eq!(doc["schema"], SCHEMA);
    /// assert_eq!(doc["generator"]["version"], "0.0.0");
    /// let members: Vec<&String> = doc.as_object().unwrap().keys().collect();
    /// assert_eq!(members.first().map(|k| k.as_str()), Some("schema"));
    /// assert_eq!(members.last().map(|k| k.as_str()), Some("excluded"));
    /// ```
    pub fn document(&self, version: &str) -> Value {
        let kinds: Vec<Value> = SubjectKind::ALL
            .into_iter()
            .map(|k| match k.section() {
                Some(section) => json!({ "kind": k, "section": section }),
                None => json!({ "kind": k }),
            })
            .collect();
        json!({
            "schema": SCHEMA,
            "generated": crate::generate::json_banner(SOURCE),
            "generator": { "id": "majordomus-cli", "version": version },
            "route": ROUTE,
            "kinds": kinds,
            "categories": Category::ALL,
            "pages": self.pages(),
            "subjects": self.subjects,
            "excluded": self.excluded,
        })
    }
}

// ---------------------------------------------------------------- the declarations

/// What the index is derived from: every declaration, as its owner reads it, and the tests
/// on disk. `derive` is a pure function of this.
#[derive(Debug, Clone, Default)]
pub(crate) struct Declarations {
    pub(crate) claims: Vec<ClaimDecl>,
    pub(crate) rules: Vec<RuleDecl>,
    pub(crate) features: Vec<FeatureDecl>,
    pub(crate) use_cases: Vec<UseCaseDecl>,
    /// Public shell commands.
    pub(crate) commands: Vec<String>,
    /// Native command paths with documented examples, non-empty.
    pub(crate) examples: Vec<String>,
    pub(crate) capabilities: Vec<CapabilityDecl>,
    /// `test/cases/*.sh` on disk.
    pub(crate) cases: Vec<CaseDecl>,
    /// `apps/majordomus-cli/tests/*.rs` on disk, as `crate:<stem>`.
    pub(crate) crate_tests: Vec<String>,
}

/// A claim, with structure only: what the evidence join reads, never a state.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClaimDecl {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) source: Option<String>,
    pub(crate) implementation: Option<String>,
    pub(crate) test_path: Option<String>,
    pub(crate) test: Option<String>,
}

/// A rule: its id without the version, its status, its file and every path it names.
#[derive(Debug, Clone, Default)]
pub(crate) struct RuleDecl {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) path: String,
    pub(crate) tests: Vec<String>,
}

/// A feature and the four references a subject is made of.
#[derive(Debug, Clone, Default)]
pub(crate) struct FeatureDecl {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) claims: Vec<String>,
    pub(crate) rules: Vec<String>,
    pub(crate) commands: Vec<String>,
    pub(crate) use_cases: Vec<String>,
}

/// A use case and the four references `graph.rs` draws edges from.
#[derive(Debug, Clone, Default)]
pub(crate) struct UseCaseDecl {
    pub(crate) id: String,
    pub(crate) status: Option<String>,
    pub(crate) path: String,
    pub(crate) commands: Vec<String>,
    pub(crate) doctrines: Vec<String>,
    pub(crate) claims: Vec<String>,
    pub(crate) mcp_tools: Vec<String>,
}

/// A builtin capability: its id, its command-line path, its MCP tool and the file its
/// module is composed in.
#[derive(Debug, Clone, Default)]
pub(crate) struct CapabilityDecl {
    pub(crate) id: String,
    pub(crate) cli: Option<String>,
    pub(crate) mcp_tool: Option<String>,
    pub(crate) source: String,
}

/// A case of the suite: its identity, its path and the words of its first covers and
/// first negative header.
#[derive(Debug, Clone, Default)]
pub(crate) struct CaseDecl {
    pub(crate) test: String,
    pub(crate) path: String,
    pub(crate) covers: Vec<String>,
    pub(crate) negative: Vec<String>,
}

/// Every subject of a repository, derived from what it declares.
///
/// ```
/// use majordomus_cli::capability::Provenance;
/// use majordomus_cli::evidence::subject;
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let s = subject::index(&ctx.index, &ctx.product, &ctx.registry);
/// // every builtin capability with a tool is reachable by that tool's name
/// for c in ctx.registry.iter() {
///     if !matches!(c.provenance, Provenance::Builtin { .. }) {
///         continue;
///     }
///     if let Some(tool) = c.exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()) {
///         let alias = s.get(&format!("mcp:{tool}")).unwrap();
///         assert_eq!(alias.alias_of, Some(format!("capability:{}", c.id)));
///     }
/// }
/// ```
pub fn index(index: &Index, product: &ProductModel, registry: &CapabilityRegistry) -> SubjectIndex {
    derive(&declarations(index, product, registry))
}

/// Read every declaration from its owner. Deterministic, and reads no clock and no ledger.
pub(crate) fn declarations(
    index: &Index,
    product: &ProductModel,
    registry: &CapabilityRegistry,
) -> Declarations {
    let root = PathBuf::from(&index.repository.root);
    // the claims as the evidence join reads them, with structure only: an empty ledger, so
    // no state of it is read and the join's own test grammar is the one used
    let claims = crate::evidence::report(index, &Ledger::empty())
        .claims
        .into_iter()
        .map(|c| ClaimDecl {
            id: c.id,
            status: c.status,
            source: c.source,
            implementation: c.implementation,
            test_path: c.test_path,
            test: c.test,
        })
        .collect();
    let rules = crate::rules::definitions(index)
        .into_iter()
        .map(|r| RuleDecl {
            id: r.id,
            status: r.status,
            path: r.path,
            tests: r.enforcement.tests,
        })
        .collect();
    let features = product
        .all()
        .iter()
        .map(|r| FeatureDecl {
            id: r.feature.id.clone(),
            status: r.feature.status.clone(),
            claims: r.feature.claims.clone(),
            rules: r.feature.rules.clone(),
            commands: r.feature.commands.clone(),
            use_cases: r.feature.use_cases.clone(),
        })
        .collect();
    let strings = |o: &crate::Object, key: &str| crate::graph::metadata_strings(&o.metadata, key);
    let use_cases = index
        .objects
        .iter()
        .filter(|o| o.kind == "use-case")
        .map(|o| UseCaseDecl {
            id: o.identity.clone(),
            status: o
                .metadata
                .get("status")
                .and_then(Value::as_str)
                .map(str::to_string),
            path: o.provenance.path.clone(),
            commands: strings(o, "commands"),
            doctrines: strings(o, "doctrines"),
            claims: strings(o, "claims"),
            mcp_tools: strings(o, "mcp_tools"),
        })
        .collect();
    // the set the product model reads: a public command only, since an internal one is
    // dispatched and absent from the help text
    let commands = index
        .objects
        .iter()
        .filter(|o| o.kind == "command")
        .filter(|o| o.metadata.get("visibility").and_then(Value::as_str) == Some("public"))
        .map(|o| o.identity.clone())
        .collect();
    let examples = crate::cli::EXAMPLES
        .iter()
        .map(|e| e.command)
        .filter(|c| !c.is_empty())
        .map(str::to_string)
        .collect();
    let capabilities = registry
        .iter()
        .filter(|c| matches!(c.provenance, Provenance::Builtin { .. }))
        .map(|c| CapabilityDecl {
            id: c.id.as_str().to_string(),
            cli: c.exposure.cli.as_ref().map(|x| x.path.join(" ")),
            mcp_tool: c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()),
            source: c.provenance.source_path(),
        })
        .collect();
    Declarations {
        claims,
        rules,
        features,
        use_cases,
        commands,
        examples,
        capabilities,
        cases: read_cases(&root),
        crate_tests: read_crate_tests(&root),
    }
}

/// The words of the first line of `text` that starts with `header`, `none` dropped. The
/// first line decides, as the command pages read it; a later line of the same kind names
/// nothing.
fn header_words(text: &str, header: &str) -> Vec<String> {
    text.lines()
        .find_map(|line| line.strip_prefix(header))
        .map(|rest| {
            rest.split_whitespace()
                .filter(|w| *w != "none")
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The regular files of `root/dir` whose repository path `TestId::of` accepts, with that
/// path. A directory that is not there holds none.
fn test_files(root: &Path, dir: &str) -> Vec<(String, TestId, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            // read lossily rather than dropped: the suite's own glob runs a file whatever
            // bytes its name holds, so a test the runner sees is never one the index hides
            let name = e.file_name().to_string_lossy().into_owned();
            let path = format!("{dir}/{name}");
            let test = TestId::of(&path)?;
            Some((path, test, e.path()))
        })
        .collect()
}

/// Every case of the suite on disk, with its first covers and first negative header. An
/// unreadable file declares no words.
fn read_cases(root: &Path) -> Vec<CaseDecl> {
    test_files(root, CASES_DIR)
        .into_iter()
        .map(|(path, test, file)| {
            let text = std::fs::read(&file)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            CaseDecl {
                test: test.as_string(),
                path,
                covers: header_words(&text, COVERS),
                negative: header_words(&text, NEGATIVE),
            }
        })
        .collect()
}

/// Every integration test binary of the crate on disk, as `crate:<stem>`.
fn read_crate_tests(root: &Path) -> Vec<String> {
    test_files(root, CRATE_TESTS_DIR)
        .into_iter()
        .map(|(_, test, _)| test.as_string())
        .collect()
}

// ---------------------------------------------------------------- the derivation

/// The site's route segment for an id: every `.`, `_` and space becomes `-`, as Zola
/// slugifies a path (`scripts/lib/executable-site.jq`'s `slug`, plus the space of a command
/// path).
fn slug(id: &str) -> String {
    id.chars()
        .map(|c| if matches!(c, '.' | '_' | ' ') { '-' } else { c })
        .collect()
}

/// The page of a subject of a kind that has one.
fn page_of(kind: SubjectKind, id: &str) -> Option<String> {
    kind.section()
        .map(|section| format!("{ROUTE}{section}/{}/", slug(id)))
}

/// A subject while it is being derived: its kind and id as it was opened with, and every
/// collection a set, so that the result does not depend on the order the declarations came
/// in.
struct Draft {
    kind: SubjectKind,
    id: String,
    status: Option<String>,
    alias_of: Option<String>,
    programs: BTreeSet<Program>,
    members: BTreeSet<String>,
    routes: BTreeSet<Route>,
    mechanisms: BTreeSet<String>,
}

/// The draft of the subject of this kind with this id, opened empty the first time it is
/// asked for. Every draft is opened here, so its key, kind and id always agree.
fn draft_of<'d>(
    drafts: &'d mut BTreeMap<String, Draft>,
    kind: SubjectKind,
    id: &str,
) -> &'d mut Draft {
    drafts.entry(kind.key(id)).or_insert_with(|| Draft {
        kind,
        id: id.to_string(),
        status: None,
        alias_of: None,
        programs: BTreeSet::new(),
        members: BTreeSet::new(),
        routes: BTreeSet::new(),
        mechanisms: BTreeSet::new(),
    })
}

impl Draft {
    /// Two declarations of one subject keep the smaller status, whichever came first.
    fn status(&mut self, status: &str) {
        let keep = match self.status.take() {
            Some(held) if held.as_str() <= status => held,
            _ => status.to_string(),
        };
        self.status = Some(keep);
    }
}

/// Derive the index. Pure: no I/O, and the result does not depend on the order of any
/// input vector.
pub(crate) fn derive(d: &Declarations) -> SubjectIndex {
    let mut drafts: BTreeMap<String, Draft> = BTreeMap::new();

    // ---- leaves: claims, rules, commands
    for c in &d.claims {
        let inputs: Vec<String> = [
            c.source.clone(),
            c.implementation.clone(),
            c.test_path
                .as_deref()
                .and_then(TestId::of)
                .map(|t| t.source()),
        ]
        .into_iter()
        .flatten()
        .collect();
        let draft = draft_of(&mut drafts, SubjectKind::Claim, &c.id);
        draft.status(&c.status);
        draft.routes.insert(Route {
            via: Via::Claim,
            test: c.test.clone(),
            path: c.test_path.clone(),
            category: c.test.as_deref().and_then(Category::of),
            inputs: Some(inputs),
        });
    }
    for r in &d.rules {
        let draft = draft_of(&mut drafts, SubjectKind::Rule, &r.id);
        draft.status(&r.status);
        for p in &r.tests {
            match TestId::of(p) {
                Some(t) => {
                    let test = t.as_string();
                    draft.routes.insert(Route {
                        via: Via::Rule,
                        category: Category::of(&test),
                        test: Some(test),
                        path: Some(p.clone()),
                        inputs: Some(vec![t.source(), r.path.clone()]),
                    });
                }
                None => {
                    draft.mechanisms.insert(p.clone());
                }
            }
        }
    }

    let shell: BTreeSet<&str> = d.commands.iter().map(String::as_str).collect();
    let examples: BTreeSet<&str> = d.examples.iter().map(String::as_str).collect();
    let native: BTreeSet<&str> = examples
        .iter()
        .copied()
        .chain(d.capabilities.iter().filter_map(|c| c.cli.as_deref()))
        .collect();
    // the example route exists only where the binary that runs the examples does: a route
    // to a test the repository does not carry would read `not_run` forever
    let example_route = TestId::of(CLI_EXAMPLES)
        .filter(|t| d.crate_tests.contains(&t.as_string()))
        .map(|t| {
            let test = t.as_string();
            Route {
                via: Via::Example,
                category: Category::of(&test),
                test: Some(test),
                path: Some(t.source()),
                inputs: None,
            }
        });
    for w in shell.union(&native) {
        let draft = draft_of(&mut drafts, SubjectKind::Command, w);
        if native.contains(w) {
            draft.programs.insert(Program::Native);
        }
        if shell.contains(w) {
            draft.programs.insert(Program::Shell);
        }
        if let Some(route) = example_route.as_ref().filter(|_| examples.contains(w)) {
            draft.routes.insert(route.clone());
        }
    }

    // ---- capabilities and their aliases
    for c in &d.capabilities {
        let key = SubjectKind::Capability.key(&c.id);
        let draft = draft_of(&mut drafts, SubjectKind::Capability, &c.id);
        if let Some(cli) = &c.cli {
            draft.members.insert(SubjectKind::Command.key(cli));
        }
        // a claim implemented in the file the capability's module is composed in belongs to
        // every capability of that module, and a claim implemented anywhere else to none
        for claim in &d.claims {
            if claim.implementation.as_deref() == Some(c.source.as_str()) {
                draft.members.insert(SubjectKind::Claim.key(&claim.id));
            }
        }
        if let Some(tool) = &c.mcp_tool {
            let alias = draft_of(&mut drafts, SubjectKind::Mcp, tool);
            alias.members.insert(key.clone());
            alias.alias_of = match alias.alias_of.take() {
                Some(held) if held <= key => Some(held),
                _ => Some(key),
            };
        }
    }

    // ---- the case headers: a command word routes to its command, a `capability:<id>` word
    // to that capability, and every other prefixed word to nothing
    for case in &d.cases {
        for (via, words) in [
            (Via::Behaviour, &case.covers),
            (Via::Negative, &case.negative),
        ] {
            for w in words {
                let key = if w.contains(':') {
                    match SubjectKind::parse(w) {
                        Some((SubjectKind::Capability, _)) => w.clone(),
                        _ => continue,
                    }
                } else {
                    SubjectKind::Command.key(w)
                };
                if let Some(draft) = drafts.get_mut(&key) {
                    draft.routes.insert(Route {
                        via,
                        test: Some(case.test.clone()),
                        path: Some(case.path.clone()),
                        category: Category::of(&case.test),
                        inputs: None,
                    });
                }
            }
        }
    }

    // ---- composites: use cases, then features. A member is a key that exists; a
    // reference that resolves to nothing is the product validation's to report.
    let exists = |drafts: &BTreeMap<String, Draft>, kind: SubjectKind, id: &str| {
        let key = kind.key(id);
        drafts.contains_key(&key).then_some(key)
    };
    for u in &d.use_cases {
        let members: BTreeSet<String> = [
            (SubjectKind::Command, &u.commands),
            (SubjectKind::Rule, &u.doctrines),
            (SubjectKind::Claim, &u.claims),
            (SubjectKind::Mcp, &u.mcp_tools),
        ]
        .into_iter()
        .flat_map(|(kind, ids)| ids.iter().map(move |id| (kind, id)))
        .filter_map(|(kind, id)| exists(&drafts, kind, id))
        .collect();
        let draft = draft_of(&mut drafts, SubjectKind::UseCase, &u.id);
        if let Some(status) = &u.status {
            draft.status(status);
        }
        draft.members.extend(members);
        draft.routes.insert(Route {
            via: Via::Scenario,
            test: None,
            path: Some(u.path.clone()),
            category: Some(Category::Scenario),
            inputs: None,
        });
    }
    for f in &d.features {
        let members: BTreeSet<String> = [
            (SubjectKind::Claim, &f.claims),
            (SubjectKind::Rule, &f.rules),
            (SubjectKind::Command, &f.commands),
            (SubjectKind::UseCase, &f.use_cases),
        ]
        .into_iter()
        .flat_map(|(kind, ids)| ids.iter().map(move |id| (kind, id)))
        .filter_map(|(kind, id)| exists(&drafts, kind, id))
        .collect();
        let draft = draft_of(&mut drafts, SubjectKind::Feature, &f.id);
        draft.status(&f.status);
        draft.members.extend(members);
    }

    // ---- the entries
    let mut subjects: BTreeMap<String, SubjectEntry> = BTreeMap::new();
    for (key, draft) in drafts {
        let mut findings = Vec::new();
        if draft.programs.len() > 1 {
            findings.push(SubjectFinding {
                code: COMMAND_IN_TWO_PROGRAMS.to_string(),
                severity: Severity::Warning,
                subject: key.clone(),
                message: format!(
                    "the word `{}` is a public command of the shell tool and a command path \
                     of the executable: its behaviour and negative routes come from the shell \
                     tool's case headers, its example route from the executable's documented \
                     examples, and one page shows both",
                    draft.id
                ),
            });
        }
        let page = page_of(draft.kind, &draft.id);
        let entry = SubjectEntry {
            kind: draft.kind,
            id: draft.id,
            status: draft.status,
            page,
            alias_of: draft.alias_of,
            programs: draft.programs.into_iter().collect(),
            members: draft.members.into_iter().collect(),
            routes: draft.routes.into_iter().collect(),
            mechanisms: draft.mechanisms.into_iter().collect(),
            findings,
        };
        subjects.insert(key, entry);
    }

    // ---- every test a runner owns that no route reaches
    let reached: BTreeSet<&str> = subjects
        .values()
        .flat_map(|e| e.routes.iter().filter_map(|r| r.test.as_deref()))
        .collect();
    let excluded: Vec<String> = d
        .cases
        .iter()
        .map(|c| c.test.as_str())
        .chain(d.crate_tests.iter().map(String::as_str))
        .filter(|t| !reached.contains(t))
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut index = SubjectIndex { subjects, excluded };

    // ---- the structural finding, read off the one walk
    let features: Vec<(String, String, Option<String>)> = index
        .subjects
        .iter()
        .filter(|(_, e)| e.kind == SubjectKind::Feature)
        .filter(|(key, _)| {
            index
                .reach(key)
                .map(|routes| routes.iter().all(|r| r.test.is_none()))
                .unwrap_or(false)
        })
        .map(|(key, e)| (key.clone(), e.id.clone(), e.status.clone()))
        .collect();
    for (key, id, status) in features {
        let severity = if status.as_deref() == Some(crate::product::STABLE) {
            Severity::Error
        } else {
            Severity::Warning
        };
        if let Some(e) = index.subjects.get_mut(&key) {
            e.findings.push(SubjectFinding {
                code: FEATURE_WITHOUT_EVIDENCE.to_string(),
                severity,
                subject: key.clone(),
                message: format!(
                    "the feature `{id}` reaches no test a runner drives through its claims, \
                     rules, commands or use cases"
                ),
            });
        }
    }
    index
}

// ---------------------------------------------------------------- the artifact

/// The committed index, as `majordomus generate site` writes it to [`PATH`].
///
/// Refuses rather than writes when two subjects claim one page, and when the rendered
/// document carries anything `generate::forbidden_in` keeps out of published text.
///
/// ```
/// use majordomus_cli::evidence::subject::{self, DOCUMENT, PATH, SCHEMA};
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let a = subject::artifact(&ctx, "0.0.0").unwrap();
/// assert_eq!(a.path, PATH);
/// assert_eq!(a.document, DOCUMENT);
/// assert_eq!(a.schema.as_deref(), Some(SCHEMA));
/// assert!(a.content.ends_with("}\n"));
/// ```
pub fn artifact(ctx: &Context, version: &str) -> crate::error::Result<crate::generate::Artifact> {
    rendered(&index(&ctx.index, &ctx.product, &ctx.registry), version)
}

/// Steps two to five of [`artifact`], over an index already derived.
fn rendered(s: &SubjectIndex, version: &str) -> crate::error::Result<crate::generate::Artifact> {
    let duplicates = s.duplicate_pages();
    if !duplicates.is_empty() {
        let named: Vec<String> = duplicates
            .iter()
            .map(|(path, keys)| format!("{path} ({})", keys.join(", ")))
            .collect();
        return Err(crate::error::Error::Protocol {
            reason: format!(
                "two subjects claim one evidence page, so neither page would say which it is \
                 about: {}",
                named.join("; ")
            ),
        });
    }
    let content = crate::http::openapi::render(&s.document(version));
    if let Some((marker, what)) = crate::generate::forbidden_in(&content) {
        return Err(crate::error::Error::Protocol {
            reason: format!(
                "{PATH} would publish {what} (it contains `{marker}`); nothing was written"
            ),
        });
    }
    Ok(crate::generate::Artifact::verbatim(
        PATH,
        DOCUMENT,
        crate::generate::ArtifactFormat::Json,
        Some(SCHEMA.to_string()),
        SOURCE,
        content,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(id: &str, status: &str, implementation: &str, test: &str) -> ClaimDecl {
        let opt = |s: &str| (!s.is_empty() && s != "-").then(|| s.to_string());
        ClaimDecl {
            id: id.into(),
            status: status.into(),
            source: Some("docs/A.md".into()),
            implementation: opt(implementation),
            test_path: opt(test),
            test: opt(test)
                .as_deref()
                .and_then(TestId::of)
                .map(|t| t.as_string()),
        }
    }
    fn case(name: &str, covers: &[&str], negative: &[&str]) -> CaseDecl {
        let path = format!("test/cases/{name}.sh");
        CaseDecl {
            test: TestId::of(&path).unwrap().as_string(),
            path,
            covers: covers.iter().map(|s| s.to_string()).collect(),
            negative: negative.iter().map(|s| s.to_string()).collect(),
        }
    }
    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }
    fn cap(id: &str, cli: Option<&str>, tool: Option<&str>, module: &str) -> CapabilityDecl {
        CapabilityDecl {
            id: id.into(),
            cli: cli.map(str::to_string),
            mcp_tool: tool.map(str::to_string),
            source: format!("apps/majordomus-cli/src/capability/builtin/{module}.rs"),
        }
    }

    /// A small world: a claim, a rule, two commands, a capability with a tool, a use case,
    /// two features and a few cases.
    fn world() -> Declarations {
        Declarations {
            claims: vec![
                claim(
                    "alpha",
                    "guaranteed",
                    "lib/alpha.sh",
                    "test/cases/01_alpha.sh",
                ),
                claim("planned", "planned", "-", "-"),
                claim(
                    "in-module",
                    "advisory",
                    "apps/majordomus-cli/src/capability/builtin/things.rs",
                    "test/cases/02_mod.sh",
                ),
            ],
            rules: vec![RuleDecl {
                id: "project.alpha".into(),
                status: "active".into(),
                path: ".ai/repo/rules/project/alpha.v1.md".into(),
                tests: strings(&["test/cases/01_alpha.sh", "scripts/alpha-gate"]),
            }],
            features: vec![
                FeatureDecl {
                    id: "alpha".into(),
                    status: "stable".into(),
                    claims: strings(&["alpha", "planned", "no-such-claim"]),
                    rules: strings(&["project.alpha"]),
                    commands: strings(&["version"]),
                    use_cases: strings(&["see-alpha"]),
                },
                FeatureDecl {
                    id: "lonely".into(),
                    status: "stable".into(),
                    claims: strings(&["planned"]),
                    ..FeatureDecl::default()
                },
                FeatureDecl {
                    id: "drafty".into(),
                    status: "draft".into(),
                    claims: strings(&["planned"]),
                    ..FeatureDecl::default()
                },
            ],
            use_cases: vec![UseCaseDecl {
                id: "see-alpha".into(),
                status: Some("active".into()),
                path: ".ai/repo/use-cases/see-alpha.md".into(),
                commands: strings(&["version"]),
                doctrines: strings(&["project.alpha"]),
                claims: strings(&["alpha"]),
                mcp_tools: strings(&["majordomus_things"]),
            }],
            commands: strings(&["version", "bench"]),
            examples: strings(&["bench", "commit plan"]),
            capabilities: vec![
                cap(
                    "things.list",
                    Some("things list"),
                    Some("majordomus_things"),
                    "things",
                ),
                cap("things.show_one", None, None, "things"),
                cap("other.x", None, Some("majordomus_other"), "other"),
            ],
            cases: vec![
                case("01_alpha", &["version"], &["version"]),
                case("02_mod", &[], &[]),
                case("03_bench", &["bench", "gate:x"], &[]),
                case("04_orphan", &[], &[]),
                case(
                    "05_cap",
                    &["capability:things.list"],
                    &["capability:no.such"],
                ),
            ],
            crate_tests: strings(&["crate:cli_examples", "crate:lonely"]),
        }
    }

    fn entry<'a>(s: &'a SubjectIndex, key: &str) -> &'a SubjectEntry {
        s.get(key).unwrap_or_else(|e| panic!("{e}"))
    }
    fn tests_of(e: &SubjectEntry, via: Via) -> Vec<&str> {
        e.routes
            .iter()
            .filter(|r| r.via == via)
            .filter_map(|r| r.test.as_deref())
            .collect()
    }

    #[test]
    fn a_key_parses_back_to_its_kind() {
        for k in SubjectKind::ALL {
            let key = k.key("an.id with space");
            assert_eq!(SubjectKind::parse(&key), Some((k, "an.id with space")));
            assert_eq!(serde_json::to_value(k).unwrap(), k.prefix());
        }
        assert_eq!(
            SubjectKind::parse("command:commit plan"),
            Some((SubjectKind::Command, "commit plan"))
        );
        // the first colon splits, so an id keeps its own
        assert_eq!(
            SubjectKind::parse("capability:a:b"),
            Some((SubjectKind::Capability, "a:b"))
        );
        for bad in ["gate:x", "claim:", "x", ""] {
            assert_eq!(SubjectKind::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_category_is_read_off_the_test_identity() {
        assert_eq!(Category::of("suite:a"), Some(Category::E2e));
        assert_eq!(Category::of("crate:lib::x"), Some(Category::Unit));
        assert_eq!(Category::of("crate:why"), Some(Category::Integration));
        assert_eq!(Category::of("doc:x"), Some(Category::Doctest));
        assert_eq!(Category::of("other:x"), None);
    }

    #[test]
    fn a_claim_has_one_route_with_the_joins_inputs() {
        let s = derive(&world());
        let alpha = entry(&s, "claim:alpha");
        assert_eq!(alpha.routes.len(), 1);
        let r = &alpha.routes[0];
        assert_eq!(r.via, Via::Claim);
        assert_eq!(r.test.as_deref(), Some("suite:01_alpha"));
        assert_eq!(r.path.as_deref(), Some("test/cases/01_alpha.sh"));
        assert_eq!(r.category, Some(Category::E2e));
        assert_eq!(
            r.inputs.as_deref(),
            Some(&strings(&["docs/A.md", "lib/alpha.sh", "test/cases/01_alpha.sh"])[..])
        );
        assert_eq!(alpha.status.as_deref(), Some("guaranteed"));
        assert_eq!(alpha.page.as_deref(), Some("/evidence/claims/alpha/"));

        // a claim naming '-' still has its route, with neither test nor path
        let planned = entry(&s, "claim:planned");
        assert_eq!(planned.routes.len(), 1);
        assert!(planned.routes[0].test.is_none() && planned.routes[0].path.is_none());

        // a path no runner drives is named, and has no test
        let mut d = world();
        d.claims = vec![claim("gamma", "advisory", "lib/g.sh", "lib/gamma.sh")];
        let s = derive(&d);
        let r = &entry(&s, "claim:gamma").routes[0];
        assert_eq!(r.path.as_deref(), Some("lib/gamma.sh"));
        assert!(r.test.is_none() && r.category.is_none());
        assert_eq!(
            r.inputs.as_deref(),
            Some(&strings(&["docs/A.md", "lib/g.sh"])[..])
        );
    }

    #[test]
    fn a_rule_path_no_runner_drives_is_a_mechanism() {
        let s = derive(&world());
        let rule = entry(&s, "rule:project.alpha");
        assert_eq!(rule.mechanisms, strings(&["scripts/alpha-gate"]));
        assert_eq!(rule.routes.len(), 1);
        let r = &rule.routes[0];
        assert_eq!(
            (r.via, r.test.as_deref()),
            (Via::Rule, Some("suite:01_alpha"))
        );
        assert_eq!(
            r.inputs.as_deref(),
            Some(
                &strings(&[
                    "test/cases/01_alpha.sh",
                    ".ai/repo/rules/project/alpha.v1.md"
                ])[..]
            )
        );
        assert!(rule.page.is_none(), "a rule has no page of its own");
        assert_eq!(rule.status.as_deref(), Some("active"));
    }

    #[test]
    fn the_first_header_decides() {
        // read from files, as the index reads them
        let dir = tempfile::tempdir().unwrap();
        let cases = dir.path().join(CASES_DIR);
        std::fs::create_dir_all(&cases).unwrap();
        let header = |kind: &str, words: &str| format!("# majordomus-{kind}: {words}\n");
        std::fs::write(
            cases.join("01_first.sh"),
            header("covers", "none") + &header("covers", "version"),
        )
        .unwrap();
        std::fs::write(
            cases.join("02_words.sh"),
            header("covers", "gate:x capability:c.x version") + &header("negative", "none version"),
        )
        .unwrap();
        std::fs::write(cases.join("README.md"), header("covers", "version")).unwrap();
        std::fs::create_dir_all(cases.join("99_dir.sh")).unwrap();
        let read = read_cases(dir.path());
        assert_eq!(
            read.len(),
            2,
            "a file no runner drives and a directory are not cases"
        );
        let first = read.iter().find(|c| c.test == "suite:01_first").unwrap();
        assert!(
            first.covers.is_empty(),
            "`none` then a second line names nothing"
        );
        let words = read.iter().find(|c| c.test == "suite:02_words").unwrap();
        assert_eq!(
            words.covers,
            strings(&["gate:x", "capability:c.x", "version"])
        );
        assert_eq!(
            words.negative,
            strings(&["version"]),
            "`none` names nothing"
        );

        let d = Declarations {
            commands: strings(&["version"]),
            capabilities: vec![cap("c.x", None, None, "c")],
            cases: read,
            ..Declarations::default()
        };
        let s = derive(&d);
        assert_eq!(
            tests_of(entry(&s, "command:version"), Via::Behaviour),
            vec!["suite:02_words"]
        );
        assert_eq!(
            tests_of(entry(&s, "capability:c.x"), Via::Behaviour),
            vec!["suite:02_words"]
        );
        assert!(
            s.get("gate:x").is_err(),
            "a prefixed word that is no kind is ignored"
        );
        assert!(s.subjects.keys().all(|k| !k.contains("gate")));
    }

    #[test]
    fn a_missing_directory_holds_no_tests() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_cases(dir.path()).is_empty());
        assert!(read_crate_tests(dir.path()).is_empty());
        let tests = dir.path().join(CRATE_TESTS_DIR);
        std::fs::create_dir_all(&tests).unwrap();
        std::fs::write(tests.join("why.rs"), "").unwrap();
        std::fs::write(tests.join("notes.txt"), "").unwrap();
        assert_eq!(read_crate_tests(dir.path()), strings(&["crate:why"]));
    }

    #[test]
    fn a_negative_header_is_its_own_route() {
        let s = derive(&world());
        let version = entry(&s, "command:version");
        assert_eq!(tests_of(version, Via::Behaviour), vec!["suite:01_alpha"]);
        assert_eq!(tests_of(version, Via::Negative), vec!["suite:01_alpha"]);
        assert_eq!(
            version.routes.len(),
            2,
            "one behaviour route and one negative route"
        );
        // a negative word naming a capability that is not there routes nowhere
        assert!(s.get("capability:no.such").is_err());
    }

    #[test]
    fn a_documented_command_routes_to_the_examples_only_where_they_run() {
        let s = derive(&world());
        let plan = entry(&s, "command:commit plan");
        let example: Vec<&Route> = plan
            .routes
            .iter()
            .filter(|r| r.via == Via::Example)
            .collect();
        assert_eq!(example.len(), 1);
        assert_eq!(example[0].test.as_deref(), Some("crate:cli_examples"));
        assert_eq!(example[0].path.as_deref(), Some(CLI_EXAMPLES));
        assert_eq!(example[0].category, Some(Category::Integration));
        // a shell command has no example route, whatever runs
        assert!(tests_of(entry(&s, "command:version"), Via::Example).is_empty());

        let mut d = world();
        d.crate_tests.retain(|t| t != "crate:cli_examples");
        let s = derive(&d);
        assert!(tests_of(entry(&s, "command:commit plan"), Via::Example).is_empty());
    }

    #[test]
    fn members_form_a_dag() {
        let s = derive(&world());
        let feature = entry(&s, "feature:alpha");
        assert!(feature.members.contains(&"use_case:see-alpha".to_string()));
        for (key, e) in &s.subjects {
            let starts = |p: &str| e.members.iter().all(|m| m.starts_with(p));
            match e.kind {
                SubjectKind::UseCase => assert!(
                    e.members
                        .iter()
                        .all(|m| !m.starts_with("feature:") && !m.starts_with("use_case:")),
                    "{key}"
                ),
                SubjectKind::Capability => assert!(
                    e.members
                        .iter()
                        .all(|m| m.starts_with("command:") || m.starts_with("claim:")),
                    "{key}"
                ),
                SubjectKind::Mcp => {
                    assert_eq!(e.members.len(), 1, "{key}");
                    assert!(starts("capability:"), "{key}");
                }
                SubjectKind::Claim | SubjectKind::Rule | SubjectKind::Command => {
                    assert!(e.members.is_empty(), "{key}")
                }
                SubjectKind::Feature => assert!(
                    e.members.iter().all(|m| !m.starts_with("feature:")
                        && !m.starts_with("mcp:")
                        && !m.starts_with("capability:")),
                    "{key}"
                ),
            }
        }
        // a reference that resolves to nothing is skipped, never invented
        assert!(!feature.members.contains(&"claim:no-such-claim".to_string()));
        assert!(s.get("claim:no-such-claim").is_err());
    }

    #[test]
    fn a_capability_is_made_of_its_modules_claims() {
        let mut d = world();
        d.claims.push(claim(
            "in-another-module",
            "advisory",
            "apps/majordomus-cli/src/capability/builtin/other.rs",
            "-",
        ));
        d.claims.push(claim(
            "in-the-cli",
            "advisory",
            "apps/majordomus-cli/src/cli.rs",
            "-",
        ));
        let s = derive(&d);
        for key in ["capability:things.list", "capability:things.show_one"] {
            let e = entry(&s, key);
            assert!(e.members.contains(&"claim:in-module".to_string()), "{key}");
            assert!(
                !e.members.contains(&"claim:in-another-module".to_string()),
                "{key}"
            );
            assert!(
                !e.members.contains(&"claim:in-the-cli".to_string()),
                "{key}"
            );
        }
        assert_eq!(
            entry(&s, "capability:things.list").members,
            strings(&["claim:in-module", "command:things list"])
        );
        assert_eq!(
            entry(&s, "capability:other.x").members,
            strings(&["claim:in-another-module"])
        );
    }

    #[test]
    fn a_word_both_programs_answer_to_is_named() {
        let d = Declarations {
            commands: strings(&["bench", "version"]),
            examples: strings(&["bench", "commit plan"]),
            capabilities: vec![cap(
                "capabilities.list",
                Some("capabilities list"),
                None,
                "c",
            )],
            claims: vec![claim("a", "guaranteed", "-", "-")],
            ..Declarations::default()
        };
        let s = derive(&d);
        let bench = entry(&s, "command:bench");
        assert_eq!(bench.programs, vec![Program::Native, Program::Shell]);
        assert_eq!(bench.findings.len(), 1);
        assert_eq!(bench.findings[0].code, COMMAND_IN_TWO_PROGRAMS);
        assert_eq!(bench.findings[0].severity, Severity::Warning);
        assert_eq!(bench.findings[0].subject, "command:bench");
        assert_eq!(entry(&s, "command:version").programs, vec![Program::Shell]);
        for key in ["command:commit plan", "command:capabilities list"] {
            assert_eq!(entry(&s, key).programs, vec![Program::Native], "{key}");
        }
        for (key, e) in &s.subjects {
            if key != "command:bench" {
                assert!(e.findings.is_empty(), "{key}");
            }
            if e.kind != SubjectKind::Command {
                assert!(e.programs.is_empty(), "{key}");
            }
        }
    }

    #[test]
    fn a_use_case_names_what_graph_rs_names() {
        let s = derive(&world());
        let u = entry(&s, "use_case:see-alpha");
        assert_eq!(
            u.members,
            strings(&[
                "claim:alpha",
                "command:version",
                "mcp:majordomus_things",
                "rule:project.alpha"
            ])
        );
        assert_eq!(u.routes.len(), 1);
        let r = &u.routes[0];
        assert_eq!(r.via, Via::Scenario);
        assert_eq!(r.category, Some(Category::Scenario));
        assert_eq!(r.path.as_deref(), Some(".ai/repo/use-cases/see-alpha.md"));
        assert!(r.test.is_none());
        assert_eq!(u.page.as_deref(), Some("/evidence/use-cases/see-alpha/"));
        assert_eq!(u.status.as_deref(), Some("active"));
    }

    #[test]
    fn an_mcp_tool_is_an_alias_of_its_capability() {
        let s = derive(&world());
        let alias = entry(&s, "mcp:majordomus_things");
        assert_eq!(alias.alias_of.as_deref(), Some("capability:things.list"));
        assert_eq!(alias.members, strings(&["capability:things.list"]));
        assert!(alias.page.is_none() && alias.routes.is_empty());
        assert!(s
            .pages()
            .iter()
            .all(|p| p.subject != "mcp:majordomus_things"));

        // over the real registry: the alias is the registry's own answer for the tool
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let s = index(&ctx.index, &ctx.product, &ctx.registry);
        let mut aliases = 0;
        for (key, e) in s
            .subjects
            .iter()
            .filter(|(_, e)| e.kind == SubjectKind::Mcp)
        {
            let by_registry = ctx.registry.by_mcp_tool(&e.id).unwrap();
            assert_eq!(
                e.alias_of,
                Some(format!("capability:{}", by_registry.id)),
                "{key}"
            );
            aliases += 1;
        }
        assert!(aliases > 0, "a check over no alias is not a pass");
    }

    #[test]
    fn the_slug_matches_the_site() {
        assert_eq!(slug("session_domain.identity"), "session-domain-identity");
        assert_eq!(slug("commit plan"), "commit-plan");
        assert_eq!(
            page_of(SubjectKind::Capability, "repository.scope_classify").as_deref(),
            Some("/evidence/capabilities/repository-scope-classify/")
        );
        assert_eq!(page_of(SubjectKind::Rule, "project.x"), None);
    }

    #[test]
    fn a_feature_that_reaches_no_test_is_a_finding() {
        let s = derive(&world());
        let lonely = entry(&s, "feature:lonely");
        assert_eq!(lonely.findings.len(), 1);
        assert_eq!(lonely.findings[0].code, FEATURE_WITHOUT_EVIDENCE);
        assert_eq!(lonely.findings[0].severity, Severity::Error);
        assert_eq!(
            entry(&s, "feature:drafty").findings[0].severity,
            Severity::Warning
        );
        // a feature that reaches a test through a transitive member has none
        assert!(entry(&s, "feature:alpha").findings.is_empty());

        // a planned claim and a scenario route count for nothing
        let mut d = world();
        d.features = vec![FeatureDecl {
            id: "only-a-scenario".into(),
            status: "stable".into(),
            claims: strings(&["planned"]),
            use_cases: strings(&["bare"]),
            ..FeatureDecl::default()
        }];
        d.use_cases.push(UseCaseDecl {
            id: "bare".into(),
            path: ".ai/repo/use-cases/bare.md".into(),
            ..UseCaseDecl::default()
        });
        let s = derive(&d);
        let f = entry(&s, "feature:only-a-scenario");
        assert_eq!(f.findings.len(), 1);
        assert_eq!(f.findings[0].code, FEATURE_WITHOUT_EVIDENCE);
    }

    #[test]
    fn excluded_lists_what_nothing_reaches() {
        let s = derive(&world());
        assert_eq!(s.excluded, strings(&["crate:lonely", "suite:04_orphan"]));
        // reached by a claim (01, 02), a rule (01), a covers header (03, 05) or the examples
        for t in [
            "suite:01_alpha",
            "suite:02_mod",
            "suite:03_bench",
            "suite:05_cap",
        ] {
            assert!(!s.excluded.contains(&t.to_string()), "{t}");
        }
        // a case only a rule reaches is reached
        let mut d = world();
        d.claims.clear();
        d.cases = vec![case("01_alpha", &[], &[])];
        let s = derive(&d);
        assert!(!s.excluded.contains(&"suite:01_alpha".to_string()));
    }

    #[test]
    fn the_derivation_does_not_depend_on_input_order() {
        let d = world();
        let expected = crate::http::openapi::render(&derive(&d).document("1"));
        let permute = |d: &Declarations, f: &dyn Fn(&mut Vec<String>)| {
            let mut p = d.clone();
            p.claims.reverse();
            p.rules.reverse();
            p.features.reverse();
            p.use_cases.reverse();
            p.capabilities.reverse();
            p.cases.reverse();
            f(&mut p.commands);
            f(&mut p.examples);
            f(&mut p.crate_tests);
            for feature in &mut p.features {
                f(&mut feature.claims);
                f(&mut feature.use_cases);
            }
            for u in &mut p.use_cases {
                f(&mut u.commands);
                f(&mut u.mcp_tools);
            }
            p
        };
        let reverse = permute(&d, &|v: &mut Vec<String>| v.reverse());
        let rotate = permute(&d, &|v: &mut Vec<String>| {
            if !v.is_empty() {
                v.rotate_left(1)
            }
        });
        for p in [reverse, rotate] {
            assert_eq!(
                crate::http::openapi::render(&derive(&p).document("1")),
                expected
            );
        }
    }

    #[test]
    fn the_document_carries_no_ledger_content() {
        let doc = derive(&world()).document("1");
        fn keys(v: &Value, out: &mut BTreeSet<String>) {
            match v {
                Value::Object(o) => {
                    for (k, v) in o {
                        out.insert(k.clone());
                        keys(v, out);
                    }
                }
                Value::Array(a) => a.iter().for_each(|v| keys(v, out)),
                _ => {}
            }
        }
        let mut found = BTreeSet::new();
        keys(&doc, &mut found);
        for forbidden in [
            "commit",
            "head",
            "outcome",
            "state",
            "execution",
            "at",
            "digest",
        ] {
            assert!(!found.contains(forbidden), "{forbidden}");
        }
        assert!(
            found.contains("subjects") && found.contains("routes"),
            "the walk saw the document"
        );
        assert!(crate::generate::forbidden_in(&crate::http::openapi::render(&doc)).is_none());
        assert!(PATH.starts_with(crate::generate::SITE_DATA_DIR));
        let order: Vec<&str> = doc
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            order,
            [
                "schema",
                "generated",
                "generator",
                "route",
                "kinds",
                "categories",
                "pages",
                "subjects",
                "excluded"
            ]
        );
        assert_eq!(doc["kinds"][1], json!({ "kind": "rule" }));
        assert_eq!(
            doc["kinds"][6],
            json!({ "kind": "use_case", "section": "use-cases" })
        );
    }

    #[test]
    fn two_subjects_on_one_page_are_found() {
        // `a.b` and `a_b` slug to one page
        let d = Declarations {
            capabilities: vec![cap("a.b", None, None, "a"), cap("a_b", None, None, "a")],
            ..Declarations::default()
        };
        let s = derive(&d);
        assert_eq!(
            s.duplicate_pages(),
            vec![(
                "/evidence/capabilities/a-b/".to_string(),
                strings(&["capability:a.b", "capability:a_b"])
            )]
        );
        let refused = rendered(&s, "1").unwrap_err().to_string();
        assert!(refused.contains("/evidence/capabilities/a-b/"), "{refused}");
        assert!(refused.contains("capability:a_b"), "{refused}");
        assert!(derive(&world()).duplicate_pages().is_empty());
    }

    #[test]
    fn a_leak_is_refused_and_nothing_is_rendered() {
        let marker = format!("{}someone/x.sh", "/Users/");
        let d = Declarations {
            claims: vec![claim("leaky", "advisory", "-", &marker)],
            ..Declarations::default()
        };
        let refused = rendered(&derive(&d), "1").unwrap_err().to_string();
        assert!(
            refused.contains(PATH) && refused.contains("/Users/"),
            "{refused}"
        );

        let a = rendered(&derive(&world()), "1").unwrap();
        assert_eq!((a.path.as_str(), a.document.as_str()), (PATH, DOCUMENT));
        assert!(a.content.contains("\"feature:alpha\""));
    }

    #[test]
    fn reach_is_the_walk_the_finding_uses() {
        let s = derive(&world());
        let reached = s.reach("feature:alpha").unwrap();
        let shown: Vec<(Via, Option<&str>)> =
            reached.iter().map(|r| (r.via, r.test.as_deref())).collect();
        // members in byte order: claim:alpha, claim:planned, command:version,
        // rule:project.alpha, use_case:see-alpha — whose own members were walked already,
        // except its MCP alias and that alias's capability
        assert_eq!(
            shown,
            vec![
                (Via::Claim, Some("suite:01_alpha")),
                (Via::Claim, None),
                (Via::Behaviour, Some("suite:01_alpha")),
                (Via::Negative, Some("suite:01_alpha")),
                (Via::Rule, Some("suite:01_alpha")),
                (Via::Scenario, None),
                (Via::Behaviour, Some("suite:05_cap")),
                (Via::Claim, Some("suite:02_mod")),
            ]
        );
        // each route once, although the claim is reached directly and through the use case
        let unique: BTreeSet<&&Route> = reached.iter().collect();
        assert_eq!(unique.len(), reached.len());
        assert!(s.reach("feature:no-such").is_err());
        // the finding is exactly a feature whose reach has no route with a test
        for (key, e) in s
            .subjects
            .iter()
            .filter(|(_, e)| e.kind == SubjectKind::Feature)
        {
            let none = s.reach(key).unwrap().iter().all(|r| r.test.is_none());
            let found = e
                .findings
                .iter()
                .any(|f| f.code == FEATURE_WITHOUT_EVIDENCE);
            assert_eq!(none, found, "{key}");
        }
    }

    #[test]
    fn an_unknown_key_is_refused_by_name() {
        let s = derive(&world());
        for key in ["feature:no-such", "gate:x", "nonsense"] {
            let refused = s.get(key).unwrap_err();
            assert_eq!(refused.key, key);
            let said = refused.to_string();
            assert!(
                said.starts_with(&format!("`{key}` is not a subject of this repository: ")),
                "{said}"
            );
            assert!(said.ends_with("MCP tool or use case"), "{said}");
            let as_error: &dyn std::error::Error = &refused;
            assert_eq!(as_error.to_string(), said);
        }
    }

    #[test]
    fn a_subject_declared_twice_is_one_subject_whatever_the_order() {
        // a claim id written twice, and one MCP tool two capabilities declare: neither is
        // sound, both are reported by their owners, and the index must not depend on which
        // declaration came first
        let twice = |first: &str, second: &str| Declarations {
            claims: vec![claim("a", first, "-", "-"), claim("a", second, "-", "-")],
            capabilities: vec![
                cap("x.one", None, Some("majordomus_x"), "x"),
                cap("x.two", None, Some("majordomus_x"), "x"),
            ],
            ..Declarations::default()
        };
        let forward = derive(&twice("advisory", "guaranteed"));
        let mut backward = twice("guaranteed", "advisory");
        backward.capabilities.reverse();
        assert_eq!(derive(&backward), forward);

        let once = entry(&forward, "claim:a");
        assert_eq!(
            once.status.as_deref(),
            Some("advisory"),
            "the smaller status"
        );
        assert_eq!(
            once.routes.len(),
            1,
            "one route, however often it is declared"
        );
        let alias = entry(&forward, "mcp:majordomus_x");
        assert_eq!(alias.alias_of.as_deref(), Some("capability:x.one"));
        assert_eq!(
            alias.members,
            strings(&["capability:x.one", "capability:x.two"])
        );
    }

    /// An object as the index holds it, for a kind the synthetic repository does not
    /// declare.
    fn object(kind: &str, identity: &str, path: &str, metadata: Value) -> crate::Object {
        crate::Object {
            kind: kind.into(),
            identity: identity.into(),
            uri: format!("majordomus://{kind}/{identity}"),
            title: None,
            description: None,
            metadata,
            body: String::new(),
            content: String::new(),
            media_type: "text/markdown",
            provenance: crate::model::Provenance {
                path: path.into(),
                directory: path
                    .rsplit_once('/')
                    .map(|(dir, _)| dir.to_string())
                    .unwrap_or_default(),
                source_class: kind.into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    #[test]
    fn the_declarations_are_read_from_their_owners() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let root = repo.root().to_path_buf();
        std::fs::create_dir_all(root.join(CASES_DIR)).unwrap();
        std::fs::write(
            root.join(CASES_DIR).join("01_alpha.sh"),
            format!("{COVERS} version\n{NEGATIVE} none\n"),
        )
        .unwrap();
        std::fs::create_dir_all(root.join(CRATE_TESTS_DIR)).unwrap();
        std::fs::write(root.join(CLI_EXAMPLES), "").unwrap();

        let mut index = repo.index().unwrap();
        index.objects.extend([
            object(
                "claim",
                "alpha",
                "docs/CLAIMS.yaml",
                json!({
                    "claim": "Alpha holds",
                    "status": "guaranteed",
                    "source": "docs/A.md",
                    "implementation": "lib/alpha.sh",
                    "test": "test/cases/01_alpha.sh",
                }),
            ),
            object(
                "feature",
                "alpha",
                ".ai/repo/features/alpha.md",
                json!({
                    "id": "alpha",
                    "title": "Alpha",
                    "headline": "Alpha holds.",
                    "summary": "A feature of the test.",
                    "status": "stable",
                    "claims": ["alpha"],
                    "rules": ["project.rule-0"],
                    "commands": ["version"],
                    "use_cases": ["see-alpha"],
                }),
            ),
            object(
                "use-case",
                "see-alpha",
                ".ai/repo/use-cases/see-alpha.md",
                json!({
                    "status": "active",
                    "commands": ["version"],
                    "doctrines": ["project.rule-0"],
                    "claims": ["alpha"],
                    "mcp_tools": "majordomus_capabilities",
                }),
            ),
            object("use-case", "bare", ".ai/repo/use-cases/bare.md", json!({})),
            object(
                "command",
                "version",
                "share/commands.yaml",
                json!({ "visibility": "public" }),
            ),
            object(
                "command",
                "hook",
                "share/commands.yaml",
                json!({ "visibility": "internal" }),
            ),
            object("command", "unstated", "share/commands.yaml", json!({})),
        ]);
        let registry = CapabilityRegistry::builder()
            .with_modules(crate::capability::builtin::modules())
            .with_index(&index)
            .build()
            .unwrap();
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        let d = declarations(&ctx.index, &ctx.product, &ctx.registry);

        // a public command of the registry only: an internal one is dispatched and unlisted
        assert_eq!(d.commands, strings(&["version"]));

        let alpha = d.claims.iter().find(|c| c.id == "alpha").unwrap();
        assert_eq!(alpha.status, "guaranteed");
        assert_eq!(alpha.source.as_deref(), Some("docs/A.md"));
        assert_eq!(alpha.implementation.as_deref(), Some("lib/alpha.sh"));
        assert_eq!(alpha.test_path.as_deref(), Some("test/cases/01_alpha.sh"));
        assert_eq!(alpha.test.as_deref(), Some("suite:01_alpha"));

        let rule = d.rules.iter().find(|r| r.id == "project.rule-0").unwrap();
        assert_eq!(rule.status, "active");

        let feature = d
            .features
            .iter()
            .find(|f| f.id == "alpha")
            .expect("the product model reads the feature");
        assert_eq!(feature.status, "stable");
        assert_eq!(feature.claims, strings(&["alpha"]));
        assert_eq!(feature.rules, strings(&["project.rule-0"]));
        assert_eq!(feature.commands, strings(&["version"]));
        assert_eq!(feature.use_cases, strings(&["see-alpha"]));

        let see = d.use_cases.iter().find(|u| u.id == "see-alpha").unwrap();
        assert_eq!(see.status.as_deref(), Some("active"));
        assert_eq!(see.path, ".ai/repo/use-cases/see-alpha.md");
        assert_eq!(see.commands, strings(&["version"]));
        assert_eq!(see.doctrines, strings(&["project.rule-0"]));
        assert_eq!(see.claims, strings(&["alpha"]));
        assert_eq!(
            see.mcp_tools,
            strings(&["majordomus_capabilities"]),
            "a lone string is a list of one"
        );
        let bare = d.use_cases.iter().find(|u| u.id == "bare").unwrap();
        assert!(bare.status.is_none() && bare.commands.is_empty() && bare.mcp_tools.is_empty());

        // the documented examples, each non-empty, and the builtin capabilities with the
        // file each module is composed in
        let documented = crate::cli::EXAMPLES
            .iter()
            .filter(|e| !e.command.is_empty())
            .count();
        assert_eq!(d.examples.len(), documented);
        assert!(d.examples.iter().all(|e| !e.is_empty()));
        let listing = d
            .capabilities
            .iter()
            .find(|c| c.mcp_tool.as_deref() == Some("majordomus_capabilities"))
            .unwrap();
        assert_eq!(listing.cli.as_deref(), Some("capabilities list"));
        assert!(
            listing
                .source
                .starts_with("apps/majordomus-cli/src/capability/builtin/")
                && listing.source.ends_with(".rs"),
            "{}",
            listing.source
        );

        assert_eq!(d.cases.len(), 1);
        assert_eq!(d.cases[0].test, "suite:01_alpha");
        assert_eq!(d.cases[0].covers, strings(&["version"]));
        assert!(d.cases[0].negative.is_empty());
        assert_eq!(d.crate_tests, strings(&["crate:cli_examples"]));

        // and the committed index of that repository carries what was read
        let a = artifact(&ctx, "1").unwrap();
        assert!(a.content.contains("\"use_case:see-alpha\""));
        assert!(a.content.contains("\"command:version\""));
        assert!(!a.content.contains("\"command:hook\""));
        assert!(!a.content.contains("\"command:unstated\""));
    }
}
