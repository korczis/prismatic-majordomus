//! A node's offer of a shared build cache, and the choice among offers (ADR 0157).
//!
//! **A capability, not a host.** An [`Offer`] says that some node serves an S3-compatible
//! store for compiler output: the protocol, the provider, how far its endpoint is meant to
//! be reached, a non-secret endpoint, a bucket, a priority, the headroom it has and how its
//! own probe judges it. Nothing here names a machine as special, and the type has no field
//! a credential could live in: a client key is provisioned onto a node and read from a file
//! outside the repository, never from an offer, the canonical state, an answer or a page.
//!
//! **Selection is a pure function.** [`select`] ranks the offers a client may use and
//! returns the choice *and why*, including every offer it passed over and the rule that
//! passed it over. The same offers and policy give the same answer in any order, because
//! the last tie-break is the node id. With no usable offer the answer is "none", which is a
//! normal answer: the build goes on with its local level alone (ADR 0157, decision 5).
//!
//! It reads no network and no clock: the age of an offer and a measured round trip come in
//! with it, so a test is a table.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The only protocol a client of this model speaks.
pub const PROTOCOL_S3: &str = "s3";

/// How far an offer's endpoint is meant to be reached, nearest first.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Reachable from the machine that offers it and no other.
    Host,
    /// Reachable on the offering machine's local network.
    Lan,
    /// Reachable by every node of the mesh.
    Mesh,
}

/// How the offering node's own probe judges the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    /// Ready and answering.
    Healthy,
    /// Answering, but something the policy cares about is wrong (low headroom, slow).
    Degraded,
    /// Not answering, or refusing.
    Unhealthy,
    /// The probe could not say: never `Healthy`.
    Unknown,
}

/// One node's offer. Carries no credential and no secret of any kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Offer {
    /// The offering node's id: the last tie-break, so ordering is total.
    pub node: String,
    /// The protocol the store speaks (`s3`).
    pub protocol: String,
    /// What serves it (`minio`, `garage`, ...): data, not a code path.
    pub provider: String,
    /// How far the endpoint is meant to be reached.
    pub scope: Scope,
    /// The non-secret address a client connects to.
    pub endpoint: String,
    /// The bucket the cache lives in.
    pub bucket: String,
    /// The offering node's own preference, higher first.
    pub priority: i32,
    /// Free space on the data path, in GiB, when the node said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headroom_gib: Option<u64>,
    /// The node's own judgement of the store.
    pub health: Health,
    /// Seconds since the offer was last refreshed.
    pub age_seconds: u64,
    /// A measured round trip from the asking node, in milliseconds, when one was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<u32>,
}

/// What a client accepts. Read from policy, never hard-coded at a call site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Policy {
    /// Protocols a client speaks; an offer of another protocol is passed over.
    pub protocols: Vec<String>,
    /// Providers accepted; empty accepts any.
    #[serde(default)]
    pub providers: Vec<String>,
    /// An offer not refreshed within this many seconds is stale and passed over.
    pub max_age_seconds: u64,
    /// An offer with less headroom than this many GiB is passed over; an offer that
    /// states none is not.
    pub min_headroom_gib: u64,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            protocols: vec![PROTOCOL_S3.to_string()],
            providers: Vec::new(),
            max_age_seconds: 90,
            min_headroom_gib: 20,
        }
    }
}

/// An offer that was not chosen, and the rule that said no.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PassedOver {
    /// The offer's node.
    pub node: String,
    /// The rule, in words.
    pub why: String,
}

/// The answer of [`select`]: the choice with its reason, and everything passed over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Selection {
    /// The chosen offer, or none.
    pub chosen: Option<Offer>,
    /// Why this one, or why none: always a sentence.
    pub reason: String,
    /// Every offer not chosen, in node order, with the rule that passed it over or the
    /// ranking it lost by.
    pub passed_over: Vec<PassedOver>,
}

/// Why `offer` may not be used under `policy`, or `None` when it may.
fn refusal(offer: &Offer, policy: &Policy) -> Option<String> {
    if !policy.protocols.iter().any(|p| p == &offer.protocol) {
        return Some(format!(
            "speaks {}, which this client does not",
            offer.protocol
        ));
    }
    if !policy.providers.is_empty() && !policy.providers.iter().any(|p| p == &offer.provider) {
        return Some(format!("provider {} is not accepted", offer.provider));
    }
    match offer.health {
        Health::Healthy => {}
        Health::Degraded => return Some("its own probe says degraded".into()),
        Health::Unhealthy => return Some("its own probe says unhealthy".into()),
        Health::Unknown => return Some("its own probe could not say".into()),
    }
    if offer.age_seconds > policy.max_age_seconds {
        return Some(format!(
            "stale: last refreshed {} s ago, the bound is {} s",
            offer.age_seconds, policy.max_age_seconds
        ));
    }
    if let Some(h) = offer.headroom_gib {
        if h < policy.min_headroom_gib {
            return Some(format!(
                "headroom {h} GiB is under the {} GiB floor",
                policy.min_headroom_gib
            ));
        }
    }
    None
}

/// The ranking key among usable offers; smaller is better. Total, because the node id
/// ends it.
fn rank(
    o: &Offer,
) -> (
    Scope,
    std::cmp::Reverse<i32>,
    std::cmp::Reverse<u64>,
    u32,
    &str,
) {
    (
        o.scope,
        std::cmp::Reverse(o.priority),
        std::cmp::Reverse(o.headroom_gib.unwrap_or(0)),
        o.rtt_ms.unwrap_or(u32::MAX),
        o.node.as_str(),
    )
}

/// Choose among `offers` under `policy`. Pure: no clock, no network, any order.
pub fn select(offers: &[Offer], policy: &Policy) -> Selection {
    let mut usable: Vec<&Offer> = Vec::new();
    let mut passed: Vec<PassedOver> = Vec::new();
    for o in offers {
        match refusal(o, policy) {
            Some(why) => passed.push(PassedOver {
                node: o.node.clone(),
                why,
            }),
            None => usable.push(o),
        }
    }
    usable.sort_by(|a, b| rank(a).cmp(&rank(b)));
    let chosen = usable.first().map(|o| (*o).clone());
    for o in usable.iter().skip(1) {
        passed.push(PassedOver {
            node: o.node.clone(),
            why: "usable, ranked lower: scope, then priority, headroom, round trip, node id".into(),
        });
    }
    passed.sort_by(|a, b| a.node.cmp(&b.node));
    let reason = match (&chosen, offers.len()) {
        (Some(c), _) => format!(
            "{} ({}, {} scope, priority {}) is the best of {} usable offer(s) of {}",
            c.node,
            c.provider,
            match c.scope {
                Scope::Host => "host",
                Scope::Lan => "lan",
                Scope::Mesh => "mesh",
            },
            c.priority,
            usable.len(),
            offers.len()
        ),
        (None, 0) => "no node offers a build cache: the build uses its local level alone".into(),
        (None, n) => {
            format!("none of {n} offer(s) is usable: the build uses its local level alone")
        }
    };
    Selection {
        chosen,
        reason,
        passed_over: passed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(node: &str) -> Offer {
        Offer {
            node: node.into(),
            protocol: PROTOCOL_S3.into(),
            provider: "minio".into(),
            scope: Scope::Mesh,
            endpoint: format!("http://{node}:9000"),
            bucket: "majordomus-build-cache".into(),
            priority: 0,
            headroom_gib: Some(500),
            health: Health::Healthy,
            age_seconds: 5,
            rtt_ms: Some(10),
        }
    }

    fn permutations(v: &[Offer]) -> Vec<Vec<Offer>> {
        if v.len() <= 1 {
            return vec![v.to_vec()];
        }
        let mut out = Vec::new();
        for i in 0..v.len() {
            let mut rest = v.to_vec();
            let head = rest.remove(i);
            for mut p in permutations(&rest) {
                p.insert(0, head.clone());
                out.push(p);
            }
        }
        out
    }

    #[test]
    fn no_offer_is_a_normal_answer_and_says_the_build_goes_on() {
        let s = select(&[], &Policy::default());
        assert_eq!(s.chosen, None);
        assert!(s.reason.contains("local level alone"), "{}", s.reason);
        assert!(s.passed_over.is_empty());
    }

    #[test]
    fn one_healthy_offer_is_chosen_and_explained() {
        let s = select(&[offer("jetson")], &Policy::default());
        assert_eq!(s.chosen.as_ref().map(|o| o.node.as_str()), Some("jetson"));
        assert!(s.reason.contains("best of 1 usable"), "{}", s.reason);
    }

    #[test]
    fn the_answer_does_not_depend_on_the_order_of_the_offers() {
        let mut a = offer("a");
        a.scope = Scope::Lan;
        let mut b = offer("b");
        b.priority = 5;
        let mut c = offer("c");
        c.headroom_gib = Some(900);
        let d = offer("d");
        let all = vec![a, b, c, d];
        let want = select(&all, &Policy::default());
        for p in permutations(&all) {
            assert_eq!(
                select(&p, &Policy::default()),
                want,
                "order changed the answer"
            );
        }
        // nearest scope wins before priority or headroom
        assert_eq!(want.chosen.map(|o| o.node), Some("a".into()));
    }

    #[test]
    fn ties_end_at_the_node_id() {
        let s = select(&[offer("zulu"), offer("alpha")], &Policy::default());
        assert_eq!(s.chosen.map(|o| o.node), Some("alpha".into()));
    }

    #[test]
    fn a_stale_an_unhealthy_and_an_unknown_offer_are_passed_over_with_the_rule() {
        let mut stale = offer("stale");
        stale.age_seconds = 600;
        let mut sick = offer("sick");
        sick.health = Health::Unhealthy;
        let mut unsure = offer("unsure");
        unsure.health = Health::Unknown;
        let good = offer("good");
        let s = select(&[stale, sick, unsure, good], &Policy::default());
        assert_eq!(s.chosen.map(|o| o.node), Some("good".into()));
        let why = |n: &str| {
            s.passed_over
                .iter()
                .find(|p| p.node == n)
                .map(|p| p.why.clone())
                .unwrap_or_default()
        };
        assert!(why("stale").contains("stale"), "{}", why("stale"));
        assert!(why("sick").contains("unhealthy"), "{}", why("sick"));
        // an offer whose probe could not say is never treated as healthy
        assert!(why("unsure").contains("could not say"), "{}", why("unsure"));
    }

    #[test]
    fn an_unusable_set_selects_none_and_says_how_many_it_looked_at() {
        let mut o = offer("x");
        o.protocol = "gcs".into();
        let s = select(&[o], &Policy::default());
        assert_eq!(s.chosen, None);
        assert!(s.reason.contains("none of 1"), "{}", s.reason);
        assert!(s.passed_over[0].why.contains("gcs"));
    }

    #[test]
    fn low_headroom_is_refused_and_an_unstated_headroom_is_not() {
        let mut low = offer("low");
        low.headroom_gib = Some(3);
        let mut silent = offer("silent");
        silent.headroom_gib = None;
        let s = select(&[low, silent], &Policy::default());
        assert_eq!(s.chosen.map(|o| o.node), Some("silent".into()));
        assert!(s.passed_over[0].why.contains("GiB floor"));
    }

    #[test]
    fn an_offer_carries_no_credential_field() {
        // the whole serialized shape is one this list names; a new field must be added here
        // on purpose, and a secret is not one
        let v = serde_json::to_value(offer("n")).expect("serialises");
        let mut keys: Vec<&str> = v
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "age_seconds",
                "bucket",
                "endpoint",
                "headroom_gib",
                "health",
                "node",
                "priority",
                "protocol",
                "provider",
                "rtt_ms",
                "scope"
            ]
        );
        let text = v.to_string().to_ascii_lowercase();
        for forbidden in ["secret", "password", "token", "access_key", "credential"] {
            assert!(!text.contains(forbidden), "{forbidden} appears in an offer");
        }
    }
}
