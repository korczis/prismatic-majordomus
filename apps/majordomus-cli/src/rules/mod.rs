//! The rules module: a rule, and everything the repository can show that proves it.
//!
//! A rule of this repository is a Markdown object under the rules section with YAML front
//! matter: an id, a version, a class (`blocking` or `advisory`), a statement, and — since
//! the proof relation became load-bearing — an `x-majordomus` block naming what enforces
//! it. This module is the one typed reading of that object, and the one place the
//! repository decides what a rule's proof amounts to.
//!
//! # Why this exists rather than another list
//!
//! Before it, "is this rule enforced?" was answered by `o.metadata.get("x-majordomus")
//! .is_some()` — a green badge for the presence of a YAML key. A rule could name
//! `test/cases/28_no_hardcoded_values.sh` after that case was deleted and still read as
//! enforced on every surface that asked. The proof graph here answers the question the
//! badge pretended to: the named validator is in the tree, the named cases are in the tree,
//! a runner owns them, a run of them was recorded, and that run is not older than what it
//! is about.
//!
//! # What it does not do
//!
//! It does not re-implement evidence. [`crate::evidence`] already binds a test path to the
//! executions recorded against it and already decides, with [`ProofState`], what a passing
//! run from an older commit is worth. A rule's test-level proof *is* that vocabulary; this
//! module joins rules to it rather than inventing a second one. Nor does it dispatch: the
//! portable shell layer's `lib/doctrine.sh` runs the validators, and `scripts/ci/
//! rule-proof-check` is the gate. This is the reading all of those project from.
//!
//! # The two enforcement modes
//!
//! The canonical front matter allows exactly two shapes, and the distinction is not
//! cosmetic:
//!
//! * **dispatched** — the block names a `validator`, which `lib/doctrine.sh` calls at run
//!   time. It carries `category`, `exit_code` and `enforced_by` with it; a half-declaration
//!   (a category without a validator) is a defect the loader refuses rather than a rule
//!   with a missing field.
//! * **gated** — the block names `tests` and no validator. Nothing dispatches it; the
//!   behavioural cases are what prove it, and a CI gate is what runs them.
//!
//! Either mode names at least one test. A blocking rule that names neither is `Unproven`,
//! which is the state this module exists to make visible rather than absent.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::evidence::{Execution, Ledger, ProofState, TestId};
use crate::index::Index;

/// The URI under which the whole rule verification report is read as an MCP resource.
pub const RULES_URI: &str = "majordomus://rules";

/// Where the CI model of this repository is declared. Read to answer "which gate runs the
/// case this rule names", so that a rule's gate coverage is derived from the planner's own
/// input rather than restated here.
pub const GATES_PATH: &str = ".ai/repo/ci/gates.yaml";

/// Where the behavioural cases live, and the runner that drives all of them at once. A gate
/// whose command names that runner runs every case under that directory, which is how a
/// case gets from a rule's front matter to a gate without either naming the other.
const CASES_DIR: &str = "test/cases/";
const SUITE_RUNNER: &str = "test/run.sh";
/// The same, for the crate's own integration tests.
const CRATE_TESTS_DIR: &str = "apps/majordomus-cli/tests/";
const CRATE_RUNNER: &str = "rust-check";

// ---------------------------------------------------------------- the canonical definition

/// What a rule declares about how strongly it binds.
///
/// The repository has exactly two, and they are not severities on a scale: `blocking` means
/// a gate refuses the work, `advisory` means a reader is expected to have read it. A rule
/// whose front matter says neither is reported as [`Class::Unknown`] rather than defaulted,
/// because guessing here is how a blocking rule quietly becomes advice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "RuleClass")]
pub enum Class {
    /// A gate refuses work that violates it.
    Blocking,
    /// Normative for whoever reads it; nothing refuses a violation.
    Advisory,
    /// The front matter declared no class, or one this repository does not know.
    Unknown,
}

impl Class {
    /// The word a surface prints.
    pub fn label(self) -> &'static str {
        match self {
            Class::Blocking => "blocking",
            Class::Advisory => "advisory",
            Class::Unknown => "unknown",
        }
    }

    /// Read a class from the front matter, without defaulting.
    pub fn parse(word: &str) -> Class {
        match word.trim() {
            "blocking" => Class::Blocking,
            "advisory" => Class::Advisory,
            _ => Class::Unknown,
        }
    }
}

/// How a rule is enforced, as its own front matter declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "RuleMode")]
pub enum Mode {
    /// Names a validator the doctrine dispatcher calls at run time.
    Dispatched,
    /// Names behavioural cases and no validator; a gate runs them.
    Gated,
    /// Carries a reason why nothing executable can express it, and so declares that a
    /// reader enforces it. An honest declaration, not an absence: the reason *is* the
    /// declaration — there is no flag beside it, because a flag can be set and a reason
    /// cannot be set without writing one — it is counted apart from every executable mode,
    /// and it separates a rule nobody could automate from a rule nobody got round to.
    Reviewed,
    /// Names neither. Correct for a rule nothing executable can express; a defect for a
    /// blocking one.
    Declarative,
}

impl Mode {
    /// The word a surface prints.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Dispatched => "dispatched",
            Mode::Gated => "gated",
            Mode::Reviewed => "reviewed",
            Mode::Declarative => "declarative",
        }
    }
}

/// The enforcement block of a rule, read from `x-majordomus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RuleEnforcement")]
pub struct Enforcement {
    /// Which of the two canonical modes, or neither.
    pub mode: Mode,
    /// The validator the dispatcher calls, when the mode is dispatched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator: Option<String>,
    /// The diagnostic category the validator reports under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// The exit code the validator uses to refuse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    /// Which boundary enforces it: the word the rule's own block uses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforced_by: Option<String>,
    /// Every test the block names, in the order it names them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<String>,
    /// Why nothing executable can express this rule, when the mode is reviewed. Required
    /// there: a declared exemption with no reason is the debt it exists to replace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_because: Option<String>,
}

/// One rule, as the repository canonically declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleDefinition {
    /// The rule id, `<namespace>.<stem>`, without the version.
    pub id: String,
    /// The identity the index holds, with the version.
    pub identity: String,
    /// `majordomus://rule/<identity>`.
    pub uri: String,
    /// The declared version.
    pub version: u64,
    /// The title.
    pub title: String,
    /// The one-line description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The normative sentence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement: Option<String>,
    /// `active`, or whatever else the front matter declares.
    pub status: String,
    /// How strongly it binds.
    pub class: Class,
    /// Whether it came with the vendored package or belongs to this project.
    pub namespace: String,
    /// The rules it depends on, by id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// The declared tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Repository-relative path of the rule object.
    pub path: String,
    /// How it is enforced, as declared.
    pub enforcement: Enforcement,
}

/// A rule is ordered by its namespace, then by what a reader reads, then by its id — so
/// that the vendored package and the project's own rules stay in their blocks and the
/// sequence is total whatever order discovery handed them.
impl crate::order::Ordered for RuleDefinition {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::grouped(&self.namespace, &self.id, &self.identity)
    }
}

/// A proof is ordered by the rule it is about: one comparator, so a filtered report and the
/// whole report cannot disagree about the sequence.
impl crate::order::Ordered for RuleProof {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::Ordered::order_key(&self.rule)
    }
}

// ---------------------------------------------------------------- the proof graph

/// The validator a dispatched rule names, and whether anything defines it.
///
/// A validator is **not a path**. `validator: adr` names the shell function
/// `mj_validate_adr`, which the doctrine dispatcher calls; the tool finds it by scanning
/// `lib/`, exactly as `majordomus doctor` does. Reading it as a path is a mistake worth
/// recording here, because it is the one this type was introduced to fix: it makes every
/// dispatched rule in the vendored package report as naming something that is not in the
/// tree, which is the opposite of true — those are the best-enforced rules there are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RuleValidator")]
pub struct ValidatorRef {
    /// The name the rule declares, without the `mj_validate_` prefix.
    pub name: String,
    /// The function the dispatcher calls.
    pub function: String,
    /// The file that defines it, when one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defined_in: Option<String>,
    /// Whether anything in `lib/` defines it.
    pub present: bool,
}

/// What a path a rule names actually is, decided by what drives it rather than by where it
/// sits.
///
/// The corpus names two kinds and this distinction is the difference between a verdict and
/// a mechanism. A **case** is driven by a runner, so an execution of it can be recorded and
/// the repository can show that it passed. A **gate** is an executable check a CI gate runs
/// as its own command: it refuses violations on every run, and this repository records no
/// verdict for it, so what can be shown is the mechanism and not the result. Anything else
/// a rule names is proof only in prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "RuleArtifactKind")]
pub enum ArtifactKind {
    /// A behavioural case a runner drives; executions of it are recorded.
    Case,
    /// An executable check a CI gate runs as its own command.
    Gate,
    /// Neither. Nothing can run it, so nothing can ever record it.
    Unknown,
}

impl ArtifactKind {
    /// The word a surface prints.
    pub fn label(self) -> &'static str {
        match self {
            ArtifactKind::Case => "case",
            ArtifactKind::Gate => "gate",
            ArtifactKind::Unknown => "unknown",
        }
    }
}

/// One test a rule names, joined to what was recorded against it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RuleTestProof")]
pub struct TestProof {
    /// The path the rule names.
    pub path: String,
    /// What drives it: a runner, a CI gate, or nothing.
    pub kind: ArtifactKind,
    /// Whether that path is in the tree.
    pub present: bool,
    /// The canonical test identity (`suite:<case>`, `crate:<binary>`), when a runner owns
    /// the path. Absent means no runner drives it, so no run of it can ever be recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    /// What the repository can say about this test's latest run.
    pub state: ProofState,
    /// That state, in one sentence, from the one place it is worded.
    pub meaning: String,
    /// The execution behind the state, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<Execution>,
    /// The command that runs this test again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reproduce: Option<String>,
    /// The CI gates that run this test, derived from the repository's own CI model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<String>,
}

/// What the repository can say about one rule's proof, as a whole.
///
/// Ordered strongest to weakest so that a summary sorted by this reads as a ranking, and so
/// that a rule's state is the weakest of its parts by `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuleState {
    /// Every artifact it names is present, and every test it names has a passing run that
    /// nothing has invalidated.
    Proven,
    /// Every artifact is present and every test passed, but at least one run is only
    /// un-invalidated rather than measured against this tree.
    InputsUnchanged,
    /// A passing run exists, but something it is about has changed since.
    Stale,
    /// An executable gate runs what this rule names, on every CI run that selects it. The
    /// mechanism is in the tree and wired; no verdict for it is recorded here.
    Gated,
    /// The latest run of a test this rule names did not pass.
    Failing,
    /// Every artifact is present, and no run of at least one named test was ever recorded.
    NotRun,
    /// The rule declares, with a stated reason, that nothing executable can express it. An
    /// honest exemption: counted apart from every proven rule, never counted as one.
    Reviewed,
    /// The rule names a path no runner in this repository drives.
    Unrunnable,
    /// The rule names a path that is not in the tree. A proof that does not exist is worse
    /// than no proof, because the rule reads as proven.
    Dangling,
    /// The rule names neither a validator nor a test. Correct for a rule nothing executable
    /// can express; a defect for a blocking one.
    Unproven,
}

impl RuleState {
    /// The word a surface prints.
    pub fn label(self) -> &'static str {
        match self {
            RuleState::Proven => "proven",
            RuleState::InputsUnchanged => "inputs unchanged",
            RuleState::Stale => "stale",
            RuleState::Gated => "gated",
            RuleState::Failing => "failing",
            RuleState::NotRun => "not run",
            RuleState::Reviewed => "reviewed",
            RuleState::Unrunnable => "unrunnable",
            RuleState::Dangling => "dangling",
            RuleState::Unproven => "unproven",
        }
    }

    /// One sentence: what the state means, worded once so that every surface says the same
    /// thing rather than each inventing its own gloss.
    pub fn meaning(self) -> &'static str {
        match self {
            RuleState::Proven => {
                "Everything this rule names is in the tree, and every test it names has a passing \
                 run that nothing in the repository has changed since."
            }
            RuleState::InputsUnchanged => {
                "Everything this rule names is in the tree and passed, but at least one run is \
                 only un-invalidated: nothing it names has changed since, which is the absence of \
                 a known invalidation rather than proof against this commit."
            }
            RuleState::Stale => {
                "A passing run exists, but a file it is about has changed since. The proof is \
                 older than its subject."
            }
            RuleState::Gated => {
                "An executable check refuses violations of this rule, and a CI gate runs it. \
                 That is the mechanism; this repository records no verdict for a gate, so what \
                 can be shown is that violations are refused and not that the last run passed."
            }
            RuleState::Failing => {
                "The most recent recorded run of a test this rule names did not pass."
            }
            RuleState::NotRun => {
                "This rule names a test a runner owns, and no run of it has ever been recorded."
            }
            RuleState::Reviewed => {
                "This rule declares that nothing executable can express it and gives its reason. \
                 A reader enforces it. That is a weaker guarantee than any gate, and it is \
                 counted apart from every rule that has one."
            }
            RuleState::Unrunnable => {
                "This rule names a path no runner in this repository drives, so no execution of \
                 it can ever be recorded."
            }
            RuleState::Dangling => {
                "This rule names a path that is not in the tree. It reads as proven on every \
                 surface that checks only the name."
            }
            RuleState::Unproven => "This rule names neither a validator nor a test.",
        }
    }

    /// Does this state carry a passing execution behind it, of any freshness?
    pub fn passing(self) -> bool {
        matches!(
            self,
            RuleState::Proven | RuleState::InputsUnchanged | RuleState::Stale
        )
    }
}

/// One rule, joined to everything the repository can show about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleProof {
    /// The canonical definition.
    pub rule: RuleDefinition,
    /// The validator, when the mode is dispatched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator: Option<ValidatorRef>,
    /// Every test the rule names, joined to the ledger.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<TestProof>,
    /// Every CI gate that runs something this rule names, deduplicated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<String>,
    /// The rules that depend on this one, by id — the converse of `depends_on`, which no
    /// rule declares and which a reader always wants.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_by: Vec<String>,
    /// What the repository can say, as a whole.
    pub state: RuleState,
    /// That state, in one sentence.
    pub meaning: String,
    /// Whether the state is one this rule's class can live with.
    pub satisfied: bool,
}

/// One rule whose declared class the proof does not support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RuleFinding")]
pub struct Finding {
    /// The rule id.
    pub rule: String,
    /// The class it declares.
    pub class: Class,
    /// The state its proof is in.
    pub state: RuleState,
    /// Why that combination is a finding.
    pub reason: String,
    /// The command that shows it again.
    pub reproduce: String,
}

/// How much of the rule corpus carries each part of a proof.
///
/// Every number here is counted from the tree. Nothing in this struct may be written down
/// anywhere else: a hardcoded rule count is the defect this repository keeps rediscovering.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RuleCoverage")]
pub struct Coverage {
    /// Rules discovered.
    pub rules: usize,
    /// Of those, blocking.
    pub blocking: usize,
    /// Of those, advisory.
    pub advisory: usize,
    /// Rules naming a validator or at least one test.
    pub named_proof: usize,
    /// Rules every one of whose named paths is in the tree.
    pub artifacts_present: usize,
    /// Rules at least one of whose tests a runner owns.
    pub runnable: usize,
    /// Rules every one of whose tests has a recorded run.
    pub recorded: usize,
    /// Rules whose recorded runs all passed.
    pub passing: usize,
    /// Rules some CI gate runs.
    pub gated: usize,
    /// Rules whose only executable proof is a gate: a mechanism that refuses violations,
    /// with no recorded verdict behind it. Counted apart from `passing` so that the two
    /// are never read as one number.
    pub mechanism_only: usize,
    /// Rules that declare, with a reason, that a reader enforces them. Counted apart from
    /// every executable mode: this number going up is governance getting weaker, and a
    /// summary that folded it into a total would hide exactly that.
    pub review_only: usize,
    /// Rules whose state their class can live with.
    pub satisfied: usize,
}

/// The whole rule corpus, verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RulesReport")]
pub struct RulesReport {
    /// The commit the report was derived against, when git could say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// `clean`, `dirty` or `unknown` at the time of derivation.
    pub working_tree: String,
    /// Every rule, in canonical id order.
    pub rules: Vec<RuleProof>,
    /// How many rules are in each state, by the state's printed word.
    pub states: BTreeMap<String, usize>,
    /// The parts of a proof, counted.
    pub coverage: Coverage,
    /// Every rule whose declared class the proof does not support.
    pub findings: Vec<Finding>,
    /// Whether there are no findings.
    pub satisfied: bool,
}

impl RulesReport {
    /// Does the proof support every rule that declares it blocks?
    pub fn satisfied(&self) -> bool {
        self.findings.is_empty()
    }
}

// ---------------------------------------------------------------- the derivation

/// Read a string field of the front matter, treating an empty value as absence.
fn field(meta: &Value, key: &str) -> Option<String> {
    let v = meta.get(key)?;
    let s = match v {
        Value::String(s) => s.trim().trim_matches('\'').to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => return None,
    };
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Read a list-of-strings field, accepting both the YAML flow list the rules use and a
/// single scalar, which a hand-written rule occasionally is.
fn list(meta: &Value, key: &str) -> Vec<String> {
    match meta.get(key) {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
        _ => Vec::new(),
    }
}

/// Read the enforcement block, deciding the mode from what it actually names rather than
/// from a word it could also declare. A block that names a validator is dispatched; one
/// that names only tests is gated; one that names neither is declarative, whatever else it
/// carries.
fn enforcement_of(meta: &Value) -> Enforcement {
    let block = meta.get("x-majordomus");
    let Some(block) = block else {
        return Enforcement {
            mode: Mode::Declarative,
            validator: None,
            category: None,
            exit_code: None,
            enforced_by: None,
            tests: Vec::new(),
            reviewed_because: None,
        };
    };
    let validator = field(block, "validator");
    let tests = list(block, "tests");
    let enforced_by = field(block, "enforced_by");
    let reviewed_because = field(block, "reviewed_because");
    // The order matters. An executable proof wins over a declaration of review, so a rule
    // that acquires a case stops being review-enforced without anyone remembering to
    // delete the declaration. And a review declaration with no reason is not a mode: it
    // falls through to declarative, which is the finding it was meant to replace.
    let mode = if validator.is_some() {
        Mode::Dispatched
    } else if !tests.is_empty() {
        Mode::Gated
    } else if reviewed_because.is_some() {
        Mode::Reviewed
    } else {
        Mode::Declarative
    };
    Enforcement {
        mode,
        validator,
        category: field(block, "category"),
        exit_code: block.get("exit_code").and_then(Value::as_i64),
        enforced_by,
        tests,
        reviewed_because,
    }
}

/// Every rule the index holds, as a canonical definition.
///
/// The denominator is read from the index, so a rule added tomorrow is measured tomorrow
/// and this function never carries a list of what exists.
pub fn definitions(index: &Index) -> Vec<RuleDefinition> {
    let mut out: Vec<RuleDefinition> = index
        .objects
        .iter()
        .filter(|o| o.kind == "rule")
        .map(|o| {
            let id = field(&o.metadata, "id").unwrap_or_else(|| o.identity.clone());
            let namespace = id.split('.').next().unwrap_or_default().to_string();
            RuleDefinition {
                version: o
                    .metadata
                    .get("version")
                    .and_then(Value::as_u64)
                    .unwrap_or(1),
                title: o.title.clone().unwrap_or_else(|| id.clone()),
                description: o.description.clone(),
                statement: field(&o.metadata, "statement"),
                status: field(&o.metadata, "status").unwrap_or_else(|| "unknown".into()),
                class: Class::parse(&field(&o.metadata, "class").unwrap_or_default()),
                namespace,
                depends_on: list(&o.metadata, "depends_on"),
                tags: list(&o.metadata, "tags"),
                path: o.provenance.path.clone(),
                enforcement: enforcement_of(&o.metadata),
                identity: o.identity.clone(),
                uri: o.uri.clone(),
                id,
            }
        })
        .collect();
    // canonical order, from the one comparator the crate has: two runs of this over the
    // same tree, whatever order discovery handed it, produce the same report
    crate::order::canonical(&mut out);
    out
}

/// Which gates this repository declares, and the command each runs.
///
/// `gates.yaml` is the CI model: `scripts/ci-plan` reads it and nothing else to decide what
/// a change must run. What a rule needs from it is narrower — which gate would run the
/// thing this rule names — and that is answerable from the one line each gate states
/// directly, its `runs:`. No gate id is written down here: a gate renamed in that file is
/// renamed in this answer, which is the whole reason to read it rather than to restate it.
fn gate_commands(root: &Path) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(root.join(GATES_PATH)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut gate: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("- id: ") {
            gate = Some(rest.trim().to_string());
        } else if let Some(rest) = t.strip_prefix("runs: ") {
            if let Some(g) = gate.take() {
                out.push((g, rest.trim().to_string()));
            }
        }
    }
    out
}

/// The gates that run one path the rule names.
///
/// Three bindings, each read from the gate's own command rather than assumed:
/// the gate whose command *is* that path (a validator wired as its own gate); the gate that
/// drives the behavioural suite, for a path under the cases directory; and the gate that
/// drives the crate's tests, for a path under the crate's test directory. A path none of
/// these resolves gets no gate, which is reported rather than guessed — an over-claimed
/// gate is the same defect as an over-claimed test.
fn gates_for(path: &str, commands: &[(String, String)]) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for (gate, runs) in commands {
        let names_path = runs.split_whitespace().any(|w| w == path);
        let drives_suite = path.starts_with(CASES_DIR) && runs.contains(SUITE_RUNNER);
        let drives_crate = path.starts_with(CRATE_TESTS_DIR) && runs.contains(CRATE_RUNNER);
        if names_path || drives_suite || drives_crate {
            out.insert(gate.clone());
        }
    }
    out.into_iter().collect()
}

/// Where `lib/` defines `mj_validate_<name>`, if anywhere.
///
/// The same scan `majordomus doctor` does, and the same one `scripts/generate-site-data`
/// does before it will build a doctrine page: a validator is a shell function, and the
/// question "is it there" is answered by reading the library rather than the filesystem.
fn validator_defined_in(root: &Path, name: &str) -> Option<String> {
    let needle = format!("mj_validate_{name}()");
    let dir = root.join("lib");
    let mut hits: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "sh"))
        .filter(|e| {
            std::fs::read_to_string(e.path())
                .map(|t| t.lines().any(|l| l.trim_start().starts_with(&needle)))
                .unwrap_or(false)
        })
        .map(|e| format!("lib/{}", e.file_name().to_string_lossy()))
        .collect();
    hits.sort();
    hits.into_iter().next()
}

/// Does this gate run exactly this path as its own command, as opposed to driving a whole
/// directory of cases? The distinction is what separates a check wired as a gate from a
/// case that happens to be swept up by the suite runner.
fn gate_runs_exactly(gate: &str, path: &str, commands: &[(String, String)]) -> bool {
    commands
        .iter()
        .any(|(g, runs)| g == gate && runs.split_whitespace().any(|w| w == path))
}

/// Every path that differs between `commit` and the working tree.
///
/// `None` when git could not answer, which the caller must not read as "nothing changed".
fn changed_since(root: &Path, commit: &str) -> Option<BTreeSet<String>> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["diff", "--name-only", commit, "--"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
    )
}

/// Map one test's [`ProofState`] onto the rule vocabulary. The two agree everywhere they
/// overlap; `Dangling` and `Unproven` are the rule-level states evidence has no word for,
/// because a claim naming a missing file is already `Unrunnable` to it.
fn rule_state_of(test: &TestProof) -> RuleState {
    if !test.present {
        return RuleState::Dangling;
    }
    // A gate is a mechanism, not a verdict: it is not joined to the ledger, so the
    // evidence vocabulary has nothing to say about it and reading its `Unrunnable` as a
    // defect would be wrong in the opposite direction from a fake green.
    if test.kind == ArtifactKind::Gate {
        return RuleState::Gated;
    }
    match test.state {
        ProofState::Proven => RuleState::Proven,
        ProofState::InputsUnchanged => RuleState::InputsUnchanged,
        ProofState::Stale => RuleState::Stale,
        ProofState::Failing => RuleState::Failing,
        ProofState::NotRun => RuleState::NotRun,
        ProofState::Unrunnable => RuleState::Unrunnable,
        ProofState::NoTest => RuleState::Unproven,
    }
}

/// Why a rule's state does not support the class it declares, or `None` when it does.
///
/// A blocking rule is the subject: it claims a gate refuses work that violates it, and
/// every state but a passing one makes that claim false. An advisory rule claims nothing
/// executable, so only a name that does not resolve is a finding — a dangling path is a
/// defect at any class, because it reads as proof and is not.
fn unsupported(class: Class, state: RuleState) -> Option<String> {
    match (class, state) {
        (_, RuleState::Dangling) => Some(
            "it names a path that is not in the tree, so it reads as proven and is not".into(),
        ),
        // Reviewed is deliberately not a finding: the rule says in its own front matter
        // that nothing executable can express it and why, which is a declaration a reader
        // can audit. What it is not is proof, and the report counts it apart.
        (Class::Blocking, RuleState::Unproven) => Some(
            "it is blocking and names neither a validator nor a test that proves it".into(),
        ),
        (Class::Blocking, RuleState::Failing) => {
            Some("it is blocking and the latest run of what proves it did not pass".into())
        }
        // Gated is deliberately not a finding: an executable check refuses violations, which
        // is exactly what a blocking rule claims. What it does not have is a recorded
        // verdict, which the state itself says and the report counts separately.
        (Class::Blocking, RuleState::Unrunnable) => Some(
            "it is blocking and names a path no runner drives, so nothing can ever record it"
                .into(),
        ),
        _ => None,
    }
}

/// Join every rule of the index with the tree and the ledger, and decide, per rule, what
/// the repository can honestly say.
///
/// The git comparison is done once per distinct recorded commit and shared by every rule
/// that recorded against it, as [`crate::evidence::report`] does — the two derivations read
/// the same ledger and must not disagree about what a commit's diff is.
pub fn report(index: &Index, ledger: &Ledger) -> RulesReport {
    let root = PathBuf::from(&index.repository.root);
    let git = crate::git::inspect(&root);
    let (head, working_tree) = match &git {
        crate::git::GitState::Available(i) => (i.head.clone(), i.working_tree.clone()),
        crate::git::GitState::Unavailable { .. } => (None, "unknown".to_string()),
    };

    let mut diffs: BTreeMap<String, Option<BTreeSet<String>>> = BTreeMap::new();
    for e in &ledger.executions {
        diffs
            .entry(e.commit.clone())
            .or_insert_with(|| changed_since(&root, &e.commit));
    }
    let gate_commands = gate_commands(&root);

    let defs = definitions(index);
    // the converse of depends_on, which no rule declares
    let mut required_by: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for d in &defs {
        for dep in &d.depends_on {
            required_by.entry(dep.clone()).or_default().push(d.id.clone());
        }
    }

    let mut rules = Vec::new();
    let mut states: BTreeMap<String, usize> = BTreeMap::new();
    let mut findings = Vec::new();
    let mut coverage = Coverage::default();

    for def in defs {
        let validator = def.enforcement.validator.as_ref().map(|v| {
            let defined_in = validator_defined_in(&root, v);
            ValidatorRef {
                name: v.clone(),
                function: format!("mj_validate_{v}"),
                present: defined_in.is_some(),
                defined_in,
            }
        });

        let mut tests = Vec::new();
        for path in &def.enforcement.tests {
            let present = root.join(path).exists();
            let id = TestId::of(path);
            let gates = gates_for(path, &gate_commands);
            let kind = if id.is_some() {
                ArtifactKind::Case
            } else if gates.iter().any(|g| gate_runs_exactly(g, path, &gate_commands)) {
                ArtifactKind::Gate
            } else {
                ArtifactKind::Unknown
            };
            let execution = id
                .as_ref()
                .and_then(|t| ledger.latest(&t.as_string()))
                .cloned();
            let state = match (&id, &execution) {
                (None, _) => ProofState::Unrunnable,
                (Some(_), None) => ProofState::NotRun,
                (Some(t), Some(e)) => {
                    if !e.outcome.proves() {
                        ProofState::Failing
                    } else {
                        match diffs.get(&e.commit).and_then(|d| d.as_ref()) {
                            // git could not compare: not knowing is not proof
                            None => ProofState::Stale,
                            Some(d) => {
                                let changed_at_all: Vec<&String> = d
                                    .iter()
                                    .filter(|p| p.as_str() != crate::evidence::LEDGER_PATH)
                                    .collect();
                                // what this rule names, and nothing else
                                let names_changed = d.contains(&t.source()) || d.contains(&def.path);
                                let test_moved = e.digest_matches(&root) == Some(false);
                                if names_changed || test_moved {
                                    ProofState::Stale
                                } else if changed_at_all.is_empty() {
                                    ProofState::Proven
                                } else {
                                    ProofState::InputsUnchanged
                                }
                            }
                        }
                    }
                }
            };
            tests.push(TestProof {
                gates,
                kind,
                path: path.clone(),
                present,
                test: id.as_ref().map(TestId::as_string),
                state,
                meaning: state.meaning().to_string(),
                execution,
                reproduce: id.as_ref().map(TestId::reproduce),
            });
        }

        // The rule's state is the weakest of the parts that can carry proof — and which
        // parts those are is the whole subtlety, learned by running this against the real
        // corpus.
        //
        // `dangling` always dominates: a named path that is not in the tree is a lie
        // whatever else the rule names, because the rule reads as proven on its strength.
        //
        // An artifact no runner and no gate drives is different. It contributes nothing,
        // and that is not a defect in the rule: `project.web-surface-declared-once` names a
        // case, two gates and `scripts/generate-site-data`, which nothing runs as its own
        // command. Letting that one path drag the rule to `unrunnable` reported six rules
        // as unprovable while each had a perfectly good case — a false finding, which is
        // the failure this whole subsystem exists to refuse. So such a path is reported,
        // and judged only when it is all the rule has.
        let mut state = if def.enforcement.mode == Mode::Reviewed {
            RuleState::Reviewed
        } else if def.enforcement.mode == Mode::Declarative {
            RuleState::Unproven
        } else if tests.iter().any(|t| !t.present) {
            RuleState::Dangling
        } else {
            let provable = tests
                .iter()
                .filter(|t| t.kind != ArtifactKind::Unknown)
                .map(rule_state_of)
                .max();
            match provable {
                Some(worst) => worst,
                // nothing it names can ever be recorded: that is the honest verdict
                None => tests
                    .iter()
                    .map(rule_state_of)
                    .max()
                    .unwrap_or(RuleState::Unproven),
            }
        };
        if let Some(v) = &validator {
            if !v.present {
                state = RuleState::Dangling;
            } else if tests.is_empty() {
                // a dispatched rule with a present validator and no case: the dispatcher
                // runs it, and nothing behavioural proves it does the right thing
                state = state.max(RuleState::NotRun);
            }
        }

        let gates: Vec<String> = tests
            .iter()
            .flat_map(|t| t.gates.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        let reason = unsupported(def.class, state);
        let satisfied = reason.is_none();
        if let Some(reason) = reason {
            findings.push(Finding {
                rule: def.id.clone(),
                class: def.class,
                state,
                reason,
                reproduce: "scripts/ci/rule-proof-check".to_string(),
            });
        }

        // the tallies, all counted from what was just derived
        coverage.rules += 1;
        match def.class {
            Class::Blocking => coverage.blocking += 1,
            Class::Advisory => coverage.advisory += 1,
            Class::Unknown => {}
        }
        if matches!(def.enforcement.mode, Mode::Dispatched | Mode::Gated) {
            coverage.named_proof += 1;
        }
        if state != RuleState::Dangling
            && matches!(def.enforcement.mode, Mode::Dispatched | Mode::Gated)
        {
            coverage.artifacts_present += 1;
        }
        if tests.iter().any(|t| t.test.is_some()) {
            coverage.runnable += 1;
        }
        if !tests.is_empty() && tests.iter().all(|t| t.execution.is_some()) {
            coverage.recorded += 1;
        }
        if state.passing() {
            coverage.passing += 1;
        }
        if state == RuleState::Gated {
            coverage.mechanism_only += 1;
        }
        if state == RuleState::Reviewed {
            coverage.review_only += 1;
        }
        if !gates.is_empty() {
            coverage.gated += 1;
        }
        if satisfied {
            coverage.satisfied += 1;
        }
        *states.entry(state.label().to_string()).or_insert(0) += 1;

        rules.push(RuleProof {
            required_by: required_by.get(&def.id).cloned().unwrap_or_default(),
            validator,
            tests,
            gates,
            state,
            meaning: state.meaning().to_string(),
            satisfied,
            rule: def,
        });
    }

    RulesReport {
        head,
        working_tree,
        rules,
        states,
        coverage,
        satisfied: findings.is_empty(),
        findings,
    }
}

#[cfg(test)]
mod tests;
