# majordomus-covers: landing
# Landing is a closure over delivery, and the property under test is that it composes rather
# than measures. The case builds a disposable repository, asks `landing.closure` about it,
# and holds the answer to four things:
#
#   1. every stage of the invariant is present, in the order delivery happens in, and each
#      one names the capability that decided it and a remedy — never a generic refusal;
#   2. a stage whose owner could not be asked is `unknown` and never `pass`, and `landed`
#      is false while any stage is unanswered;
#   3. the answer is a reading of other capabilities: each stage's verdict is re-derived
#      here from that capability's own document, so a stage that disagreed with its own
#      source would fail this case;
#   4. the command line, the JSON and the exit code are three renderings of one execution.
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
S="$(mktemp -d "${TMPDIR:-/tmp}/mj137.XXXXXX")"; trap 'rm -rf "$S"' EXIT
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null && git commit -qm base

# A managed repository is not a distribution and carries no share/ of its own, so the
# executable is told where the tool's is — the same directory `majordomus` reads from.
export MAJORDOMUS_SHARE="$ROOT/share"
closure() { "$BIN" run landing.closure --input '{}' --quiet --format json --repo . 2>/dev/null | jq -c '.output'; }

C="$S/closure.json"
closure > "$C"
[ -s "$C" ] || { echo "    landing.closure answered nothing"; exit 1; }

# ---------------------------------------------------------------- the invariant is whole
WANT="implementation tests documentation generated commit push integration ci deployment published project cleanup"
GOT="$(jq -r '[.stages[].id] | join(" ")' "$C")"
[ "$GOT" = "$WANT" ] || { echo "    the stages are '$GOT', not the order delivery happens in"; exit 1; }
[ "$(jq -r '.schema' "$C")" = "majordomus/landing-closure/v1" ] || { echo "    the document does not name its schema"; exit 1; }

# ---------------------------------------------------------------- every stage is actionable
# A stage that names no owner and no remedy is a stage a reader cannot act on, which is the
# "not ready" this report exists instead of.
jq -e 'all(.stages[]; (.source | length > 0) and (.remediation | length > 0) and (.question | length > 0) and (.evidence | length > 0))' "$C" >/dev/null \
  || { echo "    a stage carries no source, remedy, question or evidence:"; jq -r '.stages[] | select((.source|length==0) or (.remediation|length==0) or (.evidence|length==0)) | .id' "$C"; exit 1; }

# and every status is a word of the one vocabulary the gates module declares
jq -e 'all(.stages[].status; . == "pass" or . == "fail" or . == "stale" or . == "blocked" or . == "queued" or . == "exempt" or . == "unknown")' "$C" >/dev/null \
  || { echo "    a stage reports a status outside the gate vocabulary"; exit 1; }

# ---------------------------------------------------------------- absence is never a pass
# Nothing is passed without something behind it: every `pass` stage read a source, and
# `landed` is false whenever a stage is unanswered.
UNANSWERED="$(jq -r '[.stages[] | select(.status == "unknown" or .status == "queued")] | length' "$C")"
LANDED="$(jq -r '.landed' "$C")"
if [ "$UNANSWERED" -gt 0 ] && [ "$LANDED" != false ]; then
  echo "    $UNANSWERED stage(s) unanswered and the repository still reads as landed"; exit 1
fi
jq -e '(.unverified | length) == ([.stages[] | select(.status == "unknown" or .status == "queued")] | length)' "$C" >/dev/null \
  || { echo "    the unverified list does not match the unanswered stages"; exit 1; }
jq -e '(.blocking | length) == ([.stages[] | select(.status == "fail" or .status == "stale" or .status == "blocked")] | length)' "$C" >/dev/null \
  || { echo "    the blocking list does not match the refusing stages"; exit 1; }
# the verdict says which, rather than saying nothing
[ "$LANDED" = true ] && expect_grep 'LANDED' <<<"$(jq -r .verdict "$C")"
jq -e '.verdict | length > 0' "$C" >/dev/null || { echo "    no verdict line"; exit 1; }

# ---------------------------------------------------------------- it composes, it does not measure
# Each of these is the same fact read twice: once by the capability that owns it and once by
# the closure. A stage that decided for itself would disagree with its own source here.
DEPLOY="$("$BIN" run deploy.check --input '{}' --quiet --format json --repo . 2>/dev/null | jq -r '.output.refusals | length')"
DSTAGE="$(jq -r '.stages[] | select(.id == "deployment") | .status' "$C")"
if [ "$DEPLOY" = 0 ]; then
  case "$DSTAGE" in pass|exempt) ;; *) echo "    deploy.check refuses nothing and the deployment stage is $DSTAGE"; exit 1;; esac
else
  [ "$DSTAGE" = fail ] || { echo "    deploy.check refuses $DEPLOY and the stage is $DSTAGE"; exit 1; }
fi

STALE="$("$BIN" run artifacts.list --input '{}' --quiet --format json --repo . 2>/dev/null | jq -r '.output.tallies.stale')"
GSTAGE="$(jq -r '.stages[] | select(.id == "generated") | .status' "$C")"
if [ "$STALE" = 0 ]; then
  [ "$GSTAGE" = pass ] || { echo "    nothing is stale and the generated stage is $GSTAGE"; exit 1; }
else
  [ "$GSTAGE" = stale ] || { echo "    $STALE artifact(s) are stale and the stage is $GSTAGE"; exit 1; }
fi

DIRTY="$("$BIN" run worktree.topology --input '{}' --quiet --format json --repo . 2>/dev/null \
  | jq -r '[.output.worktrees[] | select((.standing != "ephemeral") and (.detached != true)) | select(.dirty != null and .dirty.clean == false)] | length')"
CSTAGE="$(jq -r '.stages[] | select(.id == "commit") | .status' "$C")"
if [ "$DIRTY" = 0 ]; then
  [ "$CSTAGE" = pass ] || { echo "    every worktree is clean and the commit stage is $CSTAGE"; exit 1; }
else
  [ "$CSTAGE" = fail ] || { echo "    $DIRTY worktree(s) are dirty and the stage is $CSTAGE"; exit 1; }
fi

# and the stage that names its source names a capability that exists
for src in $(jq -r '.stages[].source' "$C" | tr ' ' '\n' | grep -E '^[a-z_]+\.[a-z_]+$' | sort -u); do
  "$BIN" capabilities describe "$src" --repo . >/dev/null 2>&1 \
    || { echo "    a stage is decided by '$src', which is not a capability"; exit 1; }
done

# ---------------------------------------------------------------- uncommitted work refuses
# The one stage a case can move on purpose: work in the tree and not in the history.
echo 'uncommitted' > loose.txt
closure > "$S/dirty.json"
[ "$(jq -r '.stages[] | select(.id == "commit") | .status' "$S/dirty.json")" = fail ] \
  || { echo "    a dirty working tree did not refuse the commit stage"; exit 1; }
[ "$(jq -r '.landed' "$S/dirty.json")" = false ] || { echo "    a dirty tree is not a landed repository"; exit 1; }
jq -e '[.stages[] | select(.id == "commit") | .findings[]] | length > 0' "$S/dirty.json" >/dev/null \
  || { echo "    the commit stage refuses and names nothing"; exit 1; }
rm -f loose.txt

# ---------------------------------------------------------------- one execution, three renderings
# The command line renders the same document, and the exit code is the verdict.
if [ "$(jq -r '.landed' "$C")" = true ]; then expect_exit 0 "$BIN" landing --repo .
else expect_exit 10 "$BIN" landing --repo .; fi
expect_grep 'verdict'
for id in $WANT; do expect_grep "  $id " || { echo "    the report omits the $id stage"; exit 1; }; done
# the exit code is the verdict and not a property of the rendering: --format json answers
# with the same code, so a script that reads the document still learns the verdict from it
if [ "$(jq -r '.landed' "$C")" = true ]; then WANT_RC=0; else WANT_RC=10; fi
expect_exit "$WANT_RC" "$BIN" landing --repo . --format json
expect_grep '"stages"'
expect_grep '"landed"'
