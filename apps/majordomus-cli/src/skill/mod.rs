//! Skills as proven capabilities: what the repository can show about each skill, derived.
//!
//! A skill is data: `SKILL.md` under the layer's skills section, discovered by the source class
//! `skill` and validated against its schema by the index (ADR 0007). That makes a skill
//! *exist*. It says nothing about whether the procedure is exercised by anything, reached by
//! anything, published anywhere or held by any gate — and a catalogue whose entries are
//! merely present is the hand-maintained registry this repository refuses, one level up.
//!
//! So four facts are derived here, per skill, on every read, and none is authored:
//!
//! * **tested** — a behavioural case or crate test names the skill with the marker
//!   `majordomus-skill: <id>` on a comment line, and the evidence ledger holds a run of that
//!   test. The run is judged by the same judgement a rule's
//!   named test gets, so a test cannot be current for a rule and stale for a skill;
//! * **documented** — the site's page projection for the skill is a tracked file;
//! * **enforced** — a dispatched rule validates skills (its validator is `skills`) and a CI
//!   gate whose command runs the skills verification is selected by a change to the skill;
//! * **used** — an invocation surface (a workflow, a prompt, a profile, a provider template,
//!   a recipe, a CI workflow, a root bootstrap) references the skill by its URI
//!   `majordomus://skill/<id>` or by the path of its `SKILL.md`.
//!
//! An active skill that no test names, or that nothing invokes, is an **orphan**: a failure.
//! A test that names a skill that does not exist, and an invocation that references one, are
//! failures too — a binding to nothing reads as proof. Evidence that is not current, a missing
//! page and a missing gate are warnings: each is a debt with a command that settles it, and
//! none of them can be settled by the author of the skill alone. A draft or deprecated skill
//! owes nothing.
//!
//! ```
//! use majordomus_cli::evidence::Ledger;
//! use majordomus_cli::skill::Skills;
//! use majordomus_cli::synthetic::SyntheticRepository;
//!
//! let repo = SyntheticRepository::small().unwrap();
//! let index = repo.index().unwrap();
//! let skills = Skills::build(&index, &Ledger::empty());
//! // every skill the index holds is answered, and a repository with none owes nothing
//! assert_eq!(
//!     skills.skills.len(),
//!     index.objects.iter().filter(|o| o.kind == "skill").count()
//! );
//! assert_eq!(skills.failures() + skills.warnings(), skills.findings.len());
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::discovery::glob::Glob;
use crate::evidence::{Ledger, ProofState, TestId};
use crate::gates::model::GateModel;
use crate::index::Index;
use crate::model::Severity;

/// The kind of a skill in the index.
pub const SKILL: &str = "skill";

/// The marker a test carries, on a comment line, to name the skills it exercises:
/// `# majordomus-skill: implement repo-review` in a case, `//! majordomus-skill: implement`
/// in a crate test.
pub const TEST_MARKER: &str = "majordomus-skill:";

/// The URI prefix an invocation references a skill by.
pub const SKILL_URI_PREFIX: &str = "majordomus://skill/";

/// Where the site's page for a skill is projected, repository-relative: `<dir>/<id>.md`.
pub const SKILL_PAGES_DIR: &str = "site/content/skills";

/// The words a gate's command carries when it runs the skills verification. A gate is read
/// by what it runs, never by its id, so a gate renamed in the CI model is renamed here.
pub const SKILLS_VERIFY_COMMAND: &str = "skills verify";

/// The validator a dispatched rule names when it is the doctrine that validates skills.
pub const SKILLS_VALIDATOR: &str = "skills";

/// Where tests live that a skill marker is read from: the behavioural cases and the crate's
/// integration tests, the two runners the evidence ledger records.
const TEST_DIRS: &[&str] = &["test/cases", "apps/majordomus-cli/tests"];

/// The invocation surfaces outside the layer's own sections: recipes, CI workflows and the
/// root bootstraps. The layer's workflows, prompts, profiles and provider templates are added
/// from the manifest's sections by [`invocation_surfaces`].
const ROOT_SURFACES: &[&str] = &[
    "justfile",
    ".just/**",
    ".github/workflows/**",
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
];

// ---------------------------------------------------------------- vocabulary

/// Where a skill came from, when it was not written here first: an opaque marker into a
/// ledger kept outside the repository. The committed half names no source; the ledger entry
/// it points at does, locally.
///
/// ```
/// use majordomus_cli::skill::SkillProvenance;
/// let p = SkillProvenance {
///     origin: "prior-art".into(),
///     ledger: "import-2026-09-09#1".into(),
///     decision: "adapted".into(),
/// };
/// assert!(p.ledger.starts_with("import-"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillProvenance {
    /// `prior-art`: the concept was studied elsewhere before this skill was written.
    pub origin: String,
    /// `import-<date>#<n>`: the entry of the local import ledger that holds the mapping.
    pub ledger: String,
    /// `adapted`, `reimplemented` or `merged`.
    pub decision: String,
}

/// What the evidence ledger can say about a skill's tests, as a whole: the weakest state of
/// the tests that name it, or `untested` when none does.
///
/// ```
/// use majordomus_cli::skill::SkillEvidence;
/// assert!(SkillEvidence::Proven.current());
/// assert!(SkillEvidence::InputsUnchanged.current());
/// assert!(!SkillEvidence::Stale.current());
/// assert!(SkillEvidence::Proven < SkillEvidence::Untested);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SkillEvidence {
    /// Every naming test has a passing run, and nothing but the ledger changed since.
    Proven,
    /// Every naming test passed, and neither the test nor the skill changed since.
    InputsUnchanged,
    /// A passing run exists, and the test or the skill changed since.
    Stale,
    /// The latest recorded run of a naming test did not pass.
    Failing,
    /// A test names the skill and no run of it was ever recorded.
    NotRun,
    /// A test names the skill from a path no runner drives.
    Unrunnable,
    /// No test names the skill.
    Untested,
}

impl SkillEvidence {
    /// Does this state carry a passing run nothing known has invalidated?
    ///
    /// ```
    /// use majordomus_cli::skill::SkillEvidence;
    /// assert!(!SkillEvidence::NotRun.current());
    /// ```
    pub fn current(self) -> bool {
        matches!(self, SkillEvidence::Proven | SkillEvidence::InputsUnchanged)
    }

    /// The word every surface prints for this evidence state: the text listing, the JSON
    /// answer and the site page all read it here, so they cannot spell a state two ways.
    ///
    /// ```
    /// use majordomus_cli::skill::SkillEvidence;
    /// assert_eq!(SkillEvidence::InputsUnchanged.label(), "inputs_unchanged");
    /// ```
    pub fn label(self) -> &'static str {
        match self {
            SkillEvidence::Proven => "proven",
            SkillEvidence::InputsUnchanged => "inputs_unchanged",
            SkillEvidence::Stale => "stale",
            SkillEvidence::Failing => "failing",
            SkillEvidence::NotRun => "not_run",
            SkillEvidence::Unrunnable => "unrunnable",
            SkillEvidence::Untested => "untested",
        }
    }

    /// The skill-level word for one test's proof state.
    ///
    /// ```
    /// use majordomus_cli::evidence::ProofState;
    /// use majordomus_cli::skill::SkillEvidence;
    /// assert_eq!(SkillEvidence::of(ProofState::NotRun), SkillEvidence::NotRun);
    /// ```
    pub fn of(state: ProofState) -> SkillEvidence {
        match state {
            ProofState::Proven => SkillEvidence::Proven,
            ProofState::InputsUnchanged => SkillEvidence::InputsUnchanged,
            ProofState::Stale => SkillEvidence::Stale,
            ProofState::Failing => SkillEvidence::Failing,
            ProofState::NotRun => SkillEvidence::NotRun,
            ProofState::Unrunnable | ProofState::NoTest => SkillEvidence::Unrunnable,
        }
    }
}

/// Where a skill stands, derived from the four facts.
///
/// ```
/// use majordomus_cli::skill::SkillStanding;
/// assert_eq!(SkillStanding::Orphan.label(), "orphan");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SkillStanding {
    /// Active, and tested with current evidence, documented, enforced and used.
    Proven,
    /// Active, named by a test and invoked, with at least one debt a warning names.
    Partial,
    /// Active, and no test names it or nothing invokes it.
    Orphan,
    /// Its file does not satisfy the skill contract; one the index refused outright is listed
    /// here too, so a listing never silently shrinks.
    Invalid,
    /// Draft or deprecated: nothing is owed.
    NotRequired,
}

impl SkillStanding {
    /// The word every surface prints for this standing, the same snake_case word the JSON
    /// answer serialises, so a script filtering on it and a person reading it agree.
    ///
    /// ```
    /// use majordomus_cli::skill::SkillStanding;
    /// assert_eq!(SkillStanding::NotRequired.label(), "not_required");
    /// ```
    pub fn label(self) -> &'static str {
        match self {
            SkillStanding::Proven => "proven",
            SkillStanding::Partial => "partial",
            SkillStanding::Orphan => "orphan",
            SkillStanding::Invalid => "invalid",
            SkillStanding::NotRequired => "not_required",
        }
    }
}

/// How bad a finding is: a failure refuses, a warning is reported.
///
/// ```
/// use majordomus_cli::skill::SkillFindingLevel;
/// assert_ne!(SkillFindingLevel::Fail, SkillFindingLevel::Warn);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SkillFindingLevel {
    /// Refuses: `skills verify` exits 10 and the doctrine fails.
    Fail,
    /// Reported, and does not refuse.
    Warn,
}

/// One thing the derivation found wrong, with the command that settles or reproduces it.
///
/// ```
/// use majordomus_cli::skill::{SkillFinding, SkillFindingLevel};
/// let f = SkillFinding {
///     level: SkillFindingLevel::Fail,
///     code: "untested".into(),
///     skill: Some("implement".into()),
///     subject: ".ai/repo/skills/implement/SKILL.md".into(),
///     message: "no test names this skill".into(),
///     reproduce: "majordomus skills explain implement".into(),
/// };
/// assert!(f.reproduce.contains("explain"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillFinding {
    /// `fail` or `warn`.
    pub level: SkillFindingLevel,
    /// A stable code: `contract`, `untested`, `unused`, `unevidenced`, `failing`,
    /// `undocumented`, `unenforced`, `unknown_skill_in_test`, `unknown_skill_invoked`.
    pub code: String,
    /// The skill concerned, when the finding is about one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    /// The file the finding is about, repository-relative.
    pub subject: String,
    /// What is wrong, and what to do about it.
    pub message: String,
    /// The command that reproduces or settles it.
    pub reproduce: String,
}

/// One test that names a skill, and what the ledger says about it.
///
/// ```
/// use majordomus_cli::evidence::ProofState;
/// use majordomus_cli::skill::SkillTest;
/// let t = SkillTest {
///     path: "test/cases/95_skills.sh".into(),
///     test: Some("suite:95_skills".into()),
///     state: ProofState::NotRun,
///     reproduce: Some("bash test/run.sh 95_skills".into()),
///     recorded_at: None,
/// };
/// assert_eq!(t.state, ProofState::NotRun);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillTest {
    /// The test's path.
    pub path: String,
    /// Its identity in the ledger, when a runner owns the path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    /// What the ledger says about its latest run, for this skill.
    pub state: ProofState,
    /// The command that runs it again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reproduce: Option<String>,
    /// When the run behind the state was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
}

/// The tested fact: the tests that name the skill, and their weakest state.
///
/// ```
/// use majordomus_cli::skill::{SkillEvidence, SkillTested};
/// let t = SkillTested { state: SkillEvidence::Untested, tests: vec![] };
/// assert!(!t.state.current());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillTested {
    /// The weakest state of the naming tests; `untested` when there are none.
    pub state: SkillEvidence,
    /// Every test that names the skill, in path order.
    pub tests: Vec<SkillTest>,
}

/// The documented fact: the page the site projects for the skill.
///
/// ```
/// use majordomus_cli::skill::SkillDocumented;
/// let d = SkillDocumented { documented: true, page: "site/content/skills/implement.md".into() };
/// assert!(d.page.ends_with("implement.md"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillDocumented {
    /// Whether the page is a tracked file.
    pub documented: bool,
    /// Where the page is projected.
    pub page: String,
}

/// The enforced fact: the doctrine that validates skills and the gates a change to this one
/// selects.
///
/// ```
/// use majordomus_cli::skill::SkillEnforced;
/// let e = SkillEnforced { enforced: false, doctrine: None, gates: vec![] };
/// assert!(!e.enforced);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillEnforced {
    /// A doctrine validates skills, and a gate running the verification covers this file.
    pub enforced: bool,
    /// The rule whose validator is `skills`, when the index holds one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doctrine: Option<String>,
    /// The CI gates a change to the skill selects whose command runs `skills verify`.
    pub gates: Vec<String>,
}

/// One place a skill is invoked from.
///
/// ```
/// use majordomus_cli::skill::SkillInvocation;
/// let i = SkillInvocation {
///     path: ".ai/repo/prompts/review.md".into(),
///     line: 3,
///     reference: "majordomus://skill/repo-review".into(),
/// };
/// assert_eq!(i.line, 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillInvocation {
    /// The invocation surface, repository-relative.
    pub path: String,
    /// The 1-based line the reference is on.
    pub line: usize,
    /// The reference as written.
    pub reference: String,
}

/// The used fact: every invocation surface that references the skill.
///
/// ```
/// use majordomus_cli::skill::SkillUsed;
/// let u = SkillUsed { used: false, invocations: vec![] };
/// assert!(!u.used);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillUsed {
    /// At least one invocation surface references the skill.
    pub used: bool,
    /// Every reference, in path and line order.
    pub invocations: Vec<SkillInvocation>,
}

/// One skill with everything derived about it.
///
/// ```
/// use majordomus_cli::skill::SkillStatus;
/// // every fact is always present in the answer, so a client never has to tell "absent"
/// // from "false"; only the provenance is optional
/// let schema = serde_json::to_value(schemars::schema_for!(SkillStatus)).unwrap();
/// let required = schema["required"].as_array().unwrap();
/// for field in ["id", "valid", "tested", "documented", "enforced", "used", "standing"] {
///     assert!(required.iter().any(|f| f == field), "{field}");
/// }
/// assert!(!required.iter().any(|f| f == "provenance"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillStatus {
    /// The skill's id, which is its directory name.
    pub id: String,
    /// `majordomus://skill/<id>`.
    pub uri: String,
    /// The title as authored.
    pub title: String,
    /// The description as authored.
    pub description: String,
    /// The lifecycle status as authored: `draft`, `active` or `deprecated`; `unknown` for a
    /// file the index refused.
    pub status: String,
    /// The version as authored.
    pub version: u64,
    /// The file, repository-relative.
    pub path: String,
    /// Where it came from, when it declares that.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<SkillProvenance>,
    /// Whether the file satisfies the skill contract, as the index judged it.
    pub valid: bool,
    /// The tested fact.
    pub tested: SkillTested,
    /// The documented fact.
    pub documented: SkillDocumented,
    /// The enforced fact.
    pub enforced: SkillEnforced,
    /// The used fact.
    pub used: SkillUsed,
    /// Where it stands, derived from the four.
    pub standing: SkillStanding,
    /// The findings about this skill.
    pub findings: Vec<SkillFinding>,
}

/// Every skill of a repository, derived, and every finding.
///
/// ```
/// use majordomus_cli::evidence::Ledger;
/// use majordomus_cli::skill::Skills;
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// let skills = Skills::build(&repo.index().unwrap(), &Ledger::empty());
/// assert!(skills.skill("no-such-skill").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Skills {
    /// Every skill, in index order.
    pub skills: Vec<SkillStatus>,
    /// Every finding: each skill's, then those about no one skill (a binding to nothing).
    pub findings: Vec<SkillFinding>,
}

impl Skills {
    /// Derive every skill's standing from the index, the evidence ledger, the tracked tests,
    /// the tracked invocation surfaces and the CI model.
    ///
    /// ```
    /// use majordomus_cli::evidence::Ledger;
    /// use majordomus_cli::skill::Skills;
    /// use majordomus_cli::synthetic::SyntheticRepository;
    /// let repo = SyntheticRepository::small().unwrap();
    /// let a = Skills::build(&repo.index().unwrap(), &Ledger::empty());
    /// let b = Skills::build(&repo.index().unwrap(), &Ledger::empty());
    /// assert_eq!(a, b, "two reads of one tree derive the same answer");
    /// ```
    pub fn build(index: &Index, ledger: &Ledger) -> Skills {
        let root = PathBuf::from(&index.repository.root);
        let skills_dir = index
            .repository
            .sections
            .get("skills")
            .cloned()
            .unwrap_or_else(|| ".ai/repo/skills".to_string());
        let tracked: Vec<String> = crate::git::ls_files_all(&root).unwrap_or_default();
        let tracked_set: BTreeSet<&str> = tracked.iter().map(String::as_str).collect();

        let objects: Vec<&crate::model::Object> =
            index.objects.iter().filter(|o| o.kind == SKILL).collect();
        let ids: BTreeSet<String> = objects.iter().map(|o| skill_id(o)).collect();

        let markers = test_markers(&root, &tracked);
        let invocations = invocation_references(&root, &tracked, index, &skills_dir);
        let comparisons = crate::rules::ledger_comparisons(&root, ledger);
        let doctrine = crate::rules::definitions(index)
            .into_iter()
            .find(|d| d.enforcement.validator.as_deref() == Some(SKILLS_VALIDATOR))
            .map(|d| d.id);
        let model = GateModel::load(&root).ok();

        let mut findings = Vec::new();
        let mut skills = Vec::new();
        for o in objects {
            let id = skill_id(o);
            let path = o.provenance.path.clone();
            let meta = &o.metadata;
            let status = str_field(meta, "status");
            let active = status == "active";
            let contract: Vec<&crate::model::Diagnostic> = index
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error && d.path.as_deref() == Some(&path))
                .collect();
            let mut own = Vec::new();
            let explain = format!("majordomus skills explain {id}");
            for d in &contract {
                own.push(SkillFinding {
                    level: SkillFindingLevel::Fail,
                    code: "contract".into(),
                    skill: Some(id.clone()),
                    subject: path.clone(),
                    message: format!("{}: {}", d.code, d.message),
                    reproduce: "majordomus skills check".into(),
                });
            }

            // tested
            let mut tests = Vec::new();
            for (test_path, named) in &markers {
                if !named.contains(&id) {
                    continue;
                }
                let tid = TestId::of(test_path);
                let execution = tid.as_ref().and_then(|t| ledger.latest(&t.as_string()));
                let state =
                    crate::rules::test_state(&root, &comparisons, tid.as_ref(), execution, &path);
                tests.push(SkillTest {
                    path: test_path.clone(),
                    test: tid.as_ref().map(TestId::as_string),
                    state,
                    reproduce: tid.as_ref().map(TestId::reproduce),
                    recorded_at: execution.map(|e| e.at.clone()),
                });
            }
            let evidence = tests
                .iter()
                .map(|t| SkillEvidence::of(t.state))
                .max()
                .unwrap_or(SkillEvidence::Untested);
            if active {
                match evidence {
                    SkillEvidence::Untested => own.push(SkillFinding {
                        level: SkillFindingLevel::Fail,
                        code: "untested".into(),
                        skill: Some(id.clone()),
                        subject: path.clone(),
                        message: format!(
                            "orphan: no test names skill '{id}'; add `{TEST_MARKER} {id}` on a \
                             comment line of the case or crate test that exercises it"
                        ),
                        reproduce: explain.clone(),
                    }),
                    SkillEvidence::Failing => own.push(SkillFinding {
                        level: SkillFindingLevel::Fail,
                        code: "failing".into(),
                        skill: Some(id.clone()),
                        subject: path.clone(),
                        message: format!(
                            "the latest recorded run of a test naming skill '{id}' did not pass"
                        ),
                        reproduce: weakest_reproduce(&tests, &explain),
                    }),
                    e if !e.current() => own.push(SkillFinding {
                        level: SkillFindingLevel::Warn,
                        code: "unevidenced".into(),
                        skill: Some(id.clone()),
                        subject: path.clone(),
                        message: format!(
                            "skill '{id}' is named by {} test(s) and its evidence is {}: run the \
                             test and record it (majordomus evidence record)",
                            tests.len(),
                            e.label()
                        ),
                        reproduce: weakest_reproduce(&tests, &explain),
                    }),
                    _ => {}
                }
            }

            // documented
            let page = format!("{SKILL_PAGES_DIR}/{id}.md");
            let documented = tracked_set.contains(page.as_str());
            if active && !documented {
                own.push(SkillFinding {
                    level: SkillFindingLevel::Warn,
                    code: "undocumented".into(),
                    skill: Some(id.clone()),
                    subject: path.clone(),
                    message: format!("skill '{id}' has no tracked page at {page}"),
                    reproduce: "scripts/generate-site-data".into(),
                });
            }

            // enforced
            let gates = verifying_gates(model.as_ref(), &path);
            let enforced = doctrine.is_some() && !gates.is_empty();
            if active && !enforced {
                own.push(SkillFinding {
                    level: SkillFindingLevel::Warn,
                    code: "unenforced".into(),
                    skill: Some(id.clone()),
                    subject: path.clone(),
                    message: match &doctrine {
                        None => format!(
                            "skill '{id}': no dispatched rule validates skills (validator \
                             `{SKILLS_VALIDATOR}`)"
                        ),
                        Some(_) => format!(
                            "skill '{id}': no CI gate selected by a change to {path} runs \
                             `{SKILLS_VERIFY_COMMAND}`"
                        ),
                    },
                    reproduce: "scripts/ci-plan".into(),
                });
            }

            // used
            let used: Vec<SkillInvocation> = invocations
                .iter()
                .filter(|(target, _)| target == &id)
                .map(|(_, inv)| inv.clone())
                .collect();
            if active && used.is_empty() {
                own.push(SkillFinding {
                    level: SkillFindingLevel::Fail,
                    code: "unused".into(),
                    skill: Some(id.clone()),
                    subject: path.clone(),
                    message: format!(
                        "orphan: nothing invokes skill '{id}'; reference `{SKILL_URI_PREFIX}{id}` \
                         from a workflow, prompt, profile, provider template, recipe or CI \
                         workflow that should follow it"
                    ),
                    reproduce: explain.clone(),
                });
            }

            let standing = if !contract.is_empty() {
                SkillStanding::Invalid
            } else if !active {
                SkillStanding::NotRequired
            } else if evidence == SkillEvidence::Untested || used.is_empty() {
                SkillStanding::Orphan
            } else if evidence.current() && documented && enforced {
                SkillStanding::Proven
            } else {
                SkillStanding::Partial
            };

            findings.extend(own.iter().cloned());
            skills.push(SkillStatus {
                uri: format!("{SKILL_URI_PREFIX}{id}"),
                title: o.title.clone().unwrap_or_else(|| id.clone()),
                description: o.description.clone().unwrap_or_default(),
                status,
                version: meta.get("version").and_then(Value::as_u64).unwrap_or(0),
                provenance: provenance_of(meta),
                valid: contract.is_empty(),
                tested: SkillTested {
                    state: evidence,
                    tests,
                },
                documented: SkillDocumented { documented, page },
                enforced: SkillEnforced {
                    enforced,
                    doctrine: doctrine.clone(),
                    gates,
                },
                used: SkillUsed {
                    used: !used.is_empty(),
                    invocations: used,
                },
                standing,
                findings: own,
                path,
                id,
            });
        }

        // a skill file the index refused holds no object, and a listing that dropped it would
        // silently shrink: it is answered as invalid, with every diagnostic as a finding
        let indexed: BTreeSet<&str> = skills.iter().map(|s| s.path.as_str()).collect();
        let prefix = format!("{}/", skills_dir.trim_end_matches('/'));
        let mut refused: BTreeMap<String, Vec<&crate::model::Diagnostic>> = BTreeMap::new();
        for d in index
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
        {
            let Some(p) = d.path.as_deref() else { continue };
            let Some(rest) = p.strip_prefix(&prefix) else {
                continue;
            };
            if rest.ends_with("/SKILL.md") && rest.matches('/').count() == 1 && !indexed.contains(p)
            {
                refused.entry(p.to_string()).or_default().push(d);
            }
        }
        for (path, diagnostics) in refused {
            let id = path
                .trim_start_matches(&prefix)
                .trim_end_matches("/SKILL.md")
                .to_string();
            let own: Vec<SkillFinding> = diagnostics
                .iter()
                .map(|d| SkillFinding {
                    level: SkillFindingLevel::Fail,
                    code: "contract".into(),
                    skill: Some(id.clone()),
                    subject: path.clone(),
                    message: format!("{}: {}", d.code, d.message),
                    reproduce: "majordomus skills check".into(),
                })
                .collect();
            findings.extend(own.iter().cloned());
            skills.push(SkillStatus {
                uri: format!("{SKILL_URI_PREFIX}{id}"),
                title: id.clone(),
                description: String::new(),
                status: "unknown".into(),
                version: 0,
                provenance: None,
                valid: false,
                tested: SkillTested {
                    state: SkillEvidence::Untested,
                    tests: Vec::new(),
                },
                documented: SkillDocumented {
                    documented: false,
                    page: format!("{SKILL_PAGES_DIR}/{id}.md"),
                },
                enforced: SkillEnforced {
                    enforced: false,
                    doctrine: doctrine.clone(),
                    gates: Vec::new(),
                },
                used: SkillUsed {
                    used: false,
                    invocations: Vec::new(),
                },
                standing: SkillStanding::Invalid,
                findings: own,
                path,
                id,
            });
        }

        // bindings to nothing: a test or an invocation naming a skill that is not there
        for (test_path, named) in &markers {
            for n in named.iter().filter(|n| !ids.contains(*n)) {
                findings.push(SkillFinding {
                    level: SkillFindingLevel::Fail,
                    code: "unknown_skill_in_test".into(),
                    skill: None,
                    subject: test_path.clone(),
                    message: format!(
                        "the test names skill '{n}', which does not exist; a binding to nothing \
                         reads as proof"
                    ),
                    reproduce: "majordomus skills list".into(),
                });
            }
        }
        for (target, inv) in &invocations {
            if !ids.contains(target) {
                findings.push(SkillFinding {
                    level: SkillFindingLevel::Fail,
                    code: "unknown_skill_invoked".into(),
                    skill: None,
                    subject: inv.path.clone(),
                    message: format!(
                        "line {} invokes skill '{target}' ({}), which does not exist",
                        inv.line, inv.reference
                    ),
                    reproduce: "majordomus skills list".into(),
                });
            }
        }

        Skills { skills, findings }
    }

    /// One skill by id, the directory name under `.ai/repo/skills/`; `None` only when the
    /// repository has no skill of that name, since one the index refused is still listed,
    /// standing `invalid`.
    ///
    /// ```
    /// use majordomus_cli::skill::Skills;
    /// let empty = Skills { skills: vec![], findings: vec![] };
    /// assert!(empty.skill("implement").is_none());
    /// ```
    pub fn skill(&self, id: &str) -> Option<&SkillStatus> {
        self.skills.iter().find(|s| s.id == id)
    }

    /// How many findings refuse: `skills verify` is valid only when this is zero. Warnings
    /// are counted apart, by [`Skills::warnings`], and never make it invalid.
    ///
    /// ```
    /// use majordomus_cli::skill::Skills;
    /// assert_eq!(Skills { skills: vec![], findings: vec![] }.failures(), 0);
    /// ```
    pub fn failures(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.level == SkillFindingLevel::Fail)
            .count()
    }

    /// How many findings are warnings: debts a skill still owes that do not refuse the
    /// verdict, such as a partial standing. Refusals are counted by [`Skills::failures`].
    ///
    /// ```
    /// use majordomus_cli::skill::Skills;
    /// assert_eq!(Skills { skills: vec![], findings: vec![] }.warnings(), 0);
    /// ```
    pub fn warnings(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.level == SkillFindingLevel::Warn)
            .count()
    }
}

// ---------------------------------------------------------------- derivations

fn skill_id(o: &crate::model::Object) -> String {
    let id = str_field(&o.metadata, "id");
    if id.is_empty() {
        o.identity.clone()
    } else {
        id
    }
}

fn str_field(meta: &Value, key: &str) -> String {
    meta.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn provenance_of(meta: &Value) -> Option<SkillProvenance> {
    let p = meta.get("provenance")?;
    Some(SkillProvenance {
        origin: str_field(p, "origin"),
        ledger: str_field(p, "ledger"),
        decision: str_field(p, "decision"),
    })
}

/// The command that reproduces the weakest of `tests`, or `otherwise` when that test names
/// no runner that could reproduce it.
fn weakest_reproduce(tests: &[SkillTest], otherwise: &str) -> String {
    tests
        .iter()
        .max_by_key(|t| SkillEvidence::of(t.state))
        .and_then(|t| t.reproduce.clone())
        .unwrap_or_else(|| otherwise.to_string())
}

fn is_skill_id(word: &str) -> bool {
    let mut chars = word.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The skill ids a marker line names, when the line is a comment carrying the marker.
///
/// Only a comment line counts — `#`, `//` or `//!` first — so that a test which writes a
/// marker into a fixture, as data, does not bind itself to the skill it writes.
///
/// ```
/// use majordomus_cli::skill::marker_ids;
/// let marker = concat!("majordomus-", "skill:");
/// assert_eq!(marker_ids(&format!("# {marker} implement repo-review")), ["implement", "repo-review"]);
/// assert_eq!(marker_ids(&format!("//! {marker} implement")), ["implement"]);
/// assert!(marker_ids(&format!("printf '# {marker} x'")).is_empty());
/// ```
pub fn marker_ids(line: &str) -> Vec<String> {
    let t = line.trim_start();
    let body = if let Some(r) = t.strip_prefix("//!") {
        r
    } else if let Some(r) = t.strip_prefix("//") {
        r
    } else if let Some(r) = t.strip_prefix('#') {
        r
    } else {
        return Vec::new();
    };
    let Some(rest) = body.trim_start().strip_prefix(TEST_MARKER) else {
        return Vec::new();
    };
    rest.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| is_skill_id(w))
        .map(str::to_string)
        .collect()
}

/// Every tracked test a runner owns that carries a marker, with the ids it names.
fn test_markers(root: &Path, tracked: &[String]) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in tracked {
        if !TEST_DIRS.iter().any(|d| path.starts_with(&format!("{d}/"))) {
            continue;
        }
        if TestId::of(path).is_none() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(path)) else {
            continue;
        };
        let named: BTreeSet<String> = text.lines().flat_map(marker_ids).collect();
        if !named.is_empty() {
            out.insert(path.clone(), named);
        }
    }
    out
}

/// The patterns of every invocation surface: the layer's workflows, prompts, profiles and
/// provider templates (read from the manifest's sections, beside the skills section), and the
/// recipes, CI workflows and root bootstraps.
///
/// ```
/// use majordomus_cli::skill::invocation_surfaces;
/// let mut sections = std::collections::BTreeMap::new();
/// sections.insert("prompts".to_string(), ".ai/repo/prompts".to_string());
/// let s = invocation_surfaces(&sections, ".ai/repo/skills");
/// assert!(s.contains(&".ai/repo/prompts/**".to_string()));
/// assert!(s.contains(&".ai/repo/providers/**".to_string()));
/// assert!(s.contains(&"justfile".to_string()));
/// ```
pub fn invocation_surfaces(sections: &BTreeMap<String, String>, skills_dir: &str) -> Vec<String> {
    let mut out: Vec<String> = ["workflows", "prompts", "profiles"]
        .iter()
        .filter_map(|k| sections.get(*k))
        .map(|d| format!("{}/**", d.trim_end_matches('/')))
        .collect();
    if let Some((layer, _)) = skills_dir.trim_end_matches('/').rsplit_once('/') {
        out.push(format!("{layer}/providers/**"));
    }
    out.extend(ROOT_SURFACES.iter().map(|s| s.to_string()));
    out
}

/// Every reference an invocation surface makes to a skill: `(target id, where)`, in path and
/// line order.
fn invocation_references(
    root: &Path,
    tracked: &[String],
    index: &Index,
    skills_dir: &str,
) -> Vec<(String, SkillInvocation)> {
    let globs: Vec<Glob> = invocation_surfaces(&index.repository.sections, skills_dir)
        .iter()
        .map(|p| Glob::new(p))
        .collect();
    let path_prefix = format!("{}/", skills_dir.trim_end_matches('/'));
    let mut out = Vec::new();
    for path in tracked {
        if !globs.iter().any(|g| g.matches(path)) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(path)) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            for (target, reference) in references_in(line, &path_prefix) {
                out.push((
                    target,
                    SkillInvocation {
                        path: path.clone(),
                        line: n + 1,
                        reference,
                    },
                ));
            }
        }
    }
    out
}

/// The skill references one line makes: `majordomus://skill/<id>`, and
/// `<skills dir>/<id>/SKILL.md`.
///
/// ```
/// use majordomus_cli::skill::references_in;
/// let r = references_in(
///     "Follow majordomus://skill/repo-review, then .ai/repo/skills/implement/SKILL.md.",
///     ".ai/repo/skills/",
/// );
/// assert_eq!(r[0].0, "repo-review");
/// assert_eq!(r[1], ("implement".to_string(), ".ai/repo/skills/implement/SKILL.md".to_string()));
/// ```
pub fn references_in(line: &str, skills_prefix: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let id_end = |s: &str| {
        s.find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
            .unwrap_or(s.len())
    };
    let mut rest = line;
    while let Some(at) = rest.find(SKILL_URI_PREFIX) {
        let after = &rest[at + SKILL_URI_PREFIX.len()..];
        let id = after[..id_end(after)].trim_end_matches('-');
        if is_skill_id(id) {
            out.push((id.to_string(), format!("{SKILL_URI_PREFIX}{id}")));
        }
        rest = &after[id.len()..];
    }
    let mut rest = line;
    while let Some(at) = rest.find(skills_prefix) {
        let after = &rest[at + skills_prefix.len()..];
        let end = id_end(after);
        let id = &after[..end];
        if is_skill_id(id) && after[end..].starts_with("/SKILL.md") {
            out.push((id.to_string(), format!("{skills_prefix}{id}/SKILL.md")));
        }
        rest = &after[end..];
    }
    out
}

/// The gates a change to `path` selects whose command runs the skills verification.
fn verifying_gates(model: Option<&GateModel>, path: &str) -> Vec<String> {
    let Some(model) = model else {
        return Vec::new();
    };
    let verifying: BTreeSet<&str> = model
        .gates
        .iter()
        .filter(|g| g.runs.contains(SKILLS_VERIFY_COMMAND))
        .map(|g| g.id.as_str())
        .collect();
    let mut out: BTreeSet<String> = BTreeSet::new();
    for class in model.classes.iter().filter(|c| c.matches(path)) {
        if class.escalates() {
            out.extend(verifying.iter().map(|g| g.to_string()));
        } else {
            out.extend(
                class
                    .named()
                    .iter()
                    .filter(|g| verifying.contains(g.as_str()))
                    .cloned(),
            );
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: &str = concat!("majordomus-", "skill:");

    #[test]
    fn a_marker_binds_only_from_a_comment_line() {
        assert_eq!(marker_ids(&format!("  # {M} a b-2, c")), ["a", "b-2", "c"]);
        assert!(marker_ids(&format!("echo '{M} a'")).is_empty());
        assert!(marker_ids(&format!("# {M} Bad UPPER")).is_empty());
        assert!(marker_ids("# majordomus-covers: skills").is_empty());
    }

    #[test]
    fn a_reference_is_a_uri_or_the_path_of_the_skill_file_and_nothing_near_it() {
        let p = ".ai/repo/skills/";
        assert!(references_in("see .ai/repo/skills/README.md", p).is_empty());
        assert!(references_in("see .ai/repo/skills/implement/examples/x.md", p).is_empty());
        assert_eq!(
            references_in("`majordomus://skill/deploy-site`.", p),
            [(
                "deploy-site".to_string(),
                "majordomus://skill/deploy-site".to_string()
            )]
        );
        assert_eq!(
            references_in("majordomus://skill/a majordomus://skill/b", p).len(),
            2
        );
    }

    #[test]
    fn the_weakest_test_decides_and_no_test_is_weaker_than_any() {
        let states = [ProofState::Proven, ProofState::NotRun];
        let worst = states.iter().map(|s| SkillEvidence::of(*s)).max().unwrap();
        assert_eq!(worst, SkillEvidence::NotRun);
        assert!(SkillEvidence::Unrunnable < SkillEvidence::Untested);
    }

    /// Every evidence state and every standing prints as the word it serialises to, and
    /// every proof state of a test maps to the evidence word a skill carries.
    #[test]
    fn every_state_prints_as_the_word_it_serialises_to() {
        for e in [
            SkillEvidence::Proven,
            SkillEvidence::InputsUnchanged,
            SkillEvidence::Stale,
            SkillEvidence::Failing,
            SkillEvidence::NotRun,
            SkillEvidence::Unrunnable,
            SkillEvidence::Untested,
        ] {
            assert_eq!(
                serde_json::to_string(&e).unwrap(),
                format!("\"{}\"", e.label())
            );
        }
        for s in [
            SkillStanding::Proven,
            SkillStanding::Partial,
            SkillStanding::Orphan,
            SkillStanding::Invalid,
            SkillStanding::NotRequired,
        ] {
            assert_eq!(
                serde_json::to_string(&s).unwrap(),
                format!("\"{}\"", s.label())
            );
        }
        for (state, evidence) in [
            (ProofState::Proven, SkillEvidence::Proven),
            (ProofState::InputsUnchanged, SkillEvidence::InputsUnchanged),
            (ProofState::Stale, SkillEvidence::Stale),
            (ProofState::Failing, SkillEvidence::Failing),
            (ProofState::NotRun, SkillEvidence::NotRun),
            (ProofState::Unrunnable, SkillEvidence::Unrunnable),
            (ProofState::NoTest, SkillEvidence::Unrunnable),
        ] {
            assert_eq!(SkillEvidence::of(state), evidence, "{state:?}");
        }
    }

    fn test_of(state: ProofState, reproduce: Option<&str>) -> SkillTest {
        SkillTest {
            path: "test/cases/01_x.sh".into(),
            test: None,
            state,
            reproduce: reproduce.map(str::to_string),
            recorded_at: None,
        }
    }

    /// The finding's reproduce command is the weakest test's, because that is the run that
    /// would change the verdict; with no runner to name, it is the explanation instead.
    #[test]
    fn the_weakest_test_names_the_reproduce_command_or_the_explanation_does() {
        let explain = "majordomus skills explain x";
        let tests = [
            test_of(ProofState::Proven, Some("test/run.sh 01")),
            test_of(ProofState::Failing, Some("test/run.sh 02")),
        ];
        assert_eq!(weakest_reproduce(&tests, explain), "test/run.sh 02");
        let unrunnable = [
            test_of(ProofState::Proven, Some("test/run.sh 01")),
            test_of(ProofState::Unrunnable, None),
        ];
        assert_eq!(weakest_reproduce(&unrunnable, explain), explain);
        assert_eq!(weakest_reproduce(&[], explain), explain);
    }

    /// A URI prefix followed by no id is no reference, and a skills directory with no parent
    /// layer adds no provider surface.
    #[test]
    fn a_prefix_without_an_id_is_no_reference_and_a_bare_directory_has_no_layer() {
        let p = ".ai/repo/skills/";
        assert!(references_in("majordomus://skill/ and majordomus://skill/Upper", p).is_empty());
        let surfaces = invocation_surfaces(&BTreeMap::new(), "skills");
        assert!(
            !surfaces.iter().any(|s| s.ends_with("providers/**")),
            "{surfaces:?}"
        );
        assert!(surfaces.contains(&"justfile".to_string()));
    }

    /// A tracked test or surface that is gone from the disk contributes nothing, and a
    /// tracked file under a test directory that is no test is never read for a marker.
    #[test]
    fn a_tracked_file_that_cannot_be_read_binds_and_invokes_nothing() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let index = repo.index().unwrap();
        let root = repo.root();
        let workflows = index
            .repository
            .sections
            .get("workflows")
            .cloned()
            .expect("the synthetic manifest declares workflows");
        let write = |rel: &str, text: &str| {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        write("test/cases/02_present.sh", &format!("# {M} alpha\ntrue\n"));
        write("test/cases/README.md", &format!("# {M} beta\n"));
        write(
            &format!("{workflows}/present.md"),
            "Follow majordomus://skill/alpha.\n",
        );
        let tracked: Vec<String> = [
            "test/cases/01_gone.sh",
            "test/cases/02_present.sh",
            "test/cases/README.md",
            &format!("{workflows}/gone.md"),
            &format!("{workflows}/present.md"),
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let markers = test_markers(root, &tracked);
        assert_eq!(
            markers.keys().collect::<Vec<_>>(),
            ["test/cases/02_present.sh"]
        );

        let invoked = invocation_references(root, &tracked, &index, ".ai/repo/skills");
        assert_eq!(invoked.len(), 1, "{invoked:?}");
        assert_eq!(invoked[0].0, "alpha");
        assert_eq!(invoked[0].1.path, format!("{workflows}/present.md"));
        assert_eq!(invoked[0].1.line, 1);
    }

    /// A class that escalates selects every gate that verifies skills; a class that names
    /// gates selects only the verifying ones it names; no model selects nothing.
    #[test]
    fn an_escalating_class_selects_every_verifying_gate() {
        let model: GateModel = serde_json::from_value(serde_json::json!({
            "version": 1,
            "gates": [
                { "id": "named", "job": "j", "runs": "majordomus skills verify" },
                { "id": "unnamed", "job": "j", "runs": "x skills verify --strict" },
                { "id": "other", "job": "j", "runs": "true" }
            ],
            "classes": [
                { "id": "skills", "paths": [".ai/repo/skills/**"], "gates": ["named", "other"] },
                { "id": "ci", "paths": [".ai/repo/ci/**"], "gates": "full" }
            ]
        }))
        .unwrap();
        assert_eq!(
            verifying_gates(Some(&model), ".ai/repo/skills/a/SKILL.md"),
            ["named"]
        );
        assert_eq!(
            verifying_gates(Some(&model), ".ai/repo/ci/gates.yaml"),
            ["named", "unnamed"]
        );
        assert!(verifying_gates(None, ".ai/repo/skills/a/SKILL.md").is_empty());
    }

    fn skill_object(path: &str, identity: &str, metadata: Value) -> crate::model::Object {
        crate::model::Object {
            kind: SKILL.into(),
            identity: identity.into(),
            uri: format!("majordomus://skill/{identity}"),
            title: None,
            description: None,
            metadata,
            body: String::new(),
            content: String::new(),
            media_type: "text/markdown",
            provenance: crate::model::Provenance {
                path: path.into(),
                directory: path.rsplit_once('/').unwrap().0.into(),
                source_class: "skill".into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    /// A skill the index holds with an error against it is invalid and the error is its
    /// finding; its id and title fall back to its identity when the file declares neither.
    /// A skill file the index refused is listed as invalid too, under the skills directory
    /// the layout implies when the manifest names none; an error elsewhere, or on a file
    /// beside a skill, or on no file at all, is no skill.
    #[test]
    fn a_contract_error_makes_a_skill_invalid_and_nothing_else_is_a_skill() {
        use crate::model::Diagnostic;
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let mut index = repo.index().unwrap();
        index.repository.sections.remove("skills");
        let plain = ".ai/repo/skills/plain/SKILL.md";
        index.objects.push(skill_object(
            plain,
            "plain",
            serde_json::json!({ "status": "active" }),
        ));
        for (path, message) in [
            (Some(plain), "a field is wrong"),
            (Some(".ai/repo/skills/refused/SKILL.md"), "refused outright"),
            (Some(".ai/repo/skills/refused/notes.md"), "beside a skill"),
            (Some(".ai/repo/skills/deep/er/SKILL.md"), "too deep"),
            (Some(".ai/repo/rules/x.md"), "elsewhere"),
            (None, "no file"),
        ] {
            index.diagnostics.push(Diagnostic::error(
                "schema",
                path.map(str::to_string),
                message,
            ));
        }

        let skills = Skills::build(&index, &Ledger::empty());
        let ids: Vec<&str> = skills.skills.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["plain", "refused"]);

        let s = skills.skill("plain").unwrap();
        assert_eq!(s.standing, SkillStanding::Invalid);
        assert!(!s.valid);
        assert_eq!(s.title, "plain", "no title: the id stands in");
        let contract: Vec<&SkillFinding> =
            s.findings.iter().filter(|f| f.code == "contract").collect();
        assert_eq!(contract.len(), 1);
        assert_eq!(contract[0].message, "schema: a field is wrong");
        assert_eq!(contract[0].reproduce, "majordomus skills check");
        assert_eq!(contract[0].level, SkillFindingLevel::Fail);

        assert_eq!(
            skills.skill("refused").unwrap().standing,
            SkillStanding::Invalid
        );
    }

    #[test]
    fn surfaces_follow_the_manifest_not_a_fixed_layout() {
        let mut sections = BTreeMap::new();
        sections.insert("workflows".to_string(), "layer/flows".to_string());
        let s = invocation_surfaces(&sections, "layer/skills");
        assert!(s.contains(&"layer/flows/**".to_string()));
        assert!(s.contains(&"layer/providers/**".to_string()));
        assert!(!s.iter().any(|p| p.starts_with(".ai/")));
    }
}
