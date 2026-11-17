# majordomus-covers: none
# The published UI reads the evidence CI recorded, and a publication that would show tests as
# not run is refused (I2786, rule project.published-evidence-is-what-ci-recorded).
#
# On 2026-10-16 the published kiosk showed 154 test claims as "not run": the Pages runner read
# the tracked ledger, which holds a handful of local runs, while the validation of the same
# commit had recorded 606 executions in its `evidence` artifact. This case holds both halves
# through the real script, against this repository's own history, read-only:
#
#   1  `scripts/pages evidence --ledger-out FILE` puts the run's ledger at FILE when the
#      artifact is of the published commit's history, and the dataset it writes says so
#   2  an artifact of a commit outside that history copies nothing: FILE is left as it was
#   3  an artifact with no ledger copies nothing and says why, and the dataset is still written
#   4  `scripts/pages published-evidence` passes within the declared baseline, refuses over it
#      with exit 10 naming the count, and exits 12 when the report cannot be read or the
#      baseline holds no count — an unreadable report is never a pass
. "$ROOT/test/lib.sh"

W="$T/w"
mkdir -p "$W"
HERE="$(git -C "$ROOT" rev-parse HEAD)"
OUTSIDE="0123456789abcdef0123456789abcdef01234567"

artifact() {     # artifact <dir> <commit> [ledger] — a gathered evidence directory
  mkdir -p "$1"
  jq -n --arg c "$2" '{schema: 1, commit: $c, working_tree: "clean", report_tree: "clean",
    absent: [], refused: [], totals: {outcomes: {pass: 1}},
    panels: [{label: "evidence", command: "majordomus evidence show", exit: 0}]}' > "$1/manifest.json"
  printf 'evidence\n' > "$1/report.txt"
  if [ "${3:-}" = ledger ]; then
    jq -n --arg c "$2" '{schema: 1, executions: [{test: "suite:1_x", outcome: "pass", commit: $c,
      origin: "ci", at: "2026-10-17T00:00:00Z"}]}' > "$1/ledger.json"
  fi
}

# ---------------------------------------------------------------- 1. the run's ledger, in place
artifact "$W/ours" "$HERE" ledger
printf '{"schema":1,"executions":[]}\n' > "$W/ledger.json"
expect_exit 0 "$ROOT/scripts/pages" evidence --commit "$HERE" --from "$W/ours" --out "$W/ours.json" --ledger-out "$W/ledger.json"
expect_grep "the ledger of ${HERE:0:12}'s run \(1 executions\) is"
cmp -s "$W/ours/ledger.json" "$W/ledger.json" \
  || { echo "    the run's ledger was not put in place"; cat "$W/ledger.json"; exit 1; }
jq -e '.available and .commit == "'"$HERE"'"' "$W/ours.json" >/dev/null \
  || { echo "    the dataset does not name the run it read"; cat "$W/ours.json"; exit 1; }

# ---------------------------------------------------------------- 2. outside the history: nothing
artifact "$W/theirs" "$OUTSIDE" ledger
printf '{"schema":1,"executions":[]}\n' > "$W/ledger.json"
expect_exit 0 "$ROOT/scripts/pages" evidence --commit "$HERE" --from "$W/theirs" --out "$W/theirs.json" --ledger-out "$W/ledger.json"
jq -e '.executions == []' "$W/ledger.json" >/dev/null \
  || { echo "    a ledger of a commit outside the published history was put in place"; exit 1; }
jq -e '.available == false' "$W/theirs.json" >/dev/null \
  || { echo "    evidence of another history was published as available"; exit 1; }

# ---------------------------------------------------------------- 3. no ledger in the artifact
artifact "$W/bare" "$HERE"
expect_exit 0 "$ROOT/scripts/pages" evidence --commit "$HERE" --from "$W/bare" --out "$W/bare.json" --ledger-out "$W/ledger.json"
expect_grep "carries no ledger this build reads"
jq -e '.executions == []' "$W/ledger.json" >/dev/null || { echo "    the ledger changed"; exit 1; }
expect_file "$W/bare.json"

# ---------------------------------------------------------------- 4. the published-evidence check
report() {       # report <totals JSON> — an executable answering evidence.report with those totals
  printf '#!/usr/bin/env bash\nprintf %%s %q\n' "{\"output\":{\"totals\":$1}}" > "$W/bin"
  chmod +x "$W/bin"
}
printf '# the declared count\n0\n' > "$W/zero.txt"
printf '3\n' > "$W/three.txt"

report '{"proven":235,"no test":12}'
expect_exit 0 "$ROOT/scripts/pages" published-evidence --bin "$W/bin" --baseline "$W/zero.txt"
expect_grep "0 claim\(s\) not run, within the baseline of 0"

report '{"not run":214,"stale":21}'
expect_exit 10 "$ROOT/scripts/pages" published-evidence --bin "$W/bin" --baseline "$W/zero.txt"
expect_grep "214 claim\(s\) would be published as not run, over the baseline of 0"
expect_exit 10 "$ROOT/scripts/pages" published-evidence --bin "$W/bin" --baseline "$W/three.txt"

report '{"not run":3}'
expect_exit 0 "$ROOT/scripts/pages" published-evidence --bin "$W/bin" --baseline "$W/three.txt"

printf '#!/usr/bin/env bash\nexit 1\n' > "$W/bin"
expect_exit 12 "$ROOT/scripts/pages" published-evidence --bin "$W/bin" --baseline "$W/zero.txt"
expect_grep "could not be read"

report '{"not run":0}'
printf '# nothing\n' > "$W/empty.txt"
expect_exit 12 "$ROOT/scripts/pages" published-evidence --bin "$W/bin" --baseline "$W/empty.txt"
expect_exit 12 "$ROOT/scripts/pages" published-evidence --bin "$W/bin" --baseline "$W/absent.txt"

# the repository declares its own baseline, and it is a count
grep -vE '^[[:space:]]*(#|$)' "$ROOT/.ai/repo/evidence/published-not-run.txt" | head -1 | grep -qE '^[0-9]+$' \
  || { echo "    .ai/repo/evidence/published-not-run.txt declares no count"; exit 1; }
