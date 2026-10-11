//! The derive-offload declaration: which machines may derive a branch of this repository
//! for a session, and the limits each must meet. One typed object of the `.ai/` layer (kind
//! `derive-offload`, schema `derive-offload/v1`), parsed here and nowhere else, so a host
//! address, a free-disk minimum or a queue path is written once — in
//! `.ai/repo/offload/*.yaml` — and never typed into a script.

use std::collections::BTreeSet;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::Object;

/// The kind the declaration is discovered under.
pub const KIND: &str = "derive-offload";

/// The schema version this executable reads.
pub const SCHEMA_VERSION: &str = "derive-offload/v1";

/// The whole declaration. Unknown keys are refused, so a typo is an error and not a
/// silently ignored limit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OffloadDeclaration {
    /// The format version: `derive-offload/v1`.
    pub schema: String,
    /// The kind: `derive-offload`.
    pub kind: String,
    /// The declaration's identity within the kind.
    pub id: String,
    /// What the declaration is for, in a sentence a person reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The ref the bundle is cut against: the host has every commit it reaches, so only the
    /// branch's own commits travel.
    #[serde(default = "default_base")]
    pub base: String,
    /// The free space, in GiB under the host's home directory, below which a host is not
    /// chosen. A host may raise or lower it for itself.
    #[serde(default = "default_min_free")]
    pub min_free_gib: u64,
    /// The FIFO queue and the lock on every host.
    #[serde(default)]
    pub queue: QueueConfig,
    /// Where the checkout, its worktrees and the shared build directory are on a host, and
    /// what runs there.
    #[serde(default)]
    pub remote: RemoteLayout,
    /// The machines that may derive, in the order a tie between them is broken.
    pub hosts: Vec<OffloadHost>,
}

/// The queue every waiter on one host joins. Its paths are the ones the v2 script used, so
/// a waiter of either kind sees the other's ticket and lock while sessions move over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueueConfig {
    /// The directory of tickets, absolute on the host: one file per waiter, named
    /// `<arrival>-<pid>-<tag>`, served oldest first.
    #[serde(default = "default_queue_dir")]
    pub directory: String,
    /// The lock directory, absolute on the host, made atomically by `mkdir`. Holds one empty
    /// file per process of the derive that holds it, named by its pid.
    #[serde(default = "default_lock_dir")]
    pub lock: String,
    /// Seconds between looks at the queue.
    #[serde(default = "default_poll")]
    pub poll_seconds: u64,
    /// Seconds a waiter waits in all before it gives up.
    #[serde(default = "default_wait_max")]
    pub wait_max_seconds: u64,
}

impl Default for QueueConfig {
    fn default() -> Self {
        QueueConfig {
            directory: default_queue_dir(),
            lock: default_lock_dir(),
            poll_seconds: default_poll(),
            wait_max_seconds: default_wait_max(),
        }
    }
}

/// The layout of a host, relative to its home directory unless absolute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoteLayout {
    /// The host's checkout of this repository, which fetches the bundle.
    #[serde(default = "default_checkout")]
    pub checkout: String,
    /// Where the detached worktree of each offloaded derive is made.
    #[serde(default = "default_worktrees")]
    pub worktrees: String,
    /// The shared build directory every derive's private one is cloned from.
    #[serde(default = "default_target")]
    pub target: String,
    /// The directory the bundles are written to and read from on the host.
    #[serde(default = "default_spool")]
    pub spool: String,
    /// Directories put before the host's own `PATH`, relative to its home unless absolute.
    #[serde(default = "default_path")]
    pub path: Vec<String>,
    /// The command that derives, relative to the worktree.
    #[serde(default = "default_derive")]
    pub derive: String,
    /// Files touched before the derive, so a build directory cloned from another tree's
    /// build never judges this tree's sources older than its executable.
    #[serde(default = "default_touch")]
    pub touch: Vec<String>,
}

impl Default for RemoteLayout {
    fn default() -> Self {
        RemoteLayout {
            checkout: default_checkout(),
            worktrees: default_worktrees(),
            target: default_target(),
            spool: default_spool(),
            path: default_path(),
            derive: default_derive(),
            touch: default_touch(),
        }
    }
}

/// One machine that may derive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OffloadHost {
    /// A short name, unique in the declaration: what `--host` names and a report prints.
    pub id: String,
    /// `user@host`, as ssh takes it.
    pub address: String,
    /// The ssh identity file, when the host needs one; `~/` is the invoking user's home.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_file: Option<String>,
    /// This host's own free-space minimum, in GiB; absent means the declaration's.
    #[serde(
        default,
        rename = "min_free_gib",
        skip_serializing_if = "Option::is_none"
    )]
    pub own_min_free_gib: Option<u64>,
    /// What the machine is, for a person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl OffloadHost {
    /// The free-space minimum that applies to this host.
    pub fn min_free_gib(&self, declaration: &OffloadDeclaration) -> u64 {
        self.own_min_free_gib.unwrap_or(declaration.min_free_gib)
    }
}

fn default_base() -> String {
    "origin/master".into()
}
fn default_min_free() -> u64 {
    10
}
fn default_queue_dir() -> String {
    "/tmp/offload-queue".into()
}
fn default_lock_dir() -> String {
    "/tmp/offload-derive.lock".into()
}
fn default_poll() -> u64 {
    5
}
fn default_wait_max() -> u64 {
    14_400
}
fn default_checkout() -> String {
    "dev/prismatic-majordomus".into()
}
fn default_worktrees() -> String {
    "dev/prismatic-majordomus-wt".into()
}
fn default_target() -> String {
    ".cache/majordomus-offload-target".into()
}
fn default_spool() -> String {
    "/tmp".into()
}
fn default_path() -> Vec<String> {
    [
        ".asdf/shims",
        ".cargo/bin",
        ".local/bin",
        "/opt/homebrew/bin",
        "/usr/local/bin",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
fn default_derive() -> String {
    "scripts/derive".into()
}
fn default_touch() -> Vec<String> {
    vec![
        "apps/majordomus-cli/src/lib.rs".into(),
        "apps/majordomus-cli/src/main.rs".into(),
    ]
}

/// A word that is safe to hand ssh or a remote shell as one argument without quoting doubts:
/// no whitespace, no control character, no quote, not empty.
fn plain(word: &str) -> bool {
    !word.is_empty()
        && !word
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\'' || c == '"')
}

impl OffloadDeclaration {
    /// Refuse what parsing cannot: no host, a duplicate host id, an address that is not
    /// `user@host`, a relative queue path, a zero poll, an empty path or command.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA_VERSION {
            return Err(format!("schema {} is not {SCHEMA_VERSION}", self.schema));
        }
        if self.kind != KIND {
            return Err(format!("kind {} is not {KIND}", self.kind));
        }
        if self.hosts.is_empty() {
            return Err("no host is declared: a derive has nowhere to go".into());
        }
        let mut seen = BTreeSet::new();
        for h in &self.hosts {
            if !plain(&h.id) {
                return Err(format!(
                    "host id '{}' is empty or carries a space or quote",
                    h.id
                ));
            }
            if !seen.insert(h.id.as_str()) {
                return Err(format!("host id '{}' is declared twice", h.id));
            }
            let (user, host) = h.address.split_once('@').unwrap_or(("", ""));
            if !plain(&h.address)
                || user.is_empty()
                || host.is_empty()
                || h.address.starts_with('-')
            {
                return Err(format!(
                    "host {}: address '{}' is not user@host",
                    h.id, h.address
                ));
            }
            if let Some(f) = &h.identity_file {
                if !plain(f) {
                    return Err(format!(
                        "host {}: identity_file '{f}' is not one plain word",
                        h.id
                    ));
                }
            }
        }
        for (name, p) in [
            ("queue.directory", &self.queue.directory),
            ("queue.lock", &self.queue.lock),
        ] {
            if !plain(p) || !p.starts_with('/') || p.len() < 2 {
                return Err(format!("{name} '{p}' is not an absolute path on the host"));
            }
        }
        if self.queue.directory == self.queue.lock {
            return Err("queue.directory and queue.lock are the same directory".into());
        }
        if self.queue.poll_seconds == 0 {
            return Err("queue.poll_seconds of 0 would be a busy loop".into());
        }
        let r = &self.remote;
        for (name, p) in [
            ("remote.checkout", &r.checkout),
            ("remote.worktrees", &r.worktrees),
            ("remote.target", &r.target),
            ("remote.spool", &r.spool),
            ("remote.derive", &r.derive),
            ("base", &self.base),
        ] {
            if !plain(p) {
                return Err(format!("{name} '{p}' is empty or carries a space or quote"));
            }
        }
        for p in r.path.iter().chain(r.touch.iter()) {
            if !plain(p) {
                return Err(format!(
                    "remote path entry '{p}' is empty or carries a space or quote"
                ));
            }
        }
        Ok(())
    }

    /// Parse one indexed object, or say why it is not a declaration, naming its URI.
    pub fn parse(object: &Object) -> Result<OffloadDeclaration, String> {
        let parsed: OffloadDeclaration = serde_json::from_value(object.metadata.clone())
            .map_err(|e| format!("{}: {e}", object.uri))?;
        parsed
            .validate()
            .map_err(|e| format!("{}: {e}", object.uri))?;
        Ok(parsed)
    }

    /// Parse a declaration file directly, for `--declaration`: the same struct and the
    /// same validation as the indexed object, read without building the index.
    pub fn read(path: &Path) -> Result<OffloadDeclaration, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let parsed: OffloadDeclaration = crate::metadata::yaml::parse_into(&text)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        parsed
            .validate()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(parsed)
    }

    /// The host a `--host` names: its id, or its address.
    pub fn host(&self, name: &str) -> Option<&OffloadHost> {
        self.hosts
            .iter()
            .find(|h| h.id == name || h.address == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(hosts: serde_json::Value) -> OffloadDeclaration {
        serde_json::from_value(serde_json::json!({
            "schema": SCHEMA_VERSION, "kind": KIND, "id": "derive", "hosts": hosts
        }))
        .unwrap()
    }

    #[test]
    fn a_complete_declaration_is_accepted_and_keeps_its_order() {
        let d = declaration(serde_json::json!([
            { "id": "lundra", "address": "korczis@100.65.22.118" },
            { "id": "macbook", "address": "korczis@100.108.237.68",
              "identity_file": "~/.ssh/cluster_ed25519", "min_free_gib": 20 }
        ]));
        d.validate().unwrap();
        assert_eq!(d.hosts[0].id, "lundra");
        assert_eq!(d.hosts[1].min_free_gib(&d), 20);
        assert_eq!(d.host("korczis@100.108.237.68").unwrap().id, "macbook");
        assert_eq!(
            d.min_free_gib, 10,
            "the default minimum is the one the v2 script used"
        );
        assert_eq!(d.hosts[0].min_free_gib(&d), 10);
        assert_eq!(d.queue.directory, "/tmp/offload-queue");
        assert_eq!(d.base, "origin/master");
        assert!(d.host("nowhere").is_none());
    }

    #[test]
    fn an_unknown_key_is_refused_not_ignored() {
        let r: Result<OffloadDeclaration, _> = serde_json::from_value(serde_json::json!({
            "schema": SCHEMA_VERSION, "kind": KIND, "id": "d", "hosts": [], "min_free_gb": 3
        }));
        assert!(r.is_err());
    }

    #[test]
    fn what_parsing_cannot_refuse_validation_does() {
        assert!(declaration(serde_json::json!([]))
            .validate()
            .unwrap_err()
            .contains("no host"));
        for bad in ["lundra", "@host", "user@", "-oProxyCommand=x@h", "a b@c"] {
            let d = declaration(serde_json::json!([{ "id": "x", "address": bad }]));
            assert!(d.validate().is_err(), "{bad} was accepted");
        }
        let mut d = declaration(serde_json::json!([{ "id": "x", "address": "u@h" }]));
        d.queue.directory = "relative".into();
        assert!(d.validate().unwrap_err().contains("absolute"));
        d.queue.directory = "/tmp/q".into();
        d.queue.poll_seconds = 0;
        assert!(d.validate().unwrap_err().contains("busy loop"));
        d.queue.poll_seconds = 1;
        d.queue.lock = "/tmp/q".into();
        assert!(d.validate().unwrap_err().contains("same directory"));
    }
}
