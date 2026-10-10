//! The trust root: which copy of the mesh declaration a runtime obeys (ADR 0050 rule 6,
//! I2135).
//!
//! The declaration names who this runtime trusts (`trust.policy`, `trust.allow`) and where it
//! sends its card and envelope (`rendezvous.endpoints`, `cooperation.seeds`,
//! `broadcast.networks`). Until this module, the server read it from the checkout's working
//! tree, so a branch — or an uncommitted edit — could add a key, point the runtime at a host
//! of its author's choosing, or switch the policy to trust on first use, and that checkout's
//! runtime obeyed. A reviewer running a contributor's branch is exactly the person that
//! reaches.
//!
//! The rule is that a working tree may **narrow** trust and never **widen** it. When the
//! trunk holds a committed copy of the declaration at the same path, the working tree's
//! declaration is used with every widening removed: the mesh is on only if the trunk's copy
//! is on as well, the policy is no looser than the trunk's, and every key and every address
//! is one the trunk's copy also lists — or a loopback address, which reaches no other host. When the trunk holds no copy — a repository that has
//! never committed one, or a directory that is not a git repository — the working tree is
//! all there is, and the one widening that needs no address at all, trust on first use, is
//! refused. Every widening removed is named, so `mesh doctor` can say what this checkout's
//! runtime ignores and why.
//!
//! The trunk is the repository's, as [`crate::worktree::RepositoryIdentity`] names it: the
//! local branch of that name. A stale local trunk is older trust, which is the safe
//! direction.
//!
//! ```
//! use majordomus_cli::mesh::root::{narrow, resolve};
//! use majordomus_cli::mesh::MeshConfig;
//! // no declaration in the working tree: nothing is obeyed and nothing was narrowed
//! let nowhere = tempfile::tempdir().unwrap();
//! let none = resolve(nowhere.path(), None);
//! assert!(none.declaration.is_none() && none.narrowed.is_empty());
//! // a declaration held against the trunk's copy keeps only what the trunk also says
//! let parse = |v: serde_json::Value| MeshConfig::from_metadata(&v, "x").unwrap();
//! let trunk = parse(serde_json::json!({"schema": "mesh/v1", "kind": "mesh-declaration", "id": "x",
//!     "trust": {"allow": ["aa"]}}));
//! let branch = parse(serde_json::json!({"schema": "mesh/v1", "kind": "mesh-declaration", "id": "x",
//!     "trust": {"allow": ["aa", "bb"]}}));
//! let (held, narrowed) = narrow(branch, &trunk);
//! assert_eq!(held.trust.allow, vec!["aa".to_string()]);
//! assert_eq!(narrowed, vec!["trust.allow: bb is not on the trunk".to_string()]);
//! ```

use std::path::Path;

use serde_json::Value;

use super::config::MeshConfig;
use super::trust::TrustPolicy;
use super::MeshError;
use crate::model::Object;

/// The declaration in force, and how it was arrived at.
///
/// ```
/// use majordomus_cli::mesh::root::{resolve, Resolved};
/// let nowhere = tempfile::tempdir().unwrap();
/// let resolved: Resolved = resolve(nowhere.path(), None);
/// assert!(resolved.declaration.is_none());
/// assert!(!resolved.committed && resolved.trunk.is_none());
/// ```
#[derive(Debug)]
pub struct Resolved {
    /// What the runtime obeys: `None` when there is no declaration, an error when the one
    /// there is (or the trunk's copy) does not parse.
    pub declaration: Option<Result<MeshConfig, MeshError>>,
    /// The trunk that was consulted, when one could be named.
    pub trunk: Option<String>,
    /// Whether the trunk holds a committed copy at the declaration's path.
    pub committed: bool,
    /// Each widening of the working tree that was not admitted, as one line naming the key.
    pub narrowed: Vec<String>,
}

/// Resolve the declaration in force for the repository at `root`, given the working tree's
/// declaration object as the index found it. `None` means the index found none, and then
/// there is nothing to obey and nothing to ask git.
///
/// ```
/// use majordomus_cli::mesh::root::resolve;
/// let nowhere = tempfile::tempdir().unwrap();
/// assert!(resolve(nowhere.path(), None).narrowed.is_empty());
/// ```
pub fn resolve(root: &Path, working: Option<&Object>) -> Resolved {
    let Some(object) = working else {
        return Resolved {
            declaration: None,
            trunk: None,
            committed: false,
            narrowed: Vec::new(),
        };
    };
    let trunk = crate::worktree::RepositoryIdentity::discover(root)
        .ok()
        .and_then(|identity| identity.trunk().branch.clone());
    let config = match MeshConfig::parse(object) {
        Ok(config) => config,
        Err(e) => {
            return Resolved {
                declaration: Some(Err(e)),
                trunk,
                committed: false,
                narrowed: Vec::new(),
            }
        }
    };
    let held = trunk
        .as_deref()
        .and_then(|branch| committed(root, branch, &object.provenance.path));
    match held {
        Some(Ok(on_trunk)) => {
            let (config, narrowed) = narrow(config, &on_trunk);
            Resolved {
                declaration: Some(Ok(config)),
                trunk,
                committed: true,
                narrowed,
            }
        }
        // A trunk copy that does not parse is not a reason to trust the working tree
        // instead: the mesh stays off, with the trunk's own refusal as the reason.
        Some(Err(e)) => Resolved {
            declaration: Some(Err(e)),
            trunk,
            committed: true,
            narrowed: Vec::new(),
        },
        None => {
            let (config, narrowed) = uncommitted(config);
            Resolved {
                declaration: Some(Ok(config)),
                trunk,
                committed: false,
                narrowed,
            }
        }
    }
}

/// The trunk's committed copy of the declaration at `path`, parsed; `None` when the trunk
/// holds no file there or git cannot be asked.
fn committed(root: &Path, branch: &str, path: &str) -> Option<Result<MeshConfig, MeshError>> {
    let spec = format!("refs/heads/{branch}:{path}");
    let out = crate::git::read_only(root)
        .args(["show", &spec])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let named = format!("{branch}:{path}");
    let parsed = String::from_utf8(out.stdout)
        .map_err(|e| e.to_string())
        .and_then(|text| crate::metadata::yaml::parse_mapping(&text))
        .map_err(|e| MeshError::Config(format!("{named}: {e}")))
        .and_then(|map| MeshConfig::from_metadata(&Value::Object(map), &named));
    Some(parsed)
}

/// How much a policy trusts a key it has not been told about: nothing, or its first
/// appearance.
fn looseness(policy: &TrustPolicy) -> u8 {
    match policy {
        TrustPolicy::DenyUnknown | TrustPolicy::Allowlist => 0,
        TrustPolicy::Tofu => 1,
    }
}

/// The working tree's declaration with every widening of `trunk` removed, and a line for each.
///
/// ```
/// use majordomus_cli::mesh::root::narrow;
/// use majordomus_cli::mesh::MeshConfig;
/// let parse = |v: serde_json::Value| MeshConfig::from_metadata(&v, "x").unwrap();
/// let trunk = parse(serde_json::json!({
///     "schema": "mesh/v1", "kind": "mesh-declaration", "id": "x", "enabled": true,
///     "trust": { "policy": "deny_unknown", "allow": ["aa"] },
/// }));
/// let branch = parse(serde_json::json!({
///     "schema": "mesh/v1", "kind": "mesh-declaration", "id": "x", "enabled": true,
///     "trust": { "policy": "tofu", "allow": ["aa", "bb"] },
///     "rendezvous": { "endpoints": ["http://198.51.100.7:8791"] },
/// }));
/// let (held, narrowed) = narrow(branch, &trunk);
/// assert_eq!(held.trust.allow, vec!["aa".to_string()]);
/// assert_eq!(held.trust.policy, trunk.trust.policy);
/// assert!(held.rendezvous.endpoints.is_empty());
/// assert_eq!(narrowed.len(), 3, "{narrowed:?}");
/// // narrowing is always admitted: a branch that trusts less is obeyed as it is
/// let (same, none) = narrow(trunk.clone(), &trunk);
/// assert!(none.is_empty() && same.trust.allow == trunk.trust.allow);
/// ```
pub fn narrow(mut working: MeshConfig, trunk: &MeshConfig) -> (MeshConfig, Vec<String>) {
    let mut narrowed = Vec::new();
    if working.enabled && !trunk.enabled {
        working.enabled = false;
        narrowed.push("enabled: the trunk's declaration is disabled".to_string());
    }
    if looseness(&working.trust.policy) > looseness(&trunk.trust.policy) {
        narrowed.push(format!(
            "trust.policy: {} is looser than the trunk's {}",
            working.trust.policy.as_str(),
            trunk.trust.policy.as_str()
        ));
        working.trust.policy = trunk.trust.policy.clone();
    }
    keep(
        &mut working.trust.allow,
        &trunk.trust.allow,
        "trust.allow",
        &mut narrowed,
        |_| false,
    );
    keep(
        &mut working.rendezvous.endpoints,
        &trunk.rendezvous.endpoints,
        "rendezvous.endpoints",
        &mut narrowed,
        is_loopback,
    );
    keep(
        &mut working.cooperation.seeds,
        &trunk.cooperation.seeds,
        "cooperation.seeds",
        &mut narrowed,
        is_loopback,
    );
    keep(
        &mut working.broadcast.networks,
        &trunk.broadcast.networks,
        "broadcast.networks",
        &mut narrowed,
        |_| false,
    );
    (working, narrowed)
}

/// Does an endpoint or a seed (`http://host:port`, or `host:port`) reach only this machine?
/// A loopback address carries the card and the envelope nowhere a person on another host
/// can read them, which is how two worktrees of one machine find each other without a hub,
/// so a working tree may name one without the trunk's say-so.
///
/// ```
/// use majordomus_cli::mesh::root::is_loopback;
/// assert!(is_loopback("http://127.0.0.1:8791"));
/// assert!(is_loopback("http://localhost:7"));
/// assert!(is_loopback("[::1]:8791"));
/// assert!(!is_loopback("http://192.168.100.30:8791"));
/// assert!(!is_loopback("http://127.0.0.1.attacker.example:8791"));
/// ```
pub fn is_loopback(endpoint: &str) -> bool {
    let rest = endpoint
        .strip_prefix("http://")
        .or_else(|| endpoint.strip_prefix("https://"))
        .unwrap_or(endpoint);
    let authority = rest.split('/').next().unwrap_or_default();
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or_default(),
        None => authority.rsplit_once(':').map_or(authority, |(h, _)| h),
    };
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.to_canonical().is_loopback())
}

/// A declaration no trunk holds: obeyed as written, except trust on first use.
fn uncommitted(mut working: MeshConfig) -> (MeshConfig, Vec<String>) {
    let mut narrowed = Vec::new();
    if working.trust.policy == TrustPolicy::Tofu {
        working.trust.policy = TrustPolicy::DenyUnknown;
        narrowed.push(
            "trust.policy: tofu is honoured only from a declaration committed on the trunk"
                .to_string(),
        );
    }
    (working, narrowed)
}

/// Keep the entries of `working` that `trunk` also lists, or that `local` says reach only
/// this machine, naming each one dropped.
fn keep(
    working: &mut Vec<String>,
    trunk: &[String],
    key: &str,
    narrowed: &mut Vec<String>,
    local: fn(&str) -> bool,
) {
    working.retain(|entry| {
        let held = trunk.contains(entry) || local(entry);
        if !held {
            narrowed.push(format!("{key}: {entry} is not on the trunk"));
        }
        held
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(v: serde_json::Value) -> MeshConfig {
        MeshConfig::from_metadata(&v, "test").unwrap()
    }

    #[test]
    fn an_uncommitted_tofu_is_refused_and_everything_else_is_obeyed() {
        let (held, narrowed) = uncommitted(config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "x", "enabled": true,
            "trust": { "policy": "tofu", "allow": ["aa"] },
        })));
        assert_eq!(held.trust.policy, TrustPolicy::DenyUnknown);
        assert_eq!(held.trust.allow, vec!["aa".to_string()]);
        assert!(held.enabled);
        assert_eq!(narrowed.len(), 1);
    }

    /// The declaration the index found in the working tree.
    fn working(metadata: serde_json::Value) -> Object {
        Object {
            kind: "mesh-declaration".into(),
            identity: "majordomus".into(),
            uri: "majordomus://mesh-declaration/majordomus".into(),
            title: None,
            description: None,
            metadata,
            body: String::new(),
            content: String::new(),
            media_type: "application/yaml",
            provenance: crate::model::Provenance {
                path: ".ai/repo/mesh/majordomus.yaml".into(),
                directory: ".ai/repo/mesh".into(),
                source_class: "mesh-declaration".into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    /// A repository whose trunk, `master`, holds `committed` at the declaration's path, or
    /// holds no declaration at all.
    fn trunk_holding(committed: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(["-c", "user.email=t@t.invalid", "-c", "user.name=t"])
                .args(args)
                .current_dir(dir.path())
                .output()
                .unwrap()
                .status;
            assert!(status.success(), "git {args:?}");
        };
        git(&["init", "-q", "-b", "master"]);
        std::fs::write(dir.path().join("README.md"), "r\n").unwrap();
        if let Some(text) = committed {
            std::fs::create_dir_all(dir.path().join(".ai/repo/mesh")).unwrap();
            std::fs::write(dir.path().join(".ai/repo/mesh/majordomus.yaml"), text).unwrap();
        }
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "trunk"]);
        dir
    }

    const TRUNK: &str =
        "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\ntrust:\n  policy: deny_unknown\n";

    fn tofu() -> serde_json::Value {
        serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "majordomus",
            "enabled": true, "trust": { "policy": "tofu" },
        })
    }

    #[test]
    fn the_trunk_copy_narrows_the_working_tree_and_a_copy_that_does_not_parse_refuses() {
        // committed: the trunk's deny_unknown holds against the working tree's tofu
        let repo = trunk_holding(Some(TRUNK));
        let resolved = resolve(repo.path(), Some(&working(tofu())));
        assert_eq!(resolved.trunk.as_deref(), Some("master"));
        assert!(resolved.committed);
        let held = resolved.declaration.unwrap().unwrap();
        assert_eq!(held.trust.policy, TrustPolicy::DenyUnknown);
        assert_eq!(resolved.narrowed.len(), 1, "{:?}", resolved.narrowed);

        // a working tree that does not parse is refused, whatever the trunk says
        let broken = resolve(
            repo.path(),
            Some(&working(serde_json::json!({ "schema": "mesh/v0" }))),
        );
        assert!(matches!(broken.declaration, Some(Err(_))));
        assert!(!broken.committed);

        // a trunk copy that does not parse turns the mesh off with the trunk's refusal,
        // rather than trusting the working tree instead
        let repo = trunk_holding(Some("schema: mesh/v0\nkind: mesh-declaration\nid: x\n"));
        let resolved = resolve(repo.path(), Some(&working(tofu())));
        assert!(resolved.committed);
        let refusal = resolved.declaration.unwrap().unwrap_err().to_string();
        assert!(
            refusal.contains("master:.ai/repo/mesh/majordomus.yaml"),
            "{refusal}"
        );
    }

    #[test]
    fn a_declaration_the_trunk_does_not_hold_is_read_as_uncommitted() {
        let repo = trunk_holding(None);
        let resolved = resolve(repo.path(), Some(&working(tofu())));
        assert!(!resolved.committed);
        let held = resolved.declaration.unwrap().unwrap();
        assert_eq!(held.trust.policy, TrustPolicy::DenyUnknown);
    }

    #[test]
    fn loopback_is_named_by_scheme_bracket_or_bare_authority() {
        assert!(is_loopback("https://127.0.0.1:8791/x"));
        assert!(is_loopback("http://[::1]:8791"));
        assert!(is_loopback("localhost:8791"));
        assert!(!is_loopback("https://[2001:db8::1]:8791"));
    }

    #[test]
    fn a_branch_cannot_switch_on_a_mesh_the_trunk_switched_off() {
        let trunk = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "x", "enabled": false,
        }));
        let branch = config(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "x", "enabled": true,
        }));
        let (held, narrowed) = narrow(branch, &trunk);
        assert!(!held.enabled);
        assert_eq!(
            narrowed,
            vec!["enabled: the trunk's declaration is disabled"]
        );
    }
}
