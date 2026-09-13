# The site generator reads part of its subject through the tool. When one of those reads
# fails, the run must refuse and say which part it could not reach — never publish the
# projection of the smaller repository the failed read leaves behind.
#
# The failure this prevents is exact. `scripts/generate-site-data` answered a failed
# `session list --all --json` with
#
#     || printf '{"schema":1,"sessions":[]}' > "$STAGE/sessions.raw.json"
#
# and `session list` exits 15 (refused) whenever MAJORDOMUS_SHARE names another worktree of
# this repository — which it does in every linked worktree here. The refusal became "this
# repository has closed no sessions", the run carried on, and the only channel a caller reads
# — the exit status — said nothing had gone wrong. Silence read as success.
#
# The rule is project.a-verdict-states-its-subject; the code is 12, could not measure, which
# this script already exits when jq is absent or a canonical input is missing.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
# .github comes along because a directory contract under .ai/repo/ci tracks the workflow
# files, and `context list` — one of the reads this case is about — reports a contract whose
# tracked path matches nothing as a problem
fixture_repo "$T" AGENTS.md docs .github
mkdir -p "$T/site/data"; cp "$ROOT/site/data/marketing.toml" "$ROOT/site/data/nav.toml" "$T/site/data/"
cp -R "$ROOT/site/content-src" "$T/site/"
mkdir -p "$T/test"; cp -R "$ROOT/test/cases" "$T/test/"
git -C "$T" add -A >/dev/null; git -C "$T" commit -qm fixture

TOOL="$T/bin/majordomus"
PRISTINE="$T/majordomus.pristine"
cp "$TOOL" "$PRISTINE"

# A generation that succeeds, to compare against: everything below asserts that a failed read
# changes the exit status and changes nothing on disk.
expect_exit 0 "$T/scripts/generate-site-data" --no-scenarios
expect_file "$T/site/data/generated/sessions.json"
before_files="$(ls "$T/site/data/generated" | LC_ALL=C sort)"
before_sessions="$(cat "$T/site/data/generated/sessions.json")"

# Mutate the tool so that one subcommand behaves as the real one does when it refuses, and
# prove the mutation took before trusting anything the run says about it. A patch script that
# silently no-ops reports a clean pass over an unmutated tree — the same class of defect this
# case is about, one level up.
mutate_tool() {   # mutate_tool <one-line shell snippet guarding `session list`>
  printf '%s\n' "MJ_CASE_319_MUTATED=1; $1" > "$T/mutation.sh"
  sed "1r $T/mutation.sh" "$PRISTINE" > "$TOOL.new" && mv "$TOOL.new" "$TOOL"
  chmod +x "$TOOL"
  # the file differs from the original
  cmp -s "$TOOL" "$PRISTINE" && { echo "    the mutation left bin/majordomus unchanged"; exit 1; }
  # the sentinel is present
  grep -q 'MJ_CASE_319_MUTATED' "$TOOL" || { echo "    the mutation sentinel is not in bin/majordomus"; exit 1; }
  # the original is otherwise intact, so the mutation is the only difference
  grep -q '^MJ_VERSION=' "$TOOL" || { echo "    the mutation destroyed bin/majordomus"; exit 1; }
  # and the result is still shell
  bash -n "$TOOL" || { echo "    the mutated bin/majordomus does not parse"; exit 1; }
}
restore_tool() {
  cp "$PRISTINE" "$TOOL"; chmod +x "$TOOL"
  grep -q 'MJ_CASE_319_MUTATED' "$TOOL" && { echo "    the tool was not restored"; exit 1; }
  return 0
}

# --- the tool refuses the read (exit 15, what MAJORDOMUS_SHARE produces)
mutate_tool 'case " $* " in *" session list "*) echo "majordomus: refused, this is the mutation" >&2; exit 15 ;; esac'
# the mutation reaches the command the generator calls, and only that one
expect_exit 15 "$TOOL" --repo "$T" session list --all --json
expect_exit 0 "$TOOL" --repo "$T" --help

expect_exit 12 "$T/scripts/generate-site-data" --no-scenarios
expect_grep 'the closed sessions could not be read'
expect_grep 'exited 15'
expect_grep 'refused, this is the mutation'
expect_grep 'subject this run could not reach'
# an empty listing is never what a refusal means
expect_no_grep 'in sync'
# and nothing was published: the previous generation is intact, byte for byte
[ "$(ls "$T/site/data/generated" | LC_ALL=C sort)" = "$before_files" ]
[ "$(cat "$T/site/data/generated/sessions.json")" = "$before_sessions" ]
[ -z "$(find "$T/site" "$T/docs" -name '*.mj-old' -o -name '*.mj-tmp' 2>/dev/null)" ]

# --- the tool succeeds and writes something that is not JSON
# A prefix of a document is what a tool that dies part way through a JSON stream leaves in the
# redirect, and it is as unreachable as a document never written. The read includes the parse.
restore_tool
mutate_tool 'case " $* " in *" session list "*) printf %s "{\"schema\":1,\"sessions\":[" ; exit 0 ;; esac'
expect_exit 12 "$T/scripts/generate-site-data" --no-scenarios
expect_grep 'the closed sessions could not be read'
expect_grep 'did not write JSON that parses'
[ "$(cat "$T/site/data/generated/sessions.json")" = "$before_sessions" ]

# --- the same contract for the other reads the generator makes through the tool
restore_tool
mutate_tool 'case " $* " in *" context list "*) echo "majordomus: refused, this is the mutation" >&2; exit 15 ;; esac'
expect_exit 12 "$T/scripts/generate-site-data" --no-scenarios
expect_grep 'the directory contracts could not be read'

restore_tool
mutate_tool 'case " $* " in *" usecase coverage "*) echo "majordomus: refused, this is the mutation" >&2; exit 15 ;; esac'
expect_exit 12 "$T/scripts/generate-site-data" --no-scenarios
expect_grep 'the use-case coverage could not be read'
# the tool's own words reach the reader; `2>/dev/null` here left a headline with nothing
# under it, which is a finding nobody can act on
expect_grep 'refused, this is the mutation'

# --- and the unmutated tool still generates, so none of the above passed because the fixture
#     had stopped working
restore_tool
rm -f "$T/mutation.sh"
expect_exit 0 "$T/scripts/generate-site-data" --no-scenarios
[ "$(cat "$T/site/data/generated/sessions.json")" = "$before_sessions" ]
