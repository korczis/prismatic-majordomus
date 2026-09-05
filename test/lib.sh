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

# The Rust executable a case drives. MAJORDOMUS_BIN names a prebuilt one (CI hands the
# artifact of its rust job to a later job this way, a person points at a release build);
# without it the crate is built once, debug profile, and the target path is printed.
# Prints the path on stdout and any complaint on stderr (the caller captures stdout).
# Returns 3 when there is neither cargo nor MAJORDOMUS_BIN, which is the case's cue to skip
# as the Rust cases always have, and 1 when the build fails or the named executable does
# not exist, which is a failure and never a skip.
rust_bin() {
  local manifest="$ROOT/apps/majordomus-cli/Cargo.toml" log
  if [ -n "${MAJORDOMUS_BIN:-}" ]; then
    [ -x "$MAJORDOMUS_BIN" ] || { echo "    MAJORDOMUS_BIN is not an executable: $MAJORDOMUS_BIN" >&2; return 1; }
    printf '%s' "$MAJORDOMUS_BIN"; return 0
  fi
  command -v cargo >/dev/null 2>&1 || return 3
  log="$(mktemp "${TMPDIR:-/tmp}/mj-rust-bin.XXXXXX")"
  RUSTFLAGS='' cargo build -q --manifest-path "$manifest" 2>"$log" || { cat "$log" >&2; rm -f "$log"; echo "    cargo build failed" >&2; return 1; }
  rm -f "$log"
  printf '%s' "$ROOT/apps/majordomus-cli/target/debug/majordomus"
}
# The line a Rust case runs first: the executable into RB, or the skip/failure exit.
#   RB="$(rust_bin)" || rust_bin_exit $?
rust_bin_exit() { [ "$1" = 3 ] && { echo "    skip: no cargo and no MAJORDOMUS_BIN"; exit 0; }; exit 1; }

# restore the seeded policy and profiles from the skeleton after a case mutated them; the
# files belong to the repository after init, so init itself never rewrites them
reset_policy() {
  cp "$ROOT/share/skeleton/policy.yaml" .ai/repo/policy.yaml
  cp "$ROOT"/share/skeleton/profiles/*.yaml .ai/repo/profiles/
}

# ---------------------------------------------------------------- project model fixtures
# A canonical project model small enough to reason about, built in the disposable repository
# the case runs in. Cases append extra fields to the files these produce.
pj_init() {
  mkdir -p .ai/repo/project/milestones .ai/repo/project/issues
  cat > .ai/repo/project/project.yaml <<'Y'
schema_version: 1
name: Fixture
repository: example/fixture
default_branch: master
Y
}
# pj_milestone ID [ORDER]
pj_milestone() {
  cat > ".ai/repo/project/milestones/$1.yaml" <<Y
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
  } > ".ai/repo/project/issues/$id.yaml"
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
  # the generator validates and runs the repository's use cases through the tool, which
  # needs the layer (manifest, policy, rules, sources), the fixtures the scenarios prepare
  # repositories from, and the executable's registry the MCP tools resolve against
  if [ ! -f "$dst/.ai/manifest.yaml" ]; then
    mkdir -p "$dst/.ai/repo"; cp "$ROOT/.ai/README.md" "$ROOT/.ai/manifest.yaml" "$dst/.ai/"
    for p in README.md policy.yaml scope.yaml knowledge rules profiles prompts workflows use-cases applications adrs; do
      [ -e "$ROOT/.ai/repo/$p" ] && [ ! -e "$dst/.ai/repo/$p" ] && cp -R "$ROOT/.ai/repo/$p" "$dst/.ai/repo/$p"
    done
    for p in "$ROOT"/.ai/repo/use-cases/* "$ROOT"/.ai/repo/applications/*; do
      [ -e "$dst/.ai/repo/${p#"$ROOT"/.ai/repo/}" ] || cp "$p" "$dst/.ai/repo/${p#"$ROOT"/.ai/repo/}"
    done
  fi
  [ -e "$dst/test/lib.sh" ] || { mkdir -p "$dst/test"; cp "$ROOT/test/lib.sh" "$dst/test/lib.sh"; }
  [ -e "$dst/test/fixtures" ] || { mkdir -p "$dst/test"; cp -R "$ROOT/test/fixtures" "$dst/test/fixtures"; }
  [ -e "$dst/docs/generated/registry.json" ] || { mkdir -p "$dst/docs/generated"; cp "$ROOT/docs/generated/registry.json" "$dst/docs/generated/"; }
  [ -e "$dst/docs/generated/cli.json" ] || { mkdir -p "$dst/docs/generated"; cp "$ROOT/docs/generated/cli.json" "$dst/docs/generated/"; }
  # every path a claim names must resolve where the generator runs, so the fixture carries
  # them too, read from the matrix rather than listed here: a claim implemented outside the
  # trees copied above (the Rust executable under apps/) is otherwise "missing". After the
  # caller's trees, so that a tree copied whole is never pre-created and copied into itself.
  for p in $(awk '/^    (source|implementation|test): /{print $2}' "$ROOT/docs/CLAIMS.yaml" | tr -d "'" | grep -v '^-$' | sort -u); do
    [ -f "$ROOT/$p" ] && [ ! -e "$dst/$p" ] || continue
    mkdir -p "$dst/$(dirname "$p")"
    cp "$ROOT/$p" "$dst/$p"
  done
  # and the Rust file each builtin capability was composed in: the registry manifest names
  # them and the generator refuses a manifest that names a file the tree does not have
  if [ -f "$ROOT/docs/generated/registry.json" ] && command -v jq >/dev/null 2>&1; then
    for p in $(jq -r '[.capabilities[].source_path] | unique[]' "$ROOT/docs/generated/registry.json"); do
      [ -f "$ROOT/$p" ] && [ ! -e "$dst/$p" ] || continue
      mkdir -p "$dst/$(dirname "$p")"
      cp "$ROOT/$p" "$dst/$p"
    done
  fi
  # and the file the command line is declared in, for the same reason: the command-line
  # document names it as the source every generated page links, and the generator refuses a
  # document that names a file the tree does not have. Read from the document, never listed.
  if [ -f "$dst/docs/generated/cli.json" ] && command -v jq >/dev/null 2>&1; then
    p="$(jq -r '.source' "$dst/docs/generated/cli.json")"
    if [ -f "$ROOT/$p" ] && [ ! -e "$dst/$p" ]; then mkdir -p "$dst/$(dirname "$p")"; cp "$ROOT/$p" "$dst/$p"; fi
  fi
}
