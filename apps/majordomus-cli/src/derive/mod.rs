//! Whether a derive may leave stage B alone: the decision behind `scripts/derive
//! --incremental`, and the record that decision is made against.
//!
//! # What this is for
//!
//! `scripts/derive` runs four stages, and one of them — stage B, `scripts/generate-site-data`
//! — is nearly all of its time: 512 of 546 seconds on 2026-10-12, and 415 seconds for a
//! derive that changed no file. It had no way to notice that nothing it reads had moved.
//! This module is that way, for stage B as a whole and nothing finer (ADR 0143).
//!
//! # The decision
//!
//! Stage B is left alone only when all of these hold, and it runs otherwise:
//!
//! 1. the tree has a key at all ([`mod@key`] says which states do not);
//! 2. a record of a successful run exists, parses, and was made under this key schema;
//! 3. no earlier run was left unfinished;
//! 4. the key of the tree now equals the recorded key, which covers every input the stage
//!    can read and not only the ones it lists;
//! 5. every file under the stage's outputs is on disk with the digest that run left, and
//!    nothing else is there.
//!
//! When they hold, a run would read exactly what the recorded run read and so would write
//! exactly what is already on disk. That is the whole argument, and it rests on the stage
//! being deterministic over its inputs — which `scripts/derive-check` holds it to on every
//! change, by running it in full and never consulting anything here.
//!
//! # Who decides what
//!
//! `scripts/derive` keeps what it always owned: the arguments, the preflight, the machine's
//! lock, the progress lines and the exit codes. It asks three questions of this module
//! through the command line ([`begin()`], [`record()`], and for a person [`plan()`]) and acts on
//! the exit code. Nothing about the key, the record or the comparison is written in shell.
//!
//! # Modes
//!
//! The derive's default is the full one and nothing here is reached by it. `--incremental`
//! skips a proven stage B. `--shadow` decides the same way, runs stage B regardless, and
//! refuses (exit 13) when the run changed a byte the skip would have kept: the full run's
//! bytes stay in the tree, and the record that allowed the skip is dropped.

pub(crate) mod key;
pub(crate) mod record;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use self::key::{measure, Ambient, Fingerprint, Measured, NODE};
use self::record::{
    Divergence, DivergentOutput, Loaded, Pending, Record, Store, DIVERGENCE_SCHEMA, PENDING_SCHEMA,
    RECORD_SCHEMA,
};
use crate::evidence::record::now_rfc3339;

/// The schema of a [`Plan`].
pub(crate) const PLAN_SCHEMA: &str = "majordomus/derive-plan/v1";

/// The stage is proven current: the derive may leave it alone.
pub(crate) const EXIT_REUSE: u8 = 0;
/// The stage must run. A finding about the tree, in the repository's vocabulary.
pub(crate) const EXIT_RUN: u8 = 10;
/// A shadow run changed what the skip would have kept. The tree holds the full run's bytes;
/// the decision was wrong.
pub(crate) const EXIT_DIVERGED: u8 = 13;

/// How many moved paths a plan names before it says how many more there are.
const PATHS_NAMED: usize = 20;

/// What every line this module hands the derive begins with: the stage's own progress
/// prefix, so the lines read as the script's.
const SAYS: &str = "==> derive: stage B:";

/// The two modes that reach this module. The full derive does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mode {
    /// Skip the stage when it is proven current.
    Incremental,
    /// Decide as incremental does, run the stage regardless, and compare.
    Shadow,
}

/// What an incremental derive does with the stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum State {
    /// Leave it: its inputs and its outputs are what the recorded run had and left.
    Reuse,
    /// Run it.
    Regenerate,
}

/// Why a plan says what it says. A closed vocabulary, so that a reader can act on the kind
/// without reading the sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Reason {
    /// The key of the tree equals the recorded key.
    KeyMatch,
    /// Every output is on disk as the recorded run left it.
    OutputsVerified,
    /// No run is recorded for this checkout.
    NoRecord,
    /// A record file exists and is not one this executable reads.
    RecordInvalid {
        /// What is wrong with it.
        detail: String,
    },
    /// The run before this one began the stage and never recorded its end.
    InterruptedRun,
    /// The tree is in a state no key covers.
    Uncacheable {
        /// Which state.
        detail: String,
    },
    /// The generator's own text changed.
    OperationVersionChanged,
    /// The executable is of another generation, profile or target.
    ExecutableChanged,
    /// A tool on `PATH`, or the operating system, changed.
    ToolchainChanged,
    /// An input variable of the environment, or the share directory, changed.
    EnvironmentChanged,
    /// Git's global or system configuration changed.
    GitConfigChanged,
    /// The ignored local evidence the stage reads changed.
    LocalStateChanged,
    /// An input's bytes or mode changed, or it went missing.
    InputChanged {
        /// The input.
        path: String,
    },
    /// A path joined the index, or an ignored file appeared.
    InputAdded {
        /// The input.
        path: String,
    },
    /// A path left the index, or an ignored file went away.
    InputRemoved {
        /// The input.
        path: String,
    },
    /// An output the recorded run left is not on disk.
    OutputMissing {
        /// The output.
        path: String,
    },
    /// An output's bytes are not the ones the recorded run left.
    OutputTampered {
        /// The output.
        path: String,
    },
    /// A file is under the stage's outputs that the recorded run did not leave.
    OutputUnexpected {
        /// The file.
        path: String,
    },
}

/// What an incremental derive would do with stage B right now, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Plan {
    /// [`PLAN_SCHEMA`].
    pub(crate) schema: &'static str,
    /// The node, [`NODE`].
    pub(crate) node: &'static str,
    /// The decision.
    pub(crate) state: State,
    /// Whether this tree has a key. When it has none, nothing is skipped and nothing is
    /// recorded.
    pub(crate) cacheable: bool,
    /// The key of the tree as it stands.
    pub(crate) key: Option<String>,
    /// The key of the recorded run.
    pub(crate) recorded_key: Option<String>,
    /// When that run was recorded.
    pub(crate) recorded_at: Option<String>,
    /// How many inputs the key covers.
    pub(crate) inputs: usize,
    /// How many output files are on disk.
    pub(crate) outputs: usize,
    /// How many inputs and outputs differ from the record in all; `reasons` names the first.
    pub(crate) moved: usize,
    /// Why, each kind once and at most a bounded number of paths.
    pub(crate) reasons: Vec<Reason>,
    /// Each component of the key, by name.
    pub(crate) components: BTreeMap<String, String>,
}

impl Plan {
    /// One sentence for the derive's progress line: the reasons, in words.
    pub(crate) fn sentence(&self) -> String {
        let mut said: Vec<String> = Vec::new();
        let paths = |pick: &dyn Fn(&Reason) -> Option<String>| -> Vec<String> {
            self.reasons.iter().filter_map(pick).collect()
        };
        for reason in &self.reasons {
            let words = match reason {
                Reason::KeyMatch => "its inputs are the recorded run's",
                Reason::OutputsVerified => "its outputs are as that run left them",
                Reason::NoRecord => "no run of it is recorded for this checkout",
                Reason::InterruptedRun => "the run before this one did not finish",
                Reason::OperationVersionChanged => "the generator changed",
                Reason::ExecutableChanged => "the executable changed",
                Reason::ToolchainChanged => "a tool or the operating system changed",
                Reason::EnvironmentChanged => "the environment changed",
                Reason::GitConfigChanged => "git's configuration changed",
                Reason::LocalStateChanged => "the local use-case evidence changed",
                Reason::RecordInvalid { detail } => {
                    said.push(format!("the record is not usable: {detail}"));
                    continue;
                }
                Reason::Uncacheable { detail } => {
                    said.push(detail.clone());
                    continue;
                }
                _ => continue,
            };
            said.push(words.to_string());
        }
        let inputs = paths(&|r| match r {
            Reason::InputChanged { path } => Some(path.clone()),
            Reason::InputAdded { path } => Some(format!("{path} (new)")),
            Reason::InputRemoved { path } => Some(format!("{path} (gone)")),
            _ => None,
        });
        let outputs = paths(&|r| match r {
            Reason::OutputMissing { path } => Some(format!("{path} (missing)")),
            Reason::OutputTampered { path } => Some(format!("{path} (changed)")),
            Reason::OutputUnexpected { path } => Some(format!("{path} (not generated)")),
            _ => None,
        });
        let shown = inputs.len() + outputs.len();
        let more = match self.moved.saturating_sub(shown) {
            0 => String::new(),
            n => format!(" and {n} more"),
        };
        if !inputs.is_empty() {
            said.push(format!(
                "{} input(s) moved since the recorded run: {}",
                inputs.len(),
                inputs.join(", ")
            ));
        }
        if !outputs.is_empty() {
            said.push(format!(
                "{} output(s) are not what the recorded run left: {}",
                outputs.len(),
                outputs.join(", ")
            ));
        }
        format!("{}{more}", said.join("; "))
    }
}

/// What a step of the derive is told: the exit code that carries the decision, and the
/// lines to print on stderr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Step {
    /// [`EXIT_REUSE`], [`EXIT_RUN`] or [`EXIT_DIVERGED`]; 0 for a step that decides nothing.
    pub(crate) code: u8,
    /// What happened, each line already carrying the stage's prefix.
    pub(crate) lines: Vec<String>,
}

/// A plan with what it was decided from, for the steps that go on to act on it.
struct Decision {
    plan: Plan,
    fingerprint: Option<Box<Fingerprint>>,
    record: Option<Box<Record>>,
}

/// The first twelve characters of a key: enough to tell two apart in a log line.
fn short(key: &str) -> &str {
    key.get(..12).unwrap_or(key)
}

/// Decide, from the store as it stands and the tree as it stands. Writes nothing.
fn decide(root: &Path, store: &Store, ambient: &Ambient) -> Decision {
    let loaded = store.record();
    let fingerprint = match measure(root, ambient) {
        Measured::Keyed(f) => f,
        Measured::Uncacheable(detail) => {
            let (recorded_key, recorded_at) = match &loaded {
                Loaded::Found(r) => (Some(r.key.clone()), Some(r.recorded_at.clone())),
                _ => (None, None),
            };
            return Decision {
                plan: Plan {
                    schema: PLAN_SCHEMA,
                    node: NODE,
                    state: State::Regenerate,
                    cacheable: false,
                    key: None,
                    recorded_key,
                    recorded_at,
                    inputs: 0,
                    outputs: 0,
                    moved: 0,
                    reasons: vec![Reason::Uncacheable { detail }],
                    components: BTreeMap::new(),
                },
                fingerprint: None,
                record: None,
            };
        }
    };

    let mut reasons = Vec::new();
    let mut moved = 0;
    if store.pending().is_some() {
        reasons.push(Reason::InterruptedRun);
    }
    let record = match loaded {
        Loaded::Absent => {
            reasons.push(Reason::NoRecord);
            None
        }
        Loaded::Invalid(detail) => {
            reasons.push(Reason::RecordInvalid { detail });
            None
        }
        Loaded::Found(record) => {
            for (name, value) in &fingerprint.components {
                if record.components.get(name) == Some(value) {
                    continue;
                }
                match name.as_str() {
                    "operation" => reasons.push(Reason::OperationVersionChanged),
                    "executable" => reasons.push(Reason::ExecutableChanged),
                    "toolchain" => reasons.push(Reason::ToolchainChanged),
                    "environment" => reasons.push(Reason::EnvironmentChanged),
                    "git_config" => reasons.push(Reason::GitConfigChanged),
                    "local_state" => reasons.push(Reason::LocalStateChanged),
                    // the tree is named path by path below
                    _ => {}
                }
            }
            let mut named = 0;
            for change in compare(&record.inputs, &fingerprint.inputs) {
                moved += 1;
                if named < PATHS_NAMED {
                    named += 1;
                    reasons.push(match change {
                        Moved::Changed(path, _, _) => Reason::InputChanged { path },
                        Moved::Added(path, _) => Reason::InputAdded { path },
                        Moved::Removed(path, _) => Reason::InputRemoved { path },
                    });
                }
            }
            for change in compare(&record.outputs, &fingerprint.outputs) {
                moved += 1;
                if named < PATHS_NAMED {
                    named += 1;
                    reasons.push(match change {
                        Moved::Changed(path, _, _) => Reason::OutputTampered { path },
                        Moved::Added(path, _) => Reason::OutputUnexpected { path },
                        Moved::Removed(path, _) => Reason::OutputMissing { path },
                    });
                }
            }
            // The key is the decision, not the list above: a component nothing here names
            // must still refuse the skip.
            if reasons.is_empty() && record.key != fingerprint.key {
                reasons.push(Reason::RecordInvalid {
                    detail: "its key differs from this tree's in a way its components do not show"
                        .into(),
                });
            }
            Some(record)
        }
    };

    let state = if reasons.is_empty() {
        reasons = vec![Reason::KeyMatch, Reason::OutputsVerified];
        State::Reuse
    } else {
        State::Regenerate
    };
    Decision {
        plan: Plan {
            schema: PLAN_SCHEMA,
            node: NODE,
            state,
            cacheable: true,
            key: Some(fingerprint.key.clone()),
            recorded_key: record.as_ref().map(|r| r.key.clone()),
            recorded_at: record.as_ref().map(|r| r.recorded_at.clone()),
            inputs: fingerprint.inputs.len(),
            outputs: fingerprint.outputs.len(),
            moved,
            reasons,
            components: fingerprint.components.clone(),
        },
        fingerprint: Some(fingerprint),
        record,
    }
}

/// One difference between two sorted lists of `(path, state)`.
enum Moved {
    /// In both, with different states: the path, before, after.
    Changed(String, String, String),
    /// Only in the second: the path and its state.
    Added(String, String),
    /// Only in the first: the path and its state.
    Removed(String, String),
}

/// Every difference between two lists sorted by path, in path order.
fn compare(before: &[(String, String)], after: &[(String, String)]) -> Vec<Moved> {
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < before.len() || j < after.len() {
        match (before.get(i), after.get(j)) {
            (Some((p, a)), Some((q, b))) if p == q => {
                if a != b {
                    out.push(Moved::Changed(p.clone(), a.clone(), b.clone()));
                }
                i += 1;
                j += 1;
            }
            (Some((p, a)), Some((q, _))) if p < q => {
                out.push(Moved::Removed(p.clone(), a.clone()));
                i += 1;
            }
            (Some((p, a)), None) => {
                out.push(Moved::Removed(p.clone(), a.clone()));
                i += 1;
            }
            (_, Some((q, b))) => {
                out.push(Moved::Added(q.clone(), b.clone()));
                j += 1;
            }
            (None, None) => break,
        }
    }
    out
}

/// What an incremental derive would do with stage B, without doing or writing anything.
pub(crate) fn plan(root: &Path, ambient: &Ambient) -> Result<Plan, String> {
    let store = Store::open(root)?;
    Ok(decide(root, &store, ambient).plan)
}

/// Open stage B: decide, and leave the store saying a run is in flight when one is.
///
/// [`EXIT_REUSE`] when the stage is proven current, [`EXIT_RUN`] when it must run. Under
/// [`Mode::Incremental`] a reuse writes nothing. Whenever the stage is going to run, the
/// record is dropped first — it describes outputs that are about to be replaced — except
/// under shadow when the decision was to reuse, where it is what the run is compared with.
pub(crate) fn begin(root: &Path, ambient: &Ambient, mode: Mode) -> Result<Step, String> {
    let store = Store::open(root)?;
    let Decision {
        plan,
        fingerprint,
        record,
    } = decide(root, &store, ambient);
    let key = plan.key.as_deref().map(short).unwrap_or("none").to_string();
    let pending = |expected: State, outputs: Option<Vec<(String, String)>>| {
        fingerprint.as_ref().map(|f| Pending {
            schema: PENDING_SCHEMA.into(),
            mode,
            expected,
            settled: f.settled.clone(),
            components: f.components.clone(),
            outputs,
            begun_at: now_rfc3339(),
        })
    };
    match (plan.state, mode) {
        (State::Reuse, Mode::Incremental) => Ok(Step {
            code: EXIT_REUSE,
            lines: vec![format!(
                "{SAYS} skipped (key {key}, inputs and outputs unchanged since the run recorded {})",
                plan.recorded_at.as_deref().unwrap_or("earlier")
            )],
        }),
        (State::Reuse, Mode::Shadow) => {
            let kept = record.map(|r| r.outputs);
            if let Some(p) = pending(State::Reuse, kept) {
                store.write(&store.pending_path(), &p)?;
            }
            Ok(Step {
                code: EXIT_REUSE,
                lines: vec![format!(
                    "{SAYS} shadow: the incremental mode would skip it (key {key}); running it to compare"
                )],
            })
        }
        (State::Regenerate, _) => {
            store.remove(&store.record_path())?;
            match pending(State::Regenerate, None) {
                Some(p) => store.write(&store.pending_path(), &p)?,
                None => store.remove(&store.pending_path())?,
            }
            Ok(Step {
                code: EXIT_RUN,
                lines: vec![format!("{SAYS} must run: {}", plan.sentence())],
            })
        }
    }
}

/// Close stage B after it succeeded: record what it ran over and what it left, or say why
/// nothing is recorded.
///
/// Called only after the stage exited 0. Nothing is recorded when no run was begun, when
/// the tree has no key, or when anything the stage could read moved while it ran; each is
/// said and exits 0, because the derive itself succeeded. Under shadow, a run that changed
/// an output the skip would have kept is [`EXIT_DIVERGED`]: the difference is written
/// beside the record and the record is dropped.
pub(crate) fn record(root: &Path, ambient: &Ambient) -> Result<Step, String> {
    let store = Store::open(root)?;
    let dropped = |why: String| -> Result<Step, String> {
        store.remove(&store.pending_path())?;
        store.remove(&store.record_path())?;
        Ok(Step {
            code: 0,
            lines: vec![format!("{SAYS} not recorded: {why}")],
        })
    };
    let pending = match store.pending() {
        None => {
            return Ok(Step {
                code: 0,
                lines: vec![format!(
                    "{SAYS} not recorded: no run was begun in this checkout"
                )],
            })
        }
        Some(Err(e)) => return dropped(format!("the marker of the run is unreadable ({e})")),
        Some(Ok(p)) => p,
    };
    let after = match measure(root, ambient) {
        Measured::Uncacheable(why) => return dropped(why),
        Measured::Keyed(f) => f,
    };
    if after.settled != pending.settled {
        let names: Vec<&str> = after
            .components
            .iter()
            .filter(|(n, v)| n.as_str() != "local_state" && pending.components.get(*n) != Some(*v))
            .map(|(n, _)| n.as_str())
            .collect();
        return dropped(format!(
            "something it could read moved while it ran ({})",
            names.join(", ")
        ));
    }
    let mut lines = Vec::new();
    if pending.mode == Mode::Shadow {
        match (&pending.expected, &pending.outputs) {
            (State::Reuse, Some(kept)) => {
                let differing: Vec<DivergentOutput> = compare(kept, &after.outputs)
                    .into_iter()
                    .map(|m| match m {
                        Moved::Changed(path, a, b) => DivergentOutput {
                            path,
                            kept: Some(a),
                            written: Some(b),
                        },
                        Moved::Added(path, b) => DivergentOutput {
                            path,
                            kept: None,
                            written: Some(b),
                        },
                        Moved::Removed(path, a) => DivergentOutput {
                            path,
                            kept: Some(a),
                            written: None,
                        },
                    })
                    .collect();
                if !differing.is_empty() {
                    let named: Vec<&str> = differing
                        .iter()
                        .take(PATHS_NAMED)
                        .map(|d| d.path.as_str())
                        .collect();
                    let lines = vec![
                        format!(
                            "{SAYS} SHADOW DIVERGENCE: the incremental mode would have skipped it (key {}), and the run changed {} output(s): {}",
                            short(&after.key),
                            differing.len(),
                            named.join(", ")
                        ),
                        format!(
                            "{SAYS} the tree holds the full run's bytes; the record that allowed the skip is dropped, and the difference is in {}",
                            store.divergence_path().display()
                        ),
                    ];
                    store.write(
                        &store.divergence_path(),
                        &Divergence {
                            schema: DIVERGENCE_SCHEMA.into(),
                            key: after.key.clone(),
                            differing,
                            observed_at: now_rfc3339(),
                        },
                    )?;
                    store.remove(&store.pending_path())?;
                    store.remove(&store.record_path())?;
                    return Ok(Step {
                        code: EXIT_DIVERGED,
                        lines,
                    });
                }
                lines.push(format!(
                    "{SAYS} shadow: the run left every output as the skip would have ({} file(s))",
                    after.outputs.len()
                ));
            }
            _ => lines.push(format!(
                "{SAYS} shadow: the incremental mode would have run it too; nothing to compare"
            )),
        }
    }
    let written = Record {
        schema: RECORD_SCHEMA.into(),
        key_schema: key::KEY_SCHEMA,
        node: NODE.into(),
        key: after.key.clone(),
        components: after.components.clone(),
        inputs: after.inputs.clone(),
        outputs: after.outputs.clone(),
        generation: crate::GENERATION.into(),
        recorded_at: now_rfc3339(),
    };
    store.write(&store.record_path(), &written)?;
    store.remove(&store.pending_path())?;
    lines.push(format!(
        "{SAYS} recorded (key {}, {} input(s), {} output file(s))",
        short(&after.key),
        after.inputs.len(),
        after.outputs.len()
    ));
    Ok(Step { code: 0, lines })
}

/// Drop what this checkout remembers about stage B. The next incremental derive runs it.
pub(crate) fn forget(root: &Path) -> Result<Step, String> {
    let store = Store::open(root)?;
    store.remove(&store.record_path())?;
    store.remove(&store.pending_path())?;
    Ok(Step {
        code: 0,
        lines: vec![format!(
            "{SAYS} forgotten; the next incremental derive of this checkout runs it"
        )],
    })
}

#[cfg(test)]
mod tests;
