//! The completion policy: `share/completion.yaml`, read as the one definition of done.
//!
//! # Why the definition of done is data
//!
//! Before this file, "done" was stated in four places that could not disagree loudly: the
//! policy's `finish_requires` selected the shell validators, `share/obligations.yaml` named
//! the tokens a task owes, `.ai/repo/ci/gates.yaml` named the gates, and `gates::done`
//! carried nineteen questions as a Rust literal. Each was right about its own half and the
//! only reconciliation was a reader's. The provider bootstraps (AGENTS.md, CLAUDE.md)
//! restated the same thing a fifth time, in prose, by hand.
//!
//! The policy names the questions and the *source* each one is answered from; the sources
//! are the three older declarations. Nothing is judged here — this module reads the
//! declaration, resolves every source it names against the vocabulary and the gate model,
//! and refuses a policy that names something that does not exist. The judgement is
//! [`super::done`]'s; the stage is [`super::stage`]'s; the projections (the report, the
//! bootstrap fragment, the site) are theirs.
//!
//! ```
//! use majordomus_cli::gates::{CompletionPolicy, QuestionSource};
//!
//! let text = "\
//! version: 1
//! stages:
//!   - id: build
//!     title: Build
//!     summary: The work exists.
//!   - id: ship
//!     title: Ship
//!     summary: It is out.
//! questions:
//!   - id: committed
//!     stage: build
//!     question: Is it committed?
//!     source: obligation:commit
//!     remediation: git commit
//!   - id: ci
//!     stage: ship
//!     question: Did every gate pass?
//!     source: gates
//!     remediation: majordomus evidence --run-gates
//! ";
//! let policy = CompletionPolicy::parse(text, "share/completion.yaml").unwrap();
//! assert_eq!(policy.stages.len(), 2);
//! assert_eq!(policy.questions[0].source, "obligation:commit", "carried as the file states it");
//! assert_eq!(policy.questions[0].kind(), QuestionSource::Obligation("commit".into()));
//! assert_eq!(policy.questions[1].kind(), QuestionSource::Gates);
//!
//! // a token the vocabulary lacks is a defect of the distribution, named by question
//! let problems = policy.validate(&["push".into()]);
//! assert!(problems.iter().any(|p| p.contains("committed") && p.contains("commit")));
//! // a gate the repository never declares is not a problem; it is a question never asked
//! assert!(policy.validate(&["commit".into()]).is_empty());
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::metadata::yaml;

/// The file, relative to the distribution's `share/` directory.
pub const POLICY_FILE: &str = "completion.yaml";

/// One stage of the lifecycle, in the order the policy states them. A stage carries no
/// question of its own: the questions name it by `id`, and
/// [`CompletionPolicy::questions_of`] gathers them back, so a stage is a heading over
/// whatever the policy hangs under it.
///
/// ```
/// use majordomus_cli::gates::StageDecl;
///
/// let stage = StageDecl {
///     id: "commit".into(),
///     title: "Commit and push".into(),
///     summary: "The work is in the branch's history.".into(),
/// };
/// // the same shape the file carries, so a report and the policy agree byte for byte
/// let json = serde_json::to_value(&stage).unwrap();
/// assert_eq!(json["id"], "commit");
/// assert_eq!(serde_json::from_value::<StageDecl>(json).unwrap(), stage);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StageDecl {
    /// The stage's identity, stable across reports.
    pub id: String,
    /// The stage as a heading.
    pub title: String,
    /// What it means for the stage to be complete.
    pub summary: String,
}

/// Where a question's answer is taken from.
///
/// ```
/// use majordomus_cli::gates::QuestionSource;
/// assert_eq!(QuestionSource::parse("obligation:tests").unwrap(), QuestionSource::Obligation("tests".into()));
/// assert_eq!(QuestionSource::parse("gate:release-check").unwrap(), QuestionSource::Gate("release-check".into()));
/// assert_eq!(QuestionSource::parse("gates").unwrap(), QuestionSource::Gates);
/// assert_eq!(QuestionSource::parse("release:impact").unwrap(), QuestionSource::ReleaseImpact);
/// assert!(QuestionSource::parse("magic").is_err(), "a source nothing answers is refused");
/// assert_eq!(QuestionSource::Elsewhere("x y".into()).to_string(), "elsewhere:x y");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "name", rename_all = "kebab-case")]
pub enum QuestionSource {
    /// A token of `share/obligations.yaml`, judged by `obligations.closure`.
    Obligation(String),
    /// The aggregate of every gate the change selects.
    Gates,
    /// One gate of the CI model, by id.
    Gate(String),
    /// The change set itself: whether a test path is among the touched files.
    ChangeTestPath,
    /// The structural version analysis, `release.analysis`.
    ReleaseImpact,
    /// The task record's `issue` field, resolved against the plan.
    TaskIssue,
    /// A continuation record for this task.
    SessionHandover,
    /// Nothing reachable from the report; the command that answers it.
    Elsewhere(String),
}

impl QuestionSource {
    /// Parse the `source:` field of a question. A named form (`obligation:`, `gate:`,
    /// `elsewhere:`) carries what follows the colon with its whitespace trimmed and must
    /// name something; a bare word is one of the fixed sources; anything else is refused
    /// with the list of what would have been accepted, so the fix is one edit of the file.
    ///
    /// ```
    /// use majordomus_cli::gates::QuestionSource;
    ///
    /// let parsed = QuestionSource::parse(" gate: release-check ").unwrap();
    /// assert_eq!(parsed, QuestionSource::Gate("release-check".into()));
    /// assert_eq!(QuestionSource::parse("task:issue").unwrap(), QuestionSource::TaskIssue);
    /// assert!(QuestionSource::parse("obligation:").unwrap_err().contains("names nothing"));
    /// assert!(QuestionSource::parse("nope").unwrap_err().contains("obligation:<token>"));
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if let Some(rest) = text.strip_prefix("obligation:") {
            return Self::named(rest, "obligation").map(Self::Obligation);
        }
        if let Some(rest) = text.strip_prefix("gate:") {
            return Self::named(rest, "gate").map(Self::Gate);
        }
        if let Some(rest) = text.strip_prefix("elsewhere:") {
            return Self::named(rest, "elsewhere").map(Self::Elsewhere);
        }
        match text {
            "gates" => Ok(Self::Gates),
            "change:test-path" => Ok(Self::ChangeTestPath),
            "release:impact" => Ok(Self::ReleaseImpact),
            "task:issue" => Ok(Self::TaskIssue),
            "session:handover" => Ok(Self::SessionHandover),
            other => Err(format!(
                "source '{other}' is not one this executable answers (obligation:<token>, \
                 gates, gate:<id>, change:test-path, release:impact, task:issue, \
                 session:handover, elsewhere:<command>)"
            )),
        }
    }

    fn named(rest: &str, kind: &str) -> Result<String, String> {
        let rest = rest.trim();
        if rest.is_empty() {
            return Err(format!("source '{kind}:' names nothing"));
        }
        Ok(rest.to_string())
    }
}

impl std::fmt::Display for QuestionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Obligation(t) => write!(f, "obligation:{t}"),
            Self::Gates => write!(f, "gates"),
            Self::Gate(g) => write!(f, "gate:{g}"),
            Self::ChangeTestPath => write!(f, "change:test-path"),
            Self::ReleaseImpact => write!(f, "release:impact"),
            Self::TaskIssue => write!(f, "task:issue"),
            Self::SessionHandover => write!(f, "session:handover"),
            Self::Elsewhere(c) => write!(f, "elsewhere:{c}"),
        }
    }
}

/// One question of the policy, as declared.
///
/// `source` is carried as the text the file states (`obligation:tests`, `gates`,
/// `release:impact`), never as a structure: every projection — the report, the site's
/// dataset written with or without the executable, the bootstrap fragment — then states the
/// same bytes for the same question, and a reader compares the two by eye. [`Self::kind`]
/// parses it; [`CompletionPolicy::parse`] has already refused a text nothing answers.
///
/// ```
/// use majordomus_cli::gates::{QuestionDecl, QuestionSource};
///
/// let decl = QuestionDecl {
///     id: "pushed".into(),
///     stage: "commit".into(),
///     question: "Has the commit reached the remote?".into(),
///     source: "obligation:push".into(),
///     remediation: "git push".into(),
/// };
/// assert_eq!(decl.kind(), QuestionSource::Obligation("push".into()));
/// // the source is carried as text, and serialises as the text the file stated
/// assert_eq!(serde_json::to_value(&decl).unwrap()["source"], "obligation:push");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QuestionDecl {
    /// The question's identity, stable across reports.
    pub id: String,
    /// The stage it belongs to.
    pub stage: String,
    /// The question, as a person would ask it.
    pub question: String,
    /// What answers it, as the policy states it.
    pub source: String,
    /// What would settle it, as a command.
    pub remediation: String,
}

impl QuestionDecl {
    /// The source, parsed. A text the parser refuses — impossible for a policy that came
    /// through [`CompletionPolicy::parse`] — is answered by nothing, which is what
    /// [`QuestionSource::Elsewhere`] means.
    ///
    /// ```
    /// use majordomus_cli::gates::{QuestionDecl, QuestionSource};
    ///
    /// let mut decl = QuestionDecl {
    ///     id: "ci".into(),
    ///     stage: "gates".into(),
    ///     question: "Did every gate pass?".into(),
    ///     source: "gates".into(),
    ///     remediation: "majordomus evidence --run-gates".into(),
    /// };
    /// assert_eq!(decl.kind(), QuestionSource::Gates);
    /// // a text the parser refuses is answered by nothing, and says so rather than failing
    /// decl.source = "oracle".into();
    /// assert_eq!(decl.kind(), QuestionSource::Elsewhere("oracle".into()));
    /// ```
    pub fn kind(&self) -> QuestionSource {
        QuestionSource::parse(&self.source)
            .unwrap_or_else(|_| QuestionSource::Elsewhere(self.source.clone()))
    }
}

/// The policy, as read: the lifecycle's stages in order and every question with the
/// source that answers it. It is the one definition of done, and every projection of that
/// definition — the completion report, the bootstrap fragment, the site's dataset — is
/// derived from a value of this type rather than restated beside it. A value only exists
/// once [`Self::parse`] has accepted the file, so a policy in hand is structurally sound:
/// every question names a declared stage and a source this executable answers.
///
/// ```
/// use majordomus_cli::gates::CompletionPolicy;
///
/// let text = "\
/// version: 1
/// stages:
///   - id: build
///     title: Build
///     summary: The work exists.
///   - id: ship
///     title: Ship
///     summary: It is out.
/// questions:
///   - id: committed
///     stage: build
///     question: Is it committed?
///     source: obligation:commit
///     remediation: git commit
///   - id: changelog
///     stage: ship
///     question: Is the release record current?
///     source: gate:release-check
///     remediation: scripts/ci/release-check
/// ";
/// let policy = CompletionPolicy::parse(text, "inline").unwrap();
/// assert_eq!(policy.version, 1);
/// assert_eq!(policy.source, "inline");
/// assert_eq!(policy.stages[1].title, "Ship");
/// assert_eq!(policy.questions_of("build").count(), 1);
/// assert_eq!(policy.bootstrap_fragment(), "- Build: committed\n- Ship: changelog\n");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CompletionPolicy {
    /// The format version the file states.
    pub version: u32,
    /// Where it was read from.
    pub source: String,
    /// The lifecycle, in order.
    pub stages: Vec<StageDecl>,
    /// Every question, in declaration order.
    pub questions: Vec<QuestionDecl>,
}

#[derive(Deserialize)]
struct RawQuestion {
    id: String,
    stage: String,
    question: String,
    source: String,
    remediation: String,
}

#[derive(Deserialize)]
struct RawFile {
    version: u32,
    stages: Vec<StageDecl>,
    questions: Vec<RawQuestion>,
}

impl CompletionPolicy {
    /// Parse the policy text. `source` is what the report names as where it came from,
    /// and every refusal starts with it. A file of another version, a question whose
    /// source nothing answers, a stage declared twice or never declared, an empty question
    /// and a policy with no question at all are all refused here, so that no later reader
    /// meets them.
    ///
    /// ```
    /// use majordomus_cli::gates::CompletionPolicy;
    ///
    /// let text = "\
    /// version: 1
    /// stages:
    ///   - id: build
    ///     title: Build
    ///     summary: The work exists.
    /// questions:
    ///   - id: committed
    ///     stage: build
    ///     question: Is it committed?
    ///     source: '  obligation:commit  '
    ///     remediation: git commit
    /// ";
    /// let policy = CompletionPolicy::parse(text, "share/completion.yaml").unwrap();
    /// assert_eq!(policy.questions[0].source, "obligation:commit", "trimmed as it is read");
    ///
    /// // a source nothing answers is refused by question, and the refusal names the file
    /// let err = CompletionPolicy::parse(&text.replace("obligation:commit", "oracle"), "t").unwrap_err();
    /// assert!(err.starts_with("t: question 'committed'"), "{err}");
    /// assert!(CompletionPolicy::parse("version: 2\nstages: []\nquestions: []\n", "t")
    ///     .unwrap_err()
    ///     .contains("version 1"));
    /// ```
    pub fn parse(text: &str, source: &str) -> Result<Self, String> {
        let raw: RawFile = yaml::parse_into(text).map_err(|e| format!("{source}: {e}"))?;
        if raw.version != 1 {
            return Err(format!(
                "{source}: the completion policy must be version 1, and this one is {}",
                raw.version
            ));
        }
        let mut questions = Vec::with_capacity(raw.questions.len());
        for q in raw.questions {
            QuestionSource::parse(&q.source)
                .map_err(|e| format!("{source}: question '{}': {e}", q.id))?;
            questions.push(QuestionDecl {
                id: q.id,
                stage: q.stage,
                question: q.question,
                source: q.source.trim().to_string(),
                remediation: q.remediation,
            });
        }
        let policy = Self {
            version: raw.version,
            source: source.to_string(),
            stages: raw.stages,
            questions,
        };
        let structural = policy.structural_problems();
        if !structural.is_empty() {
            return Err(format!("{source}: {}", structural.join("; ")));
        }
        Ok(policy)
    }

    /// Read the policy from a distribution's `share/` directory: the file [`POLICY_FILE`]
    /// under it, parsed with the path as its source. A directory without the file is an
    /// error naming the path, since a distribution without a definition of done is not
    /// one this executable can judge completion against.
    ///
    /// ```
    /// use majordomus_cli::gates::{CompletionPolicy, POLICY_FILE};
    ///
    /// let share = tempfile::tempdir().unwrap();
    /// assert!(CompletionPolicy::load(share.path()).unwrap_err().contains(POLICY_FILE));
    ///
    /// std::fs::write(
    ///     share.path().join(POLICY_FILE),
    ///     "version: 1\nstages:\n  - id: a\n    title: Alpha\n    summary: s\nquestions:\n  - id: q\n    stage: a\n    question: Is it?\n    source: gates\n    remediation: run it\n",
    /// )
    /// .unwrap();
    /// let policy = CompletionPolicy::load(share.path()).unwrap();
    /// assert!(policy.source.ends_with(POLICY_FILE), "the report names where it came from");
    /// assert_eq!(policy.questions[0].id, "q");
    /// ```
    pub fn load(share_dir: &std::path::Path) -> Result<Self, String> {
        let path = share_dir.join(POLICY_FILE);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text, &path.to_string_lossy())
    }

    /// The problems a policy has on its own: a duplicate id, a question naming a stage the
    /// policy does not declare, a stage with no question, an empty question.
    fn structural_problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for s in &self.stages {
            if !seen.insert(s.id.as_str()) {
                out.push(format!("stage '{}' is declared twice", s.id));
            }
        }
        let mut qseen = std::collections::BTreeSet::new();
        for q in &self.questions {
            if !qseen.insert(q.id.as_str()) {
                out.push(format!("question '{}' is declared twice", q.id));
            }
            if !seen.contains(q.stage.as_str()) {
                out.push(format!(
                    "question '{}' names stage '{}', which the policy does not declare",
                    q.id, q.stage
                ));
            }
            if q.question.trim().is_empty() || q.remediation.trim().is_empty() {
                out.push(format!(
                    "question '{}' must carry a question and a remediation",
                    q.id
                ));
            }
        }
        if self.questions.is_empty() {
            out.push("the policy declares no question".into());
        }
        out
    }

    /// The problems a policy has against the vocabulary it is shipped beside: an
    /// obligation token `share/obligations.yaml` does not declare. Both files travel
    /// together, so a token one names and the other lacks is a defect of the distribution,
    /// and each problem names the question so the fix is one edit.
    ///
    /// A gate the *repository's* CI model does not declare is not a problem: the model is
    /// the repository's, the policy is shipped, and a question a repository never asks is
    /// answered `exempt` by name in the report. [`Self::unanswered_gates`] lists those.
    ///
    /// ```
    /// use majordomus_cli::gates::CompletionPolicy;
    ///
    /// let text = "\
    /// version: 1
    /// stages:
    ///   - id: build
    ///     title: Build
    ///     summary: The work exists.
    ///   - id: ship
    ///     title: Ship
    ///     summary: It is out.
    /// questions:
    ///   - id: committed
    ///     stage: build
    ///     question: Is it committed?
    ///     source: obligation:commit
    ///     remediation: git commit
    ///   - id: changelog
    ///     stage: ship
    ///     question: Is the release record current?
    ///     source: gate:release-check
    ///     remediation: scripts/ci/release-check
    /// ";
    /// let policy = CompletionPolicy::parse(text, "t").unwrap();
    /// assert!(policy.validate(&["commit".into(), "push".into()]).is_empty());
    ///
    /// let problems = policy.validate(&["push".into()]);
    /// assert_eq!(problems.len(), 1);
    /// assert!(problems[0].contains("question 'committed'") && problems[0].contains("'commit'"));
    /// ```
    pub fn validate(&self, obligations: &[String]) -> Vec<String> {
        let mut out = self.structural_problems();
        for q in &self.questions {
            if let QuestionSource::Obligation(token) = q.kind() {
                if !obligations.contains(&token) {
                    out.push(format!(
                        "question '{}' is answered by obligation '{token}', which \
                         share/obligations.yaml does not declare",
                        q.id
                    ));
                }
            }
        }
        out
    }

    /// The questions answered by a gate this repository's CI model does not declare, as
    /// `question:gate` pairs: what the policy would ask and the repository never does.
    ///
    /// ```
    /// use majordomus_cli::gates::CompletionPolicy;
    ///
    /// let text = "\
    /// version: 1
    /// stages:
    ///   - id: build
    ///     title: Build
    ///     summary: The work exists.
    ///   - id: ship
    ///     title: Ship
    ///     summary: It is out.
    /// questions:
    ///   - id: committed
    ///     stage: build
    ///     question: Is it committed?
    ///     source: obligation:commit
    ///     remediation: git commit
    ///   - id: changelog
    ///     stage: ship
    ///     question: Is the release record current?
    ///     source: gate:release-check
    ///     remediation: scripts/ci/release-check
    /// ";
    /// let policy = CompletionPolicy::parse(text, "t").unwrap();
    /// assert!(policy.unanswered_gates(&["release-check".into()]).is_empty());
    /// // a repository whose model declares no such gate is told which question it never asks
    /// assert_eq!(policy.unanswered_gates(&["rust-check".into()]), ["changelog:release-check"]);
    /// ```
    pub fn unanswered_gates(&self, gates: &[String]) -> Vec<String> {
        self.questions
            .iter()
            .filter_map(|q| match q.kind() {
                QuestionSource::Gate(g) if !gates.contains(&g) => Some(format!("{}:{g}", q.id)),
                _ => None,
            })
            .collect()
    }

    /// The questions of one stage, in declaration order. A stage the policy declares and
    /// no question names yields nothing, and so does a stage id the policy never declared;
    /// the difference is the stage list's to state, not this iterator's.
    ///
    /// ```
    /// use majordomus_cli::gates::CompletionPolicy;
    ///
    /// let text = "\
    /// version: 1
    /// stages:
    ///   - id: build
    ///     title: Build
    ///     summary: The work exists.
    ///   - id: ship
    ///     title: Ship
    ///     summary: It is out.
    /// questions:
    ///   - id: committed
    ///     stage: build
    ///     question: Is it committed?
    ///     source: obligation:commit
    ///     remediation: git commit
    ///   - id: changelog
    ///     stage: ship
    ///     question: Is the release record current?
    ///     source: gate:release-check
    ///     remediation: scripts/ci/release-check
    /// ";
    /// let policy = CompletionPolicy::parse(text, "t").unwrap();
    /// let ids: Vec<&str> = policy.questions_of("ship").map(|q| q.id.as_str()).collect();
    /// assert_eq!(ids, ["changelog"]);
    /// assert_eq!(policy.questions_of("nowhere").count(), 0);
    /// ```
    pub fn questions_of<'a>(
        &'a self,
        stage: &'a str,
    ) -> impl Iterator<Item = &'a QuestionDecl> + 'a {
        self.questions.iter().filter(move |q| q.stage == stage)
    }

    /// The bootstrap fragment: the definition of done as the generated section of a
    /// provider instruction file carries it. One line per stage, its questions after a
    /// colon, so the always-loaded file spends a dozen lines on it and not a page. Plain
    /// list items on purpose: `doctor` refuses a bootstrap carrying rule bullets of its
    /// own (`- **...**`), and this is a projection of the policy, not a rule corpus.
    ///
    /// The shell tool renders the same fragment from the same file (`mj_build_fragments`
    /// in `lib/update.sh`), byte for byte; `generate --check` is what notices if the two
    /// ever differ, because the stamp on the bootstrap would stop matching.
    ///
    /// ```
    /// use majordomus_cli::gates::CompletionPolicy;
    /// let p = CompletionPolicy::parse("version: 1\nstages:\n  - id: a\n    title: Alpha\n    summary: s\nquestions:\n  - id: q\n    stage: a\n    question: Is it?\n    source: gates\n    remediation: run it\n", "t").unwrap();
    /// assert_eq!(p.bootstrap_fragment(), "- Alpha: q\n");
    /// ```
    pub fn bootstrap_fragment(&self) -> String {
        let mut out = String::new();
        for s in &self.stages {
            let ids: Vec<&str> = self.questions_of(&s.id).map(|q| q.id.as_str()).collect();
            if ids.is_empty() {
                continue;
            }
            out.push_str(&format!("- {}: {}\n", s.title, ids.join(", ")));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> CompletionPolicy {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        CompletionPolicy::load(&root).expect("the shipped policy parses")
    }

    #[test]
    fn the_shipped_policy_parses_and_every_stage_has_a_question() {
        let p = shipped();
        assert_eq!(p.version, 1);
        // every stage but the first (context) is answered by at least one question; the
        // first is held by the task record's existence, which `present` already states
        for s in p.stages.iter().skip(1) {
            assert!(
                p.questions_of(&s.id).next().is_some(),
                "stage {} has no question",
                s.id
            );
        }
    }

    #[test]
    fn the_shipped_policy_resolves_against_the_shipped_vocabulary_and_this_repository_s_model() {
        let p = shipped();
        let share = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        let text = std::fs::read_to_string(share.join("obligations.yaml")).unwrap();
        let v = yaml::parse_mapping(&text).unwrap();
        let tokens: Vec<String> = v["obligations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["id"].as_str().unwrap().to_string())
            .collect();
        let model =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.ai/repo/ci/gates.yaml");
        let gates: Vec<String> = yaml::parse_mapping(&std::fs::read_to_string(model).unwrap())
            .unwrap()["gates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["id"].as_str().unwrap().to_string())
            .collect();
        let problems = p.validate(&tokens);
        assert!(problems.is_empty(), "{problems:?}");
        let unanswered = p.unanswered_gates(&gates);
        assert!(
            unanswered.is_empty(),
            "this repository's model answers every gate question: {unanswered:?}"
        );
        assert_eq!(
            p.unanswered_gates(&[]),
            ["parity:projection-closure", "changelog:release-check"]
        );
    }

    #[test]
    fn a_policy_naming_an_unknown_stage_or_a_duplicate_is_refused() {
        let text = "version: 1\nstages:\n  - id: a\n    title: A\n    summary: s\nquestions:\n  - id: q\n    stage: b\n    question: x\n    source: gates\n    remediation: r\n";
        let err = CompletionPolicy::parse(text, "t").unwrap_err();
        assert!(err.contains("stage 'b'"), "{err}");
        let dup = "version: 1\nstages:\n  - id: a\n    title: A\n    summary: s\nquestions:\n  - id: q\n    stage: a\n    question: x\n    source: gates\n    remediation: r\n  - id: q\n    stage: a\n    question: y\n    source: gates\n    remediation: r\n";
        assert!(CompletionPolicy::parse(dup, "t")
            .unwrap_err()
            .contains("declared twice"));
        let v2 = "version: 2\nstages: []\nquestions: []\n";
        assert!(CompletionPolicy::parse(v2, "t")
            .unwrap_err()
            .contains("version 1"));
    }

    #[test]
    fn the_fragment_is_deterministic_and_names_every_stage_with_a_question() {
        let p = shipped();
        let a = p.bootstrap_fragment();
        let b = p.bootstrap_fragment();
        assert_eq!(a, b);
        assert!(a.contains("- Live verification: "));
        assert!(a
            .lines()
            .all(|l| l.starts_with("- ") && !l.starts_with("- **")));
    }
}
