//! Which advisors can be asked right now, and why each one that cannot, cannot.
//!
//! Three inputs, all cheap and none over the network (ADR 0032):
//!
//! * **presence** — an executable on `PATH`, a credential variable *set* (its value is
//!   never read). Presence is what the crate can know; it says "installed/configured",
//!   not "answering", and the status says which it measured.
//! * **the reasoning mode** — `offline` admits only advisors that stay on the machine,
//!   `ci` admits none, the rest admit all; an explicit disable list removes any.
//! * **recorded outcomes** — every consultation a session recorded carries its status.
//!   A run of transient failures opens the advisor's circuit for a cooldown, a rate limit
//!   for its own, an authentication failure for longer; once the cooldown passes the
//!   advisor is available again and marked `recovering`. The circuit is therefore a pure
//!   function of the records and the clock: deterministic, replayable, and never a
//!   permanent failure nobody configured.
//!
//! An empty result is an ordinary answer. A session with no advisor still reasons; it
//! reasons locally, and says so.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::catalogue::{AdvisorCatalogue, AdvisorTransport, References};
use super::record::ConsultationStatus;

/// How much independent review a session may use, and where from.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningMode {
    /// No network: only advisors that stay on this machine.
    Offline,
    /// Deterministic: no advisor at all, whatever is installed.
    Ci,
    /// Review only where the blast radius is high.
    Fast,
    /// Review where uncertainty is material.
    #[default]
    Standard,
    /// Review wider, and complementary review where the stakes are highest.
    Strict,
}

impl ReasoningMode {
    /// The mode a word names.
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word.trim() {
            "offline" => Self::Offline,
            "ci" => Self::Ci,
            "fast" => Self::Fast,
            "standard" => Self::Standard,
            "strict" => Self::Strict,
            _ => return None,
        })
    }

    /// The word.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Ci => "ci",
            Self::Fast => "fast",
            Self::Standard => "standard",
            Self::Strict => "strict",
        }
    }
}

/// The mode in force and what decided it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModeResolution {
    /// The mode.
    pub mode: ReasoningMode,
    /// What decided it: `MAJORDOMUS_REASONING_MODE`, `CI`, `profile <name>`, or `default`.
    pub source: String,
}

/// The environment variable that names the mode outright.
pub const MODE_ENV: &str = "MAJORDOMUS_REASONING_MODE";
/// The environment variable listing advisors to leave out (comma-separated, or `all`).
pub const DISABLE_ENV: &str = "MAJORDOMUS_ADVISORS_DISABLE";
/// The environment variable overriding every circuit cooldown, in seconds.
pub const COOLDOWN_ENV: &str = "MAJORDOMUS_REASONING_COOLDOWN_SECONDS";

/// Decide the mode: the variable, else a CI runner, else the active profile's declared
/// mode, else `standard`. An unreadable word is skipped with its source, never guessed.
///
/// ```
/// use majordomus_cli::reasoning::availability::{resolve_mode, ReasoningMode};
///
/// assert_eq!(resolve_mode(Some("offline"), true, None).mode, ReasoningMode::Offline);
/// assert_eq!(resolve_mode(None, true, Some(("implementation", Some("strict")))).mode, ReasoningMode::Ci);
/// let r = resolve_mode(None, false, Some(("deep-work", Some("strict"))));
/// assert_eq!((r.mode, r.source.as_str()), (ReasoningMode::Strict, "profile deep-work"));
/// assert_eq!(resolve_mode(Some("loud"), false, None).source, "default");
/// ```
pub fn resolve_mode(
    explicit: Option<&str>,
    ci: bool,
    profile: Option<(&str, Option<&str>)>,
) -> ModeResolution {
    if let Some(mode) = explicit.and_then(ReasoningMode::parse) {
        return ModeResolution {
            mode,
            source: MODE_ENV.into(),
        };
    }
    if ci {
        return ModeResolution {
            mode: ReasoningMode::Ci,
            source: "CI".into(),
        };
    }
    if let Some((name, Some(word))) = profile {
        if let Some(mode) = ReasoningMode::parse(word) {
            return ModeResolution {
                mode,
                source: format!("profile {name}"),
            };
        }
    }
    ModeResolution {
        mode: ReasoningMode::Standard,
        source: "default".into(),
    }
}

/// What the crate can observe about an advisor's installation. The one seam tests fake.
pub trait Presence {
    /// Whether an executable of this name is on `PATH`.
    fn executable(&self, name: &str) -> bool;
    /// Whether an environment variable of this name is set and non-empty. Never its value.
    fn variable(&self, name: &str) -> bool;
}

/// The process's own `PATH` and environment.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPresence;

impl Presence for SystemPresence {
    fn executable(&self, name: &str) -> bool {
        use std::os::unix::fs::PermissionsExt;
        if name.contains('/') {
            return false;
        }
        std::env::var_os("PATH").is_some_and(|path| {
            std::env::split_paths(&path).any(|dir| {
                std::fs::metadata(dir.join(name))
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            })
        })
    }

    fn variable(&self, name: &str) -> bool {
        std::env::var_os(name).is_some_and(|v| !v.is_empty())
    }
}

/// An advisor's standing.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AdvisorStatus {
    /// It may be asked. Presence was observed; whether it answers is learned by asking.
    Available,
    /// Not installed: its executable is not on `PATH`.
    Unavailable,
    /// Installed or reachable in principle, but its credential is not configured.
    NotConfigured,
    /// Left out by the mode or by an explicit disable.
    Disabled,
    /// Its recent consultations failed; its circuit is open until the cooldown passes.
    TemporarilyFailed,
    /// It answered with a rate limit; not asked again until the limit passes.
    RateLimited,
}

impl AdvisorStatus {
    /// The word.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::NotConfigured => "not_configured",
            Self::Disabled => "disabled",
            Self::TemporarilyFailed => "temporarily_failed",
            Self::RateLimited => "rate_limited",
        }
    }
}

/// An open circuit: why, and until when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AdvisorCircuit {
    /// The consultation outcome that opened it.
    pub opened_by: String,
    /// The consecutive failures behind it.
    pub failures: u32,
    /// When it closes again, RFC 3339.
    pub until: String,
}

/// One advisor as it stands now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AdvisorState {
    /// The advisor id (a peer is `peer:<runtime>`).
    pub id: String,
    /// The human name.
    pub title: String,
    /// How it is reached.
    pub transport: AdvisorTransport,
    /// The adapter that speaks to it.
    pub adapter: String,
    /// The advisory capabilities it offers.
    pub capabilities: Vec<String>,
    /// The model it asks, when the catalogue names one: what its adapter sends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The executable its adapter runs, when it runs one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    /// The name of the credential variable its adapter reads — a name, never a value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
    /// Its standing.
    pub status: AdvisorStatus,
    /// Why, as one machine word: `present`, `executable_not_found`, `credential_absent`,
    /// `mode_ci`, `mode_offline`, `disabled_by_configuration`, `consecutive_failures`,
    /// `rate_limited`, `authentication_failed`, `peer_linked`.
    pub reason: String,
    /// What was observed, for a person: which executable, which variable (never a value).
    pub probe: String,
    /// Available again after an open circuit's cooldown: the next consultation is its test.
    #[serde(default)]
    pub recovering: bool,
    /// The open circuit, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circuit: Option<AdvisorCircuit>,
}

/// The recorded outcome of one consultation, as availability reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The consultation record id; the tie-break of equal stamps.
    pub id: String,
    /// The advisor asked.
    pub advisor: String,
    /// How it ended.
    pub status: ConsultationStatus,
    /// When it was recorded, seconds since the epoch.
    pub at: i64,
    /// A rate limit's own retry-after, seconds.
    pub retry_after: Option<u64>,
}

/// A linked mesh runtime carrying the review feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedPeer {
    /// `<node>-<runtime>`.
    pub runtime: String,
    /// Its display name.
    pub name: String,
    /// The link features it carries; only those carrying the catalogue's are advisors.
    pub features: Vec<String>,
}

/// Consecutive transient failures that open a circuit.
pub const FAILURES_TO_OPEN: u32 = 2;
/// How long a circuit opened by transient failures stays open, seconds.
pub const FAILURE_COOLDOWN: i64 = 600;
/// How long a rate limit is honoured when it names no retry-after, seconds.
pub const RATE_LIMIT_COOLDOWN: i64 = 900;
/// How long an authentication failure keeps an advisor out, seconds: a credential is not
/// fixed by asking again, so it is not asked again soon.
pub const AUTH_COOLDOWN: i64 = 3_600;

/// Everything availability reads besides the catalogue.
pub struct Inputs<'a> {
    /// The resolved references.
    pub refs: &'a References,
    /// Presence.
    pub presence: &'a dyn Presence,
    /// The mode.
    pub mode: ReasoningMode,
    /// Advisor ids left out explicitly; `all` leaves out every one.
    pub disabled: &'a [String],
    /// ReasoningRecorded consultation outcomes, any order.
    pub history: &'a [Outcome],
    /// Linked reviewing peers.
    pub peers: &'a [LinkedPeer],
    /// Now, seconds since the epoch.
    pub now: i64,
    /// A cooldown override for every circuit, seconds.
    pub cooldown: Option<i64>,
}

/// Every advisor — declared ones in preference order, then linked peers by runtime — with
/// its status. Pure over its inputs.
///
/// ```
/// use majordomus_cli::reasoning::availability::*;
/// use majordomus_cli::reasoning::catalogue::{AdvisorCatalogue, References};
///
/// struct Nothing;
/// impl Presence for Nothing {
///     fn executable(&self, _: &str) -> bool { false }
///     fn variable(&self, _: &str) -> bool { false }
/// }
/// let c: AdvisorCatalogue = majordomus_cli::metadata::yaml::parse_into(
///     "version: 1\ncapabilities:\n  - id: code_review\n    description: x\nadvisors:\n  - id: a\n    title: A\n    transport: cli\n    adapter: a-cli\n    executable: a\n    capabilities: [code_review]\n",
/// ).unwrap();
/// let refs = References::default();
/// let states = derive_availability(&c, &Inputs {
///     refs: &refs, presence: &Nothing, mode: ReasoningMode::Standard, disabled: &[],
///     history: &[], peers: &[], now: 0, cooldown: None,
/// });
/// assert_eq!(states[0].status, AdvisorStatus::Unavailable);
/// assert_eq!(states[0].reason, "executable_not_found");
/// ```
pub fn derive_availability(catalogue: &AdvisorCatalogue, inputs: &Inputs<'_>) -> Vec<AdvisorState> {
    let all_disabled = inputs.disabled.iter().any(|d| d == "all");
    let mut out = Vec::new();
    for a in &catalogue.advisors {
        let (probe, present, absent_status, absent_reason) = match a.transport {
            AdvisorTransport::Api => match catalogue.credential_of(a, inputs.refs) {
                Some(var) => (
                    format!("credential variable {var} set"),
                    inputs.presence.variable(var),
                    AdvisorStatus::NotConfigured,
                    "credential_absent",
                ),
                None => (
                    "no credential variable declared".to_string(),
                    false,
                    AdvisorStatus::NotConfigured,
                    "credential_undeclared",
                ),
            },
            AdvisorTransport::Cli | AdvisorTransport::LocalRuntime => match &a.executable {
                Some(exe) => (
                    format!("executable {exe} on PATH"),
                    inputs.presence.executable(exe),
                    AdvisorStatus::Unavailable,
                    "executable_not_found",
                ),
                None => (
                    "no executable declared".to_string(),
                    false,
                    AdvisorStatus::Unavailable,
                    "executable_undeclared",
                ),
            },
            AdvisorTransport::Peer => (
                "peers are discovered, not declared".to_string(),
                false,
                AdvisorStatus::Unavailable,
                "peer_declared_statically",
            ),
        };
        let mut state = AdvisorState {
            id: a.id.clone(),
            title: a.title.clone(),
            transport: a.transport,
            adapter: a.adapter.clone(),
            capabilities: a.capabilities.clone(),
            model: a.model.as_deref().map(|m| {
                inputs
                    .refs
                    .models
                    .resolve(m)
                    .map_or(m, |e| e.native_id.as_str())
                    .to_string()
            }),
            executable: a.executable.clone(),
            credential: catalogue.credential_of(a, inputs.refs).map(str::to_string),
            status: AdvisorStatus::Available,
            reason: "present".into(),
            probe,
            recovering: false,
            circuit: None,
        };
        if let Some((status, reason)) = gate(a.transport, &a.id, inputs, all_disabled) {
            state.status = status;
            state.reason = reason;
        } else if !present {
            state.status = absent_status;
            state.reason = absent_reason.into();
        } else {
            apply_circuit(&mut state, inputs);
        }
        out.push(state);
    }
    if let Some(decl) = &catalogue.peers {
        // by runtime, which is a peer's durable identity
        let peers: std::collections::BTreeMap<&str, &LinkedPeer> = inputs
            .peers
            .iter()
            .filter(|p| p.features.iter().any(|f| f == &decl.feature))
            .map(|p| (p.runtime.as_str(), p))
            .collect();
        for p in peers.into_values() {
            let id = format!("peer:{}", p.runtime);
            let mut state = AdvisorState {
                title: format!("Mesh peer {}", p.name),
                transport: AdvisorTransport::Peer,
                adapter: decl.adapter.clone(),
                capabilities: decl.capabilities.clone(),
                model: None,
                executable: None,
                credential: None,
                status: AdvisorStatus::Available,
                reason: "peer_linked".into(),
                probe: format!("linked runtime carrying the {} feature", decl.feature),
                recovering: false,
                circuit: None,
                id,
            };
            if let Some((status, reason)) =
                gate(AdvisorTransport::Peer, &state.id, inputs, all_disabled)
            {
                state.status = status;
                state.reason = reason;
            } else {
                apply_circuit(&mut state, inputs);
            }
            out.push(state);
        }
    }
    out
}

/// The mode and the disable list, which outrank presence.
fn gate(
    transport: AdvisorTransport,
    id: &str,
    inputs: &Inputs<'_>,
    all_disabled: bool,
) -> Option<(AdvisorStatus, String)> {
    match inputs.mode {
        ReasoningMode::Ci => return Some((AdvisorStatus::Disabled, "mode_ci".into())),
        ReasoningMode::Offline if transport.is_remote() => {
            return Some((AdvisorStatus::Disabled, "mode_offline".into()))
        }
        _ => {}
    }
    if all_disabled || inputs.disabled.iter().any(|d| d == id) {
        return Some((AdvisorStatus::Disabled, "disabled_by_configuration".into()));
    }
    None
}

/// Open or close the advisor's circuit from its recorded outcomes: newest first, a
/// completed consultation closes it, and only failures that say something about the
/// advisor's standing count — a malformed or empty answer, or a cancellation, is about
/// one exchange, not about whether the advisor can be asked.
fn apply_circuit(state: &mut AdvisorState, inputs: &Inputs<'_>) {
    // newest first: by stamp, then id, so equal stamps still have one order
    let by_time: std::collections::BTreeMap<(i64, &str), &Outcome> = inputs
        .history
        .iter()
        .filter(|o| o.advisor == state.id)
        .map(|o| ((o.at, o.id.as_str()), o))
        .collect();
    let mine: Vec<&Outcome> = by_time.into_values().rev().collect();
    let Some(newest) = mine.first() else { return };
    let cooldown = |default: i64| inputs.cooldown.unwrap_or(default);
    let (status, reason, until, failures) = match newest.status {
        ConsultationStatus::AuthFailed => (
            AdvisorStatus::TemporarilyFailed,
            "authentication_failed",
            newest.at + cooldown(AUTH_COOLDOWN),
            1,
        ),
        ConsultationStatus::RateLimited => {
            let wait = newest
                .retry_after
                .map(|s| s as i64)
                .unwrap_or(RATE_LIMIT_COOLDOWN);
            (
                AdvisorStatus::RateLimited,
                "rate_limited",
                newest.at + cooldown(wait),
                1,
            )
        }
        s if s.is_transient() => {
            let run = mine.iter().take_while(|o| o.status.is_transient()).count() as u32;
            if run < FAILURES_TO_OPEN {
                return;
            }
            (
                AdvisorStatus::TemporarilyFailed,
                "consecutive_failures",
                newest.at + cooldown(FAILURE_COOLDOWN),
                run,
            )
        }
        _ => return,
    };
    if inputs.now >= until {
        state.recovering = true;
        return;
    }
    state.status = status;
    state.reason = reason.into();
    state.circuit = Some(AdvisorCircuit {
        opened_by: newest.id.clone(),
        failures,
        until: super::stamp_of(until),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        executables: Vec<&'static str>,
        variables: Vec<&'static str>,
    }
    impl Presence for Fake {
        fn executable(&self, name: &str) -> bool {
            self.executables.contains(&name)
        }
        fn variable(&self, name: &str) -> bool {
            self.variables.contains(&name)
        }
    }

    fn catalogue() -> (AdvisorCatalogue, References) {
        let c = crate::metadata::yaml::parse_into(
            "version: 1\ncapabilities:\n  - id: independent_reasoning\n    description: x\n  - id: code_review\n    description: y\nadvisors:\n  - id: remote\n    title: Remote\n    transport: api\n    adapter: remote-api\n    vendor: acme\n    capabilities: [independent_reasoning]\n  - id: tool\n    title: Tool\n    transport: cli\n    adapter: tool-cli\n    executable: tool\n    capabilities: [code_review]\n  - id: local\n    title: Local\n    transport: local_runtime\n    adapter: local-rt\n    executable: rt\n    capabilities: [independent_reasoning]\npeers:\n  feature: reviews\n  adapter: mesh-review\n  capabilities: [code_review]\n",
        )
        .unwrap();
        let models = crate::metadata::yaml::parse_into(
            "version: 1\nvendors:\n  - id: acme\n    title: Acme\n    credential_env: ACME_KEY\nmodels: []\n",
        )
        .unwrap();
        (
            c,
            References {
                providers: vec![],
                models,
            },
        )
    }

    fn states(
        presence: &Fake,
        mode: ReasoningMode,
        history: &[Outcome],
        now: i64,
    ) -> Vec<AdvisorState> {
        let (c, refs) = catalogue();
        derive_availability(
            &c,
            &Inputs {
                refs: &refs,
                presence,
                mode,
                disabled: &[],
                history,
                peers: &[],
                now,
                cooldown: None,
            },
        )
    }

    fn everything() -> Fake {
        Fake {
            executables: vec!["tool", "rt"],
            variables: vec!["ACME_KEY"],
        }
    }

    fn outcome(id: &str, status: ConsultationStatus, at: i64) -> Outcome {
        Outcome {
            id: id.into(),
            advisor: "remote".into(),
            status,
            at,
            retry_after: None,
        }
    }

    #[test]
    fn nothing_present_is_an_ordinary_answer() {
        let none = Fake {
            executables: vec![],
            variables: vec![],
        };
        let s = states(&none, ReasoningMode::Standard, &[], 0);
        let words: Vec<(&str, &str)> = s
            .iter()
            .map(|a| (a.status.as_str(), a.reason.as_str()))
            .collect();
        assert_eq!(
            words,
            [
                ("not_configured", "credential_absent"),
                ("unavailable", "executable_not_found"),
                ("unavailable", "executable_not_found")
            ]
        );
    }

    #[test]
    fn ci_admits_nothing_and_offline_admits_only_the_machine() {
        let ci = states(&everything(), ReasoningMode::Ci, &[], 0);
        assert!(ci
            .iter()
            .all(|a| a.status == AdvisorStatus::Disabled && a.reason == "mode_ci"));
        let offline = states(&everything(), ReasoningMode::Offline, &[], 0);
        let available: Vec<&str> = offline
            .iter()
            .filter(|a| a.status == AdvisorStatus::Available)
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(available, ["local"]);
    }

    #[test]
    fn two_transient_failures_open_the_circuit_and_the_cooldown_closes_it() {
        let one = [outcome("c1", ConsultationStatus::Timeout, 100)];
        assert_eq!(
            states(&everything(), ReasoningMode::Standard, &one, 101)[0].status,
            AdvisorStatus::Available
        );
        let two = [
            outcome("c1", ConsultationStatus::Timeout, 100),
            outcome("c2", ConsultationStatus::Unavailable, 110),
        ];
        let open = &states(&everything(), ReasoningMode::Standard, &two, 111)[0];
        assert_eq!(open.status, AdvisorStatus::TemporarilyFailed);
        assert_eq!(open.circuit.as_ref().unwrap().failures, 2);
        assert_eq!(open.circuit.as_ref().unwrap().opened_by, "c2");
        let later = &states(
            &everything(),
            ReasoningMode::Standard,
            &two,
            110 + FAILURE_COOLDOWN,
        )[0];
        assert_eq!(later.status, AdvisorStatus::Available);
        assert!(later.recovering, "back after the cooldown, and marked so");
    }

    #[test]
    fn a_success_closes_the_circuit_and_order_is_by_time_not_by_arrival() {
        // ReasoningRecorded out of order: the success is newest, so the circuit is closed.
        let h = [
            outcome("c3", ConsultationStatus::Completed, 120),
            outcome("c1", ConsultationStatus::Timeout, 100),
            outcome("c2", ConsultationStatus::Timeout, 110),
        ];
        let s = &states(&everything(), ReasoningMode::Standard, &h, 121)[0];
        assert_eq!(s.status, AdvisorStatus::Available);
        assert!(!s.recovering);
        // The same outcomes with the success oldest: open.
        let h = [
            outcome("c3", ConsultationStatus::Completed, 90),
            outcome("c1", ConsultationStatus::Timeout, 100),
            outcome("c2", ConsultationStatus::Timeout, 110),
        ];
        assert_eq!(
            states(&everything(), ReasoningMode::Standard, &h, 111)[0].status,
            AdvisorStatus::TemporarilyFailed
        );
    }

    #[test]
    fn rate_limits_and_authentication_are_their_own_states() {
        let mut limited = outcome("c1", ConsultationStatus::RateLimited, 100);
        limited.retry_after = Some(30);
        let s = &states(
            &everything(),
            ReasoningMode::Standard,
            &[limited.clone()],
            120,
        )[0];
        assert_eq!(s.status, AdvisorStatus::RateLimited);
        assert_eq!(
            states(&everything(), ReasoningMode::Standard, &[limited], 131)[0].status,
            AdvisorStatus::Available
        );
        let auth = [outcome("c1", ConsultationStatus::AuthFailed, 100)];
        let s = &states(
            &everything(),
            ReasoningMode::Standard,
            &auth,
            100 + AUTH_COOLDOWN - 1,
        )[0];
        assert_eq!(
            (s.status, s.reason.as_str()),
            (AdvisorStatus::TemporarilyFailed, "authentication_failed")
        );
    }

    #[test]
    fn a_malformed_answer_says_nothing_about_standing() {
        let h = [
            outcome("c1", ConsultationStatus::Malformed, 100),
            outcome("c2", ConsultationStatus::Malformed, 110),
            outcome("c3", ConsultationStatus::Cancelled, 120),
        ];
        assert_eq!(
            states(&everything(), ReasoningMode::Standard, &h, 121)[0].status,
            AdvisorStatus::Available
        );
    }

    #[test]
    fn linked_peers_become_advisors_from_the_declaration() {
        let (c, refs) = catalogue();
        let peers = [
            LinkedPeer {
                runtime: "n1-r1".into(),
                name: "laptop".into(),
                features: vec!["reviews".into()],
            },
            LinkedPeer {
                runtime: "n2-r1".into(),
                name: "ci".into(),
                features: vec!["sessions".into()],
            },
        ];
        let s = derive_availability(
            &c,
            &Inputs {
                refs: &refs,
                presence: &everything(),
                mode: ReasoningMode::Standard,
                disabled: &["tool".into()],
                history: &[],
                peers: &peers,
                now: 0,
                cooldown: None,
            },
        );
        let peer = s
            .iter()
            .find(|a| a.id == "peer:n1-r1")
            .expect("the peer is an advisor");
        assert_eq!(peer.capabilities, ["code_review"]);
        assert_eq!(peer.status, AdvisorStatus::Available);
        assert!(
            s.iter().all(|a| a.id != "peer:n2-r1"),
            "a peer without the feature is no advisor"
        );
        assert_eq!(
            s.iter().find(|a| a.id == "tool").unwrap().reason,
            "disabled_by_configuration"
        );
    }
}
