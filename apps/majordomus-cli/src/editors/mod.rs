//! Editors and agents as projections of the layer: the files Zed, Google Antigravity and
//! Claude Code read from a project, derived from the canonical registries and from nothing
//! else.
//!
//! # One core, three adapters
//!
//! [`EditorEnvironment`] is what the repository says that an editor or an agent can use —
//! its skills, from the index, and its workflows, from the tracked justfile — each with the
//! source it was read from and that source's fingerprint. An adapter ([`zed`],
//! [`antigravity`]) declares what its product reads, with the evidence for each claim, and
//! maps the environment onto the files it reads. Neither adapter discovers anything: adding
//! a skill to `.ai/repo/skills/` or a recipe to the justfile changes what both project, and
//! no list here names either.
//!
//! The third adapter, [`claude`], projects something else from the same plan: the
//! capabilities of the registry that declare a skill ([`commands`]), each as the file
//! Claude Code reads to call it by name (ADR 0146).
//!
//! Where two products read the same file the projection is written once. Zed and
//! Antigravity both discover `.agents/skills/<name>/SKILL.md`, so one artifact serves both
//! and names both as its consumers.
//!
//! # What is never projected
//!
//! Personal settings, credentials, model choice, approval and permission modes, and anything
//! whose project-level format could not be verified against the installed product. Those
//! are reported as [`EditorSupport::RequiresUserAction`] or [`EditorSupport::Unknown`] with the evidence,
//! never written as a guess (ADR 0140).
//!
//! # Ownership
//!
//! Every artifact carries the generated-file banner, and [`conflicts`] refuses to overwrite a
//! file at a projected path that does not: a hand-written `.zed/tasks.json` or skill is the
//! person's, and `majordomus generate` stops rather than taking it over. A banner-carrying
//! file under a projected directory that the plan no longer contains is an [`orphans`] entry:
//! generator-owned and stale, removed by `generate` and reported by `generate --check`.
//!
//! # Determinism and cost
//!
//! The projection is a pure function of the environment, which is ordered canonically on the
//! way in, so enumeration order never reaches a byte. It is cheap enough — one index walk and
//! one `just --dump` over a copy of the tracked justfile — that no persisted cache is kept;
//! each artifact's [`EditorDerivation::key`] is the content address of its inputs, which is what
//! `editors.status` explains and what a cache would be keyed by if one ever earned its place.

pub mod antigravity;
pub mod claude;
pub mod zed;

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::environment::workflows;
use crate::environment::WorkflowDescriptor;
use crate::generate::{banner_lines, Artifact, ArtifactFormat, HEADER};
use crate::index::Index;
use crate::model::Severity;
use crate::policy::sha256_hex;

/// The schema of the projection report `editors.status` answers with.
pub const SCHEMA: &str = "majordomus/editor-projection/v1";

/// The version of the projection rules. A change to what any adapter emits for the same
/// environment bumps it, and every [`EditorDerivation::key`] with it.
pub const PROJECTION_VERSION: u32 = 1;

/// The prefix every editor artifact's document id carries; the generated-artifact contract
/// reads it to accept the banner forms these files are allowed (see [`banner_is_present`]).
pub const DOCUMENT_PREFIX: &str = "editors/";

/// Where the shared skill projection lives: read by Zed and by Antigravity alike.
pub const SKILLS_DIR: &str = ".agents/skills";

/// The kind whose objects are skills.
const SKILL_KIND: &str = "skill";

/// The finding that the justfile exists and `just` could not describe it. `generate` refuses
/// on it: a task list that is present on a machine with `just` and absent on one without
/// would make the committed projection depend on the machine, not on the tree.
pub const WORKFLOWS_UNREADABLE: &str = "workflows-unreadable";

/// The source line of every artifact derived from the justfile.
const JUSTFILE_SOURCE: &str = "the recipes of the tracked justfile and .just/*.just";

// ---------------------------------------------------------------- the canonical model

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
/// How far an input may change without the repository changing.
pub enum EditorInputClass {
    /// Tracked content of the repository: changes only by a commit. The only class an
    /// input has today: what the checkout merely holds (an instruction file, a legacy
    /// directory) changes a finding, never a projected byte.
    Repository,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One input of the projection: a file, what kind of input it is, and its content address.
pub struct EditorInput {
    /// Repository-relative path.
    pub path: String,
    /// Tracked content or checkout state.
    pub mutability: EditorInputClass,
    /// SHA-256 of its content.
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A skill as an editor sees it.
pub struct SkillRef {
    /// The canonical identity, which is also the projected directory name.
    pub id: String,
    /// The one-line title.
    pub title: String,
    /// When to use it, as the skill states it.
    pub description: String,
    /// The canonical definition, repository-relative.
    pub path: String,
    /// The content address of the definition.
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A capability as an agent's user invokes it by name: the alias and the field its typed
/// text fills, both from the capability's own declaration, with how it is reached.
pub struct CommandRef {
    /// The name typed after the slash, which is also the projected directory name.
    pub alias: String,
    /// The input field the typed text fills.
    pub argument: String,
    /// The capability's id.
    pub capability: String,
    /// The capability's own title.
    pub title: String,
    /// Its MCP tool, when it has one.
    pub tool: Option<String>,
    /// The words after `majordomus` on the command line, when it has them.
    pub cli: Option<String>,
    /// Whether running it changes anything, so that only a person may invoke it.
    pub changes_something: bool,
    /// The file that declares it, repository-relative.
    pub source: String,
    /// The content address of its descriptor.
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One capability of a domain, as its skill lists it.
pub struct DomainEntry {
    /// The capability's id.
    pub capability: String,
    /// The capability's own title.
    pub title: String,
    /// Its MCP tool; every entry of a domain has one.
    pub tool: String,
    /// The words after `majordomus` on the command line, when it has them.
    pub cli: Option<String>,
    /// Whether running it changes anything, so that only a person's request runs it.
    pub changes_something: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A module of the registry as one skill (ADR 0165): every public capability of the module
/// that has an MCP tool, reachable as `/mj-<module>` and discoverable by the model from the
/// skill's description. Derived; nothing names a domain.
pub struct DomainRef {
    /// The module the capabilities are declared in.
    pub module: String,
    /// The skill's name: `mj-` and the module, underscores as hyphens.
    pub alias: String,
    /// Its capabilities, by id.
    pub entries: Vec<DomainEntry>,
    /// The content address of every entry's descriptor, in id order.
    pub sha256: String,
}

/// The prefix every derived domain skill carries, so that the listing groups them and no
/// domain takes a name a product or a workflow skill wants.
pub const DOMAIN_PREFIX: &str = "mj-";

#[derive(Debug, Clone, PartialEq, Eq)]
/// A workflow as an editor sees it: a recipe a person may run with one keystroke.
pub struct TaskRef {
    /// What `just` is given: the recipe, prefixed by its module path when it has one.
    pub recipe: String,
    /// The group it is filed under.
    pub group: Option<String>,
    /// The group's position in the justfile's declaration order; unknown groups sort last.
    pub group_rank: usize,
}

#[derive(Debug, Clone, Default)]
/// Everything the adapters project from, with where each part came from.
pub struct EditorEnvironment {
    /// Every active skill, by identity.
    pub skills: Vec<SkillRef>,
    /// Every public, unconfirmed recipe of the tracked justfile, in declaration-group order.
    pub tasks: Vec<TaskRef>,
    /// Whether the justfile could be read; `None` when the repository has no justfile.
    pub workflows_resolved: Option<bool>,
    /// The inputs the artifacts are derived from, sorted by path.
    pub inputs: Vec<EditorInput>,
    /// Facts of the checkout that change what an editor does but not what is projected.
    pub diagnostics: Vec<EditorDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// What an adapter can do with one capability. Never a boolean: "we do not know" is an
/// answer of its own and is not "no".
pub enum EditorSupport {
    /// Projected, from a verified format.
    Supported,
    /// The product has no project-level form for it.
    Unsupported,
    /// A project-level form is claimed somewhere and could not be verified; nothing written.
    Unknown,
    /// The product reads it, from a form this projection will not write.
    Deprecated,
    /// A person can enable it, outside the repository; the remedy says how.
    RequiresUserAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One capability of one adapter.
pub struct EditorCapability {
    /// Stable identity within the adapter: `skills`, `tasks`, `instructions`, `mcp`, ...
    pub id: String,
    /// What the adapter does with it.
    pub support: EditorSupport,
    /// The files that carry it, repository-relative. Empty when nothing is projected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<String>,
    /// Where the claim about the product comes from.
    pub evidence: String,
    /// What it means here, and what to do when nothing is projected.
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One adapter: the product, the version its claims were verified against, its capabilities.
pub struct EditorAdapter {
    /// `zed` or `antigravity`.
    pub id: String,
    /// The product's name.
    pub title: String,
    /// The release the evidence was taken from.
    pub verified_against: String,
    /// Its capabilities, by id.
    pub capabilities: Vec<EditorCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One projected file and how it came to be.
pub struct EditorDerivation {
    /// Repository-relative path.
    pub path: String,
    /// The document id in the generated-artifact manifest.
    pub document: String,
    /// The adapters that read it.
    pub adapters: Vec<String>,
    /// The inputs it is derived from, by path.
    pub sources: Vec<String>,
    /// The content address of the derivation: the projection version and the fingerprints of
    /// its sources. The same key always yields the same bytes.
    pub key: String,
    /// SHA-256 of the projected content.
    pub sha256: String,
    /// The state of the file in this checkout.
    pub state: EditorFileState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// A projected file as this checkout holds it.
pub enum EditorFileState {
    /// Byte for byte what the projection says.
    Current,
    /// Generator-owned and different: `majordomus generate editors` rewrites it.
    Stale,
    /// Not there: `majordomus generate editors` writes it.
    Absent,
    /// A file without the generated banner sits at the path: it is someone's, and nothing
    /// overwrites it.
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// Something about the projection worth acting on.
pub struct EditorDiagnostic {
    /// `info`, `warning` or `error`.
    pub severity: Severity,
    /// Stable identity of the finding.
    pub code: String,
    /// The adapter it concerns, when it concerns one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// The file it concerns, when it concerns one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// What was found.
    pub message: String,
    /// What to do about it.
    pub remedy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// `editors.status`: every adapter, every projected file and its state, every finding.
pub struct EditorProjection {
    /// [`SCHEMA`].
    pub schema: String,
    /// [`PROJECTION_VERSION`].
    pub projection_version: u32,
    /// The adapters, by id.
    pub adapters: Vec<EditorAdapter>,
    /// The inputs, by path.
    pub inputs: Vec<EditorInput>,
    /// The projected files, by path.
    pub artifacts: Vec<EditorDerivation>,
    /// Generator-owned files under a projected directory that the plan no longer contains.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub orphans: Vec<String>,
    /// The findings, by severity then code then path.
    pub diagnostics: Vec<EditorDiagnostic>,
}

/// The plan: the artifacts, how each was derived, and what the adapters found.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The files `majordomus generate editors` writes.
    pub artifacts: Vec<Artifact>,
    /// One per artifact, same order.
    pub derivations: Vec<EditorDerivation>,
    /// The adapters.
    pub adapters: Vec<EditorAdapter>,
    /// The environment's inputs.
    pub inputs: Vec<EditorInput>,
    /// The findings.
    pub diagnostics: Vec<EditorDiagnostic>,
}

// ---------------------------------------------------------------- discovery

/// The environment of the repository at `root`, as the index and the tracked justfile state it.
pub fn environment(index: &Index, root: &Path) -> EditorEnvironment {
    let mut env = EditorEnvironment::default();
    for o in index.objects.iter().filter(|o| o.kind == SKILL_KIND) {
        let status = o.metadata.get("status").and_then(Value::as_str);
        if status.is_some_and(|s| s != "active") {
            continue;
        }
        let id = o
            .metadata
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or(&o.identity)
            .to_string();
        let sha256 = sha256_hex(&o.content);
        env.inputs.push(EditorInput {
            path: o.provenance.path.clone(),
            mutability: EditorInputClass::Repository,
            sha256: sha256.clone(),
        });
        env.skills.push(SkillRef {
            title: o.title.clone().unwrap_or(id.clone()),
            description: o.description.clone().unwrap_or_default(),
            path: o.provenance.path.clone(),
            id,
            sha256,
        });
    }
    crate::order::canonical(&mut env.skills);

    match tracked_workflows(root) {
        Some((sources, workflows, groups)) => {
            env.workflows_resolved = Some(true);
            env.inputs.extend(sources);
            env.tasks = tasks_of(&workflows, &groups, &mut env.diagnostics);
        }
        None if root.join("justfile").is_file() => {
            env.workflows_resolved = Some(false);
            env.diagnostics.push(EditorDiagnostic {
                severity: Severity::Error,
                code: WORKFLOWS_UNREADABLE.into(),
                adapter: Some("zed".into()),
                path: Some("justfile".into()),
                message: "the justfile exists and `just --dump` could not describe it, so the task list cannot be derived here".into(),
                remedy: "install `just` (every CI job that builds the crate has it, through .github/actions/setup-rust) and run `just --dump` to see why it refuses".into(),
            });
        }
        None => env.workflows_resolved = None,
    }
    crate::order::canonical(&mut env.inputs);
    env.inputs.dedup_by(|a, b| a.path == b.path);
    env
}

/// The crate's sources, repository-relative: where a builtin capability's module lives.
const CRATE_SRC: &str = "apps/majordomus-cli/src";

/// Every capability that declares a skill, as an agent's user will invoke it, by alias.
/// Nothing here is listed: a capability gains a command by saying `.skill(..)` in its own
/// declaration, and loses it the same way.
pub fn commands(registry: &crate::capability::CapabilityRegistry) -> Vec<CommandRef> {
    registry
        .skills()
        .filter_map(|c| {
            let skill = c.skill.as_ref()?;
            let source = match &c.provenance {
                crate::capability::Provenance::Builtin { module } => format!(
                    "{CRATE_SRC}/{}.rs",
                    module.split("::").skip(1).collect::<Vec<_>>().join("/")
                ),
                crate::capability::Provenance::Declarative { path, .. } => path.clone(),
            };
            Some(CommandRef {
                alias: skill.alias.clone(),
                argument: skill.argument.clone(),
                capability: c.id.to_string(),
                title: c.title.clone(),
                tool: c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()),
                cli: c.exposure.cli.as_ref().map(|cli| cli.path.join(" ")),
                changes_something: c.execution.effect != crate::capability::Effect::Read,
                source,
                sha256: sha256_hex(&serde_json::to_string(c).unwrap_or_default()),
            })
        })
        .collect()
}

/// Every module of the registry that offers at least one public capability with an MCP tool,
/// as one skill (ADR 0165). Nothing here is listed: a capability joins its module's skill by
/// having a tool, and the module is the one its declaration already names.
pub fn domains(registry: &crate::capability::CapabilityRegistry) -> Vec<DomainRef> {
    // module -> capability id -> (entry, digest): a capability id is unique in the registry,
    // so the inner map holds each module's entries in id order without a sort of its own
    type ByCapability = std::collections::BTreeMap<String, (DomainEntry, String)>;
    let mut by_module: std::collections::BTreeMap<String, ByCapability> =
        std::collections::BTreeMap::new();
    for c in registry.iter() {
        if c.visibility != crate::capability::Visibility::Public {
            continue;
        }
        let Some(tool) = c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()) else {
            continue;
        };
        let entry = DomainEntry {
            capability: c.id.to_string(),
            title: c.title.clone(),
            tool,
            cli: c.exposure.cli.as_ref().map(|cli| cli.path.join(" ")),
            changes_something: c.execution.effect != crate::capability::Effect::Read,
        };
        let digest = sha256_hex(&serde_json::to_string(c).unwrap_or_default());
        by_module
            .entry(c.module.as_str().to_string())
            .or_default()
            .insert(entry.capability.clone(), (entry, digest));
    }
    by_module
        .into_iter()
        .map(|(module, by_capability)| {
            let entries: Vec<(DomainEntry, String)> = by_capability.into_values().collect();
            let digests: String = entries.iter().map(|(_, d)| d.as_str()).collect();
            DomainRef {
                alias: format!("{DOMAIN_PREFIX}{}", module.replace('_', "-")),
                module,
                entries: entries.into_iter().map(|(e, _)| e).collect(),
                sha256: sha256_hex(&digests),
            }
        })
        .filter(|d| is_skill_name(&d.alias))
        .collect()
}

/// The recipes of the *tracked* justfile: `justfile` and `.just/*.just`, read through `just`
/// from a private copy. The root justfile also imports `.ai/local/cache/command-graph/
/// bridge.just`, which each checkout materialises for itself; read in place, a checkout that
/// has the bridge would project a different file than one that has not, and a committed
/// projection would be stale on every machine but the one that wrote it.
fn tracked_workflows(
    root: &Path,
) -> Option<(Vec<EditorInput>, Vec<WorkflowDescriptor>, Vec<String>)> {
    let justfile = root.join("justfile");
    if !justfile.is_file() {
        return None;
    }
    let mut files: Vec<PathBuf> = vec![PathBuf::from("justfile")];
    if let Ok(entries) = std::fs::read_dir(root.join(".just")) {
        let mut more: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "just") && p.is_file())
            .filter_map(|p| {
                p.strip_prefix(root)
                    .ok()
                    .map(|r| r.to_string_lossy().replace('\\', "/"))
            })
            .collect();
        crate::order::canonical_strings(&mut more);
        files.extend(more.into_iter().map(PathBuf::from));
    }
    // every file is read before anything is asked: one that cannot be read means the recipes
    // cannot be known, which is the same answer as a justfile `just` refuses
    let texts: Vec<(&PathBuf, String)> = files
        .iter()
        .filter_map(|rel| {
            std::fs::read_to_string(root.join(rel))
                .ok()
                .map(|text| (rel, text))
        })
        .collect();
    if texts.len() != files.len() {
        return None;
    }
    // A copy that could not be written is not handled here: `just` is then asked about an
    // incomplete justfile and refuses it, and that refusal is the one answer below.
    let scratch = Scratch::new();
    let mut inputs = Vec::with_capacity(texts.len());
    for (rel, text) in &texts {
        let _ = std::fs::write(scratch.path.join(rel), text);
        inputs.push(EditorInput {
            path: rel.to_string_lossy().replace('\\', "/"),
            mutability: EditorInputClass::Repository,
            sha256: sha256_hex(text),
        });
    }
    let catalogue = workflows::resolve(&scratch.path);
    if catalogue.state != crate::environment::TierState::Resolved {
        return None;
    }
    let groups = workflows::groups(&scratch.path).unwrap_or_default();
    Some((inputs, catalogue.workflows, groups))
}

/// A directory of the process's own under the system's temporary directory, removed when
/// dropped.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    /// The directory, with the `.just/` the copied justfile imports from. A directory that
    /// could not be made is found out by whoever writes into it.
    fn new() -> Scratch {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("majordomus-editors-{}-{seq}", std::process::id()));
        let _ = std::fs::create_dir_all(path.join(".just"));
        Scratch { path }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A recipe name `just` accepts and no shell reads as anything but a word.
fn is_plain_recipe(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':')
}

/// Every public recipe a person may run with one keystroke: not one that asks before it runs
/// (`[confirm]` is the justfile's own statement that it is dangerous), not one whose name is
/// anything but a plain word, and not one that needs an argument it has no default for.
fn tasks_of(
    workflows: &[WorkflowDescriptor],
    groups: &[String],
    diagnostics: &mut Vec<EditorDiagnostic>,
) -> Vec<TaskRef> {
    let mut out = Vec::new();
    for w in workflows {
        let recipe = match &w.namespace {
            Some(ns) if !ns.is_empty() => format!("{ns}::{}", w.name),
            _ => w.name.clone(),
        };
        if w.confirm || w.parameters.iter().any(|p| p.required) {
            continue;
        }
        if !is_plain_recipe(&recipe) {
            diagnostics.push(EditorDiagnostic {
                severity: Severity::Warning,
                code: "task-name-refused".into(),
                adapter: Some("zed".into()),
                path: Some(".zed/tasks.json".into()),
                message: format!(
                    "the recipe {recipe:?} is not a plain name and is not projected as a task"
                ),
                remedy: "name the recipe with letters, digits, `-` and `_` only".into(),
            });
            continue;
        }
        let group_rank = w
            .group
            .as_ref()
            .and_then(|g| groups.iter().position(|x| x == g))
            .unwrap_or(usize::MAX);
        out.push(TaskRef {
            recipe,
            group: w.group.clone(),
            group_rank,
        });
    }
    crate::order::canonical(&mut out);
    out
}

// ---------------------------------------------------------------- projection

/// The content address of a derivation: the projection version, the document, and the
/// fingerprints of its sources in path order. Independent of the generator's own version,
/// which the banner states and which changes no rule.
pub fn derivation_key(document: &str, sources: &[&EditorInput]) -> String {
    let mut sorted: Vec<&EditorInput> = sources.to_vec();
    crate::order::canonical(&mut sorted);
    let mut text = format!("editors/v{PROJECTION_VERSION}\n{document}\n");
    for i in sorted {
        text.push_str(&format!("{}\t{}\n", i.path, i.sha256));
    }
    sha256_hex(&text)
}

/// Every artifact both adapters read, every finding, every capability — as data.
pub fn plan(
    index: &Index,
    registry: &crate::capability::CapabilityRegistry,
    root: &Path,
    version: &str,
) -> Plan {
    let mut env = environment(index, root);
    let commands = commands(registry);
    let domains = claude::reachable_domains(domains(registry), &commands, &env.skills);
    // a command's input is its capability's descriptor, named by the capability
    for c in &commands {
        env.inputs.push(EditorInput {
            path: format!("capability:{}", c.capability),
            mutability: EditorInputClass::Repository,
            sha256: c.sha256.clone(),
        });
    }
    // a domain's input is the descriptors of its capabilities, named by the module
    for d in &domains {
        env.inputs.push(EditorInput {
            path: format!("module:{}", d.module),
            mutability: EditorInputClass::Repository,
            sha256: d.sha256.clone(),
        });
    }
    crate::order::canonical(&mut env.inputs);
    let mut diagnostics = env.diagnostics.clone();
    let mut artifacts: Vec<(Artifact, Vec<String>, Vec<String>)> = Vec::new();

    // .agents/ sorts before .claude/, and what is written under .claude/ is keyed by path

    for skill in &env.skills {
        match skill_artifact(skill, version) {
            Ok(a) => artifacts.push((
                a,
                vec!["antigravity".into(), "zed".into()],
                vec![skill.path.clone()],
            )),
            Err(d) => diagnostics.push(*d),
        }
    }
    // Claude Code discovers skills under .claude/skills/ alone: every workflow skill is
    // written there too, beside the commands, so that /<id> reaches the layer's own
    // definition. A command that has the same name keeps it (and is a finding below).
    // keyed by path, so that what is written under .claude/ comes out in path order
    let mut for_claude: std::collections::BTreeMap<String, (Artifact, Vec<String>, Vec<String>)> =
        std::collections::BTreeMap::new();
    for c in &commands {
        let a = claude::command_artifact(c, version);
        for_claude.insert(
            a.path.clone(),
            (
                a,
                vec![claude::ID.to_string()],
                vec![format!("capability:{}", c.capability)],
            ),
        );
    }
    for d in &domains {
        let a = claude::domain_artifact(d, version);
        for_claude.insert(
            a.path.clone(),
            (
                a,
                vec![claude::ID.to_string()],
                vec![format!("module:{}", d.module)],
            ),
        );
    }
    for skill in claude::workflow_skills(&commands, &env.skills) {
        // a name both products refuse was already reported by the loop above
        if let Ok(a) = skill_artifact_in(claude::SKILLS_DIR, skill, version) {
            for_claude.insert(
                a.path.clone(),
                (a, vec![claude::ID.to_string()], vec![skill.path.clone()]),
            );
        }
    }
    artifacts.extend(for_claude.into_values());
    if let Some(a) = zed::tasks_artifact(&env, version) {
        let sources = env
            .inputs
            .iter()
            .filter(|i| i.path == "justfile" || i.path.starts_with(".just/"))
            .map(|i| i.path.clone())
            .collect();
        artifacts.push((a, vec!["zed".into()], sources));
    }
    // in path order by construction: every skill (canonical by identity) under
    // .agents/, every command and workflow skill (by path) under .claude/, then the task
    // list under .zed/

    diagnostics.extend(description_findings(&env.skills));
    diagnostics.extend(claude::name_findings(&commands, &env.skills));
    diagnostics.extend(checkout_findings(root));
    sort_diagnostics(&mut diagnostics);

    let derivations = artifacts
        .iter()
        .map(|(a, adapters, sources)| {
            let inputs: Vec<&EditorInput> = env
                .inputs
                .iter()
                .filter(|i| sources.contains(&i.path))
                .collect();
            EditorDerivation {
                path: a.path.clone(),
                document: a.document.clone(),
                adapters: adapters.clone(),
                sources: sources.clone(),
                key: derivation_key(&a.document, &inputs),
                sha256: sha256_hex(&a.content),
                state: state_of(root, a),
            }
        })
        .collect();
    let adapters = vec![
        antigravity::adapter(&env),
        claude::adapter(&commands, &domains, &env.skills),
        zed::adapter(&env),
    ];
    Plan {
        artifacts: artifacts.into_iter().map(|(a, _, _)| a).collect(),
        derivations,
        adapters,
        inputs: env.inputs,
        diagnostics,
    }
}

/// The report `editors.status` answers with.
pub fn report(
    index: &Index,
    registry: &crate::capability::CapabilityRegistry,
    root: &Path,
    version: &str,
) -> EditorProjection {
    let plan = plan(index, registry, root, version);
    let orphans = orphans(root, &plan.artifacts);
    // A person's own commands and skills are read here and never in `plan`: the plan is what
    // `generate` writes, and a committed file must not depend on whose machine wrote it.
    let projected: Vec<String> = plan
        .artifacts
        .iter()
        .filter_map(|a| claude::skill_name(&a.path))
        .collect();
    let mut diagnostics = plan.diagnostics;
    diagnostics.extend(claude::shadow_findings(
        &projected,
        claude::personal_dir().as_deref(),
    ));
    sort_diagnostics(&mut diagnostics);
    EditorProjection {
        schema: SCHEMA.into(),
        projection_version: PROJECTION_VERSION,
        adapters: plan.adapters,
        inputs: plan.inputs,
        artifacts: plan.derivations,
        orphans,
        diagnostics,
    }
}

fn sort_diagnostics(d: &mut [EditorDiagnostic]) {
    crate::order::canonical(d);
}

// The canonical orders of the editor projection's collections (project.canonical-order).

impl crate::order::Ordered for SkillRef {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.id, &self.path)
    }
}

impl crate::order::Ordered for EditorInput {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.path, &self.path)
    }
}

// By the position of the recipe's group in the justfile's declaration, then by name: the
// order the justfile already shows a person. A recipe in no declared group comes last.
impl crate::order::Ordered for TaskRef {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        let rank = i64::try_from(self.group_rank).unwrap_or(i64::MAX);
        crate::order::OrderKey::plain(&self.recipe, &self.recipe).ranked(rank)
    }
}

// Errors first, then warnings, then information; inside a severity by code, then path.
impl crate::order::Ordered for EditorDiagnostic {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        let rank = match self.severity {
            Severity::Error => 0,
            Severity::Warning => 1,
            Severity::Info => 2,
        };
        crate::order::OrderKey::plain(&self.code, self.path.as_deref().unwrap_or("")).ranked(rank)
    }
}

/// A skill name both products accept: lowercase letters, digits and single hyphens, at most
/// 64 characters (Zed's limit; Antigravity documents the same shape and no length).
pub fn is_skill_name(name: &str) -> bool {
    name.len() <= 64
        && !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The descriptions Zed recommends at most; longer ones are projected and reported.
const DESCRIPTION_BUDGET: usize = 1024;

/// The shared skill projection: the front matter both products require — `name` and
/// `description` — the banner, and a pointer to the one definition. The procedure itself is
/// never copied: it lives in the canonical file, which the agent is told to read.
pub fn skill_artifact(skill: &SkillRef, version: &str) -> Result<Artifact, Box<EditorDiagnostic>> {
    skill_artifact_in(SKILLS_DIR, skill, version)
}

/// The same projection under another directory a product discovers skills in: Claude Code
/// reads `.claude/skills/` and not `.agents/skills/`, so a workflow skill is reachable there
/// by its name only when its pointer is written there too. One definition, one text; the
/// directory is the only thing that differs.
pub fn skill_artifact_in(
    dir: &str,
    skill: &SkillRef,
    version: &str,
) -> Result<Artifact, Box<EditorDiagnostic>> {
    let path = format!("{dir}/{}/SKILL.md", skill.id);
    if !is_skill_name(&skill.id) {
        return Err(Box::new(EditorDiagnostic {
            severity: Severity::Warning,
            code: "skill-name-refused".into(),
            adapter: None,
            path: Some(skill.path.clone()),
            message: format!(
                "the skill {:?} is not lowercase letters, digits and hyphens (at most 64), which both products require; it is not projected",
                skill.id
            ),
            remedy: "rename the skill's `id` to a lowercase, hyphenated name".into(),
        }));
    }
    let description = if skill.description.trim().is_empty() {
        skill.title.clone()
    } else {
        skill.description.trim().to_string()
    };
    let one_line = description.split_whitespace().collect::<Vec<_>>().join(" ");
    // a JSON string is a YAML double-quoted scalar: one line, every quote and escape safe
    let quoted = serde_json::Value::String(one_line).to_string();
    let [a, b, c] = banner_lines(&skill.path, version);
    let depth = path.matches('/').count();
    let up = "../".repeat(depth);
    let content = format!(
        "---\nname: {name}\ndescription: {quoted}\n---\n<!-- {a}\n     {b}\n     {c} -->\n\n\
         # {title}\n\n\
         This skill is defined once, in [`{src}`]({up}{src}). Read that file and follow it: it is \
         the procedure, its inputs and what it must produce. This file only makes the skill \
         discoverable to agents that load `{dir}/`, and says nothing the definition does \
         not.\n\n\
         The same definition is served as `majordomus skills explain {name}` and as the MCP \
         resource `majordomus://skill/{name}`.\n",
        name = skill.id,
        title = skill.title,
        src = skill.path,
    );
    Ok(Artifact::verbatim(
        path,
        format!("{DOCUMENT_PREFIX}skill"),
        ArtifactFormat::Markdown,
        None,
        skill.path.clone(),
        content,
    ))
}

/// Description length is a finding, not a refusal.
pub(crate) fn description_findings(skills: &[SkillRef]) -> Vec<EditorDiagnostic> {
    skills
        .iter()
        .filter(|s| s.description.len() > DESCRIPTION_BUDGET)
        .map(|s| EditorDiagnostic {
            severity: Severity::Info,
            code: "skill-description-long".into(),
            adapter: Some("zed".into()),
            path: Some(s.path.clone()),
            message: format!(
                "the description is {} bytes; Zed recommends at most {DESCRIPTION_BUDGET}",
                s.description.len()
            ),
            remedy: "shorten the skill's `description` to the sentence that says when to use it"
                .into(),
        })
        .collect()
}

/// The `//` banner a JSONC artifact opens with: Zed reads every `.zed/**/*.json` as JSON
/// with comments, and a task list is an array, which has no member to carry provenance.
pub fn jsonc_banner(source: &str, version: &str) -> String {
    banner_lines(source, version)
        .iter()
        .map(|l| format!("// {l}\n"))
        .collect()
}

/// The JSON below a JSONC banner, when the content opens with one.
pub fn jsonc_body(content: &str) -> Option<&str> {
    if !content.starts_with(&format!("// {HEADER}")) {
        return None;
    }
    let mut rest = content;
    while rest.starts_with("//") {
        rest = rest.split_once('\n').map(|(_, r)| r).unwrap_or("");
    }
    Some(rest)
}

/// Whether an editor artifact carries its banner in the form its file allows: after the
/// front matter for a skill, as `//` lines for JSONC, first for anything else.
pub fn banner_is_present(content: &str) -> bool {
    if jsonc_body(content).is_some() {
        return true;
    }
    let after_front_matter = content
        .strip_prefix("---\n")
        .and_then(|r| r.split_once("\n---\n").map(|(_, rest)| rest))
        .unwrap_or(content);
    after_front_matter.starts_with(&format!("<!-- {HEADER}"))
}

/// The first lines of a file, enough to find a banner after a front matter.
fn head(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut buf = vec![0u8; 4096];
    let mut f = std::fs::File::open(path).ok()?;
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    Some(String::from_utf8_lossy(&buf).into_owned())
}

fn state_of(root: &Path, a: &Artifact) -> EditorFileState {
    let path = root.join(&a.path);
    match std::fs::symlink_metadata(&path) {
        Err(_) => EditorFileState::Absent,
        Ok(m) if !m.file_type().is_file() => EditorFileState::Conflict,
        Ok(_) => match std::fs::read_to_string(&path) {
            Ok(on_disk) if on_disk == a.content => EditorFileState::Current,
            Ok(on_disk) if banner_is_present(&on_disk) => EditorFileState::Stale,
            _ => EditorFileState::Conflict,
        },
    }
}

/// Every projected path where a file sits that this generator does not own: one without the
/// banner, a symbolic link, a directory. `majordomus generate` refuses to write while any
/// remains, and names each with what to do.
pub fn conflicts(root: &Path, artifacts: &[Artifact]) -> Vec<String> {
    let mut out = Vec::new();
    for a in artifacts
        .iter()
        .filter(|a| a.document.starts_with(DOCUMENT_PREFIX))
    {
        if escapes(root, &a.path) {
            out.push(format!("{} (outside the repository)", a.path));
            continue;
        }
        if state_of(root, a) == EditorFileState::Conflict {
            out.push(a.path.clone());
        }
    }
    out
}

/// Whether any existing ancestor of `rel` under `root` is a symbolic link, which would carry
/// a write somewhere the repository does not decide.
fn escapes(root: &Path, rel: &str) -> bool {
    if rel.starts_with('/') || rel.split('/').any(|s| s == ".." || s.is_empty()) {
        return true;
    }
    let mut at = root.to_path_buf();
    let parts: Vec<&str> = rel.split('/').collect();
    for part in &parts[..parts.len().saturating_sub(1)] {
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(m) if m.file_type().is_symlink() => return true,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    false
}

/// Generator-owned skill projections the plan no longer contains: a skill was retired or
/// renamed and its projection remained. Only a `SKILL.md` that carries the banner is ours.
pub fn orphans(root: &Path, artifacts: &[Artifact]) -> Vec<String> {
    let planned: std::collections::BTreeSet<&str> =
        artifacts.iter().map(|a| a.path.as_str()).collect();
    let mut out = Vec::new();
    // both directories a skill is projected to; a hand-written skill beside ours carries no
    // banner and is never an orphan
    for dir in [SKILLS_DIR, claude::SKILLS_DIR] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for e in entries.filter_map(|e| e.ok()) {
            let rel = format!("{dir}/{}/SKILL.md", e.file_name().to_string_lossy());
            if planned.contains(rel.as_str()) {
                continue;
            }
            let path = root.join(&rel);
            let is_file = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file());
            if is_file && head(&path).is_some_and(|h| banner_is_present(&h)) {
                out.push(rel);
            }
        }
    }
    crate::order::canonical_strings(&mut out);
    out
}

/// Remove every orphan and the directory it leaves empty. Returns what was removed; one that
/// could not be removed is still an orphan, and the next `--check` says so.
pub fn remove_orphans(root: &Path, orphans: &[String]) -> Vec<String> {
    let mut removed = Vec::with_capacity(orphans.len());
    for rel in orphans {
        let path = root.join(rel);
        if std::fs::remove_file(&path).is_ok() {
            removed.push(rel.clone());
        }
        // the directory only when that left it empty: anything else in it is someone's
        let _ = std::fs::remove_dir(path.with_file_name(""));
    }
    removed
}

/// Facts of this checkout an adapter acts on differently. None changes a projected byte.
fn checkout_findings(root: &Path) -> Vec<EditorDiagnostic> {
    let mut out = Vec::new();
    out.extend(zed::instruction_findings(root));
    out.extend(antigravity::legacy_findings(root));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(id: &str, description: &str) -> SkillRef {
        SkillRef {
            id: id.into(),
            title: format!("The {id} skill"),
            description: description.into(),
            path: format!(".ai/repo/skills/{id}/SKILL.md"),
            sha256: sha256_hex(id),
        }
    }

    #[test]
    fn the_same_skill_is_written_where_claude_code_looks() {
        let s = skill("evolve", "Turn what was observed into what is enforced.");
        let shared = skill_artifact(&s, "0.0.0").unwrap();
        let claude = skill_artifact_in(claude::SKILLS_DIR, &s, "0.0.0").unwrap();
        assert_eq!(claude.path, ".claude/skills/evolve/SKILL.md");
        assert!(claude
            .content
            .starts_with("---\nname: evolve\ndescription: "));
        // one definition: the pointer leads to the same file from the same depth
        assert!(claude
            .content
            .contains("(../../../.ai/repo/skills/evolve/SKILL.md)"));
        assert!(claude
            .content
            .contains("agents that load `.claude/skills/`"));
        // and differs from the shared projection in the directory it names alone
        assert_eq!(
            claude.content.replace(".claude/skills/", ".agents/skills/"),
            shared.content
        );
        assert_eq!(claude.source, shared.source);
    }

    #[test]
    fn a_skill_projection_opens_with_the_front_matter_both_products_require() {
        let a = skill_artifact(&skill("implement", "Change the tree."), "0.0.0").unwrap();
        assert_eq!(a.path, ".agents/skills/implement/SKILL.md");
        assert!(a
            .content
            .starts_with("---\nname: implement\ndescription: \"Change the tree.\"\n---\n"));
        assert!(banner_is_present(&a.content));
        assert!(a
            .content
            .contains("](../../../.ai/repo/skills/implement/SKILL.md)"));
    }

    #[test]
    fn a_description_cannot_break_out_of_its_front_matter() {
        let a = skill_artifact(&skill("x", "a\n---\nname: evil\n\"quoted\""), "0.0.0").unwrap();
        let front: Vec<&str> = a.content.lines().take(4).collect();
        assert_eq!(front[0], "---");
        assert_eq!(front[1], "name: x");
        assert!(front[2].starts_with("description: \"a --- name: evil \\\"quoted\\\"\""));
        assert_eq!(front[3], "---");
    }

    #[test]
    fn a_name_neither_product_accepts_is_refused_not_projected() {
        for bad in [
            "Upper",
            "under_score",
            "-lead",
            "trail-",
            "two--hyphens",
            "",
        ] {
            assert!(skill_artifact(&skill(bad, "d"), "0.0.0").is_err(), "{bad}");
        }
        assert!(is_skill_name(&"a".repeat(64)));
        assert!(!is_skill_name(&"a".repeat(65)));
    }

    #[test]
    fn the_key_is_the_inputs_and_not_their_order() {
        let a = EditorInput {
            path: "a".into(),
            mutability: EditorInputClass::Repository,
            sha256: "1".into(),
        };
        let b = EditorInput {
            path: "b".into(),
            mutability: EditorInputClass::Repository,
            sha256: "2".into(),
        };
        assert_eq!(
            derivation_key("d", &[&a, &b]),
            derivation_key("d", &[&b, &a])
        );
        let b2 = EditorInput {
            sha256: "3".into(),
            ..b.clone()
        };
        assert_ne!(
            derivation_key("d", &[&a, &b]),
            derivation_key("d", &[&a, &b2])
        );
        assert_ne!(derivation_key("d", &[&a]), derivation_key("e", &[&a]));
    }

    #[test]
    fn tasks_follow_the_declared_groups_whatever_order_the_runner_listed_them_in() {
        let w = |name: &str, group: &str, confirm: bool| WorkflowDescriptor {
            name: name.into(),
            namespace: None,
            description: None,
            group: Some(group.into()),
            parameters: Vec::new(),
            dependencies: Vec::new(),
            confirm,
        };
        let groups = vec!["build".to_string(), "test".to_string()];
        let mut forward = vec![
            w("test", "test", false),
            w("build", "build", false),
            w("clean", "build", true),
        ];
        let mut d = Vec::new();
        let one = tasks_of(&forward, &groups, &mut d);
        forward.reverse();
        let two = tasks_of(&forward, &groups, &mut d);
        assert_eq!(one, two);
        let names: Vec<&str> = one.iter().map(|t| t.recipe.as_str()).collect();
        assert_eq!(
            names,
            ["build", "test"],
            "a [confirm] recipe is never one keystroke away"
        );
    }

    #[test]
    fn a_recipe_name_a_shell_would_read_is_refused() {
        let mut d = Vec::new();
        let w = WorkflowDescriptor {
            name: "x; rm -rf /".into(),
            namespace: None,
            description: None,
            group: None,
            parameters: Vec::new(),
            dependencies: Vec::new(),
            confirm: false,
        };
        assert!(tasks_of(&[w], &[], &mut d).is_empty());
        assert_eq!(d[0].code, "task-name-refused");
    }

    #[test]
    fn a_hand_written_file_at_a_projected_path_is_a_conflict_and_ours_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let a = skill_artifact(&skill("implement", "d"), "0.0.0").unwrap();
        assert!(
            conflicts(dir.path(), std::slice::from_ref(&a)).is_empty(),
            "absent is not a conflict"
        );
        let path = dir.path().join(&a.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "---\nname: implement\ndescription: mine\n---\nmy own words\n",
        )
        .unwrap();
        assert_eq!(
            conflicts(dir.path(), std::slice::from_ref(&a)),
            vec![a.path.clone()]
        );
        let older = skill_artifact(&skill("implement", "older"), "0.0.0").unwrap();
        std::fs::write(&path, &older.content).unwrap();
        assert!(
            conflicts(dir.path(), std::slice::from_ref(&a)).is_empty(),
            "stale and ours is rewritten"
        );
        assert_eq!(state_of(dir.path(), &a), EditorFileState::Stale);
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_on_the_way_to_a_projected_path_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), dir.path().join(".agents")).unwrap();
        let a = skill_artifact(&skill("implement", "d"), "0.0.0").unwrap();
        let found = conflicts(dir.path(), std::slice::from_ref(&a));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("outside the repository"));
    }

    #[test]
    fn only_a_banner_carrying_projection_is_an_orphan() {
        let dir = tempfile::tempdir().unwrap();
        let gone = skill_artifact(&skill("retired", "d"), "0.0.0").unwrap();
        let mine = dir.path().join(".agents/skills/mine/SKILL.md");
        std::fs::create_dir_all(mine.parent().unwrap()).unwrap();
        std::fs::write(
            &mine,
            "---\nname: mine\ndescription: d\n---\nhand written\n",
        )
        .unwrap();
        let path = dir.path().join(&gone.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &gone.content).unwrap();
        let found = orphans(dir.path(), &[]);
        assert_eq!(found, vec![gone.path.clone()]);
        assert_eq!(remove_orphans(dir.path(), &found), found);
        // one that is not there to remove is not reported as removed
        assert!(remove_orphans(dir.path(), &found).is_empty());
        assert!(!path.exists() && !path.parent().unwrap().exists());
        assert!(mine.exists(), "a hand-written skill is never removed");
    }

    #[test]
    fn jsonc_body_is_what_follows_the_banner() {
        let text = format!("{}[1]\n", jsonc_banner("s", "0.0.0"));
        assert_eq!(jsonc_body(&text), Some("[1]\n"));
        assert_eq!(jsonc_body("[1]\n"), None);
    }

    // ---- the projection over a real repository, in process

    const SKILL_SOURCE: &str = "  - id: skill\n    kind: skill\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/skills/*/SKILL.md'\n    required: false\n";

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn skill_md(id: &str, description: &str, status: &str) -> String {
        format!(
            "---\nschema: skill/v1\nid: {id}\nversion: 1\ntitle: The {id} skill\n\
             description: {description}\nstatus: {status}\ntags: [t]\n---\n# Purpose\n\nDo it.\n\n\
             # Procedure\n\n1. Do.\n\n# Output\n\nDone.\n"
        )
    }

    fn has_just() -> bool {
        std::process::Command::new("just")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    /// A synthetic repository with skills of every shape and, when asked, a justfile.
    fn repository(justfile: bool) -> crate::synthetic::SyntheticRepository {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let root = repo.root().to_path_buf();
        let sources =
            std::fs::read_to_string(root.join(".ai/repo/knowledge/sources.yaml")).unwrap();
        write(
            &root,
            ".ai/repo/knowledge/sources.yaml",
            &format!("{sources}{SKILL_SOURCE}"),
        );
        write(
            &root,
            ".ai/repo/skills/triage/SKILL.md",
            &skill_md("triage", "Sort a report.", "active"),
        );
        write(
            &root,
            ".ai/repo/skills/long/SKILL.md",
            &skill_md("long", &"word ".repeat(260), "active"),
        );
        write(
            &root,
            ".ai/repo/skills/gone/SKILL.md",
            &skill_md("gone", "Retired.", "deprecated"),
        );
        // a name the layer accepts and neither product does: longer than 64 characters
        let long_name = "n".repeat(70);
        write(
            &root,
            &format!(".ai/repo/skills/{long_name}/SKILL.md"),
            &skill_md(&long_name, "Named at length.", "active"),
        );
        if justfile {
            write(
                &root,
                "justfile",
                "import '.just/more.just'\n\n# Build.\n[group('build')]\nbuild:\n    @echo b\n\n\
                 [group('build')]\n[confirm]\nclean:\n    @echo c\n\n[group('test')]\ntest:\n    @echo t\n\n\
                 loose:\n    @echo l\n",
            );
            write(
                &root,
                ".just/more.just",
                "[group('test')]\nlint:\n    @echo l\n",
            );
        }
        repo
    }

    #[test]
    fn a_repository_projects_its_active_skills_and_its_runnable_recipes() {
        let repo = repository(true);
        let index = repo.index().unwrap();
        let plan = plan(
            &index,
            &repo.context().unwrap().registry,
            repo.root(),
            "0.0.0",
        );
        let paths: Vec<&str> = plan.artifacts.iter().map(|a| a.path.as_str()).collect();
        assert!(
            paths.contains(&".agents/skills/triage/SKILL.md"),
            "{paths:?}"
        );
        assert!(paths.contains(&".agents/skills/long/SKILL.md"));
        assert!(
            !paths.iter().any(|p| p.contains("/gone/")),
            "a retired skill is not projected"
        );
        assert!(!paths.iter().any(|p| p.contains("nnnnnnnn")));
        assert!(plan
            .diagnostics
            .iter()
            .any(|d| d.code == "skill-name-refused"));
        assert!(plan
            .diagnostics
            .iter()
            .any(|d| d.code == "skill-description-long"));
        if has_just() {
            let tasks = plan
                .artifacts
                .iter()
                .find(|a| a.path == zed::TASKS)
                .expect("tasks");
            let body: serde_json::Value =
                serde_json::from_str(jsonc_body(&tasks.content).unwrap()).unwrap();
            let labels: Vec<&str> = body
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t["label"].as_str().unwrap())
                .collect();
            assert_eq!(
                labels,
                ["just build", "just lint", "just test", "just loose"]
            );
            assert!(plan.inputs.iter().any(|i| i.path == ".just/more.just"));
        } else {
            assert!(plan
                .diagnostics
                .iter()
                .any(|d| d.code == WORKFLOWS_UNREADABLE));
        }
        // the derivation keys are the inputs: the same repository, the same keys
        let again = super::plan(
            &index,
            &repo.context().unwrap().registry,
            repo.root(),
            "0.0.0",
        );
        assert_eq!(plan.derivations, again.derivations);
    }

    #[test]
    fn the_report_follows_each_file_through_absent_current_stale_and_conflict() {
        let repo = repository(false);
        let root = repo.root().to_path_buf();
        let index = repo.index().unwrap();
        let first = report(&index, &repo.context().unwrap().registry, &root, "0.0.0");
        assert_eq!(first.schema, SCHEMA);
        let state = |r: &EditorProjection, path: &str| {
            r.artifacts.iter().find(|a| a.path == path).map(|a| a.state)
        };
        let triage = ".agents/skills/triage/SKILL.md";
        assert_eq!(state(&first, triage), Some(EditorFileState::Absent));

        let planned = plan(&index, &repo.context().unwrap().registry, &root, "0.0.0").artifacts;
        crate::generate::write(&root, &planned).unwrap();
        assert_eq!(
            state(
                &report(&index, &repo.context().unwrap().registry, &root, "0.0.0"),
                triage
            ),
            Some(EditorFileState::Current)
        );

        let older = plan(&index, &repo.context().unwrap().registry, &root, "older").artifacts;
        crate::generate::write(&root, &older).unwrap();
        assert_eq!(
            state(
                &report(&index, &repo.context().unwrap().registry, &root, "0.0.0"),
                triage
            ),
            Some(EditorFileState::Stale)
        );

        std::fs::remove_file(root.join(triage)).unwrap();
        std::fs::create_dir_all(root.join(triage)).unwrap();
        assert_eq!(
            state(
                &report(&index, &repo.context().unwrap().registry, &root, "0.0.0"),
                triage
            ),
            Some(EditorFileState::Conflict)
        );
        assert_eq!(conflicts(&root, &planned), vec![triage.to_string()]);

        // a projection left behind by a retired skill is named, and only it
        write(
            &root,
            ".agents/skills/gone/SKILL.md",
            &skill_artifact(&skill("gone", "d"), "0.0.0")
                .unwrap()
                .content,
        );
        assert_eq!(
            report(&index, &repo.context().unwrap().registry, &root, "0.0.0").orphans,
            vec![".agents/skills/gone/SKILL.md"]
        );
    }

    #[test]
    fn a_path_that_climbs_out_or_is_absolute_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(escapes(dir.path(), "/etc/passwd"));
        assert!(escapes(dir.path(), "../outside"));
        assert!(escapes(dir.path(), "a//b"));
        assert!(!escapes(dir.path(), ".agents/skills/x/SKILL.md"));
        let mut a = skill_artifact(&skill("x", "d"), "0.0.0").unwrap();
        a.path = "../x/SKILL.md".into();
        assert_eq!(
            conflicts(dir.path(), &[a]),
            vec!["../x/SKILL.md (outside the repository)"]
        );
    }

    #[test]
    fn a_head_of_something_unreadable_is_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(head(&dir.path().join("absent")).is_none());
        assert!(head(dir.path()).is_none(), "a directory has no head");
    }

    #[test]
    fn a_skill_with_no_description_is_described_by_its_title() {
        let mut s = skill("plain", "");
        s.title = "Plain".into();
        let a = skill_artifact(&s, "0.0.0").unwrap();
        assert!(
            a.content.contains("description: \"Plain\""),
            "{}",
            a.content
        );
    }

    #[test]
    fn findings_order_errors_first_then_by_code_then_path() {
        let d = |severity, code: &str, path: Option<&str>| EditorDiagnostic {
            severity,
            code: code.into(),
            adapter: None,
            path: path.map(Into::into),
            message: String::new(),
            remedy: String::new(),
        };
        let mut all = vec![
            d(Severity::Info, "a", None),
            d(Severity::Warning, "b", Some("y")),
            d(Severity::Warning, "b", Some("x")),
            d(Severity::Error, "z", None),
        ];
        sort_diagnostics(&mut all);
        let order: Vec<(&str, Option<&str>)> = all
            .iter()
            .map(|d| (d.code.as_str(), d.path.as_deref()))
            .collect();
        assert_eq!(
            order,
            [("z", None), ("b", Some("x")), ("b", Some("y")), ("a", None)]
        );
    }

    #[test]
    fn a_justfile_just_refuses_is_an_error_finding_and_projects_no_task() {
        let repo = repository(false);
        write(repo.root(), "justfile", "this is { not a justfile\n");
        let index = repo.index().unwrap();
        let plan = plan(
            &index,
            &repo.context().unwrap().registry,
            repo.root(),
            "0.0.0",
        );
        let found = plan
            .diagnostics
            .iter()
            .find(|d| d.code == WORKFLOWS_UNREADABLE)
            .expect("the unreadable justfile is a finding");
        assert_eq!(found.severity, Severity::Error);
        assert!(!plan.artifacts.iter().any(|a| a.path == zed::TASKS));
    }

    #[cfg(unix)]
    #[test]
    fn a_recipe_file_that_cannot_be_read_means_the_recipes_are_not_known() {
        use std::os::unix::fs::PermissionsExt;
        let repo = repository(true);
        let locked = repo.root().join(".just/more.just");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        // root reads anything; where that is so there is nothing to observe
        if std::fs::read_to_string(&locked).is_err() {
            assert!(tracked_workflows(repo.root()).is_none());
        }
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[test]
    fn a_recipe_of_a_module_is_run_by_its_module_path_and_one_that_needs_an_argument_is_not() {
        let w = |name: &str, namespace: Option<&str>, required: bool| WorkflowDescriptor {
            name: name.into(),
            namespace: namespace.map(Into::into),
            description: None,
            group: None,
            parameters: vec![crate::environment::WorkflowParameter {
                name: "arg".into(),
                variadic: false,
                required,
            }],
            dependencies: Vec::new(),
            confirm: false,
        };
        let mut d = Vec::new();
        let found = tasks_of(
            &[
                w("build", Some("site"), false),
                w("deploy", None, true),
                w("top", Some(""), false),
            ],
            &[],
            &mut d,
        );
        let recipes: Vec<&str> = found.iter().map(|t| t.recipe.as_str()).collect();
        assert_eq!(recipes, ["site::build", "top"]);
        assert!(d.is_empty());
    }
}
