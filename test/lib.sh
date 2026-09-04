# Sourced by every test case. Provides expect_exit / expect_grep / expect_no_grep.
LAST_OUT=""
expect_exit() {
  local want="$1"; shift
  local got=0
  LAST_OUT="$("$@" 2>&1)" || got=$?
  if [ "$got" != "$want" ]; then
    printf '    expected exit %s, got %s from: %s\n    output: %s\n' "$want" "$got" "$*" "$LAST_OUT"
    return 1
  fi
}
expect_grep() {
  local pat="$1" src="${2:--}"
  if [ "$src" = "-" ]; then grep -qE -- "$pat" <<<"$LAST_OUT" || { printf '    expected /%s/ in output:\n%s\n' "$pat" "$LAST_OUT"; return 1; }
  else grep -qE -- "$pat" "$src" || { printf '    expected /%s/ in %s\n' "$pat" "$src"; return 1; }; fi
}
expect_no_grep() {
  local pat="$1" src="${2:--}"
  if [ "$src" = "-" ]; then grep -qE -- "$pat" <<<"$LAST_OUT" && { printf '    did not expect /%s/ in output:\n%s\n' "$pat" "$LAST_OUT"; return 1; }
  else grep -qE -- "$pat" "$src" && { printf '    did not expect /%s/ in %s\n' "$pat" "$src"; return 1; }; fi
  return 0
}
# octal permission bits of a file, GNU stat first (BSD stat has no -c and fails), then BSD
file_mode() { stat -c %a "$1" 2>/dev/null || stat -f %Lp "$1"; }

# ---------------------------------------------------------------- project model fixtures
# A canonical project model small enough to reason about, built in the disposable repository
# the case runs in. Cases append extra fields to the files these produce.
pj_init() {
  mkdir -p .majordomus/project/milestones .majordomus/project/issues
  cat > .majordomus/project/project.yaml <<'Y'
schema_version: 1
name: Fixture
repository: example/fixture
default_branch: master
Y
}
# pj_milestone ID [ORDER]
pj_milestone() {
  cat > ".majordomus/project/milestones/$1.yaml" <<Y
id: $1
title: Milestone $1
slug: milestone-$1
order: ${2:-0}
priority: p1
problem: "A problem worth solving."
outcome: "The outcome once it is solved."
acceptance_criteria:
  - The outcome is reached
validation:
  - true
evidence_required:
  - proof
Y
}
# pj_issue ID MILESTONE [DEP ...]   — a minimal valid issue; extra fields are appended by the case
pj_issue() {
  local id="$1" m="$2"; shift 2
  { cat <<Y
id: $id
milestone: $m
title: Issue $id
slug: issue-$id
priority: p1
profile: implementation
objective: "Do the bounded piece of work called $id."
scope:
  - src/$id
acceptance_criteria:
  - The work is done
validation:
  - true
evidence_required:
  - proof
Y
    if [ $# -gt 0 ]; then printf 'depends_on:\n'; for d in "$@"; do printf -- '  - %s\n' "$d"; done; fi
  } > ".majordomus/project/issues/$id.yaml"
}
# pj_status ID  — the derived status of one issue, from the tool
pj_status() { "$MJ" plan list | awk -v i="$1" '$1==i{print $2}'; }

# Build a fixture copy of the repository into $1: the runtime the tool needs to run, plus
# every canonical input the site generator declares, plus any extra paths given after $1.
#
# The input list comes from `generate-site-data --inputs`, not from a list written here. Six
# fixtures used to carry their own copy list, and when the generator gained a new canonical
# input every one of them went stale at once — five cases failed with "canonical input
# missing" on a repository that had the file. A fixture that derives its inputs cannot drift
# from the thing it is a fixture for.
fixture_repo() {
  local dst="$1" p; shift
  mkdir -p "$dst"
  cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$ROOT/scripts" "$dst/"
  for p in $("$ROOT/scripts/generate-site-data" --inputs); do
    mkdir -p "$dst/$(dirname "$p")"
    cp "$ROOT/$p" "$dst/$p"
  done
  for p in "$@"; do
    [ -e "$ROOT/$p" ] || continue
    mkdir -p "$dst/$(dirname "$p")"
    cp -R "$ROOT/$p" "$dst/$p"
  done
}
