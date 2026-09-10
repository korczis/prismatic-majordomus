# majordomus-covers: evidence
# majordomus-negative: evidence check finish
# A task owes what its `requires` declares, and evidence discharges an obligation only
# while it still describes the tree it was taken over. This case starts a task that owes
# something, watches `finish --outcome completed` refuse it while nothing is recorded,
# records evidence and watches the same command accept it, then changes a file the
# obligation names and watches the evidence go stale rather than stay true. It also proves
# the honest cases: a token nobody declared is refused, narrative is refused, and an
# outcome that is not completed is never refused for owing something.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj103.XXXXXX")"; trap 'rm -rf "$S"' EXIT
"$MJ" init >/dev/null; "$MJ" update >/dev/null
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nx\n' > "$S/partial.md"
mkdir -p lib && echo a > lib/a && git add -A >/dev/null && git commit -qm base

# the task record carries `requires` beside `scope`, and `start --requires` writes it. This
# helper is the *other* way in: it replaces the list on a task that is already active, which
# no command does, so the sections below can hand one task a different set of things to owe
# without restarting it and losing the evidence recorded against it.
owes() {
  python3 - "$@" <<'PY'
import io, re, sys
p = '.ai/local/state/current.yaml'
s = io.open(p, encoding='utf-8').read()
s = re.sub(r'(?m)^requires:\n(?:  - .*\n)*', '', s)
block = 'requires:\n' + ''.join('  - %s\n' % t for t in sys.argv[1:])
s = s.replace('outcome:', block + 'outcome:', 1)
io.open(p, 'w', encoding='utf-8').write(s)
PY
}

# ---------------------------------------------------------------- no task owes nothing
expect_exit 2 "$MJ" evidence
expect_grep 'usage: majordomus evidence'
expect_exit 12 "$MJ" evidence --covers tests --command 'bash test/run.sh'
expect_grep 'no active task'

# ---------------------------------------------------------------- start declares them
# `requires` has a producer: a token the vocabulary does not declare is refused here, at the
# beginning, rather than at the moment a worker tries to discharge it and is told the task
# never asked for it.
expect_exit 2 "$MJ" start "obligations" --scope lib/ --requires nonsense
expect_grep "'nonsense' is not an obligation"
expect_exit 0 "$MJ" start "obligations" --scope lib/ --requires tests
expect_grep 'requires=tests'
grep -q '^requires:' .ai/local/state/current.yaml || { echo "    start --requires wrote no requires block"; exit 1; }
# a token the vocabulary does not know
expect_exit 2 "$MJ" evidence --covers nonsense --command x
expect_grep "no obligation 'nonsense'"
# a token the vocabulary knows but this task never promised
expect_exit 15 "$MJ" evidence --covers docs --command x
expect_grep "the active task does not require 'docs'"
# narrative is not evidence
expect_exit 2 "$MJ" evidence --covers tests
expect_grep 'narrative is not evidence'

# ---------------------------------------------------------------- owed and unproved
expect_exit 10 "$MJ" finish --outcome completed --note "done"
expect_grep 'FAIL obligation +tests — owed, and no evidence was recorded'
# an honest outcome is never refused for owing something
expect_exit 0 "$MJ" check
expect_grep 'obligation +tests — owed, and no evidence was recorded \(not refused'

# ---------------------------------------------------------------- recorded, then accepted
expect_exit 0 "$MJ" evidence --covers tests --type test --command 'bash test/run.sh'
expect_grep 'evidence: tests recorded'
expect_exit 0 "$MJ" check
expect_grep 'OK   obligation  tests — discharged over inputs'

# ---------------------------------------------------------------- the point: staleness
echo 'changed' >> lib/a
git add -A >/dev/null && git commit -qm change
expect_exit 10 "$MJ" finish --outcome completed --note "done"
expect_grep 'it no longer describes what it proved'
# recording it again over the new tree discharges it
expect_exit 0 "$MJ" evidence --covers tests --type test --command 'bash test/run.sh'
expect_exit 0 "$MJ" check
expect_grep 'OK   obligation  tests — discharged over inputs'

# ---------------------------------------------------------------- the vocabulary is data
expect_exit 0 "$MJ" doctrine show majordomus.obligation-closure
expect_grep 'validator.*obligations'

# ------------------------------------------------- the pathspecs reach every depth
# `docs` is taken over `.ai/repo/**/README.md`, and the hash is only worth anything if that
# selects every README at every depth. It did not: the specs were word-split unquoted, so
# bash globbed them before git ever saw them, and without `globstar` a `**` matches exactly
# one directory level. The nested READMEs were outside the hash, and a change to one of them
# left the evidence looking fresh — this feature's own failure mode, inside the feature.
# A nested README is what proves it, because a top-level one passes either way.
owes docs
nested="$(git ls-files -- '.ai/repo/**/README.md' | awk -F/ 'NF >= 5' | head -1)"
[ -n "$nested" ] || { echo "    no README nested two levels under .ai/repo; the case cannot prove the depth"; exit 1; }
expect_exit 0 "$MJ" evidence --covers docs --command 'scripts/ci/reference-check'
expect_exit 0 "$MJ" check
expect_grep 'OK   obligation  docs — discharged over inputs'
echo 'a line the obligation is supposed to notice' >> "$nested"
git add -A >/dev/null && git commit -qm nested
expect_exit 10 "$MJ" finish --outcome completed --note "done"
expect_grep 'docs — the evidence was taken over inputs .* it no longer describes what it proved'


# ================================================================ the outer obligations
# Six tokens name a fact outside the working tree. Five of them are facts something already
# holds — git, and the publication probe `scripts/pages verify` — and the point of the rest
# of this case is that the tool establishes those itself: it discharges them without a
# ledger line, and it refuses a ledger line that says otherwise. The vocabulary of staleness
# stays the four values `mj_git_label` already uses; nothing here invents a fifth.

# the data and the code that establishes it are paired in both directions, which is the
# failure this section would otherwise hide: a token declared establishable with nothing to
# establish it passes silently, and one declared unestablishable with no reason is a gap
# nobody wrote down
python3 - "$ROOT" <<'PY'
import io, re, sys
root = sys.argv[1]
voc = io.open(root + '/share/obligations.yaml', encoding='utf-8').read()
lib = io.open(root + '/lib/evidence.sh', encoding='utf-8').read()
bad = []
for block in re.split(r'(?m)^  - id: ', voc)[1:]:
    tok = block.split('\n', 1)[0].strip()
    by = re.search(r'(?m)^    established_by: (.*)$', block)
    if not by:
        bad.append('%s declares no established_by' % tok); continue
    if by.group(1).strip() == 'none':
        if not re.search(r'(?m)^    unestablished: \S', block):
            bad.append('%s is established by nothing and says no reason why' % tok)
    elif ('mj_obl_est_%s()' % tok) not in lib:
        bad.append('%s says %r establishes it and no mj_obl_est_%s exists' % (tok, by.group(1), tok))
if bad:
    print('    ' + '\n    '.join(bad)); sys.exit(1)
PY

owes commit push target pages
# the pages section puts a probe shim under scripts/, which the scope validator would
# otherwise report as work outside the claimed scope — a true finding about a different rule
python3 - <<'PY'
import io, re
p = '.ai/local/state/current.yaml'
s = io.open(p, encoding='utf-8').read()
io.open(p, 'w', encoding='utf-8').write(re.sub(r'(?m)^scope:\n(?:  - .*\n)*', 'scope:\n  - lib\n  - scripts\n', s, count=1))
PY

# ---------------------------------------------------------------- commit
# work in the tree is not work in the history, and git is asked rather than the worker
echo 'outer' >> lib/a
expect_exit 10 "$MJ" finish --outcome completed --note "done"
expect_grep 'FAIL obligation +commit — 1 file\(s\) the task touched are still in the working tree'
git add -A >/dev/null && git commit -qm outer
expect_exit 0 "$MJ" check
expect_grep 'OK   obligation  commit — exact: the tree is clean'

# ---------------------------------------------------------------- push, with no remote
# Nothing here can settle it, so the recorded line is consulted exactly as it always was,
# and judged against the commit it was taken at.
expect_grep 'push — owed, and no evidence was recorded \(the checkout has no remote'
expect_exit 0 "$MJ" evidence --covers push --command 'git push'
expect_exit 0 "$MJ" check
expect_grep 'OK   obligation  push — discharged at this commit'

# ---------------------------------------------------------------- push, with a remote
# The same recorded line, now against a checkout where the question can be answered: the
# answer wins. This is the whole point — a worker cannot hand-record a fact git holds.
git init -q --bare "$S/remote.git"
git remote add origin "$S/remote.git"
expect_exit 0 "$MJ" check
expect_grep 'push — no remote-tracking ref reaches'
B="$(git branch --show-current)"
git push -q -u origin "$B"
git remote set-head origin "$B" >/dev/null
expect_exit 0 "$MJ" check
expect_grep "OK   obligation  push — exact: origin/$B contains"
expect_grep "OK   obligation  target — exact: origin/$B reaches"

# ---------------------------------------------------------------- one commit further on
# Pushed and integrated are statements about a commit, not about a branch. A commit made
# after the push is neither, and both tokens say so in their own words.
echo 'further' >> lib/a && git add -A >/dev/null && git commit -qm further
expect_exit 10 "$MJ" finish --outcome completed --note "done"
expect_grep 'push — no remote-tracking ref reaches'
expect_grep "target — origin/$B does not reach"
git push -q origin "$B"
expect_exit 0 "$MJ" check
expect_grep "OK   obligation  target — exact: origin/$B reaches"

# ---------------------------------------------------------------- pages
# The probe is `scripts/pages verify`, which has answered this question since it was
# written and had one caller — the Pages workflow. The sandbox reaches the real script and
# substitutes only the URL, so what runs here is the shipped probe against a real server.
mkdir -p "$S/public" scripts
if start_http "$S/public"; then
  printf '#!/bin/sh\nexec "%s/scripts/pages" "$@" --url "%s"\n' "$ROOT" "$HTTP_BASE" > scripts/pages
  chmod +x scripts/pages
  printf '{"commit":"%s"}\n' "$(git rev-parse HEAD)" > "$S/public/build.json"
  expect_exit 0 "$MJ" check
  expect_grep 'OK   obligation  pages — exact: the published site serves'

  # the site serves an older commit: unpublished, and said in those words
  printf '{"commit":"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef"}\n' > "$S/public/build.json"
  expect_exit 10 "$MJ" finish --outcome completed --note "done"
  expect_grep 'pages — after 0 s .* still serves deadbeef'

  # and a hand-recorded line does not rescue what the probe refutes
  expect_exit 0 "$MJ" evidence --covers pages --command 'scripts/pages verify'
  expect_exit 0 "$MJ" check
  expect_grep 'pages — after 0 s .* still serves deadbeef'

  # unreachable is not the same fact as unpublished. The probe exits 12 rather than 10,
  # nothing is established, and the recorded line stands — a laptop with no network must
  # not be able to fail a task it cannot measure.
  stop_http
  expect_exit 0 env MJ_PAGES_PROBE_SECONDS=2 "$MJ" check
  expect_grep 'OK   obligation  pages — discharged at this commit'
  rm -f scripts/pages
else
  echo "    (no python3 or node: the pages probe was not exercised)"
fi

# ---------------------------------------------------------------- deploy and verify
# Left to a person on purpose, and the vocabulary says why rather than leaving a reader to
# infer it. A deployment is a fact about a machine this repository never contacts.
owes deploy verify
expect_exit 10 "$MJ" finish --outcome completed --note "done"
expect_grep 'FAIL obligation +deploy — owed, and no evidence was recorded'
expect_no_grep 'deploy — .*could not be established'
expect_exit 0 "$MJ" evidence --covers deploy --type manual --command 'majordomus deployment'
expect_exit 0 "$MJ" evidence --covers verify --type manual --artifact 'https://example.invalid/ready'
expect_exit 0 "$MJ" check
expect_grep 'OK   obligation  deploy — discharged at this commit'
expect_grep 'OK   obligation  verify — discharged at this commit'
# and the hand-held record still goes stale the moment the branch moves past it
echo 'after' >> lib/a && git add -A >/dev/null && git commit -qm after
expect_exit 10 "$MJ" finish --outcome completed --note "done"
expect_grep 'deploy — the evidence names .*, and the branch has moved since'

# ================================================ an honest outcome is not stranded
# `start` refuses while a task is active and `finish` is the only way to close one, so a
# scope that turns out too narrow used to leave a worker with no move at all: scope-integrity
# refused every outcome, including the ones that say the work did not finish. A completed
# finish is still refused; a non-completed one names the files as warnings and closes the
# record. Its own task, because it ends by closing it.
expect_exit 0 "$MJ" finish --outcome partial --note "$S/partial.md"
expect_exit 0 "$MJ" start "a scope that turns out too narrow" --scope lib/
mkdir -p elsewhere && echo out > elsewhere/f && git add -A >/dev/null && git commit -qm outside
expect_exit 10 "$MJ" finish --outcome completed --note "$S/partial.md"
expect_grep 'FAIL scope +elsewhere/f — outside claimed scope'
expect_exit 0 "$MJ" finish --outcome partial --note "$S/partial.md"
expect_grep 'WARN scope +elsewhere/f — outside claimed scope'
expect_grep 'outcome is partial, not completed, so scope does not refuse it'
