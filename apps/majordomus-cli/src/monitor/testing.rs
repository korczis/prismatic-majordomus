//! What the monitor's tests share: a context over a synthetic repository that is also a git
//! repository, a cooperation runtime that writes no file, and a session opened in it.

use std::sync::Arc;

use crate::capability::Context;
use crate::mesh::cooperation::Cooperation;
use crate::mesh::journal::SessionInfo;
use crate::synthetic::SyntheticRepository;

/// A context over `repo`, made a git repository first so the executor's trail has a place.
pub(crate) fn context(repo: &SyntheticRepository) -> Context {
    let git = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.root())
        .status()
        .expect("git");
    assert!(git.success());
    let registry = crate::capability::CapabilityRegistry::builder()
        .with_modules(crate::capability::builtin::modules())
        .build()
        .unwrap();
    Context::new(Arc::new(repo.index().unwrap()), Arc::new(registry))
}

/// A cooperation runtime with an ephemeral identity and no journal file.
pub(crate) fn cooperation() -> Arc<Cooperation> {
    crate::mesh::testing::cooperation()
}

/// Open `session` for `worker`, saying `intent`.
pub(crate) fn session(c: &Cooperation, session: &str, worker: &str, intent: &str) {
    c.open_session(SessionInfo {
        session: session.into(),
        client: "test".into(),
        worker: Some(worker.into()),
        intent: Some(intent.into()),
        task: None,
        issue: None,
        milestone: None,
        checkout: None,
        branch: None,
        head: None,
        dirty: None,
        context: None,
    })
    .unwrap();
}

/// A server on the loopback address that answers `/` as the leaseholder of `id` and every
/// other path with `body`, for as many requests as are made.
pub(crate) fn http_stub(id: String, body: &'static str) -> String {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(&stream);
            let mut request = String::new();
            let _ = reader.read_line(&mut request);
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                line.clear();
            }
            let answer = if request.starts_with("GET / ") {
                serde_json::json!({ "name": "majordomus", "repository_id": id }).to_string()
            } else {
                body.to_string()
            };
            let _ = write!(
                &stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
                answer.len()
            );
        }
    });
    url
}
