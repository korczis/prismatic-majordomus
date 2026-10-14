//! The stdio transport: one JSON message per line in, one per line out. Nothing but
//! protocol messages is ever written to the output; diagnostics go through `tracing`,
//! which the executable points at stderr. EOF on input ends the session cleanly, and so
//! does a closed output, because both mean the client has gone.
//!
//! The loop does not know what answers a message: a local [`Server`], or a bridge to the
//! shared server of another process, is a function from a message to an optional
//! response, and the loop drives whichever it is given.

use std::io::{BufRead, Read, Write};

use serde_json::Value;

use crate::error::{Error, Result};

use super::protocol::Server;

/// The longest message a client may send on one line: the bound a request body has over
/// HTTP, so that both transports refuse the same message (I2147).
pub const MAX_LINE_BYTES: usize = crate::http::server::MAX_BODY_BYTES;

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
pub fn serve<R, W, H, M>(input: R, mut output: W, mut handle: H) -> Result<usize>
where
    R: BufRead,
    W: Write,
    H: FnMut(Value) -> Option<M>,
    M: serde::Serialize,
{
    let mut answered = 0;
    let mut input = input;
    loop {
        // one line at most MAX_LINE_BYTES long: a longer one is skipped to its newline without
        // being held, answered as a parse error, and the session goes on (I2147)
        let mut raw = Vec::new();
        let read = (&mut input)
            .take(MAX_LINE_BYTES as u64 + 1)
            .read_until(b'\n', &mut raw)
            .map_err(Error::Transport)?;
        if read == 0 {
            break;
        }
        if raw.len() > MAX_LINE_BYTES && !raw.ends_with(b"\n") {
            input.skip_until(b'\n').map_err(Error::Transport)?;
            write_line(
                &mut output,
                &Server::parse_error(&format!(
                    "the message is over the {MAX_LINE_BYTES}-byte bound and was not read"
                )),
            )?;
            answered += 1;
            continue;
        }
        let line = match String::from_utf8(raw) {
            Ok(l) => l,
            Err(_) => {
                write_line(&mut output, &Server::parse_error("input is not UTF-8"))?;
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let written = match serde_json::from_str::<Value>(&line) {
            Ok(message) => match handle(message) {
                Some(response) => write_line(&mut output, &response),
                None => continue,
            },
            Err(e) => write_line(&mut output, &Server::parse_error(&e.to_string())),
        };
        match written {
            Ok(()) => answered += 1,
            Err(Error::Transport(e)) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                tracing::info!("client closed the output; stopping");
                return Ok(answered);
            }
            Err(e) => return Err(e),
        }
    }
    tracing::info!(answered, "input ended; stopping");
    Ok(answered)
}

fn write_line<W: Write, M: serde::Serialize>(output: &mut W, message: &M) -> Result<()> {
    let mut text = serde_json::to_string(message).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })?;
    text.push('\n');
    output
        .write_all(text.as_bytes())
        .map_err(Error::Transport)?;
    output.flush().map_err(Error::Transport)
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor, Read, Write};

    use serde_json::{json, Value};

    use super::*;

    /// A source or sink that fails on every call, as a closed descriptor does.
    struct Broken;

    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("the input is gone"))
        }
    }

    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("the output is gone"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("the output is gone"))
        }
    }

    fn echo(m: Value) -> Option<Value> {
        Some(json!({ "jsonrpc": "2.0", "id": m["id"], "result": {} }))
    }

    #[test]
    fn an_oversized_message_is_answered_as_a_parse_error_and_the_session_goes_on() {
        let mut input = vec![b'x'; MAX_LINE_BYTES + 10];
        input.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n");
        let mut out = Vec::new();
        let answered = serve(Cursor::new(input), &mut out, echo).unwrap();
        assert_eq!(answered, 2, "the refusal and the next message");
        let lines: Vec<Value> = std::str::from_utf8(&out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines[0]["id"], Value::Null);
        assert_eq!(lines[0]["error"]["code"], -32700);
        let text = lines[0]["error"]["message"].as_str().unwrap();
        assert!(
            text.contains("bound") && text.contains("not read"),
            "{text}"
        );
        assert_eq!(lines[1]["id"], 2, "the message after it is answered");
    }

    #[test]
    fn an_input_that_fails_ends_the_session_with_a_transport_error() {
        let result = serve(BufReader::new(Broken), Vec::new(), echo);
        assert!(matches!(result, Err(Error::Transport(_))), "{result:?}");
    }

    #[test]
    fn an_input_that_fails_inside_an_oversized_message_ends_the_session() {
        // over the bound, and the input breaks while the rest of the line is skipped
        let input = Cursor::new(vec![b'x'; MAX_LINE_BYTES + 10]).chain(Broken);
        let mut out = Vec::new();
        let result = serve(BufReader::new(input), &mut out, echo);
        assert!(matches!(result, Err(Error::Transport(_))), "{result:?}");
        assert!(
            out.is_empty(),
            "nothing is answered for a line never read to its end"
        );
    }

    #[test]
    fn an_output_that_fails_on_the_refusal_of_an_oversized_message_ends_the_session() {
        let mut input = vec![b'x'; MAX_LINE_BYTES + 10];
        input.push(b'\n');
        let result = serve(Cursor::new(input), Broken, echo);
        assert!(matches!(result, Err(Error::Transport(_))), "{result:?}");
    }
}
