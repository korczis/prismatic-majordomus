//! Generated projections that are committed for review: the OpenAPI document, the
//! capability reference, and the shell tool's key allow-lists derived from the JSON
//! Schemas. All come through this one pipeline; the committed files are caches of it,
//! never sources, and `--check` says when they are stale.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::app::App;
use crate::bench::{BenchmarkProjection, Coverage, CoverageState, SystemTarget, Transport};
use crate::capability::registry::ModuleSource;
use crate::capability::{CapabilityKind, CapabilityRegistry, CaseContext, Context, Provenance};
use crate::error::{Error, Result};
use crate::http::openapi;
use crate::metadata::KindSchema;
use crate::policy::LoadedPolicy;
use crate::share::Share;

/// Where generated artifacts live, relative to the repository root.
pub const OUT_DIR: &str = "docs/generated";
/// Where the registry dataset the site renders lives, relative to the repository root. The
/// shell site generator owns `site/data/generated/` wholesale; this directory is the Rust
/// executable's, so that no directory has two writers.
pub const SITE_DATA_DIR: &str = "site/data/registry";

/// The first line of every generated Markdown artifact.
pub const HEADER: &str = "GENERATED FILE — DO NOT EDIT DIRECTLY";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// What can be generated.
pub enum Target {
    /// `docs/generated/openapi.json`.
    OpenApi,
    /// `docs/generated/capabilities.md` (the index), `docs/generated/modules/<id>.md`,
    /// `docs/generated/cli.md` and `docs/generated/cli.json` (the command line as clap
    /// declares it, with the examples declared beside it).
    Docs,
    /// `docs/generated/benchmarks.md`: every benchmark target and the coverage, from the projection.
    Benchmarks,
    /// `docs/generated/registry.json`: the builtin registry as data, `majordomus/capability-registry/v1`.
    Registry,
    /// `<share>/allow/<name>.txt` for every schema that carries `x-majordomus-allow`.
    Allow,
    /// The provider bootstraps the policy's `projections[]` declare, rendered from the
    /// provider templates: `AGENTS.md`, `CLAUDE.md`, ... (see [`crate::providers`]).
    Providers,
    /// `site/data/registry/registry.json`: the registry dataset GitHub Pages renders
    /// (see [`crate::site`]).
    Site,
}

impl Target {
    /// Every target, in generation order.
    pub const ALL: &'static [Target] = &[
        Target::OpenApi,
        Target::Docs,
        Target::Benchmarks,
        Target::Registry,
        Target::Allow,
        Target::Providers,
        Target::Site,
    ];
}

/// The schema of `registry.json`.
pub const REGISTRY_SCHEMA: &str = "majordomus/capability-registry/v1";

/// The schema extension that names the allow-list a schema derives.
pub const ALLOW_EXTENSION: &str = "x-majordomus-allow";

#[derive(Debug, Clone, PartialEq, Eq)]
/// One generated file: where it goes and what it holds.
pub struct Artifact {
    /// Repository-relative path.
    pub path: String,
    /// The whole file.
    pub content: String,
}
/// The registry's artifacts of the selected targets: the OpenAPI document, the
/// reference index with one file per builtin module, and the registry manifest.
/// `Target::Benchmarks` needs the repository's index for its cases and is answered by
/// [`context_artifacts`]; `Target::Allow` and `Target::Providers` are not the registry's
/// and are answered by [`allow_artifacts`] and [`crate::providers::artifacts`].
pub fn artifacts(
    registry: &CapabilityRegistry,
    version: &str,
    cases: Option<&CaseContext<'_>>,
    targets: &[Target],
) -> Result<Vec<Artifact>> {
    let mut out = Vec::new();
    for t in targets {
        match t {
            Target::OpenApi => out.push(Artifact {
                path: format!("{OUT_DIR}/openapi.json"),
                content: openapi::render(
                    &openapi::document(registry, version, cases)
                        .map_err(|reason| Error::Http { reason })?,
                ),
            }),
            Target::Docs => {
                out.push(Artifact {
                    path: format!("{OUT_DIR}/capabilities.md"),
                    content: reference(registry, version),
                });
                let cli = crate::cli::tree();
                out.push(Artifact {
                    path: format!("{OUT_DIR}/cli.md"),
                    content: cli_reference(&cli, version),
                });
                out.push(Artifact {
                    path: format!("{OUT_DIR}/cli.json"),
                    content: cli_document(&cli, version),
                });
                for m in registry
                    .modules()
                    .filter(|m| m.source != ModuleSource::Declarative)
                {
                    out.push(Artifact {
                        path: format!("{OUT_DIR}/modules/{}.md", m.id),
                        content: module_reference(registry, m.id.as_str(), version),
                    });
                }
            }
            Target::Registry => out.push(Artifact {
                path: format!("{OUT_DIR}/registry.json"),
                content: registry_manifest(registry, version),
            }),
            Target::Benchmarks | Target::Allow | Target::Providers | Target::Site => {}
        }
    }
    Ok(out)
}

/// Every artifact of the selected targets, from every source: the registry (OpenAPI, the
/// reference, the registry manifest, the benchmark matrix), the schemas (allow-lists), the
/// policy and the templates (provider bootstraps), and the registry with the index (the
/// site dataset). This is the one plan `generate` writes and `generate --check` compares;
/// nothing else assembles artifacts.
pub fn plan(app: &App, targets: &[Target]) -> Result<Vec<Artifact>> {
    let mut out = context_artifacts(&app.context, crate::VERSION, targets)?;
    if targets.contains(&Target::Allow) {
        out.extend(allow_artifacts(
            &app.schema,
            &app.share,
            app.repository.root(),
        ));
    }
    let needs_policy = targets.contains(&Target::Providers) || targets.contains(&Target::Site);
    if needs_policy {
        let policy = LoadedPolicy::load(&app.repository)?;
        if targets.contains(&Target::Providers) {
            out.extend(crate::providers::artifacts(
                &app.repository,
                &app.share,
                &policy,
            )?);
        }
        if targets.contains(&Target::Site) {
            let dataset =
                crate::site::dataset(&app.context, &app.schema, &policy, &app.repository)?;
            out.push(Artifact {
                path: format!("{SITE_DATA_DIR}/registry.json"),
                content: crate::site::render(&dataset),
            });
        }
    }
    Ok(out)
}

/// Every artifact of the selected targets, the benchmark matrix included: what
/// `majordomus generate` writes and `--check` compares.
pub fn context_artifacts(
    ctx: &Context,
    version: &str,
    targets: &[Target],
) -> Result<Vec<Artifact>> {
    let cases = CaseContext { index: &ctx.index };
    let mut out = artifacts(&ctx.registry, version, Some(&cases), targets)?;
    if targets.contains(&Target::Benchmarks) {
        out.push(Artifact {
            path: format!("{OUT_DIR}/benchmarks.md"),
            content: benchmark_matrix(ctx, version),
        });
    }
    Ok(out)
}

/// The builtin registry as data: modules, descriptors with their schemas, and the
/// declarative kinds by name. No fingerprint and no count of declarative objects, so the
/// file changes when the code changes and not when a document is added.
pub fn registry_manifest(registry: &CapabilityRegistry, version: &str) -> String {
    let modules: Vec<&crate::capability::registry::ModuleInfo> = registry
        .modules()
        .filter(|m| m.source != ModuleSource::Declarative)
        .collect();
    // every descriptor as it is, plus the file it was composed in: the one place a reader
    // of the manifest (the site generator among them) learns where a capability's source is
    let capabilities: Vec<Value> = registry
        .iter()
        .filter(|c| matches!(c.provenance, Provenance::Builtin { .. }))
        .map(|c| {
            let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
            if let Some(o) = v.as_object_mut() {
                o.insert(
                    "source_path".into(),
                    Value::String(c.provenance.source_path()),
                );
            }
            v
        })
        .collect();
    let declarative_kinds: Vec<&str> = registry
        .modules()
        .filter(|m| m.source == ModuleSource::Declarative)
        .map(|m| m.id.as_str())
        .collect();
    let doc = serde_json::json!({
        "schema": REGISTRY_SCHEMA,
        "generated": format!("{HEADER}; source: the canonical capability registry; regenerate with `majordomus generate`"),
        "generator": format!("majordomus-cli {version}"),
        "modules": modules,
        "capabilities": capabilities,
        "declarative_kinds": declarative_kinds,
        "system_benchmark_targets": SystemTarget::ALL.iter().map(|s| serde_json::json!({ "key": s.key(), "transport": s.transport(), "description": s.description() })).collect::<Vec<_>>(),
    });
    openapi::render(&doc)
}

/// Every benchmark target and the coverage, from the projection of this repository.
pub fn benchmark_matrix(ctx: &Context, version: &str) -> String {
    let projection = BenchmarkProjection::from_context(ctx);
    let coverage = Coverage::compute(ctx, &projection);
    let mut s = String::new();
    s.push_str(&format!("<!-- {HEADER}\n     Source: the benchmark projection of the canonical capability registry; regenerate with `majordomus generate`\n     Generator: majordomus-cli {version} -->\n"));
    s.push_str("# Benchmark targets and coverage\n\n");
    s.push_str("Every externally callable operation is a benchmark target, derived from the registry: each executable capability directly and on every transport its exposure declares, with the cases its input type provides, plus the transports' own operations. Nothing below is listed by hand; `majordomus bench coverage` computes the same table live, `majordomus bench` times it, and `capabilities validate` fails when a requirement is missing.\n\n");
    s.push_str(
        "## Coverage\n\n| scope | required | covered | missing | waived |\n|---|---|---|---|---|\n",
    );
    for (name, t) in &coverage.tallies {
        s.push_str(&format!(
            "| {name} | {} | {} | {} | {} |\n",
            t.required, t.covered, t.missing, t.waived
        ));
    }
    s.push_str("\n## Capabilities\n\n| capability | module | kind | cache | direct | mcp | http | cases |\n|---|---|---|---|---|---|---|---|\n");
    for c in ctx.registry.iter().filter(|c| c.kind.is_executable()) {
        let cell = |transport: Transport| -> String {
            let line = coverage
                .lines
                .iter()
                .find(|l| l.subject == c.id.as_str() && l.transport == transport);
            match line.map(|l| l.state) {
                Some(CoverageState::Covered) => "covered".into(),
                Some(CoverageState::Missing) => "**missing**".into(),
                Some(CoverageState::Waived) => "waived".into(),
                None => "—".into(),
            }
        };
        let mut cases: Vec<String> = projection
            .of_capability(c.id.as_str())
            .filter(|t| t.transport() == Transport::Direct)
            .filter_map(|t| match &t.kind {
                crate::bench::TargetKind::Capability { case, .. } => Some(format!("`{case}`")),
                _ => None,
            })
            .collect();
        cases.dedup();
        let cache = match c.cache {
            crate::capability::CachePolicy::Disabled => "—".to_string(),
            crate::capability::CachePolicy::Process {
                max_entries,
                ttl_seconds,
            } => match ttl_seconds {
                Some(ttl) => format!("process, {max_entries} entries, {ttl}s"),
                None => format!("process, {max_entries} entries"),
            },
        };
        let kind = serde_json::to_value(c.kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} |\n",
            c.id,
            c.module,
            kind,
            cache,
            cell(Transport::Direct),
            cell(Transport::Mcp),
            cell(Transport::Http),
            if cases.is_empty() {
                "—".to_string()
            } else {
                cases.join(", ")
            }
        ));
    }
    s.push_str("\n## System targets\n\n| key | transport | measures |\n|---|---|---|\n");
    for t in SystemTarget::ALL {
        s.push_str(&format!(
            "| `{}` | {} | {} |\n",
            t.key(),
            t.transport().name(),
            t.description()
        ));
    }
    s.push_str("\nCache modes: a cached capability is measured cold (the cache cleared before every sample) and warm (the same input repeated); the direct transport reports the handler invocations of each. Evidence: `.ai/local/benchmarks/` for local runs, `.ai/repo/benchmarks/rust/` for the accepted baselines and the regression policy.\n");
    s
}

/// The shell tool's allow-lists, one per schema that carries `x-majordomus-allow`, under
/// the share directory: a repository-relative path when the share is inside the
/// repository, absolute otherwise.
pub fn allow_artifacts(schema: &KindSchema, share: &Share, root: &Path) -> Vec<Artifact> {
    let dir = share.allow_dir();
    let dir = dir
        .strip_prefix(root)
        .map(|p| p.to_path_buf())
        .unwrap_or(dir);
    schema
        .schemas()
        .filter_map(|(_, sch)| {
            let name = sch.json.get(ALLOW_EXTENSION).and_then(Value::as_str)?;
            Some(Artifact {
                path: format!("{}/{name}.txt", dir.display()),
                content: allow_lines(&sch.json).join("\n") + "\n",
            })
        })
        .collect()
}

/// The allow-list a JSON Schema derives: one anchored pattern per key path the schema
/// allows, in the schema's order, as the shell tool's `mj_yaml_unknown_keys` reads them
/// (`grep -E -f`). A list of scalars is `name(\.[0-9]+)?`, a list of objects recurses
/// through `name\.[0-9]+\.`, a nested object through `name\.`. Local `$ref`s into
/// `$defs` are followed. No blank lines and no comments: every line is a pattern.
///
/// ```
/// use majordomus_cli::generate::allow_lines;
/// use serde_json::json;
/// let schema = json!({ "type": "object", "properties": {
///     "id": { "type": "string" },
///     "tags": { "type": "array", "items": { "type": "string" } },
///     "x-majordomus": { "type": "object", "properties": { "tests": { "type": "array", "items": { "type": "string" } } } }
/// } });
/// assert_eq!(allow_lines(&schema), [
///     "^id$",
///     "^tags(\\.[0-9]+)?$",
///     "^x-majordomus\\.tests(\\.[0-9]+)?$",
/// ]);
/// ```
pub fn allow_lines(schema: &Value) -> Vec<String> {
    let mut out = Vec::new();
    walk_allow(schema, schema, "", &mut out);
    out
}

fn resolve<'a>(root: &'a Value, node: &'a Value) -> &'a Value {
    match node
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| r.strip_prefix("#/"))
    {
        Some(pointer) => pointer.split('/').fold(root, |v, seg| &v[seg]),
        None => node,
    }
}

fn escape(key: &str) -> String {
    let mut s = String::with_capacity(key.len());
    for c in key.chars() {
        if ".+*?()[]{}^$|\\".contains(c) {
            s.push('\\');
        }
        s.push(c);
    }
    s
}

fn walk_allow(root: &Value, node: &Value, prefix: &str, out: &mut Vec<String>) {
    let node = resolve(root, node);
    let Some(props) = node.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (key, prop) in props {
        let prop = resolve(root, prop);
        let path = format!("{prefix}{}", escape(key));
        let is_object = prop.get("properties").is_some();
        let items = prop.get("items").map(|i| resolve(root, i));
        if is_object {
            walk_allow(root, prop, &format!("{path}\\."), out);
        } else if let Some(items) = items {
            if items.get("properties").is_some() {
                walk_allow(root, items, &format!("{path}\\.[0-9]+\\."), out);
            } else {
                out.push(format!("^{path}(\\.[0-9]+)?$"));
            }
        } else {
            out.push(format!("^{path}$"));
        }
    }
}

/// Write every artifact under `root`, creating the directory. Returns the paths written.
pub fn write(root: &Path, artifacts: &[Artifact]) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for a in artifacts {
        let path = root.join(&a.path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        std::fs::write(&path, &a.content).map_err(|e| Error::io(&path, e))?;
        written.push(path);
    }
    Ok(written)
}

/// Compare every artifact with what is on disk under `root`; write nothing.
pub fn check(root: &Path, artifacts: &[Artifact]) -> Result<()> {
    let mut stale = Vec::new();
    for a in artifacts {
        let path = root.join(&a.path);
        match std::fs::read_to_string(&path) {
            Ok(on_disk) if on_disk == a.content => {}
            Ok(_) => stale.push(format!("{} (differs)", a.path)),
            Err(_) => stale.push(format!("{} (missing)", a.path)),
        }
    }
    if stale.is_empty() {
        Ok(())
    } else {
        Err(Error::Stale { files: stale })
    }
}

/// The capability reference's index: every module, every builtin capability with its
/// projections, and the declarative kinds by rule. Declarative objects are not enumerated
/// here because that inventory belongs to the repository's own state, changes with it, and
/// is answered live by `majordomus capabilities list`.
fn reference(registry: &CapabilityRegistry, version: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("<!-- {HEADER}\n     Source: the canonical Majordomus capability registry; regenerate with `majordomus generate`\n     Generator: majordomus-cli {version} -->\n"));
    s.push_str("# Capability reference\n\n");
    s.push_str("Every capability this executable ships, as the registry holds it. MCP tools and resources, HTTP routes, the OpenAPI document (`openapi.json` beside this file, and `/openapi.json` when serving), Swagger UI, the command line's `capabilities` commands, the benchmark targets (`benchmarks.md`) and the registry manifest (`registry.json`) are projections of the same entries; nothing below is declared anywhere else.\n\n");
    s.push_str("## Modules\n\n| module | title | stability | capabilities | reference |\n|---|---|---|---|---|\n");
    for m in registry
        .modules()
        .filter(|m| m.source != ModuleSource::Declarative)
    {
        let stability = m
            .stability
            .and_then(|st| serde_json::to_value(st).ok())
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | [`modules/{}.md`](modules/{}.md) |\n",
            m.id, m.title, stability, m.capabilities, m.id, m.id
        ));
    }
    s.push_str("\n## Executable capabilities\n\n");
    s.push_str("| id | module | kind | stability | MCP tool | MCP resource | HTTP | CLI | cache | benchmark |\n|---|---|---|---|---|---|---|---|---|---|\n");
    for c in registry
        .iter()
        .filter(|c| matches!(c.provenance, Provenance::Builtin { .. }))
    {
        s.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            c.id,
            c.module,
            enum_name(c.kind),
            enum_name(c.stability),
            c.exposure
                .mcp
                .as_ref()
                .and_then(|m| m.tool.clone())
                .map(|t| format!("`{t}`"))
                .unwrap_or("—".into()),
            c.exposure
                .mcp
                .as_ref()
                .and_then(|m| m.resource.as_ref())
                .map(|r| format!("`{}`", r.uri))
                .unwrap_or("—".into()),
            c.exposure
                .http
                .as_ref()
                .map(|h| format!("`{} {}`", h.method.as_str(), h.path))
                .unwrap_or("—".into()),
            c.exposure
                .cli
                .as_ref()
                .map(|x| format!("`majordomus {}`", x.path.join(" ")))
                .unwrap_or("—".into()),
            cache_cell(c.cache),
            benchmark_cell(c.benchmark),
        ));
    }
    s.push_str("\n## Declarative resources\n\n");
    s.push_str("Every object of the repository's AI layer is a capability of kind `resource` with the id `<kind>.<identity>` (`rule.majordomus.scope-integrity@1`, `prompt.continue`, `document.docs/CLI.md`), exposed as the MCP resource `majordomus://<kind>/<identity>` and read over HTTP through `objects.get`; its module is its kind. They are not listed here: they are the repository's, not the executable's, and `majordomus capabilities list --kind resource` answers for the repository at hand. Kinds present in this repository at generation: ");
    let kinds: Vec<String> = registry
        .modules()
        .filter(|m| m.source == ModuleSource::Declarative)
        .map(|m| format!("`{}`", m.id))
        .collect();
    s.push_str(&if kinds.is_empty() {
        "none".to_string()
    } else {
        kinds.join(", ")
    });
    s.push_str(".\n\n## Infrastructure routes\n\n");
    s.push_str("The HTTP projection's own routes, not capabilities: ");
    s.push_str(
        &openapi::INFRASTRUCTURE_ROUTES
            .iter()
            .map(|r| format!("`{r}`"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    s.push_str(
        ". `/docs` is a Swagger UI shell that loads `/openapi.json`; it embeds no specification. `/mcp` is MCP over HTTP on the shared server.\n",
    );
    let _ = CapabilityKind::Query; // the kind vocabulary is documented in docs/CAPABILITIES.md
    s
}

/// One module's reference: its metadata and each capability in full.
fn module_reference(registry: &CapabilityRegistry, module: &str, version: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("<!-- {HEADER}\n     Source: the canonical Majordomus capability registry, module `{module}`; regenerate with `majordomus generate`\n     Generator: majordomus-cli {version} -->\n"));
    if let Some(m) = registry.module(module) {
        s.push_str(&format!(
            "# Module `{}` — {}\n\n{}\n\n",
            m.id, m.title, m.description
        ));
        if let Some(st) = m.stability {
            s.push_str(&format!(
                "Stability: {}. Capabilities: {}.\n\n",
                enum_name(st),
                m.capabilities
            ));
        }
    }
    for c in registry.iter().filter(|c| {
        c.module.as_str() == module && matches!(c.provenance, Provenance::Builtin { .. })
    }) {
        s.push_str(&format!(
            "## `{}` — {}\n\n{}\n\n",
            c.id, c.title, c.description
        ));
        s.push_str("| | |\n|---|---|\n");
        s.push_str(&format!("| kind | {} |\n", enum_name(c.kind)));
        s.push_str(&format!("| stability | {} |\n", enum_name(c.stability)));
        if let Some(t) = c.exposure.mcp.as_ref().and_then(|m| m.tool.as_ref()) {
            s.push_str(&format!("| MCP tool | `{t}` |\n"));
        }
        if let Some(r) = c.exposure.mcp.as_ref().and_then(|m| m.resource.as_ref()) {
            s.push_str(&format!("| MCP resource | `{}` |\n", r.uri));
        }
        if let Some(h) = &c.exposure.http {
            s.push_str(&format!("| HTTP | `{} {}` |\n", h.method.as_str(), h.path));
        }
        if let Some(x) = &c.exposure.cli {
            s.push_str(&format!("| CLI | `majordomus {}` |\n", x.path.join(" ")));
        }
        s.push_str(&format!("| cache | {} |\n", cache_cell(c.cache)));
        s.push_str(&format!(
            "| benchmark | {} |\n",
            benchmark_cell(c.benchmark)
        ));
        s.push_str(&format!("| provenance | {} |\n", c.provenance));
        if !c.tags.is_empty() {
            s.push_str(&format!("| tags | {} |\n", c.tags.join(", ")));
        }
        s.push('\n');
        let (props, required) = c.input.properties();
        if props.is_empty() {
            s.push_str("Input: none.\n\n");
        } else {
            s.push_str("| input | type | required | description |\n|---|---|---|---|\n");
            for (name, schema) in props {
                let ty = match schema.get("type") {
                    Some(serde_json::Value::String(t)) => t.clone(),
                    Some(serde_json::Value::Array(a)) => a
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(" or "),
                    _ => schema
                        .get("$ref")
                        .and_then(|r| r.as_str())
                        .map(|r| r.rsplit('/').next().unwrap_or(r).to_string())
                        .unwrap_or("object".into()),
                };
                let desc = schema
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .replace('|', "\\|");
                s.push_str(&format!(
                    "| `{name}` | {ty} | {} | {desc} |\n",
                    if required.contains(&name) {
                        "yes"
                    } else {
                        "no"
                    }
                ));
            }
            s.push('\n');
        }
        s.push_str(&format!(
            "Output: `{}`.\n\n",
            c.output.name.as_deref().unwrap_or("object")
        ));
    }
    s
}

/// The native command line as Markdown: every command with its arguments, from the clap
/// declaration ([`crate::cli::tree`]). The site renders the same tree from the registry
/// dataset; neither is typed by hand.
pub fn cli_reference(tree: &crate::cli::CommandDoc, version: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("<!-- {HEADER}\n     Source: the clap declaration of the command line and the examples declared with it ({});\n     regenerate with `majordomus generate`\n     Generator: majordomus-cli {version} -->\n", crate::cli::DECLARATION));
    s.push_str("# Command line of the Rust executable\n\n");
    s.push_str(&format!("{}\n\n", tree.about));
    if let Some(long) = &tree.long_about {
        s.push_str(&format!("{long}\n\n"));
    }
    s.push_str(&format!("Every command below is declared once, in [`{}`](../../{}), together with its examples; this file is a projection of that declaration, as `--help` is, as `docs/generated/cli.json` is, and as the website's reference under `{}/` is. Every example printed here is executed against the built executable by `apps/majordomus-cli/tests/cli_examples.rs`. The task lifecycle (`init`, `start`, `check`, `finish`, `doctor`, ...) is the *shell* tool `bin/majordomus`, a different program, documented in `docs/CLI.md`.\n\n", crate::cli::DECLARATION, crate::cli::DECLARATION, crate::cli::ROUTE_PREFIX));
    s.push_str("## Commands\n\n| command | route | does |\n|---|---|---|\n");
    for c in tree.flatten().into_iter().skip(1) {
        s.push_str(&format!(
            "| [`{}`](#{}) | `{}` | {} |\n",
            c.path.join(" "),
            c.path.join("-"),
            c.route,
            c.about
        ));
    }
    s.push('\n');
    for c in tree.flatten() {
        let name = c.path.join(" ");
        let anchor = c.path.join("-");
        s.push_str(&format!("<a id=\"{anchor}\"></a>\n## `{name}`\n\n"));
        if !c.subcommands.is_empty() || c.path.len() > 1 {
            s.push_str(&format!("{}\n\n", c.about));
        }
        if let (Some(long), true) = (&c.long_about, c.path.len() > 1) {
            s.push_str(&format!("{long}\n\n"));
        }
        if !c.subcommands.is_empty() {
            s.push_str("Subcommands: ");
            s.push_str(
                &c.subcommands
                    .iter()
                    .map(|sc| format!("[`{}`](#{})", sc.path.join(" "), sc.path.join("-")))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            s.push_str(".\n\n");
        }
        s.push_str(&format!("```text\n{}\n```\n\n", c.usage));
        if c.args.is_empty() {
            s.push_str("Arguments: none.\n\n");
        } else {
            s.push_str("| argument | value | default | description |\n|---|---|---|---|\n");
            for a in &c.args {
                let flag = match (&a.long, a.short, a.positional) {
                    (_, _, true) => format!(
                        "`<{}>`",
                        a.value_name.clone().unwrap_or(a.name.to_uppercase())
                    ),
                    (Some(l), Some(sh), _) => format!("`-{sh}`, `--{l}`"),
                    (Some(l), None, _) => format!("`--{l}`"),
                    (None, Some(sh), _) => format!("`-{sh}`"),
                    (None, None, _) => format!("`{}`", a.name),
                };
                let value = if !a.takes_value {
                    "flag".to_string()
                } else if !a.possible_values.is_empty() {
                    a.possible_values
                        .iter()
                        .map(|v| format!("`{}`", v.name))
                        .collect::<Vec<_>>()
                        .join(" \\| ")
                } else {
                    format!(
                        "`<{}>`",
                        a.value_name.clone().unwrap_or(a.name.to_uppercase())
                    )
                };
                let default = if a.defaults.is_empty() || !a.takes_value {
                    if a.required {
                        "required".to_string()
                    } else {
                        "—".to_string()
                    }
                } else {
                    a.defaults
                        .iter()
                        .map(|d| format!("`{d}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let mut help = a.help.replace('|', "\\|");
                if a.global {
                    help.push_str(" (accepted by every subcommand)");
                }
                let values: Vec<String> = a
                    .possible_values
                    .iter()
                    .filter_map(|v| {
                        v.help
                            .as_ref()
                            .map(|h| format!("`{}`: {}", v.name, h.replace('|', "\\|")))
                    })
                    .collect();
                if !values.is_empty() {
                    help.push_str(" — ");
                    help.push_str(&values.join("; "));
                }
                s.push_str(&format!("| {flag} | {value} | {default} | {help} |\n"));
            }
            s.push('\n');
        }
        s.push_str(&cli_examples(c));
    }
    s
}

/// The whole command line as the committed machine-readable projection,
/// `docs/generated/cli.json`: pretty JSON with a trailing newline, the same tree
/// `cli_reference` renders as Markdown. The website's generator reads this file and never
/// the Rust source.
pub fn cli_document(tree: &crate::cli::CommandDoc, version: &str) -> String {
    let mut s =
        serde_json::to_string_pretty(&crate::cli::document(tree, version)).unwrap_or_default();
    s.push('\n');
    s
}

/// The examples of one command, as the reference prints them: the title, what it does, the
/// session a reader copies (the setup lines first, then the example itself), and what the
/// example test asserts about the run. Rendered from the same declaration the tests
/// execute, so nothing here can be true of the page and false of the executable.
fn cli_examples(c: &crate::cli::CommandDoc) -> String {
    let mut s = String::new();
    if c.examples.is_empty() {
        if c.executable {
            // cli::validate refuses this before it can be generated; the branch exists so
            // that the renderer never invents a heading for an empty list.
            return s;
        }
        return s;
    }
    s.push_str("Examples:\n\n");
    for e in &c.examples {
        s.push_str(&format!("- **{}** — {}\n\n", e.title, e.description));
        s.push_str("  ```console\n");
        for step in &e.setup {
            s.push_str(&format!("  $ {}\n", step.command));
        }
        s.push_str(&format!("  $ {}\n  ```\n\n", e.command));
        s.push_str(&format!("  Verified: {}.\n\n", e.expectation));
    }
    s
}

fn enum_name<T: serde::Serialize>(v: T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn cache_cell(policy: crate::capability::CachePolicy) -> String {
    match policy {
        crate::capability::CachePolicy::Disabled => "—".into(),
        crate::capability::CachePolicy::Process {
            max_entries,
            ttl_seconds: None,
        } => format!("process, {max_entries} entries"),
        crate::capability::CachePolicy::Process {
            max_entries,
            ttl_seconds: Some(ttl),
        } => format!("process, {max_entries} entries, {ttl}s"),
    }
}

fn benchmark_cell(policy: crate::capability::BenchmarkPolicy) -> String {
    match policy {
        crate::capability::BenchmarkPolicy::Required => "required".into(),
        crate::capability::BenchmarkPolicy::Waived { reason } => {
            format!("waived ({})", enum_name(reason))
        }
    }
}
