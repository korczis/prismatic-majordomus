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

use std::path::Path;
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
        let held: Vec<String> = match self.mesh.ask("GET", "/api/v1/mesh/state", None) {
            Ok((200, v)) => v["state"]["claims"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["state"]["state"] == json!("held"))
                .filter(|c| c["session"] == json!(own))
                .filter_map(|c| c["key"].as_str())
                .filter(|k| *k != self.key)
                .map(str::to_string)
                .collect(),
            other => {
                eprintln!("majordomus: the mesh's claims could not be read: {other:?}");
                return;
            }
        };
        for key in held {
            if let Err(e) = release(self.mesh.as_ref(), &key) {
                eprintln!("majordomus: a leftover integration claim {key} stays held: {e}");
            }
        }
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
        return Ok(Reach::Repository(MeshClaim {
            mesh,
            key,
            session: session.to_string(),
        }));
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
    let Ok((200, v)) = mesh.ask("GET", "/api/v1/mesh/state", None) else {
        return String::new();
    };
    let named: Vec<String> = v["state"]["claims"]
        .as_array()
        .into_iter()
        .flatten()
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
            if let Some(claim) = lease.mesh_claim() {
                claim.release_leftovers();
            }
            Ok(lease)
        }
        Reach::CloneOnly => IntegrationLease::acquire(root, base),
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
        let held = MeshClaim {
            mesh: Box::new(mesh.clone()),
            key: "s1/c-new".into(),
            session: "integration-me".into(),
        };
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
        let held = MeshClaim {
            mesh: Box::new(mesh.clone()),
            key: "k".into(),
            session: "integration-me".into(),
        };
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
        let held = MeshClaim {
            mesh: Box::new(mesh.clone()),
            key: "k".into(),
            session: "integration-me".into(),
        };
        held.release_leftovers();
        drop(held);
        assert_eq!(mesh.asked().len(), 3);
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
}
