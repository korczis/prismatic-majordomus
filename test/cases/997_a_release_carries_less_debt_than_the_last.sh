# majordomus-covers: none
# A release carries less debt than the one before it, and no baseline grows in any change.
# Rule: project.a-release-carries-less-debt-than-the-last.
#
# Through the real command line, `majordomus release debt`, over a repository this case
# builds: two baselines — a list of three violations and a counter that states two — a
# declaration that names them, and one release whose record carries what it measured.
#
#   1. no release on record: the counts are recorded and compared with nothing, said in
#      those words; `--release` does not refuse a first measurement, and `--record` prints
#      the block a release record carries
#   2. a release on record with the same counts: a change may carry the tree (exit 0), a
#      release may not (exit 10), and the report says the largest total a release may carry
#   3. one entry paid: the release passes, and says by how much the debt fell
#   4. the mutation: one line added to the list is refused in a change, without --release,
#      naming the baseline, its gate and both numbers
#   5. the counter grew from 2 to 9 while the list was paid down: refused, though a line
#      count would have called it a payment
#   6. a file named like a baseline that the declaration does not count is refused, and one
#      it excludes with a reason is not
#   7. a previous release that recorded no counts: compared with nothing, said so
#   8. no declaration: exit 12 — nothing is known to be debt, which is not a tree with none
#   9. the MCP tool answers with the same counts the command prints
#  10. debt no change can pay: declared `payable: false` with its reason, the counter is
#      still counted, still recorded and still refused when it grows, and is left out of what
#      a release must reduce; declared so without a reason, it is refused
#  11. this repository's own declaration: every baseline under .ai/repo is counted or
#      excluded with a reason, and the only one declared unpayable is commit-policy
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

git config user.email a@b.c; git config user.name a
"$MJ" init >/dev/null
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

# release records are discovered as the repository's own are
grep -q 'kind: release-record' .ai/repo/knowledge/sources.yaml || cat >> .ai/repo/knowledge/sources.yaml <<'YAML'

  - id: release
    kind: release-record
    discovery: vcs
    pathspec: ':(glob).ai/repo/releases/*.yaml'
    required: false
YAML

mkdir -p .ai/repo/ci .ai/repo/releases
cat > .ai/repo/ci/debt.yaml <<'YAML'
version: 1
minimum_reduction: 1
baselines:
  - id: list
    path: .ai/repo/list-baseline.txt
    gate: list-check
    form: entries
  - id: numbers
    path: .ai/repo/numbers-baseline.txt
    gate: numbers-check
    form: counts
not_debt:
  - path: .ai/repo/ci/baseline.json
    because: recorded timings, a measurement and not a list of violations
YAML
list() {   # <n>: a list baseline of n entries, with a comment and a blank line that are not entries
  { printf '# accepted on the day the gate landed\n\n'; i=0; while [ "$i" -lt "$1" ]; do printf 'docs/A.md\tcommand %s\n' "$i"; i=$((i + 1)); done; } > .ai/repo/list-baseline.txt
}
numbers() { printf '# a counter\nviolations=%s\n' "$1" > .ai/repo/numbers-baseline.txt; }
list 3; numbers 2
echo '{}' > .ai/repo/ci/baseline.json
commit() { git add -A >/dev/null && git commit -qm "$1"; }
commit "the debt as it stands"

debt() { rc=0; out="$("$RB" release --repo . debt "$@" 2>"$T/err")" || rc=$?; }
says() { case "$out" in *"$1"*) ;; *) echo "    $2:"; printf '%s\n' "$out" | sed 's/^/      /'; sed 's/^/      | /' "$T/err" | tail -3; exit 1 ;; esac; }
exits() { [ "$rc" = "$1" ] || { echo "    $2 exited $rc, not $1:"; printf '%s\n' "$out" | sed 's/^/      /'; sed 's/^/      | /' "$T/err" | tail -3; exit 1; }; }

# ---------------------------------------------------------------- 1. nothing to compare with
debt --release
exits 0 "a first measurement, judged as a release,"
says "no release is recorded: 5 is recorded now and compared with nothing" "a first measurement does not say it is compared with nothing"
debt --record
exits 0 "--record"
[ "$out" = "debt:
  total: 5
  payable: 5
  baselines:
    list: 3
    numbers: 2" ] || { echo "    --record does not print the block a release record carries:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }

# ---------------------------------------------------------------- 2. a release on record
record() {   # <version> [<the debt block>]
  { cat <<YAML
schema: release/v1
version: "$1"
tag: v$1
channel: stable
commit: 0123456789abcdef0123456789abcdef01234567
published_at: "2026-10-08T00:00:00Z"
YAML
    [ $# -lt 2 ] || printf '%s\n' "$2"
    printf 'artifacts:\n  - target: macos-aarch64\n    name: fixture-v%s-aarch64-apple-darwin.tar.gz\n    url: https://github.com/example/fixture/releases/download/v%s/fixture-v%s-aarch64-apple-darwin.tar.gz\n    sha256: b92c3af90eed4214687d47f213552525a468f9155b09533d21c2c382a4d532be\n    size: 1\n' "$1" "$1" "$1"
  } > ".ai/repo/releases/v$1.yaml"
}
record 1.0.0 "$out"
commit "v1.0.0, with what it carried"
debt
exits 0 "a change that leaves the debt where the release left it"
says "must carry at most 4" "the report does not say what the next release may carry"
debt --release
exits 10 "a release whose debt stood still"
says "FAIL the debt is 5 and was 5 at v1.0.0" "a release that paid nothing is not refused in those words"
says "pay at least the minimum before the tag" "the refusal does not say what to do"

# What shipped twice (v0.16.0, v0.17.0, the batch after): no baseline file changed by a byte
# between two releases. Nothing here reads a diff, so a second release over the same files is
# refused like the first, and against the release that is now the previous one.
block="$("$RB" release --repo . debt --record 2>/dev/null)"
record 1.1.0 "$block"
commit "v1.1.0, released over the same baselines"
git --no-pager diff --quiet HEAD~1 HEAD -- .ai/repo/list-baseline.txt .ai/repo/numbers-baseline.txt \
  || { echo "    the fixture changed a baseline between the two releases"; exit 1; }
debt --release
exits 10 "a third release over baselines no change touched"
says "FAIL the debt is 5 and was 5 at v1.1.0" "a release over untouched baselines is not refused against the previous release"
rm .ai/repo/releases/v1.1.0.yaml; commit "the fixture goes back to one release on record"

# ---------------------------------------------------------------- 3. one entry paid
list 2; commit "one entry paid"
debt --release
exits 0 "a release that paid one entry"
says "OK   the debt fell from 5 to 4 since v1.0.0, by at least the minimum of 1" "a release that paid one entry does not say so"
says "list                          2  was      3" "the report does not show the baseline that was paid"

# ---------------------------------------------------------------- 4. the mutation: one line added
list 4; commit "one entry added"
debt
exits 10 "a change that added one line to a baseline"
says "FAIL .ai/repo/list-baseline.txt grew from 3 to 4 since v1.0.0: a baseline may only shrink (gate list-check)" "the growth of one line is not named with its baseline, its gate and both numbers"

# ---------------------------------------------------------------- 5. a counter that grew, under a lower total
list 0; numbers 4; commit "the list paid, the counter doubled"
debt --release
exits 10 "a counter that grew while the total fell"
says ".ai/repo/numbers-baseline.txt grew from 2 to 4 since v1.0.0" "a counter's growth is not seen as growth"
[ "$(printf '%s\n' "$out" | awk '$1 == "total" { print $2 }')" = 4 ] || { echo "    the total is not the sum of the counts:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
numbers 2; list 2; commit "back to one entry paid"

# ---------------------------------------------------------------- 6. a baseline nobody declared
printf 'x\n' > .ai/repo/new-baseline.txt; commit "a new baseline"
debt
exits 10 "a baseline the declaration does not count"
says ".ai/repo/new-baseline.txt is named like a baseline and .ai/repo/ci/debt.yaml neither counts it nor says why it is not debt" "an undeclared baseline is not named"
case "$out" in *"baseline.json is named like"*) echo "    a file the declaration excludes with a reason is reported:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1 ;; esac
git rm -q .ai/repo/new-baseline.txt; commit "the new baseline is gone"

# ---------------------------------------------------------------- 7. a release that recorded nothing
record 2.0.0
commit "v2.0.0, which recorded no debt"
debt --release
exits 0 "a release after one that recorded no debt"
says "v2.0.0 recorded no debt: 4 is recorded now and compared with nothing" "a previous release without counts is not said to be that"
case "$out" in *"fell from"*) echo "    a first measurement claims the debt fell:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1 ;; esac

# ---------------------------------------------------------------- 9. the MCP tool, before the declaration goes
frame="$({ printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case997","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_release_debt","arguments":{}}}\n'
} | "$RB" mcp --standalone --repo . 2>"$T/mcp.err" | sed -n 2p)"
printf '%s' "$frame" | jq -e '.result.structuredContent | .total == 4 and .standing == "unrecorded" and .previous_release == "v2.0.0"
  and .release_allowed == true and ([.baselines[] | {(.id): .count}] | add) == {"list": 2, "numbers": 2}' >/dev/null \
  || { echo "    majordomus_release_debt does not answer with the counts the command prints:"; printf '%s' "$frame" | head -c 700; echo; sed 's/^/      | /' "$T/mcp.err" | tail -3; exit 1; }

# ---------------------------------------------------------------- 10. debt that cannot be paid
frozen() {   # <the reason line, or nothing>: the counter is declared unpayable
  awk -v because="$1" '{ print } /^    form: counts$/ { print "    payable: false"; if (because != "") print "    because: " because }' \
    "$T/debt.yaml" > .ai/repo/ci/debt.yaml
}
cp .ai/repo/ci/debt.yaml "$T/debt.yaml"
record 3.0.0 "debt:
  total: 4
  payable: 4
  baselines:
    list: 2
    numbers: 2"
frozen "published history cannot be rewritten"; commit "v3.0.0, and the counter is declared unpayable"
debt --release
exits 10 "a release that paid nothing of what can be paid"
says "FAIL the debt is 2 and was 2 at v3.0.0: a release must carry at most 1, lower by the minimum of 1; 2 more cannot be paid and is counted, not owed" "the owed debt is not judged without the unpayable part"
says "numbers                       2  was      2  .ai/repo/numbers-baseline.txt  (cannot be paid)" "the unpayable baseline is not shown as counted"
# the hatch is never silent: named with its reason in every report, and marked where it was opened
says ".ai/repo/numbers-baseline.txt is declared unpayable and holds 2, newly declared so since v3.0.0: published history cannot be rewritten" "an unpayable baseline is not named with its reason, or not marked as newly declared"
list 1; commit "one payable entry paid"
debt --release
exits 0 "a release that paid one payable entry"
says "OK   the debt fell from 2 to 1 since v3.0.0" "the payment is not judged on the payable debt"
debt --record
case "$out" in *"total: 3"*"payable: 1"*"unpayable:"*"- numbers"*"numbers: 2"*) ;; *) echo "    the record does not keep the unpayable baseline, by id and by count, beside the payable total:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1 ;; esac
# a release that recorded it as unpayable: still named, no longer new
record 4.0.0 "$out"; commit "v4.0.0, which records the counter as unpayable"
debt
says ".ai/repo/numbers-baseline.txt is declared unpayable and holds 2: published history cannot be rewritten" "an unpayable baseline a release already recorded is not named"
case "$out" in *"newly declared"*) echo "    a baseline the previous release recorded as unpayable is called newly declared:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1 ;; esac
git rm -q .ai/repo/releases/v4.0.0.yaml; commit "back to v3.0.0 as the newest release"
numbers 3; commit "the unpayable counter grew"
debt
exits 10 "an unpayable baseline that grew"
says ".ai/repo/numbers-baseline.txt grew from 2 to 3 since v3.0.0" "an unpayable baseline may grow unseen"
numbers 2; frozen ""; commit "unpayable, and no reason given"
debt
exits 10 "a baseline declared unpayable without a reason"
says ".ai/repo/numbers-baseline.txt is declared \`payable: false\` without a reason" "an unpayable baseline with no reason is not refused"
cp "$T/debt.yaml" .ai/repo/ci/debt.yaml; commit "the declaration as it was"

# ---------------------------------------------------------------- 12. evidence older than the release before
# What shipped twice beside the debt that stood still: a tracked evidence ledger recorded
# weeks before the releases it stood behind, read by a check that passes while supporting
# no claim. Declared, the ledger's newest CI execution may not predate the previous release.
ledger() { mkdir -p .ai/repo/evidence; printf '{"version":1,"executions":[%s]}\n' "$1" > .ai/repo/evidence/ledger.json; }
printf 'evidence_ledger: .ai/repo/evidence/ledger.json\n' >> .ai/repo/ci/debt.yaml
commit "the declaration names an evidence ledger nobody recorded"
debt --release
exits 10 "a release whose declared evidence ledger does not exist"
says ".ai/repo/evidence/ledger.json cannot be read as an evidence ledger" "a missing ledger is not refused as one that says nothing"
debt
exits 0 "a change over a ledger that does not exist"
ledger '{"origin":"ci","at":"2026-09-17T01:14:59Z"},{"origin":"local","at":"2026-10-09T00:00:00Z"}'
commit "CI recorded before v3.0.0 was published, and a person ran something after it"
debt --release
exits 10 "a release over evidence CI recorded before the previous release"
says "FAIL the newest CI execution in .ai/repo/evidence/ledger.json is 2026-09-17T01:14:59Z, before the previous release was published (2026-10-08T00:00:00Z): record the evidence of this tree before the tag" "stale evidence is not refused with both times and what to do"
case "$out" in *"pay at least the minimum"*) echo "    stale evidence is answered with advice to pay debt:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1 ;; esac
debt
exits 0 "a change over stale evidence: a change cannot be asked to re-record it"
ledger '{"origin":"ci","at":"2026-09-17T01:14:59Z"},{"origin":"ci","at":"2026-10-08T12:00:00Z"}'
commit "CI recorded after v3.0.0"
debt --release
exits 0 "a release over evidence recorded since the previous release"
git rm -q .ai/repo/evidence/ledger.json; cp "$T/debt.yaml" .ai/repo/ci/debt.yaml; commit "the declaration as it was, and no ledger"

# ---------------------------------------------------------------- 13. a baseline above the truth
# The list's gate is a script of the fixture; what it prints is what its real ones print.
mkdir -p scripts
slack() { awk -v cmd="$1" '{ print } /^    gate: list-check$/ { seen = 1 } seen && /^    form: entries$/ { print "    slack:"; print "      command: " cmd; print "      says: tighten the baseline"; seen = 0 }' "$T/debt.yaml" > .ai/repo/ci/debt.yaml; }
printf '#!/bin/sh\necho "list-check: no new debt"\n' > scripts/list-check; chmod +x scripts/list-check
slack scripts/list-check; commit "the list's gate is asked for slack, and has none"
debt --release
exits 0 "a release whose baseline its gate calls exact"
says "INFO slack not measured, the gate cannot state it: numbers" "a baseline whose gate cannot state slack is not named"
printf '#!/bin/sh\necho "FIXED docs/A.md - tighten the baseline"\n' > scripts/list-check
commit "what the list accepted was fixed, and the baseline was not written again"
debt --release
exits 10 "a release whose baseline its own gate calls tightenable"
says "FAIL .ai/repo/list-baseline.txt declares 1 and its own gate says it can be tightened (scripts/list-check): write the baseline before the tag" "a baseline above the truth is not refused by name, count and command"
debt
exits 0 "a change over a baseline above the truth"
git rm -q scripts/list-check; commit "the gate the declaration names is gone"
debt --release
exits 10 "a release whose slack gate cannot be run"
says "its gate could not be asked whether the baseline is above the truth (scripts/list-check)" "a gate that could not be run is read as a gate with no slack"
cp "$T/debt.yaml" .ai/repo/ci/debt.yaml; commit "the declaration as it was"

# ---------------------------------------------------------------- 11. this repository's own declaration
own="$("$RB" release --repo "$ROOT" debt --format json 2>"$T/own.err")" \
  || { echo "    this repository's own debt is refused, or could not be counted:"; "$RB" release --repo "$ROOT" debt 2>&1 | grep -E '^FAIL' | sed 's/^/      /'; tail -2 "$T/own.err" | sed 's/^/      | /'; exit 1; }
[ "$(printf '%s' "$own" | jq -r '[.baselines[] | select(.payable | not) | .id] | join(" ")')" = commit-policy ] \
  || { echo "    the baselines this repository declares unpayable are not commit-policy alone: $(printf '%s' "$own" | jq -c '[.baselines[] | select(.payable | not) | .id]')"; exit 1; }
printf '%s' "$own" | jq -e '(.baselines | length) >= 18 and .total == ([.baselines[].count] | add) and .payable == ([.baselines[] | select(.payable) | .count] | add)' >/dev/null \
  || { echo "    this repository's totals are not the sums of its baselines"; exit 1; }

# ---------------------------------------------------------------- 8. no declaration
git rm -q .ai/repo/ci/debt.yaml; commit "the declaration is gone"
debt
exits 12 "a repository without the declaration"
case "$(cat "$T/err")" in *"nothing is known to be debt"*) ;; *) echo "    a missing declaration is not said to leave nothing known:"; sed 's/^/      | /' "$T/err" | tail -3; exit 1 ;; esac

echo "    standing still is refused at a release and one entry paid passes; a baseline that grew by one line, a counter that grew, an undeclared baseline and an unpayable one without a reason are refused in any change"
