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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_key_change_under_one_node_id_is_rejected() {
        let state = evaluate(&TrustPolicy::Tofu, &[], "bb", Some("aa"), None);
        assert!(matches!(state, TrustState::Rejected(_)));
    }
}
