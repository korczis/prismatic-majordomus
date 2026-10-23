//! The sccache client configuration, derived from the selected store (I2673, ADR 0157).
//!
//! **Derived, never typed.** [`report`] answers what a compiler cache client on this machine
//! should be configured with *now*: the local disk as the first level and, only when every
//! condition below holds, the store [`select`](super::select) chose as the second. Nothing is
//! written anywhere; the answer is variables for the environment of the process that starts
//! the sccache server, and `~/.cargo/config.toml` is never touched (ADR 0157, alternatives).
//!
//! **The second level is a start-up decision.** sccache fixes its chain when its server
//! starts and refuses to start when an S3 level does not answer then (measured on 0.17.0 and
//! 0.18.0: the compile exits 2). A store that stops while the server runs is harmless (a
//! miss). So the second level is handed over only when all of these hold at this moment, and
//! the answer says which one did not when it is not:
//!
//! 1. `sccache` answers `--version`;
//! 2. the node file (outside the repository) offers or knows a store, and [`select`](super::select)
//!    chooses one: the store answered its readiness route within the budget, so an unknown is
//!    never taken for healthy;
//! 3. a credential file exists, is a regular file readable by its owner alone, and holds the
//!    profile [`PROFILE`];
//! 4. the installed sccache has the multi-level chain, which is *measured*, not assumed: a
//!    throwaway server is started on a private port with a chain naming a level that does
//!    not exist, and an sccache that knows the chain refuses it by name where one that does
//!    not simply starts.
//!
//! **No secret.** The credential is named by the path of its file (`AWS_SHARED_CREDENTIALS_FILE`)
//! and a profile; the file is opened only to see that the profile is in it, and nothing read
//! from it is kept. An exported key is readable in every compiler's environment, in
//! `ps eww` and in every build script, so no key is ever exported.
//!
//! # Example
//!
//! ```
//! use majordomus_cli::mesh::build_cache::env::{report, EnvOptions, Mode};
//! // no sccache here: the build is exactly what it was, and the answer says why
//! let answer = report(&EnvOptions {
//!     sccache: "/no/such/sccache".into(),
//!     node_file: None,
//!     credentials: None,
//! });
//! assert_eq!(answer.mode, Mode::LocalOnly);
//! assert!(answer.variables.is_empty());
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{offers_of, read_node_file, select, FileError, NodeFile, PassedOver, Policy};
use crate::environment::probe::bounded_output;
use crate::environment::shell::quote_posix;

/// The profile of the node's credential file.
pub const PROFILE: &str = "mesh-build-cache";

/// The region an S3-compatible store of this kind is addressed under when it states none.
pub const DEFAULT_REGION: &str = "us-east-1";

/// The node a store declared by this machine is attributed to when it is the asking node's.
const OWN_NODE: &str = "this-node";

/// How long the store's readiness answer is waited for, connect and answer together.
const HEALTH_BUDGET: Duration = Duration::from_millis(300);

/// How long one `sccache` invocation may take.
const SCCACHE_BUDGET: Duration = Duration::from_secs(10);

/// What the client is configured as.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::env::Mode;
/// assert_ne!(Mode::Mesh, Mode::LocalOnly);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Local disk first, the selected store second.
    Mesh,
    /// The local disk alone: the build is what it was.
    LocalOnly,
}

/// One variable of the client's environment.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::env::EnvVar;
/// let v = EnvVar { name: "SCCACHE_REGION".into(), value: "us-east-1".into() };
/// assert_eq!(v.name, "SCCACHE_REGION");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "BuildCacheEnvVar")]
pub struct EnvVar {
    /// The variable.
    pub name: String,
    /// Its value: never a key, at most the path of a credential file.
    pub value: String,
}

/// What the installed sccache is, as measured.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::env::Sccache;
/// let s = Sccache { found: false, version: None, multilevel: None, detail: "absent".into() };
/// assert!(!s.found);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "BuildCacheSccache")]
pub struct Sccache {
    /// Whether `--version` was answered.
    pub found: bool,
    /// The version it stated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Whether it has the multi-level chain: measured, and unknown when the measurement
    /// could not be made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multilevel: Option<bool>,
    /// How that was found out, in words.
    pub detail: String,
}

/// The store the second level is.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::env::Store;
/// let s = Store { node: "n1".into(), provider: "minio".into(),
///     endpoint: "http://192.168.100.30:9000".into(), bucket: "b".into() };
/// assert_eq!(s.provider, "minio");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "BuildCacheStore")]
pub struct Store {
    /// The offering node.
    pub node: String,
    /// What serves the store.
    pub provider: String,
    /// The non-secret endpoint.
    pub endpoint: String,
    /// The bucket.
    pub bucket: String,
}

/// The answer of [`report`].
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::env::{EnvReport, Mode, Sccache};
/// let r = EnvReport {
///     mode: Mode::LocalOnly, reason: "sccache is not installed".into(),
///     sccache: Sccache { found: false, version: None, multilevel: None, detail: String::new() },
///     store: None, credentials_file: None, variables: vec![], passed_over: vec![],
/// };
/// assert!(r.variables.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "BuildCacheEnvReport")]
pub struct EnvReport {
    /// What the client is configured as.
    pub mode: Mode,
    /// Why, always one sentence; for a local-only answer, the condition that did not hold.
    pub reason: String,
    /// The installed sccache.
    pub sccache: Sccache,
    /// The selected store, when one was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<Store>,
    /// The path of the credential file the variables name; never its content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials_file: Option<String>,
    /// The variables to set in the environment of the process that starts the sccache
    /// server; empty when the client is local-only.
    pub variables: Vec<EnvVar>,
    /// The offers selection passed over, with the rule.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub passed_over: Vec<PassedOver>,
}

/// What [`report`] reads: the sccache executable to ask, and optionally the node file and the
/// credential file to use instead of the defaults under the user's home.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::env::EnvOptions;
/// let o = EnvOptions { sccache: "sccache".into(), node_file: None, credentials: None };
/// assert_eq!(o.sccache, "sccache");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvOptions {
    /// The sccache executable, a name on `PATH` or a path.
    pub sccache: String,
    /// The node file; `<config>/majordomus/build-cache.json` when unset.
    pub node_file: Option<PathBuf>,
    /// The credential file; `<config>/mesh-build-cache/credentials` when unset.
    pub credentials: Option<PathBuf>,
}

/// `$XDG_CONFIG_HOME`, else `$HOME/.config`.
fn config_dir() -> Option<PathBuf> {
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(x));
    }
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(|h| PathBuf::from(h).join(".config"))
}

/// Where this node keeps what it offers and knows.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::env::default_node_file;
/// if let Some(p) = default_node_file() {
///     assert!(p.ends_with("majordomus/build-cache.json"));
/// }
/// ```
pub fn default_node_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join("majordomus").join("build-cache.json"))
}

/// Where this node keeps its own client credential, an AWS-format file.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::env::default_credentials;
/// if let Some(p) = default_credentials() {
///     assert!(p.ends_with("mesh-build-cache/credentials"));
/// }
/// ```
pub fn default_credentials() -> Option<PathBuf> {
    config_dir().map(|d| d.join("mesh-build-cache").join("credentials"))
}

/// A string on one line, so that a reason can be a shell comment.
fn one_line(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Whether the credential file at `path` may be named: a regular file only its owner can
/// reach, holding the profile. Its content is read to find the profile header and is not kept.
fn credentials_problem(path: &Path) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return Some(format!("no credential file at {}", path.display())),
    };
    if !meta.is_file() {
        return Some(format!("{} is not a regular file", path.display()));
    }
    if meta.permissions().mode() & 0o077 != 0 {
        return Some(format!(
            "{} is reachable by others (mode {:o}): it must be 0600",
            path.display(),
            meta.permissions().mode() & 0o777
        ));
    }
    match std::fs::read_to_string(path) {
        Ok(text) if text.lines().any(|l| l.trim() == format!("[{PROFILE}]")) => None,
        Ok(_) => Some(format!("{} has no [{PROFILE}] profile", path.display())),
        Err(_) => Some(format!("{} cannot be read", path.display())),
    }
}

/// A port nothing listens on, for a throwaway server.
fn private_port() -> Option<u16> {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0)).ok()?;
    Some(l.local_addr().ok()?.port())
}

/// A private directory under the temporary directory, removed when it goes out of scope.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new() -> Option<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!(
            "majordomus-sccache-probe-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).ok()?;
        Some(ScratchDir(dir))
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sccache_version(program: &str) -> Option<String> {
    let out = bounded_output(Command::new(program).arg("--version"), SCCACHE_BUDGET)?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Whether `program` has the multi-level chain, found out by asking its server: one is
/// started on a private port, in a private home so that no user configuration reaches it,
/// with a chain naming a level that does not exist. A sccache that has the chain refuses to
/// start and names the level; one that does not ignores the variable and starts, and that
/// throwaway server is stopped again. `None` when the question could not be put.
fn measure_multilevel(program: &str) -> (Option<bool>, String) {
    let Some(port) = private_port() else {
        return (None, "no private port to measure on".into());
    };
    let Some(home) = ScratchDir::new() else {
        return (None, "no scratch directory to measure in".into());
    };
    let mut start = Command::new(program);
    start
        .arg("--start-server")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", &home.0)
        .env("XDG_CONFIG_HOME", &home.0)
        .env("SCCACHE_DIR", home.0.join("cache"))
        .env("SCCACHE_SERVER_PORT", port.to_string())
        .env("SCCACHE_MULTILEVEL_CHAIN", "majordomus-probe");
    let Some(out) = bounded_output(&mut start, SCCACHE_BUDGET) else {
        return (None, "the throwaway server did not answer in time".into());
    };
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .to_ascii_lowercase();
    if out.status.success() {
        // it started with a chain it should have refused: it does not know the chain
        let mut stop = Command::new(program);
        stop.arg("--stop-server")
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &home.0)
            .env("SCCACHE_DIR", home.0.join("cache"))
            .env("SCCACHE_SERVER_PORT", port.to_string());
        let _ = bounded_output(&mut stop, SCCACHE_BUDGET);
        return (
            Some(false),
            "a server started with an unknown chain level: SCCACHE_MULTILEVEL_CHAIN is ignored"
                .into(),
        );
    }
    if said.contains("cache level") || said.contains("multilevel") {
        return (
            Some(true),
            "a server refused an unknown chain level by name: SCCACHE_MULTILEVEL_CHAIN is understood"
                .into(),
        );
    }
    (
        None,
        "a throwaway server failed for a reason that says nothing about the chain".into(),
    )
}

fn local_only(reason: String, sccache: Sccache) -> EnvReport {
    EnvReport {
        mode: Mode::LocalOnly,
        reason,
        sccache,
        store: None,
        credentials_file: None,
        variables: Vec::new(),
        passed_over: Vec::new(),
    }
}

/// The host triple the store's keys are prefixed with: rustc's own, else this executable's.
fn host_triple() -> String {
    let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match super::key::state_toolchain(super::key::Kind::CargoBuildDir, &dir) {
        Ok((Some(host), _)) => host,
        _ => super::key::machine_triple(),
    }
}

/// The variables for the second level, given the store, the credential file and the triple.
fn variables_for(store: &Store, credentials: &Path, triple: &str) -> Vec<EnvVar> {
    let v = |n: &str, value: String| EnvVar {
        name: n.into(),
        value,
    };
    vec![
        v("SCCACHE_MULTILEVEL_CHAIN", "disk,s3".into()),
        v("SCCACHE_MULTILEVEL_WRITE_ERROR_POLICY", "l0".into()),
        v("SCCACHE_BUCKET", store.bucket.clone()),
        v("SCCACHE_ENDPOINT", store.endpoint.clone()),
        v("SCCACHE_REGION", DEFAULT_REGION.into()),
        v(
            "SCCACHE_S3_USE_SSL",
            if store.endpoint.starts_with("https://") {
                "true"
            } else {
                "false"
            }
            .into(),
        ),
        v("SCCACHE_S3_KEY_PREFIX", format!("rust/{triple}/")),
        v(
            "AWS_SHARED_CREDENTIALS_FILE",
            credentials.display().to_string(),
        ),
        v("AWS_PROFILE", PROFILE.into()),
    ]
}

/// What the sccache client of this machine should be configured with now. Cheapest check
/// first, so the usual local-only answer costs no process but `sccache --version`.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::env::{report, EnvOptions, Mode};
/// let answer = report(&EnvOptions {
///     sccache: "/no/such/sccache".into(), node_file: None, credentials: None,
/// });
/// assert_eq!(answer.mode, Mode::LocalOnly);
/// assert!(!answer.sccache.found);
/// ```
pub fn report(opts: &EnvOptions) -> EnvReport {
    let Some(version) = sccache_version(&opts.sccache) else {
        return local_only(
            format!(
                "{} does not answer `--version`: sccache is absent, and the build is what it was",
                opts.sccache
            ),
            Sccache {
                found: false,
                version: None,
                multilevel: None,
                detail: "not found".into(),
            },
        );
    };
    let mut sccache = Sccache {
        found: true,
        version: Some(version),
        multilevel: None,
        detail: "not measured: an earlier condition already ended in local-only".into(),
    };

    let node_file = opts.node_file.clone().or_else(default_node_file);
    let file = match node_file.as_deref().map(read_node_file) {
        Some(Ok(f)) => f,
        Some(Err(FileError::Invalid(why))) => {
            return local_only(format!("the node file is unusable: {why}"), sccache)
        }
        Some(Err(FileError::Absent)) | None => NodeFile::default(),
    };
    let offers = offers_of(&file, OWN_NODE, HEALTH_BUDGET);
    let selection = select(
        &offers,
        &Policy {
            own_node: Some(OWN_NODE.into()),
            ..Policy::default()
        },
    );
    let Some(chosen) = selection.chosen.clone() else {
        let mut r = local_only(selection.reason.clone(), sccache);
        r.passed_over = selection.passed_over;
        return r;
    };
    let store = Store {
        node: chosen.node.clone(),
        provider: chosen.provider.clone(),
        endpoint: chosen.endpoint.clone(),
        bucket: chosen.bucket.clone(),
    };

    let credentials = opts.credentials.clone().or_else(default_credentials);
    let Some(credentials) = credentials else {
        let mut r = local_only(
            "this process has no home to find a credential file in".into(),
            sccache,
        );
        r.store = Some(store);
        r.passed_over = selection.passed_over;
        return r;
    };
    if let Some(problem) = credentials_problem(&credentials) {
        let mut r = local_only(
            format!("{problem}: the store is left out and the build uses its local level alone"),
            sccache,
        );
        r.store = Some(store);
        r.passed_over = selection.passed_over;
        return r;
    }

    let (multilevel, detail) = measure_multilevel(&opts.sccache);
    sccache.multilevel = multilevel;
    sccache.detail = detail;
    if multilevel != Some(true) {
        let reason = match multilevel {
            Some(false) => "the installed sccache has no multi-level chain: the build uses its local level alone",
            _ => "the installed sccache's multi-level chain could not be measured: the build uses its local level alone",
        };
        let mut r = local_only(reason.into(), sccache);
        r.store = Some(store);
        r.passed_over = selection.passed_over;
        return r;
    }

    let variables = variables_for(&store, &credentials, &host_triple());
    EnvReport {
        mode: Mode::Mesh,
        reason: format!(
            "{} is healthy now: the local disk is the first level and it the second, started with this server",
            selection.reason
        ),
        sccache,
        store: Some(store),
        credentials_file: Some(credentials.display().to_string()),
        variables,
        passed_over: selection.passed_over,
    }
}

/// The report as a POSIX script that `eval` can read: one comment line and one `export` per
/// variable, every value single-quoted. It runs nothing and assigns only the variables.
///
/// # Example
///
/// ```
/// use majordomus_cli::mesh::build_cache::env::{script, report, EnvOptions};
/// let answer = report(&EnvOptions {
///     sccache: "/no/such/sccache".into(), node_file: None, credentials: None,
/// });
/// let text = script(&answer);
/// assert!(text.starts_with("# build cache: local-only"));
/// assert!(!text.contains("export"));
/// ```
pub fn script(r: &EnvReport) -> String {
    let mode = match r.mode {
        Mode::Mesh => "mesh",
        Mode::LocalOnly => "local-only",
    };
    let mut out = format!("# build cache: {mode}: {}\n", one_line(&r.reason));
    for v in &r.variables {
        out.push_str(&format!("export {}={}\n", v.name, quote_posix(&v.value)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::os::unix::fs::PermissionsExt;

    const SECRET: &str = "wJalrXUtnFEMI-NOT-A-REAL-KEY-7MDENG";

    /// A store that answers every request 200, until the returned listener thread is
    /// dropped with the process; the port is the endpoint's.
    fn store_up() -> u16 {
        let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(mut s) = s else { break };
                let mut buf = [0u8; 512];
                let _ = s.read(&mut buf);
                let _ = s.write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 0\r\n\r\n");
            }
        });
        port
    }

    /// A port nothing listens on.
    fn store_down() -> u16 {
        private_port().unwrap()
    }

    fn write_exec(path: &Path, body: &str) {
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// An sccache that knows the chain: a server asked to start with a level it does not
    /// know refuses it by name, as 0.17 and 0.18 do. It logs the port and home it was given.
    fn capable(dir: &Path) -> String {
        let log = dir.join("capable.log");
        let path = dir.join("sccache-capable");
        write_exec(
            &path,
            &format!(
                "#!/bin/sh\ncase \"$1\" in\n--version) echo 'sccache 0.18.0';;\n\
                 --start-server) echo \"$SCCACHE_SERVER_PORT $HOME ${{SCCACHE_BUCKET:-unset}}\" >> {}; \
                 echo \"sccache: error: Server startup failed: Unknown cache level: '$SCCACHE_MULTILEVEL_CHAIN'\" >&2; exit 2;;\n\
                 esac\n",
                log.display()
            ),
        );
        path.display().to_string()
    }

    /// An sccache from before the chain: it ignores the variable and starts, and is stopped.
    fn incapable(dir: &Path) -> String {
        let stopped = dir.join("incapable.stopped");
        let path = dir.join("sccache-incapable");
        write_exec(
            &path,
            &format!(
                "#!/bin/sh\ncase \"$1\" in\n--version) echo 'sccache 0.9.0';;\n\
                 --start-server) echo 'sccache: Listening on address 127.0.0.1:1';;\n\
                 --stop-server) : > {};;\nesac\n",
                stopped.display()
            ),
        );
        path.display().to_string()
    }

    fn node_file(dir: &Path, port: u16) -> PathBuf {
        let path = dir.join("build-cache.json");
        std::fs::write(
            &path,
            format!(
                r#"{{"provider":{{"provider":"minio","scope":"host","endpoint":"http://127.0.0.1:{port}","bucket":"the-bucket","ready_path":"/ready"}}}}"#
            ),
        )
        .unwrap();
        path
    }

    fn credentials(dir: &Path, mode: u32, body: &str) -> PathBuf {
        let path = dir.join(format!("credentials-{mode:o}-{}", body.len()));
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    fn good_credentials(dir: &Path) -> PathBuf {
        credentials(
            dir,
            0o600,
            &format!(
                "[{PROFILE}]\naws_access_key_id = AKIDEXAMPLE\naws_secret_access_key = {SECRET}\n"
            ),
        )
    }

    fn opts(sccache: String, node: Option<PathBuf>, creds: Option<PathBuf>) -> EnvOptions {
        EnvOptions {
            sccache,
            node_file: node,
            credentials: creds,
        }
    }

    fn value<'a>(r: &'a EnvReport, name: &str) -> Option<&'a str> {
        r.variables
            .iter()
            .find(|v| v.name == name)
            .map(|v| v.value.as_str())
    }

    #[test]
    fn with_everything_in_place_the_store_is_the_second_level() {
        let d = tempfile::tempdir().unwrap();
        let port = store_up();
        let creds = good_credentials(d.path());
        let r = report(&opts(
            capable(d.path()),
            Some(node_file(d.path(), port)),
            Some(creds.clone()),
        ));
        assert_eq!(r.mode, Mode::Mesh, "{}", r.reason);
        assert_eq!(r.sccache.multilevel, Some(true));
        assert_eq!(value(&r, "SCCACHE_MULTILEVEL_CHAIN"), Some("disk,s3"));
        assert_eq!(value(&r, "SCCACHE_BUCKET"), Some("the-bucket"));
        assert_eq!(
            value(&r, "SCCACHE_ENDPOINT"),
            Some(format!("http://127.0.0.1:{port}").as_str())
        );
        assert_eq!(value(&r, "SCCACHE_REGION"), Some(DEFAULT_REGION));
        assert_eq!(value(&r, "SCCACHE_S3_USE_SSL"), Some("false"));
        assert_eq!(value(&r, "AWS_PROFILE"), Some(PROFILE));
        assert_eq!(
            value(&r, "AWS_SHARED_CREDENTIALS_FILE"),
            Some(creds.display().to_string().as_str())
        );
        let prefix = value(&r, "SCCACHE_S3_KEY_PREFIX").unwrap();
        assert!(
            prefix.starts_with("rust/") && prefix.ends_with('/'),
            "{prefix}"
        );
        assert!(prefix.contains("-"), "a host triple is in {prefix}");
        assert_eq!(r.store.as_ref().unwrap().bucket, "the-bucket");
    }

    #[test]
    fn no_answer_names_a_key_only_the_path_of_the_file() {
        let d = tempfile::tempdir().unwrap();
        let port = store_up();
        let r = report(&opts(
            capable(d.path()),
            Some(node_file(d.path(), port)),
            Some(good_credentials(d.path())),
        ));
        assert_eq!(r.mode, Mode::Mesh);
        let json = serde_json::to_string(&r).unwrap();
        let text = script(&r);
        for said in [json.as_str(), text.as_str()] {
            assert!(!said.contains(SECRET), "the secret is in the answer");
            assert!(!said.contains("AKIDEXAMPLE"), "the key id is in the answer");
            assert!(!said.contains("AWS_SECRET_ACCESS_KEY"));
            assert!(!said.contains("AWS_ACCESS_KEY_ID"));
        }
    }

    #[test]
    fn the_multilevel_question_is_put_to_a_private_server_with_no_ambient_store() {
        let d = tempfile::tempdir().unwrap();
        let (answer, detail) = measure_multilevel(&capable(d.path()));
        assert_eq!(answer, Some(true), "{detail}");
        let log = std::fs::read_to_string(d.path().join("capable.log")).unwrap();
        let mut f = log.split_whitespace();
        let port: u16 = f.next().unwrap().parse().unwrap();
        assert_ne!(port, 4226, "the default server's port is never used");
        let home = f.next().unwrap();
        assert!(!home.is_empty() && Some(home) != std::env::var("HOME").ok().as_deref());
        assert_eq!(
            f.next(),
            Some("unset"),
            "no ambient bucket reaches the question"
        );
    }

    #[test]
    fn an_sccache_without_the_chain_is_local_only_and_its_throwaway_server_is_stopped() {
        let d = tempfile::tempdir().unwrap();
        let port = store_up();
        let r = report(&opts(
            incapable(d.path()),
            Some(node_file(d.path(), port)),
            Some(good_credentials(d.path())),
        ));
        assert_eq!(r.mode, Mode::LocalOnly);
        assert_eq!(r.sccache.multilevel, Some(false));
        assert!(r.reason.contains("no multi-level chain"), "{}", r.reason);
        assert!(r.variables.is_empty());
        assert!(
            d.path().join("incapable.stopped").exists(),
            "the throwaway server was left running"
        );
    }

    #[test]
    fn an_sccache_that_fails_for_another_reason_is_not_taken_for_capable() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("sccache-odd");
        write_exec(
            &path,
            "#!/bin/sh\ncase \"$1\" in\n--version) echo 'sccache 1';;\n--start-server) echo 'address in use' >&2; exit 2;;\nesac\n",
        );
        let (answer, _) = measure_multilevel(&path.display().to_string());
        assert_eq!(answer, None);
        let port = store_up();
        let r = report(&opts(
            path.display().to_string(),
            Some(node_file(d.path(), port)),
            Some(good_credentials(d.path())),
        ));
        assert_eq!(r.mode, Mode::LocalOnly);
        assert!(r.reason.contains("could not be measured"), "{}", r.reason);
    }

    #[test]
    fn a_store_that_is_down_when_the_server_would_start_is_left_out() {
        let d = tempfile::tempdir().unwrap();
        let r = report(&opts(
            capable(d.path()),
            Some(node_file(d.path(), store_down())),
            Some(good_credentials(d.path())),
        ));
        assert_eq!(r.mode, Mode::LocalOnly);
        assert!(r.variables.is_empty());
        assert!(r.reason.contains("local level alone"), "{}", r.reason);
        assert!(
            r.passed_over.iter().any(|p| p.why.contains("unhealthy")),
            "the store was passed over by name: {:?}",
            r.passed_over
        );
        assert_eq!(
            r.sccache.multilevel, None,
            "no server is started for a store that is not there"
        );
    }

    #[test]
    fn the_same_store_is_in_while_it_answers_and_out_when_it_stops() {
        let d = tempfile::tempdir().unwrap();
        let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        let up = std::thread::spawn(move || {
            // answer exactly one readiness request, then stop listening
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 512];
                let _ = s.read(&mut buf);
                let _ = s.write_all(b"HTTP/1.0 200 OK\r\n\r\n");
            }
        });
        let o = opts(
            capable(d.path()),
            Some(node_file(d.path(), port)),
            Some(good_credentials(d.path())),
        );
        assert_eq!(report(&o).mode, Mode::Mesh);
        up.join().unwrap();
        assert_eq!(report(&o).mode, Mode::LocalOnly, "a stopped store is out");
    }

    #[test]
    fn a_node_that_offers_and_knows_nothing_is_local_only() {
        let d = tempfile::tempdir().unwrap();
        let r = report(&opts(
            capable(d.path()),
            Some(d.path().join("absent.json")),
            Some(good_credentials(d.path())),
        ));
        assert_eq!(r.mode, Mode::LocalOnly);
        assert!(r.reason.contains("no node offers"), "{}", r.reason);
        assert!(r.store.is_none() && r.variables.is_empty());
    }

    #[test]
    fn a_broken_node_file_says_so() {
        let d = tempfile::tempdir().unwrap();
        let bad = d.path().join("bad.json");
        std::fs::write(&bad, "{").unwrap();
        let r = report(&opts(capable(d.path()), Some(bad), None));
        assert_eq!(r.mode, Mode::LocalOnly);
        assert!(r.reason.contains("node file is unusable"), "{}", r.reason);
    }

    #[test]
    fn a_credential_that_is_missing_open_to_others_or_without_the_profile_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let port = store_up();
        let node = node_file(d.path(), port);
        let cases: [(Option<PathBuf>, &str); 4] = [
            (Some(d.path().join("none")), "no credential file"),
            (
                Some(credentials(
                    d.path(),
                    0o644,
                    &format!("[{PROFILE}]\nk = {SECRET}\n"),
                )),
                "must be 0600",
            ),
            (
                Some(credentials(
                    d.path(),
                    0o600,
                    &format!("[default]\nk = {SECRET}\n"),
                )),
                "no [mesh-build-cache] profile",
            ),
            (Some(d.path().to_path_buf()), "not a regular file"),
        ];
        for (creds, want) in cases {
            let r = report(&opts(capable(d.path()), Some(node.clone()), creds));
            assert_eq!(r.mode, Mode::LocalOnly, "{want}");
            assert!(r.reason.contains(want), "{want}: {}", r.reason);
            assert!(r.variables.is_empty());
            assert!(!r.reason.contains(SECRET) && !script(&r).contains(SECRET));
            assert_eq!(r.sccache.multilevel, None, "{want}: nothing was started");
        }
    }

    #[test]
    fn an_absent_sccache_is_local_only_and_says_the_build_is_what_it_was() {
        let r = report(&opts("/no/such/sccache".into(), None, None));
        assert_eq!(r.mode, Mode::LocalOnly);
        assert!(!r.sccache.found);
        assert!(r.reason.contains("absent"), "{}", r.reason);
    }

    #[test]
    fn tls_follows_the_scheme_of_the_endpoint() {
        let store = |endpoint: &str| Store {
            node: "n".into(),
            provider: "minio".into(),
            endpoint: endpoint.into(),
            bucket: "b".into(),
        };
        let creds = Path::new("/c");
        let plain = variables_for(&store("http://10.0.0.1:9000"), creds, "x-y");
        let tls = variables_for(&store("https://s3.example:9000"), creds, "x-y");
        let ssl = |v: &[EnvVar]| {
            v.iter()
                .find(|e| e.name == "SCCACHE_S3_USE_SSL")
                .unwrap()
                .value
                .clone()
        };
        assert_eq!(ssl(&plain), "false");
        assert_eq!(ssl(&tls), "true");
        assert_eq!(plain[6].value, "rust/x-y/");
    }

    #[test]
    fn the_script_quotes_every_value_and_keeps_a_reason_on_one_line() {
        let r = EnvReport {
            mode: Mode::Mesh,
            reason: "one\ntwo; $(touch /tmp/pwned)".into(),
            sccache: Sccache {
                found: true,
                version: None,
                multilevel: Some(true),
                detail: String::new(),
            },
            store: None,
            credentials_file: None,
            variables: vec![EnvVar {
                name: "AWS_SHARED_CREDENTIALS_FILE".into(),
                value: "/home/o'brien/$(id)/c".into(),
            }],
            passed_over: vec![],
        };
        let text = script(&r);
        let mut lines = text.lines();
        let first = lines.next().unwrap();
        assert!(
            first.starts_with("# build cache: mesh: one two;"),
            "{first}"
        );
        assert_eq!(
            lines.next().unwrap(),
            r#"export AWS_SHARED_CREDENTIALS_FILE='/home/o'\''brien/$(id)/c'"#
        );
        assert!(
            lines.next().is_none(),
            "no second line a reason could have made"
        );
    }

    #[test]
    fn the_default_files_are_under_the_config_directory() {
        if let (Some(n), Some(c)) = (default_node_file(), default_credentials()) {
            assert!(n.ends_with("majordomus/build-cache.json"));
            assert!(c.ends_with("mesh-build-cache/credentials"));
        }
    }
}
