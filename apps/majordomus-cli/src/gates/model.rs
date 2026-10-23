//! The CI model, typed: the gates this repository declares, the path classes that select
//! them, and the applicability the two derive together.
//!
//! `.ai/repo/ci/gates.yaml` is the canonical declaration and this is a reader of it, not a
//! second copy. `scripts/ci-plan` is the other reader; both take the same file, apply the
//! same rules the file's own header states, and `test/cases/131_completion_gates.sh`
//! asserts that they select the same gates for the same paths and give each gate, and the
//! plan, the same reason in the same words: `ci-plan`'s, which case 94 and the CI logs read.
//! The one place they part is the shapes, which this reader does not apply (the model's
//! header says so), and the case says why it sets those plans aside. Nothing here carries a
//! list of gates, of jobs or of paths: a gate added to the model is answered by every
//! surface the next time the model is read.
//!
//! The one deliberate difference from `ci-plan`'s matcher is documented on
//! [`Class::matches`].

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::discovery::glob::Glob;
use crate::metadata::yaml;

/// Where the CI model lives, repository-relative. The path `scripts/ci-plan` defaults to.
pub const MODEL_PATH: &str = ".ai/repo/ci/gates.yaml";

/// The word a class uses to say that any change in it escalates the whole plan.
const FULL: &str = "full";

// ---------------------------------------------------------------- the declaration

/// ```
/// use majordomus_cli::gates::Lane;
///
/// // the three lanes of ADR 0186, spelled as the model spells them
/// let lane: Lane = serde_json::from_value(serde_json::json!("feedback")).unwrap();
/// assert_eq!(lane, Lane::Feedback);
/// assert_eq!(Lane::Proof.as_str(), "proof");
/// // a word that is none of the three is no lane
/// assert!(serde_json::from_value::<Lane>(serde_json::json!("fast")).is_err());
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
/// The time horizon a gate's verdict belongs to (ADR 0186). The model's `tiers` name, for
/// each moment a plan is asked at, the lanes it runs.
pub enum Lane {
    /// Cheap, fast and decisive: a pull request's required verdict needs it green.
    Feedback,
    /// The heavy half, which judges the combined top before it lands.
    Proof,
    /// A gate only a release asks.
    Release,
}

impl Lane {
    /// Every lane, in the order the model's header names them.
    const ALL: [Lane; 3] = [Lane::Feedback, Lane::Proof, Lane::Release];

    /// The word the model spells the lane with.
    pub fn as_str(self) -> &'static str {
        match self {
            Lane::Feedback => "feedback",
            Lane::Proof => "proof",
            Lane::Release => "release",
        }
    }

    /// The lane a word names, if it names one.
    fn parse(word: &str) -> Option<Lane> {
        Lane::ALL.into_iter().find(|l| l.as_str() == word)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
/// What a class selects: the word `full`, or the gates it names.
pub enum GateClassSelects {
    /// `gates: full` — any change in this class escalates the whole plan.
    Word(String),
    /// `gates: [a, b]` — exactly these, and `gates: []` — nothing.
    Named(Vec<String>),
}

/// ```
/// use majordomus_cli::gates::GateDecl;
///
/// // one gate as `.ai/repo/ci/gates.yaml` declares it
/// let decl: GateDecl = serde_json::from_value(serde_json::json!({
///     "id": "shell-lint",
///     "job": "structure",
///     "runs": "shellcheck lib/*.sh",
///     "summary": "every script parses",
///     "always": true
/// }))
/// .unwrap();
/// assert_eq!(decl.id, "shell-lint");
/// // `always` is what makes every plan select it, so any change can invalidate its evidence
/// assert!(decl.always);
/// assert!(!decl.on_demand);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One gate of the model, as the file declares it.
pub struct GateDecl {
    /// The gate's identity, unique in the model.
    pub id: String,
    /// The CI job that runs it.
    pub job: String,
    /// The command it runs.
    pub runs: String,
    #[serde(default)]
    /// One line saying what it proves.
    pub summary: String,
    #[serde(default)]
    /// True when every plan selects it, whatever changed.
    pub always: bool,
    #[serde(default, rename = "on-demand")]
    /// True when only a plan that asks for it selects it: its runner cannot be had on
    /// demand, so planning it on every push is planning a verdict that never arrives.
    pub on_demand: bool,
    #[serde(default, rename = "at-finish")]
    /// True when the gate measures the deployment rather than the tree, so no change to
    /// any file can select it and the path classes can never reach it. `finish` asks it
    /// instead: the question "is what the public is being served still a projection of
    /// the trunk" has no answer a commit can give, and its moment is when a session
    /// claims to be done.
    pub at_finish: bool,
    #[serde(default)]
    /// Gates that are selected whenever this one is.
    pub implies: Vec<String>,
    #[serde(default)]
    /// Gates this one cannot run without.
    pub requires: Vec<String>,
    #[serde(default, rename = "subsumed-by")]
    /// The one other gate that runs everything this gate runs: a plan that selects that gate
    /// does not select this one beside it, and says so. It is how a gate that exists for the
    /// plans that do not run the suite (the case gates of the record shapes) stops running
    /// the same cases a second time in a plan that does. It is decided last, after `implies`
    /// and `requires` have closed the selection, so a subsuming gate that was only implied
    /// still subsumes; a subsuming gate the plan does not select subsumes nothing.
    ///
    /// ```
    /// use majordomus_cli::gates::GateDecl;
    ///
    /// let decl: GateDecl = serde_json::from_value(serde_json::json!({
    ///     "id": "plan-record-cases",
    ///     "job": "rust",
    ///     "runs": "bash test/run.sh 26_ci_wiring",
    ///     "subsumed-by": "shell-suite"
    /// }))
    /// .unwrap();
    /// // the field is spelled with a hyphen in the model, as `on-demand` and `at-finish` are
    /// assert_eq!(decl.subsumed_by.as_deref(), Some("shell-suite"));
    /// ```
    pub subsumed_by: Option<String>,
    #[serde(default)]
    /// The lane the gate's verdict belongs to (ADR 0186). Optional in a model that declares
    /// no `tiers`, which is read exactly as before; required of every gate once it does. A
    /// plan asked at a tier leaves out every gate whose lane the tier does not run (I3947).
    ///
    /// ```
    /// use majordomus_cli::gates::{GateDecl, Lane};
    ///
    /// let decl: GateDecl = serde_json::from_value(serde_json::json!({
    ///     "id": "shell-suite",
    ///     "job": "suite",
    ///     "runs": "bash test/run.sh",
    ///     "lane": "proof"
    /// }))
    /// .unwrap();
    /// assert_eq!(decl.lane, Some(Lane::Proof));
    /// ```
    pub lane: Option<Lane>,
}

/// ```
/// use majordomus_cli::gates::GateClass;
///
/// // one path class as the CI model declares it: which paths select which gates
/// let class: GateClass = serde_json::from_value(serde_json::json!({
///     "id": "docs",
///     "paths": ["docs/**"],
///     "gates": ["reference-check"]
/// }))
/// .unwrap();
/// assert_eq!(class.id, "docs");
/// assert_eq!(class.paths, ["docs/**"]);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One path class: the paths it covers and the gates a change to them selects.
pub struct GateClass {
    /// The class's identity.
    pub id: String,
    #[serde(default)]
    /// The pathspecs it covers, in the subset the model's header defines.
    pub paths: Vec<String>,
    #[serde(default)]
    /// Single paths its patterns cover and the class does not: a data file under an
    /// escalating directory, read by named gates only. Each one is a path, never a
    /// pattern, so a file that appears under the directory tomorrow still falls in it.
    pub except: Vec<String>,
    #[serde(default)]
    /// `full`, or the gates it names.
    pub gates: Option<GateClassSelects>,
}

impl GateClass {
    /// True when any change in this class escalates the whole plan.
    pub(crate) fn escalates(&self) -> bool {
        matches!(&self.gates, Some(GateClassSelects::Word(w)) if w == FULL)
    }

    /// The gates this class names; empty for `full` (which selects everything instead) and
    /// for `gates: []`.
    pub(crate) fn named(&self) -> &[String] {
        match &self.gates {
            Some(GateClassSelects::Named(g)) => g,
            _ => &[],
        }
    }

    /// Does one repository-relative path fall in this class?
    ///
    /// The pathspec subset is [`Glob`]'s, which is git's, which is what the model's header
    /// says the patterns mean. It differs from `scripts/ci-plan`'s matcher in exactly one
    /// place: `ci-plan` compiles `share/**` to a regular expression requiring a slash, so
    /// the bare directory `share` does not match, and `Glob` lets `**` stand for no
    /// segments, so it does. Nothing is decided over a directory here — the paths are
    /// `git diff --name-only`'s, which are files — so the two agree over every input either
    /// is given, and `test/cases/131_completion_gates.sh` asserts that on this model.
    ///
    /// A path the class excepts is not in it, whatever its patterns say; it still falls in
    /// every other class that matches it, and in none it is unclassified.
    pub(crate) fn matches(&self, path: &str) -> bool {
        !self.except.iter().any(|e| e == path)
            && self.paths.iter().any(|p| Glob::new(p).matches(path))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The CI model as `.ai/repo/ci/gates.yaml` declares it.
pub struct GateModel {
    /// The schema version; 1 is the only one.
    pub version: u32,
    /// Every gate, in declaration order.
    pub gates: Vec<GateDecl>,
    /// Every path class, in declaration order.
    pub classes: Vec<GateClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// For each tier a plan is asked at (`local`, `pull-request`, `integration`, `release`),
    /// the lanes it runs (ADR 0186). Absent in a model that declares no lanes.
    pub tiers: Option<BTreeMap<String, Vec<Lane>>>,
}

impl GateModel {
    /// Read the model from a repository root, or say why not.
    ///
    /// A repository with no model is not an error here: it is a repository whose gates
    /// cannot be known, which every caller reports as [`super::judge::GateStatus`]
    /// `unknown` rather than as a pass.
    pub(crate) fn load(root: &Path) -> Result<GateModel, String> {
        let path = root.join(MODEL_PATH);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("{MODEL_PATH} could not be read: {e}"))?;
        let raw =
            yaml::parse_mapping(&text).map_err(|e| format!("{MODEL_PATH} does not parse: {e}"))?;
        // the lanes are judged on the document as written, before it is typed: a lane that is
        // none of the three would otherwise be a parse error in serde's words, not ci-plan's
        if let Some(problem) = GateModel::lane_problems(&raw).into_iter().next() {
            return Err(format!("{MODEL_PATH}: {problem}"));
        }
        let model: GateModel = serde_json::from_value(Value::Object(raw))
            .map_err(|e| format!("{MODEL_PATH} does not parse: {e}"))?;
        if model.version != 1 {
            return Err(format!(
                "{MODEL_PATH}: version must be 1, not {}",
                model.version
            ));
        }
        if model.gates.is_empty() {
            return Err(format!("{MODEL_PATH} declares no gates"));
        }
        if let Some(problem) = model.subsumption_problems().into_iter().next() {
            return Err(format!("{MODEL_PATH}: {problem}"));
        }
        Ok(model)
    }

    /// What is wrong with the model's `subsumed-by` declarations, one line each; empty when
    /// nothing is. The three things `scripts/ci-plan --check` refuses, in its words: a gate
    /// subsumed by itself, by a gate that does not exist, and by a gate that is itself
    /// subsumed (a chain would drop a gate for a gate the plan then drops, and nothing would
    /// run the cases).
    pub(crate) fn subsumption_problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        for gate in &self.gates {
            let Some(sub) = gate.subsumed_by.as_deref() else {
                continue;
            };
            if sub == gate.id {
                out.push(format!("gate {} is subsumed by itself", gate.id));
            }
            match self.gate(sub) {
                None => out.push(format!(
                    "gate {} is subsumed by a gate that does not exist: {sub}",
                    gate.id
                )),
                Some(other) if other.subsumed_by.is_some() => out.push(format!(
                    "gate {} is subsumed by {sub}, which is itself subsumed; a chain is not a \
                     relation the planner applies",
                    gate.id
                )),
                Some(_) => {}
            }
        }
        out
    }

    /// What is wrong with the model's lanes and tiers, one line each, in `scripts/ci-plan
    /// --check`'s words and order: the gates in declaration order, then the tiers as written.
    /// A lane is feedback, proof or release whether or not tiers are declared; once they are,
    /// every gate declares one, `tiers` maps each tier to a list, and every lane a tier names
    /// is one of the three. A model with neither has nothing to answer here. A list or a map
    /// where a lane belongs is named as such, as ci-plan names it.
    pub(crate) fn lane_problems(raw: &Map<String, Value>) -> Vec<String> {
        fn named(v: &Value) -> String {
            match v {
                Value::Array(_) => "a list".to_string(),
                Value::Object(_) => "a map".to_string(),
                v => yaml::scalar_string(v).unwrap_or_default(),
            }
        }
        let mut out = Vec::new();
        let tiers = raw.get("tiers");
        let gates = raw.get("gates").and_then(Value::as_array);
        for gate in gates.into_iter().flatten() {
            let id = gate
                .get("id")
                .and_then(yaml::scalar_string)
                .unwrap_or_default();
            match gate.get("lane").map(named) {
                None if tiers.is_some() => out.push(format!(
                    "gate {id} declares no lane, and with tiers declared every gate is in one of \
                     feedback, proof and release"
                )),
                Some(word) if Lane::parse(&word).is_none() => out.push(format!(
                    "gate {id}: lane is feedback, proof or release, not {word}"
                )),
                _ => {}
            }
        }
        let Some(tiers) = tiers else {
            return out;
        };
        let numbered = |m: &Map<String, Value>| {
            m.keys()
                .any(|k| !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()))
        };
        let map = match tiers {
            Value::Object(m) if !numbered(m) => m,
            _ => {
                out.push("tiers maps each tier to a list of the lanes it runs".to_string());
                return out;
            }
        };
        for (tier, lanes) in map {
            let Value::Array(lanes) = lanes else {
                out.push(format!("tier {tier} names its lanes as a list"));
                continue;
            };
            for word in lanes.iter().map(named) {
                if Lane::parse(&word).is_none() {
                    out.push(format!(
                        "tier {tier} names a lane that does not exist: {word}"
                    ));
                }
            }
        }
        out
    }

    /// The tier a plan is asked at, resolved against the model's `tiers` (ADR 0186, I3947),
    /// or the usage error that refuses it, in `scripts/ci-plan --tier`'s words: a model that
    /// declares no tiers cannot plan one, and a tier it does not declare is named beside the
    /// ones it does, in byte order as ci-plan's `LC_ALL=C sort` lists them.
    pub(crate) fn tier<'a>(&'a self, name: &str) -> Result<Tier<'a>, String> {
        let Some(tiers) = &self.tiers else {
            return Err(format!(
                "tier {name}: the model declares no tiers, so it cannot plan one"
            ));
        };
        match tiers.get_key_value(name) {
            Some((name, lanes)) => Ok(Tier { name, lanes }),
            None => {
                let declared: Vec<&str> = tiers.keys().map(String::as_str).collect();
                Err(format!(
                    "tier {name}: the model declares no such tier; it declares {}",
                    declared.join(", ")
                ))
            }
        }
    }

    /// One gate by id.
    pub(crate) fn gate(&self, id: &str) -> Option<&GateDecl> {
        self.gates.iter().find(|g| g.id == id)
    }

    /// The pathspecs a gate's evidence is taken over: the union of the paths of every class
    /// that names it, plus every class that escalates (those select it too), and everything
    /// for a gate the model marks `always` — a gate every plan selects is a gate any change
    /// can invalidate, and saying otherwise would be evidence that never goes stale.
    ///
    /// Derived from the model and from nothing else: a class that starts naming a gate
    /// starts invalidating its evidence in the same commit.
    pub(crate) fn inputs_of(&self, id: &str) -> Vec<String> {
        if self.gate(id).is_some_and(|g| g.always) {
            return vec!["*".to_string()];
        }
        let mut out: BTreeSet<String> = BTreeSet::new();
        for class in &self.classes {
            if class.escalates() || class.named().iter().any(|g| g == id) {
                out.extend(class.paths.iter().cloned());
            }
        }
        out.into_iter().collect()
    }
}

// ---------------------------------------------------------------- the plan

/// A tier of the model, resolved by [`GateModel::tier`]: its name and the lanes it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tier<'a> {
    name: &'a str,
    lanes: &'a [Lane],
}

impl Tier<'_> {
    /// Why this tier leaves a gate out, in `scripts/ci-plan`'s words, or `None` when it runs
    /// the gate's lane. A gate that declares no lane is not left out: a model whose tiers are
    /// declared does not load with one (see [`GateModel::lane_problems`]).
    fn excludes(&self, gate: &GateDecl) -> Option<String> {
        let lane = gate.lane.filter(|l| !self.lanes.contains(l))?;
        Some(format!(
            "lane {}: not in the {} tier",
            lane.as_str(),
            self.name
        ))
    }
}

/// ```
/// use majordomus_cli::gates::GatePlanMode;
///
/// // a plan is either the gates the change selects, or everything the runners allow
/// assert_eq!(GatePlanMode::Affected.as_str(), "affected");
/// assert_eq!(GatePlanMode::Full.as_str(), "full");
/// assert_ne!(GatePlanMode::Affected, GatePlanMode::Full);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
/// Why the plan selects what it selects.
pub enum GatePlanMode {
    /// The gates of the classes the changed paths fall in.
    Affected,
    /// Every gate but the on-demand ones: asked for, or escalated to.
    Full,
}

impl GatePlanMode {
    /// The word this mode is reported under; `scripts/ci-plan`'s own.
    ///
    /// ```
    /// use majordomus_cli::gates::GatePlanMode;
    /// // the same words the shell planner prints, because there is one model and two readers
    /// assert_eq!(GatePlanMode::Full.as_str(), "full");
    /// assert_eq!(GatePlanMode::Affected.as_str(), "affected");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            GatePlanMode::Affected => "affected",
            GatePlanMode::Full => "full",
        }
    }
}

/// ```
/// use majordomus_cli::gates::GateClassMatch;
///
/// let hit = GateClassMatch { id: "docs".into(), paths: vec!["docs/CLI.md".into()] };
/// // the class, and the files of the change that put it in the plan — never the class alone,
/// // because a plan a reader cannot check is a plan a reader has to believe
/// assert_eq!(hit.paths.len(), 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One class a change fell in, and the paths that put it there.
pub struct GateClassMatch {
    /// The class's identity.
    pub id: String,
    /// The changed paths this class covers.
    pub paths: Vec<String>,
}

/// ```
/// use majordomus_cli::gates::{GateClassMatch, GatePlan, GatePlanMode};
/// use std::collections::BTreeMap;
///
/// let plan = GatePlan {
///     mode: GatePlanMode::Affected,
///     reason: "3 path(s) fall in 1 class".into(),
///     changed: vec!["docs/CLI.md".into()],
///     classes: vec![GateClassMatch { id: "docs".into(), paths: vec!["docs/CLI.md".into()] }],
///     unclassified: Vec::new(),
///     selected: BTreeMap::from([("reference-check".to_string(), "class docs".to_string())]),
///     excluded: BTreeMap::new(),
/// };
/// // the plan says what it selected and why, so a reader can check it rather than believe it
/// assert_eq!(plan.selected["reference-check"], "class docs");
/// assert!(!plan.selected.contains_key("rust-check"), "a document change cannot break the crate");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// Which gates a change must pass, and why each one is in or out.
pub struct GatePlan {
    /// Affected, or full.
    pub mode: GatePlanMode,
    /// Why, in words.
    pub reason: String,
    /// The changed paths the plan was computed over, sorted.
    pub changed: Vec<String>,
    /// The classes they fall in.
    pub classes: Vec<GateClassMatch>,
    /// Changed paths no class of the model covers; each one escalates the plan.
    pub unclassified: Vec<String>,
    /// Gate id to the reason it was selected, for the gates in the plan.
    pub selected: BTreeMap<String, String>,
    /// Gate id to the reason it was left out, for the gates that are not.
    pub excluded: BTreeMap<String, String>,
}

impl GatePlan {
    /// Is this gate in the plan?
    pub(crate) fn selects(&self, id: &str) -> bool {
        self.selected.contains_key(id)
    }
}

/// The plan for a change set, by the rules the model's own header states: `always` gates
/// run in every plan; a changed path selects the gates of every class it falls in (the
/// union, never the first match); a class whose gates are `full` and a path no class covers
/// both escalate the whole plan; `implies` adds a gate whenever its parent is selected;
/// `requires` names a gate a gate cannot run without, and the plan adds it; and an
/// on-demand gate is left out of every plan that did not ask for it, the full one included;
/// and, last of all, a gate whose `subsumed-by` gate is selected is dropped, because the
/// selected gate runs every case it runs.
///
/// A plan asked at a `tier` (ADR 0186, I3947) first leaves out every gate whose lane the tier
/// does not run, before anything else is decided about it: the on-demand rule, `implies` and
/// `requires` (neither brings it back), and `subsumed-by` (a subsuming gate the tier left out
/// is not in the plan, so it subsumes nothing and the gate it would have replaced still
/// runs). With no tier every lane is planned, exactly as before lanes existed.
///
/// Every reason, the plan's and each gate's, is `scripts/ci-plan`'s word for word, because a
/// reader of a CI log and a reader of `gates done` are told one thing: a class names its
/// classes (`class a b`), an escalation names the first three paths that escalated, in the
/// order the change gave them, and a full plan that leaves the on-demand gates out says so.
///
/// The worked example is `this_repositorys_own_model_plans_by_its_own_rules` below: it reads
/// this repository's `.ai/repo/ci/gates.yaml` when there is one, which a doctest cannot do
/// now that the model is the crate's own.
pub(crate) fn plan(
    model: &GateModel,
    changed: &[String],
    on_demand: bool,
    tier: Option<&Tier<'_>>,
) -> GatePlan {
    let mut sorted: Vec<String> = changed
        .iter()
        .filter(|p| !p.trim().is_empty())
        .cloned()
        .collect();
    crate::order::canonical(&mut sorted);
    sorted.dedup();

    let mut classes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut unclassified: Vec<String> = Vec::new();
    let mut from_classes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for path in &sorted {
        let mut hit = false;
        // the union, never the first match: an overlap can only widen a plan
        for class in &model.classes {
            if !class.matches(path) {
                continue;
            }
            hit = true;
            classes
                .entry(class.id.clone())
                .or_default()
                .push(path.clone());
            for gate in class.named() {
                from_classes
                    .entry(gate.clone())
                    .or_default()
                    .insert(class.id.clone());
            }
        }
        if !hit {
            unclassified.push(path.clone());
        }
    }

    // The paths an escalation names are `ci-plan`'s: at most three, in the order the change
    // gave them and not sorted, one per escalating class a path falls in. A plan of
    // `git diff --name-only` is in git's order either way; a listed change is in its own.
    let given = changed.iter().filter(|p| !p.trim().is_empty());
    let escalating: Vec<&str> = given
        .clone()
        .flat_map(|p| {
            model
                .classes
                .iter()
                .filter(|c| c.escalates() && c.matches(p))
                .map(move |_| p.as_str())
        })
        .take(3)
        .collect();
    let nowhere: Vec<&str> = given
        .filter(|p| !model.classes.iter().any(|c| c.matches(p)))
        .map(String::as_str)
        .take(3)
        .collect();

    let (mode, mut reason) = if !escalating.is_empty() {
        (
            GatePlanMode::Full,
            format!(
                "a changed path is in a class that escalates: {}",
                escalating.join(" ")
            ),
        )
    } else if !nowhere.is_empty() {
        (
            GatePlanMode::Full,
            format!(
                "a changed path is in no class of the model: {}",
                nowhere.join(" ")
            ),
        )
    } else if sorted.is_empty() {
        (
            GatePlanMode::Affected,
            "no changed path; only the gates that always run".to_string(),
        )
    } else {
        (
            GatePlanMode::Affected,
            "the gates of the classes the changed paths fall in".to_string(),
        )
    };
    // a full plan that leaves the on-demand gates out says which, in byte order as
    // `ci-plan`'s `LC_ALL=C sort` lists them
    let on_demand_ids: BTreeSet<&str> = model
        .gates
        .iter()
        .filter(|g| g.on_demand)
        .map(|g| g.id.as_str())
        .collect();
    if mode == GatePlanMode::Full && !on_demand && !on_demand_ids.is_empty() {
        let ids: Vec<&str> = on_demand_ids.into_iter().collect();
        reason = format!("{reason} (without the on-demand gates: {})", ids.join(" "));
    }

    let mut selected: BTreeMap<String, String> = BTreeMap::new();
    let mut excluded: BTreeMap<String, String> = BTreeMap::new();
    // the order gates entered the plan, which is the order the closure below walks them in,
    // as `ci-plan` walks its own list: it decides which parent a gate reachable from two is
    // said to be implied or required by
    let mut order: Vec<String> = Vec::new();
    for gate in &model.gates {
        if let Some(why) = tier.and_then(|t| t.excludes(gate)) {
            excluded.insert(gate.id.clone(), why);
            continue;
        }
        if gate.on_demand && !on_demand {
            excluded.insert(
                gate.id.clone(),
                "on demand only: its runner is not available on demand; the schedule, a \
                 dispatch and a pull request labelled ci:full plan it"
                    .to_string(),
            );
            continue;
        }
        let why = if mode == GatePlanMode::Full {
            "full".to_string()
        } else if gate.always {
            "always".to_string()
        } else if let Some(by) = from_classes.get(&gate.id) {
            let by: Vec<&str> = by.iter().map(String::as_str).collect();
            format!("class {}", by.join(" "))
        } else {
            excluded.insert(
                gate.id.clone(),
                "no changed path in a class that needs it".to_string(),
            );
            continue;
        };
        selected.insert(gate.id.clone(), why);
        order.push(gate.id.clone());
    }

    // `implies` and `requires`, to a fixed point: a full Rust gate includes the registry
    // checks, and the probe cannot run without the built site
    loop {
        let mut added = false;
        for id in order.clone() {
            let Some(gate) = model.gate(&id) else {
                continue;
            };
            let implied = gate.implies.iter().map(|g| (g, "implied by"));
            let required = gate.requires.iter().map(|g| (g, "required by"));
            for (other, how) in implied.chain(required) {
                if selected.contains_key(other) {
                    continue;
                }
                let Some(decl) = model.gate(other) else {
                    continue;
                };
                if decl.on_demand && !on_demand {
                    continue;
                }
                // nothing implies or requires its way past the tier asked for
                if tier.is_some_and(|t| t.excludes(decl).is_some()) {
                    continue;
                }
                excluded.remove(other);
                selected.insert(other.clone(), format!("{how} {}", gate.id));
                order.push(other.clone());
                added = true;
            }
        }
        if !added {
            break;
        }
    }

    // a gate whose subsuming gate is in the plan is not planned beside it. This is the last
    // step, after the closure and the on-demand filter, so a subsuming gate that was only
    // implied still counts; the reason replaces the one that selected the gate, because the
    // plan must say why a gate is absent, not why it once was present. The words are
    // `scripts/ci-plan`'s, byte for byte, so a reader comparing the reasons agrees too.
    let subsumed: Vec<(String, String)> = model
        .gates
        .iter()
        .filter(|g| selected.contains_key(&g.id))
        .filter_map(|g| {
            let sub = g.subsumed_by.as_ref()?;
            selected
                .contains_key(sub)
                .then(|| (g.id.clone(), sub.clone()))
        })
        .collect();
    for (id, sub) in subsumed {
        selected.remove(&id);
        excluded.insert(
            id,
            format!(
                "subsumed by {sub}: the planned {sub} runs every case this gate runs, so \
                 planning it too would run them twice"
            ),
        );
    }

    GatePlan {
        mode,
        reason,
        changed: sorted,
        classes: classes
            .into_iter()
            .map(|(id, mut paths)| {
                crate::order::canonical(&mut paths);
                paths.dedup();
                GateClassMatch { id, paths }
            })
            .collect(),
        unclassified,
        selected,
        excluded,
    }
}

#[cfg(test)]
mod tests {
    /// The rules the header states, over this repository's own model rather than a fixture:
    /// a documentation change plans the document gates and not the crate's, and a path the
    /// model classifies nowhere escalates the whole plan rather than passing quietly.
    #[test]
    fn this_repositorys_own_model_plans_by_its_own_rules() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let Ok(model) = GateModel::load(&root) else {
            // a checkout with no CI model has nothing to assert about one
            return;
        };
        let p = plan(&model, &["docs/README.md".to_string()], false, None);
        assert_eq!(p.mode, GatePlanMode::Affected);
        assert!(
            !p.selects("rust-check"),
            "a document does not rebuild the crate"
        );

        let p = plan(&model, &["nowhere/at/all.txt".to_string()], false, None);
        assert_eq!(p.mode, GatePlanMode::Full, "an unclassified path escalates");
    }

    use super::*;

    const MODEL: &str = r#"version: 1
gates:
  - id: lint
    job: structure
    always: true
    runs: scripts/lint
    summary: every script parses
  - id: build
    job: rust
    implies: [registry]
    runs: cargo build
    summary: the crate builds
  - id: registry
    job: rust
    runs: cargo run -- generate --check
    summary: the projections are current
  - id: probe
    job: site
    requires: [site]
    runs: scripts/probe
    summary: every route answers
  - id: site
    job: site
    runs: scripts/site-build
    summary: the site builds
  - id: mac
    job: macos
    on-demand: true
    runs: bash test/run.sh
    summary: the suite on macOS
classes:
  - id: pipeline
    paths: [.ai/repo/ci/**]
    gates: full
  - id: rust
    paths: [apps/**]
    gates: [build]
  - id: web
    paths: [site/**]
    gates: [site, probe]
  - id: inert
    paths: [.gitignore]
    gates: []
"#;

    fn model() -> GateModel {
        yaml::parse_into(MODEL).expect("the fixture model parses")
    }

    #[test]
    fn an_excepted_path_leaves_its_class_and_falls_in_the_others() {
        let mut m = model();
        let pipeline = m.classes.iter_mut().find(|c| c.id == "pipeline").unwrap();
        pipeline.except = vec![".ai/repo/ci/durations.tsv".into()];
        m.classes.push(GateClass {
            id: "pipeline-data".into(),
            paths: vec![".ai/repo/ci/durations.tsv".into()],
            except: vec![],
            gates: Some(GateClassSelects::Named(vec!["build".into()])),
        });
        let p = plan(&m, &[".ai/repo/ci/durations.tsv".into()], false, None);
        assert_eq!(
            p.mode,
            GatePlanMode::Affected,
            "a named data file does not escalate"
        );
        assert!(p.selects("build"), "its readers' class plans it");
        assert!(!p.selects("site"), "and nothing else");

        // the directory stays escalating around the one path it names
        let p = plan(&m, &[".ai/repo/ci/gates.yaml".into()], false, None);
        assert_eq!(p.mode, GatePlanMode::Full);
        let p = plan(&m, &[".ai/repo/ci/added-tomorrow.tsv".into()], false, None);
        assert_eq!(
            p.mode,
            GatePlanMode::Full,
            "an unnamed file still escalates"
        );
    }

    #[test]
    fn an_always_gate_is_in_every_plan_and_a_class_gate_only_in_its_own() {
        let m = model();
        let p = plan(&m, &["apps/majordomus-cli/src/lib.rs".into()], false, None);
        assert_eq!(p.mode, GatePlanMode::Affected);
        assert!(p.selects("lint"), "always");
        assert!(p.selects("build"), "the rust class names it");
        assert!(p.selects("registry"), "build implies it");
        assert!(!p.selects("site"), "no changed path is the site's");
    }

    #[test]
    fn a_required_gate_is_planned_with_the_gate_that_needs_it() {
        let m = model();
        let p = plan(&m, &["site/content/index.md".into()], false, None);
        assert!(p.selects("probe"));
        assert!(p.selects("site"), "probe cannot run without it");
        assert_eq!(p.selected["site"], "class web");
    }

    #[test]
    fn an_escalating_class_and_an_unclassified_path_both_widen_the_plan() {
        let m = model();
        let p = plan(&m, &[".ai/repo/ci/gates.yaml".into()], false, None);
        assert_eq!(p.mode, GatePlanMode::Full);
        assert!(p.selects("build") && p.selects("site"));

        let p = plan(&m, &["something/nobody/declared".into()], false, None);
        assert_eq!(p.mode, GatePlanMode::Full);
        assert_eq!(
            p.unclassified,
            vec!["something/nobody/declared".to_string()]
        );
    }

    #[test]
    fn an_on_demand_gate_is_left_out_of_the_full_plan_and_says_why() {
        let m = model();
        let p = plan(&m, &[".ai/repo/ci/gates.yaml".into()], false, None);
        assert!(!p.selects("mac"));
        assert!(p.excluded["mac"].contains("on demand"));
        // and it is in the plan that asked for it
        assert!(plan(&m, &[".ai/repo/ci/gates.yaml".into()], true, None).selects("mac"));
    }

    // The reasons below are `scripts/ci-plan`'s, word for word; case 131 compares the two
    // readers over this repository's model, and these say what each word is for.

    #[test]
    fn a_class_reason_names_every_class_that_selected_the_gate_in_byte_order() {
        let mut m = model();
        m.classes.push(GateClass {
            id: "site-sources".into(),
            paths: vec!["site/**".into()],
            except: vec![],
            gates: Some(GateClassSelects::Named(vec!["site".into()])),
        });
        let p = plan(&m, &["site/content/index.md".into()], false, None);
        assert_eq!(p.selected["site"], "class site-sources web");
        assert_eq!(p.selected["probe"], "class web");
        assert_eq!(p.selected["lint"], "always");
    }

    #[test]
    fn an_implied_and_a_required_gate_name_the_gate_that_brought_them() {
        let mut m = model();
        m.classes.push(GateClass {
            id: "probes".into(),
            paths: vec!["probes/**".into()],
            except: vec![],
            gates: Some(GateClassSelects::Named(vec!["probe".into()])),
        });
        let p = plan(&m, &["probes/x.sh".into(), "apps/x.rs".into()], false, None);
        assert_eq!(p.selected["probe"], "class probes");
        assert_eq!(p.selected["site"], "required by probe");
        assert_eq!(p.selected["registry"], "implied by build");
    }

    #[test]
    fn an_escalation_names_the_first_three_paths_in_the_order_given() {
        let m = model();
        let given: Vec<String> = ["d", "b", "c", "a"]
            .iter()
            .map(|n| format!(".ai/repo/ci/{n}"))
            .collect();
        let p = plan(&m, &given, false, None);
        assert_eq!(
            p.reason,
            "a changed path is in a class that escalates: .ai/repo/ci/d .ai/repo/ci/b \
             .ai/repo/ci/c (without the on-demand gates: mac)"
        );
        // every gate of a full plan is in it because the plan is full, as `ci-plan` says
        assert_eq!(p.selected["build"], "full");
        assert_eq!(p.selected["lint"], "full");
        // a plan that asked for the on-demand gates has no note to add, and no trailing space
        let p = plan(&m, &given[..1], true, None);
        assert_eq!(
            p.reason,
            "a changed path is in a class that escalates: .ai/repo/ci/d"
        );
    }

    #[test]
    fn an_unclassified_escalation_names_three_paths_and_the_plan_keeps_them_all() {
        let m = model();
        let given: Vec<String> = ["z/1", "y/2", "x/3", "w/4"]
            .iter()
            .map(|p| p.to_string())
            .collect();
        let p = plan(&m, &given, false, None);
        assert_eq!(
            p.reason,
            "a changed path is in no class of the model: z/1 y/2 x/3 (without the on-demand \
             gates: mac)"
        );
        assert_eq!(p.unclassified, ["w/4", "x/3", "y/2", "z/1"]);
    }

    #[test]
    fn a_gate_left_out_says_why_in_the_planners_words() {
        let m = model();
        let p = plan(&m, &["apps/x.rs".into()], false, None);
        assert_eq!(
            p.excluded["site"],
            "no changed path in a class that needs it"
        );
        assert_eq!(
            p.excluded["mac"],
            "on demand only: its runner is not available on demand; the schedule, a dispatch \
             and a pull request labelled ci:full plan it"
        );
        // an affected plan carries no on-demand note: it was never going to run everything
        assert_eq!(
            p.reason,
            "the gates of the classes the changed paths fall in"
        );
    }

    #[test]
    fn a_class_that_names_no_gate_plans_only_what_always_runs() {
        let m = model();
        let p = plan(&m, &[".gitignore".into()], false, None);
        assert_eq!(p.mode, GatePlanMode::Affected, "the class covers the path");
        assert!(p.selects("lint"));
        assert!(!p.selects("build") && !p.selects("site"));
    }

    /// The fixture model with `build` subsumed by `site`, to say what the field does.
    fn subsuming() -> GateModel {
        let mut m = model();
        m.gates
            .iter_mut()
            .find(|g| g.id == "build")
            .unwrap()
            .subsumed_by = Some("site".into());
        m
    }

    #[test]
    fn a_gate_is_not_planned_beside_the_selected_gate_that_subsumes_it() {
        let m = subsuming();
        // the full plan selects both; the subsumed one is dropped and says why
        let p = plan(&m, &[".ai/repo/ci/gates.yaml".into()], false, None);
        assert!(p.selects("site") && !p.selects("build"));
        assert_eq!(
            p.excluded["build"],
            "subsumed by site: the planned site runs every case this gate runs, so planning \
             it too would run them twice"
        );
        // a subsuming gate that is only required still subsumes: the closure comes first
        let p = plan(
            &m,
            &["site/content/index.md".into(), "apps/x.rs".into()],
            false,
            None,
        );
        assert_eq!(p.selected["site"], "class web");
        assert!(!p.selects("build"));
        // and what the dropped gate implied stays: it was selected, and so was its implied gate
        assert!(p.selects("registry"));
    }

    #[test]
    fn a_subsuming_gate_the_plan_does_not_select_subsumes_nothing() {
        let m = subsuming();
        let p = plan(&m, &["apps/majordomus-cli/src/lib.rs".into()], false, None);
        assert!(!p.selects("site"), "the plan does not select the subsumer");
        assert!(
            p.selects("build"),
            "so the gate is planned as without the field"
        );
        assert_eq!(p.selected["build"], "class rust");
        assert!(!p.excluded.contains_key("build"));
    }

    #[test]
    fn a_subsumption_the_planner_cannot_apply_is_refused() {
        assert!(subsuming().subsumption_problems().is_empty());

        let mut m = model();
        m.gates[1].subsumed_by = Some("build".into());
        let problems = m.subsumption_problems();
        assert!(problems.contains(&"gate build is subsumed by itself".to_string()));

        let mut m = model();
        m.gates[1].subsumed_by = Some("no-such-gate".into());
        assert_eq!(
            m.subsumption_problems(),
            vec!["gate build is subsumed by a gate that does not exist: no-such-gate".to_string()]
        );

        let mut m = model();
        m.gates[1].subsumed_by = Some("site".into());
        m.gates[4].subsumed_by = Some("registry".into());
        let problems = m.subsumption_problems();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("gate build is subsumed by site, which is itself subsumed"));
    }

    #[test]
    fn a_model_whose_subsumption_cannot_be_applied_is_not_loaded() {
        let root = tempfile::tempdir().expect("a directory");
        let path = root.path().join(MODEL_PATH);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        // `build` names a subsumer that no gate declares: the model parses and is refused
        let text = MODEL.replace(
            "    implies: [registry]\n",
            "    implies: [registry]\n    subsumed-by: no-such-gate\n",
        );
        assert_ne!(text, MODEL, "the fixture gained the field");
        std::fs::write(&path, &text).expect("a model");
        let err = GateModel::load(root.path()).expect_err("the subsumption is refused");
        assert_eq!(
            err,
            format!(
                "{MODEL_PATH}: gate build is subsumed by a gate that does not exist: no-such-gate"
            )
        );
        // without the field the same model loads
        std::fs::write(&path, MODEL).expect("a model");
        assert!(GateModel::load(root.path()).is_ok());
    }

    #[test]
    fn an_always_gate_is_invalidated_by_anything_and_a_class_gate_by_its_class() {
        let m = model();
        assert_eq!(m.inputs_of("lint"), vec!["*".to_string()]);
        // the escalating class selects every gate, so its paths invalidate every gate
        let build = m.inputs_of("build");
        assert!(build.contains(&"apps/**".to_string()));
        assert!(build.contains(&".ai/repo/ci/**".to_string()));
        assert!(!build.contains(&"site/**".to_string()));
    }

    /// The fixture model with every gate in a lane and the tiers of ADR 0186 declared.
    fn laned() -> String {
        let mut text = String::new();
        for line in MODEL.lines() {
            text.push_str(line);
            text.push('\n');
            if let Some(job) = line.strip_prefix("    job: ") {
                let lane = if job == "structure" {
                    "feedback"
                } else {
                    "proof"
                };
                text.push_str(&format!("    lane: {lane}\n"));
            }
            if line == "version: 1" {
                text.push_str(
                    "tiers:\n  local: [feedback]\n  pull-request: [feedback]\n  \
                     integration: [feedback, proof]\n  release: [feedback, proof, release]\n",
                );
            }
        }
        text
    }

    fn lane_problems_of(text: &str) -> Vec<String> {
        GateModel::lane_problems(&yaml::parse_mapping(text).expect("the fixture parses"))
    }

    #[test]
    fn a_laned_model_parses_its_lanes_and_tiers() {
        let text = laned();
        assert_eq!(lane_problems_of(&text), Vec::<String>::new());
        let m: GateModel = yaml::parse_into(&text).expect("the laned fixture parses");
        assert_eq!(m.gate("lint").and_then(|g| g.lane), Some(Lane::Feedback));
        assert_eq!(m.gate("mac").and_then(|g| g.lane), Some(Lane::Proof));
        let tiers = m.tiers.as_ref().expect("the tiers are read");
        assert_eq!(tiers["pull-request"], [Lane::Feedback]);
        assert_eq!(tiers["integration"], [Lane::Feedback, Lane::Proof]);
        assert_eq!(
            tiers["release"],
            [Lane::Feedback, Lane::Proof, Lane::Release]
        );
        assert_eq!(Lane::Release.as_str(), "release");
        // with no tier asked for, the lanes select nothing: every plan is the plan without them
        for changed in [
            ".ai/repo/ci/gates.yaml",
            "apps/x.rs",
            "site/a.html",
            ".gitignore",
        ] {
            let with = plan(&m, &[changed.to_string()], false, None);
            let without = plan(&model(), &[changed.to_string()], false, None);
            assert_eq!(with.selected, without.selected, "{changed}");
            assert_eq!(with.excluded, without.excluded, "{changed}");
            assert_eq!(with.reason, without.reason, "{changed}");
        }
    }

    #[test]
    fn a_model_without_tiers_needs_no_lane() {
        assert_eq!(lane_problems_of(MODEL), Vec::<String>::new());
        let m = model();
        assert!(m.tiers.is_none() && m.gates.iter().all(|g| g.lane.is_none()));
    }

    #[test]
    fn a_gate_without_a_lane_is_refused_once_tiers_are_declared() {
        let text = laned().replacen("    lane: proof\n", "", 1);
        assert_eq!(
            lane_problems_of(&text),
            vec![
                "gate build declares no lane, and with tiers declared every gate is in one of \
                 feedback, proof and release"
                    .to_string()
            ]
        );
    }

    #[test]
    fn a_lane_that_is_none_of_the_three_is_refused_with_or_without_tiers() {
        let text = laned().replacen("    lane: feedback\n", "    lane: fast\n", 1);
        let want = vec!["gate lint: lane is feedback, proof or release, not fast".to_string()];
        assert_eq!(lane_problems_of(&text), want);
        let untiered = MODEL.replacen(
            "    job: structure\n",
            "    job: structure\n    lane: fast\n",
            1,
        );
        assert_eq!(lane_problems_of(&untiered), want);
        // a list where a lane belongs is named as one, as ci-plan names it
        let listed = laned().replacen("    lane: feedback\n", "    lane: [feedback]\n", 1);
        assert_eq!(
            lane_problems_of(&listed),
            vec!["gate lint: lane is feedback, proof or release, not a list".to_string()]
        );
    }

    #[test]
    fn a_tier_naming_a_lane_that_does_not_exist_is_refused() {
        let text = laned().replace(
            "  integration: [feedback, proof]",
            "  integration: [feedback, lane-x]",
        );
        assert_eq!(
            lane_problems_of(&text),
            vec!["tier integration names a lane that does not exist: lane-x".to_string()]
        );
        let scalar = laned().replace("  local: [feedback]", "  local: feedback");
        assert_eq!(
            lane_problems_of(&scalar),
            vec!["tier local names its lanes as a list".to_string()]
        );
        let listed = MODEL.replacen("gates:\n", "tiers: [feedback]\ngates:\n", 1);
        let problems = lane_problems_of(&listed);
        assert_eq!(
            problems.last().map(String::as_str),
            Some("tiers maps each tier to a list of the lanes it runs")
        );
    }

    #[test]
    fn loading_refuses_a_lane_problem_in_ci_plans_words() {
        let dir = tempfile::tempdir().unwrap();
        let ci = dir.path().join(".ai/repo/ci");
        std::fs::create_dir_all(&ci).unwrap();
        std::fs::write(ci.join("gates.yaml"), laned()).unwrap();
        let m = GateModel::load(dir.path()).expect("the laned model loads");
        assert!(m.gates.iter().all(|g| g.lane.is_some()));
        let bad = laned().replacen("    lane: feedback\n", "    lane: fast\n", 1);
        std::fs::write(ci.join("gates.yaml"), bad).unwrap();
        assert_eq!(
            GateModel::load(dir.path()).unwrap_err(),
            format!("{MODEL_PATH}: gate lint: lane is feedback, proof or release, not fast")
        );
    }

    /// The laned fixture with these gates in the feedback lane and every other gate in proof.
    fn tiered(feedback: &[&str]) -> GateModel {
        let mut m: GateModel = yaml::parse_into(&laned()).expect("the laned fixture parses");
        for g in &mut m.gates {
            g.lane = Some(if feedback.contains(&g.id.as_str()) {
                Lane::Feedback
            } else {
                Lane::Proof
            });
        }
        m
    }

    #[test]
    fn a_tier_is_resolved_or_refused_in_ci_plans_words() {
        let m = tiered(&["lint"]);
        let t = m.tier("integration").expect("a declared tier resolves");
        assert_eq!(t.name, "integration");
        assert_eq!(t.lanes, [Lane::Feedback, Lane::Proof]);
        assert_eq!(
            m.tier("nightly").unwrap_err(),
            "tier nightly: the model declares no such tier; it declares integration, local, \
             pull-request, release"
        );
        // a model with no tiers cannot plan one, whatever its name
        assert_eq!(
            model().tier("local").unwrap_err(),
            "tier local: the model declares no tiers, so it cannot plan one"
        );
    }

    #[test]
    fn a_tier_leaves_out_the_gates_of_a_lane_it_does_not_run_and_says_why() {
        let m = tiered(&["lint"]);
        let pr = m.tier("pull-request").unwrap();
        let p = plan(&m, &["apps/x.rs".into()], false, Some(&pr));
        assert!(
            p.selects("lint"),
            "a feedback gate runs in the pull-request tier"
        );
        assert_eq!(
            p.excluded["build"],
            "lane proof: not in the pull-request tier"
        );
        // the tier is decided before the on-demand rule, so it is the reason a gate gives
        assert_eq!(
            p.excluded["mac"],
            "lane proof: not in the pull-request tier"
        );
        // a tier that runs the lane plans it exactly as no tier does
        let integration = m.tier("integration").unwrap();
        for changed in [".ai/repo/ci/gates.yaml", "apps/x.rs", "site/a.html"] {
            let with = plan(&m, &[changed.to_string()], false, Some(&integration));
            let without = plan(&m, &[changed.to_string()], false, None);
            assert_eq!(with.selected, without.selected, "{changed}");
            assert_eq!(with.excluded, without.excluded, "{changed}");
        }
        // and --on-demand cannot bring back a gate the tier leaves out
        assert!(!plan(&m, &[".ai/repo/ci/gates.yaml".into()], true, Some(&pr)).selects("mac"));
    }

    #[test]
    fn neither_implies_nor_requires_brings_back_a_gate_the_tier_left_out() {
        // build (feedback) implies registry (proof); probe (feedback) requires site (proof)
        let m = tiered(&["lint", "build", "probe"]);
        let pr = m.tier("pull-request").unwrap();
        let p = plan(
            &m,
            &["apps/x.rs".into(), "site/a.html".into()],
            false,
            Some(&pr),
        );
        assert!(p.selects("build") && p.selects("probe"));
        assert!(
            !p.selects("registry"),
            "an implication does not pass the tier"
        );
        assert!(!p.selects("site"), "a requirement does not pass the tier");
        // each says the tier left it out, which is decided before anything else about it
        assert_eq!(
            p.excluded["site"],
            "lane proof: not in the pull-request tier"
        );
        assert_eq!(
            p.excluded["registry"],
            "lane proof: not in the pull-request tier"
        );
        // without the tier both are brought in, by the gate that names them
        let p = plan(&m, &["apps/x.rs".into(), "site/a.html".into()], false, None);
        assert_eq!(p.selected["registry"], "implied by build");
        assert_eq!(p.selected["site"], "class web");
    }

    #[test]
    fn a_subsuming_gate_the_tier_leaves_out_subsumes_nothing() {
        // build (feedback) is subsumed by site (proof)
        let mut m = tiered(&["lint", "build"]);
        m.gates
            .iter_mut()
            .find(|g| g.id == "build")
            .unwrap()
            .subsumed_by = Some("site".into());
        let changed = ["site/a.html".to_string(), "apps/x.rs".to_string()];
        assert!(!plan(&m, &changed, false, None).selects("build"));
        let pr = m.tier("pull-request").unwrap();
        let p = plan(&m, &changed, false, Some(&pr));
        assert_eq!(
            p.selected["build"], "class rust",
            "nothing runs its cases otherwise"
        );
        assert!(!p.selects("site"));
    }

    #[test]
    fn a_gate_without_a_lane_is_not_left_out_by_a_tier() {
        // a model built in code, past the loader that refuses this
        let mut m = tiered(&["lint"]);
        m.gates.iter_mut().find(|g| g.id == "build").unwrap().lane = None;
        let pr = m.tier("pull-request").unwrap();
        let p = plan(&m, &["apps/x.rs".into()], false, Some(&pr));
        assert_eq!(p.selected["build"], "class rust");
    }

    #[test]
    fn the_repositorys_own_model_reads_and_every_reference_resolves() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let Ok(m) = GateModel::load(&root) else {
            return; // an installed crate has no repository model; that is `unknown`, not a failure
        };
        for gate in &m.gates {
            for other in gate.implies.iter().chain(gate.requires.iter()) {
                assert!(m.gate(other).is_some(), "{} names {other}", gate.id);
            }
            assert!(!gate.job.is_empty() && !gate.runs.is_empty());
        }
        assert_eq!(m.subsumption_problems(), Vec::<String>::new());
        // ADR 0186: the model declares its tiers, so every gate is in a lane
        if m.tiers.is_some() {
            let unlaned: Vec<&str> = m
                .gates
                .iter()
                .filter(|g| g.lane.is_none())
                .map(|g| g.id.as_str())
                .collect();
            assert!(unlaned.is_empty(), "gates without a lane: {unlaned:?}");
        }
        for class in &m.classes {
            assert!(!class.paths.is_empty(), "class {} names no path", class.id);
            for gate in class.named() {
                assert!(m.gate(gate).is_some(), "class {} names {gate}", class.id);
            }
        }
    }
}
