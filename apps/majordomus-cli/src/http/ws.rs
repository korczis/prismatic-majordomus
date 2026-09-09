//! WebSocket, by hand: the handshake and the frames of RFC 6455, and nothing else.
//!
//! There is no dependency here for the same reason there is none for base64 or for
//! percent-encoding: what this server needs of the protocol is a handshake, text frames it
//! writes, a ping it writes and a close it writes, and that is less code than the seam an
//! async runtime would introduce into a synchronous server. `Socket.IO` would be a second
//! protocol on top of this one, with its own framing, its own reconnection semantics and
//! its own client to ship, in exchange for nothing this control plane needs.
//!
//! What is deliberately absent is reading. The server this crate runs is
//! [`tiny_http`], whose upgrade hands over one `Read + Write` value that cannot be split;
//! a reader thread blocking on it would have to hold the same lock the writer needs. So
//! the stream this module writes is one-way by design: everything a client wants to say —
//! which executions to follow, and from which sequence — it says in the request target,
//! and the server ignores whatever the client sends afterwards. A client that goes away is
//! noticed by the next write failing, which the heartbeat guarantees will happen.

use std::io::Write;

/// The constant RFC 6455 appends to the client's key before hashing it.
const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// The largest frame this server writes in one piece.
pub const MAX_FRAME_BYTES: usize = 256 * 1024;

/// Is this request asking to become a WebSocket?
///
/// ```
/// use majordomus_cli::http::{ws, Request};
/// let plain = Request::parse_target("GET", "/events", vec![]);
/// assert!(!ws::is_upgrade(&plain));
/// let asking = plain.clone().with_headers(vec![
///     ("Upgrade".into(), "websocket".into()),
///     ("Connection".into(), "Upgrade".into()),
/// ]);
/// assert!(ws::is_upgrade(&asking));
/// ```
pub fn is_upgrade(req: &super::Request) -> bool {
    req.header("upgrade")
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
        && req
            .header("connection")
            .is_some_and(|v| v.to_ascii_lowercase().contains("upgrade"))
}

/// What a handshake needs from the request, or why it cannot be done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handshake {
    /// The value of `Sec-WebSocket-Accept` to answer with.
    Accept(String),
    /// The request cannot become a WebSocket, and this is what to tell the client.
    Refuse {
        /// The status to answer with.
        status: u16,
        /// Why, for a person reading a network panel.
        reason: String,
    },
}

/// Read the handshake headers and compute the answer.
///
/// ```
/// use majordomus_cli::http::{ws::{handshake, Handshake}, Request};
/// // the example key of RFC 6455 §1.3 and the accept value it specifies
/// let req = Request::parse_target("GET", "/events", vec![]).with_headers(vec![
///     ("Upgrade".into(), "websocket".into()),
///     ("Connection".into(), "Upgrade".into()),
///     ("Sec-WebSocket-Key".into(), "dGhlIHNhbXBsZSBub25jZQ==".into()),
///     ("Sec-WebSocket-Version".into(), "13".into()),
/// ]);
/// assert_eq!(
///     handshake(&req),
///     Handshake::Accept("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=".into())
/// );
/// ```
pub fn handshake(req: &super::Request) -> Handshake {
    if req.method != "GET" {
        return Handshake::Refuse {
            status: 405,
            reason: "a WebSocket is opened with GET".into(),
        };
    }
    match req.header("sec-websocket-version") {
        Some("13") => {}
        Some(other) => {
            return Handshake::Refuse {
                status: 400,
                reason: format!("Sec-WebSocket-Version {other} is not 13"),
            }
        }
        None => {
            return Handshake::Refuse {
                status: 400,
                reason: "no Sec-WebSocket-Version header".into(),
            }
        }
    }
    let Some(key) = req.header("sec-websocket-key") else {
        return Handshake::Refuse {
            status: 400,
            reason: "no Sec-WebSocket-Key header".into(),
        };
    };
    Handshake::Accept(accept_for(key))
}

/// The `Sec-WebSocket-Accept` value for a client key.
pub fn accept_for(key: &str) -> String {
    crate::cockpit::base64(&sha1(format!("{key}{GUID}").as_bytes()))
}

/// The opcodes this server writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    /// A UTF-8 payload.
    Text,
    /// A heartbeat the client answers automatically.
    Ping,
    /// The end of the conversation.
    Close,
}

impl Opcode {
    fn bits(self) -> u8 {
        match self {
            Opcode::Text => 0x1,
            Opcode::Ping => 0x9,
            Opcode::Close => 0x8,
        }
    }
}

/// One unfragmented frame, from the server, unmasked as RFC 6455 requires of a server.
///
/// ```
/// use majordomus_cli::http::ws::{frame, Opcode};
/// assert_eq!(frame(Opcode::Text, b"hi"), vec![0x81, 0x02, b'h', b'i']);
/// assert_eq!(&frame(Opcode::Ping, b"")[..2], &[0x89, 0x00]);
/// // a payload of 126 bytes or more carries an extended length
/// assert_eq!(&frame(Opcode::Text, &[b'x'; 200])[..4], &[0x81, 126, 0, 200]);
/// ```
pub fn frame(opcode: Opcode, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 10);
    out.push(0x80 | opcode.bits());
    match payload.len() {
        n if n < 126 => out.push(n as u8),
        n if n <= u16::MAX as usize => {
            out.push(126);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            out.push(127);
            out.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    out.extend_from_slice(payload);
    out
}

/// Write one text frame, refusing a payload over [`MAX_FRAME_BYTES`].
///
/// The refusal is not a truncation: a client that received half a JSON document would
/// have to guess, and a bounded producer is why this cannot happen — every event this
/// server writes is already bounded by the store's own limits.
pub fn write_text(stream: &mut dyn Write, text: &str) -> std::io::Result<()> {
    if text.len() > MAX_FRAME_BYTES {
        return Err(std::io::Error::other(format!(
            "a frame of {} bytes is over the {MAX_FRAME_BYTES}-byte bound",
            text.len()
        )));
    }
    stream.write_all(&frame(Opcode::Text, text.as_bytes()))?;
    stream.flush()
}

/// Write a ping with no payload.
pub fn write_ping(stream: &mut dyn Write) -> std::io::Result<()> {
    stream.write_all(&frame(Opcode::Ping, b""))?;
    stream.flush()
}

/// Write a close frame carrying a status code and a reason, then flush.
///
/// 1000 is a normal close; 1001 is "going away", which is what a server that is shutting
/// down means.
pub fn write_close(stream: &mut dyn Write, code: u16, reason: &str) -> std::io::Result<()> {
    let mut payload = code.to_be_bytes().to_vec();
    payload.extend_from_slice(&reason.as_bytes()[..reason.len().min(120)]);
    stream.write_all(&frame(Opcode::Close, &payload))?;
    stream.flush()
}

/// SHA-1 of a byte string, for the one thing RFC 6455 specifies it for.
///
/// The handshake is the only place this algorithm appears in this crate, and it is not
/// used for anything that needs to be hard to forge — the accept value proves the peer
/// read the request, nothing more. Everything this executable hashes for real (the
/// registry fingerprint, the index, the artifacts) is SHA-256.
///
/// ```
/// use majordomus_cli::http::ws::sha1;
/// let digest: String = sha1(b"abc").iter().map(|b| format!("{b:02x}")).collect();
/// assert_eq!(digest, "a9993e364706816aba3e25717850c26c9cd0d89d");
/// let empty: String = sha1(b"").iter().map(|b| format!("{b:02x}")).collect();
/// assert_eq!(empty, "da39a3ee5e6b4b0d3255bfef95601890afd80709");
/// ```
pub fn sha1(message: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let mut padded = message.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&((message.len() as u64) * 8).to_be_bytes());

    for block in padded.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::Request;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn sha1_matches_the_published_vectors() {
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hex(&sha1(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1",
            "a message that crosses a block boundary"
        );
        assert_eq!(
            hex(&sha1(&[b'a'; 1000])),
            "291e9a6c66994949b57ba5e650361e98fc36b1ba"
        );
        // exactly one block of padding overflow: 56 bytes needs a second block
        assert_eq!(sha1(&[b'x'; 56]).len(), 20);
        assert_eq!(sha1(&[b'x'; 64]).len(), 20);
    }

    #[test]
    fn the_accept_value_is_the_one_rfc_6455_specifies() {
        assert_eq!(
            accept_for("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn a_handshake_is_refused_with_a_reason_a_person_can_read() {
        let base = Request::parse_target("GET", "/events", vec![]);
        let refuse =
            |headers: Vec<(String, String)>| match handshake(&base.clone().with_headers(headers)) {
                Handshake::Refuse { status, reason } => (status, reason),
                Handshake::Accept(_) => panic!("accepted"),
            };
        assert_eq!(refuse(vec![]).0, 400);
        assert!(refuse(vec![]).1.contains("Version"));
        assert!(refuse(vec![("Sec-WebSocket-Version".into(), "8".into())])
            .1
            .contains("not 13"));
        assert!(refuse(vec![("Sec-WebSocket-Version".into(), "13".into())])
            .1
            .contains("Key"));
        let post = Request::parse_target("POST", "/events", vec![]);
        assert_eq!(
            match handshake(&post) {
                Handshake::Refuse { status, .. } => status,
                Handshake::Accept(_) => 0,
            },
            405
        );
    }

    #[test]
    fn only_a_request_that_asks_is_an_upgrade() {
        let base = Request::parse_target("GET", "/events", vec![]);
        assert!(!is_upgrade(&base));
        assert!(!is_upgrade(
            &base
                .clone()
                .with_headers(vec![("Upgrade".into(), "websocket".into())])
        ));
        assert!(is_upgrade(&base.clone().with_headers(vec![
            ("upgrade".into(), "WebSocket".into()),
            ("connection".into(), "keep-alive, Upgrade".into()),
        ])));
    }

    #[test]
    fn frames_carry_the_lengths_the_protocol_specifies() {
        assert_eq!(frame(Opcode::Text, b""), vec![0x81, 0x00]);
        assert_eq!(frame(Opcode::Close, b""), vec![0x88, 0x00]);
        let medium = frame(Opcode::Text, &[b'x'; 65_535]);
        assert_eq!(&medium[..4], &[0x81, 126, 0xff, 0xff]);
        assert_eq!(medium.len(), 65_535 + 4);
        let large = frame(Opcode::Text, &[b'x'; 70_000]);
        assert_eq!(large[1], 127);
        assert_eq!(
            u64::from_be_bytes(large[2..10].try_into().unwrap()),
            70_000,
            "the 64-bit length"
        );
        // no mask bit is ever set: a server frame is never masked
        for f in [
            frame(Opcode::Text, b"a"),
            frame(Opcode::Ping, b""),
            frame(Opcode::Close, b"x"),
        ] {
            assert_eq!(f[1] & 0x80, 0, "a server frame carries no mask");
            assert_eq!(f[0] & 0x80, 0x80, "and is never fragmented");
        }
    }

    #[test]
    fn writing_refuses_an_oversized_frame_rather_than_cutting_it() {
        let mut sink: Vec<u8> = Vec::new();
        let big = "x".repeat(MAX_FRAME_BYTES + 1);
        assert!(write_text(&mut sink, &big).is_err());
        assert!(sink.is_empty(), "nothing half-written reaches the wire");
        assert!(write_text(&mut sink, "{\"a\":1}").is_ok());
        assert_eq!(&sink[2..], b"{\"a\":1}");
        sink.clear();
        assert!(write_ping(&mut sink).is_ok());
        assert_eq!(sink, vec![0x89, 0x00]);
        sink.clear();
        assert!(write_close(&mut sink, 1001, "going away").is_ok());
        assert_eq!(sink[0], 0x88);
        assert_eq!(u16::from_be_bytes([sink[2], sink[3]]), 1001);
        assert_eq!(&sink[4..], b"going away");
    }
}
