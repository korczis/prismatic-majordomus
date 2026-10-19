//! `plan.allocate`: an identifier is taken by one atomic claim (I3040, ADR 0177).
//!
//! # Why a survey is not enough
//!
//! `scripts/case-numbers next` and `majordomus adr next` survey this tree, the sibling
//! worktrees, every ref and the board, and both say of their answer that it is a best
//! reading, not a reservation: a number two sessions read in the same minute is the same
//! number. On 2026-10-17 I2795, case 3178 and case 3221 were each taken twice in one day, and
//! the seats fell back on hand-assigned blocks of ids. What a reading cannot give, one claim
//! can: an exclusive mesh claim on the identifier's own key, `id:<kind>/<number>`, is admitted
//! under the runtime's admission lock (`Cooperation::claim`), so of two callers on one runtime
//! exactly one holds a number and the other is told who does.
//!
//! # What is surveyed
//!
//! Four sources, each answering what the others cannot, and each reported with what it held:
//! this tree, the sibling worktrees (authored and uncommitted is a real claim), every ref
//! (every identity ever added on any branch, tag, remote or rescue ref: a renamed or deleted
//! file leaves its number spent) and every claim the mesh has folded on `id:<kind>/*`, held,
//! released or expired, so that a number reserved and given up is never handed out again
//! while the journal remembers it. The candidate is one past the highest of them, or the
//! caller's `at_least` when that is higher (a survey the caller ran itself, the board's
//! announced blocks), and allocation never fills a hole.
//!
//! # What it does not promise
//!
//! Across linked runtimes two claims made before they sync are both admitted, and the fold
//! then names one winner on every runtime (ADR 0067); the loser's claim reads `conflicted`
//! on `mesh.state` and its holder must allocate again. Fencing is
//! `mesh-journal-converges`' work, not this capability's. The reservation lives while its
//! session does; once the record carrying the number is pushed, every ref survey sees it and
//! the claim may be released with `majordomus mesh release <key>`.
//!
//! ```
//! use majordomus_cli::capability::builtin::allocate::IdKind;
//!
//! // the three kinds, spelled as every surface spells them
//! let words: Vec<_> = [IdKind::Issue, IdKind::Case, IdKind::Adr]
//!     .iter()
//!     .map(|k| serde_json::to_value(k).unwrap())
//!     .collect();
//! assert_eq!(words, ["issue", "case", "adr"]);
//! ```

use std::collections::BTreeSet;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::mesh::cooperation::{Cooperation, CooperationError};
use crate::mesh::journal::{ClaimMode, SessionInfo};

/// How many numbers past the surveyed candidate an allocation tries before it gives up.
const ATTEMPTS: u32 = 20;

// ---------------------------------------------------------------- input and output

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema,
    clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
/// The kind of identifier allocated: what `majordomus mesh allocate <kind>` and
/// `plan.allocate` take.
///
/// ```
/// use majordomus_cli::capability::builtin::allocate::IdKind;
///
/// let kind: IdKind = serde_json::from_value(serde_json::json!("adr")).unwrap();
/// assert_eq!(kind, IdKind::Adr);
/// assert!(serde_json::from_value::<IdKind>(serde_json::json!("ticket")).is_err());
/// ```
pub enum IdKind {
    /// An issue id, `I<nnnn>`, the name of `.ai/repo/project/issues/<id>.yaml`.
    Issue,
    /// A case number, the identity before the first underscore of `test/cases/<n>_*.sh`.
    Case,
    /// An ADR number, the four digits that open `.ai/repo/adrs/<nnnn>-*.md`.
    Adr,
}

impl IdKind {
    /// The word of the reservation key and of every message.
    pub(crate) fn word(self) -> &'static str {
        match self {
            IdKind::Issue => "issue",
            IdKind::Case => "case",
            IdKind::Adr => "adr",
        }
    }

    /// The directory, relative to a checkout's root, whose file names carry the identities.
    pub(crate) fn dir(self) -> &'static str {
        match self {
            IdKind::Issue => ".ai/repo/project/issues",
            IdKind::Case => "test/cases",
            IdKind::Adr => ".ai/repo/adrs",
        }
    }

    /// The number a file name carries, when it is one of this kind's files.
    pub(crate) fn number_of(self, file_name: &str) -> Option<u32> {
        let digits = |s: &str| -> Option<u32> {
            let d: String = s.chars().take_while(char::is_ascii_digit).collect();
            (!d.is_empty()).then(|| d.parse().ok()).flatten()
        };
        match self {
            IdKind::Issue => file_name
                .strip_prefix('I')
                .and_then(|r| r.strip_suffix(".yaml"))
                .filter(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()))
                .and_then(|r| r.parse().ok()),
            IdKind::Case => file_name
                .strip_suffix(".sh")
                .filter(|r| r.contains('_'))
                .and_then(digits),
            IdKind::Adr => file_name
                .strip_suffix(".md")
                .filter(|r| r.len() > 5 && r.as_bytes()[4] == b'-')
                .and_then(|r| digits(&r[..4]))
                .filter(|_| file_name[..4].chars().all(|c| c.is_ascii_digit())),
        }
    }

    /// How the number is written where people read it.
    pub(crate) fn render(self, n: u32) -> String {
        match self {
            IdKind::Issue => format!("I{n:04}"),
            IdKind::Case => n.to_string(),
            IdKind::Adr => format!("{n:04}"),
        }
    }

    /// The mesh key the number is reserved under.
    pub(crate) fn key(self, n: u32) -> String {
        format!("id:{}/{n}", self.word())
    }

    /// The number a reservation key carries, when it is one of this kind's.
    pub(crate) fn number_of_key(self, key: &str) -> Option<u32> {
        key.strip_prefix(&format!("id:{}/", self.word()))
            .and_then(|n| n.parse().ok())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `plan.allocate`.
pub(crate) struct AllocateInput {
    /// `issue`, `case` or `adr`.
    pub kind: IdKind,
    /// The mesh session the number is reserved for; the calling MCP session's own when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The lowest number the caller will take: a survey it ran itself, or a block it was given. The candidate is the higher of this and one past every source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_least: Option<u32>,
}

impl BenchmarkCases for AllocateInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "case_from_a_floor",
            Self {
                kind: IdKind::Case,
                session: Some("bench".into()),
                at_least: Some(9000),
            },
        )]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// A number reserved for one session, with the survey it was chosen from.
pub(crate) struct Allocation {
    /// The kind.
    pub kind: IdKind,
    /// The identifier as it is written: `I3041`, `3441`, `0178`.
    pub value: String,
    /// The number.
    pub number: u32,
    /// The mesh key it is reserved under, `id:<kind>/<number>`.
    pub reservation: String,
    /// The key of the exclusive claim that holds it, for `majordomus mesh release`.
    pub claim: String,
    /// What each source held.
    pub sources: Vec<IdSource>,
    /// Numbers tried before this one and held by another session's claim, with the holder.
    pub skipped: Vec<IdSkip>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One source of an allocation's survey.
pub(crate) struct IdSource {
    /// `tree`, `worktrees`, `refs`, `mesh` or `at_least`.
    pub source: String,
    /// The highest number it holds; absent when it holds none.
    pub highest: Option<u32>,
    /// Whether it could be read.
    pub read: bool,
    /// What was read, or why not.
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// A number another session holds.
pub(crate) struct IdSkip {
    /// The number.
    pub number: u32,
    /// Its claim's holding session.
    pub holder: String,
    /// Its claim's key.
    pub claim: String,
}

// ---------------------------------------------------------------- the survey

/// The numbers of `kind` among the file names under `root/<dir>`.
fn listed(root: &Path, kind: IdKind) -> Vec<u32> {
    std::fs::read_dir(root.join(kind.dir()))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| kind.number_of(&e.file_name().to_string_lossy()))
                .collect()
        })
        .unwrap_or_default()
}

fn source(name: &str, numbers: &[u32], read: bool, detail: String) -> IdSource {
    IdSource {
        source: name.into(),
        highest: numbers.iter().copied().max(),
        read,
        detail,
    }
}

/// What this tree, the sibling worktrees, every ref and the mesh hold of `kind`.
pub(crate) fn survey(root: &Path, kind: IdKind, cooperation: &Cooperation) -> Vec<IdSource> {
    let tree = listed(root, kind);
    let mut out = vec![source(
        "tree",
        &tree,
        true,
        format!("{} file(s) under {}", tree.len(), kind.dir()),
    )];

    out.push(match crate::worktree::topology::read(root) {
        Ok(records) => {
            let others: Vec<_> = records
                .iter()
                .filter(|r| !r.bare && r.path.canonicalize().ok() != root.canonicalize().ok())
                .collect();
            let numbers: Vec<u32> = others.iter().flat_map(|r| listed(&r.path, kind)).collect();
            source(
                "worktrees",
                &numbers,
                true,
                format!("{} sibling worktree(s)", others.len()),
            )
        }
        Err(e) => source(
            "worktrees",
            &[],
            false,
            format!("git worktree list failed: {e}"),
        ),
    });

    let log = crate::git::read_only(root)
        .args([
            "log",
            "--all",
            "--diff-filter=A",
            "--name-only",
            "--format=",
            "--",
        ])
        .arg(kind.dir())
        .output();
    out.push(match log {
        Ok(o) if o.status.success() => {
            let numbers: Vec<u32> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.rsplit('/').next())
                .filter_map(|f| kind.number_of(f))
                .collect();
            let distinct: BTreeSet<u32> = numbers.iter().copied().collect();
            source(
                "refs",
                &numbers,
                true,
                format!("{} identit(y/ies) ever added on any ref", distinct.len()),
            )
        }
        Ok(o) => source(
            "refs",
            &[],
            false,
            format!("git log --all exited {}", o.status),
        ),
        Err(e) => source("refs", &[], false, format!("git could not run: {e}")),
    });

    let state = cooperation.state();
    let numbers: Vec<u32> = state
        .claims
        .iter()
        .flat_map(|c| c.scope.iter())
        .filter_map(|s| kind.number_of_key(s))
        .collect();
    out.push(source(
        "mesh",
        &numbers,
        true,
        format!(
            "{} reservation(s) the journal remembers, held or ended",
            numbers.len()
        ),
    ));
    out
}

// ---------------------------------------------------------------- the reservation

/// Reserve the first number from `start` that no other session holds, trying `attempts`.
pub(crate) fn reserve(
    cooperation: &Cooperation,
    info: &SessionInfo,
    kind: IdKind,
    start: u32,
    attempts: u32,
) -> Result<(u32, String, Vec<IdSkip>), CapabilityError> {
    let mut skipped = Vec::new();
    for n in start..start.saturating_add(attempts) {
        let intent = format!(
            "{} {} reserved for {}",
            kind.word(),
            kind.render(n),
            info.session
        );
        match cooperation.claim(
            info,
            vec![kind.key(n)],
            Some(intent),
            ClaimMode::Exclusive,
            None,
        ) {
            Ok(w) => return Ok((n, w.key, skipped)),
            Err(CooperationError::Conflict(met)) => {
                skipped.extend(met.into_iter().map(|c| IdSkip {
                    number: n,
                    holder: c.session,
                    claim: c.key,
                }))
            }
            Err(e) => return Err(super::mesh::map_error(e)),
        }
    }
    Err(CapabilityError::Refused(format!(
        "claim_conflict: every {} from {} to {} is held by another session",
        kind.word(),
        kind.render(start),
        kind.render(start.saturating_add(attempts) - 1)
    )))
}

pub(crate) fn allocate(ctx: &Context, input: AllocateInput) -> Result<Allocation, CapabilityError> {
    let cooperation = ctx.mesh.cooperation().ok_or_else(|| {
        CapabilityError::Refused(
            "not_active: a number is reserved by the mesh of the server holding this checkout's lease; ask it (majordomus mesh allocate), or start one with a mesh declaration"
                .into(),
        )
    })?;
    let session = super::mesh::session_of(ctx, input.session)?;
    let root = Path::new(&ctx.index.repository.root);
    let mut sources = survey(root, input.kind, &cooperation);
    if let Some(floor) = input.at_least {
        // the caller's floor is the lowest it takes, so the source holds the number before it
        sources.push(source(
            "at_least",
            &[floor.saturating_sub(1)],
            true,
            format!("the caller takes nothing below {floor}"),
        ));
    }
    let start = sources
        .iter()
        .filter_map(|s| s.highest)
        .max()
        .map_or(1, |h| h + 1);
    let info = SessionInfo {
        session,
        client: "plan.allocate".into(),
        ..SessionInfo::default()
    };
    let (number, claim, skipped) = reserve(&cooperation, &info, input.kind, start, ATTEMPTS)?;
    Ok(Allocation {
        kind: input.kind,
        value: input.kind.render(number),
        number,
        reservation: input.kind.key(number),
        claim,
        sources,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_name_carries_its_kinds_number_and_no_other() {
        assert_eq!(IdKind::Issue.number_of("I3040.yaml"), Some(3040));
        assert_eq!(IdKind::Issue.number_of("I3040.md"), None);
        assert_eq!(IdKind::Issue.number_of("Ix.yaml"), None);
        assert_eq!(IdKind::Case.number_of("3440_an_id_is_taken.sh"), Some(3440));
        assert_eq!(IdKind::Case.number_of("87b_the_second_half.sh"), Some(87));
        assert_eq!(IdKind::Case.number_of("lib.sh"), None);
        assert_eq!(
            IdKind::Adr.number_of("0177-a-session-proceeds.md"),
            Some(177)
        );
        assert_eq!(IdKind::Adr.number_of("README.md"), None);
        assert_eq!(IdKind::Adr.number_of("017-x.md"), None);
        assert_eq!(IdKind::Issue.render(41), "I0041");
        assert_eq!(IdKind::Adr.render(178), "0178");
        assert_eq!(IdKind::Case.render(3441), "3441");
        assert_eq!(
            IdKind::Case.number_of_key(&IdKind::Case.key(3441)),
            Some(3441)
        );
        assert_eq!(IdKind::Adr.number_of_key("id:case/3441"), None);
    }

    fn session(s: &str) -> SessionInfo {
        SessionInfo {
            session: s.into(),
            client: "test".into(),
            ..SessionInfo::default()
        }
    }

    #[test]
    fn a_held_number_is_skipped_naming_its_holder_and_the_next_is_reserved() {
        let c = crate::mesh::testing::cooperation();
        let (first, _, skipped) = reserve(&c, &session("a"), IdKind::Case, 10, 3).unwrap();
        assert_eq!((first, skipped.len()), (10, 0));
        let (second, _, skipped) = reserve(&c, &session("b"), IdKind::Case, 10, 3).unwrap();
        assert_eq!(second, 11);
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].number, 10);
        assert!(skipped[0].holder.ends_with("/a"), "{}", skipped[0].holder);
        // every candidate held: refused, naming the range
        let e = reserve(&c, &session("c"), IdKind::Case, 10, 2).unwrap_err();
        assert!(
            e.to_string().contains("every case from 10 to 11 is held"),
            "{e}"
        );
    }

    #[test]
    fn the_survey_reads_the_tree_the_refs_and_every_reservation_the_journal_remembers() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let cases = repo.root().join("test/cases");
        std::fs::create_dir_all(&cases).unwrap();
        std::fs::write(cases.join("41_a.sh"), "").unwrap();
        let c = crate::mesh::testing::cooperation();
        reserve(&c, &session("a"), IdKind::Case, 77, 1).unwrap();
        let sources = survey(repo.root(), IdKind::Case, &c);
        let high = |n: &str| {
            sources
                .iter()
                .find(|s| s.source == n)
                .and_then(|s| s.highest)
        };
        assert_eq!(high("tree"), Some(41));
        assert_eq!(high("mesh"), Some(77));
        assert!(sources.iter().any(|s| s.source == "refs"));
        assert!(sources.iter().any(|s| s.source == "worktrees"));
    }

    #[test]
    fn allocate_reserves_past_every_source_and_refuses_without_a_mesh() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let input = |at_least| AllocateInput {
            kind: IdKind::Adr,
            session: Some("me".into()),
            at_least,
        };
        let e = allocate(&ctx, input(None)).unwrap_err();
        assert!(e.to_string().contains("not_active"), "{e}");
        crate::mesh::testing::attach(&ctx, crate::mesh::testing::cooperation());
        let a = allocate(&ctx, input(Some(9000))).unwrap();
        assert_eq!(
            (a.number, a.value.as_str(), a.reservation.as_str()),
            (9000, "9000", "id:adr/9000")
        );
        let b = allocate(&ctx, input(None)).unwrap();
        assert_eq!(b.number, 9001, "a reservation the journal holds is spent");
        assert!(b.skipped.is_empty());
    }
}
