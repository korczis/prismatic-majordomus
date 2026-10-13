//! The forge's workflow runs, asked through the forge's own tool.
//!
//! Three questions, each one call of `gh` in the repository's directory, bounded in time,
//! with the caller's credential: the trunk's recent runs, one run with its jobs, and the log
//! of one run's failed steps. They go through the runner the monitor's GitHub source
//! already has (`crate::monitor::github`), so there is one way this executable starts `gh`
//! and one way a missing or refusing `gh` is said.
//!
//! Every function answers `Err(reason)` when the forge could not be asked. That is a state
//! with its reason, never an empty list: a caller that reads "no failed runs" from a forge
//! nobody reached would be reporting a green trunk it did not see.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::monitor::github::{capture, json_within};

use super::{owes, ForgeRun};

/// The forge's command-line tool.
pub(crate) const GH: &str = "gh";

/// The fields of a run every question here asks for.
pub(crate) const RUN_FIELDS: &str =
    "databaseId,workflowName,conclusion,headSha,createdAt,updatedAt,url,headBranch";

/// How long a list of runs or one run may take.
const BUDGET: Duration = Duration::from_secs(20);

/// How long the log of a run's failed steps may take: it is the largest answer asked for.
const LOG_BUDGET: Duration = Duration::from_secs(90);

/// Whether `id` is a run identifier: digits and nothing else, so that it can never be read
/// as an option of the tool it is handed to.
pub(crate) fn is_run_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 20 && id.bytes().all(|b| b.is_ascii_digit())
}

fn refuse_unless_run_id(id: &str) -> Result<(), String> {
    if is_run_id(id) {
        Ok(())
    } else {
        Err(format!("`{id}` is not a run identifier: digits only"))
    }
}

/// The newest `limit` runs of `branch`, as the forge lists them.
pub(crate) fn runs_with(
    program: &str,
    root: &Path,
    branch: &str,
    limit: u64,
) -> Result<Vec<ForgeRun>, String> {
    if branch.is_empty() || branch.starts_with('-') {
        return Err(format!("`{branch}` is not a branch"));
    }
    let printed = json_within(
        program,
        root,
        &[
            "run",
            "list",
            "--branch",
            branch,
            "--limit",
            &limit.to_string(),
            "--json",
            RUN_FIELDS,
        ],
        BUDGET,
    )
    .map_err(|(_, reason)| reason)?;
    Ok(ForgeRun::list_from_forge(&printed))
}

/// One run and the names of its jobs that failed.
pub(crate) fn run_with(
    program: &str,
    root: &Path,
    id: &str,
) -> Result<(ForgeRun, Vec<String>), String> {
    refuse_unless_run_id(id)?;
    let fields = format!("{RUN_FIELDS},jobs");
    let printed = json_within(
        program,
        root,
        &["run", "view", id, "--json", &fields],
        BUDGET,
    )
    .map_err(|(_, reason)| reason)?;
    let run =
        ForgeRun::from_forge(&printed).ok_or_else(|| format!("gh run view {id} named no run"))?;
    Ok((run, failed_jobs(&printed)))
}

/// The names of the jobs of a run that failed, in the order the forge lists them.
pub(crate) fn failed_jobs(run: &Value) -> Vec<String> {
    run["jobs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|job| owes(job["conclusion"].as_str().unwrap_or_default()))
        .filter_map(|job| job["name"].as_str())
        .map(str::to_string)
        .collect()
}

/// The log of the failed steps of one run, as text.
pub(crate) fn failed_log_with(program: &str, root: &Path, id: &str) -> Result<String, String> {
    refuse_unless_run_id(id)?;
    capture(
        program,
        root,
        &["run", "view", id, "--log-failed"],
        LOG_BUDGET,
    )
    .map_err(|(_, reason)| reason)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    /// A program standing in for `gh`: `run list` prints `list.json` beside it, `run view
    /// <id> --json` prints `view.json`, `run view <id> --log-failed` prints `failed.log`, and
    /// every call's arguments are appended to `calls`. A file that is absent fails the call
    /// the way an unauthenticated `gh` does.
    #[cfg(unix)]
    pub(crate) fn fake_gh(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let program = dir.join("fake-gh");
        std::fs::write(
            &program,
            concat!(
                "#!/bin/sh\n",
                "here=\"$(dirname \"$0\")\"\n",
                "echo \"$*\" >> \"$here/calls\"\n",
                "answer() { [ -e \"$here/$1\" ] || { echo 'gh: To get started with GitHub CLI, please run:  gh auth login' >&2; exit 4; }; cat \"$here/$1\"; }\n",
                "case \"$1 $2 $4\" in\n",
                "  'run list '*) answer list.json ;;\n",
                "  'run view --json') answer view.json ;;\n",
                "  'run view --log-failed') answer failed.log ;;\n",
                "  *) echo \"gh: unknown command $*\" >&2; exit 1 ;;\n",
                "esac\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        program
    }

    #[test]
    #[cfg(unix)]
    fn the_trunk_s_runs_one_run_and_its_log_are_asked_through_the_tool() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_gh(dir.path());
        let program = program.to_str().unwrap();
        std::fs::write(
            dir.path().join("list.json"),
            json!([
                {"databaseId": 11, "workflowName": "validate", "conclusion": "failure",
                 "headSha": "abcdef1", "createdAt": "2026-10-13T05:00:00Z",
                 "updatedAt": "2026-10-13T05:30:00Z", "headBranch": "master",
                 "url": "https://forge.example/o/r/actions/runs/11"},
                {"databaseId": 12, "workflowName": "pages", "conclusion": "success",
                 "headSha": "abcdef2", "createdAt": "2026-10-13T06:00:00Z",
                 "updatedAt": "2026-10-13T06:10:00Z", "headBranch": "master", "url": ""}
            ])
            .to_string(),
        )
        .unwrap();
        let runs = runs_with(program, dir.path(), "master", 60).unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(
            (runs[0].id.as_str(), runs[0].conclusion.as_str()),
            ("11", "failure")
        );
        let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
        assert_eq!(
            calls.trim(),
            format!("run list --branch master --limit 60 --json {RUN_FIELDS}")
        );

        std::fs::write(
            dir.path().join("view.json"),
            json!({"databaseId": 11, "workflowName": "validate", "conclusion": "failure",
            "headSha": "abcdef1", "createdAt": "2026-10-13T05:00:00Z",
            "updatedAt": "2026-10-13T05:30:00Z", "headBranch": "master", "url": "u",
            "jobs": [
                {"name": "structure", "conclusion": "success"},
                {"name": "rust (3)", "conclusion": "failure"},
                {"name": "bench", "conclusion": "timed_out"},
                {"name": "deploy", "conclusion": "skipped"},
                {"conclusion": "failure"}
            ]})
            .to_string(),
        )
        .unwrap();
        let (run, jobs) = run_with(program, dir.path(), "11").unwrap();
        assert_eq!(run.id, "11");
        assert_eq!(run.commit, "abcdef1");
        assert_eq!(jobs, ["rust (3)", "bench"]);

        // a log larger than a pipe holds is read whole
        let line = "rust (3)\tcargo test\terror: the lease was lost\n";
        let log = line.repeat(4000);
        assert!(log.len() > 128 * 1024);
        std::fs::write(dir.path().join("failed.log"), &log).unwrap();
        assert_eq!(failed_log_with(program, dir.path(), "11").unwrap(), log);
    }

    #[test]
    #[cfg(unix)]
    fn a_forge_that_cannot_be_asked_is_a_reason_and_never_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_gh(dir.path());
        let program = program.to_str().unwrap();
        // no credential: the tool says so, and so does the answer
        let refused = runs_with(program, dir.path(), "master", 5).unwrap_err();
        assert!(
            refused.starts_with("gh run failed: gh: To get started"),
            "{refused}"
        );
        assert!(run_with(program, dir.path(), "11").is_err());
        assert!(failed_log_with(program, dir.path(), "11").is_err());
        // no tool at all
        let absent = runs_with("majordomus-no-such-gh-binary", dir.path(), "master", 5);
        assert!(absent.unwrap_err().contains("could not be started"));
        // a run the tool printed without an identifier is no run
        std::fs::write(dir.path().join("view.json"), "{}").unwrap();
        let unnamed = run_with(program, dir.path(), "11").unwrap_err();
        assert_eq!(unnamed, "gh run view 11 named no run");
    }

    #[test]
    fn what_is_not_a_run_or_a_branch_never_reaches_the_tool() {
        let dir = std::env::temp_dir();
        for bad in ["", "--web", "12a", "1 2", "123456789012345678901"] {
            assert!(!is_run_id(bad), "{bad}");
            let refused = run_with("majordomus-no-such-gh-binary", &dir, bad).unwrap_err();
            assert!(refused.contains("is not a run identifier"), "{refused}");
            assert!(failed_log_with("majordomus-no-such-gh-binary", &dir, bad)
                .unwrap_err()
                .contains("is not a run identifier"));
        }
        assert!(is_run_id("38359869013"));
        for bad in ["", "--repo=x"] {
            let refused = runs_with("majordomus-no-such-gh-binary", &dir, bad, 1).unwrap_err();
            assert!(refused.contains("is not a branch"), "{refused}");
        }
        assert!(failed_jobs(&json!({"jobs": "none"})).is_empty());
    }
}
