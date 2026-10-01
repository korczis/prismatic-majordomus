//! The one writer of reasoning records, and the rules a record must satisfy before it
//! becomes durable.
//!
//! Records live under `.ai/local/state/reasoning/<task>/`, one JSON file per record,
//! written to a temporary name and renamed into place so that a reader never meets half a
//! record and two writers never share a file. They are checkout state (ADR 0005): never
//! tracked, never a source, never published.
//!
//! Admission is where the design's promises become refusals:
//!
//! * a material assessment carries evidence — evidence before opinion;
//! * a plan is computed here, from the availability at this moment, never supplied;
//! * a consultation names an advisor its recorded plan selected — no review nobody planned,
//!   and no claim of review from an advisor that was not available (the plan's own
//!   snapshot is the proof); a completed one carries a conclusion, a failed one none;
//! * a disagreement is settled by a resolution that cites evidence, and a conclusion is
//!   refused while a disagreement on its assessment is unsettled — there is no path from
//!   "two advisors said A" to a decision that skips the evidence;
//! * a conclusion on material uncertainty carries evidence and a validation plan, weighs
//!   every completed consultation of its assessment, and has its review count computed.

use std::path::{Path, PathBuf};

use super::availability::{AdvisorState, ModeResolution};
use super::policy::{plan, Materiality, PlanRequest};
use super::record::*;

/// The store's directory, relative to the repository root.
pub const STATE_DIR: &str = ".ai/local/state/reasoning";
/// The task name of a record written while no task was open.
pub const UNTASKED: &str = "untasked";

/// Why a record was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal(pub String);

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn refuse<T>(reason: impl Into<String>) -> Result<T, Refusal> {
    Err(Refusal(reason.into()))
}

/// Records are in the order they were written: by their millisecond stamp, then by id.
/// The stamps are fixed-width, so the label orders as time does.
impl crate::order::Ordered for ReasoningRecord {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.recorded_at, &self.id)
    }
}

/// The records of a store, read.
#[derive(Debug, Default)]
pub struct Loaded {
    /// Every record that parsed, ordered by `recorded_at` then id.
    pub records: Vec<ReasoningRecord>,
    /// Files that did not parse, by path relative to the store.
    pub unreadable: Vec<String>,
}

impl Loaded {
    /// The record of an id.
    pub fn get(&self, id: &str) -> Option<&ReasoningRecord> {
        self.records.iter().find(|r| r.id == id)
    }
}

/// The store of one checkout.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// The store of the repository at `root`.
    pub fn at(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// Its directory.
    pub fn dir(&self) -> PathBuf {
        self.root.join(STATE_DIR)
    }

    /// The task open in this checkout, from `.ai/local/state/current.yaml`, or
    /// [`UNTASKED`].
    pub fn current_task(&self) -> String {
        std::fs::read_to_string(self.root.join(".ai/local/state/current.yaml"))
            .ok()
            .and_then(|text| {
                text.lines().find_map(|l| {
                    l.strip_prefix("id:")
                        .map(|v| v.trim().trim_matches('"').to_string())
                })
            })
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| UNTASKED.to_string())
    }

    /// Every record of every task, in canonical order.
    pub fn load(&self) -> Loaded {
        let mut out = Loaded::default();
        let Ok(tasks) = std::fs::read_dir(self.dir()) else {
            return out;
        };
        for task in tasks.flatten() {
            let Ok(files) = std::fs::read_dir(task.path()) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let parsed = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|t| serde_json::from_str::<ReasoningRecord>(&t).ok())
                    .filter(|r| r.schema == RECORD_SCHEMA && r.version == RECORD_VERSION);
                match parsed {
                    Some(r) => out.records.push(r),
                    None => out.unreadable.push(
                        path.strip_prefix(self.dir())
                            .unwrap_or(&path)
                            .display()
                            .to_string(),
                    ),
                }
            }
        }
        crate::order::canonical(&mut out.records);
        crate::order::canonical(&mut out.unreadable);
        out
    }

    /// Make a record durable: temporary name, then rename. Returns its path.
    pub fn write(&self, record: &ReasoningRecord) -> std::io::Result<PathBuf> {
        let dir = self.dir().join(&record.task);
        std::fs::create_dir_all(&dir)?;
        let name = format!("{}.json", record.id);
        let path = dir.join(&name);
        if path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("{} already exists", path.display()),
            ));
        }
        let tmp = dir.join(format!(".{name}.tmp"));
        let text = serde_json::to_string_pretty(record).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, format!("{text}\n"))?;
        std::fs::rename(&tmp, &path)?;
        Ok(path)
    }
}

/// What admission needs besides the records.
pub struct Admission<'a> {
    /// The mode in force.
    pub mode: &'a ModeResolution,
    /// Every advisor's standing now.
    pub availability: &'a [AdvisorState],
}

fn subject_key(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The conclusion that stands for an assessment: its latest not superseded by another.
pub fn standing_conclusion<'a>(
    loaded: &'a Loaded,
    assessment: &str,
) -> Option<&'a ReasoningRecord> {
    let superseded: Vec<&str> = loaded
        .records
        .iter()
        .filter_map(|r| match &r.body {
            ReasoningBody::Conclusion(c) => c.input.supersedes.as_deref(),
            _ => None,
        })
        .collect();
    loaded.records.iter().rev().find(|r| {
        matches!(&r.body, ReasoningBody::Conclusion(c) if c.input.assessment == assessment)
            && !superseded.contains(&r.id.as_str())
    })
}

/// A standing conclusion on the same subject as an assessment, from any task: what a
/// later session reuses instead of asking again.
pub fn prior_on_subject<'a>(
    loaded: &'a Loaded,
    assessment: &ReasoningRecord,
) -> Option<&'a ReasoningRecord> {
    let ReasoningBody::Assessment(a) = &assessment.body else {
        return None;
    };
    let key = subject_key(&a.subject);
    loaded
        .records
        .iter()
        .filter(|r| r.id != assessment.id)
        .filter(
            |r| matches!(&r.body, ReasoningBody::Assessment(o) if subject_key(&o.subject) == key),
        )
        .rev()
        .find_map(|r| standing_conclusion(loaded, &r.id))
}

fn kind_of<'a>(loaded: &'a Loaded, id: &str, kind: &str) -> Result<&'a ReasoningRecord, Refusal> {
    match loaded.get(id) {
        Some(r) if r.body.kind() == kind => Ok(r),
        Some(r) => refuse(format!("{id} is a {}, not a {kind}", r.body.kind())),
        None => refuse(format!("no {kind} {id} is recorded")),
    }
}

fn nonempty(field: &str, value: &str) -> Result<(), Refusal> {
    if value.trim().is_empty() {
        return refuse(format!("{field} is empty"));
    }
    Ok(())
}

/// The assessment a record belongs to, when it belongs to one.
pub fn assessment_of(loaded: &Loaded, record: &ReasoningRecord) -> Option<String> {
    match &record.body {
        ReasoningBody::Assessment(_) => Some(record.id.clone()),
        ReasoningBody::Plan(p) => Some(p.assessment.clone()),
        ReasoningBody::Consultation(c) => {
            loaded.get(&c.plan).and_then(|p| assessment_of(loaded, p))
        }
        ReasoningBody::Disagreement(d) => Some(d.assessment.clone()),
        ReasoningBody::Resolution(r) => loaded
            .get(&r.disagreement)
            .and_then(|d| assessment_of(loaded, d)),
        ReasoningBody::Conclusion(c) => Some(c.input.assessment.clone()),
        ReasoningBody::Validation(v) => loaded
            .get(&v.conclusion)
            .and_then(|c| assessment_of(loaded, c)),
        ReasoningBody::Attempt(a) => a.assessment.clone(),
    }
}

/// Admit an input against the records already durable: the body to store, or why not.
pub fn admit(
    input: ReasoningInput,
    loaded: &Loaded,
    admission: &Admission<'_>,
) -> Result<ReasoningBody, Refusal> {
    Ok(match input {
        ReasoningInput::Assessment(a) => {
            nonempty("subject", &a.subject)?;
            if a.materiality >= Materiality::Material && a.evidence.is_empty() {
                return refuse(
                    "a material assessment carries the evidence gathered before anyone is asked: `evidence` is empty",
                );
            }
            ReasoningBody::Assessment(a)
        }
        ReasoningInput::Plan(p) => {
            let assessment = kind_of(loaded, &p.assessment, "assessment")?;
            let ReasoningBody::Assessment(a) = &assessment.body else {
                unreachable!()
            };
            let prior = prior_on_subject(loaded, assessment).map(|r| r.id.clone());
            let request = PlanRequest {
                materiality: a.materiality,
                confidence: a.confidence,
                capabilities: a.capabilities.clone(),
                prior_conclusion: prior,
                new_evidence: a.new_evidence,
            };
            ReasoningBody::Plan(StoredPlan {
                assessment: p.assessment,
                plan: plan(&request, admission.mode, admission.availability),
                availability: admission.availability.to_vec(),
            })
        }
        ReasoningInput::Consultation(c) => {
            let plan_record = kind_of(loaded, &c.plan, "plan")?;
            let ReasoningBody::Plan(p) = &plan_record.body else {
                unreachable!()
            };
            if !p.plan.selected.iter().any(|s| s.advisor == c.advisor) {
                let selected: Vec<&str> =
                    p.plan.selected.iter().map(|s| s.advisor.as_str()).collect();
                return refuse(format!(
                    "plan {} did not select advisor '{}' (it selected: {}); a consultation no plan selected would be a review nobody planned",
                    c.plan,
                    c.advisor,
                    if selected.is_empty() { "none".to_string() } else { selected.join(", ") }
                ));
            }
            let has_conclusion = c
                .conclusion
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty());
            match (c.status, has_conclusion) {
                (ConsultationStatus::Completed, false) => {
                    return refuse("a completed consultation carries the advisor's conclusion")
                }
                (ConsultationStatus::Completed, true) if c.stance.is_none() => {
                    return refuse(
                        "a completed consultation says where the advice stands: `stance`",
                    )
                }
                (s, true) if s != ConsultationStatus::Completed => {
                    return refuse(format!(
                        "a {} consultation received no answer, so it carries no conclusion",
                        s.as_str()
                    ))
                }
                _ => {}
            }
            if c.status == ConsultationStatus::Completed {
                let twice = loaded.records.iter().any(|r| {
                    matches!(&r.body, ReasoningBody::Consultation(o)
                        if o.plan == c.plan && o.advisor == c.advisor && o.status == ConsultationStatus::Completed)
                });
                if twice {
                    return refuse(format!(
                        "advisor '{}' already answered plan {}; one plan, one answer per advisor",
                        c.advisor, c.plan
                    ));
                }
            }
            ReasoningBody::Consultation(c)
        }
        ReasoningInput::Disagreement(d) => {
            kind_of(loaded, &d.assessment, "assessment")?;
            nonempty("discriminating_question", &d.discriminating_question)?;
            if d.positions.len() < 2 {
                return refuse("a disagreement has at least two positions");
            }
            let sources: std::collections::BTreeSet<&str> =
                d.positions.iter().map(|p| p.source.as_str()).collect();
            if sources.len() != d.positions.len() {
                return refuse("each position of a disagreement has its own source");
            }
            for p in &d.positions {
                nonempty("position", &p.position)?;
                if p.source != "primary" {
                    let r = kind_of(loaded, &p.source, "consultation")?;
                    if !matches!(&r.body, ReasoningBody::Consultation(c) if c.status == ConsultationStatus::Completed)
                    {
                        return refuse(format!(
                            "{} did not complete, so it holds no position",
                            p.source
                        ));
                    }
                }
            }
            ReasoningBody::Disagreement(d)
        }
        ReasoningInput::Resolution(r) => {
            let d = kind_of(loaded, &r.disagreement, "disagreement")?;
            let ReasoningBody::Disagreement(dis) = &d.body else {
                unreachable!()
            };
            nonempty("experiment", &r.experiment)?;
            nonempty("outcome", &r.outcome)?;
            if r.evidence.is_empty() {
                return refuse(
                    "a disagreement is settled by evidence, never by a count: `evidence` is empty",
                );
            }
            if r.favours != "neither" && !dis.positions.iter().any(|p| p.source == r.favours) {
                return refuse(format!(
                    "'{}' is not a position of {}; favour one of its sources or `neither`",
                    r.favours, r.disagreement
                ));
            }
            if resolution_of(loaded, &r.disagreement).is_some() {
                return refuse(format!("{} is already resolved", r.disagreement));
            }
            ReasoningBody::Resolution(r)
        }
        ReasoningInput::Conclusion(c) => {
            let assessment = kind_of(loaded, &c.assessment, "assessment")?;
            let ReasoningBody::Assessment(a) = &assessment.body else {
                unreachable!()
            };
            nonempty("decision", &c.decision)?;
            nonempty("rationale", &c.rationale)?;
            if a.materiality >= Materiality::Material {
                if c.evidence.is_empty() {
                    return refuse("a conclusion on material uncertainty rests on evidence: `evidence` is empty");
                }
                if c.validation_plan.is_empty() {
                    return refuse("a conclusion on material uncertainty says how it will be proven: `validation_plan` is empty");
                }
            }
            let mut reviewed_by: Vec<String> = Vec::new();
            for id in &c.consultations {
                let r = kind_of(loaded, id, "consultation")?;
                let ReasoningBody::Consultation(k) = &r.body else {
                    unreachable!()
                };
                if k.status != ConsultationStatus::Completed {
                    return refuse(format!(
                        "{id} ended {}, so no advice came from it to weigh",
                        k.status.as_str()
                    ));
                }
                if assessment_of(loaded, r).as_deref() != Some(c.assessment.as_str()) {
                    return refuse(format!("{id} was asked about another assessment"));
                }
                if !reviewed_by.contains(&k.advisor) {
                    reviewed_by.push(k.advisor.clone());
                }
            }
            for r in &loaded.records {
                if let ReasoningBody::Consultation(k) = &r.body {
                    if k.status == ConsultationStatus::Completed
                        && assessment_of(loaded, r).as_deref() == Some(c.assessment.as_str())
                        && !c.consultations.contains(&r.id)
                    {
                        return refuse(format!(
                            "{} (advisor '{}') answered this assessment and the conclusion does not weigh it; cite it, and reject it in `rejected` if it is wrong",
                            r.id, k.advisor
                        ));
                    }
                }
            }
            for id in &c.disagreements {
                kind_of(loaded, id, "disagreement")?;
            }
            for r in &loaded.records {
                if matches!(&r.body, ReasoningBody::Disagreement(d) if d.assessment == c.assessment)
                {
                    if resolution_of(loaded, &r.id).is_none() {
                        return refuse(format!(
                            "disagreement {} is unresolved: settle it with evidence (a `resolution`), not by counting who agrees",
                            r.id
                        ));
                    }
                    if !c.disagreements.contains(&r.id) {
                        return refuse(format!(
                            "disagreement {} bears on this assessment and the conclusion does not cite it",
                            r.id
                        ));
                    }
                }
            }
            match &c.supersedes {
                Some(prior) => {
                    kind_of(loaded, prior, "conclusion")?;
                }
                None => {
                    if let Some(standing) = standing_conclusion(loaded, &c.assessment) {
                        return refuse(format!(
                            "{} already concludes this assessment; a new conclusion names it in `supersedes`",
                            standing.id
                        ));
                    }
                }
            }
            crate::order::canonical(&mut reviewed_by);
            ReasoningBody::Conclusion(StoredConclusion {
                independent_review_count: reviewed_by.len(),
                reviewed_by,
                input: c,
            })
        }
        ReasoningInput::Validation(v) => {
            kind_of(loaded, &v.conclusion, "conclusion")?;
            nonempty("check", &v.check)?;
            ReasoningBody::Validation(v)
        }
        ReasoningInput::Attempt(a) => {
            if let Some(id) = &a.assessment {
                kind_of(loaded, id, "assessment")?;
            }
            nonempty("attempt", &a.attempt)?;
            nonempty("observed_failure", &a.observed_failure)?;
            ReasoningBody::Attempt(a)
        }
    })
}

/// The resolution of a disagreement, when there is one.
pub fn resolution_of<'a>(loaded: &'a Loaded, disagreement: &str) -> Option<&'a ReasoningRecord> {
    loaded
        .records
        .iter()
        .find(|r| matches!(&r.body, ReasoningBody::Resolution(x) if x.disagreement == disagreement))
}

/// A fresh record id: the kind, the stamp, and six hex digits of the moment and process.
pub fn mint_id(kind: &str, recorded_at: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seed = format!(
        "{nanos}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let compact: String = recorded_at
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    format!(
        "{kind}-{compact}-{}",
        &crate::policy::sha256_hex(&seed)[..6]
    )
}

/// A text without control characters: ANSI escape sequences dropped whole, every other
/// control character but the newline dropped.
pub fn strip_controls(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if !c.is_control() || c == '\n' {
            out.push(c);
        }
    }
    out
}

/// Remove every secret shape from every string of a body; the shapes that fired.
pub fn redact(body: ReasoningBody) -> (ReasoningBody, Vec<String>) {
    fn walk(v: &mut serde_json::Value, kinds: &mut Vec<String>) {
        match v {
            serde_json::Value::String(s) => {
                // Control characters (a terminal's colour codes in a diagnostic) reach no
                // record: every surface renders these strings, and not all of them escape.
                if s.chars().any(|c| c.is_control() && c != '\n') {
                    *s = strip_controls(s);
                }
                let r = crate::redaction::redact_secrets(s);
                if !r.kinds.is_empty() {
                    kinds.extend(r.kinds.iter().map(|k| k.to_string()));
                    *s = r.text;
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(|x| walk(x, kinds)),
            serde_json::Value::Object(o) => o.values_mut().for_each(|x| walk(x, kinds)),
            _ => {}
        }
    }
    let Ok(mut value) = serde_json::to_value(&body) else {
        return (body, Vec::new());
    };
    let mut kinds = Vec::new();
    walk(&mut value, &mut kinds);
    crate::order::canonical(&mut kinds);
    kinds.dedup();
    match serde_json::from_value(value) {
        Ok(b) => (b, kinds),
        Err(_) => (body, kinds),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::availability::{AdvisorStatus, ReasoningMode};
    use crate::reasoning::catalogue::AdvisorTransport;

    struct Fixture {
        loaded: Loaded,
        mode: ModeResolution,
        states: Vec<AdvisorState>,
        clock: u32,
    }

    fn advisor(id: &str, adapter: &str) -> AdvisorState {
        AdvisorState {
            id: id.into(),
            title: id.into(),
            transport: AdvisorTransport::Api,
            adapter: adapter.into(),
            capabilities: vec!["independent_reasoning".into()],
            model: None,
            executable: None,
            credential: None,
            status: AdvisorStatus::Available,
            reason: "present".into(),
            probe: String::new(),
            recovering: false,
            circuit: None,
        }
    }

    impl Fixture {
        fn new(states: Vec<AdvisorState>, mode: ReasoningMode) -> Self {
            Self {
                loaded: Loaded::default(),
                mode: ModeResolution {
                    mode,
                    source: "test".into(),
                },
                states,
                clock: 0,
            }
        }

        fn add(&mut self, input: ReasoningInput) -> Result<String, Refusal> {
            let body = admit(
                input,
                &self.loaded,
                &Admission {
                    mode: &self.mode,
                    availability: &self.states,
                },
            )?;
            self.clock += 1;
            let at = format!("2026-10-01T00:00:{:02}Z", self.clock);
            let id = format!("{}-{}", body.kind(), self.clock);
            self.loaded.records.push(ReasoningRecord {
                schema: RECORD_SCHEMA.into(),
                version: RECORD_VERSION,
                id: id.clone(),
                task: "t".into(),
                recorded_at: at,
                head: None,
                redacted: vec![],
                body,
            });
            Ok(id)
        }

        fn conclusion(&self, id: &str) -> StoredConclusion {
            match &self.loaded.get(id).unwrap().body {
                ReasoningBody::Conclusion(c) => c.clone(),
                _ => panic!("not a conclusion"),
            }
        }
    }

    fn evidence(r: &str) -> Vec<ReasoningEvidence> {
        vec![ReasoningEvidence {
            kind: ReasoningEvidenceKind::File,
            reference: r.into(),
            note: None,
        }]
    }

    fn assessment(m: Materiality) -> ReasoningInput {
        ReasoningInput::Assessment(AssessmentInput {
            subject: "timeout semantics of the worker".into(),
            materiality: m,
            hypothesis: Some("A: the timeout is a typed domain error".into()),
            evidence: evidence("src/worker.rs:10"),
            ..Default::default()
        })
    }

    fn consultation(plan: &str, advisor: &str, stance: Stance, says: &str) -> ReasoningInput {
        ReasoningInput::Consultation(ConsultationInput {
            plan: plan.into(),
            advisor: advisor.into(),
            status: ConsultationStatus::Completed,
            question: None,
            evidence: vec![],
            conclusion: Some(says.into()),
            stance: Some(stance),
            assumptions: vec![],
            risks: vec![],
            actions: vec![],
            falsifiers: vec![],
            confidence: None,
            duration_ms: None,
            usage: None,
            diagnostic: None,
            retry_after_seconds: None,
        })
    }

    fn conclude(a: &str, decision: &str) -> ConclusionInput {
        ConclusionInput {
            assessment: a.into(),
            decision: decision.into(),
            rationale: "the failure tests show it".into(),
            evidence: evidence("test/worker_timeout.rs"),
            validation_plan: vec!["cargo test worker".into()],
            ..Default::default()
        }
    }

    #[test]
    fn zero_advisors_still_reaches_a_conclusion_with_no_review_claimed() {
        let mut f = Fixture::new(vec![], ReasoningMode::Standard);
        let a = f.add(assessment(Materiality::High)).unwrap();
        let p = f
            .add(ReasoningInput::Plan(PlanInput {
                assessment: a.clone(),
            }))
            .unwrap();
        // no plan selected anyone, so no consultation can be claimed
        let err = f
            .add(consultation(&p, "anyone", Stance::Supports, "yes"))
            .unwrap_err();
        assert!(err.0.contains("did not select"), "{err}");
        let k = f
            .add(ReasoningInput::Conclusion(conclude(&a, "A")))
            .unwrap();
        let c = f.conclusion(&k);
        assert_eq!(c.independent_review_count, 0);
        assert!(c.reviewed_by.is_empty());
        f.add(ReasoningInput::Validation(ValidationInput {
            conclusion: k,
            check: "cargo test worker".into(),
            command: Some("cargo test worker".into()),
            outcome: ValidationOutcome::Pass,
            evidence: vec![],
        }))
        .unwrap();
    }

    #[test]
    fn evidence_comes_before_opinion() {
        let mut f = Fixture::new(vec![], ReasoningMode::Standard);
        let err = f
            .add(ReasoningInput::Assessment(AssessmentInput {
                subject: "x".into(),
                materiality: Materiality::Material,
                ..Default::default()
            }))
            .unwrap_err();
        assert!(err.0.contains("evidence"));
        // trivia needs none
        f.add(ReasoningInput::Assessment(AssessmentInput {
            subject: "a name".into(),
            materiality: Materiality::Trivial,
            ..Default::default()
        }))
        .unwrap();
    }

    #[test]
    fn agreement_is_not_a_verdict_a_disagreement_is_settled_by_evidence() {
        // Two advisors from independent sources; one supports A, one proposes B.
        let mut f = Fixture::new(
            vec![advisor("one", "x"), advisor("two", "y")],
            ReasoningMode::Standard,
        );
        let a = f.add(assessment(Materiality::High)).unwrap();
        let p = f
            .add(ReasoningInput::Plan(PlanInput {
                assessment: a.clone(),
            }))
            .unwrap();
        let c1 = f
            .add(consultation(&p, "one", Stance::Supports, "A"))
            .unwrap();
        let c2 = f
            .add(consultation(
                &p,
                "two",
                Stance::Opposes,
                "B: exit and let the supervisor restart",
            ))
            .unwrap();
        let d = f
            .add(ReasoningInput::Disagreement(DisagreementInput {
                assessment: a.clone(),
                positions: vec![
                    DisagreementPosition { source: "primary".into(), position: "A".into() },
                    DisagreementPosition { source: c1.clone(), position: "A".into() },
                    DisagreementPosition { source: c2.clone(), position: "B".into() },
                ],
                divergent_assumptions: vec!["whether a timeout is an infrastructure failure".into()],
                discriminating_question: "do callers retry on the domain error?".into(),
                experiment: Some("read the callers and run the supervision test".into()),
            }))
            .unwrap();
        // Two of three positions say A: that decides nothing.
        let mut premature = conclude(&a, "A");
        premature.consultations = vec![c1.clone(), c2.clone()];
        premature.disagreements = vec![d.clone()];
        let err = f.add(ReasoningInput::Conclusion(premature)).unwrap_err();
        assert!(err.0.contains("unresolved"), "{err}");
        // A resolution without evidence is refused.
        let err = f
            .add(ReasoningInput::Resolution(ResolutionInput {
                disagreement: d.clone(),
                experiment: "we voted".into(),
                outcome: "2 to 1".into(),
                favours: "primary".into(),
                evidence: vec![],
            }))
            .unwrap_err();
        assert!(err.0.contains("never by a count"));
        // The experiment favours the minority position, B.
        f.add(ReasoningInput::Resolution(ResolutionInput {
            disagreement: d.clone(),
            experiment: "run the supervision failure test".into(),
            outcome: "no caller retries; the supervisor restart is the recovery".into(),
            favours: c2.clone(),
            evidence: evidence("test/supervision.rs"),
        }))
        .unwrap();
        let mut decided = conclude(&a, "B");
        decided.consultations = vec![c1.clone(), c2.clone()];
        decided.disagreements = vec![d];
        decided.rejected = vec!["A: callers never handle the domain error".into()];
        let k = f.add(ReasoningInput::Conclusion(decided)).unwrap();
        let c = f.conclusion(&k);
        assert_eq!(c.input.decision, "B");
        assert_eq!(c.reviewed_by, ["one", "two"]);
        assert_eq!(c.independent_review_count, 2);
    }

    #[test]
    fn a_conclusion_weighs_every_answer_it_received() {
        let mut f = Fixture::new(vec![advisor("one", "x")], ReasoningMode::Standard);
        let a = f.add(assessment(Materiality::Material)).unwrap();
        let p = f
            .add(ReasoningInput::Plan(PlanInput {
                assessment: a.clone(),
            }))
            .unwrap();
        f.add(consultation(&p, "one", Stance::Opposes, "B"))
            .unwrap();
        let err = f
            .add(ReasoningInput::Conclusion(conclude(&a, "A")))
            .unwrap_err();
        assert!(err.0.contains("does not weigh it"), "{err}");
    }

    #[test]
    fn a_failed_consultation_is_recordable_and_is_not_review() {
        let mut f = Fixture::new(vec![advisor("one", "x")], ReasoningMode::Standard);
        let a = f.add(assessment(Materiality::Material)).unwrap();
        let p = f
            .add(ReasoningInput::Plan(PlanInput {
                assessment: a.clone(),
            }))
            .unwrap();
        let mut timeout = consultation(&p, "one", Stance::Supports, "yes");
        if let ReasoningInput::Consultation(c) = &mut timeout {
            c.status = ConsultationStatus::Timeout;
        }
        assert!(f
            .add(timeout.clone())
            .unwrap_err()
            .0
            .contains("carries no conclusion"));
        if let ReasoningInput::Consultation(c) = &mut timeout {
            c.conclusion = None;
            c.stance = None;
        }
        let t = f.add(timeout).unwrap();
        let mut cited = conclude(&a, "A");
        cited.consultations = vec![t];
        assert!(f
            .add(ReasoningInput::Conclusion(cited))
            .unwrap_err()
            .0
            .contains("ended timeout"));
        let k = f
            .add(ReasoningInput::Conclusion(conclude(&a, "A")))
            .unwrap();
        assert_eq!(f.conclusion(&k).independent_review_count, 0);
    }

    #[test]
    fn a_later_assessment_of_the_same_subject_reuses_the_standing_conclusion() {
        let mut f = Fixture::new(vec![advisor("one", "x")], ReasoningMode::Standard);
        let a = f.add(assessment(Materiality::Material)).unwrap();
        let p = f
            .add(ReasoningInput::Plan(PlanInput {
                assessment: a.clone(),
            }))
            .unwrap();
        let c = f
            .add(consultation(&p, "one", Stance::Supports, "A"))
            .unwrap();
        let mut k = conclude(&a, "A");
        k.consultations = vec![c];
        let k1 = f.add(ReasoningInput::Conclusion(k)).unwrap();
        // session B: the same subject, no new evidence
        let b = f.add(assessment(Materiality::Material)).unwrap();
        let pb = f
            .add(ReasoningInput::Plan(PlanInput {
                assessment: b.clone(),
            }))
            .unwrap();
        let ReasoningBody::Plan(stored) = &f.loaded.get(&pb).unwrap().body else {
            panic!()
        };
        assert_eq!(
            stored.plan.outcome,
            crate::reasoning::PlanOutcome::ReusePrior
        );
        assert!(stored.plan.reason.contains(&k1));
        // new evidence reopens it
        let mut fresh = assessment(Materiality::Material);
        if let ReasoningInput::Assessment(x) = &mut fresh {
            x.new_evidence = true;
        }
        let n = f.add(fresh).unwrap();
        let pn = f
            .add(ReasoningInput::Plan(PlanInput { assessment: n }))
            .unwrap();
        let ReasoningBody::Plan(stored) = &f.loaded.get(&pn).unwrap().body else {
            panic!()
        };
        assert_eq!(stored.plan.outcome, crate::reasoning::PlanOutcome::Consult);
    }

    #[test]
    fn a_result_arriving_after_the_circuit_opened_is_still_admitted_by_its_plan() {
        // Admission reads the plan's snapshot, not the availability of the moment.
        let mut f = Fixture::new(vec![advisor("one", "x")], ReasoningMode::Standard);
        let a = f.add(assessment(Materiality::Material)).unwrap();
        let p = f
            .add(ReasoningInput::Plan(PlanInput { assessment: a }))
            .unwrap();
        f.states[0].status = AdvisorStatus::TemporarilyFailed;
        f.add(consultation(&p, "one", Stance::Supports, "A"))
            .unwrap();
    }

    #[test]
    fn terminal_escapes_never_reach_the_store() {
        assert_eq!(
            strip_controls("\u{1b}[31mred\u{1b}[0m\ttab\nline"),
            "redtab\nline"
        );
    }

    #[test]
    fn secrets_never_reach_the_store() {
        let key = format!("{}{}", "sk-ant-", "b".repeat(24));
        let body = ReasoningBody::Assessment(AssessmentInput {
            subject: format!("the key {key} leaks"),
            ..Default::default()
        });
        let (body, kinds) = redact(body);
        let ReasoningBody::Assessment(a) = body else {
            panic!()
        };
        assert!(!a.subject.contains("sk-ant-"));
        assert_eq!(kinds, ["anthropic-key"]);
    }
}
