//! Cross-machine continuity: a handover written on one machine is resumed on another.
//!
//! The machine is where work runs, not what owns it. A handover written on a laptop
//! (`majordomus handover`) is *published* into a git ref of this repository
//! ([`store::REF`]), *synced* through an ordinary git remote, *planned* against the
//! receiving checkout — is it this repository, is its author trusted, does the local source
//! hold what it was written against, does it continue the work this checkout is on or
//! diverge from it — and only then *resumed*: written into the receiving checkout's
//! handovers and decision log, where every reader of the lifecycle finds it as it would
//! any handover of the same branch.
//!
//! The pieces, each in its module:
//!
//! - [`record`] — the published handover, its id, signature and the checks that refuse a
//!   secret, a machine path, a forged or foreign record;
//! - [`lineage`] — which record continues which, and what two records or two stores are to
//!   each other, decided by parent links and never by a clock;
//! - [`store`] — the git ref, written with plumbing so that no branch, index or working
//!   tree moves;
//! - [`local`] — what this checkout contributes to a publication and receives on a resume;
//! - this module — the five operations, as typed values every surface renders the same.
//!
//! What does not travel is decided by type rather than by care: a [`record::Record`] has no
//! field for an absolute path, a process, a port or a socket, and the one field that can
//! hold free text — the handover body — is refused when it carries a credential or a path
//! of a developer's disk. Machine-local state (the worktree path, the episode's open
//! record, the repository id the shell compares, the working context) is recomputed on the
//! receiving machine, and the plan lists it as recomputed.

pub mod lineage;
pub mod local;
pub mod record;
pub mod store;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mesh::config::MeshConfig;
use crate::mesh::identity::NodeIdentity;
use crate::model::Diagnostic;
use lineage::{Graph, LineState, LineSync, Relation};
use local::{LocalState, Offer, Position, SyncNote, Via};
use record::{Device, Record, Refusal, SignedRecord, TaskRef, WorkingTree};

/// An age past which a record published from an open episode no longer suggests the
/// episode is still running.
pub const ACTIVE_WINDOW_MINUTES: i64 = 60;

/// Everything the operations need that is not a value of the store: the checkout, this
/// device, this repository's identity, and the mesh declaration whose trust list decides
/// whose records are resumed.
pub struct Machine<'a> {
    /// The checkout's root.
    pub root: &'a Path,
    /// This device's key.
    pub identity: NodeIdentity,
    /// This repository's identity (32 hex).
    pub repository: String,
    /// The mesh declaration, when the repository has one.
    pub mesh: Option<MeshConfig>,
    /// False when this device has no identity yet and a read stood in a throwaway one: a
    /// read never creates a key, the first publication does.
    pub identity_known: bool,
}

/// The repository identity a mesh declaration states outright, when the repository has a
/// declaration and it states one; otherwise the identity is computed from the clone.
fn declared_repository(mesh: Option<&MeshConfig>) -> Option<String> {
    mesh.and_then(|m| m.cooperation.repository.clone())
}

impl<'a> Machine<'a> {
    /// The machine for `root`: the device key at the mesh's identity path (created on first
    /// use, as the mesh creates it), the repository identity the mesh computes, and the
    /// declaration.
    pub fn open(root: &'a Path, mesh: Option<MeshConfig>) -> Result<Self, String> {
        Self::opened(root, mesh, true)
    }

    /// The machine for a read: the device key when this device has one, and a throwaway
    /// stand-in when it does not, so that asking a question never writes a key file.
    pub fn open_read(root: &'a Path, mesh: Option<MeshConfig>) -> Result<Self, String> {
        Self::opened(root, mesh, false)
    }

    fn opened(root: &'a Path, mesh: Option<MeshConfig>, create: bool) -> Result<Self, String> {
        let path = crate::mesh::default_identity_path()
            .ok_or("no HOME and no XDG_STATE_HOME: nowhere to keep this device's identity")?;
        let identity_known = create || path.is_file();
        let identity = if identity_known {
            NodeIdentity::load_or_create(&path)
        } else {
            NodeIdentity::ephemeral()
        }
        .map_err(|e| e.to_string())?;
        let declared = declared_repository(mesh.as_ref());
        let repository = crate::mesh::repository::resolve(root, declared.as_deref())
            .map_err(|e| e.to_string())?
            .id;
        Ok(Machine {
            root,
            identity,
            repository,
            mesh,
            identity_known,
        })
    }

    /// This device.
    pub fn device(&self) -> Device {
        Device {
            node: self.identity.public.node_id.as_str().to_string(),
            label: self.identity.public.display_name.clone(),
        }
    }

    /// Whether `public_key` is trusted to be resumed from.
    pub fn trust(&self, public_key: &str) -> Trust {
        if public_key == self.identity.public.public_key {
            return Trust::ThisDevice;
        }
        let Some(mesh) = &self.mesh else {
            return Trust::Undeclared;
        };
        let state = crate::mesh::trust::evaluate(
            &mesh.trust.policy,
            &mesh.trust.allow,
            public_key,
            None,
            None,
        );
        if state.is_trusted() {
            Trust::Trusted
        } else {
            Trust::Untrusted
        }
    }
}

/// Whether the device that signed a record is one this repository trusts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Trust {
    /// This device signed it.
    ThisDevice,
    /// The mesh declaration's trust list admits the key.
    Trusted,
    /// The mesh declaration exists and does not admit the key: never resumed from.
    Untrusted,
    /// The repository declares no mesh, so no list exists to check the key against. The
    /// signature still binds the record to its key; the plan warns.
    Undeclared,
}

/// One record as every surface shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecordView {
    /// The record id.
    pub id: String,
    /// Its line.
    pub line: String,
    /// The record it continues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The device that published it.
    pub device: Device,
    /// Whether that device is this one.
    pub this_device: bool,
    /// Whether its signer is trusted here.
    pub trust: Trust,
    /// The episode it was published from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The branch it was written on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The commit it was written at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// Clean or dirty at the origin.
    pub working_tree: WorkingTree,
    /// How many paths differed from HEAD at the origin.
    pub changed_total: usize,
    /// When it was published, by the publisher's clock.
    pub published_at: String,
    /// Minutes since then, by this machine's clock; absent when either clock's reading is
    /// unusable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_minutes: Option<i64>,
    /// The task's title: the intent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The handover's `# Objective`, first line.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub objective: String,
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn now_rfc3339() -> String {
    crate::peers::rfc3339(std::time::SystemTime::now())
}

/// One `# <name>` section of a handover body, trimmed.
pub fn section(body: &str, name: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in body.lines() {
        if let Some(h) = line.strip_prefix("# ") {
            inside = h.trim().eq_ignore_ascii_case(name);
            continue;
        }
        if inside {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

impl RecordView {
    fn of(machine: &Machine<'_>, r: &SignedRecord) -> Self {
        let rec = &r.record;
        let age = crate::capability::builtin::continuity::epoch_seconds(&rec.published_at)
            .map(|t| (now_seconds() - t) / 60)
            .filter(|m| *m >= 0);
        RecordView {
            id: r.id.clone(),
            line: rec.line_of(&r.id),
            parent: rec.parent.clone(),
            device: rec.device.clone(),
            this_device: rec.device.node == machine.identity.public.node_id.as_str(),
            trust: machine.trust(&r.public_key),
            session: rec.session.clone(),
            branch: rec.source.branch.clone(),
            head: rec.source.head.clone(),
            working_tree: rec.source.working_tree,
            changed_total: rec.source.changed_total,
            published_at: rec.published_at.clone(),
            age_minutes: age,
            task: rec.task.as_ref().map(|t| t.title.clone()),
            issue: rec.handover.issue.clone(),
            objective: section(&rec.handover.body, "Objective")
                .lines()
                .next()
                .unwrap_or("")
                .to_string(),
        }
    }
}

/// A store read and checked: the admitted records as a graph, and a diagnostic for every
/// file that was refused.
pub struct Loaded {
    /// The admitted records.
    pub graph: Graph,
    /// Why each refused file was refused.
    pub refused: Vec<Diagnostic>,
    /// How many refused files declared a schema newer than this executable reads.
    pub too_new: usize,
}

/// Read the store at `reference` and admit what passes.
pub fn load(machine: &Machine<'_>, reference: &str) -> Result<Loaded, String> {
    let files = store::read(machine.root, reference)?;
    let mut records = Vec::new();
    let mut refused = Vec::new();
    let mut too_new = 0usize;
    for (path, bytes) in files {
        let well_named = path
            .strip_prefix("records/")
            .and_then(|n| n.strip_suffix(".json"))
            .is_some_and(|id| id.len() == 32 && id.chars().all(|c| c.is_ascii_hexdigit()));
        if !well_named {
            refused.push(Diagnostic::warning(
                "continuity.stray_file",
                Some(path.clone()),
                "not a record of the store; it is ignored",
            ));
            continue;
        }
        match record::admit(&path, &bytes, &machine.repository) {
            Ok(r) => records.push(r),
            Err((why, d)) => {
                if why == Refusal::TooNew {
                    too_new += 1;
                }
                refused.push(d);
            }
        }
    }
    Ok(Loaded {
        graph: Graph::new(records),
        refused,
        too_new,
    })
}

/// The key a branch is recorded under.
fn branch_key(branch: Option<&str>) -> String {
    branch.unwrap_or(local::DETACHED).to_string()
}

/// The heads other devices published that this checkout has neither resumed nor continued:
/// what it could resume. Newest first by the publisher's clock — for presentation only.
pub fn offers(machine: &Machine<'_>, graph: &Graph, state: &LocalState) -> Vec<SignedRecord> {
    // keyed newest first by the publisher's clock, then by id: a total order with no sort
    // site of its own (the clock orders the list a person reads, never the lineage)
    let mut ordered: BTreeMap<(std::cmp::Reverse<String>, String), SignedRecord> = BTreeMap::new();
    for r in graph
        .heads()
        .into_iter()
        .filter_map(|h| graph.records.get(&h).cloned())
        .filter(|r| r.record.device.node != machine.identity.public.node_id.as_str())
        .filter(|r| {
            !state
                .branches
                .values()
                .any(|p| graph.descends(&p.record, &r.id))
        })
    {
        ordered.insert(
            (
                std::cmp::Reverse(r.record.published_at.clone()),
                r.id.clone(),
            ),
            r,
        );
    }
    ordered.into_values().collect()
}

fn offer_of(r: &SignedRecord) -> Offer {
    Offer {
        record: r.id.clone(),
        device: r.record.device.label.clone(),
        branch: r.record.source.branch.clone(),
        published_at: r.record.published_at.clone(),
        task: r.record.task.as_ref().map(|t| t.title.clone()),
        issue: r.record.handover.issue.clone(),
    }
}

// ------------------------------------------------------------------- status

/// How the local store stands towards the remote's, from the refs alone — no network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    /// No remote is configured: the store is local, and that is a valid answer.
    NoRemote,
    /// Never synced with the remote.
    NeverSynced,
    /// Both hold the same records.
    InSync,
    /// This store holds records the remote has not received: a sync publishes them.
    Pending,
    /// The last fetch brought records not merged yet.
    Behind,
    /// Both of the above.
    Diverged,
    /// The store is empty and nothing was ever fetched.
    Empty,
}

/// The two stores, as refs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StoreView {
    /// The local ref.
    pub reference: String,
    /// Its commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip: Option<String>,
    /// The remote `sync` uses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    /// The commit last fetched from or pushed to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_tip: Option<String>,
    /// How they stand.
    pub sync: SyncState,
    /// Records admitted.
    pub records: usize,
    /// Files refused.
    pub refused: usize,
}

fn store_view(machine: &Machine<'_>, remote: Option<&str>, loaded: &Loaded) -> StoreView {
    let tip = store::tip(machine.root, store::REF);
    let remote_tip = remote.and_then(|r| store::tip(machine.root, &store::remote_ref(r)));
    let sync = match (remote, &tip, &remote_tip) {
        (None, _, _) => SyncState::NoRemote,
        (Some(_), None, None) => SyncState::Empty,
        (Some(_), Some(_), None) => SyncState::NeverSynced,
        (Some(_), None, Some(_)) => SyncState::Behind,
        (Some(_), Some(l), Some(r)) if l == r => SyncState::InSync,
        (Some(_), Some(l), Some(r)) => {
            let remote_in_local = store::is_ancestor(machine.root, r, l);
            let local_in_remote = store::is_ancestor(machine.root, l, r);
            match (remote_in_local, local_in_remote) {
                (true, _) => SyncState::Pending,
                (_, true) => SyncState::Behind,
                _ => SyncState::Diverged,
            }
        }
    };
    StoreView {
        reference: store::REF.into(),
        tip,
        remote: remote.map(str::to_string),
        remote_tip,
        sync,
        records: loaded.graph.records.len(),
        refused: loaded.refused.len(),
    }
}

/// A line of work as `status` shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LineView {
    /// The line id.
    pub id: String,
    /// Linear or diverged.
    pub state: LineState,
    /// How many records it holds.
    pub records: usize,
    /// Its heads.
    pub heads: Vec<RecordView>,
}

/// The answer of `continuity status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Status {
    /// This repository's identity (32 hex), the same on every clone.
    pub repository: String,
    /// This device.
    pub device: Device,
    /// The branch this checkout is on, or `DETACHED`.
    pub branch: String,
    /// The commit it is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// Clean or dirty.
    pub working_tree: WorkingTree,
    /// The record this checkout continues on its branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    /// The stores.
    pub store: StoreView,
    /// The last sync, as this checkout recorded it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<SyncNote>,
    /// Every line, by id.
    pub lines: Vec<LineView>,
    /// What other devices published that this checkout could resume, newest first.
    pub resumable: Vec<RecordView>,
    /// Refused files, broken lineage, diverged lines.
    pub diagnostics: Vec<Diagnostic>,
    /// The command to run next, when there is an obvious one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next: Vec<String>,
}

/// `continuity status`: read the local refs and the local file; no network.
pub fn status(machine: &Machine<'_>) -> Result<Status, String> {
    let loaded = load(machine, store::REF)?;
    let state = local::load(machine.root)?;
    let source = local::source_state(machine.root);
    let branch = branch_key(source.branch.as_deref());
    let remote = state
        .last_sync
        .as_ref()
        .map(|s| s.remote.clone())
        .or_else(|| store::default_remote(machine.root));
    let store_view = store_view(machine, remote.as_deref(), &loaded);
    let resumable: Vec<RecordView> = offers(machine, &loaded.graph, &state)
        .iter()
        .map(|r| RecordView::of(machine, r))
        .collect();
    let lines = loaded
        .graph
        .lines()
        .into_values()
        .map(|l| LineView {
            heads: l
                .heads
                .iter()
                .filter_map(|h| loaded.graph.records.get(h))
                .map(|r| RecordView::of(machine, r))
                .collect(),
            id: l.id,
            state: l.state,
            records: l.records,
        })
        .collect();
    let mut diagnostics = loaded.refused.clone();
    diagnostics.extend(loaded.graph.findings());
    if !machine.identity_known {
        diagnostics.push(Diagnostic::info(
            "continuity.no_device_identity",
            None,
            "this device has no identity yet; the first publication (or the mesh) creates it",
        ));
    }
    if loaded.too_new > 0 {
        diagnostics.push(Diagnostic::warning(
            "continuity.upgrade_required",
            None,
            format!(
                "{} record(s) were written by a newer majordomus; upgrade to read them",
                loaded.too_new
            ),
        ));
    }
    let mut next = Vec::new();
    match store_view.sync {
        SyncState::Pending | SyncState::NeverSynced | SyncState::Diverged | SyncState::Behind => {
            next.push("majordomus-cli continuity sync".to_string())
        }
        _ => {}
    }
    if !resumable.is_empty() {
        next.push("majordomus-cli continuity plan".to_string());
    }
    Ok(Status {
        repository: machine.repository.clone(),
        device: machine.device(),
        branch: branch.clone(),
        head: source.head,
        working_tree: source.working_tree,
        position: state.branches.get(&branch).cloned(),
        store: store_view,
        last_sync: state.last_sync,
        lines,
        resumable,
        diagnostics,
        next,
    })
}

// ------------------------------------------------------------------- publish

/// The input of a publication.
#[derive(Debug, Clone, Default)]
pub struct PublishRequest {
    /// A handover record's file name under `.ai/local/state/handovers/`; the newest when
    /// absent.
    pub handover: Option<String>,
    /// The issue the work belongs to.
    pub issue: Option<String>,
    /// The milestone.
    pub milestone: Option<String>,
}

/// The answer of `continuity publish`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Published {
    /// The record.
    pub record: RecordView,
    /// False when the same handover at the same source state was already the record this
    /// checkout stands on: nothing new was written.
    pub written: bool,
    /// The local ref's commit.
    pub tip: String,
    /// The handover record it was read from, relative to the checkout.
    pub handover: String,
    /// What is next: the record reaches another machine only through a sync.
    pub next: Vec<String>,
}

/// The handover record a publication reads: `name` under the handovers directory, or the
/// newest. Called before anything else, so that a publication with nothing to publish
/// refuses before it creates a device key.
pub fn locate_handover(root: &Path, name: Option<&str>) -> Result<std::path::PathBuf, String> {
    let dir = crate::mesh::handover::directory(root);
    match name {
        Some(name) => {
            if name.contains('/') || name.contains("..") || !name.ends_with(".md") {
                return Err(format!(
                    "`{name}` is not the file name of a handover record under {}",
                    local::relative(root, &dir)
                ));
            }
            let path = dir.join(name);
            if !path.is_file() {
                return Err(format!(
                    "no handover record {name} under {}",
                    local::relative(root, &dir)
                ));
            }
            Ok(path)
        }
        None => crate::mesh::handover::latest(root).map_err(|_| {
            format!(
                "no handover record under {}: write one with `majordomus handover` (body on \
                 stdin with # Objective, # Current State, # Next Action), then publish it",
                local::relative(root, &dir)
            )
        }),
    }
}

/// `continuity publish`: project the newest handover into a signed record, refuse it if it
/// carries anything that must not leave this machine, and add it to the local store.
/// Offline by design: it touches no remote.
pub fn publish(machine: &Machine<'_>, request: &PublishRequest) -> Result<Published, String> {
    let root = machine.root;
    let path = locate_handover(root, request.handover.as_deref())?;
    let (front, _) = crate::mesh::handover::read(&path)?;
    if let Some(origin) = front
        .get("continuity_device")
        .or_else(|| front.get("mesh_origin"))
    {
        return Err(format!(
            "the handover {} arrived from another machine ({origin}); publishing it again \
             would republish that machine's words as this one's. Write this machine's own \
             with `majordomus handover`",
            local::relative(root, &path)
        ));
    }
    let source = local::source_state(root);
    let branch = branch_key(source.branch.as_deref());
    let mut state = local::load(root)?;
    let loaded = load(machine, store::REF)?;
    let position = state
        .branches
        .get(&branch)
        .filter(|p| loaded.graph.records.contains_key(&p.record))
        .cloned();
    // the issue and milestone a line was published under carry along it until a
    // publication names others
    let parent = position
        .as_ref()
        .and_then(|p| loaded.graph.records.get(&p.record));
    let inherited = |pick: fn(&crate::mesh::journal::HandoverBody) -> Option<String>| {
        parent.and_then(|p| pick(&p.record.handover))
    };
    let handover = crate::mesh::handover::to_body(
        &path,
        request
            .issue
            .clone()
            .or_else(|| inherited(|h| h.issue.clone())),
        request
            .milestone
            .clone()
            .or_else(|| inherited(|h| h.milestone.clone())),
    )
    .map_err(|e| e.replace(&root.display().to_string(), "<repo>"))?;

    // the same handover at the same source, already stood on: nothing new to say
    if let Some(existing) = parent {
        let same_device = existing.record.device.node == machine.identity.public.node_id.as_str();
        if same_device
            && existing.record.handover.id == handover.id
            && existing.record.source == source
        {
            return Ok(Published {
                record: RecordView::of(machine, existing),
                written: false,
                tip: store::tip(root, store::REF).unwrap_or_default(),
                handover: local::relative(root, &path),
                next: vec!["majordomus-cli continuity sync".into()],
            });
        }
    }

    let task: Option<TaskRef> = handover
        .task
        .as_deref()
        .and_then(|id| local::task(root, id));
    let decisions = handover
        .task
        .as_deref()
        .map(|id| local::decisions(root, id))
        .unwrap_or_default();
    let (session, episode) = local::episode_at_publish(root);
    let record = Record {
        schema: record::SCHEMA.into(),
        repository: machine.repository.clone(),
        device: machine.device(),
        session,
        episode,
        line: position.as_ref().map(|p| p.line.clone()),
        parent: position.as_ref().map(|p| p.record.clone()),
        published_at: now_rfc3339(),
        producer: crate::VERSION.into(),
        source,
        task,
        decisions,
        handover,
    };
    record::shape(&record)?;
    let leaks = record::portability(&record, &local::secret_environment());
    if !leaks.is_empty() {
        let list: Vec<String> = leaks
            .iter()
            .map(|l| format!("{} {}: {}", l.code, l.field, l.reason))
            .collect();
        return Err(format!(
            "the handover is not published, because it would carry what must not leave this \
             machine:\n  {}\nFix: rewrite the handover (or the task title) without it and \
             publish again",
            list.join("\n  ")
        ));
    }
    let signed = SignedRecord::sign(record, &machine.identity);
    let tip = store::add(
        root,
        &[(signed.id.clone(), signed.to_bytes())],
        &format!(
            "majordomus continuity: {} from {}\n",
            &signed.id[..16],
            signed.record.device.label
        ),
    )?;
    let line = signed.record.line_of(&signed.id);
    state.branches.insert(
        branch,
        Position {
            line,
            record: signed.id.clone(),
            via: Via::Published,
            at: now_rfc3339(),
        },
    );
    let graph = Graph::new(loaded.graph.records.into_values().chain([signed.clone()]));
    state.offers = offers(machine, &graph, &state)
        .iter()
        .map(offer_of)
        .collect();
    local::save(root, &state)?;
    Ok(Published {
        record: RecordView::of(machine, &signed),
        written: true,
        tip,
        handover: local::relative(root, &path),
        next: vec!["majordomus-cli continuity sync".into()],
    })
}

// ------------------------------------------------------------------- sync

/// One line's comparison in a sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LineSyncView {
    /// The line.
    pub line: String,
    /// How the local store and the remote's stood before the merge.
    pub relation: LineSync,
    /// The line's heads after the merge.
    pub heads: Vec<String>,
}

/// What a sync did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncAction {
    /// Nothing to move: both held the same records.
    None,
    /// Records were fetched.
    Fetched,
    /// Records were published.
    Published,
    /// Both.
    Exchanged,
    /// The remote could not be reached; nothing local changed, and what is pending stays
    /// pending.
    Unreachable,
    /// No remote is configured.
    NoRemote,
}

/// The answer of `continuity sync`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Synced {
    /// The remote, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    /// What happened.
    pub action: SyncAction,
    /// Records the local store gained.
    pub fetched: usize,
    /// Records the remote gained.
    pub published: usize,
    /// Every line either side holds, compared.
    pub lines: Vec<LineSyncView>,
    /// The store after the sync.
    pub store: StoreView,
    /// What other devices published that this checkout could resume.
    pub resumable: Vec<RecordView>,
    /// Refused files, collisions, diverged lines, an unreachable remote.
    pub diagnostics: Vec<Diagnostic>,
}

/// `continuity sync`: fetch the remote's store, merge it into the local one, publish the
/// result. The one operation that uses the network, and only when asked.
pub fn sync(machine: &Machine<'_>, remote: Option<&str>) -> Result<Synced, String> {
    let root = machine.root;
    let mut state = local::load(root)?;
    let remote = match remote {
        Some(r) => Some(r.to_string()),
        None => store::default_remote(root),
    };
    let before = load(machine, store::REF)?;
    let Some(remote) = remote else {
        let view = store_view(machine, None, &before);
        return Ok(Synced {
            remote: None,
            action: SyncAction::NoRemote,
            fetched: 0,
            published: 0,
            lines: Vec::new(),
            store: view,
            resumable: offers(machine, &before.graph, &state)
                .iter()
                .map(|r| RecordView::of(machine, r))
                .collect(),
            diagnostics: vec![Diagnostic::info(
                "continuity.no_remote",
                None,
                "this repository has no remote: records stay in this clone until one is added",
            )],
        });
    };
    if !store::valid_remote(&remote) {
        return Err(format!("`{remote}` is not a remote name"));
    }
    let mut diagnostics = Vec::new();
    let unreachable = |state: &mut LocalState, detail: String, before: &Loaded| {
        state.last_sync = Some(SyncNote {
            remote: remote.clone(),
            at: now_rfc3339(),
            outcome: "unreachable".into(),
            detail: Some(detail.clone()),
        });
        local::save(root, state)?;
        let view = store_view(machine, Some(&remote), before);
        Ok::<Synced, String>(Synced {
            remote: Some(remote.clone()),
            action: SyncAction::Unreachable,
            fetched: 0,
            published: 0,
            lines: Vec::new(),
            store: view,
            resumable: offers(machine, &before.graph, state)
                .iter()
                .map(|r| RecordView::of(machine, r))
                .collect(),
            diagnostics: vec![Diagnostic::warning(
                "continuity.remote_unreachable",
                None,
                format!(
                    "{remote} could not be reached ({detail}); the local store is unchanged and \
                     its records stay pending until the next sync"
                ),
            )],
        })
    };

    let fetched = match store::fetch(root, &remote) {
        Ok(f) => f,
        Err(e) => return unreachable(&mut state, e, &before),
    };
    let remote_loaded = match &fetched {
        store::Fetched::Store(_) => load(machine, &store::remote_ref(&remote))?,
        store::Fetched::Empty => Loaded {
            graph: Graph::default(),
            refused: Vec::new(),
            too_new: 0,
        },
    };
    diagnostics.extend(remote_loaded.refused.iter().cloned());
    // what admission refused for a reason no later executable would reverse — a forged or
    // foreign record — stays out; a record too new to read here is kept, so that an older
    // executable never drops a newer one's records from the store it pushes back
    let excluded: BTreeSet<String> = remote_loaded
        .refused
        .iter()
        .filter(|d| {
            matches!(
                d.code.as_str(),
                "continuity.bad_signature"
                    | "continuity.id_mismatch"
                    | "continuity.foreign_repository"
                    | "continuity.secret"
                    | "continuity.nonportable_field"
                    | "continuity.record_too_large"
            )
        })
        .filter_map(|d| d.path.clone())
        .collect();
    let keep = |path: &str| !excluded.contains(path);
    if let store::Fetched::Store(commit) = &fetched {
        let (_, collisions) = store::merge(root, commit, &keep)?;
        for c in collisions {
            diagnostics.push(Diagnostic::error(
                "continuity.collision",
                Some(c.path),
                "the remote holds different bytes under this record's name; the local copy is \
                 kept. A record is named by its content, so one side was altered",
            ));
        }
    }
    let local_ids: BTreeSet<String> = before.graph.records.keys().cloned().collect();
    let remote_ids: BTreeSet<String> = remote_loaded.graph.records.keys().cloned().collect();
    let fetched_n = remote_ids.difference(&local_ids).count();
    let published_n = local_ids.difference(&remote_ids).count();

    // publish what the remote lacks; a remote that moved since the fetch refuses the push,
    // so fetch and merge once more and try again
    let mut pushed = true;
    if published_n > 0 {
        if let Err(first) = store::push(root, &remote) {
            pushed = match store::fetch(root, &remote) {
                // whatever stops the second attempt, the records stay pending
                Ok(store::Fetched::Store(commit)) => {
                    store::merge(root, &commit, &keep).is_ok() && store::push(root, &remote).is_ok()
                }
                _ => false,
            };
            if !pushed {
                diagnostics.push(Diagnostic::warning(
                    "continuity.push_refused",
                    None,
                    format!(
                        "{remote} did not accept the store ({first}); the records stay pending"
                    ),
                ));
            }
        }
    }
    // otherwise nothing to publish: the remote holds everything the local store does, and
    // equal content under different commits leaves the refs as git left them

    let after = load(machine, store::REF)?;
    let mut lines_ids: BTreeSet<String> = before.graph.lines().into_keys().collect();
    lines_ids.extend(remote_loaded.graph.lines().into_keys());
    let after_lines = after.graph.lines();
    let lines = lines_ids
        .into_iter()
        .map(|line| LineSyncView {
            relation: lineage::compare(&before.graph, &remote_loaded.graph, &line),
            heads: after_lines
                .get(&line)
                .map(|l| l.heads.clone())
                .unwrap_or_default(),
            line,
        })
        .collect();
    diagnostics.extend(after.graph.findings());
    let action = match (fetched_n > 0, published_n > 0 && pushed) {
        (true, true) => SyncAction::Exchanged,
        (true, false) => SyncAction::Fetched,
        (false, true) => SyncAction::Published,
        (false, false) => SyncAction::None,
    };
    let resumable_records = offers(machine, &after.graph, &state);
    state.offers = resumable_records.iter().map(offer_of).collect();
    state.last_sync = Some(SyncNote {
        remote: remote.clone(),
        at: now_rfc3339(),
        outcome: if pushed {
            "ok".into()
        } else {
            "partial".into()
        },
        detail: None,
    });
    local::save(root, &state)?;
    Ok(Synced {
        remote: Some(remote.clone()),
        action,
        fetched: fetched_n,
        published: if pushed { published_n } else { 0 },
        lines,
        store: store_view(machine, Some(&remote), &after),
        resumable: resumable_records
            .iter()
            .map(|r| RecordView::of(machine, r))
            .collect(),
        diagnostics,
    })
}

// ------------------------------------------------------------------- plan

/// The verdict of a resume plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Resume restores everything; nothing needs attention.
    Ready,
    /// Resume restores everything; the warnings say what is not what it was at the origin.
    ReadyWithWarnings,
    /// The local source does not hold what the record was written against: update git
    /// first, as the actions say.
    RequiresSourceUpdate,
    /// The same work was continued twice, or the source histories diverged: a person
    /// chooses.
    Conflict,
    /// More than one record could be meant: name one with `--record`.
    ChooseRecord,
    /// Nothing to resume.
    NothingToResume,
    /// The record is not one this checkout may resume from (an untrusted signer).
    Refused,
}

impl PlanStatus {
    /// Whether a resume proceeds on this plan.
    pub fn resumable(self) -> bool {
        matches!(self, PlanStatus::Ready | PlanStatus::ReadyWithWarnings)
    }
}

/// How the local source stands towards the record's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceRelation {
    /// Same branch, same commit.
    Exact,
    /// Same branch; the local branch has commits the record did not see.
    LocalAhead,
    /// Same branch; the record's commit is ahead of the local one.
    LocalBehind,
    /// Same branch; neither commit contains the other.
    Diverged,
    /// The record's commit is not in this clone.
    HeadMissing,
    /// This checkout is on another branch.
    BranchDiffers,
    /// The record carries no commit, or git could not answer.
    Unknown,
}

/// The source half of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceCheck {
    /// The branch the record was written on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_branch: Option<String>,
    /// The commit it was written at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_head: Option<String>,
    /// This checkout's branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_branch: Option<String>,
    /// This checkout's commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_head: Option<String>,
    /// How they stand.
    pub relation: SourceRelation,
    /// Whether the origin's tree was dirty.
    pub origin_dirty: bool,
    /// The paths that were uncommitted at the origin, as listed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub origin_changed: Vec<String>,
    /// How many were uncommitted.
    pub origin_changed_total: usize,
    /// Whether this checkout's tree is dirty.
    pub local_dirty: bool,
    /// Whether this checkout holds the very uncommitted work the origin held (their
    /// fingerprints agree); absent when either side is clean.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_uncommitted_work: Option<bool>,
}

/// One thing a plan says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanItem {
    /// A stable code: `source_incomplete`, `line_diverged`, `origin_untrusted`, …
    pub code: String,
    /// What it means here.
    pub message: String,
}

fn item(code: &str, message: impl Into<String>) -> PlanItem {
    PlanItem {
        code: code.into(),
        message: message.into(),
    }
}

/// What a resume writes, recomputes or leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Component {
    /// The handover, into this checkout's handovers.
    Handover,
    /// The task's title and scope, offered as the `start` to run.
    Intent,
    /// The issue and milestone.
    Issue,
    /// The decisions the task recorded, into this checkout's decision log.
    Decisions,
    /// The handover's next action.
    NextAction,
    /// The record this checkout continues: the parent of its next publication.
    Lineage,
    /// Who published it, where, when, from which source state.
    Provenance,
    /// This checkout's path, recomputed rather than copied.
    Worktree,
    /// The repository id the shell compares, recomputed.
    RepositoryId,
    /// The episode, opened here by the provider's hooks.
    Episode,
    /// The working context, compiled here from the restored records and this git.
    WorkingContext,
    /// The source state, from this checkout's git.
    SourceState,
}

/// The answer of `continuity plan`, and the plan `continuity resume` acts on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResumePlan {
    /// The verdict.
    pub status: PlanStatus,
    /// The record it is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<RecordView>,
    /// When the record is not decided: the records that could be meant.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<RecordView>,
    /// The source check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceCheck>,
    /// What the record is to the record this checkout stands on, when it stands on one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage: Option<Relation>,
    /// The task, as published: the intent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskRef>,
    /// The milestone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    /// The headings of the decisions a resume carries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<String>,
    /// The handover's `# Objective`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub objective: String,
    /// The handover's `# Current State`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub current_state: String,
    /// The handover's `# Next Action`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub next_action: String,
    /// What stops a resume.
    pub blockers: Vec<PlanItem>,
    /// What a resume proceeds past and a person should know.
    pub warnings: Vec<PlanItem>,
    /// The commands that resolve the blockers or follow the resume. Recommended, never run:
    /// nothing a record says is executed.
    pub actions: Vec<String>,
    /// What a resume writes into this checkout.
    pub restores: Vec<Component>,
    /// What this checkout computes for itself instead of copying.
    pub recomputes: Vec<Component>,
}

impl ResumePlan {
    fn empty(status: PlanStatus) -> Self {
        ResumePlan {
            status,
            record: None,
            candidates: Vec::new(),
            source: None,
            lineage: None,
            task: None,
            milestone: None,
            decisions: Vec::new(),
            objective: String::new(),
            current_state: String::new(),
            next_action: String::new(),
            blockers: Vec::new(),
            warnings: Vec::new(),
            actions: Vec::new(),
            restores: Vec::new(),
            recomputes: Vec::new(),
        }
    }
}

fn source_check(machine: &Machine<'_>, r: &Record) -> SourceCheck {
    let root = machine.root;
    let local = local::source_state(root);
    let relation = match (&r.source.head, &local.head) {
        (None, _) | (_, None) => SourceRelation::Unknown,
        (Some(expected), Some(here)) => {
            if !crate::git::resolves(root, &format!("{expected}^{{commit}}")) {
                SourceRelation::HeadMissing
            } else if r.source.branch.is_some() && r.source.branch != local.branch {
                SourceRelation::BranchDiffers
            } else if expected == here {
                SourceRelation::Exact
            } else if store::is_ancestor(root, expected, here) {
                SourceRelation::LocalAhead
            } else if store::is_ancestor(root, here, expected) {
                SourceRelation::LocalBehind
            } else {
                SourceRelation::Diverged
            }
        }
    };
    let origin_dirty = r.source.working_tree == WorkingTree::Dirty;
    let local_dirty = local.working_tree == WorkingTree::Dirty;
    let same = match (&r.source.fingerprint, &local.fingerprint) {
        (Some(a), Some(b)) if origin_dirty && local_dirty => Some(a == b),
        _ => None,
    };
    SourceCheck {
        expected_branch: r.source.branch.clone(),
        expected_head: r.source.head.clone(),
        local_branch: local.branch,
        local_head: local.head,
        relation,
        origin_dirty,
        origin_changed: r.source.changed.clone(),
        origin_changed_total: r.source.changed_total,
        local_dirty,
        same_uncommitted_work: same,
    }
}

fn short(id: &str) -> &str {
    &id[..12.min(id.len())]
}

/// `continuity plan`: decide, without writing anything, whether and how `record` (or the
/// one this checkout would resume) can be resumed here.
pub fn plan(machine: &Machine<'_>, record: Option<&str>) -> Result<ResumePlan, String> {
    let loaded = load(machine, store::REF)?;
    let state = local::load(machine.root)?;
    Ok(plan_with(machine, &loaded, &state, record).0)
}

/// The plan, and the record it chose when it chose one — so that a resume acts on the very
/// record the plan judged, not on one looked up again by its id.
fn plan_with(
    machine: &Machine<'_>,
    loaded: &Loaded,
    state: &LocalState,
    record: Option<&str>,
) -> (ResumePlan, Option<SignedRecord>) {
    let graph = &loaded.graph;
    let here = local::source_state(machine.root);
    let branch = branch_key(here.branch.as_deref());
    let explicit = record.is_some();

    let chosen: SignedRecord = match record {
        Some(prefix) => {
            let matches: Vec<&SignedRecord> = graph
                .records
                .values()
                .filter(|r| r.id.starts_with(prefix))
                .collect();
            match matches.as_slice() {
                [one] => (*one).clone(),
                [] => {
                    let mut p = ResumePlan::empty(PlanStatus::NothingToResume);
                    let refused = loaded
                        .refused
                        .iter()
                        .find(|d| d.path.as_deref().is_some_and(|p| p.contains(prefix)));
                    p.blockers.push(match refused {
                        Some(d) => item(&d.code, d.message.clone()),
                        None => item(
                            "record_unknown",
                            format!("no admitted record begins with `{prefix}`; sync first"),
                        ),
                    });
                    p.actions.push("majordomus-cli continuity sync".into());
                    return (p, None);
                }
                many => {
                    let mut p = ResumePlan::empty(PlanStatus::ChooseRecord);
                    p.candidates = many.iter().map(|r| RecordView::of(machine, r)).collect();
                    return (p, None);
                }
            }
        }
        None => {
            let all = offers(machine, graph, state);
            let on_branch: Vec<&SignedRecord> = all
                .iter()
                .filter(|r| branch_key(r.record.source.branch.as_deref()) == branch)
                .collect();
            let pick: Vec<&SignedRecord> = if on_branch.is_empty() {
                all.iter().collect()
            } else {
                on_branch
            };
            match pick.as_slice() {
                [] => {
                    let mut p = ResumePlan::empty(PlanStatus::NothingToResume);
                    if let Some(sync) = &state.last_sync {
                        p.warnings.push(item(
                            "nothing_published",
                            format!(
                                "no other device has published a handover this checkout has not \
                                 already resumed (last sync with {} at {})",
                                sync.remote, sync.at
                            ),
                        ));
                    } else {
                        p.actions.push("majordomus-cli continuity sync".into());
                    }
                    return (p, None);
                }
                [one] => (*one).clone(),
                many => {
                    let mut p = ResumePlan::empty(PlanStatus::ChooseRecord);
                    p.candidates = many.iter().map(|r| RecordView::of(machine, r)).collect();
                    p.blockers.push(item(
                        "several_resumable",
                        format!(
                            "{} handovers could be meant; they are independent lines of work, \
                             not a conflict. Name one with --record <id>",
                            many.len()
                        ),
                    ));
                    return (p, None);
                }
            }
        }
    };

    let r = &chosen.record;
    let view = RecordView::of(machine, &chosen);
    let mut p = ResumePlan::empty(PlanStatus::Ready);
    p.task = r.task.clone();
    p.milestone = r.handover.milestone.clone();
    p.decisions = r.decisions.iter().map(|d| d.title.clone()).collect();
    p.objective = section(&r.handover.body, "Objective");
    p.current_state = section(&r.handover.body, "Current State");
    p.next_action = section(&r.handover.body, "Next Action");
    p.restores = vec![
        Component::Handover,
        Component::Intent,
        Component::Issue,
        Component::Decisions,
        Component::NextAction,
        Component::Lineage,
        Component::Provenance,
    ];
    p.recomputes = vec![
        Component::Worktree,
        Component::RepositoryId,
        Component::Episode,
        Component::WorkingContext,
        Component::SourceState,
    ];
    let mut conflict = false;
    let mut needs_source = false;
    let mut refused = false;

    // --- who wrote it
    match view.trust {
        Trust::Untrusted => {
            refused = true;
            p.blockers.push(item(
                "origin_untrusted",
                format!(
                    "{} ({}) is not on the trust list of the mesh declaration; its handover is \
                     never resumed here. Add its key to trust.allow in .ai/repo/mesh/ if it is \
                     yours",
                    r.device.label,
                    short(&r.device.node)
                ),
            ));
        }
        Trust::Undeclared => p.warnings.push(item(
            "origin_undeclared",
            "the repository declares no mesh trust list, so the signing key is checked for \
             integrity only, not against a list of this repository's devices",
        )),
        Trust::ThisDevice | Trust::Trusted => {}
    }

    // --- the lineage
    let line = r.line_of(&chosen.id);
    let lines = graph.lines();
    if let Some(l) = lines.get(&line).filter(|l| l.state == LineState::Diverged) {
        let others: Vec<&str> = l
            .heads
            .iter()
            .filter(|h| **h != chosen.id)
            .map(|h| short(h))
            .collect();
        if explicit {
            p.warnings.push(item(
                "line_diverged",
                format!(
                    "this line was continued in more than one place; resuming {} continues \
                     it and leaves {} as heads nobody continues",
                    short(&chosen.id),
                    others.join(", ")
                ),
            ));
        } else {
            conflict = true;
            p.blockers.push(item(
                "line_diverged",
                format!(
                    "the same work was continued twice: {} and {} both continue one record. \
                     Choose one with --record <id>; nothing picks a winner by time",
                    short(&chosen.id),
                    others.join(", ")
                ),
            ));
        }
    }
    if let Some(parent) = &r.parent {
        if !graph.records.contains_key(parent) {
            p.warnings.push(item(
                "lineage_incomplete",
                format!(
                    "it continues {}, which this store does not hold",
                    short(parent)
                ),
            ));
        }
    }
    if let Some(position) = state.branches.get(&branch) {
        if graph.records.contains_key(&position.record) {
            let relation = graph.relation(&position.record, &chosen.id);
            p.lineage = Some(relation);
            match relation {
                Relation::Equal | Relation::Older => p.warnings.push(item(
                    "already_resumed",
                    format!(
                        "this checkout already stands on {}{}; resuming again rewrites nothing",
                        short(&position.record),
                        if relation == Relation::Older {
                            ", which continues this record"
                        } else {
                            ""
                        }
                    ),
                )),
                Relation::Diverged if !explicit => {
                    conflict = true;
                    p.blockers.push(item(
                        "would_diverge",
                        format!(
                            "this checkout continued the same work as {} ({}), and the record \
                             continues it elsewhere. Resume it with --record to switch to it",
                            short(&position.record),
                            match position.via {
                                Via::Published => "published here",
                                Via::Resumed => "resumed here",
                            }
                        ),
                    ));
                }
                Relation::Diverged => p.warnings.push(item(
                    "would_diverge",
                    format!(
                        "this checkout's own continuation {} stays a head nobody continues",
                        short(&position.record)
                    ),
                )),
                Relation::Newer | Relation::Independent => {}
            }
        }
    }

    // --- is the other device still at it
    if r.episode == record::EpisodeAtPublish::Open && !view.this_device {
        if let Some(age) = view.age_minutes.filter(|a| *a < ACTIVE_WINDOW_MINUTES) {
            p.warnings.push(item(
                "origin_may_be_active",
                format!(
                    "{} published this {age} min ago from an episode that was still open; the \
                     session there may still be working. This is advisory: nothing locks it",
                    r.device.label
                ),
            ));
        }
    }

    // --- the source
    let source = source_check(machine, r);
    let remote = state
        .last_sync
        .as_ref()
        .map(|s| s.remote.clone())
        .or_else(|| store::default_remote(machine.root))
        .unwrap_or_else(|| "origin".into());
    let expected_branch = source.expected_branch.clone().unwrap_or_default();
    let expected_head = source.expected_head.clone().unwrap_or_default();
    match source.relation {
        SourceRelation::Exact => {}
        SourceRelation::LocalAhead => p.warnings.push(item(
            "local_ahead",
            format!(
                "{expected_branch} here has commits after {}; the handover did not see them",
                short(&expected_head)
            ),
        )),
        SourceRelation::LocalBehind => {
            needs_source = true;
            p.blockers.push(item(
                "local_behind",
                format!(
                    "the handover was written at {}, ahead of this checkout",
                    short(&expected_head)
                ),
            ));
            p.actions
                .push(format!("git pull --ff-only {remote} {expected_branch}"));
        }
        SourceRelation::Diverged => {
            conflict = true;
            p.blockers.push(item(
                "source_diverged",
                format!(
                    "{expected_branch} here and the handover's {} have diverged; reconcile the \
                     branch with git first — nothing here rewrites history",
                    short(&expected_head)
                ),
            ));
        }
        SourceRelation::HeadMissing => {
            needs_source = true;
            p.blockers.push(item(
                "head_missing",
                format!(
                    "the handover was written at {}, which this clone does not have: the source \
                     was not pushed, or not fetched",
                    short(&expected_head)
                ),
            ));
            p.actions.push(format!("git fetch {remote}"));
        }
        SourceRelation::BranchDiffers => {
            needs_source = true;
            p.blockers.push(item(
                "branch_differs",
                format!(
                    "the handover is about {expected_branch}; this checkout is on {}",
                    source.local_branch.as_deref().unwrap_or(local::DETACHED)
                ),
            ));
            p.actions.push(format!("git switch {expected_branch}"));
        }
        SourceRelation::Unknown => p.warnings.push(item(
            "source_unknown",
            "the record or this checkout names no commit, so the source cannot be compared",
        )),
    }
    if source.origin_dirty {
        if source.same_uncommitted_work == Some(true) {
            // the uncommitted work travelled by other means and is here exactly
        } else {
            p.warnings.push(item(
                "source_incomplete",
                format!(
                    "the handover was written in a dirty tree: {} file(s) were uncommitted at \
                     the origin and are not in {}. The context is restored; that source is not, \
                     and nothing here invents it — commit and push it there, or redo it here",
                    source.origin_changed_total,
                    short(&expected_head)
                ),
            ));
        }
    }
    if source.local_dirty && source.same_uncommitted_work != Some(true) {
        p.warnings.push(item(
            "local_dirty",
            "this checkout has uncommitted changes of its own",
        ));
    }
    p.source = Some(source);

    if let Some(t) = &r.task {
        let scope = if t.scope.is_empty() {
            String::new()
        } else {
            format!(" --scope {}", t.scope.join(","))
        };
        p.actions.push(format!(
            "majordomus start {}{scope}{}",
            shell_quote(&t.title),
            t.profile
                .as_ref()
                .map(|pr| format!(" --profile {pr}"))
                .unwrap_or_default()
        ));
    }
    p.record = Some(view);
    p.status = if refused {
        PlanStatus::Refused
    } else if conflict {
        PlanStatus::Conflict
    } else if needs_source {
        PlanStatus::RequiresSourceUpdate
    } else if p.warnings.is_empty() {
        PlanStatus::Ready
    } else {
        PlanStatus::ReadyWithWarnings
    };
    if p.status.resumable() {
        p.actions.insert(
            0,
            format!(
                "majordomus-cli continuity resume --record {}",
                short(&chosen.id)
            ),
        );
    }
    (p, Some(chosen))
}

/// A string as one single-quoted shell word, so that a recommended command shows the title
/// exactly and cannot be read as more than one argument.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ------------------------------------------------------------------- resume

/// The answer of `continuity resume`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Resumed {
    /// Whether anything was restored. False when the plan did not admit a resume.
    pub resumed: bool,
    /// The plan it acted on, or declined on.
    pub plan: ResumePlan,
    /// The handover record written into this checkout, relative to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handover: Option<String>,
    /// How many decisions were appended to this checkout's log.
    pub decisions_carried: usize,
    /// The record this checkout now stands on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
}

/// `continuity resume`: plan, and when the plan admits it, write the handover and the
/// decisions into this checkout and stand on the record. Nothing in the record is run.
pub fn resume(machine: &Machine<'_>, record: Option<&str>) -> Result<Resumed, String> {
    let root = machine.root;
    let loaded = load(machine, store::REF)?;
    let mut state = local::load(root)?;
    let (plan, chosen) = plan_with(machine, &loaded, &state, record);
    let Some(chosen) = chosen.filter(|_| plan.status.resumable()) else {
        return Ok(Resumed {
            resumed: false,
            plan,
            handover: None,
            decisions_carried: 0,
            position: None,
        });
    };
    let r = &chosen.record;
    let written = crate::mesh::handover::write_record(
        root,
        &r.handover,
        &crate::mesh::handover::Provenance {
            transport: "continuity",
            marker_key: "continuity_record",
            marker: chosen.id.clone(),
            origin_key: "continuity_device",
            origin: r.device.node.clone(),
            working_tree: Some(match r.source.working_tree {
                WorkingTree::Clean => "clean",
                WorkingTree::Dirty => "dirty",
            }),
            changed_files: r.source.changed.clone(),
        },
    )?;
    let carried = local::carry_decisions(root, &r.decisions, &chosen.id, &r.device.label)?;
    let mut plan = plan;
    plan.actions
        .retain(|a| !a.starts_with("majordomus-cli continuity resume"));
    let branch = branch_key(local::source_state(root).branch.as_deref());
    let position = Position {
        line: r.line_of(&chosen.id),
        record: chosen.id.clone(),
        via: Via::Resumed,
        at: now_rfc3339(),
    };
    state.branches.insert(branch, position.clone());
    state.offers = offers(machine, &loaded.graph, &state)
        .iter()
        .map(offer_of)
        .collect();
    local::save(root, &state)?;
    Ok(Resumed {
        resumed: true,
        plan,
        handover: Some(local::relative(root, &written)),
        decisions_carried: carried,
        position: Some(position),
    })
}

// ------------------------------------------------------------------- device

/// The answer of `continuity device`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeviceView {
    /// The node id and label.
    pub device: Device,
    /// The Ed25519 public key (hex): what a trust list names.
    pub public_key: String,
}

/// `continuity device`: this device's identity — the mesh node key, created when absent —
/// optionally given a label. The key never changes here; only the label a person reads.
pub fn device(label: Option<&str>) -> Result<DeviceView, String> {
    let path = crate::mesh::default_identity_path()
        .ok_or("no HOME and no XDG_STATE_HOME: nowhere to keep this device's identity")?;
    let identity = match label {
        Some(label) => crate::mesh::identity::relabel(&path, label),
        None => NodeIdentity::load_or_create(&path),
    }
    .map_err(|e| e.to_string())?;
    Ok(DeviceView {
        device: Device {
            node: identity.public.node_id.as_str().to_string(),
            label: identity.public.display_name.clone(),
        },
        public_key: identity.public.public_key.clone(),
    })
}

// ------------------------------------------------------------------- records

/// The answer of `continuity records`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Records {
    /// Every admitted record, by line and then by id.
    pub records: Vec<RecordView>,
    /// Every line.
    pub lines: Vec<LineView>,
    /// Refused files and lineage findings.
    pub diagnostics: Vec<Diagnostic>,
}

/// `continuity records`: every record the local store admits, and what it refused.
pub fn records(machine: &Machine<'_>) -> Result<Records, String> {
    let loaded = load(machine, store::REF)?;
    let mut by_line: BTreeMap<(String, String), RecordView> = BTreeMap::new();
    for r in loaded.graph.records.values() {
        let v = RecordView::of(machine, r);
        by_line.insert((v.line.clone(), v.id.clone()), v);
    }
    let lines = loaded
        .graph
        .lines()
        .into_values()
        .map(|l| LineView {
            heads: l
                .heads
                .iter()
                .filter_map(|h| loaded.graph.records.get(h))
                .map(|r| RecordView::of(machine, r))
                .collect(),
            id: l.id,
            state: l.state,
            records: l.records,
        })
        .collect();
    let mut diagnostics = loaded.refused.clone();
    diagnostics.extend(loaded.graph.findings());
    Ok(Records {
        records: by_line.into_values().collect(),
        lines,
        diagnostics,
    })
}

/// Two machines and the remote between them, for the tests of this module and of the
/// command that renders it: real clones of one bare repository, each with its own device
/// key, so that every operation runs against git exactly as it does on a disk.
#[cfg(test)]
pub(crate) mod tests_support {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::Machine;
    use crate::mesh::config::MeshConfig;
    use crate::mesh::identity::NodeIdentity;

    /// Run git in `root` with a neutral identity, and answer its trimmed stdout.
    pub(crate) fn git(root: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A temporary directory holding `remote.git` and machine `a`'s clone on `feature/x`,
    /// pushed. The layer's local half is ignored, as `majordomus init` arranges.
    pub(crate) struct World {
        pub(crate) dir: tempfile::TempDir,
    }

    impl World {
        pub(crate) fn new() -> World {
            let dir = tempfile::tempdir().unwrap();
            let remote = dir.path().join("remote.git");
            git(
                dir.path(),
                &["init", "-q", "--bare", remote.to_str().unwrap()],
            );
            let a = dir.path().join("a");
            std::fs::create_dir_all(a.join("lib")).unwrap();
            git(&a, &["init", "-q", "-b", "main"]);
            std::fs::write(a.join(".gitignore"), ".ai/local/\n").unwrap();
            std::fs::write(a.join("lib/a"), "a\n").unwrap();
            git(&a, &["add", "-A"]);
            git(&a, &["commit", "-qm", "base"]);
            git(&a, &["remote", "add", "origin", remote.to_str().unwrap()]);
            git(&a, &["push", "-q", "origin", "main"]);
            git(&a, &["checkout", "-qb", "feature/x"]);
            git(&a, &["push", "-q", "-u", "origin", "feature/x"]);
            World { dir }
        }

        /// The checkout of machine `name`.
        pub(crate) fn root(&self, name: &str) -> PathBuf {
            self.dir.path().join(name)
        }

        /// Machine `name`'s clone of the remote, on `feature/x`.
        pub(crate) fn clone_as(&self, name: &str) -> PathBuf {
            let to = self.root(name);
            git(
                self.dir.path(),
                &[
                    "clone",
                    "-q",
                    self.dir.path().join("remote.git").to_str().unwrap(),
                    to.to_str().unwrap(),
                ],
            );
            git(&to, &["checkout", "-q", "feature/x"]);
            to
        }

        /// Machine `name`'s device key, the same on every call, labelled `label`.
        pub(crate) fn identity(&self, name: &str, label: &str) -> NodeIdentity {
            let path = self.dir.path().join("keys").join(format!("{name}.json"));
            crate::mesh::identity::relabel(&path, label)
                .or_else(|_| {
                    NodeIdentity::load_or_create(&path)?;
                    crate::mesh::identity::relabel(&path, label)
                })
                .unwrap()
        }
    }

    /// A mesh declaration whose trust list admits exactly `keys`.
    pub(crate) fn trusting(keys: &[&str]) -> MeshConfig {
        serde_json::from_value(serde_json::json!({
            "schema": "mesh/v1",
            "kind": "mesh",
            "id": "fixture",
            "trust": { "policy": "deny_unknown", "allow": keys },
        }))
        .unwrap()
    }

    /// The machine for `root`, signing as `identity`.
    pub(crate) fn machine(
        root: &Path,
        identity: NodeIdentity,
        mesh: Option<MeshConfig>,
    ) -> Machine<'_> {
        let repository = crate::mesh::repository::resolve(root, None).unwrap().id;
        Machine {
            root,
            identity,
            repository,
            mesh,
            identity_known: true,
        }
    }

    /// Write a handover record named `stamp` (a compact UTC timestamp, which orders them)
    /// with `body`, for task `task` when given.
    pub(crate) fn handover(root: &Path, stamp: &str, body: &str, task: Option<&str>) -> String {
        let dir = crate::mesh::handover::directory(root);
        std::fs::create_dir_all(&dir).unwrap();
        let name = format!("{stamp}--feature-x--0000000--00.md");
        std::fs::write(
            dir.join(&name),
            format!(
                "---\nschema_version: 1\ncreated_at: 2026-10-03T12:00:00Z\ntask_id: {}\n\
                 branch: feature/x\nworking_tree: clean\n---\n\n{body}",
                task.unwrap_or("none")
            ),
        )
        .unwrap();
        name
    }

    /// A handover body with the three sections.
    pub(crate) fn body(objective: &str, state: &str, next: &str) -> String {
        format!("# Objective\n{objective}\n\n# Current State\n{state}\n\n# Next Action\n{next}\n")
    }

    /// An active task `id` titled `title`, scoped to `lib`, with one decision recorded for it.
    pub(crate) fn task_with_decision(root: &Path, id: &str, title: &str) {
        let state = root.join(super::local::STATE_DIR);
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(
            state.join("current.yaml"),
            format!(
                "id: {id}\ntask: \"{title}\"\nprofile: implementation\nscope:\n  - lib\n\
                 started_at: 2026-10-03T11:00:00Z\n"
            ),
        )
        .unwrap();
        std::fs::write(
            state.join("decisions.md"),
            format!(
                "# Decisions\n\n## 2026-10-03 — Records travel in a ref of their own\n\
                 Task: {id}\nWhy: no branch carries them\n"
            ),
        )
        .unwrap();
    }

    /// Commit a change to `lib/a` on the current branch of `root`.
    pub(crate) fn commit(root: &Path, line: &str) {
        let path = root.join("lib/a");
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(line);
        text.push('\n');
        std::fs::write(&path, text).unwrap();
        git(root, &["commit", "-qam", line]);
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::{
        body, commit, git, handover, machine, task_with_decision, trusting, World,
    };
    use super::*;

    fn codes(items: &[PlanItem]) -> Vec<&str> {
        items.iter().map(|i| i.code.as_str()).collect()
    }

    #[test]
    fn a_section_is_read_by_its_heading_and_absent_is_empty() {
        let body = "# Objective\nship it\n\n# Next Action\nrun the tests\n";
        assert_eq!(section(body, "Next Action"), "run the tests");
        assert_eq!(section(body, "Current State"), "");
    }

    /// The whole journey of ADR 0105 on two clones: publish, sync, plan, resume, continue,
    /// and the lineage and the store states each step leaves behind.
    #[test]
    fn a_handover_moves_between_machines_and_continues_its_line() {
        let w = World::new();
        let a_root = w.root("a");
        let ia = w.identity("a", "macbook-pro");
        let ib = w.identity("b", "mac-mini");
        let keys = [ia.public.public_key.clone(), ib.public.public_key.clone()];
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        let a = machine(&a_root, ia, Some(trusting(&keys)));

        // nothing to publish yet
        let err = publish(&a, &PublishRequest::default()).unwrap_err();
        assert!(err.contains("no handover record"), "{err}");

        task_with_decision(&a_root, "t-1", "Ship the 'fixture'");
        handover(
            &a_root,
            "20261003T120000Z",
            &body("Ship x", "parser done", "write the test"),
            Some("t-1"),
        );
        let published = publish(
            &a,
            &PublishRequest {
                issue: Some("#184".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(published.written);
        assert_eq!(published.record.issue.as_deref(), Some("#184"));
        assert_eq!(published.record.task.as_deref(), Some("Ship the 'fixture'"));
        assert_eq!(published.record.objective, "Ship x");
        assert!(published.record.this_device);
        assert_eq!(published.record.trust, Trust::ThisDevice);
        let rec_a = published.record.id.clone();

        // publishing the same handover at the same source writes nothing
        let again = publish(&a, &PublishRequest::default()).unwrap();
        assert!(!again.written);
        assert_eq!(again.record.id, rec_a);

        // a remote exists and has never been asked
        let s = status(&a).unwrap();
        assert_eq!(s.store.sync, SyncState::NeverSynced);
        assert_eq!(s.next, ["majordomus-cli continuity sync"]);
        assert_eq!(s.position.as_ref().unwrap().record, rec_a);
        assert_eq!(s.branch, "feature/x");

        let synced = sync(&a, None).unwrap();
        assert_eq!(synced.action, SyncAction::Published);
        assert_eq!(synced.published, 1);
        assert_eq!(synced.lines[0].relation, LineSync::LocalOnly);
        assert_eq!(status(&a).unwrap().store.sync, SyncState::InSync);

        // machine B: a clone that has asked nothing yet
        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), Some(trusting(&keys)));
        let s = status(&b).unwrap();
        assert_eq!(s.store.sync, SyncState::Empty);
        assert!(s.resumable.is_empty(), "status must not use the network");
        let p = plan(&b, None).unwrap();
        assert_eq!(p.status, PlanStatus::NothingToResume);
        assert_eq!(p.actions, ["majordomus-cli continuity sync"]);

        let synced = sync(&b, None).unwrap();
        assert_eq!(synced.action, SyncAction::Fetched);
        assert_eq!(synced.fetched, 1);
        assert_eq!(synced.resumable[0].id, rec_a);
        assert_eq!(synced.resumable[0].trust, Trust::Trusted);
        assert_eq!(status(&b).unwrap().next, ["majordomus-cli continuity plan"]);

        let p = plan(&b, None).unwrap();
        assert_eq!(p.status, PlanStatus::Ready, "{:?}", p.warnings);
        assert_eq!(p.source.as_ref().unwrap().relation, SourceRelation::Exact);
        assert_eq!(p.next_action, "write the test");
        assert_eq!(p.current_state, "parser done");
        assert_eq!(
            p.decisions,
            ["2026-10-03 — Records travel in a ref of their own"]
        );
        assert!(p.actions[0].starts_with("majordomus-cli continuity resume --record "));
        assert_eq!(
            p.actions[1],
            "majordomus start 'Ship the '\\''fixture'\\''' --scope lib --profile implementation"
        );

        let r = resume(&b, None).unwrap();
        assert!(r.resumed);
        assert_eq!(r.decisions_carried, 1);
        assert_eq!(r.position.as_ref().unwrap().via, Via::Resumed);
        let written = b_root.join(r.handover.as_ref().unwrap());
        let text = std::fs::read_to_string(&written).unwrap();
        assert!(
            text.contains(&format!("continuity_record: {rec_a}")),
            "{text}"
        );
        assert!(!r
            .plan
            .actions
            .iter()
            .any(|a| a.contains("continuity resume")));

        // resuming again rewrites nothing and says so
        let again = resume(&b, Some(&rec_a[..8])).unwrap();
        assert!(again.resumed);
        assert_eq!(again.decisions_carried, 0);
        assert_eq!(again.handover, r.handover);
        assert!(codes(&again.plan.warnings).contains(&"already_resumed"));
        assert_eq!(again.plan.lineage, Some(Relation::Equal));
        // and a resumed handover is not this machine's to publish
        let err = publish(&b, &PublishRequest::default()).unwrap_err();
        assert!(err.contains("arrived from another machine"), "{err}");

        // B continues the work; its record continues A's line
        commit(&b_root, "b");
        handover(
            &b_root,
            "20261003T130000Z",
            &body("Ship x", "test written", "open the PR"),
            None,
        );
        let pb = publish(&b, &PublishRequest::default()).unwrap();
        assert_eq!(pb.record.parent.as_deref(), Some(rec_a.as_str()));
        assert_eq!(pb.record.line, rec_a);
        assert_eq!(
            pb.record.issue.as_deref(),
            Some("#184"),
            "the issue carries along the line"
        );
        assert_eq!(status(&b).unwrap().store.sync, SyncState::Pending);
        let rec_b = pb.record.id.clone();
        git(&b_root, &["push", "-q", "origin", "feature/x"]);
        assert_eq!(sync(&b, None).unwrap().action, SyncAction::Published);

        // A fetched B's continuation; its source is behind until it pulls
        let synced = sync(&a, None).unwrap();
        assert_eq!(synced.action, SyncAction::Fetched);
        assert_eq!(synced.lines[0].relation, LineSync::RemoteNewer);
        let all = records(&a).unwrap();
        assert_eq!(all.records.len(), 2);
        assert_eq!(all.lines.len(), 1);
        assert_eq!(all.lines[0].heads[0].id, rec_b);
        assert_eq!(
            plan(&a, None).unwrap().source.unwrap().relation,
            SourceRelation::HeadMissing
        );
        git(&a_root, &["fetch", "-q", "origin"]);
        let p = plan(&a, None).unwrap();
        assert_eq!(p.status, PlanStatus::RequiresSourceUpdate);
        assert_eq!(
            p.source.as_ref().unwrap().relation,
            SourceRelation::LocalBehind
        );
        assert!(codes(&p.blockers).contains(&"local_behind"));
        assert_eq!(p.actions, ["git pull --ff-only origin feature/x"]);
        git(&a_root, &["pull", "-q", "--ff-only", "origin", "feature/x"]);
        let p = plan(&a, None).unwrap();
        assert_eq!(p.status, PlanStatus::Ready, "{:?}", p.warnings);
        assert_eq!(p.lineage, Some(Relation::Newer));
        assert_eq!(p.record.as_ref().unwrap().device.label, "mac-mini");

        // idempotent: nothing new moves nothing
        let quiet = sync(&a, None).unwrap();
        assert_eq!(quiet.action, SyncAction::None);
        assert_eq!((quiet.fetched, quiet.published), (0, 0));
        assert_eq!(quiet.lines[0].relation, LineSync::Equal);

        // A publishes on top of B's while B's store has not seen it: pending here
        commit(&a_root, "a2");
        handover(
            &a_root,
            "20261003T140000Z",
            &body("Ship x", "reviewed", "merge"),
            None,
        );
        resume(&a, None).unwrap();
        let pa = publish(&a, &PublishRequest::default()).unwrap();
        assert_eq!(pa.record.parent.as_deref(), Some(rec_b.as_str()));
        assert_eq!(status(&a).unwrap().store.sync, SyncState::Pending);
    }

    /// The same line continued on two machines is a conflict a person resolves, and naming
    /// one of the heads resolves it with a warning instead.
    #[test]
    fn the_same_work_continued_twice_is_a_conflict_until_a_record_is_named() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(
            &a_root,
            "20261003T120000Z",
            &body("Ship x", "one", "two"),
            None,
        );
        let first = publish(&a, &PublishRequest::default()).unwrap().record.id;
        sync(&a, None).unwrap();

        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        sync(&b, None).unwrap();
        let p = plan(&b, None).unwrap();
        // no mesh: the key is checked for integrity only
        assert_eq!(p.status, PlanStatus::ReadyWithWarnings);
        assert_eq!(codes(&p.warnings), ["origin_undeclared"]);
        assert!(resume(&b, None).unwrap().resumed);

        // both continue the same record
        handover(
            &a_root,
            "20261003T130000Z",
            &body("Ship x", "a went on", "a"),
            None,
        );
        let ra = publish(&a, &PublishRequest::default()).unwrap().record.id;
        handover(
            &b_root,
            "20261003T130001Z",
            &body("Ship x", "b went on", "b"),
            None,
        );
        let rb = publish(&b, &PublishRequest::default()).unwrap().record.id;
        sync(&a, None).unwrap();
        let synced = sync(&b, None).unwrap();
        assert_eq!(synced.action, SyncAction::Exchanged);
        assert_eq!(synced.lines[0].relation, LineSync::Diverged);
        assert_eq!(synced.lines[0].heads.len(), 2);
        let s = status(&b).unwrap();
        assert_eq!(s.lines[0].state, LineState::Diverged);
        assert!(
            s.diagnostics.iter().any(|d| d.code.contains("diverged")),
            "{:?}",
            s.diagnostics
        );

        let p = plan(&b, None).unwrap();
        assert_eq!(p.status, PlanStatus::Conflict);
        assert_eq!(p.record.as_ref().unwrap().id, ra);
        assert!(codes(&p.blockers).contains(&"line_diverged"));
        assert!(codes(&p.blockers).contains(&"would_diverge"));
        assert!(!resume(&b, None).unwrap().resumed);

        let named = plan(&b, Some(&ra)).unwrap();
        assert!(named.status.resumable(), "{:?}", named.blockers);
        assert!(codes(&named.warnings).contains(&"line_diverged"));
        assert!(codes(&named.warnings).contains(&"would_diverge"));

        // a prefix every record shares names none of them
        let p = plan(&b, Some("")).unwrap();
        assert_eq!(p.status, PlanStatus::ChooseRecord);
        assert_eq!(p.candidates.len(), 3);
        let p = plan(&b, Some("zz")).unwrap();
        assert_eq!(p.status, PlanStatus::NothingToResume);
        assert_eq!(codes(&p.blockers), ["record_unknown"]);
        assert_ne!(first, rb);
    }

    /// Independent lines are not a conflict, and an untrusted signer is never resumed.
    #[test]
    fn separate_lines_are_a_choice_and_an_untrusted_signer_is_refused() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("one", "x", "y"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        sync(&a, None).unwrap();
        let c_root = w.clone_as("c");
        let c = machine(&c_root, w.identity("c", "mac-studio"), None);
        handover(&c_root, "20261003T120500Z", &body("two", "x", "y"), None);
        publish(&c, &PublishRequest::default()).unwrap();
        sync(&c, None).unwrap();

        let b_root = w.clone_as("b");
        // B's trust list admits nobody
        let b = machine(&b_root, w.identity("b", "mac-mini"), Some(trusting(&[])));
        sync(&b, None).unwrap();
        let p = plan(&b, None).unwrap();
        assert_eq!(p.status, PlanStatus::ChooseRecord);
        assert_eq!(codes(&p.blockers), ["several_resumable"]);
        assert_eq!(p.candidates.len(), 2);

        let id = p.candidates[0].id.clone();
        let p = plan(&b, Some(&id)).unwrap();
        assert_eq!(p.status, PlanStatus::Refused);
        assert!(codes(&p.blockers).contains(&"origin_untrusted"));
        let r = resume(&b, Some(&id)).unwrap();
        assert!(!r.resumed);
        assert!(r.handover.is_none());
    }

    /// Every way the local source can stand towards a record's is its own verdict.
    #[test]
    fn the_source_check_names_how_this_checkout_stands() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);

        // a commit A never pushed: B does not have it
        commit(&a_root, "unpushed");
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        let rec = publish(&a, &PublishRequest::default()).unwrap().record.id;
        sync(&a, None).unwrap();
        sync(&b, None).unwrap();
        let p = plan(&b, Some(&rec)).unwrap();
        assert_eq!(p.status, PlanStatus::RequiresSourceUpdate);
        assert_eq!(
            p.source.as_ref().unwrap().relation,
            SourceRelation::HeadMissing
        );
        assert_eq!(p.actions, ["git fetch origin"]);

        // the commit arrives, but B is on another branch
        git(&a_root, &["push", "-q", "origin", "feature/x"]);
        git(&b_root, &["fetch", "-q", "origin"]);
        git(&b_root, &["checkout", "-q", "main"]);
        let p = plan(&b, Some(&rec)).unwrap();
        assert_eq!(
            p.source.as_ref().unwrap().relation,
            SourceRelation::BranchDiffers
        );
        assert_eq!(p.actions, ["git switch feature/x"]);

        // on the branch, with commits of its own after the record's
        git(&b_root, &["checkout", "-q", "feature/x"]);
        git(&b_root, &["merge", "-q", "--ff-only", "origin/feature/x"]);
        commit(&b_root, "later");
        let p = plan(&b, Some(&rec)).unwrap();
        assert_eq!(
            p.source.as_ref().unwrap().relation,
            SourceRelation::LocalAhead
        );
        assert!(codes(&p.warnings).contains(&"local_ahead"));

        // A moves on separately: the histories diverge
        commit(&a_root, "elsewhere");
        handover(&a_root, "20261003T130000Z", &body("o", "s2", "n2"), None);
        let rec2 = publish(&a, &PublishRequest::default()).unwrap().record.id;
        git(&a_root, &["push", "-q", "origin", "feature/x"]);
        sync(&a, None).unwrap();
        sync(&b, None).unwrap();
        git(&b_root, &["fetch", "-q", "origin"]);
        let p = plan(&b, Some(&rec2)).unwrap();
        assert_eq!(
            p.source.as_ref().unwrap().relation,
            SourceRelation::Diverged
        );
        assert_eq!(p.status, PlanStatus::Conflict);
        assert!(codes(&p.blockers).contains(&"source_diverged"));
    }

    /// A handover written in a dirty tree restores the context and says the source is not
    /// restored, unless the very same uncommitted work is here.
    #[test]
    fn a_dirty_origin_restores_context_and_names_what_it_cannot() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        std::fs::write(a_root.join("lib/a"), "a\nuncommitted\n").unwrap();
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        let published = publish(&a, &PublishRequest::default()).unwrap();
        assert_eq!(published.record.working_tree, WorkingTree::Dirty);
        assert_eq!(published.record.changed_total, 1);
        sync(&a, None).unwrap();

        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        sync(&b, None).unwrap();
        let p = plan(&b, None).unwrap();
        assert!(codes(&p.warnings).contains(&"source_incomplete"));
        assert!(!codes(&p.warnings).contains(&"local_dirty"));
        assert_eq!(p.source.as_ref().unwrap().origin_changed, ["lib/a"]);

        // the same uncommitted work, here too
        std::fs::write(b_root.join("lib/a"), "a\nuncommitted\n").unwrap();
        let p = plan(&b, None).unwrap();
        assert_eq!(p.source.as_ref().unwrap().same_uncommitted_work, Some(true));
        assert!(!codes(&p.warnings).contains(&"source_incomplete"));
        assert!(!codes(&p.warnings).contains(&"local_dirty"));

        // different uncommitted work here
        std::fs::write(b_root.join("lib/a"), "a\nmine\n").unwrap();
        let p = plan(&b, None).unwrap();
        assert_eq!(
            p.source.as_ref().unwrap().same_uncommitted_work,
            Some(false)
        );
        assert!(codes(&p.warnings).contains(&"local_dirty"));
    }

    /// What a publication refuses, it refuses before it writes anything.
    #[test]
    fn a_publication_refuses_what_must_not_leave_the_machine() {
        let w = World::new();
        let root = w.root("a");
        let a = machine(&root, w.identity("a", "macbook-pro"), None);
        handover(
            &root,
            "20261003T120000Z",
            &body("o", "token ghp_0123456789abcdefghijklmnopqrstuvwxyzAB", "n"),
            None,
        );
        let err = publish(&a, &PublishRequest::default()).unwrap_err();
        assert!(err.contains("is not published"), "{err}");
        assert!(
            store::tip(&root, store::REF).is_none(),
            "nothing reached the store"
        );

        for name in ["../x.md", "a/b.md", "x.txt"] {
            let err = locate_handover(&root, Some(name)).unwrap_err();
            assert!(err.contains("is not the file name"), "{name}: {err}");
        }
        let err = locate_handover(&root, Some("20990101T000000Z--absent.md")).unwrap_err();
        assert!(err.contains("no handover record"), "{err}");
        let named = handover(&root, "20261003T110000Z", &body("o", "s", "n"), None);
        assert!(locate_handover(&root, Some(&named)).unwrap().is_file());
        let published = publish(
            &a,
            &PublishRequest {
                handover: Some(named.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(published.handover.ends_with(&named));
    }

    /// Without a remote the store is local, an unreachable remote changes nothing local,
    /// and a remote name that is not one is refused.
    #[test]
    fn a_sync_without_a_reachable_remote_moves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("solo");
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "-b", "main"]);
        std::fs::write(root.join(".gitignore"), ".ai/local/\n").unwrap();
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-qm", "base"]);
        let id = crate::mesh::identity::relabel(&dir.path().join("k.json"), "solo")
            .or_else(|_| {
                NodeIdentity::load_or_create(&dir.path().join("k.json"))?;
                crate::mesh::identity::relabel(&dir.path().join("k.json"), "solo")
            })
            .unwrap();
        let m = machine(&root, id, None);
        let synced = sync(&m, None).unwrap();
        assert_eq!(synced.action, SyncAction::NoRemote);
        assert_eq!(synced.diagnostics[0].code, "continuity.no_remote");
        assert_eq!(status(&m).unwrap().store.sync, SyncState::NoRemote);
        assert_eq!(status(&m).unwrap().branch, "main");

        assert!(sync(&m, Some("a:b"))
            .unwrap_err()
            .contains("is not a remote name"));
        let gone = dir.path().join("gone.git");
        git(&root, &["remote", "add", "gone", gone.to_str().unwrap()]);
        let synced = sync(&m, Some("gone")).unwrap();
        assert_eq!(synced.action, SyncAction::Unreachable);
        assert_eq!(synced.diagnostics[0].code, "continuity.remote_unreachable");
        let note = local::load(&root).unwrap().last_sync.unwrap();
        assert_eq!(note.outcome, "unreachable");
        // the next plan says nothing was ever published, rather than to sync
        let p = plan(&m, None).unwrap();
        assert_eq!(codes(&p.warnings), ["nothing_published"]);
        assert!(p.actions.is_empty());
    }

    /// What admission refuses is reported, never read: a stray file, bytes that are not a
    /// record, a record from a newer schema.
    #[test]
    fn a_store_reports_every_file_it_refused() {
        let w = World::new();
        let root = w.root("a");
        let m = machine(&root, w.identity("a", "macbook-pro"), None);
        let newer = serde_json::json!({ "record": { "schema": "majordomus-continuity/v9" } });
        store::add(
            &root,
            &[
                ("not-a-record".into(), b"{}".to_vec()),
                ("a".repeat(32), b"not json".to_vec()),
                ("b".repeat(32), newer.to_string().into_bytes()),
            ],
            "fixture\n",
        )
        .unwrap();
        let loaded = load(&m, store::REF).unwrap();
        assert_eq!(loaded.refused.len(), 3);
        assert_eq!(loaded.too_new, 1);
        let s = status(&m).unwrap();
        let codes: Vec<&str> = s.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert!(codes.contains(&"continuity.stray_file"), "{codes:?}");
        assert!(codes.contains(&"continuity.upgrade_required"), "{codes:?}");
        assert_eq!(s.store.refused, 3);
        assert_eq!(records(&m).unwrap().diagnostics.len(), 3);
        // a refused file is named when a plan is asked for it
        let p = plan(&m, Some(&"a".repeat(8))).unwrap();
        assert_eq!(codes_of(&p.blockers), ["continuity.record_malformed"]);
    }

    fn codes_of(items: &[PlanItem]) -> Vec<String> {
        items.iter().map(|i| i.code.clone()).collect()
    }

    /// A read never creates a key: the status says this device has none yet.
    #[test]
    fn a_status_without_a_device_identity_says_so() {
        let w = World::new();
        let root = w.root("a");
        let mut m = machine(&root, NodeIdentity::ephemeral().unwrap(), None);
        m.identity_known = false;
        let s = status(&m).unwrap();
        assert!(s
            .diagnostics
            .iter()
            .any(|d| d.code == "continuity.no_device_identity"));
        assert_eq!(m.trust("00"), Trust::Undeclared);
        let m = machine(
            &root,
            NodeIdentity::ephemeral().unwrap(),
            Some(trusting(&[])),
        );
        assert_eq!(m.trust("00"), Trust::Untrusted);
    }

    /// The state directory with writing taken away, and given back when this goes: a
    /// directory left unwritable could not be removed with the rest of the fixture.
    struct ReadOnly(std::path::PathBuf);
    impl ReadOnly {
        fn of(root: &Path) -> ReadOnly {
            use std::os::unix::fs::PermissionsExt;
            let dir = root.join(local::STATE_DIR);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
            ReadOnly(dir)
        }
    }
    impl Drop for ReadOnly {
        fn drop(&mut self) {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
        }
    }

    /// Every operation reads this checkout's state and the store before it decides
    /// anything, and none of them proceeds on one it could not read: the state holds the
    /// parent of the next publication, and a store read in part is a lineage made up.
    #[test]
    fn an_unreadable_state_or_store_stops_every_operation() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        publish(&a, &PublishRequest::default()).unwrap();

        // the local state, cut short
        let state = local::local_path(&a_root);
        let good = std::fs::read(&state).unwrap();
        std::fs::write(&state, "{ not json").unwrap();
        let failures = [
            ("status", status(&a).map(drop)),
            ("publish", publish(&a, &PublishRequest::default()).map(drop)),
            ("sync", sync(&a, None).map(drop)),
            ("plan", plan(&a, None).map(drop)),
            ("resume", resume(&a, None).map(drop)),
        ];
        for (what, answer) in failures {
            let err = answer.expect_err(what);
            assert!(err.contains(local::LOCAL_FILE), "{what}: {err}");
        }
        std::fs::write(&state, &good).unwrap();

        // the store, pointed at one holding more files than a store is read for
        let tip = store::tip(&a_root, store::REF).unwrap();
        let oversized = store::tests_support::oversized(&a_root);
        git(&a_root, &["update-ref", store::REF, &oversized]);
        let failures = [
            ("status", status(&a).map(drop)),
            ("records", records(&a).map(drop)),
            ("publish", publish(&a, &PublishRequest::default()).map(drop)),
            ("sync", sync(&a, None).map(drop)),
            ("plan", plan(&a, None).map(drop)),
            ("resume", resume(&a, None).map(drop)),
        ];
        for (what, answer) in failures {
            answer.expect_err(what);
        }
        // nothing was repaired behind anyone's back: the ref is where it was put, and put
        // back the store reads as before
        assert_eq!(store::tip(&a_root, store::REF).unwrap(), oversized);
        git(&a_root, &["update-ref", store::REF, &tip]);
        assert_eq!(records(&a).unwrap().records.len(), 1);
    }

    /// An operation that cannot record where it now stands says so instead of answering as
    /// if it had: a publication, a sync that reached its remote, a sync that did not, and a
    /// resume each end in the state file, and each fails when it cannot be written.
    #[test]
    fn a_state_that_cannot_be_written_is_an_error_and_not_a_success() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        sync(&a, None).unwrap();

        // B fetches A's record while it still can, so that a resume has something to resume
        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        sync(&b, None).unwrap();
        std::fs::create_dir_all(crate::mesh::handover::directory(&b_root)).unwrap();
        {
            let _held = ReadOnly::of(&b_root);
            let err = resume(&b, None).unwrap_err();
            assert!(err.contains(local::LOCAL_FILE), "{err}");
        }

        // A: a second handover, published, synced and synced to nowhere, with the state shut
        handover(&a_root, "20261003T130000Z", &body("o", "s2", "n2"), None);
        let gone = w.dir.path().join("gone.git");
        git(&a_root, &["remote", "add", "gone", gone.to_str().unwrap()]);
        let _held = ReadOnly::of(&a_root);
        for (what, answer) in [
            ("publish", publish(&a, &PublishRequest::default()).map(drop)),
            ("sync", sync(&a, None).map(drop)),
            ("unreachable sync", sync(&a, Some("gone")).map(drop)),
        ] {
            let err = answer.expect_err(what);
            assert!(err.contains(local::LOCAL_FILE), "{what}: {err}");
        }
    }

    /// How the local store stands against the remote's is read from the two refs: behind
    /// when the remote's tip is fetched and nothing local exists, behind when the local tip
    /// is contained in the remote's, diverged when each holds what the other lacks.
    #[test]
    fn a_store_is_behind_or_diverged_by_what_each_ref_contains() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        sync(&a, None).unwrap();

        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        // fetched, not merged: the remote's tip is known and there is no local store
        store::fetch(&b_root, "origin").unwrap();
        assert_eq!(status(&b).unwrap().store.sync, SyncState::Behind);
        sync(&b, None).unwrap();
        assert_eq!(status(&b).unwrap().store.sync, SyncState::InSync);

        // A moves the remote on; B fetches and does not merge: its tip is inside the remote's
        handover(&a_root, "20261003T130000Z", &body("o", "s2", "n2"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        sync(&a, None).unwrap();
        store::fetch(&b_root, "origin").unwrap();
        assert_eq!(status(&b).unwrap().store.sync, SyncState::Behind);

        // and B publishes without merging: each side now holds a commit the other lacks
        handover(&b_root, "20261003T140000Z", &body("o", "b", "n"), None);
        publish(&b, &PublishRequest::default()).unwrap();
        assert_eq!(status(&b).unwrap().store.sync, SyncState::Diverged);
        // a sync reconciles them
        sync(&b, None).unwrap();
        assert_eq!(status(&b).unwrap().store.sync, SyncState::InSync);
    }
    /// A remote that takes no push: its `pre-receive` hook refuses every ref.
    fn refuse_pushes(w: &World) {
        use std::os::unix::fs::PermissionsExt;
        let hook = w.dir.path().join("remote.git/hooks/pre-receive");
        std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// A remote that does not accept the store leaves the records pending and says so: the
    /// sync is partial, nothing is counted as published, and the local store is whole. It
    /// is so whether the remote held no store yet or already held one.
    #[test]
    fn a_remote_that_refuses_the_push_leaves_the_records_pending() {
        // the remote holds no store yet
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        refuse_pushes(&w);
        let refused = sync(&a, None).unwrap();
        assert_eq!(refused.published, 0);
        assert_eq!(refused.action, SyncAction::None);
        assert!(refused
            .diagnostics
            .iter()
            .any(|d| d.code == "continuity.push_refused"));
        assert_eq!(status(&a).unwrap().last_sync.unwrap().outcome, "partial");
        assert_eq!(records(&a).unwrap().records.len(), 1);

        // the remote already holds one: the second attempt fetches, merges and is refused too
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        sync(&a, None).unwrap();
        handover(&a_root, "20261003T130000Z", &body("o", "s2", "n2"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        refuse_pushes(&w);
        let refused = sync(&a, None).unwrap();
        assert_eq!(refused.published, 0);
        assert!(refused
            .diagnostics
            .iter()
            .any(|d| d.code == "continuity.push_refused"));
        assert_eq!(status(&a).unwrap().store.sync, SyncState::Pending);
        assert_eq!(records(&a).unwrap().records.len(), 2);
    }

    /// What the remote holds under a record's name is not taken on trust. A file with other
    /// bytes under the name of a record this store holds is a collision and the local copy
    /// stays; a genuine record under a name that is not its own is refused for good and is
    /// not merged in, so a sync never spreads it.
    #[test]
    fn a_record_altered_or_misnamed_on_the_remote_is_not_merged() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        let published = publish(&a, &PublishRequest::default()).unwrap();
        let id = published.record.id.clone();
        let genuine = store::read(&a_root, store::REF)
            .unwrap()
            .remove(&format!("records/{id}.json"))
            .unwrap();

        // another clone reaches the remote first, with other bytes under A's record name
        // and A's bytes under a name that is not theirs
        let kept = genuine.clone();
        let c_root = w.clone_as("c");
        let misnamed = format!(
            "{}{}",
            if id.starts_with('0') { "1" } else { "0" },
            &id[1..]
        );
        store::add(
            &c_root,
            &[(id.clone(), b"{}\n".to_vec()), (misnamed.clone(), genuine)],
            "altered\n",
        )
        .unwrap();
        store::push(&c_root, "origin").unwrap();

        // a clone with no store of its own takes the remote's without the misnamed file
        let d_root = w.clone_as("d");
        let d = machine(&d_root, w.identity("d", "mac-studio"), None);
        sync(&d, None).unwrap();
        let files = store::read(&d_root, store::REF).unwrap();
        assert!(files.contains_key(&format!("records/{id}.json")));
        assert!(!files.contains_key(&format!("records/{misnamed}.json")));

        let synced = sync(&a, None).unwrap();
        let codes: Vec<&str> = synced.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert!(codes.contains(&"continuity.collision"), "{codes:?}");
        assert!(codes.contains(&"continuity.id_mismatch"), "{codes:?}");
        // the local copy is the one that was signed here, and the misnamed file was left out
        let files = store::read(&a_root, store::REF).unwrap();
        assert_eq!(files[&format!("records/{id}.json")], kept);
        assert!(!files.contains_key(&format!("records/{misnamed}.json")));
        assert_eq!(records(&a).unwrap().records.len(), 1);
    }

    /// A clone whose remote was taken away still knows what it fetched: a sync says there
    /// is no remote, moves nothing, and goes on offering the record another device left.
    #[test]
    fn a_sync_with_no_remote_still_offers_what_was_fetched() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        sync(&a, None).unwrap();
        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        sync(&b, None).unwrap();
        git(&b_root, &["remote", "remove", "origin"]);
        let alone = sync(&b, None).unwrap();
        assert_eq!(alone.action, SyncAction::NoRemote);
        assert_eq!(alone.store.sync, SyncState::NoRemote);
        assert_eq!((alone.fetched, alone.published), (0, 0));
        assert_eq!(alone.resumable.len(), 1);
        assert_eq!(alone.resumable[0].device.label, "macbook-pro");
    }
    /// A record another device signed, added to `root`'s store as a sync would have left
    /// it: the sample record, about this repository and this checkout's commit, with
    /// `change` applied before it is signed.
    fn planted(
        root: &Path,
        m: &Machine<'_>,
        signer: &NodeIdentity,
        change: impl FnOnce(&mut Record),
    ) -> String {
        let mut r = record::tests_support::sample();
        r.repository = m.repository.clone();
        r.device = record::Device {
            node: signer.public.node_id.as_str().to_string(),
            label: "macbook-pro".into(),
        };
        let head = git(root, &["rev-parse", "HEAD"]);
        r.source.head = Some(head.clone());
        r.handover.head = Some(head);
        change(&mut r);
        let signed = SignedRecord::sign(r, signer);
        store::add(root, &[(signed.id.clone(), signed.to_bytes())], "planted\n").unwrap();
        signed.id
    }

    /// What a plan warns of and asks for when the record itself is unusual: it continues a
    /// record this store never received, it was published minutes ago from an episode
    /// still open on the other device, it names no commit, its task has no scope, or its
    /// commit is not here and no sync ever named a remote to fetch it from.
    #[test]
    fn a_plan_says_what_is_unusual_about_the_record_it_was_given() {
        let w = World::new();
        let a_root = w.root("a");
        let signer = w.identity("a", "macbook-pro");
        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);

        // it continues a record this store does not hold
        let orphan = planted(&b_root, &b, &signer, |r| {
            r.parent = Some("f".repeat(32));
            r.line = Some("e".repeat(32));
        });
        let p = plan(&b, Some(&orphan)).unwrap();
        assert!(codes(&p.warnings).contains(&"lineage_incomplete"), "{p:?}");
        assert!(p.status.resumable(), "{:?}", p.status);

        // it was published a moment ago from an episode that was still open there
        let fresh = planted(&b_root, &b, &signer, |r| {
            r.episode = record::EpisodeAtPublish::Open;
            r.published_at = now_rfc3339();
            r.handover.body.push_str("\nfresh\n");
            r.handover.id = crate::mesh::journal::HandoverBody::digest_of(&r.handover.body);
        });
        let p = plan(&b, Some(&fresh)).unwrap();
        assert!(
            codes(&p.warnings).contains(&"origin_may_be_active"),
            "{p:?}"
        );
        // the same record from long ago is not suspected of anything
        let p = plan(&b, Some(&orphan)).unwrap();
        assert!(!codes(&p.warnings).contains(&"origin_may_be_active"));

        // it names no commit: the source cannot be compared, and that is said
        let unplaced = planted(&b_root, &b, &signer, |r| {
            r.source.head = None;
            r.handover.head = None;
        });
        let p = plan(&b, Some(&unplaced)).unwrap();
        assert!(codes(&p.warnings).contains(&"source_unknown"), "{p:?}");
        assert_eq!(p.source.unwrap().relation, SourceRelation::Unknown);

        // its task has no scope: the task is offered again without one
        let unscoped = planted(&b_root, &b, &signer, |r| {
            r.task.as_mut().unwrap().scope.clear();
            r.task.as_mut().unwrap().title = "ship the unscoped thing".into();
        });
        let p = plan(&b, Some(&unscoped)).unwrap();
        let start = p
            .actions
            .iter()
            .find(|a| a.starts_with("majordomus start "))
            .unwrap();
        assert!(!start.contains("--scope"), "{start}");

        // its commit is not here, no sync ever ran and no remote is configured: the fetch
        // it asks for names origin, the name a clone would have
        let _ = a_root;
        git(&b_root, &["remote", "remove", "origin"]);
        let elsewhere = planted(&b_root, &b, &signer, |r| {
            r.source.head = Some("d".repeat(40));
            r.handover.head = Some("d".repeat(40));
        });
        let p = plan(&b, Some(&elsewhere)).unwrap();
        assert!(codes(&p.blockers).contains(&"head_missing"), "{p:?}");
        assert!(
            p.actions.contains(&"git fetch origin".to_string()),
            "{:?}",
            p.actions
        );
    }

    /// A checkout whose state names a record its store no longer holds plans as one that
    /// stands nowhere: the position is not compared with anything, and nothing is said
    /// about a lineage that cannot be read.
    #[test]
    fn a_position_the_store_no_longer_holds_is_compared_with_nothing() {
        let w = World::new();
        let signer = w.identity("a", "macbook-pro");
        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        handover(&b_root, "20261003T120000Z", &body("o", "s", "n"), None);
        publish(&b, &PublishRequest::default()).unwrap();
        // the store is gone; the state still says where this checkout stood
        git(&b_root, &["update-ref", "-d", store::REF]);
        assert!(local::load(&b_root)
            .unwrap()
            .branches
            .values()
            .next()
            .is_some());
        let id = planted(&b_root, &b, &signer, |_| {});
        let p = plan(&b, Some(&id)).unwrap();
        assert!(p.lineage.is_none(), "{p:?}");
        assert!(!codes(&p.warnings).contains(&"already_resumed"));
        assert!(!codes(&p.blockers).contains(&"would_diverge"));
    }

    /// A mesh declaration that states the repository's identity is believed; one that does
    /// not, and no declaration at all, leave it to be computed from the clone.
    #[test]
    fn a_declared_repository_identity_is_the_one_used() {
        let stated: MeshConfig = serde_json::from_value(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh", "id": "fixture",
            "cooperation": { "repository": "a".repeat(32) },
        }))
        .unwrap();
        assert_eq!(declared_repository(Some(&stated)), Some("a".repeat(32)));
        assert_eq!(declared_repository(Some(&trusting(&[]))), None);
        assert_eq!(declared_repository(None), None);
    }

    /// A handover written in a dirty tree arrives saying so: the record this checkout
    /// writes on resume carries the origin's working tree as it was, not as it is here.
    #[test]
    fn a_resume_from_a_dirty_origin_records_that_it_was_dirty() {
        let w = World::new();
        let signer = w.identity("a", "macbook-pro");
        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        let dirty = planted(&b_root, &b, &signer, |r| {
            r.source.working_tree = WorkingTree::Dirty;
            r.source.changed = vec!["lib/a".into()];
            r.source.changed_total = 1;
        });
        let resumed = resume(&b, Some(&dirty)).unwrap();
        assert!(resumed.resumed, "{:?}", resumed.plan);
        let written = std::fs::read_to_string(b_root.join(resumed.handover.unwrap())).unwrap();
        assert!(written.contains("working_tree: dirty"), "{written}");
        assert!(written.contains("continuity_record:"), "{written}");
    }

    /// A checkout that already stands past the record it is asked to resume says which
    /// record it stands on and that it continues the one asked for.
    #[test]
    fn resuming_a_record_this_checkout_already_continued_says_so() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        let first = publish(&a, &PublishRequest::default()).unwrap().record.id;
        handover(&a_root, "20261003T130000Z", &body("o", "s2", "n2"), None);
        let second = publish(&a, &PublishRequest::default()).unwrap().record.id;
        sync(&a, None).unwrap();

        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        sync(&b, None).unwrap();
        assert!(resume(&b, Some(&second)).unwrap().resumed);
        let p = plan(&b, Some(&first)).unwrap();
        let already = p
            .warnings
            .iter()
            .find(|i| i.code == "already_resumed")
            .unwrap_or_else(|| panic!("{p:?}"));
        assert!(
            already.message.contains(", which continues this record"),
            "{}",
            already.message
        );
    }
    /// A publication reads one handover and adds one record, and says which step failed:
    /// a file with no front matter, one with nothing under it, an identifier that is not
    /// one line, and a store whose ref another writer holds.
    #[test]
    fn a_publication_that_cannot_be_made_names_what_stopped_it() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        let dir = crate::mesh::handover::directory(&a_root);
        std::fs::create_dir_all(&dir).unwrap();
        let named = |name: &str| PublishRequest {
            handover: Some(name.into()),
            ..Default::default()
        };

        std::fs::write(dir.join("20261003T100000Z--words.md"), "just words\n").unwrap();
        let err = publish(&a, &named("20261003T100000Z--words.md")).unwrap_err();
        assert!(err.contains("no front matter"), "{err}");

        std::fs::write(
            dir.join("20261003T110000Z--empty.md"),
            "---\nschema_version: 1\ncreated_at: 2026-10-03T11:00:00Z\ntask_id: none\n---\n\n",
        )
        .unwrap();
        let err = publish(&a, &named("20261003T110000Z--empty.md")).unwrap_err();
        assert!(err.contains("no body"), "{err}");
        // a message names the repository's paths, never this machine's
        assert!(!err.contains(&a_root.display().to_string()), "{err}");

        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        let err = publish(
            &a,
            &PublishRequest {
                issue: Some("one\ntwo".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("handover") || err.contains("line"), "{err}");

        let lock = store::tests_support::locked(&a_root, store::REF);
        publish(&a, &PublishRequest::default()).unwrap_err();
        std::fs::remove_file(lock).unwrap();
        assert!(publish(&a, &PublishRequest::default()).unwrap().written);
    }

    /// A sync that cannot read what it fetched, or cannot move its own ref, fails rather
    /// than report a store it did not finish reconciling.
    #[test]
    fn a_sync_that_cannot_read_or_move_a_store_is_an_error() {
        // the remote holds more files than a store is read for
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        let c_root = w.clone_as("c");
        let big = store::tests_support::oversized(&c_root);
        git(
            &c_root,
            &["push", "-q", "origin", &format!("{big}:{}", store::REF)],
        );
        let err = sync(&a, None).unwrap_err();
        assert!(err.contains("more than"), "{err}");

        // the local ref is held by another writer while the remote's store is merged in
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        sync(&a, None).unwrap();
        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        sync(&b, None).unwrap();
        handover(&b_root, "20261003T130000Z", &body("o", "b", "n"), None);
        publish(&b, &PublishRequest::default()).unwrap();
        sync(&b, None).unwrap();
        let lock = store::tests_support::locked(&a_root, store::REF);
        sync(&a, None).unwrap_err();
        std::fs::remove_file(lock).unwrap();
        assert_eq!(sync(&a, None).unwrap().fetched, 1);
    }

    /// The remote moved between the fetch and the push: the first push is refused, the
    /// sync fetches and merges once more, and the second push is accepted. Nothing is left
    /// pending and nothing is reported as refused.
    #[test]
    fn a_push_refused_once_is_tried_again_after_a_second_fetch() {
        use std::os::unix::fs::PermissionsExt;
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        sync(&a, None).unwrap();
        handover(&a_root, "20261003T130000Z", &body("o", "s2", "n2"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        // a remote that refuses exactly one push
        let hooks = w.dir.path().join("remote.git/hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        let once = w.dir.path().join("refused-once");
        std::fs::write(
            hooks.join("pre-receive"),
            format!(
                "#!/bin/sh\nif [ -e '{0}' ]; then exit 0; fi\n: > '{0}'\nexit 1\n",
                once.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(
            hooks.join("pre-receive"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let synced = sync(&a, None).unwrap();
        assert!(
            once.exists(),
            "the first push was not refused, so nothing was retried"
        );
        assert_eq!(synced.published, 1);
        assert_eq!(synced.action, SyncAction::Published);
        assert!(!synced
            .diagnostics
            .iter()
            .any(|d| d.code == "continuity.push_refused"));
        assert_eq!(status(&a).unwrap().store.sync, SyncState::InSync);
    }

    /// A local store replaced while a sync was pushing is read again afterwards, and a
    /// store that can no longer be read ends the sync as an error: what the sync reports
    /// is the store as it stands, never the one it started from.
    #[test]
    fn a_store_that_breaks_during_a_sync_is_an_error_and_not_a_report() {
        use std::os::unix::fs::PermissionsExt;
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        let big = store::tests_support::oversized(&a_root);
        // the remote's hook, run while the push is in flight, points this clone's store at
        // a tree no store is read for
        let hooks = w.dir.path().join("remote.git/hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        std::fs::write(
            hooks.join("pre-receive"),
            format!(
                "#!/bin/sh\nenv -u GIT_DIR -u GIT_QUARANTINE_PATH -u GIT_OBJECT_DIRECTORY \\\n  -u GIT_ALTERNATE_OBJECT_DIRECTORIES git -C '{}' update-ref {} {big}\nexit 0\n",
                a_root.display(),
                store::REF
            ),
        )
        .unwrap();
        std::fs::set_permissions(
            hooks.join("pre-receive"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let err = sync(&a, None).unwrap_err();
        assert!(err.contains("more than"), "{err}");
    }

    /// A checkout that resumed one continuation is told so when the plan would take it to
    /// another: the blocker names the record it stands on and that it was resumed here.
    #[test]
    fn a_plan_away_from_a_resumed_continuation_says_it_was_resumed_here() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        sync(&a, None).unwrap();

        // C continues A's record and publishes; A continues it too
        let c_root = w.clone_as("c");
        let c = machine(&c_root, w.identity("c", "mac-studio"), None);
        sync(&c, None).unwrap();
        assert!(resume(&c, None).unwrap().resumed);
        handover(&c_root, "20261003T130000Z", &body("o", "from c", "n"), None);
        let theirs = publish(&c, &PublishRequest::default()).unwrap().record.id;
        sync(&c, None).unwrap();
        handover(&a_root, "20261003T140000Z", &body("o", "from a", "n"), None);
        publish(&a, &PublishRequest::default()).unwrap();
        sync(&a, None).unwrap();

        // B resumes C's continuation by name, then asks what else there is
        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        sync(&b, None).unwrap();
        assert!(resume(&b, Some(&theirs)).unwrap().resumed);
        let p = plan(&b, None).unwrap();
        let would = p
            .blockers
            .iter()
            .find(|i| i.code == "would_diverge")
            .unwrap_or_else(|| panic!("{p:?}"));
        assert!(would.message.contains("resumed here"), "{}", would.message);
    }

    /// A resume writes the handover and the decisions before it records where it stands,
    /// and fails at the one it could not write.
    #[test]
    fn a_resume_that_cannot_write_what_it_restores_is_an_error() {
        use std::os::unix::fs::PermissionsExt;
        let w = World::new();
        let signer = w.identity("a", "macbook-pro");
        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        let id = planted(&b_root, &b, &signer, |r| {
            r.decisions = vec![record::CarriedDecision {
                title: "Records travel in a ref of their own".into(),
                text: "Why: no branch carries them".into(),
            }];
        });

        // the handovers directory takes no file
        let handovers = crate::mesh::handover::directory(&b_root);
        std::fs::create_dir_all(&handovers).unwrap();
        std::fs::set_permissions(&handovers, std::fs::Permissions::from_mode(0o555)).unwrap();
        let refused = resume(&b, Some(&id));
        std::fs::set_permissions(&handovers, std::fs::Permissions::from_mode(0o755)).unwrap();
        refused.unwrap_err();

        // the handover is written, and the decision log cannot be
        {
            let _held = ReadOnly::of(&b_root);
            resume(&b, Some(&id)).unwrap_err();
        }
        assert!(resume(&b, Some(&id)).unwrap().resumed);
    }
}
