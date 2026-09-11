//! The `rules` module: from a rule to what proves it, and back.
//!
//! The rules section declares what this repository refuses; [`crate::rules`] decides what
//! the repository can actually show for each one. These capabilities are the one
//! declaration of that reading, and every surface — the command line, the HTTP API and its
//! OpenAPI operation, the MCP tool an agent reads, the Cockpit page, the generated
//! reference — is derived from it. There is no second rule inventory behind any of them,
//! which is the property the whole exercise exists to buy: a rule added to the rules
//! section tomorrow appears on all of them without a line being edited anywhere.
//!
//! # Validate and verify are different questions
//!
//! `rules.verify` asks whether the *rule* is in order: is what it names in the tree, does a
//! runner own it, was it ever run, did that run pass, is the run older than its subject.
//! Whether the *repository* currently satisfies a rule is the dispatcher's question, and
//! the portable layer's `majordomus doctrine` is where it is asked — one rule, one
//! validator, run against the working tree. Collapsing the two is how "the rule is fine"
//! and "the repository is fine" become one green badge that means neither.
//!
//! # Both directions
//!
//! A relation that can only be walked one way is half a relation. `rules.show` answers
//! "what proves this rule"; `rules.proves` answers "what does this test prove", listing
//! every rule that names it. They read the same derivation, so the two answers cannot
//! disagree — and the second is the one that says whether deleting a case would leave a
//! rule unenforced, which is the question nobody could ask before.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, CliExposure, Exposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::evidence::{Ledger, TestId};
use crate::rules::{self, Class, RuleProof, RuleState, RulesReport, RULES_URI};
use crate::{capability, module};

use super::{get, mcp};

// ---------------------------------------------------------------- inputs

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which part of the corpus to answer for.
pub struct RulesReportInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only rules in this proof state (`proven`, `inputs_unchanged`, `stale`, `failing`,
    /// `not_run`, `unrunnable`, `dangling`, `unproven`). Absent: every rule.
    pub state: Option<RuleState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only rules of this class (`blocking`, `advisory`). Absent: every rule.
    pub class: Option<Class>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only rules of this namespace — `project` for this repository's own, `majordomus` for
    /// the vendored package. Absent: both.
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    /// Only the rules whose declared class the proof does not support, with the findings.
    /// The tallies still count the whole corpus, so a filtered answer never misreports how
    /// much of it was examined.
    pub findings_only: bool,
}

impl BenchmarkCases for RulesReportInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", RulesReportInput::default()),
            NamedCase::new(
                "findings",
                RulesReportInput {
                    findings_only: true,
                    ..Default::default()
                },
            ),
            NamedCase::new(
                "blocking",
                RulesReportInput {
                    class: Some(Class::Blocking),
                    ..Default::default()
                },
            ),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// One rule of the corpus.
pub struct RuleInput {
    /// The rule id, with or without its version: `project.scope-is-declared`, or the
    /// identity the index holds.
    pub rule: String,
}

impl BenchmarkCases for RuleInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // the corpus is read from the index, so the case names a rule that exists in
        // whatever repository the benchmark runs against rather than one written down here
        let first = ctx
            .index
            .objects
            .iter()
            .find(|o| o.kind == "rule")
            .map(|o| o.identity.clone())
            .unwrap_or_else(|| "project.scope-is-declared@1".into());
        vec![NamedCase::new("first", RuleInput { rule: first })]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// One test, by the path a rule names it with or by its canonical identity.
pub struct RuleTestInput {
    /// `test/cases/07_scope.sh`, or `suite:07_scope`, or `crate:product`.
    pub test: String,
}

impl BenchmarkCases for RuleTestInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "suite",
            RuleTestInput {
                test: "test/cases/125_rule_proof.sh".into(),
            },
        )]
    }
}

// ---------------------------------------------------------------- outputs

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One rule, with everything that proves it and everything that depends on it.
pub struct RuleDetail {
    /// The rule, joined to the tree and the ledger.
    pub proof: RuleProof,
    /// The rules this one depends on, with the state each is in — a rule whose dependency
    /// is unproven is proven only as far as that dependency is.
    pub depends_on: Vec<RuleSummary>,
    /// The rules that depend on this one.
    pub required_by: Vec<RuleSummary>,
    /// What would have to be true for this rule to reach `proven`, in words, or nothing
    /// when it is there already.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// A rule named from somewhere else, with just enough to decide whether to follow it.
pub struct RuleSummary {
    /// The rule id.
    pub id: String,
    /// The title.
    pub title: String,
    /// How strongly it binds.
    pub class: Class,
    /// What the repository can say about its proof.
    pub state: RuleState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// What one test proves: the reverse of [`RuleDetail`].
pub struct TestSubjects {
    /// The canonical test identity, when a runner owns the path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    /// The path as given.
    pub path: String,
    /// Whether that path is in the tree.
    pub present: bool,
    /// The command that runs it again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reproduce: Option<String>,
    /// Every rule that names this test, with the state it is in.
    pub proves: Vec<RuleSummary>,
    /// Every rule that would lose its only proof if this test were deleted. This is the
    /// question the one-way relation could never answer.
    pub sole_proof_of: Vec<String>,
}

// ---------------------------------------------------------------- handlers

fn root_of(ctx: &Context) -> std::path::PathBuf {
    std::path::PathBuf::from(&ctx.index.repository.root)
}

fn internal(e: crate::error::Error) -> CapabilityError {
    CapabilityError::Internal(e.to_string())
}

/// The whole joined report, read fresh: the ledger is a file that changes outside this
/// process, and a cached answer would be the stale evidence this module exists to name.
fn full_report(ctx: &Context) -> Result<RulesReport, CapabilityError> {
    let ledger = Ledger::load(&root_of(ctx)).map_err(internal)?;
    Ok(rules::report(&ctx.index, &ledger))
}

fn summary(p: &RuleProof) -> RuleSummary {
    RuleSummary {
        id: p.rule.id.clone(),
        title: p.rule.title.clone(),
        class: p.rule.class,
        state: p.state,
    }
}

/// What is missing, in the words a reader can act on. One sentence per state, derived from
/// the rule in hand rather than from a template, because "governance failed" is not a
/// diagnostic.
fn missing_for(p: &RuleProof) -> Option<String> {
    let named = |paths: Vec<&str>| paths.join(", ");
    match p.state {
        RuleState::Proven => None,
        RuleState::InputsUnchanged => Some(
            "Run what it names against this commit and record the run: \
             the latest run passed but was measured against an earlier tree."
                .into(),
        ),
        RuleState::Stale => Some(format!(
            "Run what it names again and record the run: {} passed against an older tree than \
             the one it is about.",
            named(p.tests.iter().map(|t| t.path.as_str()).collect())
        )),
        RuleState::Gated => Some(
            "Record a verdict for the gate that refuses violations of it: the mechanism is \
             wired and nothing here says what its last run decided."
                .into(),
        ),
        RuleState::Failing => Some(format!(
            "Fix the behaviour or the case: the latest recorded run of {} did not pass.",
            named(
                p.tests
                    .iter()
                    .filter(|t| !t.state.passing())
                    .map(|t| t.path.as_str())
                    .collect()
            )
        )),
        RuleState::Reviewed => Some(
            "Nothing here is broken: this rule declares that no program can express it and \
             says why. If that ever stops being true, name the case and the declaration \
             falls away on its own."
                .into(),
        ),
        RuleState::NotRun => Some(format!(
            "Run and record: {}",
            p.tests
                .iter()
                .filter_map(|t| t.reproduce.clone())
                .collect::<Vec<_>>()
                .join("; ")
        )),
        RuleState::Unrunnable => Some(format!(
            "Name a path a runner drives — a case under test/cases/ or an integration test of \
             the crate. {} is driven by neither.",
            named(
                p.tests
                    .iter()
                    .filter(|t| t.test.is_none())
                    .map(|t| t.path.as_str())
                    .collect()
            )
        )),
        RuleState::Dangling => Some(format!(
            "Restore or rename what it names: {} is not in the tree, so this rule reads as \
             proven and is not.",
            named(
                p.tests
                    .iter()
                    .filter(|t| !t.present)
                    .map(|t| t.path.as_str())
                    .chain(
                        p.validator
                            .iter()
                            .filter(|v| !v.present)
                            .map(|v| v.function.as_str())
                    )
                    .collect()
            )
        )),
        RuleState::Unproven => Some(
            "Name the behavioural case that proves it in its x-majordomus block: \
             tests: [test/cases/NN_name.sh]."
                .into(),
        ),
    }
}

fn report(ctx: &Context, input: RulesReportInput) -> Result<RulesReport, CapabilityError> {
    let mut r = full_report(ctx)?;
    // the tallies are taken before the filter: a filtered answer says how much of the
    // corpus it looked at, never how much it returned
    if input.findings_only {
        let named: std::collections::BTreeSet<&str> =
            r.findings.iter().map(|f| f.rule.as_str()).collect();
        r.rules.retain(|p| named.contains(p.rule.id.as_str()));
    }
    if let Some(state) = input.state {
        r.rules.retain(|p| p.state == state);
    }
    if let Some(class) = input.class {
        r.rules.retain(|p| p.rule.class == class);
    }
    if let Some(ns) = &input.namespace {
        r.rules.retain(|p| &p.rule.namespace == ns);
    }
    Ok(r)
}

fn show(ctx: &Context, input: RuleInput) -> Result<RuleDetail, CapabilityError> {
    let r = full_report(ctx)?;
    // a rule is named by its id or by the identity the index holds; a client that has one
    // should not have to know the other's spelling
    let wanted = input.rule.trim();
    let Some(proof) = r
        .rules
        .iter()
        .find(|p| p.rule.id == wanted || p.rule.identity == wanted)
        .cloned()
    else {
        return Err(CapabilityError::NotFound(format!(
            "`{wanted}` is not a rule of this repository; `rules show` lists what is"
        )));
    };
    let by_id = |id: &String| r.rules.iter().find(|p| &p.rule.id == id).map(summary);
    Ok(RuleDetail {
        depends_on: proof.rule.depends_on.iter().filter_map(by_id).collect(),
        required_by: proof.required_by.iter().filter_map(by_id).collect(),
        missing: missing_for(&proof),
        proof,
    })
}

fn proves(ctx: &Context, input: RuleTestInput) -> Result<TestSubjects, CapabilityError> {
    let r = full_report(ctx)?;
    let given = input.test.trim();
    // a path and an identity both name the same test
    let id = TestId::of(given).or_else(|| {
        let (prefix, name) = given.split_once(':')?;
        let runner = match prefix {
            "suite" => crate::evidence::Runner::Suite,
            "crate" => crate::evidence::Runner::Crate,
            _ => return None,
        };
        Some(TestId {
            runner,
            name: name.to_string(),
        })
    });
    let key = id.as_ref().map(TestId::as_string);
    let path = id
        .as_ref()
        .map(TestId::source)
        .unwrap_or_else(|| given.to_string());

    let mut proves = Vec::new();
    let mut sole = Vec::new();
    for p in &r.rules {
        let names_it = p.tests.iter().any(|t| {
            t.path == path || (key.is_some() && t.test == key) || t.path == given
        });
        if !names_it {
            continue;
        }
        proves.push(summary(p));
        // the question the one-way relation could never answer: would deleting this leave
        // the rule with nothing?
        if p.tests.len() == 1 && p.validator.is_none() {
            sole.push(p.rule.id.clone());
        }
    }
    if proves.is_empty() && id.is_none() {
        return Err(CapabilityError::InvalidInput(format!(
            "`{given}` names no test and no rule names it: give `suite:<case>`, `crate:<binary>`, \
             or the path of a case under test/cases/ or an integration test of the crate"
        )));
    }
    Ok(TestSubjects {
        present: root_of(ctx).join(&path).exists(),
        reproduce: id.as_ref().map(TestId::reproduce),
        test: key,
        path,
        proves,
        sole_proof_of: sole,
    })
}

// ---------------------------------------------------------------- the module

/// The `rules` module: rules joined to what proves them.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "rules",
        title: "Rules",
        description: "What this repository refuses, and what it can actually show for each refusal. A rule declares a class — blocking, where a gate refuses the work, or advisory, where a reader is expected to have read it — and an enforcement block naming either the validator the dispatcher calls or the behavioural cases that prove it. This module joins that declaration to the tree and to the ledger of recorded runs, and reports, per rule, whether what it names is there, whether a runner owns it, whether it was ever run, whether the run passed, and whether the run is older than what it is about. A blocking rule whose proof is missing, dangling or failing is a finding, because a rule nobody can prove is a rule nobody enforces.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "rules.report",
                title: "Every rule against the proof there is for it",
                description: "The whole rule corpus joined to the tree and the ledger: per rule, its class, its enforcement mode, the validator and the cases it names, whether each is in the tree, the execution behind each, the CI gates that run them, and the sentence explaining how the state was derived. The tallies count the whole corpus even when the answer is filtered, and the findings name every rule whose declared class the proof does not support. Read fresh on every call: the ledger is a file that changes outside this process.",
                input: RulesReportInput,
                output: RulesReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(crate::capability::model::McpExposure {
                        tool: Some("majordomus_rules".into()),
                        resource: Some(McpResource { uri: RULES_URI.into(), name: "rules".into() }),
                    }),
                    http: get("/api/v1/rules"),
                    cli: Some(CliExposure { path: vec!["rules".into(), "report".into()] }),
                },
                tags: ["rules", "governance", "verification", "evidence", "gates"],
                cache: CachePolicy::Disabled,
                handler: report,
            },
            capability! {
                id: "rules.show",
                title: "What proves this rule",
                description: "One rule with its full proof: the validator and the cases it names, the execution behind each, the gates that run them, the rules it depends on and the rules that depend on it — each with the state it is in, because a rule is proven only as far as its dependencies are — and, when it is not proven, the one sentence saying what would have to be true for it to be. A rule this repository does not declare is a not-found rather than an empty answer, because a typo that read as 'this rule has no proof' is the one answer this capability must never give.",
                input: RuleInput,
                output: RuleDetail,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_rule"),
                    http: get("/api/v1/rules/rule"),
                    cli: Some(CliExposure { path: vec!["rules".into(), "show".into()] }),
                },
                tags: ["rules", "governance", "verification", "evidence"],
                cache: CachePolicy::Disabled,
                handler: show,
            },
            capability! {
                id: "rules.proves",
                title: "What this test proves",
                description: "One test — by identity or by the path a rule names it with — with every rule that names it and the rules that would be left with no proof at all if it were deleted. This is the reverse of rules.show and reads the same derivation, so the two directions cannot disagree. It is the question that could not be asked while the relation ran one way only: before deleting or renaming a case, what stops being enforced.",
                input: RuleTestInput,
                output: TestSubjects,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_rule_proves"),
                    http: get("/api/v1/rules/proves"),
                    cli: Some(CliExposure { path: vec!["rules".into(), "proves".into()] }),
                },
                tags: ["rules", "governance", "tests", "verification"],
                cache: CachePolicy::Disabled,
                handler: proves,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; every projection derives from
    /// it. This is the assertion a refactor that dropped an exposure would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "rules");
        let expected: &[(&str, &str, &str, &[&str])] = &[
            (
                "rules.report",
                "majordomus_rules",
                "/api/v1/rules",
                &["rules", "report"],
            ),
            (
                "rules.show",
                "majordomus_rule",
                "/api/v1/rules/rule",
                &["rules", "show"],
            ),
            (
                "rules.proves",
                "majordomus_rule_proves",
                "/api/v1/rules/proves",
                &["rules", "proves"],
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _, _)| *id).collect();
        assert_eq!(ids, want);
        for (e, (id, tool, path, cli)) in m.capabilities.iter().zip(expected) {
            let x = &e.capability.exposure;
            assert_eq!(
                x.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id} lost its MCP tool"
            );
            assert_eq!(
                x.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost its HTTP operation"
            );
            assert_eq!(
                x.cli.as_ref().map(|c| c.path.clone()),
                Some(cli.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
                "{id} lost or renamed its command line"
            );
            assert!(
                !e.capability.cache.is_enabled(),
                "{id} must not cache: the ledger changes outside this process"
            );
            assert!(e.capability.kind.is_read_only(), "{id} writes");
        }
    }

    /// Every capability of this module is a read, reachable from all three surfaces. A rule
    /// is governance: an agent that can read it over MCP and a person who can read it on the
    /// command line must be reading the same thing, and neither may be able to edit it from
    /// there — a rule changed over the network is a rule changed without review.
    #[test]
    fn the_whole_module_is_read_only_and_reachable_everywhere() {
        for e in module().capabilities {
            let id = e.capability.id.as_str().to_string();
            let x = &e.capability.exposure;
            assert!(e.capability.kind.is_read_only(), "{id} writes");
            assert!(x.mcp.is_some(), "{id} is not reachable over MCP");
            assert!(x.http.is_some(), "{id} is not reachable over HTTP");
            let cli = x
                .cli
                .as_ref()
                .unwrap_or_else(|| panic!("{id} has no command line"));
            assert_eq!(cli.path.first().map(String::as_str), Some("rules"), "{id}");
            assert_eq!(cli.path.len(), 2, "{id}");
        }
    }

    /// The whole report is the resource an agent reads without calling a tool, and it is
    /// the one this module names. A URI that drifted from the module's own constant would
    /// leave the resource pointing at nothing.
    #[test]
    fn the_report_is_the_resource() {
        let m = module();
        let r = m
            .capabilities
            .iter()
            .find(|e| e.capability.id.as_str() == "rules.report")
            .expect("the report is declared");
        let res = r
            .capability
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.resource.as_ref())
            .expect("the report is a resource");
        assert_eq!(res.uri, RULES_URI);
        assert_eq!(res.name, "rules");
    }

    /// Every state a rule can be in says what to do about it, except the one that needs
    /// nothing done. A state with no remedy is a dead end for whoever hits it.
    #[test]
    fn every_state_but_proven_says_what_would_fix_it() {
        let base = |state: RuleState| RuleProof {
            rule: crate::rules::RuleDefinition {
                id: "project.x".into(),
                identity: "project.x@1".into(),
                uri: "majordomus://rule/project.x@1".into(),
                version: 1,
                title: "X".into(),
                description: None,
                statement: None,
                status: "active".into(),
                class: Class::Blocking,
                namespace: "project".into(),
                depends_on: vec![],
                tags: vec![],
                path: ".ai/repo/rules/project/x.v1.md".into(),
                enforcement: crate::rules::Enforcement {
                    mode: crate::rules::Mode::Gated,
                    validator: None,
                    category: None,
                    exit_code: None,
                    enforced_by: None,
                    tests: vec!["test/cases/07_scope.sh".into()],
                    reviewed_because: None,
                },
            },
            validator: None,
            tests: vec![crate::rules::TestProof {
                path: "test/cases/07_scope.sh".into(),
                kind: crate::rules::ArtifactKind::Case,
                present: state != RuleState::Dangling,
                test: Some("suite:07_scope".into()),
                state: crate::evidence::ProofState::NotRun,
                meaning: String::new(),
                execution: None,
                reproduce: Some("bash test/run.sh 07_scope".into()),
                gates: vec![],
            }],
            gates: vec![],
            required_by: vec![],
            state,
            meaning: state.meaning().into(),
            satisfied: false,
        };
        assert!(missing_for(&base(RuleState::Proven)).is_none());
        for state in [
            RuleState::InputsUnchanged,
            RuleState::Stale,
            RuleState::Gated,
            RuleState::Reviewed,
            RuleState::Failing,
            RuleState::NotRun,
            RuleState::Unrunnable,
            RuleState::Dangling,
            RuleState::Unproven,
        ] {
            let m = missing_for(&base(state))
                .unwrap_or_else(|| panic!("{} says nothing about what would fix it", state.label()));
            assert!(
                m.len() > 20,
                "{}: `{m}` is not a diagnostic",
                state.label()
            );
        }
    }
}
