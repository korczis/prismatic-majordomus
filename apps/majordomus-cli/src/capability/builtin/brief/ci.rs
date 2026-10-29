//! The CI section: the hosted runs in progress and queued, and the self-hosted runners.
//!
//! Composed by the leaseholder only, through the monitor's shared cache (runs 30 s, runners
//! 60 s), so that a status line ticking in every session never multiplies calls to the
//! forge: every session's brief is answered by the one leaseholder, and a brief composed
//! elsewhere says it did not read the pool and names the command that would.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::monitor::github::{self, Unanswered};

use super::{BriefCiSection, BriefObservation, BriefRunner, BriefSectionState, BriefVerdict};

/// What one call of the section may take; the section's own budget bounds them together.
const CALL_BUDGET: Duration = Duration::from_secs(4);

/// The section, as the composing process may read it.
pub(crate) fn section(root: &Path, leaseholder: bool) -> BriefCiSection {
    if !leaseholder {
        return BriefCiSection::default();
    }
    let started = Instant::now();
    let (running, queued, runners, repo) = std::thread::scope(|s| {
        let a = s.spawn(|| github::run_total(root, "in_progress", CALL_BUDGET));
        let b = s.spawn(|| github::run_total(root, "queued", CALL_BUDGET));
        let c = s.spawn(|| github::runners(root, CALL_BUDGET));
        let d = s.spawn(|| github::repository(root, CALL_BUDGET));
        let gone = || Err(Unanswered::Failed("the reading thread panicked".into()));
        (
            a.join().unwrap_or_else(|_| gone()),
            b.join().unwrap_or_else(|_| gone()),
            c.join().unwrap_or_else(|_| gone()),
            d.join().unwrap_or_else(|_| gone()),
        )
    });
    let total = |r: &Result<github::Answer, Unanswered>| {
        r.as_ref().ok().and_then(|a| a.value["total_count"].as_u64())
    };
    let age = [&running, &queued, &runners]
        .iter()
        .filter_map(|r| r.as_ref().ok().map(|a| a.age))
        .max()
        .unwrap_or_default();
    let repository = repo
        .as_ref()
        .ok()
        .and_then(|a| a.value["nameWithOwner"].as_str().map(str::to_string))
        .unwrap_or_else(|| "{owner}/{repo}".into());
    compose(
        total(&running),
        total(&queued),
        runners.map(|a| {
            a.value["runners"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| BriefRunner {
                    name: r["name"].as_str().unwrap_or_default().to_string(),
                    status: r["status"].as_str().unwrap_or_default().to_string(),
                    busy: r["busy"].as_bool().unwrap_or(false),
                })
                .collect()
        }),
        [running.err(), queued.err()].into_iter().flatten().map(|e| e.reason().to_string()).collect(),
        repository,
        age,
        started.elapsed(),
    )
}

/// The section from what the forge answered: kept apart so that a test hands it answers.
pub(crate) fn compose(
    running: Option<u64>,
    queued: Option<u64>,
    runners: Result<Vec<BriefRunner>, Unanswered>,
    errors: Vec<String>,
    repository: String,
    age: Duration,
    elapsed: Duration,
) -> BriefCiSection {
    let (list, unread) = match runners {
        Ok(l) => (l, None),
        Err(Unanswered::Forbidden(r)) => (Vec::new(), Some(format!("{r}, needs admin on the repository"))),
        Err(Unanswered::Failed(r)) => (Vec::new(), Some(r)),
    };
    let online = list.iter().filter(|r| r.status == "online").count();
    let busy = list.iter().filter(|r| r.status == "online" && r.busy).count();
    let offline = list.len() - online;
    let complete = running.is_some() && queued.is_some() && unread.is_none();
    let state = if running.is_none() && queued.is_none() {
        BriefSectionState::Unread
    } else if complete {
        BriefSectionState::Read
    } else {
        BriefSectionState::Partial
    };
    let verdict = match state {
        BriefSectionState::Read => BriefVerdict::Current,
        BriefSectionState::Partial => BriefVerdict::Partial,
        _ => BriefVerdict::Unread,
    };
    let n = |v: Option<u64>| v.map(|n| n.to_string()).unwrap_or_else(|| "?".into());
    let mut summary = format!(
        "[{}] hosted {} [running], {} [queued]",
        verdict.as_str(),
        n(running),
        n(queued)
    );
    match &unread {
        Some(r) => summary.push_str(&format!("; self-hosted [unread] ({r})")),
        None if list.is_empty() => summary.push_str("; no self-hosted runner"),
        None => {
            let names: Vec<&str> = list.iter().filter(|r| r.status == "online").map(|r| r.name.as_str()).collect();
            summary.push_str(&format!(
                "; self-hosted {online} online ({}), {busy} busy, {offline} offline",
                names.join(", ")
            ));
            if queued.unwrap_or(0) > 0 && online > 0 && busy < online {
                summary.push_str("; queued for hosted labels, self-hosted idle");
            }
            if queued.unwrap_or(0) > 0 && online == 0 {
                summary.push_str("; [blocked] no runner online");
            }
        }
    }
    if !errors.is_empty() {
        summary.push_str(&format!("; {}", errors.join("; ")));
    }
    summary.push_str(&format!("; gh via the monitor cache, {} s old", age.as_secs()));
    BriefCiSection {
        state,
        observed: BriefObservation {
            source: "gh api actions/runs and actions/runners (monitor cache)".into(),
            observed_at: Some(crate::ledger::now()),
            age_seconds: Some(age.as_secs()),
            bound_seconds: Some(github::BRIEF_TTL.as_secs()),
            verdict,
            remedy: (verdict != BriefVerdict::Current)
                .then(|| format!("gh api repos/{repository}/actions/runners")),
            elapsed_ms: elapsed.as_millis() as u64,
            budget_ms: 0,
        },
        repository,
        in_progress: running,
        queued,
        online,
        busy,
        offline,
        runners: list,
        runners_unread: unread,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner(name: &str, status: &str, busy: bool) -> BriefRunner {
        BriefRunner { name: name.into(), status: status.into(), busy }
    }

    #[test]
    fn ci_from_a_cli_composition_is_unread_not_the_leaseholder() {
        let s = section(Path::new("/nowhere"), false);
        assert_eq!(s.state, BriefSectionState::Unread);
        assert_eq!(s.observed.verdict, BriefVerdict::Unread);
        assert_eq!(s.observed.remedy.as_deref(), Some("majordomus serve ensure"));
        assert!(s.summary.contains("not the leaseholder"), "{}", s.summary);
    }

    #[test]
    fn ci_queued_with_no_runner_online_is_blocked_on_a_person() {
        let s = compose(
            Some(0),
            Some(10),
            Ok(vec![runner("lundra-1", "offline", false)]),
            vec![],
            "o/r".into(),
            Duration::from_secs(12),
            Duration::from_millis(5),
        );
        assert!(s.summary.contains("[blocked] no runner online"), "{}", s.summary);
        let idle = compose(
            Some(1),
            Some(10),
            Ok(vec![runner("lundra-1", "online", false), runner("lundra-2", "online", false)]),
            vec![],
            "o/r".into(),
            Duration::from_secs(12),
            Duration::from_millis(5),
        );
        assert_eq!((idle.online, idle.busy), (2, 0));
        assert!(idle.summary.contains("queued for hosted labels, self-hosted idle"), "{}", idle.summary);
        let forbidden = compose(
            Some(1),
            Some(0),
            Err(Unanswered::Forbidden("403: Resource not accessible".into())),
            vec![],
            "o/r".into(),
            Duration::ZERO,
            Duration::ZERO,
        );
        assert_eq!(forbidden.state, BriefSectionState::Partial);
        assert!(forbidden.runners_unread.as_deref().is_some_and(|r| r.contains("needs admin")));
    }
}
