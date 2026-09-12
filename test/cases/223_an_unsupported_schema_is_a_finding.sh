# majordomus-covers: session doctor
# majordomus-negative: session doctor
# A store that holds records this executable was not written for, and what every reader of it
# is obliged to say.
#
# A closed episode is a shared object of the layer (ADR 0014): it is committed, it travels to
# every clone, and the clone that reads it may be running an older or a newer Majordomus than
# the one that wrote it. So the store is a migration surface, and the rule for one is not
# "read what you can" — it is that a version this executable does not read must produce a
# visible finding, in every surface that touches the record, and never a silent skip.
#
# The twin of a silent skip is a silent accept, and that is what was here. `session list` read
# a record's front matter without looking at its version at all: a record written `schema:
# session/v2` by a newer Majordomus was listed as an ordinary closed episode, with its
# outcome, its branch and its divergence label, and nothing said that those fields had been
# read under a contract that no longer describes them. `mj_resolve_latest` was worse: its
# version test was "`schema:` is not empty", so the same record was eligible to be *the record
# the next worker resumes from*. A worker cannot tell a misread record is wrong until it has
# acted on it.
#
# Five fixtures, which are the five states a store can be in:
#
#   the current version         read, listed, resolved
#   a supported older shape     `schema_version: 1`, which is what the local records have
#                               always said and which the shared resolver still reads (ADR 0014)
#   an unsupported future one   `session/v2`: refused, by name, with the version it found
#   a missing version           refused the same way, because the contract has `schema` as a
#                               field every record carries, so absent is not "older"
#   a mixed store               the readable records are still read; one bad record does not
#                               take the store with it, and the count of what was skipped is
#                               part of the answer
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null 2>&1; git commit -qm base >/dev/null 2>&1 || true
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
STORE=.ai/repo/sessions

# ---------------------------------------------------------------- the current version
# Written by the tool itself, so the fixture below is a mutation of a real record rather than
# a hand-made one: a fixture that invents the shape it is testing tests the fixture.
"$MJ" session start --owner tester >/dev/null
"$MJ" session close >/dev/null
good="$(find "$STORE" -maxdepth 1 -name '2*.md' | head -n 1)"
must "the close wrote no record to build the fixtures from" [ -n "$good" ]
expect_grep '^schema: session/v1$' "$good"
current="$(sed -n 's/^session_id: //p' "$good" | head -n 1)"

# ---------------------------------------------------------------- four more fixtures
mk() {  # mk <name> <session id> <sed script>
  sed -e "s/^session_id: .*/session_id: $2/" -e "$3" "$good" > "$STORE/$1.md"
}
# `schema_version: 1` is deliberately NOT one of these. It is the *local* records' version
# field — handovers and checkpoints — and a session record carrying it instead of `schema:`
# is not an older session record but a record of another family in the wrong directory. The
# supported older shape is exercised where it belongs, over a handover, at the end.
mk future  s-future  's|^schema: session/v1$|schema: session/v2|'
mk absent  s-absent  '/^schema: session\/v1$/d'
mk ancient s-ancient 's|^schema: session/v1$|schema: session/v0|'

# ---------------------------------------------------------------- the listing
# Every readable record is listed; every unreadable one is named on stderr with the version it
# carries, and the count of what was skipped is stated rather than left to be inferred from a
# listing that is shorter than the directory.
"$MJ" session list --all > "$T/list.out" 2> "$T/list.err" || { echo "    session list failed on a mixed store"; sed 's/^/    | /' "$T/list.err"; exit 1; }
expect_grep "$current" "$T/list.out"
# THE REGRESSION: this record used to appear in that listing as an ordinary closed episode.
expect_no_grep 's-future' "$T/list.out"
expect_no_grep 's-absent' "$T/list.out"
expect_no_grep 's-ancient' "$T/list.out"
expect_grep "session/v2" "$T/list.err"
expect_grep "session/v0" "$T/list.err"
expect_grep "no schema" "$T/list.err"
expect_grep '3 record\(s\) were skipped' "$T/list.out"
# A finding names what to do about it. Without this the message is an observation.
expect_grep 'majordomus doctor' "$T/list.err"
# A mixed store still answers. One record this version cannot read does not cost a reader the
# records it can: `empty is not failure` has a twin, and it is `unreadable is not empty`.
must "a mixed store answered with nothing at all" [ -s "$T/list.out" ]

# The machine-readable projection carries only records that were read, and the diagnostics
# stay on stderr: every caller of --json redirects stdout into a file, and a warning written
# there is a warning nobody sees inside a document nobody can parse.
"$MJ" --json session list --all > "$T/list.json" 2>/dev/null
if command -v jq >/dev/null 2>&1; then
  jq -e . "$T/list.json" >/dev/null || { echo "    --json stopped being JSON on a mixed store"; cat "$T/list.json" | sed 's/^/    | /'; exit 1; }
  must "the JSON listing carries a record this executable cannot read" \
    [ "$(jq -r '[.sessions[] | select(.session_id | startswith("s-future") or startswith("s-absent") or startswith("s-ancient"))] | length' "$T/list.json")" = 0 ]
fi

# ---------------------------------------------------------------- the resolver
# `session latest` is the one that decides what a worker is handed. A record from the future
# is not offered, and the refusal says which version it found.
"$MJ" session latest --path > "$T/latest.out" 2> "$T/latest.err" || true
expect_grep "$(basename "$good")" "$T/latest.out"
expect_grep "session/v2" "$T/latest.err"

# ---------------------------------------------------------------- reading one by name
# A refusal that names the wrong cause sends a reader to the wrong place: a record that does
# not parse is damaged here, a record from the future is intact and was written by something
# newer, and those are different things to do next.
expect_exit 10 "$MJ" session show s-future
expect_grep "session/v2"
expect_grep 'this executable reads session/v1'
expect_no_grep 'does not parse'

# ---------------------------------------------------------------- and doctor refuses it
# The finding of record. `doctor` is the surface that judges the store as a whole, and it must
# name each unreadable record by file rather than reporting a count. Its exit status is not
# asserted: a fixture repository legitimately fails other doctrines (its git hooks are not
# installed), and a case that asserted the whole exit would be asserting those.
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
expect_grep "future.md — schema is 'session/v2'" "$T/doctor.out"
expect_grep "ancient.md — schema is 'session/v0'" "$T/doctor.out"
expect_grep "absent.md — schema is ''" "$T/doctor.out"
# and the record this version does read is not caught by the same net
expect_no_grep "$(basename "$good") — schema is" "$T/doctor.out"

# ---------------------------------------------------------------- the older shape
# `schema_version: 1` is the local records' version field, and the shared resolver reads it
# because the two record families met when a closed session became a shared object (ADR 0014).
# It is a supported older shape, not an unreadable one, and the resolver must keep reading it:
# a handover written before that merge is still the thing a worker resumes from.
mkdir -p .ai/local/state/handovers
sed -e 's/^session_id: .*/session_id: s-handover/' "$good" > "$T/h.md"
{ printf -- '---\nschema_version: 1\ncreated_at: %s\ntask_id: none\nprofile: none\nowner: "t"\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'repository_id: %s\nworktree: %s\nbranch: %s\nhead: %s\nworking_tree: clean\nchanged_files:\n---\n\n# Next Action\n\nResume here.\n' \
    "$(git rev-parse --absolute-git-dir)" "$(pwd -P)" "$(git rev-parse --abbrev-ref HEAD)" "$(git rev-parse HEAD)"
} > .ai/local/state/handovers/legacy-shape.md
"$MJ" context > "$T/ctx.out" 2>"$T/ctx.err" || true
expect_grep 'Resume here' "$T/ctx.out"
expect_no_grep 'legacy-shape.md: schema' "$T/ctx.err"
# and a handover from the future is not offered either, by the same rule
sed 's/^schema_version: 1$/schema: handover\/v2/' .ai/local/state/handovers/legacy-shape.md > .ai/local/state/handovers/future-shape.md
sed -i.bak 's/Resume here\./Do not resume here./' .ai/local/state/handovers/future-shape.md 2>/dev/null || true
rm -f .ai/local/state/handovers/future-shape.md.bak
"$MJ" context > "$T/ctx2.out" 2>"$T/ctx2.err" || true
expect_no_grep 'Do not resume here' "$T/ctx2.out"
expect_grep 'handover/v2' "$T/ctx2.err"
