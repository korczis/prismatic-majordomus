//! Token economics, behaviourally: what the one calculator says for each state of the
//! evidence, and that every surface — the library, the command line, HTTP, MCP, the Cockpit
//! and the generated report — says the same thing about one fixture.
//!
//! The runs here are synthesised as the runner records them (raw usage and gate verdicts);
//! nothing calls a provider. The honesty cases are the ones the methodology exists for: no
//! observations, context evidence only, a treatment that costs more, failed and incomparable
//! pairs, stale evidence, and the one case where a quantitative claim is allowed.

mod common;

use std::collections::BTreeMap;
use std::io::Write;
use std::process::{Command, Stdio};

use common::{run_in, Fixture, Served};
use majordomus_cli::economics::model::*;
use majordomus_cli::economics::{self, NO_CLAIM};
use serde_json::{json, Value};

const DIR: &str = ".ai/repo/benchmarks/economics";

/// The methodology this repository declares, so the tests hold the real publication rule.
const METHODOLOGY: &str = include_str!("../../../.ai/repo/benchmarks/economics/methodology.yaml");

const CATEGORIES: [&str; 4] = ["bug-fix", "small-feature", "cross-module", "multi-session"];

fn task_yaml(id: &str, category: &str, sessions: usize) -> String {
    let mut s = format!(
        "schema: economics-task/v1\nid: {id}\ntitle: \"{id}\"\ncategory: {category}\ncomplexity: small\nsource: \"synthetic\"\nfixture: fixture\nacceptance: acceptance/{id}\nreference: reference/{id}.patch\nverify: \"true\"\nscope: [src]\nsessions:\n"
    );
    for i in 1..=sessions {
        s.push_str(&format!("  - prompt: prompt-{i}.md\n"));
    }
    s
}

/// A fixture declaring the real methodology, one live suite over `tasks` (id, category) with
/// `reps` repetitions, and one context suite.
fn declared(tasks: &[(&str, &str)], reps: u32) -> Fixture {
    let f = Fixture::new();
    f.write(&format!("{DIR}/methodology.yaml"), METHODOLOGY);
    let ids: Vec<&str> = tasks.iter().map(|(t, _)| *t).collect();
    f.write(
        &format!("{DIR}/suites/pilot.yaml"),
        &format!(
            "schema: economics-suite/v1\nid: pilot\nversion: 1\nkind: live\ntitle: Pilot\nprovider: anthropic\nharness: claude-code\nmodel: claude-sonnet-5\nrepetitions: {reps}\ncontrol: baseline\ntreatment: majordomus\ntasks: [{}]\nfreshness_inputs:\n  - {DIR}/methodology.yaml\n  - {DIR}/tasks\n",
            ids.join(", ")
        ),
    );
    f.write(
        &format!("{DIR}/suites/context.yaml"),
        &format!("schema: economics-suite/v1\nid: context\nversion: 1\nkind: context\ntitle: Context\nseeds: .ai/repo/project/issues\ntokenizer: o200k_base\nfreshness_inputs:\n  - {DIR}/methodology.yaml\n"),
    );
    for (id, category) in tasks {
        let sessions = if *category == "multi-session" { 2 } else { 1 };
        f.write(&format!("{DIR}/tasks/{id}/task.yaml"), &task_yaml(id, category, sessions));
        for i in 1..=sessions {
            f.write(&format!("{DIR}/tasks/{id}/prompt-{i}.md"), "Do the thing.\n");
        }
    }
    f.commit("declare the benchmark");
    f
}

fn digest(f: &Fixture) -> String {
    economics::inputs_digest(
        &f.root(),
        &[format!("{DIR}/methodology.yaml"), format!("{DIR}/tasks")],
    )
    .unwrap()
}

fn session(index: u32, input: u64, output: u64, first_input: u64, tool_calls: u64) -> EconomicsSession {
    let mut tools = BTreeMap::new();
    tools.insert("Bash".to_string(), tool_calls);
    EconomicsSession {
        index,
        prompt_sha256: "0".repeat(64),
        duration_ms: 1000,
        turns: Some(3),
        ended: "success".into(),
        is_error: false,
        requests: vec![
            EconomicsRequest {
                model: "claude-sonnet-5".into(),
                input_tokens: 2,
                cache_creation_input_tokens: first_input - 2,
                cache_read_input_tokens: 0,
                output_tokens: 10,
                thinking_tokens: None,
            },
            EconomicsRequest {
                model: "claude-sonnet-5".into(),
                input_tokens: 2,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: input - first_input - 2,
                output_tokens: output - 10,
                thinking_tokens: None,
            },
        ],
        models: vec![EconomicsModelTotal {
            model: "claude-sonnet-5".into(),
            input_tokens: 4,
            cache_creation_input_tokens: first_input - 2,
            cache_read_input_tokens: input - first_input - 2,
            output_tokens: output,
            cost_microusd: Some(input / 10),
        }],
        reported_cost_microusd: Some(input / 10),
        tools,
        orientation: EconomicsOrientation::default(),
    }
}

struct Spec<'a> {
    task: &'a str,
    variant: &'a str,
    rep: u32,
    /// total tokens per session, input and output split nine to one
    totals: Vec<u64>,
    completed: bool,
    model: &'a str,
    digest: String,
}

fn run(s: Spec) -> EconomicsRun {
    let sessions: Vec<EconomicsSession> = s
        .totals
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let output = t / 10;
            let input = t - output;
            let first = if s.variant == "majordomus" { 8_000 } else { 5_000 };
            session(i as u32 + 1, input, output, first.min(input - 10), 10)
        })
        .collect();
    EconomicsRun {
        schema: economics::RUN_SCHEMA.into(),
        id: format!("{}--{}--r{}", s.task, s.variant, s.rep),
        suite: "pilot".into(),
        suite_version: 1,
        methodology: 1,
        task: s.task.into(),
        variant: s.variant.into(),
        repetition: s.rep,
        provider: "anthropic".into(),
        harness: EconomicsHarness { name: "claude-code".into(), version: "2.1.281".into() },
        model_requested: s.model.into(),
        models_reported: vec![s.model.into()],
        repository: EconomicsRevision { commit: "a".repeat(40), dirty: false },
        majordomus_version: "0.8.0".into(),
        fixture_digest: "f".repeat(64),
        inputs_digest: s.digest,
        configuration_digest: "c".repeat(64),
        started_at: "2026-09-24T10:00:00Z".into(),
        finished_at: "2026-09-24T10:05:00Z".into(),
        sessions,
        outcome: EconomicsOutcome {
            completed: s.completed,
            checks: vec![
                EconomicsCheck { id: "visible_tests".into(), passed: true, detail: "exit 0".into() },
                EconomicsCheck { id: "acceptance_tests".into(), passed: s.completed, detail: if s.completed { "exit 0".into() } else { "exit 1: FAILED".into() } },
                EconomicsCheck { id: "tests_kept".into(), passed: true, detail: "all present".into() },
            ],
            changed_files: vec!["src/a.py".into()],
        },
    }
}

fn record(f: &Fixture, r: &EconomicsRun) {
    f.write(
        &format!("{DIR}/runs/pilot/{}.json", r.id),
        &(serde_json::to_string_pretty(r).unwrap() + "\n"),
    );
}

/// Record a matched pair: control and treatment of one task and repetition.
fn pair(f: &Fixture, task: &str, rep: u32, control: Vec<u64>, treatment: Vec<u64>) {
    let d = digest(f);
    for (variant, totals) in [("baseline", control), ("majordomus", treatment)] {
        record(
            f,
            &run(Spec { task, variant, rep, totals, completed: true, model: "claude-sonnet-5", digest: d.clone() }),
        );
    }
}

fn metric<'a>(s: &'a EconomicsSummary, id: &str) -> &'a EconomicsMetric {
    s.metrics.iter().find(|m| m.id == id).unwrap_or_else(|| panic!("no metric {id}"))
}

fn summary(f: &Fixture) -> EconomicsSummary {
    economics::summarize(&f.root(), &EconomicsQuery::default())
}

#[test]
fn with_no_methodology_nothing_is_claimed_and_nothing_is_numbered() {
    let f = Fixture::new();
    let s = summary(&f);
    assert!(!s.present);
    assert!(!s.verdict.publishable);
    assert!(s.verdict.statement.starts_with(NO_CLAIM));
    assert!(s.metrics.is_empty());
}

#[test]
fn with_no_observations_the_statement_is_that_no_claim_is_available() {
    let f = declared(&[("t1", "bug-fix")], 2);
    let s = summary(&f);
    assert!(s.present);
    assert!(!s.verdict.publishable);
    assert!(s.verdict.statement.starts_with(NO_CLAIM), "{}", s.verdict.statement);
    let m = metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION);
    assert_eq!(m.status, EconomicsMetricStatus::NotMeasured);
    assert_eq!(m.value, None, "an absent measurement is absent, not zero");
    assert_eq!(s.pairs.len(), 2, "declared pairs are listed even when nothing ran");
    assert!(s.pairs.iter().all(|p| p.status == EconomicsPairStatus::Missing));
    assert_eq!(s.suites.iter().find(|v| v.id == "pilot").unwrap().freshness, EconomicsFreshness::NoEvidence);
}

/// A context record over `seeds` (seed, candidate tokens, selected tokens), measured under
/// `methodology` against the fixture's current inputs.
fn context_run(f: &Fixture, seeds: &[(&str, u64, u64)], methodology: u32) -> EconomicsContextRun {
    EconomicsContextRun {
        schema: economics::CONTEXT_RUN_SCHEMA.into(),
        suite: "context".into(),
        suite_version: 1,
        methodology,
        repository: EconomicsRevision { commit: "b".repeat(40), dirty: false },
        majordomus_version: "0.8.0".into(),
        tokenizer: economics::context::tokenizer(),
        inputs_digest: economics::inputs_digest(&f.root(), &[format!("{DIR}/methodology.yaml")]).unwrap(),
        budget_tokens: 24_000,
        measured_at: "2026-09-24T10:00:00Z".into(),
        seeds: seeds
            .iter()
            .map(|(seed, c, sel)| EconomicsContextSeed {
                seed: seed.to_string(),
                candidates: 10,
                candidate_bytes: c * 4,
                candidate_tokens: *c,
                considered: 12,
                considered_tokens: c + 100,
                selected: 3,
                selected_bytes: sel * 4,
                selected_tokens: *sel,
                estimated_selected_tokens: *sel,
                excluded_tokens: BTreeMap::from([("budget".to_string(), c - sel)]),
                unresolved: 0,
                over_budget: false,
            })
            .collect(),
        refused: Vec::new(),
    }
}

fn record_context(f: &Fixture, run: &EconomicsContextRun) {
    f.write(&format!("{DIR}/runs/context/b.json"), &serde_json::to_string_pretty(run).unwrap());
}

#[test]
fn context_evidence_alone_never_becomes_a_total_token_claim() {
    let f = declared(&[("t1", "bug-fix")], 1);
    let run = context_run(&f, &[("I1", 1000, 250), ("I2", 2000, 500), ("I3", 400, 400)], 1);
    record_context(&f, &run);
    let s = summary(&f);
    let ctx = metric(&s, economics::CONTEXT_REDUCTION);
    assert_eq!(ctx.status, EconomicsMetricStatus::Measured);
    assert_eq!(ctx.value, Some(0.75), "median of 0.75, 0.75 and 0");
    assert_eq!(ctx.inputs, Some(EconomicsClass::Counted));
    assert!(ctx.not.as_deref().unwrap().contains("not total token savings"));
    assert_eq!(metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION).status, EconomicsMetricStatus::NotMeasured);
    assert!(s.verdict.statement.starts_with(NO_CLAIM));
    assert!(!s.verdict.statement.contains("75"), "the context figure never reaches the verdict");
    assert_eq!(s.context.as_ref().unwrap().seeds, 3);
}

#[test]
fn matched_runs_give_the_median_of_per_pair_reductions_labelled_preliminary() {
    let f = declared(&[("t1", "bug-fix"), ("t2", "small-feature"), ("t3", "cross-module")], 1);
    pair(&f, "t1", 1, vec![100_000], vec![50_000]);
    pair(&f, "t2", 1, vec![100_000], vec![80_000]);
    pair(&f, "t3", 1, vec![200_000], vec![220_000]);
    let s = summary(&f);
    let m = metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION);
    assert_eq!(m.n, 3);
    assert_eq!(m.value, Some(0.2), "median of 0.5, 0.2 and -0.1");
    assert_eq!(m.class, EconomicsClass::Derived);
    assert_eq!(m.inputs, Some(EconomicsClass::Observed));
    assert_eq!(m.status, EconomicsMetricStatus::Preliminary);
    assert!(m.interval.is_none(), "three pairs are too few for an interval");
    assert!(!s.verdict.publishable);
    assert!(s.verdict.statement.starts_with(NO_CLAIM));
    assert!(s.verdict.statement.contains("Preliminary"), "{}", s.verdict.statement);
    assert!(
        s.verdict.statement.contains("reduced median total token consumption by 20.0% (no interval: too few pairs)"),
        "{}",
        s.verdict.statement
    );
    assert!(s.verdict.statement.contains("over 3 valid matched pair(s) across 3 task(s) in 3 categor(ies)"));
    assert!(s.verdict.statement.contains("using claude-sonnet-5"), "the preliminary figure names its model");
    let d = m.distribution.as_ref().unwrap();
    assert_eq!((d.min, d.max), (-0.1, 0.5), "the regression is in the distribution");
}

#[test]
fn a_treatment_that_costs_more_is_reported_as_it_is() {
    let f = declared(&[("t1", "bug-fix")], 1);
    pair(&f, "t1", 1, vec![100_000], vec![120_000]);
    let s = summary(&f);
    let m = metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION);
    assert_eq!(m.value, Some(-0.2), "never clamped to zero");
    assert!(
        s.verdict.statement.contains("increased median total token consumption by 20.0%"),
        "{}",
        s.verdict.statement
    );
    let seg = s.segments.iter().find(|g| g.dimension == "category" && g.value == "bug-fix").unwrap();
    assert_eq!(seg.token_reduction.as_ref().unwrap().median, -0.2);
    let overhead = metric(&s, "first_request_overhead");
    assert_eq!(overhead.value, Some(3000.0), "the treatment's first request carried more");
}

#[test]
fn a_pair_with_a_failed_side_is_invalid_named_and_left_out_of_every_value() {
    let f = declared(&[("t1", "bug-fix"), ("t2", "bug-fix"), ("t3", "bug-fix"), ("t4", "bug-fix")], 1);
    let d = digest(&f);
    let mk = |task: &'static str, variant: &'static str, completed: bool, total: u64| {
        run(Spec { task, variant, rep: 1, totals: vec![total], completed, model: "claude-sonnet-5", digest: d.clone() })
    };
    // t1: the control failed; t2: the treatment failed (and cheaply); t3: both; t4: valid
    for r in [
        mk("t1", "baseline", false, 90_000),
        mk("t1", "majordomus", true, 100_000),
        mk("t2", "baseline", true, 100_000),
        mk("t2", "majordomus", false, 10_000),
        mk("t3", "baseline", false, 100_000),
        mk("t3", "majordomus", false, 100_000),
        mk("t4", "baseline", true, 100_000),
        mk("t4", "majordomus", true, 90_000),
    ] {
        record(&f, &r);
    }
    let s = summary(&f);
    let status = |t: &str| s.pairs.iter().find(|p| p.task == t).unwrap();
    assert_eq!(status("t1").status, EconomicsPairStatus::ControlFailed);
    assert_eq!(status("t2").status, EconomicsPairStatus::TreatmentFailed);
    assert!(status("t2").reasons.iter().any(|r| r.contains("acceptance_tests")), "{:?}", status("t2").reasons);
    assert_eq!(status("t3").status, EconomicsPairStatus::BothFailed);
    assert_eq!(status("t4").status, EconomicsPairStatus::Valid);
    let m = metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION);
    assert_eq!((m.n, m.value), (1, Some(0.1)), "the cheap failure saved nothing");
    let v = s.suites.iter().find(|v| v.id == "pilot").unwrap();
    assert_eq!(
        (v.pairs.valid, v.pairs.control_failed, v.pairs.treatment_failed, v.pairs.both_failed, v.pairs.attempted),
        (1, 1, 1, 1, 4)
    );
    assert_eq!(metric(&s, "completion_rate.majordomus").value, Some(0.5));
}

#[test]
fn runs_that_differ_in_model_or_fixture_are_not_compared() {
    let f = declared(&[("t1", "bug-fix"), ("t2", "bug-fix")], 1);
    let d = digest(&f);
    record(&f, &run(Spec { task: "t1", variant: "baseline", rep: 1, totals: vec![100_000], completed: true, model: "claude-sonnet-5", digest: d.clone() }));
    record(&f, &run(Spec { task: "t1", variant: "majordomus", rep: 1, totals: vec![50_000], completed: true, model: "claude-opus-5-5", digest: d.clone() }));
    let mut c = run(Spec { task: "t2", variant: "baseline", rep: 1, totals: vec![100_000], completed: true, model: "claude-sonnet-5", digest: d.clone() });
    c.fixture_digest = "e".repeat(64);
    record(&f, &c);
    record(&f, &run(Spec { task: "t2", variant: "majordomus", rep: 1, totals: vec![50_000], completed: true, model: "claude-sonnet-5", digest: d }));
    let s = summary(&f);
    for (t, what) in [("t1", "requested model"), ("t2", "fixture digest")] {
        let p = s.pairs.iter().find(|p| p.task == t).unwrap();
        assert_eq!(p.status, EconomicsPairStatus::Incomparable);
        assert!(p.reasons.iter().any(|r| r.contains(what)), "{:?}", p.reasons);
        assert_eq!(p.token_reduction, None);
    }
    assert_eq!(metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION).n, 0);
}

#[test]
fn a_run_that_reported_no_usage_makes_its_pair_unusable_rather_than_zero() {
    let f = declared(&[("t1", "bug-fix")], 1);
    pair(&f, "t1", 1, vec![100_000], vec![50_000]);
    let path = f.path(&format!("{DIR}/runs/pilot/t1--majordomus--r1.json"));
    let mut r: EconomicsRun = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    r.sessions[0].models.clear();
    r.sessions[0].requests.clear();
    std::fs::write(&path, serde_json::to_string_pretty(&r).unwrap()).unwrap();
    let s = summary(&f);
    assert_eq!(s.pairs[0].status, EconomicsPairStatus::UsageUnavailable);
    assert_eq!(metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION).value, None);
}

#[test]
fn an_excluded_run_is_named_and_its_pair_is_left_out() {
    let f = declared(&[("t1", "bug-fix"), ("t2", "bug-fix")], 1);
    let excluded = METHODOLOGY.replace(
        "  excluded: []",
        "  excluded:\n    - run: t1--majordomus--r1\n      reason: the provider returned an overload error mid-session",
    );
    assert_ne!(excluded, METHODOLOGY);
    f.write(&format!("{DIR}/methodology.yaml"), &excluded);
    f.commit("exclude");
    pair(&f, "t1", 1, vec![100_000], vec![10_000]);
    pair(&f, "t2", 1, vec![100_000], vec![90_000]);
    let s = summary(&f);
    let p = s.pairs.iter().find(|p| p.task == "t1").unwrap();
    assert_eq!(p.status, EconomicsPairStatus::Excluded);
    assert!(p.reasons[0].contains("overload"));
    assert_eq!(metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION).value, Some(0.1));
    let x = economics::explain(&f.root(), economics::EFFECTIVE_TOKEN_REDUCTION).unwrap();
    assert_eq!(x.excluded.len(), 1);
}

#[test]
fn a_run_recorded_twice_is_a_diagnostic() {
    let f = declared(&[("t1", "bug-fix")], 1);
    pair(&f, "t1", 1, vec![100_000], vec![50_000]);
    let r = std::fs::read_to_string(f.path(&format!("{DIR}/runs/pilot/t1--baseline--r1.json"))).unwrap();
    f.write(&format!("{DIR}/runs/pilot/zz-copy.json"), &r);
    let s = summary(&f);
    assert!(s.diagnostics.iter().any(|d| d.contains("recorded twice")), "{:?}", s.diagnostics);
}

/// Enough evidence to meet the publication rule this repository declares: eight tasks in four
/// categories, four repetitions each, every pair valid, reductions tightly spread.
fn publishable() -> Fixture {
    publishable_with(60_000)
}

/// [`publishable`] with the treatment's total tokens starting at `base` against a control
/// of 100 000: below it the treatment saves, above it the treatment costs more.
fn publishable_with(base: u64) -> Fixture {
    let tasks: Vec<(String, &str)> = (0..8).map(|i| (format!("t{i}"), CATEGORIES[i % 4])).collect();
    let refs: Vec<(&str, &str)> = tasks.iter().map(|(t, c)| (t.as_str(), *c)).collect();
    let f = declared(&refs, 4);
    for (i, (t, c)) in tasks.iter().enumerate() {
        for rep in 1..=4u32 {
            let treated = base + (i as u64 * 4 + rep as u64) * 150;
            if *c == "multi-session" {
                pair(&f, t, rep, vec![60_000, 40_000], vec![30_000, treated - 30_000]);
            } else {
                pair(&f, t, rep, vec![100_000], vec![treated]);
            }
        }
    }
    f
}

#[test]
fn past_the_publication_rule_the_statement_carries_its_sample_interval_and_models() {
    let f = publishable();
    let s = summary(&f);
    assert!(s.verdict.unmet.is_empty(), "{:?}", s.verdict.unmet);
    assert!(s.verdict.publishable);
    let m = metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION);
    assert_eq!(m.status, EconomicsMetricStatus::Verified);
    assert_eq!(m.n, 32);
    let i = m.interval.as_ref().expect("an interval");
    assert!(i.low <= m.value.unwrap() && m.value.unwrap() <= i.high);
    assert!(i.method.contains("cluster bootstrap over tasks"), "{}", i.method);
    assert!(
        s.verdict.statement.starts_with("Across methodology 1, 32 valid matched pairs across 8 tasks in 4 categories"),
        "{}",
        s.verdict.statement
    );
    assert!(s.verdict.statement.contains("claude-sonnet-5"));
    assert!(s.verdict.statement.contains("reduced median total token consumption by"), "{}", s.verdict.statement);
    assert!(s.verdict.statement.contains("(95% CI: a reduction of "), "{}", s.verdict.statement);
    assert!(!s.verdict.statement.contains("excluded"), "nothing was excluded");
    assert_eq!(metric(&s, "continuation_reduction").n, 8, "only the multi-session pairs continue");
}

/// The two bounds of the interval a statement gives after `lead`, as numbers.
fn interval_in(statement: &str, lead: &str) -> (f64, f64) {
    let rest = &statement[statement.find(lead).unwrap_or_else(|| panic!("no {lead:?} in {statement}")) + lead.len()..];
    let rest = &rest[..rest.find(')').unwrap()];
    let (a, b) = rest.split_once(" to ").unwrap();
    let num = |s: &str| s.trim().trim_end_matches('%').parse::<f64>().unwrap();
    (num(a), num(b))
}

#[test]
fn a_publishable_increase_is_stated_as_an_increase_with_an_interval_of_increases() {
    let f = publishable_with(120_000);
    let s = summary(&f);
    assert!(s.verdict.publishable, "{:?}", s.verdict.unmet);
    let m = metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION);
    assert!(m.value.unwrap() < 0.0, "the treatment costs more");
    assert_eq!(m.status, EconomicsMetricStatus::Verified, "a regression is publishable too");
    let st = &s.verdict.statement;
    assert!(st.contains("Majordomus increased median total token consumption by"), "{st}");
    assert!(!st.contains("reduction"), "the interval is in the words' direction: {st}");
    let (a, b) = interval_in(st, "(95% CI: an increase of ");
    let i = m.interval.as_ref().unwrap();
    assert!(0.0 < a && a <= b, "an interval of increases, low to high: {st}");
    assert!((a - -i.high * 100.0).abs() < 0.051 && (b - -i.low * 100.0).abs() < 0.051, "{st} {i:?}");
}

#[test]
fn a_narrowed_query_never_yields_a_verified_metric() {
    let f = publishable();
    assert_eq!(
        metric(&summary(&f), economics::EFFECTIVE_TOKEN_REDUCTION).status,
        EconomicsMetricStatus::Verified
    );
    for q in [
        EconomicsQuery { category: Some("bug-fix".into()), ..Default::default() },
        EconomicsQuery { suite: Some("pilot".into()), ..Default::default() },
        EconomicsQuery { model: Some("claude-sonnet-5".into()), ..Default::default() },
        EconomicsQuery { task: Some("t0".into()), ..Default::default() },
    ] {
        let s = economics::summarize(&f.root(), &q);
        assert!(s.verdict.publishable, "the verdict is about all the evidence, whatever is narrowed");
        let m = metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION);
        assert!(m.n > 0, "{q:?} selects pairs");
        assert_eq!(m.status, EconomicsMetricStatus::Preliminary, "{q:?}");
        assert!(m.warnings.iter().any(|w| w == economics::NARROWED), "{:?}", m.warnings);
        assert!(s.metrics.iter().all(|x| x.status != EconomicsMetricStatus::Verified), "{q:?}");
    }
}

#[test]
fn no_secondary_metric_is_verified_even_when_the_primary_is() {
    let f = publishable();
    let s = summary(&f);
    assert!(s.verdict.publishable);
    for m in &s.metrics {
        if m.id == economics::EFFECTIVE_TOKEN_REDUCTION {
            assert_eq!(m.status, EconomicsMetricStatus::Verified);
            continue;
        }
        assert_ne!(m.status, EconomicsMetricStatus::Verified, "{} was never evaluated by the rule", m.id);
        if m.n > 0 && m.inputs != Some(EconomicsClass::Counted) {
            assert_eq!(m.status, EconomicsMetricStatus::Preliminary, "{}", m.id);
        }
    }
    for id in ["cost_reduction", "tool_call_reduction", "continuation_reduction", "first_request_overhead", "completion_rate.majordomus", "tokens_per_completed_task.baseline"] {
        let m = metric(&s, id);
        assert_eq!(m.status, EconomicsMetricStatus::Preliminary, "{id}");
        assert!(m.warnings.iter().any(|w| w.contains("never a verified claim")), "{id}: {:?}", m.warnings);
    }
}

#[test]
fn the_repetition_rule_holds_for_every_task_and_variant_not_the_busiest() {
    let f = declared(&[("t1", "bug-fix"), ("t2", "bug-fix")], 4);
    for rep in 1..=4 {
        pair(&f, "t1", rep, vec![100_000], vec![60_000]);
    }
    pair(&f, "t2", 1, vec![100_000], vec![60_000]);
    let s = summary(&f);
    assert!(
        s.verdict.unmet.iter().any(|u| u.starts_with("1 repetition(s) of t2 (")),
        "the least-repeated task decides, not the most: {:?}",
        s.verdict.unmet
    );

    // a failed run is a repetition that ran; a run under another methodology is not
    let g = declared(&[("t1", "bug-fix")], 3);
    let d = digest(&g);
    for rep in 1..=3 {
        for variant in ["baseline", "majordomus"] {
            let mut r = run(Spec { task: "t1", variant, rep, totals: vec![100_000], completed: rep != 2, model: "claude-sonnet-5", digest: d.clone() });
            if variant == "majordomus" && rep == 3 {
                r.methodology = 0;
            }
            record(&g, &r);
        }
    }
    let s = summary(&g);
    assert!(
        s.verdict.unmet.iter().any(|u| u.starts_with("2 repetition(s) of t1 (majordomus)")),
        "{:?}",
        s.verdict.unmet
    );
    assert!(!s.verdict.unmet.iter().any(|u| u.contains("of t1 (baseline)")), "{:?}", s.verdict.unmet);
}

#[test]
fn the_cluster_bootstrap_is_no_narrower_than_the_pooled_one_on_clustered_data() {
    use economics::stats::{bootstrap_median, bootstrap_median_clustered};
    // eight tasks, four near-identical repetitions each: thirty-two values, eight tasks
    let clusters: Vec<Vec<f64>> =
        (0..8).map(|t| (0..4).map(|r| 0.05 * t as f64 + 0.0005 * r as f64).collect()).collect();
    let pooled: Vec<f64> = clusters.iter().flatten().copied().collect();
    let p = bootstrap_median(&pooled, 9500, 10_000, 20260924, 5).unwrap();
    let c = bootstrap_median_clustered(&clusters, 9500, 10_000, 20260924, 5).unwrap();
    assert!(c.high - c.low >= p.high - p.low, "cluster {c:?} against pooled {p:?}");
    assert!(c.method.contains("cluster bootstrap over tasks"));
    assert!(!p.method.contains("cluster"));
}

#[test]
fn an_excluded_run_is_reported_both_ways() {
    let f = declared(&[("t1", "bug-fix"), ("t2", "bug-fix")], 1);
    let excluded = METHODOLOGY.replace(
        "  excluded: []",
        "  excluded:\n    - run: t1--majordomus--r1\n      reason: the provider returned an overload error mid-session",
    );
    f.write(&format!("{DIR}/methodology.yaml"), &excluded);
    f.commit("exclude");
    pair(&f, "t1", 1, vec![100_000], vec![10_000]);
    pair(&f, "t2", 1, vec![100_000], vec![90_000]);
    let s = summary(&f);
    let p = s.pairs.iter().find(|p| p.task == "t1").unwrap();
    assert_eq!(p.status, EconomicsPairStatus::Excluded, "the pair stays excluded");
    assert_eq!(p.token_reduction, Some(0.9), "and shows what it would have said");
    let primary = metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION);
    assert_eq!((primary.n, primary.value), (1, Some(0.1)), "the primary figure leaves it out");
    let with = metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION_INCLUDING_EXCLUDED);
    assert_eq!((with.n, with.value), (2, Some(0.5)), "median of 0.9 and 0.1");
    assert_eq!(with.status, EconomicsMetricStatus::Preliminary);
    assert!(with.evidence.contains(&"pilot/t1/r1".to_string()));
    assert!(
        s.verdict.statement.contains("1 run(s) excluded by the methodology's outlier rule"),
        "{}",
        s.verdict.statement
    );
    let x = economics::explain(&f.root(), economics::EFFECTIVE_TOKEN_REDUCTION_INCLUDING_EXCLUDED).unwrap();
    assert!(x.pairs.iter().any(|p| p.task == "t1"), "explain shows the excluded pair it rests on");

    // without an exclusion the metric does not exist at all
    let g = declared(&[("t1", "bug-fix")], 1);
    pair(&g, "t1", 1, vec![100_000], vec![50_000]);
    let s = summary(&g);
    assert!(s.metrics.iter().all(|m| m.id != economics::EFFECTIVE_TOKEN_REDUCTION_INCLUDING_EXCLUDED));
    assert!(!s.verdict.statement.contains("excluded"));
}

#[test]
fn a_context_record_with_no_seeds_is_not_a_measurement() {
    let f = declared(&[("t1", "bug-fix")], 1);
    record_context(&f, &context_run(&f, &[], 1));
    let s = summary(&f);
    for id in [economics::CONTEXT_REDUCTION, "context_cost_model_error"] {
        let m = metric(&s, id);
        assert_eq!(m.status, EconomicsMetricStatus::NotMeasured, "{id}");
        assert_eq!((m.value, m.n), (None, 0), "{id}: absence stays absence");
    }
}

#[test]
fn a_context_record_of_another_methodology_is_incompatible_not_measured() {
    let f = declared(&[("t1", "bug-fix")], 1);
    record_context(&f, &context_run(&f, &[("I1", 1000, 250)], 0));
    let s = summary(&f);
    for id in [economics::CONTEXT_REDUCTION, "context_cost_model_error"] {
        let m = metric(&s, id);
        assert_eq!((m.status, m.value), (EconomicsMetricStatus::NotMeasured, None), "{id}");
        assert!(m.warnings.iter().any(|w| w.starts_with("incompatible")), "{id}: {:?}", m.warnings);
    }
}

#[test]
fn a_context_record_says_what_it_did_not_measure_and_how_it_stands() {
    let f = declared(&[("t1", "bug-fix")], 1);
    let mut run = context_run(&f, &[("I1", 1000, 250), ("I2", 2000, 500)], 1);
    run.refused = vec!["I9: no such issue".into()];
    run.repository.dirty = true;
    record_context(&f, &run);
    let s = summary(&f);
    let m = metric(&s, economics::CONTEXT_REDUCTION);
    assert_eq!(m.status, EconomicsMetricStatus::Measured);
    assert!(m.warnings.iter().any(|w| w == "1 seed(s) refused by the compiler were not measured"), "{:?}", m.warnings);
    assert!(m.warnings.iter().any(|w| w.starts_with("dirty")), "{:?}", m.warnings);
    assert!(!m.warnings.iter().any(|w| w.starts_with("stale")), "{:?}", m.warnings);

    // a changed mechanism leaves the record measured, and says it is stale
    f.write(&format!("{DIR}/methodology.yaml"), &format!("{METHODOLOGY}\n# changed\n"));
    let s = summary(&f);
    for id in [economics::CONTEXT_REDUCTION, "context_cost_model_error"] {
        let m = metric(&s, id);
        assert_eq!(m.status, EconomicsMetricStatus::Measured, "{id}");
        assert!(m.warnings.iter().any(|w| w.starts_with("stale")), "{id}: {:?}", m.warnings);
    }
}

#[test]
fn runs_that_report_different_models_or_configurations_are_not_compared() {
    let f = declared(&[("t1", "bug-fix"), ("t2", "bug-fix")], 1);
    let d = digest(&f);
    let spec = |task: &'static str, variant: &'static str, total: u64| Spec {
        task,
        variant,
        rep: 1,
        totals: vec![total],
        completed: true,
        model: "claude-sonnet-5",
        digest: d.clone(),
    };
    record(&f, &run(spec("t1", "baseline", 100_000)));
    let mut t = run(spec("t1", "majordomus", 50_000));
    t.models_reported = vec!["claude-haiku-5".into(), "claude-sonnet-5".into()];
    record(&f, &t);
    record(&f, &run(spec("t2", "baseline", 100_000)));
    let mut t = run(spec("t2", "majordomus", 50_000));
    t.configuration_digest = "d".repeat(64);
    record(&f, &t);
    let s = summary(&f);
    for (task, what) in [("t1", "reported models"), ("t2", "configuration digest")] {
        let p = s.pairs.iter().find(|p| p.task == task).unwrap();
        assert_eq!(p.status, EconomicsPairStatus::Incomparable, "{task}");
        assert!(p.reasons.iter().any(|r| r.contains(what)), "{:?}", p.reasons);
        assert_eq!(p.token_reduction, None);
    }
    assert_eq!(metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION).n, 0);
    assert_eq!(metric(&s, "completion_rate.majordomus").n, 0, "an incomparable pair is no population");
}

#[test]
fn run_based_metrics_read_the_same_population_as_the_pairs() {
    let f = declared(&[("t1", "bug-fix"), ("t2", "small-feature")], 1);
    let d = digest(&f);
    let mk = |task: &'static str, variant: &'static str, completed: bool| {
        run(Spec { task, variant, rep: 1, totals: vec![100_000], completed, model: "claude-sonnet-5", digest: d.clone() })
    };
    // t1 valid; t2 the treatment failed; a stray treatment run with no control pairs nothing
    for r in [mk("t1", "baseline", true), mk("t1", "majordomus", true), mk("t2", "baseline", true), mk("t2", "majordomus", false)] {
        record(&f, &r);
    }
    let mut stray = mk("t1", "majordomus", true);
    stray.id = "t1--majordomus--r2".into();
    stray.repetition = 2;
    record(&f, &stray);
    let s = summary(&f);
    let rate = metric(&s, "completion_rate.majordomus");
    assert_eq!((rate.n, rate.value), (2, Some(0.5)), "the failure counts, the unpaired run does not");
    let tokens = metric(&s, "tokens_per_completed_task.majordomus");
    assert_eq!(tokens.n, 1, "only runs of valid pairs, so both arms describe the same tasks");
    assert_eq!(metric(&s, "tokens_per_completed_task.baseline").n, 1);

    let narrowed = economics::summarize(
        &f.root(),
        &EconomicsQuery { category: Some("small-feature".into()), ..Default::default() },
    );
    let rate = metric(&narrowed, "completion_rate.majordomus");
    assert_eq!((rate.n, rate.value), (1, Some(0.0)), "a category narrows the runs by their task");
    assert_eq!(metric(&narrowed, "tokens_per_completed_task.majordomus").n, 0);

    let x = economics::explain(&f.root(), "completion_rate.majordomus").unwrap();
    let tasks: Vec<&str> = x.pairs.iter().map(|p| p.task.as_str()).collect();
    assert_eq!(tasks, ["t1", "t2"], "the pairs its runs belong to, the failed one included");
}

#[test]
fn generation_reads_only_the_records_git_tracks() {
    let f = declared(&[("t1", "bug-fix"), ("t2", "bug-fix")], 1);
    pair(&f, "t1", 1, vec![100_000], vec![50_000]);
    f.commit("record t1");
    pair(&f, "t2", 1, vec![100_000], vec![70_000]);
    let q = EconomicsQuery::default();
    assert_eq!(metric(&economics::summarize(&f.root(), &q), economics::EFFECTIVE_TOKEN_REDUCTION).n, 2);
    let tracked = economics::summarize_tracked(&f.root(), &q);
    assert_eq!(metric(&tracked, economics::EFFECTIVE_TOKEN_REDUCTION).n, 1, "the uncommitted pair is not read");
    assert!(tracked.diagnostics.is_empty(), "{:?}", tracked.diagnostics);
}

#[test]
fn a_changed_mechanism_makes_the_evidence_stale_and_withdraws_the_claim() {
    let f = publishable();
    assert!(summary(&f).verdict.publishable);
    let task = f.path(&format!("{DIR}/tasks/t0/prompt-1.md"));
    std::fs::write(&task, "Do the thing differently.\n").unwrap();
    let s = summary(&f);
    let v = s.suites.iter().find(|v| v.id == "pilot").unwrap();
    assert_eq!(v.freshness, EconomicsFreshness::Stale);
    assert!(!s.verdict.publishable);
    assert!(s.verdict.unmet.iter().any(|u| u.contains("not current")), "{:?}", s.verdict.unmet);
    assert!(s.verdict.statement.starts_with(NO_CLAIM));
}

#[test]
fn evidence_from_a_dirty_tree_cannot_be_published() {
    let f = publishable();
    let path = f.path(&format!("{DIR}/runs/pilot/t0--baseline--r1.json"));
    let mut r: EconomicsRun = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    r.repository.dirty = true;
    std::fs::write(&path, serde_json::to_string_pretty(&r).unwrap()).unwrap();
    let s = summary(&f);
    assert!(!s.verdict.publishable);
    assert!(s.verdict.unmet.iter().any(|u| u.contains("uncommitted")), "{:?}", s.verdict.unmet);
}

#[test]
fn evidence_under_another_methodology_is_incompatible_and_never_pooled() {
    let f = declared(&[("t1", "bug-fix")], 1);
    pair(&f, "t1", 1, vec![100_000], vec![50_000]);
    for v in ["baseline", "majordomus"] {
        let path = f.path(&format!("{DIR}/runs/pilot/t1--{v}--r1.json"));
        let mut r: EconomicsRun = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        r.methodology = 0;
        std::fs::write(&path, serde_json::to_string_pretty(&r).unwrap()).unwrap();
    }
    let s = summary(&f);
    assert_eq!(s.suites.iter().find(|v| v.id == "pilot").unwrap().freshness, EconomicsFreshness::Incompatible);
    assert_eq!(metric(&s, economics::EFFECTIVE_TOKEN_REDUCTION).n, 0);
}

#[test]
fn explain_names_what_a_metric_rests_on_and_refuses_one_that_does_not_exist() {
    let f = declared(&[("t1", "bug-fix")], 1);
    pair(&f, "t1", 1, vec![100_000], vec![50_000]);
    let x = economics::explain(&f.root(), economics::EFFECTIVE_TOKEN_REDUCTION).unwrap();
    assert_eq!(x.metric.value, Some(0.5));
    assert_eq!(x.pairs.len(), 1);
    assert!(x.class_meaning.contains("Arithmetic"));
    assert!(x.reproduce.iter().any(|r| r.contains("economics run --suite pilot")));
    let e = economics::explain(&f.root(), "savings_percent").unwrap_err();
    assert!(e.contains("effective_token_reduction"), "the refusal lists what exists: {e}");
}

#[test]
fn the_check_refuses_a_typed_savings_number_and_an_unsupported_guarantee() {
    let f = declared(&[("t1", "bug-fix")], 1);
    f.write("docs/CLAIMS.yaml", "version: 1\nclaims:\n  - id: context-selection-counted\n    claim: counted\n    status: guaranteed\n  - id: token-savings-measured\n    claim: planned\n    status: planned\n");
    f.write("docs/PITCH.md", "# Pitch\n\nMajordomus saves 50% of tokens on every task.\n");
    f.commit("claims");
    let r = economics::claims::check(&f.root());
    assert!(!r.ok);
    assert!(r.findings.iter().any(|x| x.path == "docs/PITCH.md" && x.line == Some(3)), "{:?}", r.findings);
    assert!(
        r.findings.iter().any(|x| x.kind == "claim_unsupported" && x.detail.contains("context-selection-counted")),
        "a guaranteed claim with no context evidence is refused: {:?}",
        r.findings
    );
    // numbers in generated files are the calculator's, and are not findings; "generated" is
    // provenance (the generation manifest lists the report), never the directory it sits in
    f.remove("docs/PITCH.md");
    f.write("docs/generated/economics.md", "# report\n\nMajordomus changed tokens by 50% here.\n");
    f.write(
        "docs/generated/artifacts.json",
        "{\"artifacts\":[{\"path\":\"docs/generated/economics.md\"}]}\n",
    );
    f.write("docs/CLAIMS.yaml", "version: 1\nclaims:\n  - id: context-selection-counted\n    claim: counted\n    status: planned\n  - id: token-savings-measured\n    claim: planned\n    status: planned\n");
    f.commit("fixed");
    let r = economics::claims::check(&f.root());
    assert!(r.ok, "{:?}", r.findings);
}

fn mcp_call(f: &Fixture, tool: &str, arguments: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_majordomus"))
        .arg("mcp")
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .current_dir(f.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } })).unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": tool, "arguments": arguments } })).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let frames: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    frames[1]["result"]["structuredContent"].clone()
}

/// One fixture, every surface, the same answer: the library, the command line, HTTP, MCP and
/// the Cockpit are projections of one calculator, so no surface can hold a different number.
#[test]
fn every_surface_reports_the_same_metric_value_class_and_sample() {
    let f = declared(&[("t1", "bug-fix"), ("t2", "small-feature"), ("t3", "cross-module")], 1);
    pair(&f, "t1", 1, vec![100_000], vec![50_000]);
    pair(&f, "t2", 1, vec![100_000], vec![80_000]);
    pair(&f, "t3", 1, vec![200_000], vec![220_000]);
    let library = serde_json::to_value(summary(&f)).unwrap();

    let (code, out, err) = run_in(&f.root(), &["economics", "summary", "--format", "json"], "");
    assert_eq!(code, 0, "{err}");
    let cli: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(cli, library, "the command line");

    let mcp = mcp_call(&f, "majordomus_economics", json!({}));
    assert_eq!(mcp, library, "MCP");

    let mut served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/economics");
    assert_eq!(status, 200);
    assert_eq!(http, library, "HTTP");
    let (status, explained) = served.get("/api/v1/economics/explain?metric=effective_token_reduction");
    assert_eq!(status, 200);
    assert_eq!(explained["metric"], library["metrics"][0], "explain carries the summary's metric");
    let (status, _, page) = served.request("GET", "/cockpit/economics", None);
    assert_eq!(status, 200);
    let statement = library["verdict"]["statement"].as_str().unwrap();
    assert!(page.contains(&statement.replace('\'', "&#39;").replace('"', "&quot;")) || page.contains(statement), "the Cockpit shows the verdict");
    assert!(page.contains("+20.0%"), "the Cockpit shows the median the calculator derived");
    served.stop();

    let report = economics::report::markdown(&summary(&f));
    assert!(report.contains(statement));
    assert!(report.contains("| `effective_token_reduction` | +20.0% | preliminary | derived from observed | 3 |"), "{report}");
}

#[test]
fn the_summary_does_not_depend_on_the_order_runs_are_found_in() {
    let a = declared(&[("t1", "bug-fix"), ("t2", "bug-fix")], 1);
    pair(&a, "t1", 1, vec![100_000], vec![50_000]);
    pair(&a, "t2", 1, vec![100_000], vec![70_000]);
    let b = declared(&[("t2", "bug-fix"), ("t1", "bug-fix")], 1);
    pair(&b, "t2", 1, vec![100_000], vec![70_000]);
    pair(&b, "t1", 1, vec![100_000], vec![50_000]);
    let (sa, sb) = (summary(&a), summary(&b));
    assert_eq!(sa.metrics, sb.metrics);
    assert_eq!(sa.verdict, sb.verdict);
}
