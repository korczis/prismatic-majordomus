//! The identifiers the local contract carries, and what each one is the identity of.
//!
//! Nothing here is a new notion of "which repository". A context *is* a checkout, so its id
//! is [`CheckoutId`]'s digest of the canonical root, the value the lease and the index are
//! already keyed by; the repository a context belongs to is [`RepositoryId`], the digest of
//! git's common directory (`repository::git_identity`); the host is the mesh's
//! [`crate::mesh::identity::NodeId`], and one run of it the mesh's
//! [`crate::mesh::identity::InstanceId`]. What this module adds is the wire form of each,
//! validated where it is read, and the two identities only the host mints: a session and a
//! request.
//!
//! | Identity | Scope | Lives | Unique among |
//! |---|---|---|---|
//! | `NodeId` | the OS user on one machine | as long as its key file | every node of a mesh |
//! | `InstanceId` | one run of the host | one process | the runs of one node |
//! | [`ContextId`] | one checkout | as long as the checkout's canonical root | the checkouts of one machine |
//! | [`RepositoryId`] | one git repository, every worktree of it | as long as git's common directory | the repositories of one machine |
//! | [`SessionId`] | one client attachment to one context | until detach, revocation or the host's exit | every session a host minted |
//! | [`RequestId`] | one request | until its reply | the requests in flight on one connection |
//!
//! A [`RepositoryId`] is local: two clones of one project on two machines have two. The
//! cross-machine identity is [`crate::mesh::repository::MeshRepositoryIdentity`], derived
//! from root commits, and it belongs to host-to-host routing (a later phase), never here.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::refusal::{Refusal, RefusalCode};
use crate::session::{CheckoutId, RepositoryId};

/// The bound on a checkout root's length in bytes: `PATH_MAX` on Linux, more than macOS's.
pub const MAX_ROOT_BYTES: usize = 4096;
/// The most features one side may name.
pub const MAX_FEATURES: usize = 32;
/// The longest feature name.
pub const MAX_FEATURE_CHARS: usize = 32;
/// The longest client label.
pub const MAX_CLIENT_LABEL_CHARS: usize = 64;

const SESSION_PREFIX: &str = "hs-";

fn is_lower_hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn malformed(message: String) -> Refusal {
    Refusal::new(RefusalCode::MessageMalformed, message)
}

/// One context: a checkout the host serves, named by the digest of its canonical root.
///
/// The id names a context and grants nothing. A client that sends one is asking about that
/// context; whether it may is decided against the session the request arrives on
/// ([`super::message::Call::admit`]). It reads back only in its own grammar, 32 lowercase
/// hex characters, which other 32-hex identities share: a node id or a repository id sent
/// where a context is wanted reads, names no context of the session, and is refused there.
///
/// ```
/// use majordomus_cli::fabric::ContextId;
/// use majordomus_cli::session::CheckoutId;
/// use std::path::Path;
///
/// let id = ContextId::of(&CheckoutId::of(Path::new("/src/repo"))).unwrap();
/// assert_eq!(id.as_str().len(), 32);
/// assert!(ContextId::parse("/src/repo").is_err(), "a path is never a context id");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct ContextId(#[schemars(regex(pattern = r"^[0-9a-f]{32}$"))] String);

impl ContextId {
    /// The context of a checkout.
    pub fn of(checkout: &CheckoutId) -> Result<Self, Refusal> {
        Self::parse(checkout.as_str())
    }

    /// Read a context id, or say why it is not one.
    pub fn parse(text: &str) -> Result<Self, Refusal> {
        if is_lower_hex(text, 32) {
            Ok(ContextId(text.to_string()))
        } else {
            Err(malformed(format!(
                "a context id is 32 lowercase hex characters, and {text:?} is not"
            )))
        }
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ContextId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        ContextId::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// One client's attachment to one context: `hs-` and 32 lowercase hex characters of OS
/// entropy, minted by the host at attach and bound there to the connection and the context.
///
/// It is a handle, not a credential that travels: a session is honoured only on the
/// connection it was minted on, so a handle read off another connection, replayed after
/// detach or guessed is [`RefusalCode::SessionUnknown`]. The host takes the entropy from the
/// OS; this type only spells it.
///
/// ```
/// use majordomus_cli::fabric::SessionId;
/// let id = SessionId::from_entropy([0xab; 16]);
/// assert_eq!(id.as_str(), format!("hs-{}", "ab".repeat(16)));
/// assert_eq!(SessionId::parse(id.as_str()).unwrap(), id);
/// assert!(SessionId::parse(&"ab".repeat(16)).is_err(), "a context id is not a session");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct SessionId(#[schemars(regex(pattern = r"^hs-[0-9a-f]{32}$"))] String);

impl SessionId {
    /// The session id sixteen bytes of entropy spell.
    pub fn from_entropy(bytes: [u8; 16]) -> Self {
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        SessionId(format!("{SESSION_PREFIX}{hex}"))
    }

    /// Read a session id, or say why it is not one.
    pub fn parse(text: &str) -> Result<Self, Refusal> {
        match text.strip_prefix(SESSION_PREFIX) {
            Some(hex) if is_lower_hex(hex, 32) => Ok(SessionId(text.to_string())),
            _ => Err(malformed(format!(
                "a session id is {SESSION_PREFIX:?} and 32 lowercase hex characters, and {text:?} is not"
            ))),
        }
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SessionId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        SessionId::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// The client's number for one request, echoed on its reply. Any value; unique among the
/// requests in flight on one connection, which the host enforces
/// ([`RefusalCode::RequestDuplicate`]).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct RequestId(pub u64);

/// The name of an optional protocol feature: `[a-z][a-z0-9_-]*`, at most
/// [`MAX_FEATURE_CHARS`] characters.
///
/// ```
/// use majordomus_cli::fabric::FeatureName;
/// assert!(FeatureName::parse("cancel").is_ok());
/// assert!(FeatureName::parse("Cancel").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct FeatureName(#[schemars(regex(pattern = r"^[a-z][a-z0-9_-]{0,31}$"))] String);

impl FeatureName {
    /// Read a feature name, or say why it is not one.
    pub fn parse(text: &str) -> Result<Self, Refusal> {
        let mut chars = text.chars();
        let ok = chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
            && text.len() <= MAX_FEATURE_CHARS;
        if ok {
            Ok(FeatureName(text.to_string()))
        } else {
            Err(malformed(format!(
                "a feature name is [a-z][a-z0-9_-]*, at most {MAX_FEATURE_CHARS} characters, and {text:?} is not"
            )))
        }
    }

    /// The name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for FeatureName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        FeatureName::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// A set of feature names, serialised sorted and without repetition whatever order it was
/// read in, and at most [`MAX_FEATURES`] long.
///
/// ```
/// use majordomus_cli::fabric::FeatureSet;
/// let set: FeatureSet = serde_json::from_str(r#"["b", "a", "b"]"#).unwrap();
/// assert_eq!(serde_json::to_string(&set).unwrap(), r#"["a","b"]"#);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct FeatureSet(#[schemars(length(max = 32))] BTreeSet<FeatureName>);

impl FeatureSet {
    /// The set of these names.
    pub fn of(names: &[&str]) -> Result<Self, Refusal> {
        let names = names
            .iter()
            .map(|n| FeatureName::parse(n))
            .collect::<Result<Vec<_>, _>>()?;
        Self::bounded(names)
    }

    fn bounded(names: Vec<FeatureName>) -> Result<Self, Refusal> {
        if names.len() > MAX_FEATURES {
            return Err(malformed(format!(
                "at most {MAX_FEATURES} features are named, and this names {}",
                names.len()
            )));
        }
        Ok(FeatureSet(names.into_iter().collect()))
    }

    /// Whether the set names this feature.
    pub fn contains(&self, name: &str) -> bool {
        self.0.iter().any(|f| f.as_str() == name)
    }

    /// The names, sorted.
    pub fn iter(&self) -> impl Iterator<Item = &FeatureName> {
        self.0.iter()
    }

    /// The names in both sets.
    pub fn intersection(&self, other: &FeatureSet) -> FeatureSet {
        FeatureSet(self.0.intersection(&other.0).cloned().collect())
    }

    /// The names in either set.
    pub fn union(&self, other: &FeatureSet) -> FeatureSet {
        FeatureSet(self.0.union(&other.0).cloned().collect())
    }

    /// The names of this set the other lacks.
    pub fn missing_from(&self, other: &FeatureSet) -> Vec<&str> {
        self.0
            .difference(&other.0)
            .map(FeatureName::as_str)
            .collect()
    }
}

impl<'de> Deserialize<'de> for FeatureSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let names = Vec::<FeatureName>::deserialize(deserializer)?;
        FeatureSet::bounded(names).map_err(serde::de::Error::custom)
    }
}

/// What a client says it is (`majordomus-mcp`, `majordomus-cli`): presentation for the
/// host's listings, at most [`MAX_CLIENT_LABEL_CHARS`] characters and no control character.
/// Never identity and never authority; the host knows the peer from the socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct ClientLabel(#[schemars(length(min = 1, max = 64))] String);

impl ClientLabel {
    /// Read a client label, or say why it is not one.
    ///
    /// ```
    /// use majordomus_cli::fabric::ClientLabel;
    /// assert_eq!(ClientLabel::parse("majordomus-mcp").unwrap().as_str(), "majordomus-mcp");
    /// assert!(ClientLabel::parse("a\nb").is_err());
    /// ```
    pub fn parse(text: &str) -> Result<Self, Refusal> {
        let length = text.chars().count();
        if length == 0 || length > MAX_CLIENT_LABEL_CHARS || text.chars().any(char::is_control) {
            return Err(malformed(format!(
                "a client label is 1 to {MAX_CLIENT_LABEL_CHARS} characters with no control \
                 character, and this one has {length}"
            )));
        }
        Ok(ClientLabel(text.to_string()))
    }

    /// The label as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ClientLabel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        ClientLabel::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// A checkout root as a client names it: a request for resolution, never proof of anything.
///
/// Read only when it is absolute, at most [`MAX_ROOT_BYTES`] long, free of NUL, and normal:
/// no `.` and no `..` component, so that what a client names and what the host resolves can
/// differ by a symbolic link and nothing else. Unicode is taken as sent, unnormalised; the
/// filesystem's canonicalisation, which the host runs, is the only judge of which directory
/// a spelling names ([`ContextDescriptor::resolve`]).
///
/// ```
/// use majordomus_cli::fabric::CheckoutRoot;
/// assert!(CheckoutRoot::parse("/src/repo").is_ok());
/// assert!(CheckoutRoot::parse("src/repo").is_err(), "relative");
/// assert!(CheckoutRoot::parse("/src/../etc").is_err(), "not normal");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct CheckoutRoot(#[schemars(length(min = 1, max = 4096))] String);

impl CheckoutRoot {
    /// Read a root, or say why it is not one ([`RefusalCode::RootInvalid`]).
    pub fn parse(text: &str) -> Result<Self, Refusal> {
        let invalid = |why: &str| Err(Refusal::new(RefusalCode::RootInvalid, why.to_string()));
        if text.len() > MAX_ROOT_BYTES {
            return invalid("a checkout root is at most 4096 bytes");
        }
        if text.contains('\0') {
            return invalid("a checkout root carries no NUL");
        }
        let path = Path::new(text);
        if !path.is_absolute() {
            return invalid("a checkout root is an absolute path");
        }
        if path
            .components()
            .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
        {
            return invalid("a checkout root has no '.' or '..' component");
        }
        Ok(CheckoutRoot(text.to_string()))
    }

    /// The root as named.
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl<'de> Deserialize<'de> for CheckoutRoot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        CheckoutRoot::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// What the host established about a context, derived on the host from the filesystem and
/// git and never from what the client sent: the client's root is the starting point of the
/// walk, and every field here is the host's own reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContextDescriptor {
    /// The context: the digest of the canonical root.
    pub id: ContextId,
    /// The canonical root, which differs from the one asked for by symbolic links and by
    /// the walk up to the nearest `.ai/` layer. Sent only to the client that asked.
    pub root: PathBuf,
    /// The git repository the checkout belongs to; absent when git cannot be asked, since
    /// a layer does not have to be version controlled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<RepositoryId>,
    /// Whether the checkout is a linked worktree rather than the primary one.
    pub linked: bool,
}

impl ContextDescriptor {
    /// Resolve a root a client named into the context the host would serve: the root is
    /// canonicalised, the nearest `.ai/` layer at or above it is the checkout, and its id
    /// and repository are computed as the lease and the index compute them. One
    /// canonicalisation, one manifest read and one `git rev-parse`.
    ///
    /// ```
    /// use majordomus_cli::fabric::{CheckoutRoot, ContextDescriptor, RefusalCode};
    /// let plain = tempfile::tempdir().unwrap();
    /// let root = CheckoutRoot::parse(plain.path().to_str().unwrap()).unwrap();
    /// let refused = ContextDescriptor::resolve(&root).unwrap_err();
    /// assert_eq!(refused.code(), RefusalCode::RootUnresolved, "no .ai/ layer here");
    /// ```
    pub fn resolve(root: &CheckoutRoot) -> Result<Self, Refusal> {
        let repository = crate::repository::Repository::discover(root.as_path())
            .map_err(|e| Refusal::new(RefusalCode::RootUnresolved, e.to_string()))?;
        let canonical = repository.root().to_path_buf();
        let git = crate::repository::git_identity(&canonical);
        Ok(ContextDescriptor {
            // the digest this executable computes is the grammar's by construction
            id: ContextId(CheckoutId::of(&canonical).as_str().to_string()),
            repository: RepositoryId::of(&canonical),
            linked: git.is_some_and(|g| g.linked),
            root: canonical,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::process::Command;

    const MANIFEST: &str = "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n  rules: repo/rules\n";

    fn layer(root: &Path) {
        std::fs::create_dir_all(root.join(".ai")).unwrap();
        std::fs::write(root.join(".ai/manifest.yaml"), MANIFEST).unwrap();
    }

    fn git(root: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["-c", "user.email=t@e.x", "-c", "user.name=t"])
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }

    fn root_of(path: &Path) -> CheckoutRoot {
        CheckoutRoot::parse(path.to_str().unwrap()).unwrap()
    }

    #[test]
    fn identifiers_of_one_scope_never_read_as_another() {
        let session = SessionId::from_entropy([7; 16]);
        let context = ContextId::parse(&"07".repeat(16)).unwrap();
        assert!(ContextId::parse(session.as_str()).is_err());
        assert!(SessionId::parse(context.as_str()).is_err());
        assert!(SessionId::parse("hs-").is_err());
        assert!(
            SessionId::parse(&format!("hs-{}", "AB".repeat(16))).is_err(),
            "uppercase"
        );
        assert!(ContextId::parse(&"0".repeat(31)).is_err(), "short");
        assert!(ContextId::parse(&"g".repeat(32)).is_err(), "not hex");
        let refused = ContextId::parse("x").unwrap_err();
        assert_eq!(refused.code(), RefusalCode::MessageMalformed);
        let checkout = CheckoutId::of(Path::new("/src/repo"));
        assert_eq!(
            ContextId::of(&checkout).unwrap().as_str(),
            checkout.as_str()
        );
        assert!(
            ContextId::of(&CheckoutId::recorded("2b0f1c8e")).is_err(),
            "an old spelling"
        );
    }

    #[test]
    fn identifiers_read_back_through_serde_and_refuse_a_bad_spelling() {
        let context = ContextId::parse(&"a".repeat(32)).unwrap();
        let wire = serde_json::to_value(&context).unwrap();
        assert_eq!(serde_json::from_value::<ContextId>(wire).unwrap(), context);
        assert!(serde_json::from_str::<ContextId>(r#""nope""#).is_err());
        let session = SessionId::from_entropy([1; 16]);
        let wire = serde_json::to_value(&session).unwrap();
        assert_eq!(serde_json::from_value::<SessionId>(wire).unwrap(), session);
        assert!(serde_json::from_str::<SessionId>(r#""hs-zz""#).is_err());
        let label: ClientLabel = serde_json::from_str(r#""cli""#).unwrap();
        assert_eq!(label.as_str(), "cli");
        assert!(serde_json::from_str::<ClientLabel>(r#""""#).is_err());
        assert!(ClientLabel::parse(&"x".repeat(65)).is_err());
        let root: CheckoutRoot = serde_json::from_str(r#""/a/b""#).unwrap();
        assert_eq!(root.as_path(), Path::new("/a/b"));
        assert!(serde_json::from_str::<CheckoutRoot>(r#""a/b""#).is_err());
        assert!(serde_json::from_str::<FeatureName>(r#""-x""#).is_err());
        assert!(serde_json::from_str::<RequestId>("7").unwrap() == RequestId(7));
    }

    #[test]
    fn a_root_is_refused_unless_absolute_bounded_and_normal() {
        let code = |text: &str| CheckoutRoot::parse(text).unwrap_err().code();
        assert_eq!(code("relative/path"), RefusalCode::RootInvalid);
        assert_eq!(code("/a/./b"), RefusalCode::RootInvalid);
        assert_eq!(code("/a/\0/b"), RefusalCode::RootInvalid);
        assert_eq!(
            code(&format!("/{}", "a".repeat(MAX_ROOT_BYTES))),
            RefusalCode::RootInvalid
        );
        assert!(CheckoutRoot::parse(&format!("/{}", "a".repeat(MAX_ROOT_BYTES - 1))).is_ok());
        // Unicode is carried as sent: NFC and NFD spellings stay two strings here, and only
        // the filesystem decides whether they name one directory
        let nfc = CheckoutRoot::parse("/src/P\u{0159}\u{00ed}ru\u{010d}ka").unwrap();
        let nfd = CheckoutRoot::parse("/src/Pr\u{030c}i\u{0301}ruc\u{030c}ka").unwrap();
        assert_ne!(nfc, nfd);
    }

    #[test]
    fn feature_sets_are_bounded_sorted_and_compared_by_name() {
        let names: Vec<String> = (0..=MAX_FEATURES).map(|i| format!("f{i}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        assert_eq!(
            FeatureSet::of(&refs).unwrap_err().code(),
            RefusalCode::MessageMalformed
        );
        let too_many = serde_json::to_string(&names).unwrap();
        assert!(serde_json::from_str::<FeatureSet>(&too_many).is_err());
        assert!(FeatureSet::of(&["UPPER"]).is_err());
        assert!(FeatureName::parse(&"a".repeat(MAX_FEATURE_CHARS + 1)).is_err());
        let a = FeatureSet::of(&["cancel", "subscribe"]).unwrap();
        let b = FeatureSet::of(&["cancel"]).unwrap();
        assert!(a.contains("cancel") && !b.contains("subscribe"));
        assert_eq!(a.intersection(&b), b);
        assert_eq!(b.union(&a), a);
        assert_eq!(a.missing_from(&b), ["subscribe"]);
        let listed: Vec<&str> = a.iter().map(FeatureName::as_str).collect();
        assert_eq!(listed, ["cancel", "subscribe"]);
    }

    #[test]
    fn a_context_is_its_checkout_and_two_worktrees_share_a_repository() {
        let dir = tempfile::tempdir().unwrap();
        let primary = dir.path().join("repo");
        std::fs::create_dir_all(&primary).unwrap();
        git(&primary, &["init", "-q", "."]);
        layer(&primary);
        git(&primary, &["add", "."]);
        git(&primary, &["commit", "-q", "-m", "init"]);
        let linked = dir.path().join("repo-wt");
        git(
            &primary,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feature/x",
                linked.to_str().unwrap(),
            ],
        );

        let a = ContextDescriptor::resolve(&root_of(&primary)).unwrap();
        let b = ContextDescriptor::resolve(&root_of(&linked)).unwrap();
        assert_ne!(a.id, b.id, "two checkouts are two contexts");
        assert_eq!(a.repository, b.repository, "of one repository");
        assert!(a.repository.is_some());
        assert!(!a.linked && b.linked);
        let canonical = primary.canonicalize().unwrap();
        assert_eq!(a.root, canonical);
        assert_eq!(
            a.id.as_str(),
            CheckoutId::of(&canonical).as_str(),
            "the lease's key"
        );

        // a directory inside the checkout resolves to the checkout
        let inner = primary.join(".ai");
        assert_eq!(
            ContextDescriptor::resolve(&root_of(&inner)).unwrap().id,
            a.id
        );

        // a symbolic link to the checkout is the same context, never a second one
        #[cfg(unix)]
        {
            let link = dir.path().join("alias");
            std::os::unix::fs::symlink(&primary, &link).unwrap();
            assert_eq!(ContextDescriptor::resolve(&root_of(&link)).unwrap(), a);
        }

        let wire = serde_json::to_value(&a).unwrap();
        assert_eq!(
            serde_json::from_value::<ContextDescriptor>(wire).unwrap(),
            a
        );
    }

    #[test]
    fn a_layer_outside_git_is_a_context_with_no_repository() {
        let dir = tempfile::tempdir().unwrap();
        layer(dir.path());
        let descriptor = ContextDescriptor::resolve(&root_of(dir.path())).unwrap();
        assert!(descriptor.repository.is_none() && !descriptor.linked);
        let wire = serde_json::to_value(&descriptor).unwrap();
        assert!(wire.get("repository").is_none(), "absent, not null: {wire}");
        let missing = root_of(&dir.path().join("gone"));
        assert_eq!(
            ContextDescriptor::resolve(&missing).unwrap_err().code(),
            RefusalCode::RootUnresolved
        );
    }

    proptest! {
        #[test]
        fn every_entropy_spells_a_session_that_reads_back(bytes in any::<[u8; 16]>()) {
            let id = SessionId::from_entropy(bytes);
            prop_assert_eq!(SessionId::parse(id.as_str()).unwrap(), id.clone());
            prop_assert!(ContextId::parse(id.as_str()).is_err());
        }

        #[test]
        fn no_string_reads_as_both_a_context_and_a_session(text in ".{0,40}") {
            prop_assert!(!(ContextId::parse(&text).is_ok() && SessionId::parse(&text).is_ok()));
        }
    }
}
