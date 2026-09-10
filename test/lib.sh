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
expect_file() {
  [ -f "$1" ] || { printf '    expected the file %s, which the run did not produce\n' "$1"; return 1; }
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
# Run a command whose output the caller wants but whose noise it does not, and say what it
# said if it fails.
#
# The idiom this replaces is `cmd 2>/dev/null > file`. Under `bash -eu` a non-zero exit
# there ends the case having printed nothing anywhere: no message, no assertion, a zero-byte
# log and a bare FAIL. Three commands in 76_capabilities_projections and the zola build in
# 95_skills were written that way, and each of them failed exactly like that. `rust_bin`
# above already does the right thing — stderr to a file, `cat` it on failure — and this is
# that, reusable.
#
#   run_quiet "$S/list.err" "$RB" capabilities list --format json > "$S/list.json"
#
# The report goes to stderr, not stdout. Every caller redirects stdout into the file it
# wants the command's output in, so a diagnostic written to stdout lands in that file
# instead of the log — the same silence one layer along. The two `>&2` are load-bearing.
run_quiet() {
  local err="$1"; shift
  "$@" 2> "$err" || {
    local rc=$?
    printf '    %s failed (exit %s):\n' "$1" "$rc" >&2
    sed 's/^/    | /' "$err" >&2
    return 1
  }
}

# octal permission bits of a file, GNU stat first (BSD stat has no -c and fails), then BSD
file_mode() { stat -c %a "$1" 2>/dev/null || stat -f %Lp "$1"; }

# The SHA-256 of a file, for a case that must prove a file did not change rather than that a
# command said it did not. sha256sum on Linux, shasum on macOS.
sha256_of_file() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

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
  # Where cargo put it, not where it would have without CARGO_TARGET_DIR — which is how
  # several worktrees of this repository share one build directory, and composing the path
  # under the crate then names a file that was never written. The variable is read rather
  # than `cargo metadata` asked (lib/rust_bin.sh's mj_cargo_target_dir will ask, for callers
  # that want a .cargo/config.toml honoured too): this runs once per case, and a suite of
  # forty Rust cases should not spend forty processes learning that a variable is unset.
  printf '%s' "${CARGO_TARGET_DIR:-$ROOT/apps/majordomus-cli/target}/debug/majordomus"
}
# The line a Rust case runs first: the executable into RB, or the skip/failure exit.
#   RB="$(rust_bin)" || rust_bin_exit $?
rust_bin_exit() { [ "$1" = 3 ] && { echo "    skip: no cargo and no MAJORDOMUS_BIN"; exit 0; }; exit 1; }

# The whole workflow declaration of this repository, written to a file a case can grep.
#
# The root justfile imports one file per bounded context, so a case that reads only the root
# file is reading a fragment of the declaration and will report a recipe missing the day it
# is moved rather than the day it is removed. The generated bridge is not read: it is not
# tracked, it is a projection of the command graph, and a case asserting what it contains
# would be asserting what `majordomus commands bridge` writes rather than what this
# repository declares.
#   JF="$(just_declaration)"
just_declaration() {
  local out
  out="$(mktemp "${TMPDIR:-/tmp}/mj.just.XXXXXX")"
  cat "$ROOT/justfile" "$ROOT"/.just/*.just > "$out" 2>/dev/null
  printf '%s' "$out"
}

# restore the seeded policy and profiles from the skeleton after a case mutated them; the
# files belong to the repository after init, so init itself never rewrites them
reset_policy() {
  cp "$ROOT/share/skeleton/policy.yaml" .ai/repo/policy.yaml
  cp "$ROOT"/share/skeleton/profiles/*.yaml .ai/repo/profiles/
}

# ---------------------------------------------------------------- project model fixtures
# A canonical project model small enough to reason about, built in the disposable repository
# the case runs in. Cases append extra fields to the files these produce.
#
# `validation: - "true"` is quoted on purpose. Unquoted it is a YAML boolean, which the
# flattener renders as the same text the shell engine sees but which
# `share/schemas/majordomus.issue/v1` refuses as "not of type string" — so the Rust
# executable dropped every fixture record and the two engines could not be compared on one.
# The quotes cost the shell nothing and make the fixture valid under the repository's own
# schema, which is what a fixture ought to be.
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
  - "true"
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
  - "true"
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
  mkdir -p "$dst/site"; cp -R "$ROOT/site/templates" "$dst/site/templates"
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
    for p in README.md policy.yaml scope.yaml knowledge rules profiles prompts workflows use-cases applications adrs why features; do
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
  [ -e "$dst/docs/generated/artifacts.json" ] || { mkdir -p "$dst/docs/generated"; cp "$ROOT/docs/generated/artifacts.json" "$dst/docs/generated/"; }
  # Every file the templates load. `--inputs` names the site generator's own canonical
  # inputs, which is not the same set: site/data/registry/registry.json and
  # distribution.json are written by `majordomus generate`, so the generator does not call
  # them inputs and a fixture built from that list alone had two of the four files the
  # templates read. Zola then failed on the first page that loaded one, which is how
  # 95_skills reported "zola could not build the fixture site".
  #
  # Read out of the templates rather than listed here, for the reason the input list is:
  # a fixture that derives what it carries cannot drift from the thing it is a fixture for.
  # data/generated/ is skipped: that is the generator's own output directory, and the
  # fixture must produce it rather than inherit it, or a case would be checking this
  # repository's data instead of what the run under test wrote.
  for p in $(grep -rhoE 'load_data\(path="[^"]+"' "$ROOT/site/templates" 2>/dev/null \
             | sed 's/.*path="//; s/"$//' | sort -u); do
    case "$p" in data/generated/*) continue ;; esac
    [ -f "$ROOT/site/$p" ] && [ ! -e "$dst/site/$p" ] || continue
    mkdir -p "$dst/site/$(dirname "$p")"
    cp "$ROOT/site/$p" "$dst/site/$p"
  done
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
  # A directory of the layer owes a context document (ADR 0011). A fixture that copied a
  # section's files through --inputs without the section's own README would be a tree the
  # tool refuses, so every directory that arrived here brings the contract it has upstream.
  ( cd "$dst" && find .ai -type d -not -path '.ai/local*' -print 2>/dev/null ) | LC_ALL=C sort | while IFS= read -r d; do
    if [ ! -f "$dst/$d/README.md" ] && [ -f "$ROOT/$d/README.md" ]; then
      cp "$ROOT/$d/README.md" "$dst/$d/README.md"
    fi
    :                       # the loop body never ends on a false test: `set -e` would stop it
  done
}

# A local HTTP server over a directory, for the cases that must exercise a real download
# without reaching the network. Sets HTTP_PORT, HTTP_BASE and HTTP_PID; the caller stops it
# with `stop_http`. Answers 1 when no server this harness knows how to start is present, so
# a case skips rather than fails on a machine without python or node.
start_http() {
  local dir="$1" tries=0 port
  HTTP_PORT=""; HTTP_BASE=""; HTTP_PID=""
  local runner=""
  if command -v python3 >/dev/null 2>&1; then runner=python3
  elif command -v node >/dev/null 2>&1; then runner=node
  else return 1; fi
  while [ "$tries" -lt 20 ]; do
    port=$(( 20000 + ((( $$ + tries * 977 ) * 31 ) % 20000) ))
    case "$runner" in
      python3) ( cd "$dir" && exec python3 -m http.server "$port" --bind 127.0.0.1 ) >/dev/null 2>&1 &
        ;;
      node) node -e '
          const http=require("http"),fs=require("fs"),p=require("path");
          const root=process.argv[1], port=Number(process.argv[2]);
          http.createServer((q,s)=>{
            const f=p.join(root, decodeURIComponent(q.url.split("?")[0]));
            if(!p.resolve(f).startsWith(p.resolve(root))){s.writeHead(403);return s.end();}
            fs.readFile(f,(e,d)=>{ if(e){s.writeHead(404);return s.end("not found");} s.writeHead(200);s.end(d); });
          }).listen(port,"127.0.0.1");
        ' "$dir" "$port" >/dev/null 2>&1 &
        ;;
    esac
    HTTP_PID=$!
    local waited=0
    while [ "$waited" -lt 40 ]; do
      code="$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$port/" 2>/dev/null || true)"
      if [ -n "$code" ] && [ "$code" != 000 ]; then
        HTTP_PORT="$port"; HTTP_BASE="http://127.0.0.1:$port"
        export HTTP_PORT HTTP_BASE HTTP_PID
        return 0
      fi
      kill -0 "$HTTP_PID" 2>/dev/null || break
      sleep 0.25; waited=$((waited + 1))
    done
    kill "$HTTP_PID" 2>/dev/null || true
    tries=$((tries + 1))
  done
  HTTP_PID=""
  return 1
}

stop_http() { [ -n "${HTTP_PID:-}" ] && kill "$HTTP_PID" 2>/dev/null; HTTP_PID=""; return 0; }
