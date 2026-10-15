//! Node identity: who this machine's Majordomus is, independently of any repository,
//! process, address or port. The identity is an Ed25519 keypair created once per user
//! and machine and kept under the user's state directory; the node id is a digest of
//! the public key, so claiming an id means holding the key, and a hostname, an IP
//! address or a PID is never identity — those are runtime attributes that change.
//!
//! The signing key never leaves this module: everything above it sees [`NodeIdentity`],
//! which signs and exposes public material only. The instance id distinguishes runs of
//! the same node — a machine that restarts keeps its node id and changes its instance.
//!
//! ```
//! use majordomus_cli::mesh::identity::{verify, NodeIdentity};
//!
//! let node = NodeIdentity::ephemeral().unwrap();
//! let signature = node.sign(b"a majordomus exists here");
//! assert!(verify(&node.public.public_key, b"a majordomus exists here", &signature));
//! assert!(!verify(&node.public.public_key, b"something else", &signature));
//! ```

use std::fmt;
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::MeshError;

/// The stable identity of a node: 32 hex characters of the SHA-256 of its Ed25519 public
/// key. Derived, never chosen: two nodes with one id hold one key.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
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

    /// The id as text: 32 lowercase hex characters, the form listings and allowlists carry.
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

    /// The id as text: 16 hex characters, stable for the life of this process alone.
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
        let file: IdentityFile = serde_json::from_str(&text).map_err(|e| {
            MeshError::Identity(format!("{}: not an identity file: {e}", path.display()))
        })?;
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
            MeshError::Identity(format!(
                "{}: the secret key is not 32 bytes",
                path.display()
            ))
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
        let text =
            serde_json::to_string_pretty(&file).map_err(|e| MeshError::Identity(e.to_string()))?;
        write_private(path, &text)
    }

    /// Sign a message with this node's key.
    pub fn sign(&self, message: &[u8]) -> String {
        hex(&self.signing.sign(message).to_bytes())
    }

    /// Replace the machine's key: make a new one, have this key sign a [`Succession`] naming
    /// it and the new key countersign, and write the new key to `path` (owner-only from its
    /// first byte), which destroys the old secret on disk. The identity in memory keeps the
    /// old key, because a running runtime still closes its old-key stream with it (ADR 0067,
    /// addendum of 2026-10-15). The display name carries over; the node id does not, because
    /// it is the new key's digest. Nothing is written when the statement cannot be made.
    ///
    /// ```
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// let dir = tempfile::tempdir().unwrap();
    /// let path = dir.path().join("node.json");
    /// let old = NodeIdentity::load_or_create(&path).unwrap();
    /// let (new, succession) = old.rotate(&path).unwrap();
    /// assert!(succession.verify().is_ok());
    /// assert_eq!(succession.new_key, new.public.public_key);
    /// assert_eq!(
    ///     NodeIdentity::load_or_create(&path).unwrap().public.node_id,
    ///     new.public.node_id,
    ///     "the file holds the new key"
    /// );
    /// ```
    pub fn rotate(&self, path: &Path) -> Result<(NodeIdentity, Succession), MeshError> {
        let mut new = Self::create()?;
        new.public.display_name = self.public.display_name.clone();
        let at = crate::peers::rfc3339(std::time::SystemTime::now());
        let succession = Succession::sign(self, &new, &at)?;
        new.persist(path)?;
        Ok((new, succession))
    }
}

/// The succession statement's format version, and the domain both of its signatures are
/// made under, so that no other signed message of the mesh can be read as one.
pub const SUCCESSION_SCHEMA: &str = "majordomus-mesh-succession/v1";

/// A node key handing over to its successor: the old key signs that it hands over to the
/// new one, and the new key countersigns that its holder exists and agreed. The statement
/// names no repository, because the key it succeeds is the machine's, and it carries public
/// halves only. What a succession confers is decided in [`super::trust`], never here: a
/// statement that verifies proves who signed it, not that anyone should follow it.
///
/// ```
/// use majordomus_cli::mesh::identity::{NodeIdentity, Succession};
/// let old = NodeIdentity::ephemeral().unwrap();
/// let new = NodeIdentity::ephemeral().unwrap();
/// let statement = Succession::sign(&old, &new, "2026-10-15T08:00:00Z").unwrap();
/// assert!(statement.verify().is_ok());
/// let wire = serde_json::to_string(&statement).unwrap();
/// let read: Succession = serde_json::from_str(&wire).unwrap();
/// assert!(read.verify().is_ok(), "the statement survives its serialization");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Succession {
    /// [`SUCCESSION_SCHEMA`]; any other value is refused by [`Succession::verify`].
    pub schema: String,
    /// The key handing over, 64 hex characters.
    pub old_key: String,
    /// The key taking over, 64 hex characters; never the old key.
    pub new_key: String,
    /// When the old key signed, RFC 3339. One line: it is part of the signed bytes.
    pub at: String,
    /// The old key's signature over [`Succession::signing_bytes`].
    pub old_signature: String,
    /// The new key's countersignature over [`Succession::countersigning_bytes`], which
    /// include the old signature, so it agrees to this hand-over and no other.
    pub new_signature: String,
}

impl Succession {
    /// The statement `old` signs and `new` countersigns, made at `at`. Refused when the two
    /// are one key or `at` is not one non-empty line, because neither would verify.
    ///
    /// ```
    /// use majordomus_cli::mesh::identity::{NodeIdentity, Succession};
    /// let node = NodeIdentity::ephemeral().unwrap();
    /// assert!(Succession::sign(&node, &node, "2026-10-15T08:00:00Z").is_err());
    /// ```
    pub fn sign(old: &NodeIdentity, new: &NodeIdentity, at: &str) -> Result<Self, MeshError> {
        let mut statement = Succession {
            schema: SUCCESSION_SCHEMA.into(),
            old_key: old.public.public_key.clone(),
            new_key: new.public.public_key.clone(),
            at: at.into(),
            old_signature: String::new(),
            new_signature: String::new(),
        };
        statement.check_shape().map_err(MeshError::Identity)?;
        statement.old_signature = old.sign(&statement.signing_bytes());
        statement.new_signature = new.sign(&statement.countersigning_bytes());
        Ok(statement)
    }

    /// The canonical bytes the old key signs: the domain, the role `hand-over`, both keys and
    /// the time, one per line. The role keeps one signature from being replayed as the other.
    ///
    /// ```
    /// use majordomus_cli::mesh::identity::{NodeIdentity, Succession};
    /// let s = Succession::sign(
    ///     &NodeIdentity::ephemeral().unwrap(),
    ///     &NodeIdentity::ephemeral().unwrap(),
    ///     "2026-10-15T08:00:00Z",
    /// )
    /// .unwrap();
    /// assert!(s.signing_bytes().starts_with(b"majordomus-mesh-succession/v1\nhand-over\n"));
    /// ```
    pub fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "{}\nhand-over\n{}\n{}\n{}",
            self.schema, self.old_key, self.new_key, self.at
        )
        .into_bytes()
    }

    /// The canonical bytes the new key countersigns: the domain, the role `take-over`, both
    /// keys, the time and the old key's signature, one per line.
    ///
    /// ```
    /// use majordomus_cli::mesh::identity::{NodeIdentity, Succession};
    /// let s = Succession::sign(
    ///     &NodeIdentity::ephemeral().unwrap(),
    ///     &NodeIdentity::ephemeral().unwrap(),
    ///     "2026-10-15T08:00:00Z",
    /// )
    /// .unwrap();
    /// let bytes = String::from_utf8(s.countersigning_bytes()).unwrap();
    /// assert!(bytes.ends_with(&s.old_signature), "the countersignature covers the signature");
    /// ```
    pub fn countersigning_bytes(&self) -> Vec<u8> {
        format!(
            "{}\ntake-over\n{}\n{}\n{}\n{}",
            self.schema, self.old_key, self.new_key, self.at, self.old_signature
        )
        .into_bytes()
    }

    /// Whether the statement is what it says: the schema is [`SUCCESSION_SCHEMA`], both keys
    /// parse and differ, `at` is one non-empty line, the old key's signature covers
    /// [`Succession::signing_bytes`] and the new key's covers
    /// [`Succession::countersigning_bytes`]. The error says which part failed; malformed
    /// material is an error, never a panic.
    ///
    /// ```
    /// use majordomus_cli::mesh::identity::{NodeIdentity, Succession};
    /// let mut s = Succession::sign(
    ///     &NodeIdentity::ephemeral().unwrap(),
    ///     &NodeIdentity::ephemeral().unwrap(),
    ///     "2026-10-15T08:00:00Z",
    /// )
    /// .unwrap();
    /// s.at = "2026-10-16T08:00:00Z".into();
    /// assert!(s.verify().is_err(), "a changed time is not what the old key signed");
    /// ```
    pub fn verify(&self) -> Result<(), String> {
        self.check_shape()?;
        if !verify(&self.old_key, &self.signing_bytes(), &self.old_signature) {
            return Err("the old key's signature does not verify".into());
        }
        if !verify(
            &self.new_key,
            &self.countersigning_bytes(),
            &self.new_signature,
        ) {
            return Err("the new key's countersignature does not verify".into());
        }
        Ok(())
    }

    /// Everything [`Succession::verify`] checks before a signature.
    fn check_shape(&self) -> Result<(), String> {
        if self.schema != SUCCESSION_SCHEMA {
            return Err(format!("schema {} is not {SUCCESSION_SCHEMA}", self.schema));
        }
        if node_id_of_key(&self.old_key).is_none() {
            return Err("the old key is not a key".into());
        }
        if node_id_of_key(&self.new_key).is_none() {
            return Err("the new key is not a key".into());
        }
        if self.old_key == self.new_key {
            return Err("a key cannot succeed itself".into());
        }
        if self.at.is_empty() || self.at.contains(['\n', '\r']) {
            return Err("the time is not one non-empty line".into());
        }
        Ok(())
    }
}

/// Verify `signature` (hex) over `message` against `public_key` (hex). False on any
/// malformed material: a verifier never panics on hostile input.
///
/// ```
/// use majordomus_cli::mesh::identity::verify;
/// assert!(!verify("zz", b"m", "zz"), "garbage material is false, not a panic");
/// ```
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

/// Give the identity at `path` a display label, keeping its key: the node id, and every
/// signature it ever made, are unchanged. The label is presentation — what a person reads
/// as `macbook-pro` in a list of devices — and is refused unless it is a short name of
/// letters, digits, `.`, `_` and `-`, because it is printed wherever this node is named.
///
/// ```
/// use majordomus_cli::mesh::identity::{relabel, NodeIdentity};
/// let dir = tempfile::tempdir().unwrap();
/// let path = dir.path().join("node.json");
/// let before = NodeIdentity::load_or_create(&path).unwrap();
/// let after = relabel(&path, "mac-mini").unwrap();
/// assert_eq!(after.public.display_name, "mac-mini");
/// assert_eq!(after.public.node_id, before.public.node_id, "the key is the identity");
/// assert!(relabel(&path, "two words").is_err());
/// ```
pub fn relabel(path: &Path, label: &str) -> Result<NodeIdentity, MeshError> {
    let valid = !label.is_empty()
        && label.len() <= 64
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !valid {
        return Err(MeshError::Identity(format!(
            "`{label}` is not a device label: 1 to 64 letters, digits, `.`, `_` or `-`"
        )));
    }
    // a file that is not an identity is refused here, before anything is written
    let identity = NodeIdentity::load_or_create(path)?;
    // the moment the key was made stays what the file says it was
    let created_at = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<IdentityFile>(&text).ok())
        .map(|file| file.created_at)
        .unwrap_or_default();
    let file = IdentityFile {
        schema: SCHEMA.into(),
        secret_key: hex(&identity.signing.to_bytes()),
        display_name: Some(label.to_string()),
        created_at,
    };
    let text = serde_json::to_string_pretty(&file).expect("an identity file is plain data");
    write_private(path, &text)?;
    NodeIdentity::load(path)
}

/// Write `text` to `path` with owner-only permissions, atomically enough for one user's
/// state directory: a temp file beside the target, then a rename.
///
/// The temp file is created owner-only. It was once written at the umask's mode and narrowed
/// afterwards, with the narrowing's failure ignored, so a secret key stood readable by the
/// group for as long as the write took, and for good when the narrowing failed. A stale temp
/// file is removed first, because one that already exists keeps the mode it was created with.
fn write_private(path: &Path, text: &str) -> Result<(), MeshError> {
    use std::io::Write;
    let tmp = path.with_extension("tmp");
    let fail = |p: &Path, e: std::io::Error| MeshError::Identity(format!("{}: {e}", p.display()));
    match std::fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(fail(&tmp, e)),
    }
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.mode(0o600);
    }
    let mut file = open.open(&tmp).map_err(|e| fail(&tmp, e))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| fail(&tmp, e))?;
    drop(file);
    std::fs::rename(&tmp, path).map_err(|e| fail(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node key is owner-only from the moment it exists, whatever the umask, and a stale
    /// temp file left by an interrupted write does not lend it its mode.
    #[cfg(unix)]
    #[test]
    fn a_node_key_is_never_readable_by_others() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("node.json");
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, "stale").unwrap();
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o644)).unwrap();
        NodeIdentity::load_or_create(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "the key file is {mode:o}");
        assert!(
            !tmp.exists(),
            "the temp file is renamed, not left beside the key"
        );
        // rewritten with a label, it stays owner-only
        write_private(&path, "{}").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "the rewritten key file is {mode:o}");
    }

    #[test]
    fn a_label_is_a_short_name_and_changes_nothing_but_itself() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("node.json");
        let before = NodeIdentity::load_or_create(&path).unwrap();
        let created = |p: &Path| {
            let text = std::fs::read_to_string(p).unwrap();
            serde_json::from_str::<IdentityFile>(&text)
                .unwrap()
                .created_at
        };
        let made = created(&path);
        let after = relabel(&path, "mac-mini_2.local").unwrap();
        assert_eq!(after.public.display_name, "mac-mini_2.local");
        assert_eq!(after.public.public_key, before.public.public_key);
        assert_eq!(created(&path), made, "the key is as old as it was");

        for label in ["", "two words", "semi;colon", &"x".repeat(65)] {
            let err = relabel(&path, label).unwrap_err().to_string();
            assert!(err.contains("is not a device label"), "{label}: {err}");
        }
        // an absent identity is made, already labelled
        let fresh = dir.path().join("deeper/node.json");
        assert_eq!(relabel(&fresh, "new").unwrap().public.display_name, "new");
    }

    #[test]
    fn a_file_that_is_not_an_identity_is_never_relabelled_into_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("node.json");
        std::fs::write(&path, "not an identity").unwrap();
        assert!(relabel(&path, "mac").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not an identity");

        // the label cannot be written: the temporary file's place is taken
        let path = dir.path().join("other.json");
        NodeIdentity::load_or_create(&path).unwrap();
        std::fs::create_dir(path.with_extension("tmp")).unwrap();
        assert!(relabel(&path, "mac").is_err());
    }

    #[test]
    fn an_identity_is_created_once_and_loaded_stable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("node.json");
        let first = NodeIdentity::load_or_create(&path).unwrap();
        let second = NodeIdentity::load_or_create(&path).unwrap();
        assert_eq!(
            first.public.node_id, second.public.node_id,
            "the node id survives a reload"
        );
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
        assert!(
            !verify("zz", b"hello mesh", &sig),
            "malformed key material is false, not a panic"
        );
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
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "not json",
            "the file is untouched"
        );
    }

    /// I2705: a rotation is a statement the old key signs and the new key countersigns, the
    /// file then holds the new key alone and owner-only, and a forged or altered statement
    /// does not verify.
    #[test]
    fn a_rotation_is_signed_by_the_old_key_and_countersigned_by_the_new() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("node.json");
        let old = relabel(&path, "mac-mini").unwrap();
        let (new, succession) = old.rotate(&path).unwrap();

        assert_eq!(succession.schema, SUCCESSION_SCHEMA);
        assert_eq!(succession.old_key, old.public.public_key);
        assert_eq!(succession.new_key, new.public.public_key);
        assert_ne!(
            new.public.node_id, old.public.node_id,
            "a new key is a new node"
        );
        assert_eq!(
            new.public.display_name, "mac-mini",
            "the label carries over"
        );
        assert_eq!(succession.verify(), Ok(()));
        assert!(verify(
            &old.public.public_key,
            &succession.signing_bytes(),
            &succession.old_signature
        ));
        assert!(verify(
            &new.public.public_key,
            &succession.countersigning_bytes(),
            &succession.new_signature
        ));

        // the file holds the new key, owner-only, and the old secret is gone from it
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains(&hex(&old.signing.to_bytes())));
        assert!(text.contains(&hex(&new.signing.to_bytes())));
        let reloaded = NodeIdentity::load_or_create(&path).unwrap();
        assert_eq!(reloaded.public.node_id, new.public.node_id);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "the rotated key file is {mode:o}");
        }

        // the statement survives its serialization byte for byte
        let wire = serde_json::to_string(&succession).unwrap();
        let read: Succession = serde_json::from_str(&wire).unwrap();
        assert_eq!(read, succession);
        assert_eq!(read.verify(), Ok(()));

        // a countersignature by any key but the named successor is a forgery
        let thief = NodeIdentity::ephemeral().unwrap();
        let mut forged = succession.clone();
        forged.new_signature = thief.sign(&forged.countersigning_bytes());
        assert!(forged.verify().unwrap_err().contains("countersignature"));
        // and naming the thief instead breaks the old key's signature
        let mut forged = succession.clone();
        forged.new_key = thief.public.public_key.clone();
        forged.new_signature = thief.sign(&forged.countersigning_bytes());
        assert!(forged.verify().unwrap_err().contains("old key's signature"));
        // one signature cannot stand in for the other: the roles differ
        let mut swapped = succession.clone();
        swapped.new_signature = new.sign(&swapped.signing_bytes());
        assert!(swapped.verify().is_err());
        // a countersignature is bound to the exact signature it countersigned
        let mut resigned = succession.clone();
        resigned.old_signature = thief.sign(b"anything");
        assert!(resigned.verify().is_err());

        for (field, value) in [
            ("schema", "majordomus-mesh-succession/v2"),
            ("old_key", "zz"),
            ("new_key", ""),
            ("at", "2026-10-15T08:00:00Z\nextra"),
            ("at", ""),
            ("old_signature", "not hex"),
            ("new_signature", "00"),
        ] {
            let mut bad = serde_json::to_value(&succession).unwrap();
            bad[field] = value.into();
            let bad: Succession = serde_json::from_value(bad).unwrap();
            assert!(bad.verify().is_err(), "{field} = {value:?} verified");
        }
        assert!(Succession::sign(&old, &old, "2026-10-15T08:00:00Z").is_err());
        assert!(Succession::sign(&old, &new, "two\nlines").is_err());
    }

    /// A rotation whose new key cannot be written leaves the old key where it was.
    #[test]
    fn a_rotation_that_cannot_write_keeps_the_old_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("node.json");
        let old = NodeIdentity::load_or_create(&path).unwrap();
        std::fs::create_dir(path.with_extension("tmp")).unwrap();
        assert!(old.rotate(&path).is_err());
        let still = NodeIdentity::load_or_create(&path).unwrap();
        assert_eq!(still.public.node_id, old.public.node_id);
    }

    #[test]
    fn debug_shows_no_secret() {
        let dir = tempfile::tempdir().unwrap();
        let identity = NodeIdentity::load_or_create(&dir.path().join("node.json")).unwrap();
        let rendered = format!("{identity:?}");
        let secret = hex(&identity.signing.to_bytes());
        assert!(
            !rendered.contains(&secret),
            "Debug must not leak the signing key"
        );
    }
}
