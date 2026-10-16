//! What one read of the queue leaves in this process for the next read of the same checkout.
//!
//! Deciding a pull request's relation to master spawns about a dozen git processes, and the
//! queue decides every open one: with ninety open, a read took minutes. The file cache
//! (`relations.json`) is written only by the paths that just observed the forge, so a read
//! that finds it missing or kept for another master decides everything again, and so does
//! the next. And each concurrent reader did all of that at once.
//!
//! Three facts make it safe to remember instead:
//!
//! - a relation and a shape are about two commit ids, `master..head`, the same keys
//!   `relations.json` already keeps forever ([`super::RelationCache`]), so this process keeps
//!   them too, for the master it last read against;
//! - whether one mirrored head contains another is about two commit ids as well: a commit's
//!   ancestry never changes, so a pair once decided stays decided, and only a head or a mirror
//!   that moved is asked again ([`CheckoutMemo::containing`]);
//! - reads of one checkout take turns ([`Memos::checkout`]), so a second reader finds what the
//!   first decided instead of deciding it beside it.
//!
//! What is not a fact about two commits is asked of git on every read: which observed heads
//! this clone has (one `cat-file --batch-check`, [`present`]), which commit each mirror names
//! (one `for-each-ref`), the trail and the observation. Nothing here writes a file: a read
//! still leaves the checkout as it was. `MAJORDOMUS_INTEGRATION_MEMO=off` reads without it
//! (`project.cache-is-invisible`).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};

use super::RelationCache;

/// How many checkouts one process remembers. A server serves one checkout; the bound keeps a
/// process that reads many (a test run) from keeping all of them.
const CHECKOUTS: usize = 16;

/// The environment switch that reads the queue without this process's memory.
pub(super) const SWITCH: &str = "MAJORDOMUS_INTEGRATION_MEMO";

/// Whether this process was told to read the queue without its memory.
pub(super) fn bypassed() -> bool {
    std::env::var_os(SWITCH).is_some_and(|v| v == "off")
}

/// How many answers a checkout's memory had to ask git for, since it was made: what a read
/// that found them remembered did not cost.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Decided {
    /// Relations decided by git.
    pub(super) relations: u64,
    /// Shapes decided by git.
    pub(super) shapes: u64,
    /// Containment questions put to git: one per head asked against every mirror, one per
    /// mirror asked against every head.
    pub(super) containment: u64,
}

/// What this process remembers of one checkout.
#[derive(Debug, Default)]
pub(super) struct CheckoutMemo {
    /// The relations and shapes decided against the last master read, under the keys of
    /// `relations.json`, for the heads that read asked about.
    pub(super) relations: RelationCache,
    /// `(mirror commit, head)` → whether the mirror contains the head.
    contains: HashMap<(String, String), bool>,
    /// What had to be decided, for the cases that prove the rest was not.
    pub(super) decided: Decided,
}

/// The memories of the checkouts this process read, each behind its own lock.
#[derive(Debug)]
pub(super) struct Memos {
    slots: Mutex<BTreeMap<PathBuf, Slot>>,
    capacity: usize,
}

#[derive(Debug)]
struct Slot {
    used: u64,
    memo: Arc<Mutex<CheckoutMemo>>,
}

/// The memory of this process.
pub(super) static PROCESS: Memos = Memos::new(CHECKOUTS);

pub(super) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // every entry is a fact on its own: one a panicking reader left half written is still one
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Memos {
    /// An empty memory of at most `capacity` checkouts.
    pub(super) const fn new(capacity: usize) -> Self {
        Memos {
            slots: Mutex::new(BTreeMap::new()),
            capacity,
        }
    }

    /// The memory of `root`, made empty the first time. Its lock is the turn a read of that
    /// checkout takes: a reader holds it while it decides, so a concurrent reader waits for
    /// the answers instead of asking git the same questions beside it. Past the capacity, the
    /// checkout read longest ago is forgotten.
    pub(super) fn checkout(&self, root: &Path) -> Arc<Mutex<CheckoutMemo>> {
        let mut slots = lock(&self.slots);
        let used = slots.values().map(|s| s.used).max().unwrap_or(0) + 1;
        if let Some(slot) = slots.get_mut(root) {
            slot.used = used;
            return Arc::clone(&slot.memo);
        }
        while slots.len() >= self.capacity.max(1) {
            let oldest = slots
                .iter()
                .min_by_key(|(_, s)| s.used)
                .map(|(k, _)| k.clone());
            oldest.map(|k| slots.remove(&k));
        }
        let memo = Arc::new(Mutex::new(CheckoutMemo::default()));
        slots.insert(
            root.to_path_buf(),
            Slot {
                used,
                memo: Arc::clone(&memo),
            },
        );
        memo
    }

    /// What `root`'s memory has decided so far; nothing for a checkout it does not hold.
    #[cfg(test)]
    pub(super) fn decided(&self, root: &Path) -> Decided {
        let memo = lock(&self.slots).get(root).map(|s| Arc::clone(&s.memo));
        memo.map(|m| lock(&m).decided).unwrap_or_default()
    }
}

/// Which of `ids` name a commit this clone has, asked of git once: what
/// [`super::relation::has_commit`] answers for each. `None` when git could not answer, and
/// each is then asked on its own.
pub(super) fn present<'a>(
    root: &Path,
    ids: impl IntoIterator<Item = &'a str>,
) -> Option<HashSet<String>> {
    // a line break in a name would split it into two questions: only commit ids are asked
    let ids: Vec<&str> = ids
        .into_iter()
        .filter(|s| super::is_object_id(s))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let input: Vec<u8> = ids
        .iter()
        .flat_map(|id| format!("{id}^{{commit}}\n").into_bytes())
        .collect();
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "--batch-check"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            // written from another thread while this one reads, as derived_paths does
            let stdin = child.stdin.take();
            let writer = std::thread::spawn(move || stdin.map(|mut s| s.write_all(&input)));
            let out = child.wait_with_output();
            let _ = writer.join();
            out
        })
        .ok()
        .filter(|o| o.status.success())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = text.lines().collect();
    // one line per question, in order; any other count is an answer not understood
    (lines.len() == ids.len()).then(|| {
        ids.iter()
            .zip(lines)
            .filter(|(_, l)| l.split(' ').nth(1) == Some("commit"))
            .map(|(id, _)| (*id).to_string())
            .collect()
    })
}

/// The pull-request mirrors (`refs/majordomus/prs/<n>`), each with the commit it names, in
/// git's order: the refs [`super::relation::containing`] reads.
fn mirrors(root: &Path) -> Option<Vec<(u64, String)>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "for-each-ref",
            "--format=%(objectname) %(refname)",
            super::forge::PR_REF_PREFIX,
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| {
                l.split_once(' ').and_then(|(sha, name)| {
                    name.strip_prefix(super::forge::PR_REF_PREFIX)
                        .and_then(|n| n.parse().ok())
                        .map(|n| (n, sha.to_string()))
                })
            })
            .collect(),
    )
}

/// The commits of the mirrors that contain `head`: [`super::relation::containing`]'s
/// question. `None` when git cannot say.
fn tips_containing(root: &Path, head: &str) -> Option<HashSet<String>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "for-each-ref",
            "--contains",
            head,
            "--format=%(objectname)",
            super::forge::PR_REF_PREFIX,
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .collect(),
    )
}

/// Every commit `tip` reaches, itself included: the heads it contains are among them.
fn ancestry(root: &Path, tip: &str) -> Option<HashSet<String>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-list", tip])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .collect(),
    )
}

impl CheckoutMemo {
    /// For each of `heads`, the pull requests whose mirror contains it, each with the commit
    /// the mirror names: what [`super::relation::containing`] answers, with every pair of a
    /// mirror commit and a head decided once in this process.
    ///
    /// A head `present` says this clone lacks is in no mirror, and is answered empty without
    /// asking. A head missing most of its pairs is asked against every mirror at once; what is
    /// left is asked per mirror commit, against every head at once — so a pull request that
    /// moved costs two questions, not one per open pull request. A head missing from the
    /// answer is one git could not decide here, for the caller to ask on its own.
    pub(super) fn containing(
        &mut self,
        root: &Path,
        heads: &[String],
        present: impl Fn(&str) -> bool,
    ) -> BTreeMap<String, Vec<(u64, String)>> {
        let Some(listed) = mirrors(root) else {
            return BTreeMap::new();
        };
        let tips: BTreeSet<&str> = listed.iter().map(|(_, t)| t.as_str()).collect();
        let heads: BTreeSet<&str> = heads.iter().map(String::as_str).collect();
        let (asked, absent): (BTreeSet<&str>, BTreeSet<&str>) =
            heads.iter().partition(|h| present(h));
        // decided in this read, kept only once the mirrors are known not to have moved under it
        let mut learnt: HashMap<(String, String), bool> = HashMap::new();
        let mut undecided: BTreeSet<&str> = BTreeSet::new();
        let known = |learnt: &HashMap<(String, String), bool>, t: &str, h: &str| {
            let pair = (t.to_string(), h.to_string());
            self.contains
                .get(&pair)
                .or_else(|| learnt.get(&pair))
                .copied()
        };
        let mut by_head = false;
        for h in &asked {
            let missing = tips
                .iter()
                .filter(|t| known(&learnt, t, h).is_none())
                .count();
            if missing * 2 <= tips.len() {
                continue;
            }
            self.decided.containment += 1;
            by_head = true;
            match tips_containing(root, h) {
                Some(found) => {
                    for t in &tips {
                        learnt.insert((t.to_string(), h.to_string()), found.contains(*t));
                    }
                }
                None => drop(undecided.insert(*h)),
            }
        }
        let pending: BTreeSet<&str> = tips
            .iter()
            .filter(|t| {
                asked
                    .iter()
                    .any(|h| !undecided.contains(h) && known(&learnt, t, h).is_none())
            })
            .copied()
            .collect();
        for t in pending {
            self.decided.containment += 1;
            match ancestry(root, t) {
                Some(reached) => {
                    for h in &asked {
                        learnt.insert((t.to_string(), h.to_string()), reached.contains(*h));
                    }
                }
                None => undecided.extend(
                    asked
                        .iter()
                        .filter(|h| known(&learnt, t, h).is_none())
                        .copied(),
                ),
            }
        }
        let answers: BTreeMap<String, Vec<(u64, String)>> = asked
            .iter()
            .filter(|h| !undecided.contains(*h))
            .map(|h| {
                let found = listed
                    .iter()
                    .filter(|(_, t)| known(&learnt, t, h) == Some(true))
                    .cloned()
                    .collect();
                (h.to_string(), found)
            })
            .chain(absent.iter().map(|h| (h.to_string(), Vec::new())))
            .collect();
        // an answer per head names the mirrors as listed before it was asked: a mirror moved
        // meanwhile may have answered for its new commit, which is no fact about the old one
        if !by_head || mirrors(root).as_ref() == Some(&listed) {
            self.contains.extend(learnt);
        }
        // bounded: only the pairs of the current mirrors and heads are worth keeping
        self.contains
            .retain(|(t, h), _| tips.contains(t.as_str()) && heads.contains(h.as_str()));
        answers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.email=t@example.com", "-c", "user.name=t"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// The memory of a checkout is one per root and survives while it is read; past the
    /// capacity the checkout read longest ago is forgotten, never the one read last.
    #[test]
    fn a_checkout_has_one_memory_and_the_oldest_is_forgotten() {
        let memos = Memos::new(2);
        let (a, b, c) = (Path::new("/a"), Path::new("/b"), Path::new("/c"));
        let first = memos.checkout(a);
        lock(&first).decided.relations = 7;
        assert!(Arc::ptr_eq(&first, &memos.checkout(a)));
        assert_eq!(memos.decided(a).relations, 7);
        memos.checkout(b);
        memos.checkout(a);
        memos.checkout(c);
        assert_eq!(memos.decided(a).relations, 7, "read last, kept");
        assert_eq!(lock(&memos.slots).len(), 2);
        assert!(!lock(&memos.slots).contains_key(b), "read longest ago");
        assert_eq!(memos.decided(Path::new("/none")), Decided::default());
        assert!(!bypassed() || std::env::var_os(SWITCH).is_some());
    }

    /// One question answers what `has_commit` answers for each id; a name that is not a
    /// commit id is not asked, and a directory that is no repository answers nothing.
    #[test]
    fn presence_is_asked_once_and_answers_as_has_commit() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        git(r, &["init", "-q", "-b", "master"]);
        git(r, &["commit", "-q", "--allow-empty", "-m", "one"]);
        let one = git(r, &["rev-parse", "HEAD"]);
        let tree = git(r, &["rev-parse", "HEAD^{tree}"]);
        let absent = "0".repeat(40);
        let found = present(r, [one.as_str(), tree.as_str(), absent.as_str(), "HEAD"]).unwrap();
        for id in [&one, &tree, &absent] {
            assert_eq!(
                found.contains(id.as_str()),
                super::super::relation::has_commit(r, id),
                "{id}"
            );
        }
        assert!(!found.contains("HEAD"), "a name is never asked");
        let none = tempfile::tempdir().unwrap();
        assert_eq!(present(none.path(), [one.as_str()]), None);
        assert_eq!(present(r, std::iter::empty()), Some(HashSet::new()));
    }

    /// Containment answers what `relation::containing` answers, head by head; a pair decided
    /// once is not asked again, and git that cannot list the mirrors leaves every head to the
    /// caller.
    #[test]
    fn containment_answers_as_containing_and_asks_each_pair_once() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        git(r, &["init", "-q", "-b", "master"]);
        git(r, &["commit", "-q", "--allow-empty", "-m", "base"]);
        git(r, &["commit", "-q", "--allow-empty", "-m", "one"]);
        let one = git(r, &["rev-parse", "HEAD"]);
        git(r, &["commit", "-q", "--allow-empty", "-m", "two"]);
        let two = git(r, &["rev-parse", "HEAD"]);
        git(r, &["update-ref", "refs/majordomus/prs/1", &one]);
        git(r, &["update-ref", "refs/majordomus/prs/2", &two]);
        git(r, &["update-ref", "refs/majordomus/prs/x", &two]);
        let absent = "0".repeat(40);
        let heads = vec![one.clone(), two.clone(), absent.clone()];
        let mut memo = CheckoutMemo::default();
        let all = |_: &str| true;
        let answers = memo.containing(r, &heads, |h| h != absent);
        for h in [&one, &two] {
            assert_eq!(
                answers[h.as_str()],
                super::super::relation::containing(r, h),
                "{h}"
            );
        }
        assert!(answers[absent.as_str()].is_empty());
        let asked = memo.decided.containment;
        assert_eq!(memo.containing(r, &heads, |h| h != absent), answers);
        assert_eq!(memo.decided.containment, asked, "every pair was decided");
        // a mirror whose history git cannot walk decides nothing: the heads not yet asked
        // against it are left to the caller
        git(r, &["checkout", "-q", "--orphan", "broken"]);
        git(r, &["commit", "-q", "--allow-empty", "-m", "lost"]);
        let lost = git(r, &["rev-parse", "HEAD"]);
        git(r, &["commit", "-q", "--allow-empty", "-m", "kept"]);
        let kept = git(r, &["rev-parse", "HEAD"]);
        git(r, &["checkout", "-q", "master"]);
        git(r, &["update-ref", "refs/majordomus/prs/3", &kept]);
        let objects = r.join(".git/objects").join(&lost[..2]).join(&lost[2..]);
        std::fs::remove_file(objects).unwrap();
        let walked = memo.containing(r, &heads[..2], all);
        assert!(walked.is_empty(), "{walked:?}");
        // a head git cannot name, told present, is left to the caller
        let unnamed = memo.containing(r, std::slice::from_ref(&absent), all);
        assert!(!unnamed.contains_key(absent.as_str()));
        let none = tempfile::tempdir().unwrap();
        assert!(memo.containing(none.path(), &heads, all).is_empty());
    }
}
