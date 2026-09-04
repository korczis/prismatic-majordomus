//! The stdio transport: one JSON message per line in, one per line out. Nothing but
//! protocol messages is ever written to the output; diagnostics go through `tracing`,
//! which the executable points at stderr. EOF on input ends the session cleanly, and so
//! does a closed output, because both mean the client has gone.

use std::io::{BufRead, Write};

use crate::error::{Error, Result};

use super::protocol::Server;

/// Serve until the input ends. Returns the number of messages answered.
pub fn serve<R: BufRead, W: Write>(server: &mut Server, input: R, mut output: W) -> Result<usize> {
    let mut answered = 0;
    for line in input.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                write_line(&mut output, &Server::parse_error("input is not UTF-8"))?;
                continue;
            }
            Err(e) => return Err(Error::Transport(e)),
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(message) => server.handle(message),
            Err(e) => Some(Server::parse_error(&e.to_string())),
        };
        if let Some(response) = response {
            match write_line(&mut output, &response) {
                Ok(()) => answered += 1,
                Err(Error::Transport(e)) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                    tracing::info!("client closed the output; stopping");
                    return Ok(answered);
                }
                Err(e) => return Err(e),
            }
        }
    }
    tracing::info!(answered, "input ended; stopping");
    Ok(answered)
}

fn write_line<W: Write>(output: &mut W, message: &serde_json::Value) -> Result<()> {
    let mut text = serde_json::to_string(message).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })?;
    text.push('\n');
    output
        .write_all(text.as_bytes())
        .map_err(Error::Transport)?;
    output.flush().map_err(Error::Transport)
}
