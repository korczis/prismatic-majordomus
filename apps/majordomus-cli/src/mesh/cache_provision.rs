//! A node provisioned as the mesh build cache's S3 provider (I2674, ADR 0157).
//!
//! The reviewed host layer of the build host — a disk guard, services that restart always,
//! a bounded journal and container log, a bounded MinIO bucket with expiry and a
//! least-privilege identity per node, and the rustc wrapper that makes a node's own builds
//! use the cache — as one declared policy
//! (`apps/majordomus-cli/assets/mesh-cache-provider/policy.json`) and the files it installs,
//! instead of a shell script an operator runs once. The files live inside the package
//! because they are compiled in, and the image's builder stage compiles the package alone.
//!
//! [`plan`] is a pure function of the policy and what was observed on the host: it names
//! every file, edit and command the host still owes and changes nothing. [`observe`] reads
//! the host (never a secret's content), and [`apply`] carries the plan out: files only under
//! a root that is not `/` (so a test or a staging directory needs no privilege), and files,
//! credentials and commands under `/`, as root, refusing without it. A second apply finds
//! every file current and changes nothing.
//!
//! No secret is declared here, printed, logged or returned: a node's credential is generated
//! on the host into a mode 0600 file outside the repository, and every report names its path
//! only.
//!
//! No secret travels on a command line either (I3996): an argument is readable by every user
//! of the host in `ps` and `/proc/<pid>/cmdline` for as long as the command runs. A node's
//! identity is made with `mc admin user add <alias> <access key>`, and its secret is the
//! command's standard input. That is mc's own documented form: in `cmd/admin-user-add.go`
//! (`fetchUserKeys`, minio/mc master at 77f82e18b54) a call with the alias and the access
//! key alone, on a standard input that is not a terminal, reads the secret as the first line
//! of that input, and the command's help shows it piped ("using piped keys"). Standard input
//! was chosen over `MC_HOST_<alias>`, which would carry the admin's credential in the
//! environment of the child, and over an alias file, which would put a secret on disk for the
//! length of the call.
//!

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The declared policy, embedded from `assets/mesh-cache-provider/policy.json` in the package.
const POLICY: &str = include_str!("../../assets/mesh-cache-provider/policy.json");
const CRITICAL: &str = include_str!("../../assets/mesh-cache-provider/critical.conf");
const JOURNALD: &str = include_str!("../../assets/mesh-cache-provider/journald.conf");
const DAEMON: &str = include_str!("../../assets/mesh-cache-provider/daemon.json");
const GUARD: &str = include_str!("../../assets/mesh-cache-provider/disk-guard");
const GUARD_SERVICE: &str = include_str!("../../assets/mesh-cache-provider/disk-guard.service");
const GUARD_TIMER: &str = include_str!("../../assets/mesh-cache-provider/disk-guard.timer");
const RW_POLICY: &str = include_str!("../../assets/mesh-cache-provider/mesh-build-cache-rw.json");
const WRAPPER: &str = include_str!("../../assets/mesh-cache-provider/sccache-mesh");

/// Where the per-node credentials and environments live on the provider, outside any
/// repository and readable by root alone.
const NODE_DIR: &str = "/etc/majordomus/mesh-build-cache";

/// What the provisioning declares: who runs the builds, which store, how bounded, which
/// nodes receive an identity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// The user whose builds use the cache on this host.
    pub user: String,
    /// That user's home directory.
    pub home: String,
    /// The `mc` alias of the store.
    pub alias: String,
    /// The address other nodes reach the store at.
    pub tailnet_address: String,
    /// The store's port.
    pub port: u16,
    /// The cache bucket.
    pub bucket: String,
    /// The region sccache signs for.
    pub region: String,
    /// Days after which an entry expires.
    pub expire_days: u32,
    /// The cache bucket's hard quota, in GiB.
    pub bucket_quota_gib: u32,
    /// The CI cache bucket this host already serves, bounded the same way.
    pub gitlab_bucket: String,
    /// Its quota, in GiB.
    pub gitlab_quota_gib: u32,
    /// The name of the least-privilege policy every node identity carries.
    pub policy_name: String,
    /// The node this host is: its builds use the loopback endpoint.
    pub local_node: String,
    /// Every node that receives an identity.
    pub nodes: Vec<String>,
    /// The services that restart always and are the last the OOM killer takes.
    pub critical_services: Vec<String>,
}

impl Policy {
    /// The policy this repository declares.
    pub fn declared() -> Policy {
        serde_json::from_str(POLICY).expect("assets/mesh-cache-provider/policy.json parses")
    }

    fn endpoint(&self, node: &str) -> String {
        if node == self.local_node {
            format!("http://127.0.0.1:{}", self.port)
        } else {
            format!("http://{}:{}", self.tailnet_address, self.port)
        }
    }

    fn identity(node: &str) -> String {
        format!("mesh-cache-{node}")
    }

    fn node_env(&self, node: &str) -> String {
        format!(
            "MESH_CACHE_ENDPOINT={}\nMESH_CACHE_BUCKET={}\nMESH_CACHE_REGION={}\nMESH_CACHE_ACCESS_KEY={}\n",
            self.endpoint(node),
            self.bucket,
            self.region,
            Self::identity(node)
        )
    }

    fn cargo_config(&self) -> String {
        format!("{}/.cargo/config.toml", self.home)
    }

    fn old_wrapper(&self) -> String {
        format!("rustc-wrapper = \"{}/.cargo/bin/sccache\"", self.home)
    }

    fn new_wrapper(&self) -> String {
        format!("rustc-wrapper = \"{}/.local/bin/sccache-mesh\"", self.home)
    }
}

/// What a file's content is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Content {
    /// Text the repository declares.
    Text(String),
    /// A secret generated on the host when the file is missing; never declared.
    Generated,
    /// A copy of another file of the host (the local node's credential into its home).
    CopyOf(String),
}

/// One file the provisioning owns.
struct Want {
    path: String,
    mode: u32,
    owner: Option<String>,
    content: Content,
    source: String,
}

/// What a file is on the host, as far as it was readable. A secret's content is never
/// carried: only whether it exists, its mode, and whether it equals its source.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Observed {
    /// The path exists.
    pub exists: bool,
    /// Its mode bits, when it exists.
    pub mode: Option<u32>,
    /// Its text, for a declared (non-secret) file.
    pub text: Option<String>,
    /// For a copy: whether it equals its source; for a secret: nothing.
    pub same_as_source: Option<bool>,
    /// The host would not let this process look (a root-only directory).
    pub unreadable: bool,
}

/// Where a file stands against what the policy wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileState {
    /// Absent.
    Missing,
    /// Present with other content.
    Different,
    /// Right content, wrong mode.
    Mode,
    /// As wanted.
    Current,
    /// Could not be observed by this process; apply as root decides.
    Unobservable,
}

/// One file of the plan.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileAction {
    /// The path on the host.
    pub path: String,
    /// The mode it must have, in octal.
    pub mode: String,
    /// The owner it must have, when not root.
    pub owner: Option<String>,
    /// Where its content comes from: `declared`, `generated` or `copy:<path>`. Never the content.
    pub source: String,
    /// Where it stands.
    pub state: FileState,
}

/// A line edit the plan owes: the cargo config of the build user.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditAction {
    /// The file edited.
    pub path: String,
    /// The line replaced.
    pub from: String,
    /// The line it becomes.
    pub to: String,
    /// The edit is still owed.
    pub owed: bool,
}

/// A command the apply runs once the files are in place, as root.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Step {
    /// A stable name.
    pub id: String,
    /// The command as it is run. A node's secret is never on it: `<secret of NODE>` names
    /// what the command reads on its standard input.
    pub line: String,
    /// Run only when a file of the named group changed (`systemd`, `journald`, `docker`), or always.
    pub when: String,
}

/// The plan: every file, edit and command the host still owes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Plan {
    /// Every file the policy owns.
    pub files: Vec<FileAction>,
    /// The line edits.
    pub edits: Vec<EditAction>,
    /// The commands, in the order they run.
    pub steps: Vec<Step>,
    /// How many files are missing, different or of the wrong mode, plus edits owed.
    pub changes: usize,
    /// How many files this process could not observe.
    pub unobservable: usize,
}

fn wants(policy: &Policy) -> Vec<Want> {
    let declared = |path: String, mode: u32, text: &str, src: &str| Want {
        path,
        mode,
        owner: None,
        content: Content::Text(text.to_string()),
        source: format!("declared:{src}"),
    };
    let mut out = vec![
        declared(
            "/usr/local/libexec/majordomus/disk-guard".into(),
            0o755,
            GUARD,
            "disk-guard",
        ),
        declared(
            "/etc/systemd/system/disk-guard.service".into(),
            0o644,
            GUARD_SERVICE,
            "disk-guard.service",
        ),
        declared(
            "/etc/systemd/system/disk-guard.timer".into(),
            0o644,
            GUARD_TIMER,
            "disk-guard.timer",
        ),
        declared(
            "/etc/systemd/journald.conf.d/50-build-host.conf".into(),
            0o644,
            JOURNALD,
            "journald.conf",
        ),
        declared(
            "/etc/docker/daemon.json".into(),
            0o644,
            DAEMON,
            "daemon.json",
        ),
        declared(
            format!("{NODE_DIR}/{}.json", policy.policy_name),
            0o644,
            RW_POLICY,
            "mesh-build-cache-rw.json",
        ),
    ];
    for unit in &policy.critical_services {
        out.push(declared(
            format!("/etc/systemd/system/{unit}.service.d/50-build-host.conf"),
            0o644,
            CRITICAL,
            "critical.conf",
        ));
    }
    for node in &policy.nodes {
        let env = policy.node_env(node);
        out.push(Want {
            path: format!("{NODE_DIR}/{node}/credentials"),
            mode: 0o600,
            owner: None,
            content: Content::Generated,
            source: "generated".into(),
        });
        out.push(Want {
            path: format!("{NODE_DIR}/{node}/env"),
            mode: 0o600,
            owner: None,
            content: Content::Text(env),
            source: "declared:policy".into(),
        });
    }
    let home = &policy.home;
    let local = &policy.local_node;
    out.push(Want {
        path: format!("{home}/.config/mesh-build-cache/env"),
        mode: 0o644,
        owner: Some(policy.user.clone()),
        content: Content::Text(policy.node_env(local)),
        source: "declared:policy".into(),
    });
    out.push(Want {
        path: format!("{home}/.config/mesh-build-cache/credentials"),
        mode: 0o600,
        owner: Some(policy.user.clone()),
        content: Content::CopyOf(format!("{NODE_DIR}/{local}/credentials")),
        source: format!("copy:{NODE_DIR}/{local}/credentials"),
    });
    out.push(Want {
        path: format!("{home}/.local/bin/sccache-mesh"),
        mode: 0o755,
        owner: Some(policy.user.clone()),
        content: Content::Text(WRAPPER.to_string()),
        source: "declared:sccache-mesh".into(),
    });
    out
}

fn state_of(want: &Want, seen: Option<&Observed>) -> FileState {
    let Some(o) = seen else {
        return FileState::Missing;
    };
    if o.unreadable {
        return FileState::Unobservable;
    }
    if !o.exists {
        return FileState::Missing;
    }
    let same = match &want.content {
        Content::Text(t) => o.text.as_deref() == Some(t.as_str()),
        Content::Generated => true,
        Content::CopyOf(_) => o.same_as_source == Some(true),
    };
    if !same {
        return FileState::Different;
    }
    if o.mode.map(|m| m & 0o777) != Some(want.mode) {
        return FileState::Mode;
    }
    FileState::Current
}

/// The plan, a pure function of the policy and what was observed. An absent observation is
/// a missing file; a file the process could not look at is `unobservable`, counted apart and
/// never as a change.
pub fn plan(policy: &Policy, observed: &BTreeMap<String, Observed>) -> Plan {
    let mut files = Vec::new();
    for w in wants(policy) {
        let state = state_of(&w, observed.get(&w.path));
        files.push(FileAction {
            path: w.path,
            mode: format!("{:04o}", w.mode),
            owner: w.owner,
            source: w.source,
            state,
        });
    }
    let cfg = policy.cargo_config();
    let owed = observed
        .get(&cfg)
        .and_then(|o| o.text.as_deref())
        .is_some_and(|t| t.lines().any(|l| l == policy.old_wrapper()));
    let edits = vec![EditAction {
        path: cfg,
        from: policy.old_wrapper(),
        to: policy.new_wrapper(),
        owed,
    }];
    let changes = files
        .iter()
        .filter(|f| {
            matches!(
                f.state,
                FileState::Missing | FileState::Different | FileState::Mode
            )
        })
        .count()
        + edits.iter().filter(|e| e.owed).count();
    let unobservable = files
        .iter()
        .filter(|f| f.state == FileState::Unobservable)
        .count();
    Plan {
        files,
        edits,
        steps: steps(policy),
        changes,
        unobservable,
    }
}

fn mc(policy: &Policy, rest: &str) -> String {
    format!("runuser -u {} -- mc --quiet {rest}", policy.user)
}

fn steps(policy: &Policy) -> Vec<Step> {
    let a = &policy.alias;
    let b = &policy.bucket;
    let s = |id: &str, line: String, when: &str| Step {
        id: id.into(),
        line,
        when: when.into(),
    };
    let mut v = vec![
        s(
            "systemd-reload",
            "systemctl daemon-reload".into(),
            "systemd",
        ),
        s(
            "journald-restart",
            "systemctl restart systemd-journald".into(),
            "journald",
        ),
        s(
            "docker-validate",
            "dockerd --validate --config-file /etc/docker/daemon.json".into(),
            "docker",
        ),
        s(
            "docker-restart",
            "systemctl restart docker (only when no container runs)".into(),
            "docker",
        ),
        s(
            "guard-timer",
            "systemctl enable --now disk-guard.timer".into(),
            "always",
        ),
        s(
            "ci-cache-quota",
            mc(
                policy,
                &format!(
                    "quota set {a}/{} --size {}GiB",
                    policy.gitlab_bucket, policy.gitlab_quota_gib
                ),
            ),
            "always",
        ),
        s(
            "bucket",
            mc(policy, &format!("mb --ignore-existing {a}/{b}")),
            "always",
        ),
        s(
            "expiry",
            mc(
                policy,
                &format!(
                    "ilm rule add --expire-days {} {a}/{b} (only when no such rule)",
                    policy.expire_days
                ),
            ),
            "always",
        ),
        s(
            "quota",
            mc(
                policy,
                &format!("quota set {a}/{b} --size {}GiB", policy.bucket_quota_gib),
            ),
            "always",
        ),
        s(
            "policy",
            mc(
                policy,
                &format!(
                    "admin policy create {a} {} {NODE_DIR}/{}.json",
                    policy.policy_name, policy.policy_name
                ),
            ),
            "always",
        ),
    ];
    for node in &policy.nodes {
        let id = Policy::identity(node);
        v.push(s(
            &format!("identity-{node}"),
            mc(
                policy,
                &format!("admin user add {a} {id} (standard input: <secret of {node}>)"),
            ),
            "always",
        ));
        v.push(s(
            &format!("attach-{node}"),
            mc(
                policy,
                &format!("admin policy attach {a} {} --user {id}", policy.policy_name),
            ),
            "always",
        ));
    }
    v.push(s(
        "guard-run",
        "systemctl start disk-guard.service".into(),
        "always",
    ));
    v
}

fn physical(root: &Path, logical: &str) -> PathBuf {
    root.join(logical.trim_start_matches('/'))
}

fn mode_of(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .ok()
        .map(|m| m.permissions().mode() & 0o777)
}

/// Read the host under `root` for every path of the policy. A secret file's content is never
/// kept; a copy is compared in this process and only the verdict is returned.
pub fn observe(policy: &Policy, root: &Path) -> BTreeMap<String, Observed> {
    let mut seen = BTreeMap::new();
    let all = wants(policy);
    for w in &all {
        let p = physical(root, &w.path);
        let mut o = Observed::default();
        match std::fs::symlink_metadata(&p) {
            Ok(_) => {
                o.exists = true;
                o.mode = mode_of(&p);
                match &w.content {
                    Content::Text(_) => o.text = std::fs::read_to_string(&p).ok(),
                    Content::Generated => {}
                    Content::CopyOf(from) => {
                        o.same_as_source =
                            match (std::fs::read(&p), std::fs::read(physical(root, from))) {
                                (Ok(a), Ok(b)) => Some(a == b),
                                _ => None,
                            };
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => o.unreadable = true,
            Err(_) => {}
        }
        seen.insert(w.path.clone(), o);
    }
    let cfg = policy.cargo_config();
    let p = physical(root, &cfg);
    let o = Observed {
        exists: p.exists(),
        mode: mode_of(&p),
        text: std::fs::read_to_string(&p).ok(),
        ..Observed::default()
    };
    seen.insert(cfg, o);
    seen
}

/// What an apply did.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplyReport {
    /// Nothing failed and nothing was refused.
    pub ok: bool,
    /// Why nothing was done, when it was refused.
    pub refused: Option<String>,
    /// The root the files were written under.
    pub root: String,
    /// Paths written or corrected, never their content.
    pub written: Vec<String>,
    /// Paths of credentials this run generated.
    pub generated: Vec<String>,
    /// The command lines that ran, each with its outcome.
    pub ran: Vec<Ran>,
    /// Commands that did not run, and why.
    pub skipped: Vec<String>,
    /// The plan after the apply: `changes` is 0 when the host now holds the policy.
    pub after: Option<Plan>,
}

/// One command that ran.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Ran {
    /// The line, with a secret as a placeholder.
    pub line: String,
    /// It exited zero.
    pub ok: bool,
    /// Its output when it failed (a secret removed).
    pub output: String,
}

fn is_root() -> bool {
    // SAFETY: geteuid reads the effective user id and cannot fail.
    unsafe { libc::geteuid() == 0 }
}

fn write_file(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    // a path without a parent (`/`, or an empty one) has nothing to make: "" is made already
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new("")))?;
    let tmp = path.with_extension(format!("mj-tmp-{}", std::process::id()));
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(&tmp)
        .and_then(|mut f| f.write_all(bytes))?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))
        .and_then(|()| std::fs::rename(&tmp, path))
}

fn secret_hex() -> Result<String, String> {
    secret_from(getrandom::getrandom)
}

/// 24 bytes from `fill` as 48 hex digits; a refusal to fill is "no entropy".
fn secret_from<E: std::fmt::Display>(
    fill: impl FnOnce(&mut [u8]) -> Result<(), E>,
) -> Result<String, String> {
    let mut b = [0u8; 24];
    fill(&mut b).map_err(|e| format!("no entropy: {e}"))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

fn secret_of(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("aws_secret_access_key = "))
        .map(str::to_string)
}

/// Carry the plan out. Under a root other than `/` it writes the files alone and needs no
/// privilege; under `/` it needs root, refuses without it (before touching anything), and
/// also installs ownership and runs the commands. Idempotent: a current file is not
/// rewritten, an existing credential is never regenerated, and the report's `after` plan is
/// the host re-observed.
pub fn apply(policy: &Policy, root: &Path) -> ApplyReport {
    apply_with(policy, root, is_root(), &run_command)
}

/// Runs one argv, with what its standard input carries (a secret travels there and never in
/// the argv), and answers whether it succeeded and what it said.
type Runner<'a> = &'a dyn Fn(&[String], Option<&str>) -> (bool, String);

fn run_command(argv: &[String], stdin: Option<&str>) -> (bool, String) {
    use std::process::Stdio;
    let Some((program, args)) = argv.split_first() else {
        return (false, "empty command".into());
    };
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd.spawn().and_then(|mut child| {
        // the input is written and its pipe closed before the wait, so a child that reads to
        // the end sees the end; a child that exits without reading is not an error here
        if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
            let _ = pipe.write_all(input.as_bytes());
        }
        child.wait_with_output()
    });
    match output {
        Ok(o) => (
            o.status.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ),
        ),
        Err(e) => (false, e.to_string()),
    }
}

fn apply_with(policy: &Policy, root: &Path, root_user: bool, run: Runner<'_>) -> ApplyReport {
    let system = root == Path::new("/");
    if system && !root_user {
        return refused_without_root(root);
    }
    apply_in(policy, root, system, run, &secret_hex)
}

fn refused_without_root(root: &Path) -> ApplyReport {
    ApplyReport {
        ok: false,
        refused: Some(
            "provisioning this host needs root: run `sudo majordomus mesh build-cache provision --apply` (or give --root a staging directory)"
                .into(),
        ),
        root: root.display().to_string(),
        written: Vec::new(),
        generated: Vec::new(),
        ran: Vec::new(),
        skipped: Vec::new(),
        after: None,
    }
}

/// Carry the policy out under `root`; `system` says the files are the host's own, so
/// ownership is installed and the commands run, and `secret` makes a new credential's secret.
fn apply_in(
    policy: &Policy,
    root: &Path,
    system: bool,
    run: Runner<'_>,
    secret: &dyn Fn() -> Result<String, String>,
) -> ApplyReport {
    let mut report = ApplyReport {
        ok: true,
        refused: None,
        root: root.display().to_string(),
        written: Vec::new(),
        generated: Vec::new(),
        ran: Vec::new(),
        skipped: Vec::new(),
        after: None,
    };
    let before = observe(policy, root);
    let mut groups: Vec<&str> = Vec::new();
    for w in wants(policy) {
        let state = state_of(&w, before.get(&w.path));
        if state == FileState::Current {
            continue;
        }
        let p = physical(root, &w.path);
        let result = match &w.content {
            Content::Text(t) => write_file(&p, t.as_bytes(), w.mode),
            Content::Generated => match (state, secret()) {
                (FileState::Mode, _) => set_mode(&p, w.mode),
                (_, Ok(secret)) => {
                    let body = format!(
                        "[mesh-build-cache]\naws_access_key_id = {}\naws_secret_access_key = {secret}\n",
                        Policy::identity(w.path.rsplit('/').nth(1).unwrap_or(""))
                    );
                    report.generated.push(w.path.clone());
                    write_file(&p, body.as_bytes(), w.mode)
                }
                (_, Err(e)) => Err(std::io::Error::other(e)),
            },
            Content::CopyOf(from) => match std::fs::read(physical(root, from)) {
                Ok(bytes) => write_file(&p, &bytes, w.mode),
                Err(e) => Err(e),
            },
        };
        match result {
            Ok(()) => {
                report.written.push(w.path.clone());
                for (needle, group) in [
                    ("/systemd/system/", "systemd"),
                    ("/journald.conf.d/", "journald"),
                    ("/docker/daemon.json", "docker"),
                ] {
                    if w.path.contains(needle) && !groups.contains(&group) {
                        groups.push(group);
                    }
                }
                if system {
                    if let Some(owner) = &w.owner {
                        let (ok, out) = run(
                            &["chown".into(), format!("{owner}:{owner}"), w.path.clone()],
                            None,
                        );
                        report.ran.push(Ran {
                            line: format!("chown {owner}:{owner} {}", w.path),
                            ok,
                            output: out,
                        });
                        report.ok &= ok;
                    }
                }
            }
            Err(e) => {
                report.ok = false;
                report.skipped.push(format!("{}: {e}", w.path));
            }
        }
    }
    edit_cargo(policy, root, &mut report);
    if system {
        run_steps(policy, root, &groups, run, &mut report);
    } else {
        report.skipped.push(format!(
            "commands: the root is {}, not /, so only files are written",
            report.root
        ));
    }
    report.after = Some(plan(policy, &observe(policy, root)));
    report.ok &= report.after.as_ref().is_some_and(|p| p.changes == 0);
    report
}

fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

fn edit_cargo(policy: &Policy, root: &Path, report: &mut ApplyReport) {
    let cfg = policy.cargo_config();
    let p = physical(root, &cfg);
    let Ok(text) = std::fs::read_to_string(&p) else {
        return;
    };
    if !text.lines().any(|l| l == policy.old_wrapper()) {
        return;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = PathBuf::from(format!("{}.bak.{stamp}", p.display()));
    let edited: Vec<String> = text
        .lines()
        .map(|l| {
            if l == policy.old_wrapper() {
                policy.new_wrapper()
            } else {
                l.to_string()
            }
        })
        .collect();
    let mode = mode_of(&p).unwrap_or(0o644);
    let done = std::fs::copy(&p, &backup)
        .and_then(|_| write_file(&p, format!("{}\n", edited.join("\n")).as_bytes(), mode));
    match done {
        Ok(()) => report.written.push(cfg),
        Err(e) => {
            report.ok = false;
            report.skipped.push(format!("{cfg}: {e}"));
        }
    }
}

fn run_steps(
    policy: &Policy,
    root: &Path,
    groups: &[&str],
    run: Runner<'_>,
    report: &mut ApplyReport,
) {
    // `secret`, when there is one, is the command's standard input and is redacted from what
    // it said; it is never an element of `argv`
    let push = |report: &mut ApplyReport, line: &str, argv: Vec<String>, secret: Option<&str>| {
        let stdin = secret.map(|s| format!("{s}\n"));
        let (ok, out) = run(&argv, stdin.as_deref());
        let out = match secret {
            Some(s) if !s.is_empty() => out.replace(s, "<redacted>"),
            _ => out,
        };
        report.ok &= ok;
        report.ran.push(Ran {
            line: line.to_string(),
            ok,
            output: if ok {
                String::new()
            } else {
                out.trim().to_string()
            },
        });
        (ok, out)
    };
    let sh = |s: &str| -> Vec<String> { s.split_whitespace().map(String::from).collect() };
    let mcv = |rest: &str| -> Vec<String> {
        let mut v = sh(&format!("runuser -u {} -- mc --quiet", policy.user));
        v.extend(sh(rest));
        v
    };
    for step in steps(policy) {
        let due = step.when == "always" || groups.contains(&step.when.as_str());
        if !due {
            report
                .skipped
                .push(format!("{}: no file of its group changed", step.id));
            continue;
        }
        let a = &policy.alias;
        let b = &policy.bucket;
        match step.id.as_str() {
            "docker-restart" => {
                let (ok, out) = run(&sh("docker ps -q"), None);
                if ok && out.trim().is_empty() {
                    push(
                        report,
                        "systemctl restart docker",
                        sh("systemctl restart docker"),
                        None,
                    );
                } else {
                    report.skipped.push(
                        "docker-restart: a job is running; daemon.json applies on its next restart"
                            .into(),
                    );
                }
            }
            "expiry" => {
                let (_, out) = run(&mcv(&format!("ilm rule ls {a}/{b}")), None);
                if out.contains(&format!("{} ", policy.expire_days)) {
                    report
                        .skipped
                        .push("expiry: the rule is already there".into());
                } else {
                    push(
                        report,
                        &step.line,
                        mcv(&format!(
                            "ilm rule add --expire-days {} {a}/{b}",
                            policy.expire_days
                        )),
                        None,
                    );
                }
            }
            id if id.starts_with("identity-") => {
                let node = &id["identity-".len()..];
                let cred = physical(root, &format!("{NODE_DIR}/{node}/credentials"));
                let Some(secret) = secret_of(&cred) else {
                    report.ok = false;
                    report
                        .skipped
                        .push(format!("{id}: the credential file has no secret"));
                    continue;
                };
                // the alias and the access key alone: mc reads the secret from its standard
                // input (cmd/admin-user-add.go, fetchUserKeys), so it is in no process's argv
                let argv = mcv(&format!("admin user add {a} {}", Policy::identity(node)));
                push(report, &step.line, argv, Some(&secret));
            }
            _ => {
                let argv = if step.line.starts_with("runuser") {
                    let rest = step.line.split("mc --quiet").nth(1).unwrap_or("");
                    // the lines that carry a note in parentheses are handled above
                    mcv(rest.split(" (").next().unwrap_or(rest))
                } else {
                    sh(step.line.split(" (").next().unwrap_or(&step.line))
                };
                push(report, &step.line, argv, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::os::unix::fs::PermissionsExt;

    fn scratch(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("mj-cache-provision-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_declared_policy_names_every_node_with_its_own_identity() {
        let p = Policy::declared();
        let planned = plan(&p, &BTreeMap::new());
        for node in &p.nodes {
            assert!(planned
                .files
                .iter()
                .any(|f| f.path == format!("{NODE_DIR}/{node}/credentials")));
            assert!(planned
                .steps
                .iter()
                .any(|s| s.id == format!("identity-{node}")));
        }
        assert_eq!(planned.unobservable, 0);
    }

    #[test]
    fn a_plan_never_carries_a_secret_or_a_credential_content() {
        let p = Policy::declared();
        let json = serde_json::to_string(&plan(&p, &BTreeMap::new())).unwrap();
        assert!(!json.contains("aws_secret_access_key"));
        assert!(json.contains("<secret of jetson>"));
        // credentials are generated, not declared
        assert!(
            plan(&p, &BTreeMap::new())
                .files
                .iter()
                .filter(|f| f.path.ends_with("/credentials") && f.source == "generated")
                .count()
                == p.nodes.len()
        );
    }

    #[test]
    fn an_unreadable_file_is_not_a_change() {
        let p = Policy::declared();
        let mut seen = BTreeMap::new();
        let path = format!("{NODE_DIR}/lundra/credentials");
        seen.insert(
            path.clone(),
            Observed {
                unreadable: true,
                ..Observed::default()
            },
        );
        let planned = plan(&p, &seen);
        assert_eq!(planned.unobservable, 1);
        assert_eq!(
            planned.files.iter().find(|f| f.path == path).unwrap().state,
            FileState::Unobservable
        );
    }

    #[test]
    fn a_wrong_mode_on_a_credential_is_a_change() {
        let p = Policy::declared();
        let w = wants(&p)
            .into_iter()
            .find(|w| w.content == Content::Generated)
            .unwrap();
        let o = Observed {
            exists: true,
            mode: Some(0o644),
            ..Observed::default()
        };
        assert_eq!(state_of(&w, Some(&o)), FileState::Mode);
        let ok = Observed {
            exists: true,
            mode: Some(0o600),
            ..Observed::default()
        };
        assert_eq!(state_of(&w, Some(&ok)), FileState::Current);
    }

    #[test]
    fn apply_into_a_staging_root_is_idempotent_and_keeps_credentials() {
        let root = scratch("idem");
        let p = Policy::declared();
        let first = apply(&p, &root);
        assert!(first.ok, "{first:?}");
        assert_eq!(first.generated.len(), p.nodes.len());
        let cred = physical(&root, &format!("{NODE_DIR}/mbp/credentials"));
        assert_eq!(
            std::fs::metadata(&cred).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let secret = std::fs::read_to_string(&cred).unwrap();
        let second = apply(&p, &root);
        assert!(
            second.ok && second.written.is_empty() && second.generated.is_empty(),
            "{second:?}"
        );
        assert_eq!(second.after.as_ref().unwrap().changes, 0);
        assert_eq!(std::fs::read_to_string(&cred).unwrap(), secret);
        // the local node's home copy equals its provider copy
        let home_cred = physical(
            &root,
            &format!("{}/.config/mesh-build-cache/credentials", p.home),
        );
        let local = physical(&root, &format!("{NODE_DIR}/{}/credentials", p.local_node));
        assert_eq!(
            std::fs::read(home_cred).unwrap(),
            std::fs::read(local).unwrap()
        );
        let json = serde_json::to_string(&first).unwrap();
        assert!(
            !json.contains("aws_secret_access_key")
                && !json.contains(secret.lines().nth(2).unwrap())
        );
    }

    #[test]
    fn apply_repairs_drift_and_edits_the_cargo_wrapper_once() {
        let root = scratch("drift");
        let p = Policy::declared();
        apply(&p, &root);
        let daemon = physical(&root, "/etc/docker/daemon.json");
        std::fs::write(&daemon, "{}").unwrap();
        let cfg = physical(&root, &p.cargo_config());
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(&cfg, format!("[build]\n{}\n", p.old_wrapper())).unwrap();
        let planned = plan(&p, &observe(&p, &root));
        assert_eq!(planned.changes, 2);
        let r = apply(&p, &root);
        assert!(r.ok, "{r:?}");
        let text = std::fs::read_to_string(&cfg).unwrap();
        assert!(text.contains(&p.new_wrapper()) && !text.contains(&p.old_wrapper()));
        assert_eq!(std::fs::read_to_string(&daemon).unwrap(), DAEMON);
        assert!(apply(&p, &root).written.is_empty());
    }

    #[test]
    fn the_system_root_is_refused_without_root_before_anything_is_touched() {
        let called = RefCell::new(0);
        let run = |_: &[String], _: Option<&str>| {
            *called.borrow_mut() += 1;
            (true, String::new())
        };
        let r = apply_with(&Policy::declared(), Path::new("/"), false, &run);
        assert!(!r.ok && r.refused.is_some() && r.written.is_empty());
        assert_eq!(*called.borrow(), 0);
    }

    #[test]
    fn steps_run_in_order_with_the_secret_redacted_from_every_record() {
        let root = scratch("steps");
        let p = Policy::declared();
        // files first, so the credentials exist; then only the steps run through a recorder
        apply(&p, &root);
        let lines = RefCell::new(Vec::<String>::new());
        let secret = secret_of(&physical(&root, &format!("{NODE_DIR}/imac/credentials"))).unwrap();
        let run = |argv: &[String], _: Option<&str>| {
            lines.borrow_mut().push(argv.join(" "));
            (true, format!("echo {secret}"))
        };
        let mut report = apply_with(&p, &root, true, &run);
        run_steps(&p, &root, &["systemd", "docker"], &run, &mut report);
        let recorded = serde_json::to_string(&report).unwrap();
        assert!(!recorded.contains(&secret));
        let all = lines.borrow().join("\n");
        assert!(all.contains("systemctl daemon-reload"));
        assert!(all.contains("admin user add vtminio mesh-cache-imac"));
        let reload = all.find("daemon-reload").unwrap();
        let attach = all.find("admin policy attach").unwrap();
        assert!(reload < attach);
    }

    /// I3996: every node's secret is the standard input of its own `admin user add`, and no
    /// argv of any command the apply runs carries any node's secret.
    #[test]
    fn a_node_secret_is_the_standard_input_of_its_identity_step_and_in_no_argv() {
        let (root, p) = staged("stdin");
        let secrets: Vec<(String, String)> = p
            .nodes
            .iter()
            .map(|n| {
                let cred = physical(&root, &format!("{NODE_DIR}/{n}/credentials"));
                (n.clone(), secret_of(&cred).unwrap())
            })
            .collect();
        let calls = RefCell::new(Vec::<(Vec<String>, Option<String>)>::new());
        let run = |argv: &[String], stdin: Option<&str>| {
            calls
                .borrow_mut()
                .push((argv.to_vec(), stdin.map(str::to_string)));
            (true, String::new())
        };
        let mut report = apply_with(&p, &root, true, &run);
        run_steps(&p, &root, &[], &run, &mut report);
        let calls = calls.into_inner();
        for (_, secret) in &secrets {
            assert!(
                calls
                    .iter()
                    .all(|(argv, _)| argv.iter().all(|a| !a.contains(secret.as_str()))),
                "a secret is in an argv"
            );
        }
        for (node, secret) in &secrets {
            let id = Policy::identity(node);
            let tail = ["add".to_string(), p.alias.clone(), id.clone()];
            let add: Vec<_> = calls
                .iter()
                .filter(|(argv, _)| argv.ends_with(&tail))
                .collect();
            // a staging root runs no command of its own: the steps run once, from run_steps
            assert_eq!(add.len(), 1, "{node}: its identity step runs once");
            for (argv, stdin) in add {
                // mc's form with the secret read from standard input: alias and access key only
                assert_eq!(
                    &argv[argv.len() - 5..argv.len() - 2],
                    ["admin", "user", "add"]
                );
                assert_eq!(stdin.as_deref(), Some(format!("{secret}\n").as_str()));
            }
        }
        // every other command reads nothing
        assert!(calls
            .iter()
            .filter(|(argv, _)| !argv.iter().any(|a| a == "add"))
            .all(|(_, stdin)| stdin.is_none()));
        // and the plan says where the secret goes, by name only
        let step = steps(&p)
            .into_iter()
            .find(|s| s.id == "identity-mbp")
            .unwrap();
        assert!(step.line.ends_with("(standard input: <secret of mbp>)"));
    }

    /// The real runner hands the input to a stand-in `mc` on its standard input and nothing
    /// else on its command line; a command given no input reads an empty, closed stdin.
    #[test]
    fn the_runner_gives_the_secret_to_the_childs_stdin_and_never_to_its_argv() {
        let dir = scratch("standin");
        let mc = dir.join("mc");
        let argv_log = dir.join("argv");
        let stdin_log = dir.join("stdin");
        std::fs::write(
            &mc,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\ncat > '{}'\necho done\n",
                argv_log.display(),
                stdin_log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&mc, std::fs::Permissions::from_mode(0o755)).unwrap();
        let secret = secret_from(|b| {
            b.fill(0x5a);
            Ok::<(), String>(())
        })
        .unwrap();
        let mut argv = vec![mc.display().to_string()];
        argv.extend(
            ["admin", "user", "add", "vtminio", "mesh-cache-mbp"]
                .iter()
                .map(|s| s.to_string()),
        );
        let (ok, out) = run_command(&argv, Some(&format!("{secret}\n")));
        assert!(ok, "{out}");
        assert_eq!(out.trim(), "done");
        let seen_argv = std::fs::read_to_string(&argv_log).unwrap();
        assert_eq!(seen_argv, "admin\nuser\nadd\nvtminio\nmesh-cache-mbp\n");
        assert!(!seen_argv.contains(&secret));
        assert_eq!(
            std::fs::read_to_string(&stdin_log).unwrap(),
            format!("{secret}\n")
        );
        // no input: the child's stdin is empty, never the parent's
        let (ok, _) = run_command(&argv, None);
        assert!(ok);
        assert_eq!(std::fs::read_to_string(&stdin_log).unwrap(), "");
        // a child that exits without reading its input is not an error of the runner
        let (ok, _) = run_command(&["true".into()], Some("unread\n"));
        assert!(ok);
    }

    fn staged(name: &str) -> (PathBuf, Policy) {
        let root = scratch(name);
        let p = Policy::declared();
        apply(&p, &root);
        (root, p)
    }

    fn run_all(
        root: &Path,
        p: &Policy,
        groups: &[&str],
        answer: impl Fn(&str, Option<&str>) -> (bool, String),
    ) -> (ApplyReport, Vec<String>) {
        let lines = RefCell::new(Vec::<String>::new());
        let run = |argv: &[String], stdin: Option<&str>| {
            let line = argv.join(" ");
            let out = answer(&line, stdin);
            lines.borrow_mut().push(line);
            out
        };
        let mut report = apply_with(p, root, true, &run);
        report.skipped.clear();
        report.ran.clear();
        report.ok = true;
        run_steps(p, root, groups, &run, &mut report);
        (report, lines.into_inner())
    }

    #[test]
    fn docker_restarts_only_when_no_container_runs() {
        let (root, p) = staged("docker");
        let (idle, lines) = run_all(&root, &p, &["docker"], |_, _| (true, String::new()));
        assert!(lines.iter().any(|l| l == "systemctl restart docker"));
        assert!(idle
            .ran
            .iter()
            .any(|r| r.line == "systemctl restart docker"));
        let (busy, lines) = run_all(&root, &p, &["docker"], |l, _| {
            if l == "docker ps -q" {
                (true, "abc123\n".into())
            } else {
                (true, String::new())
            }
        });
        assert!(!lines.iter().any(|l| l == "systemctl restart docker"));
        assert!(busy.skipped.iter().any(|s| s.starts_with("docker-restart")));
    }

    #[test]
    fn the_expiry_rule_is_added_once() {
        let (root, p) = staged("expiry");
        let has = format!("{} ", p.expire_days);
        let (present, lines) = run_all(&root, &p, &[], |l, _| {
            if l.contains("ilm rule ls") {
                (true, format!("rule {has}days"))
            } else {
                (true, String::new())
            }
        });
        assert!(!lines.iter().any(|l| l.contains("ilm rule add")));
        assert!(present.skipped.iter().any(|s| s.starts_with("expiry")));
        let (_, lines) = run_all(&root, &p, &[], |_, _| (true, String::new()));
        assert!(lines
            .iter()
            .any(|l| l.contains("ilm rule add --expire-days")));
    }

    #[test]
    fn a_failing_command_is_reported_without_the_secret_and_fails_the_report() {
        let (root, p) = staged("fail");
        let secret = secret_of(&physical(&root, &format!("{NODE_DIR}/mbp/credentials"))).unwrap();
        // a failing mc that repeats what it read: the secret reaches the output and must not
        // reach the report
        let (report, _) = run_all(&root, &p, &[], |l, stdin| {
            if l.contains("admin user add") {
                (false, format!("denied for {}", stdin.unwrap_or("").trim()))
            } else {
                (true, String::new())
            }
        });
        assert!(!report.ok);
        let bad: Vec<_> = report.ran.iter().filter(|r| !r.ok).collect();
        assert!(!bad.is_empty() && bad.iter().all(|r| r.output.contains("<redacted>")));
        assert!(!serde_json::to_string(&report).unwrap().contains(&secret));
    }

    #[test]
    fn a_credential_without_a_secret_stops_its_identity_step() {
        let (root, p) = staged("nosecret");
        let cred = physical(&root, &format!("{NODE_DIR}/mbp/credentials"));
        std::fs::write(&cred, "[mesh-build-cache]\n").unwrap();
        let (report, lines) = run_all(&root, &p, &[], |_, _| (true, String::new()));
        assert!(!report.ok);
        assert!(report.skipped.iter().any(|s| s.starts_with("identity-mbp")));
        assert!(!lines.iter().any(|l| l.contains("mesh-cache-mbp ")));
    }

    #[test]
    fn a_credential_with_the_wrong_mode_is_repaired_not_regenerated() {
        let (root, p) = staged("mode");
        let cred = physical(&root, &format!("{NODE_DIR}/mbp/credentials"));
        let before = std::fs::read_to_string(&cred).unwrap();
        std::fs::set_permissions(&cred, std::fs::Permissions::from_mode(0o644)).unwrap();
        let r = apply(&p, &root);
        assert!(r.ok && r.generated.is_empty(), "{r:?}");
        assert_eq!(std::fs::read_to_string(&cred).unwrap(), before);
        assert_eq!(mode_of(&cred), Some(0o600));
    }

    #[test]
    fn a_home_copy_that_differs_is_put_back_from_the_provider_copy() {
        let (root, p) = staged("copy");
        let home = physical(
            &root,
            &format!("{}/.config/mesh-build-cache/credentials", p.home),
        );
        std::fs::write(&home, "stale").unwrap();
        let seen = observe(&p, &root);
        let key = format!("{}/.config/mesh-build-cache/credentials", p.home);
        assert_eq!(seen[&key].same_as_source, Some(false));
        let r = apply(&p, &root);
        assert!(r.ok && r.written == vec![key], "{r:?}");
    }

    #[test]
    fn a_directory_the_process_cannot_enter_is_unobservable_not_missing() {
        let root = scratch("denied");
        let p = Policy::declared();
        let etc = root.join("etc");
        std::fs::create_dir_all(&etc).unwrap();
        std::fs::set_permissions(&etc, std::fs::Permissions::from_mode(0o000)).unwrap();
        let seen = observe(&p, &root);
        std::fs::set_permissions(&etc, std::fs::Permissions::from_mode(0o755)).unwrap();
        if is_root() {
            return;
        }
        let planned = plan(&p, &seen);
        assert!(planned.unobservable > 0);
        assert!(planned
            .files
            .iter()
            .any(|f| f.path == "/etc/docker/daemon.json" && f.state == FileState::Unobservable));
    }

    #[test]
    fn a_cargo_config_without_the_old_wrapper_is_left_alone() {
        let (root, p) = staged("cargo");
        let cfg = physical(&root, &p.cargo_config());
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(&cfg, "[build]\njobs = 3\n").unwrap();
        let planned = plan(&p, &observe(&p, &root));
        assert!(planned.edits.iter().all(|e| !e.owed));
        assert!(apply(&p, &root).written.is_empty());
        assert_eq!(
            std::fs::read_to_string(&cfg).unwrap(),
            "[build]\njobs = 3\n"
        );
    }

    #[test]
    fn write_file_and_secret_helpers_hold_their_contracts() {
        let root = scratch("helpers");
        let f = root.join("a/b/c.txt");
        write_file(&f, b"x", 0o600).unwrap();
        assert_eq!(mode_of(&f), Some(0o600));
        assert!(!f
            .with_extension(format!("mj-tmp-{}", std::process::id()))
            .exists());
        let a = secret_hex().unwrap();
        assert_eq!(a.len(), 48);
        assert_ne!(a, secret_hex().unwrap());
        assert_eq!(secret_of(&root.join("missing")), None);
        assert!(!run_command(&[], None).0);
        assert!(!run_command(&["/no/such/program".into()], None).0);
        assert!(run_command(&["true".into()], None).0);
    }

    #[test]
    fn a_secret_is_hex_of_what_the_source_gave_or_no_entropy() {
        assert_eq!(
            secret_from(|b| {
                b.fill(0xab);
                Ok::<(), String>(())
            })
            .unwrap(),
            "ab".repeat(24)
        );
        assert_eq!(
            secret_from(|_| Err("device gone")),
            Err("no entropy: device gone".into())
        );
    }

    #[test]
    fn a_home_copy_whose_provider_copy_cannot_be_read_is_unknown_and_not_written() {
        let (root, p) = staged("nosource");
        let key = format!("{}/.config/mesh-build-cache/credentials", p.home);
        let provider = physical(&root, &format!("{NODE_DIR}/{}/credentials", p.local_node));
        std::fs::remove_file(&provider).unwrap();
        std::fs::create_dir(&provider).unwrap();
        let seen = observe(&p, &root);
        assert_eq!(
            seen[&key].same_as_source, None,
            "unreadable source: no verdict"
        );
        let r = apply(&p, &root);
        assert!(!r.ok, "{r:?}");
        assert!(
            r.skipped.iter().any(|s| s.starts_with(&format!("{key}: "))),
            "the copy that could not be made is named: {r:?}"
        );
        assert!(!r.written.contains(&key));
    }

    #[test]
    fn write_file_reports_a_parent_that_is_a_file_and_a_temp_name_that_is_taken() {
        let root = scratch("writefail");
        let file = root.join("plain");
        std::fs::write(&file, "x").unwrap();
        assert!(
            write_file(&file.join("child"), b"x", 0o600).is_err(),
            "a file cannot be a parent directory"
        );
        let target = root.join("target.txt");
        let tmp = target.with_extension(format!("mj-tmp-{}", std::process::id()));
        std::fs::create_dir(&tmp).unwrap();
        assert!(
            write_file(&target, b"x", 0o600).is_err(),
            "a directory sits where the temporary file goes"
        );
        assert!(
            !target.exists(),
            "nothing is renamed into place after a failure"
        );
    }

    #[test]
    fn a_file_that_cannot_be_written_fails_the_report_and_is_named() {
        let root = scratch("applyfail");
        let p = Policy::declared();
        // /etc is a file, so nothing under it can be created
        std::fs::write(root.join("etc"), "x").unwrap();
        let r = apply(&p, &root);
        assert!(!r.ok, "{r:?}");
        assert!(r
            .skipped
            .iter()
            .any(|s| s.starts_with("/etc/docker/daemon.json: ")));
        assert!(!r.written.contains(&"/etc/docker/daemon.json".to_string()));
    }

    #[test]
    fn no_entropy_for_a_new_credential_skips_it_and_fails_the_report() {
        let root = scratch("noentropy");
        let p = Policy::declared();
        let run = |_: &[String], _: Option<&str>| (true, String::new());
        let r = apply_in(&p, &root, false, &run, &|| Err("no entropy: test".into()));
        assert!(!r.ok, "{r:?}");
        assert!(r.generated.is_empty());
        assert!(r
            .skipped
            .iter()
            .any(|s| s.contains("/credentials: no entropy: test")));
    }

    #[test]
    fn the_host_itself_gets_ownership_and_commands_and_a_staging_root_does_not() {
        let root = scratch("system");
        let p = Policy::declared();
        let lines = RefCell::new(Vec::<String>::new());
        let run = |argv: &[String], _: Option<&str>| {
            lines.borrow_mut().push(argv.join(" "));
            (true, String::new())
        };
        let r = apply_in(&p, &root, true, &run, &secret_hex);
        let owner = format!("chown {u}:{u} ", u = p.user);
        let owned: Vec<&Ran> = r
            .ran
            .iter()
            .filter(|x| x.line.starts_with(&owner))
            .collect();
        assert!(!owned.is_empty(), "files with an owner are chowned: {r:?}");
        assert!(owned.iter().all(|x| x.ok));
        assert!(
            lines.borrow().iter().any(|l| l.contains("admin user add")),
            "the steps run on the host"
        );
        assert!(!r
            .skipped
            .iter()
            .any(|s| s.starts_with("commands: the root is")));
        let staged_run = apply(&p, &scratch("system-staged"));
        assert!(staged_run
            .skipped
            .iter()
            .any(|s| s.starts_with("commands: the root is")));
        assert!(staged_run.ran.is_empty());
    }

    #[test]
    fn a_failed_chown_fails_the_report() {
        let root = scratch("chownfail");
        let p = Policy::declared();
        let run =
            |argv: &[String], _: Option<&str>| (argv[0] != "chown", "no such user".to_string());
        let r = apply_in(&p, &root, true, &run, &secret_hex);
        assert!(!r.ok);
        assert!(r.ran.iter().any(|x| x.line.starts_with("chown ") && !x.ok));
    }

    #[test]
    fn a_cargo_config_that_cannot_be_rewritten_is_reported() {
        let (root, p) = staged("cargofail");
        let cfg = physical(&root, &p.cargo_config());
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(&cfg, format!("[build]\n{}\n", p.old_wrapper())).unwrap();
        // the backup goes next to the config; a directory with that name makes the copy fail
        let stamp_dir_blocker = cfg.parent().unwrap();
        std::fs::set_permissions(stamp_dir_blocker, std::fs::Permissions::from_mode(0o555))
            .unwrap();
        let blocked = std::fs::File::create(stamp_dir_blocker.join("probe")).is_ok();
        let mut report = ApplyReport {
            ok: true,
            refused: None,
            root: root.display().to_string(),
            written: Vec::new(),
            generated: Vec::new(),
            ran: Vec::new(),
            skipped: Vec::new(),
            after: None,
        };
        edit_cargo(&p, &root, &mut report);
        std::fs::set_permissions(stamp_dir_blocker, std::fs::Permissions::from_mode(0o755))
            .unwrap();
        if blocked {
            // running as a user the mode does not bind: the edit simply succeeds
            return;
        }
        assert!(!report.ok);
        assert!(report
            .skipped
            .iter()
            .any(|s| s.starts_with(&p.cargo_config())));
        assert!(report.written.is_empty());
    }
}
