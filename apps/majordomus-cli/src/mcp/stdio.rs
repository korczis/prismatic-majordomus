//! The stdio transport: one JSON message per line in, one per line out. Nothing but
//! protocol messages is ever written to the output; diagnostics go through `tracing`,
//! which the executable points at stderr. EOF on input ends the session cleanly, and so
//! does a closed output, because both mean the client has gone.
//!
//! The loop does not know what answers a message: a local [`Server`], or a bridge to the
//! shared server of another process, is a function from a message to an optional
//! response, and the loop drives whichever it is given.

use std::io::{BufRead, Write};

use serde_json::Value;

use crate::error::{Error, Result};

use super::protocol::Server;

/// Serve until the input ends, answering every message through `handle`. Returns the
/// number of messages answered.
///
/// ```
/// use majordomus_cli::mcp::stdio;
/// use serde_json::json;
/// let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\nnot json\n\n";
/// let mut out = Vec::new();
/// let n = stdio::serve(&input[..], &mut out, |m| Some(json!({ "jsonrpc": "2.0", "id": m["id"], "result": {} }))).unwrap();
/// let lines: Vec<&str> = std::str::from_utf8(&out).unwrap().lines().collect();
/// assert_eq!(n, 2, "one answer, one parse error");
/// assert!(lines[0].contains("\"result\""));
/// assert!(lines[1].contains("-32700"));
/// ```
pub fn serve<R, W, H>(input: R, mut output: W, mut handle: H) -> Result<usize>
where
    R: BufRead,
    W: Write,
    H: FnMut(Value) -> Option<Value>,
{
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
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle(message),
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

fn write_line<W: Write>(output: &mut W, message: &Value) -> Result<()> {
    let mut text = serde_json::to_string(message).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })?;
    text.push('\n');
    output
        .write_all(text.as_bytes())
        .map_err(Error::Transport)?;
    output.flush().map_err(Error::Transport)
}
