# Sourced by every test case. Provides expect_exit / expect_grep / expect_no_grep.

# A case runs only in the fixture the runner made for it. Every case writes into the
# directory it starts in (`printf ... > docs/CLAIMS.yaml`, `git add -A`, `git commit`),
# because test/run.sh starts it in $T, a disposable repository of its own. Started by hand
# from a checkout, it writes into that checkout. On 2026-09-28 the fixture of case 506,
# run with a checkout as its working directory, emptied that checkout's claims matrix,
# replaced its toolchain pin, added two stub files and staged all four; only the checkout's
# own pre-commit hook stopped the `git commit -qm fixture` behind them.
#
# So before a case's first line runs: $T and $ROOT are set, the case stands in $T, and $T
# is a fixture rather than a checkout of this repository, so it is neither $ROOT nor inside
# it, and it has no test/run.sh. Setting T to the checkout is the first thing a caller tries
# when this refuses, and it is refused too. The other callers that load this library
# themselves (scripts/shell-coverage, case 35, the harness of case 413, and the use-case
# runner in lib/usecase.sh, whose scenario setups use its helpers) set T and stand in it,
# as test/run.sh does. Case 94 proves the refusal. The cost is two subshells.
mj_case_in_its_fixture() {
  local here fixture root
  [ -n "${T:-}" ] && [ -n "${ROOT:-}" ] || return 1
  here="$(pwd -P)" || return 1
  fixture="$(cd "$T" 2>/dev/null && pwd -P)" || return 1
  root="$(cd "$ROOT" 2>/dev/null && pwd -P)" || return 1
  [ "$here" = "$fixture" ] || return 1
  case "$fixture/" in "$root/"*) return 1 ;; esac
  [ ! -e "$fixture/test/run.sh" ]
}
mj_case_in_its_fixture || {
  printf '    run this case through test/run.sh: a case writes its fixture into the directory it starts in, and %s is not a fixture test/run.sh made (T=%s)\n' \
    "$(pwd)" "${T:-unset}" >&2
  exit 1
}

# No git housekeeping outlives a case.
#
# After a commit, a fetch or a merge, git may start `maintenance run --auto` detached: a
# process of its own that goes on writing into .git after the command that started it has
# returned. A case that made a repository removes it in its EXIT trap, and a `rm -rf` that
# meets a directory still being written to fails with "Directory not empty"; under `set -e`
# that failure is the case's status. Case 489, which clones this whole repository, commits
# and pushes in the clone, failed exactly so on 2026-10-08, with every assertion it makes
# already passed.
#
# So every git a case runs, and every git the tool under test runs for it, is told to start
# none: gc.auto=0 and maintenance.auto=false, through the environment, because a fixture's
# repositories are made by the case and by the tool alike and no one config file reaches
# them all. The two entries are appended to whatever the environment already carries (a
# runner that names an identity this way keeps it), and appended once: a harness that
# sources this library again adds nothing. Case 995 holds all three.
if [ -z "${MJ_CASE_GIT_IS_QUIET:-}" ]; then
  mj_case_git_n="${GIT_CONFIG_COUNT:-0}"
  case "$mj_case_git_n" in ''|*[!0-9]*) mj_case_git_n=0 ;; esac
  export "GIT_CONFIG_KEY_${mj_case_git_n}=gc.auto" "GIT_CONFIG_VALUE_${mj_case_git_n}=0"
  export "GIT_CONFIG_KEY_$((mj_case_git_n + 1))=maintenance.auto" "GIT_CONFIG_VALUE_$((mj_case_git_n + 1))=false"
  GIT_CONFIG_COUNT=$((mj_case_git_n + 2)); export GIT_CONFIG_COUNT
  MJ_CASE_GIT_IS_QUIET=1; export MJ_CASE_GIT_IS_QUIET
  unset mj_case_git_n
fi

LAST_OUT=""

# The exit status a case uses to say it declined to run. It is not 0 and it is not 1: the
# runner maps it to SKIP, and every other non-zero status stays a failure.
#
# A case that cannot meet a precondition -- no jq, no zola, no built executable -- used to
# say so with `echo "    skip: ..."; exit 0`, and exit 0 is the word the runner writes for a
# case that ran and asserted everything it was written to assert. The two then became one
# `ok` in the TSV, `majordomus evidence record` entered that `ok` into the ledger, and the
# claim the case proves read as supported on the strength of a run that proved nothing. The
# ledger already had the word for this (`Outcome::Skip`, which does not prove); what was
# missing was a runner that could ever write it.
#
#   command -v jq >/dev/null 2>&1 || skip "no jq"
#
# The status alone is not the declaration. A case runs under `set -e`, so any command that
# fails with the same status ends the case with it -- `jq -e` exits 4 when it produced no
# result, which is what it does on the empty output of a command that broke -- and a runner
# that read 4 as a skip would record that failure as a case that declined. So `skip` also
# writes the file the runner names in MJ_SKIP_MARK, and the runner reads a skip only when
# both are there; a 4 nobody declared stays a failure.
MJ_SKIP_STATUS=4
skip() {
  printf '    skip: %s\n' "$*"
  if [ -n "${MJ_SKIP_MARK:-}" ]; then printf '%s\n' "$*" > "$MJ_SKIP_MARK"; fi
  exit "$MJ_SKIP_STATUS"
}
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

# SHA-256 for the cases, from the tool's own lib/sha256.sh: mj_sha256sum (files, or stdin),
# mj_sha256_xargs (paths on stdin) and mj_sha256_hex. sha256sum, else openssl; never shasum,
# which on macOS is a Perl script (case 575).
# shellcheck source=../lib/sha256.sh
. "$ROOT/lib/sha256.sh"

# The SHA-256 of a file, for a case that must prove a file did not change rather than that a
# command said it did not.
sha256_of_file() { mj_sha256_hex "$1"; }

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
    # Not there. The words matter more than the exit code: this failure is about the
    # environment the suite is running in and not about the code the case was written to
    # measure, and a reader who takes it for the second spends an afternoon on a branch that
    # was never broken. lib/rust_bin.sh says it, once, for every caller that resolves one.
    if [ ! -x "$MAJORDOMUS_BIN" ]; then
      . "$ROOT/lib/rust_bin.sh"
      mj_rust_bin_missing "$ROOT" "$MAJORDOMUS_BIN" '    '
      return 1
    fi
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
rust_bin_exit() { [ "$1" = 3 ] && skip "no cargo and no MAJORDOMUS_BIN"; exit 1; }

# The end of a case that cannot measure its subject here, and why.
#
# The harness has no skip state: a case that exits 0 is reported `ok`, so a skip is a pass
# that measured nothing, and the machine that lacks every tool shows the greenest suite
# (cases 12 and 33 each read as fixed that way while they were red). Two rules follow. Under
# CI (CI=true, which GitHub Actions sets) the job installs every tool the suite needs, so an
# absence there is the job's setup failing and the case fails saying so — the path that
# protects master never reports `ok` for a case that did not run. Anywhere else the case ends
# with a line that names what was not measured, so a reader who greps a log for `skip:` finds
# every pass that proved nothing.
#   command -v zola >/dev/null || skip_case "zola is absent, so the site build was not measured"
skip_case() {
  if [ "${CI:-}" = true ]; then
    printf '    %s; under CI every tool the suite needs is installed, so this is a failure, not a skip\n' "$1"
    exit 1
  fi
  printf '    skip: %s\n' "$1"
  exit 0
}

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
             | sed 's/.*path="//; s/"$//' | LC_ALL=C sort -u); do
    case "$p" in data/generated/*) continue ;; esac
    [ -f "$ROOT/site/$p" ] && [ ! -e "$dst/site/$p" ] || continue
    mkdir -p "$dst/site/$(dirname "$p")"
    cp "$ROOT/site/$p" "$dst/site/$p"
  done
  # every path a claim names must resolve where the generator runs, so the fixture carries
  # them too, read from the matrix rather than listed here: a claim implemented outside the
  # trees copied above (the Rust executable under apps/) is otherwise "missing". After the
  # caller's trees, so that a tree copied whole is never pre-created and copied into itself.
  for p in $(awk '/^    (source|implementation|test): /{print $2}' "$ROOT/docs/CLAIMS.yaml" | tr -d "'" | grep -v '^-$' | LC_ALL=C sort -u); do
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
    # and a contract that tracks files brings the files it tracks. The validator resolves a
    # `tracks:` entry with `git ls-files`, so a README copied here without its tracked paths
    # is a broken reference in the fixture that does not exist upstream: .ai/repo/ci/ arrived
    # through a claim's source path, its README tracks .github/workflows/validate.yml, and
    # eight cases built on this helper failed `context validate` for a file the fixture had
    # simply never been given. Resolved with the same pathspec the validator uses, and read
    # from the front matter only: a body line that happened to start with `tracks: [` would
    # otherwise copy files no contract named, and nothing would report it.
    if [ -f "$dst/$d/README.md" ]; then
      for t in $(awk 'NR==1&&/^---/{f=1;next} f&&/^---/{exit} f&&/^tracks:/{sub(/^tracks:[ \t]*\[/,"");sub(/\].*$/,"");print;exit}' "$dst/$d/README.md" | tr ',' ' '); do
        for f in $(cd "$ROOT" && git ls-files -- "$t" 2>/dev/null); do
          [ -e "$dst/$f" ] && continue
          mkdir -p "$dst/$(dirname "$f")"
          cp "$ROOT/$f" "$dst/$f"
        done
      done
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
      # -m: curl has no default timeout, so a server that accepts the connection and then
      # never answers would block here forever -- and the loop's own bound below would
      # never advance. A bounded retry whose body can hang is not bounded.
      code="$(curl -s -m 2 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$port/" 2>/dev/null || true)"
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

# `majordomus serve` of the current directory on an ephemeral port, for the cases that ask a
# real socket. Once it listens, SRV is its pid and U its base URL (http://127.0.0.1:<port>);
# `serve_down` stops it. The caller has set RB (rust_bin) and kills $SRV in its EXIT trap, as
# it does for anything it starts. Answers 1, having said why and printed the server's log,
# when the server exits before it listens or has not listened within 30 s.
#   serve_up <out> <err>      the server's stdout and stderr, which the case may read after
#
# Two things here are what the copies of this in cases 89 and 488 got wrong, seen on a loaded
# runner as "no URL on the listening line" from a server that had listened:
#  * The log is emptied in this shell before the server starts. The redirection empties it
#    too, but in the background child, whenever that child gets to it: a wait that reads the
#    file at once can find the previous server's listening line there, and the read after it
#    the file the child has just emptied.
#  * The wait polls for the URL itself, not for the words before it, and U is the value the
#    poll found. Nothing reads the file a second time, so nothing can read it between states.
serve_up() {
  local out="$1" err="$2" i=0
  local url='/listening on http:\/\/127\.0\.0\.1:[0-9]/'
  url="$url"'{s#.*listening on \(http://127\.0\.0\.1:[0-9][0-9]*\).*#\1#p;q;}'
  U=""
  : > "$err"
  "$RB" serve --repo "$PWD" --port 0 > "$out" 2> "$err" & SRV=$!
  while :; do
    U="$(sed -n "$url" "$err")"
    [ -z "$U" ] || return 0
    kill -0 "$SRV" 2>/dev/null || { echo "    the server exited before listening"; cat "$err"; return 1; }
    i=$((i+1))
    [ "$i" -lt 300 ] || { echo "    the server has not listened in 30 s"; cat "$err"; return 1; }
    sleep 0.1
  done
}
# `wait` on a signalled child reports its signal, which is the expected outcome here and not
# a failure of the case, so neither it nor the kill is allowed to trip `set -e`.
serve_down() {
  [ -n "${SRV:-}" ] || return 0
  kill "$SRV" 2>/dev/null || true
  wait "$SRV" 2>/dev/null || true
  SRV=""
}

# A crate where the repository's own crate lives, shaped the way the rustdoc producer expects
# the real one and small enough to document in seconds: the package majordomus-cli, its
# library majordomus_cli, its executable majordomus, and a COMMIT constant the build is handed
# the commit through, as the real crate's build.rs is (MAJORDOMUS_BUILD_COMMIT). Written into
# the current directory beside the producer itself — scripts/rust-check and the file it
# sources — and the repository's toolchain pin, so the fixture is documented by the rustdoc the
# published reference is, and handed off by the script that hands that one off rather than by
# a case's copy of its steps. The caller commits: the producer records HEAD.
#   rustdoc_fixture_crate            the crate, one module (alpha) and one exported macro,
#                                    the producer and the pin
#   rustdoc_fixture_module NAME      `pub mod NAME;` with one documented struct in it
#   rustdoc_fixture_produce          scripts/rust-check --doc, quietly; says why when it fails
rustdoc_fixture_crate() {
  local c=apps/majordomus-cli version
  # the version the executable was built at: `majordomus generate` refuses to stamp a tree
  # whose crate declares another one
  version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/$c/Cargo.toml" | head -n 1)"
  mkdir -p "$c/src" scripts lib
  cp "$ROOT/scripts/rust-check" scripts/rust-check
  cp "$ROOT/lib/rust_bin.sh" "$ROOT/lib/sha256.sh" lib/
  cp "$ROOT/rust-toolchain.toml" rust-toolchain.toml
  printf 'target/\n' > .gitignore
  cat > "$c/Cargo.toml" <<TOML
[package]
name = "majordomus-cli"
version = "$version"
edition = "2021"
publish = false

[lib]
name = "majordomus_cli"
path = "src/lib.rs"

[[bin]]
name = "majordomus"
path = "src/main.rs"
TOML
  cat > "$c/build.rs" <<'RUST'
//! The commit the build was handed, compiled in, as the real crate's build script does.
fn main() {
    println!("cargo:rerun-if-env-changed=MAJORDOMUS_BUILD_COMMIT");
    let commit = std::env::var("MAJORDOMUS_BUILD_COMMIT").unwrap_or_else(|_| "unknown".into());
    println!("cargo:rustc-env=MAJORDOMUS_COMMIT={commit}");
}
RUST
  # the macro is there for the redirect stub rustdoc writes for it (macro.nothing!.html)
  cat > "$c/src/lib.rs" <<'RUST'
//! The fixture crate.
#![warn(missing_docs)]

/// The commit this build was handed.
pub const COMMIT: &str = env!("MAJORDOMUS_COMMIT");

/// Expands to nothing.
#[macro_export]
macro_rules! nothing {
    () => {};
}

pub mod alpha;
RUST
  printf '//! The executable.\n\nfn main() {}\n' > "$c/src/main.rs"
  rustdoc_fixture_module alpha
}
rustdoc_fixture_module() {
  local c=apps/majordomus-cli
  grep -qx "pub mod $1;" "$c/src/lib.rs" || printf 'pub mod %s;\n' "$1" >> "$c/src/lib.rs"
  printf '//! The module %s.\n\n/// The one thing %s has.\npub struct Thing;\n' "$1" "$1" > "$c/src/$1.rs"
}
rustdoc_fixture_produce() {
  local log; log="$(mktemp "${TMPDIR:-/tmp}/mj-rustdoc-fixture.XXXXXX")"
  env -u CARGO_TARGET_DIR -u MAJORDOMUS_BUILD_COMMIT scripts/rust-check --doc > "$log" 2>&1 || {
    printf '    scripts/rust-check --doc failed on the fixture crate:\n' >&2
    sed 's/^/    | /' "$log" >&2; rm -f "$log"; return 1; }
  rm -f "$log"
}

# ---------------------------------------------------------------- reasoning (ADR 0098)
# A fixture repository for the reasoning cases, and a way to run the executable and the
# transport in an environment that holds exactly what the case gives it: `env -i`, a PATH
# with git, node and a directory of stubs the case fills ($RZ_BIN), and no credential. On a
# machine where every advisor is installed the zero-advisor case must still be zero, so
# nothing of the caller's PATH or environment leaks in.
#   reasoning_fixture            R (the repository, committed), RZ_BIN (empty stub dir)
#   rz <args>                    the executable in $R, with $RZ_ENV (space-separated
#                                NAME=value words) added to the environment
#   rz_consult <args>            scripts/advisor-consult in $R, the same environment, with
#                                the fixture adapters of test/fixtures/advisors
#   rz_stub <name>               an executable named <name> on the isolated PATH
#   rz_record <json>             record one reasoning step; prints its id
reasoning_fixture() {
  R="$T/repo"
  fixture_repo "$R" >/dev/null
  git -C "$R" init -q .
  git -C "$R" config user.email t@example.com
  git -C "$R" config user.name t
  git -C "$R" add -A >/dev/null
  git -C "$R" commit -qm fixture >/dev/null
  RZ_BIN="$T/rz-bin"; mkdir -p "$RZ_BIN"
  ln -sf "$(command -v git)" "$RZ_BIN/git"
  # A skipped case reports ok, so on a CI runner a missing node is a failure, not a skip.
  local node; node="$(node -p 'process.execPath' 2>/dev/null)" || {
    [ -z "${CI:-}" ] || { echo "    node is required on CI: the reasoning transport cannot run"; exit 1; }
    skip "no node"; }
  ln -sf "$node" "$RZ_BIN/node"
  RZ_ENV=""
}
rz_env() {
  # shellcheck disable=SC2086 # RZ_ENV is a list of NAME=value words by contract
  env -i HOME="$T" TMPDIR="${TMPDIR:-/tmp}" PATH="$RZ_BIN:/usr/bin:/bin" LANG=C.UTF-8 \
    MAJORDOMUS_SHARE="$R/share" MAJORDOMUS_CLI="$RB" $RZ_ENV "$@"
}
rz() { ( cd "$R" && rz_env "$RB" "$@" ); }
rz_consult() { ( cd "$R" && rz_env node "$R/scripts/advisor-consult" --adapters "$ROOT/test/fixtures/advisors" "$@" ); }
rz_stub() { printf '#!/bin/sh\nexit 0\n' > "$RZ_BIN/$1"; chmod +x "$RZ_BIN/$1"; }
rz_record() {
  local out err="$T/rz_record.err"
  out="$(printf '%s' "$1" | rz reasoning record --format json 2> "$err")" \
    || { printf '    the record was refused: %s\n    input: %s\n' "$(cat "$err")" "$1" >&2; return 1; }
  printf '%s' "$out" | jq -r '.record.id'
}
#   rz_json <args>               the executable's JSON answer alone (stdout); its stderr is
#                                shown, and the case fails, when it exits non-zero
rz_json() {
  local err="$T/rz_json.err"
  rz "$@" --format json 2> "$err" || { printf '    rz %s failed:\n' "$*" >&2; cat "$err" >&2; return 1; }
}
