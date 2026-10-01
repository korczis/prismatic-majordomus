# majordomus-covers: none
# Every active use case is named by at least one feature's `use_cases:`.
#
# A homepage panel routes to a use case, and a feature is what carries the use case to its
# rules, its claims and the tests behind them. An active use case that no feature names is a
# panel whose chain stops at a description: nothing on the feature side answers for it. So
# this case asks the two programs that read the two halves — the Rust executable for the
# features, the shell tool for the use cases — and fails naming every active use case outside
# the union of the features' `use_cases:`.
#
# The check is written once, as a function over a repository, and run twice: on this
# repository, where it must find nothing, and on a disposable one holding one feature and
# use cases owned, unowned and in draft, where it must name exactly the unowned active one.
# A list it reads empty, or a command it runs failing, is a failure and never a pass: a
# check that read nothing has not checked anything.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq is not installed; the answers cannot be read"; exit 1; }
MJB="$(rust_bin)" || rust_bin_exit $?
[ -x "$MJB" ] || { echo "    the build produced no executable at $MJB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj503.XXXXXX")"; trap 'rm -rf "$S"' EXIT

# unowned_use_cases <repository>
# Prints, one per line and sorted, every active use case of the repository that no feature
# names. Returns 1, saying why on stderr, when a command fails or any of the three lists it
# collects (the features, the use cases they name, the active use cases) is empty.
#
# It is called in a command substitution, where `set -e` does not reach, and the suite runs
# without pipefail: every command's status is therefore taken where it runs, and its output
# is held in a variable before jq reads it.
unowned_use_cases() {
  local repo="$1" out features owned="" active f id gap=""
  out="$("$MJB" product --repo "$repo" list --format json 2>"$S/err")" \
    || { echo "    product list failed in $repo: $(cat "$S/err")" >&2; return 1; }
  features="$(jq -r '.features[].id' <<<"$out")" \
    || { echo "    product list answered no features array in $repo" >&2; return 1; }
  [ -n "$features" ] || { echo "    product list read no feature in $repo" >&2; return 1; }
  for f in $features; do
    out="$("$MJB" product --repo "$repo" show "$f" --format json 2>"$S/err")" \
      || { echo "    product show $f failed in $repo: $(cat "$S/err")" >&2; return 1; }
    out="$(jq -r '.use_cases[]' <<<"$out")" \
      || { echo "    product show $f answered no use_cases array in $repo" >&2; return 1; }
    owned="$owned $(printf '%s' "$out" | tr '\n' ' ')"
  done
  [ -n "$(printf '%s' "$owned" | tr -d ' ')" ] \
    || { echo "    no feature of $repo names a use case" >&2; return 1; }
  out="$("$MJ" --repo "$repo" usecase list --json 2>"$S/err")" \
    || { echo "    usecase list failed in $repo: $(cat "$S/err")" >&2; return 1; }
  active="$(jq -r '.use_cases[] | select(.status == "active") | .id' <<<"$out")" \
    || { echo "    usecase list answered no use_cases array in $repo" >&2; return 1; }
  [ -n "$active" ] || { echo "    usecase list read no active use case in $repo" >&2; return 1; }
  for id in $active; do
    case " $owned " in *" $id "*) ;; *) gap="$gap$id"$'\n' ;; esac
  done
  printf '%s' "$gap" | LC_ALL=C sort
}

# ---------------------------------------------------------------- this repository
gap="$(unowned_use_cases "$ROOT")" || { echo "    the check could not read this repository"; exit 1; }
if [ -n "$gap" ]; then
  echo "    active use case(s) that no feature names under use_cases:"
  printf '%s\n' "$gap" | sed 's/^/      /'
  exit 1
fi

# ---------------------------------------------------------------- a repository that fails it
# The same function over a repository built to fail it: one feature naming one active use
# case, a second active use case nobody names, and a draft nobody names. It must name the
# second and only the second: the draft is not owed a feature, and the owned one is owned.
"$MJ" init >/dev/null
mkdir -p .ai/repo/features
cat > .ai/repo/features/a-probe-feature.md <<'MD'
---
schema: feature/v1
id: a-probe-feature
kind: feature
title: 'The feature this case adds'
short_title: 'The probe'
headline: 'One feature, naming one of the use cases beside it.'
summary: 'A feature that exists so that one use case has an owner and another does not.'
status: stable
weight: 10
featured: true
modules: [repository]
kinds: [rule]
docs: [.ai/repo/workflows/task-lifecycle.md]
use_cases: [an-owned-use-case]
tags: [probe]
---

## What it does

Names the use case this case owns.

## What it does not do

Name the other one.
MD
use_case() {   # id status
  cat > ".ai/repo/use-cases/$1.md" <<MD
---
id: $1
kind: use-case
title: 'See which version runs ($1)'
summary: 'Print the version.'
category: adoption
status: $2
target: advisory
actors: [operator]
difficulty: basic
commands: [version]
doctrines: []
claims: []
responsibilities: []
applications: []
---

# Situation

Which tool is this?

# Scenario

\`\`\`yaml
setup: bare
given:
  - 'nothing installed'
steps:
  - id: print
    run: ['version']
    expect:
      exit: 0
then:
  - 'the version needs no repository'
\`\`\`

# Outcome

The version, from anywhere.
MD
}
use_case an-owned-use-case active
use_case an-unowned-use-case active
use_case an-unowned-draft draft
git add -A >/dev/null && git commit -qm "a feature and three use cases"

# the fixture is a valid instance of both kinds, so what the check names is not a defect of
# the fixture that either validator would have refused first
expect_exit 0 "$MJB" product --repo "$T" validate
expect_exit 0 "$MJ" --repo "$T" usecase validate

gap="$(unowned_use_cases "$T")" || { echo "    the check could not read the fixture"; exit 1; }
[ "$gap" = "an-unowned-use-case" ] \
  || { echo "    expected the check to name exactly an-unowned-use-case; it named:"; printf '%s\n' "$gap" | sed 's/^/      /'; exit 1; }

# ...and naming it from the feature closes the gap
sed -i.bak 's/^use_cases: \[an-owned-use-case\]$/use_cases: [an-owned-use-case, an-unowned-use-case]/' \
  .ai/repo/features/a-probe-feature.md
rm -f .ai/repo/features/a-probe-feature.md.bak
git add -A >/dev/null && git commit -qm "the feature names both"
gap="$(unowned_use_cases "$T")" || { echo "    the check could not read the fixture"; exit 1; }
[ -z "$gap" ] || { echo "    the feature names both and the check still named: $gap"; exit 1; }

# ---------------------------------------------------------------- reading nothing is not a pass
# With no feature at all every list after the first is empty, and the one answer the check
# must not give is an empty gap: that would read as every use case owned.
git rm -q .ai/repo/features/a-probe-feature.md && git commit -qm "no feature"
rc=0; gap="$(unowned_use_cases "$T" 2>"$S/why")" || rc=$?
[ "$rc" != 0 ] || { echo "    with no feature the check passed, naming '$gap'"; exit 1; }
grep -q 'read no feature' "$S/why" \
  || { echo "    with no feature the check failed for another reason:"; sed 's/^/      /' "$S/why"; exit 1; }
