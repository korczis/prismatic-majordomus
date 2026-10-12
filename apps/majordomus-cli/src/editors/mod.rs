//! Editors and agents as projections of the layer: the files Zed and Google Antigravity
//! read from a project, derived from the canonical registries and from nothing else.
//!
//! # One core, two adapters
//!
//! [`EditorEnvironment`] is what the repository says that an editor or an agent can use —
//! its skills, from the index, and its workflows, from the tracked justfile — each with the
//! source it was read from and that source's fingerprint. An adapter ([`zed`],
//! [`antigravity`]) declares what its product reads, with the evidence for each claim, and
//! maps the environment onto the files it reads. Neither adapter discovers anything: adding
//! a skill to `.ai/repo/skills/` or a recipe to the justfile changes what both project, and
//! no list here names either.
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

/// The source line of every artifact derived from the justfile.
const JUSTFILE_SOURCE: &str = "the recipes of the tracked justfile and .just/*.just";

// ---------------------------------------------------------------- the canonical model

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
/// How far an input may change without the repository changing.
pub enum EditorInputClass {
    /// Tracked content of the repository: changes only by a commit.
    Repository,
    /// The state of this checkout: which files happen to exist here.
    Worktree,
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
            title: o.title.clone().unwrap_or_else(|| id.clone()),
            description: o.description.clone().unwrap_or_default(),
            path: o.provenance.path.clone(),
            id,
            sha256,
        });
    }
    env.skills
        .sort_by(|a, b| a.id.cmp(&b.id).then(a.path.cmp(&b.path)));

    match tracked_workflows(root) {
        Some((sources, workflows, groups)) => {
            env.workflows_resolved = Some(true);
            env.inputs.extend(sources);
            env.tasks = tasks_of(&workflows, &groups, &mut env.diagnostics);
        }
        None if root.join("justfile").is_file() => {
            env.workflows_resolved = Some(false);
            env.diagnostics.push(EditorDiagnostic {
                severity: Severity::Warning,
                code: "workflows-unreadable".into(),
                adapter: Some("zed".into()),
                path: Some("justfile".into()),
                message: "the justfile exists and `just --dump` could not describe it, so no task is projected".into(),
                remedy: "install `just` (the CI runner has it) and run `just --dump` to see why it refuses".into(),
            });
        }
        None => env.workflows_resolved = None,
    }
    env.inputs.sort_by(|a, b| a.path.cmp(&b.path));
    env.inputs.dedup_by(|a, b| a.path == b.path);
    env
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
        let mut more: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "just") && p.is_file())
            .filter_map(|p| p.strip_prefix(root).ok().map(Path::to_path_buf))
            .collect();
        more.sort();
        files.extend(more);
    }
    let scratch = Scratch::new()?;
    let mut inputs = Vec::with_capacity(files.len());
    for rel in &files {
        let text = std::fs::read_to_string(root.join(rel)).ok()?;
        let dest = scratch.path.join(rel);
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir).ok()?;
        }
        std::fs::write(&dest, &text).ok()?;
        inputs.push(EditorInput {
            path: rel.to_string_lossy().replace('\\', "/"),
            mutability: EditorInputClass::Repository,
            sha256: sha256_hex(&text),
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
    fn new() -> Option<Scratch> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("majordomus-editors-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&path).ok()?;
        Some(Scratch { path })
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
    out.sort_by(|a, b| {
        a.group_rank
            .cmp(&b.group_rank)
            .then_with(|| a.group.cmp(&b.group))
            .then_with(|| a.recipe.cmp(&b.recipe))
    });
    out
}

// ---------------------------------------------------------------- projection

/// The content address of a derivation: the projection version, the document, and the
/// fingerprints of its sources in path order. Independent of the generator's own version,
/// which the banner states and which changes no rule.
pub fn derivation_key(document: &str, sources: &[&EditorInput]) -> String {
    let mut sorted: Vec<&&EditorInput> = sources.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut text = format!("editors/v{PROJECTION_VERSION}\n{document}\n");
    for i in sorted {
        text.push_str(&format!("{}\t{}\n", i.path, i.sha256));
    }
    sha256_hex(&text)
}

/// Every artifact both adapters read, every finding, every capability — as data.
pub fn plan(index: &Index, root: &Path, version: &str) -> Plan {
    let env = environment(index, root);
    let mut diagnostics = env.diagnostics.clone();
    let mut artifacts: Vec<(Artifact, Vec<String>, Vec<String>)> = Vec::new();

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
    if let Some(a) = zed::tasks_artifact(&env, version) {
        let sources = env
            .inputs
            .iter()
            .filter(|i| i.path == "justfile" || i.path.starts_with(".just/"))
            .map(|i| i.path.clone())
            .collect();
        artifacts.push((a, vec!["zed".into()], sources));
    }
    artifacts.sort_by(|a, b| a.0.path.cmp(&b.0.path));

    diagnostics.extend(description_findings(&env.skills));
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
    let adapters = vec![antigravity::adapter(&env), zed::adapter(&env)];
    Plan {
        artifacts: artifacts.into_iter().map(|(a, _, _)| a).collect(),
        derivations,
        adapters,
        inputs: env.inputs,
        diagnostics,
    }
}

/// The report `editors.status` answers with.
pub fn report(index: &Index, root: &Path, version: &str) -> EditorProjection {
    let plan = plan(index, root, version);
    let orphans = orphans(root, &plan.artifacts);
    EditorProjection {
        schema: SCHEMA.into(),
        projection_version: PROJECTION_VERSION,
        adapters: plan.adapters,
        inputs: plan.inputs,
        artifacts: plan.derivations,
        orphans,
        diagnostics: plan.diagnostics,
    }
}

fn sort_diagnostics(d: &mut [EditorDiagnostic]) {
    let rank = |s: Severity| match s {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    };
    d.sort_by(|a, b| {
        rank(a.severity)
            .cmp(&rank(b.severity))
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.path.cmp(&b.path))
    });
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
    let path = format!("{SKILLS_DIR}/{}/SKILL.md", skill.id);
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
    let quoted = serde_json::to_string(&one_line).unwrap_or_else(|_| "\"\"".into());
    let [a, b, c] = banner_lines(&skill.path, version);
    let depth = path.matches('/').count();
    let up = "../".repeat(depth);
    let content = format!(
        "---\nname: {name}\ndescription: {quoted}\n---\n<!-- {a}\n     {b}\n     {c} -->\n\n\
         # {title}\n\n\
         This skill is defined once, in [`{src}`]({up}{src}). Read that file and follow it: it is \
         the procedure, its inputs and what it must produce. This file only makes the skill \
         discoverable to agents that load `{SKILLS_DIR}/`, and says nothing the definition does \
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
    let Ok(entries) = std::fs::read_dir(root.join(SKILLS_DIR)) else {
        return out;
    };
    for e in entries.filter_map(|e| e.ok()) {
        let rel = format!("{SKILLS_DIR}/{}/SKILL.md", e.file_name().to_string_lossy());
        if planned.contains(rel.as_str()) {
            continue;
        }
        let path = root.join(&rel);
        let is_file = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file());
        if is_file && head(&path).is_some_and(|h| banner_is_present(&h)) {
            out.push(rel);
        }
    }
    out.sort();
    out
}

/// Remove every orphan and the directory it leaves empty. Returns what was removed.
pub fn remove_orphans(root: &Path, orphans: &[String]) -> std::io::Result<Vec<String>> {
    for rel in orphans {
        let path = root.join(rel);
        std::fs::remove_file(&path)?;
        if let Some(dir) = path.parent() {
            // only when empty: anything else in it is someone's
            let _ = std::fs::remove_dir(dir);
        }
    }
    Ok(orphans.to_vec())
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
        remove_orphans(dir.path(), &found).unwrap();
        assert!(!path.exists() && !path.parent().unwrap().exists());
        assert!(mine.exists(), "a hand-written skill is never removed");
    }

    #[test]
    fn jsonc_body_is_what_follows_the_banner() {
        let text = format!("{}[1]\n", jsonc_banner("s", "0.0.0"));
        assert_eq!(jsonc_body(&text), Some("[1]\n"));
        assert_eq!(jsonc_body("[1]\n"), None);
    }
}
