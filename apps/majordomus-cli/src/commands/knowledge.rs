//! `majordomus knowledge`, `majordomus canonicality` and `majordomus explain`: the
//! command line's rendering of the repository knowledge system.
//!
//! Every read goes through the registry's own `knowledge.*` capabilities, so that the
//! command line, MCP, HTTP and the Cockpit answer from one scan and one function. The
//! three operations that write — recording the baseline, accepting a reconciliation and
//! running a semantic provider — are here and nowhere else, because a capability never
//! writes to the repository; they call the same functions of [`crate::knowledge`] the
//! capabilities read with.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{
    CanonicalityArgs, CanonicalityCommand, ChangeArgs, ChangeCommand, ExplainArgs, KnowledgeArgs,
    KnowledgeBaselineCommand as BaselineCommand, KnowledgeCommand, OutputFormat,
};
use crate::error::{Error, Result};
use crate::knowledge::baseline::{Baseline, Mode};
use crate::knowledge::{self, Inputs};

/// The exit code when a check, a validation or an audit refuses.
pub const EXIT_REFUSED: u8 = 10;

type Out<'a> = std::io::StdoutLock<'a>;

fn w(out: &mut Out<'_>, s: impl AsRef<str>) -> Result<()> {
    writeln!(out, "{}", s.as_ref()).map_err(Error::Transport)
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

fn map(e: CapabilityError) -> Error {
    match e {
        CapabilityError::NotFound(reason) => Error::Refused {
            code: 12,
            reason,
        },
        CapabilityError::InvalidInput(reason) => Error::Refused {
            code: crate::cli::EXIT_USAGE,
            reason,
        },
        CapabilityError::Refused(reason) => Error::Refused {
            code: EXIT_REFUSED,
            reason,
        },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

fn refused(reason: impl Into<String>) -> Error {
    Error::Refused {
        code: EXIT_REFUSED,
        reason: reason.into(),
    }
}

/// Execute the capability exposed at a command path.
fn call(app: &App, path: &[&str], input: Value) -> Result<Value> {
    let ctx = &app.context;
    let words: Vec<String> = path.iter().map(|w| (*w).to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!("no capability is exposed as `majordomus {}`", path.join(" ")),
        })?;
    ctx.execute(id, input).map_err(map)
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn n(v: &Value) -> u64 {
    v.as_u64().unwrap_or(0)
}

/// A vocabulary word from the command line: the model's spelling uses underscores, and
/// a person may type hyphens. A word outside the vocabulary is refused by the capability
/// with the words listed, so nothing is decided here.
fn word_arg(value: Option<String>) -> Value {
    match value {
        None => Value::Null,
        Some(v) => Value::String(v.replace('-', "_")),
    }
}

/// Run `majordomus knowledge`.
pub fn run(args: KnowledgeArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let format = args.format;
    match args.command {
        None | Some(KnowledgeCommand::Status) => status(&app, format, &mut out),
        Some(KnowledgeCommand::Bootstrap { force }) => bootstrap(&app, force, format, &mut out),
        Some(KnowledgeCommand::Scan { public }) => {
            let v = call(&app, &["knowledge", "scan"], json!({ "public": public }))?;
            w(&mut out, pretty(&v))?;
            Ok(0)
        }
        Some(KnowledgeCommand::List {
            kind,
            provenance,
            freshness,
            ownership,
            extractor,
            query,
            debt,
            offset,
            limit,
        }) => {
            let input = json!({
                "kind": kind,
                "provenance": word_arg(provenance),
                "freshness": word_arg(freshness),
                "ownership": word_arg(ownership),
                "extractor": extractor,
                "query": query,
                "debt": debt,
                "offset": offset,
                "limit": limit,
            });
            let v = call(&app, &["knowledge", "list"], strip_nulls(input))?;
            list(&v, format, &mut out)
        }
        Some(KnowledgeCommand::Stale) => {
            let v = call(&app, &["knowledge", "list"], json!({ "debt": true, "limit": 1000 }))?;
            match format {
                OutputFormat::Json => w(&mut out, pretty(&v))?,
                OutputFormat::Text => {
                    let nodes = v["nodes"].as_array().cloned().unwrap_or_default();
                    if nodes.is_empty() {
                        w(&mut out, "nothing is stale: every node is current")?;
                    }
                    for node in &nodes {
                        w(
                            &mut out,
                            format!(
                                "{:<14} {:<56} {}",
                                s(&node["freshness"]),
                                s(&node["id"]),
                                s(&node["freshness_reason"])
                            ),
                        )?;
                    }
                    w(&mut out, format!("{} node(s) in debt", n(&v["total"])))?;
                }
            }
            Ok(0)
        }
        Some(KnowledgeCommand::Show { id }) => {
            let v = call(&app, &["knowledge", "show"], json!({ "id": id }))?;
            show(&v, format, &mut out)
        }
        Some(KnowledgeCommand::Search { query, limit }) => {
            let v = call(&app, &["knowledge", "search"], json!({ "query": query, "limit": limit }))?;
            match format {
                OutputFormat::Json => w(&mut out, pretty(&v))?,
                OutputFormat::Text => {
                    for h in v["hits"].as_array().into_iter().flatten() {
                        w(
                            &mut out,
                            format!("{:<56} {:<8} {:<12} {}", s(&h["id"]), s(&h["matched"]), s(&h["freshness"]), s(&h["excerpt"])),
                        )?;
                    }
                }
            }
            Ok(0)
        }
        Some(KnowledgeCommand::Explain { id }) => explain(&app, &id, format, &mut out),
        Some(KnowledgeCommand::Graph {
            root,
            depth,
            kind,
            limit,
        }) => {
            let v = call(
                &app,
                &["knowledge", "graph"],
                strip_nulls(json!({ "root": root, "depth": depth, "kind": kind, "limit": limit })),
            )?;
            match format {
                OutputFormat::Json => w(&mut out, pretty(&v))?,
                OutputFormat::Text => {
                    for node in v["nodes"].as_array().into_iter().flatten() {
                        w(&mut out, format!("node {:<56} {:<12} {}", s(&node["id"]), s(&node["freshness"]), s(&node["title"])))?;
                    }
                    for e in v["edges"].as_array().into_iter().flatten() {
                        w(&mut out, format!("edge {} --{}--> {}", s(&e["source"]), s(&e["kind"]), s(&e["target"])))?;
                    }
                    if n(&v["omitted"]) > 0 {
                        w(&mut out, format!("{} node(s) omitted by the cap", n(&v["omitted"])))?;
                    }
                }
            }
            Ok(0)
        }
        Some(KnowledgeCommand::Impact { base, to, paths }) => {
            let v = call(
                &app,
                &["knowledge", "impact"],
                strip_nulls(json!({ "base": base, "to": to, "paths": paths })),
            )?;
            impact(&v, format, &mut out)
        }
        Some(KnowledgeCommand::Gaps { category }) => {
            let v = call(
                &app,
                &["knowledge", "gaps"],
                strip_nulls(json!({ "category": word_arg(category) })),
            )?;
            match format {
                OutputFormat::Json => w(&mut out, pretty(&v))?,
                OutputFormat::Text => {
                    for g in v["gaps"].as_array().into_iter().flatten() {
                        w(&mut out, format!("{:<24} {}", s(&g["category"]), s(&g["subject"])))?;
                        w(&mut out, format!("    {}", s(&g["reason"])))?;
                        w(&mut out, format!("    remedy: {}", s(&g["remedy"])))?;
                    }
                    let t = v["tallies"].as_object().cloned().unwrap_or_default();
                    let line: Vec<String> = t.iter().map(|(k, v)| format!("{k} {}", n(v))).collect();
                    w(&mut out, format!("gaps: {}", if line.is_empty() { "none".into() } else { line.join(", ") }))?;
                }
            }
            Ok(0)
        }
        Some(KnowledgeCommand::Coverage) => {
            let v = call(&app, &["knowledge", "coverage"], json!({}))?;
            match format {
                OutputFormat::Json => w(&mut out, pretty(&v))?,
                OutputFormat::Text => coverage_rows(&v["rows"], &mut out)?,
            }
            Ok(0)
        }
        Some(KnowledgeCommand::Conflicts { open_only }) => {
            let v = call(&app, &["knowledge", "conflicts"], json!({ "open_only": open_only }))?;
            match format {
                OutputFormat::Json => w(&mut out, pretty(&v))?,
                OutputFormat::Text => {
                    for c in v["conflicts"].as_array().into_iter().flatten() {
                        w(&mut out, format!("{} [{}] {}", s(&c["id"]), s(&c["severity"]), s(&c["resolution"])))?;
                        for side in c["sides"].as_array().into_iter().flatten() {
                            w(&mut out, format!("    {:<9} says {}", s(&side["provenance"]), side["value"]))?;
                        }
                        w(&mut out, format!("    remedy: {}", s(&c["remedy"])))?;
                    }
                    w(&mut out, format!("conflicts: {} open, {} accepted", n(&v["open"]), n(&v["accepted"])))?;
                }
            }
            Ok(0)
        }
        Some(KnowledgeCommand::Accept { conflict, reason }) => accept_conflict(&app, &conflict, &reason, format, &mut out),
        Some(KnowledgeCommand::Reconcile { accept }) => reconcile(&app, accept, format, &mut out),
        Some(KnowledgeCommand::Validate) => validate(&app, format, &mut out),
        Some(KnowledgeCommand::Baseline { command }) => match command {
            None | Some(BaselineCommand::Show) => baseline_show(&app, format, &mut out),
            Some(BaselineCommand::Record { force }) => record(&app, force, format, &mut out),
            Some(BaselineCommand::Migrate) => migrate(&app, format, &mut out),
        },
        Some(KnowledgeCommand::Check { mode }) => check(&app, mode, format, &mut out),
        Some(KnowledgeCommand::Canonicality { capability }) => canonicality(&app, capability, format, &mut out),
        Some(KnowledgeCommand::Derive { kinds, dry_run }) => derive(&app, kinds, dry_run, format, &mut out),
        Some(KnowledgeCommand::Extractors) => {
            let v = call(&app, &["knowledge", "extractors"], json!({}))?;
            match format {
                OutputFormat::Json => w(&mut out, pretty(&v))?,
                OutputFormat::Text => {
                    for e in v["extractors"].as_array().into_iter().flatten() {
                        w(&mut out, format!("{:<12} v{:<3} {}", s(&e["id"]), n(&e["version"]), s(&e["title"])))?;
                        w(&mut out, format!("    kinds: {}", names(&e["kinds"], "kind")))?;
                        w(&mut out, format!("    relations: {}", names(&e["relations"], "kind")))?;
                        w(&mut out, format!("    predicates: {}", names(&e["predicates"], "name")))?;
                    }
                    for p in v["providers"].as_array().into_iter().flatten() {
                        w(&mut out, format!("provider {:<8} remote={} operations={}", s(&p["id"]), p["remote"], names(&p["operations"], "")))?;
                    }
                }
            }
            Ok(0)
        }
        Some(KnowledgeCommand::Context {
            paths,
            budget,
            public,
        }) => {
            let v = call(
                &app,
                &["knowledge", "context"],
                json!({ "paths": paths, "budget": budget, "public_only": public }),
            )?;
            match format {
                OutputFormat::Json => w(&mut out, pretty(&v))?,
                OutputFormat::Text => {
                    for node in v["nodes"].as_array().into_iter().flatten() {
                        w(&mut out, format!("{:<14} {:<12} {:<56} {}", s(&node["kind"]), s(&node["freshness"]), s(&node["id"]), s(&node["title"])))?;
                    }
                    for c in v["caveats"].as_array().into_iter().flatten() {
                        w(&mut out, format!("caveat {} is {}: {}", s(&c["id"]), s(&c["freshness"]), s(&c["reason"])))?;
                    }
                    w(
                        &mut out,
                        format!(
                            "{} node(s), {} caveat(s), {} open conflict(s), {} gap(s), {} omitted by the budget of {} bytes",
                            v["nodes"].as_array().map(Vec::len).unwrap_or(0),
                            v["caveats"].as_array().map(Vec::len).unwrap_or(0),
                            v["conflicts"].as_array().map(Vec::len).unwrap_or(0),
                            v["gaps"].as_array().map(Vec::len).unwrap_or(0),
                            n(&v["omitted"]),
                            n(&v["budget"])
                        ),
                    )?;
                }
            }
            Ok(0)
        }
        Some(KnowledgeCommand::Inspect { base }) => inspect(&app, base, format, &mut out),
        Some(KnowledgeCommand::Ids { kind }) => {
            let v = call(&app, &["knowledge", "list"], strip_nulls(json!({ "kind": kind, "limit": 100_000 })))?;
            for node in v["nodes"].as_array().into_iter().flatten() {
                w(&mut out, s(&node["id"]))?;
            }
            Ok(0)
        }
    }
}

/// Run `majordomus canonicality`.
pub fn run_canonicality(args: CanonicalityArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.command {
        None | Some(CanonicalityCommand::Check) => canonicality(&app, None, args.format, &mut out),
        Some(CanonicalityCommand::Explain { capability }) => {
            canonicality(&app, Some(capability), args.format, &mut out)
        }
    }
}

/// Run `majordomus change`.
pub fn run_change(args: ChangeArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.command {
        None => inspect(&app, None, args.format, &mut out),
        Some(ChangeCommand::Inspect { base }) => inspect(&app, base, args.format, &mut out),
    }
}

fn inspect(app: &App, base: Option<String>, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["knowledge", "inspect"], strip_nulls(json!({ "base": base })))?;
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        OutputFormat::Text => {
            w(out, format!("change set: working tree against {}", s(&v["base"])))?;
            for c in v["impact"]["changed"].as_array().into_iter().flatten() {
                w(out, format!("  {:<10} {}", s(&c["status"]), s(&c["path"])))?;
            }
            let direct = v["impact"]["direct"].as_array().cloned().unwrap_or_default();
            let transitive = v["impact"]["transitive"].as_array().cloned().unwrap_or_default();
            w(out, format!("touches {} node(s) directly, {} along relations", direct.len(), transitive.len()))?;
            for a in direct.iter().chain(transitive.iter()).take(40) {
                w(out, format!("  {} {:<56} {}", n(&a["depth"]), s(&a["id"]), s(&a["reason"])))?;
            }
            let added = v["added_capabilities"].as_array().cloned().unwrap_or_default();
            if !added.is_empty() {
                w(out, format!("adds {} capability(ies):", added.len()))?;
                for c in &added {
                    w(out, format!("  {} declared in {} (MMS {})", s(&c["id"]), s(&c["declared_in"]), n(&c["mms"])))?;
                    for sf in c["surfaces"].as_array().into_iter().flatten() {
                        w(out, format!("    {} {:<10} {}", if sf["derived"].as_bool().unwrap_or(false) { "✓" } else { "✗" }, s(&sf["name"]), s(&sf["detail"])))?;
                    }
                    w(out, "    checklist: canonical source named above; MCP, HTTP, OpenAPI, CLI, docs, Cockpit and benchmark derived from it; no hand-written table to update")?;
                }
            }
            for d in v["new_debt"].as_array().into_iter().flatten() {
                w(out, format!("debt      {:<13} {:<12} {:<56} {}", s(&d["class"]), s(&d["state"]), s(&d["id"]), s(&d["reason"])))?;
            }
            for viol in v["violations"].as_array().into_iter().flatten() {
                w(out, format!("violation {:<22} {}", s(&viol["class"]), s(&viol["reason"])))?;
            }
            w(out, format!("change inspect: {} ({})", s(&v["verdict"]), s(&v["summary"])))?;
        }
    }
    if s(&v["verdict"]) == "fail" {
        return Err(refused("change inspect failed: the change introduces debt the baseline does not tolerate, or a canonicality violation that counts"));
    }
    Ok(0)
}

/// Run `majordomus explain`.
pub fn run_explain(args: ExplainArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let subject = match args.subject.as_slice() {
        [one] => one.clone(),
        [kind, id] => format!("{kind}:{id}"),
        _ => {
            return Err(Error::Refused {
                code: crate::cli::EXIT_USAGE,
                reason: "explain: one subject, or `capability <id>`".into(),
            })
        }
    };
    explain(&app, &subject, args.format, &mut out)
}

fn strip_nulls(v: Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.into_iter().filter(|(_, v)| !v.is_null()).collect()),
        other => other,
    }
}

fn names(v: &Value, key: &str) -> String {
    let items: Vec<String> = v
        .as_array()
        .into_iter()
        .flatten()
        .map(|x| if key.is_empty() { s(x).to_string() } else { s(&x[key]).to_string() })
        .collect();
    if items.is_empty() {
        "-".into()
    } else {
        items.join(", ")
    }
}

// ------------------------------------------------------------------- status

fn status(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["knowledge", "status"], json!({}))?;
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        OutputFormat::Text => status_text(&v, out)?,
    }
    Ok(0)
}

fn tallies(v: &Value) -> String {
    let m = v.as_object().cloned().unwrap_or_default();
    let mut parts: Vec<String> = m.iter().filter(|(_, n)| n.as_u64().unwrap_or(0) > 0).map(|(k, n)| format!("{k} {n}")).collect();
    if parts.is_empty() {
        parts.push("none".into());
    }
    parts.join(", ")
}

fn status_text(v: &Value, out: &mut Out<'_>) -> Result<()> {
    w(out, format!("repository   {} ({})", s(&v["repository"]["name"]), s(&v["repository"]["branch"])))?;
    w(out, format!("mode         {}", s(&v["mode"])))?;
    w(
        out,
        format!(
            "baseline     {}{}",
            if v["baseline"]["recorded"].as_bool().unwrap_or(false) { "recorded" } else { "not recorded (run `majordomus knowledge bootstrap`)" },
            v["baseline"]["recorded_at"].as_str().map(|r| format!(" at {}", &r[..r.len().min(12)])).unwrap_or_default()
        ),
    )?;
    w(out, format!("nodes        {} ({} claims, {} evidence, {} relations)", n(&v["nodes"]), n(&v["claims"]), n(&v["evidence"]), n(&v["relations"])))?;
    w(out, format!("kinds        {}", tallies(&v["kinds"])))?;
    w(out, format!("freshness    {}", tallies(&v["freshness"])))?;
    w(out, format!("provenance   {}", tallies(&v["provenance"])))?;
    w(out, format!("conflicts    {} open, {} accepted", n(&v["conflicts_open"]), n(&v["conflicts_accepted"])))?;
    w(out, format!("gaps         {}", tallies(&v["gaps"])))?;
    w(out, "coverage")?;
    coverage_rows(&v["coverage"]["rows"], out)?;
    let c = &v["check"];
    w(out, format!("check        {} ({})", s(&c["verdict"]), s(&c["summary"])))?;
    let a = &v["canonicality"];
    w(
        out,
        format!(
            "canonicality {} ({} capabilities, {} canonical, MMS {}.{:02}, {} violation(s), {} counting)",
            s(&a["verdict"]),
            n(&a["capabilities"]),
            n(&a["canonical"]),
            n(&a["mms_centi"]) / 100,
            n(&a["mms_centi"]) % 100,
            n(&a["violations"]),
            n(&a["counting"])
        ),
    )?;
    let sem = &v["semantic"];
    w(
        out,
        format!(
            "semantic     {} (provider {}, remote {}, {} derived claim(s))",
            if sem["enabled"].as_bool().unwrap_or(false) { "enabled" } else { "off" },
            s(&sem["provider"]),
            if sem["allow_remote"].as_bool().unwrap_or(false) { "allowed" } else { "refused" },
            n(&sem["derived_claims"])
        ),
    )?;
    w(out, format!("extractors   {}", names(&v["extractors"], "")))?;
    if n(&v["diagnostics"]) > 0 {
        w(out, format!("diagnostics  {} (see `majordomus knowledge validate`)", n(&v["diagnostics"])))?;
    }
    Ok(())
}

fn coverage_rows(rows: &Value, out: &mut Out<'_>) -> Result<()> {
    for r in rows.as_array().into_iter().flatten() {
        w(
            out,
            format!(
                "  {:<28} {:>5} / {:<5} {}",
                s(&r["id"]),
                n(&r["covered"]),
                n(&r["discovered"]),
                match r["missing"].as_array().map(Vec::len).unwrap_or(0) {
                    0 => String::new(),
                    k => format!("missing {k}"),
                }
            ),
        )?;
    }
    Ok(())
}

// --------------------------------------------------------------------- list

fn list(v: &Value, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    match format {
        OutputFormat::Json => w(out, pretty(v))?,
        OutputFormat::Text => {
            for node in v["nodes"].as_array().into_iter().flatten() {
                w(
                    out,
                    format!(
                        "{:<56} {:<14} {:<9} {}",
                        s(&node["id"]),
                        s(&node["freshness"]),
                        s(&node["provenance"]),
                        s(&node["freshness_reason"])
                    ),
                )?;
            }
            w(out, format!("{} of {} node(s) (offset {}, limit {})", v["nodes"].as_array().map(Vec::len).unwrap_or(0), n(&v["total"]), n(&v["offset"]), n(&v["limit"])))?;
        }
    }
    Ok(0)
}

// --------------------------------------------------------------------- show

fn show(v: &Value, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    match format {
        OutputFormat::Json => w(out, pretty(v))?,
        OutputFormat::Text => {
            let node = &v["node"];
            w(out, format!("{} — {}", s(&node["id"]), s(&node["title"])))?;
            if let Some(summary) = node["summary"].as_str() {
                w(out, format!("  {summary}"))?;
            }
            w(
                out,
                format!(
                    "  kind {} · provenance {} · ownership {} · visibility {} · extractor {}",
                    s(&node["kind"]),
                    s(&node["provenance"]),
                    s(&node["ownership"]),
                    s(&node["visibility"]),
                    s(&node["extractor"])
                ),
            )?;
            w(out, format!("  freshness {}{}", s(&node["freshness"]), node["freshness_reason"].as_str().map(|r| format!(": {r}")).unwrap_or_default()))?;
            if let Some(source) = node["source"].as_str() {
                w(out, format!("  source {source}"))?;
            }
            let claims = node["claims"].as_array().cloned().unwrap_or_default();
            w(out, format!("claims ({})", claims.len()))?;
            for c in &claims {
                w(
                    out,
                    format!(
                        "  {:<20} {:<9} {:<14} {}{}",
                        s(&c["predicate"]),
                        s(&c["provenance"]),
                        s(&c["freshness"]),
                        value_line(&c["value"]),
                        c["reason"].as_str().map(|r| format!("  ({r})")).unwrap_or_default()
                    ),
                )?;
            }
            let evidence = v["evidence"].as_array().cloned().unwrap_or_default();
            w(out, format!("evidence ({})", evidence.len()))?;
            for e in &evidence {
                w(out, format!("  {:<56} {:<10} {}", s(&e["id"]), s(&e["kind"]), s(&e["fingerprint"]["value"]).chars().take(12).collect::<String>()))?;
            }
            for (label, key) in [("relates to", "outgoing"), ("related from", "incoming")] {
                let rels = v[key].as_array().cloned().unwrap_or_default();
                if !rels.is_empty() {
                    w(out, format!("{label} ({})", rels.len()))?;
                    for r in &rels {
                        w(out, format!("  {} --{}--> {}", s(&r["source"]), s(&r["kind"]), s(&r["target"])))?;
                    }
                }
            }
            for c in v["conflicts"].as_array().into_iter().flatten() {
                w(out, format!("conflict {} [{}]: {}", s(&c["id"]), s(&c["severity"]), s(&c["basis"])))?;
            }
            for g in v["gaps"].as_array().into_iter().flatten() {
                w(out, format!("gap {}: {} — {}", s(&g["id"]), s(&g["reason"]), s(&g["remedy"])))?;
            }
        }
    }
    Ok(0)
}

fn value_line(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ------------------------------------------------------------------ explain

fn explain(app: &App, id: &str, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["knowledge", "explain"], json!({ "id": id }))?;
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        OutputFormat::Text => {
            w(out, format!("{} — {}", s(&v["id"]), s(&v["title"])))?;
            if s(&v["kind"]) == "capability" {
                // a capability's explanation opens with its canonicality: the one source
                // and what is derived from it, which is the question a person asks first
                let cid = s(&v["id"]).trim_start_matches("capability:").to_string();
                if let Ok(a) = call(app, &["knowledge", "canonicality"], json!({ "capability": cid })) {
                    if let Some(row) = a["capabilities"].as_array().and_then(|c| c.first()) {
                        w(out, format!("  canonical source: {}", s(&row["canonical_source"])))?;
                        for sf in row["surfaces"].as_array().into_iter().flatten() {
                            w(out, format!("  derived surface   {} {:<10} {}", if sf["derived"].as_bool().unwrap_or(false) { "✓" } else { "✗" }, s(&sf["name"]), s(&sf["detail"])))?;
                        }
                        w(out, format!("  manual maintenance surface: {} ({})", n(&row["mms"]), s(&row["verdict"])))?;
                        for m in row["mentions"].as_array().into_iter().flatten() {
                            w(out, format!("  mentioned by hand in {}:{}", s(&m["path"]), n(&m["line"])))?;
                        }
                    }
                }
            }
            for line in v["why"].as_array().into_iter().flatten() {
                w(out, format!("  {}", s(line)))?;
            }
            w(out, format!("  confidence: {}", s(&v["confidence"])))?;
            let evidence = v["evidence"].as_array().cloned().unwrap_or_default();
            w(out, format!("evidence ({})", evidence.len()))?;
            for e in &evidence {
                w(out, format!("  {:<56} {:<10} {} by {}{}", s(&e["id"]), s(&e["kind"]), s(&e["fingerprint"]), s(&e["extractor"]), if e["remote_processing"].as_bool().unwrap_or(false) { "" } else { " (never leaves the machine)" }))?;
            }
            let claims = v["claims"].as_array().cloned().unwrap_or_default();
            w(out, format!("claims ({})", claims.len()))?;
            for c in &claims {
                w(out, format!("  {:<20} {:<9} {:<14} {}{}", s(&c["predicate"]), s(&c["provenance"]), s(&c["freshness"]), value_line(&c["value"]), c["reason"].as_str().map(|r| format!("  ({r})")).unwrap_or_default()))?;
            }
            for r in v["relates_to"].as_array().into_iter().flatten() {
                w(out, format!("  relates to   {}", s(r)))?;
            }
            for r in v["related_from"].as_array().into_iter().flatten() {
                w(out, format!("  related from {}", s(r)))?;
            }
            for c in v["conflicts"].as_array().into_iter().flatten() {
                w(out, format!("conflict {} [{}]: {}", s(&c["id"]), s(&c["severity"]), s(&c["basis"])))?;
            }
            for g in v["gaps"].as_array().into_iter().flatten() {
                w(out, format!("gap {}: {}", s(&g["id"]), s(&g["reason"])))?;
            }
            let remedies = v["remedies"].as_array().cloned().unwrap_or_default();
            if !remedies.is_empty() {
                w(out, "what to do")?;
                for r in &remedies {
                    w(out, format!("  - {}", s(r)))?;
                }
            }
        }
    }
    Ok(0)
}

// ------------------------------------------------------------------- impact

fn impact(v: &Value, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    match format {
        OutputFormat::Json => w(out, pretty(v))?,
        OutputFormat::Text => {
            w(out, format!("change set {}{}", s(&v["change_set"]), v["base"].as_str().map(|b| format!(" against {b}")).unwrap_or_default()))?;
            for c in v["changed"].as_array().into_iter().flatten() {
                w(out, format!("  {:<10} {}", s(&c["status"]), s(&c["path"])))?;
            }
            for e in v["entries"].as_array().into_iter().flatten() {
                w(out, format!("  entry {:<10} {}#{}", s(&e["status"]), s(&e["path"]), s(&e["member"])))?;
            }
            let direct = v["direct"].as_array().cloned().unwrap_or_default();
            w(out, format!("directly affected ({})", direct.len()))?;
            for a in &direct {
                w(out, format!("  {:<56} {}", s(&a["id"]), s(&a["reason"])))?;
            }
            let transitive = v["transitive"].as_array().cloned().unwrap_or_default();
            w(out, format!("reached along relations ({})", transitive.len()))?;
            for a in &transitive {
                w(out, format!("  {} {:<56} {}", n(&a["depth"]), s(&a["id"]), s(&a["reason"])))?;
            }
            w(out, format!("claims resting on the change: {}", v["claims"].as_array().map(Vec::len).unwrap_or(0)))?;
            w(out, format!("extractors to re-run: {}", names(&v["extractors"], "")))?;
            let un = v["unmodelled"].as_array().cloned().unwrap_or_default();
            if !un.is_empty() {
                w(out, format!("paths nothing in the model knows about: {}", un.iter().map(s).collect::<Vec<_>>().join(", ")))?;
            }
        }
    }
    Ok(0)
}

// ------------------------------------------------------------- the writes

fn bootstrap(app: &App, force: bool, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let inputs = Inputs::from_app(app)?;
    let path = inputs.baseline_path();
    if path.exists() && !force {
        return Err(refused(format!(
            "a baseline is recorded at {}; `majordomus knowledge baseline record --force` records over it, and `majordomus knowledge check` is what runs after adoption",
            rel(app, &path)
        )));
    }
    let previous = inputs.baseline()?;
    let model = knowledge::scan(&inputs, &previous);
    let recorded = Baseline::record(&model, &previous);
    recorded.write(&path)?;
    knowledge::forget();
    // the model, held against what was just recorded: what adoption leaves
    let after = knowledge::scan(&inputs, &recorded);
    let check = knowledge::baseline::check(&after, &recorded, inputs.policy.mode);
    match format {
        OutputFormat::Json => w(out, pretty(&json!({
            "baseline": rel(app, &path),
            "recorded": true,
            "nodes": recorded.nodes.len(),
            "verified": recorded.verified.len(),
            "debt": recorded.debt.len(),
            "canonicality": recorded.canonicality.len(),
            "check": check,
        })))?,
        OutputFormat::Text => {
            w(out, format!("baseline recorded at {}", rel(app, &path)))?;
            w(out, format!("  {} node fingerprint(s), {} claim(s) verified, {} node(s) of debt tolerated, {} canonicality violation(s) tolerated", recorded.nodes.len(), recorded.verified.len(), recorded.debt.len(), recorded.canonicality.len()))?;
            for d in &recorded.debt {
                w(out, format!("  tolerated {:<14} {:<56} {}", d.freshness.as_str(), d.node, d.reason.as_deref().unwrap_or("")))?;
            }
            for c in &recorded.canonicality {
                w(out, format!("  tolerated canonicality {} {}", c.violation, c.reason.as_deref().unwrap_or("")))?;
            }
            w(out, format!("check {} in mode {}: {}", check.verdict, check.mode.as_str(), check.summary))?;
            w(out, "commit the baseline; from now on `majordomus knowledge check` refuses new debt and the recorded debt may only shrink")?;
        }
    }
    Ok(0)
}

fn record(app: &App, force: bool, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let inputs = Inputs::from_app(app)?;
    let path = inputs.baseline_path();
    if path.exists() && !force {
        return Err(refused(format!(
            "a baseline is recorded at {}; pass --force to record over it (the diff is the review)",
            rel(app, &path)
        )));
    }
    bootstrap(app, true, format, out)
}

fn rel(app: &App, path: &std::path::Path) -> String {
    path.strip_prefix(app.repository.root())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn reconcile(app: &App, accept: bool, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let inputs = Inputs::from_app(app)?;
    let baseline = inputs.baseline()?;
    let model = knowledge::scan(&inputs, &baseline);
    let (r, next) = knowledge::reconcile::reconcile(&model, &baseline, accept);
    if let Some(next) = next {
        next.write(&inputs.baseline_path())?;
        knowledge::forget();
    }
    match format {
        OutputFormat::Json => w(out, pretty(&serde_json::to_value(&r).unwrap_or(Value::Null)))?,
        OutputFormat::Text => {
            for p in &r.proposals {
                w(out, format!("proposal {:<18} {:<8} {:<56} {}", p.class, p.applies_by, p.subject, p.action))?;
            }
            w(out, format!("{} proposal(s), {} applied by --accept", r.proposals.len(), r.acceptable))?;
            if accept {
                if r.baseline_written {
                    w(out, format!("accepted {} claim(s); baseline written at {}", r.accepted.len(), rel(app, &inputs.baseline_path())))?;
                } else {
                    w(out, "nothing to accept: every verifiable claim is verified against its present evidence")?;
                }
            }
        }
    }
    Ok(0)
}

fn accept_conflict(app: &App, conflict: &str, reason: &str, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let inputs = Inputs::from_app(app)?;
    let mut baseline = inputs.baseline()?;
    let model = knowledge::scan(&inputs, &baseline);
    let Some(c) = model.conflicts.iter().find(|c| c.id == conflict) else {
        let open: Vec<&str> = model.conflicts.iter().map(|c| c.id.as_str()).collect();
        return Err(Error::Refused {
            code: 12,
            reason: if open.is_empty() {
                format!("no conflict `{conflict}`: the model holds none")
            } else {
                format!("no conflict `{conflict}`; the open ones are: {}", open.join(", "))
            },
        });
    };
    if reason.trim().is_empty() {
        return Err(Error::Refused {
            code: crate::cli::EXIT_USAGE,
            reason: "knowledge accept: a reason is required".into(),
        });
    }
    baseline.accepted.retain(|a| a.conflict != c.id);
    baseline.accepted.push(crate::knowledge::baseline::AcceptedConflict {
        conflict: c.id.clone(),
        reason: Some(reason.trim().to_string()),
    });
    if baseline.schema.is_empty() {
        baseline.schema = crate::knowledge::baseline::SCHEMA.into();
    }
    baseline.write(&inputs.baseline_path())?;
    knowledge::forget();
    match format {
        OutputFormat::Json => w(out, pretty(&json!({ "accepted": c.id, "reason": reason.trim(), "baseline": rel(app, &inputs.baseline_path()) })))?,
        OutputFormat::Text => w(out, format!("accepted {} ({}); baseline written at {}", c.id, reason.trim(), rel(app, &inputs.baseline_path())))?,
    }
    Ok(0)
}

fn validate(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let inputs = Inputs::from_app(app)?;
    let mut findings: Vec<Value> = Vec::new();
    let mut errors = 0usize;
    let mut note = |severity: &str, code: &str, path: Option<String>, message: String| {
        if severity == "error" {
            errors += 1;
        }
        findings.push(json!({ "severity": severity, "code": code, "path": path, "message": message }));
    };
    let baseline = match Baseline::load(&inputs.baseline_path()) {
        Ok(b) => {
            if !b.is_empty() {
                let text = std::fs::read_to_string(inputs.baseline_path()).unwrap_or_default();
                if let Ok(m) = crate::metadata::yaml::parse_mapping(&text) {
                    if let Ok((_, steps)) = knowledge::migrate::migrate(knowledge::migrate::Family::Baseline, Value::Object(m)) {
                        for st in steps {
                            note("warning", "knowledge_baseline_migration", Some(rel(app, &inputs.baseline_path())), format!("the baseline needs a migration: {st}; run `majordomus knowledge baseline migrate`"));
                        }
                    }
                }
            }
            b
        }
        Err(e) => {
            note("error", "knowledge_baseline_invalid", Some(rel(app, &inputs.baseline_path())), e.to_string());
            Baseline::default()
        }
    };
    if let Err(e) = knowledge::canonicality::Exceptions::load(&inputs.exceptions_path()) {
        note("error", "knowledge_exceptions_invalid", Some(rel(app, &inputs.exceptions_path())), e.to_string());
    }
    let model = knowledge::scan(&inputs, &baseline);
    for d in &model.diagnostics {
        let sev = match d.severity {
            crate::model::Severity::Error => "error",
            crate::model::Severity::Warning => "warning",
            crate::model::Severity::Info => "info",
        };
        note(sev, &d.code, d.path.clone(), d.message.clone());
    }
    match format {
        OutputFormat::Json => w(out, pretty(&json!({ "findings": findings, "errors": errors, "nodes": model.nodes.len(), "fingerprint": model.fingerprint })))?,
        OutputFormat::Text => {
            for f in &findings {
                w(out, format!("{:<8} {:<40} {}{}", s(&f["severity"]), s(&f["code"]), f["path"].as_str().map(|p| format!("{p}: ")).unwrap_or_default(), s(&f["message"])))?;
            }
            w(out, format!("knowledge model: {} node(s), fingerprint {}, {} finding(s), {} error(s)", model.nodes.len(), &model.fingerprint[..12.min(model.fingerprint.len())], findings.len(), errors))?;
        }
    }
    if errors > 0 {
        return Err(refused(format!("knowledge validate: {errors} error(s)")));
    }
    Ok(0)
}

fn baseline_show(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let inputs = Inputs::from_app(app)?;
    let baseline = inputs.baseline()?;
    match format {
        OutputFormat::Json => w(out, pretty(&serde_json::to_value(&baseline).unwrap_or(Value::Null)))?,
        OutputFormat::Text => {
            if baseline.is_empty() {
                w(out, format!("no baseline recorded at {}; run `majordomus knowledge bootstrap`", rel(app, &inputs.baseline_path())))?;
                return Ok(0);
            }
            w(out, format!("baseline {} recorded{}", rel(app, &inputs.baseline_path()), baseline.recorded_at.as_deref().map(|r| format!(" at {}", &r[..r.len().min(12)])).unwrap_or_default()))?;
            w(out, format!("  schema {}", baseline.schema))?;
            w(out, format!("  {} node fingerprint(s), {} verified claim(s), {} tolerated debt, {} accepted conflict(s), {} tolerated canonicality violation(s)", baseline.nodes.len(), baseline.verified.len(), baseline.debt.len(), baseline.accepted.len(), baseline.canonicality.len()))?;
            let model = knowledge::scan(&inputs, &baseline);
            let check = knowledge::baseline::check(&model, &baseline, inputs.policy.mode);
            w(out, format!("  against the present scan: {} new, {} tolerated, {} resolved", check.new_debt.len(), check.tolerated.len(), check.resolved.len()))?;
            for r in &check.resolved {
                w(out, format!("  resolved {r} — record the baseline again so that it cannot come back"))?;
            }
        }
    }
    Ok(0)
}

fn migrate(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let inputs = Inputs::from_app(app)?;
    let path = inputs.baseline_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            w(out, format!("no baseline at {}; nothing to migrate", rel(app, &path)))?;
            return Ok(0);
        }
        Err(e) => {
            return Err(Error::Io {
                path,
                source: e,
            })
        }
    };
    let value = crate::metadata::yaml::parse_mapping(&text).map_err(|r| refused(format!("{}: {r}", rel(app, &path))))?;
    let (migrated, steps) = knowledge::migrate::migrate(knowledge::migrate::Family::Baseline, Value::Object(value))
        .map_err(|r| refused(format!("{}: {r}", rel(app, &path))))?;
    if steps.is_empty() {
        match format {
            OutputFormat::Json => w(out, pretty(&json!({ "baseline": rel(app, &path), "migrated": false, "steps": [] })))?,
            OutputFormat::Text => w(out, format!("baseline {} is current ({})", rel(app, &path), knowledge::baseline::SCHEMA))?,
        }
        return Ok(0);
    }
    let b: Baseline = serde_json::from_value(migrated).map_err(|e| refused(format!("{}: {e}", rel(app, &path))))?;
    b.write(&path)?;
    knowledge::forget();
    match format {
        OutputFormat::Json => w(out, pretty(&json!({ "baseline": rel(app, &path), "migrated": true, "steps": steps })))?,
        OutputFormat::Text => {
            w(out, format!("baseline {} migrated:", rel(app, &path)))?;
            for st in &steps {
                w(out, format!("  {st}"))?;
            }
        }
    }
    Ok(0)
}

fn check(app: &App, mode: Option<String>, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let mode = match mode {
        None => None,
        Some(word) => Some(Mode::parse(&word).ok_or_else(|| Error::Refused {
            code: crate::cli::EXIT_USAGE,
            reason: format!("no check mode `{word}`; one of observe, warn, protect, strict"),
        })?),
    };
    let v = call(app, &["knowledge", "check"], strip_nulls(json!({ "mode": mode })))?;
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        OutputFormat::Text => {
            if !v["baseline_recorded"].as_bool().unwrap_or(false) {
                w(out, "no baseline recorded: every debt item is new; run `majordomus knowledge bootstrap` to adopt the present state")?;
            }
            for d in v["new_debt"].as_array().into_iter().flatten() {
                w(out, format!("new       {:<13} {:<12} {:<56} {}", s(&d["class"]), s(&d["state"]), s(&d["id"]), s(&d["reason"])))?;
            }
            if s(&v["mode"]) != "protect" || v["new_debt"].as_array().is_some_and(|a| a.is_empty()) {
                for d in v["tolerated"].as_array().into_iter().flatten() {
                    w(out, format!("tolerated {:<13} {:<12} {}", s(&d["class"]), s(&d["state"]), s(&d["id"])))?;
                }
            }
            for r in v["resolved"].as_array().into_iter().flatten() {
                w(out, format!("resolved  {} — record the baseline again", s(r)))?;
            }
            w(out, format!("knowledge check: {} ({})", s(&v["verdict"]), s(&v["summary"])))?;
        }
    }
    if s(&v["verdict"]) == "fail" {
        return Err(refused(format!(
            "knowledge check failed in mode {}: {} new debt item(s); fix them, or record the baseline deliberately",
            s(&v["mode"]),
            v["new_debt"].as_array().map(Vec::len).unwrap_or(0)
        )));
    }
    Ok(0)
}

fn canonicality(app: &App, capability: Option<String>, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let one = capability.is_some();
    let v = call(app, &["knowledge", "canonicality"], strip_nulls(json!({ "capability": capability })))?;
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        OutputFormat::Text => {
            for c in v["capabilities"].as_array().into_iter().flatten() {
                if one {
                    w(out, format!("{}", s(&c["id"])))?;
                    w(out, format!("  canonical source: {}", s(&c["canonical_source"])))?;
                    for sf in c["surfaces"].as_array().into_iter().flatten() {
                        w(out, format!("  derived surface   {} {:<10} {}", if sf["derived"].as_bool().unwrap_or(false) { "✓" } else { "✗" }, s(&sf["name"]), s(&sf["detail"])))?;
                    }
                    for m in c["mentions"].as_array().into_iter().flatten() {
                        w(out, format!("  mentioned by hand in {}:{}  {}", s(&m["path"]), n(&m["line"]), s(&m["text"])))?;
                    }
                    w(out, format!("  manual maintenance surface: {} — {}", n(&c["mms"]), s(&c["verdict"])))?;
                } else {
                    w(out, format!("{:<40} MMS {:<3} {:<7} {}", s(&c["id"]), n(&c["mms"]), s(&c["verdict"]), s(&c["canonical_source"])))?;
                }
            }
            for viol in v["violations"].as_array().into_iter().flatten() {
                let standing = if viol["excepted_by"].is_string() {
                    format!("excepted by {}", s(&viol["excepted_by"]))
                } else if viol["tolerated"].as_bool().unwrap_or(false) {
                    "tolerated".into()
                } else {
                    "counting".into()
                };
                w(out, format!("violation {:<22} {:<12} {}", s(&viol["class"]), standing, s(&viol["reason"])))?;
                w(out, format!("    remedy: {}", s(&viol["remedy"])))?;
            }
            for e in v["exceptions"].as_array().into_iter().flatten() {
                w(out, format!("exception {:<8} {} (owner {}, expires {}): {}", s(&e["status"]), s(&e["id"]), s(&e["owner"]), s(&e["expires"]), s(&e["reason"])))?;
            }
            w(out, format!("canonicality verdict: {} — {}", s(&v["verdict"]), s(&v["summary"])))?;
        }
    }
    if !one && s(&v["verdict"]) == "fail" {
        return Err(refused("canonicality check failed: a violation counts that neither the baseline tolerates nor an exception covers"));
    }
    Ok(0)
}

fn derive(app: &App, kinds: Vec<String>, dry_run: bool, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let inputs = Inputs::from_app(app)?;
    let provider = knowledge::semantic::provider_for(&inputs.policy.semantic)?;
    let baseline = inputs.baseline()?;
    let model = knowledge::scan(&inputs, &baseline);
    if dry_run {
        let (given, withheld) = knowledge::semantic::preview(&model, provider.remote(), &kinds);
        match format {
            OutputFormat::Json => w(out, pretty(&json!({
                "provider": provider.id(),
                "remote": provider.remote(),
                "given": given.iter().map(|n| n.id.clone()).collect::<Vec<_>>(),
                "withheld": withheld.iter().map(|n| n.id.clone()).collect::<Vec<_>>(),
            })))?,
            OutputFormat::Text => {
                w(out, format!("provider {} ({}), remote {}", provider.id(), provider.model(), provider.remote()))?;
                for node in &given {
                    w(out, format!("  give     {}", node.id))?;
                }
                for node in &withheld {
                    w(out, format!("  withhold {} (not for remote processing)", node.id))?;
                }
                w(out, format!("{} node(s) would be given, {} withheld; nothing was run", given.len(), withheld.len()))?;
            }
        }
        return Ok(0);
    }
    let ctx = inputs.context();
    let read = |e: &knowledge::model::Evidence| e.locator.path.as_deref().and_then(|p| ctx.read(p));
    let cache = knowledge::semantic::cache_path(inputs.root, &inputs.local);
    let report = knowledge::semantic::derive(&model, provider.as_ref(), &inputs.policy.semantic, &kinds, &read, &cache)?;
    knowledge::forget();
    match format {
        OutputFormat::Json => w(out, pretty(&serde_json::to_value(&report).unwrap_or(Value::Null)))?,
        OutputFormat::Text => {
            w(out, format!("provider {} ({}), remote {}, operations {}", report.provider, report.model, report.remote, report.operations.join(", ")))?;
            w(out, format!("{} node(s) considered, {} withheld, {} line(s) redacted, {} derivation(s) written to {}", report.considered, report.withheld, report.redacted_lines, report.derivations, rel(app, std::path::Path::new(&report.cache))))?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capability_error_maps_to_the_exit_code_contract() {
        assert!(matches!(map(CapabilityError::NotFound("x".into())), Error::Refused { code: 12, .. }));
        assert!(matches!(map(CapabilityError::Refused("x".into())), Error::Refused { code: 10, .. }));
        assert!(matches!(map(CapabilityError::InvalidInput("x".into())), Error::Refused { code: 2, .. }));
    }

    #[test]
    fn nulls_are_stripped_so_that_an_unset_option_is_absent_not_null() {
        let v = strip_nulls(json!({ "kind": Value::Null, "debt": true }));
        assert_eq!(v, json!({ "debt": true }));
    }
}
