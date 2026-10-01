//! The human-readable benchmark report, `docs/generated/economics.md`: a rendering of one
//! [`EconomicsSummary`] and nothing else. Every number in it is a field of the summary,
//! spelled for a reader; the report computes none, so it cannot disagree with the CLI, the
//! API, MCP, the Cockpit or the site, which render the same summary.
//!
//! ```
//! use majordomus_cli::economics::{report, summarize};
//! let dir = tempfile::tempdir().unwrap();
//! let text = report::markdown(&summarize(dir.path(), &Default::default()));
//! assert!(text.contains("No verified total-token-savings claim is available"));
//! ```

use std::fmt::Write as _;

use serde::Serialize;

use super::model::{
    EconomicsInterval, EconomicsMetric, EconomicsMetricStatus, EconomicsPairStatus,
    EconomicsSummary,
};
use super::stats::{level_text, percent_text};
use super::{wire, CONTEXT_REDUCTION};

/// A signed whole number, with no sign at all when it rounds to zero: a figure that is,
/// to the precision shown, no difference never reads as `+0` or `-0`.
fn signed_count(v: f64) -> String {
    if !v.is_finite() {
        return "n/a".into();
    }
    let whole = v.round();
    if whole == 0.0 {
        "0".into()
    } else {
        format!("{whole:+.0}")
    }
}

/// A figure in its unit, as every surface spells it. A `ratio` is a signed percentage with
/// one decimal ([`percent_text`]); any other unit is a signed whole number followed by the
/// unit. Neither carries a sign when it rounds to zero, and a value that is not a number
/// prints `n/a`, so no surface ever shows `-0.0%` or a figure nobody computed.
///
/// Every ratio the calculator derives from a pair is a reduction, `1 - treatment /
/// control`: positive means Majordomus used less, and a negative reduction means
/// Majordomus used more. The spelling keeps that sign and never flips it.
///
/// ```
/// use majordomus_cli::economics::report::amount;
/// assert_eq!(amount(0.2, "ratio"), "+20.0%");
/// assert_eq!(amount(-0.1, "ratio"), "-10.0%", "a negative reduction stays negative");
/// assert_eq!(amount(-0.0004, "ratio"), "0.0%", "never -0.0%");
/// assert_eq!(amount(-1500.4, "tokens"), "-1500 tokens");
/// assert_eq!(amount(0.3, "tokens"), "0 tokens");
/// assert_eq!(amount(f64::NAN, "ratio"), "n/a");
/// ```
pub fn amount(v: f64, unit: &str) -> String {
    if unit == "ratio" {
        percent_text(v)
    } else {
        format!("{} {unit}", signed_count(v))
    }
}

/// A metric's value as the report spells it: [`amount`] in the metric's unit, and an
/// absent value the words `not measured`, so that an unmeasured metric never reads as a
/// change of zero.
///
/// ```
/// use majordomus_cli::economics::{model::EconomicsMetric, report};
/// let m: EconomicsMetric = serde_json::from_value(serde_json::json!({
///     "id": "effective_token_reduction", "title": "Effective token reduction",
///     "class": "derived", "unit": "ratio", "formula": "1 - treatment / control",
///     "value": 0.2, "status": "preliminary", "n": 3, "suite": "pilot"
/// }))
/// .unwrap();
/// assert_eq!(report::value(&m), "+20.0%");
/// assert_eq!(report::value(&EconomicsMetric { value: None, ..m }), "not measured");
/// ```
pub fn value(m: &EconomicsMetric) -> String {
    match m.value {
        None => "not measured".into(),
        Some(v) => amount(v, &m.unit),
    }
}

/// An interval as every surface spells it: the confidence level ([`level_text`], so that
/// `9750` reads `97.5%` and is never rounded into a level the methodology did not
/// declare), then both bounds in the metric's unit ([`amount`]). The bounds of a ratio are
/// reductions, like the value they bracket.
///
/// ```
/// use majordomus_cli::economics::{model::EconomicsInterval, report::interval_text};
/// let i = EconomicsInterval {
///     level_bp: 9750, low: -0.05, high: 0.3, method: "percentile bootstrap".into(),
///     resamples: 2000, seed: 7,
/// };
/// assert_eq!(interval_text(&i, "ratio"), "97.5%: -5.0% to +30.0%");
/// let tokens = EconomicsInterval { level_bp: 9500, low: 1200.0, high: 4800.0, ..i };
/// assert_eq!(interval_text(&tokens, "tokens"), "95%: +1200 tokens to +4800 tokens");
/// ```
pub fn interval_text(i: &EconomicsInterval, unit: &str) -> String {
    format!(
        "{}: {} to {}",
        level_text(i.level_bp),
        amount(i.low, unit),
        amount(i.high, unit)
    )
}

/// An enum of the summary as a person reads it: its serialised name ([`wire`], the one the
/// API and MCP answer with) with each underscore a space, so `no_evidence` reads `no
/// evidence`. A surface never spells a state from `Debug`, which would print
/// `NoEvidence`, a word the API never uses.
///
/// ```
/// use majordomus_cli::economics::model::{EconomicsFreshness, EconomicsPairStatus};
/// use majordomus_cli::economics::report::words;
/// assert_eq!(words(&EconomicsFreshness::NoEvidence), "no evidence");
/// assert_eq!(words(&EconomicsPairStatus::ControlFailed), "control failed");
/// assert_eq!(words(&EconomicsPairStatus::Valid), "valid");
/// ```
pub fn words<T: Serialize>(value: &T) -> String {
    wire(value).replace('_', " ")
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// The whole report for one summary, as Markdown. It reads nothing but `s`, so the same
/// summary always renders to the same bytes. When no methodology is declared (`present` is
/// false) the report stops after the verdict, its unmet thresholds and the records that
/// could not be read, because every later section would describe evidence that cannot
/// exist. Free text placed in a table cell has its pipes escaped and its line breaks
/// flattened, so a reason can never split a row. Every per-pair and aggregate token figure
/// is a reduction, spelled by [`amount`]: a negative reduction means Majordomus used more
/// tokens, and no figure is negated on the way to the page.
///
/// ```
/// use majordomus_cli::economics::{report, summarize};
/// let dir = tempfile::tempdir().unwrap();
/// let s = summarize(dir.path(), &Default::default());
/// let text = report::markdown(&s);
/// assert!(text.starts_with("# Token economics: benchmark report"));
/// assert!(text.contains(&s.verdict.statement));
/// assert!(!s.present && !text.contains("## Metrics"));
/// ```
pub fn markdown(s: &EconomicsSummary) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "# Token economics: benchmark report\n");
    let _ = writeln!(o, "Generated from recorded evidence by `majordomus generate economics`. Every figure below is computed by one calculator (`apps/majordomus-cli/src/economics`) from the raw runs under `.ai/repo/benchmarks/economics/runs/`; nothing here is typed by hand.\n");
    let _ = writeln!(o, "## Verdict\n");
    let _ = writeln!(o, "{}\n", s.verdict.statement);
    if !s.verdict.unmet.is_empty() {
        let _ = writeln!(o, "The publication rule is not met:\n");
        for u in &s.verdict.unmet {
            let _ = writeln!(o, "- {u}");
        }
        let _ = writeln!(o);
    }
    if !s.diagnostics.is_empty() {
        let _ = writeln!(o, "## Records that could not be read\n");
        let _ = writeln!(o, "Nothing in them is counted anywhere below.\n");
        for d in &s.diagnostics {
            let _ = writeln!(o, "- {d}");
        }
        let _ = writeln!(o);
    }
    if !s.present {
        return o;
    }
    let _ = writeln!(o, "## Question and methodology\n");
    if let Some(q) = &s.question {
        let _ = writeln!(o, "{q}\n");
    }
    let _ = writeln!(
        o,
        "Methodology version {}; primary metric `{}`. The methodology is `.ai/repo/benchmarks/economics/methodology.yaml`.\n",
        s.methodology.unwrap_or(0),
        s.primary_metric.clone().unwrap_or_default()
    );
    let _ = writeln!(o, "### Control and treatment\n");
    for v in &s.variants {
        let _ = writeln!(
            o,
            "- **{}** ({}, `{}`): {}",
            v.title, v.role, v.id, v.description
        );
        if !v.excludes.is_empty() {
            let _ = writeln!(o, "  Excludes: {}.", v.excludes.join(", "));
        }
    }
    let _ = writeln!(o);
    if let Some(p) = &s.publication {
        let _ = writeln!(o, "### When a total-token claim may be published\n");
        let _ = writeln!(
            o,
            "At least {} valid matched pairs; at least {} task categories with {} valid pairs each; at least {} repetitions of every task and variant; at least {} in 10000 attempted pairs valid; a bootstrap interval no wider than {} in 10000; evidence {}; no run recorded from a working tree with uncommitted changes.\n",
            p.min_valid_pairs,
            p.min_categories,
            p.min_pairs_per_category,
            p.min_repetitions,
            p.min_valid_pair_rate_bp,
            p.max_interval_width_bp,
            if p.require_current { "current" } else { "of any age" }
        );
    }
    let _ = writeln!(o, "## Evidence\n");
    let _ = writeln!(o, "| suite | kind | freshness | runs | valid pairs | attempted | control failed | treatment failed | both failed | other | revisions | harness | models |");
    let _ = writeln!(o, "|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for v in &s.suites {
        let _ = writeln!(
            o,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            v.id,
            v.kind,
            words(&v.freshness),
            v.runs,
            v.pairs.valid,
            v.pairs.attempted,
            v.pairs.control_failed,
            v.pairs.treatment_failed,
            v.pairs.both_failed,
            v.pairs.other,
            v.revisions
                .iter()
                .map(|r| format!("`{}`", &r[..r.len().min(12)]))
                .collect::<Vec<_>>()
                .join(" "),
            v.harnesses.join(", "),
            v.models_reported.join(", ")
        );
    }
    let _ = writeln!(o);
    for v in s.suites.iter().filter(|v| v.freshness_detail.is_some()) {
        let _ = writeln!(
            o,
            "- `{}`: {}",
            v.id,
            v.freshness_detail.as_deref().unwrap_or("")
        );
    }
    let _ = writeln!(o, "\n## Metrics\n");
    let _ = writeln!(
        o,
        "| metric | value | status | class | n | interval | what it is not |"
    );
    let _ = writeln!(o, "|---|---|---|---|---|---|---|");
    for m in &s.metrics {
        let class = match m.inputs {
            Some(i) => format!("{} from {}", m.class.word(), i.word()),
            None => m.class.word().to_string(),
        };
        let interval = m
            .interval
            .as_ref()
            .map(|i| interval_text(i, &m.unit))
            .unwrap_or_else(|| "—".into());
        let _ = writeln!(
            o,
            "| `{}` | {} | {} | {} | {} | {} | {} |",
            m.id,
            value(m),
            words(&m.status),
            class,
            m.n,
            interval,
            cell(m.not.as_deref().unwrap_or(""))
        );
    }
    let _ = writeln!(o, "\nFormulas and warnings:\n");
    for m in &s.metrics {
        let _ = writeln!(o, "- `{}`: {}.", m.id, m.formula);
        if let Some(i) = &m.interval {
            let _ = writeln!(
                o,
                "  - interval: {}; {} resamples, seed {}.",
                i.method, i.resamples, i.seed
            );
        }
        for w in &m.warnings {
            let _ = writeln!(o, "  - {w}");
        }
    }
    let _ = writeln!(o);
    if !s.segments.is_empty() {
        let _ = writeln!(o, "## Segments\n");
        let _ = writeln!(o, "Median per-pair total-token reduction over valid pairs, by segment: positive means Majordomus used fewer tokens, and a negative reduction means Majordomus used more tokens.\n");
        let _ = writeln!(
            o,
            "| dimension | segment | valid pairs | median reduction | lowest | highest |"
        );
        let _ = writeln!(o, "|---|---|---|---|---|---|");
        for g in &s.segments {
            if let Some(d) = &g.token_reduction {
                let _ = writeln!(
                    o,
                    "| {} | {} | {} | {} | {} | {} |",
                    g.dimension,
                    g.value,
                    g.n,
                    percent_text(d.median),
                    percent_text(d.min),
                    percent_text(d.max)
                );
            }
        }
        let _ = writeln!(o);
    }
    let live: Vec<_> = s.pairs.iter().collect();
    if !live.is_empty() {
        let _ = writeln!(o, "## Pairs\n");
        let _ = writeln!(o, "Every declared pair, valid or not. Tokens are the provider-reported totals of every session of the run.\n");
        let _ = writeln!(o, "| task | category | rep | status | control tokens | treatment tokens | token reduction | cost reduction | first-request overhead | reasons |");
        let _ = writeln!(o, "|---|---|---|---|---|---|---|---|---|---|");
        for p in live {
            let _ = writeln!(
                o,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                p.task,
                p.category,
                p.repetition,
                words(&p.status),
                p.control_usage
                    .as_ref()
                    .map(|u| u.total.to_string())
                    .unwrap_or_else(|| "—".into()),
                p.treatment_usage
                    .as_ref()
                    .map(|u| u.total.to_string())
                    .unwrap_or_else(|| "—".into()),
                p.token_reduction
                    .map(percent_text)
                    .unwrap_or_else(|| "—".into()),
                p.cost_reduction
                    .map(percent_text)
                    .unwrap_or_else(|| "—".into()),
                p.first_request_overhead
                    .map(|x| signed_count(x as f64))
                    .unwrap_or_else(|| "—".into()),
                cell(&p.reasons.join("; "))
            );
        }
        let _ = writeln!(o, "\n\"Token reduction\" is `1 - treatment / control` on total tokens, and \"cost reduction\" the same on the harness's cost projection: positive means Majordomus used fewer tokens, and a negative reduction means Majordomus used more tokens. \"First-request overhead\" is not a reduction: it is the treatment's first-request input minus the control's, so positive means Majordomus added tokens before the model acted. An excluded pair shows its reductions so that the outlier rule's effect is visible; only valid pairs enter the metrics.\n");
        let valid = s
            .pairs
            .iter()
            .filter(|p| p.status == EconomicsPairStatus::Valid)
            .count();
        let _ = writeln!(
            o,
            "{valid} of {} declared pair(s) are valid.\n",
            s.pairs.len()
        );
    }
    if let Some(c) = &s.context {
        let _ = writeln!(o, "## Context selection (deterministic)\n");
        let _ = writeln!(
            o,
            "At revision `{}`, {} seeds (the issues of this repository's plan) were compiled by `majordomus devcontext` under its default budget of {} (bytes-over-four) tokens. Counted with {} ({}): {} candidate tokens (files the compiler judged relevant), {} selected; {} tokens reached in all. The compiler's own estimate of the selected tokens was {}; {} seed(s) exceed the budget when counted.\n",
            &c.revision[..c.revision.len().min(12)],
            c.seeds,
            c.budget_tokens,
            c.tokenizer.encoding,
            c.tokenizer.implementation,
            c.candidate_tokens,
            c.selected_tokens,
            c.considered_tokens,
            c.estimated_selected_tokens,
            c.seeds_over_budget_counted
        );
        let _ = writeln!(o, "This is context *selection*, not total token savings: it says what the compiler put in front of a worker out of what it found relevant, not what a session consumed.\n");
        let unmeasured = s
            .metrics
            .iter()
            .find(|m| m.id == CONTEXT_REDUCTION)
            .filter(|m| m.status == EconomicsMetricStatus::NotMeasured);
        if let Some(m) = unmeasured {
            let _ = writeln!(
                o,
                "No figure is taken from this record: `{}` is not measured ({}).\n",
                m.id,
                m.warnings.join("; ")
            );
        }
    }
    if !s.history.is_empty() {
        let _ = writeln!(o, "## History\n");
        let _ = writeln!(
            o,
            "| suite | methodology | revision | recorded | metric | value | n |"
        );
        let _ = writeln!(o, "|---|---|---|---|---|---|---|");
        for h in &s.history {
            let _ = writeln!(
                o,
                "| {} | {} | `{}` | {} | `{}` | {} | {} |",
                h.suite,
                h.methodology,
                &h.revision[..h.revision.len().min(12)],
                h.at,
                h.metric,
                percent_text(h.value),
                h.n
            );
        }
        let _ = writeln!(o);
    }
    let _ = writeln!(o, "## Hypotheses\n");
    let _ = writeln!(o, "Stated before any evidence existed, so that the evidence can refute them. They are not results.\n");
    for h in &s.hypotheses {
        let _ = writeln!(o, "- `{}` ({}): {}", h.id, h.status, h.statement);
    }
    let _ = writeln!(o, "\n## Reproduce\n");
    let _ = writeln!(o, "```sh");
    let _ = writeln!(o, "majordomus economics references        # every task fails on its start and passes on its reference; no model");
    let _ = writeln!(
        o,
        "majordomus economics measure            # the context suite; deterministic, no model"
    );
    let _ = writeln!(o, "majordomus economics run --suite pilot  # live sessions; needs a Claude Code login and spends usage");
    let _ = writeln!(
        o,
        "majordomus economics summary            # recompute every figure from the recorded runs"
    );
    let _ = writeln!(o, "majordomus economics explain effective_token_reduction");
    let _ = writeln!(o, "```\n");
    let _ = writeln!(o, "## Limitations\n");
    let _ = writeln!(o, "- One fixture repository, small by design; a large repository is where context selection matters most, and it is not measured here.");
    let _ = writeln!(o, "- One harness (Claude Code) and the model each suite names. Tokens are the provider's; counts from different providers are never compared directly.");
    let _ = writeln!(o, "- Sessions are stochastic. A pair is one sample of each arm; only the distribution over many pairs says anything.");
    let _ = writeln!(
        o,
        "- Cost is the harness's own projection at run time, not a bill."
    );
    let _ = writeln!(
        o,
        "- Context reduction is measured against what the compiler itself judged relevant."
    );
    let _ = writeln!(o, "- The context suite counts the corpus at the revision it recorded, not at the current one: a later change to the plan or to the files it selects from moves nothing until the suite is measured again.");
    let _ = writeln!(o, "- The treatment is Majordomus as a user who installs only the shell tool gets it, and its generated bootstrap advises commands that installation cannot run (`majordomus worktree`). What a session spends on them is part of what that user gets, and it is measured as such, not corrected for.");
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economics::model::{EconomicsClass, EconomicsPair, EconomicsVerdict};

    fn metric(unit: &str, value: Option<f64>) -> EconomicsMetric {
        EconomicsMetric {
            id: "effective_token_reduction".into(),
            title: "Effective token reduction".into(),
            class: EconomicsClass::Derived,
            inputs: Some(EconomicsClass::Observed),
            unit: unit.into(),
            formula: "1 - treatment / control".into(),
            not: None,
            value,
            status: EconomicsMetricStatus::Preliminary,
            n: 3,
            distribution: None,
            interval: None,
            suite: "pilot".into(),
            evidence: vec![],
            warnings: vec![],
        }
    }

    fn pair(reasons: &[&str]) -> EconomicsPair {
        EconomicsPair {
            suite: "pilot".into(),
            task: "vat-rounding".into(),
            category: "bugfix".into(),
            sessions: 1,
            repetition: 1,
            control: None,
            treatment: None,
            status: EconomicsPairStatus::Incomparable,
            reasons: reasons.iter().map(|r| r.to_string()).collect(),
            control_usage: None,
            treatment_usage: None,
            token_reduction: None,
            cost_reduction: None,
            tool_call_reduction: None,
            continuation_reduction: None,
            first_request_overhead: None,
        }
    }

    fn summary(present: bool, pairs: Vec<EconomicsPair>) -> EconomicsSummary {
        EconomicsSummary {
            present,
            methodology: Some(1),
            question: Some("How many tokens does a session consume?".into()),
            primary_metric: Some("effective_token_reduction".into()),
            verdict: EconomicsVerdict {
                publishable: false,
                statement: "No verified total-token-savings claim is available.".into(),
                unmet: vec!["at least 30 valid matched pairs".into()],
            },
            metrics: vec![metric("ratio", Some(0.2))],
            suites: vec![],
            pairs,
            segments: vec![],
            context: None,
            history: vec![],
            variants: vec![],
            hypotheses: vec![],
            publication: None,
            diagnostics: vec![],
        }
    }

    #[test]
    fn a_ratio_is_a_signed_percentage_with_one_decimal() {
        assert_eq!(value(&metric("ratio", Some(0.2))), "+20.0%");
        assert_eq!(value(&metric("ratio", Some(-0.125))), "-12.5%");
        assert_eq!(value(&metric("ratio", Some(0.0))), "0.0%");
        assert_eq!(
            value(&metric("ratio", Some(-0.0004))),
            "0.0%",
            "never -0.0%"
        );
    }

    #[test]
    fn a_count_carries_its_unit_and_an_absent_value_is_not_measured() {
        assert_eq!(value(&metric("tokens", Some(1500.0))), "+1500 tokens");
        assert_eq!(value(&metric("calls", Some(-4.0))), "-4 calls");
        assert_eq!(value(&metric("ratio", None)), "not measured");
        assert_eq!(value(&metric("tokens", None)), "not measured");
    }

    #[test]
    fn without_a_methodology_the_report_stops_after_the_verdict() {
        let text = markdown(&summary(false, vec![pair(&["control run missing"])]));
        assert!(text.contains("## Verdict"));
        assert!(text.contains("No verified total-token-savings claim is available."));
        assert!(text.contains("- at least 30 valid matched pairs"));
        assert!(text
            .trim_end()
            .ends_with("- at least 30 valid matched pairs"));
        for later in [
            "## Question and methodology",
            "## Evidence",
            "## Metrics",
            "## Pairs",
        ] {
            assert!(
                !text.contains(later),
                "{later} rendered without a methodology"
            );
        }
        assert!(!text.contains("## Limitations"));
    }

    #[test]
    fn a_pipe_inside_a_reason_is_escaped_in_the_pairs_table() {
        let text = markdown(&summary(
            true,
            vec![pair(&["model a|b differs", "no usage"])],
        ));
        let row = text
            .lines()
            .find(|l| l.starts_with("| vat-rounding |"))
            .expect("the pair has a row");
        assert!(row.ends_with("| model a\\|b differs; no usage |"), "{row}");
        let unescaped = row.replace("\\|", "");
        assert_eq!(unescaped.matches('|').count(), 11, "{row}");
    }

    fn valid(reduction: f64) -> EconomicsPair {
        EconomicsPair {
            status: EconomicsPairStatus::Valid,
            reasons: vec![],
            token_reduction: Some(reduction),
            cost_reduction: Some(reduction),
            first_request_overhead: Some(0),
            ..pair(&[])
        }
    }

    #[test]
    fn a_pair_shows_its_reduction_with_the_sign_the_calculator_gave_it() {
        let text = markdown(&summary(
            true,
            vec![valid(-0.1), valid(0.25), valid(-0.0001)],
        ));
        assert!(
            text.contains("| token reduction | cost reduction |"),
            "{text}"
        );
        assert!(!text.contains("token change"), "{text}");
        let rows: Vec<&str> = text
            .lines()
            .filter(|l| l.starts_with("| vat-rounding |"))
            .collect();
        assert_eq!(rows.len(), 3);
        assert!(
            rows[0].contains("| valid | — | — | -10.0% | -10.0% | 0 |"),
            "{}",
            rows[0]
        );
        assert!(rows[1].contains("| +25.0% | +25.0% |"), "{}", rows[1]);
        assert!(rows[2].contains("| 0.0% | 0.0% |"), "{}", rows[2]);
        assert!(!text.contains("-0.0%") && !text.contains("+-"), "{text}");
        assert!(text.contains("a negative reduction means Majordomus used more tokens"));
    }

    #[test]
    fn a_state_is_spelt_as_it_serialises_and_an_interval_at_its_declared_level() {
        use crate::economics::model::{
            EconomicsFreshness, EconomicsInterval, EconomicsPairCounts, EconomicsSuiteView,
        };
        let mut s = summary(
            true,
            vec![EconomicsPair {
                status: EconomicsPairStatus::ControlFailed,
                ..pair(&[])
            }],
        );
        s.metrics[0].interval = Some(EconomicsInterval {
            level_bp: 9750,
            low: -0.05,
            high: 0.3,
            method: "cluster bootstrap over tasks".into(),
            resamples: 2000,
            seed: 7,
        });
        s.suites.push(EconomicsSuiteView {
            id: "pilot".into(),
            kind: "live".into(),
            version: 1,
            title: "Pilot".into(),
            model: None,
            freshness: EconomicsFreshness::NoEvidence,
            freshness_detail: None,
            runs: 0,
            pairs: EconomicsPairCounts::default(),
            revisions: vec![],
            harnesses: vec![],
            models_reported: vec![],
        });
        let text = markdown(&s);
        assert!(text.contains("| pilot | live | no evidence |"), "{text}");
        assert!(text.contains("| control failed |"), "{text}");
        assert!(text.contains("| 97.5%: -5.0% to +30.0% |"), "{text}");
        assert!(
            text.contains("  - interval: cluster bootstrap over tasks; 2000 resamples, seed 7.")
        );
        for debug in ["noevidence", "ControlFailed", "NoEvidence"] {
            assert!(!text.contains(debug), "{debug} in {text}");
        }
    }

    #[test]
    fn records_that_could_not_be_read_are_listed_even_without_a_methodology() {
        let mut s = summary(false, vec![]);
        s.diagnostics = vec!["runs/pilot/r1.json: expected value at line 1".into()];
        let text = markdown(&s);
        assert!(text.contains("## Records that could not be read"));
        assert!(text.contains("- runs/pilot/r1.json: expected value at line 1"));
        s.diagnostics.clear();
        assert!(
            !markdown(&s).contains("could not be read"),
            "no problem, no section"
        );
    }

    #[test]
    fn the_limitations_name_the_shell_only_bootstrap_and_the_recorded_revision() {
        let text = markdown(&summary(true, vec![]));
        let limits = &text[text.find("## Limitations").expect("a limitations section")..];
        assert!(limits.contains("`majordomus worktree`"), "{limits}");
        assert!(limits.contains("measured as such"), "{limits}");
        assert!(
            limits.contains("at the revision it recorded, not at the current one"),
            "{limits}"
        );
    }

    #[test]
    fn the_metrics_table_spells_the_value_as_value_does() {
        let text = markdown(&summary(true, vec![]));
        assert!(text.contains(
            "| `effective_token_reduction` | +20.0% | preliminary | derived from observed | 3 | — |"
        ));
        assert!(!text.contains("## Pairs"));
    }
}
