//! The Observatory: `/ui/observatory`, the repository and its mesh drawn as rotating
//! scenes for a wall display.
//!
//! Nothing here is a second model. The screen asks the capabilities the other screens ask
//! ([`SOURCES`]), projects each answer into the small, bounded model one scene draws
//! ([`Projection`]), and puts that projection on the page as data. The script
//! (`share/ui/observatory.js`) draws it and decides nothing: which scenes exist, what each
//! needs, whether it is available and why not are all said here, once, in [`SCENES`].
//!
//! Three rules hold for every scene:
//!
//! - **a mark is a fact or it is decoration.** A node, a pulse, a bar or a ring is drawn from
//!   a value a source answered; the slow background motion is decoration and the page says so;
//! - **absent is not zero.** A scene whose source did not answer, or answered that it is not
//!   running here, is unavailable with the source's own reason, and is never drawn empty;
//! - **what is projected is what the screen may show.** An event is projected as its kind,
//!   its stream and its clock, never its body: an intent's text, a path or a host name stays
//!   in the capability that owns it, behind the link the scene carries.
//!
//! A published snapshot of the UI (ADR 0141) carries this page like any other: the
//! projection is then the one reading taken at the published commit, the script reads nothing
//! again, and the page says it is a snapshot. A process that runs no mesh, which is what
//! reads a snapshot, has no mesh scene to show and says why.

use serde::Serialize;
use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::el;

use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::view::{hero, link, panel, table};

/// The Observatory's route.
pub const ROUTE: &str = "/ui/observatory";

/// The unattended preset: the same screen, the same projection and the same script, told
/// to fill the display and rotate without a reader.
pub const KIOSK: &str = "/ui/kiosk";

/// Earlier and mistyped names, each with the route it is answered with a redirect to: the
/// screensaver is the kiosk under its earlier name, and the misspelling is the screen.
pub const REDIRECTS: &[(&str, &str)] = &[("/ui/screensaver", KIOSK), ("/ui/obsrvatory", ROUTE)];

/// The projection's schema word: what the script checks before it draws.
pub const SCHEMA: &str = "observatory/v1";

/// The script that draws the projection, among the UI's assets.
pub const LIB: &str = "observatory.js";

/// The scenes that read what the repository proves (ADR 0160), drawn with p5 when the build
/// has it. A second file beside [`LIB`]: the director and the first eight scenes are that
/// one's, and this one only registers scenes with it.
pub const SCENES_LIB: &str = "observatory-kiosk.js";

/// The vendored p5, loaded only by a screen that draws one of those scenes. Built by
/// `scripts/cockpit-assets` and not committed; a screen without it says so and its scenes
/// stay available as rows.
pub const P5_LIB: &str = "vendor/p5.min.js";

/// The three scripts a page names, as the digested URLs the server emits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Libs {
    /// The director and the first scenes ([`LIB`]).
    pub director: String,
    /// The scenes of ADR 0160 ([`SCENES_LIB`]).
    pub scenes: String,
    /// The optional drawing library ([`P5_LIB`]).
    pub p5: String,
}

#[cfg(test)]
impl Libs {
    /// The three scripts under one prefix, plain: what a fixture names.
    pub fn under(prefix: &str) -> Libs {
        Libs {
            director: format!("{prefix}{LIB}"),
            scenes: format!("{prefix}{SCENES_LIB}"),
            p5: format!("{prefix}{P5_LIB}"),
        }
    }
}

const PEERS: Source = Source::of("mesh.peers");
const EVENTS: Source = Source::with("mesh.events", r#"{"limit":200,"newest":true}"#);
const STATE: Source = Source::with("mesh.state", r#"{"view":"full"}"#);
const ROADMAP: Source = Source::of("plan.roadmap");
const RELEASES: Source = Source::of("distribution.releases");
const VERSION: Source = Source::of("release.version");
const HEALTH: Source = Source::of("health.report");
const ECONOMICS: Source = Source::of("economics.runs");
const REPOSITORY: Source = Source::of("repository.info");
const RULES: Source = Source::of("rules.report");
const EVIDENCE: Source = Source::of("evidence.report");
const QUALITY: Source = Source::of("quality.report");
const WAVES: Source = Source::of("plan.waves");
const DISTRIBUTION: Source = Source::of("distribution.status");
const CAPABILITIES: Source = Source::of("capabilities.list");
const REPLAY: Source = Source::of("observatory.replay");

/// Every source the Observatory reads, asked together.
pub const SOURCES: &[Source] = &[
    PEERS,
    EVENTS,
    STATE,
    ROADMAP,
    RELEASES,
    VERSION,
    HEALTH,
    ECONOMICS,
    REPOSITORY,
    RULES,
    EVIDENCE,
    QUALITY,
    WAVES,
    DISTRIBUTION,
    CAPABILITIES,
    REPLAY,
];

/// Where a scene's model came from when it is a replay: the recording the repository
/// carries, and when that was recorded.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReplayStanding {
    /// When the recorded process read the scene.
    pub recorded_at: String,
    /// The commit that process stood at, when it said.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// The capabilities the recording names as read, from its manifest.
    pub sources: Vec<String>,
}

/// How many marks one scene carries at most: a wall display is read from metres away, and
/// a projection is bounded whatever the repository holds.
pub const BOUND: usize = 200;

/// Whether a scene's marks are facts or decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Character {
    /// Every mark is drawn from a value a source answered.
    Semantic,
    /// Decorative motion, with the sources' own standing as its only labels.
    Ambient,
}

/// One scene: its identity, what it needs, what its marks mean and how long it is shown.
/// The one inventory of scenes; the page, the script's rotation and the documentation all
/// read this list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scene {
    /// The stable id, also the `scene` query value.
    pub id: &'static str,
    /// The name on the display.
    pub label: &'static str,
    /// Whether its marks are facts.
    pub character: Character,
    /// The sources it cannot be drawn without.
    pub requires: &'static [Source],
    /// What a mark of this scene means, in one sentence: the legend.
    pub meaning: &'static str,
    /// The screen of the UI that holds the same facts as rows.
    pub href: &'static str,
    /// How many seconds the rotation shows it.
    pub dwell_seconds: u32,
}

/// The scenes there are, in rotation order.
pub const SCENES: &[Scene] = &[
    Scene {
        id: "mesh",
        label: "Mesh Cosmos",
        character: Character::Semantic,
        requires: &[PEERS],
        meaning: "A body is a machine the mesh runtime knows; a satellite is one of its runtimes, lit by the liveness the runtime observed. No line is drawn between machines: the source reports none.",
        href: super::mesh::ROUTE,
        dwell_seconds: 45,
    },
    Scene {
        id: "activity",
        label: "Event River",
        character: Character::Semantic,
        requires: &[EVENTS],
        meaning: "A mark is one event of the mesh journal, placed by its time and banded by its kind. A quiet journal is a quiet river.",
        href: super::mesh::ROUTE,
        dwell_seconds: 45,
    },
    Scene {
        id: "intents",
        label: "Intent Nebula",
        character: Character::Semantic,
        requires: &[ROADMAP],
        meaning: "A cluster is a milestone of the plan; its filled arc is the share of its issues that are done, and a line joins a milestone to one it depends on.",
        href: super::roadmap::ROUTE,
        dwell_seconds: 40,
    },
    Scene {
        id: "tokens",
        label: "Token Spectrum",
        character: Character::Semantic,
        requires: &[ECONOMICS],
        meaning: "A band is one recorded run, as wide as the tokens it reports. Without a recorded run there is no spectrum.",
        href: super::evidence::ROUTE,
        dwell_seconds: 30,
    },
    Scene {
        id: "heartbeat",
        label: "System Pulse",
        character: Character::Semantic,
        requires: &[HEALTH],
        meaning: "A ring is one dimension of the health report, coloured by the word its engine decided. Unknown is drawn as unknown, never as healthy.",
        href: super::PREFIX,
        dwell_seconds: 35,
    },
    Scene {
        id: "delivery",
        label: "Delivery Tree",
        character: Character::Semantic,
        requires: &[RELEASES],
        meaning: "A branch is a published release, in the order they were published; the bud at the tip is the version the trunk declares next.",
        href: super::delivery::ROUTE,
        dwell_seconds: 40,
    },
    Scene {
        id: "coop",
        label: "Cooperation Choreography",
        character: Character::Semantic,
        requires: &[STATE],
        meaning: "A dancer is a session of the mesh; a ribbon is a claim it holds, a spark a handover or a review it asked for. Sessions are joined only by what the journal records between them.",
        href: super::mesh::ROUTE,
        dwell_seconds: 40,
    },
    Scene {
        id: "proof",
        label: "Proof Lattice",
        character: Character::Semantic,
        requires: &[RULES],
        meaning: "A cell is one rule of the repository, coloured by the word the proof report decided for it: gated, reviewed, not run, stale or unproven. A rule that no run has proved is never drawn as passing.",
        href: super::governance::ROUTE,
        dwell_seconds: 45,
    },
    Scene {
        id: "claims",
        label: "Claim Ledger",
        character: Character::Semantic,
        requires: &[EVIDENCE],
        meaning: "A row is one kind of claim the documents make; its bar is split by what the evidence says of those claims: not run, stale or without a test. The width is a count of claims.",
        href: super::evidence::ROUTE,
        dwell_seconds: 40,
    },
    Scene {
        id: "surface",
        label: "Surface Atlas",
        character: Character::Semantic,
        requires: &[QUALITY],
        meaning: "An arc is a count the quality report measured: the operations each surface projects, and how much of the public API is documented, shown by an example and tested. Nothing is estimated.",
        href: super::capabilities::ROUTE,
        dwell_seconds: 40,
    },
    Scene {
        id: "waves",
        label: "Wave Front",
        character: Character::Semantic,
        requires: &[WAVES],
        meaning: "A column is a wave of the plan: the issues that may run at the same time once the earlier waves are done. Its height is a count of issues, split by their recorded status.",
        href: super::roadmap::ROUTE,
        dwell_seconds: 40,
    },
    Scene {
        id: "matrix",
        label: "Release Matrix",
        character: Character::Semantic,
        requires: &[DISTRIBUTION],
        meaning: "A tile is one check of the published installation: model, version, release, artifacts, metadata and public site, each in the word the check decided. Unknown is drawn as unknown.",
        href: super::delivery::ROUTE,
        dwell_seconds: 35,
    },
    Scene {
        id: "registry",
        label: "Registry Treemap",
        character: Character::Semantic,
        requires: &[CAPABILITIES],
        meaning: "A tile is one module of the capability registry, as large as the number of capabilities it declares: the tests, issues, documents, claims, rules and decisions the repository is made of, and the operations it serves.",
        href: super::capabilities::ROUTE,
        dwell_seconds: 40,
    },
    Scene {
        id: "ladder",
        label: "Milestone Ladder",
        character: Character::Semantic,
        requires: &[ROADMAP],
        meaning: "A rung is one milestone of the plan; its bar is as long as its issues are many and is split by their recorded status. The number beside it is the share that is done.",
        href: super::roadmap::ROUTE,
        dwell_seconds: 40,
    },
    Scene {
        id: "ambient",
        label: "Ambient Intelligence",
        character: Character::Ambient,
        requires: &[],
        meaning: "Decorative motion. The only facts on this scene are the words beside each source: whether it answered.",
        href: super::PREFIX,
        dwell_seconds: 60,
    },
];

/// One source as the Observatory read it. It carries no link: the page's own source lines
/// link each capability's technical view, and a link inside this document would be a second
/// copy of one that a publication has to lead somewhere (case 2982).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceStanding {
    /// The capability asked.
    pub capability: &'static str,
    /// `answered`, or `failed`.
    pub state: &'static str,
    /// Why it did not answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// How long the executor took, in milliseconds.
    pub took_ms: u64,
}

/// One scene as projected: the descriptor, whether it can be drawn, and the model it draws.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SceneProjection {
    /// The scene's id.
    pub id: &'static str,
    /// Its name.
    pub label: &'static str,
    /// Whether its marks are facts.
    pub character: Character,
    /// The capabilities it requires.
    pub requires: Vec<&'static str>,
    /// The legend.
    pub meaning: &'static str,
    /// Where the same facts are rows.
    pub href: &'static str,
    /// Seconds in the rotation.
    pub dwell_seconds: u32,
    /// Whether there is something true to draw.
    pub available: bool,
    /// Why not, in the source's own words where it gave any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// What the scene draws; `null` when it is unavailable.
    pub model: Value,
    /// Present when the model is the repository's recording and not this process's reading:
    /// a scene of the mesh, drawn where no mesh runs, and labelled a replay.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replay: Option<ReplayStanding>,
}

/// The whole projection: what the page carries and the script draws.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Projection {
    /// [`SCHEMA`].
    pub schema: &'static str,
    /// When the sources were read.
    pub observed_at: String,
    /// The commit the repository stood at, short; absent when version control did not answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Every source, with its standing.
    pub sources: Vec<SourceStanding>,
    /// Every scene, in rotation order.
    pub scenes: Vec<SceneProjection>,
}

/// The answers the Observatory projects from, by source.
type Answers = std::collections::BTreeMap<Source, Answer>;

/// Read every source and project it.
pub fn project(ctx: &Context) -> (Projection, Answers) {
    let answers = ask_all(ctx, SOURCES);
    let projection = project_from(
        &answers,
        crate::peers::rfc3339(std::time::SystemTime::now()),
    );
    (projection, answers)
}

/// The projection of a set of answers: a function of them alone, so the same answers give
/// the same projection.
pub fn project_from(answers: &Answers, observed_at: String) -> Projection {
    let sources = SOURCES
        .iter()
        .filter_map(|s| answers.get(s))
        .map(|a| SourceStanding {
            capability: a.source.capability,
            state: if a.value.is_ok() {
                "answered"
            } else {
                "failed"
            },
            reason: a.value.as_ref().err().cloned(),
            took_ms: a.took.as_millis() as u64,
        })
        .collect::<Vec<_>>();
    let scenes = SCENES
        .iter()
        .map(|scene| {
            let (mut model, mut reason) = match unanswered(scene, answers) {
                Some(reason) => (None, Some(reason)),
                None => match model(scene.id, answers, &sources) {
                    Ok(model) => (Some(model), None),
                    Err(reason) => (None, Some(reason)),
                },
            };
            // a scene of the mesh in a process that runs none may draw the recording the
            // repository carries, as a replay; a source that failed is not "no mesh here"
            let mut replay = None;
            if model.is_none() && no_mesh_here(scene, answers) {
                if let Some((recorded, standing)) = recorded(scene.id, answers) {
                    model = Some(recorded);
                    reason = None;
                    replay = Some(standing);
                }
            }
            SceneProjection {
                id: scene.id,
                label: scene.label,
                character: scene.character,
                requires: scene.requires.iter().map(|s| s.capability).collect(),
                meaning: scene.meaning,
                href: scene.href,
                dwell_seconds: scene.dwell_seconds,
                available: model.is_some(),
                reason,
                model: model.unwrap_or(Value::Null),
                replay,
            }
        })
        .collect();
    let revision = answers
        .get(&REPOSITORY)
        .and_then(|a| a.value.as_ref().ok())
        .and_then(|v| v.pointer("/repository/git/head"))
        .and_then(Value::as_str)
        .map(|h| h[..12.min(h.len())].to_string());
    Projection {
        schema: SCHEMA,
        observed_at,
        revision,
        sources,
        scenes,
    }
}

/// The first required source that did not answer, as the reason the scene is unavailable.
fn unanswered(scene: &Scene, answers: &Answers) -> Option<String> {
    scene.requires.iter().find_map(|s| match answers.get(s) {
        None => Some(format!("{} was not asked", s.capability)),
        Some(a) => a
            .value
            .as_ref()
            .err()
            .map(|e| format!("{} did not answer: {e}", s.capability)),
    })
}

fn answer(answers: &Answers, source: Source) -> &Value {
    answers
        .get(&source)
        .and_then(Answer::ok)
        .unwrap_or(&Value::Null)
}

fn items<'a>(value: &'a Value, pointer: &str) -> &'a [Value] {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn word(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// A source that says it is not running here: its own reason, or the plain fact.
/// Whether every source the scene requires answered, and one of them answered that no mesh
/// runs in this process: the one case a recording may stand in for.
fn no_mesh_here(scene: &Scene, answers: &Answers) -> bool {
    crate::capability::builtin::observatory::RECORDED.contains(&scene.id)
        && scene.requires.iter().all(|s| {
            answers.get(s).is_some_and(|a| {
                a.value
                    .as_ref()
                    .is_ok_and(|v| v.get("active") == Some(&Value::Bool(false)))
            })
        })
}

/// The repository's recording of one scene, with when it was recorded.
fn recorded(id: &str, answers: &Answers) -> Option<(Value, ReplayStanding)> {
    let replay = answers.get(&REPLAY)?.value.as_ref().ok()?;
    let recording = replay.get("recording")?;
    let model = recording.get("scenes")?.get(id)?.clone();
    let standing = ReplayStanding {
        recorded_at: recording.get("recorded_at")?.as_str()?.to_string(),
        revision: recording
            .get("revision")
            .and_then(Value::as_str)
            .map(str::to_string),
        sources: recording
            .pointer("/manifest/sources")?
            .as_array()?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
    };
    Some((model, standing))
}

fn inactive(value: &Value, capability: &str) -> Option<String> {
    (value.get("active").and_then(Value::as_bool) != Some(true)).then(|| {
        match value.get("reason").and_then(Value::as_str) {
            Some(reason) => format!("{capability}: {reason}"),
            None => format!("{capability} reports no mesh runtime active in this process"),
        }
    })
}

/// The first twelve characters of an identity: enough to tell two apart on a wall, and the
/// whole of what a mark is labelled with.
fn short(id: &str) -> String {
    id.chars().take(12).collect()
}

/// The last twelve characters of a compound identity (`node-runtime`, `node-runtime/session`):
/// the part that tells two runtimes of one machine apart, which its first characters do not.
fn tail(id: &str) -> String {
    let plain: Vec<char> = id.chars().filter(char::is_ascii_alphanumeric).collect();
    plain[plain.len().saturating_sub(12)..].iter().collect()
}

/// The marks of a page of `mesh.events`, newest first and at most [`BOUND`]: an event is a mark
/// by its identity (stream and sequence), its kind, its node, its logical clock and its time,
/// and never its body. One that states no sequence has no identity and is not drawn under an
/// invented one. The scene and `observatory.events` both project through this, so a mark the
/// stream delivers is the mark the page would have drawn.
pub fn event_marks(listed: &[Value]) -> Vec<Value> {
    listed
        .iter()
        .rev()
        .take(BOUND)
        .filter_map(|e| {
            let stream = word(e, "stream");
            let seq = e.get("seq").and_then(Value::as_u64)?;
            Some(serde_json::json!({
                "id": format!("{stream}:{seq}"),
                "kind": e.pointer("/body/kind").and_then(Value::as_str).unwrap_or("unknown"),
                "node": short(&stream),
                "lamport": e.get("lamport").and_then(Value::as_u64),
                "at": e.get("at").and_then(Value::as_u64),
            }))
        })
        .collect()
}

/// The model one scene draws, or why there is nothing true to draw.
fn model(id: &str, answers: &Answers, sources: &[SourceStanding]) -> Result<Value, String> {
    match id {
        "mesh" => {
            let peers = answer(answers, PEERS);
            if let Some(reason) = inactive(peers, PEERS.capability) {
                return Err(reason);
            }
            let machines = items(peers, "/machines");
            if machines.is_empty() {
                return Err("mesh.peers knows no machine".into());
            }
            let nodes: Vec<Value> = machines
                .iter()
                .take(BOUND)
                .map(|m| {
                    let runtimes: Vec<Value> = items(m, "/runtimes")
                        .iter()
                        .take(BOUND)
                        .map(|r| {
                            serde_json::json!({
                                "id": tail(&word(r, "runtime")),
                                "liveness": word(r, "liveness"),
                                "this": r.get("this_runtime").and_then(Value::as_bool).unwrap_or(false),
                                "sessions": items(r, "/sessions").len(),
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "id": word(m, "node"),
                        "label": word(m, "name"),
                        "local": m.get("local").and_then(Value::as_bool),
                        "runtimes": runtimes,
                    })
                })
                .collect();
            Ok(serde_json::json!({
                "machines": nodes,
                "known": machines.len(),
                "refused": items(peers, "/refused").len(),
            }))
        }
        "activity" => {
            let events = answer(answers, EVENTS);
            if let Some(reason) = inactive(events, EVENTS.capability) {
                return Err(reason);
            }
            let marks = event_marks(items(events, "/events"));
            Ok(serde_json::json!({
                "events": marks,
                "lamport": events.get("lamport").and_then(Value::as_u64),
            }))
        }
        "intents" => {
            let roadmap = answer(answers, ROADMAP);
            let milestones = items(roadmap, "/milestones");
            if milestones.is_empty() {
                return Err("plan.roadmap holds no milestone".into());
            }
            let nodes: Vec<Value> = milestones
                .iter()
                .take(BOUND)
                .map(|m| {
                    let id = word(m, "id");
                    serde_json::json!({
                        "id": id,
                        "label": word(m, "title"),
                        "status": word(m, "status"),
                        "done": m.pointer("/counts/by_status/DONE").and_then(Value::as_u64),
                        "total": m.pointer("/counts/total").and_then(Value::as_u64),
                        "depends_on": items(m, "/depends_on"),
                        "href": format!("{}?milestone={id}", super::roadmap::ROUTE),
                    })
                })
                .collect();
            Ok(serde_json::json!({
                "milestones": nodes,
                "now": roadmap.get("now"),
                "next": roadmap.get("next"),
            }))
        }
        "tokens" => {
            let runs = items(answer(answers, ECONOMICS), "/runs");
            if runs.is_empty() {
                return Err(
                    "economics.runs holds no recorded run, so there is no usage to draw".into(),
                );
            }
            Ok(serde_json::json!({ "runs": runs.iter().take(BOUND).collect::<Vec<_>>() }))
        }
        "heartbeat" => {
            let health = answer(answers, HEALTH);
            let checks = items(health, "/checks");
            if checks.is_empty() {
                return Err("health.report lists no dimension".into());
            }
            Ok(serde_json::json!({
                "status": health.get("status"),
                "checks": checks.iter().take(BOUND).map(|c| serde_json::json!({
                    "id": word(c, "id"),
                    "label": word(c, "title"),
                    "status": word(c, "status"),
                })).collect::<Vec<_>>(),
            }))
        }
        "delivery" => {
            let releases = items(answer(answers, RELEASES), "/releases");
            if releases.is_empty() {
                return Err("distribution.releases records no release".into());
            }
            // the next version is the trunk's own claim, drawn only when its source answered
            let version = answer(answers, VERSION);
            Ok(serde_json::json!({
                "releases": releases.iter().take(BOUND).map(|r| serde_json::json!({
                    "version": word(r, "version"),
                    "published_at": word(r, "published_at"),
                    "yanked": r.get("yanked").and_then(Value::as_bool).unwrap_or(false),
                })).collect::<Vec<_>>(),
                "count": releases.len(),
                "next": version.get("next"),
                "changes": version.get("changes").and_then(Value::as_array).map(Vec::len),
            }))
        }
        "coop" => {
            let state = answer(answers, STATE);
            if let Some(reason) = inactive(state, STATE.capability) {
                return Err(reason);
            }
            let sessions = items(state, "/state/sessions");
            if sessions.is_empty() {
                return Err("mesh.state holds no session".into());
            }
            let of = |list: &[Value], key: &str, session: &str| {
                list.iter().filter(|c| word(c, key) == session).count()
            };
            let claims = items(state, "/state/claims");
            let reviews = items(state, "/state/reviews");
            Ok(serde_json::json!({
                "sessions": sessions.iter().take(BOUND).map(|s| {
                    let key = word(s, "key");
                    serde_json::json!({
                        "id": tail(&key),
                        "node": short(&word(s, "node")),
                        "state": word(s, "state"),
                        "claims": of(claims, "session", &key),
                        "reviews": of(reviews, "session", &key),
                    })
                }).collect::<Vec<_>>(),
                "claims": claims.len(),
                "overlaps": items(state, "/state/overlaps").len(),
                "handovers": items(state, "/state/handovers").len(),
                "reviews": reviews.len(),
            }))
        }
        "proof" => {
            let report = answer(answers, RULES);
            let rules = items(report, "/rules");
            if rules.is_empty() {
                return Err("rules.report lists no rule".into());
            }
            let mut states = std::collections::BTreeMap::<String, u64>::new();
            for r in rules {
                *states.entry(word(r, "state")).or_default() += 1;
            }
            Ok(serde_json::json!({
                "rules": rules.iter().take(BOUND).map(|r| serde_json::json!({
                    "id": r.pointer("/rule/id").and_then(Value::as_str).unwrap_or_default(),
                    "state": word(r, "state"),
                    "class": r.pointer("/rule/class").and_then(Value::as_str),
                })).collect::<Vec<_>>(),
                "total": rules.len(),
                "states": states,
                "verdict": report.get("verdict"),
                "coverage": report.get("coverage"),
            }))
        }
        "claims" => {
            let report = answer(answers, EVIDENCE);
            let claims = items(report, "/claims");
            if claims.is_empty() {
                return Err("evidence.report examined no claim".into());
            }
            // a group per kind of claim, counted by what the evidence says of it; the claims
            // themselves are not marks, there are more of them than a display can hold
            let mut groups =
                std::collections::BTreeMap::<String, std::collections::BTreeMap<String, u64>>::new(
                );
            for c in claims {
                *groups
                    .entry(word(c, "status"))
                    .or_default()
                    .entry(word(c, "state"))
                    .or_default() += 1;
            }
            Ok(serde_json::json!({
                "groups": groups.iter().take(BOUND).map(|(status, states)| serde_json::json!({
                    "status": status,
                    "total": states.values().sum::<u64>(),
                    "states": states,
                })).collect::<Vec<_>>(),
                "total": claims.len(),
                "examined": report.pointer("/subject/examined"),
                "complete": report.pointer("/subject/complete"),
            }))
        }
        "surface" => {
            let report = answer(answers, QUALITY);
            let ops = report.pointer("/report/operations");
            let Some(ops) = ops.filter(|o| o.get("canonical").is_some()) else {
                return Err("quality.report states no operation count".into());
            };
            Ok(serde_json::json!({
                "operations": {
                    "canonical": ops.get("canonical"),
                    "cli": ops.get("cli"),
                    "http": ops.get("http"),
                    "openapi": ops.get("openapi"),
                    "mcp": ops.get("mcp"),
                },
                "commands": {
                    "total": ops.get("cli_commands"),
                    "from_capability": ops.get("cli_from_capability"),
                    "local": ops.get("cli_local"),
                },
                "modules": {
                    "total": report.pointer("/report/modules/modules"),
                    "documented": report.pointer("/report/modules/documented"),
                    "exampled": report.pointer("/report/modules/exampled"),
                    "tested": report.pointer("/report/modules/behaviourally_tested"),
                },
                "api": {
                    "items": report.pointer("/report/public_api/items"),
                    "documented": report.pointer("/report/public_api/documented"),
                    "owe_example": report.pointer("/report/public_api/owe_example"),
                    "exampled": report.pointer("/report/public_api/exampled"),
                },
                "target": report.pointer("/report/target"),
            }))
        }
        "waves" => {
            let report = answer(answers, WAVES);
            let waves = items(report, "/waves");
            if waves.is_empty() {
                return Err("plan.waves holds no wave".into());
            }
            Ok(serde_json::json!({
                "waves": waves.iter().take(BOUND).map(|w| {
                    let mut by = std::collections::BTreeMap::<String, u64>::new();
                    for i in items(w, "/issues") {
                        *by.entry(word(i, "status")).or_default() += 1;
                    }
                    serde_json::json!({
                        "wave": w.get("wave"),
                        "issues": items(w, "/issues").len(),
                        "by_status": by,
                    })
                }).collect::<Vec<_>>(),
                "serialised": items(report, "/serialised_by_scope").len(),
            }))
        }
        "matrix" => {
            let status = answer(answers, DISTRIBUTION);
            let checks = items(status, "/checks");
            if checks.is_empty() {
                return Err("distribution.status lists no check".into());
            }
            Ok(serde_json::json!({
                "checks": checks.iter().take(BOUND).map(|c| serde_json::json!({
                    "id": word(c, "id"),
                    "state": word(c, "state"),
                    "observed": c.get("observed"),
                })).collect::<Vec<_>>(),
                "installable": status.get("installable"),
                "stable_tag": status.get("stable_tag"),
                "local_version": status.get("local_version"),
                "required_targets": status.get("required_targets"),
                "published_artifacts": status.get("published_artifacts"),
            }))
        }
        "registry" => {
            let list = answer(answers, CAPABILITIES);
            let caps = items(list, "/capabilities");
            if caps.is_empty() {
                return Err("capabilities.list holds no capability".into());
            }
            let mut modules = std::collections::BTreeMap::<String, u64>::new();
            for c in caps {
                *modules.entry(word(c, "module")).or_default() += 1;
            }
            let summary = list.get("summary");
            let get = |k: &str| summary.and_then(|s| s.get(k));
            Ok(serde_json::json!({
                "modules": modules.iter().take(BOUND).map(|(module, count)| serde_json::json!({
                    "module": module,
                    "count": count,
                })).collect::<Vec<_>>(),
                "total": caps.len(),
                "builtin": get("builtin"),
                "declarative": get("declarative"),
                "by_kind": get("by_kind"),
                "by_stability": get("by_stability"),
                "mcp_tools": get("mcp_tools"),
                "http_routes": get("http_routes"),
                "cli_commands": get("cli_commands"),
            }))
        }
        "ladder" => {
            let milestones = items(answer(answers, ROADMAP), "/milestones");
            if milestones.is_empty() {
                return Err("plan.roadmap holds no milestone".into());
            }
            Ok(serde_json::json!({
                "milestones": milestones.iter().take(BOUND).map(|m| serde_json::json!({
                    "id": word(m, "id"),
                    "title": word(m, "title"),
                    "status": word(m, "status"),
                    "total": m.pointer("/counts/total").and_then(Value::as_u64),
                    "by_status": m.pointer("/counts/by_status"),
                })).collect::<Vec<_>>(),
            }))
        }
        "ambient" => Ok(serde_json::json!({
            "sources": sources.iter().map(|s| serde_json::json!({
                "capability": s.capability,
                "state": s.state,
            })).collect::<Vec<_>>(),
        })),
        other => Err(format!("no projection is written for the scene {other}")),
    }
}

/// The Observatory screen: the stage the script draws on, the projection it draws, and the
/// same facts as a table for a reader without the script.
pub fn screen(ctx: &Context, query: &[(String, String)], libs: &Libs, kiosk: bool) -> View {
    // the scene a reader asked for by name; a name that is no scene selects nothing
    let selected = query
        .iter()
        .find(|(k, _)| k == "scene")
        .and_then(|(_, v)| SCENES.iter().find(|s| s.id == v))
        .map(|s| s.id);
    let (projection, answers) = project(ctx);
    let available = projection.scenes.iter().filter(|s| s.available).count();
    let top = hero(
        "eye",
        "violet",
        "Observatory",
        "The repository and its mesh, drawn from what the sources answered.",
        &format!(
            "{available} of {} scenes have something true to draw; the others say why not.",
            projection.scenes.len()
        ),
        Vec::new(),
    );

    let stage = el("section")
        .class("mj-obs")
        .attr("aria-label", "Observatory")
        .attr(
            "data-observatory",
            serde_json::to_string(&projection).unwrap_or_else(|_| "{}".into()),
        )
        .attr("data-observatory-lib", libs.director.as_str())
        .attr("data-observatory-companion", libs.scenes.as_str())
        .attr("data-p5-lib", libs.p5.as_str())
        .attr_if("data-observatory-scene", selected)
        .attr("data-observatory-mode", if kiosk { "kiosk" } else { "screen" })
        .child(
            el("canvas")
                .class("mj-obs-canvas")
                .attr("role", "img")
                .attr(
                    "aria-label",
                    "The scenes listed below, drawn; every fact on it is in the table",
                )
                .flag("hidden"),
        )
        .child(
            el("p")
                .class("mj-obs-noscript mj-ui-dim")
                .text("The scenes are drawn by the page's script. Without it, the table below holds every fact a scene would show."),
        );

    let rows = projection
        .scenes
        .iter()
        .map(|s| {
            el("tr")
                .attr("data-scene", s.id)
                .child(el("td").child(link(format!("{ROUTE}?scene={}", s.id), s.label)))
                .child(el("td").text(match (s.available, s.character) {
                    (true, Character::Semantic) => "available",
                    (true, Character::Ambient) => "decorative",
                    (false, _) => "unavailable",
                }))
                .child(el("td").text(s.reason.clone().unwrap_or_else(|| s.meaning.to_string())))
                .child(el("td").class("mj-ui-mono").text(if s.requires.is_empty() {
                    "none".to_string()
                } else {
                    s.requires.join(", ")
                }))
                .child(el("td").child(link(s.href, "rows")))
        })
        .collect::<Vec<_>>();
    let asked: Vec<&Answer> = SOURCES.iter().filter_map(|s| answers.get(s)).collect();
    let scenes = panel(
        "Scenes",
        "Every scene, whether it has something true to draw, and what its marks mean.",
        table(
            &[
                "Scene",
                "State",
                "Meaning, or why not",
                "Requires",
                "As rows",
            ],
            rows,
            "No scene is declared.",
        ),
        &asked,
    );

    let other = if kiosk {
        link(ROUTE, "Open it as a screen of the UI")
    } else {
        link(
            KIOSK,
            "Open it as a kiosk: the same display, filling the window, unattended",
        )
    };
    View::new(
        if kiosk {
            "Observatory kiosk"
        } else {
            "Observatory"
        },
        el("div")
            .class("mj-ui-stack")
            .child(top)
            .child(stage)
            .child(el("p").class("mj-ui-dim").child(other))
            .child(scenes),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::{Shape, SyntheticRepository};
    use crate::ui::source::{Exposure, Standing};
    use std::time::Duration;

    fn answered(source: Source, value: Result<Value, String>) -> Answer {
        Answer {
            source,
            route: None,
            exposure: Exposure::default(),
            value: value.map(Into::into),
            took: Duration::from_millis(3),
            standing: Standing::NOW,
        }
    }

    fn all(with: &[(Source, Result<Value, String>)]) -> Answers {
        SOURCES
            .iter()
            .map(|s| {
                let value = with
                    .iter()
                    .find(|(w, _)| w == s)
                    .map(|(_, v)| v.clone())
                    .unwrap_or(Ok(serde_json::json!({})));
                (*s, answered(*s, value))
            })
            .collect()
    }

    fn scene<'a>(p: &'a Projection, id: &str) -> &'a SceneProjection {
        p.scenes.iter().find(|s| s.id == id).expect("the scene")
    }

    /// The inventory is one list: ids are unique, every scene but the ambient one requires a
    /// source, and every required source is one the screen asks.
    #[test]
    fn the_scenes_are_declared_once_and_require_what_is_asked() {
        let mut ids: Vec<&str> = SCENES.iter().map(|s| s.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SCENES.len());
        assert_eq!(SCENES.len(), 15);
        for s in SCENES {
            assert_eq!(
                s.requires.is_empty(),
                s.character == Character::Ambient,
                "{}",
                s.id
            );
            for r in s.requires {
                assert!(SOURCES.contains(r), "{} requires an unasked source", s.id);
            }
            assert!(s.dwell_seconds >= 30, "{}", s.id);
        }
    }

    /// A source that says no mesh runs here makes its scenes unavailable with that reason,
    /// and never an empty constellation; a failed source says it failed.
    #[test]
    fn absent_is_unavailable_with_the_sources_reason_and_never_zero() {
        let quiet = serde_json::json!({ "active": false, "reason": "not active: no shared server", "machines": [] });
        let p = project_from(
            &all(&[
                (PEERS, Ok(quiet.clone())),
                (
                    EVENTS,
                    Ok(serde_json::json!({ "active": false, "events": [] })),
                ),
                (STATE, Err("refused".into())),
            ]),
            "2026-10-14T00:00:00Z".into(),
        );
        let mesh = scene(&p, "mesh");
        assert!(!mesh.available && mesh.model.is_null());
        assert_eq!(
            mesh.reason.as_deref(),
            Some("mesh.peers: not active: no shared server")
        );
        assert!(scene(&p, "activity")
            .reason
            .as_deref()
            .unwrap()
            .contains("no mesh runtime active"));
        assert_eq!(
            scene(&p, "coop").reason.as_deref(),
            Some("mesh.state did not answer: refused")
        );
        assert!(scene(&p, "tokens")
            .reason
            .as_deref()
            .unwrap()
            .contains("no recorded run"));
        // decoration needs no source, and says which sources answered
        let ambient = scene(&p, "ambient");
        assert!(ambient.available);
        assert_eq!(
            ambient.model["sources"].as_array().map(Vec::len),
            Some(SOURCES.len())
        );
        let failed = p
            .sources
            .iter()
            .find(|s| s.capability == "mesh.state")
            .unwrap();
        assert_eq!(
            (failed.state, failed.reason.as_deref()),
            ("failed", Some("refused"))
        );
    }

    /// Where no mesh runs, a scene of the mesh draws the repository's recording and says it
    /// is a replay; where the source failed, or a mesh runs, the recording is not used.
    #[test]
    fn a_recording_stands_in_only_where_no_mesh_runs_and_says_so() {
        let quiet =
            serde_json::json!({ "active": false, "reason": "no shared server", "machines": [] });
        let recording = serde_json::json!({ "present": true, "path": "x", "recording": {
            "schema": "observatory-recording/v1", "recorded_at": "2026-10-14T05:00:00Z", "revision": "abc",
            "scenes": { "mesh": { "machines": [{ "id": "m1", "label": "machine 1", "runtimes": [] }], "known": 1, "refused": 0 } },
            "manifest": { "sources": ["mesh.peers"], "removed": ["identities"] },
        }});
        let p = project_from(
            &all(&[(PEERS, Ok(quiet.clone())), (REPLAY, Ok(recording.clone()))]),
            "2026-10-14T06:00:00Z".into(),
        );
        let mesh = scene(&p, "mesh");
        assert!(mesh.available && mesh.reason.is_none());
        assert_eq!(mesh.model["machines"][0]["label"], "machine 1");
        assert_eq!(
            mesh.replay.as_ref().map(|r| r.recorded_at.as_str()),
            Some("2026-10-14T05:00:00Z")
        );
        assert_eq!(
            mesh.replay.as_ref().map(|r| r.sources.clone()),
            Some(vec!["mesh.peers".to_string()])
        );
        // a scene the recording does not carry stays unavailable, with its source's reason
        assert!(!scene(&p, "activity").available && scene(&p, "activity").replay.is_none());
        // a source that failed is not "no mesh here"
        let failed = project_from(
            &all(&[(PEERS, Err("refused".into())), (REPLAY, Ok(recording))]),
            "2026-10-14T06:00:00Z".into(),
        );
        assert!(!scene(&failed, "mesh").available);
        assert!(scene(&failed, "mesh").replay.is_none());
    }

    /// What a source answered is what the scene draws: machines with their runtimes'
    /// liveness, events as kind, stream and clock, and nothing of an event's body.
    #[test]
    fn a_scene_draws_what_its_source_answered_and_no_more() {
        let peers = serde_json::json!({
            "active": true,
            "machines": [{
                "node": "aa11", "name": "aa11", "local": true,
                "runtimes": [
                    { "runtime": "aa11-01", "this_runtime": true, "liveness": "live", "sessions": [{}, {}] },
                    { "runtime": "aa11-02", "liveness": "expired", "sessions": [] },
                ],
            }],
            "refused": [{}],
        });
        let events = serde_json::json!({
            "active": true, "lamport": 9,
            "events": [
                { "stream": "aa11-01", "seq": 1, "lamport": 8, "at": 100,
                  "body": { "kind": "session_opened", "info": { "intent": "a private sentence" } } },
                { "stream": "aa11-01", "seq": 2, "lamport": 9, "at": 101, "body": { "kind": "claim" } },
            ],
        });
        let answers = all(&[(PEERS, Ok(peers)), (EVENTS, Ok(events))]);
        let p = project_from(&answers, "2026-10-14T00:00:00Z".into());
        let mesh = &scene(&p, "mesh").model;
        assert_eq!(mesh["known"], 1);
        assert_eq!(mesh["refused"], 1);
        assert_eq!(mesh["machines"][0]["runtimes"][0]["liveness"], "live");
        assert_eq!(mesh["machines"][0]["runtimes"][0]["sessions"], 2);
        assert_eq!(mesh["machines"][0]["runtimes"][1]["liveness"], "expired");
        // two runtimes of one machine are two marks
        assert_ne!(
            mesh["machines"][0]["runtimes"][0]["id"],
            mesh["machines"][0]["runtimes"][1]["id"]
        );
        let river = &scene(&p, "activity").model;
        assert_eq!(river["lamport"], 9);
        // an event that states no sequence has no identity, and is no mark
        let unnumbered = serde_json::json!({
            "active": true, "lamport": 3,
            "events": [{ "stream": "aa11-01", "lamport": 3, "body": { "kind": "claim" } }],
        });
        let q = project_from(
            &all(&[(EVENTS, Ok(unnumbered))]),
            "2026-10-14T00:00:00Z".into(),
        );
        assert_eq!(
            scene(&q, "activity").model["events"]
                .as_array()
                .map(Vec::len),
            Some(0)
        );
        // newest first, each with a stable identity
        assert_eq!(river["events"][0]["id"], "aa11-01:2");
        assert_eq!(river["events"][1]["kind"], "session_opened");
        let text = serde_json::to_string(&p).unwrap();
        assert!(
            !text.contains("a private sentence"),
            "an event's body was projected"
        );
        // the same answers give the same projection
        assert_eq!(p, project_from(&answers, "2026-10-14T00:00:00Z".into()));
    }

    /// The scenes of ADR 0160 draw counts the sources reported and nothing they did not: the
    /// proof lattice's cells are the rules, its states are tallied from them, a claim ledger
    /// groups claims rather than drawing each, and a source with nothing in it is a scene that
    /// is not drawn, with the reason.
    #[test]
    fn the_proof_scenes_are_counted_from_what_the_sources_answered() {
        let at = || "2026-10-14T00:00:00Z".to_string();
        let answers = all(&[
            (
                RULES,
                Ok(serde_json::json!({
                    "verdict": "unproven",
                    "rules": [
                        {"rule": {"id": "a.one", "class": "blocking"}, "state": "gated"},
                        {"rule": {"id": "a.two", "class": "advisory"}, "state": "not_run"},
                        {"rule": {"id": "a.three", "class": "blocking"}, "state": "not_run"},
                    ],
                })),
            ),
            (
                EVIDENCE,
                Ok(serde_json::json!({
                    "subject": {"examined": 3, "complete": false},
                    "claims": [
                        {"status": "guaranteed", "state": "not_run"},
                        {"status": "guaranteed", "state": "stale"},
                        {"status": "planned", "state": "no_test"},
                    ],
                })),
            ),
            (
                QUALITY,
                Ok(serde_json::json!({"report": {
                    "operations": {"canonical": 10, "cli": 6, "http": 9, "openapi": 9, "mcp": 8,
                                   "cli_commands": 12, "cli_from_capability": 6, "cli_local": 6},
                    "modules": {"modules": 4, "documented": 4, "exampled": 2,
                                "behaviourally_tested": 3},
                    "public_api": {"items": 20, "documented": 20, "owe_example": 8, "exampled": 5},
                    "target": "apps/x",
                }})),
            ),
            (
                WAVES,
                Ok(serde_json::json!({
                    "waves": [
                        {"wave": 0, "issues": [{"status": "DONE"}, {"status": "READY"}]},
                        {"wave": 1, "issues": [{"status": "PLANNED"}]},
                    ],
                    "serialised_by_scope": [{}, {}],
                })),
            ),
            (
                CAPABILITIES,
                Ok(serde_json::json!({
                    "summary": {"builtin": 2, "declarative": 3, "mcp_tools": 2},
                    "capabilities": [
                        {"module": "test"}, {"module": "test"}, {"module": "issue"},
                        {"module": "plan"}, {"module": "plan"},
                    ],
                })),
            ),
            (
                ROADMAP,
                Ok(serde_json::json!({"milestones": [
                    {"id": "M1", "title": "one", "status": "DONE",
                     "counts": {"total": 4, "by_status": {"DONE": 4}}},
                ]})),
            ),
            (
                DISTRIBUTION,
                Ok(serde_json::json!({
                    "installable": true, "stable_tag": "v1", "local_version": "0.9",
                    "required_targets": 6, "published_artifacts": 6,
                    "checks": [{"id": "model", "state": "ok"}, {"id": "public", "state": "unknown"}],
                })),
            ),
        ]);
        let p = project_from(&answers, at());
        let proof = scene(&p, "proof");
        assert!(proof.available);
        assert_eq!(proof.model["total"], 3);
        assert_eq!(proof.model["states"]["not_run"], 2);
        assert_eq!(proof.model["states"]["gated"], 1);
        assert_eq!(proof.model["rules"].as_array().unwrap().len(), 3);
        let claims = scene(&p, "claims");
        assert_eq!(claims.model["total"], 3);
        assert_eq!(claims.model["groups"][0]["status"], "guaranteed");
        assert_eq!(claims.model["groups"][0]["states"]["stale"], 1);
        let surface = scene(&p, "surface");
        assert_eq!(surface.model["operations"]["canonical"], 10);
        assert_eq!(surface.model["modules"]["tested"], 3);
        assert_eq!(surface.model["api"]["owe_example"], 8);
        let waves = scene(&p, "waves");
        assert_eq!(waves.model["waves"][0]["issues"], 2);
        assert_eq!(waves.model["waves"][0]["by_status"]["DONE"], 1);
        assert_eq!(waves.model["serialised"], 2);
        let registry = scene(&p, "registry");
        assert_eq!(registry.model["total"], 5);
        assert_eq!(registry.model["modules"][2]["module"], "test");
        assert_eq!(registry.model["modules"][2]["count"], 2);
        assert_eq!(registry.model["builtin"], 2);
        let ladder = scene(&p, "ladder");
        assert_eq!(ladder.model["milestones"][0]["total"], 4);
        assert_eq!(ladder.model["milestones"][0]["by_status"]["DONE"], 4);
        let matrix = scene(&p, "matrix");
        assert_eq!(matrix.model["checks"][1]["state"], "unknown");
        assert_eq!(matrix.model["required_targets"], 6);

        // nothing in a source is a scene that is not drawn, never an empty picture
        let empty = all(&[
            (RULES, Ok(serde_json::json!({"rules": []}))),
            (EVIDENCE, Ok(serde_json::json!({"claims": []}))),
            (QUALITY, Ok(serde_json::json!({"report": {}}))),
            (WAVES, Ok(serde_json::json!({"waves": []}))),
            (CAPABILITIES, Ok(serde_json::json!({"capabilities": []}))),
            (ROADMAP, Ok(serde_json::json!({"milestones": []}))),
            (DISTRIBUTION, Err("no network".into())),
        ]);
        let p = project_from(&empty, at());
        for id in [
            "proof", "claims", "surface", "waves", "matrix", "registry", "ladder",
        ] {
            let s = scene(&p, id);
            assert!(!s.available, "{id}");
            assert_eq!(s.model, Value::Null, "{id}");
            assert!(s.reason.is_some(), "{id}");
        }
    }

    /// The screen renders in a process that runs no mesh, which is what reads a published
    /// snapshot: the page carries the projection and the script's address, every scene is a
    /// row, and the mesh scenes say why they are not drawn.
    #[test]
    fn the_screen_carries_the_projection_and_every_scene_as_a_row() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let libs = Libs {
            director: "/ui/assets/observatory.js?v=1".into(),
            scenes: "/ui/assets/observatory-kiosk.js?v=1".into(),
            p5: "/ui/assets/vendor/p5.min.js".into(),
        };
        let html = screen(&ctx, &[], &libs, false).main.render();
        assert!(html.contains("data-observatory=\"{"), "{html}");
        assert!(html.contains("data-observatory-lib=\"/ui/assets/observatory.js?v=1\""));
        assert!(html.contains("data-observatory-companion=\"/ui/assets/observatory-kiosk.js?v=1\""));
        assert!(html.contains("data-p5-lib=\"/ui/assets/vendor/p5.min.js\""));
        assert!(html.contains(SCHEMA));
        for s in SCENES {
            assert!(
                html.contains(&format!("data-scene=\"{}\"", s.id)),
                "{}",
                s.id
            );
            assert!(html.contains(s.label), "{}", s.label);
        }
        assert!(!html.contains("data-observatory-scene"));
        let chosen = |v: &str| {
            screen(
                &ctx,
                &[("scene".into(), v.into())],
                &Libs::under("x/"),
                false,
            )
            .main
            .render()
        };
        assert!(chosen("delivery").contains("data-observatory-scene=\"delivery\""));
        assert!(!chosen("no-such-scene").contains("data-observatory-scene"));
        // the kiosk is the same screen with one word changed
        assert!(html.contains("data-observatory-mode=\"screen\""));
        let kiosk = screen(&ctx, &[], &Libs::under("x/"), true).main.render();
        assert!(kiosk.contains("data-observatory-mode=\"kiosk\""));
        assert!(
            kiosk.contains(&format!("href=\"{ROUTE}\""))
                && html.contains(&format!("href=\"{KIOSK}\""))
        );
        let (p, _) = project(&ctx);
        assert!(!scene(&p, "mesh").available);
        assert!(
            scene(&p, "heartbeat").available,
            "{:?}",
            scene(&p, "heartbeat").reason
        );
        assert!(scene(&p, "ambient").available);
        for s in SOURCES {
            assert!(html.contains(s.capability), "{} is not named", s.capability);
        }
        // the document holds no link into another surface; the source lines below it do
        let carried = serde_json::to_string(&p).unwrap();
        assert!(!carried.contains("/cockpit"), "{carried}");
    }
}
