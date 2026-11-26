//! The frame: four bytes of big-endian length, then that many bytes of one JSON message.
//!
//! The length comes first so that a reader knows the size of what it is about to hold
//! before it holds any of it: a prefix over the bound is refused on its four bytes, and the
//! body behind it is never read ([`RefusalCode::FrameOversized`]). The bounds are the ones
//! the other transports already have — a request is bounded as an HTTP body and an MCP
//! stdio line are, a reply as the MCP bridge bounds a reply — so a call admitted on one
//! transport is admitted on every other (rule `project.operation-transport-parity`).
//!
//! ```
//! use majordomus_cli::fabric::frame::{encode, read_frame, MAX_REQUEST_FRAME_BYTES};
//!
//! let bytes = encode(br#"{"request":1}"#, MAX_REQUEST_FRAME_BYTES).unwrap();
//! assert_eq!(&bytes[..4], &[0, 0, 0, 13]);
//! let mut reader = &bytes[..];
//! let body = read_frame(&mut reader, MAX_REQUEST_FRAME_BYTES).unwrap().unwrap();
//! assert_eq!(body, br#"{"request":1}"#);
//! assert!(read_frame(&mut reader, MAX_REQUEST_FRAME_BYTES).unwrap().is_none(), "a clean end");
//! ```
//!
//! These bytes travel on a socket only this OS user can open. They are not the bytes a
//! host-to-host link carries, which are encrypted and authenticated by the link; nothing
//! here is signed, because nothing here crosses a machine.

use std::io::{ErrorKind, Read};

use super::refusal::{Refusal, RefusalCode};

/// The length prefix.
pub const HEADER_BYTES: usize = 4;
/// The largest request body: the bound on an HTTP request body and an MCP stdio line.
pub const MAX_REQUEST_FRAME_BYTES: usize = crate::http::server::MAX_BODY_BYTES;
/// The largest reply body: the bound the MCP bridge puts on a reply.
pub const MAX_REPLY_FRAME_BYTES: usize = crate::mcp::bridge::MAX_REPLY_BYTES;

/// One body in its frame, or `None` when the body is over `max`.
pub fn encode(body: &[u8], max: usize) -> Option<Vec<u8>> {
    if body.len() > max {
        return None;
    }
    // max is at most MAX_REPLY_FRAME_BYTES, far inside u32
    let length = (body.len() as u32).to_be_bytes();
    let mut bytes = Vec::with_capacity(HEADER_BYTES + body.len());
    bytes.extend_from_slice(&length);
    bytes.extend_from_slice(body);
    Some(bytes)
}

/// Read one frame's body, at most `max` bytes, reading nothing past it. `Ok(None)` is the
/// connection ending cleanly between frames. A connection that ends or fails inside a frame
/// is [`RefusalCode::FrameTruncated`]; a prefix over `max` is
/// [`RefusalCode::FrameOversized`], with the body left unread. Both are terminal: a reader
/// that has lost its place in the stream has no next frame to find.
pub fn read_frame<R: Read>(reader: &mut R, max: usize) -> Result<Option<Vec<u8>>, Refusal> {
    let mut header = [0u8; HEADER_BYTES];
    match fill(reader, &mut header)? {
        0 => return Ok(None),
        HEADER_BYTES => {}
        n => {
            return Err(truncated(format!(
                "the connection ended {n} bytes into a length prefix"
            )))
        }
    }
    let length = u32::from_be_bytes(header) as usize;
    if length > max {
        return Err(Refusal::new(
            RefusalCode::FrameOversized,
            format!("a frame of {length} bytes is over the {max}-byte bound and was not read"),
        ));
    }
    let mut body = vec![0u8; length];
    let read = fill(reader, &mut body)?;
    if read < length {
        return Err(truncated(format!(
            "the connection ended {read} bytes into a {length}-byte frame"
        )));
    }
    Ok(Some(body))
}

/// Read until `buffer` is full or the reader ends, retrying an interrupted read.
fn fill<R: Read>(reader: &mut R, buffer: &mut [u8]) -> Result<usize, Refusal> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => {
                return Err(truncated(format!(
                    "the connection failed inside a frame: {e}"
                )))
            }
        }
    }
    Ok(filled)
}

fn truncated(message: String) -> Refusal {
    Refusal::new(RefusalCode::FrameTruncated, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::io;

    /// A reader that hands out its bytes in the chunk sizes it is given, then fails or ends.
    struct Chunked {
        bytes: Vec<u8>,
        at: usize,
        sizes: Vec<usize>,
        interrupt_first: bool,
        fail_at_end: bool,
    }

    impl Read for Chunked {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.interrupt_first {
                self.interrupt_first = false;
                return Err(io::Error::from(ErrorKind::Interrupted));
            }
            if self.at == self.bytes.len() {
                if self.fail_at_end {
                    return Err(io::Error::from(ErrorKind::ConnectionReset));
                }
                return Ok(0);
            }
            let size = self.sizes.pop().unwrap_or(1).max(1);
            let n = size.min(buf.len()).min(self.bytes.len() - self.at);
            buf[..n].copy_from_slice(&self.bytes[self.at..self.at + n]);
            self.at += n;
            Ok(n)
        }
    }

    fn chunked(bytes: Vec<u8>, sizes: Vec<usize>) -> Chunked {
        Chunked {
            bytes,
            at: 0,
            sizes,
            interrupt_first: false,
            fail_at_end: false,
        }
    }

    #[test]
    fn a_prefix_over_the_bound_is_refused_before_its_body_is_read() {
        let mut bytes = (MAX_REQUEST_FRAME_BYTES as u32 + 1).to_be_bytes().to_vec();
        bytes.extend_from_slice(b"never read");
        let mut reader = &bytes[..];
        let refused = read_frame(&mut reader, MAX_REQUEST_FRAME_BYTES).unwrap_err();
        assert_eq!(refused.code(), RefusalCode::FrameOversized);
        assert!(refused.terminal());
        assert_eq!(reader, b"never read", "the body stays where it was");
        // the largest prefix there is, too
        let mut reader = &[0xff, 0xff, 0xff, 0xff][..];
        assert_eq!(
            read_frame(&mut reader, MAX_REPLY_FRAME_BYTES)
                .unwrap_err()
                .code(),
            RefusalCode::FrameOversized
        );
    }

    #[test]
    fn a_body_over_the_bound_is_not_framed() {
        assert!(encode(
            &vec![b'x'; MAX_REQUEST_FRAME_BYTES + 1],
            MAX_REQUEST_FRAME_BYTES
        )
        .is_none());
        let exact = encode(
            &vec![b'x'; MAX_REQUEST_FRAME_BYTES],
            MAX_REQUEST_FRAME_BYTES,
        )
        .unwrap();
        let mut reader = &exact[..];
        let body = read_frame(&mut reader, MAX_REQUEST_FRAME_BYTES)
            .unwrap()
            .unwrap();
        assert_eq!(
            body.len(),
            MAX_REQUEST_FRAME_BYTES,
            "the bound itself is admitted"
        );
    }

    #[test]
    fn an_end_inside_a_prefix_or_a_body_is_truncation() {
        for cut in 1..HEADER_BYTES {
            let mut reader = &[0u8, 0, 0, 5][..cut];
            let refused = read_frame(&mut reader, 16).unwrap_err();
            assert_eq!(refused.code(), RefusalCode::FrameTruncated, "cut at {cut}");
        }
        let mut reader = &[0u8, 0, 0, 5, b'a', b'b'][..];
        let refused = read_frame(&mut reader, 16).unwrap_err();
        assert_eq!(refused.code(), RefusalCode::FrameTruncated);
        assert!(
            refused.message().contains("2 bytes into a 5-byte frame"),
            "{refused}"
        );
    }

    #[test]
    fn an_interrupted_read_is_retried_and_a_failed_one_is_truncation() {
        let bytes = encode(b"{}", 16).unwrap();
        let mut reader = chunked(bytes.clone(), vec![]);
        reader.interrupt_first = true;
        assert_eq!(read_frame(&mut reader, 16).unwrap().unwrap(), b"{}");

        let mut reader = chunked(bytes[..5].to_vec(), vec![]);
        reader.fail_at_end = true;
        let refused = read_frame(&mut reader, 16).unwrap_err();
        assert_eq!(refused.code(), RefusalCode::FrameTruncated);
        assert!(refused.message().contains("failed"), "{refused}");
    }

    #[test]
    fn an_empty_body_is_a_frame_and_two_frames_are_read_apart() {
        let mut bytes = encode(b"", 16).unwrap();
        bytes.extend(encode(b"[1]", 16).unwrap());
        let mut reader = &bytes[..];
        assert_eq!(read_frame(&mut reader, 16).unwrap().unwrap(), b"");
        assert_eq!(read_frame(&mut reader, 16).unwrap().unwrap(), b"[1]");
        assert!(read_frame(&mut reader, 16).unwrap().is_none());
    }

    proptest! {
        #[test]
        fn any_bodies_in_any_chunking_read_back_in_order(
            bodies in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 0..64), 0..8),
            sizes in proptest::collection::vec(1usize..9, 0..64),
        ) {
            let mut bytes = Vec::new();
            for body in &bodies {
                bytes.extend(encode(body, 64).unwrap());
            }
            let mut reader = chunked(bytes, sizes);
            for body in &bodies {
                prop_assert_eq!(read_frame(&mut reader, 64).unwrap().unwrap(), body.clone());
            }
            prop_assert!(read_frame(&mut reader, 64).unwrap().is_none());
        }

        #[test]
        fn any_cut_of_a_frame_is_refused_never_misread(
            body in proptest::collection::vec(any::<u8>(), 1..64),
            cut in 1usize..68,
        ) {
            let bytes = encode(&body, 64).unwrap();
            let cut = cut.min(bytes.len() - 1);
            let mut reader = &bytes[..cut];
            let refused = read_frame(&mut reader, 64).unwrap_err();
            prop_assert_eq!(refused.code(), RefusalCode::FrameTruncated);
        }

        #[test]
        fn any_prefix_over_the_bound_is_oversized(length in 65u32..) {
            let mut reader = &length.to_be_bytes()[..];
            prop_assert_eq!(read_frame(&mut reader, 64).unwrap_err().code(), RefusalCode::FrameOversized);
        }
    }
}
