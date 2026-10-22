//! What a dependency install or a build directory is cached under: a triplet of platform,
//! toolchain and inputs, derived from the machine and the repository (I3950).
//!
//! **Derived, never typed.** The kinds a repository has are the lockfiles it tracks: a
//! `Cargo.lock` makes a cargo build directory, a `mix.lock` a set of mix dependencies, a
//! `package-lock.json` an npm `node_modules`. The platform is the toolchain's own host triple
//! where it states one (`rustc -vV`) and this machine's otherwise. The toolchain is read from
//! the executable that will run in that directory — so a `rust-toolchain.toml` or an asdf pin
//! is honoured because the executable honours it, never because this module read the pin. The
//! inputs are the files whose change changes what the install produces, hashed with their
//! paths.
//!
//! **Path-free.** The key names no directory, so two checkouts — or two repositories — with
//! the same lockfile on the same platform and toolchain share one entry, which is the reuse
//! the mesh exists for. Where the entry is restored *to* is a separate field.
//!
//! **Unknown is said.** A kind whose toolchain is absent, or does not answer within its
//! budget, has no key and says why; nothing is guessed, and the caller does the work itself.
//!
//! ```
//! use majordomus_cli::mesh::build_cache::key::{Kind, KindKey, KEY_SCHEMA};
//! // a lockfile names a kind; the kind's word and the schema open every key
//! let kind = Kind::parse("npm-modules").unwrap();
//! let key = format!("{KEY_SCHEMA}/{}/aarch64-apple-darwin/node-24-npm-11/{}", kind.as_str(), "0".repeat(32));
//! let entry = KindKey {
//!     kind, dir: "site".into(), location: "site/node_modules".into(),
//!     platform: "aarch64-apple-darwin".into(), toolchain: Some("node-24-npm-11".into()),
//!     inputs: vec!["site/package-lock.json".into()], digest: "0".repeat(32),
//!     key: Some(key), unavailable: None,
//! };
//! assert!(entry.key.unwrap().starts_with("v1/npm-modules/"));
//! ```

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::s3::hex;
use crate::environment::probe::bounded_output;

/// The schema of a key: a key derived by an executable that reads inputs differently carries
/// another version, so it is never mistaken for this one's.
pub const KEY_SCHEMA: &str = "v1";

/// The platform of what is the same on every platform.
pub(crate) const PLATFORM_ANY: &str = "any";

/// How long one toolchain executable may take to state its version.
const TOOLCHAIN_BUDGET: Duration = Duration::from_secs(10);

/// What is cached: the thing a lockfile makes reusable, named by the directory an install or
/// a build fills. The word is part of every key, so two kinds never share an entry.
///
/// ```
/// use majordomus_cli::mesh::build_cache::key::Kind;
/// assert_eq!(Kind::NpmModules.as_str(), "npm-modules");
/// assert_eq!(Kind::parse("mix-deps"), Some(Kind::MixDeps));
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    clap::ValueEnum,
)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// The cargo build directory of a workspace: the dependencies compiled on the way, without
    /// incremental state. Restored only into a directory that is cold.
    CargoBuildDir,
    /// The `deps/` directory `mix deps.get` fills.
    MixDeps,
    /// The `node_modules/` directory `npm ci` fills.
    NpmModules,
}

impl Kind {
    /// Every kind, in a stable order.
    pub const ALL: [Kind; 3] = [Kind::CargoBuildDir, Kind::MixDeps, Kind::NpmModules];

    /// The word the key, the CLI and the answer use.
    ///
    /// ```
    /// use majordomus_cli::mesh::build_cache::key::Kind;
    /// assert_eq!(Kind::CargoBuildDir.as_str(), "cargo-build-dir");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::CargoBuildDir => "cargo-build-dir",
            Kind::MixDeps => "mix-deps",
            Kind::NpmModules => "npm-modules",
        }
    }

    /// The kind a word names, if any.
    ///
    /// ```
    /// use majordomus_cli::mesh::build_cache::key::Kind;
    /// assert_eq!(Kind::parse("cargo-build-dir"), Some(Kind::CargoBuildDir));
    /// assert_eq!(Kind::parse("pip"), None);
    /// ```
    pub fn parse(word: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == word)
    }

    /// The lockfile whose presence makes a directory an instance of this kind.
    pub(crate) fn lockfile(self) -> &'static str {
        match self {
            Kind::CargoBuildDir => "Cargo.lock",
            Kind::MixDeps => "mix.lock",
            Kind::NpmModules => "package-lock.json",
        }
    }

    /// Tracked files beside the lockfile, under the instance's directory, whose change changes
    /// what the install produces.
    fn companions(self, name: &str) -> bool {
        match self {
            Kind::CargoBuildDir => name == "Cargo.toml" || name == "rust-toolchain.toml",
            Kind::MixDeps => false,
            Kind::NpmModules => name == ".npmrc",
        }
    }

    /// Environment variables whose value changes what the install produces.
    fn environment(self) -> &'static [&'static str] {
        match self {
            Kind::CargoBuildDir => &["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_INCREMENTAL"],
            Kind::MixDeps => &[],
            Kind::NpmModules => &["NODE_ENV"],
        }
    }
}

/// One cached thing of a repository, with its key or the reason it has none.
///
/// ```
/// use majordomus_cli::mesh::build_cache::key::{Kind, KindKey};
/// let k = KindKey {
///     kind: Kind::NpmModules,
///     dir: "site".into(),
///     location: "site/node_modules".into(),
///     platform: "aarch64-apple-darwin".into(),
///     toolchain: Some("node-24.8.0-npm-11".into()),
///     inputs: vec!["site/package-lock.json".into()],
///     digest: "0".repeat(32),
///     key: Some(format!("v1/npm-modules/aarch64-apple-darwin/node-24.8.0-npm-11/{}", "0".repeat(32))),
///     unavailable: None,
/// };
/// assert!(k.key.unwrap().starts_with("v1/npm-modules/"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "BuildCacheKindKey")]
pub struct KindKey {
    /// What is cached.
    pub kind: Kind,
    /// The directory holding the lockfile, relative to the repository root (`.` for the root).
    pub dir: String,
    /// Where the entry is restored to: relative to the repository, or `~/`-relative when it
    /// lies under the home directory (a shared cargo build directory).
    pub location: String,
    /// The platform triple the entry is built for.
    pub platform: String,
    /// The toolchain as its executables state it; absent when they could not be asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain: Option<String>,
    /// The files hashed into the digest, relative to the repository root.
    pub inputs: Vec<String>,
    /// The first 32 hex digits of the SHA-256 over the inputs and the declared environment.
    pub digest: String,
    /// `v1/<kind>/<platform>/<toolchain>/<digest>`; absent when the toolchain is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Why there is no key, when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<String>,
}

/// Every cached thing a repository has, as `cache.key` answers it.
///
/// ```
/// use majordomus_cli::mesh::build_cache::key::KeyReport;
/// let r = KeyReport { platform: "x86_64-unknown-linux-gnu".into(), kinds: vec![] };
/// assert!(r.kinds.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "BuildCacheKeyReport")]
pub struct KeyReport {
    /// This machine's platform triple.
    pub platform: String,
    /// One entry per lockfile the repository tracks, by kind then directory.
    pub kinds: Vec<KindKey>,
}

/// This machine's platform triple, from the target this executable was built for.
pub(crate) fn machine_triple() -> String {
    triple_of(
        std::env::consts::ARCH,
        std::env::consts::OS,
        cfg!(target_env = "musl"),
    )
}

fn triple_of(arch: &str, os: &str, musl: bool) -> String {
    match os {
        "macos" => format!("{arch}-apple-darwin"),
        "linux" if musl => format!("{arch}-unknown-linux-musl"),
        "linux" => format!("{arch}-unknown-linux-gnu"),
        other => format!("{arch}-unknown-{other}"),
    }
}

/// Ask an executable for its version, in `dir`, within the budget. `None` when it is absent,
/// fails or does not answer.
fn ask(dir: &Path, program: &str, args: &[&str]) -> Option<String> {
    let output = bounded_output(
        Command::new(program).args(args).current_dir(dir),
        TOOLCHAIN_BUDGET,
    )?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// What a toolchain states about itself: `(platform, toolchain)`, or why it could not.
pub(crate) type Stated = Result<(Option<String>, String), String>;

/// Ask the toolchain of `kind` in `dir`.
pub(crate) fn state_toolchain(kind: Kind, dir: &Path) -> Stated {
    match kind {
        Kind::CargoBuildDir => {
            let vv = ask(dir, "rustc", &["-vV"]).ok_or("rustc does not answer `rustc -vV`")?;
            parse_rustc(&vv)
        }
        Kind::MixDeps => {
            let elixir = ask(dir, "elixir", &["--short-version"])
                .ok_or("elixir does not answer `elixir --short-version`")?;
            let otp = ask(
                dir,
                "erl",
                &[
                    "-noshell",
                    "-eval",
                    "io:format(\"~s\",[erlang:system_info(otp_release)]),halt().",
                ],
            )
            .ok_or("erl does not state its OTP release")?;
            // mix dependencies are source: one fetch serves every platform
            Ok((
                Some(PLATFORM_ANY.to_string()),
                format!("elixir-{elixir}-otp-{otp}"),
            ))
        }
        Kind::NpmModules => {
            let node =
                ask(dir, "node", &["--version"]).ok_or("node does not answer `node --version`")?;
            let npm =
                ask(dir, "npm", &["--version"]).ok_or("npm does not answer `npm --version`")?;
            let major = npm.split('.').next().unwrap_or(&npm).to_string();
            Ok((
                None,
                format!("node-{}-npm-{major}", node.trim_start_matches('v')),
            ))
        }
    }
}

/// `rustc -vV` read: the host triple and `rustc-<release>-<commit10>`.
pub(crate) fn parse_rustc(vv: &str) -> Stated {
    let field = |name: &str| {
        vv.lines()
            .find_map(|l| l.strip_prefix(name)?.strip_prefix(':'))
            .map(|v| v.trim().to_string())
    };
    let host = field("host").ok_or("`rustc -vV` names no host")?;
    let release = field("release").ok_or("`rustc -vV` names no release")?;
    let commit = field("commit-hash").unwrap_or_default();
    let commit: String = commit.chars().take(10).collect();
    let toolchain = if commit.is_empty() || commit == "unknown" {
        format!("rustc-{release}")
    } else {
        format!("rustc-{release}-{commit}")
    };
    Ok((Some(host), toolchain))
}

/// The digest of the inputs: each file's repository-relative path and bytes, in path order,
/// then each declared variable's name and value.
pub(crate) fn digest(root: &Path, inputs: &[String], env: &[(String, Option<String>)]) -> String {
    let mut h = Sha256::new();
    for rel in inputs {
        let bytes = std::fs::read(root.join(rel)).unwrap_or_default();
        h.update(rel.as_bytes());
        h.update([0]);
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(&bytes);
    }
    for (name, value) in env {
        h.update(name.as_bytes());
        h.update([0]);
        h.update(value.as_deref().unwrap_or("\u{0}unset").as_bytes());
        h.update([0]);
    }
    hex(&h.finalize())[..32].to_string()
}

/// Where a cargo workspace's build directory is: `[build] build-dir` of the nearest
/// `.cargo/config.toml` at or above `dir` (inside the repository), with `{cargo-cache-home}`
/// resolved; the workspace's own `target/` when none is declared.
pub(crate) fn cargo_build_dir(root: &Path, dir: &Path, cargo_home: &Path) -> PathBuf {
    let mut at = Some(dir);
    while let Some(d) = at {
        if let Ok(text) = std::fs::read_to_string(d.join(".cargo/config.toml")) {
            if let Some(value) = build_dir_setting(&text) {
                let resolved = value.replace("{cargo-cache-home}", &cargo_home.to_string_lossy());
                let p = PathBuf::from(resolved);
                return if p.is_absolute() { p } else { d.join(p) };
            }
        }
        if d == root {
            break;
        }
        at = d.parent();
    }
    dir.join("target")
}

fn build_dir_setting(toml: &str) -> Option<String> {
    let mut in_build = false;
    for line in toml.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_build = line == "[build]";
            continue;
        }
        if in_build {
            if let Some(rest) = line.strip_prefix("build-dir") {
                let value = rest.trim_start().strip_prefix('=')?.trim();
                return Some(value.trim_matches('"').to_string());
            }
        }
    }
    None
}

/// Cargo's home: `CARGO_HOME`, else `~/.cargo`.
pub(crate) fn cargo_home(env_cargo_home: Option<&str>, home: Option<&Path>) -> PathBuf {
    match (env_cargo_home, home) {
        (Some(h), _) if !h.is_empty() => PathBuf::from(h),
        (_, Some(home)) => home.join(".cargo"),
        _ => PathBuf::from(".cargo"),
    }
}

/// A location for the answer: `~/`-relative under the home, repository-relative inside the
/// repository, absolute otherwise.
pub(crate) fn display_location(path: &Path, root: &Path, home: Option<&Path>) -> String {
    if let Ok(rel) = path.strip_prefix(root) {
        let s = rel.to_string_lossy();
        return if s.is_empty() {
            ".".into()
        } else {
            s.into_owned()
        };
    }
    if let Some(rel) = home.and_then(|h| path.strip_prefix(h).ok()) {
        return format!("~/{}", rel.to_string_lossy());
    }
    path.to_string_lossy().into_owned()
}

/// Every lockfile instance among `tracked` (repository-relative paths): `(kind, dir)`, by
/// kind then directory. A lockfile under `node_modules/` or `deps/` belongs to a dependency,
/// not to the repository, and is skipped.
pub(crate) fn instances(tracked: &[String]) -> Vec<(Kind, String)> {
    // a set: one entry per (kind, directory), in kind then byte order, with no sort site
    let mut found: BTreeSet<(Kind, String)> = BTreeSet::new();
    for path in tracked {
        if path
            .split('/')
            .any(|seg| seg == "node_modules" || seg == "deps")
        {
            continue;
        }
        let (dir, name) = match path.rsplit_once('/') {
            Some((d, n)) => (d.to_string(), n),
            None => (".".to_string(), path.as_str()),
        };
        if let Some(kind) = Kind::ALL.into_iter().find(|k| k.lockfile() == name) {
            found.insert((kind, dir));
        }
    }
    found.into_iter().collect()
}

/// The input files of one instance: its lockfile and the companions tracked under its
/// directory, plus, for cargo, the `.cargo/config.toml` and `rust-toolchain.toml` of its
/// ancestors inside the repository.
pub(crate) fn inputs_of(kind: Kind, dir: &str, tracked: &[String]) -> Vec<String> {
    let prefix = if dir == "." {
        String::new()
    } else {
        format!("{dir}/")
    };
    let mut inputs: BTreeSet<String> = tracked
        .iter()
        .filter(|p| p.starts_with(&prefix))
        .filter(|p| {
            let name = p.rsplit('/').next().unwrap_or(p);
            name == kind.lockfile() || kind.companions(name)
        })
        .filter(|p| {
            !p.split('/')
                .any(|seg| seg == "node_modules" || seg == "deps")
        })
        .cloned()
        .collect();
    if kind == Kind::CargoBuildDir {
        let mut ancestor = dir.to_string();
        loop {
            for name in [".cargo/config.toml", "rust-toolchain.toml"] {
                let p = if ancestor == "." {
                    name.to_string()
                } else {
                    format!("{ancestor}/{name}")
                };
                if tracked.contains(&p) {
                    inputs.insert(p);
                }
            }
            if ancestor == "." {
                break;
            }
            ancestor = ancestor
                .rsplit_once('/')
                .map(|(a, _)| a.to_string())
                .unwrap_or_else(|| ".".into());
        }
    }
    inputs.into_iter().collect()
}

/// The repository's tracked paths, as git's index lists them.
pub(crate) fn tracked_files(root: &Path) -> Vec<String> {
    let output = bounded_output(
        Command::new("git")
            .args(["ls-files", "-z"])
            .current_dir(root),
        Duration::from_secs(30),
    );
    output
        .filter(|o| o.status.success())
        .map(|o| {
            o.stdout
                .split(|b| *b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// Where keys are derived: the repository, what it tracks, the home and cargo's home.
pub(crate) struct Site<'a> {
    pub(crate) root: &'a Path,
    pub(crate) tracked: &'a [String],
    pub(crate) home: Option<&'a Path>,
    pub(crate) cargo_home: &'a Path,
}

/// The key of one instance, with the toolchain already stated.
pub(crate) fn key_of(
    kind: Kind,
    dir: &str,
    site: &Site<'_>,
    stated: Stated,
    env: &dyn Fn(&str) -> Option<String>,
) -> KindKey {
    let Site {
        root,
        tracked,
        home,
        cargo_home,
    } = *site;
    let inputs = inputs_of(kind, dir, tracked);
    let declared: Vec<(String, Option<String>)> = kind
        .environment()
        .iter()
        .map(|n| (n.to_string(), env(n)))
        .collect();
    let digest = digest(root, &inputs, &declared);
    let abs_dir = if dir == "." {
        root.to_path_buf()
    } else {
        root.join(dir)
    };
    let location = match kind {
        Kind::CargoBuildDir => cargo_build_dir(root, &abs_dir, cargo_home),
        Kind::MixDeps => abs_dir.join("deps"),
        Kind::NpmModules => abs_dir.join("node_modules"),
    };
    let location = display_location(&location, root, home);
    match stated {
        Ok((host, toolchain)) => {
            let platform = host.unwrap_or_else(machine_triple);
            KindKey {
                kind,
                dir: dir.to_string(),
                location,
                key: Some(format!(
                    "{KEY_SCHEMA}/{}/{platform}/{toolchain}/{digest}",
                    kind.as_str()
                )),
                platform,
                toolchain: Some(toolchain),
                inputs,
                digest,
                unavailable: None,
            }
        }
        Err(why) => KindKey {
            kind,
            dir: dir.to_string(),
            location,
            platform: machine_triple(),
            toolchain: None,
            inputs,
            digest,
            key: None,
            unavailable: Some(why),
        },
    }
}

/// Every key of the repository at `root`, the toolchains asked in each instance's directory;
/// only `only` when it is given.
pub(crate) fn report(root: &Path, only: Option<Kind>) -> KeyReport {
    let tracked = tracked_files(root);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let cargo_home = cargo_home(std::env::var("CARGO_HOME").ok().as_deref(), home.as_deref());
    let env = |name: &str| std::env::var(name).ok();
    let site = Site {
        root,
        tracked: &tracked,
        home: home.as_deref(),
        cargo_home: &cargo_home,
    };
    let kinds = instances(&tracked)
        .into_iter()
        .filter(|(k, _)| only.is_none_or(|o| o == *k))
        .map(|(kind, dir)| {
            let abs = if dir == "." {
                root.to_path_buf()
            } else {
                root.join(&dir)
            };
            let stated = state_toolchain(kind, &abs);
            key_of(kind, &dir, &site, stated, &env)
        })
        .collect();
    KeyReport {
        platform: machine_triple(),
        kinds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracked(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| p.to_string()).collect()
    }

    fn fixture() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mj-key-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("site")).unwrap();
        std::fs::create_dir_all(dir.join(".cargo")).unwrap();
        std::fs::write(dir.join("Cargo.lock"), "lock-1").unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[workspace]").unwrap();
        std::fs::write(
            dir.join(".cargo/config.toml"),
            "[env]\nX = 1\n[build]\nbuild-dir = \"{cargo-cache-home}/build-shared/majordomus\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("site/package-lock.json"), "{}").unwrap();
        dir
    }

    #[test]
    fn kinds_are_words() {
        for k in Kind::ALL {
            assert_eq!(Kind::parse(k.as_str()), Some(k));
        }
        assert_eq!(Kind::parse("x"), None);
        assert!(!Kind::MixDeps.companions("mix.exs"));
        assert!(Kind::NpmModules.companions(".npmrc"));
        assert!(Kind::MixDeps.environment().is_empty());
    }

    #[test]
    fn triples_follow_the_target() {
        assert_eq!(triple_of("aarch64", "macos", false), "aarch64-apple-darwin");
        assert_eq!(
            triple_of("x86_64", "linux", false),
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            triple_of("aarch64", "linux", true),
            "aarch64-unknown-linux-musl"
        );
        assert_eq!(
            triple_of("x86_64", "freebsd", false),
            "x86_64-unknown-freebsd"
        );
        assert!(!machine_triple().is_empty());
    }

    #[test]
    fn rustc_states_host_and_release() {
        let vv = "rustc 1.98.1 (abc 2026-09-01)\nbinary: rustc\ncommit-hash: 0123456789abcdef\nhost: aarch64-apple-darwin\nrelease: 1.98.1\n";
        assert_eq!(
            parse_rustc(vv),
            Ok((
                Some("aarch64-apple-darwin".into()),
                "rustc-1.98.1-0123456789".into()
            ))
        );
        let vv = "host: x\nrelease: 1.0.0\ncommit-hash: unknown\n";
        assert_eq!(
            parse_rustc(vv),
            Ok((Some("x".into()), "rustc-1.0.0".into()))
        );
        assert!(parse_rustc("release: 1\n").unwrap_err().contains("no host"));
        assert!(parse_rustc("host: x\n").unwrap_err().contains("no release"));
    }

    #[test]
    fn lockfiles_make_instances_and_dependencies_do_not() {
        let t = tracked(&[
            "Cargo.lock",
            "site/package-lock.json",
            "site/node_modules/x/package-lock.json",
            "app/mix.lock",
            "app/deps/y/mix.lock",
            "README.md",
        ]);
        assert_eq!(
            instances(&t),
            vec![
                (Kind::CargoBuildDir, ".".to_string()),
                (Kind::MixDeps, "app".to_string()),
                (Kind::NpmModules, "site".to_string()),
            ]
        );
    }

    #[test]
    fn inputs_are_the_lockfile_its_companions_and_cargos_ancestors() {
        let t = tracked(&[
            "Cargo.lock",
            "Cargo.toml",
            "apps/cli/Cargo.toml",
            "apps/cli/src/main.rs",
            ".cargo/config.toml",
            "rust-toolchain.toml",
            "site/package-lock.json",
            "site/.npmrc",
            "site/package.json",
        ]);
        assert_eq!(
            inputs_of(Kind::CargoBuildDir, ".", &t),
            tracked(&[
                ".cargo/config.toml",
                "Cargo.lock",
                "Cargo.toml",
                "apps/cli/Cargo.toml",
                "rust-toolchain.toml"
            ])
        );
        assert_eq!(
            inputs_of(Kind::NpmModules, "site", &t),
            tracked(&["site/.npmrc", "site/package-lock.json"])
        );
        let nested = tracked(&["w/Cargo.lock", "w/Cargo.toml", ".cargo/config.toml"]);
        assert_eq!(
            inputs_of(Kind::CargoBuildDir, "w", &nested),
            tracked(&[".cargo/config.toml", "w/Cargo.lock", "w/Cargo.toml"])
        );
    }

    #[test]
    fn the_digest_moves_with_an_input_and_with_nothing_else() {
        let root = fixture();
        let inputs = tracked(&["Cargo.lock"]);
        let env = vec![("RUSTFLAGS".to_string(), None)];
        let a = digest(&root, &inputs, &env);
        assert_eq!(a.len(), 32);
        std::fs::write(root.join("unrelated"), "x").unwrap();
        assert_eq!(
            digest(&root, &inputs, &env),
            a,
            "an unrelated file moves nothing"
        );
        std::fs::write(root.join("Cargo.lock"), "lock-2").unwrap();
        let b = digest(&root, &inputs, &env);
        assert_ne!(a, b, "a changed lockfile moves the digest");
        let flagged = vec![("RUSTFLAGS".to_string(), Some("-C lto".to_string()))];
        assert_ne!(
            digest(&root, &inputs, &flagged),
            b,
            "a declared variable moves it"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_build_dir_is_read_from_cargo_config() {
        let root = fixture();
        let home = Path::new("/home/u");
        let cargo_home = cargo_home(None, Some(home));
        assert_eq!(cargo_home, PathBuf::from("/home/u/.cargo"));
        assert_eq!(
            cargo_build_dir(&root, &root, &cargo_home),
            PathBuf::from("/home/u/.cargo/build-shared/majordomus")
        );
        assert_eq!(
            super::cargo_home(Some("/c"), Some(home)),
            PathBuf::from("/c")
        );
        assert_eq!(super::cargo_home(None, None), PathBuf::from(".cargo"));
        let site = root.join("site");
        std::fs::create_dir_all(site.join(".cargo")).unwrap();
        std::fs::write(
            site.join(".cargo/config.toml"),
            "[build]\nbuild-dir = \"out\"\n",
        )
        .unwrap();
        assert_eq!(cargo_build_dir(&root, &site, &cargo_home), site.join("out"));
        std::fs::write(root.join(".cargo/config.toml"), "[build]\njobs = 2\n").unwrap();
        assert_eq!(
            cargo_build_dir(&root, &root, &cargo_home),
            root.join("target")
        );
        assert_eq!(build_dir_setting("[build]\nbuild-dir 3"), None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn locations_are_relative_where_they_can_be() {
        let root = Path::new("/r");
        let home = Some(Path::new("/h"));
        assert_eq!(
            display_location(Path::new("/r/site/node_modules"), root, home),
            "site/node_modules"
        );
        assert_eq!(display_location(Path::new("/r"), root, home), ".");
        assert_eq!(
            display_location(Path::new("/h/.cargo/b"), root, home),
            "~/.cargo/b"
        );
        assert_eq!(display_location(Path::new("/x"), root, home), "/x");
    }

    #[test]
    fn a_key_is_path_free_and_an_absent_toolchain_has_none() {
        let root = fixture();
        let t = tracked(&[
            "Cargo.lock",
            "Cargo.toml",
            ".cargo/config.toml",
            "site/package-lock.json",
        ]);
        let env = |_: &str| None;
        let ch = PathBuf::from("/home/u/.cargo");
        let site = Site {
            root: &root,
            tracked: &t,
            home: Some(Path::new("/home/u")),
            cargo_home: &ch,
        };
        let stated = Ok((
            Some("aarch64-apple-darwin".to_string()),
            "rustc-1.98.1".to_string(),
        ));
        let k = key_of(Kind::CargoBuildDir, ".", &site, stated, &env);
        let key = k.key.clone().unwrap();
        assert!(key.starts_with("v1/cargo-build-dir/aarch64-apple-darwin/rustc-1.98.1/"));
        assert!(!key.contains(&*root.to_string_lossy()));
        assert_eq!(k.location, "~/.cargo/build-shared/majordomus");

        let n = key_of(
            Kind::NpmModules,
            "site",
            &site,
            Err("node is absent".into()),
            &env,
        );
        assert_eq!(n.key, None);
        assert_eq!(n.toolchain, None);
        assert_eq!(n.unavailable.as_deref(), Some("node is absent"));
        assert_eq!(n.location, "site/node_modules");
        let m = key_of(
            Kind::MixDeps,
            ".",
            &site,
            Ok((None, "elixir-1-otp-2".into())),
            &env,
        );
        assert_eq!(
            m.platform,
            machine_triple(),
            "a toolchain that states no platform gets the machine's"
        );
        assert_eq!(m.location, "deps");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_toolchain_that_is_absent_is_said() {
        let dir = std::env::temp_dir();
        assert_eq!(ask(&dir, "mj-no-such-program-xyz", &["--version"]), None);
        // whichever of these the test host has, each answers or says why it did not
        for kind in Kind::ALL {
            match state_toolchain(kind, &dir) {
                Ok((_, t)) => assert!(!t.is_empty()),
                Err(why) => assert!(why.contains("does not")),
            }
        }
    }

    #[test]
    fn the_report_reads_a_real_repository() {
        let root = fixture();
        let git = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(&root)
                .output()
                .unwrap()
        };
        git(&["init", "-q"]);
        git(&["add", "-A"]);
        let r = report(&root, Some(Kind::NpmModules));
        assert_eq!(r.kinds.len(), 1);
        assert_eq!(r.kinds[0].dir, "site");
        assert_eq!(report(&root, None).kinds.len(), 2);
        std::fs::remove_dir_all(root).unwrap();
    }
}
