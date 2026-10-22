//! The owner's standing approvals, declared once for every machine, and how far this
//! machine's permission layer holds them (I3900).
//!
//! The rule `project.a-standing-approval-is-written-where-permissions-are-read` puts an
//! approval into the client's user settings, because that is the file the permission layer
//! reads. That file belongs to one machine, so an approval given on one machine was never
//! held on the next: on 2026-10-20 the owner's MacBook asked for what the other MacBook had
//! been granted a week before, and nothing said so. The declaration (kind
//! `autonomy-declaration`, under `.ai/repo/autonomy/`) is the repository's record of the
//! owner's words; [`parity`] compares a machine's user settings with it, and [`merged`]
//! is what `majordomus autonomy apply` writes when a person asks it to.
//!
//! What is compared is the declared text, exactly: an approval is held when its rule is an
//! entry of `autoMode.allow`, and nothing else counts. An entry the settings carry and the
//! declaration does not is counted, never quoted, because it may name a host or a person.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::Object;

/// The kind the declaration is discovered under.
pub const KIND: &str = "autonomy-declaration";

/// The schema version this executable reads.
pub const SCHEMA_VERSION: &str = "autonomy-declaration/v1";

/// The schema of a [`parity`] answer.
pub(crate) const APPROVALS_SCHEMA: &str = "majordomus.autonomy.approvals/v1";

/// The one client a declaration may name today.
const CLIENT: &str = "claude-code";

/// The entry of `autoMode.allow` that keeps the client's own defaults. It stays first.
const DEFAULTS: &str = "$defaults";

/// The top-level settings a declaration may set beside the approvals. Narrow on purpose:
/// nothing that decides a permission mode, a hook or a guard is ever written from here.
const SETTABLE: &[&str] = &["crossSessionInbound"];

/// The prefix every declared rule carries, so that the classifier reads it as the owner's.
const RULE_PREFIX: &str = "Owner standing approval (";

/// The whole declaration. Unknown keys are refused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutonomyDeclaration {
    /// The format version: `autonomy-declaration/v1`.
    pub schema: String,
    /// The kind: `autonomy-declaration`.
    pub kind: String,
    /// The declaration's identity within the kind.
    pub id: String,
    /// The client whose permission layer reads it: `claude-code`.
    pub client: String,
    /// What the declaration is for, in a sentence a person reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Top-level settings beside the approvals, only those [`SETTABLE`] names.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub settings: BTreeMap<String, Value>,
    /// The approvals, oldest first.
    #[serde(default)]
    pub approvals: Vec<StandingApproval>,
    /// The lines the auto-mode classifier is told about the environment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment: Vec<String>,
}

/// One approval the owner gave for the future.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StandingApproval {
    /// A short stable name.
    pub id: String,
    /// The day the owner gave it, `YYYY-MM-DD`.
    pub date: String,
    /// The rule exactly as the classifier reads it, the owner's words quoted inside.
    pub rule: String,
}

impl AutonomyDeclaration {
    /// Every check the schema cannot make.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA_VERSION {
            return Err(format!("schema {} is not {SCHEMA_VERSION}", self.schema));
        }
        if self.kind != KIND {
            return Err(format!("kind {} is not {KIND}", self.kind));
        }
        if self.client != CLIENT {
            return Err(format!("client {} is not {CLIENT}", self.client));
        }
        for key in self.settings.keys() {
            if !SETTABLE.contains(&key.as_str()) {
                return Err(format!(
                    "setting '{key}' may not be declared; only {} may",
                    SETTABLE.join(", ")
                ));
            }
        }
        let mut ids = BTreeSet::new();
        let mut rules = BTreeSet::new();
        for a in &self.approvals {
            let slug = !a.id.is_empty()
                && a.id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if !slug {
                return Err(format!("approval id '{}' is not a lowercase slug", a.id));
            }
            if !ids.insert(a.id.as_str()) {
                return Err(format!("approval id '{}' is declared twice", a.id));
            }
            if !rules.insert(a.rule.as_str()) {
                return Err(format!("approval {}: its rule is declared twice", a.id));
            }
            if !valid_date(&a.date) {
                return Err(format!(
                    "approval {}: date '{}' is not YYYY-MM-DD",
                    a.id, a.date
                ));
            }
            if !a.rule.starts_with(&format!("{RULE_PREFIX}{}", a.date)) {
                return Err(format!(
                    "approval {}: the rule does not begin '{RULE_PREFIX}{}'",
                    a.id, a.date
                ));
            }
        }
        if self.environment.iter().any(|e| e.trim().is_empty()) {
            return Err("an environment line is empty".into());
        }
        Ok(())
    }

    /// Parse one indexed object, or say why it is not a declaration, naming its URI.
    pub fn parse(object: &Object) -> Result<AutonomyDeclaration, String> {
        let parsed: AutonomyDeclaration = serde_json::from_value(object.metadata.clone())
            .map_err(|e| format!("{}: {e}", object.uri))?;
        parsed
            .validate()
            .map_err(|e| format!("{}: {e}", object.uri))?;
        Ok(parsed)
    }
}

fn valid_date(d: &str) -> bool {
    let b = d.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// The repository's declaration for `client`, read from its index: `Ok(None)` when it
/// declares none, an error naming the object when one is invalid or when two name the
/// same client.
pub fn from_objects(objects: &[Object]) -> Result<Option<(String, AutonomyDeclaration)>, String> {
    let mut found = Vec::new();
    for o in objects.iter().filter(|o| o.kind == KIND) {
        let d = AutonomyDeclaration::parse(o)?;
        if d.client == CLIENT {
            found.push((o.provenance.path.clone(), d));
        }
    }
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.pop()),
        n => Err(format!(
            "{n} autonomy declarations name {CLIENT} ({}); a repository declares one",
            found
                .iter()
                .map(|(p, _)| p.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// The declaration a machine is held to: the working tree's, narrowed to what the trunk's
/// committed copy also says.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Effective {
    /// The declaration's path in the repository.
    pub(crate) path: String,
    /// What remains of the working tree's declaration once narrowed.
    pub(crate) declaration: AutonomyDeclaration,
    /// The trunk it was held against, when the repository names one.
    pub(crate) trunk: Option<String>,
    /// Every approval, line and setting the working tree says and the trunk does not.
    pub(crate) narrowed: Vec<String>,
}

/// The working tree's `d`, keeping only what `trunk` also declares identically. An
/// approval is a grant, so a branch, a pull request or an uncommitted edit may withdraw
/// one and never add one: the trunk is the owner's word, the working tree anybody's.
pub(crate) fn narrow(
    mut d: AutonomyDeclaration,
    trunk: Option<&AutonomyDeclaration>,
) -> (AutonomyDeclaration, Vec<String>) {
    let mut narrowed = Vec::new();
    let on = |what: &str| format!("{what} is not on the trunk");
    d.approvals.retain(|a| {
        let kept = trunk.is_some_and(|t| t.approvals.iter().any(|x| x.rule == a.rule));
        if !kept {
            narrowed.push(on(&format!("approval {}", a.id)));
        }
        kept
    });
    let before = d.environment.len();
    d.environment
        .retain(|e| trunk.is_some_and(|t| t.environment.contains(e)));
    if d.environment.len() < before {
        narrowed.push(on(&format!(
            "{} environment line(s)",
            before - d.environment.len()
        )));
    }
    d.settings.retain(|k, v| {
        let kept = trunk.is_some_and(|t| t.settings.get(k) == Some(v));
        if !kept {
            narrowed.push(on(&format!("setting {k}")));
        }
        kept
    });
    (d, narrowed)
}

/// The trunk's committed copy of the declaration at `path`: `None` when it holds none.
fn on_trunk(root: &Path, branch: &str, path: &str) -> Option<Result<AutonomyDeclaration, String>> {
    let spec = format!("refs/heads/{branch}:{path}");
    let out = crate::git::read_only(root)
        .args(["show", &spec])
        .output()
        .ok()
        .filter(|out| out.status.success())?;
    let named = format!("{branch}:{path}");
    let parsed = String::from_utf8(out.stdout)
        .map_err(|e| e.to_string())
        .and_then(|text| crate::metadata::yaml::parse_mapping(&text))
        .and_then(|map| {
            serde_json::from_value::<AutonomyDeclaration>(Value::Object(map))
                .map_err(|e| e.to_string())
        })
        .and_then(|d| d.validate().map(|()| d))
        .map_err(|e| format!("{named}: {e}"));
    Some(parsed)
}

/// The declaration this machine is held to, from the index and the trunk of the checkout
/// at `root`. A trunk copy that does not parse is an error, never a reason to trust the
/// working tree instead.
pub(crate) fn resolve(root: &Path, objects: &[Object]) -> Result<Option<Effective>, String> {
    let Some((path, working)) = from_objects(objects)? else {
        return Ok(None);
    };
    let trunk = crate::worktree::RepositoryIdentity::discover(root)
        .ok()
        .and_then(|identity| identity.trunk().branch.clone());
    let held = match trunk.as_deref().and_then(|b| on_trunk(root, b, &path)) {
        Some(Ok(d)) => Some(d),
        Some(Err(e)) => return Err(e),
        None => None,
    };
    let (declaration, narrowed) = narrow(working, held.as_ref());
    Ok(Some(Effective {
        path,
        declaration,
        trunk,
        narrowed,
    }))
}

/// How far a machine holds the declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Parity {
    /// Every approval, environment line and setting is held.
    Full,
    /// Some are held and some are not.
    Partial,
    /// None is held.
    None,
    /// The repository declares nothing to hold.
    Undeclared,
    /// The user settings exist and cannot be read as JSON: nothing can be said, and nothing
    /// will be written over them.
    Unreadable,
}

/// One approval, and whether this machine holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ApprovalHeld {
    /// Its id in the declaration.
    pub(crate) id: String,
    /// The day the owner gave it.
    pub(crate) date: String,
    /// Whether its rule is an entry of `autoMode.allow`.
    pub(crate) held: bool,
}

/// One declared setting, and whether this machine holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct SettingHeld {
    /// The top-level key.
    pub(crate) key: String,
    /// The declared value.
    pub(crate) declared: Value,
    /// The value the user settings carry, when they carry one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) actual: Option<Value>,
    /// Whether the two are equal.
    pub(crate) held: bool,
}

/// The answer of `autonomy.approvals`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ApprovalParity {
    /// `majordomus.autonomy.approvals/v1`.
    pub(crate) schema: String,
    /// The declaration's path in the repository, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) declaration: Option<String>,
    /// The user settings judged, with the home directory written `~`.
    pub(crate) user_settings: String,
    /// Whether that file exists.
    pub(crate) present: bool,
    /// Why it could not be read; absent when it could.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    /// The verdict.
    pub(crate) parity: Parity,
    /// Each declared approval, in declaration order.
    pub(crate) approvals: Vec<ApprovalHeld>,
    /// Whether `autoMode.allow` keeps the client's defaults first.
    pub(crate) defaults_first: bool,
    /// How many declared environment lines there are.
    pub(crate) environment_declared: usize,
    /// How many of them `autoMode.environment` carries.
    pub(crate) environment_held: usize,
    /// Each declared setting.
    pub(crate) settings: Vec<SettingHeld>,
    /// How many `autoMode.allow` entries are neither the defaults nor declared. Counted,
    /// never quoted: an approval given on this machine alone, or one not yet declared.
    pub(crate) undeclared: usize,
    /// One line a person reads.
    pub(crate) summary: String,
    /// What brings this machine to full parity, when it is not there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) remedy: Option<String>,
    /// The trunk the declaration was held against, when the repository names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) trunk: Option<String>,
    /// What the working tree declares and the trunk does not, so is neither judged nor
    /// applied: a grant reaches a machine only once the owner's trunk holds it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) narrowed: Vec<String>,
}

/// The command that writes what is missing.
pub(crate) const APPLY_COMMAND: &str = "majordomus autonomy apply";

fn strings(v: &Value, pointer: &str) -> Vec<String> {
    v.pointer(pointer)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Read the user settings at `path`: `Ok(None)` when absent, the reason when unreadable.
pub(crate) fn read_settings(path: &Path) -> Result<Option<Value>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if !v.is_object() {
        return Err("the settings are not a JSON object".into());
    }
    Ok(Some(v))
}

/// How far `settings` (the user settings, `None` when absent) hold `declared`.
pub(crate) fn parity(
    declared: Option<(&str, &AutonomyDeclaration)>,
    settings: Result<Option<&Value>, &str>,
    shown: &str,
) -> ApprovalParity {
    let mut answer = ApprovalParity {
        schema: APPROVALS_SCHEMA.into(),
        declaration: declared.map(|(p, _)| p.to_string()),
        user_settings: shown.into(),
        present: matches!(settings, Ok(Some(_)) | Err(_)),
        error: settings.err().map(str::to_string),
        parity: Parity::Undeclared,
        approvals: Vec::new(),
        defaults_first: false,
        environment_declared: 0,
        environment_held: 0,
        settings: Vec::new(),
        undeclared: 0,
        summary: String::new(),
        remedy: None,
        trunk: None,
        narrowed: Vec::new(),
    };
    let empty = Value::Object(Default::default());
    let v = match settings {
        Ok(Some(v)) => v,
        Ok(None) => &empty,
        Err(e) => {
            answer.parity = Parity::Unreadable;
            answer.summary = format!("{shown} cannot be read ({e}); nothing is judged or written");
            return answer;
        }
    };
    let allow = strings(v, "/autoMode/allow");
    let environment: BTreeSet<String> = strings(v, "/autoMode/environment").into_iter().collect();
    answer.defaults_first = allow.first().map(String::as_str) == Some(DEFAULTS);
    let Some((_, d)) = declared else {
        answer.undeclared = allow.iter().filter(|r| r.as_str() != DEFAULTS).count();
        answer.summary = "the repository declares no standing approval (.ai/repo/autonomy/)".into();
        return answer;
    };
    let held: BTreeSet<&str> = allow.iter().map(String::as_str).collect();
    let declared_rules: BTreeSet<&str> = d.approvals.iter().map(|a| a.rule.as_str()).collect();
    answer.approvals = d
        .approvals
        .iter()
        .map(|a| ApprovalHeld {
            id: a.id.clone(),
            date: a.date.clone(),
            held: held.contains(a.rule.as_str()),
        })
        .collect();
    answer.undeclared = allow
        .iter()
        .filter(|r| r.as_str() != DEFAULTS && !declared_rules.contains(r.as_str()))
        .count();
    answer.environment_declared = d.environment.len();
    answer.environment_held = d
        .environment
        .iter()
        .filter(|e| environment.contains(*e))
        .count();
    answer.settings = d
        .settings
        .iter()
        .map(|(k, want)| {
            let actual = v.get(k).cloned();
            SettingHeld {
                key: k.clone(),
                held: actual.as_ref() == Some(want),
                declared: want.clone(),
                actual,
            }
        })
        .collect();
    let approvals_held = answer.approvals.iter().filter(|a| a.held).count();
    let settings_held = answer.settings.iter().filter(|s| s.held).count();
    let total = d.approvals.len() + d.environment.len() + d.settings.len();
    let got = approvals_held + answer.environment_held + settings_held;
    let defaults_ok = answer.defaults_first || d.approvals.is_empty();
    answer.parity = if total == 0 {
        Parity::Undeclared
    } else if got == total && defaults_ok {
        Parity::Full
    } else if got == 0 {
        Parity::None
    } else {
        Parity::Partial
    };
    answer.summary = format!(
        "{approvals_held} of {} standing approval(s), {} of {} environment line(s), {settings_held} of {} setting(s) held{}{}",
        d.approvals.len(),
        answer.environment_held,
        d.environment.len(),
        d.settings.len(),
        if defaults_ok { "" } else { "; $defaults is not first" },
        if answer.undeclared > 0 {
            format!("; {} undeclared", answer.undeclared)
        } else {
            String::new()
        },
    );
    if !matches!(answer.parity, Parity::Full | Parity::Undeclared) {
        answer.remedy = Some(format!(
            "{APPLY_COMMAND} writes what is missing into {shown}, keeping everything else"
        ));
    }
    answer
}

/// How far this machine's user settings hold the repository's declaration, from `facts`
/// and the indexed `objects`. An invalid declaration is an error, never an empty answer.
pub(crate) fn judge(facts: &super::Facts, objects: &[Object]) -> Result<ApprovalParity, String> {
    let effective = resolve(&facts.root, objects)?;
    let Some(path) = facts.user_settings() else {
        return Err("no home directory: the user settings cannot be found".into());
    };
    let shown = facts.shown(&path);
    let read = read_settings(&path);
    let settings = match &read {
        Ok(v) => Ok(v.as_ref()),
        Err(e) => Err(e.as_str()),
    };
    let mut answer = parity(
        effective
            .as_ref()
            .map(|e| (e.path.as_str(), &e.declaration)),
        settings,
        &shown,
    );
    if let Some(e) = effective {
        answer.trunk = e.trunk;
        if !e.narrowed.is_empty() {
            answer.summary.push_str(&format!(
                "; {} declared item(s) are not on the trunk and are neither judged nor applied",
                e.narrowed.len()
            ));
        }
        answer.narrowed = e.narrowed;
    }
    Ok(answer)
}

/// `settings` with everything `d` declares added: `$defaults` first in `autoMode.allow`,
/// each missing rule and environment line appended in declaration order, each declared
/// setting set. Nothing present is removed or reordered, and no other key is touched.
pub(crate) fn merged(d: &AutonomyDeclaration, settings: &Value) -> Value {
    let mut out = settings.clone();
    if !out.is_object() {
        out = Value::Object(Default::default());
    }
    let obj = out.as_object_mut().expect("an object");
    for (k, v) in &d.settings {
        obj.insert(k.clone(), v.clone());
    }
    if d.approvals.is_empty() && d.environment.is_empty() {
        return out;
    }
    let auto = obj
        .entry("autoMode")
        .or_insert_with(|| Value::Object(Default::default()));
    if !auto.is_object() {
        *auto = Value::Object(Default::default());
    }
    let auto = auto.as_object_mut().expect("an object");
    if !d.approvals.is_empty() {
        let mut allow = strings(&Value::Object(auto.clone()), "/allow");
        if !allow.iter().any(|r| r == DEFAULTS) {
            allow.insert(0, DEFAULTS.into());
        }
        for a in &d.approvals {
            if !allow.contains(&a.rule) {
                allow.push(a.rule.clone());
            }
        }
        auto.insert("allow".into(), Value::from(allow));
    }
    if !d.environment.is_empty() {
        let mut env = strings(&Value::Object(auto.clone()), "/environment");
        for e in &d.environment {
            if !env.contains(e) {
                env.push(e.clone());
            }
        }
        auto.insert("environment".into(), Value::from(env));
    }
    out
}

/// The environment variables an agent client sets for the processes its session runs.
const AGENT_SESSION_VARIABLES: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CODEX_SANDBOX",
    "GEMINI_CLI",
];

/// The variable that says this process runs inside an agent session, when one does.
pub(crate) fn agent_session(env: &BTreeMap<String, String>) -> Option<&'static str> {
    AGENT_SESSION_VARIABLES
        .iter()
        .copied()
        .find(|v| env.get(*v).is_some_and(|x| !x.is_empty()))
}

/// What `apply` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Applied {
    /// The file written, with the home directory written `~`.
    pub(crate) path: String,
    /// Whether anything changed.
    pub(crate) changed: bool,
    /// The copy of the file as it was, when one was kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) backup: Option<String>,
}

/// Write [`merged`] into the user settings at `path`, keeping a copy of the file as it was
/// beside it, and write nothing when nothing would change. An unreadable file is refused,
/// never overwritten.
pub(crate) fn apply(
    d: &AutonomyDeclaration,
    path: &Path,
    shown: &dyn Fn(&Path) -> String,
    stamp: &str,
) -> Result<Applied, String> {
    let current = read_settings(path)
        .map_err(|e| format!("{} cannot be read ({e}); it is left as it is", shown(path)))?;
    let before = current
        .clone()
        .unwrap_or_else(|| Value::Object(Default::default()));
    let after = merged(d, &before);
    if current.is_some() && after == before {
        return Ok(Applied {
            path: shown(path),
            changed: false,
            backup: None,
        });
    }
    let mut backup = None;
    if current.is_some() {
        let copy = path.with_file_name(format!(
            "{}.bak-majordomus-{stamp}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("settings.json")
        ));
        std::fs::copy(path, &copy).map_err(|e| format!("{}: {e}", shown(&copy)))?;
        backup = Some(shown(&copy));
    } else if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", shown(dir)))?;
    }
    let mut text = serde_json::to_string_pretty(&after).map_err(|e| e.to_string())?;
    text.push('\n');
    let tmp = path.with_extension("json.majordomus-tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", shown(&tmp)))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", shown(path)))?;
    Ok(Applied {
        path: shown(path),
        changed: true,
        backup,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn declaration() -> AutonomyDeclaration {
        serde_json::from_value(json!({
            "schema": SCHEMA_VERSION, "kind": KIND, "id": "claude-code", "client": CLIENT,
            "settings": { "crossSessionInbound": "accept" },
            "approvals": [
                { "id": "one", "date": "2026-10-14",
                  "rule": "Owner standing approval (2026-10-14): the first act." },
                { "id": "two", "date": "2026-10-16",
                  "rule": "Owner standing approval (2026-10-16, in Czech): the second act." }
            ],
            "environment": ["**Trusted repo**: this one"]
        }))
        .unwrap()
    }

    fn judged(settings: &Value) -> ApprovalParity {
        let d = declaration();
        parity(
            Some(("decl.yaml", &d)),
            Ok(Some(settings)),
            "~/.claude/settings.json",
        )
    }

    #[test]
    fn the_declaration_validates_and_refuses_what_it_must() {
        assert_eq!(declaration().validate(), Ok(()));
        let mut d = declaration();
        d.settings.insert(
            "permissions".into(),
            json!({ "defaultMode": "bypassPermissions" }),
        );
        assert!(d.validate().unwrap_err().contains("may not be declared"));
        let mut d = declaration();
        d.approvals[1].id = "one".into();
        assert!(d.validate().unwrap_err().contains("declared twice"));
        let mut d = declaration();
        d.approvals[0].rule = "allow anything".into();
        assert!(d.validate().unwrap_err().contains("does not begin"));
        let mut d = declaration();
        d.approvals[0].date = "14.10.2026".into();
        assert!(d.validate().unwrap_err().contains("YYYY-MM-DD"));
        let mut d = declaration();
        d.client = "codex".into();
        assert!(d.validate().unwrap_err().contains("client"));
    }

    #[test]
    fn a_machine_that_holds_nothing_has_no_parity_and_is_told_the_remedy() {
        let p = judged(&json!({ "permissions": { "defaultMode": "auto" } }));
        assert_eq!(p.parity, Parity::None);
        assert!(p.approvals.iter().all(|a| !a.held));
        assert_eq!(p.environment_held, 0);
        assert!(p.remedy.as_deref().unwrap().starts_with(APPLY_COMMAND));
        assert!(
            p.summary.starts_with("0 of 2 standing approval(s)"),
            "{}",
            p.summary
        );
    }

    #[test]
    fn what_merged_writes_is_full_parity_and_keeps_everything_else() {
        let before = json!({
            "permissions": { "allow": ["Bash(ssh *)"], "defaultMode": "auto" },
            "hooks": { "PreToolUse": [{ "hooks": [{ "type": "command", "command": "guard" }] }] },
            "autoMode": { "allow": ["a rule given on this machine only"] }
        });
        let d = declaration();
        let after = merged(&d, &before);
        assert_eq!(after["permissions"], before["permissions"]);
        assert_eq!(after["hooks"], before["hooks"]);
        let allow = strings(&after, "/autoMode/allow");
        assert_eq!(allow[0], DEFAULTS, "the defaults are kept first");
        assert_eq!(
            allow[1], "a rule given on this machine only",
            "nothing is reordered"
        );
        let p = judged(&after);
        assert_eq!(p.parity, Parity::Full, "{}", p.summary);
        assert_eq!(
            p.undeclared, 1,
            "the machine's own rule is counted, not dropped"
        );
        assert!(p.remedy.is_none());
        assert_eq!(merged(&d, &after), after, "merging twice changes nothing");
    }

    #[test]
    fn an_edited_rule_is_not_held() {
        let d = declaration();
        let mut s = merged(&d, &json!({}));
        s["autoMode"]["allow"][1] =
            json!("Owner standing approval (2026-10-14): the first act, widened.");
        let p = judged(&s);
        assert_eq!(p.parity, Parity::Partial);
        assert!(!p.approvals[0].held);
        assert_eq!(p.undeclared, 1);
    }

    #[test]
    fn unreadable_settings_are_judged_unreadable_and_never_written() {
        let d = declaration();
        let p = parity(Some(("decl.yaml", &d)), Err("expected value"), "~/s.json");
        assert_eq!(p.parity, Parity::Unreadable);
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        let shown = |p: &Path| p.display().to_string();
        assert!(apply(&d, &path, &shown, "t")
            .unwrap_err()
            .contains("left as it is"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
    }

    #[test]
    fn apply_keeps_a_copy_and_a_second_apply_writes_nothing() {
        let d = declaration();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(&path, "{\"theme\": \"dark\"}").unwrap();
        let shown = |p: &Path| p.display().to_string();
        let first = apply(&d, &path, &shown, "20261020T000000Z").unwrap();
        assert!(first.changed);
        let backup = first.backup.unwrap();
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            "{\"theme\": \"dark\"}"
        );
        let written = read_settings(&path).unwrap().unwrap();
        assert_eq!(written["theme"], "dark");
        assert_eq!(judged(&written).parity, Parity::Full);
        let second = apply(&d, &path, &shown, "later").unwrap();
        assert!(!second.changed && second.backup.is_none());
    }

    #[test]
    fn absent_settings_are_created_on_apply() {
        let d = declaration();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".claude/settings.json");
        let shown = |p: &Path| p.display().to_string();
        let a = apply(&d, &path, &shown, "t").unwrap();
        assert!(a.changed && a.backup.is_none());
        assert_eq!(
            judged(&read_settings(&path).unwrap().unwrap()).parity,
            Parity::Full
        );
    }

    #[test]
    fn a_working_tree_may_withdraw_a_grant_and_never_add_one() {
        let trunk = declaration();
        let mut branch = declaration();
        branch.approvals.push(StandingApproval {
            id: "planted".into(),
            date: "2026-10-20".into(),
            rule: "Owner standing approval (2026-10-20): anything at all.".into(),
        });
        branch.approvals[0].rule.push_str(" Widened.");
        branch.environment.push("**Planted**: a line".into());
        branch
            .settings
            .insert("crossSessionInbound".into(), json!("accept"));
        let (held, narrowed) = narrow(branch, Some(&trunk));
        assert_eq!(
            held.approvals
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            ["two"],
            "the planted approval and the edited one are dropped"
        );
        assert_eq!(held.environment, trunk.environment);
        assert_eq!(narrowed.len(), 3, "{narrowed:?}");

        let (nothing, narrowed) = narrow(declaration(), None);
        assert!(nothing.approvals.is_empty() && nothing.environment.is_empty());
        assert!(nothing.settings.is_empty(), "no trunk copy grants nothing");
        assert_eq!(narrowed.len(), 4);

        let mut fewer = declaration();
        fewer.approvals.pop();
        let (held, narrowed) = narrow(fewer.clone(), Some(&trunk));
        assert_eq!(held, fewer, "withdrawing is always allowed");
        assert!(narrowed.is_empty());
    }

    #[test]
    fn an_agent_session_is_recognised_by_its_variables() {
        let mut env = BTreeMap::new();
        assert_eq!(agent_session(&env), None);
        env.insert("CLAUDECODE".to_string(), String::new());
        assert_eq!(agent_session(&env), None, "an empty variable says nothing");
        env.insert("CLAUDECODE".to_string(), "1".to_string());
        assert_eq!(agent_session(&env), Some("CLAUDECODE"));
    }

    #[test]
    fn no_declaration_is_undeclared() {
        let p = parity(None, Ok(None), "~/s.json");
        assert_eq!(p.parity, Parity::Undeclared);
        assert!(p.remedy.is_none());
    }
}
