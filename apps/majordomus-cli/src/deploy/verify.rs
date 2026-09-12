//! Live verification: ask each applicable target what it is serving, and compare it with
//! what the change expects. The one place a deployment is judged by looking at it.
//!
//! # Why a 200 is not a verification
//!
//! The failure this exists for is the one every green pipeline hides: the deploy command
//! exited 0 and the old revision stayed live. On 2026-09-09 the public site served a commit
//! from an unmerged branch for half an hour; on 2026-09-10 it sat hours behind the trunk
//! while every tree-level gate stayed green. Reachability says nothing about either. So a
//! target is verified only when the identity it states — `/build.json` for the site,
//! `releases/latest.json` for the release metadata, `/api/v1/distribution/build` for a
//! running executable — names the commit, version or tag the change expects. A target that
//! answers with a different identity is **stale**, which refuses; one that does not answer
//! is **unreachable**, which also refuses; and neither is ever reported as a pass.
//!
//! # The network is behind a trait
//!
//! Nothing else in this executable reaches the network from a capability, and the
//! comparison must be testable without one. [`Fetcher`] is the one seam: the default
//! implementation shells out to `curl` — the tool the installer already requires, with a
//! bounded timeout and no header of any kind, so that no credential can be sent or
//! recorded — and a test supplies answers of its own. The evidence a verification produces
//! carries the URL that was asked and the identity it stated, never a header, a token or
//! a body beyond the fields compared.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::targets::{DeploymentPlan, DeploymentTarget, Identity, TargetKind};

/// How long one identity request may take.
pub const TIMEOUT_SECONDS: u64 = 20;

/// Where one target stands after being asked.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// The surface states the expected identity.
    Verified,
    /// The surface answered, and states a different identity: the deployment did not
    /// land, or an older one is still live.
    Stale,
    /// The surface did not answer.
    Unreachable,
    /// The surface answered with something this executable cannot read as an identity.
    Unreadable,
    /// The target does not apply to this change; nothing was asked.
    NotApplicable,
    /// Nothing is expected of the target, so nothing could be compared; it was reached.
    Unverifiable,
}

impl VerificationStatus {
    /// Whether this status refuses completion.
    pub fn refuses(self) -> bool {
        matches!(self, Self::Stale | Self::Unreachable | Self::Unreadable)
    }
    /// The word, as serialised.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Stale => "stale",
            Self::Unreachable => "unreachable",
            Self::Unreadable => "unreadable",
            Self::NotApplicable => "not_applicable",
            Self::Unverifiable => "unverifiable",
        }
    }
}

/// One target, asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Verification {
    /// The target's identity.
    pub target: String,
    /// Which kind of surface it is.
    pub kind: TargetKind,
    /// Where it stands.
    pub status: VerificationStatus,
    /// The address that was asked, when one was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asked: Option<String>,
    /// What was expected.
    #[serde(default)]
    pub expected: Identity,
    /// What the surface stated.
    #[serde(default)]
    pub observed: Identity,
    /// What was read, in one sentence: never a restatement of the status.
    pub detail: String,
    /// When it was asked.
    pub checked_at: String,
}

/// Every target, asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VerificationReport {
    /// True when every applicable target is verified and none refuses.
    pub ok: bool,
    /// How many targets were asked.
    pub asked: usize,
    /// The applicable targets whose status refuses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusing: Vec<String>,
    /// Each target, in the plan's order.
    pub verifications: Vec<Verification>,
    /// What could not be derived, from the plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

/// The one seam to the network.
pub trait Fetcher {
    /// GET a URL and return its body, or why it could not be read. No headers are ever
    /// sent and none are returned.
    fn get(&self, url: &str) -> Result<String, String>;
}

/// The default: `curl`, bounded, silent, following redirects, with no header.
pub struct CurlFetcher;

impl Fetcher for CurlFetcher {
    fn get(&self, url: &str) -> Result<String, String> {
        let out = std::process::Command::new("curl")
            .args([
                "-fsSL",
                "--max-time",
                &TIMEOUT_SECONDS.to_string(),
                "--no-progress-meter",
                url,
            ])
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| format!("curl could not be run: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "curl exited {} for {url}",
                out.status.code().unwrap_or(-1)
            ));
        }
        String::from_utf8(out.stdout).map_err(|_| format!("{url} answered with non-UTF-8 bytes"))
    }
}

/// A fetcher that answers from a table: what a test uses.
#[derive(Default)]
pub struct StaticFetcher {
    answers: std::collections::BTreeMap<String, Result<String, String>>,
}

impl StaticFetcher {
    /// Answer `url` with `body`.
    pub fn answers(mut self, url: &str, body: &str) -> Self {
        self.answers.insert(url.to_string(), Ok(body.to_string()));
        self
    }
    /// Refuse `url` with `why`.
    pub fn refuses(mut self, url: &str, why: &str) -> Self {
        self.answers.insert(url.to_string(), Err(why.to_string()));
        self
    }
}

impl Fetcher for StaticFetcher {
    fn get(&self, url: &str) -> Result<String, String> {
        self.answers
            .get(url)
            .cloned()
            .unwrap_or_else(|| Err(format!("no answer for {url}")))
    }
}

/// Read the identity a surface states, by the kind of surface it is.
fn identity_of(kind: TargetKind, body: &str) -> Result<Identity, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("not JSON: {e}"))?;
    let s = |key: &str| v.get(key).and_then(|x| x.as_str()).map(str::to_string);
    let id = match kind {
        TargetKind::Pages => Identity {
            commit: s("commit"),
            version: s("source_version"),
            tag: None,
        },
        TargetKind::Release => Identity {
            commit: s("commit"),
            version: s("version"),
            tag: s("tag"),
        },
        TargetKind::Application => Identity {
            commit: s("commit"),
            version: s("version"),
            tag: None,
        },
    };
    if id.is_empty() {
        return Err("the answer states no commit, version or tag".into());
    }
    Ok(id)
}

/// Compare what was expected with what was observed. A field the expectation does not
/// carry is not compared; a field it carries must match, commit by prefix so that a short
/// revision and a full one agree.
fn compare(expected: &Identity, observed: &Identity) -> Vec<String> {
    let mut mismatches = Vec::new();
    if let Some(e) = &expected.commit {
        match &observed.commit {
            Some(o) if o.starts_with(e) || e.starts_with(o.as_str()) => {}
            Some(o) => mismatches.push(format!("commit {o} is live, {e} expected")),
            None => mismatches.push(format!("no commit stated, {e} expected")),
        }
    }
    if let Some(e) = &expected.version {
        match &observed.version {
            Some(o) if o == e => {}
            Some(o) => mismatches.push(format!("version {o} is live, {e} expected")),
            None => mismatches.push(format!("no version stated, {e} expected")),
        }
    }
    if let Some(e) = &expected.tag {
        match &observed.tag {
            Some(o) if o == e => {}
            Some(o) => mismatches.push(format!("tag {o} is live, {e} expected")),
            None => mismatches.push(format!("no tag stated, {e} expected")),
        }
    }
    mismatches
}

/// Ask one target.
pub fn verify_target(target: &DeploymentTarget, fetcher: &dyn Fetcher, now: &str) -> Verification {
    let base = Verification {
        target: target.id.clone(),
        kind: target.kind,
        status: VerificationStatus::NotApplicable,
        asked: None,
        expected: target.expected.clone(),
        observed: Identity::default(),
        detail: target.reason.clone(),
        checked_at: now.to_string(),
    };
    if !target.applicable {
        return base;
    }
    let Some(url) = &target.identity_url else {
        return Verification {
            status: VerificationStatus::Unreachable,
            detail: "the target states no address that would name what it serves".into(),
            ..base
        };
    };
    let asked = Some(url.clone());
    match fetcher.get(url) {
        Err(why) => Verification {
            status: VerificationStatus::Unreachable,
            asked,
            detail: why,
            ..base
        },
        Ok(body) => match identity_of(target.kind, &body) {
            Err(why) => Verification {
                status: VerificationStatus::Unreadable,
                asked,
                detail: why,
                ..base
            },
            Ok(observed) => {
                if target.expected.is_empty() {
                    return Verification {
                        status: VerificationStatus::Unverifiable,
                        asked,
                        detail: format!(
                            "reached, and states {}; nothing was expected of it, so nothing \
                             was compared",
                            describe(&observed)
                        ),
                        observed,
                        ..base
                    };
                }
                let mismatches = compare(&target.expected, &observed);
                if mismatches.is_empty() {
                    Verification {
                        status: VerificationStatus::Verified,
                        asked,
                        detail: format!("states {}", describe(&observed)),
                        observed,
                        ..base
                    }
                } else {
                    Verification {
                        status: VerificationStatus::Stale,
                        asked,
                        detail: mismatches.join("; "),
                        observed,
                        ..base
                    }
                }
            }
        },
    }
}

fn describe(id: &Identity) -> String {
    let mut parts = Vec::new();
    if let Some(t) = &id.tag {
        parts.push(format!("tag {t}"));
    }
    if let Some(v) = &id.version {
        parts.push(format!("version {v}"));
    }
    if let Some(c) = &id.commit {
        parts.push(format!("commit {}", &c[..c.len().min(12)]));
    }
    parts.join(", ")
}

/// Ask every target of the plan.
pub fn verify(plan: &DeploymentPlan, fetcher: &dyn Fetcher, now: &str) -> VerificationReport {
    let verifications: Vec<Verification> = plan
        .targets
        .iter()
        .map(|t| verify_target(t, fetcher, now))
        .collect();
    let refusing: Vec<String> = verifications
        .iter()
        .filter(|v| v.status.refuses())
        .map(|v| v.target.clone())
        .collect();
    let asked = verifications.iter().filter(|v| v.asked.is_some()).count();
    VerificationReport {
        ok: refusing.is_empty() && asked > 0,
        asked,
        refusing,
        verifications,
        findings: plan.findings.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploy::targets::{plan, ApplicationFact, PlanFacts};

    fn facts() -> PlanFacts {
        PlanFacts {
            site_base_url: Some("https://s.test".into()),
            site_inputs: vec!["docs/**".into()],
            surface_inputs: vec!["apps/**".into()],
            latest_release: Some(Identity {
                commit: Some("aaaa".into()),
                version: Some("0.5.0".into()),
                tag: Some("v0.5.0".into()),
            }),
            applications: vec![ApplicationFact {
                id: "app".into(),
                status: "active".into(),
                url: Some("https://a.test".into()),
                inputs: vec!["apps/**".into()],
            }],
            expected_commit: Some("bbbbbbbbbbbb".into()),
            declared_version: Some("0.6.0".into()),
        }
    }

    #[test]
    fn a_surface_serving_the_expected_revision_is_verified_and_an_older_one_is_stale() {
        let p = plan(&facts(), &[], true);
        let fresh = StaticFetcher::default()
            .answers("https://s.test/build.json", r#"{"commit":"bbbbbbbbbbbbcccc","source_version":"0.6.0"}"#)
            .answers("https://s.test/releases/latest.json", r#"{"tag":"v0.5.0","version":"0.5.0","commit":"aaaa"}"#)
            .answers("https://a.test/api/v1/distribution/build", r#"{"version":"0.6.0","commit":"bbbbbbbbbbbb","target":"x"}"#);
        let r = verify(&p, &fresh, "now");
        assert!(r.ok, "{:?}", r.refusing);
        assert!(r.verifications.iter().all(|v| v.status == VerificationStatus::Verified));

        // the deploy command exited 0 and the old revision stayed live
        let stale = StaticFetcher::default()
            .answers("https://s.test/build.json", r#"{"commit":"aaaa","source_version":"0.5.0"}"#)
            .answers("https://s.test/releases/latest.json", r#"{"tag":"v0.5.0","version":"0.5.0","commit":"aaaa"}"#)
            .answers("https://a.test/api/v1/distribution/build", r#"{"version":"0.5.2","commit":"aaaa"}"#);
        let r = verify(&p, &stale, "now");
        assert!(!r.ok);
        assert_eq!(r.refusing, ["pages", "app"]);
        let pages = &r.verifications[0];
        assert_eq!(pages.status, VerificationStatus::Stale);
        assert!(pages.detail.contains("commit aaaa is live"), "{}", pages.detail);
        assert!(pages.detail.contains("version 0.5.0 is live"));
    }

    #[test]
    fn an_unreachable_or_unreadable_surface_refuses_and_a_200_is_not_a_pass() {
        let p = plan(&facts(), &[], true);
        let f = StaticFetcher::default()
            .refuses("https://s.test/build.json", "curl exited 22")
            .answers("https://s.test/releases/latest.json", "<html>200 OK but not metadata</html>")
            .answers("https://a.test/api/v1/distribution/build", r#"{"ok":true}"#);
        let r = verify(&p, &f, "now");
        assert!(!r.ok);
        assert_eq!(r.verifications[0].status, VerificationStatus::Unreachable);
        assert_eq!(r.verifications[1].status, VerificationStatus::Unreadable);
        assert_eq!(r.verifications[2].status, VerificationStatus::Unreadable, "a 200 with no identity");
        assert_eq!(r.refusing.len(), 3);
    }

    #[test]
    fn a_target_the_change_does_not_reach_is_not_asked() {
        let p = plan(&facts(), &["docs/x.md".into()], false);
        let f = StaticFetcher::default()
            .answers("https://s.test/build.json", r#"{"commit":"bbbbbbbbbbbb","source_version":"0.6.0"}"#);
        let r = verify(&p, &f, "now");
        assert!(r.ok);
        assert_eq!(r.asked, 1);
        let release = r.verifications.iter().find(|v| v.target == "release").unwrap();
        assert_eq!(release.status, VerificationStatus::NotApplicable);
        assert!(release.asked.is_none(), "nothing was asked");
    }

    #[test]
    fn nothing_asked_is_not_ok() {
        let mut f = facts();
        f.applications.clear();
        let p = plan(&f, &[], false);
        let r = verify(&p, &StaticFetcher::default(), "now");
        assert!(!r.ok, "a report that asked nothing verified nothing");
        assert_eq!(r.asked, 0);
    }

    #[test]
    fn nothing_expected_is_reached_but_unverifiable() {
        let mut f = facts();
        f.expected_commit = None;
        f.declared_version = None;
        f.applications.clear();
        let p = plan(&f, &["docs/x.md".into()], false);
        let fetch = StaticFetcher::default()
            .answers("https://s.test/build.json", r#"{"commit":"cccc","source_version":"0.9.0"}"#);
        let r = verify(&p, &fetch, "now");
        assert_eq!(r.verifications[0].status, VerificationStatus::Unverifiable);
        assert!(r.ok, "reached and nothing contradicted");
    }

    #[test]
    fn the_evidence_carries_no_header_and_only_the_fields_compared() {
        let p = plan(&facts(), &[], true);
        let f = StaticFetcher::default().answers(
            "https://s.test/build.json",
            r#"{"commit":"bbbbbbbbbbbb","source_version":"0.6.0","token":"ghp_secret","registry_fingerprint":"x"}"#,
        );
        let v = verify_target(&p.targets[0], &f, "now");
        let json = serde_json::to_string(&v).unwrap();
        assert!(!json.contains("ghp_secret"), "{json}");
        assert!(!json.contains("registry_fingerprint"));
    }
}
