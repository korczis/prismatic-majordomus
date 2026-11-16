# majordomus-covers: none
# A landing that changes what GitHub shows applies the change itself
# (project.the-projection-is-applied-at-landing).
#
# GitHub's issues and milestones are a projection of the plan on the trunk. Until 2026-10-12
# the projection reached GitHub when a person ran `scripts/github-sync --apply` from master
# after a landing, and `scripts/ci/github-check` refused master once the window declared in
# .ai/repo/ci/github.yaml had passed. On that day it did: I2460 to I2478 stayed behind while
# the executor was busy, and validate's run 38278173940 turned master red at 81178f5362 for a
# step nothing performed. The judge had no actor.
#
# The actor is .github/workflows/github.yml, an adapter over .ai/repo/ci/github.yaml. This
# case holds the two to each other, and then takes the steps' own scripts out of the workflow
# and runs them against a scripted adapter and a local remote, so what is proved is what the
# runner executes and not a pattern found in a file. It needs no network and no token.
#
# Every assertion names the mutation that breaks it. Each was applied on 2026-10-12, one at a
# time, to the workflow (W) or the model (M), and each failed this case at the line it says:
#
#   W  a `paths:` filter under the push trigger                          -> 1
#   W  a `pull_request` trigger                                          -> 1
#   M  another cron in trigger.schedule                                  -> 1
#   W  `contents: write` beside the declared permissions                 -> 2
#   WM `contents: write` in the workflow and the model together          -> 2
#   W  GH_TOKEN in the build step's environment                          -> 2
#   W  `cancel-in-progress: true`                                        -> 3
#   W  a step without `timeout-minutes`                                  -> 4
#   W  `fetch-depth: 0` removed from the checkout                        -> 5
#   W  the `git remote set-head` step removed                            -> 5
#   W  the gate replaced by `scripts/github-sync --plan`                 -> 6
#   W  the gate's condition replaced by `always()`                       -> 6
#   W  `--force` added to the apply                                      -> 7
#   W  the executable guard removed from the apply step                  -> 8
#   W  exit 12 of the first reading taken for "nothing is owed"          -> 9
#   W  the question "is this still the tip" replaced by `false`          -> 10
. "$ROOT/test/lib.sh"
# The fixture's git must answer about the fixture: under a hook these name another repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX

WF="$ROOT/.github/workflows/github.yml"
MODEL="$ROOT/.ai/repo/ci/github.yaml"
fail() { printf '    %s\n' "$*"; exit 1; }
[ -f "$MODEL" ] || fail "no .ai/repo/ci/github.yaml: the projection has no model"
[ -f "$WF" ] || fail "no .github/workflows/github.yml: the projection is judged by a gate and applied by nothing"

STEPS="$T/steps"; mkdir -p "$STEPS"

# ---------------------------------------------------------------- the workflow against the model
# The three steps that run a phase are written to $STEPS/<phase id>.sh for the second half.
python3 - "$WF" "$MODEL" "$STEPS" <<'PY' || exit 1
import sys, yaml
wf_path, model_path, out = sys.argv[1:4]
wf = yaml.safe_load(open(wf_path, encoding='utf-8'))
model = yaml.safe_load(open(model_path, encoding='utf-8'))

def no(msg):
    print('    ' + msg)
    sys.exit(1)

on = wf.get(True) or wf.get('on') or {}
trigger = model.get('trigger') or {}
actor = model.get('actor') or {}
phases = model.get('phases') or []
if actor.get('workflow') != '.github/workflows/github.yml':
    no("the model names no actor.workflow, or another file: %r" % actor.get('workflow'))

# --- 1. the landing starts it, every landing, and nothing a pull request can reach
want = {'push'}
if trigger.get('schedule'): want.add('schedule')
if trigger.get('dispatch'): want.add('workflow_dispatch')
if set(on) != want:
    no("the workflow's triggers are %s and the model declares %s; a trigger the model does not"
       " declare (a pull request's above all) would project a tree that is not the trunk"
       % (sorted(map(str, on)), sorted(want)))
push = on.get('push') or {}
if push.get('branches') != trigger.get('push', {}).get('branches'):
    no("the push trigger names %r and the model %r" % (push.get('branches'), trigger.get('push', {}).get('branches')))
extra = sorted(set(push) - {'branches'})
if extra:
    no("the push trigger carries %s: with a filter a landing can change a rendering and start no"
       " run, and a superseded run could no longer rely on its successor" % extra)
crons = [s.get('cron') for s in (on.get('schedule') or [])]
if crons != [trigger.get('schedule')]:
    no("the workflow's schedule is %r and the model declares %r" % (crons, trigger.get('schedule')))
print("    every push to %s starts it, with no path filter, and so does the declared clock" % push.get('branches'))

# --- 2. issues: write and contents: read, and nothing broader, and the token only where it is used
ALLOWED = {'contents': 'read', 'issues': 'write'}
if model.get('permissions') != ALLOWED:
    no("the model declares the permissions %r; the projection needs %r and nothing broader"
       % (model.get('permissions'), ALLOWED))
if wf.get('permissions') != model.get('permissions'):
    no("the workflow's permissions are %r and the model declares %r" % (wf.get('permissions'), model.get('permissions')))
jobs = wf.get('jobs') or {}
for name, job in jobs.items():
    if 'permissions' in job:
        no("the job %s declares permissions of its own; they are declared once, for the workflow" % name)
print("    the token may write issues and read contents, and nothing else")

# --- 3. one at a time, never cancelled
conc = wf.get('concurrency') or {}
if conc.get('group') != trigger.get('concurrency'):
    no("the concurrency group is %r and the model declares %r" % (conc.get('group'), trigger.get('concurrency')))
if conc.get('cancel-in-progress') is not False:
    no("the workflow may cancel a run in progress; a cancelled apply is a remote left half written")
print("    one run at a time, and a newer landing never cancels an apply")

# --- 4. the job is bounded and every step is bounded under it
if list(jobs) != [actor.get('job')]:
    no("the workflow's jobs are %r and the model declares the one job %r" % (list(jobs), actor.get('job')))
job = jobs[actor['job']]
bound = job.get('timeout-minutes')
if bound != actor.get('bound'):
    no("the job's bound is %r minutes and the model declares %r" % (bound, actor.get('bound')))
steps = job.get('steps') or []
def label(s): return s.get('name') or s.get('id') or s.get('uses') or '(unnamed)'
for s in steps:
    t = s.get('timeout-minutes')
    if not isinstance(t, int) or t <= 0 or t >= bound:
        no("the step '%s' declares the bound %r under a job of %r minutes; a step that hangs must"
           " report itself before its job is killed" % (label(s), t, bound))
    if s.get('continue-on-error'):
        no("the step '%s' is allowed to fail" % label(s))
print("    the job is bounded at %d minutes and each of its %d steps under it" % (bound, len(steps)))

# --- 5. the trunk's tip, with its history, and the ref the adapter finds the trunk through
checkout = [s for s in steps if str(s.get('uses', '')).startswith('actions/checkout@')]
if len(checkout) != 1:
    no("the job checks out %d times" % len(checkout))
w = checkout[0].get('with') or {}
if w.get('fetch-depth') != 0:
    no("the checkout is shallow (fetch-depth %r): the gate reads the age of a landing from the"
       " history and refuses a clone that cannot show it" % w.get('fetch-depth'))
if w.get('ref') != '${{ github.event.repository.default_branch }}':
    no("the checkout takes %r, not the default branch: the adapter projects the trunk's tip" % w.get('ref'))
runs = [(i, str(s.get('run', ''))) for i, s in enumerate(steps)]
head_at = [i for i, r in runs if 'git remote set-head origin --auto' in r]
if not head_at:
    no("no step sets refs/remotes/origin/HEAD (git remote set-head origin --auto); --apply finds"
       " the trunk through it and refuses a clone without it")

# --- 6. ask, apply, prove: each phase of the model once, in the model's order
if [p.get('id') for p in phases] != ['owed', 'apply', 'prove']:
    no("the model's phases are %r, not owed, apply, prove" % [p.get('id') for p in phases])
at = []
for p in phases:
    hits = [i for i, r in runs if any(line.strip().startswith(p['runs']) for line in r.splitlines())]
    if len(hits) != 1:
        no("the phase %s runs `%s`, which %d step(s) of the workflow run as a command of their own"
           % (p['id'], p['runs'], len(hits)))
    at.append(hits[0])
if at != sorted(at) or len(set(at)) != 3:
    no("the phases run in the order %r; the model's order is owed, apply, prove" % at)
if head_at[0] > at[1]:
    no("refs/remotes/origin/HEAD is set after the apply that needs it")
owed, apply, prove = (steps[i] for i in at)
if owed.get('id') != 'owed' or apply.get('id') != 'apply':
    no("the steps the conditions below read are not named owed and apply")
if 'if' in owed:
    no("the first reading is conditional (%r)" % owed['if'])
if apply.get('if') != "steps.owed.outputs.owed == 'yes'":
    no("the apply runs on the condition %r, not when something is owed" % apply.get('if'))
if prove.get('if') != "steps.owed.outputs.owed == 'yes' && steps.apply.outputs.superseded == 'no'":
    no("the gate runs on the condition %r, not exactly when this run applied: after a superseded"
       " run it would judge a trunk this run did not project, and anywhere else it is skipped" % prove.get('if'))
print("    it asks, applies and proves, in the model's order, and the gate runs exactly when it applied")

# the token reaches the three phases and nothing else: a dependency's build script never sees it
if 'env' in wf and any('token' in str(v).lower() for v in wf['env'].values()):
    no("the workflow hands its token to every step")
if any('token' in str(v).lower() for v in (job.get('env') or {}).values()):
    no("the job hands its token to every step, the build of every dependency among them")
for i, s in enumerate(steps):
    has = any('token' in str(v).lower() for v in (s.get('env') or {}).values())
    if has != (i in at):
        no("the step '%s' %s the token; it belongs to the three phases and to nothing else"
           % (label(s), 'carries' if has else 'runs a phase without'))

# --- 7. what the actor may not do
everything = '\n'.join(r for _, r in runs)
for bad, why in (('--force', "a region a person edited would be overwritten with nobody deciding it"),
                 ('--write-baseline', "the drift baseline is a ceiling a person lowers"),
                 ('MJ_GH_FIXTURE', "a fixture is a remote to observe")):
    if bad in everything:
        no("the workflow passes %s: %s" % (bad, why))
print("    it never forces a person's text and never writes the baseline")

# the executable is built here and named to the adapter by the variable the model declares
var = model.get('executable')
if var != 'MAJORDOMUS_BIN':
    no("the model names the executable through %r" % var)
if not any('cargo build' in r and '--locked' in r and (var + '=') in r and 'GITHUB_ENV' in r for _, r in runs[:at[0]]):
    no("no step before the first reading builds the executable (cargo build --locked) and exports %s" % var)

for p, i in zip(phases, at):
    open('%s/%s.sh' % (out, p['id']), 'w', encoding='utf-8').write(steps[i]['run'])
PY

# ---------------------------------------------------------------- 11. the one run a pull request may start
# Version 2 of the rule admits one workflow a pull request reaches: the pull request
# projection, on pull_request_target, from the default branch and never the head, reading
# issues and writing none. Every other workflow that a pull request reaches stays outside
# this rule; the issue projection above has no such trigger.
#   W  pr-meta.yml checks out ${{ github.event.pull_request.head.sha }}             -> 11
#   W  pr-meta.yml asks for `issues: write`                                         -> 11
#   W  pr-meta.yml on `pull_request` instead of pull_request_target                 -> 11
python3 - "$ROOT/.github/workflows/pr-meta.yml" "$ROOT/.ai/repo/ci/pr-meta.yaml" <<'PY' || exit 1
import sys, yaml
wf_path, model_path = sys.argv[1:3]
wf = yaml.safe_load(open(wf_path, encoding='utf-8'))
model = yaml.safe_load(open(model_path, encoding='utf-8'))
def no(msg):
    print('    ' + msg); sys.exit(1)
on = wf.get(True) or wf.get('on') or {}
if 'pull_request' in on:
    no("pr-meta.yml runs on pull_request; the record asks for pull_request_target, whose run is the base's")
if 'pull_request_target' not in on:
    no("pr-meta.yml is not started by a pull request at all")
if (on['pull_request_target'] or {}).get('types') != model['trigger']['pull_request_target']['types']:
    no("pr-meta.yml's event types are not the model's")
want = {'pull-requests': 'write', 'issues': 'read', 'contents': 'read'}
if wf.get('permissions') != want or model.get('permissions') != want:
    no("pr-meta.yml asks for %r (model %r); a pull request's run writes pull requests and reads"
       " issues, nothing broader" % (wf.get('permissions'), model.get('permissions')))
for name, job in (wf.get('jobs') or {}).items():
    if 'permissions' in job:
        no("the job %s of pr-meta.yml widens its permissions" % name)
    for st in job.get('steps') or []:
        if str(st.get('uses', '')).startswith('actions/checkout@'):
            ref = (st.get('with') or {}).get('ref')
            if ref != '${{ github.event.repository.default_branch }}':
                no("pr-meta.yml checks out %r; with a token that writes, only the default branch is run" % ref)
        if 'head' in str(st.get('run', '')) and 'github.event.pull_request.head' in str(st.get('run', '')):
            no("a step of pr-meta.yml interpolates the head into a command")
print("    the one run a pull request starts is pr-meta.yml, from the default branch, reading issues")
PY

# ---------------------------------------------------------------- the steps, run
# A tree with a remote, a scripted adapter and a scripted gate. The adapter records how it was
# called and exits as the scenario says; nothing here can reach GitHub.
gitq() { git -c user.email=t@example.com -c user.name=t -c core.hooksPath=/dev/null "$@"; }
TREE="$T/tree"; ORIGIN="$T/origin.git"; LOG="$T/calls.log"; OUT="$T/output"; BIN="$T/majordomus"
mkdir -p "$TREE/scripts/ci" "$T/runner"
gitq init -q --bare -b master "$ORIGIN"
gitq -C "$TREE" init -q -b master .
cat > "$TREE/scripts/github-sync" <<'SH'
#!/bin/sh
printf 'github-sync %s\n' "$*" >> "$CALLS"
echo "DRIFT  behind    issue I0001 — the canonical record moved"
echo "github-sync: 1 drift finding(s)"
exit "${SYNC_RC:-0}"
SH
cat > "$TREE/scripts/ci/github-check" <<'SH'
#!/bin/sh
printf 'github-check %s\n' "$*" >> "$CALLS"
exit "${GATE_RC:-0}"
SH
chmod +x "$TREE/scripts/github-sync" "$TREE/scripts/ci/github-check"
gitq -C "$TREE" add scripts
gitq -C "$TREE" commit -qm "the trunk"
gitq -C "$TREE" remote add origin "$ORIGIN"
gitq -C "$TREE" push -q origin master
printf '#!/bin/sh\nexit 0\n' > "$BIN"; chmod +x "$BIN"
: > "$T/not-executable"

# One step of the workflow, as the runner runs it: bash, in the checkout, with the runner's
# files and only the variables the scenario names.
#   step <phase> [NAME=value ...]
step() {
  local phase="$1"; shift
  : > "$LOG"; : > "$OUT"
  ( cd "$TREE" && env -u MAJORDOMUS_BIN -u SYNC_RC -u GATE_RC CALLS="$LOG" GITHUB_OUTPUT="$OUT" \
      RUNNER_TEMP="$T/runner" TRUNK=master "$@" bash -e "$STEPS/$phase.sh" )
}
calls() { tr '\n' ';' < "$LOG"; }

# --- 8. without the executable no phase reaches the adapter
for phase in owed apply prove; do
  expect_exit 12 step "$phase" || fail "the $phase step ran with no MAJORDOMUS_BIN"
  [ ! -s "$LOG" ] || fail "the $phase step reached the adapter with no MAJORDOMUS_BIN: $(calls)"
  expect_exit 12 step "$phase" MAJORDOMUS_BIN="$T/not-executable" || fail "the $phase step ran with a MAJORDOMUS_BIN that is not executable"
  [ ! -s "$LOG" ] || fail "the $phase step reached the adapter with a MAJORDOMUS_BIN that is not executable: $(calls)"
done
echo "    without the executable nothing is read, applied or judged"

# --- 9. the first reading: clean, owed, or not taken
expect_exit 0 step owed MAJORDOMUS_BIN="$BIN" SYNC_RC=0
[ "$(calls)" = "github-sync --check;" ] || fail "the first reading called: $(calls)"
grep -qx 'owed=no' "$OUT" || fail "a clean reading (exit status 0) did not say owed=no: $(cat "$OUT")"
expect_exit 0 step owed MAJORDOMUS_BIN="$BIN" SYNC_RC=11
grep -qx 'owed=yes' "$OUT" || fail "drift (exit 11) did not say owed=yes: $(cat "$OUT")"
expect_grep 'DRIFT  behind    issue I0001' || fail "the reading does not show what is owed"
expect_exit 12 step owed MAJORDOMUS_BIN="$BIN" SYNC_RC=12
[ ! -s "$OUT" ] || fail "a reading that could not be taken (exit 12) answered anyway: $(cat "$OUT")"
echo "    a clean reading owes nothing, drift is owed, and a reading not taken fails the run"

# --- 10. the apply: the tip is applied, a mutation GitHub refused fails, a moved trunk is left alone
expect_exit 0 step apply MAJORDOMUS_BIN="$BIN"
[ "$(calls)" = "github-sync --apply;" ] || fail "at the tip the apply called: $(calls)"
grep -qx 'superseded=no' "$OUT" || fail "an apply at the tip did not say superseded=no: $(cat "$OUT")"
expect_exit 13 step apply MAJORDOMUS_BIN="$BIN" SYNC_RC=13 || fail "a refused mutation (exit 13) did not fail the step"
expect_exit 15 step apply MAJORDOMUS_BIN="$BIN" SYNC_RC=15 || fail "a refused apply (exit 15) did not fail the step"

# the trunk moves on the remote, as the next landing moves it
OTHER="$T/other"
gitq clone -q "$ORIGIN" "$OTHER"
: > "$OTHER/landed"
gitq -C "$OTHER" add landed
gitq -C "$OTHER" commit -qm "the next landing"
gitq -C "$OTHER" push -q origin master
expect_exit 0 step apply MAJORDOMUS_BIN="$BIN"
[ ! -s "$LOG" ] || fail "a checkout the trunk has moved past still applied: $(calls)"
grep -qx 'superseded=yes' "$OUT" || fail "a superseded run did not say so: $(cat "$OUT")"
# and a remote that does not answer is not read as "still the tip"
gitq -C "$TREE" remote set-url origin "$T/no-such-remote.git"
if step apply MAJORDOMUS_BIN="$BIN" >/dev/null 2>&1; then fail "the apply ran against a remote that did not answer"; fi
[ ! -s "$LOG" ] || fail "the apply reached the adapter with no answer from the remote: $(calls)"
gitq -C "$TREE" remote set-url origin "$ORIGIN"
echo "    the tip is applied, a refusal fails the run, and a run the trunk has moved past applies nothing"

# --- 11. the gate's verdict is the step's
expect_exit 0 step prove MAJORDOMUS_BIN="$BIN"
[ "$(calls)" = "github-check ;" ] || fail "the proof called: $(calls)"
expect_exit 10 step prove MAJORDOMUS_BIN="$BIN" GATE_RC=10 || fail "a finding of the gate did not fail the step"
echo "    the gate judges the trunk after the apply, and its finding fails the run"

# ---------------------------------------------------------------- the fallback keeps its bound
# The window is what a failed run is bounded by. One declaration, read by the gate.
[ "$(grep -cE '^ *window: *[0-9]+ *$' "$MODEL")" = 1 ] || fail ".ai/repo/ci/github.yaml does not declare exactly one apply.window"
grep -q 'fallback: scripts/github-sync --apply' "$MODEL" || fail "the model names no fallback for a failed run"
echo "    the projection is applied at landing, and the window bounds the fallback"
