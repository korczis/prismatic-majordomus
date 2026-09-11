//! The CI model, typed: the gates this repository declares, the path classes that select
//! them, and the applicability the two derive together.
//!
//! `.ai/repo/ci/gates.yaml` is the canonical declaration and this is a reader of it, not a
//! second copy. `scripts/ci-plan` is the other reader; both take the same file, apply the
//! same rules the file's own header states, and `test/cases/131_completion_gates.sh`
//! asserts that they select the same gates for the same paths. Nothing here carries a list
//! of gates, of jobs or of paths: a gate added to the model is answered by every surface
//! the next time the model is read.
//!
//! The one deliberate difference from `ci-plan`'s matcher is documented on
//! [`Class::matches`].

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::discovery::glob::Glob;
use crate::metadata::yaml;

/// Where the CI model lives, repository-relative. The path `scripts/ci-plan` defaults to.
pub const MODEL_PATH: &str = ".ai/repo/ci/gates.yaml";

/// The word a class uses to say that any change in it escalates the whole plan.
const FULL: &str = "full";

// ---------------------------------------------------------------- the declaration

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
/// What a class selects: the word `full`, or the gates it names.
pub enum GateClassSelects {
    /// `gates: full` — any change in this class escalates the whole plan.
    Word(String),
    /// `gates: [a, b]` — exactly these, and `gates: []` — nothing.
    Named(Vec<String>),
}

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
    #[serde(default)]
    /// Gates that are selected whenever this one is.
    pub implies: Vec<String>,
    #[serde(default)]
    /// Gates this one cannot run without.
    pub requires: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One path class: the paths it covers and the gates a change to them selects.
pub struct GateClass {
    /// The class's identity.
    pub id: String,
    #[serde(default)]
    /// The pathspecs it covers, in the subset the model's header defines.
    pub paths: Vec<String>,
    #[serde(default)]
    /// `full`, or the gates it names.
    pub gates: Option<GateClassSelects>,
}

impl GateClass {
    /// True when any change in this class escalates the whole plan.
    pub fn escalates(&self) -> bool {
        matches!(&self.gates, Some(GateClassSelects::Word(w)) if w == FULL)
    }

    /// The gates this class names; empty for `full` (which selects everything instead) and
    /// for `gates: []`.
    pub fn named(&self) -> &[String] {
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
    pub fn matches(&self, path: &str) -> bool {
        self.paths.iter().any(|p| Glob::new(p).matches(path))
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
}

impl GateModel {
    /// Read the model from a repository root, or say why not.
    ///
    /// A repository with no model is not an error here: it is a repository whose gates
    /// cannot be known, which every caller reports as [`super::judge::GateStatus`]
    /// `unknown` rather than as a pass.
    pub fn load(root: &Path) -> Result<GateModel, String> {
        let path = root.join(MODEL_PATH);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("{MODEL_PATH} could not be read: {e}"))?;
        let model: GateModel =
            yaml::parse_into(&text).map_err(|e| format!("{MODEL_PATH} does not parse: {e}"))?;
        if model.version != 1 {
            return Err(format!(
                "{MODEL_PATH}: version must be 1, not {}",
                model.version
            ));
        }
        if model.gates.is_empty() {
            return Err(format!("{MODEL_PATH} declares no gates"));
        }
        Ok(model)
    }

    /// One gate by id.
    pub fn gate(&self, id: &str) -> Option<&GateDecl> {
        self.gates.iter().find(|g| g.id == id)
    }

    /// The pathspecs a gate's evidence is taken over: the union of the paths of every class
    /// that names it, plus every class that escalates (those select it too), and everything
    /// for a gate the model marks `always` — a gate every plan selects is a gate any change
    /// can invalidate, and saying otherwise would be evidence that never goes stale.
    ///
    /// Derived from the model and from nothing else: a class that starts naming a gate
    /// starts invalidating its evidence in the same commit.
    pub fn inputs_of(&self, id: &str) -> Vec<String> {
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
    pub fn as_str(self) -> &'static str {
        match self {
            GatePlanMode::Affected => "affected",
            GatePlanMode::Full => "full",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One class a change fell in, and the paths that put it there.
pub struct GateClassMatch {
    /// The class's identity.
    pub id: String,
    /// The changed paths this class covers.
    pub paths: Vec<String>,
}

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
    pub fn selects(&self, id: &str) -> bool {
        self.selected.contains_key(id)
    }
}

/// The plan for a change set, by the rules the model's own header states: `always` gates
/// run in every plan; a changed path selects the gates of every class it falls in (the
/// union, never the first match); a class whose gates are `full` and a path no class covers
/// both escalate the whole plan; `implies` adds a gate whenever its parent is selected;
/// `requires` names a gate a gate cannot run without, and the plan adds it; and an
/// on-demand gate is left out of every plan that did not ask for it, the full one included.
///
/// ```
/// use majordomus_cli::gates::model::{plan, GateModel, GatePlanMode};
/// # use std::path::Path;
/// # let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
/// # if let Ok(model) = GateModel::load(&root) {
/// // a documentation change does not plan the Rust gates
/// let p = plan(&model, &["docs/README.md".to_string()], false);
/// assert_eq!(p.mode, GatePlanMode::Affected);
/// assert!(!p.selects("rust-check"), "a document does not rebuild the crate");
///
/// // and a path the model classifies nowhere escalates rather than passing quietly
/// let p = plan(&model, &["nowhere/at/all.txt".to_string()], false);
/// assert_eq!(p.mode, GatePlanMode::Full);
/// # }
/// ```
pub fn plan(model: &GateModel, changed: &[String], on_demand: bool) -> GatePlan {
    let mut sorted: Vec<String> = changed
        .iter()
        .filter(|p| !p.trim().is_empty())
        .cloned()
        .collect();
    sorted.sort();
    sorted.dedup();

    let mut classes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut unclassified: Vec<String> = Vec::new();
    let mut escalated_by: Vec<String> = Vec::new();
    let mut from_classes: BTreeMap<String, String> = BTreeMap::new();

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
            if class.escalates() {
                escalated_by.push(class.id.clone());
                continue;
            }
            for gate in class.named() {
                from_classes
                    .entry(gate.clone())
                    .or_insert_with(|| format!("the class {} covers a changed path", class.id));
            }
        }
        if !hit {
            unclassified.push(path.clone());
        }
    }
    escalated_by.dedup();

    let (mode, reason) = if !escalated_by.is_empty() {
        (
            GatePlanMode::Full,
            format!(
                "a changed path is in a class that escalates: {}",
                escalated_by.join(" ")
            ),
        )
    } else if !unclassified.is_empty() {
        (
            GatePlanMode::Full,
            format!(
                "a changed path is in no class of the model: {}",
                unclassified.join(" ")
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

    let mut selected: BTreeMap<String, String> = BTreeMap::new();
    let mut excluded: BTreeMap<String, String> = BTreeMap::new();
    for gate in &model.gates {
        if gate.on_demand && !on_demand {
            excluded.insert(
                gate.id.clone(),
                "on demand only: its runner is not available on demand, so a plan that \
                 selected it would wait for a verdict that never arrives"
                    .to_string(),
            );
            continue;
        }
        if mode == GatePlanMode::Full {
            selected.insert(gate.id.clone(), reason.clone());
        } else if gate.always {
            selected.insert(gate.id.clone(), "always".to_string());
        } else if let Some(why) = from_classes.get(&gate.id) {
            selected.insert(gate.id.clone(), why.clone());
        } else {
            excluded.insert(
                gate.id.clone(),
                "no class of the model covers a changed path that selects it".to_string(),
            );
        }
    }

    // `implies` and `requires`, to a fixed point: a full Rust gate includes the registry
    // checks, and the probe cannot run without the built site
    loop {
        let mut added = false;
        for gate in &model.gates {
            if !selected.contains_key(&gate.id) {
                continue;
            }
            for other in gate.implies.iter().chain(gate.requires.iter()) {
                if selected.contains_key(other) {
                    continue;
                }
                let Some(decl) = model.gate(other) else {
                    continue;
                };
                if decl.on_demand && !on_demand {
                    continue;
                }
                let why = if gate.implies.iter().any(|g| g == other) {
                    format!("implied by {}", gate.id)
                } else {
                    format!("{} cannot run without it", gate.id)
                };
                excluded.remove(other);
                selected.insert(other.clone(), why);
                added = true;
            }
        }
        if !added {
            break;
        }
    }

    GatePlan {
        mode,
        reason,
        changed: sorted,
        classes: classes
            .into_iter()
            .map(|(id, mut paths)| {
                paths.sort();
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
    fn an_always_gate_is_in_every_plan_and_a_class_gate_only_in_its_own() {
        let m = model();
        let p = plan(&m, &["apps/majordomus-cli/src/lib.rs".into()], false);
        assert_eq!(p.mode, GatePlanMode::Affected);
        assert!(p.selects("lint"), "always");
        assert!(p.selects("build"), "the rust class names it");
        assert!(p.selects("registry"), "build implies it");
        assert!(!p.selects("site"), "no changed path is the site's");
    }

    #[test]
    fn a_required_gate_is_planned_with_the_gate_that_needs_it() {
        let m = model();
        let p = plan(&m, &["site/content/index.md".into()], false);
        assert!(p.selects("probe"));
        assert!(p.selects("site"), "probe cannot run without it");
        assert_eq!(p.selected["site"], "the class web covers a changed path");
    }

    #[test]
    fn an_escalating_class_and_an_unclassified_path_both_widen_the_plan() {
        let m = model();
        let p = plan(&m, &[".ai/repo/ci/gates.yaml".into()], false);
        assert_eq!(p.mode, GatePlanMode::Full);
        assert!(p.selects("build") && p.selects("site"));

        let p = plan(&m, &["something/nobody/declared".into()], false);
        assert_eq!(p.mode, GatePlanMode::Full);
        assert_eq!(
            p.unclassified,
            vec!["something/nobody/declared".to_string()]
        );
    }

    #[test]
    fn an_on_demand_gate_is_left_out_of_the_full_plan_and_says_why() {
        let m = model();
        let p = plan(&m, &[".ai/repo/ci/gates.yaml".into()], false);
        assert!(!p.selects("mac"));
        assert!(p.excluded["mac"].contains("on demand"));
        // and it is in the plan that asked for it
        assert!(plan(&m, &[".ai/repo/ci/gates.yaml".into()], true).selects("mac"));
    }

    #[test]
    fn a_class_that_names_no_gate_plans_only_what_always_runs() {
        let m = model();
        let p = plan(&m, &[".gitignore".into()], false);
        assert_eq!(p.mode, GatePlanMode::Affected, "the class covers the path");
        assert!(p.selects("lint"));
        assert!(!p.selects("build") && !p.selects("site"));
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
        for class in &m.classes {
            assert!(!class.paths.is_empty(), "class {} names no path", class.id);
            for gate in class.named() {
                assert!(m.gate(gate).is_some(), "class {} names {gate}", class.id);
            }
        }
    }
}
