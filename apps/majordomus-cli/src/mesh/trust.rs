//! Trust: what a verified advertisement is allowed to mean. Verification (the signature
//! proves the key) happens in the protocol; trust (this key is one of ours) is decided
//! here, by policy — and by default the policy is that an unknown key is observed,
//! recorded, visible in every listing, and trusted for nothing.
//!
//! Discovery creates awareness, not authority: even a `Trusted` node gains no execution
//! rights anywhere in this codebase. Trust gates which nodes surfaces mark as ours and
//! which endpoints later negotiation may approach — nothing else.
//!
//! ```
//! use majordomus_cli::mesh::trust::{evaluate, TrustPolicy, TrustState};
//!
//! // the default: a valid unknown key is observed and trusted for nothing
//! assert_eq!(evaluate(&TrustPolicy::DenyUnknown, &[], "aa", None, None), TrustState::Observed);
//! // an allowlisted key is trusted under every policy
//! let listed = evaluate(&TrustPolicy::DenyUnknown, &["aa".into()], "aa", None, None);
//! assert!(listed.is_trusted());
//! ```
//!
//! A key can also be trusted through a [`Succession`] its predecessor signed, and only while
//! the declaration in force trusts that predecessor ([`evaluate_with_successions`], ADR 0067
//! addendum of 2026-10-15).

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::identity::Succession;

/// The policy, from the mesh declaration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrustPolicy {
    /// The default: a valid unknown node is recorded as observed and trusted for
    /// nothing. The safe answer when nobody decided otherwise.
    #[default]
    DenyUnknown,
    /// Trust-on-first-use: the first valid appearance of a key is trusted, a later
    /// appearance of the same node id under a different key is rejected. A development
    /// convenience, and every listing says the trust came from TOFU rather than a person.
    Tofu,
    /// Only keys the declaration lists are trusted; everything else is observed.
    Allowlist,
}

impl TrustPolicy {
    /// The policy's wire word, as declarations write it and listings show it.
    ///
    /// ```
    /// use majordomus_cli::mesh::trust::TrustPolicy;
    /// assert_eq!(TrustPolicy::DenyUnknown.as_str(), "deny_unknown");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustPolicy::DenyUnknown => "deny_unknown",
            TrustPolicy::Tofu => "tofu",
            TrustPolicy::Allowlist => "allowlist",
        }
    }
}

/// What the policy decided about one node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum TrustState {
    /// Valid, recorded, trusted for nothing. The resting state of an unknown node.
    Observed,
    /// The policy trusts this key; `detail` says which rule did (`allowlist`, `tofu`).
    Trusted(String),
    /// The policy rejected this node; `detail` says why (for instance a TOFU key
    /// change). A rejected node stays listed — an operator debugging a mesh needs to
    /// see what was refused, not wonder about silence.
    Rejected(String),
    /// The key was succeeded by a trusted successor, whose key is the `detail`: its history
    /// stays legitimate, and it is trusted for nothing new. Retired, not withdrawn.
    Retired(String),
}

impl TrustState {
    /// Whether this state trusts the node — the one question consumers ask; the detail
    /// stays on the variant for whoever renders it.
    pub fn is_trusted(&self) -> bool {
        matches!(self, TrustState::Trusted(_))
    }
}

/// Evaluate the policy for a node's key against what the registry already knows.
///
/// ```
/// use majordomus_cli::mesh::trust::{evaluate, TrustPolicy, TrustState};
/// // TOFU trusts first use; a key change under a known node id is rejected regardless
/// assert!(evaluate(&TrustPolicy::Tofu, &[], "aa", None, None).is_trusted());
/// assert!(matches!(
///     evaluate(&TrustPolicy::Tofu, &[], "bb", Some("aa"), None),
///     TrustState::Rejected(_)
/// ));
/// ```
///
/// `known_key` is the key the registry last associated with this node id — with node
/// ids derived from keys the two cannot disagree, so a mismatch can only mean a digest
/// collision or registry corruption, and it is rejected loudly.
pub fn evaluate(
    policy: &TrustPolicy,
    allow: &[String],
    public_key: &str,
    known_key: Option<&str>,
    previous: Option<&TrustState>,
) -> TrustState {
    if let Some(known) = known_key {
        if known != public_key {
            return TrustState::Rejected("the node id is already bound to a different key".into());
        }
    }
    if allow.iter().any(|k| k == public_key) {
        return TrustState::Trusted("allowlist".into());
    }
    match policy {
        TrustPolicy::DenyUnknown => TrustState::Observed,
        TrustPolicy::Allowlist => TrustState::Observed,
        TrustPolicy::Tofu => match previous {
            // The first use is what TOFU trusts; a node once rejected stays rejected.
            Some(state @ TrustState::Rejected(_)) => state.clone(),
            _ => TrustState::Trusted("tofu".into()),
        },
    }
}

/// [`evaluate`], following the successions a runtime holds (ADR 0067, addendum of
/// 2026-10-15). Only statements that verify count; of those:
///
/// - a key that two different successors claim to succeed is a fork, which only a stolen
///   key can produce: the key and both successors are `Rejected("succession fork")`, unless
///   the allowlist names a successor itself;
/// - a key with one successor that is trusted (the allowlist names the key or the successor)
///   is `Retired(successor)`, whatever the allowlist says of the key;
/// - a key the allowlist does not name is `Trusted("succession")` when a predecessor the
///   allowlist names hands over to it. The depth is one: a key trusted only through a
///   succession confers nothing on a successor of its own until the allowlist names it, and
///   withdrawing a predecessor withdraws its successor with it.
///
/// ```
/// use majordomus_cli::mesh::identity::{NodeIdentity, Succession};
/// use majordomus_cli::mesh::trust::{evaluate_with_successions, TrustPolicy, TrustState};
/// let old = NodeIdentity::ephemeral().unwrap();
/// let new = NodeIdentity::ephemeral().unwrap();
/// let (old_key, new_key) = (old.public.public_key.clone(), new.public.public_key.clone());
/// let statements = [Succession::sign(&old, &new, "2026-10-15T08:00:00Z").unwrap()];
/// let trust = |allow: &[String], key: &str| {
///     evaluate_with_successions(&TrustPolicy::Allowlist, allow, &statements, key, None, None)
/// };
/// let allow = [old_key.clone()];
/// assert_eq!(trust(&allow, &new_key), TrustState::Trusted("succession".into()));
/// assert_eq!(trust(&allow, &old_key), TrustState::Retired(new_key.clone()));
/// // a withdrawn predecessor confers nothing
/// assert_eq!(trust(&[], &new_key), TrustState::Observed);
/// ```
pub fn evaluate_with_successions(
    policy: &TrustPolicy,
    allow: &[String],
    successions: &[Succession],
    public_key: &str,
    known_key: Option<&str>,
    previous: Option<&TrustState>,
) -> TrustState {
    if let Some(known) = known_key {
        if known != public_key {
            return TrustState::Rejected("the node id is already bound to a different key".into());
        }
    }
    let valid: Vec<&Succession> = successions.iter().filter(|s| s.verify().is_ok()).collect();
    let listed = |key: &str| allow.iter().any(|k| k == key);
    let successors = |key: &str| -> BTreeSet<&str> {
        valid
            .iter()
            .filter(|s| s.old_key == key)
            .map(|s| s.new_key.as_str())
            .collect()
    };

    let own = successors(public_key);
    if own.len() > 1 {
        return TrustState::Rejected("succession fork".into());
    }
    if let Some(successor) = own.first() {
        if listed(public_key) || listed(successor) {
            return TrustState::Retired((*successor).to_string());
        }
    }
    if listed(public_key) {
        return TrustState::Trusted("allowlist".into());
    }
    let predecessors: BTreeSet<&str> = valid
        .iter()
        .filter(|s| s.new_key == public_key)
        .map(|s| s.old_key.as_str())
        .collect();
    if predecessors.iter().any(|p| successors(p).len() > 1) {
        return TrustState::Rejected("succession fork".into());
    }
    if predecessors.iter().any(|p| listed(p)) {
        return TrustState::Trusted("succession".into());
    }
    evaluate(policy, allow, public_key, None, previous)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_key_that_is_the_presented_one_is_judged_as_if_unknown() {
        let allow = ["aa".to_string()];
        let unknown =
            evaluate_with_successions(&TrustPolicy::Allowlist, &allow, &[], "aa", None, None);
        let known =
            evaluate_with_successions(&TrustPolicy::Allowlist, &allow, &[], "aa", Some("aa"), None);
        assert_eq!(known, unknown);
        assert!(!matches!(known, TrustState::Rejected(_)), "{known:?}");
    }

    #[test]
    fn deny_unknown_observes_and_never_trusts() {
        let state = evaluate(&TrustPolicy::DenyUnknown, &[], "aa", None, None);
        assert_eq!(state, TrustState::Observed);
        assert!(!state.is_trusted());
    }

    #[test]
    fn the_allowlist_trusts_listed_keys_under_every_policy() {
        for policy in [
            TrustPolicy::DenyUnknown,
            TrustPolicy::Tofu,
            TrustPolicy::Allowlist,
        ] {
            let state = evaluate(&policy, &["aa".into()], "aa", None, None);
            assert_eq!(state, TrustState::Trusted("allowlist".into()), "{policy:?}");
        }
    }

    #[test]
    fn allowlist_policy_observes_the_unlisted() {
        let state = evaluate(&TrustPolicy::Allowlist, &["aa".into()], "bb", None, None);
        assert_eq!(state, TrustState::Observed);
    }

    #[test]
    fn tofu_trusts_first_use_and_keeps_a_rejection() {
        let first = evaluate(&TrustPolicy::Tofu, &[], "aa", None, None);
        assert_eq!(first, TrustState::Trusted("tofu".into()));
        let rejected = TrustState::Rejected("test".into());
        let after = evaluate(&TrustPolicy::Tofu, &[], "aa", Some("aa"), Some(&rejected));
        assert_eq!(
            after, rejected,
            "a rejection is not washed away by reappearing"
        );
    }

    use crate::mesh::identity::NodeIdentity;

    /// Three ephemeral keys and their public halves.
    fn keys() -> (Vec<NodeIdentity>, Vec<String>) {
        let nodes: Vec<NodeIdentity> = (0..3).map(|_| NodeIdentity::ephemeral().unwrap()).collect();
        let keys = nodes.iter().map(|n| n.public.public_key.clone()).collect();
        (nodes, keys)
    }

    fn hand_over(old: &NodeIdentity, new: &NodeIdentity) -> Succession {
        Succession::sign(old, new, "2026-10-15T08:00:00Z").unwrap()
    }

    fn judge(allow: &[String], successions: &[Succession], key: &str) -> TrustState {
        evaluate_with_successions(&TrustPolicy::Allowlist, allow, successions, key, None, None)
    }

    /// I2705: the successor is trusted through the trunk's trust in its predecessor and only
    /// while it holds; the predecessor is retired once succeeded; a withdrawn predecessor
    /// takes its successor with it, so a rotation cannot escape a revocation.
    #[test]
    fn a_successor_is_trusted_only_while_its_predecessor_is() {
        let (nodes, k) = keys();
        let rotation = [hand_over(&nodes[0], &nodes[1])];

        let trunk = [k[0].clone()];
        assert_eq!(
            judge(&trunk, &rotation, &k[1]),
            TrustState::Trusted("succession".into())
        );
        assert_eq!(
            judge(&trunk, &rotation, &k[0]),
            TrustState::Retired(k[1].clone()),
            "the succeeded key is retired although the trunk still lists it"
        );
        assert!(!judge(&trunk, &rotation, &k[0]).is_trusted());

        // the predecessor withdrawn: the successor its thief may have named is not trusted,
        // and the old key is not retired either (nothing trusted took over)
        assert_eq!(judge(&[], &rotation, &k[1]), TrustState::Observed);
        assert_eq!(judge(&[], &rotation, &k[0]), TrustState::Observed);
        // nor does any policy but the declaration trust the successor through the statement
        assert_eq!(
            evaluate_with_successions(&TrustPolicy::DenyUnknown, &[], &rotation, &k[1], None, None),
            TrustState::Observed
        );

        // the replacement committed: the new key trusted directly, the old one retired
        let committed = [k[1].clone()];
        assert_eq!(
            judge(&committed, &rotation, &k[1]),
            TrustState::Trusted("allowlist".into())
        );
        assert_eq!(
            judge(&committed, &rotation, &k[0]),
            TrustState::Retired(k[1].clone())
        );

        // a forged countersignature confers nothing and retires nothing
        let mut forged = rotation[0].clone();
        forged.new_signature = nodes[2].sign(&forged.countersigning_bytes());
        let forged = [forged];
        assert_eq!(judge(&trunk, &forged, &k[1]), TrustState::Observed);
        assert_eq!(
            judge(&trunk, &forged, &k[0]),
            TrustState::Trusted("allowlist".into())
        );

        // the depth is one: a key trusted only through a succession names no successor
        let chain = [rotation[0].clone(), hand_over(&nodes[1], &nodes[2])];
        assert_eq!(judge(&trunk, &chain, &k[2]), TrustState::Observed);
        assert_eq!(
            judge(&trunk, &chain, &k[1]),
            TrustState::Trusted("succession".into()),
            "an inert statement does not retire the key that made it"
        );
        // until the trunk lists it
        let listed = [k[1].clone()];
        assert_eq!(
            judge(&listed, &chain, &k[2]),
            TrustState::Trusted("succession".into())
        );
        assert_eq!(
            judge(&listed, &chain, &k[1]),
            TrustState::Retired(k[2].clone())
        );
    }

    /// I2705: two different successors of one key are a fork, which only a stolen key can
    /// produce: neither successor is trusted and the old key is rejected, listed or not.
    #[test]
    fn a_fork_trusts_neither() {
        let (nodes, k) = keys();
        let fork = [
            hand_over(&nodes[0], &nodes[1]),
            hand_over(&nodes[0], &nodes[2]),
        ];
        let trunk = [k[0].clone()];
        let fork_state = TrustState::Rejected("succession fork".into());
        for key in &k {
            assert_eq!(judge(&trunk, &fork, key), fork_state, "{key}");
        }
        // the same statement twice is one succession, not a fork
        let twice = [fork[0].clone(), fork[0].clone()];
        assert_eq!(
            judge(&trunk, &twice, &k[1]),
            TrustState::Trusted("succession".into())
        );
        // a forged second statement makes no fork
        let mut forged = fork[1].clone();
        forged.old_signature = nodes[2].sign(&forged.signing_bytes());
        let one = [fork[0].clone(), forged];
        assert_eq!(
            judge(&trunk, &one, &k[1]),
            TrustState::Trusted("succession".into())
        );
        // a person who lists a successor directly is the trust root
        let chosen = [k[1].clone()];
        assert_eq!(
            judge(&chosen, &fork, &k[1]),
            TrustState::Trusted("allowlist".into())
        );
        assert_eq!(judge(&chosen, &fork, &k[0]), fork_state);
    }

    /// Without a succession, the succession-aware evaluation is the plain one.
    #[test]
    fn without_successions_nothing_changes() {
        for policy in [
            TrustPolicy::DenyUnknown,
            TrustPolicy::Tofu,
            TrustPolicy::Allowlist,
        ] {
            for (allow, key, known) in [
                (vec![], "aa", None),
                (vec!["aa".to_string()], "aa", None),
                (vec![], "bb", Some("aa")),
            ] {
                assert_eq!(
                    evaluate_with_successions(&policy, &allow, &[], key, known, None),
                    evaluate(&policy, &allow, key, known, None),
                    "{policy:?} {allow:?} {key} {known:?}"
                );
            }
        }
    }

    #[test]
    fn a_key_change_under_one_node_id_is_rejected() {
        let state = evaluate(&TrustPolicy::Tofu, &[], "bb", Some("aa"), None);
        assert!(matches!(state, TrustState::Rejected(_)));
    }

    #[test]
    fn a_key_change_under_one_node_id_is_rejected_with_successions_in_hand() {
        let state = evaluate_with_successions(&TrustPolicy::Tofu, &[], &[], "bb", Some("aa"), None);
        assert_eq!(
            state,
            TrustState::Rejected("the node id is already bound to a different key".into())
        );
    }
}
