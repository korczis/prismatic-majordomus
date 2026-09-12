# majordomus-covers: none
# The gate of project.worktree-topology, driven against fixture trees.
#
# 96_worktree_topology.sh proves the executable does what the rule says: the container is
# derived from git identity, a misplaced worktree migrates with its work, the hook refuses a
# feature branch committed from the wrong place. What it cannot prove is that the *wiring*
# around that behaviour holds — that the rule has one constant, that the hook is installed,
# that the policy declares it so doctor verifies it on every install, and that no document
# has quietly started describing a different convention.
#
# That is `scripts/ci/worktree-check`, and until this case it was named by nothing in the
# suite. A wiring gate is exactly the kind that rots unnoticed: it passes on the tree it was
# written against for as long as nobody changes anything, and the day somebody does, nobody
# knows whether the red is the tree or the gate.
#
# So each of the gate's findings is written into a fixture here. The section that matters
# most is the last one, because it is the property the whole gate exists for: the constant
# in the crate is the *source* of the convention, not a copy of it. Change the constant and
# every document that still names the old container is wrong — which is what makes this a
# gate rather than eight independent greps that happen to agree today.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/worktree-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }

HOOKDIR=".githooks"

# A tree with the shape the gate reads: the crate's constant, the hook that asks the guard,
# the policy that declares it, the four document kinds that must name the container, an ADR
# that records the convention, and the behavioural case the gate runs.
fixture() {   # fixture <n> -> prints the tree
  F="$T/tree$1"
  rm -rf "$F"
  mkdir -p "$F/apps/majordomus-cli/src/worktree" "$F/$HOOKDIR" "$F/.ai/repo/rules/project" \
           "$F/.ai/repo/providers" "$F/.ai/repo/adrs" "$F/share/providers" "$F/docs" "$F/test/cases"
  printf 'pub const CONTAINER_SUFFIX: &str = "-wt";\n' > "$F/apps/majordomus-cli/src/worktree/path.rs"
  printf 'pub fn guard() {}\n' > "$F/apps/majordomus-cli/src/worktree/guard.rs"
  printf '#!/usr/bin/env bash\nmajordomus-cli worktree guard || exit $?\n' > "$F/$HOOKDIR/pre-commit"
  printf 'enforcement:\n  - id: worktree-guard\n    args: [worktree, guard]\n' > "$F/.ai/repo/policy.yaml"
  printf 'Worktrees live at <repository>-wt/<branch>.\n' > "$F/.ai/repo/rules/project/worktree-topology.v1.md"
  printf 'Worktrees live at <repository>-wt/<branch>.\n' > "$F/docs/WORKTREES.md"
  printf 'The container is <repository>-wt/<branch>.\n' > "$F/.ai/repo/providers/claude.tmpl"
  printf 'The container is <repository>-wt/<branch>.\n' > "$F/share/providers/claude.tmpl"
  printf '# ADR 0044\n\nThe container suffix is "-wt".\n' > "$F/.ai/repo/adrs/0044-worktrees.md"
  printf 'echo ok\n' > "$F/test/cases/96_worktree_topology.sh"
  printf '#!/usr/bin/env bash\nexit 0\n' > "$F/test/run.sh"
  printf '%s' "$F"
}

# ---------------------------------------------------------------- the tree it accepts
F="$(fixture 0)"
expect_exit 0 env MJ_ROOT="$F" "$GATE" --no-suite
expect_grep 'the topology rule has one constant, one guard, one convention in every document'
expect_grep '"-wt" is written once in code, in apps/majordomus-cli/src/worktree/path.rs'

# ---------------------------------------------------------------- 1. one constant
# A second literal in code is a second derivation waiting to disagree: the day the container
# is renamed, one of the two moves and the other quietly keeps building the old path.
F="$(fixture 1)"
printf 'fn container(name: &str) -> String { format!("{}{}", name, "-wt") }\n' \
  > "$F/apps/majordomus-cli/src/worktree/other.rs"
expect_exit 10 env MJ_ROOT="$F" "$GATE" --no-suite
expect_grep '"-wt" is written in code outside apps/majordomus-cli/src/worktree/path.rs'
expect_grep 'apps/majordomus-cli/src/worktree/other.rs:1'

# restored: the same file deriving the path from the constant is the shape the rule asks for
printf 'use super::path::CONTAINER_SUFFIX;\nfn container(n: &str) -> String { format!("{n}{CONTAINER_SUFFIX}") }\n' \
  > "$F/apps/majordomus-cli/src/worktree/other.rs"
expect_exit 0 env MJ_ROOT="$F" "$GATE" --no-suite

# and a comment that explains the convention is not a derivation of it — a gate that could
# not tell prose from code would make the convention undocumentable in its own source
F="$(fixture 2)"
printf '// the container is the checkout sibling named with "-wt"\npub fn note() {}\n' \
  > "$F/apps/majordomus-cli/src/worktree/other.rs"
expect_exit 0 env MJ_ROOT="$F" "$GATE" --no-suite

# ---------------------------------------------------------------- 2. the guard is wired
# A rule the executable enforces and no hook invokes is a rule that runs when somebody
# remembers to ask. The hook must call the guard, and the policy must declare it — the
# second is what lets doctor prove the wiring on a machine this gate will never see.
F="$(fixture 3)"
printf '#!/usr/bin/env bash\nexit 0\n' > "$F/$HOOKDIR/pre-commit"
expect_exit 10 env MJ_ROOT="$F" "$GATE" --no-suite
expect_grep "pre-commit does not invoke 'majordomus-cli worktree guard'"

F="$(fixture 4)"
printf 'enforcement: []\n' > "$F/.ai/repo/policy.yaml"
expect_exit 10 env MJ_ROOT="$F" "$GATE" --no-suite
expect_grep "policy.yaml declares no enforcement entry for 'worktree guard'"

# ---------------------------------------------------------------- 3. one convention
# A document that names another container is not a documentation defect: it is a second
# answer to where a branch belongs, and whoever reads that document builds it there.
F="$(fixture 5)"
printf 'Worktrees live at <repository>.worktrees/<branch>.\n' > "$F/docs/WORKTREES.md"
expect_exit 10 env MJ_ROOT="$F" "$GATE" --no-suite
expect_grep 'docs/WORKTREES.md does not name the container with the suffix -wt'

# a provider template is a document too: it is the text every worker of that provider reads
F="$(fixture 6)"
printf 'The container is <repository>.worktrees/<branch>.\n' > "$F/share/providers/claude.tmpl"
expect_exit 10 env MJ_ROOT="$F" "$GATE" --no-suite
expect_grep 'share/providers/claude.tmpl does not name the container with the suffix -wt'

# a document that is gone is a finding of its own, named as missing rather than as silent
F="$(fixture 7)"
rm -f "$F/.ai/repo/rules/project/worktree-topology.v1.md"
expect_exit 10 env MJ_ROOT="$F" "$GATE" --no-suite
expect_grep 'worktree-topology.v1.md is missing'

# and the decision has to be recorded somewhere a reader can find the reasoning
F="$(fixture 8)"
rm -f "$F/.ai/repo/adrs/0044-worktrees.md"
expect_exit 10 env MJ_ROOT="$F" "$GATE" --no-suite
expect_grep 'no ADR under .ai/repo/adrs names the -wt convention'

# ---------------------------------------------------------------- 4. the behavioural case
# The gate's last check runs the case that proves the executable behaves. Without it the
# gate would hold together the wiring around a behaviour nobody had measured.
F="$(fixture 9)"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep 'test/cases/96_worktree_topology.sh passes'

F="$(fixture 10)"
printf '#!/usr/bin/env bash\nexit 1\n' > "$F/test/run.sh"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'test/cases/96_worktree_topology.sh fails'

F="$(fixture 11)"
rm -f "$F/test/cases/96_worktree_topology.sh"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'test/cases/96_worktree_topology.sh is missing'

# ---------------------------------------------------------------- the constant is the source
# The property the whole gate exists for. Rename the container in the crate and leave the
# documents alone: every document is now wrong, and the gate says so about each of them by
# name. This is what distinguishes a gate from eight greps that agree today — the greps have
# no source, so renaming the convention would make all eight pass while the tree disagreed
# with itself.
F="$(fixture 12)"
printf 'pub const CONTAINER_SUFFIX: &str = "-trees";\n' > "$F/apps/majordomus-cli/src/worktree/path.rs"
expect_exit 10 env MJ_ROOT="$F" "$GATE" --no-suite
expect_grep '"-trees" is written once in code'
expect_grep 'worktree-topology.v1.md does not name the container with the suffix -trees'
expect_grep 'docs/WORKTREES.md does not name the container with the suffix -trees'
expect_grep 'providers/claude.tmpl does not name the container with the suffix -trees'
expect_grep 'no ADR under .ai/repo/adrs names the -trees convention'
expect_grep '5 finding\(s\); the rule is project.worktree-topology'

# and renaming the convention everywhere in one commit is accepted, which is the whole point
# of having a source: the change is possible, it is just not possible by halves
F="$(fixture 13)"
printf 'pub const CONTAINER_SUFFIX: &str = "-trees";\n' > "$F/apps/majordomus-cli/src/worktree/path.rs"
printf 'Worktrees live at <repository>-trees/<branch>.\n' > "$F/.ai/repo/rules/project/worktree-topology.v1.md"
printf 'Worktrees live at <repository>-trees/<branch>.\n' > "$F/docs/WORKTREES.md"
printf 'The container is <repository>-trees/<branch>.\n' > "$F/.ai/repo/providers/claude.tmpl"
printf 'The container is <repository>-trees/<branch>.\n' > "$F/share/providers/claude.tmpl"
printf '# ADR 0044\n\nThe container suffix is "-trees".\n' > "$F/.ai/repo/adrs/0044-worktrees.md"
expect_exit 0 env MJ_ROOT="$F" "$GATE" --no-suite

# ---------------------------------------------------------------- an unusable tree
# No constant is not "clean": a gate that cannot find the source of the convention knows
# nothing about the tree, and 12 says so rather than reporting a verdict it does not have.
F="$(fixture 14)"
printf 'pub fn nothing() {}\n' > "$F/apps/majordomus-cli/src/worktree/path.rs"
expect_exit 12 env MJ_ROOT="$F" "$GATE" --no-suite
expect_grep 'CONTAINER_SUFFIX is not declared in apps/majordomus-cli/src/worktree/path.rs'

F="$(fixture 15)"
rm -f "$F/apps/majordomus-cli/src/worktree/path.rs"
expect_exit 12 env MJ_ROOT="$F" "$GATE" --no-suite
expect_grep 'apps/majordomus-cli/src/worktree/path.rs is missing; nothing to hold the rule to'

expect_exit 2 env MJ_ROOT="$F" "$GATE" --quick
expect_grep 'unknown option --quick'

echo "    the topology gate holds one constant, one guard and one convention, and moves them together"
