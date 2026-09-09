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

use crate::app::App;
use crate::cli::ServeArgs;
use crate::deploy::{Deployment, Listen, KIND};
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
                "a shared server for this repository is already running at {url} (its home page lists what it serves); not starting a second one"
            );
            return Ok(0);
        }
        Role::Server(lease) => lease,
    };
    let app = App::load(&args.repo)?;
    // The address: the local default, or the one a deployment object declares. The port is
    // never typed twice — the object states it once and the process, the image and the
    // provider configuration all read that one.
    let declared = match &args.deployment {
        Some(id) => Some(deployment(&app, id)?),
        None => None,
    };
    let (host, port) = match &declared {
        Some((_, listen)) => (listen.interface.host().to_string(), listen.port.get()),
        None => (args.host.clone(), args.port),
    };
    let shared = SharedServer::start(
        app.context.clone(),
        crate::VERSION,
        &host,
        port,
        false,
        declared.as_ref().map(|(file, _)| file.as_str()),
        lease,
        Some(app.share.dir()),
    )?;
    if stdin_is_a_pipe() {
        tracing::info!(
            "stdin is a pipe; the server stops when it closes and the last peer has left"
        );
        let mut sink = Vec::new();
        let _ = std::io::stdin().lock().read_to_end(&mut sink);
        tracing::info!("stdin closed; stopping");
    } else {
        tracing::info!(
            "stdin is not a pipe; the server runs until the process is stopped, or until its executable is rebuilt underneath it"
        );
        while shared.tick() {}
    }
    shared.wait_until_peers_leave();
    shared.stop();
    Ok(0)
}

/// The listen address one deployment object declares, with the file that declares it. An
/// id the layer does not have is refused by name rather than falling back to a default: a
/// hosted process that quietly bound loopback would pass every check here and be
/// unreachable in production.
fn deployment(app: &App, id: &str) -> Result<(String, Listen)> {
    let object = app
        .context
        .index
        .objects
        .iter()
        .find(|o| o.kind == KIND && o.identity == id)
        .ok_or_else(|| Error::DeploymentNotFound { id: id.into() })?;
    let parsed = Deployment::parse(object).map_err(|refusal| Error::InvalidDeployment {
        reason: refusal.to_string(),
    })?;
    Ok((object.provenance.path.clone(), parsed.listen))
}
