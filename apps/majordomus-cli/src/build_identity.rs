//! Which commit an executable was built from, and whether that tree was dirty: the decision
//! `build.rs` makes once, at build time, and compiles in as `MAJORDOMUS_COMMIT` and
//! `MAJORDOMUS_DIRTY` (read back as [`crate::COMMIT`] and [`crate::DIRTY`]).
//!
//! It lives here rather than inline in the build script so that it can be tested: a build
//! script has no test harness, and the constants it emits can only be compared with
//! themselves. `build.rs` compiles this file through `#[path]`, the way it compiles
//! `generation.rs`, so there is one definition of the decision and the tests below exercise
//! the one the build runs. For the same reason nothing outside `std` may be used here, and
//! every item must be reached by the build script or it is dead code in that compilation.
//!
//! The dirty flag describes the crate's own directory (the build script asks git with the
//! pathspec `.` from `CARGO_MANIFEST_DIR`), as it stood when the build script last ran: an
//! edit to documentation elsewhere in the repository does not mark the executable dirty, and
//! a commit that touches no crate source does not rerun the build script, so the flag says
//! what the build saw and not what the tree says now.
//!
//! ```
//! use majordomus_cli::build_identity::{identify, HEAD_ARGS, STATUS_ARGS};
//!
//! // a plain `cargo build` in a checkout: git names the commit and reports a clean crate
//! let sha = "0123456789abcdef0123456789abcdef01234567";
//! let git = |args: &[&str]| match args {
//!     a if a == HEAD_ARGS => Some(format!("{sha}\n")),
//!     a if a == STATUS_ARGS => Some(String::new()),
//!     _ => None,
//! };
//! assert_eq!(identify(None, None, git), (sha.to_string(), "false"));
//!
//! // a release build hands the commit and the flag in, and git is never asked
//! let never = |args: &[&str]| -> Option<String> { panic!("git asked {args:?}") };
//! assert_eq!(identify(Some(sha), Some("0"), never), (sha.to_string(), "false"));
//!
//! // a source tarball has neither git nor a declaration: both halves are `unknown`
//! assert_eq!(identify(None, None, |_| None), ("unknown".to_string(), "unknown"));
//! ```

/// The arguments the build script hands git to learn the commit.
pub const HEAD_ARGS: &[&str] = &["rev-parse", "HEAD"];
/// The arguments the build script hands git to learn whether the crate's tree is dirty:
/// tracked changes only, under the build script's working directory (the crate), and without
/// the optional index refresh — a build is a read and must not rewrite the index of a
/// repository another session may be staging into.
pub const STATUS_ARGS: &[&str] = &[
    "--no-optional-locks",
    "status",
    "--porcelain",
    "--untracked-files=no",
    "--",
    ".",
];

/// The commit the build was handed: `MAJORDOMUS_BUILD_COMMIT` when it names one (neither
/// empty nor `unknown`), else a hosting provider's `SOURCE_VERSION` when that is a full 40-hex
/// commit (the Heroku-compatible variable Gigalixir sets to the pushed commit, ADR 0219),
/// else whatever `MAJORDOMUS_BUILD_COMMIT` held, which [`identify`] then judges exactly as
/// before. A `SOURCE_VERSION` that is not a commit (a branch name, a short sha) is ignored
/// rather than compiled in as one (I3820).
///
/// ```
/// use majordomus_cli::build_identity::declared_commit;
///
/// let sha = "0123456789abcdef0123456789abcdef01234567";
/// // a provider build hands only SOURCE_VERSION: it is the commit
/// assert_eq!(declared_commit(None, Some(sha)), Some(sha.to_string()));
/// // a branch name is not a commit and is never compiled in as one
/// assert_eq!(declared_commit(None, Some("master")), None);
/// ```
pub fn declared_commit(majordomus: Option<&str>, source_version: Option<&str>) -> Option<String> {
    let names_one = |v: &str| !v.trim().is_empty() && v.trim() != "unknown";
    if let Some(v) = majordomus.filter(|v| names_one(v)) {
        return Some(v.to_string());
    }
    let is_commit = |v: &str| v.len() == 40 && v.bytes().all(|b| b.is_ascii_hexdigit());
    source_version
        .map(str::trim)
        .filter(|v| is_commit(v))
        .map(str::to_string)
        .or_else(|| majordomus.map(str::to_string))
}

/// Decide the commit and the dirty flag from what the build was handed and what git says.
///
/// `declared_commit` is `MAJORDOMUS_BUILD_COMMIT` and `declared_dirty` is
/// `MAJORDOMUS_BUILD_DIRTY`; `git` runs git with the given arguments and returns its standard
/// output on success. The commit is the declared one when it names something (neither empty
/// nor `unknown`), else git's `HEAD`, else `unknown`. The flag is `true`/`false` when it was
/// declared (`true`/`1`, `false`/`0`); otherwise it is read from git only when git also named
/// the commit — a commit handed in from outside says nothing about the tree it came from —
/// and is `unknown` when neither said.
///
/// ```
/// use majordomus_cli::build_identity::{identify, STATUS_ARGS};
///
/// // git names the commit and finds a tracked change in the crate: the build is dirty
/// let git = |args: &[&str]| {
///     Some(if args == STATUS_ARGS { " M src/lib.rs\n" } else { "abc123\n" }.to_string())
/// };
/// assert_eq!(identify(None, None, git), ("abc123".to_string(), "true"));
///
/// // a declared commit says nothing about the tree, so its dirtiness stays `unknown`
/// let never = |args: &[&str]| -> Option<String> { panic!("git asked {args:?}") };
/// assert_eq!(identify(Some(" def456 "), None, never), ("def456".to_string(), "unknown"));
/// ```
pub fn identify(
    declared_commit: Option<&str>,
    declared_dirty: Option<&str>,
    mut git: impl FnMut(&[&str]) -> Option<String>,
) -> (String, &'static str) {
    let declared = declared_commit
        .map(str::trim)
        .filter(|c| !c.is_empty() && *c != "unknown")
        .map(str::to_string);
    let from_git = declared.is_none();
    let commit = declared
        .or_else(|| {
            git(HEAD_ARGS)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "unknown".into());
    let dirty = match declared_dirty.map(str::trim) {
        Some("true" | "1") => "true",
        Some("false" | "0") => "false",
        _ if from_git && commit != "unknown" => match git(STATUS_ARGS) {
            Some(out) if out.trim().is_empty() => "false",
            Some(_) => "true",
            None => "unknown",
        },
        _ => "unknown",
    };
    (commit, dirty)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
    const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba98";

    /// A git that answers `HEAD` with [`SHA`] and `status` with `status`, and records what
    /// it was asked.
    fn git(status: Option<&'static str>) -> impl FnMut(&[&str]) -> Option<String> {
        move |args: &[&str]| match args {
            a if a == HEAD_ARGS => Some(format!("{SHA}\n")),
            a if a == STATUS_ARGS => status.map(str::to_string),
            other => panic!("git asked something the build does not ask: {other:?}"),
        }
    }

    fn no_git(_: &[&str]) -> Option<String> {
        None
    }

    #[test]
    fn git_names_the_commit_and_a_clean_tree() {
        assert_eq!(identify(None, None, git(Some(""))), (SHA.into(), "false"));
    }

    #[test]
    fn git_names_a_dirty_tree() {
        assert_eq!(
            identify(None, None, git(Some(" M src/lib.rs\n"))),
            (SHA.into(), "true")
        );
    }

    #[test]
    fn a_status_git_cannot_answer_is_unknown() {
        assert_eq!(identify(None, None, git(None)), (SHA.into(), "unknown"));
    }

    #[test]
    fn a_declared_commit_without_a_dirty_flag_is_unknown_dirtiness() {
        // git is not asked about a tree the declared commit may not be
        let asked = |args: &[&str]| -> Option<String> { panic!("git asked {args:?}") };
        assert_eq!(
            identify(Some(OTHER), None, asked),
            (OTHER.into(), "unknown")
        );
    }

    #[test]
    fn a_declared_dirty_flag_is_taken_as_declared() {
        for (v, want) in [
            ("true", "true"),
            ("1", "true"),
            ("false", "false"),
            ("0", "false"),
        ] {
            assert_eq!(
                identify(Some(OTHER), Some(v), no_git),
                (OTHER.into(), want),
                "{v}"
            );
            assert_eq!(
                identify(None, Some(v), git(Some(" M x\n"))),
                (SHA.into(), want),
                "{v}"
            );
        }
    }

    #[test]
    fn a_declared_flag_that_is_neither_falls_back() {
        assert_eq!(
            identify(None, Some("maybe"), git(Some(""))),
            (SHA.into(), "false")
        );
        assert_eq!(
            identify(Some(OTHER), Some("unknown"), no_git),
            (OTHER.into(), "unknown")
        );
    }

    #[test]
    fn a_declared_unknown_or_empty_commit_falls_back_to_git() {
        for declared in ["unknown", "", "  "] {
            assert_eq!(
                identify(Some(declared), None, git(Some(""))),
                (SHA.into(), "false")
            );
        }
    }

    #[test]
    fn a_declared_commit_is_trimmed() {
        let padded = format!(" {OTHER}\n");
        assert_eq!(
            identify(Some(&padded), Some("0"), no_git),
            (OTHER.into(), "false")
        );
    }

    #[test]
    fn a_provider_source_version_is_the_commit_when_nothing_else_names_one() {
        assert_eq!(declared_commit(None, Some(SHA)), Some(SHA.into()));
        let padded = format!(" {SHA}\n");
        assert_eq!(declared_commit(None, Some(&padded)), Some(SHA.into()));
        // what the build then compiles in: that commit, with its dirtiness unknown
        let declared = declared_commit(None, Some(SHA));
        assert_eq!(
            identify(declared.as_deref(), None, no_git),
            (SHA.into(), "unknown")
        );
    }

    #[test]
    fn a_named_majordomus_commit_wins_over_the_provider_source_version() {
        assert_eq!(declared_commit(Some(OTHER), Some(SHA)), Some(OTHER.into()));
        // `unknown` and empty name nothing, so the provider's commit is taken
        assert_eq!(
            declared_commit(Some("unknown"), Some(SHA)),
            Some(SHA.into())
        );
        assert_eq!(declared_commit(Some(" "), Some(SHA)), Some(SHA.into()));
    }

    #[test]
    fn a_source_version_that_is_not_a_full_commit_is_ignored() {
        let not_commits = [
            "master".to_string(),
            "0123456".to_string(),
            SHA[..39].to_string(),
            format!("{SHA}0"),
            "z".repeat(40),
        ];
        for v in &not_commits {
            assert_eq!(declared_commit(None, Some(v)), None, "{v:?}");
            // what MAJORDOMUS_BUILD_COMMIT held is handed on for identify to judge as before
            assert_eq!(
                declared_commit(Some("unknown"), Some(v)),
                Some("unknown".into())
            );
        }
        assert_eq!(declared_commit(None, None), None);
    }

    #[test]
    fn no_git_and_nothing_declared_is_unknown() {
        assert_eq!(identify(None, None, no_git), ("unknown".into(), "unknown"));
        let empty = |_: &[&str]| Some("\n".to_string());
        assert_eq!(identify(None, None, empty), ("unknown".into(), "unknown"));
    }
}
