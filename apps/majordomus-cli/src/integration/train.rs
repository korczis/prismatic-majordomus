//! Trains of pull requests (ADR 0138): the open pull requests stacked on each other, as one
//! typed object rather than a story told in session messages.
//!
//! A *train* is a connected group of open pull requests joined by confirmed dependencies —
//! a base branch that is another open pull request's head, or a `Stacked on #N` /
//! `Depends on #N` line in the body ([`super::classify::confirmed_dependencies`]). Each of
//! its *links* carries what the executor already decided about it (disposition, reasons,
//! required checks, relation to master) and two things nothing showed before: the link's
//! turn — the order in which the train lands, lower links first — and the prepared head the
//! executor holds for it under `refs/prep/<n>`, with whether that head is the one the forge
//! serves (pushed) or not yet (pending).
//!
//! Nothing here decides anything the queue did not. [`trains`] is a pure function of the
//! queue ([`super::queue_of`]), the facts the recorded observation carries about each pull
//! request ([`LinkFacts`]) and the prepared refs; [`trains_of`] reads those three from the
//! checkout — the observation and the refs as they are, with no network and no write.
//!
//! ```text
//! use majordomus_cli::integration::train::parse_prep_refs;
//! let refs = parse_prep_refs("refs/prep/807 0123abcd\nrefs/prep/not-a-number 99\n");
//! assert_eq!(refs.get(&807).map(String::as_str), Some("0123abcd"));
//! assert_eq!(refs.len(), 1, "a ref that names no pull request is not a prepared head");
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::classify::marked_numbers;
use super::model::{
    CheckRunState, DependencyCertainty, DependencyState, IntegrationLane, PullRequestAssessment,
    PullRequestDisposition, ReasonCode, RelationToMaster, RequiredCheckState,
};
use super::IntegrationQueue;

/// The namespace the executor keeps the prepared next head of each link in:
/// `refs/prep/<number>`, local to the clone that prepared it, pushed at the link's turn.
pub const PREP_NAMESPACE: &str = "refs/prep/";

/// The body marker that makes a declared dependency a stack rather than an ordering.
const STACK_MARKERS: &[&str] = &["stacked on"];

/// Every train of the observed queue, and why there is none when there is none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TrainsView {
    /// Whether a forge observation is recorded in this checkout.
    pub observed: bool,
    /// Why there are no trains to show, when nothing is observed.
    pub reason: Option<String>,
    /// `owner/name`, when observed.
    pub repository: Option<String>,
    /// The integration base.
    pub base: Option<String>,
    /// The master commit every link's relation was decided against.
    pub master_sha: Option<String>,
    /// When the forge was observed.
    pub observed_at: Option<String>,
    /// Every train, the one whose next link ranks highest in the queue first.
    pub trains: Vec<Train>,
    /// Prepared heads under `refs/prep/` whose pull request is no link of any train: open
    /// and stacked on nothing, or not open at all (landed, closed).
    pub unattached_prep: Vec<UnattachedPrep>,
    /// Counts over every train.
    pub tallies: TrainTallies,
    /// What makes this answer less than a full one: the queue's own diagnostics (a stale
    /// observation first), and a dependency cycle when one is found.
    pub diagnostics: Vec<String>,
}

impl TrainsView {
    /// The answer for a checkout with no usable observation.
    ///
    /// ```text
    /// use majordomus_cli::integration::train::TrainsView;
    /// let v = TrainsView::unobserved("no forge observation".into());
    /// assert!(!v.observed);
    /// assert!(v.trains.is_empty());
    /// ```
    pub fn unobserved(reason: String) -> Self {
        TrainsView {
            observed: false,
            reason: Some(reason),
            repository: None,
            base: None,
            master_sha: None,
            observed_at: None,
            trains: Vec::new(),
            unattached_prep: Vec::new(),
            tallies: TrainTallies::default(),
            diagnostics: Vec::new(),
        }
    }
}

/// Counts over every train.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TrainTallies {
    /// Trains.
    pub trains: usize,
    /// Open links across every train.
    pub links: usize,
    /// Links whose prepared head is not the head the forge serves.
    pub pending_prep: usize,
    /// Links by disposition word.
    pub by_disposition: BTreeMap<String, usize>,
}

/// One train: open pull requests joined by confirmed dependencies, in the order they land.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Train {
    /// `train-<n>`, after the first root in turn order: stable while that link is open.
    pub id: String,
    /// The links that land first: no open link of the train is below them.
    pub roots: Vec<u64>,
    /// The links at the top: no link of the train is stacked on them.
    pub tops: Vec<u64>,
    /// The first link, in turn order, whose every dependency has landed: the train's next
    /// merge as far as order is concerned (its disposition says whether it can merge now).
    pub next: Option<u64>,
    /// Every open link, in turn order — [`TrainLink::position`] 1 first.
    pub links: Vec<TrainLink>,
    /// Every confirmed dependency of a link, upper link first.
    pub edges: Vec<TrainEdge>,
    /// The pull requests below the train that are no longer open — landed, closed without a
    /// merge, or unread — which a link still declares.
    pub beneath: Vec<BeneathLink>,
    /// Links by disposition word.
    pub by_disposition: BTreeMap<String, usize>,
}

/// One open pull request as a link of a train.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TrainLink {
    /// The pull request number.
    pub number: u64,
    /// Its title.
    pub title: String,
    /// Its author's login.
    pub author: String,
    /// Its head branch.
    pub head_ref: String,
    /// The branch it targets: the integration base, or the head of the link below.
    pub base_ref: String,
    /// The head commit it was assessed at, as the forge served it.
    pub head_sha: String,
    /// The pull request on the forge, when the repository is known.
    pub url: Option<String>,
    /// Its checks on the forge, when the repository is known.
    pub checks_url: Option<String>,
    /// The executor's classification.
    pub disposition: PullRequestDisposition,
    /// The lane that disposition belongs to.
    pub lane: IntegrationLane,
    /// The machine-readable reasons, as every surface spells them (`stacked_on:#807`).
    pub reasons: Vec<String>,
    /// What happens next, in words, when anything does.
    pub next_action: Option<String>,
    /// The required checks on its head.
    pub required_checks: RequiredCheckState,
    /// Every check the forge reported on its head, required or not.
    pub checks: Vec<LinkCheck>,
    /// What its head is to master, decided by git.
    pub relation: RelationToMaster,
    /// That relation's word (`behind`, `up_to_date`, `carries_master`, ...).
    pub relation_kind: String,
    /// How many master commits its head lacks, when the relation counts them.
    pub behind: Option<u64>,
    /// Whether its head carries master by a merge of its own (ADR 0129) — read from the
    /// relation's word, so an executable that does not know that relation says `false`.
    pub carries_master: bool,
    /// The head the executor prepared for it, when `refs/prep/<n>` exists.
    pub prep: Option<PreparedHead>,
    /// Its turn in the train, 1-based: every link below it comes earlier.
    pub position: usize,
    /// How many links are below it on its longest path to master; a root is 0.
    pub depth: usize,
    /// Every dependency it declares has landed: it is the link the order allows next.
    pub at_turn: bool,
    /// The pull requests it stands on, open links and those beneath alike.
    pub on: Vec<u64>,
    /// The links stacked on it.
    pub carries: Vec<u64>,
    /// Its rank in the queue, 1-based.
    pub rank: usize,
    /// Caught in a cycle of confirmed dependencies: no link of the cycle can land first.
    pub in_cycle: bool,
}

/// One check run on a link's head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LinkCheck {
    /// The check's name.
    pub name: String,
    /// Where it stands.
    pub state: CheckRunState,
}

/// The head the executor prepared for a link under `refs/prep/<n>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PreparedHead {
    /// The ref.
    pub reference: String,
    /// The commit it names.
    pub sha: String,
    /// Whether that commit is the head the forge serves.
    pub state: PrepState,
}

/// Whether a prepared head has reached the forge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrepState {
    /// The prepared head is the pull request's head: it was pushed.
    Pushed,
    /// The prepared head is another commit: it waits for its turn to be pushed.
    Pending,
}

impl PrepState {
    /// The word as serialised.
    ///
    /// ```text
    /// use majordomus_cli::integration::train::PrepState;
    /// assert_eq!(PrepState::Pending.as_str(), "pending");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            PrepState::Pushed => "pushed",
            PrepState::Pending => "pending",
        }
    }
}

/// A confirmed dependency between two pull requests of a train.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TrainEdge {
    /// The upper link: the one that depends.
    pub from: u64,
    /// The pull request it stands on.
    pub to: u64,
    /// How the dependency is declared.
    pub kind: TrainEdgeKind,
    /// Whether it is satisfied: the pull request below landed.
    pub satisfied: bool,
}

/// How a dependency between two links is declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrainEdgeKind {
    /// Its base branch is the other's head, or its body says `Stacked on #N`.
    StackedOn,
    /// Its body says `Depends on #N`, `Requires #N` or `Land after #N`.
    DependsOn,
}

impl TrainEdgeKind {
    /// The relationship's label, as a graph draws it.
    ///
    /// ```text
    /// use majordomus_cli::integration::train::TrainEdgeKind;
    /// assert_eq!(TrainEdgeKind::StackedOn.label(), "STACKED_ON");
    /// ```
    pub fn label(self) -> &'static str {
        match self {
            TrainEdgeKind::StackedOn => "STACKED_ON",
            TrainEdgeKind::DependsOn => "DEPENDS_ON",
        }
    }
}

/// A pull request below a train that is not open any more.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BeneathLink {
    /// Its number.
    pub number: u64,
    /// What the forge says became of it.
    pub state: DependencyState,
    /// The pull request on the forge, when the repository is known.
    pub url: Option<String>,
}

/// A prepared head whose pull request is no link of any train.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UnattachedPrep {
    /// The pull request the ref is named for.
    pub number: u64,
    /// The commit it names.
    pub sha: String,
    /// Whether that pull request is open.
    pub open: bool,
}

/// What the recorded observation says about one pull request beyond its assessment: the
/// body its stacks are declared in and the checks on its head.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkFacts {
    /// Bodies by number.
    pub bodies: BTreeMap<u64, String>,
    /// Checks by number.
    pub checks: BTreeMap<u64, Vec<LinkCheck>>,
}

/// The prepared heads in `git for-each-ref --format='%(refname) %(objectname)' refs/prep/`
/// output, by pull-request number. A line whose ref is not `refs/prep/<number>` is not one.
pub fn parse_prep_refs(text: &str) -> BTreeMap<u64, String> {
    text.lines()
        .filter_map(|line| line.split_once(' '))
        .filter_map(|(name, sha)| {
            let n = name.strip_prefix(PREP_NAMESPACE)?.parse::<u64>().ok()?;
            let sha = sha.trim();
            (!sha.is_empty()).then(|| (n, sha.to_string()))
        })
        .collect()
}

/// The prepared heads of the checkout at `root`. A read of local refs: nothing is fetched
/// and nothing is moved. A git that cannot answer leaves the map empty.
pub fn prep_refs(root: &Path) -> BTreeMap<u64, String> {
    std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            PREP_NAMESPACE,
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| parse_prep_refs(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// [`trains`] over the checkout at `root`: the queue of its recorded observation, the
/// observation's bodies and checks, and its `refs/prep/` refs. Offline and read-only.
pub fn trains_of(root: &Path) -> TrainsView {
    let queue = match super::queue_of(root) {
        Ok(q) => q,
        Err(reason) => return TrainsView::unobserved(reason),
    };
    let facts = match super::load_observation(root) {
        Ok(Some(obs)) => LinkFacts {
            bodies: obs
                .pull_requests
                .iter()
                .map(|p| (p.number, p.body.clone()))
                .collect(),
            checks: obs
                .pull_requests
                .iter()
                .map(|p| {
                    let checks = p
                        .checks
                        .iter()
                        .map(|c| LinkCheck {
                            name: c.name.clone(),
                            state: c.state,
                        })
                        .collect();
                    (p.number, checks)
                })
                .collect(),
        },
        _ => LinkFacts::default(),
    };
    trains(&queue, &facts, &prep_refs(root))
}

/// The pull request's page on the forge, for an `owner/name` repository.
///
/// ```text
/// use majordomus_cli::integration::train::pull_request_url;
/// assert_eq!(pull_request_url("o/r", 7).as_deref(), Some("https://github.com/o/r/pull/7"));
/// assert_eq!(pull_request_url("", 7), None);
/// ```
pub fn pull_request_url(repository: &str, number: u64) -> Option<String> {
    let (owner, name) = repository.split_once('/')?;
    (!owner.is_empty() && !name.is_empty())
        .then(|| format!("https://github.com/{repository}/pull/{number}"))
}

/// The word a relation serialises under.
fn relation_word(relation: &RelationToMaster) -> String {
    serde_json::to_value(relation)
        .ok()
        .and_then(|v| v.get("kind").and_then(|k| k.as_str().map(str::to_string)))
        .unwrap_or_else(|| "unknown".into())
}

/// The confirmed dependencies of one assessment, each with how it is declared, in the order
/// the assessment lists them, never itself.
fn confirmed(a: &PullRequestAssessment, body: &str) -> Vec<(u64, TrainEdgeKind, DependencyState)> {
    let stacked_base: BTreeSet<u64> = a
        .reasons
        .iter()
        .filter_map(|r| match r {
            ReasonCode::StackedOn { number } => Some(*number),
            _ => None,
        })
        .collect();
    let stacked_body: BTreeSet<u64> = marked_numbers(body, STACK_MARKERS).into_iter().collect();
    let mut seen = BTreeSet::new();
    a.dependencies
        .iter()
        .filter(|d| d.certainty == DependencyCertainty::Confirmed && d.number != a.number)
        .filter(|d| seen.insert(d.number))
        .map(|d| {
            let kind = if stacked_base.contains(&d.number) || stacked_body.contains(&d.number) {
                TrainEdgeKind::StackedOn
            } else {
                TrainEdgeKind::DependsOn
            };
            (d.number, kind, d.state)
        })
        .collect()
}

/// Every train of `queue`.
///
/// A train is a connected component of open pull requests under their confirmed
/// dependencies on each other, together with any open pull request that declares a
/// dependency at all (a link whose lower links already landed is still the rest of its
/// train). Links are ordered by turn: a topological order, lower links first, ties broken by
/// queue rank; links caught in a cycle come last, marked. Inferred dependencies (a head that
/// merely contains another) are evidence and join nothing.
///
/// Pure: it reads its arguments and nothing else.
pub fn trains(
    queue: &IntegrationQueue,
    facts: &LinkFacts,
    prep: &BTreeMap<u64, String>,
) -> TrainsView {
    let empty = String::new();
    let rank: BTreeMap<u64, usize> = queue
        .assessments
        .iter()
        .enumerate()
        .map(|(i, a)| (a.number, i + 1))
        .collect();
    let by_number: BTreeMap<u64, &PullRequestAssessment> =
        queue.assessments.iter().map(|a| (a.number, a)).collect();

    // every confirmed dependency, by the upper link
    let deps: BTreeMap<u64, Vec<(u64, TrainEdgeKind, DependencyState)>> = queue
        .assessments
        .iter()
        .map(|a| {
            let body = facts.bodies.get(&a.number).unwrap_or(&empty);
            (a.number, confirmed(a, body))
        })
        .collect();

    // the members: open pull requests that declare a dependency or are declared one by an
    // open pull request
    let mut members: BTreeSet<u64> = BTreeSet::new();
    for (n, ds) in &deps {
        if ds.is_empty() {
            continue;
        }
        members.insert(*n);
        for (d, _, _) in ds {
            if rank.contains_key(d) {
                members.insert(*d);
            }
        }
    }

    // connected components over open-to-open edges (union-find by smallest number)
    let mut parent: BTreeMap<u64, u64> = members.iter().map(|n| (*n, *n)).collect();
    fn find(parent: &mut BTreeMap<u64, u64>, n: u64) -> u64 {
        let p = parent[&n];
        if p == n {
            return n;
        }
        let root = find(parent, p);
        parent.insert(n, root);
        root
    }
    for (n, ds) in &deps {
        for (d, _, _) in ds {
            if members.contains(n) && members.contains(d) {
                let (a, b) = (find(&mut parent, *n), find(&mut parent, *d));
                if a != b {
                    parent.insert(a.max(b), a.min(b));
                }
            }
        }
    }
    let mut components: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for n in &members {
        let r = find(&mut parent, *n);
        components.entry(r).or_default().push(*n);
    }

    let mut diagnostics = queue.diagnostics.clone();
    let mut trains: Vec<Train> = components
        .into_values()
        .map(|nodes| {
            let train = one_train(queue, &nodes, &deps, &rank, &by_number, facts, prep);
            let caught: Vec<String> = train
                .links
                .iter()
                .filter(|l| l.in_cycle)
                .map(|l| format!("#{}", l.number))
                .collect();
            if !caught.is_empty() {
                diagnostics.push(format!(
                    "{} is caught in a dependency cycle ({}): no link of it can land first",
                    train.id,
                    caught.join(", ")
                ));
            }
            train
        })
        .collect();
    // the train whose best link ranks highest first, then by id for a stable answer
    trains.sort_by_key(|t| {
        (
            t.links.iter().map(|l| l.rank).min().unwrap_or(usize::MAX),
            t.id.clone(),
        )
    });

    let linked: BTreeSet<u64> = trains
        .iter()
        .flat_map(|t| t.links.iter().map(|l| l.number))
        .collect();
    let unattached_prep = prep
        .iter()
        .filter(|(n, _)| !linked.contains(n))
        .map(|(n, sha)| UnattachedPrep {
            number: *n,
            sha: sha.clone(),
            open: rank.contains_key(n),
        })
        .collect();

    let mut tallies = TrainTallies {
        trains: trains.len(),
        ..TrainTallies::default()
    };
    for l in trains.iter().flat_map(|t| t.links.iter()) {
        tallies.links += 1;
        if l.prep
            .as_ref()
            .is_some_and(|p| p.state == PrepState::Pending)
        {
            tallies.pending_prep += 1;
        }
        *tallies
            .by_disposition
            .entry(l.disposition.as_str().to_string())
            .or_default() += 1;
    }

    TrainsView {
        observed: true,
        reason: None,
        repository: Some(queue.repository.clone()),
        base: Some(queue.base.clone()),
        master_sha: Some(queue.master_sha.clone()),
        observed_at: Some(queue.observed_at.clone()),
        trains,
        unattached_prep,
        tallies,
        diagnostics,
    }
}

/// One component as a train: its turn order, depths, roots, tops and edges.
fn one_train(
    queue: &IntegrationQueue,
    nodes: &[u64],
    deps: &BTreeMap<u64, Vec<(u64, TrainEdgeKind, DependencyState)>>,
    rank: &BTreeMap<u64, usize>,
    by_number: &BTreeMap<u64, &PullRequestAssessment>,
    facts: &LinkFacts,
    prep: &BTreeMap<u64, String>,
) -> Train {
    let inside: BTreeSet<u64> = nodes.iter().copied().collect();
    let below = |n: u64| -> Vec<u64> {
        deps.get(&n)
            .map(|ds| {
                ds.iter()
                    .map(|(d, _, _)| *d)
                    .filter(|d| inside.contains(d))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut carries: BTreeMap<u64, Vec<u64>> = nodes.iter().map(|n| (*n, Vec::new())).collect();
    for n in nodes {
        for d in below(*n) {
            carries.entry(d).or_default().push(*n);
        }
    }

    // Kahn's order, lower links first, the best-ranked of the free ones each time
    let mut waiting: BTreeMap<u64, usize> = nodes.iter().map(|n| (*n, below(*n).len())).collect();
    let mut order: Vec<u64> = Vec::new();
    let mut depth: BTreeMap<u64, usize> = BTreeMap::new();
    loop {
        let free = waiting
            .iter()
            .filter(|(_, w)| **w == 0)
            .map(|(n, _)| *n)
            .min_by_key(|n| (rank.get(n).copied().unwrap_or(usize::MAX), *n));
        let Some(n) = free else { break };
        waiting.remove(&n);
        let d = below(n)
            .iter()
            .filter_map(|b| depth.get(b))
            .map(|d| d + 1)
            .max()
            .unwrap_or(0);
        depth.insert(n, d);
        order.push(n);
        for up in carries.get(&n).cloned().unwrap_or_default() {
            if let Some(w) = waiting.get_mut(&up) {
                *w = w.saturating_sub(1);
            }
        }
    }
    // whatever is left waits on itself: a cycle, appended in rank order
    let mut cycle: Vec<u64> = waiting.keys().copied().collect();
    cycle.sort_by_key(|n| (rank.get(n).copied().unwrap_or(usize::MAX), *n));
    let in_cycle: BTreeSet<u64> = cycle.iter().copied().collect();
    order.extend(cycle);

    let mut links = Vec::new();
    let mut edges = Vec::new();
    let mut beneath: BTreeMap<u64, DependencyState> = BTreeMap::new();
    for (i, n) in order.iter().enumerate() {
        let Some(a) = by_number.get(n) else { continue };
        let ds = deps.get(n).cloned().unwrap_or_default();
        for (d, kind, state) in &ds {
            let open = rank.contains_key(d);
            if !open {
                beneath.insert(*d, *state);
            }
            edges.push(TrainEdge {
                from: *n,
                to: *d,
                kind: *kind,
                satisfied: !open && *state == DependencyState::Merged,
            });
        }
        let at_turn = !in_cycle.contains(n)
            && ds
                .iter()
                .all(|(d, _, state)| !rank.contains_key(d) && *state == DependencyState::Merged);
        let relation_kind = relation_word(&a.relation);
        let behind = match &a.relation {
            RelationToMaster::Behind { behind, .. } => Some(*behind),
            RelationToMaster::UpToDate { .. } | RelationToMaster::Contained => Some(0),
            _ => None,
        };
        let head_sha = a.evaluated_against.head_sha.clone();
        let prepared = prep.get(n).map(|sha| PreparedHead {
            reference: format!("{PREP_NAMESPACE}{n}"),
            sha: sha.clone(),
            state: if *sha == head_sha {
                PrepState::Pushed
            } else {
                PrepState::Pending
            },
        });
        let url = pull_request_url(&queue.repository, *n);
        links.push(TrainLink {
            number: *n,
            title: a.title.clone(),
            author: a.author.clone(),
            head_ref: a.head_ref.clone(),
            base_ref: a.base_ref.clone(),
            head_sha,
            checks_url: url.as_ref().map(|u| format!("{u}/checks")),
            url,
            disposition: a.disposition,
            lane: a.lane,
            reasons: a.reasons.iter().map(ToString::to_string).collect(),
            next_action: a.next_action.clone(),
            required_checks: a.required_checks,
            checks: facts.checks.get(n).cloned().unwrap_or_default(),
            carries_master: relation_kind == "carries_master",
            relation_kind,
            behind,
            relation: a.relation.clone(),
            prep: prepared,
            position: i + 1,
            depth: depth.get(n).copied().unwrap_or(0),
            at_turn,
            on: ds.iter().map(|(d, _, _)| *d).collect(),
            carries: carries.get(n).cloned().unwrap_or_default(),
            rank: rank.get(n).copied().unwrap_or(0),
            in_cycle: in_cycle.contains(n),
        });
    }

    let roots: Vec<u64> = links
        .iter()
        .filter(|l| !l.on.iter().any(|d| inside.contains(d)))
        .map(|l| l.number)
        .collect();
    let tops: Vec<u64> = links
        .iter()
        .filter(|l| l.carries.is_empty())
        .map(|l| l.number)
        .collect();
    let next = links.iter().find(|l| l.at_turn).map(|l| l.number);
    let mut by_disposition = BTreeMap::new();
    for l in &links {
        *by_disposition
            .entry(l.disposition.as_str().to_string())
            .or_insert(0) += 1;
    }
    let id = format!(
        "train-{}",
        roots.first().or(order.first()).copied().unwrap_or_default()
    );
    Train {
        id,
        roots,
        tops,
        next,
        links,
        edges,
        beneath: beneath
            .into_iter()
            .map(|(number, state)| BeneathLink {
                number,
                state,
                url: pull_request_url(&queue.repository, number),
            })
            .collect(),
        by_disposition,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::model::{
        EvaluatedAgainst, IntegrationRisk, PullRequestDependency, PullRequestReview,
    };
    use crate::integration::IntegrationPolicy;

    fn train<'a>(v: &'a TrainsView, id: &str) -> Option<&'a Train> {
        v.trains.iter().find(|t| t.id == id)
    }

    fn dep(number: u64, state: DependencyState) -> PullRequestDependency {
        PullRequestDependency {
            number,
            certainty: DependencyCertainty::Confirmed,
            satisfied: state == DependencyState::Merged,
            state,
        }
    }

    fn pr(number: u64, disposition: PullRequestDisposition) -> PullRequestAssessment {
        PullRequestAssessment {
            number,
            title: format!("change {number}"),
            author: "someone".into(),
            head_ref: format!("feature/{number}"),
            base_ref: "master".into(),
            evaluated_against: EvaluatedAgainst {
                master_sha: "m".into(),
                head_sha: format!("h{number}"),
                observed_at: "2026-10-11T00:00:00Z".into(),
            },
            disposition,
            superseded_by: None,
            issue: None,
            milestone: None,
            lane: disposition.lane(),
            reasons: Vec::new(),
            gates: Vec::new(),
            next_action: None,
            required_checks: RequiredCheckState::Passed,
            review: PullRequestReview::NotRequired,
            relation: RelationToMaster::Behind {
                behind: 3,
                authored: Vec::new(),
            },
            dependencies: Vec::new(),
            authored_paths: Vec::new(),
            overlaps: Vec::new(),
            risk: IntegrationRisk::Low,
            risk_factors: Vec::new(),
            evidence: Vec::new(),
            created_at: "2026-10-01T00:00:00Z".into(),
            wait: None,
            rank_factors: None,
            change_shape: None,
        }
    }

    fn queue(assessments: Vec<PullRequestAssessment>) -> IntegrationQueue {
        IntegrationQueue {
            repository: "o/r".into(),
            base: "master".into(),
            master_sha: "m".into(),
            observed_at: "2026-10-11T00:00:00Z".into(),
            observed_base_sha: "m".into(),
            policy: IntegrationPolicy {
                base: "master".into(),
                required_checks: None,
                review_policy: None,
                skipped_permitted: Vec::new(),
                labels: Vec::new(),
                merge_method: Some("merge".into()),
                up_to_date_required: None,
            },
            assessments,
            next_merge: None,
            next_refresh: Vec::new(),
            tallies: Default::default(),
            diagnostics: Vec::new(),
            starving: Vec::new(),
        }
    }

    /// #1 on master, #2 on #1 by its base branch, #3 on #2 by its body, #9 alone.
    fn three_and_one() -> (IntegrationQueue, LinkFacts) {
        let a = pr(1, PullRequestDisposition::Ready);
        let mut b = pr(2, PullRequestDisposition::WaitingForDependency);
        b.base_ref = "feature/1".into();
        b.reasons = vec![ReasonCode::StackedOn { number: 1 }];
        b.dependencies = vec![dep(1, DependencyState::Open)];
        let mut c = pr(3, PullRequestDisposition::WaitingForDependency);
        c.dependencies = vec![dep(2, DependencyState::Open)];
        let d = pr(9, PullRequestDisposition::NeedsRefresh);
        // ranked as the queue would put them: #9 before the train's upper links
        let q = queue(vec![a, d, c, b]);
        let mut facts = LinkFacts::default();
        facts.bodies.insert(3, "Stacked on #2.\nSee #9.".into());
        (q, facts)
    }

    #[test]
    fn a_stack_is_one_train_in_landing_order_and_a_lone_pull_request_is_none() {
        let (q, facts) = three_and_one();
        let v = trains(&q, &facts, &BTreeMap::new());
        assert!(v.observed);
        assert_eq!(v.trains.len(), 1, "{v:#?}");
        let t = &v.trains[0];
        assert_eq!(t.id, "train-1");
        let order: Vec<u64> = t.links.iter().map(|l| l.number).collect();
        assert_eq!(
            order,
            [1, 2, 3],
            "lower links first, whatever the queue rank"
        );
        assert_eq!(
            t.links.iter().map(|l| l.position).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(
            t.links.iter().map(|l| l.depth).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(t.roots, [1]);
        assert_eq!(t.tops, [3]);
        assert_eq!(
            t.next,
            Some(1),
            "the root is the only link whose order allows it"
        );
        assert!(t.links[0].at_turn && !t.links[1].at_turn && !t.links[2].at_turn);
        assert_eq!(t.links[1].on, [1]);
        assert_eq!(t.links[0].carries, [2]);
        assert_eq!(v.tallies.trains, 1);
        assert_eq!(v.tallies.links, 3);
        assert_eq!(
            v.tallies.by_disposition.get("waiting_for_dependency"),
            Some(&2)
        );
    }

    #[test]
    fn a_base_branch_or_a_stacked_on_line_is_a_stack_and_any_other_marker_an_ordering() {
        let (q, mut facts) = three_and_one();
        let v = trains(&q, &facts, &BTreeMap::new());
        let kinds: Vec<(u64, u64, TrainEdgeKind)> = v.trains[0]
            .edges
            .iter()
            .map(|e| (e.from, e.to, e.kind))
            .collect();
        assert_eq!(
            kinds,
            [
                (2, 1, TrainEdgeKind::StackedOn),
                (3, 2, TrainEdgeKind::StackedOn)
            ]
        );
        facts.bodies.insert(3, "Depends on #2".into());
        let v = trains(&q, &facts, &BTreeMap::new());
        assert_eq!(v.trains[0].edges[1].kind, TrainEdgeKind::DependsOn);
        assert_eq!(v.trains[0].edges[1].kind.label(), "DEPENDS_ON");
    }

    #[test]
    fn a_prepared_head_is_pushed_when_it_is_the_head_and_pending_otherwise() {
        let (q, facts) = three_and_one();
        let prep: BTreeMap<u64, String> = [
            (1, "h1".to_string()),
            (2, "abc".to_string()),
            (9, "h9".to_string()),
            (77, "dead".to_string()),
        ]
        .into_iter()
        .collect();
        let v = trains(&q, &facts, &prep);
        let t = &v.trains[0];
        let p1 = t.links[0].prep.as_ref().expect("#1 is prepared");
        assert_eq!(p1.state, PrepState::Pushed);
        assert_eq!(p1.reference, "refs/prep/1");
        assert_eq!(
            t.links[1].prep.as_ref().map(|p| p.state),
            Some(PrepState::Pending)
        );
        assert!(t.links[2].prep.is_none());
        assert_eq!(v.tallies.pending_prep, 1);
        assert_eq!(
            v.unattached_prep,
            [
                UnattachedPrep {
                    number: 9,
                    sha: "h9".into(),
                    open: true
                },
                UnattachedPrep {
                    number: 77,
                    sha: "dead".into(),
                    open: false
                }
            ]
        );
    }

    #[test]
    fn a_link_whose_lower_link_landed_is_still_a_train_and_at_its_turn() {
        let mut top = pr(5, PullRequestDisposition::Ready);
        top.dependencies = vec![dep(4, DependencyState::Merged)];
        let mut over = pr(6, PullRequestDisposition::WaitingForDependency);
        over.dependencies = vec![dep(5, DependencyState::Open)];
        let mut stray = pr(7, PullRequestDisposition::WaitingForDependency);
        stray.dependencies = vec![dep(3, DependencyState::ClosedUnmerged)];
        let v = trains(
            &queue(vec![top, over, stray]),
            &LinkFacts::default(),
            &BTreeMap::new(),
        );
        assert_eq!(v.trains.len(), 2);
        let t = train(&v, "train-5").expect("train-5");
        assert_eq!(t.roots, [5]);
        assert_eq!(t.next, Some(5));
        assert!(t.links[0].at_turn);
        assert_eq!(t.beneath.len(), 1);
        assert_eq!(t.beneath[0].state, DependencyState::Merged);
        assert!(t
            .edges
            .iter()
            .any(|e| e.from == 5 && e.to == 4 && e.satisfied));
        let s = train(&v, "train-7").expect("a lone link on a closed pull request is a train");
        assert_eq!(
            s.next, None,
            "a dependency closed without a merge never satisfies"
        );
        assert!(!s.links[0].at_turn);
    }

    #[test]
    fn a_cycle_is_ordered_last_marked_and_diagnosed() {
        let mut a = pr(1, PullRequestDisposition::WaitingForDependency);
        a.dependencies = vec![dep(2, DependencyState::Open)];
        let mut b = pr(2, PullRequestDisposition::WaitingForDependency);
        b.dependencies = vec![dep(1, DependencyState::Open)];
        let v = trains(&queue(vec![a, b]), &LinkFacts::default(), &BTreeMap::new());
        let t = &v.trains[0];
        assert!(t.links.iter().all(|l| l.in_cycle && !l.at_turn));
        assert_eq!(t.next, None);
        assert!(
            v.diagnostics.iter().any(|d| d.contains("dependency cycle")),
            "{:?}",
            v.diagnostics
        );
    }

    #[test]
    fn two_stacks_are_two_trains_the_better_ranked_first() {
        let a = pr(10, PullRequestDisposition::Ready);
        let mut b = pr(11, PullRequestDisposition::WaitingForChecks);
        b.dependencies = vec![dep(10, DependencyState::Open)];
        let c = pr(20, PullRequestDisposition::Ready);
        let mut d = pr(21, PullRequestDisposition::Draft);
        d.dependencies = vec![dep(20, DependencyState::Open)];
        let v = trains(
            &queue(vec![c, a, b, d]),
            &LinkFacts::default(),
            &BTreeMap::new(),
        );
        let ids: Vec<&str> = v.trains.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["train-20", "train-10"]);
    }

    #[test]
    fn a_diamond_has_two_roots_one_top_and_the_longest_depth() {
        let a = pr(1, PullRequestDisposition::Ready);
        let b = pr(2, PullRequestDisposition::Ready);
        let mut c = pr(3, PullRequestDisposition::WaitingForDependency);
        c.dependencies = vec![dep(1, DependencyState::Open)];
        let mut d = pr(4, PullRequestDisposition::WaitingForDependency);
        d.dependencies = vec![dep(3, DependencyState::Open), dep(2, DependencyState::Open)];
        let v = trains(
            &queue(vec![b, a, c, d]),
            &LinkFacts::default(),
            &BTreeMap::new(),
        );
        let t = &v.trains[0];
        assert_eq!(
            t.roots,
            [2, 1],
            "roots in turn order, which follows queue rank"
        );
        assert_eq!(t.tops, [4]);
        assert_eq!(t.id, "train-2");
        let four = t.links.iter().find(|l| l.number == 4).unwrap();
        assert_eq!(four.depth, 2);
        assert_eq!(four.position, 4);
    }

    #[test]
    fn the_relation_is_read_for_its_count_and_its_word() {
        let (q, facts) = three_and_one();
        let v = trains(&q, &facts, &BTreeMap::new());
        let l = &v.trains[0].links[0];
        assert_eq!(l.relation_kind, "behind");
        assert_eq!(l.behind, Some(3));
        assert!(!l.carries_master);
        assert_eq!(l.url.as_deref(), Some("https://github.com/o/r/pull/1"));
        assert_eq!(
            l.checks_url.as_deref(),
            Some("https://github.com/o/r/pull/1/checks")
        );
    }

    #[test]
    fn an_inferred_dependency_joins_nothing() {
        let a = pr(1, PullRequestDisposition::Ready);
        let mut b = pr(2, PullRequestDisposition::Ready);
        b.dependencies = vec![PullRequestDependency {
            number: 1,
            certainty: DependencyCertainty::Inferred,
            satisfied: false,
            state: DependencyState::Open,
        }];
        let v = trains(&queue(vec![a, b]), &LinkFacts::default(), &BTreeMap::new());
        assert!(v.trains.is_empty(), "{v:#?}");
    }

    #[test]
    fn the_queue_diagnostics_are_carried_and_unobserved_says_why() {
        let (mut q, facts) = three_and_one();
        q.diagnostics.push("the observation is stale".into());
        let v = trains(&q, &facts, &BTreeMap::new());
        assert_eq!(v.diagnostics, ["the observation is stale"]);
        let u = TrainsView::unobserved("nothing".into());
        assert_eq!(u.reason.as_deref(), Some("nothing"));
    }

    #[test]
    fn prep_refs_are_parsed_by_number_only() {
        let refs = parse_prep_refs(
            "refs/prep/807 aaa\nrefs/prep/883 bbb\nrefs/prep/x ccc\nrefs/heads/807 ddd\n\n",
        );
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[&883], "bbb");
    }

    #[test]
    fn prep_refs_of_a_repository_are_read_and_nothing_is_written() {
        let dir = std::env::temp_dir().join(format!("mj-train-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["-c", "user.email=t@example.com", "-c", "user.name=t"])
                .args(args)
                .output()
                .unwrap()
        };
        git(&["init", "-q"]);
        git(&["commit", "-q", "--allow-empty", "-m", "x"]);
        git(&["update-ref", "refs/prep/12", "HEAD"]);
        let head = String::from_utf8(git(&["rev-parse", "HEAD"]).stdout).unwrap();
        let before = String::from_utf8(git(&["for-each-ref"]).stdout).unwrap();
        let refs = prep_refs(&dir);
        assert_eq!(refs.get(&12).map(String::as_str), Some(head.trim()));
        let after = String::from_utf8(git(&["for-each-ref"]).stdout).unwrap();
        assert_eq!(before, after);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            prep_refs(&dir).is_empty(),
            "no repository, no prepared heads"
        );
    }
}
