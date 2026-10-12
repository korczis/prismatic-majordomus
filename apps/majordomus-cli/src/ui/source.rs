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
//!
//! A screen does not wait for its sources either. It asks for the *latest reading* of each
//! (`crate::capability::readings`): what this process knows now, handed over at once, read
//! again behind the reader whenever it is not current. So every answer also says how it
//! stands: [`Standing::age`], whether the repository has moved since, and whether a newer
//! reading is on its way. A reading that is not current is shown as what it is.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::Value;

use crate::capability::readings::{Freshness, Latest};
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
    /// How the answer stands against the repository as it is now.
    pub standing: Standing,
}

/// How an answer stands against the repository as it is now: when it was read, and whether
/// a newer reading is on its way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    /// How long ago the answer was read; `None` for a first reading that has not arrived.
    pub age: Option<Duration>,
    /// Read in the repository as it is now, before it last moved, or not yet at all.
    pub freshness: Freshness,
    /// Whether the source is being read again at this moment.
    pub refreshing: bool,
}

impl Standing {
    /// An answer taken just now.
    pub const NOW: Standing = Standing {
        age: Some(Duration::ZERO),
        freshness: Freshness::Current,
        refreshing: false,
    };

    fn of(latest: &Latest) -> Standing {
        Standing {
            age: latest.age,
            freshness: latest.freshness,
            refreshing: latest.refreshing,
        }
    }

    /// Whether the screen is to be read again shortly: the answer is not of the repository
    /// as it is now, and a newer one is being read.
    pub fn catching_up(&self) -> bool {
        self.freshness != Freshness::Current
    }

    /// What a reader is told about an answer that is not simply current, or `None` when
    /// there is nothing to tell: a reading older than [`Standing::WORTH_SAYING`], a reading
    /// the repository has moved past, a first reading still on its way.
    pub fn note(&self) -> Option<String> {
        let ago = |age: Duration| match age.as_secs() {
            s if s < 120 => format!("read {s} s ago"),
            s if s < 7200 => format!("read {} min ago", s / 60),
            s => format!("read {} h ago", s / 3600),
        };
        match (self.freshness, self.age) {
            (Freshness::Pending, _) => Some("first reading in progress".into()),
            (Freshness::Stale, Some(age)) => Some(format!(
                "{}, before the repository changed; reading again",
                ago(age)
            )),
            (Freshness::Stale, None) => Some("read before the repository changed".into()),
            (Freshness::Current, Some(age)) if age >= Standing::WORTH_SAYING => Some(ago(age)),
            (Freshness::Current, _) => None,
        }
    }

    /// The age from which a current reading says how old it is.
    pub const WORTH_SAYING: Duration = Duration::from_secs(5);
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
            .map(|(source, thread)| joined(ctx, source, thread.join()))
            .collect()
    });
    answers.into_iter().map(|a| (a.source, a)).collect()
}

/// What a source's thread came back with: its answer, or — when asking it panicked — an
/// answer that says so, so one broken source fails its own panel and not the screen.
fn joined(ctx: &Context, source: Source, thread: std::thread::Result<Answer>) -> Answer {
    match thread {
        Ok(answer) => answer,
        Err(_) => {
            let (route, exposure) = describe(ctx, source.capability);
            Answer {
                source,
                route,
                exposure,
                value: Err("the source panicked while answering".into()),
                took: Duration::ZERO,
                standing: Standing::NOW,
            }
        }
    }
}

/// One capability asked with an input a reader chose (a screen's query), through the
/// executor like any other source. The source line names the capability; the input is the
/// reader's, so it is not a constant of the screen.
pub fn ask_input(ctx: &Context, capability: &'static str, input: Value) -> Answer {
    let latest = ctx.read(capability, input);
    let (route, exposure) = describe(ctx, capability);
    Answer {
        source: Source::of(capability),
        route,
        exposure,
        standing: Standing::of(&latest),
        value: latest.value,
        took: latest.took,
    }
}

/// One source asked through the executor.
pub fn ask(ctx: &Context, source: Source) -> Answer {
    let (route, exposure) = describe(ctx, source.capability);
    let (value, took, standing) = match serde_json::from_str::<Value>(source.input) {
        Err(e) => (
            Err(format!(
                "the screen's own input for {} does not parse: {e}",
                source.capability
            )),
            Duration::ZERO,
            Standing::NOW,
        ),
        Ok(input) => {
            let latest = ctx.read(source.capability, input);
            let standing = Standing::of(&latest);
            (latest.value, latest.took, standing)
        }
    };
    Answer {
        source,
        route,
        exposure,
        value,
        took,
        standing,
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

    /// A source whose thread panicked is a failed answer of its own, named and routed like
    /// any other; the screen around it still renders.
    #[test]
    fn a_source_that_panicked_is_a_failed_answer_not_a_broken_screen() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let source = Source::of("plan.status");
        let a = joined(&ctx, source, Err(Box::new("boom")));
        assert_eq!(a.source, source);
        assert_eq!(
            a.value.as_ref().err().map(String::as_str),
            Some("the source panicked while answering")
        );
        assert!(a.route.is_some(), "a failed source still names its route");
        let ok = joined(&ctx, source, Ok(answer(Ok(serde_json::json!({})))));
        assert!(ok.value.is_ok());
    }

    /// A screen's own input that does not parse is the source's failure, said with the
    /// capability's name; it is never asked with something else instead.
    #[test]
    fn a_source_whose_input_does_not_parse_fails_and_says_which() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let broken = Source::with("plan.status", "{not json");
        assert_eq!(broken.input, "{not json");
        let a = ask(&ctx, broken);
        let reason = a.value.expect_err("an input that does not parse");
        assert!(
            reason.contains("plan.status") && reason.contains("does not parse"),
            "{reason}"
        );
        let refused = ask_input(&ctx, "no.such.capability", serde_json::json!({}));
        assert!(refused.value.is_err(), "an unknown capability is refused");
    }

    /// The provenance line names the route when there is one, and only the capability and
    /// its time when there is none; a value that is not a string is shown as JSON.
    #[test]
    fn a_provenance_without_a_route_names_the_capability_alone() {
        let mut a = answer(Ok(serde_json::json!({ "n": 3, "on": true })));
        assert!(a.provenance().contains("GET /api/v1/health"));
        a.route = None;
        assert_eq!(a.provenance(), "health.report · 12 ms");
        assert_eq!(a.read("/on").text().as_deref(), Some("true"));
        assert_eq!(a.read("/n").text().as_deref(), Some("3"));
    }

    fn answer(value: Result<Value, String>) -> Answer {
        Answer {
            source: Source::of("health.report"),
            route: Some("GET /api/v1/health".into()),
            exposure: Exposure::default(),
            value,
            took: Duration::from_millis(12),
            standing: Standing::NOW,
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

    /// How an answer stands is said when there is something to say: an age worth saying, a
    /// reading the repository has moved past, a first reading on its way. A stale or pending
    /// reading is catching up; a current one never is, however old.
    #[test]
    fn a_standing_says_its_age_when_it_matters_and_never_calls_a_stale_reading_current() {
        let standing = |freshness, seconds: Option<u64>| Standing {
            age: seconds.map(Duration::from_secs),
            freshness,
            refreshing: true,
        };
        assert_eq!(Standing::NOW.note(), None);
        assert!(!Standing::NOW.catching_up());
        let young = standing(Freshness::Current, Some(4));
        assert_eq!(young.note(), None, "younger than WORTH_SAYING");
        let older = standing(Freshness::Current, Some(12));
        assert_eq!(older.note().as_deref(), Some("read 12 s ago"));
        assert!(!older.catching_up(), "old is not stale");
        assert_eq!(
            standing(Freshness::Current, Some(600)).note().as_deref(),
            Some("read 10 min ago")
        );
        assert_eq!(
            standing(Freshness::Current, Some(3 * 3600))
                .note()
                .as_deref(),
            Some("read 3 h ago")
        );
        let stale = standing(Freshness::Stale, Some(12));
        assert_eq!(
            stale.note().as_deref(),
            Some("read 12 s ago, before the repository changed; reading again")
        );
        assert!(stale.catching_up());
        assert_eq!(
            standing(Freshness::Stale, None).note().as_deref(),
            Some("read before the repository changed")
        );
        let pending = standing(Freshness::Pending, None);
        assert_eq!(pending.note().as_deref(), Some("first reading in progress"));
        assert!(pending.catching_up());
    }

    /// A screen's source is a reading of the executor's store: asked twice, it is read once,
    /// and what the screen is handed is current.
    #[test]
    fn a_source_is_a_reading_and_asked_twice_it_is_read_once() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let source = Source::of("plan.status");
        let first = ask(&ctx, source);
        assert!(first.value.is_ok());
        assert_eq!(first.standing.freshness, Freshness::Current);
        assert!(!first.standing.catching_up());
        let ran = ctx.executor.handler_invocations();
        let again = ask(&ctx, source);
        assert_eq!(again.value, first.value);
        assert_eq!(
            ctx.executor.handler_invocations(),
            ran,
            "a reading at hand runs no handler on the path of the request"
        );
        assert!(ctx.executor.readings_remembered() >= 1);
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
