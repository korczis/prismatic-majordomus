//! The portable core of Majordomus: the computations whose answer must be the same wherever
//! they run — on the machine that serves the repository, on a mesh peer, or in a browser's
//! WebAssembly — so that one side can recompute what the other claims.
//!
//! It holds no I/O, no clock, no randomness, no filesystem and no network, and depends on
//! nothing the executable does not already take. It compiles for the host and for
//! `wasm32-unknown-unknown`; on the latter it also exports the C ABI in [`wasm`] that a
//! browser Worker calls.
//!
//! ```
//! use majordomus_core::{canonical_json, canonical_digest_hex};
//! let a = serde_json::json!({"b": 1, "a": [{"d": 2, "c": 3}]});
//! assert_eq!(canonical_json(&a), br#"{"a":[{"c":3,"d":2}],"b":1}"#.to_vec());
//! assert_eq!(canonical_digest_hex(&a).len(), 64);
//! ```

use std::collections::BTreeSet;

use serde_json::Value;
use sha2::{Digest, Sha256};

#[cfg(target_arch = "wasm32")]
pub mod wasm;

/// The canonical bytes of a JSON value: object keys in byte order at every depth, no
/// whitespace, every scalar as `serde_json` writes it. Two values equal as data have equal
/// canonical bytes whatever order their keys were written in; arrays keep their order.
///
/// ```
/// use majordomus_core::canonical_json;
/// let one = serde_json::json!({"b": 2, "a": {"d": 1, "c": [3, {"z": 0, "y": 1}]}});
/// let two = serde_json::json!({"a": {"c": [3, {"y": 1, "z": 0}], "d": 1}, "b": 2});
/// assert_eq!(canonical_json(&one), canonical_json(&two));
/// assert_ne!(canonical_json(&serde_json::json!([1, 2])), canonical_json(&serde_json::json!([2, 1])));
/// ```
pub fn canonical_json(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Object(map) => {
            // A BTreeSet, not a sorted vector: the order is the byte order of the keys,
            // decided by the container rather than by a comparator written here.
            let keys: BTreeSet<&String> = map.keys().collect();
            out.push(b'{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                out.extend(serde_json::to_vec(key).unwrap_or_default());
                out.push(b':');
                write_canonical(&map[*key], out);
            }
            out.push(b'}');
        }
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_canonical(item, out);
            }
            out.push(b']');
        }
        other => out.extend(serde_json::to_vec(other).unwrap_or_default()),
    }
}

/// Hex SHA-256 of bytes, lowercase.
///
/// ```
/// use majordomus_core::sha256_hex;
/// assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
/// assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
/// ```
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// Hex SHA-256 of a value's canonical bytes: the digest two machines compare to agree that
/// they hold the same value.
///
/// ```
/// use majordomus_core::{canonical_digest_hex, canonical_json, sha256_hex};
/// let v = serde_json::json!({"z": [1, 2.5, "x"], "a": null});
/// assert_eq!(canonical_digest_hex(&v), sha256_hex(&canonical_json(&v)));
/// ```
pub fn canonical_digest_hex(value: &Value) -> String {
    sha256_hex(&canonical_json(value))
}

/// The digest of a JSON document given as text, or why it is not JSON. What a caller that
/// holds text rather than a parsed value — a browser, a fixture, a command line — asks.
///
/// ```
/// use majordomus_core::digest_of_json_text;
/// assert_eq!(digest_of_json_text(r#"{"b":1,"a":2}"#), digest_of_json_text(r#"{ "a": 2, "b": 1 }"#));
/// assert!(digest_of_json_text("{not json").is_err());
/// ```
pub fn digest_of_json_text(text: &str) -> Result<String, String> {
    serde_json::from_str::<Value>(text)
        .map(|v| canonical_digest_hex(&v))
        .map_err(|e| e.to_string())
}
