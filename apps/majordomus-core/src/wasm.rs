//! The C ABI a browser Worker calls, on `wasm32-unknown-unknown` only.
//!
//! No binding generator: three exports and the module's own memory, so that the core adds
//! no dependency to any lock. The caller copies UTF-8 JSON text into memory it asked
//! [`mj_alloc`] for, calls [`mj_canonical_digest`], reads the answer and frees both buffers
//! with [`mj_free`].
//!
//! ```text
//!   ptr = mj_alloc(len); write text at ptr
//!   out = mj_canonical_digest(ptr, len)   // -> [status u8][len u32 LE][bytes]
//!   status 0: bytes are the 64-char hex digest;  status 1: bytes are the parse error
//!   mj_free(ptr, len); mj_free(out, 5 + bytes_len)
//! ```

use std::alloc::{alloc, dealloc, Layout};

/// Memory for `len` bytes, owned by the caller until [`mj_free`]. A zero length is given one
/// byte, so that every pointer handed out is a real allocation.
#[no_mangle]
pub extern "C" fn mj_alloc(len: usize) -> *mut u8 {
    let layout = Layout::from_size_align(len.max(1), 1).expect("a byte layout");
    // SAFETY: the layout has a non-zero size.
    unsafe { alloc(layout) }
}

/// Free memory [`mj_alloc`] or [`mj_canonical_digest`] handed out, with the length it had.
///
/// # Safety
/// `ptr` must come from this module's allocator with exactly `len` bytes, and be freed once.
#[no_mangle]
pub unsafe extern "C" fn mj_free(ptr: *mut u8, len: usize) {
    let layout = Layout::from_size_align(len.max(1), 1).expect("a byte layout");
    dealloc(ptr, layout);
}

/// The canonical digest of the JSON text at `ptr..ptr+len`, as a framed answer the caller
/// frees: a status byte (0 digest, 1 error), the payload length as u32 little-endian, then
/// the payload (the hex digest, or the parse error's text).
///
/// # Safety
/// `ptr..ptr+len` must be readable memory this module handed out.
#[no_mangle]
pub unsafe extern "C" fn mj_canonical_digest(ptr: *const u8, len: usize) -> *mut u8 {
    let input = std::slice::from_raw_parts(ptr, len);
    let (status, payload) = match std::str::from_utf8(input) {
        Ok(text) => match crate::digest_of_json_text(text) {
            Ok(digest) => (0u8, digest.into_bytes()),
            Err(e) => (1u8, e.into_bytes()),
        },
        Err(e) => (1u8, e.to_string().into_bytes()),
    };
    let total = 5 + payload.len();
    let out = mj_alloc(total);
    *out = status;
    std::ptr::copy_nonoverlapping((payload.len() as u32).to_le_bytes().as_ptr(), out.add(1), 4);
    std::ptr::copy_nonoverlapping(payload.as_ptr(), out.add(5), payload.len());
    out
}
