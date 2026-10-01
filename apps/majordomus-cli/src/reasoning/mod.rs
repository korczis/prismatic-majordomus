//! Reasoning: how a session turns material engineering uncertainty into a recorded,
//! evidence-backed conclusion — with independent advisors when some are present, and
//! without them when none are. ADR 0098 is the decision; `docs/REASONING.md` the manual.
//!
//! Four separations hold the design together, and each is a module boundary here:
//!
//! * **Reasoning is not a provider.** [`policy`] decides *whether* independent review is
//!   worth having and which advisory capabilities it needs; it never names a vendor, a
//!   client tool or a model. A test holds that no source file of this module does.
//! * **Advisors are declared once.** [`catalogue`] reads `share/advisors.yaml`: advisory
//!   roles that *reference* the provider table (`share/providers.yaml`) and the model
//!   catalogue (`share/models.yaml`) rather than restating them. Mesh peers carrying the
//!   `reviews` feature become advisors at run time from the same declaration.
//! * **Absence is data.** [`availability`] turns presence (an executable on `PATH`, a
//!   credential variable set — never read), the reasoning mode and the recorded outcomes
//!   of earlier consultations into one typed status per advisor. No advisor at all is an
//!   ordinary answer, not an error, and nothing here touches the network (ADR 0032).
//! * **Records, not transcripts.** [`record`] is the typed vocabulary a session writes —
//!   assessment, plan, consultation, disagreement, resolution, conclusion, validation,
//!   attempt — and [`store`] the one writer, which validates every reference before a
//!   record becomes durable. A consultation may name only an advisor its recorded plan
//!   selected; a conclusion's review count is computed, never authored; a disagreement is
//!   closed by evidence, never by a count. [`state`] derives the session's reasoning state,
//!   its timeline and its report from the records alone, deterministically.
//!
//! Talking to an advisor is the one thing this module does not do. The transport is the
//! Node layer under `scripts/lib/advisors/`, the side of ADR 0032's boundary that already
//! holds the repository's network code: it asks this crate for a plan, consults what the
//! plan selected, and records each normalised outcome back through the same writer.
//!
//! # Example
//!
//! A checkout whose share declares mesh peers as advisors, with one reviewing peer
//! linked: the situation resolves it as available, and a plan for a material uncertainty
//! selects it.
//!
//! ```
//! use majordomus_cli::reasoning::availability::LinkedPeer;
//! use majordomus_cli::reasoning::{plan, Materiality, PlanOutcome, PlanRequest, Situation};
//! use majordomus_cli::share::ProviderDeclarations;
//!
//! // the mode is named outright, so the example does not depend on the runner
//! std::env::set_var("MAJORDOMUS_REASONING_MODE", "standard");
//! std::env::remove_var("MAJORDOMUS_ADVISORS_DISABLE");
//! let root = tempfile::tempdir().unwrap();
//! let share = tempfile::tempdir().unwrap();
//! std::fs::write(
//!     share.path().join("advisors.yaml"),
//!     "version: 1\ncapabilities:\n  - id: independent_reasoning\n    description: x\npeers:\n  feature: reviews\n  adapter: mesh-review\n  capabilities: [independent_reasoning]\n",
//! ).unwrap();
//! let peer = LinkedPeer { runtime: "n1-r1".into(), name: "one".into(), features: vec!["reviews".into()] };
//!
//! let situation = Situation::resolve(
//!     root.path(), Some(share.path()), &ProviderDeclarations::default(), &[peer],
//! ).unwrap();
//! assert_eq!(situation.available(), 1);
//! assert!(situation.loaded.records.is_empty(), "no record has been written yet");
//!
//! let request = PlanRequest { materiality: Materiality::Material, ..Default::default() };
//! let p = plan(&request, &situation.mode, &situation.states);
//! assert_eq!(p.outcome, PlanOutcome::Consult);
//! assert_eq!(p.selected[0].advisor, "peer:n1-r1");
//! ```

pub mod availability;
pub mod catalogue;
pub mod check;
pub mod policy;
pub mod record;
pub mod state;
pub mod store;

pub use availability::{
    derive_availability, AdvisorCircuit, AdvisorState, AdvisorStatus, ModeResolution, Presence,
    ReasoningMode, SystemPresence,
};
pub use catalogue::{AdvisorCatalogue, AdvisorDeclaration, AdvisorTransport, ADVISORS_FILE};
pub use policy::{plan, Materiality, PlanOutcome, PlanRequest, ReasoningConfidence, ReviewPlan};

/// Everything reasoning reads about this checkout, resolved once: the catalogue and the
/// references it resolves against, the mode, every advisor's standing and the records.
/// Each surface — the capabilities, the environment snapshot — starts here, so none of
/// them derives availability a second way.
///
/// ```
/// use majordomus_cli::reasoning::Situation;
/// use majordomus_cli::share::ProviderDeclarations;
///
/// // no share: no catalogue, no advisor, and still a situation to reason in
/// let root = tempfile::tempdir().unwrap();
/// let s = Situation::resolve(root.path(), None, &ProviderDeclarations::default(), &[]).unwrap();
/// assert!(s.catalogue.advisors.is_empty() && s.states.is_empty());
/// assert!(s.diagnostics.is_empty());
/// assert_eq!(s.available(), 0);
/// ```
pub struct Situation {
    /// The advisor catalogue.
    pub catalogue: AdvisorCatalogue,
    /// What it references.
    pub refs: catalogue::References,
    /// The mode in force.
    pub mode: ModeResolution,
    /// Every advisor's standing now.
    pub states: Vec<AdvisorState>,
    /// The record store.
    pub store: store::Store,
    /// Its records.
    pub loaded: store::Loaded,
    /// The catalogue's own findings.
    pub diagnostics: Vec<String>,
}

/// The profile of the open task and the reasoning mode it declares.
fn profile_mode(root: &std::path::Path) -> Option<(String, Option<String>)> {
    let current = std::fs::read_to_string(root.join(".ai/local/state/current.yaml")).ok()?;
    let name = current
        .lines()
        .find_map(|l| l.strip_prefix("profile:"))?
        .trim()
        .trim_matches('"')
        .to_string();
    let profile = std::fs::read_to_string(root.join(format!(".ai/repo/profiles/{name}.yaml"))).ok();
    let mode = profile.and_then(|t| {
        t.lines()
            .find_map(|l| l.strip_prefix("reasoning:"))
            .map(|v| v.trim().trim_matches('"').to_string())
    });
    Some((name, mode))
}

fn env_word(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

impl Situation {
    /// Resolve the situation of the checkout at `root`: no network, no model, no build.
    /// The catalogue comes from `share` (none without one), the mode from the environment,
    /// a CI runner or the open task's profile, availability from presence, the disable
    /// list, the recorded consultations and the linked `peers`. Only a catalogue that
    /// cannot be read is an error.
    ///
    /// ```
    /// use majordomus_cli::reasoning::{ReasoningMode, Situation};
    /// use majordomus_cli::share::ProviderDeclarations;
    ///
    /// std::env::set_var("MAJORDOMUS_REASONING_MODE", "offline");
    /// let root = tempfile::tempdir().unwrap();
    /// let s = Situation::resolve(root.path(), None, &ProviderDeclarations::default(), &[]).unwrap();
    /// assert_eq!(s.mode.mode, ReasoningMode::Offline);
    /// assert_eq!(s.mode.source, "MAJORDOMUS_REASONING_MODE");
    ///
    /// let share = tempfile::tempdir().unwrap();
    /// std::fs::write(share.path().join("advisors.yaml"), "version: one\n").unwrap();
    /// let refused = Situation::resolve(root.path(), Some(share.path()), &ProviderDeclarations::default(), &[]);
    /// assert!(refused.is_err());
    /// ```
    pub fn resolve(
        root: &std::path::Path,
        share: Option<&std::path::Path>,
        providers: &crate::share::ProviderDeclarations,
        peers: &[availability::LinkedPeer],
    ) -> Result<Self, String> {
        let (catalogue, refs) = match share {
            Some(share) => (
                AdvisorCatalogue::load(share)?,
                AdvisorCatalogue::references(share, providers),
            ),
            None => (
                AdvisorCatalogue::default(),
                catalogue::References::default(),
            ),
        };
        let profile = profile_mode(root);
        let ci = env_word("CI").is_some_and(|v| v != "false" && v != "0");
        let mode = availability::resolve_mode(
            env_word(availability::MODE_ENV).as_deref(),
            ci,
            profile.as_ref().map(|(n, m)| (n.as_str(), m.as_deref())),
        );
        let disabled: Vec<String> = env_word(availability::DISABLE_ENV)
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let cooldown = env_word(availability::COOLDOWN_ENV).and_then(|v| v.parse::<i64>().ok());
        let store = store::Store::at(root);
        let loaded = store.load();
        let history: Vec<availability::Outcome> = loaded
            .records
            .iter()
            .filter_map(|r| match &r.body {
                record::ReasoningBody::Consultation(c) => Some(availability::Outcome {
                    id: r.id.clone(),
                    advisor: c.advisor.clone(),
                    status: c.status,
                    at: epoch_seconds(&r.recorded_at)?,
                    retry_after: c.retry_after_seconds,
                }),
                _ => None,
            })
            .collect();
        let states = derive_availability(
            &catalogue,
            &availability::Inputs {
                refs: &refs,
                presence: &SystemPresence,
                mode: mode.mode,
                disabled: &disabled,
                history: &history,
                peers,
                now: now_seconds(),
                cooldown,
            },
        );
        let diagnostics = catalogue.diagnostics(&refs);
        Ok(Self {
            catalogue,
            refs,
            mode,
            states,
            store,
            loaded,
            diagnostics,
        })
    }

    /// How many advisors may be asked now: those whose status is `available`, whatever
    /// the number declared. Zero is an ordinary answer.
    ///
    /// ```
    /// use majordomus_cli::reasoning::availability::LinkedPeer;
    /// use majordomus_cli::reasoning::Situation;
    /// use majordomus_cli::share::ProviderDeclarations;
    ///
    /// std::env::set_var("MAJORDOMUS_REASONING_MODE", "ci");
    /// let root = tempfile::tempdir().unwrap();
    /// let share = tempfile::tempdir().unwrap();
    /// std::fs::write(
    ///     share.path().join("advisors.yaml"),
    ///     "version: 1\ncapabilities:\n  - id: code_review\n    description: x\npeers:\n  feature: reviews\n  adapter: mesh-review\n  capabilities: [code_review]\n",
    /// ).unwrap();
    /// let peer = LinkedPeer { runtime: "n1-r1".into(), name: "one".into(), features: vec!["reviews".into()] };
    /// let s = Situation::resolve(root.path(), Some(share.path()), &ProviderDeclarations::default(), &[peer]).unwrap();
    /// // mode ci admits no advisor, linked or not
    /// assert_eq!((s.states.len(), s.available()), (1, 0));
    /// ```
    pub fn available(&self) -> usize {
        self.states
            .iter()
            .filter(|a| a.status == AdvisorStatus::Available)
            .count()
    }
}

/// Seconds since the epoch of an RFC 3339 UTC stamp as this crate writes them
/// (`2026-10-01T09:30:00Z`, optionally with fractional seconds). `None` for anything else:
/// a stamp that cannot be read is not guessed at.
///
/// ```
/// use majordomus_cli::reasoning::epoch_seconds;
///
/// assert_eq!(epoch_seconds("1970-01-01T00:00:00Z"), Some(0));
/// assert_eq!(epoch_seconds("2026-10-01T00:00:10Z"), Some(1_790_812_810));
/// assert_eq!(epoch_seconds("2026-10-01T00:00:10.25Z"), Some(1_790_812_810));
/// assert_eq!(epoch_seconds("yesterday"), None);
/// ```
pub fn epoch_seconds(stamp: &str) -> Option<i64> {
    let b = stamp.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || !stamp.ends_with('Z') {
        return None;
    }
    let num = |from: usize, to: usize| stamp.get(from..to)?.parse::<i64>().ok();
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hh, mm, ss) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    // days from civil (Howard Hinnant's algorithm)
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hh * 3_600 + mm * 60 + ss)
}

/// The RFC 3339 stamp of seconds since the epoch — the inverse of [`epoch_seconds`].
///
/// ```
/// use majordomus_cli::reasoning::{epoch_seconds, stamp_of};
///
/// let s = "2026-10-01T09:30:00Z";
/// assert_eq!(stamp_of(epoch_seconds(s).unwrap()), s);
/// ```
pub fn stamp_of(seconds: i64) -> String {
    let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds.max(0) as u64);
    crate::peers::rfc3339(t)
}

/// Now as an RFC 3339 stamp with milliseconds (`2026-10-01T09:30:00.123Z`): the order of
/// records written within one second is the order they were written in, and the fixed
/// width keeps byte order and time order the same.
///
/// ```
/// use majordomus_cli::reasoning::{epoch_seconds, now_stamp};
///
/// let stamp = now_stamp();
/// assert_eq!(stamp.len(), "2026-10-01T09:30:00.123Z".len());
/// assert!(epoch_seconds(&stamp).unwrap() > 0);
/// ```
pub fn now_stamp() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let whole = stamp_of(d.as_secs() as i64);
    format!("{}.{:03}Z", whole.trim_end_matches('Z'), d.subsec_millis())
}

/// The current time, seconds since the epoch: the clock availability measures cooldowns
/// against. Zero on a clock set before the epoch rather than a panic.
///
/// ```
/// use majordomus_cli::reasoning::{epoch_seconds, now_seconds};
///
/// assert!(now_seconds() > epoch_seconds("2026-01-01T00:00:00Z").unwrap());
/// ```
pub fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_round_trip_and_unreadable_ones_are_refused() {
        for s in [
            "1970-01-01T00:00:00Z",
            "2000-02-29T23:59:59Z",
            "2026-10-01T09:30:00Z",
        ] {
            assert_eq!(stamp_of(epoch_seconds(s).unwrap()), s);
        }
        for bad in [
            "2026-13-01T00:00:00Z",
            "2026-10-01 09:30:00Z",
            "2026-10-01T09:30:00",
        ] {
            assert_eq!(epoch_seconds(bad), None, "{bad}");
        }
        let now = now_stamp();
        assert!(now.ends_with('Z') && now.as_bytes()[19] == b'.');
    }

    #[test]
    fn the_open_tasks_profile_names_its_mode() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(profile_mode(root.path()), None, "no open task, no profile");
        std::fs::create_dir_all(root.path().join(".ai/local/state")).unwrap();
        std::fs::create_dir_all(root.path().join(".ai/repo/profiles")).unwrap();
        std::fs::write(
            root.path().join(".ai/local/state/current.yaml"),
            "task: t-1\nprofile: \"deep-work\"\n",
        )
        .unwrap();
        assert_eq!(
            profile_mode(root.path()),
            Some(("deep-work".to_string(), None)),
            "a profile that declares no mode"
        );
        std::fs::write(
            root.path().join(".ai/repo/profiles/deep-work.yaml"),
            "id: deep-work\nreasoning: strict\n",
        )
        .unwrap();
        assert_eq!(
            profile_mode(root.path()),
            Some(("deep-work".to_string(), Some("strict".to_string())))
        );
    }

    #[test]
    fn a_checkout_without_a_share_resolves_to_no_advisor() {
        let root = tempfile::tempdir().unwrap();
        let s = Situation::resolve(
            root.path(),
            None,
            &crate::share::ProviderDeclarations::default(),
            &[],
        )
        .unwrap();
        assert!(s.states.is_empty() && s.diagnostics.is_empty());
        assert_eq!(s.available(), 0);
        assert!(s.loaded.records.is_empty());
    }
}
