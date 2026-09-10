//! The one part of worktree management that is computation rather than IO: parsing
//! `git worktree list --porcelain` into typed records, and judging a name against the
//! policy.
//!
//! Everything else the subsystem does is a git subprocess or a filesystem call, and timing
//! those measures git and the disk. Reporting a number for them would be theatre. What is
//! worth watching is that the parse stays linear as a repository grows worktrees: this
//! repository had thirty-seven at the time of writing, and the parse runs on every
//! `worktree` command, every `doctor` run and every MCP call of `worktree.list`.
//!
//! Sizes 1, 10, 100 and 500. Numbers are reported, not asserted.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use majordomus_cli::worktree::{parse_porcelain, WorktreeName, WorktreePolicy};

/// A porcelain listing of `n` work trees: the primary, then linked ones, with a realistic
/// mixture of branches, detached heads, locks and prunable records.
fn porcelain(n: usize) -> String {
    let mut s = String::with_capacity(n * 120);
    s.push_str("worktree /home/dev/example\nHEAD 1111111111111111111111111111111111111111\nbranch refs/heads/master\n\n");
    for i in 1..n {
        s.push_str(&format!(
            "worktree /home/dev/example-wt/issue-{i}-some-reasonably-long-name\nHEAD {:040x}\n",
            i
        ));
        match i % 4 {
            0 => s.push_str("detached\n"),
            1 => s.push_str(&format!(
                "branch refs/heads/issue/{i}-some-reasonably-long-name\n"
            )),
            2 => s.push_str(&format!(
                "branch refs/heads/feature/{i}\nlocked in use by another session\n"
            )),
            _ => s.push_str(&format!(
                "branch refs/heads/fix/{i}\nprunable gitdir file points to non-existent location\n"
            )),
        }
        s.push('\n');
    }
    s
}

fn benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("worktree");

    for n in [1usize, 10, 100, 500] {
        let text = porcelain(n);
        group.bench_with_input(BenchmarkId::new("parse porcelain", n), &text, |b, text| {
            b.iter(|| parse_porcelain(text))
        });
    }

    // The derivation itself, which runs once per command and once per violation in a plan.
    let policy = WorktreePolicy::default();
    group.bench_function("derive the canonical root", |b| {
        b.iter(|| {
            policy
                .canonical_root_of(std::path::Path::new("/home/dev"), "example")
                .unwrap()
        })
    });
    group.bench_function("derive a worktree name from a branch", |b| {
        b.iter(|| WorktreeName::derive("feature/some-reasonably-long-branch-name").unwrap())
    });

    group.finish();
}

criterion_group!(worktree, benches);
criterion_main!(worktree);
