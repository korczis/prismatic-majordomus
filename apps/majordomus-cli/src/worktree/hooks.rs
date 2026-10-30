//! Each linked worktree runs the hooks of its own tree.
//!
//! A repository that tracks its hooks (`.githooks/`, named by `core.hooksPath`) means every
//! commit to be judged by the hooks of the commit's own tree. Git does not guarantee that
//! across worktrees: `core.hooksPath` lives in the configuration every worktree shares, and
//! once it holds an absolute path — `<primary>/.githooks`, written by hand or by a tool that
//! made the value absolute — every linked worktree runs the primary checkout's hooks. When
//! the primary checkout is days behind, those hooks are days behind, and on 2026-10-14 the
//! stale `doctor` they ran refused every commit of current master in every worktree, saying
//! that a gate was "not invoked by `<primary>/.githooks/pre-commit`".
//!
//! The remedy is git's own: per-worktree configuration. With `extensions.worktreeConfig`
//! on, `git config --worktree core.hooksPath <worktree>/.githooks` binds one worktree to its
//! own directory, and nothing else moves. [`carry_hooks_path`] does that for a worktree this module
//! creates, finds or moves, and only when it is needed: the policy is read from the primary
//! checkout (its effective `core.hooksPath`), the directory it names inside the primary is
//! looked for inside the worktree, and git's own resolution (`rev-parse --git-path hooks`)
//! decides whether the worktree already runs it. A repository with no `core.hooksPath` keeps
//! git's default hooks directory and gets nothing invented; a hooks directory outside the
//! primary checkout is a shared one on purpose and is left alone; a worktree whose tree does
//! not carry the directory is left as it is, because pointing it at nothing would switch its
//! hooks off.
//!
//! Every outcome is reported and none is fatal: a worktree that exists is still a worktree,
//! and the report says, before the first commit does, which hooks it runs.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::git;
use super::identity::ResolvedPath;
use super::service::display;

/// What became of the hooks directory of a worktree that was just created, found or moved.
///
/// It travels in the JSON of `worktree create`, `ensure`, `migrate` and `repair`, tagged by
/// `outcome`, so a reader branches on one word rather than parsing the line:
///
/// ```
/// use majordomus_cli::worktree::HooksPath;
///
/// let set = HooksPath::Set {
///     path: "/r-wt/fix/x/.githooks".into(),
///     enabled: vec![],
/// };
/// let json = serde_json::to_value(&set).unwrap();
/// assert_eq!(json["outcome"], "set");
/// assert_eq!(json["path"], "/r-wt/fix/x/.githooks");
/// let back: HooksPath = serde_json::from_value(json).unwrap();
/// assert_eq!(back, set);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum HooksPath {
    /// The repository's configuration sets no `core.hooksPath`: git's own hooks directory
    /// in the common directory (or a value of the user's) serves every worktree, and there
    /// is no tracked directory to follow.
    NotConfigured,
    /// Git already resolves this worktree's hooks to its own directory; nothing was set.
    Own {
        /// The directory git runs the hooks from.
        path: String,
    },
    /// This worktree's `core.hooksPath` was set, per worktree, to its own directory.
    Set {
        /// The directory it now runs the hooks from.
        path: String,
        /// Repository settings switched on so that a per-worktree value is honoured, as
        /// `key=value`; empty when they were on already.
        enabled: Vec<String>,
    },
    /// The hooks directory is outside the primary checkout — one directory shared on
    /// purpose — and every worktree runs it; nothing was set.
    Shared {
        /// The directory every worktree runs the hooks from.
        path: String,
    },
    /// The primary checkout's hooks directory has no counterpart in this worktree's tree,
    /// so the worktree keeps running the primary's: pointing it at a directory that does
    /// not exist would switch its hooks off.
    Untracked {
        /// The directory that would be the worktree's own, and is not there.
        missing: String,
        /// The directory the worktree runs the hooks from instead.
        runs: String,
    },
    /// Git was asked and refused, or could not be run; the message is its own.
    Failed {
        /// What went wrong.
        message: String,
    },
}

impl HooksPath {
    /// One line for a person, after the path the worktree is at.
    ///
    /// ```
    /// use majordomus_cli::worktree::HooksPath;
    ///
    /// let set = HooksPath::Set {
    ///     path: "/r-wt/fix/x/.githooks".into(),
    ///     enabled: vec!["extensions.worktreeConfig=true".into()],
    /// };
    /// assert_eq!(
    ///     set.describe(),
    ///     "hooks   /r-wt/fix/x/.githooks, its own (git config --worktree core.hooksPath; \
    ///      enabled extensions.worktreeConfig=true)"
    /// );
    /// assert!(HooksPath::NotConfigured.describe().contains("no core.hooksPath"));
    /// ```
    pub fn describe(&self) -> String {
        match self {
            Self::NotConfigured => {
                "hooks   no core.hooksPath in this repository; nothing tracked to follow".into()
            }
            Self::Own { path } => format!("hooks   {path}, its own"),
            Self::Set { path, enabled } if enabled.is_empty() => {
                format!("hooks   {path}, its own (git config --worktree core.hooksPath)")
            }
            Self::Set { path, enabled } => format!(
                "hooks   {path}, its own (git config --worktree core.hooksPath; enabled {})",
                enabled.join(", ")
            ),
            Self::Shared { path } => format!("hooks   {path}, shared outside the checkouts"),
            Self::Untracked { missing, runs } => {
                format!("hooks   {runs}, the primary checkout's: {missing} is not in this tree")
            }
            Self::Failed { message } => format!("hooks   not set to its own: {message}"),
        }
    }
}

/// Bind `worktree` to the hooks directory of its own tree, when the repository tracks one
/// and git would otherwise run another worktree's.
///
/// `primary` is the primary checkout, whose effective `core.hooksPath` is the repository's
/// policy. Never fails: the worst outcome is a worktree left as git left it, reported. The
/// primary checkout itself is answered with what it runs and never changed.
///
/// ```
/// use std::process::Command;
/// use majordomus_cli::worktree::{carry_hooks_path, HooksPath};
///
/// let dir = tempfile::tempdir().unwrap();
/// let r = dir.path().join("r");
/// let wt = dir.path().join("r-wt").join("x");
/// let git = |cwd: &std::path::Path, args: &[&str]| {
///     assert!(Command::new("git").arg("-C").arg(cwd).args(args).status().unwrap().success());
/// };
/// std::fs::create_dir_all(r.join(".githooks")).unwrap();
/// std::fs::write(r.join(".githooks/pre-commit"), "#!/bin/sh\n").unwrap();
/// git(dir.path(), &["init", "-q", r.to_str().unwrap()]);
/// git(&r, &["add", "."]);
/// git(&r, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "hooks"]);
/// // The value that sent every worktree to the primary's hooks: absolute, shared.
/// let primary_hooks = r.join(".githooks");
/// git(&r, &["config", "core.hooksPath", primary_hooks.to_str().unwrap()]);
/// git(&r, &["worktree", "add", "-q", "-b", "x", wt.to_str().unwrap()]);
///
/// match carry_hooks_path(&r, &wt) {
///     HooksPath::Set { path, .. } => assert!(path.ends_with("r-wt/x/.githooks")),
///     other => panic!("expected the worktree's own hooks to be set, got {other:?}"),
/// }
/// // A second call finds it done.
/// assert!(matches!(carry_hooks_path(&r, &wt), HooksPath::Own { .. }));
/// ```
pub fn carry_hooks_path(primary: &Path, worktree: &Path) -> HooksPath {
    match carry_inner(primary, worktree) {
        Ok(outcome) => outcome,
        Err(message) => HooksPath::Failed { message },
    }
}

fn carry_inner(primary: &Path, worktree: &Path) -> Result<HooksPath, String> {
    let resolved_primary = ResolvedPath::of(primary);
    let resolved_worktree = ResolvedPath::of(worktree);
    let Some(policy) = repository_policy(primary)? else {
        return Ok(HooksPath::NotConfigured);
    };
    let runs = hooks_dir(worktree)?;
    if resolved_worktree.same_as(&resolved_primary) {
        return Ok(HooksPath::Own {
            path: display(&runs),
        });
    }
    // The tracked directory, relative to the checkout that holds it. A relative policy is
    // that already; an absolute one counts only when it names a directory of the primary
    // checkout — anything else is a hooks directory shared on purpose.
    let relative = match PathBuf::from(&policy) {
        p if p.is_relative() => p,
        p => match ResolvedPath::of(&p)
            .real
            .strip_prefix(&resolved_primary.real)
        {
            Ok(rest) if ResolvedPath::of(&p).is_inside(&resolved_primary) => rest.to_path_buf(),
            _ => {
                return Ok(HooksPath::Shared {
                    path: display(&runs),
                })
            }
        },
    };
    let own = worktree.join(&relative);
    if ResolvedPath::of(&runs).same_as(&ResolvedPath::of(&own)) {
        return Ok(HooksPath::Own {
            path: display(&own),
        });
    }
    if !own.is_dir() {
        return Ok(HooksPath::Untracked {
            missing: display(&own),
            runs: display(&runs),
        });
    }
    let enabled = enable_worktree_config(primary)?;
    let own_text = display(&own);
    git::run(
        worktree,
        &["config", "--worktree", "core.hooksPath", own_text.as_str()],
    )
    .map_err(|e| e.to_string())?;
    // Verify rather than assume: the answer is what git now resolves.
    let now = hooks_dir(worktree)?;
    if !ResolvedPath::of(&now).same_as(&ResolvedPath::of(&own)) {
        return Err(format!(
            "set core.hooksPath to {own_text} for this worktree and git still resolves {}",
            display(&now)
        ));
    }
    Ok(HooksPath::Set {
        path: own_text,
        enabled,
    })
}

/// Switch on what a per-worktree value needs, and answer what was switched on. Version 1
/// first: extensions are a version-1 repository's, and a tool that reads a version-0
/// repository is entitled to ignore them.
fn enable_worktree_config(primary: &Path) -> Result<Vec<String>, String> {
    let mut enabled = Vec::new();
    let on = config_value(
        primary,
        &[
            "config",
            "--local",
            "--type=bool",
            "--get",
            "extensions.worktreeConfig",
        ],
    )?;
    if on.as_deref() == Some("true") {
        return Ok(enabled);
    }
    // `core.worktree` in the shared file would, once per-worktree files are read, be read
    // by every worktree; git's own documentation says to move it first, and that is a
    // person's decision, not this command's.
    if let Some(value) = config_value(primary, &["config", "--local", "--get", "core.worktree"])? {
        return Err(format!(
            "core.worktree is set in the shared configuration ({value}); enabling \
             extensions.worktreeConfig would apply it to every worktree, so nothing was changed"
        ));
    }
    let version = config_value(
        primary,
        &["config", "--local", "--get", "core.repositoryformatversion"],
    )?;
    if matches!(version.as_deref(), None | Some("0")) {
        git::run(
            primary,
            &["config", "--local", "core.repositoryformatversion", "1"],
        )
        .map_err(|e| e.to_string())?;
        enabled.push("core.repositoryformatversion=1".into());
    }
    git::run(
        primary,
        &["config", "--local", "extensions.worktreeConfig", "true"],
    )
    .map_err(|e| e.to_string())?;
    enabled.push("extensions.worktreeConfig=true".into());
    Ok(enabled)
}

/// The primary checkout's effective `core.hooksPath`, when the repository sets it: its own
/// per-worktree value, else the shared one. A value from the user's or the system's
/// configuration is not the repository's policy and is answered as none.
fn repository_policy(primary: &Path) -> Result<Option<String>, String> {
    let Some(line) = config_value(
        primary,
        &["config", "--show-scope", "--get", "core.hooksPath"],
    )?
    else {
        return Ok(None);
    };
    Ok(policy_of(&line))
}

/// The repository's policy in one `--show-scope` line (`<scope>\t<value>`): the value when
/// the repository's own file or the worktree's set it, and non-empty; none otherwise.
fn policy_of(line: &str) -> Option<String> {
    match line.split_once('\t') {
        Some(("local" | "worktree", value)) if !value.is_empty() => Some(value.to_string()),
        _ => None,
    }
}

/// The hooks directory git runs for `worktree`, as an absolute path.
fn hooks_dir(worktree: &Path) -> Result<PathBuf, String> {
    let out = git::run(
        worktree,
        &["rev-parse", "--path-format=absolute", "--git-path", "hooks"],
    )
    .map_err(|e| e.to_string())?;
    Ok(PathBuf::from(out.text().map_err(|e| e.to_string())?))
}

/// A configuration value, `None` when it is not set (git's exit 1), an error otherwise.
fn config_value(cwd: &Path, args: &[&str]) -> Result<Option<String>, String> {
    let out = git::try_run(cwd, args).map_err(|e| e.to_string())?;
    match out.status {
        Some(0) => {
            let v = out.text().map_err(|e| e.to_string())?;
            Ok((!v.is_empty()).then_some(v))
        }
        Some(1) => Ok(None),
        _ => Err(format!(
            "git {} failed: {}",
            args.join(" "),
            if out.stderr.is_empty() {
                "git said nothing"
            } else {
                &out.stderr
            }
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _dir: tempfile::TempDir,
        primary: PathBuf,
        root: PathBuf,
    }

    fn git(cwd: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A repository with a tracked `.githooks/`, its hooks path set as `hooks_path` names it
    /// (`None` leaves it unset), and one linked worktree per name.
    fn fixture(hooks_path: Option<&str>) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let primary = root.join("r");
        std::fs::create_dir_all(primary.join(".githooks")).unwrap();
        std::fs::write(primary.join(".githooks/pre-commit"), "#!/bin/sh\n").unwrap();
        git(
            &root,
            &["init", "-q", "-b", "master", primary.to_str().unwrap()],
        );
        git(&primary, &["add", "."]);
        git(
            &primary,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "hooks",
            ],
        );
        if let Some(v) = hooks_path {
            let v = v.replace("{primary}", primary.to_str().unwrap());
            git(&primary, &["config", "core.hooksPath", &v]);
        }
        Fixture {
            _dir: dir,
            primary,
            root,
        }
    }

    fn add(f: &Fixture, branch: &str) -> PathBuf {
        let wt = f.root.join("r-wt").join(branch);
        git(
            &f.primary,
            &["worktree", "add", "-q", "-b", branch, wt.to_str().unwrap()],
        );
        wt
    }

    #[test]
    fn a_worktree_sent_to_the_primarys_hooks_is_bound_to_its_own_and_the_primary_is_not() {
        let f = fixture(Some("{primary}/.githooks"));
        let wt = add(&f, "x");
        let outcome = carry_hooks_path(&f.primary, &wt);
        let own = display(&wt.join(".githooks"));
        assert_eq!(
            outcome,
            HooksPath::Set {
                path: own.clone(),
                enabled: vec![
                    "core.repositoryformatversion=1".into(),
                    "extensions.worktreeConfig=true".into()
                ],
            }
        );
        assert_eq!(
            git(
                &wt,
                &["rev-parse", "--path-format=absolute", "--git-path", "hooks"]
            ),
            own
        );
        // the primary checkout still runs its own, and the shared value is untouched
        assert_eq!(
            git(
                &f.primary,
                &["rev-parse", "--path-format=absolute", "--git-path", "hooks"]
            ),
            display(&f.primary.join(".githooks"))
        );
        assert_eq!(
            git(
                &f.primary,
                &["config", "--local", "--get", "core.hooksPath"]
            ),
            display(&f.primary.join(".githooks"))
        );
        // asked again, nothing more is done; a second worktree finds the extension on
        assert_eq!(
            carry_hooks_path(&f.primary, &wt),
            HooksPath::Own { path: own }
        );
        let second = add(&f, "y");
        assert!(
            matches!(carry_hooks_path(&f.primary, &second), HooksPath::Set { ref enabled, .. } if enabled.is_empty())
        );
    }

    #[test]
    fn a_moved_worktree_is_rebound_to_the_directory_at_its_new_path() {
        let f = fixture(Some("{primary}/.githooks"));
        let wt = add(&f, "x");
        assert!(matches!(
            carry_hooks_path(&f.primary, &wt),
            HooksPath::Set { .. }
        ));
        let moved = f.root.join("r-wt").join("moved");
        git(
            &f.primary,
            &[
                "worktree",
                "move",
                wt.to_str().unwrap(),
                moved.to_str().unwrap(),
            ],
        );
        let own = display(&moved.join(".githooks"));
        assert!(
            matches!(carry_hooks_path(&f.primary, &moved), HooksPath::Set { ref path, .. } if *path == own)
        );
    }

    #[test]
    fn nothing_is_invented_set_or_shared_where_the_repository_does_not_track_its_hooks() {
        // no hooksPath at all: git's own directory, and no configuration written
        let f = fixture(None);
        let wt = add(&f, "x");
        assert_eq!(carry_hooks_path(&f.primary, &wt), HooksPath::NotConfigured);
        assert!(
            git::try_run(
                &f.primary,
                &["config", "--get", "extensions.worktreeConfig"]
            )
            .unwrap()
            .status
                != Some(0)
        );

        // a relative value already resolves per worktree: reported, not rewritten
        let f = fixture(Some(".githooks"));
        let wt = add(&f, "x");
        assert_eq!(
            carry_hooks_path(&f.primary, &wt),
            HooksPath::Own {
                path: display(&wt.join(".githooks"))
            }
        );

        // a directory outside every checkout is shared on purpose
        let f = fixture(Some("/nonexistent-shared-hooks"));
        let wt = add(&f, "x");
        assert!(matches!(
            carry_hooks_path(&f.primary, &wt),
            HooksPath::Shared { .. }
        ));

        // the primary checkout is never changed
        let f = fixture(Some("{primary}/.githooks"));
        assert!(matches!(
            carry_hooks_path(&f.primary, &f.primary),
            HooksPath::Own { .. }
        ));
    }

    #[test]
    fn a_tree_without_the_hooks_directory_keeps_the_primarys_rather_than_none() {
        let f = fixture(Some("{primary}/.githooks"));
        let wt = add(&f, "x");
        std::fs::remove_dir_all(wt.join(".githooks")).unwrap();
        assert!(matches!(
            carry_hooks_path(&f.primary, &wt),
            HooksPath::Untracked { .. }
        ));
        assert!(
            git::try_run(&wt, &["config", "--worktree", "--get", "core.hooksPath"])
                .unwrap()
                .status
                != Some(0)
        );
    }

    #[test]
    fn every_outcome_reads_as_one_line_naming_the_directory() {
        for (outcome, needle) in [
            (HooksPath::Own { path: "/a".into() }, "/a, its own"),
            (HooksPath::Shared { path: "/s".into() }, "/s, shared"),
            (
                HooksPath::Untracked {
                    missing: "/w/.githooks".into(),
                    runs: "/p/.githooks".into(),
                },
                "/p/.githooks, the primary checkout's: /w/.githooks",
            ),
            (
                HooksPath::Failed {
                    message: "no".into(),
                },
                "not set to its own: no",
            ),
        ] {
            let line = outcome.describe();
            assert!(
                line.starts_with("hooks   ") && line.contains(needle),
                "{line}"
            );
            assert!(!line.contains('\n'));
        }
    }

    #[test]
    fn a_set_outcome_names_what_it_switched_on_only_when_it_switched_something_on() {
        let quiet = HooksPath::Set {
            path: "/w/.githooks".into(),
            enabled: Vec::new(),
        };
        assert_eq!(
            quiet.describe(),
            "hooks   /w/.githooks, its own (git config --worktree core.hooksPath)"
        );
        let loud = HooksPath::Set {
            path: "/w/.githooks".into(),
            enabled: vec!["a=1".into(), "b=true".into()],
        };
        assert!(loud.describe().ends_with("; enabled a=1, b=true)"));
        assert!(HooksPath::NotConfigured
            .describe()
            .starts_with("hooks   no core.hooksPath"));
    }

    /// Switch per-worktree configuration on in the primary, as a person (or an earlier run)
    /// would have, and answer the administrative directory of `wt`.
    fn per_worktree(f: &Fixture, wt: &Path) -> PathBuf {
        git(&f.primary, &["config", "core.repositoryformatversion", "1"]);
        git(&f.primary, &["config", "extensions.worktreeConfig", "true"]);
        PathBuf::from(git(wt, &["rev-parse", "--absolute-git-dir"]))
    }

    fn failed(outcome: HooksPath) -> String {
        match outcome {
            HooksPath::Failed { message } => message,
            other => panic!("expected a failure, got {other:?}"),
        }
    }

    #[test]
    fn a_version_one_repository_is_switched_on_without_its_version_being_written() {
        let f = fixture(Some("{primary}/.githooks"));
        let wt = add(&f, "x");
        git(&f.primary, &["config", "core.repositoryformatversion", "1"]);
        assert_eq!(
            carry_hooks_path(&f.primary, &wt),
            HooksPath::Set {
                path: display(&wt.join(".githooks")),
                enabled: vec!["extensions.worktreeConfig=true".into()],
            }
        );
    }

    #[test]
    fn only_a_non_empty_value_of_the_repositorys_own_files_is_its_policy() {
        assert_eq!(
            policy_of("local\t/r/.githooks"),
            Some("/r/.githooks".into())
        );
        assert_eq!(policy_of("worktree\t.githooks"), Some(".githooks".into()));
        assert_eq!(policy_of("local\t"), None, "an empty value");
        assert_eq!(policy_of("worktree\t"), None, "an empty value");
        assert_eq!(
            policy_of("global\t/home/h"),
            None,
            "the user's, not the repository's"
        );
        assert_eq!(policy_of("command\t/c"), None);
        assert_eq!(policy_of("no tab at all"), None);
    }

    #[test]
    fn an_empty_value_is_no_policy() {
        let f = fixture(Some(""));
        let wt = add(&f, "x");
        assert_eq!(repository_policy(&f.primary), Ok(None));
        assert_eq!(carry_hooks_path(&f.primary, &wt), HooksPath::NotConfigured);
    }

    #[test]
    fn what_git_refuses_is_reported_and_nothing_is_changed() {
        let f = fixture(Some("{primary}/.githooks"));

        // a primary checkout that is not there: git's own words
        let gone = f.root.join("gone");
        let message = failed(carry_hooks_path(&gone, &gone));
        assert!(message.contains("--show-scope"), "{message}");

        // a "worktree" that is no repository: the hooks directory cannot be asked for
        let plain = f.root.join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        let message = failed(carry_hooks_path(&f.primary, &plain));
        assert!(message.contains("rev-parse"), "{message}");

        // core.worktree in the shared file: a person's decision, refused before any write
        let wt = add(&f, "x");
        git(
            &f.primary,
            &["config", "core.worktree", f.primary.to_str().unwrap()],
        );
        let message = failed(carry_hooks_path(&f.primary, &wt));
        assert!(message.contains("core.worktree is set"), "{message}");
        assert!(
            git::try_run(
                &f.primary,
                &["config", "--get", "extensions.worktreeConfig"]
            )
            .unwrap()
            .status
                != Some(0)
        );
    }

    #[test]
    fn a_value_git_will_not_overwrite_or_still_overrides_is_a_failure() {
        let f = fixture(Some("{primary}/.githooks"));

        // two values in the worktree's own file: git refuses to overwrite them
        let wt = add(&f, "x");
        let admin = per_worktree(&f, &wt);
        std::fs::write(
            admin.join("config.worktree"),
            "[core]\n\thooksPath = /a\n\thooksPath = /b\n",
        )
        .unwrap();
        let message = failed(carry_hooks_path(&f.primary, &wt));
        assert!(message.contains("core.hooksPath"), "{message}");

        // an include read after the value: set, and git still resolves another directory
        let wt = add(&f, "y");
        let admin = PathBuf::from(git(&wt, &["rev-parse", "--absolute-git-dir"]));
        let extra = f.root.join("extra.conf");
        std::fs::write(&extra, "[core]\n\thooksPath = /elsewhere\n").unwrap();
        std::fs::write(
            admin.join("config.worktree"),
            format!(
                "[core]\n\thooksPath = /a\n[include]\n\tpath = {}\n",
                extra.display()
            ),
        )
        .unwrap();
        let message = failed(carry_hooks_path(&f.primary, &wt));
        assert!(
            message.contains("git still resolves /elsewhere"),
            "{message}"
        );
    }

    #[test]
    fn a_path_git_answers_that_is_not_utf8_is_a_failure_not_a_lossy_name() {
        // the worktree resolves a directory this tool cannot name
        let f = fixture(Some("{primary}/.githooks"));
        let wt = add(&f, "x");
        let admin = per_worktree(&f, &wt);
        std::fs::write(
            admin.join("config.worktree"),
            b"[core]\n\thooksPath = /\xff\n",
        )
        .unwrap();
        let message = failed(carry_hooks_path(&f.primary, &wt));
        assert!(message.contains("UTF-8"), "{message}");

        // the policy itself cannot be named
        let f = fixture(None);
        let mut config = std::fs::read(f.primary.join(".git/config")).unwrap();
        config.extend_from_slice(b"[core]\n\thooksPath = /\xff\n");
        std::fs::write(f.primary.join(".git/config"), config).unwrap();
        let message = failed(carry_hooks_path(&f.primary, &f.primary));
        assert!(message.contains("UTF-8"), "{message}");
    }

    #[test]
    fn a_configuration_read_git_fails_without_a_word_says_so() {
        let f = fixture(None);
        // `ls-remote --exit-code` exits 2, silently, when nothing matches
        let err = config_value(&f.primary, &["ls-remote", "--exit-code", ".", "refs/none"])
            .expect_err("exit 2 is neither a value nor its absence");
        assert!(err.ends_with("failed: git said nothing"), "{err}");
    }
}
