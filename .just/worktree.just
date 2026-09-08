# The branch-to-worktree topology the Rust executable derives from git: where every
# linked worktree belongs (<repo>-wt/<branch>), where each one is, and the lifecycle.

# Where this checkout stands in the branch-to-worktree topology (<repo>-wt/<branch>): `just wt`, `just wt list`, `just wt create feature/x`, `just wt migrate --plan`. Every argument passes through to `majordomus worktree`.
[group('worktree')]
wt *args: build
    "{{rust_bin}}" worktree {{args}}

# Start work on a branch: its canonical worktree, created if absent; prints the path to cd into.
[group('worktree')]
wt-create branch *args: build
    "{{rust_bin}}" worktree ensure "{{branch}}" {{args}}

# Bring every misplaced worktree to its canonical path, dirty state included, fingerprint-verified; `just wt-migrate --plan` shows the moves and changes nothing.
[group('worktree')]
wt-migrate *args: build
    "{{rust_bin}}" worktree migrate {{args}}

# Every diagnostic of the topology with its code and remedy; exit 10 when an error stands.
[group('worktree')]
wt-doctor *args: build
    "{{rust_bin}}" worktree doctor {{args}}
