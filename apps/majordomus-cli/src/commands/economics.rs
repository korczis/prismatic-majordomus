//! `majordomus economics`: token economics, measured.
//!
//! `summary`, `explain`, `runs` and `check` render what the registry's `economics.*`
//! capabilities answer; the numbers are computed there, once, and the terminal, `--format
//! json`, HTTP and MCP are renderings of the same execution. `measure`, `run` and
//! `references` are the command line's own: they produce evidence (writing records into the
//! repository, spending provider usage, or building workspaces), which no read-only
//! projection offers.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{
    EconomicsArgs, EconomicsCheckArgs, EconomicsCommand, EconomicsExplainArgs,
    EconomicsMeasureArgs, EconomicsReferencesArgs, EconomicsRunArgs, EconomicsRunsArgs,
    EconomicsSummaryArgs, OutputFormat, RepoArgs,
};
use crate::economics::claims::EconomicsCheckReport;
use crate::economics::model::{
    EconomicsExplanation, EconomicsMetric, EconomicsRunList, EconomicsSummary,
};
use crate::economics::report::{amount, interval_text, words};
use crate::economics::stats::percent_text;
use crate::economics::{self, runner};
use crate::error::{Error, Result};

/// The exit code when a check finds something, a measurement is incomplete or a run failed:
/// the code every unmet contract in this executable uses.
pub const EXIT_FINDINGS: u8 = 10;

/// Run `majordomus economics`.
pub fn run(args: EconomicsArgs) -> Result<u8> {
    match args.command {
        EconomicsCommand::Summary(a) => summary(a),
        EconomicsCommand::Explain(a) => explain(a),
        EconomicsCommand::Runs(a) => runs(a),
        EconomicsCommand::Check(a) => check(a),
        EconomicsCommand::Measure(a) => measure(a),
        EconomicsCommand::Run(a) => live(a),
        EconomicsCommand::References(a) => references(a),
    }
}

fn map(e: CapabilityError) -> Error {
    match e {
        CapabilityError::InvalidInput(reason) | CapabilityError::Refused(reason) => {
            Error::Refused {
                code: crate::cli::EXIT_USAGE,
                reason,
            }
        }
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

fn ask(repo: &RepoArgs, path: &[&str], input: Value) -> Result<Value> {
    let app = App::load(repo)?;
    let ctx = &app.context;
    let path: Vec<String> = path.iter().map(|s| s.to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&path)
        .map(|c| c.id.as_str().to_string())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })?;
    ctx.execute(&id, input).map_err(map)
}

fn read<T: serde::de::DeserializeOwned>(value: &Value, what: &str) -> Result<T> {
    serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
        reason: format!("{what} answered with something this command cannot read: {e}"),
    })
}

fn print_json(value: &Value) -> Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
    )
    .map_err(Error::Transport)
}

/// A metric's value as a person reads it, spelled as every surface spells it
/// ([`amount`]): a ratio as a signed percentage, which for a reduction is positive when
/// Majordomus used less, and tokens as a signed count. The value is the capability's; only
/// its spelling is decided here, and an absent value is a dash, never a zero.
pub fn value_text(m: &EconomicsMetric) -> String {
    match m.value {
        None => "—".into(),
        Some(v) => amount(v, &m.unit),
    }
}

/// A spread (a standard deviation) in the metric's unit. It has a size and no direction, so
/// unlike a reduction it carries no sign: a ratio is a percentage with one decimal, any other
/// unit a whole number and the unit, and a value that is not a number prints `n/a`.
fn spread_text(v: f64, unit: &str) -> String {
    if !v.is_finite() {
        return "n/a".into();
    }
    if unit == "ratio" {
        format!("{:.1}%", (v * 100.0).abs())
    } else {
        format!("{:.0} {unit}", v.abs())
    }
}

fn class_text(m: &EconomicsMetric) -> String {
    match m.inputs {
        Some(i) => format!("{} from {}", m.class.word(), i.word()),
        None => m.class.word().to_string(),
    }
}

fn render_summary(out: &mut impl Write, s: &EconomicsSummary) -> std::io::Result<()> {
    writeln!(
        out,
        "Token economics{}",
        s.methodology
            .map(|m| format!(" (methodology {m})"))
            .unwrap_or_default()
    )?;
    if let Some(q) = &s.question {
        writeln!(out, "question  {q}")?;
    }
    writeln!(out)?;
    writeln!(out, "verdict   {}", s.verdict.statement)?;
    for u in &s.verdict.unmet {
        writeln!(out, "  unmet   {u}")?;
    }
    if !s.suites.is_empty() {
        writeln!(out, "\nsuites")?;
        for v in &s.suites {
            let pairs = if v.kind == "live" {
                format!(
                    "  pairs {}/{} valid of {} declared (control failed {}, treatment failed {}, both failed {}, other {})",
                    v.pairs.valid, v.pairs.attempted, v.pairs.declared, v.pairs.control_failed,
                    v.pairs.treatment_failed, v.pairs.both_failed, v.pairs.other
                )
            } else {
                String::new()
            };
            writeln!(
                out,
                "  {:<10} {:<8} {:<12} {} run(s){}",
                v.id,
                v.kind,
                words(&v.freshness),
                v.runs,
                pairs
            )?;
            if let Some(d) = &v.freshness_detail {
                writeln!(out, "             {d}")?;
            }
        }
    }
    writeln!(out, "\nmetrics")?;
    for m in &s.metrics {
        let interval = m
            .interval
            .as_ref()
            .map(|i| format!("  CI {}", interval_text(i, &m.unit)))
            .unwrap_or_default();
        writeln!(
            out,
            "  {:<44} {:>10}  {:<13} {:<24} n={}{}",
            m.id,
            value_text(m),
            words(&m.status),
            class_text(m),
            m.n,
            interval
        )?;
    }
    if !s.segments.is_empty() {
        writeln!(
            out,
            "\nsegments (median total-token reduction over valid pairs; a negative reduction means Majordomus used more tokens)"
        )?;
        for g in &s.segments {
            let med = g
                .token_reduction
                .as_ref()
                .map(|d| percent_text(d.median))
                .unwrap_or_else(|| "—".into());
            writeln!(
                out,
                "  {:<11} {:<20} {:>8}  n={}",
                g.dimension, g.value, med, g.n
            )?;
        }
    }
    if let Some(c) = &s.context {
        writeln!(
            out,
            "\ncontext   {} seed(s) at {}, {} candidate token(s), {} selected, counted with {}",
            c.seeds,
            &c.revision[..c.revision.len().min(12)],
            c.candidate_tokens,
            c.selected_tokens,
            c.tokenizer.encoding
        )?;
    }
    if !s.diagnostics.is_empty() {
        writeln!(out, "\nrecords that could not be read")?;
        for d in &s.diagnostics {
            writeln!(out, "WARN      {d}")?;
        }
    }
    writeln!(
        out,
        "\nexplain any metric: majordomus economics explain <metric>"
    )?;
    Ok(())
}

fn summary(a: EconomicsSummaryArgs) -> Result<u8> {
    let value = ask(
        &a.repo,
        &["economics", "summary"],
        json!({ "suite": a.suite, "category": a.category, "task": a.task, "model": a.model }),
    )?;
    match a.format {
        OutputFormat::Json => print_json(&value)?,
        OutputFormat::Text => {
            let s: EconomicsSummary = read(&value, "economics.summary")?;
            render_summary(&mut std::io::stdout().lock(), &s).map_err(Error::Transport)?;
        }
    }
    Ok(0)
}

fn explain(a: EconomicsExplainArgs) -> Result<u8> {
    let value = ask(
        &a.repo,
        &["economics", "explain"],
        json!({ "metric": a.metric }),
    )?;
    if let OutputFormat::Json = a.format {
        print_json(&value)?;
        return Ok(0);
    }
    let e: EconomicsExplanation = read(&value, "economics.explain")?;
    let m = &e.metric;
    let mut out = std::io::stdout().lock();
    let w =
        |out: &mut std::io::StdoutLock, s: String| writeln!(out, "{s}").map_err(Error::Transport);
    w(&mut out, format!("{}  —  {}", m.id, m.title))?;
    w(
        &mut out,
        format!("value        {}   ({})", value_text(m), words(&m.status)),
    )?;
    w(
        &mut out,
        format!("class        {}: {}", class_text(m), e.class_meaning),
    )?;
    w(&mut out, format!("formula      {}", m.formula))?;
    if let Some(not) = &m.not {
        w(&mut out, format!("not          {not}"))?;
    }
    w(
        &mut out,
        format!("sample       n={} (methodology {})", m.n, e.methodology),
    )?;
    if let Some(d) = &m.distribution {
        let a = |v: f64| amount(v, &m.unit);
        w(
            &mut out,
            format!(
                "distribution median {}  mean {}  p25 {}  p75 {}  p95 {}  min {}  max {}{}",
                a(d.median),
                a(d.mean),
                a(d.p25),
                a(d.p75),
                a(d.p95),
                a(d.min),
                a(d.max),
                d.sd.map(|x| format!("  sd {}", spread_text(x, &m.unit)))
                    .unwrap_or_default()
            ),
        )?;
    }
    if let Some(i) = &m.interval {
        w(
            &mut out,
            format!(
                "interval     {} ({}; {} resamples, seed {})",
                interval_text(i, &m.unit),
                i.method,
                i.resamples,
                i.seed
            ),
        )?;
    }
    for s in &e.suites {
        w(
            &mut out,
            format!(
                "suite        {} v{} ({}), {} run(s), freshness {}, revisions {}",
                s.id,
                s.version,
                s.kind,
                s.runs,
                words(&s.freshness),
                s.revisions.join(", ")
            ),
        )?;
    }
    for p in &e.pairs {
        w(
            &mut out,
            format!(
                "pair         {}/{}/r{}  {}  {}{}",
                p.suite,
                p.task,
                p.repetition,
                words(&p.status),
                p.token_reduction
                    .map(|r| format!("token reduction {}", percent_text(r)))
                    .unwrap_or_default(),
                if p.reasons.is_empty() {
                    String::new()
                } else {
                    format!("  ({})", p.reasons.join("; "))
                }
            ),
        )?;
    }
    if evidence_is_seeds(m) {
        w(
            &mut out,
            format!(
                "evidence     {} seed(s): {}",
                m.evidence.len(),
                m.evidence
                    .iter()
                    .take(12)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )?;
    }
    for x in &e.excluded {
        w(&mut out, format!("excluded     {}: {}", x.run, x.reason))?;
    }
    for warning in &m.warnings {
        w(&mut out, format!("WARN         {warning}"))?;
    }
    for r in &e.reproduce {
        w(&mut out, format!("reproduce    {r}"))?;
    }
    Ok(0)
}

/// Whether a metric rests on counted seeds rather than on pairs of runs.
fn evidence_is_seeds(m: &EconomicsMetric) -> bool {
    m.inputs == Some(crate::economics::model::EconomicsClass::Counted)
}

fn runs(a: EconomicsRunsArgs) -> Result<u8> {
    let value = ask(
        &a.repo,
        &["economics", "runs"],
        json!({ "suite": a.suite, "task": a.task, "variant": a.variant }),
    )?;
    if let OutputFormat::Json = a.format {
        print_json(&value)?;
        return Ok(0);
    }
    let l: EconomicsRunList = read(&value, "economics.runs")?;
    let mut out = std::io::stdout().lock();
    writeln!(out, "{} run(s)", l.count).map_err(Error::Transport)?;
    for r in &l.runs {
        let u = r.usage.as_ref();
        writeln!(
            out,
            "  {:<44} {:<10} {} session(s)  {:>10} tokens  {:>6} tool calls  {}",
            r.id,
            if r.completed { "completed" } else { "failed" },
            r.sessions,
            u.map(|u| u.total.to_string()).unwrap_or_else(|| "—".into()),
            u.map(|u| u.tool_calls.to_string())
                .unwrap_or_else(|| "—".into()),
            r.path
        )
        .map_err(Error::Transport)?;
    }
    for d in &l.diagnostics {
        writeln!(out, "WARN {d}").map_err(Error::Transport)?;
    }
    Ok(0)
}

fn check(a: EconomicsCheckArgs) -> Result<u8> {
    let value = ask(&a.repo, &["economics", "check"], json!({}))?;
    let r: EconomicsCheckReport = read(&value, "economics.check")?;
    match a.format {
        OutputFormat::Json => print_json(&value)?,
        OutputFormat::Text => {
            let mut out = std::io::stdout().lock();
            for f in &r.findings {
                writeln!(
                    out,
                    "FAIL {:<20} {}{} — {}",
                    f.kind,
                    f.path,
                    f.line.map(|l| format!(":{l}")).unwrap_or_default(),
                    f.detail
                )
                .map_err(Error::Transport)?;
            }
            writeln!(
                out,
                "economics check: {} file(s) scanned, {} bound claim(s) checked, {} finding(s)",
                r.scanned,
                r.claims_checked,
                r.findings.len()
            )
            .map_err(Error::Transport)?;
        }
    }
    Ok(if r.ok { 0 } else { EXIT_FINDINGS })
}

fn declarations(root: &Path) -> Result<economics::Declarations> {
    match economics::load(root) {
        Ok(Some(d)) => Ok(d),
        Ok(None) => Err(Error::NotFound {
            reason: format!(
                "no benchmark methodology is declared: {}/methodology.yaml does not exist",
                economics::DIR
            ),
        }),
        Err(e) => Err(Error::Refused {
            code: EXIT_FINDINGS,
            reason: format!(
                "the benchmark declarations do not read:\n  {}",
                e.join("\n  ")
            ),
        }),
    }
}

fn measure(a: EconomicsMeasureArgs) -> Result<u8> {
    let app = App::load(&a.repo)?;
    let root = PathBuf::from(&app.context.index.repository.root);
    let decl = declarations(&root)?;
    let suite = decl.suites.get(&a.suite).ok_or_else(|| Error::NotFound {
        reason: format!(
            "no suite {:?}; the suites are: {}",
            a.suite,
            decl.suites.keys().cloned().collect::<Vec<_>>().join(", ")
        ),
    })?;
    if suite.kind != "context" {
        return Err(Error::Refused {
            code: crate::cli::EXIT_USAGE,
            reason: format!(
                "suite {} is a {} suite: `majordomus economics run --suite {}` runs it",
                suite.id, suite.kind, suite.id
            ),
        });
    }
    let at = crate::peers::rfc3339(SystemTime::now());
    let (record, errors) =
        economics::context::measure(&app.context, &decl, suite, &at).map_err(|reason| {
            Error::Refused {
                code: EXIT_FINDINGS,
                reason,
            }
        })?;
    let mut err = std::io::stderr().lock();
    for e in &errors {
        let _ = writeln!(err, "WARN seed refused by the compiler: {e}");
    }
    let totals = (
        record.seeds.iter().map(|s| s.candidate_tokens).sum::<u64>(),
        record.seeds.iter().map(|s| s.selected_tokens).sum::<u64>(),
    );
    let short = &record.repository.commit[..record.repository.commit.len().min(12)];
    let path = root
        .join(economics::DIR)
        .join("runs")
        .join(&suite.id)
        .join(format!("{short}.json"));
    let mut out = std::io::stdout().lock();
    writeln!(
        out,
        "{}: {} seed(s) at {short}{}, {} candidate token(s), {} selected ({} {})",
        suite.id,
        record.seeds.len(),
        if record.repository.dirty {
            " (dirty)"
        } else {
            ""
        },
        totals.0,
        totals.1,
        record.tokenizer.encoding,
        record.tokenizer.implementation
    )
    .map_err(Error::Transport)?;
    if a.dry_run {
        writeln!(
            out,
            "dry run: nothing written (would write {})",
            path.strip_prefix(&root).unwrap_or(&path).display()
        )
        .map_err(Error::Transport)?;
    } else {
        std::fs::create_dir_all(path.parent().unwrap_or(&root)).map_err(Error::Transport)?;
        let mut text = serde_json::to_string_pretty(&record).map_err(|e| Error::Protocol {
            reason: e.to_string(),
        })?;
        text.push('\n');
        std::fs::write(&path, text).map_err(Error::Transport)?;
        writeln!(
            out,
            "recorded {}; read it with: majordomus economics summary",
            path.strip_prefix(&root).unwrap_or(&path).display()
        )
        .map_err(Error::Transport)?;
    }
    Ok(if errors.is_empty() { 0 } else { EXIT_FINDINGS })
}

fn default_work_dir(suite: &str) -> PathBuf {
    std::env::temp_dir()
        .join("majordomus-economics")
        .join(suite)
}

fn live(a: EconomicsRunArgs) -> Result<u8> {
    let app = App::load(&a.repo)?;
    let root = PathBuf::from(&app.context.index.repository.root);
    let decl = declarations(&root)?;
    let majordomus = root.join("bin/majordomus");
    if !majordomus.is_file() {
        return Err(Error::NotFound {
            reason: format!(
                "the treatment installs Majordomus from {}, which does not exist",
                majordomus.display()
            ),
        });
    }
    let opts = runner::RunOptions {
        root: root.clone(),
        suite: a.suite.clone(),
        tasks: a.tasks,
        repetitions: a.repetitions,
        work_dir: a.work_dir.unwrap_or_else(|| default_work_dir(&a.suite)),
        harness: a.harness,
        majordomus,
        parallel: a.parallel,
        force: a.force,
        dry_run: a.dry_run,
        session_timeout: Duration::from_secs(a.session_timeout),
    };
    let log = |line: String| {
        let _ = writeln!(
            std::io::stderr().lock(),
            "{} {line}",
            crate::peers::rfc3339(SystemTime::now())
        );
    };
    let (written, failed) = runner::run(&opts, &decl, &log).map_err(|reason| Error::Refused {
        code: crate::cli::EXIT_USAGE,
        reason,
    })?;
    let mut out = std::io::stdout().lock();
    for p in &written {
        writeln!(
            out,
            "recorded {}",
            p.strip_prefix(&root).unwrap_or(p).display()
        )
        .map_err(Error::Transport)?;
    }
    let failed: Vec<&String> = failed
        .iter()
        .filter(|f| !(a.dry_run && f.ends_with("dry run")))
        .collect();
    for f in &failed {
        writeln!(out, "FAIL {f}").map_err(Error::Transport)?;
    }
    writeln!(
        out,
        "{} run(s) recorded, {} failed to produce a record",
        written.len(),
        failed.len()
    )
    .map_err(Error::Transport)?;
    Ok(if failed.is_empty() { 0 } else { EXIT_FINDINGS })
}

fn references(a: EconomicsReferencesArgs) -> Result<u8> {
    let app = App::load(&a.repo)?;
    let root = PathBuf::from(&app.context.index.repository.root);
    let decl = declarations(&root)?;
    let work = a.work_dir.unwrap_or_else(|| default_work_dir("references"));
    let mut out = std::io::stdout().lock();
    match runner::check_references(&root, &decl, &work) {
        Ok(lines) => {
            for l in lines {
                writeln!(out, "ok   {l}").map_err(Error::Transport)?;
            }
            Ok(0)
        }
        Err(lines) => {
            for l in lines {
                writeln!(out, "FAIL {l}").map_err(Error::Transport)?;
            }
            Ok(EXIT_FINDINGS)
        }
    }
}
