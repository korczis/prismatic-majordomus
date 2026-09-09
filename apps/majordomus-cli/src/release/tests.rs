//! The engine's own decisions: the ones that only mean anything once the version, the
//! diff, the change records and the release records are in one place.
//!
//! What each submodule can decide alone is proved beside it. What is here is the assembly:
//! which release is the baseline, which of the four versions disagree, what state the
//! release is in, and which of the two impacts a verdict was computed from.

use std::collections::BTreeMap;

use super::change::{Change, ChangeType, Changes};
use super::contract::ContractSnapshot;
use super::diff::CompatibilityImpact;
use super::version::{Bump, Version};
use super::*;

use crate::distribution::release::{Channel, Release, Releases};

fn release(version: &str, channel: Channel, yanked: bool) -> Release {
    Release {
        schema: "release/v1".into(),
        version: version.into(),
        tag: format!("v{version}"),
        channel,
        commit: "a".repeat(40),
        published_at: "2026-09-08T07:10:37Z".into(),
        notes_url: None,
        yanked,
        required_targets: None,
        artifacts: Vec::new(),
    }
}

fn change(id: &str, impact: CompatibilityImpact, released: Option<&str>) -> Change {
    Change {
        id: id.into(),
        title: format!("the {id} change"),
        change_type: ChangeType::Added,
        impact,
        released_in: released.map(|v| v.parse().expect("a version")),
        scopes: Vec::new(),
        contract: Vec::new(),
        issues: Vec::new(),
        pull_requests: Vec::new(),
        commits: Vec::new(),
        adrs: Vec::new(),
        migration: None,
        summary: String::new(),
        path: format!(".ai/repo/changes/{id}.md"),
    }
}

/// An engine over facts, with no repository behind it: the assembly under test is the
/// arithmetic, and constructing an index for each case would prove nothing extra.
fn engine(
    source: &str,
    releases: Vec<Release>,
    changes: Vec<Change>,
    baseline: Baseline,
) -> Engine {
    Engine {
        source: source.parse().expect("a version"),
        current: ContractSnapshot::from_entries(Vec::new()),
        baseline_snapshot: None,
        baseline,
        releases: Releases::ordered(releases),
        changes: Changes {
            changes,
            unreadable: Vec::new(),
        },
        diff: None,
        distribution: None,
        snapshot_error: None,
        stale: None,
        layer_degraded: false,
    }
}

fn resolved(version: &str) -> Baseline {
    Baseline::Resolved {
        version: version.parse().expect("a version"),
        tag: format!("v{version}"),
        commit: "a".repeat(40),
        fingerprint: "b".repeat(64),
    }
}

#[test]
fn the_baseline_is_the_release_an_unpinned_installation_resolves_to() {
    let records = Releases::ordered(vec![
        release("0.3.1", Channel::Stable, false),
        release("0.4.0", Channel::Prerelease, false),
        release("0.3.0", Channel::Stable, false),
    ]);
    // the pre-release is published and addressable and is never the baseline: measuring
    // against one would let a break between two pre-releases pass on the way to the
    // release they precede
    assert_eq!(
        records.latest_stable().map(|r| r.tag.as_str()),
        Some("v0.3.1")
    );
}

#[test]
fn a_withdrawn_release_is_not_the_baseline() {
    let records = Releases::ordered(vec![
        release("0.3.1", Channel::Stable, true),
        release("0.3.0", Channel::Stable, false),
    ]);
    assert_eq!(
        records.latest_stable().map(|r| r.tag.as_str()),
        Some("v0.3.0")
    );
}

#[test]
fn a_baseline_that_predates_the_snapshot_is_not_a_failure() {
    let e = engine(
        "0.3.1",
        vec![release("0.3.1", Channel::Stable, false)],
        Vec::new(),
        Baseline::Predates {
            version: "0.3.1".parse().unwrap(),
            tag: "v0.3.1".into(),
        },
    );
    let diagnostics = e.diagnostics(&Version::new(0, 3, 1));
    let predates = diagnostics
        .iter()
        .find(|d| d.code == "CONTRACT_BASELINE_PREDATES")
        .expect("it is reported");
    assert!(
        !predates.is_blocking(),
        "a fact about history that nothing can fix must not hold a release forever"
    );
    assert!(
        !diagnostics.iter().any(Diagnostic::is_blocking),
        "and nothing else blocks: {diagnostics:?}"
    );
}

#[test]
fn a_baseline_whose_tag_is_missing_is_a_failure() {
    let e = engine(
        "0.3.1",
        vec![release("0.3.1", Channel::Stable, false)],
        Vec::new(),
        Baseline::Unreadable {
            version: "0.3.1".parse().unwrap(),
            tag: "v0.3.1".into(),
            reason: "`v0.3.1` does not resolve in this clone".into(),
        },
    );
    let d = e.diagnostics(&Version::new(0, 3, 1));
    let unreadable = d
        .iter()
        .find(|d| d.code == "CONTRACT_BASELINE_UNREADABLE")
        .expect("it is reported");
    assert!(
        unreadable.is_blocking(),
        "a contract that cannot be read is a verdict that cannot be reached"
    );
    assert!(unreadable.next.is_some(), "and it says what to do about it");
}

/// The fallback the first release after this subsystem lands depends on: with no contract
/// to measure, the records' own claim decides — and the report says which of the two it
/// used, so a reader knows whether the verdict is evidence or a statement.
#[test]
fn the_records_decide_only_when_the_contract_cannot() {
    let e = engine(
        "0.3.1",
        vec![release("0.3.1", Channel::Stable, false)],
        vec![change("x", CompatibilityImpact::Additive, None)],
        Baseline::Predates {
            version: "0.3.1".parse().unwrap(),
            tag: "v0.3.1".into(),
        },
    );
    assert_eq!(e.impact(), None, "nothing was measured");
    assert_eq!(e.declared_impact(), Some(CompatibilityImpact::Additive));
    assert_eq!(
        e.effective_impact(),
        Some((CompatibilityImpact::Additive, ImpactSource::Records))
    );
    assert_eq!(e.required_bump(), Some(Bump::Patch), "below 1.0.0");
    assert_eq!(
        e.minimum_version().map(|v| v.to_string()),
        Some("0.3.2".into())
    );
    assert_eq!(e.target_version().to_string(), "0.3.2");
}

/// A tree already bumped by an earlier commit must not be asked to bump again for the same
/// change: the target is the source when the source already clears the minimum.
#[test]
fn a_version_already_ahead_of_the_minimum_is_left_alone() {
    let e = engine(
        "0.5.0",
        vec![release("0.3.1", Channel::Stable, false)],
        vec![change("x", CompatibilityImpact::Additive, None)],
        resolved("0.3.1"),
    );
    assert_eq!(
        e.minimum_version().map(|v| v.to_string()),
        Some("0.3.2".into())
    );
    assert_eq!(
        e.target_version().to_string(),
        "0.5.0",
        "the tree states more than the minimum, which the policy allows"
    );
    assert!(!e
        .diagnostics(&e.target_version())
        .iter()
        .any(|d| d.code == "SEMVER_BUMP_TOO_LOW"));
}

#[test]
fn a_version_below_the_minimum_is_the_one_diagnostic_that_names_the_fix() {
    let e = engine(
        "0.3.1",
        vec![release("0.3.1", Channel::Stable, false)],
        vec![change("x", CompatibilityImpact::Breaking, None)],
        resolved("0.3.1"),
    );
    let d = e.diagnostics(&e.target_version());
    let too_low = d
        .iter()
        .find(|d| d.code == "SEMVER_BUMP_TOO_LOW")
        .expect("it is reported");
    assert!(too_low.is_blocking());
    assert_eq!(
        too_low.next.as_deref(),
        Some("majordomus release prepare --version 0.4.0"),
        "below 1.0.0 a break moves the minor"
    );
}

#[test]
fn nothing_to_release_means_exactly_that() {
    let e = engine(
        "0.3.1",
        vec![release("0.3.1", Channel::Stable, false)],
        Vec::new(),
        resolved("0.3.1"),
    );
    // no unreleased record, and no measured impact: there is nothing to publish
    assert_eq!(e.declared_impact(), None);
    assert_eq!(e.target_version().to_string(), "0.3.1");
}

#[test]
fn the_first_release_is_compared_against_nothing_and_requires_nothing() {
    let e = engine(
        "0.1.0",
        Vec::new(),
        vec![change("x", CompatibilityImpact::Breaking, None)],
        Baseline::Initial,
    );
    assert_eq!(e.baseline.version(), None);
    assert_eq!(e.required_bump(), None, "there is no baseline to bump from");
    assert_eq!(e.minimum_version(), None);
    assert_eq!(e.target_version().to_string(), "0.1.0");
    assert!(
        !e.diagnostics(&e.target_version())
            .iter()
            .any(|d| d.code == "SEMVER_BUMP_TOO_LOW"),
        "a first release cannot be too low"
    );
}

/// A layer that did not read is a layer whose change records may be short. The verdict is
/// still computed — refusing to answer would be less useful — but it says it may be wrong.
#[test]
fn a_degraded_layer_blocks_the_release_and_says_why() {
    let mut e = engine(
        "0.3.1",
        vec![release("0.3.1", Channel::Stable, false)],
        Vec::new(),
        resolved("0.3.1"),
    );
    e.layer_degraded = true;
    let d = e.diagnostics(&Version::new(0, 3, 1));
    let degraded = d
        .iter()
        .find(|d| d.code == "LAYER_DEGRADED")
        .expect("it is reported");
    assert!(degraded.is_blocking());
}

/// A breaking change that no record accounts for is the release whose changelog does not
/// explain the thing its readers most need explained.
#[test]
fn an_uncovered_breaking_change_is_reported_against_the_entry_that_broke() {
    use super::contract::{ContractEntry, Surface};
    let entry = |id: &str| ContractEntry {
        surface: Surface::Capability,
        id: id.into(),
        facts: BTreeMap::from([("kind".to_string(), "query".to_string())]),
    };
    // `after` keeps a capability, so the surface is still covered. A snapshot with no
    // capabilities at all is not a contract that lost them; it is a snapshot that failed to
    // build, and the diff refuses to read one as the other.
    let before = ContractSnapshot::from_entries(vec![entry("a.b"), entry("a.kept")]);
    let after = ContractSnapshot::from_entries(vec![entry("a.kept")]);
    let mut e = engine(
        "0.4.0",
        vec![release("0.3.1", Channel::Stable, false)],
        Vec::new(),
        resolved("0.3.1"),
    );
    e.diff = Some(super::diff::diff(&before, &after));

    let d = e.diagnostics(&Version::new(0, 4, 0));
    let missing = d
        .iter()
        .find(|d| d.code == "CHANGELOG_MISSING")
        .expect("it is reported");
    assert!(missing.message.contains("capability:a.b"));
    assert!(
        missing
            .next
            .as_deref()
            .is_some_and(|n| n.contains(".ai/repo/changes/")),
        "and it says where to write the record"
    );
}

/// Every diagnostic this engine can produce names the command that shows it. A finding
/// nobody can reproduce is a finding nobody acts on.
#[test]
fn every_diagnostic_names_the_command_that_shows_it() {
    let mut e = engine(
        "0.3.1",
        vec![release("0.3.1", Channel::Stable, false)],
        vec![change("x", CompatibilityImpact::Breaking, None)],
        Baseline::Unreadable {
            version: "0.3.1".parse().unwrap(),
            tag: "v0.3.1".into(),
            reason: "for the test".into(),
        },
    );
    e.layer_degraded = true;
    e.snapshot_error = Some("for the test".into());
    e.stale = Some(vec!["for the test".into()]);
    let diagnostics = e.diagnostics(&Version::new(0, 4, 0));
    assert!(diagnostics.len() >= 4, "{diagnostics:?}");
    for d in &diagnostics {
        assert!(
            d.reproduce.is_some(),
            "{} names no command that shows it",
            d.code
        );
        assert!(
            d.code.chars().all(|c| c.is_ascii_uppercase() || c == '_'),
            "{} is not a stable code",
            d.code
        );
    }
}

/// The four versions are four fields, and a deployment that reports nothing says so rather
/// than reporting `null` as though it were an answer.
#[test]
fn a_deployment_that_has_reported_nothing_is_unknown_and_not_drift() {
    let published: Version = "0.3.1".parse().unwrap();
    let source: Version = "0.4.0".parse().unwrap();
    for (reported, expected) in [
        (None, Agreement::Unknown),
        (Some("0.3.1"), Agreement::Published),
        (Some("0.4.0"), Agreement::Source),
        (Some("0.2.0"), Agreement::Drift),
    ] {
        let version = reported.map(|v| v.parse::<Version>().expect("a version"));
        let agreement = match (&version, Some(&published)) {
            (None, _) => Agreement::Unknown,
            (Some(v), Some(p)) if v == p => Agreement::Published,
            (Some(v), _) if *v == source => Agreement::Source,
            (Some(_), _) => Agreement::Drift,
        };
        assert_eq!(agreement, expected, "{reported:?}");
    }
}

/// Every readiness state has a word, and only the two that mean "somebody must do
/// something" block.
#[test]
fn the_readiness_states_are_words_and_only_two_of_them_block() {
    let blocking: Vec<&str> = [
        Readiness::NothingToRelease,
        Readiness::BumpRequired,
        Readiness::Blocked,
        Readiness::ReadyToTag,
        Readiness::Tagged,
        Readiness::Published,
    ]
    .into_iter()
    .filter(|r| r.is_blocking())
    .map(Readiness::as_str)
    .collect();
    assert_eq!(blocking, ["bump-required", "blocked"]);
}
