//! The proof that the executor's non-mutating cycle moves nothing (`majordomus prs
//! prove-dry-run`).
//!
//! The cycle is the four reads an operator runs before trusting the executor: observe the
//! forge ([`super::refresh`]), build the plan ([`super::queue_of`]), drain without acting
//! ([`super::drain::drain`] with `dry_run`) and list what cleanup would close
//! ([`super::drain::cleanup`] without `apply`). Around it, everything the cycle could move if
//! it were wrong is snapshotted, section by section:
//!
//! ```text
//! remote  every ref origin serves                         git ls-remote origin
//! forge   every open pull request: number, head, state and labels   gh pr list
//! trail   the integration audit trail                     .ai/local/state/integration/events.jsonl
//! lease   the executor's lease files                      <git-common-dir>/majordomus/locks/
//! local   every local ref outside the two mirrored namespaces
//! ```
//!
//! The two snapshots must be equal. The refresh fetches into `refs/remotes/origin/<base>`
//! and `refs/majordomus/prs/<n>` by design, so those are not compared but checked: after the
//! cycle each must equal what origin serves. The observation, relation and summary caches
//! are rewritten by every read and are not part of the proof. Nothing here takes a flag
//! that could merge, close or push: the cycle is fixed.

use std::path::{Path, PathBuf};
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::drain::{self, ForgeIntegrator, IntegrationLease};

/// One section of a snapshot: a name and its lines, in a stable order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotSection {
    /// `remote`, `forge`, `trail`, `lease` or `local`.
    pub name: String,
    /// What the section holds, one fact per line, sorted where order carries no meaning.
    pub lines: Vec<String>,
}

/// Everything the cycle could move, at one moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Snapshot {
    /// The sections, always in the order `remote`, `forge`, `trail`, `lease`, `local`.
    pub sections: Vec<SnapshotSection>,
}

/// One line that differs between the two snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Moved {
    /// The section it belongs to.
    pub section: String,
    /// `added` (present only after the cycle) or `removed` (present only before).
    pub change: String,
    /// The line itself.
    pub line: String,
}

/// One step of the cycle and what it said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProofStep {
    /// `refresh`, `plan`, `drain --dry-run` or `cleanup`.
    pub step: String,
    /// What it reported, in one line.
    pub summary: String,
}

/// One open pull request as the cycle classified it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObservedClassification {
    /// The pull request.
    pub number: u64,
    /// The head it was decided against.
    pub head_sha: String,
    /// Its disposition, as the queue names it.
    pub disposition: String,
    /// What the executor would do with it, when anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
}

/// The verdict: what was compared, what moved, and what the cycle observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DryRunProof {
    /// True exactly when nothing moved and every mirror equals what origin serves.
    pub ok: bool,
    /// The base branch the executor works on.
    pub base: String,
    /// The cycle, step by step.
    pub steps: Vec<ProofStep>,
    /// Every line that differs between the snapshots; empty when nothing moved.
    pub moved: Vec<Moved>,
    /// Every mirrored ref that is not what origin serves; empty when they all are.
    pub mirrors: Vec<String>,
    /// The snapshot before the cycle.
    pub before: Snapshot,
    /// The snapshot after it.
    pub after: Snapshot,
    /// Every open pull request as the cycle classified it, in rank order.
    pub classification: Vec<ObservedClassification>,
}

fn run(root: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|e| format!("{program} could not run: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{program} {} exited {}: {}",
            args.join(" "),
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    run(root, "git", args)
}

fn common_dir(root: &Path) -> Result<PathBuf, String> {
    Ok(PathBuf::from(
        git(
            root,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?
        .trim(),
    ))
}

/// One `<sha> <ref>` line of `git ls-remote` or `for-each-ref` output.
struct RefLine {
    sha: String,
    reference: String,
}

/// A listing is ordered by its ref names, which git keeps unique within one listing.
impl crate::order::Ordered for RefLine {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.reference, &self.reference)
    }
}

/// The `<sha> <ref>` lines of `git ls-remote` or `for-each-ref` output, in canonical order.
fn ref_lines(text: &str) -> Vec<RefLine> {
    let mut refs: Vec<RefLine> = text
        .lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            Some(RefLine {
                sha: parts.next()?.to_string(),
                reference: parts.next()?.to_string(),
            })
        })
        .collect();
    crate::order::canonical(&mut refs);
    refs
}

/// Whether a local ref is one the refresh mirrors from origin, and so is checked rather
/// than compared.
fn mirrored(reference: &str, base: &str) -> bool {
    reference.starts_with("refs/majordomus/prs/")
        || reference == format!("refs/remotes/origin/{base}")
}

#[derive(Deserialize)]
struct ListedPullRequest {
    number: u64,
    #[serde(rename = "headRefOid", default)]
    head: String,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    labels: Vec<ListedLabel>,
}

#[derive(Deserialize)]
struct ListedLabel {
    name: String,
}

fn forge_lines(json: &str) -> Result<Vec<String>, String> {
    let prs: Vec<ListedPullRequest> = serde_json::from_str(json)
        .map_err(|e| format!("gh pr list did not answer a list of pull requests: {e}"))?;
    // in canonical order once rendered: `#9` before `#10`, by number
    let mut lines: Vec<String> = prs
        .into_iter()
        .map(|p| {
            let mut labels: Vec<String> = p.labels.into_iter().map(|l| l.name).collect();
            crate::order::canonical_strings(&mut labels);
            format!(
                "#{} {} {} [{}]",
                p.number,
                p.head,
                p.state.as_deref().unwrap_or("-"),
                labels.join(",")
            )
        })
        .collect();
    crate::order::canonical_strings(&mut lines);
    Ok(lines)
}

/// The trail as the proof compares it: its acts — every line but an `observed` one — counted
/// and hashed. A read records what it observed, and the cycle reads, so `observed` lines are
/// expected to grow; any other line the cycle adds is an act, and the proof must see it.
fn trail_acts(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let acts: Vec<&str> = text
        .lines()
        .filter(|l| {
            serde_json::from_str::<serde_json::Value>(l)
                .ok()
                .and_then(|v| v.get("action").and_then(|a| a.as_str()).map(str::to_string))
                .as_deref()
                != Some("observed")
        })
        .collect();
    vec![
        format!("acts {}", acts.len()),
        format!(
            "sha256 {}",
            crate::policy::sha256_bytes_hex(acts.join("\n").as_bytes())
        ),
    ]
}

/// What origin serves and what this clone holds, read together: `git ls-remote origin` and
/// `git for-each-ref`. One read with one failure, so a caller that cannot reach origin is
/// refused, and nothing that reads them can be half-informed.
fn refs(root: &Path) -> Result<(String, String), String> {
    git(root, &["ls-remote", "origin"]).and_then(|remote| {
        git(root, &["for-each-ref", "--format=%(objectname) %(refname)"])
            .map(|local| (remote, local))
    })
}

/// Take a snapshot of everything the cycle could move. `common` is the repository's common
/// git directory, where its trail and its lease live.
pub fn snapshot(root: &Path, common: &Path, base: &str) -> Result<Snapshot, String> {
    let (remote, local) = refs(root)?;
    let remote: Vec<String> = ref_lines(&remote)
        .into_iter()
        .map(|r| format!("{} {}", r.sha, r.reference))
        .collect();
    let forge = forge_lines(&run(
        root,
        "gh",
        &[
            "pr",
            "list",
            "--state",
            "open",
            "--limit",
            "1000",
            "--json",
            "number,headRefOid,state,labels",
        ],
    )?)?;
    // the repository's trail, where every worktree writes it — read where it is, never
    // through events_path, which would move an old checkout's trail and be a write itself
    let trail_path = common
        .join(super::COMMON_STATE_DIR)
        .join(super::EVENTS_FILE);
    let trail = match std::fs::read(&trail_path) {
        Ok(bytes) => trail_acts(&bytes),
        // no trail is a trail of no acts: a cycle that only observes may create one
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => trail_acts(b""),
        Err(e) => return Err(format!("{}: {e}", trail_path.display())),
    };
    let lock = IntegrationLease::path_for(common, base);
    let lock_dir = lock.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut lease: Vec<String> = match std::fs::read_dir(&lock_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("integration-"))
            .map(|e| {
                let digest = std::fs::read(e.path())
                    .map(|b| crate::policy::sha256_bytes_hex(&b))
                    .unwrap_or_else(|_| "unreadable".into());
                format!("{} {digest}", e.file_name().to_string_lossy())
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    crate::order::canonical_strings(&mut lease);
    if lease.is_empty() {
        lease.push("absent".into());
    }
    let local: Vec<String> = ref_lines(&local)
        .into_iter()
        .filter(|r| !mirrored(&r.reference, base))
        .map(|r| format!("{} {}", r.sha, r.reference))
        .collect();
    let section = |name: &str, lines: Vec<String>| SnapshotSection {
        name: name.into(),
        lines,
    };
    Ok(Snapshot {
        sections: vec![
            section("remote", remote),
            section("forge", forge),
            section("trail", trail),
            section("lease", lease),
            section("local", local),
        ],
    })
}

/// Every line in one snapshot and not the other, section by section, in section order:
/// a line present only before is `removed`, one present only after is `added`.
pub fn diff(before: &Snapshot, after: &Snapshot) -> Vec<Moved> {
    let mut moved = Vec::new();
    let names: Vec<&str> = before
        .sections
        .iter()
        .chain(after.sections.iter())
        .map(|s| s.name.as_str())
        .fold(Vec::new(), |mut acc, n| {
            if !acc.contains(&n) {
                acc.push(n);
            }
            acc
        });
    let lines_of = |s: &Snapshot, name: &str| -> Vec<String> {
        s.sections
            .iter()
            .find(|x| x.name == name)
            .map(|x| x.lines.clone())
            .unwrap_or_default()
    };
    for name in names {
        let (b, a) = (lines_of(before, name), lines_of(after, name));
        for line in b.iter().filter(|l| !a.contains(l)) {
            moved.push(Moved {
                section: name.into(),
                change: "removed".into(),
                line: line.clone(),
            });
        }
        for line in a.iter().filter(|l| !b.contains(l)) {
            moved.push(Moved {
                section: name.into(),
                change: "added".into(),
                line: line.clone(),
            });
        }
    }
    moved
}

/// Every mirrored local ref that is not what origin serves: `refs/remotes/origin/<base>`
/// against `refs/heads/<base>`, and each `refs/majordomus/prs/<n>` the refresh mirrored —
/// `observed`, the open pull requests and the successors it read — against
/// `refs/pull/<n>/head`. A mirror where origin serves nothing is as wrong as a stale one. A
/// mirror of a pull request the refresh did not observe is no mirror of this refresh: it is
/// not judged, because nothing the queue decides reads it.
pub fn mirror_mismatches(
    remote: &str,
    local: &str,
    base: &str,
    observed: &std::collections::BTreeSet<u64>,
) -> Vec<String> {
    let served = ref_lines(remote);
    let serves = |r: &str| {
        served
            .iter()
            .find(|l| l.reference == r)
            .map(|l| l.sha.clone())
    };
    let mut found = Vec::new();
    for RefLine { sha, reference: r } in ref_lines(local) {
        let wanted = if r == format!("refs/remotes/origin/{base}") {
            serves(&format!("refs/heads/{base}"))
        } else if let Some(n) = r.strip_prefix("refs/majordomus/prs/") {
            if !n.parse::<u64>().is_ok_and(|n| observed.contains(&n)) {
                continue;
            }
            Some(serves(&format!("refs/pull/{n}/head")).unwrap_or_default())
        } else {
            continue;
        };
        match wanted {
            Some(w) if w == sha => {}
            Some(w) if w.is_empty() => {
                found.push(format!("{r} is {sha}, and origin serves nothing there"))
            }
            Some(w) => found.push(format!("{r} is {sha}, and origin serves {w}")),
            None => {}
        }
    }
    found
}

/// Run the cycle between two snapshots and judge it.
pub fn prove_dry_run(root: &Path) -> Result<DryRunProof, String> {
    // the repository's common git directory, where its trail and lease are: asked once
    let common = common_dir(root)?;
    // the base is the executor's: from the observation when there is one, else the forge's
    let base = match super::load_observation(root).ok().flatten() {
        Some(o) => o.base,
        None => {
            #[derive(Deserialize)]
            struct View {
                #[serde(rename = "defaultBranchRef")]
                default_branch: Branch,
            }
            #[derive(Deserialize)]
            struct Branch {
                name: String,
            }
            let v: View = serde_json::from_str(&run(
                root,
                "gh",
                &["repo", "view", "--json", "defaultBranchRef"],
            )?)
            .map_err(|e| format!("gh repo view did not name the default branch: {e}"))?;
            v.default_branch.name
        }
    };
    let before = snapshot(root, &common, &base)?;

    let mut steps = Vec::new();
    // the observation and the plan built from it are one read: the plan reads what the
    // refresh recorded, so a refresh that succeeded and a plan that cannot follow are one failure
    let (obs, queue) =
        super::refresh(root).and_then(|obs| super::queue_of(root).map(|q| (obs, q)))?;
    steps.push(ProofStep {
        step: "refresh".into(),
        summary: format!(
            "observed {} open pull request(s); {} is {}",
            obs.pull_requests.len(),
            obs.base,
            obs.base_sha
        ),
    });
    steps.push(ProofStep {
        step: "plan".into(),
        summary: match queue.next_merge {
            Some(n) => format!("the next merge is #{n}"),
            None => "nothing is ready to merge".into(),
        },
    });
    let mut integrator = ForgeIntegrator { root, lease: None };
    let report = drain::drain(root, &mut integrator, 1, true, false)?;
    steps.push(ProofStep {
        step: "drain --dry-run".into(),
        // the command line's own lines: one sentence per outcome, and the stop only where
        // no step has said it
        summary: crate::commands::prs::drain_lines(&report).join("; "),
    });
    let listed = drain::cleanup(root, &mut integrator, false)?;
    steps.push(ProofStep {
        step: "cleanup".into(),
        summary: if listed.is_empty() {
            "nothing to close".into()
        } else {
            listed
                .iter()
                .map(|i| format!("#{} {}", i.pr, i.action))
                .collect::<Vec<_>>()
                .join(", ")
        },
    });

    let after = snapshot(root, &common, &base)?;
    let moved = diff(&before, &after);
    let observed: std::collections::BTreeSet<u64> = obs
        .pull_requests
        .iter()
        .map(|p| p.number)
        .chain(obs.resolved.keys().copied())
        .collect();
    // what origin serves now and the queue the cycle left, read together
    let ((remote, local), queue) =
        refs(root).and_then(|r| super::queue_of(root).map(|q| (r, q)))?;
    let mirrors = mirror_mismatches(&remote, &local, &base, &observed);
    let classification = queue
        .assessments
        .iter()
        .map(|a| ObservedClassification {
            number: a.number,
            head_sha: a.evaluated_against.head_sha.clone(),
            disposition: serde_json::to_value(a.disposition)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
            next_action: a.next_action.clone(),
        })
        .collect();
    Ok(DryRunProof {
        ok: moved.is_empty() && mirrors.is_empty(),
        base,
        steps,
        moved,
        mirrors,
        before,
        after,
        classification,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(sections: &[(&str, &[&str])]) -> Snapshot {
        Snapshot {
            sections: sections
                .iter()
                .map(|(n, l)| SnapshotSection {
                    name: (*n).into(),
                    lines: l.iter().map(|x| x.to_string()).collect(),
                })
                .collect(),
        }
    }

    #[test]
    fn equal_snapshots_move_nothing() {
        let s = snap(&[("remote", &["a refs/heads/master"]), ("trail", &["absent"])]);
        assert!(diff(&s, &s).is_empty());
    }

    /// A trail that grew, a ref that appeared and one that vanished are each named in
    /// their own section, removed before added.
    #[test]
    fn every_change_is_named_by_section() {
        let before = snap(&[
            ("remote", &["a refs/heads/master", "b refs/heads/gone"]),
            ("trail", &["absent"]),
        ]);
        let after = snap(&[
            ("remote", &["a refs/heads/master", "c refs/heads/new"]),
            ("trail", &["lines 1", "sha256 x"]),
        ]);
        let moved = diff(&before, &after);
        let seen: Vec<(&str, &str, &str)> = moved
            .iter()
            .map(|m| (m.section.as_str(), m.change.as_str(), m.line.as_str()))
            .collect();
        assert_eq!(
            seen,
            [
                ("remote", "removed", "b refs/heads/gone"),
                ("remote", "added", "c refs/heads/new"),
                ("trail", "removed", "absent"),
                ("trail", "added", "lines 1"),
                ("trail", "added", "sha256 x"),
            ]
        );
    }

    #[test]
    fn only_the_two_mirrored_namespaces_are_exempt_from_comparison() {
        assert!(mirrored("refs/majordomus/prs/12", "master"));
        assert!(mirrored("refs/remotes/origin/master", "master"));
        assert!(!mirrored("refs/remotes/origin/feature", "master"));
        assert!(!mirrored("refs/heads/master", "master"));
        assert!(!mirrored("refs/majordomus/other", "master"));
    }

    /// A mirror pointing where origin serves nothing is as wrong as one pointing at an
    /// older commit; a ref outside the mirrors is not judged here at all.
    #[test]
    fn a_mirror_must_equal_what_origin_serves() {
        let remote = "a refs/heads/master\nb refs/pull/1/head\n";
        let local = "z refs/remotes/origin/master\nb refs/majordomus/prs/1\nq refs/majordomus/prs/9\nx refs/heads/mine\n";
        let found = mirror_mismatches(remote, local, "master", &[1, 9].into());
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].contains("refs/majordomus/prs/9") && found[0].contains("serves nothing"));
        assert!(found[1].contains("refs/remotes/origin/master is z"));
    }

    /// A mirror of a pull request that is no longer open — merged days ago, its mirror left at
    /// the head it had then — is not this refresh's mirror and does not fail the proof.
    #[test]
    fn a_mirror_of_a_pull_request_not_observed_is_not_judged() {
        let remote = "a refs/heads/master\nb refs/pull/1/head\nc refs/pull/698/head\n";
        let local = "a refs/remotes/origin/master\nb refs/majordomus/prs/1\nstale refs/majordomus/prs/698\n";
        assert!(mirror_mismatches(remote, local, "master", &[1].into()).is_empty());
        assert_eq!(
            mirror_mismatches(remote, local, "master", &[1, 698].into()).len(),
            1
        );
    }

    #[test]
    fn the_forge_section_is_sorted_and_carries_head_state_and_labels() {
        let json = r#"[{"number":2,"headRefOid":"bb","state":"OPEN","labels":[{"name":"z"},{"name":"a"}]},
                       {"number":1,"headRefOid":"aa","labels":[]}]"#;
        assert_eq!(
            forge_lines(json).unwrap(),
            ["#1 aa - []", "#2 bb OPEN [a,z]"]
        );
        assert!(forge_lines("{}").is_err(), "an object is not a list");
    }
}

#[cfg(test)]
mod trail_and_helper_tests {
    use super::*;

    #[test]
    fn the_trail_compares_acts_and_lets_observations_grow() {
        let one = b"{\"action\":\"observed\"}\n{\"action\":\"lease_acquired\"}\n";
        let more = b"{\"action\":\"observed\"}\n{\"action\":\"lease_acquired\"}\n{\"action\":\"observed\"}\n";
        assert_eq!(
            trail_acts(one),
            trail_acts(more),
            "a read records what it observed"
        );
        let act = b"{\"action\":\"observed\"}\n{\"action\":\"lease_acquired\"}\n{\"action\":\"merge_attempted\"}\n";
        assert_ne!(trail_acts(one), trail_acts(act), "an act is seen");
        assert_eq!(trail_acts(act)[0], "acts 2");
        // a line that is not an event is not an observation either: it counts
        assert_eq!(trail_acts(b"not json\n")[0], "acts 1");
    }

    #[test]
    fn the_helpers_refuse_what_they_cannot_read() {
        let pairs = |text: &str| -> Vec<(String, String)> {
            ref_lines(text)
                .into_iter()
                .map(|l| (l.sha, l.reference))
                .collect()
        };
        assert_eq!(
            pairs("aa refs/heads/x\nlonely\n\nbb refs/heads/a\n"),
            [
                ("bb".to_string(), "refs/heads/a".to_string()),
                ("aa".to_string(), "refs/heads/x".to_string())
            ]
        );
        // in canonical order: a pull request's number by value, not by its digits
        assert_eq!(
            pairs("c refs/majordomus/prs/10\nd refs/majordomus/prs/9\n")
                .iter()
                .map(|(s, _)| s.as_str())
                .collect::<Vec<_>>(),
            ["d", "c"]
        );
        let dir = tempfile::tempdir().unwrap();
        let none = run(dir.path(), "majordomus-no-such-program", &[]).unwrap_err();
        assert!(none.contains("could not run"), "{none}");
        assert!(common_dir(dir.path()).is_err(), "not a repository");
        // and a proof asked of no repository refuses before it reads anything else
        assert!(prove_dry_run(dir.path()).is_err());
        // origin serving no base leaves the base's mirror unjudged, not mismatched
        let observed = std::collections::BTreeSet::new();
        assert!(mirror_mismatches(
            "aa\trefs/heads/other\n",
            "bb refs/remotes/origin/master\n",
            "master",
            &observed
        )
        .is_empty());
    }
}
