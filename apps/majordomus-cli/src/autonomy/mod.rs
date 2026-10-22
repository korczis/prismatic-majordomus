//! How much autonomy a coding-agent session in this checkout can be given, read rather than
//! assumed (I2790).
//!
//! Three questions, each answered from facts this process can observe and nothing else:
//!
//! - [`status`]: which settings layers the client (Claude Code) reads and what each declares,
//!   the evidence for or against hard isolation, and the autonomy profile and permission
//!   mode those facts support. The profile fails closed: `isolated` (the one that may run
//!   `bypassPermissions`) needs a container marker *and* no credential in reach, `trusted`
//!   needs the person's own acceptance of the checkout recorded outside the repository, and
//!   nothing the repository checks in can raise either.
//! - [`explain`]: what one shell command is, structurally, and the decision the profile gives
//!   it. A compound command (chaining, pipes, redirection, substitution) never earns a static
//!   allow, because a prefix rule cannot see past its first word; the risk boundaries
//!   (history rewrites, destructive removal, privilege, external effects) are the same
//!   under every profile.
//! - [`friction`]: what the permission layer actually cost, counted from the client's own
//!   transcripts of this repository's checkouts — classifier denials by reason, hook blocks,
//!   built-in safety refusals and rejected prompts, per tool and per thousand tool calls.
//!   Counts only: no command, path or payload is carried out of a transcript.
//!
//! Everything here reads; nothing writes a setting. The permission configuration is the
//! person's (`project.a-standing-approval-is-written-where-permissions-are-read`), and a
//! process that judged its own autonomy and then granted it would be the thing the
//! profile exists to prevent.

pub(crate) mod approvals;

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The schema of a [`status`] answer.
pub(crate) const STATUS_SCHEMA: &str = "majordomus.autonomy.status/v1";
/// The schema of a [`friction`] answer.
pub(crate) const FRICTION_SCHEMA: &str = "majordomus.autonomy.friction/v1";

/// An autonomy profile: how much a session may do without a person's approval.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AutonomyProfile {
    /// The client's own defaults: every side effect is asked for.
    Conservative,
    /// Reads, repository edits, local builds and tests run unasked; external effects are asked.
    Balanced,
    /// Repository-local development runs unasked, including local commits; the client's own
    /// classifier judges the rest, and the risk boundaries stay.
    Trusted,
    /// Hard isolation was verified and no credential is in reach: the client's permission
    /// prompts may be bypassed, and the risk boundaries still stand.
    Isolated,
}

/// A Claude Code permission mode, as the client spells it in `permissions.defaultMode` and
/// `--permission-mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PermissionMode {
    /// Ask for everything not allowed by a rule.
    Default,
    /// Accept file edits in the working directory; ask for the rest.
    AcceptEdits,
    /// The client's classifier decides what a rule does not.
    Auto,
    /// Never prompt; refuse what no rule allows.
    DontAsk,
    /// No permission prompts at all. Only inside verified isolation.
    BypassPermissions,
}

impl AutonomyProfile {
    /// The permission mode a profile launches with.
    pub(crate) fn mode(self) -> PermissionMode {
        match self {
            Self::Conservative => PermissionMode::Default,
            Self::Balanced => PermissionMode::AcceptEdits,
            Self::Trusted => PermissionMode::Auto,
            Self::Isolated => PermissionMode::BypassPermissions,
        }
    }
}

/// One settings file the client reads, and what it declares about permissions. Rule text is
/// not carried, only counts: a rule may name a host, a path or a person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct SettingsLayer {
    /// `managed`, `user`, `project` or `local`, highest precedence first.
    pub(crate) scope: String,
    /// Where the file is, with the home directory written `~`.
    pub(crate) path: String,
    /// Whether the file exists.
    pub(crate) present: bool,
    /// Why a present file could not be read as JSON; absent when it could.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    /// `permissions.defaultMode`, as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) default_mode: Option<String>,
    /// How many `permissions.allow` rules.
    pub(crate) allow: usize,
    /// How many `permissions.ask` rules.
    pub(crate) ask: usize,
    /// How many `permissions.deny` rules.
    pub(crate) deny: usize,
    /// How many `autoMode.allow` entries, `$defaults` included.
    pub(crate) auto_mode_allow: usize,
    /// Whether `permissions.disableBypassPermissionsMode` is `disable`.
    pub(crate) bypass_disabled: bool,
    /// Whether `sandbox.enabled` is true.
    pub(crate) sandbox_enabled: bool,
    /// The hook events the file wires, sorted.
    pub(crate) hooks: Vec<String>,
}

/// What this process can observe about the boundary it runs in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct IsolationEvidence {
    /// Container markers found (`/.dockerenv`, `/run/.containerenv`, the `container`
    /// variable). Empty means no container was observed, which is not proof there is none.
    pub(crate) container_markers: Vec<String>,
    /// Whether a settings layer enables the client's own sandbox.
    pub(crate) client_sandbox: bool,
    /// Names (never values) of environment variables that carry a credential.
    pub(crate) credential_variables: Vec<String>,
    /// Credential files present under the home directory, written from `~`.
    pub(crate) credential_files: Vec<String>,
    /// Whether isolation counts as verified: a container marker, and no credential in reach.
    pub(crate) verified: bool,
}

/// One profile this checkout was not given, and the fact that refused it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ProfileRefusal {
    /// The profile refused.
    pub(crate) profile: AutonomyProfile,
    /// Why.
    pub(crate) because: String,
}

/// The answer of `autonomy.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct AutonomyStatus {
    /// `majordomus.autonomy.status/v1`.
    pub(crate) schema: String,
    /// The checkout judged.
    pub(crate) root: String,
    /// The client's version, when this process runs under it (`CLAUDE_CODE_VERSION`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) client_version: Option<String>,
    /// Whether this process runs inside a Claude Code session (`CLAUDECODE`).
    pub(crate) inside_client: bool,
    /// The settings layers, highest precedence first.
    pub(crate) layers: Vec<SettingsLayer>,
    /// The mode the layers configure, from the highest-precedence layer that sets one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) configured_mode: Option<String>,
    /// Whether the person accepted this checkout in the client's trust dialog, read from
    /// the person's own `~/.claude.json`; absent when that could not be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) checkout_trusted: Option<bool>,
    /// The isolation evidence.
    pub(crate) isolation: IsolationEvidence,
    /// The profile the facts support.
    pub(crate) profile: AutonomyProfile,
    /// The permission mode that profile launches with.
    pub(crate) recommended_mode: PermissionMode,
    /// Why this profile, one fact per line.
    pub(crate) reasons: Vec<String>,
    /// Every stronger profile, and what refused it.
    pub(crate) refused: Vec<ProfileRefusal>,
    /// What this answer cannot see. The running session's effective mode is the client's
    /// and is not observable from a child process.
    pub(crate) limitations: Vec<String>,
}

/// Everything [`status`] reads, gathered once so that a test can supply it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Facts {
    /// The checkout.
    pub(crate) root: PathBuf,
    /// The home directory, when known.
    pub(crate) home: Option<PathBuf>,
    /// The managed settings file of this platform.
    pub(crate) managed: Option<PathBuf>,
    /// The environment, by name.
    pub(crate) env: BTreeMap<String, String>,
    /// Container marker files that exist.
    pub(crate) container_files: Vec<String>,
}

/// Environment variables whose presence puts a credential in a session's reach.
const CREDENTIAL_VARIABLES: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AZURE_CLIENT_SECRET",
    "CARGO_REGISTRY_TOKEN",
    "DOCKER_PASSWORD",
    "FLY_API_TOKEN",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GITLAB_TOKEN",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "NPM_TOKEN",
    "OPENAI_API_KEY",
    "SSH_AUTH_SOCK",
];

/// Credential files, relative to the home directory.
const CREDENTIAL_FILES: &[&str] = &[
    ".aws/credentials",
    ".config/gh/hosts.yml",
    ".docker/config.json",
    ".kube/config",
    ".netrc",
    ".ssh/id_ecdsa",
    ".ssh/id_ed25519",
    ".ssh/id_rsa",
];

/// Container marker files.
const CONTAINER_FILES: &[&str] = &["/.dockerenv", "/run/.containerenv"];

/// Where the client reads the organisation's managed settings on this platform.
#[cfg(target_os = "macos")]
const MANAGED_SETTINGS: &str = "/Library/Application Support/ClaudeCode/managed-settings.json";
#[cfg(not(target_os = "macos"))]
const MANAGED_SETTINGS: &str = "/etc/claude-code/managed-settings.json";

/// The paths of `paths` that exist.
fn present(paths: &[&str]) -> Vec<String> {
    paths
        .iter()
        .copied()
        .filter(|p| Path::new(p).exists())
        .map(String::from)
        .collect()
}

impl Facts {
    /// The facts of this process, for the checkout at `root`.
    pub(crate) fn observe(root: &Path) -> Self {
        Facts {
            root: root.to_path_buf(),
            home: std::env::var_os("HOME").map(PathBuf::from),
            managed: Some(PathBuf::from(MANAGED_SETTINGS)),
            env: std::env::vars().collect(),
            container_files: present(CONTAINER_FILES),
        }
    }

    /// The user settings of the client, under the home directory.
    pub(crate) fn user_settings(&self) -> Option<PathBuf> {
        self.home.as_ref().map(|h| h.join(".claude/settings.json"))
    }

    /// `path` with the home directory written `~`.
    pub(crate) fn shown(&self, path: &Path) -> String {
        match &self.home {
            Some(h) => match path.strip_prefix(h) {
                Ok(rest) => format!("~/{}", rest.display()),
                Err(_) => path.display().to_string(),
            },
            None => path.display().to_string(),
        }
    }
}

/// What an error says, for a layer that could not be read.
fn said<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn count(v: &Value, pointer: &str) -> usize {
    v.pointer(pointer)
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

/// Read one settings layer.
pub(crate) fn layer(facts: &Facts, scope: &str, path: &Path) -> SettingsLayer {
    let mut l = SettingsLayer {
        scope: scope.into(),
        path: facts.shown(path),
        present: path.is_file(),
        error: None,
        default_mode: None,
        allow: 0,
        ask: 0,
        deny: 0,
        auto_mode_allow: 0,
        bypass_disabled: false,
        sandbox_enabled: false,
        hooks: Vec::new(),
    };
    if !l.present {
        return l;
    }
    let parsed = std::fs::read_to_string(path)
        .map_err(said)
        .and_then(|t| serde_json::from_str::<Value>(&t).map_err(said));
    let v = match parsed {
        Ok(v) => v,
        Err(e) => {
            l.error = Some(e);
            return l;
        }
    };
    l.default_mode = v
        .pointer("/permissions/defaultMode")
        .and_then(Value::as_str)
        .map(str::to_string);
    l.allow = count(&v, "/permissions/allow");
    l.ask = count(&v, "/permissions/ask");
    l.deny = count(&v, "/permissions/deny");
    l.auto_mode_allow = count(&v, "/autoMode/allow");
    l.bypass_disabled = v
        .pointer("/permissions/disableBypassPermissionsMode")
        .and_then(Value::as_str)
        == Some("disable");
    l.sandbox_enabled = v.pointer("/sandbox/enabled").and_then(Value::as_bool) == Some(true);
    if let Some(hooks) = v.get("hooks").and_then(Value::as_object) {
        l.hooks = hooks.keys().cloned().collect();
        crate::order::canonical_strings(&mut l.hooks);
    }
    l
}

/// Whether the person accepted `root` in the client's trust dialog, from `~/.claude.json`.
/// Only that one field is read; the file holds account state this never touches.
fn checkout_trusted(facts: &Facts) -> Option<bool> {
    let home = facts.home.as_ref()?;
    let text = std::fs::read_to_string(home.join(".claude.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    // a checkout the record does not name is one the person has not accepted
    let key = facts.root.to_string_lossy();
    Some(v["projects"][key.as_ref()]["hasTrustDialogAccepted"].as_bool() == Some(true))
}

/// The isolation evidence of `facts`, given the settings layers already read.
pub(crate) fn isolation(facts: &Facts, layers: &[SettingsLayer]) -> IsolationEvidence {
    let mut container_markers = facts.container_files.clone();
    if facts.env.get("container").is_some_and(|v| !v.is_empty()) {
        container_markers.push("container variable".into());
    }
    let credential_variables: Vec<String> = CREDENTIAL_VARIABLES
        .iter()
        .filter(|n| facts.env.get(**n).is_some_and(|v| !v.is_empty()))
        .map(|n| (*n).to_string())
        .collect();
    let credential_files: Vec<String> = match &facts.home {
        Some(h) => CREDENTIAL_FILES
            .iter()
            .filter(|f| h.join(f).exists())
            .map(|f| format!("~/{f}"))
            .collect(),
        None => Vec::new(),
    };
    let verified = !container_markers.is_empty()
        && credential_variables.is_empty()
        && credential_files.is_empty();
    IsolationEvidence {
        container_markers,
        client_sandbox: layers.iter().any(|l| l.sandbox_enabled),
        credential_variables,
        credential_files,
        verified,
    }
}

/// The settings layers of `facts`, highest precedence first.
pub(crate) fn layers(facts: &Facts) -> Vec<SettingsLayer> {
    let mut out = Vec::new();
    if let Some(m) = &facts.managed {
        out.push(layer(facts, "managed", m));
    }
    let claude = facts.root.join(".claude");
    out.push(layer(facts, "local", &claude.join("settings.local.json")));
    out.push(layer(facts, "project", &claude.join("settings.json")));
    if let Some(h) = &facts.home {
        out.push(layer(facts, "user", &h.join(".claude/settings.json")));
    }
    out
}

/// The profile `facts` support, with the reason for it and for every stronger one refused.
pub(crate) fn status(facts: &Facts) -> AutonomyStatus {
    let layers = layers(facts);
    let isolation = isolation(facts, &layers);
    let trusted = checkout_trusted(facts);
    let managed = layers.iter().find(|l| l.scope == "managed" && l.present);
    let unreadable: Vec<&SettingsLayer> = layers.iter().filter(|l| l.error.is_some()).collect();
    let mut reasons = Vec::new();
    let mut refused = Vec::new();

    // isolated: verified isolation, and the organisation has not disabled the bypass
    let isolated_refusal = if !isolation.verified {
        let mut why = Vec::new();
        if isolation.container_markers.is_empty() {
            why.push("no container marker was observed".to_string());
        }
        if !isolation.credential_variables.is_empty() {
            why.push(format!(
                "credential variables are set: {}",
                isolation.credential_variables.join(", ")
            ));
        }
        if !isolation.credential_files.is_empty() {
            why.push(format!(
                "credential files are present: {}",
                isolation.credential_files.join(", ")
            ));
        }
        Some(format!("isolation is not verified: {}", why.join("; ")))
    } else if layers.iter().any(|l| l.bypass_disabled) {
        Some("a settings layer disables bypassPermissions".into())
    } else {
        None
    };
    // trusted: the person's own acceptance of the checkout, never the repository's word
    let trusted_refusal = match trusted {
        Some(true) => None,
        Some(false) => Some("the checkout was not accepted in the client's trust dialog".into()),
        None => Some("the client's record of trusted checkouts could not be read".into()),
    };
    // a managed default mode is the organisation's ceiling
    let managed_ceiling = managed.and_then(|l| l.default_mode.clone());

    let mut profile = match (isolated_refusal, trusted_refusal) {
        (None, _) => AutonomyProfile::Isolated,
        (Some(because), None) => {
            refused.push(ProfileRefusal {
                profile: AutonomyProfile::Isolated,
                because,
            });
            AutonomyProfile::Trusted
        }
        (Some(iso), Some(tru)) => {
            refused.push(ProfileRefusal {
                profile: AutonomyProfile::Isolated,
                because: iso,
            });
            refused.push(ProfileRefusal {
                profile: AutonomyProfile::Trusted,
                because: tru,
            });
            AutonomyProfile::Balanced
        }
    };
    // a layer the client reads and this cannot is rules nobody can see: fail closed
    if !unreadable.is_empty() && profile > AutonomyProfile::Conservative {
        let names: Vec<&str> = unreadable.iter().map(|l| l.path.as_str()).collect();
        refused.push(ProfileRefusal {
            profile,
            because: format!("a settings layer could not be read: {}", names.join(", ")),
        });
        profile = AutonomyProfile::Conservative;
    }
    if let Some(mode) = &managed_ceiling {
        let ceiling = match mode.as_str() {
            "default" | "plan" => AutonomyProfile::Conservative,
            "acceptEdits" => AutonomyProfile::Balanced,
            "auto" | "dontAsk" => AutonomyProfile::Trusted,
            _ => AutonomyProfile::Isolated,
        };
        if ceiling < profile {
            refused.push(ProfileRefusal {
                profile,
                because: format!("the managed settings set defaultMode {mode}"),
            });
            profile = ceiling;
        }
    }
    match profile {
        AutonomyProfile::Isolated => reasons.push(format!(
            "isolation verified by {} and no credential is in reach",
            isolation.container_markers.join(", ")
        )),
        AutonomyProfile::Trusted => {
            reasons.push("the person accepted this checkout in the client's trust dialog".into());
        }
        AutonomyProfile::Balanced => {
            reasons.push("every settings layer reads, and nothing establishes more".into());
        }
        AutonomyProfile::Conservative => {
            reasons.push("a fact above refused every stronger profile".into());
        }
    }
    if facts.root.join(".ai/manifest.yaml").is_file() {
        reasons.push(
            "the repository is supervised by Majordomus; that is the repository's own word and \
             raises no profile"
                .into(),
        );
    }
    let configured_mode = layers.iter().find_map(|l| l.default_mode.clone());
    AutonomyStatus {
        schema: STATUS_SCHEMA.into(),
        root: facts.shown(&facts.root),
        client_version: facts.env.get("CLAUDE_CODE_VERSION").cloned(),
        inside_client: facts.env.contains_key("CLAUDECODE"),
        layers,
        configured_mode,
        checkout_trusted: trusted,
        isolation,
        profile,
        recommended_mode: profile.mode(),
        reasons,
        refused,
        limitations: vec![
            "The running session's effective permission mode belongs to the client (a \
             --permission-mode flag or a mid-session switch) and is not observable from a \
             child process; configured_mode is what the settings say."
                .into(),
            "No container marker is not proof of no container, and a marker is not proof of \
             scoped mounts or network: isolated also requires that no credential be in reach."
                .into(),
        ],
    }
}

/// What a static rule may do with one command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Decision {
    /// A static allow rule may run it unasked under this profile.
    Allow,
    /// No static rule decides it: the client's own permission layer (its classifier, or a
    /// prompt) does.
    Defer,
    /// A person approves it, under every profile.
    RequireApproval,
}

/// The risk class of a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RiskClass {
    /// Reads the repository or the machine.
    Read,
    /// Builds, tests, formats or lints the repository.
    Quality,
    /// Writes local history (`git add`, `git commit`).
    LocalHistory,
    /// More than one command, or a redirection: the first word does not say what it does.
    Compound,
    /// Not in the table.
    Unknown,
    /// Rewrites or destroys history or files.
    Destructive,
    /// Raises privilege or stops processes.
    Privileged,
    /// Reaches a remote system or publishes.
    External,
}

/// The answer of `autonomy.explain`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct CommandDecision {
    /// The command, as asked.
    pub(crate) command: String,
    /// The profile it was judged under.
    pub(crate) profile: AutonomyProfile,
    /// The decision.
    pub(crate) decision: Decision,
    /// The risk class.
    pub(crate) risk: RiskClass,
    /// Why, in a sentence.
    pub(crate) reason: String,
    /// The table entry that matched, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) rule: Option<String>,
}

/// Characters and words that make a command more than one command, or send its output
/// somewhere its first word does not name.
const COMPOUND: &[&str] = &[
    ";", "&&", "||", "|", ">", "<", "`", "$(", "&", "\n", "<(", ">(",
];

/// The table: a word sequence, the class it puts a command in, and the entry's name. The
/// first entry whose words begin the command decides; flags listed after `!` refuse the
/// entry when present anywhere in the command.
const TABLE: &[(&str, RiskClass, &str)] = &[
    ("git push", RiskClass::External, "git.push"),
    ("git reset --hard", RiskClass::Destructive, "git.reset-hard"),
    ("git clean", RiskClass::Destructive, "git.clean"),
    ("git branch -D", RiskClass::Destructive, "git.branch-delete"),
    (
        "git checkout --",
        RiskClass::Destructive,
        "git.checkout-discard",
    ),
    ("git restore", RiskClass::Destructive, "git.restore"),
    ("git stash drop", RiskClass::Destructive, "git.stash-drop"),
    (
        "git filter-branch",
        RiskClass::Destructive,
        "git.filter-branch",
    ),
    ("rm", RiskClass::Destructive, "rm"),
    ("sudo", RiskClass::Privileged, "sudo"),
    ("kill", RiskClass::Privileged, "kill"),
    ("pkill", RiskClass::Privileged, "pkill"),
    ("killall", RiskClass::Privileged, "killall"),
    ("gh pr merge", RiskClass::External, "gh.pr-merge"),
    ("gh release", RiskClass::External, "gh.release"),
    ("gh api", RiskClass::External, "gh.api"),
    ("curl", RiskClass::External, "curl"),
    ("wget", RiskClass::External, "wget"),
    ("ssh", RiskClass::External, "ssh"),
    ("scp", RiskClass::External, "scp"),
    ("git status", RiskClass::Read, "git.status"),
    ("git diff", RiskClass::Read, "git.diff"),
    ("git log", RiskClass::Read, "git.log"),
    ("git show", RiskClass::Read, "git.show"),
    ("git rev-parse", RiskClass::Read, "git.rev-parse"),
    ("git ls-files", RiskClass::Read, "git.ls-files"),
    ("git branch", RiskClass::Read, "git.branch"),
    ("git add", RiskClass::LocalHistory, "git.add"),
    ("git commit", RiskClass::LocalHistory, "git.commit"),
    ("cargo build", RiskClass::Quality, "cargo.build"),
    ("cargo check", RiskClass::Quality, "cargo.check"),
    ("cargo test", RiskClass::Quality, "cargo.test"),
    ("cargo clippy", RiskClass::Quality, "cargo.clippy"),
    ("cargo fmt", RiskClass::Quality, "cargo.fmt"),
    ("cargo doc", RiskClass::Quality, "cargo.doc"),
    ("ls", RiskClass::Read, "ls"),
    ("cat", RiskClass::Read, "cat"),
    ("head", RiskClass::Read, "head"),
    ("tail", RiskClass::Read, "tail"),
    ("wc", RiskClass::Read, "wc"),
    ("grep", RiskClass::Read, "grep"),
    ("rg", RiskClass::Read, "rg"),
    ("find", RiskClass::Read, "find"),
];

/// Flags that turn a command the table reads as harmless into one that is not.
const ESCALATING_FLAGS: &[(&str, &str, RiskClass)] = &[
    ("git commit", "--amend", RiskClass::Destructive),
    ("git branch", "-D", RiskClass::Destructive),
    ("git branch", "--delete", RiskClass::Destructive),
    ("git branch", "-m", RiskClass::Destructive),
    ("git branch", "-M", RiskClass::Destructive),
    ("git branch", "--move", RiskClass::Destructive),
    ("git branch", "-c", RiskClass::Destructive),
    ("git branch", "-C", RiskClass::Destructive),
    ("git branch", "--copy", RiskClass::Destructive),
    ("git branch", "-f", RiskClass::Destructive),
    ("git branch", "--force", RiskClass::Destructive),
    ("find", "-delete", RiskClass::Destructive),
    ("find", "-exec", RiskClass::Compound),
    ("find", "-execdir", RiskClass::Compound),
    ("find", "-ok", RiskClass::Compound),
    ("find", "-okdir", RiskClass::Compound),
    // a read that writes a file of its choosing is not a read
    ("find", "-fprint", RiskClass::Destructive),
    ("find", "-fprint0", RiskClass::Destructive),
    ("find", "-fprintf", RiskClass::Destructive),
    ("find", "-fls", RiskClass::Destructive),
    ("git diff", "--output", RiskClass::Destructive),
    ("git log", "--output", RiskClass::Destructive),
    ("git show", "--output", RiskClass::Destructive),
    // a diff or a search that runs a program it was handed is that program
    ("git diff", "--ext-diff", RiskClass::Compound),
    ("git log", "--ext-diff", RiskClass::Compound),
    ("git show", "--ext-diff", RiskClass::Compound),
    ("git diff", "--textconv", RiskClass::Compound),
    ("git log", "--textconv", RiskClass::Compound),
    ("git show", "--textconv", RiskClass::Compound),
    ("rg", "--pre", RiskClass::Compound),
    ("rg", "--pre-glob", RiskClass::Compound),
    ("rg", "--search-zip", RiskClass::Compound),
    ("rg", "-z", RiskClass::Compound),
];

/// Whether `word` names `flag`: the flag itself, `flag=value` for a long flag (`--pre=cat`
/// is `--pre`), or a cluster of short flags that carries a one-letter one (`-uz` is `-z`).
/// Failing towards the escalation is the safe direction.
fn names_flag(word: &str, flag: &str) -> bool {
    if word == flag {
        return true;
    }
    if flag.starts_with("--") {
        return word
            .strip_prefix(flag)
            .is_some_and(|rest| rest.starts_with('='));
    }
    let letter = flag.strip_prefix('-').filter(|l| l.chars().count() == 1);
    match (letter, word.strip_prefix('-')) {
        (Some(l), Some(cluster)) => {
            !cluster.starts_with('-')
                && cluster.chars().all(|c| c.is_ascii_alphanumeric())
                && cluster.contains(l)
        }
        _ => false,
    }
}

/// Split a command into words, honouring single and double quotes; `None` when a quote
/// is left open.
fn words(command: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in command.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if quote.is_some() {
        return None;
    }
    if started || !cur.is_empty() {
        out.push(cur);
    }
    Some(out)
}

/// Whether a command is more than one command, judged outside quotes for every operator
/// but substitution, which a double-quoted string still performs.
fn compound(command: &str) -> Option<&'static str> {
    if command.contains("$(") || command.contains('`') {
        return Some(if command.contains("$(") { "$(" } else { "`" });
    }
    let mut outside = String::new();
    let mut quote: Option<char> = None;
    for c in command.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, c) => outside.push(c),
        }
    }
    COMPOUND.iter().copied().find(|op| outside.contains(op))
}

/// The decision `profile` gives `command`.
pub(crate) fn explain(command: &str, profile: AutonomyProfile) -> CommandDecision {
    let decided = |decision, risk, reason: String, rule: Option<&str>| CommandDecision {
        command: command.into(),
        profile,
        decision,
        risk,
        reason,
        rule: rule.map(str::to_string),
    };
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return decided(
            Decision::Defer,
            RiskClass::Unknown,
            "an empty command decides nothing".into(),
            None,
        );
    }
    let Some(w) = words(trimmed) else {
        return decided(
            Decision::Defer,
            RiskClass::Compound,
            "a quote is left open, so the words cannot be told apart".into(),
            None,
        );
    };
    let first_is_assignment = w
        .first()
        .is_some_and(|f| f.contains('=') && !f.starts_with('-'));
    // the risk boundaries are read before the compound test, so that `git push --force; ls`
    // is named for what it does, not only for being two commands
    let matched = TABLE.iter().find(|(prefix, _, _)| {
        let p: Vec<&str> = prefix.split(' ').collect();
        w.len() >= p.len() && w.iter().zip(&p).all(|(a, b)| a == b)
    });
    let boundary = |risk: RiskClass| {
        matches!(
            risk,
            RiskClass::Destructive | RiskClass::Privileged | RiskClass::External
        )
    };
    if let Some((prefix, risk, rule)) = matched {
        let mut risk = *risk;
        for (p, flag, escalated) in ESCALATING_FLAGS {
            if p == prefix && w.iter().any(|a| names_flag(a, flag)) {
                risk = *escalated;
            }
        }
        if boundary(risk) {
            return decided(
                Decision::RequireApproval,
                risk,
                match risk {
                    RiskClass::Destructive => {
                        "it can rewrite history or delete what nothing else holds".into()
                    }
                    RiskClass::Privileged => "it raises privilege or stops a process".into(),
                    _ => "it reaches a remote system or publishes".into(),
                },
                Some(rule),
            );
        }
    }
    if let Some(op) = compound(trimmed) {
        return decided(
            Decision::Defer,
            RiskClass::Compound,
            format!(
                "`{}` makes it more than one command; a static rule sees only the first word",
                op.escape_debug()
            ),
            None,
        );
    }
    if first_is_assignment {
        return decided(
            Decision::Defer,
            RiskClass::Compound,
            "it begins with a variable assignment, which can change what the command runs".into(),
            None,
        );
    }
    let Some((prefix, risk, rule)) = matched else {
        return decided(
            Decision::Defer,
            RiskClass::Unknown,
            "no table entry names it; the client's permission layer decides".into(),
            None,
        );
    };
    let mut risk = *risk;
    for (p, flag, escalated) in ESCALATING_FLAGS {
        if p == prefix && w.iter().any(|a| names_flag(a, flag)) {
            risk = *escalated;
        }
    }
    // what each profile lets run unasked; a compound risk is in no profile's set
    let allowed = match profile {
        AutonomyProfile::Conservative => matches!(risk, RiskClass::Read),
        AutonomyProfile::Balanced => matches!(risk, RiskClass::Read | RiskClass::Quality),
        AutonomyProfile::Trusted | AutonomyProfile::Isolated => matches!(
            risk,
            RiskClass::Read | RiskClass::Quality | RiskClass::LocalHistory
        ),
    };
    let reason = match (allowed, risk) {
        (true, RiskClass::Read) => "it reads the repository or the machine".into(),
        (true, RiskClass::Quality) => "a repository-local quality operation".into(),
        (true, _) => "it writes local history, which a push has yet to publish".into(),
        (false, RiskClass::Compound) => "it runs a command the table cannot see".into(),
        (false, _) => format!("the {profile:?} profile does not allow it unasked").to_lowercase(),
    };
    decided(
        if allowed {
            Decision::Allow
        } else {
            Decision::Defer
        },
        risk,
        reason,
        Some(rule),
    )
}

/// The answer of `autonomy.friction`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Friction {
    /// `majordomus.autonomy.friction/v1`.
    pub(crate) schema: String,
    /// The first instant counted (RFC 3339, UTC).
    pub(crate) since: String,
    /// The transcript directories read, written from `~`.
    pub(crate) directories: Vec<String>,
    /// Transcripts read.
    pub(crate) transcripts: usize,
    /// Tool calls made in the window.
    pub(crate) tool_calls: usize,
    /// Refusals by kind: `classifier_denial`, `hook_block`, `safety_check`,
    /// `user_rejection`.
    pub(crate) by_kind: BTreeMap<String, usize>,
    /// Classifier denials by the reason the classifier gave.
    pub(crate) by_reason: BTreeMap<String, usize>,
    /// Refusals by tool.
    pub(crate) by_tool: BTreeMap<String, usize>,
    /// Every refusal, per thousand tool calls; absent when no tool was called.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) per_thousand_calls: Option<f64>,
    /// What a transcript does not record.
    pub(crate) limitations: Vec<String>,
}

/// The kind of one refused tool result, from its text.
pub(crate) fn refusal_kind(text: &str) -> Option<(&'static str, Option<String>)> {
    if let Some(i) = text.find("auto mode classifier. Reason: [") {
        let rest = &text[i + "auto mode classifier. Reason: [".len()..];
        let reason = rest.split(']').next().map(str::to_string);
        return Some(("classifier_denial", reason));
    }
    if text.contains("auto mode classifier") {
        return Some(("classifier_denial", None));
    }
    if text.contains("hook error") && text.to_lowercase().contains("blocked") {
        return Some(("hook_block", None));
    }
    if text.contains("built-in Claude Code safety check") {
        return Some(("safety_check", None));
    }
    if text.contains("The user doesn't want to proceed with this tool use") {
        return Some(("user_rejection", None));
    }
    None
}

/// The client's transcript directory name of a checkout path.
pub(crate) fn transcript_slug(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// The primary checkout of `root`: a linked worktree lives at `<repository>-wt/<branch>`
/// (`project.worktree-topology`), so its primary is the path before `-wt/`.
fn primary_of(root: &Path) -> PathBuf {
    let s = root.to_string_lossy();
    match s.find("-wt/") {
        Some(i) => PathBuf::from(&s[..i]),
        None => root.to_path_buf(),
    }
}

fn result_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|i| i.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

/// Count what the permission layer cost in the transcripts of `root`'s checkouts under
/// `projects` (`~/.claude/projects`), from `since` on.
pub(crate) fn friction(
    projects: &Path,
    root: &Path,
    since: SystemTime,
    home: Option<&Path>,
) -> Friction {
    let since_text = crate::peers::rfc3339(since);
    let prefix = transcript_slug(&primary_of(root));
    let mut names: Vec<String> = std::fs::read_dir(projects)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| *n == prefix || n.starts_with(&format!("{prefix}-wt-")))
        .collect();
    crate::order::canonical_strings(&mut names);
    let dirs: Vec<PathBuf> = names.iter().map(|n| projects.join(n)).collect();
    let mut f = Friction {
        schema: FRICTION_SCHEMA.into(),
        since: since_text.clone(),
        directories: dirs
            .iter()
            .map(|d| match home.and_then(|h| d.strip_prefix(h).ok()) {
                Some(rest) => format!("~/{}", rest.display()),
                None => d.display().to_string(),
            })
            .collect(),
        transcripts: 0,
        tool_calls: 0,
        by_kind: BTreeMap::new(),
        by_reason: BTreeMap::new(),
        by_tool: BTreeMap::new(),
        per_thousand_calls: None,
        limitations: vec![
            "A prompt the person approved leaves no refusal in a transcript, so approved \
             prompts are not counted; what is counted is every refusal."
                .into(),
            "A transcript is attributed to the checkout it was started in; a session that \
             worked in another checkout is counted under its start."
                .into(),
        ],
    };
    for dir in &dirs {
        // a directory that cannot be listed holds no transcript this can read
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let fresh = entry
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|m| m >= since);
            if !fresh {
                continue;
            }
            f.transcripts += 1;
            count_transcript(&path, &since_text, &mut f);
        }
    }
    let refusals: usize = f.by_kind.values().sum();
    if f.tool_calls > 0 {
        let rate = refusals as f64 * 1000.0 / f.tool_calls as f64;
        f.per_thousand_calls = Some((rate * 100.0).round() / 100.0);
    }
    f
}

fn count_transcript(path: &Path, since: &str, f: &mut Friction) {
    let mut tools: BTreeMap<String, String> = BTreeMap::new();
    // a transcript that cannot be opened has no line to count
    let lines = std::fs::File::open(path)
        .into_iter()
        .flat_map(|file| std::io::BufReader::new(file).lines().map_while(Result::ok));
    for line in lines {
        let uses = line.contains("\"tool_use\"");
        let refused = line.contains("\"is_error\":true");
        if !uses && !refused {
            continue;
        }
        let Ok(rec) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if rec
            .get("timestamp")
            .and_then(Value::as_str)
            .is_some_and(|t| t < since)
        {
            continue;
        }
        let Some(items) = rec.pointer("/message/content").and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            match item.get("type").and_then(Value::as_str) {
                Some("tool_use") => {
                    f.tool_calls += 1;
                    if let (Some(id), Some(name)) = (
                        item.get("id").and_then(Value::as_str),
                        item.get("name").and_then(Value::as_str),
                    ) {
                        tools.insert(id.into(), name.into());
                    }
                }
                Some("tool_result")
                    if item.get("is_error").and_then(Value::as_bool) == Some(true) =>
                {
                    let text = result_text(item.get("content").unwrap_or(&Value::Null));
                    let Some((kind, reason)) = refusal_kind(&text) else {
                        continue;
                    };
                    *f.by_kind.entry(kind.into()).or_default() += 1;
                    if let Some(r) = reason {
                        *f.by_reason.entry(r).or_default() += 1;
                    }
                    let tool = item
                        .get("tool_use_id")
                        .and_then(Value::as_str)
                        .and_then(|id| tools.get(id))
                        .cloned()
                        .unwrap_or_else(|| "unknown".into());
                    *f.by_tool.entry(tool).or_default() += 1;
                }
                _ => {}
            }
        }
    }
}

/// `since_days` days before now.
pub(crate) fn days_ago(days: u32) -> SystemTime {
    SystemTime::now()
        .checked_sub(Duration::from_secs(u64::from(days) * 86_400))
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(tmp: &Path) -> Facts {
        let root = tmp.join("repo");
        let home = tmp.join("home");
        std::fs::create_dir_all(root.join(".claude")).unwrap();
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        Facts {
            root,
            home: Some(home),
            managed: Some(tmp.join("managed-settings.json")),
            env: BTreeMap::new(),
            container_files: Vec::new(),
        }
    }

    fn trust(f: &Facts, accepted: bool) {
        let doc = serde_json::json!({
            "projects": { f.root.to_str().unwrap(): { "hasTrustDialogAccepted": accepted } },
            "oauthAccount": { "secret": "never read" },
        });
        std::fs::write(
            f.home.as_ref().unwrap().join(".claude.json"),
            doc.to_string(),
        )
        .unwrap();
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mj-autonomy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_read_that_runs_or_writes_through_a_flag_is_not_a_read() {
        for command in [
            "rg --pre cat secret",
            "rg --pre=cat secret",
            "rg -uz pattern",
            "find . -ok rm {} ;",
            "find . -fprint /tmp/list",
            "git diff --output=/tmp/x",
            "git log --ext-diff",
            "git show --textconv HEAD",
            "git branch -M main",
            "git branch --force topic HEAD~3",
        ] {
            let d = explain(command, AutonomyProfile::Isolated);
            assert_ne!(d.decision, Decision::Allow, "{command}: {d:?}");
        }
        for command in [
            "rg -u pattern",
            "git diff --stat",
            "git branch -a",
            "find . -name x",
        ] {
            let d = explain(command, AutonomyProfile::Trusted);
            assert_eq!(d.decision, Decision::Allow, "{command}: {d:?}");
        }
        assert!(names_flag("--pre=cat", "--pre") && !names_flag("--prefix", "--pre"));
        assert!(names_flag("-uz", "-z") && !names_flag("--z", "-z") && !names_flag("-o", "-ok"));
    }

    #[test]
    fn an_untrusted_checkout_with_no_isolation_is_balanced_and_says_why() {
        let d = tmp("balanced");
        let f = facts(&d);
        let s = status(&f);
        assert_eq!(s.profile, AutonomyProfile::Balanced);
        assert_eq!(s.recommended_mode, PermissionMode::AcceptEdits);
        let refused: Vec<AutonomyProfile> = s.refused.iter().map(|r| r.profile).collect();
        assert_eq!(
            refused,
            [AutonomyProfile::Isolated, AutonomyProfile::Trusted]
        );
        assert!(s.refused[0].because.contains("no container marker"));
    }

    #[test]
    fn the_persons_trust_raises_the_profile_and_the_repositorys_word_does_not() {
        let d = tmp("trust");
        let f = facts(&d);
        std::fs::create_dir_all(f.root.join(".ai")).unwrap();
        std::fs::write(f.root.join(".ai/manifest.yaml"), "version: 1\n").unwrap();
        // a checked-in setting that asks for bypass raises nothing
        std::fs::write(
            f.root.join(".claude/settings.json"),
            r#"{"permissions":{"defaultMode":"bypassPermissions"}}"#,
        )
        .unwrap();
        assert_eq!(status(&f).profile, AutonomyProfile::Balanced);
        trust(&f, true);
        let s = status(&f);
        assert_eq!(s.profile, AutonomyProfile::Trusted);
        assert_eq!(s.recommended_mode, PermissionMode::Auto);
        assert_eq!(s.configured_mode.as_deref(), Some("bypassPermissions"));
        assert!(s.reasons.iter().any(|r| r.contains("raises no profile")));
        trust(&f, false);
        assert_eq!(status(&f).profile, AutonomyProfile::Balanced);
    }

    #[test]
    fn isolation_needs_a_container_and_no_credential_in_reach() {
        let d = tmp("isolated");
        let mut f = facts(&d);
        trust(&f, true);
        f.container_files = vec!["/.dockerenv".into()];
        assert_eq!(status(&f).profile, AutonomyProfile::Isolated);
        assert_eq!(
            status(&f).recommended_mode,
            PermissionMode::BypassPermissions
        );

        f.env.insert("GITHUB_TOKEN".into(), "x".into());
        let s = status(&f);
        assert_eq!(s.profile, AutonomyProfile::Trusted);
        assert_eq!(s.isolation.credential_variables, ["GITHUB_TOKEN"]);
        assert!(
            !serde_json::to_string(&s).unwrap().contains("\"x\""),
            "a value never leaves"
        );
        f.env.clear();

        let ssh = f.home.as_ref().unwrap().join(".ssh");
        std::fs::create_dir_all(&ssh).unwrap();
        std::fs::write(ssh.join("id_ed25519"), "k").unwrap();
        let s = status(&f);
        assert_eq!(s.profile, AutonomyProfile::Trusted);
        assert_eq!(s.isolation.credential_files, ["~/.ssh/id_ed25519"]);
    }

    #[test]
    fn a_layer_that_disables_bypass_or_a_managed_mode_is_a_ceiling() {
        let d = tmp("managed");
        let mut f = facts(&d);
        trust(&f, true);
        f.container_files = vec!["/run/.containerenv".into()];
        let managed = f.managed.clone().unwrap();
        std::fs::write(
            &managed,
            r#"{"permissions":{"disableBypassPermissionsMode":"disable"}}"#,
        )
        .unwrap();
        let s = status(&f);
        assert_eq!(s.profile, AutonomyProfile::Trusted);
        assert!(s.refused[0].because.contains("disables bypassPermissions"));
        std::fs::write(&managed, r#"{"permissions":{"defaultMode":"default"}}"#).unwrap();
        assert_eq!(status(&f).profile, AutonomyProfile::Conservative);
    }

    #[test]
    fn a_malformed_layer_fails_closed() {
        let d = tmp("malformed");
        let f = facts(&d);
        trust(&f, true);
        std::fs::write(f.root.join(".claude/settings.local.json"), "{ not json").unwrap();
        let s = status(&f);
        // an unreadable layer means the client may be running rules nobody can see
        assert!(s.layers.iter().any(|l| l.error.is_some()));
        assert_eq!(s.profile, AutonomyProfile::Conservative);
        assert!(s
            .refused
            .iter()
            .any(|r| r.because.contains("could not be read")));
    }

    #[test]
    fn a_layer_reports_counts_and_hooks_never_rule_text() {
        let d = tmp("layer");
        let f = facts(&d);
        std::fs::write(
            f.root.join(".claude/settings.json"),
            r#"{"permissions":{"allow":["Bash(secret-host *)"],"deny":["Bash(rm *)"]},
                "autoMode":{"allow":["$defaults","x"]},"hooks":{"SessionStart":[],"PreToolUse":[]}}"#,
        )
        .unwrap();
        let s = status(&f);
        let p = s.layers.iter().find(|l| l.scope == "project").unwrap();
        assert_eq!((p.allow, p.deny, p.auto_mode_allow), (1, 1, 2));
        assert_eq!(p.hooks, ["PreToolUse", "SessionStart"]);
        assert!(!serde_json::to_string(&s).unwrap().contains("secret-host"));
    }

    #[test]
    fn routine_work_is_allowed_by_the_profile_that_owns_it() {
        use AutonomyProfile::*;
        for (cmd, profile, want) in [
            ("git status", Conservative, Decision::Allow),
            ("cargo test --lib autonomy", Conservative, Decision::Defer),
            ("cargo test --lib autonomy", Balanced, Decision::Allow),
            ("git commit -m 'x'", Balanced, Decision::Defer),
            ("git commit -m 'x'", Trusted, Decision::Allow),
            ("grep -rn 'a;b' src", Trusted, Decision::Allow),
            ("make install", Isolated, Decision::Defer),
        ] {
            let d = explain(cmd, profile);
            assert_eq!(d.decision, want, "{cmd} under {profile:?}: {}", d.reason);
        }
    }

    #[test]
    fn the_risk_boundaries_hold_under_every_profile() {
        use AutonomyProfile::*;
        for profile in [Conservative, Balanced, Trusted, Isolated] {
            for (cmd, risk) in [
                ("git push --force origin master", RiskClass::External),
                ("git reset --hard HEAD~3", RiskClass::Destructive),
                ("git clean -fdx", RiskClass::Destructive),
                ("git branch -D feature/x", RiskClass::Destructive),
                ("git commit --amend --no-edit", RiskClass::Destructive),
                ("rm -rf target", RiskClass::Destructive),
                ("sudo true", RiskClass::Privileged),
                ("kill 1234", RiskClass::Privileged),
                ("gh pr merge 12 --admin", RiskClass::External),
                ("curl https://example.com", RiskClass::External),
                ("find . -name x -delete", RiskClass::Destructive),
                ("git push --force; ls", RiskClass::External),
            ] {
                let d = explain(cmd, profile);
                assert_eq!(
                    d.decision,
                    Decision::RequireApproval,
                    "{cmd} under {profile:?}"
                );
                assert_eq!(d.risk, risk, "{cmd}");
            }
        }
    }

    #[test]
    fn a_compound_command_never_earns_a_static_allow() {
        for cmd in [
            "git status && rm -rf /",
            "git status; curl evil",
            "cat a | sh",
            "git log > out.txt",
            "ls $(cat list)",
            "echo \"$(rm -rf x)\"",
            "ls `pwd`",
            "cargo test &",
            "FOO=bar cargo test",
            "find . -exec rm {} ;",
            "git status 'unterminated",
        ] {
            let d = explain(cmd, AutonomyProfile::Isolated);
            assert_ne!(d.decision, Decision::Allow, "{cmd}: {}", d.reason);
        }
        // an operator inside quotes is an argument, not an operator
        assert_eq!(
            explain("grep -n 'a && b' src/x.rs", AutonomyProfile::Trusted).decision,
            Decision::Allow
        );
    }

    #[test]
    fn friction_counts_refusals_by_kind_reason_and_tool_and_carries_no_payload() {
        let d = tmp("friction");
        let projects = d.join("projects");
        let root = Path::new("/w/repo");
        let primary = projects.join(transcript_slug(root));
        let linked = projects.join(format!("{}-wt-feature-x", transcript_slug(root)));
        let other = projects.join("-w-other");
        for dir in [&primary, &linked, &other] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let line = |ts: &str, content: Value| {
            serde_json::json!({ "timestamp": ts, "message": { "content": content } }).to_string()
        };
        let uses = |id: &str, name: &str| {
            serde_json::json!([{ "type": "tool_use", "id": id, "name": name,
                                  "input": { "command": "secret-command" } }])
        };
        let refused = |id: &str, text: &str| {
            serde_json::json!([{ "type": "tool_result", "tool_use_id": id, "is_error": true,
                                  "content": text }])
        };
        let t = [
            line("2026-10-16T10:00:00.000Z", uses("a", "Bash")),
            line("2026-10-16T10:00:01.000Z", refused("a",
                "Permission for this action was denied by the Claude Code auto mode classifier. Reason: [Self-Modification]. More.")),
            line("2026-10-16T10:00:02.000Z", uses("b", "Edit")),
            line("2026-10-16T10:00:03.000Z", refused("b", "PreToolUse:Edit hook error: BLOCKED — no")),
            line("2026-10-16T10:00:04.000Z", uses("c", "Bash")),
            line("2026-10-16T10:00:05.000Z", serde_json::json!([{ "type": "tool_result",
                "tool_use_id": "c", "is_error": true,
                "content": [{ "type": "text", "text": "The user doesn't want to proceed with this tool use." }] }])),
            line("2026-10-16T10:00:06.000Z", uses("d", "Bash")),
            line("2026-10-16T10:00:07.000Z", refused("d", "Exit code 1 an ordinary failure")),
            line("2000-01-01T00:00:00.000Z", uses("e", "Bash")),
        ]
        .join("\n");
        std::fs::write(primary.join("s1.jsonl"), &t).unwrap();
        std::fs::write(
            linked.join("s2.jsonl"),
            line("2026-10-16T11:00:00Z", uses("z", "Read")),
        )
        .unwrap();
        std::fs::write(other.join("s3.jsonl"), &t).unwrap();
        let since = SystemTime::UNIX_EPOCH + Duration::from_secs(1_760_000_000); // 2025-10
        let wt = Path::new("/w/repo-wt/feature/x");
        let f = friction(&projects, wt, since, None);
        assert_eq!(
            f.transcripts, 2,
            "the primary's and the linked worktree's, not another repository's"
        );
        assert_eq!(f.tool_calls, 5, "the call before the window is not counted");
        assert_eq!(f.by_kind.get("classifier_denial"), Some(&1));
        assert_eq!(f.by_kind.get("hook_block"), Some(&1));
        assert_eq!(f.by_kind.get("user_rejection"), Some(&1));
        assert_eq!(f.by_kind.len(), 3, "an ordinary failure is not a refusal");
        assert_eq!(f.by_reason.get("Self-Modification"), Some(&1));
        assert_eq!(
            (f.by_tool.get("Bash"), f.by_tool.get("Edit")),
            (Some(&2), Some(&1))
        );
        assert_eq!(f.per_thousand_calls, Some(600.0));
        assert!(!serde_json::to_string(&f)
            .unwrap()
            .contains("secret-command"));
    }

    #[test]
    fn every_managed_mode_is_its_own_ceiling() {
        use AutonomyProfile::*;
        let d = tmp("modes");
        let mut f = facts(&d);
        trust(&f, true);
        f.container_files = vec!["/.dockerenv".into()];
        let managed = f.managed.clone().unwrap();
        for (mode, want) in [
            ("default", Conservative),
            ("plan", Conservative),
            ("acceptEdits", Balanced),
            ("auto", Trusted),
            ("dontAsk", Trusted),
            ("bypassPermissions", Isolated),
        ] {
            let doc = serde_json::json!({ "permissions": { "defaultMode": mode } });
            std::fs::write(&managed, doc.to_string()).unwrap();
            assert_eq!(status(&f).profile, want, "{mode}");
        }
    }

    #[test]
    fn facts_without_a_home_or_managed_settings_read_what_there_is() {
        let d = tmp("nohome");
        let mut f = facts(&d);
        f.home = None;
        f.managed = None;
        f.env.insert("container".into(), "podman".into());
        let s = status(&f);
        let scopes: Vec<&str> = s.layers.iter().map(|l| l.scope.as_str()).collect();
        assert_eq!(scopes, ["local", "project"]);
        assert_eq!(s.isolation.container_markers, ["container variable"]);
        assert!(s.isolation.credential_files.is_empty());
        // verified isolation does not need the trust record that a missing home cannot hold
        assert_eq!(s.profile, AutonomyProfile::Isolated);
        assert_eq!(f.shown(Path::new("/x/y")), "/x/y");
    }

    #[test]
    fn a_trust_record_that_does_not_parse_or_name_the_checkout_trusts_nothing() {
        let d = tmp("record");
        let f = facts(&d);
        let record = f.home.as_ref().unwrap().join(".claude.json");
        std::fs::write(&record, "{ nope").unwrap();
        assert_eq!(checkout_trusted(&f), None);
        std::fs::write(
            &record,
            r#"{"projects":{"/elsewhere":{"hasTrustDialogAccepted":true}}}"#,
        )
        .unwrap();
        assert_eq!(checkout_trusted(&f), Some(false));
        let s = status(&f);
        assert!(s.refused.iter().any(|r| r.because.contains("not accepted")));
    }

    #[test]
    fn only_marker_files_that_exist_are_present() {
        let d = tmp("present");
        let marker = d.join("marker");
        std::fs::write(&marker, "").unwrap();
        let m = marker.to_str().unwrap();
        assert_eq!(present(&[m, "/definitely/not/here"]), [m]);
    }

    #[test]
    fn words_split_on_runs_of_space_and_an_empty_command_decides_nothing() {
        assert_eq!(words("ls  -la ").unwrap(), ["ls", "-la"]);
        assert_eq!(words("echo ''").unwrap(), ["echo", ""]);
        let d = explain("   ", AutonomyProfile::Trusted);
        assert_eq!((d.decision, d.risk), (Decision::Defer, RiskClass::Unknown));
        // a flag can turn a read into a command the table cannot see
        let d = explain("find . -execdir echo {} +", AutonomyProfile::Isolated);
        assert_eq!((d.decision, d.risk), (Decision::Defer, RiskClass::Compound));
        assert!(d.reason.contains("cannot see"), "{}", d.reason);
    }

    #[test]
    fn every_refusal_kind_is_told_from_its_text() {
        assert_eq!(
            refusal_kind("denied by the Claude Code auto mode classifier."),
            Some(("classifier_denial", None))
        );
        assert_eq!(
            refusal_kind("blocked by the built-in Claude Code safety check"),
            Some(("safety_check", None))
        );
        assert_eq!(refusal_kind("Exit code 2"), None);
        assert_eq!(result_text(&Value::Null), "");
    }

    #[test]
    fn friction_skips_what_is_not_a_fresh_transcript_record() {
        let d = tmp("skips");
        let projects = d.join("projects");
        let root = Path::new("/w/repo");
        let dir = projects.join(transcript_slug(root));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), "\"tool_use\"").unwrap();
        let lines = [
            r#"{"type":"summary"}"#.to_string(),
            r#"not json "tool_use""#.to_string(),
            r#"{"timestamp":"2026-10-16T10:00:00Z","note":"tool_use"}"#.to_string(),
            serde_json::json!({ "timestamp": "2026-10-16T10:00:01Z", "message": { "content": [
                { "type": "text", "text": "hi" },
                { "type": "tool_use" },
                { "type": "tool_use", "id": "q", "name": "Read" },
            ] } })
            .to_string(),
            serde_json::json!({ "timestamp": "2026-10-16T10:00:02Z", "message": { "content": [
                { "type": "tool_result", "tool_use_id": "gone", "is_error": true,
                  "content": "blocked by the built-in Claude Code safety check" },
            ] } })
            .to_string(),
        ];
        std::fs::write(dir.join("s.jsonl"), lines.join("\n")).unwrap();
        let since = SystemTime::UNIX_EPOCH + Duration::from_secs(1_760_000_000);
        let f = friction(&projects, root, since, Some(&d));
        assert_eq!(
            (f.transcripts, f.tool_calls),
            (1, 2),
            "only the .jsonl file is a transcript"
        );
        assert_eq!(
            f.by_tool.get("unknown"),
            Some(&1),
            "a refusal of no known call"
        );
        assert_eq!(
            f.directories,
            [format!("~/projects/{}", transcript_slug(root))]
        );
        // a transcript last written before the window is not read at all
        let later = friction(
            &projects,
            root,
            days_ago(0) + Duration::from_secs(86_400),
            None,
        );
        assert_eq!(later.transcripts, 0);
    }

    #[test]
    fn no_transcript_is_no_rate_rather_than_zero() {
        let d = tmp("empty");
        let f = friction(&d.join("absent"), Path::new("/w/repo"), days_ago(7), None);
        assert_eq!(
            (f.transcripts, f.tool_calls, f.per_thousand_calls),
            (0, 0, None)
        );
    }
}
