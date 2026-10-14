//! The `observatory` module: what the Observatory (`/ui/observatory`) may carry to a place
//! where no mesh runs, and the recording the repository carries there (ADR 0158, I2678).
//!
//! The reader of a published snapshot is a process that runs no mesh, so the three scenes of
//! the mesh have nothing live to draw on the public site. They may draw a **recording**: the
//! scenes of a process that did run one, captured at a moment, made fit for publication, and
//! committed. A recording is drawn only where no mesh runs, and it is labelled a replay with
//! the time it was recorded; it is never drawn as live.
//!
//! - `observatory.capture` answers the recording this process would make now: the mesh
//!   scenes it has live, pseudonymized. It writes nothing; whoever publishes it commits it.
//! - `observatory.replay` answers the recording the repository carries at [`RECORDING`].
//!
//! Fit for publication means that a recording names no machine, runtime, session or event by
//! its identity: machines are `machine 1`, `machine 2` in the order the mesh listed them,
//! runtimes and sessions are ordinals, and an event keeps only its kind, its logical clock and
//! its time. What remains is the shape of the cooperation, which is what the scene draws.

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::{capability, module};

use super::{get, mcp, Empty};

/// Where the repository carries its recording, relative to the repository root.
pub const RECORDING: &str = ".ai/repo/observatory/recording.json";

/// The schema word of a recording.
pub const RECORDING_SCHEMA: &str = "observatory-recording/v1";

/// The scenes a recording may carry: the ones that need a mesh.
pub const RECORDED: &[&str] = &["mesh", "activity", "coop"];

/// The mesh scenes of one process at one moment, fit for publication.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ObservatoryRecording {
    /// [`RECORDING_SCHEMA`].
    pub schema: String,
    /// When the scenes were read.
    pub recorded_at: String,
    /// The commit the recording process stood at, short; absent when it could not say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Each recorded scene's model, by scene id; a scene the process had nothing live for is
    /// absent, never empty.
    pub scenes: BTreeMap<String, Value>,
    /// What the recording is a record of, and what it leaves out.
    pub manifest: RecordingManifest,
}

/// What a recording is a record of: the capabilities read, the window of events it holds,
/// and what was removed from what they answered, with the reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecordingManifest {
    /// The capabilities whose answers the recorded scenes were projected from.
    pub sources: Vec<String>,
    /// The first and last event the recording holds, by time and by logical clock; absent
    /// when it holds none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<RecordingWindow>,
    /// Every kind of thing removed before publication, and why.
    pub removed: Vec<String>,
}

/// The events a recording holds, from first to last.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecordingWindow {
    /// How many events.
    pub events: usize,
    /// The earliest event's time, in seconds since the epoch.
    pub from_at: Option<u64>,
    /// The latest event's time, in seconds since the epoch.
    pub to_at: Option<u64>,
    /// The smallest logical clock.
    pub from_lamport: Option<u64>,
    /// The largest logical clock.
    pub to_lamport: Option<u64>,
}

/// What every recording removes, and why: the manifest states it rather than leaving a reader
/// to infer it from what is missing.
pub const REMOVED: &[&str] = &[
    "machine, runtime and session identities: replaced by ordinals, because a node id names a machine and a session key names a person's work",
    "machine names: replaced by `machine N` in the order the mesh listed them",
    "event bodies: an intent, a path or a message is the capability's to show to a reader of the repository, not a public page's",
    "event identities (stream and sequence): replaced by ordinals in the recorded order",
];

/// The manifest of a set of redacted scenes.
fn manifest(scenes: &BTreeMap<String, Value>) -> RecordingManifest {
    let sources = ["mesh.peers", "mesh.events", "mesh.state"]
        .into_iter()
        .zip(RECORDED)
        .filter(|(_, scene)| scenes.contains_key(**scene))
        .map(|(capability, _)| capability.to_string())
        .collect();
    let events: Vec<&Value> = scenes
        .get("activity")
        .and_then(|a| a.get("events"))
        .and_then(Value::as_array)
        .map(|v| v.iter().collect())
        .unwrap_or_default();
    let of = |key: &str| {
        events
            .iter()
            .filter_map(|e| e.get(key).and_then(Value::as_u64))
            .collect::<Vec<_>>()
    };
    let (at, lamport) = (of("at"), of("lamport"));
    let window = (!events.is_empty()).then(|| RecordingWindow {
        events: events.len(),
        from_at: at.iter().min().copied(),
        to_at: at.iter().max().copied(),
        from_lamport: lamport.iter().min().copied(),
        to_lamport: lamport.iter().max().copied(),
    });
    RecordingManifest {
        sources,
        window,
        removed: REMOVED.iter().map(|r| r.to_string()).collect(),
    }
}

/// The recording the repository carries, or that it carries none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ObservatoryReplay {
    /// Whether the repository carries a recording.
    pub present: bool,
    /// The file it is read from.
    pub path: String,
    /// The recording.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording: Option<ObservatoryRecording>,
}

/// The scenes of a projection made fit for publication: identities replaced by ordinals,
/// event bodies and names never carried. A scene this does not know is dropped.
///
/// ```
/// use majordomus_cli::capability::builtin::observatory::redact;
/// use serde_json::json;
/// let scenes = [("mesh".to_string(), json!({
///     "machines": [{ "id": "641bdb94dae9f622", "label": "641bdb94", "local": true,
///                    "runtimes": [{ "id": "b10f19aa2cd888a6", "liveness": "live", "this": true, "sessions": 2 }] }],
///     "known": 1, "refused": 0,
/// }))].into_iter().collect();
/// let out = redact(&scenes);
/// let text = serde_json::to_string(&out).unwrap();
/// assert!(!text.contains("641bdb94") && !text.contains("b10f19aa"));
/// assert_eq!(out["mesh"]["machines"][0]["label"], "machine 1");
/// assert_eq!(out["mesh"]["machines"][0]["runtimes"][0]["liveness"], "live");
/// ```
pub fn redact(scenes: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    let list = |v: &Value, key: &str| {
        v.get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let prefix = |id: &str| id.chars().take(12).collect::<String>();
    // a machine is named by the order the mesh listed it, and every node prefix maps to it
    let mut machines: BTreeMap<String, String> = BTreeMap::new();
    let mut out = BTreeMap::new();
    if let Some(mesh) = scenes.get("mesh") {
        let listed = list(mesh, "machines");
        let redacted: Vec<Value> = listed
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let label = format!("machine {}", i + 1);
                let id = m.get("id").and_then(Value::as_str).unwrap_or_default();
                machines.insert(prefix(id), label.clone());
                let runtimes: Vec<Value> = list(m, "runtimes")
                    .iter()
                    .enumerate()
                    .map(|(j, r)| {
                        serde_json::json!({
                            "id": format!("m{}-r{}", i + 1, j + 1),
                            "liveness": r.get("liveness").cloned().unwrap_or(Value::Null),
                            "this": r.get("this").cloned().unwrap_or(Value::Bool(false)),
                            "sessions": r.get("sessions").cloned().unwrap_or(Value::Null),
                        })
                    })
                    .collect();
                serde_json::json!({
                    "id": format!("m{}", i + 1),
                    "label": label,
                    "local": m.get("local").cloned().unwrap_or(Value::Null),
                    "runtimes": runtimes,
                })
            })
            .collect();
        out.insert(
            "mesh".to_string(),
            serde_json::json!({
                "machines": redacted,
                "known": mesh.get("known").cloned().unwrap_or(Value::Null),
                "refused": mesh.get("refused").cloned().unwrap_or(Value::Null),
            }),
        );
    }
    let machine = |node: Option<&str>| match node.map(prefix).and_then(|p| machines.get(&p)) {
        Some(label) => Value::String(label.clone()),
        None => Value::String("another machine".into()),
    };
    if let Some(activity) = scenes.get("activity") {
        let events: Vec<Value> = list(activity, "events")
            .iter()
            .enumerate()
            .map(|(k, e)| {
                serde_json::json!({
                    "id": format!("e{}", k + 1),
                    "kind": e.get("kind").cloned().unwrap_or(Value::Null),
                    "node": machine(e.get("node").and_then(Value::as_str)),
                    "lamport": e.get("lamport").cloned().unwrap_or(Value::Null),
                    "at": e.get("at").cloned().unwrap_or(Value::Null),
                })
            })
            .collect();
        out.insert(
            "activity".to_string(),
            serde_json::json!({
                "events": events,
                "lamport": activity.get("lamport").cloned().unwrap_or(Value::Null),
            }),
        );
    }
    if let Some(coop) = scenes.get("coop") {
        let sessions: Vec<Value> = list(coop, "sessions")
            .iter()
            .enumerate()
            .map(|(k, s)| {
                serde_json::json!({
                    "id": format!("s{}", k + 1),
                    "node": machine(s.get("node").and_then(Value::as_str)),
                    "state": s.get("state").cloned().unwrap_or(Value::Null),
                    "claims": s.get("claims").cloned().unwrap_or(Value::Null),
                    "reviews": s.get("reviews").cloned().unwrap_or(Value::Null),
                })
            })
            .collect();
        let mut model = serde_json::json!({ "sessions": sessions });
        for key in ["claims", "overlaps", "handovers", "reviews"] {
            model[key] = coop.get(key).cloned().unwrap_or(Value::Null);
        }
        out.insert("coop".to_string(), model);
    }
    out
}

fn capture(ctx: &Context, _: Empty) -> Result<ObservatoryRecording, CapabilityError> {
    let (projection, _) = crate::ui::observatory::project(ctx);
    // only what this process has live: a scene drawn from a replay is never recorded again
    let live: BTreeMap<String, Value> = projection
        .scenes
        .iter()
        .filter(|s| RECORDED.contains(&s.id) && s.available && s.replay.is_none())
        .map(|s| (s.id.to_string(), s.model.clone()))
        .collect();
    let scenes = redact(&live);
    Ok(ObservatoryRecording {
        schema: RECORDING_SCHEMA.into(),
        recorded_at: projection.observed_at,
        revision: projection.revision,
        manifest: manifest(&scenes),
        scenes,
    })
}

/// The recording at [`RECORDING`] under `root`. Absent is `present: false`; a file that is
/// there and cannot be read or is not a recording is an error, never an empty answer.
pub fn replay_at(root: &Path) -> Result<ObservatoryReplay, CapabilityError> {
    let path = root.join(RECORDING);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ObservatoryReplay {
                present: false,
                path: RECORDING.into(),
                recording: None,
            })
        }
        Err(e) => {
            return Err(CapabilityError::Internal(format!(
                "{RECORDING} could not be read: {e}"
            )))
        }
    };
    let recording: ObservatoryRecording = serde_json::from_str(&text)
        .map_err(|e| CapabilityError::Internal(format!("{RECORDING} is not a recording: {e}")))?;
    if recording.schema != RECORDING_SCHEMA {
        return Err(CapabilityError::Internal(format!(
            "{RECORDING} declares schema {}, not {RECORDING_SCHEMA}",
            recording.schema
        )));
    }
    if let Some(other) = recording
        .scenes
        .keys()
        .find(|k| !RECORDED.contains(&k.as_str()))
    {
        return Err(CapabilityError::Internal(format!(
            "{RECORDING} carries the scene {other}, which is not one a recording may carry"
        )));
    }
    // an event is drawn by its kind, clock and time; one that lacks any of them was not
    // recorded from a source record, and the recording is refused naming it
    let events = recording
        .scenes
        .get("activity")
        .and_then(|a| a.get("events"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (i, e) in events.iter().enumerate() {
        let whole = e.get("kind").is_some_and(Value::is_string)
            && e.get("lamport").is_some_and(Value::is_u64)
            && e.get("at").is_some_and(Value::is_u64);
        if !whole {
            let id = e
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("#{}", i + 1));
            return Err(CapabilityError::Internal(format!(
                "{RECORDING} holds the event {id} without its kind, logical clock and time: it was not recorded from a source record"
            )));
        }
    }
    Ok(ObservatoryReplay {
        present: true,
        path: RECORDING.into(),
        recording: Some(recording),
    })
}

fn replay(ctx: &Context, _: Empty) -> Result<ObservatoryReplay, CapabilityError> {
    replay_at(Path::new(&ctx.index.repository.root))
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "observatory",
        title: "Observatory",
        description: "What the Observatory may carry to a place where no mesh runs: the mesh scenes this process has live, pseudonymized for publication, and the recording the repository carries, which the published Observatory replays and labels as a replay.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "observatory.capture",
                title: "Observatory capture",
                description: "The mesh scenes of the Observatory as this process has them live, fit for publication: machines, runtimes, sessions and events by ordinal, an event by its kind, logical clock and time only. Writes nothing; committing the answer to .ai/repo/observatory/recording.json is what publishes it.",
                input: Empty,
                output: ObservatoryRecording,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_observatory_capture"), http: get("/api/v1/observatory/capture"), cli: None },
                tags: ["observatory", "mesh", "ui"],
                handler: capture,
            },
            capability! {
                id: "observatory.replay",
                title: "Observatory replay",
                description: "The recording of the mesh scenes the repository carries at .ai/repo/observatory/recording.json, which the Observatory draws, labelled as a replay, where no mesh runs. Absent is present: false; a file that cannot be read is an error.",
                input: Empty,
                output: ObservatoryReplay,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_observatory_replay"), http: get("/api/v1/observatory/replay"), cli: None },
                tags: ["observatory", "mesh", "ui"],
                handler: replay,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scenes() -> BTreeMap<String, Value> {
        [
            (
                "mesh".to_string(),
                json!({ "machines": [
                    { "id": "aaaaaaaaaaaa1111", "label": "aaaaaaaa", "local": true,
                      "runtimes": [{ "id": "rt-secret-1", "liveness": "live", "this": true, "sessions": 1 }] },
                    { "id": "bbbbbbbbbbbb2222", "label": "bbbbbbbb", "local": false, "runtimes": [] },
                ], "known": 2, "refused": 1 }),
            ),
            (
                "activity".to_string(),
                json!({ "events": [
                    { "id": "aaaaaaaaaaaa1111-x:7", "kind": "claim_acquired", "node": "bbbbbbbbbbbb", "lamport": 9, "at": 100 },
                    { "id": "cccc:1", "kind": "session_opened", "node": "cccccccccccc", "lamport": 10, "at": 101 },
                ], "lamport": 10 }),
            ),
            (
                "coop".to_string(),
                json!({ "sessions": [{ "id": "sess-secret", "node": "aaaaaaaaaaaa", "state": "active", "claims": 3, "reviews": 0 }],
                        "claims": 3, "overlaps": 0, "handovers": 1, "reviews": 0 }),
            ),
            ("heartbeat".to_string(), json!({ "checks": [] })),
        ]
        .into_iter()
        .collect()
    }

    /// A recording carries the shape of the cooperation and no identity of it: no id, label
    /// or session key of the input survives, events keep kind, clock and time, and a node is
    /// the machine it was listed as, or "another machine".
    #[test]
    fn a_recording_names_nothing_by_its_identity() {
        let out = redact(&scenes());
        let text = serde_json::to_string(&out).unwrap();
        for secret in ["aaaaaaaa", "bbbbbbbb", "cccc", "rt-secret", "sess-secret"] {
            assert!(!text.contains(secret), "{secret} survived: {text}");
        }
        assert_eq!(out.keys().collect::<Vec<_>>(), ["activity", "coop", "mesh"]);
        assert_eq!(out["mesh"]["machines"][1]["label"], "machine 2");
        assert_eq!(out["mesh"]["machines"][0]["runtimes"][0]["id"], "m1-r1");
        assert_eq!(out["activity"]["events"][0]["node"], "machine 2");
        assert_eq!(out["activity"]["events"][1]["node"], "another machine");
        assert_eq!(out["activity"]["events"][0]["kind"], "claim_acquired");
        assert_eq!(out["activity"]["events"][1]["lamport"], 10);
        assert_eq!(out["coop"]["sessions"][0]["node"], "machine 1");
        assert_eq!(out["coop"]["handovers"], 1);
    }

    /// No file is no recording; a file that is not a recording, or carries a scene a
    /// recording may not, is an error and never an empty answer.
    #[test]
    fn the_replay_is_the_file_or_says_why_not() {
        let dir = tempfile::tempdir().unwrap();
        let absent = replay_at(dir.path()).unwrap();
        assert!(!absent.present && absent.recording.is_none());
        let path = dir.path().join(RECORDING);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not json").unwrap();
        assert!(replay_at(dir.path()).is_err());
        let redacted = redact(&scenes());
        let mut recording = ObservatoryRecording {
            schema: RECORDING_SCHEMA.into(),
            recorded_at: "2026-10-14T05:00:00Z".into(),
            revision: Some("abc".into()),
            manifest: manifest(&redacted),
            scenes: redacted,
        };
        // the manifest names what was read, the window held and what was removed
        assert_eq!(
            recording.manifest.sources,
            ["mesh.peers", "mesh.events", "mesh.state"]
        );
        let window = recording.manifest.window.clone().expect("a window");
        assert_eq!(
            (window.events, window.from_lamport, window.to_lamport),
            (2, Some(9), Some(10))
        );
        assert_eq!((window.from_at, window.to_at), (Some(100), Some(101)));
        assert_eq!(recording.manifest.removed.len(), REMOVED.len());
        std::fs::write(&path, serde_json::to_string(&recording).unwrap()).unwrap();
        let read = replay_at(dir.path()).unwrap();
        assert!(read.present);
        assert_eq!(read.recording.as_ref(), Some(&recording));
        // an event without its clock is refused, by name
        let mut partial = recording.clone();
        partial.scenes.get_mut("activity").unwrap()["events"][1]
            .as_object_mut()
            .unwrap()
            .remove("lamport");
        std::fs::write(&path, serde_json::to_string(&partial).unwrap()).unwrap();
        match replay_at(dir.path()) {
            Err(CapabilityError::Internal(m)) => assert!(m.contains("the event e2 "), "{m}"),
            other => panic!("expected a refusal naming e2, got {other:?}"),
        }
        recording.scenes.insert("heartbeat".into(), json!({}));
        std::fs::write(&path, serde_json::to_string(&recording).unwrap()).unwrap();
        assert!(
            replay_at(dir.path()).is_err(),
            "a recording carries mesh scenes only"
        );
    }
}
