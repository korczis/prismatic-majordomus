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
            NodeIdentity::load_or_create(&path).map_err(|e| e.to_string())?
        } else {
            NodeIdentity::ephemeral().map_err(|e| e.to_string())?
        };
        let declared = mesh.as_ref().and_then(|m| m.cooperation.repository.clone());
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
///
/// ```
/// use majordomus_cli::continuity::section;
/// let body = "# Objective\nship it\n\n# Next Action\nrun the tests\n";
/// assert_eq!(section(body, "Next Action"), "run the tests");
/// assert_eq!(section(body, "Current State"), "");
/// ```
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
    let mut out: Vec<SignedRecord> = graph
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
        .collect();
    out.sort_by(|a, b| {
        b.record
            .published_at
            .cmp(&a.record.published_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    out
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
    if let Some(p) = &position {
        if let Some(existing) = loaded.graph.records.get(&p.record) {
            let same_device =
                existing.record.device.node == machine.identity.public.node_id.as_str();
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
    if published_n > 0 || matches!(fetched, store::Fetched::Empty) && !local_ids.is_empty() {
        if let Err(first) = store::push(root, &remote) {
            pushed = match store::fetch(root, &remote) {
                Ok(store::Fetched::Store(commit)) => {
                    store::merge(root, &commit, &keep)?;
                    store::push(root, &remote).is_ok()
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
    } else if let Some(tip) = store::tip(root, store::REF) {
        // nothing to publish: the remote holds everything the local store does
        if store::tip(root, &store::remote_ref(&remote)).as_deref() != Some(tip.as_str())
            && published_n == 0
            && fetched_n == 0
        {
            // equal content under different commits: leave the refs as git left them
        }
    }

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
    plan_with(machine, &loaded, &state, record)
}

fn plan_with(
    machine: &Machine<'_>,
    loaded: &Loaded,
    state: &LocalState,
    record: Option<&str>,
) -> Result<ResumePlan, String> {
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
                    return Ok(p);
                }
                many => {
                    let mut p = ResumePlan::empty(PlanStatus::ChooseRecord);
                    p.candidates = many.iter().map(|r| RecordView::of(machine, r)).collect();
                    return Ok(p);
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
                    return Ok(p);
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
                    return Ok(p);
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
    if let Some(l) = graph.lines().get(&line) {
        if l.state == LineState::Diverged {
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
    Ok(p)
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
    let plan = plan_with(machine, &loaded, &state, record)?;
    let Some(view) = plan.record.clone().filter(|_| plan.status.resumable()) else {
        return Ok(Resumed {
            resumed: false,
            plan,
            handover: None,
            decisions_carried: 0,
            position: None,
        });
    };
    let chosen = loaded
        .graph
        .records
        .get(&view.id)
        .ok_or("the planned record left the store")?;
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

/// The cached offers of `root`, for a surface that must not read the store — the entry
/// banner. Empty when the file is absent or unreadable.
pub fn cached_offers(root: &Path) -> Vec<Offer> {
    local::load(root).map(|s| s.offers).unwrap_or_default()
}
