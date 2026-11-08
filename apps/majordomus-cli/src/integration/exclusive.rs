//! One integration executor per repository and base branch, across machines (ADR 0101 §7).
//!
//! [`IntegrationLease`] is a `flock` in the common git directory: it serialises every
//! worktree of one clone and nothing beyond it, so two clones — two machines — would each
//! take their own lease and both merge. When this checkout's shared server runs the
//! repository's mesh, the executor therefore also holds an exclusive mesh claim on
//! [`scope`] — `integration/<repository>/<base>` — which every linked runtime admits
//! against: a second machine's executor is refused, naming the holder, before it takes its
//! lease or acts. Where no mesh runs the lease alone guards, and the lease record — what
//! `prs brief` and the Cockpit read — says so by carrying no claim.
//!
//! The claim is taken before the clone's lease, so the lease record is written once, with
//! the claim's key in it; a claim whose lease is then refused is released as it is dropped,
//! and a held claim is released after the lease it belongs to. A claim an executor of this
//! checkout left behind when it was killed carries this checkout's [`session`], which the
//! mesh never refuses to its own session: the next executor here takes the lease and then
//! releases the leftover, so a crash costs other machines a wait until this checkout runs
//! its executor again, never a claim nobody can give back.
//!
//! The claim lives by the executor's progress through its queue, not by its process existing
//! (I3991): it is beaten once as it is taken and again each time the executor advances its
//! queue ([`MeshClaim::beat`]), so an executor that holds it and does nothing has it suspended
//! by the mesh once `cooperation.session_ttl_seconds` passes, and another machine's executor
//! is admitted. Before every write to the forge the executor asks whether it still holds it
//! ([`MeshClaim::holds`]); one admitted over meanwhile is released by the mesh with
//! [`crate::mesh::cooperation::OVERTAKEN`] at that beat, and the executor acts on nothing.
//!
//! Where the repository has an `origin`, the executor's state is kept there too (I3911), so
//! any clone continues it without a mesh: an [`OriginLease`] on [`lease_ref`] names its
//! holder, host, clone and expiry and is moved only by non-forced pushes, taken over by
//! another clone only after it expired, and renewed before every write to the forge — a
//! holder that slept past its expiry and was taken over is refused at that renewal and
//! lands nothing. The record the continuous gate and the halt are read from —
//! `merge_succeeded`, `verification_failed`, `failure_acknowledged` — is appended to
//! [`TRAIL_REF`] ([`publish_trail_line`]) and read back through this clone's
//! [`TRAIL_MIRROR`].

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{json, Value};

use super::drain::IntegrationLease;

/// How long one request to this checkout's server may take.
const TIMEOUT: Duration = Duration::from_secs(10);

/// What an executor claims: one base branch of one forge repository.
pub fn scope(repository: &str, base: &str) -> String {
    format!("integration/{repository}/{base}")
}

/// The mesh session every executor of one checkout and base claims as. The same each time,
/// so that a claim a killed executor left behind is recognisably this checkout's own.
pub fn session(root: &Path, base: &str) -> String {
    use sha2::{Digest, Sha256};
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let digest = Sha256::digest(format!("{}\0{base}", root.display()));
    format!("integration-{}", &format!("{digest:x}")[..12])
}

/// The server a claim is asked of: this checkout's own ([`ServerMesh`]), or a test's.
pub trait Mesh: Send {
    /// `method target` with a JSON body: the HTTP status and the JSON answer.
    fn ask(&self, method: &str, target: &str, body: Option<Value>) -> Result<(u16, Value), String>;
}

/// This checkout's shared server, over HTTP.
pub struct ServerMesh {
    url: String,
}

impl ServerMesh {
    /// The server at `url`.
    pub fn at(url: impl Into<String>) -> Self {
        ServerMesh { url: url.into() }
    }
}

impl Mesh for ServerMesh {
    fn ask(&self, method: &str, target: &str, body: Option<Value>) -> Result<(u16, Value), String> {
        let payload = body.map(|b| b.to_string());
        let reply = crate::mcp::bridge::request(
            &self.url,
            method,
            target,
            &[("Content-Type", "application/json")],
            payload.as_deref(),
            TIMEOUT,
        )
        .map_err(|e| format!("{}{target}: {e}", self.url))?;
        let value = serde_json::from_str(&reply.body).map_err(|e| {
            format!(
                "{}{target}: status {}, not JSON: {e}",
                self.url, reply.status
            )
        })?;
        Ok((reply.status, value))
    }
}

/// This checkout's shared server, when one is ready to answer for it.
pub fn server_of(root: &Path) -> Option<ServerMesh> {
    use crate::capability::builtin::server::{local_half, standing_at, ServerStanding};
    let (standing, _, lease) = standing_at(root, &local_half(root));
    let url = lease.document().and_then(|d| d.url.clone());
    url.filter(|_| standing == ServerStanding::Ready)
        .map(ServerMesh::at)
}

/// An exclusive mesh claim, released when dropped.
pub struct MeshClaim {
    mesh: Box<dyn Mesh>,
    key: String,
    session: String,
    /// The mesh said the claim ended — released, or no longer known — so the drop has
    /// nothing to give back.
    ended: AtomicBool,
    /// A refused beat was said once; the rest are not, so an executor of a mesh with the
    /// progress rule off does not say it at every step.
    beat_refusal_said: AtomicBool,
}

impl std::fmt::Debug for MeshClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshClaim")
            .field("key", &self.key)
            .field("session", &self.session)
            .finish()
    }
}

impl MeshClaim {
    fn of(mesh: Box<dyn Mesh>, key: String, session: &str) -> Self {
        MeshClaim {
            mesh,
            key,
            session: session.to_string(),
            ended: AtomicBool::new(false),
            beat_refusal_said: AtomicBool::new(false),
        }
    }

    /// `<stream>/<claim>`, as the mesh answered it.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Release every other claim this checkout's session still holds: what an executor of
    /// this checkout left behind when it was killed. Called only by the holder of the
    /// clone's lease, so no live executor of this checkout holds any of them. Best effort:
    /// what could not be released is said and keeps excluding until it is.
    pub fn release_leftovers(&self) {
        // the mesh keys a session by the stream that wrote it, as it keys the claim: this
        // runtime's stream is the one this claim was written to
        let own = match self.key.rsplit_once('/') {
            Some((stream, _)) => format!("{stream}/{}", self.session),
            None => self.session.clone(),
        };
        let held: Vec<String> = match live_claims(self.mesh.as_ref()) {
            Ok(claims) => claims
                .iter()
                .filter(|c| c["state"]["state"] == json!("held"))
                .filter(|c| c["session"] == json!(own))
                .filter_map(|c| c["key"].as_str())
                .filter(|k| *k != self.key)
                .map(str::to_string)
                .collect(),
            Err(e) => {
                eprintln!("majordomus: the mesh's claims could not be read: {e}");
                return;
            }
        };
        for key in held {
            if let Err(e) = release(self.mesh.as_ref(), &key) {
                eprintln!("majordomus: a leftover integration claim {key} stays held: {e}");
            }
        }
    }

    /// Beat this claim's session with `phase`: the executor advanced its queue (I3991). The
    /// mesh writes at most one beat a quarter of its ttl, so this is cheap to ask. A mesh that
    /// refuses it — the progress rule off, a server older than the beat — keeps the claim by
    /// its stream alone, as before: said once, and never a reason to stop.
    pub(crate) fn beat(&self, phase: &str) {
        let body = json!({
            "session": self.session,
            "phase": phase,
            "pid": std::process::id(),
        });
        let refused = match self.mesh.ask("POST", "/api/v1/mesh/beats", Some(body)) {
            Ok((200, _)) => return,
            Ok((status, v)) => format!("status {status}: {v}"),
            Err(e) => e,
        };
        if !self.beat_refusal_said.swap(true, Ordering::SeqCst) {
            eprintln!(
                "majordomus: the integration claim {} was not beaten ({refused}); it lives by its \
                 runtime's beat alone",
                self.key
            );
        }
    }

    /// Whether this claim still holds its scope, asked before `act` reaches the forge. It
    /// beats first, so that the mesh re-arms a claim suspended while nobody was admitted over
    /// it, and releases one another session was admitted over meanwhile
    /// ([`crate::mesh::cooperation::OVERTAKEN`]); then it reads the claim. Held passes;
    /// suspended, conflicted, released, expired or unknown is refused, and so is a mesh that
    /// cannot be read: this checkout's server not answering is its runtime not beating, and
    /// its claims expire on every other machine.
    pub(crate) fn holds(&self, act: &str) -> Result<(), String> {
        self.beat(act);
        let claims = claims_of(
            self.mesh.as_ref(),
            &format!("/api/v1/mesh/state?view=full&session={}", self.session),
        )
        .map_err(|e| {
            format!(
                "the integration claim {} could not be read on the mesh: {e}",
                self.key
            )
        })?;
        let Some(claim) = claims.iter().find(|c| c["key"] == json!(self.key)) else {
            self.ended.store(true, Ordering::SeqCst);
            return Err(format!(
                "the mesh no longer knows the integration claim {}",
                self.key
            ));
        };
        let state = claim["state"]["state"].as_str().unwrap_or("unreadable");
        if state == "held" {
            return Ok(());
        }
        if state == "released" {
            self.ended.store(true, Ordering::SeqCst);
        }
        let why = claim["state"]["detail"]
            .as_str()
            .map(|d| format!(" ({d})"))
            .unwrap_or_default();
        Err(format!(
            "the integration claim {} is {state} on the mesh{why}: another executor may hold \
             its scope now",
            self.key
        ))
    }
}

/// The mesh's live claims, from the full view of its state.
fn live_claims(mesh: &dyn Mesh) -> Result<Vec<Value>, String> {
    claims_of(mesh, "/api/v1/mesh/state?view=full&claims=live")
}

/// The claims of the state `query` asks for. A server older than the full view refuses the
/// query as invalid input and answers the whole state when asked bare, so the one refusal is
/// asked again without it: every claim asked about is in the whole state too.
fn claims_of(mesh: &dyn Mesh, query: &str) -> Result<Vec<Value>, String> {
    let answer = match mesh.ask("GET", query, None)? {
        (400, _) => mesh.ask("GET", "/api/v1/mesh/state", None)?,
        other => other,
    };
    match answer {
        (200, v) => Ok(v["state"]["claims"].as_array().cloned().unwrap_or_default()),
        (status, v) => Err(format!("status {status}: {v}")),
    }
}

fn release(mesh: &dyn Mesh, key: &str) -> Result<(), String> {
    match mesh.ask(
        "POST",
        "/api/v1/mesh/claims/release",
        Some(json!({ "claim": key })),
    )? {
        (200, _) => Ok(()),
        (status, v) => Err(format!("status {status}: {v}")),
    }
}

impl Drop for MeshClaim {
    fn drop(&mut self) {
        if self.ended.load(Ordering::SeqCst) {
            return;
        }
        if let Err(e) = release(self.mesh.as_ref(), &self.key) {
            // a destructor cannot refuse; it says so where a person reads
            eprintln!(
                "majordomus: the integration claim {} was not released and excludes other \
                 machines until it is (majordomus mesh release {}): {e}",
                self.key, self.key
            );
        }
    }
}

/// How far an executor's exclusivity reaches.
#[derive(Debug)]
pub enum Reach {
    /// Every machine of the repository's mesh: the claim it holds.
    Repository(MeshClaim),
    /// This clone alone: no mesh runs for this checkout.
    CloneOnly,
}

/// Take the exclusive claim on `scope` as `session` from `mesh`.
///
/// A mesh that is not running here is no refusal — the lease still guards the clone, and
/// [`Reach::CloneOnly`] says it guards no more. Another session's live claim is: the
/// refusal names its key, its session and what it said it was for.
pub fn claim(
    mesh: Box<dyn Mesh>,
    session: &str,
    scope: &str,
    intent: &str,
) -> Result<Reach, String> {
    let body = json!({
        "session": session,
        "client": "majordomus prs",
        "scope": [scope],
        "intent": intent,
        "mode": "exclusive",
    });
    let (status, answer) = mesh.ask("POST", "/api/v1/mesh/claims", Some(body))?;
    if status == 200 {
        let key = answer["key"]
            .as_str()
            .ok_or_else(|| format!("the mesh answered a claim without a key: {answer}"))?
            .to_string();
        return Ok(Reach::Repository(MeshClaim::of(mesh, key, session)));
    }
    let message = answer["error"]["message"].as_str().unwrap_or_default();
    if message.starts_with("not_active") {
        return Ok(Reach::CloneOnly);
    }
    if message.starts_with("claim_conflict") {
        return Err(format!(
            "another integration executor holds {scope} on the mesh{}: {message}",
            holder(mesh.as_ref(), scope)
        ));
    }
    Err(format!(
        "the mesh refused the integration claim on {scope} (status {status}): {answer}"
    ))
}

/// ", held by session S (intent)" for the live claims on `scope`, read from the mesh's
/// state; empty when the state cannot be read, the refusal itself still naming the key.
fn holder(mesh: &dyn Mesh, scope: &str) -> String {
    let Ok(claims) = live_claims(mesh) else {
        return String::new();
    };
    let named: Vec<String> = claims
        .iter()
        .filter(|c| c["state"]["state"] == json!("held"))
        .filter(|c| {
            c["scope"]
                .as_array()
                .is_some_and(|s| s.contains(&json!(scope)))
        })
        .map(|c| {
            format!(
                "session {} ({})",
                c["session"].as_str().unwrap_or("?"),
                c["intent"].as_str().unwrap_or("no intent given")
            )
        })
        .collect();
    if named.is_empty() {
        String::new()
    } else {
        format!(", held by {}", named.join("; "))
    }
}

/// Take the lease of `base` for this checkout's executor, with the repository-wide claim
/// when this checkout's server runs the mesh. The one way an executor (`prs drain`,
/// `cleanup --apply`) takes its lease.
pub fn acquire(root: &Path, base: &str) -> Result<IntegrationLease, String> {
    acquire_through(
        root,
        base,
        server_of(root).map(|m| Box::new(m) as Box<dyn Mesh>),
    )
}

/// [`acquire`], asking `mesh` (or none) rather than finding this checkout's server.
pub fn acquire_through(
    root: &Path,
    base: &str,
    mesh: Option<Box<dyn Mesh>>,
) -> Result<IntegrationLease, String> {
    let reach = match mesh {
        None => Reach::CloneOnly,
        Some(mesh) => {
            let repository = super::load_observation(root)
                .ok()
                .flatten()
                .map(|o| o.repository)
                .unwrap_or_default();
            let intent = format!(
                "the integration executor of {} (pid {}) on {}",
                root.display(),
                std::process::id(),
                super::drain::host()
            );
            claim(
                mesh,
                &session(root, base),
                &scope(&repository, base),
                &intent,
            )?
        }
    };
    match reach {
        Reach::Repository(claim) => {
            let lease = IntegrationLease::acquire_holding(root, base, Some(claim))?;
            // the lease holds the claim it was given, so this releases exactly once
            lease.mesh_claim().into_iter().for_each(|claim| {
                claim.release_leftovers();
                // from its first beat the claim lives by the executor's progress (I3991)
                claim.beat("took the integration lease");
            });
            Ok(lease)
        }
        Reach::CloneOnly => IntegrationLease::acquire(root, base),
    }
}

// ------------------------------------------------------------------------------------------
// The executor's state on origin (I3911)
// ------------------------------------------------------------------------------------------

/// The ref on origin the executor's record is appended to: every verified landing
/// (`merge_succeeded`), every landing that could not be verified (`verification_failed`, the
/// halt) and every acknowledgement of one (`failure_acknowledged`), one commit each, its
/// message the event's JSON line, each the child of the one before (I3911). A clone that
/// never ran an executor reads the same record as the clone that wrote it.
pub const TRAIL_REF: &str = "refs/majordomus/integration/trail";

/// Where this clone keeps [`TRAIL_REF`] as it last fetched or pushed it: what
/// [`super::drain::events`] reads beside the clone's own trail, without asking the network.
pub const TRAIL_MIRROR: &str = "refs/majordomus/integration-origin/trail";

/// How long a lease on origin holds without being renewed: the age at which the clone's own
/// lease record is called stale ([`super::drain::LEASE_STALE_AFTER`]).
pub const ORIGIN_LEASE_TTL: Duration = super::drain::LEASE_STALE_AFTER;

/// The ref on origin the lease of `base` is recorded at.
pub fn lease_ref(base: &str) -> String {
    format!("refs/majordomus/integration/lease/{base}")
}

/// `git` in `root` with a fixed identity, so a commit of the executor's state is made on a
/// machine that has none configured. The standard output, or what git said.
fn git_at(root: &Path, args: &[&str], input: Option<&str>) -> Result<String, String> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_AUTHOR_NAME", "majordomus integration")
        .env("GIT_AUTHOR_EMAIL", "integration@majordomus.invalid")
        .env("GIT_COMMITTER_NAME", "majordomus integration")
        .env("GIT_COMMITTER_EMAIL", "integration@majordomus.invalid")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("MAJORDOMUS_SHARE")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git could not run: {e}"))?;
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Whether the repository at `root` has an `origin` to keep the executor's state on.
pub fn has_origin(root: &Path) -> bool {
    git_at(root, &["remote", "get-url", "origin"], None).is_ok()
}

/// The commit `name` points at on origin, fetched into this clone so it can be read; `None`
/// when origin has no such ref.
fn origin_tip(root: &Path, name: &str) -> Result<Option<String>, String> {
    let listed = git_at(root, &["ls-remote", "origin", name], None)?;
    let Some(tip) = listed
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .find(|(_, r)| *r == name)
        .map(|(sha, _)| sha.to_string())
    else {
        return Ok(None);
    };
    if git_at(
        root,
        &["cat-file", "-e", &format!("{tip}^{{commit}}")],
        None,
    )
    .is_err()
    {
        git_at(
            root,
            &[
                "fetch",
                "-q",
                "--no-tags",
                "--no-write-fetch-head",
                "origin",
                name,
            ],
            None,
        )?;
    }
    Ok(Some(tip))
}

/// A commit of the empty tree whose message is `message`, the child of `parent`.
fn state_commit(root: &Path, parent: Option<&str>, message: &str) -> Result<String, String> {
    let tree = git_at(
        root,
        &["hash-object", "-t", "tree", "-w", "--stdin"],
        Some(""),
    )?;
    let mut args = vec!["commit-tree", tree.as_str(), "-F", "-"];
    if let Some(p) = parent {
        args.extend(["-p", p]);
    }
    git_at(root, &args, Some(message))
}

/// Move `name` on origin to `commit`, never forced: `Ok(false)` when origin refused it
/// because the ref moved since it was read — another clone wrote first.
fn push_state(root: &Path, commit: &str, name: &str) -> Result<bool, String> {
    match git_at(
        root,
        &["push", "-q", "origin", &format!("{commit}:{name}")],
        None,
    ) {
        Ok(_) => Ok(true),
        Err(e)
            if [
                "rejected",
                "non-fast-forward",
                "fetch first",
                "stale info",
                "failed to update",
            ]
            .iter()
            .any(|w| e.contains(w)) =>
        {
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

/// The message of `commit`.
fn message_of(root: &Path, commit: &str) -> Result<String, String> {
    git_at(root, &["log", "-1", "--format=%B", commit], None)
}

/// Append `line` — one event of the trail, as JSON — to [`TRAIL_REF`] on origin and to this
/// clone's [`TRAIL_MIRROR`]. A clone that wrote meanwhile makes the push a non-fast-forward;
/// the line is then appended to what it wrote, a few times at most.
pub fn publish_trail_line(root: &Path, line: &str) -> Result<(), String> {
    let mut last = String::new();
    for _ in 0..5 {
        let tip = origin_tip(root, TRAIL_REF)?;
        let commit = state_commit(root, tip.as_deref(), line)?;
        if push_state(root, &commit, TRAIL_REF)? {
            git_at(root, &["update-ref", TRAIL_MIRROR, &commit], None)?;
            return Ok(());
        }
        last = format!("origin refused {TRAIL_REF} as no fast-forward");
    }
    Err(last)
}

/// Fetch [`TRAIL_REF`] into this clone's [`TRAIL_MIRROR`]: `Ok(false)` when origin has none.
pub fn sync_trail(root: &Path) -> Result<bool, String> {
    let Some(tip) = origin_tip(root, TRAIL_REF)? else {
        return Ok(false);
    };
    git_at(root, &["update-ref", TRAIL_MIRROR, &tip], None)?;
    Ok(true)
}

/// The lines of this clone's [`TRAIL_MIRROR`], oldest first; nothing when it has none. Reads
/// no network.
pub fn mirrored_trail_lines(root: &Path) -> Vec<String> {
    git_at(
        root,
        &["log", "--reverse", "--format=%x1e%B", TRAIL_MIRROR, "--"],
        None,
    )
    .map(|text| {
        text.split('\u{1e}')
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

/// Whether this clone has a [`TRAIL_MIRROR`] at all.
pub fn has_trail_mirror(root: &Path) -> bool {
    git_at(root, &["rev-parse", "-q", "--verify", TRAIL_MIRROR], None).is_ok()
}

/// What the lease ref on origin says: who holds it, from where, and until when.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OriginLeaseRecord {
    /// The holder: one executor's own token, never reused.
    pub holder: String,
    /// The machine.
    pub host: String,
    /// The clone, by its common git directory: an unexpired lease of this same clone is a
    /// leftover of an executor that ended, since this clone's `flock` is held by now.
    pub clone: String,
    /// The process.
    pub pid: u32,
    /// The base branch.
    pub base: String,
    /// Seconds since the epoch after which another clone may take the lease over; 0 once
    /// released.
    pub expires_at: u64,
}

/// The executor's lease of a base on origin (I3911): a commit on [`lease_ref`] naming its
/// holder, its host and its expiry, moved by non-forced pushes only. It is taken when its
/// ref is absent, released, expired, or this clone's own leftover; renewed before every write
/// to the forge by a commit on top of the one this holder pushed last; and a holder whose
/// commit is no longer the ref's — it slept past its expiry and another clone took over — is
/// refused at that renewal, so it lands nothing.
pub struct OriginLease {
    root: std::path::PathBuf,
    name: String,
    record: OriginLeaseRecord,
    ttl: u64,
    /// The commit this holder pushed last: what the ref must still be for it to hold.
    tip: std::sync::Mutex<String>,
    /// When the record this holder pushed last expires, seconds since the epoch.
    expires: std::sync::atomic::AtomicU64,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl OriginLease {
    /// Take the lease of `base` on origin now, for [`ORIGIN_LEASE_TTL`]; `Ok(None)` when the
    /// repository has no origin, where the clone's lease alone guards.
    pub fn take(root: &Path, base: &str) -> Result<Option<Self>, String> {
        if !has_origin(root) {
            return Ok(None);
        }
        Self::take_at(root, base, now_secs(), ORIGIN_LEASE_TTL.as_secs()).map(Some)
    }

    /// [`take`](Self::take) at `now`, holding for `ttl` seconds: refused, naming the holder,
    /// while another clone's lease has not expired.
    pub fn take_at(root: &Path, base: &str, now: u64, ttl: u64) -> Result<Self, String> {
        let name = lease_ref(base);
        let clone = super::drain::common_dir(root)?.display().to_string();
        let host = super::drain::host();
        let tip = origin_tip(root, &name)?;
        if let Some(tip) = &tip {
            let held: Option<OriginLeaseRecord> =
                serde_json::from_str(message_of(root, tip)?.trim()).ok();
            if let Some(h) = held.filter(|h| h.expires_at > now) {
                if !(h.host == host && h.clone == clone) {
                    return Err(format!(
                        "another integration executor holds {name} on origin: {} (pid {}) on \
                         {}, until {} ({} s from now); it is taken over only after that",
                        h.holder,
                        h.pid,
                        h.host,
                        h.expires_at,
                        h.expires_at - now
                    ));
                }
            }
        }
        let record = OriginLeaseRecord {
            holder: format!(
                "{host}/{}/{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ),
            host,
            clone,
            pid: std::process::id(),
            base: base.to_string(),
            expires_at: now + ttl,
        };
        let lease = OriginLease {
            root: root.to_path_buf(),
            name,
            record,
            ttl,
            tip: std::sync::Mutex::new(String::new()),
            expires: std::sync::atomic::AtomicU64::new(now + ttl),
        };
        let commit = lease.write(tip.as_deref(), now + ttl)?;
        if !push_state(root, &commit, &lease.name)? {
            return Err(format!(
                "another integration executor took {} on origin meanwhile",
                lease.name
            ));
        }
        *lease
            .tip
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = commit;
        Ok(lease)
    }

    /// The record this holder writes, as a commit on `parent`, expiring at `expires_at`.
    fn write(&self, parent: Option<&str>, expires_at: u64) -> Result<String, String> {
        let record = OriginLeaseRecord {
            expires_at,
            ..self.record.clone()
        };
        let text = serde_json::to_string(&record).map_err(|e| e.to_string())?;
        state_commit(&self.root, parent, &text)
    }

    /// Renew the lease now, for its ttl.
    pub fn renew(&self) -> Result<(), String> {
        self.renew_at(now_secs())
    }

    /// Renew the lease at `now`: refused when the ref on origin is no longer the commit this
    /// holder pushed last — another clone took it over — or when origin refuses the push.
    pub fn renew_at(&self, now: u64) -> Result<(), String> {
        let mut tip = self
            .tip
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let on_origin = origin_tip(&self.root, &self.name)
            .map_err(|e| format!("the integration lease on origin could not be read: {e}"))?;
        if on_origin.as_deref() != Some(tip.as_str()) {
            let by = on_origin
                .as_deref()
                .and_then(|t| message_of(&self.root, t).ok())
                .unwrap_or_else(|| "nobody".to_string());
            return Err(format!(
                "the integration lease was lost: {} on origin was taken over by {by}",
                self.name
            ));
        }
        let commit = self.write(Some(tip.as_str()), now + self.ttl)?;
        if !push_state(&self.root, &commit, &self.name)? {
            return Err(format!(
                "the integration lease was lost: origin refused the renewal of {}",
                self.name
            ));
        }
        *tip = commit;
        self.expires
            .store(now + self.ttl, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    /// Renew the lease when less than half its ttl is left, and otherwise ask origin nothing:
    /// what a drain does between its steps, so an idle executor keeps its lease without a
    /// push per observation.
    pub fn renew_if_due(&self) -> Result<(), String> {
        let now = now_secs();
        if self.expires.load(std::sync::atomic::Ordering::SeqCst) > now + self.ttl / 2 {
            return Ok(());
        }
        self.renew_at(now)
    }

    /// Who holds the lease of `base` on origin, read without taking it; `None` when origin has
    /// no lease ref, or no origin.
    #[cfg(test)]
    pub fn read(root: &Path, base: &str) -> Result<Option<OriginLeaseRecord>, String> {
        if !has_origin(root) {
            return Ok(None);
        }
        let Some(tip) = origin_tip(root, &lease_ref(base))? else {
            return Ok(None);
        };
        Ok(serde_json::from_str(message_of(root, &tip)?.trim()).ok())
    }

    /// The record as this holder took it.
    #[cfg(test)]
    pub fn record(&self) -> &OriginLeaseRecord {
        &self.record
    }
}

impl Drop for OriginLease {
    fn drop(&mut self) {
        // released by a record that expired at 0, on top of this holder's own commit, so the
        // next executor anywhere takes it at once; a lease taken over meanwhile is left alone
        let tip = self
            .tip
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let released = self
            .write(Some(tip.as_str()), 0)
            .and_then(|c| push_state(&self.root, &c, &self.name));
        if let Err(e) = released {
            eprintln!(
                "majordomus: the integration lease on origin was not released and expires on \
                 its own: {e}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    type Answer = Result<(u16, Value), String>;
    type Asked = (String, String, Option<Value>);

    /// A mesh that answers from a script and remembers what it was asked.
    #[derive(Clone)]
    struct Scripted {
        answers: Arc<Mutex<Vec<Answer>>>,
        asked: Arc<Mutex<Vec<Asked>>>,
    }

    impl Scripted {
        fn new(answers: Vec<Result<(u16, Value), String>>) -> Self {
            Scripted {
                answers: Arc::new(Mutex::new(answers)),
                asked: Arc::new(Mutex::new(Vec::new())),
            }
        }
        fn asked(&self) -> Vec<(String, String, Option<Value>)> {
            self.asked.lock().unwrap().clone()
        }
    }

    impl Mesh for Scripted {
        fn ask(
            &self,
            method: &str,
            target: &str,
            body: Option<Value>,
        ) -> Result<(u16, Value), String> {
            self.asked
                .lock()
                .unwrap()
                .push((method.into(), target.into(), body));
            let mut answers = self.answers.lock().unwrap();
            if answers.is_empty() {
                return Ok((200, json!({})));
            }
            answers.remove(0)
        }
    }

    fn refused(message: &str) -> Result<(u16, Value), String> {
        Ok((
            422,
            json!({ "error": { "code": "refused", "message": message } }),
        ))
    }

    fn state(claims: Value) -> Result<(u16, Value), String> {
        Ok((200, json!({ "state": { "claims": claims } })))
    }

    #[test]
    fn the_scope_names_the_repository_and_the_base_and_the_session_is_the_checkouts() {
        assert_eq!(scope("o/r", "master"), "integration/o/r/master");
        let here = tempfile::tempdir().unwrap();
        let s = session(here.path(), "master");
        assert!(
            s.starts_with("integration-") && s.len() == "integration-".len() + 12,
            "{s}"
        );
        assert_eq!(
            s,
            session(here.path(), "master"),
            "the same checkout, the same session"
        );
        assert_ne!(
            s,
            session(here.path(), "release"),
            "another base, another session"
        );
        // a path that does not exist (yet) is named as given rather than refused
        let nowhere = here.path().join("not-there");
        assert_ne!(session(&nowhere, "master"), s);
    }

    #[test]
    fn a_granted_claim_is_held_and_released_when_dropped() {
        let mesh = Scripted::new(vec![Ok((200, json!({ "key": "s1/c-1" })))]);
        let reach = claim(
            Box::new(mesh.clone()),
            "integration-x",
            "integration/o/r/master",
            "why",
        )
        .expect("granted");
        let Reach::Repository(held) = reach else {
            panic!("a granted claim reaches the repository");
        };
        assert_eq!(held.key(), "s1/c-1");
        assert!(format!("{held:?}").contains("s1/c-1"));
        drop(held);
        let asked = mesh.asked();
        assert_eq!(asked[0].1, "/api/v1/mesh/claims");
        let body = asked[0].2.clone().unwrap();
        assert_eq!(body["mode"], "exclusive");
        assert_eq!(body["scope"], json!(["integration/o/r/master"]));
        assert_eq!(asked[1].1, "/api/v1/mesh/claims/release");
        assert_eq!(asked[1].2.clone().unwrap()["claim"], "s1/c-1");
    }

    #[test]
    fn a_mesh_that_is_not_running_guards_nothing_beyond_the_clone_and_refuses_nothing() {
        let mesh = Scripted::new(vec![refused("not_active: the mesh is not declared")]);
        let reach = claim(Box::new(mesh), "s", "integration/o/r/master", "why").unwrap();
        assert!(
            matches!(reach, Reach::CloneOnly),
            "an inactive mesh holds no claim"
        );
    }

    #[test]
    fn another_machines_claim_is_a_refusal_that_names_its_holder() {
        let mesh = Scripted::new(vec![
            refused("claim_conflict: the scope is claimed: s2/c-9 (integration/o/r/master)"),
            state(json!([
                { "key": "s2/c-9", "session": "integration-abc", "intent": "the executor on b",
                  "scope": ["integration/o/r/master"], "state": { "state": "held" } },
                { "key": "s2/c-8", "session": "integration-old", "intent": "released",
                  "scope": ["integration/o/r/master"], "state": { "state": "released" } },
                { "key": "s3/c-1", "session": "someone", "scope": ["lib"],
                  "state": { "state": "held" } },
            ])),
        ]);
        let e = claim(
            Box::new(mesh),
            "integration-me",
            "integration/o/r/master",
            "why",
        )
        .expect_err("refused");
        assert!(e.contains("s2/c-9"), "{e}");
        assert!(
            e.contains("held by session integration-abc (the executor on b)"),
            "{e}"
        );
        assert!(
            !e.contains("integration-old") && !e.contains("someone"),
            "{e}"
        );

        // a state that cannot be read still refuses, with the key the refusal named
        let mesh = Scripted::new(vec![
            refused("claim_conflict: the scope is claimed: s2/c-9 (integration/o/r/master)"),
            Err("connection reset".into()),
        ]);
        let e = claim(
            Box::new(mesh),
            "integration-me",
            "integration/o/r/master",
            "why",
        )
        .expect_err("refused");
        assert!(e.contains("s2/c-9") && !e.contains("held by"), "{e}");

        // a held claim that states no intent is still named
        let mesh = Scripted::new(vec![
            refused("claim_conflict: the scope is claimed: s2/c-9 (integration/o/r/master)"),
            state(
                json!([{ "key": "s2/c-9", "scope": ["integration/o/r/master"],
                           "state": { "state": "held" } }]),
            ),
        ]);
        let e = claim(Box::new(mesh), "me", "integration/o/r/master", "why").unwrap_err();
        assert!(e.contains("session ? (no intent given)"), "{e}");
    }

    #[test]
    fn any_other_answer_is_a_refusal_and_never_taken_for_a_claim() {
        let mesh = Scripted::new(vec![Ok((500, json!({ "error": "boom" })))]);
        let e = claim(Box::new(mesh), "s", "integration/o/r/master", "why").unwrap_err();
        assert!(e.contains("status 500"), "{e}");

        let mesh = Scripted::new(vec![Ok((200, json!({ "event": "s1/7" })))]);
        let e = claim(Box::new(mesh), "s", "integration/o/r/master", "why").unwrap_err();
        assert!(e.contains("without a key"), "{e}");

        let mesh = Scripted::new(vec![Err("connection refused".into())]);
        let e = claim(Box::new(mesh), "s", "integration/o/r/master", "why").unwrap_err();
        assert!(e.contains("connection refused"), "{e}");
    }

    #[test]
    fn leftovers_of_this_checkouts_session_are_released_and_nothing_else() {
        let mesh = Scripted::new(vec![
            state(json!([
                { "key": "s1/c-new", "session": "s1/integration-me",
                  "state": { "state": "held" } },
                { "key": "s1/c-old", "session": "s1/integration-me",
                  "state": { "state": "held" } },
                { "key": "s1/c-gone", "session": "s1/integration-me",
                  "state": { "state": "released" } },
                // the same session name on another machine is another machine's session
                { "key": "s2/c-x", "session": "s2/integration-me",
                  "state": { "state": "held" } },
            ])),
            Ok((200, json!({}))),
        ]);
        let held = MeshClaim::of(Box::new(mesh.clone()), "s1/c-new".into(), "integration-me");
        held.release_leftovers();
        let released: Vec<Value> = mesh
            .asked()
            .into_iter()
            .filter(|a| a.1 == "/api/v1/mesh/claims/release")
            .filter_map(|a| a.2)
            .map(|b| b["claim"].clone())
            .collect();
        assert_eq!(released, [json!("s1/c-old")]);
        std::mem::forget(held);

        // what cannot be read or released is said, and nothing is released blindly
        let mesh = Scripted::new(vec![Err("down".into())]);
        let held = MeshClaim::of(Box::new(mesh.clone()), "k".into(), "integration-me");
        held.release_leftovers();
        assert_eq!(
            mesh.asked().len(),
            1,
            "a state it could not read releases nothing"
        );
        std::mem::forget(held);

        let mesh = Scripted::new(vec![
            state(json!([{ "key": "old", "session": "integration-me",
                           "state": { "state": "held" } }])),
            Ok((422, json!({ "error": { "message": "not_own: elsewhere" } }))),
            // the drop's own release, also refused: said, not raised
            Err("down".into()),
        ]);
        let held = MeshClaim::of(Box::new(mesh.clone()), "k".into(), "integration-me");
        held.release_leftovers();
        drop(held);
        assert_eq!(mesh.asked().len(), 3);
    }

    #[test]
    fn the_claims_are_read_from_the_full_view_and_asked_again_bare_of_an_older_server() {
        let held = json!([{ "key": "s/c1", "session": "x", "scope": ["integration/o/r/master"],
                            "intent": "land", "state": { "state": "held" } }]);
        let mesh = Scripted::new(vec![state(held.clone())]);
        assert_eq!(
            live_claims(&mesh).unwrap(),
            held.as_array().unwrap().clone()
        );
        assert_eq!(
            mesh.asked()[0].1,
            "/api/v1/mesh/state?view=full&claims=live"
        );

        // a server that predates the view refuses the query, and answers the whole state bare
        let older = Scripted::new(vec![
            Ok((400, json!({ "error": { "code": "invalid_input" } }))),
            state(held.clone()),
        ]);
        assert!(holder(&older, "integration/o/r/master").contains("session x (land)"));
        let asked: Vec<String> = older.asked().into_iter().map(|(_, t, _)| t).collect();
        assert_eq!(
            asked,
            [
                "/api/v1/mesh/state?view=full&claims=live",
                "/api/v1/mesh/state"
            ]
        );

        // any other answer is the reason, not an empty list
        let broken = Scripted::new(vec![Ok((503, json!({ "error": "busy" })))]);
        assert!(live_claims(&broken).unwrap_err().contains("status 503"));

        // an older server that refuses the view and then cannot be asked bare says why
        let gone = Scripted::new(vec![
            Ok((400, json!({ "error": { "code": "invalid_input" } }))),
            Err("connection reset".into()),
        ]);
        assert_eq!(live_claims(&gone).unwrap_err(), "connection reset");
        assert_eq!(gone.asked().len(), 2);
    }

    #[test]
    fn the_lease_carries_the_claim_it_was_taken_with_and_gives_it_back_after_itself() {
        let repo = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(args)
                .status()
                .unwrap()
                .success());
        };
        git(&["init", "-q", "-b", "master"]);
        let mesh = Scripted::new(vec![
            Ok((200, json!({ "key": "s1/c-1" }))),
            state(json!([])),
        ]);
        let lease = acquire_through(repo.path(), "master", Some(Box::new(mesh.clone())))
            .expect("the lease and the claim");
        let read = IntegrationLease::read(repo.path(), "master")
            .unwrap()
            .unwrap();
        assert_eq!(read.holder.unwrap().mesh_claim.as_deref(), Some("s1/c-1"));
        let claim_body = mesh.asked()[0].2.clone().unwrap();
        // nothing observed: the repository is unnamed rather than guessed
        assert_eq!(claim_body["scope"], json!(["integration//master"]));
        assert!(claim_body["intent"]
            .as_str()
            .unwrap()
            .contains("integration executor"));

        // a second executor of this clone is refused by the lease, and its claim — admitted,
        // being this checkout's session — is given back as it is refused
        let second = Scripted::new(vec![Ok((200, json!({ "key": "s1/c-2" })))]);
        let e = acquire_through(repo.path(), "master", Some(Box::new(second.clone())))
            .err()
            .expect("the clone's lease refuses");
        assert!(e.contains("another integration executor holds"), "{e}");
        assert_eq!(second.asked()[1].2.clone().unwrap()["claim"], "s1/c-2");

        drop(lease);
        assert!(IntegrationLease::read(repo.path(), "master")
            .unwrap()
            .is_none());
        let last = mesh.asked().pop().unwrap();
        assert_eq!(last.1, "/api/v1/mesh/claims/release");

        // another machine's claim refuses before the lease is taken
        let elsewhere = Scripted::new(vec![refused("claim_conflict: the scope is claimed: x")]);
        let refused_here = acquire_through(repo.path(), "master", Some(Box::new(elsewhere)));
        assert!(refused_here.is_err(), "refused across machines");
        assert!(IntegrationLease::read(repo.path(), "master")
            .unwrap()
            .is_none());

        // without a mesh, or with one not running, the lease alone guards
        let lease = acquire_through(repo.path(), "master", None).unwrap();
        let read = IntegrationLease::read(repo.path(), "master")
            .unwrap()
            .unwrap();
        assert!(read.holder.unwrap().mesh_claim.is_none());
        drop(lease);
        let off = Scripted::new(vec![refused("not_active: off")]);
        let lease = acquire_through(repo.path(), "master", Some(Box::new(off))).unwrap();
        assert!(lease.mesh_claim().is_none());
        drop(lease);
        // and with no server for a checkout, `acquire` finds none
        assert!(server_of(repo.path()).is_none());
        drop(acquire(repo.path(), "master").unwrap());
    }

    /// A git repository to take a lease in.
    fn repository() -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        assert!(std::process::Command::new("git")
            .args(["init", "-q", "-b", "master"])
            .arg(repo.path())
            .status()
            .unwrap()
            .success());
        repo
    }

    fn beats(mesh: &Scripted) -> Vec<Value> {
        mesh.asked()
            .into_iter()
            .filter(|(_, t, _)| t == "/api/v1/mesh/beats")
            .filter_map(|(_, _, b)| b)
            .collect()
    }

    #[test]
    fn a_granted_claim_beats_once_as_it_is_taken() {
        let repo = repository();
        let mesh = Scripted::new(vec![
            Ok((200, json!({ "key": "s1/c-1" }))),
            state(json!([])),
            Ok((200, json!({ "key": "s1/integration-x", "event": "s1/3" }))),
        ]);
        let lease = acquire_through(repo.path(), "master", Some(Box::new(mesh.clone()))).unwrap();
        let beaten = beats(&mesh);
        assert_eq!(beaten.len(), 1, "{beaten:?}");
        assert_eq!(beaten[0]["session"], json!(session(repo.path(), "master")));
        assert_eq!(beaten[0]["phase"], "took the integration lease");
        assert_eq!(beaten[0]["pid"], json!(std::process::id()));
        // the first beat comes after the leftovers were read: the claim is the lease's by then
        let order: Vec<String> = mesh.asked().into_iter().map(|(_, t, _)| t).collect();
        assert_eq!(
            order[..3],
            [
                "/api/v1/mesh/claims",
                "/api/v1/mesh/state?view=full&claims=live",
                "/api/v1/mesh/beats"
            ]
        );

        // each advance of the queue beats again, with what the executor did
        lease.progressed("merge_succeeded #7");
        assert_eq!(beats(&mesh).last().unwrap()["phase"], "merge_succeeded #7");
        drop(lease);
        assert_eq!(mesh.asked().pop().unwrap().1, "/api/v1/mesh/claims/release");
    }

    /// A lease that holds a mesh claim is held as far as the claim is: asked of the mesh,
    /// with the act, before the act.
    #[test]
    fn a_lease_holding_a_claim_is_held_as_far_as_the_mesh_says() {
        let repo = repository();
        let mesh = Scripted::new(vec![
            Ok((200, json!({ "key": "s1/c-1" }))),
            state(json!([])),
            Ok((200, json!({}))),
            Ok((200, json!({}))),
            state(json!([{ "key": "s1/c-1", "state": { "state": "held" } }])),
            Ok((200, json!({}))),
            state(json!([{ "key": "s1/c-1", "state": { "state": "suspended" } }])),
        ]);
        let lease = acquire_through(repo.path(), "master", Some(Box::new(mesh.clone()))).unwrap();
        lease.holds("merging #1").expect("held on the mesh");
        assert_eq!(beats(&mesh).last().unwrap()["phase"], "merging #1");
        let e = lease.holds("merging #2").unwrap_err();
        assert!(e.contains("is suspended on the mesh"), "{e}");
        drop(lease);
    }

    #[test]
    fn a_claim_read_suspended_or_released_is_not_held() {
        let ok = || Ok((200, json!({})));
        let claim = |standing: Value| {
            state(json!([
                { "key": "s1/c-1", "session": "s1/integration-me", "state": standing },
                { "key": "s2/c-9", "session": "s2/integration-me", "state": { "state": "held" } },
            ]))
        };
        let held_by = |answers: Vec<Answer>| {
            let mesh = Scripted::new(answers);
            let held = MeshClaim::of(Box::new(mesh.clone()), "s1/c-1".into(), "integration-me");
            let verdict = held.holds("merging #4");
            (mesh, held, verdict)
        };

        // held: the beat first, with the act, then the claim read by its session
        let (mesh, held, verdict) = held_by(vec![ok(), claim(json!({ "state": "held" }))]);
        verdict.expect("held");
        let asked = mesh.asked();
        assert_eq!(asked[0].1, "/api/v1/mesh/beats");
        assert_eq!(asked[0].2.clone().unwrap()["phase"], "merging #4");
        assert_eq!(
            asked[1].1,
            "/api/v1/mesh/state?view=full&session=integration-me"
        );
        drop(held);
        assert_eq!(mesh.asked().pop().unwrap().1, "/api/v1/mesh/claims/release");

        // suspended: refused, naming why, and still given back when dropped
        let (mesh, held, verdict) = held_by(vec![
            ok(),
            claim(json!({ "state": "suspended",
                          "detail": "its holder stopped making progress: no beat for over 8s" })),
        ]);
        let e = verdict.unwrap_err();
        assert!(
            e.contains("suspended") && e.contains("no beat for over 8s"),
            "{e}"
        );
        drop(held);
        assert_eq!(mesh.asked().pop().unwrap().1, "/api/v1/mesh/claims/release");

        // released by the mesh at that beat (overtaken): refused, and not released again
        let (mesh, held, verdict) = held_by(vec![
            Ok((
                400,
                json!({ "error": { "code": "invalid_input", "message": "no beat was written" } }),
            )),
            claim(json!({ "state": "released" })),
        ]);
        assert!(verdict.unwrap_err().contains("is released on the mesh"));
        drop(held);
        assert_eq!(
            mesh.asked().len(),
            2,
            "a released claim is not released twice"
        );

        // unknown to the mesh: refused, and nothing to give back
        let (mesh, held, verdict) = held_by(vec![ok(), state(json!([]))]);
        assert!(verdict.unwrap_err().contains("no longer knows"));
        drop(held);
        assert_eq!(mesh.asked().len(), 2);

        // a mesh that cannot be read is refused: its claims expire everywhere else
        let (_, held, verdict) = held_by(vec![ok(), Err("connection refused".into())]);
        assert!(verdict.unwrap_err().contains("could not be read"));
        std::mem::forget(held);

        // an older server answers the whole state bare, where the claim is found as well
        let (mesh, held, verdict) = held_by(vec![
            Ok((404, json!({ "error": "no such route" }))),
            Ok((400, json!({ "error": { "code": "invalid_input" } }))),
            claim(json!({ "state": "held" })),
        ]);
        verdict.expect("held, read from the whole state");
        assert_eq!(mesh.asked()[2].1, "/api/v1/mesh/state");
        std::mem::forget(held);
    }

    #[test]
    fn a_server_without_beats_or_with_the_rule_off_still_grants() {
        for refusal in [
            Ok((404, json!({ "error": "no such route" }))),
            Ok((
                400,
                json!({ "error": { "code": "invalid_input",
                "message": "the progress rule is off on this runtime" } }),
            )),
            Err("connection reset".to_string()),
        ] {
            let repo = repository();
            let mesh = Scripted::new(vec![
                Ok((200, json!({ "key": "s1/c-1" }))),
                state(json!([])),
                refusal,
            ]);
            let lease =
                acquire_through(repo.path(), "master", Some(Box::new(mesh.clone()))).unwrap();
            assert_eq!(lease.mesh_claim().map(MeshClaim::key), Some("s1/c-1"));
            let claim = lease.mesh_claim().unwrap();
            assert!(claim.beat_refusal_said.load(Ordering::SeqCst), "said once");
            // the next beat is asked, and refused or not, nothing stops
            lease.progressed("selected #1");
            assert_eq!(beats(&mesh).len(), 2);
            drop(lease);
        }
    }

    /// A beat refused again is asked again and not said again: the refusal was said once.
    #[test]
    fn a_beat_refused_again_is_asked_and_not_said_again() {
        let mesh = Scripted::new(vec![
            Err("connection reset".into()),
            Ok((404, json!({ "error": "no such route" }))),
        ]);
        let held = MeshClaim::of(Box::new(mesh.clone()), "s1/c-1".into(), "integration-me");
        held.beat("selected #1");
        assert!(held.beat_refusal_said.load(Ordering::SeqCst), "said once");
        held.beat("merge_attempted #1");
        assert!(held.beat_refusal_said.load(Ordering::SeqCst));
        let phases: Vec<Value> = beats(&mesh)
            .into_iter()
            .map(|b| b["phase"].clone())
            .collect();
        assert_eq!(phases, [json!("selected #1"), json!("merge_attempted #1")]);
    }

    /// The server half answers over a real socket: a JSON answer, a body that is not JSON,
    /// and nobody listening.
    #[test]
    fn the_servers_mesh_is_asked_over_http() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for body in ["{\"key\":\"s/c\"}", "not json"] {
                let (mut s, _) = listener.accept().unwrap();
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        let mesh = ServerMesh::at(format!("http://{address}"));
        let (status, v) = mesh
            .ask("POST", "/api/v1/mesh/claims", Some(json!({})))
            .unwrap();
        assert_eq!((status, v["key"].as_str()), (200, Some("s/c")));
        let e = mesh.ask("GET", "/api/v1/mesh/state", None).unwrap_err();
        assert!(e.contains("not JSON"), "{e}");
        server.join().unwrap();
        let e = mesh.ask("GET", "/api/v1/mesh/state", None).unwrap_err();
        assert!(e.contains("/api/v1/mesh/state"), "{e}");
    }

    fn git(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Criterion 2 (I3911): a lease on origin whose holder stopped renewing it is taken over
    /// by another clone after its expiry and not before; its holder's renewal is then refused,
    /// a leftover of this same clone is taken at once, and a repository without an origin
    /// keeps nothing there.
    #[test]
    fn state_on_origin_a_lease_is_taken_over_after_its_expiry_and_not_before() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().canonicalize().unwrap();
        git(
            &base,
            &["init", "-q", "--bare", "-b", "master", "origin.git"],
        );
        git(&base, &["clone", "-q", "origin.git", "a"]);
        git(&base, &["clone", "-q", "origin.git", "b"]);
        let (a, b) = (base.join("a"), base.join("b"));

        let first = OriginLease::take_at(&a, "master", 1_000, 60).expect("a takes it");
        assert_eq!(first.record().expires_at, 1_060);
        let read = OriginLease::read(&b, "master")
            .unwrap()
            .expect("b reads it");
        assert_eq!(read, *first.record(), "every clone reads the same holder");
        first.renew_at(1_030).expect("its holder renews it");
        let renewed = OriginLease::read(&b, "master").unwrap().unwrap();
        assert_eq!(renewed.expires_at, 1_090);

        // the holder stops renewing: before the expiry b is refused, naming the holder
        let refused = OriginLease::take_at(&b, "master", 1_089, 60)
            .err()
            .expect("not before its expiry");
        assert!(refused.contains(&renewed.holder), "{refused}");
        assert!(refused.contains("taken over only after"), "{refused}");
        // after it, b takes it over
        let second = OriginLease::take_at(&b, "master", 1_091, 60).expect("after its expiry");
        let err = first.renew_at(1_092).unwrap_err();
        assert!(err.contains("lease was lost"), "{err}");
        assert!(err.contains(&second.record().holder), "{err}");

        // a leftover of b's own clone (its flock is held by now) is taken at once
        std::mem::forget(second);
        let third = OriginLease::take_at(&b, "master", 1_100, 60).expect("its own leftover");
        drop(third);
        // released: the next clone takes it at once
        let fourth = OriginLease::take_at(&a, "master", 1_101, 60).expect("released");
        drop(fourth);
        drop(first);

        let alone = tempfile::tempdir().unwrap();
        git(alone.path(), &["init", "-q"]);
        assert!(OriginLease::take(alone.path(), "master").unwrap().is_none());
    }
}
