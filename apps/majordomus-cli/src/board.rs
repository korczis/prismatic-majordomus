//! The delivery board (I4294): the documents a dispatch board reads, derived from the plan,
//! the intents and the recorded pull request queue, with the board's lanes declared once in
//! `share/board.yaml`.
//!
//! A lane is a key, a title and the milestones it holds. Every open issue is filed under the
//! lane of its milestone, and a milestone no lane holds is an error rather than "other": the
//! composition refuses, naming every such milestone, and a unit test over this repository's
//! own plan fails CI the same way. Only a DONE milestone may stay in no lane.
//!
//! The documents are the shapes the board's page reads, keyed by document id: `meta`,
//! `milestones`, `intents`, `prs` and one `issues-<lane>` per lane with open work, split
//! into `issues-<lane>-2` and onwards whenever one would serialize past
//! [`DOCUMENT_LIMIT`]. The digest is a SHA-256 over their canonical JSON with the volatile
//! `meta.syncedAt` and `meta.syncedBy` left out, so that equal state is an equal digest
//! and a sync step can skip a write that would change nothing. Mesh seats, live work, CI
//! runners and blockers are the sync step's to add live; nothing here reads the network.

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::plan::{Plan, PlanMilestone};

/// The file name under the share directory.
pub(crate) const BOARD_FILE: &str = "board.yaml";

/// The most bytes one document may serialize to, compact. The board's store refuses a
/// document past 256 KiB; the bound leaves room for the envelope it is written in.
pub(crate) const DOCUMENT_LIMIT: usize = 250_000;

/// The issue statuses the board shows: everything not yet DONE or CANCELLED.
const OPEN: [&str; 4] = ["READY", "BLOCKED", "ACTIVE", "VERIFY"];

/// The meta fields that change on every sync and say nothing about the state.
const VOLATILE: [&str; 2] = ["syncedAt", "syncedBy"];

/// One lane of the board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Lane {
    /// The key a document id and an item carry: `issues-<key>`.
    pub(crate) key: String,
    /// The lane's name on the board.
    pub(crate) title: String,
    /// The milestones it holds, by id.
    #[serde(default)]
    pub(crate) milestones: Vec<String>,
}

/// The lanes, as `share/board.yaml` declares them, in declaration order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BoardDeclaration {
    /// The declaration's format version.
    #[serde(default)]
    pub(crate) version: u32,
    /// The lanes, in the board's order.
    #[serde(default)]
    pub(crate) lanes: Vec<Lane>,
}

impl BoardDeclaration {
    /// The declaration the share directory holds; none at all is a declaration of no lanes.
    pub(crate) fn load(share: &Path) -> Result<BoardDeclaration, String> {
        let path = share.join(BOARD_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                BoardDeclaration::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BoardDeclaration::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Parse and check a declaration: a lane key or a milestone declared twice is refused,
    /// because either would make one milestone's lane a matter of reading order.
    pub(crate) fn parse(text: &str) -> Result<BoardDeclaration, String> {
        let declaration: BoardDeclaration = crate::metadata::yaml::parse_into(text)?;
        let mut keys = std::collections::BTreeSet::new();
        let mut held: BTreeMap<&str, &str> = BTreeMap::new();
        for lane in &declaration.lanes {
            if lane.key.is_empty() || !keys.insert(lane.key.as_str()) {
                return Err(format!(
                    "lane key '{}' is empty or declared twice",
                    lane.key
                ));
            }
            for m in &lane.milestones {
                if let Some(other) = held.insert(m, &lane.key) {
                    return Err(format!(
                        "milestone '{m}' is in lane '{other}' and in lane '{}'",
                        lane.key
                    ));
                }
            }
        }
        Ok(declaration)
    }

    /// The key of the lane holding a milestone.
    pub(crate) fn lane_of(&self, milestone: &str) -> Option<&str> {
        self.lanes
            .iter()
            .find(|l| l.milestones.iter().any(|m| m == milestone))
            .map(|l| l.key.as_str())
    }
}

/// Every milestone of the plan that is not DONE and no lane holds, then every open issue
/// whose milestone no lane holds and the plan does not declare. Empty is the only state in
/// which the board can be composed.
pub(crate) fn unlaned(declaration: &BoardDeclaration, plan: &Plan) -> Vec<String> {
    let mut found: Vec<String> = plan
        .milestones
        .iter()
        .filter(|m| m.status != "DONE" && declaration.lane_of(&m.id).is_none())
        .map(|m| format!("milestone {} ({}) is in no lane", m.id, m.status))
        .collect();
    found.extend(
        plan.issues
            .iter()
            .filter(|i| OPEN.contains(&i.status.as_str()))
            .filter(|i| declaration.lane_of(&i.milestone).is_none())
            .filter(|i| plan.milestone(&i.milestone).is_none())
            .map(|i| {
                format!(
                    "issue {} is {} under milestone {}, which the plan does not declare and no lane holds",
                    i.id, i.status, i.milestone
                )
            }),
    );
    found
}

/// Every milestone a lane names that the plan does not declare: a lane left behind by a
/// milestone that was renamed or removed.
pub(crate) fn stale(declaration: &BoardDeclaration, plan: &Plan) -> Vec<String> {
    declaration
        .lanes
        .iter()
        .flat_map(|l| l.milestones.iter().map(move |m| (l, m)))
        .filter(|(_, m)| plan.milestone(m).is_none())
        .map(|(l, m)| {
            format!(
                "lane {} names milestone {m}, which the plan does not declare",
                l.key
            )
        })
        .collect()
}

/// The commit the snapshot describes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Master {
    /// The ref it was read from.
    pub(crate) reference: String,
    /// The commit.
    pub(crate) sha: String,
    /// Its committer time, RFC 3339.
    pub(crate) committed_at: String,
}

impl Master {
    /// The first of `origin/master` and `HEAD` that resolves in the checkout at `root`.
    pub(crate) fn read(root: &Path) -> Option<Master> {
        ["refs/remotes/origin/master", "HEAD"].iter().find_map(|r| {
            let out = crate::git::read_only(root)
                .args(["log", "-1", "--no-color", "--format=%H%x1f%cI", r, "--"])
                .output()
                .ok()
                .filter(|o| o.status.success())?;
            let text = String::from_utf8_lossy(&out.stdout);
            let (sha, at) = text.trim().split_once('\u{1f}')?;
            Some(Master {
                reference: (*r).to_string(),
                sha: sha.to_string(),
                committed_at: at.to_string(),
            })
        })
    }
}

/// One pull request as the board shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct PrItem {
    /// The number.
    pub(crate) n: u64,
    /// The title.
    pub(crate) t: String,
    /// The head branch.
    pub(crate) b: String,
    /// The disposition, in the queue's word.
    pub(crate) disp: String,
    /// The lane of the queue, in its word.
    pub(crate) lane: String,
    /// The first four reasons.
    pub(crate) why: Vec<String>,
    /// The head commit, abbreviated to eleven characters.
    pub(crate) head: String,
    /// Whether the head is a train's top.
    pub(crate) train: bool,
    /// The risk level.
    pub(crate) risk: String,
}

/// The pull request queue as the board reads it: the last recorded observation, or why
/// there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Queue {
    /// The observation, when it was taken, and its pull requests in queue order.
    Observed {
        /// When the forge was observed.
        observed_at: String,
        /// The pull requests.
        items: Vec<PrItem>,
    },
    /// No observation could be read; the reason.
    Unavailable(String),
}

/// Everything a snapshot is composed from, already read.
#[derive(Debug, Clone)]
pub(crate) struct Inputs<'a> {
    /// The lanes.
    pub(crate) declaration: &'a BoardDeclaration,
    /// The derived plan.
    pub(crate) plan: &'a Plan,
    /// The milestones in roadmap order, each with the release that carried it.
    pub(crate) roadmap: Vec<PlanMilestone>,
    /// The intents, as `intents.list` answers them.
    pub(crate) intents: Vec<Value>,
    /// The pull request queue.
    pub(crate) queue: Queue,
    /// The release currency, as `release.currency` answers it.
    pub(crate) release: Value,
    /// The commit described.
    pub(crate) master: Option<Master>,
    /// When the snapshot is taken.
    pub(crate) synced_at: String,
    /// Who takes it.
    pub(crate) synced_by: String,
}

/// The composed board.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Composed {
    /// The documents, by id.
    pub(crate) documents: BTreeMap<String, Value>,
    /// What the composition could not read, in words; never a reason to refuse.
    pub(crate) diagnostics: Vec<String>,
}

/// A value's serde word: an enum serialized as a string, empty for anything else.
pub(crate) fn word<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(s)) => s,
        _ => String::new(),
    }
}

/// Compose the documents, or refuse naming every milestone no lane holds, or every
/// document that cannot be brought under [`DOCUMENT_LIMIT`].
pub(crate) fn compose(inputs: Inputs<'_>) -> Result<Composed, String> {
    let missing = unlaned(inputs.declaration, inputs.plan);
    if !missing.is_empty() {
        return Err(format!(
            "every open milestone needs a lane in share/{BOARD_FILE}: {}",
            missing.join("; ")
        ));
    }
    let lane = |m: &str| inputs.declaration.lane_of(m).map(str::to_string);
    let mut documents: BTreeMap<String, Value> = BTreeMap::new();
    let mut diagnostics = stale(inputs.declaration, inputs.plan);

    // open issues, filed by lane in declaration order, each lane split under the bound
    for l in &inputs.declaration.lanes {
        let items: Vec<Value> = inputs
            .plan
            .issues
            .iter()
            .filter(|i| OPEN.contains(&i.status.as_str()))
            .filter(|i| lane(&i.milestone).as_deref() == Some(l.key.as_str()))
            .map(|i| {
                json!({
                    "id": i.id, "t": i.title, "ms": i.milestone, "p": i.priority,
                    "s": i.status, "w": i.wave, "d": i.depends_on, "l": l.key,
                })
            })
            .collect();
        documents.extend(split(&l.key, items, DOCUMENT_LIMIT)?);
    }

    let milestones: Vec<Value> = inputs
        .roadmap
        .iter()
        .map(|m| {
            let n = |s: &str| m.counts.by_status.get(s).copied().unwrap_or(0);
            json!({
                "id": m.id, "t": m.title, "s": m.status, "rank": m.rank, "l": lane(&m.id),
                "total": m.counts.total, "done": n("DONE"), "ready": n("READY"),
                "blocked": n("BLOCKED"), "active": n("ACTIVE") + n("VERIFY"),
                "cancelled": n("CANCELLED"), "blockedBy": m.blocked_by,
                "release": m.released_in,
            })
        })
        .collect();
    documents.insert("milestones".into(), json!({ "items": milestones }));

    let intents: Vec<Value> = inputs
        .intents
        .iter()
        .map(|x| {
            let strings = |v: &Value, key: &str, bound: usize| -> Vec<Value> {
                v.as_array()
                    .into_iter()
                    .flatten()
                    .take(bound)
                    .map(|e| e[key].clone())
                    .collect()
            };
            json!({
                "id": x["id"], "t": x["title"], "stage": x["stage"],
                "verdict": x["verdict"]["state"], "met": x["met"].as_u64().unwrap_or(0),
                "total": x["satisfaction"].as_array().map_or(0, Vec::len),
                "ms": strings(&x["milestones"], "id", usize::MAX),
                "holding": strings(&x["verdict"]["reasons"], "criterion", 6),
            })
        })
        .collect();
    documents.insert("intents".into(), json!({ "items": intents }));

    let prs = match &inputs.queue {
        Queue::Observed { observed_at, items } => {
            json!({ "items": items, "observedAt": observed_at })
        }
        Queue::Unavailable(reason) => {
            diagnostics.push(format!("the pull request queue is not observed: {reason}"));
            json!({ "items": [], "observedAt": null, "unavailable": reason })
        }
    };
    documents.insert("prs".into(), prs);

    let count = |s: &str| inputs.plan.issues.iter().filter(|i| i.status == s).count();
    let release = &inputs.release;
    let master = inputs.master.clone().unwrap_or_default();
    let lanes: Vec<Value> = inputs
        .declaration
        .lanes
        .iter()
        .map(|l| json!([l.key, l.title]))
        .collect();
    documents.insert(
        "meta".into(),
        json!({
            "sha": master.sha, "masterAt": master.committed_at,
            "syncedAt": inputs.synced_at, "syncedBy": inputs.synced_by,
            "lanes": lanes,
            "counts": {
                "total": inputs.plan.issues.len(), "done": count("DONE"),
                "cancelled": count("CANCELLED"), "ready": count("READY"),
                "blocked": count("BLOCKED"), "active": count("ACTIVE") + count("VERIFY"),
            },
            "release": {
                "version": release["release"]["version"], "recorded": release["release"]["recorded"],
                "stance": release["stance"], "landed": release["landed"],
                "summary": release["summary"],
            },
        }),
    );

    let over: Vec<String> = documents
        .iter()
        .map(|(id, doc)| (id, size(doc)))
        .filter(|(_, n)| *n > DOCUMENT_LIMIT)
        .map(|(id, n)| format!("{id} is {n} bytes"))
        .collect();
    if !over.is_empty() {
        return Err(format!(
            "a document is over the {DOCUMENT_LIMIT}-byte bound: {}",
            over.join("; ")
        ));
    }
    Ok(Composed {
        documents,
        diagnostics,
    })
}

/// The compact serialized size of a document, in bytes.
pub(crate) fn size(document: &Value) -> usize {
    serde_json::to_vec(document).map_or(0, |b| b.len())
}

/// One lane's issues as `issues-<lane>`, `issues-<lane>-2`, ...: each `{"lane", "items"}`
/// and each at most `limit` bytes, the items in order and none left out. A lane with no
/// open issue has no document; an item that alone exceeds the bound is refused.
pub(crate) fn split(
    lane: &str,
    items: Vec<Value>,
    limit: usize,
) -> Result<Vec<(String, Value)>, String> {
    let name = |n: usize| match n {
        1 => format!("issues-{lane}"),
        n => format!("issues-{lane}-{n}"),
    };
    let doc = |items: &[Value]| json!({ "lane": lane, "items": items });
    let mut out: Vec<(String, Value)> = Vec::new();
    let mut chunk: Vec<Value> = Vec::new();
    for item in items {
        chunk.push(item);
        if size(&doc(&chunk)) <= limit {
            continue;
        }
        let item = chunk.pop().unwrap_or_default();
        if chunk.is_empty() {
            return Err(format!(
                "an issue of lane {lane} alone is over the {limit}-byte bound: {item}"
            ));
        }
        out.push((name(out.len() + 1), doc(&chunk)));
        chunk = vec![item];
        if size(&doc(&chunk)) > limit {
            return Err(format!(
                "an issue of lane {lane} alone is over the {limit}-byte bound: {}",
                chunk[0]
            ));
        }
    }
    if !chunk.is_empty() {
        out.push((name(out.len() + 1), doc(&chunk)));
    }
    Ok(out)
}

/// A value with every object's keys in byte order, recursively: the canonical form the
/// digest is taken over, whatever order the value was built in.
pub(crate) fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<&String, Value> =
                map.iter().map(|(k, v)| (k, canonical(v))).collect();
            Value::Object(
                sorted
                    .into_iter()
                    .map(|(k, v)| (k.clone(), v))
                    .collect::<Map<String, Value>>(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

/// `sha256:<hex>` over the canonical JSON of the documents, without `meta.syncedAt` and
/// `meta.syncedBy`: equal state is an equal digest.
pub(crate) fn digest(documents: &BTreeMap<String, Value>) -> String {
    let mut stable: Map<String, Value> = documents
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if let Some(Value::Object(meta)) = stable.get_mut("meta") {
        for key in VOLATILE {
            meta.remove(key);
        }
    }
    let text = serde_json::to_string(&canonical(&Value::Object(stable))).unwrap_or_default();
    format!("sha256:{}", crate::policy::sha256_hex(&text))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::plan::{PlanCounts, PlanIssue};

    pub(crate) fn milestone(id: &str, status: &str) -> PlanMilestone {
        PlanMilestone {
            id: id.into(),
            status: status.into(),
            order: 0,
            priority: "p1".into(),
            title: format!("the {id} milestone"),
            slug: id.into(),
            last_accepted: String::new(),
            released_in: None,
            rank: 0,
            depends_on: Vec::new(),
            blocked_by: Vec::new(),
            dependents: Vec::new(),
            claims: Vec::new(),
            counts: PlanCounts {
                total: 1,
                required: 1,
                by_status: [("READY".to_string(), 1)].into_iter().collect(),
            },
            issues: Vec::new(),
            outcome: String::new(),
        }
    }

    pub(crate) fn issue(id: &str, milestone: &str, status: &str, title: &str) -> PlanIssue {
        PlanIssue {
            id: id.into(),
            milestone: milestone.into(),
            status: status.into(),
            wave: 0,
            priority: "p1".into(),
            profile: "implementation".into(),
            parallel_safe: true,
            title: title.into(),
            slug: id.into(),
            depends_on: Vec::new(),
            blocked_by: Vec::new(),
            dependents: Vec::new(),
            scope: Vec::new(),
            serves: Vec::new(),
            objective: String::new(),
            evidence_have: 0,
            evidence_need: 0,
            started_at: String::new(),
            verified_at: String::new(),
            completed_at: String::new(),
        }
    }

    pub(crate) fn plan(milestones: Vec<PlanMilestone>, issues: Vec<PlanIssue>) -> Plan {
        Plan {
            project: crate::plan::PlanProject {
                name: "fixture".into(),
                repository: "fixture".into(),
                default_branch: "master".into(),
                active_milestone: String::new(),
            },
            statuses: crate::plan::PlanVocabulary {
                issue: OPEN.iter().map(|s| s.to_string()).collect(),
                milestone: Vec::new(),
            },
            milestones,
            issues,
            waves: Vec::new(),
            edges: Vec::new(),
            milestone_edges: Vec::new(),
            findings: Vec::new(),
        }
    }

    const LANES: &str = "version: 1\nlanes:\n  - key: land\n    title: \"Landing, CI\"\n    milestones:\n      - m-land\n  - key: ui\n    title: UI\n    milestones:\n      - m-ui\n";

    fn inputs<'a>(declaration: &'a BoardDeclaration, plan: &'a Plan) -> Inputs<'a> {
        Inputs {
            declaration,
            plan,
            roadmap: plan.milestones.clone(),
            intents: vec![json!({
                "id": "x", "title": "X holds", "stage": "planned", "met": 1,
                "satisfaction": [{}, {}], "milestones": [{"id": "m-ui"}],
                "verdict": {"state": "unknown", "reasons": [{"criterion": "c1"}]},
            })],
            queue: Queue::Observed {
                observed_at: "2026-10-28T10:00:00Z".into(),
                items: vec![PrItem {
                    n: 7,
                    t: "feat: a thing".into(),
                    b: "integration/train-1".into(),
                    disp: "ready".into(),
                    lane: "ready".into(),
                    why: vec!["contains_master".into()],
                    head: "0123456789a".into(),
                    train: true,
                    risk: "low".into(),
                }],
            },
            release: json!({
                "stance": "current", "summary": "nothing landed", "landed": 0,
                "release": {"version": "1.0.0", "recorded": true},
            }),
            master: Some(Master {
                reference: "HEAD".into(),
                sha: "abc".into(),
                committed_at: "2026-10-27T21:09:24+01:00".into(),
            }),
            synced_at: "2026-10-28T10:00:00Z".into(),
            synced_by: "a test".into(),
        }
    }

    fn sample() -> (BoardDeclaration, Plan) {
        let declaration = BoardDeclaration::parse(LANES).expect("the lanes parse");
        let plan = plan(
            vec![
                milestone("m-land", "ACTIVE"),
                milestone("m-ui", "PLANNED"),
                milestone("m-old", "DONE"),
            ],
            vec![
                issue("I1", "m-land", "READY", "land it"),
                issue("I2", "m-ui", "BLOCKED", "draw it"),
                issue("I3", "m-ui", "DONE", "drawn"),
                issue("I4", "m-old", "DONE", "old"),
            ],
        );
        (declaration, plan)
    }

    #[test]
    fn the_declaration_is_read_in_order_and_refuses_a_double() {
        let d = BoardDeclaration::parse(LANES).expect("the lanes parse");
        assert_eq!(d.version, 1);
        let keys: Vec<&str> = d.lanes.iter().map(|l| l.key.as_str()).collect();
        assert_eq!(keys, ["land", "ui"]);
        assert_eq!(d.lanes[0].title, "Landing, CI");
        assert_eq!(d.lane_of("m-ui"), Some("ui"));
        assert_eq!(d.lane_of("nowhere"), None);

        let twice = format!("{LANES}  - key: land\n    title: Again\n");
        assert!(BoardDeclaration::parse(&twice)
            .unwrap_err()
            .contains("lane key 'land'"));
        let shared =
            format!("{LANES}  - key: other\n    title: O\n    milestones:\n      - m-ui\n");
        let e = BoardDeclaration::parse(&shared).unwrap_err();
        assert!(
            e.contains("milestone 'm-ui' is in lane 'ui' and in lane 'other'"),
            "{e}"
        );
        assert!(BoardDeclaration::parse("lanes: 3\n").is_err());
    }

    #[test]
    fn the_declaration_is_loaded_from_the_share_and_absent_means_no_lanes() {
        let dir = tempfile::tempdir().expect("a directory");
        assert_eq!(
            BoardDeclaration::load(dir.path()).expect("absent is no lanes"),
            BoardDeclaration::default()
        );
        std::fs::write(dir.path().join(BOARD_FILE), LANES).expect("written");
        assert_eq!(
            BoardDeclaration::load(dir.path())
                .expect("read")
                .lanes
                .len(),
            2
        );
        std::fs::write(dir.path().join(BOARD_FILE), "lanes: 3\n").expect("written");
        let e = BoardDeclaration::load(dir.path()).unwrap_err();
        assert!(e.contains(BOARD_FILE), "{e}");
        std::fs::remove_file(dir.path().join(BOARD_FILE)).expect("removed");
        std::fs::create_dir(dir.path().join(BOARD_FILE)).expect("a directory in its place");
        assert!(BoardDeclaration::load(dir.path()).is_err());
    }

    #[test]
    fn a_milestone_in_no_lane_is_an_error_unless_it_is_done() {
        let (declaration, mut plan) = sample();
        assert!(unlaned(&declaration, &plan).is_empty(), "m-old is DONE");
        plan.milestones.push(milestone("m-new", "PLANNED"));
        plan.issues.push(issue("I5", "m-gone", "READY", "orphan"));
        let found = unlaned(&declaration, &plan);
        assert_eq!(
            found,
            [
                "milestone m-new (PLANNED) is in no lane",
                "issue I5 is READY under milestone m-gone, which the plan does not declare and no lane holds",
            ]
        );
        let e = compose(inputs(&declaration, &plan)).unwrap_err();
        assert!(
            e.contains("m-new") && e.contains("I5") && e.contains(BOARD_FILE),
            "{e}"
        );
    }

    #[test]
    fn a_lane_naming_a_milestone_the_plan_lacks_is_reported() {
        let (declaration, mut plan) = sample();
        assert!(stale(&declaration, &plan).is_empty());
        plan.milestones.retain(|m| m.id != "m-land");
        plan.issues.retain(|i| i.milestone != "m-land");
        assert_eq!(
            stale(&declaration, &plan),
            ["lane land names milestone m-land, which the plan does not declare"]
        );
        let composed = compose(inputs(&declaration, &plan)).expect("a stale lane is no refusal");
        assert_eq!(composed.diagnostics.len(), 1);
    }

    #[test]
    fn the_documents_carry_the_shapes_the_page_reads() {
        let (declaration, plan) = sample();
        let c = compose(inputs(&declaration, &plan)).expect("composed");
        let ids: Vec<&str> = c.documents.keys().map(String::as_str).collect();
        assert_eq!(
            ids,
            [
                "intents",
                "issues-land",
                "issues-ui",
                "meta",
                "milestones",
                "prs"
            ]
        );
        let land = &c.documents["issues-land"];
        assert_eq!(land["lane"], "land");
        assert_eq!(
            land["items"][0],
            json!({"id": "I1", "t": "land it", "ms": "m-land", "p": "p1", "s": "READY",
                   "w": 0, "d": [], "l": "land"})
        );
        assert_eq!(
            c.documents["issues-ui"]["items"].as_array().map(Vec::len),
            Some(1)
        );
        let ms = &c.documents["milestones"]["items"];
        assert_eq!(ms[0]["l"], "land");
        assert_eq!(
            ms[2]["l"],
            Value::Null,
            "a DONE milestone may be in no lane"
        );
        assert_eq!(ms[0]["ready"], 1);
        let intent = &c.documents["intents"]["items"][0];
        assert_eq!(
            intent,
            &json!({"id": "x", "t": "X holds", "stage": "planned", "verdict": "unknown",
                    "met": 1, "total": 2, "ms": ["m-ui"], "holding": ["c1"]})
        );
        assert_eq!(c.documents["prs"]["items"][0]["n"], 7);
        assert_eq!(c.documents["prs"]["observedAt"], "2026-10-28T10:00:00Z");
        let meta = &c.documents["meta"];
        assert_eq!(
            meta["lanes"],
            json!([["land", "Landing, CI"], ["ui", "UI"]])
        );
        assert_eq!(meta["sha"], "abc");
        assert_eq!(meta["counts"]["total"], 4);
        assert_eq!(meta["counts"]["done"], 2);
        assert_eq!(meta["counts"]["blocked"], 1);
        assert_eq!(meta["release"]["version"], "1.0.0");
        for absent in ["seats", "work", "ci", "blockers"] {
            assert!(meta.get(absent).is_none(), "{absent} is the sync step's");
        }
        assert!(c.diagnostics.is_empty());
    }

    #[test]
    fn an_absent_queue_is_said_rather_than_refused() {
        let (declaration, plan) = sample();
        let mut i = inputs(&declaration, &plan);
        i.queue = Queue::Unavailable("no forge observation is recorded".into());
        i.master = None;
        let c = compose(i).expect("composed without a queue");
        let prs = &c.documents["prs"];
        assert_eq!(prs["items"], json!([]));
        assert_eq!(prs["observedAt"], Value::Null);
        assert_eq!(prs["unavailable"], "no forge observation is recorded");
        assert_eq!(
            c.diagnostics,
            ["the pull request queue is not observed: no forge observation is recorded"]
        );
        assert_eq!(c.documents["meta"]["sha"], "");
    }

    #[test]
    fn the_digest_is_stable_moves_with_the_state_and_ignores_the_sync() {
        let (declaration, plan) = sample();
        let a = compose(inputs(&declaration, &plan))
            .expect("composed")
            .documents;
        let b = compose(inputs(&declaration, &plan))
            .expect("composed")
            .documents;
        assert_eq!(digest(&a), digest(&b), "the same input, the same digest");
        assert!(digest(&a).starts_with("sha256:") && digest(&a).len() == 7 + 64);

        let mut later = inputs(&declaration, &plan);
        later.synced_at = "2027-01-01T00:00:00Z".into();
        later.synced_by = "another session".into();
        let later = compose(later).expect("composed").documents;
        assert_ne!(later["meta"]["syncedAt"], a["meta"]["syncedAt"]);
        assert_eq!(
            digest(&later),
            digest(&a),
            "syncedAt and syncedBy do not count"
        );

        let mut moved = plan.clone();
        moved.issues[0].status = "ACTIVE".into();
        let moved = compose(inputs(&declaration, &moved))
            .expect("composed")
            .documents;
        assert_ne!(digest(&moved), digest(&a), "a change is a different digest");

        // key order is not state: a value built in another order digests the same
        let mut reordered = a.clone();
        let meta = reordered["meta"].as_object().expect("meta").clone();
        let reversed: Map<String, Value> = meta.into_iter().rev().collect();
        reordered.insert("meta".into(), Value::Object(reversed));
        assert_eq!(digest(&reordered), digest(&a));
    }

    #[test]
    fn a_lane_is_split_under_the_bound_and_keeps_every_item_in_order() {
        let items: Vec<Value> = (0..40)
            .map(|n| json!({"id": format!("I{n:04}"), "t": "x".repeat(50)}))
            .collect();
        let docs = split("land", items.clone(), 1_000).expect("split");
        assert!(docs.len() > 2, "forty items do not fit one kilobyte");
        assert_eq!(docs[0].0, "issues-land");
        assert_eq!(docs[1].0, "issues-land-2");
        let mut back: Vec<Value> = Vec::new();
        for (id, doc) in &docs {
            assert!(size(doc) <= 1_000, "{id} is {} bytes", size(doc));
            assert_eq!(doc["lane"], "land");
            back.extend(doc["items"].as_array().expect("items").iter().cloned());
        }
        assert_eq!(back, items, "nothing lost, nothing reordered");
        assert!(split("land", Vec::new(), 1_000).expect("empty").is_empty());
        assert_eq!(
            split("land", items[..1].to_vec(), 1_000)
                .expect("one")
                .len(),
            1
        );

        let huge = vec![json!({"t": "x".repeat(2_000)})];
        assert!(split("land", huge, 1_000).unwrap_err().contains("alone"));
        let late = vec![json!({"t": "x"}), json!({"t": "x".repeat(2_000)})];
        assert!(split("land", late, 1_000).unwrap_err().contains("alone"));
    }

    #[test]
    fn a_document_over_the_bound_is_refused_by_name() {
        let (declaration, plan) = sample();
        let mut i = inputs(&declaration, &plan);
        i.intents = vec![json!({"id": "y", "title": "y".repeat(DOCUMENT_LIMIT)})];
        let e = compose(i).unwrap_err();
        assert!(e.contains("intents is"), "{e}");
    }

    #[test]
    fn an_enum_is_read_as_its_word() {
        assert_eq!(word(&crate::integration::IntegrationRisk::Low), "low");
        assert_eq!(word(&3), "");
    }

    #[test]
    fn the_master_is_read_from_git() {
        let dir = tempfile::tempdir().expect("a directory");
        assert_eq!(Master::read(dir.path()), None, "no repository, no commit");
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .status()
                .expect("git runs");
            assert!(ok.success(), "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        git(&["commit", "-q", "--allow-empty", "-m", "one"]);
        let m = Master::read(dir.path()).expect("HEAD resolves");
        assert_eq!(m.reference, "HEAD");
        assert_eq!(m.sha.len(), 40);
        assert!(m.committed_at.contains('T'), "{}", m.committed_at);
    }

    // ------------------------------------------------ this repository's own board

    /// This repository's plan and lanes, as CI checks them out.
    fn this_repository() -> (BoardDeclaration, Plan, crate::app::App) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let share = crate::synthetic::crate_share();
        let args = crate::cli::RepoArgs {
            share: Some(share.clone()),
            ..crate::cli::RepoArgs::default()
        };
        let repository =
            crate::repository::Repository::discover(&root).expect("this repository is found");
        let app = crate::app::App::load_from(&args, repository).expect("this repository loads");
        let declaration = BoardDeclaration::load(&share).expect("share/board.yaml is read");
        let plan = Plan::build(app.index());
        assert!(
            !plan.milestones.is_empty(),
            "this repository's plan was not read"
        );
        (declaration, plan, app)
    }

    /// Every open milestone of this repository's plan is in a lane of share/board.yaml: a
    /// milestone added without one fails here, naming it, before it reaches the board.
    #[test]
    fn every_open_milestone_has_a_lane() {
        let (declaration, plan, _) = this_repository();
        let missing = unlaned(&declaration, &plan);
        assert!(
            missing.is_empty(),
            "add each to a lane of share/{BOARD_FILE}:\n  {}",
            missing.join("\n  ")
        );
        let left = stale(&declaration, &plan);
        assert!(
            left.is_empty(),
            "remove or rename each in share/{BOARD_FILE}:\n  {}",
            left.join("\n  ")
        );
    }

    /// This repository's board composes, and every document is under the bound.
    #[test]
    fn this_repositorys_documents_are_under_the_bound() {
        let (declaration, plan, app) = this_repository();
        let evidence = crate::intent::RepositoryEvidence::load(app.index()).expect("evidence");
        let intents = crate::intent::Intents::build(app.index(), &plan, &evidence);
        let composed = compose(Inputs {
            declaration: &declaration,
            roadmap: plan.roadmap().into_iter().cloned().collect(),
            plan: &plan,
            intents: intents
                .intents
                .iter()
                .map(|x| serde_json::to_value(x).expect("an intent serializes"))
                .collect(),
            queue: Queue::Unavailable("not read by this test".into()),
            release: Value::Null,
            master: None,
            synced_at: String::new(),
            synced_by: String::new(),
        })
        .expect("this repository's board composes");
        for (id, doc) in &composed.documents {
            assert!(size(doc) <= DOCUMENT_LIMIT, "{id} is {} bytes", size(doc));
        }
        let open = plan
            .issues
            .iter()
            .filter(|i| OPEN.contains(&i.status.as_str()))
            .count();
        let filed: usize = composed
            .documents
            .iter()
            .filter(|(id, _)| id.starts_with("issues-"))
            .map(|(_, d)| d["items"].as_array().map_or(0, Vec::len))
            .sum();
        assert_eq!(filed, open, "every open issue is on the board once");
    }
}
