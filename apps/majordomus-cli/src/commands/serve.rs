//! `majordomus serve`: the HTTP projection on a loopback socket.

use crate::app::App;
use crate::cli::ServeArgs;
use crate::error::Result;
use crate::http::{server, Router};

pub fn run(args: ServeArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let router = Router::new(app.context.clone(), crate::VERSION);
    server::serve(router, &args.host, args.port)?;
    Ok(0)
}
