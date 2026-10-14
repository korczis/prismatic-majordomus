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
use super::view::{hero, link, panel, table, technical};

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

const PEERS: Source = Source::of("mesh.peers");
const EVENTS: Source = Source::with("mesh.events", r#"{"limit":200,"newest":true}"#);
const STATE: Source = Source::with("mesh.state", r#"{"view":"full"}"#);
const ROADMAP: Source = Source::of("plan.roadmap");
const RELEASES: Source = Source::of("distribution.releases");
const VERSION: Source = Source::of("release.version");
const HEALTH: Source = Source::of("health.report");
const ECONOMICS: Source = Source::of("economics.runs");
const REPOSITORY: Source = Source::of("repository.info");

/// Every source the Observatory reads, asked together.
pub const SOURCES: &[Source] = &[
    PEERS, EVENTS, STATE, ROADMAP, RELEASES, VERSION, HEALTH, ECONOMICS, REPOSITORY,
];

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
        id: "ambient",
        label: "Ambient Intelligence",
        character: Character::Ambient,
        requires: &[],
        meaning: "Decorative motion. The only facts on this scene are the words beside each source: whether it answered.",
        href: super::PREFIX,
        dwell_seconds: 60,
    },
];

/// One source as the Observatory read it.
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
    /// The technical view of the capability, where every other way to ask it is listed.
    pub href: String,
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
            href: technical(a.source.capability),
        })
        .collect::<Vec<_>>();
    let scenes = SCENES
        .iter()
        .map(|scene| {
            let (model, reason) = match unanswered(scene, answers) {
                Some(reason) => (None, Some(reason)),
                None => match model(scene.id, answers, &sources) {
                    Ok(model) => (Some(model), None),
                    Err(reason) => (None, Some(reason)),
                },
            };
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
        .and_then(|a| a.value.as_ref().ok())
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
            let listed = items(events, "/events");
            let marks: Vec<Value> = listed
                .iter()
                .rev()
                .take(BOUND)
                // an event is a mark by its identity, stream and sequence; one that states no
                // sequence has none and is not drawn under an invented one
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
                .collect();
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
pub fn screen(ctx: &Context, query: &[(String, String)], lib: &str, kiosk: bool) -> View {
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
        .attr("data-observatory-lib", lib)
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
            value,
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
        assert_eq!(SCENES.len(), 8);
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

    /// The screen renders in a process that runs no mesh, which is what reads a published
    /// snapshot: the page carries the projection and the script's address, every scene is a
    /// row, and the mesh scenes say why they are not drawn.
    #[test]
    fn the_screen_carries_the_projection_and_every_scene_as_a_row() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &[], "/ui/assets/observatory.js?v=1", false)
            .main
            .render();
        assert!(html.contains("data-observatory=\"{"), "{html}");
        assert!(html.contains("data-observatory-lib=\"/ui/assets/observatory.js?v=1\""));
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
            screen(&ctx, &[("scene".into(), v.into())], "x", false)
                .main
                .render()
        };
        assert!(chosen("delivery").contains("data-observatory-scene=\"delivery\""));
        assert!(!chosen("no-such-scene").contains("data-observatory-scene"));
        // the kiosk is the same screen with one word changed
        assert!(html.contains("data-observatory-mode=\"screen\""));
        let kiosk = screen(&ctx, &[], "x", true).main.render();
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
    }
}
