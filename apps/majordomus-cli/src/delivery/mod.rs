//! Delivery: whether a product feature exists, computed and never recorded.
//!
//! The law this module holds (ADR 0071) is one conjunction. A feature **exists** only when
//! every one of six dimensions passes:
//!
//! ```text
//! ON_MASTER ∧ DEPLOYED ∧ PUBLICLY_VERIFIED ∧ REQUIRED_TESTS_CURRENT ∧ TEST_EVIDENCE_PUBLISHED ∧ UI_LINKED
//! ```
//!
//! Anything short of that is not delivered, and what it *is* instead — implemented on a
//! branch, on master, tested, deployed — is a [`DevelopmentStage`], a different type from
//! [`DeliveryState::Delivered`]. A development stage cannot be mistaken for delivery
//! because the two do not share a type, and delivery carries no stage because it needs none.
//!
//! Nothing here is stored. There is no `tested: true`, no `deployed: true` and no
//! `delivered: true` in any file: every verdict is derived on every read from git, from the
//! public site's own identity document and, from phase 2, from recorded test evidence. A
//! verdict is [`Verdict::Pass`], [`Verdict::Fail`] or [`Verdict::Unknown`], each with the
//! sentence that decided it and what to do about it, and **unknown is never pass**: a site
//! that could not be reached is not deployed, a clone without the trunk is not on master.
//!
//! Three halves, kept apart so that the conjunction is a pure function a test can walk
//! branch by branch:
//!
//! - [`revision`] reads git: which paths implement a feature, the latest commit touching
//!   them at `HEAD` and on the trunk, and whether one contains the other;
//! - [`public`] reads the publication: the public site's identity document, and the
//!   verification `scripts/pages` already owns;
//! - this file joins them into [`Dimension`]s, a [`DevelopmentStage`] and a
//!   [`DeliveryState`], and reads nothing.
//!
//! Phase 1 computes `REQUIRED_TESTS_CURRENT`, `TEST_EVIDENCE_PUBLISHED` and `UI_LINKED` as
//! unknown, with the reason, because the evidence model they are read from has not landed.
//! The dimensions exist now so that phase 2 fills a slot rather than widening a type — and
//! so that, until it does, no feature can be reported as existing.

pub(crate) mod public;
pub(crate) mod revision;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use self::public::{IdentityRead, Publication, SiteVerification};
use self::revision::{Containment, Revision};

/// The schema of the delivery report.
pub(crate) const SCHEMA: &str = "majordomus/delivery/v1";

/// Why the three evidence dimensions are unknown in phase 1. One sentence, said once.
pub(crate) const EVIDENCE_PENDING: &str = "evidence model lands with PR #577";

// ---------------------------------------------------------------- the vocabulary

/// One dimension's answer. Three values and never two: a question that could not be asked
/// is not a question answered yes, and it is not one answered no either.
// `scope::Verdict` and `quality::policy::Verdict` already publish the bare name, and one
// schema component cannot mean two things: this one is named for the delivery report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DeliveryVerdict")]
pub(crate) enum Verdict {
    /// The dimension holds, and the reason says what was observed.
    Pass,
    /// The dimension was measured and does not hold.
    Fail,
    /// The dimension could not be measured. Not a pass.
    Unknown,
}

/// The six dimensions of the delivery invariant, in the order the conjunction is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DimensionId {
    /// The feature's latest implementation revision is reachable from the trunk.
    OnMaster,
    /// The public site's identity commit contains that revision.
    Deployed,
    /// The publication was verified from outside: the site answers with a clean identity,
    /// a second probe agrees, and GitHub's own build of the published branch succeeded.
    PubliclyVerified,
    /// Every test the feature requires has a current passing execution.
    RequiredTestsCurrent,
    /// The evidence of those executions is published.
    TestEvidencePublished,
    /// The feature is linked from the user interface.
    UiLinked,
}

impl DimensionId {
    /// Every dimension, in the order of the conjunction.
    #[cfg(test)]
    pub(crate) const ALL: [DimensionId; 6] = [
        DimensionId::OnMaster,
        DimensionId::Deployed,
        DimensionId::PubliclyVerified,
        DimensionId::RequiredTestsCurrent,
        DimensionId::TestEvidencePublished,
        DimensionId::UiLinked,
    ];
}

/// One dimension, decided: the verdict, the sentence that decided it, and what would move it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Dimension {
    /// Which dimension.
    pub dimension: DimensionId,
    /// Pass, fail or unknown.
    pub verdict: Verdict,
    /// What was observed, in one sentence.
    pub reason: String,
    /// What would change the verdict; absent on a pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

impl Dimension {
    fn pass(dimension: DimensionId, reason: String) -> Self {
        Dimension {
            dimension,
            verdict: Verdict::Pass,
            reason,
            remediation: None,
        }
    }

    fn fail(dimension: DimensionId, reason: String, remediation: String) -> Self {
        Dimension {
            dimension,
            verdict: Verdict::Fail,
            reason,
            remediation: Some(remediation),
        }
    }

    fn unknown(dimension: DimensionId, reason: String, remediation: String) -> Self {
        Dimension {
            dimension,
            verdict: Verdict::Unknown,
            reason,
            remediation: Some(remediation),
        }
    }
}

/// How far development has carried a feature that does not exist yet. A ladder: a rung is
/// reached only when every rung below it passes, so a feature whose tests are unknown is
/// `on_master` even when the site already serves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DevelopmentStage {
    /// Git could not say where its implementation is.
    Unknown,
    /// It names no implementation, or nothing ever committed one.
    NotImplemented,
    /// Its latest implementation revision is on a branch and not on the trunk.
    ImplementedOnBranch,
    /// Its implementation is on the trunk.
    OnMaster,
    /// On the trunk, with every required test current.
    Tested,
    /// Tested, and contained in what the public site serves.
    Deployed,
}

/// Delivered, or not — and when not, how far development got and which dimensions stand in
/// the way. Two variants of one type rather than a flag beside a stage: a delivered feature
/// has no stage to report, and a stage can never be read as delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum DeliveryState {
    /// Every dimension passes.
    Delivered,
    /// At least one dimension does not pass.
    NotDelivered {
        /// How far development has carried it.
        stage: DevelopmentStage,
        /// The dimensions that do not pass, in the order of the conjunction.
        blocking: Vec<DimensionId>,
    },
}

/// The paths a feature is implemented by and the revisions that last touched them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Implementation {
    /// Repository-relative paths, derived from what the feature names: the source file of
    /// every capability module, `lib/<command>.sh` of every shell command the dispatcher
    /// sources, and the `implementation` of every claim.
    pub paths: Vec<String>,
    /// The latest commit touching them reachable from `HEAD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_revision: Option<String>,
    /// The latest commit touching them reachable from the trunk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_revision: Option<String>,
}

/// One feature against the invariant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct FeatureDelivery {
    /// The feature's id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// The status its file declares. Editorial, and not one of the dimensions.
    pub status: String,
    /// The conjunction: true only when every dimension passes.
    pub exists: bool,
    /// Delivered, or the development stage and what blocks it.
    pub delivery: DeliveryState,
    /// Every dimension, in the order of the conjunction.
    pub dimensions: Vec<Dimension>,
    /// What it is implemented by.
    pub implementation: Implementation,
}

/// How many features pass each measured dimension, and how many exist.
///
// `mesh::registry::Tallies` already publishes the bare name, and one schema component
// cannot mean two things: the counts of the delivery report are named for the report they
// belong to, as `plan::Transition` is named for its plan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "DeliveryTallies")]
pub(crate) struct Tallies {
    /// Features examined.
    pub features: usize,
    /// On the trunk.
    pub on_master: usize,
    /// Contained in the public identity.
    pub deployed: usize,
    /// Publicly verified.
    pub publicly_verified: usize,
    /// Every dimension passes.
    pub exists: usize,
}

/// Every feature of the layer against the delivery invariant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DeliveryReport {
    /// `majordomus/delivery/v1`.
    pub schema: String,
    /// The trunk ref every `on_master` verdict was measured against, when one was found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunk: Option<String>,
    /// What the public site was observed to serve, read once for every feature.
    pub publication: Publication,
    /// The counts.
    pub tallies: Tallies,
    /// Every feature, ordered by id.
    pub features: Vec<FeatureDelivery>,
}

// ---------------------------------------------------------------- the join

fn short(sha: &str) -> &str {
    sha.get(..12).unwrap_or(sha)
}

/// `ON_MASTER`, from where git put the feature's implementation.
pub(crate) fn on_master(feature: &str, revision: &Revision) -> Dimension {
    let id = DimensionId::OnMaster;
    match revision {
        Revision::OnMaster { master, .. } => Dimension::pass(
            id,
            format!(
                "its latest implementation revision {} is on the trunk",
                short(master)
            ),
        ),
        Revision::OnBranch { head, .. } => Dimension::fail(
            id,
            format!(
                "its latest implementation revision {} is on this branch and not on the trunk",
                short(head)
            ),
            "merge the branch into master".into(),
        ),
        Revision::NotFound => Dimension::fail(
            id,
            "no commit touches the paths it is implemented by".into(),
            "commit the implementation the feature names".into(),
        ),
        Revision::NoImplementation => Dimension::fail(
            id,
            "it names no module, command or claim that has an implementation path".into(),
            format!("name what implements it in .ai/repo/features/{feature}.md"),
        ),
        Revision::Unknown { reason } => Dimension::unknown(
            id,
            format!("where its implementation is could not be read: {reason}"),
            "run it in a clone that has git and the trunk (git fetch origin)".into(),
        ),
    }
}

/// `DEPLOYED`: does the commit the public site serves contain the feature's trunk revision?
pub(crate) fn deployed(
    revision: &Revision,
    identity: &IdentityRead,
    containment: Option<Containment>,
) -> Dimension {
    let id = DimensionId::Deployed;
    let served = match identity {
        IdentityRead::Served { commit, .. } => commit,
        IdentityRead::Unreachable { reason } | IdentityRead::Unconfigured { reason } => {
            return Dimension::unknown(
                id,
                format!("the public identity was not read: {reason}"),
                "read it again where the site is reachable (scripts/pages verify --url URL --commit SHA)"
                    .into(),
            )
        }
    };
    let master = match revision {
        Revision::OnMaster { master, .. }
        | Revision::OnBranch {
            master: Some(master),
            ..
        } => master,
        Revision::Unknown { .. } => {
            return Dimension::unknown(
                id,
                "whether any of it is on the trunk is unknown, so what could be deployed is too"
                    .into(),
                "settle on_master first".into(),
            )
        }
        Revision::OnBranch { master: None, .. }
        | Revision::NotFound
        | Revision::NoImplementation => {
            return Dimension::fail(
                id,
                "nothing of it is on the trunk, so nothing of it can be deployed".into(),
                "merge its implementation into master; the pages workflow publishes master".into(),
            )
        }
    };
    match containment {
        Some(Containment::Contains) => Dimension::pass(
            id,
            format!(
                "the public site serves {}, which contains {}",
                short(served),
                short(master)
            ),
        ),
        Some(Containment::DoesNotContain) => Dimension::fail(
            id,
            format!(
                "the public site serves {}, which does not contain {}",
                short(served),
                short(master)
            ),
            "publish master: the pages workflow deploys it, and scripts/pages built says whether GitHub built it"
                .into(),
        ),
        Some(Containment::CommitUnknown) | None => Dimension::unknown(
            id,
            format!(
                "this clone does not have the commit the public site serves ({})",
                short(served)
            ),
            "git fetch origin, then ask again".into(),
        ),
    }
}

/// `PUBLICLY_VERIFIED`: deployed, and the publication verified from outside.
pub(crate) fn publicly_verified(
    deployed: &Dimension,
    verification: &SiteVerification,
) -> Dimension {
    let id = DimensionId::PubliclyVerified;
    match deployed.verdict {
        Verdict::Fail => {
            return Dimension::fail(
                id,
                "it is not deployed, so there is no publication of it to verify".into(),
                "deploy it first".into(),
            )
        }
        Verdict::Unknown => {
            return Dimension::unknown(
                id,
                "whether it is deployed is unknown, so its publication is not verified".into(),
                "settle deployed first".into(),
            )
        }
        Verdict::Pass => {}
    }
    match verification {
        SiteVerification::Verified { detail } => Dimension::pass(id, detail.clone()),
        SiteVerification::Refused { reason } => Dimension::fail(
            id,
            reason.clone(),
            "republish the site from a clean master (scripts/pages built, scripts/pages verify)"
                .into(),
        ),
        SiteVerification::Unmeasured { reason } => Dimension::unknown(
            id,
            reason.clone(),
            "run it where scripts/pages, gh and the network are available".into(),
        ),
    }
}

/// One of the three dimensions phase 1 does not measure.
pub(crate) fn pending(dimension: DimensionId) -> Dimension {
    Dimension::unknown(
        dimension,
        EVIDENCE_PENDING.into(),
        "nothing to do here yet: the dimension is computed once the evidence model is merged"
            .into(),
    )
}

/// The development stage: the highest rung every rung below which passes.
pub(crate) fn stage(revision: &Revision, dimensions: &[Dimension]) -> DevelopmentStage {
    let passes = |d: DimensionId| {
        dimensions
            .iter()
            .any(|x| x.dimension == d && x.verdict == Verdict::Pass)
    };
    match revision {
        Revision::Unknown { .. } => DevelopmentStage::Unknown,
        Revision::NotFound | Revision::NoImplementation => DevelopmentStage::NotImplemented,
        Revision::OnBranch { .. } => DevelopmentStage::ImplementedOnBranch,
        Revision::OnMaster { .. } if !passes(DimensionId::RequiredTestsCurrent) => {
            DevelopmentStage::OnMaster
        }
        Revision::OnMaster { .. } if !passes(DimensionId::Deployed) => DevelopmentStage::Tested,
        Revision::OnMaster { .. } => DevelopmentStage::Deployed,
    }
}

/// The conjunction, and the state it yields. `exists` is computed here and nowhere else.
pub(crate) fn conclude(revision: &Revision, dimensions: &[Dimension]) -> (bool, DeliveryState) {
    let blocking: Vec<DimensionId> = dimensions
        .iter()
        .filter(|d| d.verdict != Verdict::Pass)
        .map(|d| d.dimension)
        .collect();
    // an empty dimension list is not a vacuous truth: nothing was measured
    let exists = !dimensions.is_empty() && blocking.is_empty();
    if exists {
        (true, DeliveryState::Delivered)
    } else {
        (
            false,
            DeliveryState::NotDelivered {
                stage: stage(revision, dimensions),
                blocking,
            },
        )
    }
}

/// Every dimension of one feature, from the facts the two readers gathered.
pub(crate) fn dimensions(
    feature: &str,
    revision: &Revision,
    publication: &Publication,
    containment: Option<Containment>,
) -> Vec<Dimension> {
    let on_master = on_master(feature, revision);
    let deployed = deployed(revision, &publication.identity, containment);
    let verified = publicly_verified(&deployed, &publication.verification);
    vec![
        on_master,
        deployed,
        verified,
        pending(DimensionId::RequiredTestsCurrent),
        pending(DimensionId::TestEvidencePublished),
        pending(DimensionId::UiLinked),
    ]
}

/// One feature to assess: what its file says about it, and the paths that implement it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Subject {
    /// The feature's id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its declared status.
    pub status: String,
    /// The paths [`revision::implementation_paths`] derived for it.
    pub paths: Vec<String>,
}

/// Assess every subject against the invariant, in the repository at `root`, with the
/// publication observed once for all of them. Ordered by feature id whatever order the
/// subjects came in.
pub(crate) fn assess(
    root: &std::path::Path,
    subjects: impl IntoIterator<Item = Subject>,
    publication: Publication,
) -> DeliveryReport {
    let trunk = revision::trunk_ref(root);
    let served = match &publication.identity {
        IdentityRead::Served { commit, .. } => Some(commit.clone()),
        _ => None,
    };
    let by_id: std::collections::BTreeMap<String, Subject> =
        subjects.into_iter().map(|s| (s.id.clone(), s)).collect();
    let features: Vec<FeatureDelivery> = by_id
        .into_values()
        .map(|s| {
            let located =
                revision::locate(root, trunk.as_deref().map_err(String::as_str), &s.paths);
            let master = match &located {
                Revision::OnMaster { master, .. }
                | Revision::OnBranch {
                    master: Some(master),
                    ..
                } => Some(master.clone()),
                _ => None,
            };
            let containment = match (&served, &master) {
                (Some(served), Some(master)) => Some(revision::contains(root, served, master)),
                _ => None,
            };
            let dims = dimensions(&s.id, &located, &publication, containment);
            let (exists, delivery) = conclude(&located, &dims);
            let head = match &located {
                Revision::OnMaster { head, .. } => head.clone(),
                Revision::OnBranch { head, .. } => Some(head.clone()),
                _ => None,
            };
            FeatureDelivery {
                id: s.id,
                title: s.title,
                status: s.status,
                exists,
                delivery,
                dimensions: dims,
                implementation: Implementation {
                    paths: s.paths,
                    head_revision: head,
                    master_revision: master,
                },
            }
        })
        .collect();
    DeliveryReport {
        schema: SCHEMA.into(),
        trunk: trunk.ok(),
        publication,
        tallies: tally(&features),
        features,
    }
}

/// The tallies of a set of features.
pub(crate) fn tally(features: &[FeatureDelivery]) -> Tallies {
    let count = |d: DimensionId| {
        features
            .iter()
            .filter(|f| {
                f.dimensions
                    .iter()
                    .any(|x| x.dimension == d && x.verdict == Verdict::Pass)
            })
            .count()
    };
    Tallies {
        features: features.len(),
        on_master: count(DimensionId::OnMaster),
        deployed: count(DimensionId::Deployed),
        publicly_verified: count(DimensionId::PubliclyVerified),
        exists: features.iter().filter(|f| f.exists).count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "1111111111111111111111111111111111111111";
    const MASTER: &str = "2222222222222222222222222222222222222222";
    const SERVED: &str = "3333333333333333333333333333333333333333";

    fn on(master: &str) -> Revision {
        Revision::OnMaster {
            head: Some(master.into()),
            master: master.into(),
        }
    }

    fn served(dirty: Option<bool>) -> IdentityRead {
        IdentityRead::Served {
            commit: SERVED.into(),
            dirty,
        }
    }

    fn publication(identity: IdentityRead, verification: SiteVerification) -> Publication {
        Publication {
            url: Some("https://example.invalid".into()),
            identity_path: Some("build.json".into()),
            identity,
            verification,
        }
    }

    fn verified() -> SiteVerification {
        SiteVerification::Verified {
            detail: "verified".into(),
        }
    }

    fn all_pass() -> Vec<Dimension> {
        DimensionId::ALL
            .iter()
            .map(|d| Dimension::pass(*d, "observed".into()))
            .collect()
    }

    #[test]
    fn on_master_passes_only_for_a_revision_on_the_trunk() {
        assert_eq!(on_master("f", &on(MASTER)).verdict, Verdict::Pass);
        let branch = on_master(
            "f",
            &Revision::OnBranch {
                head: HEAD.into(),
                master: Some(MASTER.into()),
            },
        );
        assert_eq!(branch.verdict, Verdict::Fail);
        assert!(branch.reason.contains("111111111111"), "{}", branch.reason);
        assert_eq!(on_master("f", &Revision::NotFound).verdict, Verdict::Fail);
        let none = on_master("f", &Revision::NoImplementation);
        assert_eq!(none.verdict, Verdict::Fail);
        assert!(none.remediation.unwrap().contains("features/f.md"));
        let unknown = on_master(
            "f",
            &Revision::Unknown {
                reason: "no trunk".into(),
            },
        );
        assert_eq!(unknown.verdict, Verdict::Unknown);
        assert!(unknown.reason.contains("no trunk"));
    }

    #[test]
    fn deployed_is_an_ancestor_check_against_the_served_identity() {
        let yes = deployed(
            &on(MASTER),
            &served(Some(false)),
            Some(Containment::Contains),
        );
        assert_eq!(yes.verdict, Verdict::Pass);
        assert!(yes.remediation.is_none());
        let older = deployed(
            &on(MASTER),
            &served(Some(false)),
            Some(Containment::DoesNotContain),
        );
        assert_eq!(older.verdict, Verdict::Fail);
        assert!(older.reason.contains("does not contain"));
        let absent = deployed(
            &on(MASTER),
            &served(Some(false)),
            Some(Containment::CommitUnknown),
        );
        assert_eq!(absent.verdict, Verdict::Unknown);
        assert_eq!(
            deployed(&on(MASTER), &served(None), None).verdict,
            Verdict::Unknown
        );
    }

    #[test]
    fn an_unreadable_identity_is_unknown_and_never_deployed() {
        for identity in [
            IdentityRead::Unreachable {
                reason: "no route".into(),
            },
            IdentityRead::Unconfigured {
                reason: "no site".into(),
            },
        ] {
            let d = deployed(&on(MASTER), &identity, Some(Containment::Contains));
            assert_eq!(d.verdict, Verdict::Unknown, "{identity:?}");
        }
    }

    #[test]
    fn what_is_not_on_the_trunk_cannot_be_deployed() {
        let identity = served(Some(false));
        for revision in [
            Revision::NotFound,
            Revision::NoImplementation,
            Revision::OnBranch {
                head: HEAD.into(),
                master: None,
            },
        ] {
            let d = deployed(&revision, &identity, Some(Containment::Contains));
            assert_eq!(d.verdict, Verdict::Fail, "{revision:?}");
        }
        let unknown = Revision::Unknown {
            reason: "git".into(),
        };
        assert_eq!(
            deployed(&unknown, &identity, None).verdict,
            Verdict::Unknown
        );
        // a branch ahead of the trunk still has the trunk's revision to deploy
        let ahead = Revision::OnBranch {
            head: HEAD.into(),
            master: Some(MASTER.into()),
        };
        assert_eq!(
            deployed(&ahead, &identity, Some(Containment::Contains)).verdict,
            Verdict::Pass
        );
    }

    #[test]
    fn publicly_verified_follows_deployed_then_the_site_verification() {
        let pass = Dimension::pass(DimensionId::Deployed, "x".into());
        let fail = Dimension::fail(DimensionId::Deployed, "x".into(), "y".into());
        let unknown = Dimension::unknown(DimensionId::Deployed, "x".into(), "y".into());
        assert_eq!(publicly_verified(&fail, &verified()).verdict, Verdict::Fail);
        assert_eq!(
            publicly_verified(&unknown, &verified()).verdict,
            Verdict::Unknown
        );
        assert_eq!(publicly_verified(&pass, &verified()).verdict, Verdict::Pass);
        let refused = SiteVerification::Refused {
            reason: "dirty".into(),
        };
        assert_eq!(publicly_verified(&pass, &refused).verdict, Verdict::Fail);
        let unmeasured = SiteVerification::Unmeasured {
            reason: "no gh".into(),
        };
        assert_eq!(
            publicly_verified(&pass, &unmeasured).verdict,
            Verdict::Unknown
        );
    }

    #[test]
    fn exists_is_false_when_any_dimension_does_not_pass() {
        let (exists, state) = conclude(&on(MASTER), &all_pass());
        assert!(exists);
        assert_eq!(state, DeliveryState::Delivered);
        for (i, verdict) in [
            (0, Verdict::Fail),
            (3, Verdict::Unknown),
            (5, Verdict::Unknown),
        ] {
            let mut dims = all_pass();
            dims[i].verdict = verdict;
            let (exists, state) = conclude(&on(MASTER), &dims);
            assert!(!exists);
            match state {
                DeliveryState::NotDelivered { blocking, .. } => {
                    assert_eq!(blocking, vec![DimensionId::ALL[i]])
                }
                DeliveryState::Delivered => panic!("a non-pass was delivered"),
            }
        }
        // nothing measured is not a vacuous yes
        assert!(!conclude(&on(MASTER), &[]).0);
    }

    #[test]
    fn phase_one_never_lets_a_feature_exist() {
        let p = publication(served(Some(false)), verified());
        let dims = dimensions("f", &on(MASTER), &p, Some(Containment::Contains));
        assert_eq!(dims.len(), DimensionId::ALL.len());
        assert_eq!(
            dims.iter().map(|d| d.dimension).collect::<Vec<_>>(),
            DimensionId::ALL.to_vec()
        );
        assert_eq!(dims[0].verdict, Verdict::Pass);
        assert_eq!(dims[1].verdict, Verdict::Pass);
        assert_eq!(dims[2].verdict, Verdict::Pass);
        for d in &dims[3..] {
            assert_eq!(d.verdict, Verdict::Unknown);
            assert_eq!(d.reason, EVIDENCE_PENDING);
        }
        let (exists, state) = conclude(&on(MASTER), &dims);
        assert!(!exists);
        assert_eq!(
            state,
            DeliveryState::NotDelivered {
                stage: DevelopmentStage::OnMaster,
                blocking: DimensionId::ALL[3..].to_vec(),
            }
        );
    }

    #[test]
    fn the_stage_is_a_ladder() {
        let rev = |r: Revision| stage(&r, &all_pass());
        assert_eq!(
            rev(Revision::Unknown { reason: "x".into() }),
            DevelopmentStage::Unknown
        );
        assert_eq!(rev(Revision::NotFound), DevelopmentStage::NotImplemented);
        assert_eq!(
            rev(Revision::NoImplementation),
            DevelopmentStage::NotImplemented
        );
        assert_eq!(
            rev(Revision::OnBranch {
                head: HEAD.into(),
                master: None
            }),
            DevelopmentStage::ImplementedOnBranch
        );
        assert_eq!(rev(on(MASTER)), DevelopmentStage::Deployed);
        let mut untested = all_pass();
        untested[3].verdict = Verdict::Unknown;
        assert_eq!(stage(&on(MASTER), &untested), DevelopmentStage::OnMaster);
        let mut undeployed = all_pass();
        undeployed[1].verdict = Verdict::Fail;
        assert_eq!(stage(&on(MASTER), &undeployed), DevelopmentStage::Tested);
    }

    /// The behavioural core in one repository: merged and served is deployed, an older
    /// served identity is not, an unreachable one is unknown, and nothing exists.
    #[test]
    fn assess_measures_every_feature_against_one_publication() {
        let r = revision::tests::Repo::new();
        let old = r.git(&["rev-parse", "HEAD"]);
        let merged = r.commit("lib/merged.sh", "m");
        r.publish_master();
        r.git(&["checkout", "-q", "-b", "feature/x"]);
        r.commit("lib/branch.sh", "b");
        let subject = |id: &str, paths: &[&str]| Subject {
            id: id.into(),
            title: id.to_uppercase(),
            status: "stable".into(),
            paths: paths.iter().map(|p| p.to_string()).collect(),
        };
        let subjects = || {
            vec![
                subject("zeta", &["lib/branch.sh"]),
                subject("alpha", &["lib/merged.sh"]),
                subject("none", &[]),
            ]
        };
        let at = |commit: &str| {
            publication(
                IdentityRead::Served {
                    commit: commit.into(),
                    dirty: Some(false),
                },
                verified(),
            )
        };

        let report = assess(r.root(), subjects(), at(&merged));
        assert_eq!(report.schema, SCHEMA);
        assert_eq!(report.trunk.as_deref(), Some("origin/master"));
        let ids: Vec<&str> = report.features.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["alpha", "none", "zeta"], "ordered by id");
        let alpha = &report.features[0];
        assert_eq!(alpha.dimensions[0].verdict, Verdict::Pass);
        assert_eq!(alpha.dimensions[1].verdict, Verdict::Pass);
        assert_eq!(
            alpha.implementation.master_revision.as_deref(),
            Some(merged.as_str())
        );
        assert_eq!(
            alpha.implementation.head_revision.as_deref(),
            Some(merged.as_str())
        );
        assert!(!alpha.exists);
        let zeta = &report.features[2];
        assert!(matches!(
            zeta.delivery,
            DeliveryState::NotDelivered {
                stage: DevelopmentStage::ImplementedOnBranch,
                ..
            }
        ));
        assert!(zeta.implementation.head_revision.is_some());
        assert_eq!(report.tallies.on_master, 1);
        assert_eq!(report.tallies.deployed, 1);
        assert_eq!(report.tallies.exists, 0);

        // the same feature against an identity older than its revision is not deployed
        let older = assess(r.root(), subjects(), at(&old));
        assert_eq!(older.features[0].dimensions[1].verdict, Verdict::Fail);
        assert_eq!(older.features[0].dimensions[2].verdict, Verdict::Fail);

        // an identity that could not be read is unknown, and nothing exists
        let unreachable = assess(
            r.root(),
            subjects(),
            publication(
                IdentityRead::Unreachable {
                    reason: "offline".into(),
                },
                SiteVerification::Unmeasured {
                    reason: "offline".into(),
                },
            ),
        );
        assert_eq!(
            unreachable.features[0].dimensions[1].verdict,
            Verdict::Unknown
        );
        assert_eq!(unreachable.tallies.exists, 0);
    }

    #[test]
    fn tallies_count_passes_and_existence() {
        let feature = |dims: Vec<Dimension>| {
            let (exists, delivery) = conclude(&on(MASTER), &dims);
            FeatureDelivery {
                id: "f".into(),
                title: "F".into(),
                status: "stable".into(),
                exists,
                delivery,
                dimensions: dims,
                implementation: Implementation::default(),
            }
        };
        let p = publication(served(Some(false)), verified());
        let phase1 = dimensions("f", &on(MASTER), &p, Some(Containment::Contains));
        let t = tally(&[feature(all_pass()), feature(phase1)]);
        assert_eq!(
            t,
            Tallies {
                features: 2,
                on_master: 2,
                deployed: 2,
                publicly_verified: 2,
                exists: 1,
            }
        );
        // the state serialises as a tagged value, and a delivered feature carries no stage
        let v = serde_json::to_value(DeliveryState::Delivered).unwrap();
        assert_eq!(v, serde_json::json!({ "state": "delivered" }));
    }
}
