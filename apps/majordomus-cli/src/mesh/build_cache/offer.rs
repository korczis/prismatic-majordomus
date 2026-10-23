//! A build-cache offer as the mesh carries it: a signed, expiring record of the cooperation
//! journal (ADR 0157 decision 2, I2671).
//!
//! **Published while the store answers.** A node that serves a store declares it in its node
//! file ([`super::NodeFile`]). On every heartbeat, before it beats, its runtime probes the
//! store and compares what it would offer now with what it last published: a healthy store
//! not offered yet, or offered differently, is published as
//! [`EventBody::BuildCacheOffered`]; a store whose probe stopped passing, or that is no longer
//! declared, is withdrawn with [`EventBody::BuildCacheWithdrawn`]. Nothing is written while
//! nothing changed, so a provider does not grow the journal by an event a heartbeat.
//!
//! **Signed by its origin.** The record is an event of the journal: its node's Ed25519 key
//! signs its stream, sequence, repository and body, and a relay forwards it without being able
//! to alter or re-attribute it. [`read`] verifies the signature again rather than trusting the
//! road the event took.
//!
//! **Trusted when it is read.** An offer is read only from a key the trust in force trusts:
//! presence on a port is never trust, and a key withdrawn from the allowlist stops being read
//! at once, although its events stay in the journal until the next restart (rule
//! `project.mesh-is-observation-not-authority`).
//!
//! **Expiring with its runtime's beat.** The record states its lifetime, and the offer is fresh
//! while its stream's signed beat rose within that lifetime on the reader's own monotonic clock:
//! the liveness the journal already keeps for claims, no wall clock compared. A runtime that
//! stops stops beating, and its offer is stale within its lifetime with nobody withdrawing it;
//! and because the runtime probes its store before it beats, a beat is also its word that the
//! store answered.
//!
//! # Example
//!
//! ```
//! use majordomus_cli::mesh::build_cache::offer::read;
//! use std::time::Duration;
//! // a journal without an offer offers nothing, and that is a normal answer
//! let reading = read(&[], &|_| true, &|_| None, Duration::from_secs(30));
//! assert!(reading.offers.is_empty() && reading.ignored.is_empty());
//! ```

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{local_offer, read_node_file, FileError, Health, NodeFile, Offer, PassedOver};
use crate::mesh::identity::{node_id_of_key, verify};
use crate::mesh::journal::{
    BuildCacheOffer, EventBody, MeshEvent, StreamId, BUILD_CACHE_LIFETIME_MAX,
};

/// The journal kinds a build-cache record is written as.
pub const KINDS: [&str; 2] = ["build_cache_offered", "build_cache_withdrawn"];

/// Why a node offers nothing, when it declares no store.
const NOT_SERVED: &str = "this node serves no build cache";

/// What the journal offers now: the offers a client may select among, and every record that
/// is not one, with why.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::offer::{read, OfferReading};
/// use std::time::Duration;
/// let reading: OfferReading = read(&[], &|_| true, &|_| None, Duration::from_secs(30));
/// assert!(reading.offers.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "BuildCacheOfferReading")]
pub struct OfferReading {
    /// The offers of trusted runtimes whose beat rose within the offer's lifetime, one per
    /// node, in node order: what [`super::select`] takes.
    pub offers: Vec<Offer>,
    /// Every build-cache record that is not read as an offer, by node, each with its stream
    /// and the reason: untrusted, withdrawn, stale, unverified or out of bounds.
    pub ignored: Vec<PassedOver>,
}

/// The offer one stream's newest record makes, with the age of its beat, or why it makes none.
fn judge(
    stream: &StreamId,
    event: &MeshEvent,
    trusted: &dyn Fn(&str) -> bool,
    beat_age: &dyn Fn(&StreamId) -> Option<Duration>,
    lifetime_bound: Duration,
) -> Result<(Duration, Offer), String> {
    let bound_key = node_id_of_key(&event.pk).map(|n| n.as_str().to_string());
    if bound_key.as_deref() != Some(stream.node())
        || !verify(&event.pk, &event.signing_bytes(), &event.sig)
    {
        return Err("its signature does not verify against its stream's key".into());
    }
    if !trusted(&event.pk) {
        return Err("its key is not trusted: an offer is observation, never authority".into());
    }
    let offer = match event.body() {
        Some(EventBody::BuildCacheOffered { offer }) => offer,
        Some(EventBody::BuildCacheWithdrawn { reason }) => {
            return Err(format!("withdrawn: {reason}"))
        }
        _ => return Err("not a build-cache record this executable reads".into()),
    };
    offer
        .validate()
        .map_err(|why| format!("out of bounds: {why}"))?;
    let lifetime = Duration::from_secs(offer.lifetime_seconds).min(lifetime_bound);
    let Some(age) = beat_age(stream) else {
        return Err("stale: no beat of its runtime has been heard".into());
    };
    if age > lifetime {
        return Err(format!(
            "stale: its runtime last beat {} s ago, and the offer lives {} s",
            age.as_secs(),
            lifetime.as_secs()
        ));
    }
    Ok((
        age,
        Offer {
            node: stream.node().to_string(),
            protocol: offer.protocol,
            provider: offer.provider,
            scope: offer.scope,
            endpoint: offer.endpoint,
            bucket: offer.bucket,
            priority: offer.priority,
            headroom_gib: offer.headroom_gib,
            // published only while its own probe passed, and fresh only while it beats after
            // probing again: an offer read here is one its node last judged healthy
            health: Health::Healthy,
            age_seconds: age.as_secs(),
            rtt_ms: None,
        },
    ))
}

/// The offers `events` make now. Pure: the trust, the beat ages and the bound come in.
///
/// Each stream's newest build-cache record speaks for it. A record is an offer only when its
/// signature verifies against its stream's key, `trusted` trusts that key, it is an offer and
/// not a withdrawal, it is within bounds, and `beat_age` says its stream beat within the
/// offer's lifetime (no longer than `lifetime_bound`, the reader's own expiry). Two streams of
/// one node — two servers of one machine — make one offer: the fresher.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::offer::read;
/// use majordomus_cli::mesh::build_cache::Scope;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{BuildCacheOffer, EventBody, Journal};
/// use std::{sync::Arc, time::Duration};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::BuildCacheOffered { offer: BuildCacheOffer {
///     protocol: "s3".into(), provider: "minio".into(), scope: Scope::Lan,
///     endpoint: "http://192.0.2.10:9000".into(), bucket: "majordomus-build-cache".into(),
///     priority: 0, headroom_gib: Some(200), lifetime_seconds: 30 } }).unwrap();
/// let events = j.events();
///
/// // trusted and beating: an offer
/// let fresh = read(&events, &|_| true, &|_| Some(Duration::from_secs(2)), Duration::from_secs(30));
/// assert_eq!(fresh.offers.len(), 1);
/// // the same record from a key nobody trusts is not one
/// let untrusted = read(&events, &|_| false, &|_| Some(Duration::from_secs(2)), Duration::from_secs(30));
/// assert!(untrusted.offers.is_empty() && untrusted.ignored[0].why.contains("not trusted"));
/// // and a runtime that stopped beating offers nothing within the lifetime
/// let stale = read(&events, &|_| true, &|_| Some(Duration::from_secs(31)), Duration::from_secs(30));
/// assert!(stale.offers.is_empty() && stale.ignored[0].why.contains("stale"));
/// ```
pub fn read(
    events: &[MeshEvent],
    trusted: &dyn Fn(&str) -> bool,
    beat_age: &dyn Fn(&StreamId) -> Option<Duration>,
    lifetime_bound: Duration,
) -> OfferReading {
    let mut newest: BTreeMap<&StreamId, &MeshEvent> = BTreeMap::new();
    for event in events.iter().filter(|e| KINDS.contains(&e.kind())) {
        if newest
            .get(&event.stream)
            .is_none_or(|held| held.seq < event.seq)
        {
            newest.insert(&event.stream, event);
        }
    }
    let mut ignored = Vec::new();
    let mut best: BTreeMap<String, (Duration, &StreamId, Offer)> = BTreeMap::new();
    for (stream, event) in newest {
        let node = stream.node().to_string();
        match judge(stream, event, trusted, beat_age, lifetime_bound) {
            Err(why) => ignored.push(PassedOver {
                node,
                why: format!("{stream}: {why}"),
            }),
            Ok((age, offer)) => {
                let held = best.get(&node).map(|(a, s, _)| (*a, *s));
                let loser = match held {
                    Some(h) if h <= (age, stream) => stream,
                    _ => match best.insert(node.clone(), (age, stream, offer)) {
                        Some((_, s, _)) => s,
                        None => continue,
                    },
                };
                ignored.push(PassedOver {
                    node,
                    why: format!("{loser}: the same node offers from a fresher stream"),
                });
            }
        }
    }
    crate::order::canonical(&mut ignored);
    OfferReading {
        offers: best.into_values().map(|(_, _, offer)| offer).collect(),
        ignored,
    }
}

/// Whether `a` and `b` offer the same store: every field equal, and the headroom within a tenth
/// of the larger value, so that a disk filling by a few GiB writes no event.
fn same(a: &BuildCacheOffer, b: &BuildCacheOffer) -> bool {
    let headroom = match (a.headroom_gib, b.headroom_gib) {
        (Some(x), Some(y)) => x.abs_diff(y).saturating_mul(10) <= x.max(y),
        (None, None) => true,
        _ => false,
    };
    let bare = |o: &BuildCacheOffer| BuildCacheOffer {
        headroom_gib: None,
        ..o.clone()
    };
    headroom && bare(a) == bare(b)
}

/// The record a runtime writes now, given what it last `published` and what it would offer
/// `now` (or why it offers nothing): an offer when there is one to make and it differs from the
/// published one, a withdrawal when there is none and one was published, and nothing otherwise.
pub(crate) fn next_event(
    published: Option<&BuildCacheOffer>,
    now: Result<BuildCacheOffer, String>,
) -> Option<EventBody> {
    match (published, now) {
        (Some(p), Ok(o)) if same(p, &o) => None,
        (_, Ok(offer)) => Some(EventBody::BuildCacheOffered { offer }),
        (Some(_), Err(reason)) => Some(EventBody::BuildCacheWithdrawn { reason }),
        (None, Err(_)) => None,
    }
}

/// What this node would offer now: the store its node file at `file` declares, probed within
/// `budget`, as a record living `lifetime`; or why it offers nothing. The reasons are a fixed
/// vocabulary, since a withdrawal carries one to every runtime of the mesh: never a path, an
/// endpoint or anything the file says.
pub(crate) fn current(
    file: Option<&Path>,
    node: &str,
    lifetime: Duration,
    budget: Duration,
) -> Result<BuildCacheOffer, String> {
    let Some(path) = file else {
        return Err(NOT_SERVED.into());
    };
    let provider = match read_node_file(path) {
        Ok(NodeFile {
            provider: Some(p), ..
        }) => p,
        Ok(_) | Err(FileError::Absent) => return Err(NOT_SERVED.into()),
        Err(FileError::Invalid(_)) => {
            return Err("its build-cache file cannot be read".into());
        }
    };
    let offer = local_offer(&provider, node, budget);
    match offer.health {
        Health::Healthy => {}
        Health::Degraded => return Err("its own probe says degraded".into()),
        Health::Unhealthy => return Err("its own probe says unhealthy".into()),
        Health::Unknown => return Err("its own probe could not say".into()),
    }
    let endpoint = if offer.endpoint.contains("://") {
        offer.endpoint
    } else {
        format!("http://{}", offer.endpoint)
    };
    let record = BuildCacheOffer {
        protocol: offer.protocol,
        provider: offer.provider,
        scope: offer.scope,
        endpoint,
        bucket: offer.bucket,
        priority: offer.priority,
        headroom_gib: offer.headroom_gib,
        lifetime_seconds: lifetime.as_secs().clamp(1, BUILD_CACHE_LIFETIME_MAX),
    };
    match record.validate() {
        Ok(()) => Ok(record),
        Err(_) => Err("its declared store is not one an offer may name".into()),
    }
}

/// Where a node keeps its build-cache file: `$XDG_CONFIG_HOME/majordomus/build-cache.json`
/// when that is an absolute path, else `$HOME/.config/majordomus/build-cache.json`; none when
/// neither is set.
fn node_file_from(xdg_config_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let base = xdg_config_home
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("majordomus").join("build-cache.json"))
}

/// This process's build-cache file, by [`node_file_from`] over its environment.
pub(crate) fn node_file() -> Option<PathBuf> {
    node_file_from(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};
    use std::sync::Arc;

    use super::*;
    use crate::mesh::build_cache::{select, Policy, Scope, PROBE_BUDGET};
    use crate::mesh::config::TrustConfig;
    use crate::mesh::identity::NodeIdentity;
    use crate::mesh::journal::{Journal, Rejection};

    fn record() -> BuildCacheOffer {
        BuildCacheOffer {
            protocol: "s3".into(),
            provider: "minio".into(),
            scope: Scope::Lan,
            endpoint: "http://192.0.2.10:9000".into(),
            bucket: "majordomus-build-cache".into(),
            priority: 2,
            headroom_gib: Some(200),
            lifetime_seconds: 30,
        }
    }

    fn journal(identity: &Arc<NodeIdentity>, runtime: &str) -> Journal {
        Journal::open(Arc::clone(identity), runtime, "repo".into(), None).unwrap()
    }

    fn key() -> Arc<NodeIdentity> {
        Arc::new(NodeIdentity::ephemeral().unwrap())
    }

    const BOUND: Duration = Duration::from_secs(30);

    fn beating(_: &StreamId) -> Option<Duration> {
        Some(Duration::from_secs(1))
    }

    const READY: &str = "200 OK";
    /// What a MinIO that stopped being ready answers on its readiness route.
    const NOT_READY: &str = "503 Service Unavailable";

    /// A loopback store answering its readiness route once with each status of `answers`, in
    /// order, then gone. Returns the endpoint without a scheme, as a node file may write it.
    ///
    /// A store that stops is modelled by its 503, not by its closed port: a port closed here
    /// is free for any other test of this process to bind, and a probe that reaches one of
    /// those can say anything, so a closed port is a verdict only a case with a process of its
    /// own can hold (test/cases/3954). That a refused port is `Unhealthy` is the probe's own
    /// unit test in `mesh::build_cache`.
    fn store(answers: &'static [&'static str]) -> (String, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let served = std::thread::spawn(move || {
            for status in answers {
                let Ok((mut c, _)) = listener.accept() else {
                    return;
                };
                let mut buf = [0u8; 256];
                let _ = c.read(&mut buf);
                let _ = c.write_all(format!("HTTP/1.0 {status}\r\n\r\n").as_bytes());
            }
        });
        (addr.to_string(), served)
    }

    fn write_node_file(dir: &Path, endpoint: &str, bucket: &str) -> PathBuf {
        let path = dir.join("build-cache.json");
        let text = serde_json::json!({ "provider": {
            "provider": "minio", "scope": "lan", "endpoint": endpoint, "bucket": bucket,
            "priority": 1, "data_path": dir } });
        std::fs::write(&path, text.to_string()).unwrap();
        path
    }

    #[test]
    fn an_offer_is_signed_by_its_node_and_a_tampered_one_is_neither_stored_nor_read() {
        let (a, b) = (
            journal(&key(), "0000000000000001"),
            journal(&key(), "0000000000000002"),
        );
        a.append_own(EventBody::BuildCacheOffered { offer: record() })
            .unwrap();
        let sent = a.missing_for(&b.marks(), 1 << 20);
        assert!(!sent[0].sig.is_empty(), "signed by its origin");

        // a relay that moves the endpoint elsewhere changes bytes under the signature
        let mut tampered = sent.clone();
        tampered[0].body["offer"]["endpoint"] = "http://192.0.2.66:9000".into();
        let report = b.ingest(&tampered, &|_| Ok(()));
        assert_eq!(report.accepted, 0);
        assert_eq!(report.rejected.get(&Rejection::Signature), Some(&1));
        let forged = read(&tampered, &|_| true, &beating, BOUND);
        assert!(forged.offers.is_empty());
        assert!(
            forged.ignored[0].why.contains("signature does not verify"),
            "{:?}",
            forged.ignored
        );

        // the record as signed is stored and read, attributed to its signing node
        assert_eq!(b.ingest(&sent, &|_| Ok(())).accepted, 1);
        let reading = read(&b.events(), &|_| true, &beating, BOUND);
        assert_eq!(reading.offers.len(), 1);
        assert_eq!(reading.offers[0].node, a.own_stream().node());
        assert_eq!(reading.offers[0].endpoint, "http://192.0.2.10:9000");
        assert_eq!(reading.offers[0].health, Health::Healthy);

        // a record re-attributed to another stream does not verify against that stream's key
        let mut moved = sent[0].clone();
        moved.stream = b.own_stream().clone();
        let read_moved = read(&[moved], &|_| true, &beating, BOUND);
        assert!(read_moved.offers.is_empty());
    }

    #[test]
    fn only_a_trusted_runtimes_offer_is_read() {
        let (ours, stranger) = (key(), key());
        let (a, c) = (
            journal(&ours, "0000000000000001"),
            journal(&stranger, "0000000000000003"),
        );
        a.append_own(EventBody::BuildCacheOffered { offer: record() })
            .unwrap();
        let mut other = record();
        other.endpoint = "http://192.0.2.99:9000".into();
        other.priority = 99;
        c.append_own(EventBody::BuildCacheOffered { offer: other })
            .unwrap();
        let events: Vec<MeshEvent> = a.events().into_iter().chain(c.events()).collect();
        let trusted_key = ours.public.public_key.clone();
        let reading = read(&events, &|pk| pk == trusted_key, &beating, BOUND);
        assert_eq!(reading.offers.len(), 1);
        assert_eq!(reading.offers[0].node, a.own_stream().node());
        let refused = &reading.ignored[0];
        assert_eq!(refused.node, c.own_stream().node());
        assert!(refused.why.contains("not trusted"), "{}", refused.why);
        // the higher priority of the untrusted offer never reaches the selection
        let choice = select(&reading.offers, &Policy::default());
        assert_eq!(
            choice.chosen.map(|o| o.node),
            Some(a.own_stream().node().to_string())
        );
    }

    #[test]
    fn a_stale_offer_is_passed_over_within_its_lifetime() {
        let a = journal(&key(), "0000000000000001");
        a.append_own(EventBody::BuildCacheOffered { offer: record() })
            .unwrap();
        let events = a.events();
        let at = |secs: u64| {
            read(
                &events,
                &|_| true,
                &move |_| Some(Duration::from_secs(secs)),
                BOUND,
            )
        };
        assert_eq!(at(30).offers.len(), 1, "a beat at the lifetime still holds");
        let stale = at(31);
        assert!(stale.offers.is_empty());
        assert!(
            stale.ignored[0]
                .why
                .contains("stale: its runtime last beat 31 s ago"),
            "{:?}",
            stale.ignored
        );
        // never heard beating: stale, not fresh
        let unheard = read(&events, &|_| true, &|_| None, BOUND);
        assert!(
            unheard.ignored[0].why.contains("no beat"),
            "{:?}",
            unheard.ignored
        );
        // a reader whose own expiry is shorter than the stated lifetime holds to its own
        let short = read(
            &events,
            &|_| true,
            &|_| Some(Duration::from_secs(10)),
            Duration::from_secs(5),
        );
        assert!(short.offers.is_empty());
        assert!(
            short.ignored[0].why.contains("lives 5 s"),
            "{:?}",
            short.ignored
        );
        // and the age read is the beat's, handed to the selection as the offer's age
        assert_eq!(at(7).offers[0].age_seconds, 7);
    }

    #[test]
    fn the_newest_word_of_a_stream_speaks_for_it() {
        let a = journal(&key(), "0000000000000001");
        a.append_own(EventBody::BuildCacheOffered { offer: record() })
            .unwrap();
        a.append_own(EventBody::BuildCacheWithdrawn {
            reason: "its own probe says unhealthy".into(),
        })
        .unwrap();
        let withdrawn = read(&a.events(), &|_| true, &beating, BOUND);
        assert!(withdrawn.offers.is_empty());
        assert!(
            withdrawn.ignored[0]
                .why
                .ends_with("withdrawn: its own probe says unhealthy"),
            "{:?}",
            withdrawn.ignored
        );
        let mut moved = record();
        moved.priority = 7;
        a.append_own(EventBody::BuildCacheOffered { offer: moved })
            .unwrap();
        // in any order: the sequence decides, not the arrival
        let mut events = a.events();
        events.reverse();
        let back = read(&events, &|_| true, &beating, BOUND);
        assert_eq!(back.offers.len(), 1);
        assert_eq!(back.offers[0].priority, 7);
        assert!(back.ignored.is_empty());
    }

    #[test]
    fn two_servers_of_one_machine_make_one_offer_the_fresher() {
        let machine = key();
        let (one, two) = (
            journal(&machine, "0000000000000001"),
            journal(&machine, "0000000000000002"),
        );
        let mut older = record();
        older.priority = 1;
        one.append_own(EventBody::BuildCacheOffered { offer: older })
            .unwrap();
        two.append_own(EventBody::BuildCacheOffered { offer: record() })
            .unwrap();
        let (s1, s2) = (one.own_stream().clone(), two.own_stream().clone());
        let events: Vec<MeshEvent> = one.events().into_iter().chain(two.events()).collect();
        let ages = move |s: &StreamId| Some(Duration::from_secs(if *s == s1 { 9 } else { 2 }));
        let reading = read(&events, &|_| true, &ages, BOUND);
        assert_eq!(reading.offers.len(), 1);
        assert_eq!(reading.offers[0].priority, 2, "the fresher stream's offer");
        assert!(
            reading.ignored[0].why.contains("fresher stream"),
            "{:?}",
            reading.ignored
        );
        assert!(!reading.ignored[0].why.starts_with(s2.as_str()));
        // a tie of ages ends at the stream id, whichever arrives first
        let tied: Vec<MeshEvent> = two.events().into_iter().chain(one.events()).collect();
        let even = read(&tied, &|_| true, &beating, BOUND);
        let reversed = read(&events, &|_| true, &beating, BOUND);
        assert_eq!(even, reversed);
    }

    #[test]
    fn the_record_carries_no_secret_and_has_no_place_for_one() {
        let v = serde_json::to_value(record()).unwrap();
        let keys: std::collections::BTreeSet<&str> =
            v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys.into_iter().collect::<Vec<_>>(),
            [
                "bucket",
                "endpoint",
                "headroom_gib",
                "lifetime_seconds",
                "priority",
                "protocol",
                "provider",
                "scope"
            ]
        );
        // an endpoint is a scheme and an address: user information, a query, a path or a
        // fragment is where a credential would ride, and each is refused before signing
        for endpoint in [
            "http://key:secret@192.0.2.10:9000",
            "http://192.0.2.10:9000/?X-Amz-Credential=x",
            "http://192.0.2.10:9000/bucket",
            "http://192.0.2.10:9000#x",
            "ftp://192.0.2.10:21",
            "http://",
            "192.0.2.10:9000",
        ] {
            let mut o = record();
            o.endpoint = endpoint.into();
            let err = o.validate().expect_err(endpoint);
            assert!(
                !err.contains("secret"),
                "the refusal repeats the value: {err}"
            );
            let refused = EventBody::BuildCacheOffered { offer: o };
            assert!(journal(&key(), "0000000000000001")
                .append_own(refused)
                .is_err());
        }
        for (bucket, ok) in [
            ("ab", false),
            ("Upper", false),
            ("-x-cache", false),
            ("build.cache-1", true),
        ] {
            let mut o = record();
            o.bucket = bucket.into();
            assert_eq!(o.validate().is_ok(), ok, "{bucket}");
        }
        for (lifetime, ok) in [
            (0, false),
            (1, true),
            (BUILD_CACHE_LIFETIME_MAX, true),
            (BUILD_CACHE_LIFETIME_MAX + 1, false),
        ] {
            let mut o = record();
            o.lifetime_seconds = lifetime;
            assert_eq!(o.validate().is_ok(), ok, "{lifetime}");
        }
        let mut o = record();
        o.provider = "Mini O".into();
        assert!(o.validate().is_err());
        o = record();
        o.protocol = String::new();
        assert!(o.validate().is_err());
        o = record();
        o.endpoint = "https://[2001:db8::1]:9000".into();
        assert!(
            o.validate().is_ok(),
            "an IPv6 literal over TLS is an address"
        );
        let withdrawn = EventBody::BuildCacheWithdrawn {
            reason: "two\nlines".into(),
        };
        assert!(withdrawn.validate().is_err());
    }

    #[test]
    fn a_runtime_writes_only_when_its_word_changes() {
        let offer = record();
        // nothing published, nothing to offer: silence
        assert_eq!(next_event(None, Err(NOT_SERVED.into())), None);
        // a healthy store is offered once
        assert_eq!(
            next_event(None, Ok(offer.clone())),
            Some(EventBody::BuildCacheOffered {
                offer: offer.clone()
            })
        );
        assert_eq!(next_event(Some(&offer), Ok(offer.clone())), None);
        // a disk that fills by less than a tenth writes nothing; more, or across none, does
        let mut drift = offer.clone();
        drift.headroom_gib = Some(181);
        assert_eq!(next_event(Some(&offer), Ok(drift.clone())), None);
        drift.headroom_gib = Some(179);
        assert!(next_event(Some(&offer), Ok(drift.clone())).is_some());
        drift.headroom_gib = None;
        assert!(next_event(Some(&offer), Ok(drift)).is_some());
        // any other change is a new offer
        let mut moved = offer.clone();
        moved.priority = 9;
        assert!(next_event(Some(&offer), Ok(moved)).is_some());
        // and a store that stops answering is withdrawn with the reason
        assert_eq!(
            next_event(Some(&offer), Err("its own probe says unhealthy".into())),
            Some(EventBody::BuildCacheWithdrawn {
                reason: "its own probe says unhealthy".into()
            })
        );
    }

    #[test]
    fn what_a_node_offers_is_its_declared_store_while_it_answers() {
        let dir = tempfile::tempdir().unwrap();
        let life = Duration::from_secs(30);
        assert_eq!(
            current(None, "n", life, PROBE_BUDGET),
            Err(NOT_SERVED.into())
        );
        let absent = dir.path().join("none.json");
        assert_eq!(
            current(Some(&absent), "n", life, PROBE_BUDGET),
            Err(NOT_SERVED.into())
        );
        std::fs::write(&absent, "{}").unwrap();
        assert_eq!(
            current(Some(&absent), "n", life, PROBE_BUDGET),
            Err(NOT_SERVED.into())
        );
        std::fs::write(&absent, "{not json").unwrap();
        let broken = current(Some(&absent), "n", life, PROBE_BUDGET).unwrap_err();
        assert!(
            !broken.contains(&dir.path().display().to_string()),
            "no path travels: {broken}"
        );

        let (endpoint, served) = store(&[READY, NOT_READY]);
        let healthy = write_node_file(dir.path(), &endpoint, "majordomus-build-cache");
        let offer = current(Some(&healthy), "n", life, PROBE_BUDGET).unwrap();
        assert!(
            offer.endpoint.starts_with("http://127.0.0.1:"),
            "{}",
            offer.endpoint
        );
        assert_eq!((offer.lifetime_seconds, offer.priority), (30, 1));
        assert!(offer.headroom_gib.is_some());
        assert!(offer.validate().is_ok());

        // the same store, no longer ready
        let stopped = current(Some(&healthy), "n", life, PROBE_BUDGET).unwrap_err();
        assert_eq!(stopped, "its own probe says unhealthy");
        served.join().unwrap();
        // a store the probe cannot judge is not offered either
        let named = write_node_file(
            dir.path(),
            "http://store.invalid:9000",
            "majordomus-build-cache",
        );
        assert_eq!(
            current(Some(&named), "n", life, PROBE_BUDGET).unwrap_err(),
            "its own probe could not say"
        );
        // a healthy store an offer may not name is refused without repeating what it said
        let bad = write_node_file(dir.path(), &store(&[READY]).0, "No_Such_Bucket");
        let refused = current(Some(&bad), "n", life, PROBE_BUDGET).unwrap_err();
        assert!(
            refused.contains("not one an offer may name") && !refused.contains("No_Such"),
            "{refused}"
        );
        // a lifetime is clamped into the bounds a record admits
        let long = write_node_file(dir.path(), &store(&[READY]).0, "majordomus-build-cache");
        let clamped =
            current(Some(&long), "n", Duration::from_secs(1 << 20), PROBE_BUDGET).unwrap();
        assert_eq!(clamped.lifetime_seconds, BUILD_CACHE_LIFETIME_MAX);
    }

    #[test]
    fn the_node_file_follows_the_xdg_configuration_home() {
        let at = |xdg: Option<&str>, home: Option<&str>| {
            node_file_from(xdg.map(OsString::from), home.map(OsString::from))
        };
        assert_eq!(
            at(Some("/cfg"), Some("/home/u")),
            Some(PathBuf::from("/cfg/majordomus/build-cache.json"))
        );
        // a relative XDG_CONFIG_HOME is ignored, as the specification says
        assert_eq!(
            at(Some("cfg"), Some("/home/u")),
            Some(PathBuf::from("/home/u/.config/majordomus/build-cache.json"))
        );
        assert_eq!(at(None, None), None);
        assert!(super::node_file().is_none_or(|p| p.ends_with("majordomus/build-cache.json")));
    }

    #[test]
    fn a_linked_peer_reads_the_providers_offer_until_it_is_withdrawn_or_untrusted() {
        use crate::mesh::testing::{runtime, Net};
        let net = Arc::new(Net::default());
        let (a, b) = (runtime(&net, "a:1"), runtime(&net, "b:1"));
        let link = b.dial(&["a:1".into()]).expect("A welcomes B");
        let dir = tempfile::tempdir().unwrap();
        let (endpoint, served) = store(&[READY, NOT_READY, NOT_READY]);
        a.serve_build_cache_from(Some(write_node_file(
            dir.path(),
            &endpoint,
            "majordomus-build-cache",
        )));
        let round = || {
            a.journal().beat_own();
            b.sync_with(&link).unwrap();
        };

        // the provider's heartbeat publishes its healthy store, once
        let offered = a
            .refresh_build_cache_offer()
            .expect("a healthy store is offered");
        assert_eq!(offered.kind(), "build_cache_offered");
        round();
        let reading = b.build_cache_offers();
        assert_eq!(reading.offers.len(), 1, "{reading:?}");
        let theirs = &reading.offers[0];
        assert_eq!(theirs.node, a.journal().own_stream().node());
        // fed to the pure selection, it is the choice
        let policy = Policy {
            min_headroom_gib: 0,
            ..Policy::default()
        };
        assert_eq!(
            select(&reading.offers, &policy).chosen.as_ref(),
            Some(theirs)
        );

        // the store stops being ready: the next heartbeat withdraws it, and the peer stops
        // reading it
        let withdrawn = a
            .refresh_build_cache_offer()
            .expect("a stopped store is withdrawn");
        assert_eq!(withdrawn.kind(), "build_cache_withdrawn");
        assert!(a.refresh_build_cache_offer().is_none(), "said once");
        served.join().unwrap();
        round();
        let gone = b.build_cache_offers();
        assert!(gone.offers.is_empty());
        assert!(
            gone.ignored[0]
                .why
                .contains("withdrawn: its own probe says unhealthy"),
            "{gone:?}"
        );

        // it answers again: offered again; then the peer stops trusting its key
        a.serve_build_cache_from(Some(write_node_file(
            dir.path(),
            &store(&[READY]).0,
            "majordomus-build-cache",
        )));
        assert!(a.refresh_build_cache_offer().is_some());
        round();
        assert_eq!(b.build_cache_offers().offers.len(), 1);
        b.retrust(TrustConfig::default());
        let distrusted = b.build_cache_offers();
        assert!(
            distrusted.offers.is_empty(),
            "held in the journal, never read as an offer"
        );
        assert!(
            distrusted.ignored[0].why.contains("not trusted"),
            "{distrusted:?}"
        );
        // the provider's own reading is its own offer
        assert_eq!(a.build_cache_offers().offers.len(), 1);
    }
}
