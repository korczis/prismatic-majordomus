//! `majordomus serve`: the HTTP projection on a loopback socket, without a stdio session
//! of its own. It is the same shared server `majordomus mcp` starts: when one is already
//! running for the repository, `serve` says where and exits 0 rather than starting a
//! second one.
//!
//! Lifecycle: when stdin is a pipe or a socket, the server lives as long as it: end of
//! file (the parent closed its end) starts the shutdown, which waits for attached peers to
//! leave. When stdin is anything else (a terminal, `/dev/null` under nohup or a service
//! manager, a file) nothing is watched and the process runs until it is stopped. There is
//! no daemon mode either way: whoever started the process owns it.

use std::io::Read;
use std::time::Duration;

use crate::app::App;
use crate::cli::ServeArgs;
use crate::error::{Error, Result};
use crate::http::server::stdin_is_a_pipe;
use crate::lease::{self, Role};
use crate::repository::Repository;
use crate::shared::SharedServer;

/// Run `majordomus serve`.
pub fn run(args: ServeArgs) -> Result<u8> {
    let start = match &args.repo.repo {
        Some(p) => p.clone(),
        None => std::env::current_dir().map_err(|e| Error::io(".", e))?,
    };
    let repo = Repository::discover(&start)?;
    let lease = match lease::elect(&repo)? {
        Role::Peer { url } => {
            tracing::info!(
                url = %url,
                "a shared server for this repository is already running at {url} (swagger ui {url}/docs); not starting a second one"
            );
            return Ok(0);
        }
        Role::Server(lease) => lease,
    };
    let app = App::load(&args.repo)?;
    let shared = SharedServer::start(
        app.context.clone(),
        crate::VERSION,
        &args.host,
        args.port,
        false,
        lease,
    )?;
    if stdin_is_a_pipe() {
        tracing::info!(
            "stdin is a pipe; the server stops when it closes and the last peer has left"
        );
        let mut sink = Vec::new();
        let _ = std::io::stdin().lock().read_to_end(&mut sink);
        tracing::info!("stdin closed; stopping");
    } else {
        tracing::info!("stdin is not a pipe; the server runs until the process is stopped");
        loop {
            std::thread::sleep(Duration::from_secs(1));
            shared.endpoint().reap();
        }
    }
    shared.wait_until_peers_leave();
    shared.stop();
    Ok(0)
}
