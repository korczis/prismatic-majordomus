//! What a screen of the UI reads, and how it says where each value came from.
//!
//! A screen never computes a fact. It asks capabilities through the executor every other
//! surface calls, and every value it shows is a [`Reading`]: the value at an RFC 6901
//! pointer into one capability's answer, with the capability, the route that serves the
//! same answer over HTTP, how long the answer took and when it was taken. A reader who
//! doubts a number follows the source line to the Cockpit's page for that capability and
//! asks it the same question (ADR 0088, ADR 0135).
//!
//! A source that could not answer is a reading whose state is [`Availability::Failed`],
//! carrying the reason. A pointer that resolves to nothing is [`Availability::Absent`].
//! Neither is ever shown as a zero.

use std::collections::BTreeMap;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use crate::capability::Context;

/// One capability asked with one input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Source {
    /// The capability's id.
    pub capability: &'static str,
    /// Its input, as JSON text, so a source can be a constant.
    pub input: &'static str,
}

impl Source {
    /// A source asked with no input.
    pub const fn of(capability: &'static str) -> Self {
        Source {
            capability,
            input: "{}",
        }
    }

    /// A source asked with `input`.
    pub const fn with(capability: &'static str, input: &'static str) -> Self {
        Source { capability, input }
    }
}

/// What one source answered, and how long it took.
#[derive(Debug, Clone)]
pub struct Answer {
    /// The source.
    pub source: Source,
    /// The HTTP method and route that serve the same answer, as `capabilities.describe`
    /// answers it (`GET /api/v1/health`); `None` for a capability with no HTTP projection.
    pub route: Option<String>,
    /// Every other way to ask the same question, as the capability declares it.
    pub exposure: Exposure,
    /// The answer, or why there is none.
    pub value: Result<Value, String>,
    /// How long the executor took to answer.
    pub took: Duration,
    /// When the answer was taken.
    pub at: SystemTime,
}

impl Answer {
    /// The value at `pointer`, as a reading that names where it came from.
    pub fn read(&self, pointer: &'static str) -> Reading {
        let state = match &self.value {
            Err(reason) => Availability::Failed(reason.clone()),
            Ok(v) => match v.pointer(pointer) {
                Some(Value::Null) | None => Availability::Absent,
                Some(found) => Availability::Available(found.clone()),
            },
        };
        Reading {
            source: self.source,
            pointer,
            state,
        }
    }

    /// The whole answer, when there is one.
    pub fn ok(&self) -> Option<&Value> {
        self.value.as_ref().ok()
    }

    /// One line naming the source: what was asked, over which route, how long it took.
    pub fn provenance(&self) -> String {
        let millis = self.took.as_millis();
        match &self.route {
            Some(route) => format!("{} · {route} · {millis} ms", self.source.capability),
            None => format!("{} · {millis} ms", self.source.capability),
        }
    }
}

/// The ways a capability is exposed besides the UI, read from its declaration through
/// `capabilities.describe`: the HTTP route a reader can open, the command a person types and
/// the MCP tool an agent calls. Each is the capability's own projection, so a value on the
/// UI is never more than one step from the same answer on every other surface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exposure {
    /// The path of a GET route, which a browser can open as it is.
    pub get: Option<String>,
    /// The command line, as `majordomus <path>`.
    pub cli: Option<String>,
    /// The MCP tool's name.
    pub mcp: Option<String>,
}

/// The state of one value on a screen. The vocabulary of the pack's data contract,
/// restricted to the states this surface can actually observe.
#[derive(Debug, Clone, PartialEq)]
pub enum Availability {
    /// The source answered and the pointer holds this value.
    Available(Value),
    /// The source answered and the pointer holds nothing.
    Absent,
    /// The source could not answer, for this reason.
    Failed(String),
}

/// One value, with the source and pointer it was read from.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// The source asked.
    pub source: Source,
    /// Where in its answer the value was read.
    pub pointer: &'static str,
    /// The value, or why there is none.
    pub state: Availability,
}

impl Reading {
    /// The value as display text: a string as it is, anything else as JSON. `None` when
    /// there is no value, so the caller must decide what an absent value reads as.
    pub fn text(&self) -> Option<String> {
        match &self.state {
            Availability::Available(Value::String(s)) => Some(s.clone()),
            Availability::Available(other) => Some(other.to_string()),
            _ => None,
        }
    }

    /// The value as a count, when it is a number.
    pub fn count(&self) -> Option<u64> {
        match &self.state {
            Availability::Available(v) => v.as_u64(),
            _ => None,
        }
    }

    /// `capability pointer`, the short form a tile carries under its value.
    pub fn cite(&self) -> String {
        format!("{} {}", self.source.capability, self.pointer)
    }
}

/// Every source asked at once, each on a thread of its own, so that a screen's first
/// paint costs its slowest source and not the sum of them. The answers are keyed by
/// source; a source asked twice in `sources` is asked once.
pub fn ask_all(ctx: &Context, sources: &[Source]) -> BTreeMap<Source, Answer> {
    let mut distinct: Vec<Source> = Vec::new();
    for s in sources {
        if !distinct.contains(s) {
            distinct.push(*s);
        }
    }
    let answers: Vec<Answer> = std::thread::scope(|scope| {
        let asked: Vec<_> = distinct
            .iter()
            .map(|&source| (source, scope.spawn(move || ask(ctx, source))))
            .collect();
        asked
            .into_iter()
            .map(|(source, thread)| {
                thread.join().unwrap_or_else(|_| {
                    let (route, exposure) = describe(ctx, source.capability);
                    Answer {
                        source,
                        route,
                        exposure,
                        value: Err("the source panicked while answering".into()),
                        took: Duration::ZERO,
                        at: SystemTime::now(),
                    }
                })
            })
            .collect()
    });
    answers.into_iter().map(|a| (a.source, a)).collect()
}

/// One source asked through the executor.
pub fn ask(ctx: &Context, source: Source) -> Answer {
    let at = SystemTime::now();
    let started = Instant::now();
    let value = serde_json::from_str::<Value>(source.input)
        .map_err(|e| {
            format!(
                "the screen's own input for {} does not parse: {e}",
                source.capability
            )
        })
        .and_then(|input| {
            ctx.execute(source.capability, input)
                .map_err(|e| e.to_string())
        });
    let took = started.elapsed();
    let (route, exposure) = describe(ctx, source.capability);
    Answer {
        source,
        route,
        exposure,
        value,
        took,
        at,
    }
}

/// The HTTP method and route a capability is served on, as `capabilities.describe` answers
/// it. The UI asks rather than reading the registry itself (project.ui-derived-state, the
/// ui-integrity gate's direct-read check): what the source line says is what any client
/// asking that capability is told.
fn describe(ctx: &Context, capability: &str) -> (Option<String>, Exposure) {
    let Ok(described) = ctx.execute(
        "capabilities.describe",
        serde_json::json!({ "id": capability }),
    ) else {
        return (None, Exposure::default());
    };
    let at = |p: &str| {
        described
            .pointer(p)
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let method = at("/exposure/http/method");
    let path = at("/exposure/http/path");
    let route = method
        .as_ref()
        .zip(path.as_ref())
        .map(|(m, p)| format!("{m} {p}"));
    let cli = described
        .pointer("/exposure/cli/path")
        .and_then(Value::as_array)
        .map(|words| {
            let words: Vec<&str> = words.iter().filter_map(Value::as_str).collect();
            format!("majordomus {}", words.join(" "))
        });
    let exposure = Exposure {
        get: (method.as_deref() == Some("GET")).then_some(path).flatten(),
        cli,
        mcp: at("/exposure/mcp/tool"),
    };
    (route, exposure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::{Shape, SyntheticRepository};

    fn answer(value: Result<Value, String>) -> Answer {
        Answer {
            source: Source::of("health.report"),
            route: Some("GET /api/v1/health".into()),
            exposure: Exposure::default(),
            value,
            took: Duration::from_millis(12),
            at: SystemTime::now(),
        }
    }

    /// A reading is the value at its pointer, and nothing else: absent when the pointer
    /// holds nothing or null, failed with the reason when the source could not answer.
    #[test]
    fn a_reading_is_the_value_at_its_pointer_or_says_why_not() {
        let a = answer(Ok(
            serde_json::json!({ "status": "warn", "n": 3, "gone": null }),
        ));
        assert_eq!(a.read("/status").text().as_deref(), Some("warn"));
        assert_eq!(a.read("/n").count(), Some(3));
        assert_eq!(a.read("/missing").state, Availability::Absent);
        assert_eq!(a.read("/gone").state, Availability::Absent);
        assert_eq!(a.read("/missing").count(), None, "absent is never a zero");
        let failed = answer(Err("refused".into()));
        assert_eq!(
            failed.read("/status").state,
            Availability::Failed("refused".into())
        );
        assert_eq!(failed.read("/status").text(), None);
    }

    /// The source line names the capability, its route and the time it took.
    #[test]
    fn the_provenance_names_the_capability_the_route_and_the_time() {
        let line = answer(Ok(Value::Null)).provenance();
        assert_eq!(line, "health.report · GET /api/v1/health · 12 ms");
        assert_eq!(
            answer(Ok(Value::Null)).read("/status").cite(),
            "health.report /status"
        );
    }

    /// Asked at once against a real registry: one answer per distinct source, filed under
    /// it, each with the route its declaration names.
    #[test]
    fn every_source_is_asked_once_and_filed_under_itself() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let sources = [
            Source::of("plan.status"),
            Source::of("release.version"),
            Source::of("plan.status"),
            Source::of("no.such.capability"),
        ];
        let answers = ask_all(&ctx, &sources);
        assert_eq!(answers.len(), 3, "a source named twice is asked once");
        let plan = &answers[&Source::of("plan.status")];
        assert_eq!(plan.route.as_deref(), Some("GET /api/v1/plan/status"));
        let missing = &answers[&Source::of("no.such.capability")];
        assert!(
            missing.value.is_err(),
            "an unknown capability is a failure, not a value"
        );
        assert_eq!(missing.route, None);
    }
}
