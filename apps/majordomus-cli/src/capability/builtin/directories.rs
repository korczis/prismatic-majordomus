//! The `directories` module: the layer's directory contracts as a hierarchy, and what
//! each directory owes.
//!
//! A directory of the layer says what it is for in a context document, and that document
//! composes with the ones above it. Two questions follow from that and neither was
//! answerable on the wire: what does this repository's contract hierarchy look like, and
//! which contracts actually apply to a directory once inheritance has been resolved. This
//! module answers both — the local contract a directory declares, and the effective chain
//! composed from the root down — so inheritance is inspectable rather than folklore.
//!
//! The tree is derived from the index and from nothing else. Every directory here is the
//! directory of a file the index holds, with its ancestors filled in, so the hierarchy is
//! the one the registry already knows: adding a directory with tracked content puts it in
//! this answer, in the API, in the Cockpit and in the generated reference at once, and no
//! list anywhere names a directory. That also fixes the boundary: a directory holding
//! nothing tracked is not part of the shared contract and does not appear. The gate that
//! walks the working tree and refuses a commit is `majordomus context validate`; this is
//! the projection of the same rule, not a second enforcement of it.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, Exposure, McpExposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::model::Object;
use crate::{capability, module};

use super::{get, normalise_path};

/// The URI under which `directories.list` is read as an MCP resource.
pub const DIRECTORIES_URI: &str = "majordomus://directories";

/// The kind whose objects are directory contracts.
const CONTEXT_KIND: &str = "context";

/// The layer root every governed directory sits under.
const LAYER: &str = ".ai";

// ---------------------------------------------------------------- output types

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
/// Whether a directory of the layer carries the contract it owes.
pub enum DirectoryState {
    /// It carries a context document of its own.
    Documented,
    /// It owes none, because a contract above it says so.
    Exempt,
    /// It owes one and has none.
    Owed,
}

impl DirectoryState {
    /// The word this state is reported under.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::DirectoryState;
    /// assert_eq!(DirectoryState::Owed.as_str(), "owed");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            DirectoryState::Documented => "documented",
            DirectoryState::Exempt => "exempt",
            DirectoryState::Owed => "owed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// A directory contract as the document declares it, before anything is inherited.
pub struct ContractView {
    /// The document's identity; it survives a move, the file name does not.
    pub id: String,
    /// The document's path, repository-relative.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// One line naming the directory.
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// One line saying what the directory is for.
    pub description: Option<String>,
    /// `active` or `deprecated`; a deprecated document is listed and never applied.
    pub status: String,
    /// `directory`, `subtree` or `explicit`: how far the document reaches.
    pub scope: String,
    /// `extend`, `replace` or `final`: how it composes with what is above it.
    pub composition: String,
    /// Ties within one depth are broken by this, then by path.
    pub order: i64,
    /// `*`, or the providers the document is written for.
    pub providers: Vec<String>,
    /// Who it addresses; both when it says nothing.
    pub audience: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Pathspecs whose change names this document for review.
    pub tracks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Whether the directories below owe a contract, when this document says.
    pub children_require_contract: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Subtrees below this one that owe nothing: carried, not authored here.
    pub children_exempt: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One contract in a directory's effective chain, in the order it is applied.
pub struct EffectiveEntry {
    /// The document's identity.
    pub id: String,
    /// The document's path.
    pub path: String,
    /// Depth below the layer root; less is less specific and applies first.
    pub depth: usize,
    /// The declared order, the second sort key.
    pub order: i64,
    /// `extend`, `replace` or `final`.
    pub composition: String,
    /// Why it applies to this directory, in words.
    pub reason: String,
    /// True for the directory's own document, false for one it inherits.
    pub local: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One directory of the layer, what it owes, and what applies to it.
pub struct DirectoryNode {
    /// Repository-relative, forward slashes.
    pub path: String,
    /// Segments below the layer root; the root itself is 0.
    pub depth: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The directory above, when it is inside the layer.
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The directories immediately below, sorted.
    pub children: Vec<String>,
    /// Whether it carries the contract it owes.
    pub state: DirectoryState,
    /// Whether a contract is owed here at all.
    pub requires_contract: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The contract that made the requirement explicit, when one did.
    pub governed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The contract that released this subtree, when one did.
    pub exempted_by: Option<String>,
    /// How many objects of any kind the index holds directly in this directory.
    pub objects: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The contract this directory declares, before inheritance.
    pub contract: Option<ContractView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The contracts that apply here once inheritance is resolved, least specific first.
    /// Present when the request asked for it, or when it asked about one path.
    pub effective: Vec<EffectiveEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// How many directories are in each state.
pub struct DirectoryTallies {
    /// Every directory of the layer the index knows.
    pub directories: usize,
    /// Carrying a contract of their own.
    pub documented: usize,
    /// Released by a contract above them.
    pub exempt: usize,
    /// Owing a contract and carrying none.
    pub owed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The layer's directories, what each owes, and the contracts that apply.
pub struct DirectoryReport {
    /// The layer root the tree is rooted at.
    pub root: String,
    /// The directories in each state.
    pub tallies: DirectoryTallies,
    /// Every directory asked for, sorted by path.
    pub directories: Vec<DirectoryNode>,
}

// ---------------------------------------------------------------- input

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which directories to answer for, and how much of the hierarchy to resolve.
pub struct DirectoriesInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// One directory, repository-relative and inside the layer. Its effective chain is
    /// always resolved. Absent means every directory of the layer.
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Resolve the effective chain for every directory, not only for a named one.
    pub effective: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only directories in this state: `documented`, `exempt` or `owed`.
    pub state: Option<DirectoryState>,
}

impl BenchmarkCases for DirectoriesInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("whole-tree", DirectoriesInput::default()),
            NamedCase::new(
                "one-directory",
                DirectoriesInput {
                    path: Some(".ai/repo/rules".into()),
                    ..Default::default()
                },
            ),
            NamedCase::new(
                "effective-everywhere",
                DirectoriesInput {
                    effective: Some(true),
                    ..Default::default()
                },
            ),
            NamedCase::new(
                "owed",
                DirectoriesInput {
                    state: Some(DirectoryState::Owed),
                    ..Default::default()
                },
            ),
        ]
    }
}

// ---------------------------------------------------------------- reading the metadata

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn list(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The contract a context object declares. Defaults match the contract's own: a document
/// with no audience addresses both, and `order` and the rest are required of it.
fn contract_of(o: &Object) -> ContractView {
    let m = &o.metadata;
    let audience = {
        let a = list(m, "audience");
        if a.is_empty() {
            vec!["human".to_string(), "agent".to_string()]
        } else {
            a
        }
    };
    ContractView {
        id: o.identity.clone(),
        path: o.provenance.path.clone(),
        title: o.title.clone(),
        description: o.description.clone(),
        status: text(m, "status").unwrap_or_else(|| "active".into()),
        scope: text(m, "scope").unwrap_or_else(|| "directory".into()),
        composition: text(m, "composition").unwrap_or_else(|| "extend".into()),
        order: m.get("order").and_then(Value::as_i64).unwrap_or(0),
        providers: list(m, "providers"),
        audience,
        tracks: list(m, "tracks"),
        children_require_contract: m
            .get("children")
            .and_then(|c| c.get("require_contract"))
            .and_then(Value::as_bool),
        children_exempt: m
            .get("children")
            .and_then(|c| c.get("exempt"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// Is `child` `parent` itself, or below it?
fn under(parent: &str, child: &str) -> bool {
    child == parent || child.starts_with(&format!("{parent}/"))
}

/// Segments below the layer root.
fn depth_of(dir: &str) -> usize {
    if dir == LAYER {
        0
    } else {
        dir.split('/').count().saturating_sub(1)
    }
}

// ---------------------------------------------------------------- the tree

/// One directory's declared contract, keyed by directory.
struct Contracts {
    by_dir: BTreeMap<String, ContractView>,
}

impl Contracts {
    fn build(index: &crate::index::Index) -> Self {
        let mut by_dir = BTreeMap::new();
        for o in index.objects.iter().filter(|o| o.kind == CONTEXT_KIND) {
            by_dir.insert(o.provenance.directory.clone(), contract_of(o));
        }
        Contracts { by_dir }
    }

    /// The contract nearest at or above `dir` that says what its children owe, and what it
    /// said. Absent everywhere above means a contract is owed.
    fn requirement(&self, dir: &str) -> (bool, Option<String>) {
        let mut best: Option<(usize, bool, String)> = None;
        for (d, c) in &self.by_dir {
            let Some(v) = c.children_require_contract else {
                continue;
            };
            if c.scope != "subtree" || !under(d, dir) {
                continue;
            }
            let depth = depth_of(d);
            if best.as_ref().is_none_or(|(b, _, _)| depth > *b) {
                best = Some((depth, v, c.path.clone()));
            }
        }
        match best {
            Some((_, v, path)) => (v, Some(path)),
            None => (true, None),
        }
    }

    /// The contract that released `dir`, when one names it or a subtree above it.
    fn exemption(&self, dir: &str) -> Option<String> {
        for (d, c) in &self.by_dir {
            if c.scope != "subtree" || !under(d, dir) {
                continue;
            }
            if c.children_exempt.iter().any(|e| under(e, dir)) {
                return Some(c.path.clone());
            }
        }
        None
    }

    /// The chain that applies to `dir`: every document whose scope reaches it, least
    /// specific first — depth, then declared order, then path — with a `replace`
    /// document's supersessions dropped. This is the composition the resolver applies;
    /// a deprecated document is discovered and never applied.
    fn effective(&self, dir: &str, index: &crate::index::Index) -> Vec<EffectiveEntry> {
        let mut rows: Vec<(usize, i64, String, EffectiveEntry)> = Vec::new();
        for (d, c) in &self.by_dir {
            if c.status == "deprecated" {
                continue;
            }
            let (reason, depth) = match c.scope.as_str() {
                "directory" if d == dir => ("the target directory (scope directory)", depth_of(d)),
                "subtree" if d == dir => ("the target directory (scope subtree)", depth_of(d)),
                "subtree" if under(d, dir) => ("an ancestor (scope subtree)", depth_of(d)),
                "explicit" => {
                    let Some(p) = self
                        .paths_of(index, &c.id)
                        .into_iter()
                        .find(|p| under(p, dir))
                    else {
                        continue;
                    };
                    let depth = depth_of(&p);
                    rows.push((
                        depth,
                        c.order,
                        c.path.clone(),
                        EffectiveEntry {
                            id: c.id.clone(),
                            path: c.path.clone(),
                            depth,
                            order: c.order,
                            composition: c.composition.clone(),
                            reason: "a declared path (scope explicit)".into(),
                            local: d == dir,
                        },
                    ));
                    continue;
                }
                _ => continue,
            };
            rows.push((
                depth,
                c.order,
                c.path.clone(),
                EffectiveEntry {
                    id: c.id.clone(),
                    path: c.path.clone(),
                    depth,
                    order: c.order,
                    composition: c.composition.clone(),
                    reason: reason.into(),
                    local: d == dir,
                },
            ));
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

        // a replace document stands in for what it supersedes, which leaves the chain
        let applicable: BTreeSet<String> = rows.iter().map(|r| r.3.id.clone()).collect();
        let mut superseded: BTreeSet<String> = BTreeSet::new();
        for r in &rows {
            if r.3.composition != "replace" {
                continue;
            }
            for s in self.supersedes_of(index, &r.3.id) {
                if applicable.contains(&s) {
                    superseded.insert(s);
                }
            }
        }
        rows.into_iter()
            .map(|r| r.3)
            .filter(|e| !superseded.contains(&e.id))
            .collect()
    }

    fn paths_of(&self, index: &crate::index::Index, id: &str) -> Vec<String> {
        Self::list_field(index, id, "paths")
    }

    fn supersedes_of(&self, index: &crate::index::Index, id: &str) -> Vec<String> {
        Self::list_field(index, id, "supersedes")
    }

    fn list_field(index: &crate::index::Index, id: &str, key: &str) -> Vec<String> {
        index
            .objects
            .iter()
            .find(|o| o.kind == CONTEXT_KIND && o.identity == id)
            .map(|o| list(&o.metadata, key))
            .unwrap_or_default()
    }
}

/// Every directory of the layer the index knows: the directory of each object it holds,
/// with the ancestors between it and the layer root filled in.
fn directories_of(index: &crate::index::Index) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for o in &index.objects {
        let dir = o.provenance.directory.as_str();
        if !under(LAYER, dir) {
            continue;
        }
        *counts.entry(dir.to_string()).or_insert(0) += 1;
        // every directory between this one and the root is a directory of the layer too
        let mut cur = dir;
        while let Some((parent, _)) = cur.rsplit_once('/') {
            if !under(LAYER, parent) {
                break;
            }
            counts.entry(parent.to_string()).or_insert(0);
            cur = parent;
        }
    }
    counts
}

fn directories_list(
    ctx: &Context,
    input: DirectoriesInput,
) -> Result<DirectoryReport, CapabilityError> {
    let index = ctx.index.as_ref();
    let contracts = Contracts::build(index);
    let counts = directories_of(index);

    let target = match input.path.as_deref() {
        Some(raw) => {
            let p = normalise_path(raw)?;
            if !under(LAYER, &p) {
                return Err(CapabilityError::InvalidInput(format!(
                    "'{p}' is outside {LAYER}/; the directory contracts are the layer's"
                )));
            }
            if !counts.contains_key(&p) {
                return Err(CapabilityError::NotFound(format!(
                    "no directory '{p}' in the layer; the index holds nothing under it"
                )));
            }
            Some(p)
        }
        None => None,
    };
    let want_effective = target.is_some() || input.effective.unwrap_or(false);

    let mut tallies = DirectoryTallies {
        directories: 0,
        documented: 0,
        exempt: 0,
        owed: 0,
    };
    let mut out = Vec::new();
    for (dir, objects) in &counts {
        let contract = contracts.by_dir.get(dir).cloned();
        let (requires, governed_by) = contracts.requirement(dir);
        let exempted_by = contracts.exemption(dir);
        let state = if contract.is_some() {
            DirectoryState::Documented
        } else if exempted_by.is_some() || !requires {
            DirectoryState::Exempt
        } else {
            DirectoryState::Owed
        };

        // the tallies count the whole layer, never the filtered answer
        tallies.directories += 1;
        match state {
            DirectoryState::Documented => tallies.documented += 1,
            DirectoryState::Exempt => tallies.exempt += 1,
            DirectoryState::Owed => tallies.owed += 1,
        }

        if let Some(t) = &target {
            if dir != t {
                continue;
            }
        }
        if let Some(want) = input.state {
            if want != state {
                continue;
            }
        }

        let parent = dir
            .rsplit_once('/')
            .map(|(p, _)| p.to_string())
            .filter(|p| under(LAYER, p) && counts.contains_key(p));
        let children: Vec<String> = counts
            .keys()
            .filter(|c| c.rsplit_once('/').map(|(p, _)| p) == Some(dir.as_str()))
            .cloned()
            .collect();

        out.push(DirectoryNode {
            path: dir.clone(),
            depth: depth_of(dir),
            parent,
            children,
            state,
            requires_contract: requires,
            governed_by,
            exempted_by,
            objects: *objects,
            contract,
            effective: if want_effective {
                contracts.effective(dir, index)
            } else {
                Vec::new()
            },
        });
    }

    Ok(DirectoryReport {
        root: LAYER.to_string(),
        tallies,
        directories: out,
    })
}

// ---------------------------------------------------------------- the module

/// The `directories` module: the layer's contract hierarchy, projected once.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "directories",
        title: "Directory contracts",
        description: "The layer's directories as a hierarchy: the contract each one declares, what it owes and which contract said so, and the chain that applies to it once inheritance is resolved.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "directories.list",
                title: "The layer's directory contracts",
                description: "Every directory of the layer the index knows, with the contract it declares, whether it owes one and which contract decided, and — for a named path, or when asked for everywhere — the effective chain composed from the root down, least specific first.",
                input: DirectoriesInput,
                output: DirectoryReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_directories".into()),
                        resource: Some(McpResource { uri: DIRECTORIES_URI.into(), name: "directories".into() }),
                    }),
                    http: get("/api/v1/directories"),
                    cli: None,
                },
                tags: ["directories", "context", "introspection"],
                cache: CachePolicy::Process { max_entries: 8, ttl_seconds: Some(5) },
                handler: directories_list,
            },
        ],
    }
}
