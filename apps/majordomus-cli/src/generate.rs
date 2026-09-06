//! Generated projections that are committed for review: the OpenAPI document, the
//! capability reference, the benchmark matrix, the registry manifest, and the shell
//! tool's key allow-lists derived from the JSON Schemas. All come through this one
//! pipeline; the committed files are caches of it, never sources, and `--check` says when
//! they are stale.
//!
//! Every artifact is *typed*: it declares the document it projects, the encoding it is
//! written in, the JSON Schema its content satisfies when it has one, and the source it
//! was derived from. A structured document is written in every encoding this repository
//! commits it in — JSON for a program, YAML for a person editing configuration beside it,
//! Markdown for a reader — from one value, so the encodings cannot disagree. Every
//! artifact carries a provenance header in the form its encoding allows, and
//! [`manifest_document`] is the index of the whole set, itself generated.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::app::App;
use crate::bench::{BenchmarkProjection, Coverage, CoverageState, SystemTarget, Transport};
use crate::capability::registry::ModuleSource;
use crate::capability::{CapabilityKind, CapabilityRegistry, CaseContext, Context, Provenance};
use crate::error::{Error, Result};
use crate::http::openapi;
use crate::metadata::{yaml, KindSchema};
use crate::policy::{sha256_hex, LoadedPolicy};
use crate::share::Share;

/// Where generated artifacts live, relative to the repository root.
pub const OUT_DIR: &str = "docs/generated";
/// Where the registry dataset the site renders lives, relative to the repository root. The
/// shell site generator owns `site/data/generated/` wholesale; this directory is the Rust
/// executable's, so that no directory has two writers.
pub const SITE_DATA_DIR: &str = "site/data/registry";

/// The first thing every generated artifact says about itself, whatever its encoding
/// wraps it in.
pub const HEADER: &str = "GENERATED FILE — DO NOT EDIT DIRECTLY";

/// The command that rewrites any of them.
pub const REGENERATE: &str = "majordomus generate";

/// The schema of `registry.json`.
pub const REGISTRY_SCHEMA: &str = "majordomus/capability-registry/v1";

/// The schema of `benchmarks.json`.
pub const BENCHMARKS_SCHEMA: &str = "majordomus/benchmark-matrix/v1";

/// The schema of `artifacts.json`, the manifest of every generated artifact.
pub const MANIFEST_SCHEMA: &str = "majordomus/generated-artifacts/v1";

/// The schema extension that names the allow-list a schema derives.
pub const ALLOW_EXTENSION: &str = "x-majordomus-allow";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// What can be generated.
pub enum Target {
    /// `docs/generated/openapi.{json,yaml}`.
    OpenApi,
    /// `docs/generated/capabilities.md` (the index), `docs/generated/modules/<id>.md`,
    /// `docs/generated/cli.md` and `docs/generated/cli.{json,yaml}` (the command line as
    /// clap declares it, with the examples declared beside it).
    Docs,
    /// `docs/generated/benchmarks.{md,json,yaml}`: every benchmark target and the
    /// coverage, from the projection.
    Benchmarks,
    /// `docs/generated/registry.{json,yaml}`: the builtin registry as data,
    /// `majordomus/capability-registry/v1`.
    Registry,
    /// `<share>/allow/<name>.txt` for every schema that carries `x-majordomus-allow`.
    Allow,
    /// The projections of every `.proto` document schema: the JSON Schema beside it and
    /// `<share>/sections/<name>.txt`, the body half of its contract (see
    /// [`crate::proto::project`]).
    Documents,
    /// The provider bootstraps the policy's `projections[]` declare, rendered from the
    /// provider templates: `AGENTS.md`, `CLAUDE.md`, ... (see [`crate::providers`]).
    Providers,
    /// `site/data/registry/registry.json`: the registry dataset GitHub Pages renders
    /// (see [`crate::site`]).
    Site,
    /// `docs/generated/artifacts.{json,yaml,md}`: every artifact of every other target,
    /// with its encoding, schema, source and hash. Always planned over the whole set, so
    /// that a manifest naming half the artifacts cannot exist.
    Manifest,
}

impl Target {
    /// Every target, in generation order. [`Target::Manifest`] is last because it indexes
    /// the others.
    pub const ALL: &'static [Target] = &[
        Target::OpenApi,
        Target::Docs,
        Target::Benchmarks,
        Target::Registry,
        Target::Documents,
        Target::Allow,
        Target::Providers,
        Target::Site,
        Target::Manifest,
    ];

    /// Every target but the manifest: the artifacts the manifest indexes.
    pub const INDEXED: &'static [Target] = &[
        Target::OpenApi,
        Target::Docs,
        Target::Benchmarks,
        Target::Registry,
        Target::Allow,
        Target::Documents,
        Target::Providers,
        Target::Site,
    ];

    /// The name the command line and the manifest use.
    pub fn name(self) -> &'static str {
        match self {
            Target::OpenApi => "openapi",
            Target::Docs => "docs",
            Target::Benchmarks => "benchmarks",
            Target::Registry => "registry",
            Target::Allow => "allow",
            Target::Documents => "documents",
            Target::Providers => "providers",
            Target::Site => "site",
            Target::Manifest => "manifest",
        }
    }
}

/// The encoding one generated artifact is written in.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactFormat {
    /// A JSON document: pretty-printed, one trailing newline, provenance as members.
    Json,
    /// The same document in the layer's YAML, provenance as a comment banner.
    Yaml,
    /// Markdown for a reader, provenance as an HTML comment.
    Markdown,
    /// Line-oriented text another program reads: the shell tool's allow-lists,
    /// provenance as `#` comments.
    Text,
}

impl ArtifactFormat {
    /// The file suffix, without the dot.
    pub fn suffix(self) -> &'static str {
        match self {
            ArtifactFormat::Json => "json",
            ArtifactFormat::Yaml => "yaml",
            ArtifactFormat::Markdown => "md",
            ArtifactFormat::Text => "txt",
        }
    }

    /// The format a path's suffix declares, `Text` for anything unrecognised.
    pub fn of_path(path: &str) -> ArtifactFormat {
        match path.rsplit_once('.').map(|(_, s)| s) {
            Some("json") => ArtifactFormat::Json,
            Some("yaml") | Some("yml") => ArtifactFormat::Yaml,
            Some("md") => ArtifactFormat::Markdown,
            _ => ArtifactFormat::Text,
        }
    }
}

/// The three lines every provenance header says, whatever the encoding wraps them in.
pub fn banner_lines(source: &str, version: &str) -> [String; 3] {
    [
        HEADER.to_string(),
        format!("Source: {source}; regenerate with `{REGENERATE}`"),
        format!("Generator: majordomus-cli {version}"),
    ]
}

/// The provenance header of a Markdown artifact: an HTML comment, so it is invisible when
/// rendered and unmissable in the file.
pub fn markdown_banner(source: &str, version: &str) -> String {
    let [a, b, c] = banner_lines(source, version);
    format!("<!-- {a}\n     {b}\n     {c} -->\n")
}

/// The provenance header of a YAML or text artifact: `#` comments, which every reader of
/// both — the layer's YAML parser, `grep -E -f`, awk — skips.
pub fn comment_banner(source: &str, version: &str) -> String {
    banner_lines(source, version)
        .iter()
        .map(|l| format!("# {l}\n"))
        .collect()
}

/// The one line a JSON artifact carries under `generated`.
pub fn json_banner(source: &str) -> String {
    format!("{HEADER}; source: {source}; regenerate with `{REGENERATE}`")
}

/// Where a structured document puts its provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderStyle {
    /// `schema`, `generated` and `generator` as members: this repository's own documents.
    Members,
    /// `x-majordomus-generated` and `x-majordomus-generator`: a document whose own
    /// specification fixes the member names, so provenance goes in an extension. The
    /// OpenAPI document is the only one.
    Extension,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One generated file: what it projects, how it is encoded, what contract its content
/// satisfies, where it came from, and the whole file.
pub struct Artifact {
    /// Repository-relative path.
    pub path: String,
    /// The document it projects: `registry`, `cli`, `openapi`, `capabilities`, ... Two
    /// artifacts sharing an id are the same document in two encodings.
    pub document: String,
    /// The encoding.
    pub format: ArtifactFormat,
    /// The JSON Schema its content satisfies, by schema id, when it declares one.
    pub schema: Option<String>,
    /// One line: what it was derived from, as the header says it.
    pub source: String,
    /// The whole file.
    pub content: String,
}

impl Artifact {
    /// A Markdown artifact: the banner, then the body. Nothing writes the banner itself.
    pub fn markdown(
        path: impl Into<String>,
        document: impl Into<String>,
        source: impl Into<String>,
        version: &str,
        body: &str,
    ) -> Artifact {
        let source = source.into();
        Artifact {
            path: path.into(),
            document: document.into(),
            format: ArtifactFormat::Markdown,
            schema: None,
            content: markdown_banner(&source, version) + body,
            source,
        }
    }

    /// A text artifact another program reads line by line: the banner as `#` comments,
    /// then the lines.
    pub fn text(
        path: impl Into<String>,
        document: impl Into<String>,
        source: impl Into<String>,
        version: &str,
        body: &str,
    ) -> Artifact {
        let source = source.into();
        Artifact {
            path: path.into(),
            document: document.into(),
            format: ArtifactFormat::Text,
            schema: None,
            content: comment_banner(&source, version) + body,
            source,
        }
    }

    /// A JSON artifact whose own specification fixes its member names — a JSON Schema,
    /// the OpenAPI document — so its provenance goes in `x-majordomus-` extensions rather
    /// than in members the specification does not define. Both forms are read back by
    /// [`violations`]; neither is more generated than the other.
    pub fn json_extension(
        path: impl Into<String>,
        document: impl Into<String>,
        source: impl Into<String>,
        version: &str,
        value: Value,
    ) -> Artifact {
        let source = source.into();
        let stamped = match value {
            Value::Object(members) => {
                let mut out = members;
                out.insert(
                    "x-majordomus-generated".into(),
                    Value::String(json_banner(&source)),
                );
                out.insert(
                    "x-majordomus-generator".into(),
                    Value::String(format!("majordomus-cli {version}")),
                );
                Value::Object(out)
            }
            other => other,
        };
        Artifact {
            path: path.into(),
            document: document.into(),
            format: ArtifactFormat::Json,
            schema: None,
            source,
            content: openapi::render(&stamped),
        }
    }

    /// An artifact whose content is taken as it stands: a projection another module
    /// renders whole and stamps itself (the provider bootstraps, the site dataset).
    pub fn verbatim(
        path: impl Into<String>,
        document: impl Into<String>,
        format: ArtifactFormat,
        schema: Option<String>,
        source: impl Into<String>,
        content: String,
    ) -> Artifact {
        Artifact {
            path: path.into(),
            document: document.into(),
            format,
            schema,
            source: source.into(),
            content,
        }
    }
}

/// One generated *document*: a value with an identity, a schema and a provenance line,
/// projected into every encoding this repository commits it in. The encodings cannot
/// disagree because there is one value; adding an encoding is a line in
/// [`Document::artifacts`], never a second renderer.
#[derive(Debug, Clone)]
pub struct Document {
    /// The document id, which is also the file stem: `registry`, `cli`, `openapi`.
    pub id: String,
    /// The directory the encodings are written into, repository-relative.
    pub dir: String,
    /// The JSON Schema the value satisfies, when it declares one.
    pub schema: Option<String>,
    /// One line: what it was derived from.
    pub source: String,
    /// Where the provenance goes.
    pub style: HeaderStyle,
    /// The value, before provenance is added.
    pub value: Value,
}

impl Document {
    /// A document of this repository's own, provenance as members.
    pub fn new(
        id: impl Into<String>,
        schema: impl Into<String>,
        source: impl Into<String>,
        value: Value,
    ) -> Document {
        Document {
            id: id.into(),
            dir: OUT_DIR.to_string(),
            schema: Some(schema.into()),
            source: source.into(),
            style: HeaderStyle::Members,
            value,
        }
    }

    /// The value with its provenance in it: what both encodings serialise.
    pub fn stamped(&self, version: &str) -> Value {
        let Value::Object(members) = &self.value else {
            return self.value.clone();
        };
        let mut out = Map::new();
        match self.style {
            HeaderStyle::Members => {
                if let Some(schema) = &self.schema {
                    out.insert("schema".into(), Value::String(schema.clone()));
                }
                out.insert("generated".into(), Value::String(json_banner(&self.source)));
                out.insert(
                    "generator".into(),
                    Value::String(format!("majordomus-cli {version}")),
                );
                for (k, v) in members {
                    out.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
            HeaderStyle::Extension => {
                for (k, v) in members {
                    out.insert(k.clone(), v.clone());
                }
                out.insert(
                    "x-majordomus-generated".into(),
                    Value::String(json_banner(&self.source)),
                );
                out.insert(
                    "x-majordomus-generator".into(),
                    Value::String(format!("majordomus-cli {version}")),
                );
            }
        }
        Value::Object(out)
    }

    /// The document in every encoding it is committed in: JSON, then YAML.
    pub fn artifacts(&self, version: &str) -> Vec<Artifact> {
        let stamped = self.stamped(version);
        let base = |format: ArtifactFormat, content: String| Artifact {
            path: format!("{}/{}.{}", self.dir, self.id, format.suffix()),
            document: self.id.clone(),
            format,
            schema: self.schema.clone(),
            source: self.source.clone(),
            content,
        };
        vec![
            base(ArtifactFormat::Json, openapi::render(&stamped)),
            base(
                ArtifactFormat::Yaml,
                yaml::render_with_banner(&stamped, &banner_lines(&self.source, version).join("\n")),
            ),
        ]
    }
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
            Target::OpenApi => {
                let doc = Document {
                    id: "openapi".into(),
                    dir: OUT_DIR.into(),
                    // the document's own contract is the OpenAPI specification, named by
                    // its `openapi` member; it declares no schema of ours
                    schema: None,
                    source: "the canonical Majordomus capability registry".into(),
                    style: HeaderStyle::Extension,
                    value: openapi::document(registry, version, cases)
                        .map_err(|reason| Error::Http { reason })?,
                };
                out.extend(doc.artifacts(version));
            }
            Target::Docs => {
                out.push(Artifact::markdown(
                    format!("{OUT_DIR}/capabilities.md"),
                    "capabilities",
                    "the canonical Majordomus capability registry",
                    version,
                    &reference(registry),
                ));
                let cli = crate::cli::tree();
                out.push(Artifact::markdown(
                    format!("{OUT_DIR}/cli.md"),
                    "cli",
                    "the clap declaration in apps/majordomus-cli/src/cli.rs and the examples beside it",
                    version,
                    &cli_reference(&cli),
                ));
                out.extend(
                    Document::new(
                        "cli",
                        crate::cli::SCHEMA,
                        "the clap declaration in apps/majordomus-cli/src/cli.rs and the examples beside it",
                        cli_document(&cli, version),
                    )
                    .artifacts(version),
                );
                for m in registry
                    .modules()
                    .filter(|m| m.source != ModuleSource::Declarative)
                {
                    out.push(Artifact::markdown(
                        format!("{OUT_DIR}/modules/{}.md", m.id),
                        format!("modules/{}", m.id),
                        format!(
                            "the `{}` module of the canonical Majordomus capability registry",
                            m.id
                        ),
                        version,
                        &module_reference(registry, m.id.as_str()),
                    ));
                }
            }
            Target::Registry => out.extend(
                Document::new(
                    "registry",
                    REGISTRY_SCHEMA,
                    "the canonical capability registry",
                    registry_manifest(registry),
                )
                .artifacts(version),
            ),
            Target::Benchmarks
            | Target::Allow
            | Target::Documents
            | Target::Providers
            | Target::Site
            | Target::Manifest => {}
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
    // the manifest indexes the artifacts of the plan it belongs to. Asked for alone
    // (`generate manifest`) that plan is every other target, because a manifest of nothing
    // but itself would be a lie; asked for beside a subset, it indexes that subset.
    let wants_manifest = targets.contains(&Target::Manifest);
    let alone = targets == [Target::Manifest];
    let indexed: Vec<Target> = if alone {
        Target::INDEXED.to_vec()
    } else {
        targets
            .iter()
            .copied()
            .filter(|t| *t != Target::Manifest)
            .collect()
    };
    let mut out = indexed_plan(app, &indexed)?;
    if wants_manifest {
        let manifest = manifest_document(&out);
        if alone {
            out.clear();
        }
        out.extend(manifest_artifacts(&manifest, crate::VERSION));
    }
    Ok(out)
}

/// Every artifact the manifest indexes: everything but the manifest itself.
fn indexed_plan(app: &App, targets: &[Target]) -> Result<Vec<Artifact>> {
    let mut out = context_artifacts(&app.context, crate::VERSION, targets)?;
    if targets.contains(&Target::Documents) || targets.contains(&Target::Allow) {
        let protos = document_schemas(&app.share, app.repository.root())?;
        if targets.contains(&Target::Documents) {
            out.extend(document_artifacts(
                &protos,
                &app.share,
                app.repository.root(),
                crate::VERSION,
            )?);
        }
        if targets.contains(&Target::Allow) {
            out.extend(proto_allow_artifacts(
                &protos,
                &app.share,
                app.repository.root(),
                crate::VERSION,
            )?);
            out.extend(allow_artifacts(
                &app.schema,
                &app.share,
                app.repository.root(),
                crate::VERSION,
            ));
        }
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
            out.push(Artifact::verbatim(
                format!("{SITE_DATA_DIR}/registry.json"),
                "site-registry",
                ArtifactFormat::Json,
                Some(crate::site::SCHEMA.to_string()),
                "the capability registry and the index of this repository's layer",
                crate::site::render(&dataset),
            ));
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
        let source = "the benchmark projection of the canonical capability registry";
        out.push(Artifact::markdown(
            format!("{OUT_DIR}/benchmarks.md"),
            "benchmarks",
            source,
            version,
            &benchmark_matrix(ctx),
        ));
        out.extend(
            Document::new(
                "benchmarks",
                BENCHMARKS_SCHEMA,
                source,
                benchmark_document(ctx),
            )
            .artifacts(version),
        );
    }
    Ok(out)
}

/// The builtin registry as data: modules, descriptors with their schemas, and the
/// declarative kinds by name. No fingerprint and no count of declarative objects, so the
/// file changes when the code changes and not when a document is added.
pub fn registry_manifest(registry: &CapabilityRegistry) -> Value {
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
    serde_json::json!({
        "modules": modules,
        "capabilities": capabilities,
        "declarative_kinds": declarative_kinds,
        "system_benchmark_targets": SystemTarget::ALL.iter().map(|s| serde_json::json!({ "key": s.key(), "transport": s.transport(), "description": s.description() })).collect::<Vec<_>>(),
    })
}

/// Every benchmark target and the coverage, from the projection of this repository.
pub fn benchmark_matrix(ctx: &Context) -> String {
    let projection = BenchmarkProjection::from_context(ctx);
    let coverage = Coverage::compute(ctx, &projection);
    let mut s = String::new();
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

/// Every `.proto` document schema, from the distribution and then from the repository,
/// keyed by the identity each declares. A repository may add a schema; it may not redefine
/// one the distribution declares, and an attempt to is an error naming both files.
pub fn document_schemas(
    share: &Share,
    root: &Path,
) -> Result<std::collections::BTreeMap<String, crate::proto::ProtoFile>> {
    let dir = share.schemas_dir();
    crate::proto::read_dir(&dir, &relative_to(&dir, root))
}

/// The projections of the `.proto` document schemas: the JSON Schema beside each one, and
/// the section list the shell tool reads its body contract from.
///
/// Both are committed. `bin/majordomus doctor` is pure shell and validates a fresh
/// checkout with no Rust build having ever run, so the derived forms have to be in the
/// tree; `generate --check` is what keeps them honest.
pub fn document_artifacts(
    protos: &std::collections::BTreeMap<String, crate::proto::ProtoFile>,
    share: &Share,
    root: &Path,
    version: &str,
) -> Result<Vec<Artifact>> {
    let schemas = relative_to(&share.schemas_dir(), root);
    let sections = relative_to(&share.sections_dir(), root);
    let mut out = Vec::new();
    for file in protos.values() {
        let schema = crate::proto::project::to_json_schema(file)?;
        // Relative to the repository, never as this machine found it: the banner is
        // committed, so an absolute path would make the artifact differ by the directory it
        // was generated in — every other checkout would then regenerate all of them and
        // `generate --check` would fail for everyone but the last person to run it.
        let source = format!(
            "the document schema `{}`",
            relative_to(Path::new(&file.source), root)
        );
        let path = format!(
            "{schemas}/{}",
            crate::proto::project::schema_path(&file.schema_id)?
        );
        out.push(Artifact::json_extension(
            path.clone(),
            format!("schemas/{}", file.schema_id),
            source.clone(),
            version,
            serde_json::to_value(&schema).map_err(|e| Error::KindSchema {
                reason: format!("{}: cannot render the projected schema: {e}", file.source),
            })?,
        ));
        if let Some(name) = &file.allow_list {
            let lines = crate::proto::project::section_lines(file);
            out.push(Artifact::text(
                format!("{sections}/{name}.txt"),
                format!("sections/{name}"),
                source.clone(),
                version,
                &if lines.is_empty() {
                    String::new()
                } else {
                    lines.join("\n") + "\n"
                },
            ));
        }
    }
    Ok(out)
}

/// The allow-lists of the `.proto` document schemas, derived from the same projection the
/// JSON Schema beside each one carries, so the two can never disagree.
pub fn proto_allow_artifacts(
    protos: &std::collections::BTreeMap<String, crate::proto::ProtoFile>,
    share: &Share,
    root: &Path,
    version: &str,
) -> Result<Vec<Artifact>> {
    let dir = relative_to(&share.allow_dir(), root);
    let mut out = Vec::new();
    for file in protos.values() {
        let Some(name) = &file.allow_list else {
            continue;
        };
        let schema = crate::proto::project::to_json_schema(file)?;
        out.push(Artifact::text(
            format!("{dir}/{name}.txt"),
            format!("allow/{name}"),
            format!(
                "the document schema `{}`",
                relative_to(Path::new(&file.source), root)
            ),
            version,
            &(allow_lines(&schema).join("\n") + "\n"),
        ));
    }
    Ok(out)
}

/// A share subdirectory as the plan names it: repository-relative when the share is inside
/// the repository, absolute otherwise.
fn relative_to(dir: &Path, root: &Path) -> String {
    dir.strip_prefix(root).unwrap_or(dir).display().to_string()
}

/// The benchmark matrix as data: the same projection `benchmark_matrix` renders for a
/// reader, in the encoding a program reads. One computation, two encodings; a number that
/// differs between the Markdown and the JSON would be two computations, which is the
/// defect this exists to make impossible.
pub fn benchmark_document(ctx: &Context) -> Value {
    let projection = BenchmarkProjection::from_context(ctx);
    let coverage = Coverage::compute(ctx, &projection);
    let coverage_rows: Vec<Value> = coverage
        .tallies
        .iter()
        .map(|(name, t)| {
            serde_json::json!({
                "scope": name,
                "required": t.required,
                "covered": t.covered,
                "missing": t.missing,
                "waived": t.waived,
            })
        })
        .collect();
    let capabilities: Vec<Value> = ctx
        .registry
        .iter()
        .filter(|c| c.kind.is_executable())
        .map(|c| {
            let state = |transport: Transport| -> &'static str {
                match coverage
                    .lines
                    .iter()
                    .find(|l| l.subject == c.id.as_str() && l.transport == transport)
                    .map(|l| l.state)
                {
                    Some(CoverageState::Covered) => "covered",
                    Some(CoverageState::Missing) => "missing",
                    Some(CoverageState::Waived) => "waived",
                    None => "none",
                }
            };
            let mut cases: Vec<String> = projection
                .of_capability(c.id.as_str())
                .filter(|t| t.transport() == Transport::Direct)
                .filter_map(|t| match &t.kind {
                    crate::bench::TargetKind::Capability { case, .. } => Some(case.clone()),
                    _ => None,
                })
                .collect();
            cases.dedup();
            serde_json::json!({
                "id": c.id,
                "module": c.module,
                "kind": enum_name(c.kind),
                "cache": cache_cell(c.cache),
                "direct": state(Transport::Direct),
                "mcp": state(Transport::Mcp),
                "http": state(Transport::Http),
                "cases": cases,
            })
        })
        .collect();
    serde_json::json!({
        "coverage": coverage_rows,
        "capabilities": capabilities,
        "system_targets": SystemTarget::ALL.iter().map(|s| serde_json::json!({
            "key": s.key(),
            "transport": s.transport().name(),
            "measures": s.description(),
        })).collect::<Vec<_>>(),
    })
}

// ---------------------------------------------------------------- the manifest

/// The document id of the manifest, and the file stem of its encodings.
pub const MANIFEST_ID: &str = "artifacts";

/// Where the manifest says it came from.
pub const MANIFEST_SOURCE: &str = "the generation plan itself, over every other target";

/// The three files the manifest is written to, in the order [`manifest_artifacts`]
/// returns them.
pub fn manifest_paths() -> [String; 3] {
    [
        format!("{OUT_DIR}/{MANIFEST_ID}.json"),
        format!("{OUT_DIR}/{MANIFEST_ID}.yaml"),
        format!("{OUT_DIR}/{MANIFEST_ID}.md"),
    ]
}

/// The index of every generated artifact: what each one projects, how it is encoded, what
/// contract it satisfies, where it came from, and its size and hash.
///
/// The manifest's own three files are listed with `describes_itself: true` and no hash:
/// a document that hashed itself would have no fixed point. Their staleness is not
/// unguarded — `generate --check` compares every artifact including these byte for byte,
/// which is a stronger statement than a hash the file makes about itself.
pub fn manifest_document(indexed: &[Artifact]) -> Value {
    let mut entries: Vec<Value> = indexed
        .iter()
        .map(|a| {
            let mut o = Map::new();
            o.insert("path".into(), Value::String(a.path.clone()));
            o.insert("document".into(), Value::String(a.document.clone()));
            o.insert(
                "format".into(),
                serde_json::to_value(a.format).unwrap_or(Value::Null),
            );
            if let Some(schema) = &a.schema {
                o.insert("schema".into(), Value::String(schema.clone()));
            }
            o.insert("source".into(), Value::String(a.source.clone()));
            o.insert("bytes".into(), Value::from(a.content.len()));
            o.insert("sha256".into(), Value::String(sha256_hex(&a.content)));
            Value::Object(o)
        })
        .collect();
    for path in manifest_paths() {
        let mut o = Map::new();
        o.insert("path".into(), Value::String(path.clone()));
        o.insert("document".into(), Value::String(MANIFEST_ID.into()));
        o.insert(
            "format".into(),
            serde_json::to_value(ArtifactFormat::of_path(&path)).unwrap_or(Value::Null),
        );
        if ArtifactFormat::of_path(&path) != ArtifactFormat::Markdown {
            o.insert("schema".into(), Value::String(MANIFEST_SCHEMA.into()));
        }
        o.insert("source".into(), Value::String(MANIFEST_SOURCE.into()));
        o.insert("describes_itself".into(), Value::Bool(true));
        entries.push(Value::Object(o));
    }
    entries.sort_by(|a, b| {
        a.get("path")
            .and_then(Value::as_str)
            .cmp(&b.get("path").and_then(Value::as_str))
    });

    // one row per document: the encodings it is committed in, which is the property a
    // reader and the enforcement rule actually ask about
    let mut documents: Vec<Value> = Vec::new();
    for entry in &entries {
        let id = entry.get("document").and_then(Value::as_str).unwrap_or("");
        let format = entry.get("format").cloned().unwrap_or(Value::Null);
        match documents
            .iter_mut()
            .find(|d| d.get("id").and_then(Value::as_str) == Some(id))
        {
            Some(Value::Object(d)) => {
                if let Some(Value::Array(formats)) = d.get_mut("formats") {
                    if !formats.contains(&format) {
                        formats.push(format);
                    }
                }
            }
            _ => {
                let mut d = Map::new();
                d.insert("id".into(), Value::String(id.to_string()));
                if let Some(schema) = entry.get("schema") {
                    d.insert("schema".into(), schema.clone());
                }
                d.insert(
                    "source".into(),
                    entry.get("source").cloned().unwrap_or(Value::Null),
                );
                d.insert("formats".into(), Value::Array(vec![format]));
                documents.push(Value::Object(d));
            }
        }
    }

    documents.sort_by(|a, b| {
        a.get("id")
            .and_then(Value::as_str)
            .cmp(&b.get("id").and_then(Value::as_str))
    });
    serde_json::json!({
        "documents": documents,
        "artifacts": entries,
    })
}

/// The manifest in its three encodings: JSON and YAML from the value, Markdown for a
/// reader. The Markdown is a rendering of the same value and holds nothing of its own.
pub fn manifest_artifacts(document: &Value, version: &str) -> Vec<Artifact> {
    let doc = Document::new(
        MANIFEST_ID,
        MANIFEST_SCHEMA,
        MANIFEST_SOURCE,
        document.clone(),
    );
    let mut out = doc.artifacts(version);
    out.push(Artifact::markdown(
        format!("{OUT_DIR}/{MANIFEST_ID}.md"),
        MANIFEST_ID,
        MANIFEST_SOURCE,
        version,
        &manifest_reference(document),
    ));
    out
}

/// The manifest as a reader sees it: the documents with the encodings each is committed
/// in, then every file with its contract, size and hash.
fn manifest_reference(document: &Value) -> String {
    let mut s = String::new();
    s.push_str("# Generated artifacts\n\n");
    s.push_str("Every file `majordomus generate` writes, with the document it projects, the encoding it is written in, the JSON Schema its content satisfies, and where it came from. Nothing below is a list kept by hand: it is the generation plan, generated. A file under a path this table does not name is not generated by this repository, and a file this table names that differs from the plan is stale — `majordomus generate --check` says which, byte for byte.\n\n");
    s.push_str("## Documents\n\n| document | encodings | schema | source |\n|---|---|---|---|\n");
    for d in document
        .get("documents")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let formats: Vec<String> = d
            .get("formats")
            .and_then(Value::as_array)
            .map(|f| {
                f.iter()
                    .filter_map(Value::as_str)
                    .map(|s| format!("`{s}`"))
                    .collect()
            })
            .unwrap_or_default();
        s.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            d.get("id").and_then(Value::as_str).unwrap_or(""),
            formats.join(", "),
            d.get("schema")
                .and_then(Value::as_str)
                .map(|x| format!("`{x}`"))
                .unwrap_or_else(|| "—".into()),
            d.get("source").and_then(Value::as_str).unwrap_or(""),
        ));
    }
    s.push_str(
        "\n## Files\n\n| path | document | format | bytes | sha256 |\n|---|---|---|---|---|\n",
    );
    for a in document
        .get("artifacts")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let hash = match a.get("sha256").and_then(Value::as_str) {
            Some(h) => format!("`{}`", &h[..16.min(h.len())]),
            // the manifest's own encodings: see `manifest_document`
            None => "— (this file)".to_string(),
        };
        s.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} |\n",
            a.get("path").and_then(Value::as_str).unwrap_or(""),
            a.get("document").and_then(Value::as_str).unwrap_or(""),
            a.get("format").and_then(Value::as_str).unwrap_or(""),
            a.get("bytes")
                .and_then(Value::as_u64)
                .map(|b| b.to_string())
                .unwrap_or_else(|| "—".into()),
            hash,
        ));
    }
    s
}

/// The shell tool's allow-lists, one per schema that carries `x-majordomus-allow`, under
/// the share directory: a repository-relative path when the share is inside the
/// repository, absolute otherwise.
pub fn allow_artifacts(
    schema: &KindSchema,
    share: &Share,
    root: &Path,
    version: &str,
) -> Vec<Artifact> {
    let dir = share.allow_dir();
    let dir = dir
        .strip_prefix(root)
        .map(|p| p.to_path_buf())
        .unwrap_or(dir);
    schema
        .schemas()
        .filter_map(|(name, sch)| {
            // A schema derived from a `.proto` is projected beside its source, not here;
            // emitting it twice would make the plan disagree with itself.
            if sch
                .json
                .get(crate::proto::project::DERIVED_EXTENSION)
                .is_some()
            {
                return None;
            }
            let list = sch.json.get(ALLOW_EXTENSION).and_then(Value::as_str)?;
            Some(Artifact::text(
                format!("{}/{list}.txt", dir.display()),
                format!("allow/{list}"),
                format!("the JSON Schema `{name}` of the kind it validates"),
                version,
                &(allow_lines(&sch.json).join("\n") + "\n"),
            ))
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
    // A whole-document schema describes the front matter under `header` and the Markdown
    // body under `body`; the allow-list is the front matter's keys, because that is what
    // the shell tool reads a file's front matter against. A schema with no `header`
    // describes the metadata directly, as every YAML kind's does.
    let start = schema
        .get("properties")
        .and_then(|p| p.get("header"))
        .unwrap_or(schema);
    walk_allow(schema, start, "", &mut out);
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

// ---------------------------------------------------------------- the contract

/// The contracts of the generated documents, read from `share/schemas/generated/`, keyed
/// by the schema id each one pins through the `const` of its `schema` member. A document
/// says which contract it satisfies; nothing here maps a file name to a file name.
pub struct GeneratedSchemas {
    by_id: std::collections::BTreeMap<String, (String, jsonschema::Validator)>,
}

impl std::fmt::Debug for GeneratedSchemas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeneratedSchemas")
            .field("documents", &self.by_id.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl GeneratedSchemas {
    /// Load every schema in the directory. A file that pins no schema id is a schema for
    /// nothing and is refused, because it would silently validate no document.
    pub fn load(dir: &Path) -> Result<GeneratedSchemas> {
        let mut by_id = std::collections::BTreeMap::new();
        for (name, json) in crate::share::read_generated_schema_dir(dir)? {
            let id = json
                .pointer("/properties/schema/const")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::KindSchema {
                    reason: format!(
                        "{}/{name}.schema.json pins no document: a schema of a generated document declares the id it validates as properties.schema.const",
                        dir.display()
                    ),
                })?
                .to_string();
            let validator = jsonschema::validator_for(&json).map_err(|e| Error::KindSchema {
                reason: format!("{}/{name}.schema.json: {e}", dir.display()),
            })?;
            if let Some((first, _)) = by_id.insert(id.clone(), (name.clone(), validator)) {
                return Err(Error::KindSchema {
                    reason: format!("schema id '{id}' is pinned by both {first} and {name}"),
                });
            }
        }
        Ok(GeneratedSchemas { by_id })
    }

    /// The schema ids that have a contract, sorted.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.by_id.keys().map(String::as_str)
    }

    /// Every reason `value` fails the contract of `schema_id`; empty when it passes or
    /// when no contract is published for that id.
    pub fn violations(&self, schema_id: &str, value: &Value) -> Vec<String> {
        let Some((_, validator)) = self.by_id.get(schema_id) else {
            return Vec::new();
        };
        validator
            .iter_errors(value)
            .map(|e| format!("{}: {e}", e.instance_path()))
            .collect()
    }
}

/// One thing wrong with a generated artifact, named by its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The artifact, repository-relative.
    pub path: String,
    /// What is wrong with it.
    pub reason: String,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.reason)
    }
}

/// Every way a plan can be wrong, checked before a byte is written.
///
/// Three properties, and they are the whole of the enforcement rule
/// `project.generated-artifacts-are-typed@1`:
///
/// 1. **Every artifact carries a provenance header** in the form its encoding allows —
///    members in JSON, a comment banner in YAML and text, an HTML comment in Markdown —
///    except the provider bootstraps, which stamp themselves with the policy hash they
///    were rendered from and are checked by `majordomus update` instead.
/// 2. **Every structured artifact parses**, and a document that declares a schema
///    satisfies the contract published for it under `share/schemas/generated/`.
/// 3. **The manifest is the plan**: exactly the artifacts of the plan, each with the
///    encoding, contract, size and hash it actually has.
pub fn violations(artifacts: &[Artifact], schemas: &GeneratedSchemas) -> Vec<Violation> {
    let mut out = Vec::new();
    let at = |path: &str, reason: String| Violation {
        path: path.to_string(),
        reason,
    };
    for a in artifacts {
        if a.format != ArtifactFormat::of_path(&a.path) {
            out.push(at(
                &a.path,
                format!(
                    "declares format {} and its suffix says {}",
                    a.format.suffix(),
                    ArtifactFormat::of_path(&a.path).suffix()
                ),
            ));
        }
        if a.source.trim().is_empty() {
            out.push(at(&a.path, "names no source".into()));
        }
        // the provider bootstraps carry the `majordomus update` stamp, not this banner:
        // a region projection owns part of a file it did not write, and a header at the
        // top of it would be a claim over text this generator does not own
        let stamped_elsewhere = a.document.starts_with("providers/");
        // the opening an encoding wraps the banner in: an HTML comment in Markdown, a `#`
        // comment in YAML and in line-oriented text, and nothing here for JSON, which
        // carries it as members and is checked below
        let opening = match a.format {
            ArtifactFormat::Markdown if !stamped_elsewhere => Some(format!("<!-- {HEADER}")),
            ArtifactFormat::Yaml | ArtifactFormat::Text => Some(format!("# {HEADER}")),
            _ => None,
        };
        if opening.is_some_and(|o| !a.content.starts_with(&o)) {
            out.push(at(&a.path, "carries no generated-file banner".into()));
        }
        if a.format == ArtifactFormat::Json {
            match serde_json::from_str::<Value>(&a.content) {
                Err(e) => out.push(at(&a.path, format!("is not JSON: {e}"))),
                Ok(value) => {
                    let banner = value
                        .get("generated")
                        .or_else(|| value.get("x-majordomus-generated"))
                        .and_then(Value::as_str);
                    match banner {
                        Some(b) if b.starts_with(HEADER) => {}
                        _ if stamped_elsewhere => {}
                        _ => out.push(at(
                            &a.path,
                            "carries no `generated` member saying it is generated".into(),
                        )),
                    }
                    if let Some(schema) = &a.schema {
                        let declared = value.get("schema").and_then(Value::as_str);
                        if declared.is_some() && declared != Some(schema.as_str()) {
                            out.push(at(
                                &a.path,
                                format!(
                                    "declares schema '{schema}' and carries '{}'",
                                    declared.unwrap_or("")
                                ),
                            ));
                        }
                        for reason in schemas.violations(schema, &value) {
                            out.push(at(&a.path, format!("does not satisfy {schema} — {reason}")));
                        }
                    }
                }
            }
        }
    }

    // --- the manifest is the plan. Only when the plan holds the artifacts it indexes:
    // `generate manifest` writes the manifest alone, and its content was computed from the
    // whole plan inside `plan`, so there is nothing here to compare it against.
    let manifest = artifacts
        .iter()
        .find(|a| a.document == MANIFEST_ID && a.format == ArtifactFormat::Json)
        .filter(|_| artifacts.iter().any(|a| a.document != MANIFEST_ID));
    if let Some(manifest) = manifest {
        let listed: Vec<Value> = serde_json::from_str::<Value>(&manifest.content)
            .ok()
            .and_then(|v| v.get("artifacts").and_then(Value::as_array).cloned())
            .unwrap_or_default();
        let mut planned: Vec<&Artifact> = artifacts.iter().collect();
        planned.sort_by(|a, b| a.path.cmp(&b.path));
        if listed.len() != planned.len() {
            out.push(at(
                &manifest.path,
                format!(
                    "lists {} artifacts and the plan has {}",
                    listed.len(),
                    planned.len()
                ),
            ));
        }
        for entry in &listed {
            let path = entry
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let Some(a) = artifacts.iter().find(|a| a.path == path) else {
                out.push(at(
                    &manifest.path,
                    format!("lists {path}, which is not planned"),
                ));
                continue;
            };
            if entry.get("describes_itself").is_some() {
                continue;
            }
            if entry.get("sha256").and_then(Value::as_str) != Some(sha256_hex(&a.content).as_str())
            {
                out.push(at(
                    &manifest.path,
                    format!("records a stale hash for {path}"),
                ));
            }
            if entry.get("bytes").and_then(Value::as_u64) != Some(a.content.len() as u64) {
                out.push(at(
                    &manifest.path,
                    format!("records a stale size for {path}"),
                ));
            }
        }
        for a in &planned {
            if !listed
                .iter()
                .any(|e| e.get("path").and_then(Value::as_str) == Some(a.path.as_str()))
            {
                out.push(at(&manifest.path, format!("does not list {}", a.path)));
            }
        }
    }
    out
}

/// [`violations`] as an error: nothing, or one `Stale` naming every one of them. This is
/// what `generate` and `generate --check` refuse on.
pub fn verify(artifacts: &[Artifact], schemas: &GeneratedSchemas) -> Result<()> {
    let violations = violations(artifacts, schemas);
    if violations.is_empty() {
        Ok(())
    } else {
        Err(Error::Stale {
            files: violations.iter().map(Violation::to_string).collect(),
        })
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
fn reference(registry: &CapabilityRegistry) -> String {
    let mut s = String::new();
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
fn module_reference(registry: &CapabilityRegistry, module: &str) -> String {
    let mut s = String::new();
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
pub fn cli_reference(tree: &crate::cli::CommandDoc) -> String {
    let mut s = String::new();
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
pub fn cli_document(tree: &crate::cli::CommandDoc, version: &str) -> Value {
    serde_json::to_value(crate::cli::document(tree, version)).unwrap_or(Value::Null)
}

/// The examples of one command, as the reference prints them: the title, what it does, the
/// session a reader copies (the setup lines first, then the example itself), and what the
/// example test asserts about the run. Rendered from the same declaration the tests
/// execute, so nothing here can be true of the page and false of the executable.
fn cli_examples(c: &crate::cli::CommandDoc) -> String {
    let mut s = String::new();
    // A command that only groups others has none; a command a person can run cannot have
    // none, because `cli::validate` refuses to let one exist.
    if c.examples.is_empty() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Every target has a name and every name is distinct: the manifest and the command
    /// line both address a target by it.
    #[test]
    fn every_target_is_named_once() {
        let names: Vec<&str> = Target::ALL.iter().map(|t| t.name()).collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(names.len(), unique.len(), "{names:?}");
        assert!(names.iter().all(|n| !n.is_empty()));
        assert_eq!(Target::Manifest.name(), "manifest");
        assert_eq!(Target::INDEXED.len(), Target::ALL.len() - 1);
        assert!(!Target::INDEXED.contains(&Target::Manifest));
    }

    /// The encoding is read from the suffix, and anything the generator does not encode
    /// structurally is line-oriented text — the shell tool's allow-lists are the case.
    #[test]
    fn the_suffix_names_the_encoding() {
        for (path, format) in [
            ("docs/generated/registry.json", ArtifactFormat::Json),
            ("docs/generated/registry.yaml", ArtifactFormat::Yaml),
            ("a/b.yml", ArtifactFormat::Yaml),
            ("docs/generated/cli.md", ArtifactFormat::Markdown),
            ("share/allow/rule.txt", ArtifactFormat::Text),
            ("AGENTS", ArtifactFormat::Text),
            ("weird.suffix", ArtifactFormat::Text),
        ] {
            assert_eq!(ArtifactFormat::of_path(path), format, "{path}");
        }
        assert_eq!(ArtifactFormat::Json.suffix(), "json");
        assert_eq!(ArtifactFormat::Markdown.suffix(), "md");
        assert_eq!(ArtifactFormat::Text.suffix(), "txt");
        assert_eq!(ArtifactFormat::Yaml.suffix(), "yaml");
    }

    /// A document whose value is not a mapping has nowhere to put members, so it is
    /// written as it stands rather than silently wrapped.
    #[test]
    fn a_value_that_is_not_a_mapping_carries_no_members() {
        let doc = Document::new("fixture", "x/v1", "a test", json!([1, 2, 3]));
        assert_eq!(doc.stamped("test"), json!([1, 2, 3]));
        let arts = doc.artifacts("test");
        // the YAML encoding still carries the banner, which is a comment and not a member
        assert!(arts[1].content.starts_with(&format!("# {HEADER}")));
        assert!(arts[1].content.contains("\n- 1\n"));
    }

    /// The banner says the same three things in every encoding, and the JSON one names
    /// the command that rewrites the file.
    #[test]
    fn the_banner_is_one_text_in_three_wrappings() {
        let [a, b, c] = banner_lines("nowhere", "9.9.9");
        assert_eq!(a, HEADER);
        assert!(b.starts_with("Source: nowhere;") && b.contains(REGENERATE));
        assert_eq!(c, "Generator: majordomus-cli 9.9.9");
        assert_eq!(
            markdown_banner("nowhere", "9.9.9"),
            format!("<!-- {a}\n     {b}\n     {c} -->\n")
        );
        assert_eq!(
            comment_banner("nowhere", "9.9.9"),
            format!("# {a}\n# {b}\n# {c}\n")
        );
        assert!(json_banner("nowhere").starts_with(HEADER));
        assert!(json_banner("nowhere").contains(REGENERATE));
    }

    /// A schema id nothing publishes is not a failure: the document simply has no
    /// contract, and saying so is not the same as inventing one.
    #[test]
    fn an_unpublished_schema_id_validates_nothing_and_the_set_names_what_it_holds() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("thing.schema.json"),
            r#"{"type":"object","properties":{"schema":{"const":"x/v1"}},"required":["schema"]}"#,
        )
        .unwrap();
        let schemas = GeneratedSchemas::load(dir.path()).unwrap();
        assert_eq!(schemas.ids().collect::<Vec<_>>(), ["x/v1"]);
        assert!(format!("{schemas:?}").contains("x/v1"));
        assert_eq!(
            schemas.violations("nobody/v1", &json!({})),
            Vec::<String>::new()
        );
        assert!(!schemas.violations("x/v1", &json!({})).is_empty());
        assert_eq!(
            schemas.violations("x/v1", &json!({ "schema": "x/v1" })),
            Vec::<String>::new()
        );
    }

    /// Two schemas pinning one document would make the contract of that document
    /// ambiguous, so loading refuses rather than picking one.
    #[test]
    fn two_contracts_for_one_document_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let body = r#"{"type":"object","properties":{"schema":{"const":"x/v1"}}}"#;
        std::fs::write(dir.path().join("one.schema.json"), body).unwrap();
        std::fs::write(dir.path().join("two.schema.json"), body).unwrap();
        let err = GeneratedSchemas::load(dir.path()).unwrap_err().to_string();
        assert!(err.contains("x/v1") && err.contains("both"), "{err}");
    }

    /// A violation prints as the artifact it is about, then the reason: the one line the
    /// command shows.
    #[test]
    fn a_violation_names_the_artifact_first() {
        let v = Violation {
            path: "docs/generated/x.md".into(),
            reason: "carries no generated-file banner".into(),
        };
        assert_eq!(
            v.to_string(),
            "docs/generated/x.md: carries no generated-file banner"
        );
    }

    /// The manifest of an empty plan still describes itself, in every encoding it is
    /// committed in, and none of its own three carries a hash.
    #[test]
    fn the_manifest_of_nothing_is_still_the_manifest() {
        let doc = manifest_document(&[]);
        let entries = doc["artifacts"].as_array().unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().all(|e| e["describes_itself"] == true));
        assert!(entries.iter().all(|e| e.get("sha256").is_none()));
        let arts = manifest_artifacts(&doc, "test");
        assert_eq!(
            arts.iter().map(|a| a.path.as_str()).collect::<Vec<_>>(),
            manifest_paths()
        );
        assert!(arts[2].content.contains("— (this file)"));
        // and with nothing but itself in the plan, the verifier has nothing to compare
        let dir = tempfile::tempdir().unwrap();
        let schemas = GeneratedSchemas::load(dir.path()).unwrap();
        let manifest_only = violations(&arts, &schemas);
        assert!(
            !manifest_only
                .iter()
                .any(|v| v.reason.contains("does not list")),
            "{manifest_only:?}"
        );
    }
}
