//! `majordomus release`: the release state through the registry's own capabilities, and
//! the one command that changes it.
//!
//! Every read here executes a capability and renders what it answered; the facts are the
//! release engine's, the phrasing is this file's, and nothing between them computes a
//! version, classifies a change or renders a changelog.
//!
//! `prepare` is the exception, and it is a command of the executable rather than a
//! capability for one reason: it writes to the repository, and nothing served over the
//! loopback socket writes to the repository. It asks the same engine every read asks, so a
//! plan is a description of what preparing would do and not a simulation of it.

use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{OutputFormat, ReleaseArgs, ReleaseCommand};
use crate::error::{Error, Result};
use crate::release::version::{Bump, Version};
use crate::release::Engine;

/// The exit code when a release invariant does not hold.
///
/// The same 10 every other contract failure in this executable uses: a script that already
/// knows what 10 means about `distribution status` learns nothing new here.
pub const EXIT_INVALID: u8 = 10;

/// The exit code when a release mutation is refused.
pub const EXIT_REFUSED: u8 = 15;

/// Run `majordomus release`.
pub fn run(args: ReleaseArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.command {
        None | Some(ReleaseCommand::Status) => status(&app, args.format, &mut out),
        Some(ReleaseCommand::Version) => versions(&app, args.format, &mut out),
        Some(ReleaseCommand::Explain) => explain(&app, args.format, &mut out),
        Some(ReleaseCommand::Diff { impact, surface }) => {
            diff(&app, impact, surface, args.format, &mut out)
        }
        Some(ReleaseCommand::Changelog { version }) => {
            changelog(&app, version, args.format, &mut out)
        }
        Some(ReleaseCommand::Manifest { version }) => manifest(&app, version, &mut out),
        Some(ReleaseCommand::Plan) => plan(&app, args.format, &mut out),
        Some(ReleaseCommand::Check) => check(&app, args.format, &mut out),
        Some(ReleaseCommand::Prepare {
            version,
            bump,
            dry_run,
        }) => prepare(&app, version, bump, dry_run, args.format, &mut out),
    }
}

type Out<'a> = std::io::StdoutLock<'a>;

fn w(out: &mut Out<'_>, s: impl AsRef<str>) -> Result<()> {
    writeln!(out, "{}", s.as_ref()).map_err(Error::Transport)
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

fn map(e: CapabilityError) -> Error {
    Error::Protocol {
        reason: e.to_string(),
    }
}

/// Execute the capability exposed at a command path: the command line is a projection of
/// the registry, not a second implementation beside it.
fn call(app: &App, path: &[&str], input: Value) -> Result<Value> {
    let ctx = &app.context;
    let words: Vec<String> = path.iter().map(|w| (*w).to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })?;
    ctx.execute(id, input).map_err(map)
}

/// A string field of a JSON value, or an empty string.
fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// A right-aligned label column, so that a person reads down the values rather than across
/// the labels.
fn row(out: &mut Out<'_>, label: &str, value: impl AsRef<str>) -> Result<()> {
    w(out, format!("{label:<22}{}", value.as_ref()))
}

fn versions(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["release", "version"], json!({}))?;
    if format == OutputFormat::Json {
        w(out, pretty(&v))?;
        return Ok(0);
    }
    row(out, "source", text(&v, "source"))?;
    row(
        out,
        "published",
        v.get("published")
            .and_then(Value::as_str)
            .unwrap_or("none recorded"),
    )?;
    row(out, "running", text(&v, "running"))?;
    for d in v
        .get("deployed")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        row(
            out,
            &format!("deployed {}", text(d, "deployment")),
            format!(
                "{} ({})",
                d.get("version")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                text(d, "agreement")
            ),
        )?;
    }
    let divergences = v
        .get("divergences")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if divergences.is_empty() {
        w(out, "\nevery version this repository can see agrees.")?;
    } else {
        w(out, "")?;
        for d in divergences {
            w(out, format!("! {}", d.as_str().unwrap_or_default()))?;
        }
    }
    Ok(0)
}

fn status(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["release", "status"], json!({}))?;
    if format == OutputFormat::Json {
        w(out, pretty(&v))?;
        return Ok(0);
    }
    let versions = v.get("versions").cloned().unwrap_or_default();
    row(out, "source version", text(&versions, "source"))?;
    row(
        out,
        "published release",
        versions
            .get("published")
            .and_then(Value::as_str)
            .unwrap_or("none recorded"),
    )?;
    row(
        out,
        "baseline",
        v.get("baseline")
            .and_then(|b| b.get("tag"))
            .and_then(Value::as_str)
            .unwrap_or("none"),
    )?;
    row(
        out,
        "compatibility",
        match (
            v.get("impact").and_then(Value::as_str),
            v.get("impact_source").and_then(Value::as_str),
        ) {
            (Some(impact), Some(source)) => format!("{impact} (from the {source})"),
            (Some(impact), None) => impact.to_string(),
            (None, _) => "not decided".into(),
        },
    )?;
    row(
        out,
        "required bump",
        v.get("required_bump")
            .and_then(Value::as_str)
            .unwrap_or("none"),
    )?;
    row(
        out,
        "minimum version",
        v.get("minimum_version")
            .and_then(Value::as_str)
            .unwrap_or("none"),
    )?;
    row(out, "target version", text(&v, "target_version"))?;
    row(
        out,
        "contract changes",
        v.get("contract_changes")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .to_string(),
    )?;
    row(
        out,
        "unreleased changes",
        v.get("unreleased_changes")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .to_string(),
    )?;
    row(out, "readiness", text(&v, "readiness"))?;
    let diagnostics = v
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !diagnostics.is_empty() {
        w(out, "")?;
        for d in &diagnostics {
            w(
                out,
                format!(
                    "{:<5} {}  {}",
                    text(d, "severity"),
                    text(d, "code"),
                    text(d, "message")
                ),
            )?;
        }
    }
    Ok(0)
}

fn explain(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["release", "explain"], json!({}))?;
    if format == OutputFormat::Json {
        w(out, pretty(&v))?;
        return Ok(0);
    }
    let bump = v
        .get("required_bump")
        .and_then(Value::as_str)
        .unwrap_or("none");
    w(out, format!("Required bump: {}", bump.to_uppercase()))?;
    w(out, "")?;
    let deciding = v
        .get("deciding")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !deciding.is_empty() {
        w(out, "Changes:")?;
        for change in &deciding {
            let line = change
                .get("fact")
                .and_then(Value::as_str)
                .map(|f| {
                    format!(
                        "{} {} · {}",
                        text(change, "surface"),
                        text(change, "entry"),
                        f
                    )
                })
                .unwrap_or_else(|| {
                    format!("{} {}", text(change, "surface"), text(change, "entry"))
                });
            let mark = match text(change, "kind").as_str() {
                "added" => '+',
                "removed" => '-',
                _ => '~',
            };
            w(out, format!("  {mark} {line}"))?;
        }
        w(out, "")?;
        // the reason is the policy's own sentence, printed once for the whole group rather
        // than repeated per line
        if let Some(reason) = deciding
            .first()
            .and_then(|c| c.get("reason"))
            .and_then(Value::as_str)
        {
            w(out, format!("Because:\n  {reason}"))?;
            w(out, "")?;
        }
    }
    w(
        out,
        format!(
            "Classification:\n  {}",
            v.get("impact").and_then(Value::as_str).unwrap_or("none")
        ),
    )?;
    w(out, "")?;
    w(out, format!("Policy:\n  {}", text(&v, "policy")))?;
    w(out, "")?;
    w(
        out,
        format!(
            "Baseline:\n  {}",
            v.get("baseline")
                .and_then(|b| b.get("tag"))
                .and_then(Value::as_str)
                .unwrap_or("none recorded")
        ),
    )?;
    w(out, "")?;
    w(out, format!("Current:\n  {}", text(&v, "current")))?;
    w(out, "")?;
    w(
        out,
        format!(
            "Minimum:\n  {}",
            v.get("minimum")
                .and_then(Value::as_str)
                .unwrap_or("none required")
        ),
    )?;
    let uncompared = v
        .get("uncompared")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !uncompared.is_empty() {
        w(out, "")?;
        w(
            out,
            format!(
                "Not compared:\n  {} — the two contracts do not both carry them, so this \
                 verdict is partial",
                uncompared
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )?;
    }
    Ok(0)
}

fn diff(
    app: &App,
    impact: Option<String>,
    surface: Option<String>,
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    let mut input = serde_json::Map::new();
    if let Some(impact) = impact {
        input.insert("impact".into(), json!(impact));
    }
    if let Some(surface) = surface {
        input.insert("surface".into(), json!(surface));
    }
    let v = call(app, &["release", "diff"], Value::Object(input))?;
    if format == OutputFormat::Json {
        w(out, pretty(&v))?;
        return Ok(0);
    }
    let Some(diff) = v.get("diff").filter(|d| !d.is_null()) else {
        w(
            out,
            v.get("reason")
                .and_then(Value::as_str)
                .unwrap_or("there is nothing to compare against"),
        )?;
        return Ok(0);
    };
    let changes = diff
        .get("changes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if changes.is_empty() {
        w(out, "the public contract has not moved.")?;
        return Ok(0);
    }
    for change in &changes {
        let mark = match text(change, "kind").as_str() {
            "added" => '+',
            "removed" => '-',
            _ => '~',
        };
        let mut line = format!(
            "{mark} {:<9} {:<8} {}",
            text(change, "impact"),
            text(change, "surface"),
            text(change, "entry")
        );
        if let Some(fact) = change.get("fact").and_then(Value::as_str) {
            line.push_str(&format!(" · {fact}"));
            match (
                change.get("before").and_then(Value::as_str),
                change.get("after").and_then(Value::as_str),
            ) {
                (Some(b), Some(a)) => line.push_str(&format!(": {b} -> {a}")),
                (Some(b), None) => line.push_str(&format!(": was {b}")),
                (None, Some(a)) => line.push_str(&format!(": {a}")),
                (None, None) => {}
            }
        }
        w(out, line)?;
    }
    w(
        out,
        format!(
            "\n{} change(s); the strongest is {}",
            changes.len(),
            text(diff, "impact")
        ),
    )?;
    Ok(0)
}

fn changelog(
    app: &App,
    version: Option<String>,
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    let mut input = serde_json::Map::new();
    if let Some(version) = version {
        input.insert("version".into(), json!(version));
    }
    let v = call(app, &["release", "changelog"], Value::Object(input))?;
    if format == OutputFormat::Json {
        w(out, pretty(&v))?;
        return Ok(0);
    }
    w(out, text(&v, "markdown").trim_end())?;
    Ok(0)
}

fn manifest(app: &App, version: Option<String>, out: &mut Out<'_>) -> Result<u8> {
    let mut input = serde_json::Map::new();
    if let Some(version) = version {
        input.insert("version".into(), json!(version));
    }
    // A manifest is a machine-readable document and has one rendering; `--format text`
    // would be a second one, and there is nothing a person wants from it that the JSON does
    // not already say in the same words.
    let v = call(app, &["release", "manifest"], Value::Object(input))?;
    w(out, pretty(&v))?;
    Ok(0)
}

fn plan(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["release", "plan"], json!({}))?;
    if format == OutputFormat::Json {
        w(out, pretty(&v))?;
        return Ok(0);
    }
    render_plan(&v, out)
}

/// The plan a person reads before and after `--dry-run`, rendered once.
fn render_plan(v: &Value, out: &mut Out<'_>) -> Result<u8> {
    row(out, "target version", text(v, "target"))?;
    row(
        out,
        "bump",
        v.get("bump").and_then(Value::as_str).unwrap_or("none"),
    )?;
    row(
        out,
        "compatibility",
        v.get("impact")
            .and_then(Value::as_str)
            .unwrap_or("not computed"),
    )?;
    let changes = v
        .get("changes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    w(out, "")?;
    if changes.is_empty() {
        w(out, "No unreleased change record would be stamped.")?;
    } else {
        w(out, "Would stamp:")?;
        for change in &changes {
            w(
                out,
                format!("  {}  {}", text(change, "id"), text(change, "title")),
            )?;
        }
    }
    w(out, "")?;
    w(out, "Would write:")?;
    for path in v
        .get("writes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        w(out, format!("  {}", path.as_str().unwrap_or_default()))?;
    }
    w(out, "")?;
    w(out, "`majordomus generate` would then rewrite:")?;
    for path in v
        .get("regenerates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        w(out, format!("  {}", path.as_str().unwrap_or_default()))?;
    }
    let diagnostics = v
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let (blocking, notes): (Vec<&Value>, Vec<&Value>) = diagnostics
        .iter()
        .partition(|d| text(d, "severity") == "error");
    if !blocking.is_empty() {
        w(out, "")?;
        w(out, "Would refuse:")?;
        for d in blocking {
            w(
                out,
                format!("  {}  {}", text(d, "code"), text(d, "message")),
            )?;
        }
    }
    if !notes.is_empty() {
        w(out, "")?;
        w(out, "Worth knowing:")?;
        for d in notes {
            w(
                out,
                format!("  {}  {}", text(d, "code"), text(d, "message")),
            )?;
        }
    }
    Ok(0)
}

fn check(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["release", "check"], json!({}))?;
    if format == OutputFormat::Json {
        w(out, pretty(&v))?;
    } else {
        for d in v
            .get("diagnostics")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let mark = match text(d, "severity").as_str() {
                "error" => "FAIL",
                "warning" => "WARN",
                _ => "note",
            };
            w(
                out,
                format!("{mark} {}  {}", text(d, "code"), text(d, "message")),
            )?;
            if let Some(next) = d.get("next").and_then(Value::as_str) {
                w(out, format!("     next: {next}"))?;
            }
        }
        w(out, text(&v, "summary"))?;
    }
    Ok(if v.get("ok").and_then(Value::as_bool) == Some(true) {
        0
    } else {
        EXIT_INVALID
    })
}

/// `majordomus release prepare`.
///
/// The whole of what preparing a release does to the repository: set the canonical version,
/// stamp the unreleased change records with it, and leave every derived artifact to
/// `majordomus generate`, which is the one thing that writes a projection. It creates no
/// tag, pushes nothing and contacts nothing: a prepared release is a commit somebody reads
/// before it goes out.
fn prepare(
    app: &App,
    version: Option<String>,
    bump: Option<String>,
    dry_run: bool,
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    let root = app.repository.root();
    let engine = Engine::load(root, &app.context.index, Some(&app.context.registry))
        .map_err(|reason| Error::Protocol { reason })?;

    let target = match (version, bump) {
        (Some(_), Some(_)) => {
            return refuse(
                out,
                "--version and --bump both name the version to prepare; give one",
            )
        }
        (Some(text), None) => text
            .trim()
            .parse::<Version>()
            .map_err(|e| Error::Protocol {
                reason: format!("--version {text}: {e}"),
            })?,
        (None, Some(word)) => {
            let requested = Bump::parse(word.trim()).ok_or_else(|| Error::Protocol {
                reason: format!("--bump {word}: a bump is none, patch, minor or major"),
            })?;
            let from = engine
                .baseline
                .version()
                .cloned()
                .unwrap_or_else(|| engine.source.clone());
            requested.apply(&from)
        }
        (None, None) => engine.target_version(),
    };

    // The invariant, checked here and not only reported: a requested version below the
    // minimum the contract change implies is refused, whatever was asked for. There is no
    // flag that lifts this. A project that decides a break is worth making says so by
    // writing the change record and taking the version the policy gives it.
    if let (Some(minimum), Some((impact, _))) =
        (engine.minimum_version(), engine.effective_impact())
    {
        if target < minimum {
            return refuse(
                out,
                format!(
                    "the change is {impact} and requires at least {minimum}; \
                     {target} was asked for.\n\
                     Run `majordomus release explain` to see which changes decided it."
                ),
            );
        }
    }

    let state = engine.state(&app.context.index);
    let blocking: Vec<&crate::release::Diagnostic> = state
        .diagnostics
        .iter()
        .filter(|d| d.is_blocking() && d.code != "SEMVER_BUMP_TOO_LOW")
        .collect();

    // The plan is about the version being prepared, not about whatever the engine would
    // have chosen: a dry run that described a different release than the one asked for
    // would be a simulation rather than a description.
    let plan_value = call(
        app,
        &["release", "plan"],
        json!({ "version": target.to_string() }),
    )?;
    if dry_run {
        if format == OutputFormat::Json {
            w(out, pretty(&plan_value))?;
            return Ok(0);
        }
        w(out, "Dry run: nothing was written.\n")?;
        return render_plan(&plan_value, out);
    }

    if !blocking.is_empty() {
        let mut message = String::from("the release is not ready:\n");
        for d in blocking {
            message.push_str(&format!("  {}  {}\n", d.code, d.message));
        }
        message.push_str("Run `majordomus release check` for the whole list.");
        return refuse(out, message);
    }

    let mut written = Vec::new();
    if target != engine.source {
        write_crate_version(root, &target)?;
        written.push("apps/majordomus-cli/Cargo.toml".to_string());
    }
    for change in engine.changes.unreleased() {
        stamp(root, &change.path, &target)?;
        written.push(change.path.clone());
    }

    if written.is_empty() {
        w(
            out,
            format!("{target} is already prepared: nothing to write."),
        )?;
    } else {
        for path in &written {
            w(out, format!("wrote {path}"))?;
        }
    }
    w(
        out,
        format!(
            "\n{target} is prepared. Next:\n  \
             majordomus generate            regenerate every projection of the new version\n  \
             majordomus release check       prove the release may go out\n  \
             git tag {}                the release pipeline runs on the tag",
            target.tag()
        ),
    )?;
    Ok(0)
}

/// Refuse a mutation, saying why. Exit 15, the executable's refusal code.
fn refuse(out: &mut Out<'_>, message: impl AsRef<str>) -> Result<u8> {
    w(out, format!("refused: {}", message.as_ref()))?;
    Ok(EXIT_REFUSED)
}

/// Set the crate's version, which is the canonical one.
///
/// A line edit rather than a manifest rewrite: the manifest is hand-written, its comments
/// carry the reason for every dependency, and a round trip through a serializer would lose
/// them. The line is found the way `scripts/release-version` finds it — the first
/// `version =` under `[package]` — so the two agree on which line is the version by
/// construction.
fn write_crate_version(root: &Path, version: &Version) -> Result<()> {
    let path = root.join("apps/majordomus-cli/Cargo.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    let mut out = String::with_capacity(text.len());
    let mut in_package = false;
    let mut done = false;
    for line in text.lines() {
        if line.trim_start().starts_with('[') {
            in_package = line.trim() == "[package]";
        }
        if !done && in_package && line.trim_start().starts_with("version") {
            out.push_str(&format!("version = \"{version}\"\n"));
            done = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !done {
        return Err(Error::Protocol {
            reason: format!("{} states no version under [package]", path.display()),
        });
    }
    std::fs::write(&path, out).map_err(|e| Error::io(&path, e))
}

/// Stamp one change record with the version that publishes it.
///
/// The record's front matter gains `released_in`; nothing else in the file is touched. A
/// record that already carries one is left alone, which is what makes preparing the same
/// release twice write nothing the second time.
fn stamp(root: &Path, relative: &str, version: &Version) -> Result<()> {
    let path = root.join(relative);
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    if text
        .lines()
        .any(|l| l.trim_start().starts_with("released_in:"))
    {
        return Ok(());
    }
    // after the `impact:` line, which every valid record carries, so that the front matter
    // keeps the order its schema declares rather than growing a tail.
    let mut out = String::with_capacity(text.len() + 32);
    let mut written = false;
    for line in text.lines() {
        out.push_str(line);
        out.push('\n');
        if !written && line.trim_start().starts_with("impact:") {
            out.push_str(&format!("released_in: \"{version}\"\n"));
            written = true;
        }
    }
    if !written {
        return Err(Error::Protocol {
            reason: format!("{relative} states no `impact`, so it is not a change record"),
        });
    }
    std::fs::write(&path, out).map_err(|e| Error::io(&path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crate_version_is_set_on_the_package_line_and_nowhere_else() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let manifest = dir.path().join("apps/majordomus-cli/Cargo.toml");
        std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        std::fs::write(
            &manifest,
            "[package]\nname = \"majordomus-cli\"\nversion = \"0.3.1\"\nedition = \"2021\"\n\n\
             [dependencies]\n# clap parses the command line\nclap = { version = \"4.5\" }\n",
        )
        .unwrap();
        write_crate_version(dir.path(), &Version::new(0, 4, 0)).unwrap();
        let text = std::fs::read_to_string(&manifest).unwrap();
        assert!(text.contains("version = \"0.4.0\""));
        assert!(
            text.contains("clap = { version = \"4.5\" }"),
            "a dependency's version is not the package's"
        );
        assert!(
            text.contains("# clap parses the command line"),
            "the comments survive"
        );
    }

    #[test]
    fn a_manifest_with_no_package_version_is_refused_rather_than_appended_to() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let manifest = dir.path().join("apps/majordomus-cli/Cargo.toml");
        std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        std::fs::write(&manifest, "[package]\nname = \"x\"\n").unwrap();
        let err = write_crate_version(dir.path(), &Version::new(1, 0, 0)).unwrap_err();
        assert!(err.to_string().contains("states no version"));
    }

    #[test]
    fn stamping_a_record_adds_released_in_after_the_impact_and_is_idempotent() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let record = dir.path().join("c.md");
        std::fs::write(
            &record,
            "---\nschema: change/v1\nid: x\nkind: change\ntitle: t\ntype: added\nimpact: additive\n---\n\n## Summary\n\nx\n",
        )
        .unwrap();
        stamp(dir.path(), "c.md", &Version::new(0, 4, 0)).unwrap();
        let once = std::fs::read_to_string(&record).unwrap();
        assert!(
            once.contains("impact: additive\nreleased_in: \"0.4.0\"\n"),
            "{once}"
        );
        stamp(dir.path(), "c.md", &Version::new(0, 5, 0)).unwrap();
        let twice = std::fs::read_to_string(&record).unwrap();
        assert_eq!(once, twice, "a stamped record is left alone");
    }

    #[test]
    fn a_record_that_is_not_a_change_record_is_refused() {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::write(dir.path().join("c.md"), "---\nid: x\n---\n").unwrap();
        let err = stamp(dir.path(), "c.md", &Version::new(0, 4, 0)).unwrap_err();
        assert!(err.to_string().contains("states no `impact`"));
    }
}
