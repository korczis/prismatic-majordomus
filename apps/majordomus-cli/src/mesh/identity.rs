//! Node identity: who this machine's Majordomus is, independently of any repository,
//! process, address or port. The identity is an Ed25519 keypair created once per user
//! and machine and kept under the user's state directory; the node id is a digest of
//! the public key, so claiming an id means holding the key, and a hostname, an IP
//! address or a PID is never identity — those are runtime attributes that change.
//!
//! The signing key never leaves this module: everything above it sees [`NodeIdentity`],
//! which signs and exposes public material only. The instance id distinguishes runs of
//! the same node — a machine that restarts keeps its node id and changes its instance.

use std::fmt;
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::MeshError;

/// The stable identity of a node: 32 hex characters of the SHA-256 of its Ed25519 public
/// key. Derived, never chosen: two nodes with one id hold one key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct NodeId(String);

impl NodeId {
    /// The id of a public key.
    pub fn of(key: &VerifyingKey) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(32);
        for byte in digest.iter().take(16) {
            hex.push_str(&format!("{byte:02x}"));
        }
        NodeId(hex)
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One run of a node: 16 hex characters of OS entropy, new at every process start. A
/// restart keeps the node id and changes this, which is how a reader tells "the same
/// node came back" from "the same process is still there".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct InstanceId(String);

impl InstanceId {
    /// A fresh instance id from OS entropy.
    pub fn fresh() -> Result<Self, MeshError> {
        let mut bytes = [0u8; 8];
        getrandom::getrandom(&mut bytes)
            .map_err(|e| MeshError::Identity(format!("no OS entropy for an instance id: {e}")))?;
        Ok(InstanceId(hex(&bytes)))
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What everyone may know about this node: the id, the public key, a display name and
/// this run's instance. Everything an advertisement carries; nothing that signs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PublicIdentity {
    /// The node id: a digest of the public key.
    pub node_id: NodeId,
    /// The Ed25519 public key, 64 hex characters. Whoever holds the matching secret is
    /// this node; there is no other proof.
    pub public_key: String,
    /// A human name for the node, from the identity file; defaults to the first eight
    /// characters of the node id. Presentation only — never used for trust.
    pub display_name: String,
    /// This run of the node.
    pub instance_id: InstanceId,
}

/// The identity a running mesh signs with: the public half plus the signing key.
pub struct NodeIdentity {
    /// The public half, as an advertisement carries it.
    pub public: PublicIdentity,
    signing: SigningKey,
}

impl fmt::Debug for NodeIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The signing key stays out of every rendering, Debug included.
        f.debug_struct("NodeIdentity")
            .field("public", &self.public)
            .finish_non_exhaustive()
    }
}

/// The identity file: schema, the secret seed, an optional display name. Mode 0600,
/// under the user's state directory, never inside a repository.
#[derive(Debug, Serialize, Deserialize)]
struct IdentityFile {
    schema: String,
    secret_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    created_at: String,
}

/// The identity file's format version.
const SCHEMA: &str = "majordomus-mesh-identity/v1";

/// Where the identity lives: `$XDG_STATE_HOME/majordomus/node.json`, else
/// `$HOME/.local/state/majordomus/node.json`. `None` when neither variable is set —
/// a process with no home has no node identity, and the mesh stays off.
pub fn default_identity_path() -> Option<PathBuf> {
    if let Some(state) = std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(state).join("majordomus").join("node.json"));
    }
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(|home| {
            PathBuf::from(home)
                .join(".local")
                .join("state")
                .join("majordomus")
                .join("node.json")
        })
}

impl NodeIdentity {
    /// Load the identity at `path`, creating it (directories included) when absent.
    /// A malformed file is an error, never silently replaced: an identity is the one
    /// thing a node must not lose by accident.
    pub fn load_or_create(path: &Path) -> Result<Self, MeshError> {
        if path.is_file() {
            return Self::load(path);
        }
        let identity = Self::create()?;
        identity.persist(path)?;
        Ok(identity)
    }

    fn load(path: &Path) -> Result<Self, MeshError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| MeshError::Identity(format!("{}: {e}", path.display())))?;
        let file: IdentityFile = serde_json::from_str(&text)
            .map_err(|e| MeshError::Identity(format!("{}: not an identity file: {e}", path.display())))?;
        if file.schema != SCHEMA {
            return Err(MeshError::Identity(format!(
                "{}: schema {} is not {SCHEMA}",
                path.display(),
                file.schema
            )));
        }
        let seed = unhex(&file.secret_key).ok_or_else(|| {
            MeshError::Identity(format!("{}: the secret key is not hex", path.display()))
        })?;
        let seed: [u8; 32] = seed.try_into().map_err(|_| {
            MeshError::Identity(format!("{}: the secret key is not 32 bytes", path.display()))
        })?;
        let signing = SigningKey::from_bytes(&seed);
        Self::assemble(signing, file.display_name)
    }

    /// A throwaway identity that touches no disk: what the self-check and tests sign
    /// with. Never a node's real identity — nothing persists it.
    pub fn ephemeral() -> Result<Self, MeshError> {
        Self::create()
    }

    fn create() -> Result<Self, MeshError> {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed)
            .map_err(|e| MeshError::Identity(format!("no OS entropy for a node key: {e}")))?;
        let signing = SigningKey::from_bytes(&seed);
        Self::assemble(signing, None)
    }

    fn assemble(signing: SigningKey, display_name: Option<String>) -> Result<Self, MeshError> {
        let verifying = signing.verifying_key();
        let node_id = NodeId::of(&verifying);
        let display_name =
            display_name.unwrap_or_else(|| node_id.as_str().chars().take(8).collect());
        Ok(NodeIdentity {
            public: PublicIdentity {
                node_id,
                public_key: hex(verifying.as_bytes()),
                display_name,
                instance_id: InstanceId::fresh()?,
            },
            signing,
        })
    }

    fn persist(&self, path: &Path) -> Result<(), MeshError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| MeshError::Identity(format!("{}: {e}", dir.display())))?;
        }
        let file = IdentityFile {
            schema: SCHEMA.into(),
            secret_key: hex(&self.signing.to_bytes()),
            display_name: Some(self.public.display_name.clone()),
            created_at: crate::peers::rfc3339(std::time::SystemTime::now()),
        };
        let text = serde_json::to_string_pretty(&file)
            .map_err(|e| MeshError::Identity(e.to_string()))?;
        write_private(path, &text)
    }

    /// Sign a message with this node's key.
    pub fn sign(&self, message: &[u8]) -> String {
        hex(&self.signing.sign(message).to_bytes())
    }
}

/// Verify `signature` (hex) over `message` against `public_key` (hex). False on any
/// malformed material: a verifier never panics on hostile input.
pub fn verify(public_key: &str, message: &[u8], signature: &str) -> bool {
    let Some(key) = unhex(public_key).and_then(|b| <[u8; 32]>::try_from(b).ok()) else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(&key) else {
        return false;
    };
    let Some(sig) = unhex(signature).and_then(|b| <[u8; 64]>::try_from(b).ok()) else {
        return false;
    };
    key.verify(message, &Signature::from_bytes(&sig)).is_ok()
}

/// The node id a public key (hex) yields, when the key parses.
pub fn node_id_of_key(public_key: &str) -> Option<NodeId> {
    let key = unhex(public_key).and_then(|b| <[u8; 32]>::try_from(b).ok())?;
    let key = VerifyingKey::from_bytes(&key).ok()?;
    Some(NodeId::of(&key))
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Write `text` to `path` with owner-only permissions, atomically enough for one user's
/// state directory: a temp file beside the target, then a rename.
fn write_private(path: &Path, text: &str) -> Result<(), MeshError> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| MeshError::Identity(format!("{}: {e}", tmp.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, path)
        .map_err(|e| MeshError::Identity(format!("{}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identity_is_created_once_and_loaded_stable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("node.json");
        let first = NodeIdentity::load_or_create(&path).unwrap();
        let second = NodeIdentity::load_or_create(&path).unwrap();
        assert_eq!(first.public.node_id, second.public.node_id, "the node id survives a reload");
        assert_ne!(
            first.public.instance_id, second.public.instance_id,
            "every run is a new instance"
        );
    }

    #[test]
    fn a_signature_verifies_and_a_tampered_message_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let identity = NodeIdentity::load_or_create(&dir.path().join("node.json")).unwrap();
        let sig = identity.sign(b"hello mesh");
        assert!(verify(&identity.public.public_key, b"hello mesh", &sig));
        assert!(!verify(&identity.public.public_key, b"hello mess", &sig));
        assert!(!verify("zz", b"hello mesh", &sig), "malformed key material is false, not a panic");
        assert!(!verify(&identity.public.public_key, b"hello mesh", "zz"));
    }

    #[test]
    fn the_node_id_is_the_digest_of_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let identity = NodeIdentity::load_or_create(&dir.path().join("node.json")).unwrap();
        assert_eq!(
            node_id_of_key(&identity.public.public_key),
            Some(identity.public.node_id.clone())
        );
        assert_eq!(identity.public.node_id.as_str().len(), 32);
    }

    #[test]
    fn a_malformed_identity_file_is_an_error_not_a_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("node.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(NodeIdentity::load_or_create(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json", "the file is untouched");
    }

    #[test]
    fn debug_shows_no_secret() {
        let dir = tempfile::tempdir().unwrap();
        let identity = NodeIdentity::load_or_create(&dir.path().join("node.json")).unwrap();
        let rendered = format!("{identity:?}");
        let secret = hex(&identity.signing.to_bytes());
        assert!(!rendered.contains(&secret), "Debug must not leak the signing key");
    }
}
