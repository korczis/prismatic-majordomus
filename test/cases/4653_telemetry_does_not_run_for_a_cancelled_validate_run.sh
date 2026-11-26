# majordomus-covers: workflow:telemetry.yml
# claims: none
# Telemetry does not run for a validate run that was cancelled, and the workflow excludes
# exactly the conclusions its model names (I4524).
#
# .github/workflows/telemetry.yml triggers on workflow_run of validate, types [completed], and
# its one job, export, had no `if`. Cancelling a validate run completes it, so every cancel
# started an export job on ubuntu-latest. On 2026-10-30, from 13:30Z to 15:00Z, the landing
# seat cancelled superseded and hold-member validate runs so that train 12c's validate could
# take the hosted runners: 33 of the 34 validate runs that completed in that window were
# cancelled, 35 telemetry runs were created in it, and each took back the runner its cancel
# had freed, to trace a run whose jobs mostly never started. A job-level `if` is decided
# before a runner is requested, so a skipped job costs no runner. The model,
# .ai/repo/ci/telemetry.yaml, declares the conclusions that start no export in
# trigger.skip_conclusions, and the workflow is its adapter.
#
# What is proved, from the workflow and the model as YAML:
#   - the model's trigger.skip_conclusions is [cancelled];
#   - the export job's `if` is a conjunction of `github.event.workflow_run.conclusion !=
#     '<c>'` terms, nothing else, and the conclusions it names are exactly the model's, so
#     success, failure and timed_out are still exported;
#   - the workflow's workflow_run trigger is the model's (workflows, types);
#   - the judge refuses the workflow of 6b30ede2d13 (no `if`) when this clone has that commit,
#     and always the same workflow with the `if` removed, with the condition naming another
#     conclusion, inverted, or widened past the model, and a model that names another
#     conclusion or none, so it is not passing vacuously.
. "$ROOT/test/lib.sh"
mj_runs python3 || skip "$(mj_runs_why python3)"
python3 -c 'import yaml' 2>/dev/null || skip "python3 has no yaml module"

WF="$ROOT/.github/workflows/telemetry.yml"
MODEL="$ROOT/.ai/repo/ci/telemetry.yaml"
expect_file "$WF"
expect_file "$MODEL"

# judge <workflow> <model> -- exit 0 when the export job skips exactly the model's conclusions
judge() {
  python3 - "$1" "$2" <<'PY'
import re, sys, yaml
wf = yaml.safe_load(open(sys.argv[1], encoding='utf-8'))
model = yaml.safe_load(open(sys.argv[2], encoding='utf-8'))
def no(msg):
    print('    ' + msg)
    sys.exit(1)
trig = (model.get('trigger') or {})
skip = trig.get('skip_conclusions')
if skip != ['cancelled']:
    no('the model declares trigger.skip_conclusions %r, not [cancelled]' % (skip,))
# PyYAML reads the bare key `on` as the boolean true
on = wf.get('on', wf.get(True)) or {}
if on.get('workflow_run') != trig.get('workflow_run'):
    no('the workflow triggers on %r, the model on %r'
       % (on.get('workflow_run'), trig.get('workflow_run')))
job = (wf.get('jobs') or {}).get('export')
if not isinstance(job, dict):
    no('the workflow has no job `export`')
cond = job.get('if')
if not isinstance(cond, str) or not cond.strip():
    no('the export job has no `if`: a cancelled validate run starts it and takes a runner')
cond = cond.strip()
m = re.fullmatch(r'\$\{\{(.*)\}\}', cond, re.S)
if m:
    cond = m.group(1).strip()
term = re.compile(r"github\.event\.workflow_run\.conclusion\s*!=\s*'([a-z_]+)'")
named = []
for part in cond.split('&&'):
    t = term.fullmatch(part.strip())
    if not t:
        no('the export job\'s `if` carries %r, which is not a conclusion it excludes'
           % part.strip())
    named.append(t.group(1))
if sorted(set(named)) != sorted(set(skip)):
    no('the export job excludes %r, the model skips %r: the two disagree'
       % (sorted(set(named)), sorted(set(skip))))
print('    the export job runs for every completed validate run but %s, as the model says'
      % ', '.join(sorted(set(named))))
PY
}

judge "$WF" "$MODEL" || exit 1

# ---------------------------------------------------------------- the judge discriminates
# mutant <name> <file: wf|model> <python that rewrites `d`, the parsed file> -- refused
mutant() {
  src="$WF"; [ "$2" = model ] && src="$MODEL"
  python3 - "$src" "$T/$1.yml" "$3" <<'PY' || { echo "    mutant $1 could not be written"; exit 1; }
import sys, yaml
src, dst, edit = sys.argv[1:4]
d = yaml.safe_load(open(src, encoding='utf-8'))
exec(edit)
open(dst, 'w', encoding='utf-8').write(yaml.safe_dump(d, sort_keys=False))
PY
  w="$WF"; m="$MODEL"
  if [ "$2" = model ]; then m="$T/$1.yml"; else w="$T/$1.yml"; fi
  if out="$(judge "$w" "$m")"; then
    echo "    the judge passed mutant $1:"; echo "$out"; exit 1
  fi
}
mutant no-if wf "del d['jobs']['export']['if']"
mutant other-conclusion wf \
  "d['jobs']['export']['if'] = \"github.event.workflow_run.conclusion != 'failure'\""
mutant inverted wf \
  "d['jobs']['export']['if'] = \"github.event.workflow_run.conclusion == 'cancelled'\""
mutant widened wf \
  "d['jobs']['export']['if'] = \"github.event.workflow_run.conclusion != 'cancelled' && github.event.workflow_run.conclusion != 'failure'\""
mutant or-always wf \
  "d['jobs']['export']['if'] = \"github.event.workflow_run.conclusion != 'cancelled' || always()\""
mutant model-other model "d['trigger']['skip_conclusions'] = ['failure']"
mutant model-none model "del d['trigger']['skip_conclusions']"

# the workflow as it stood when the issue was measured, when this clone has that commit
if git -C "$ROOT" cat-file -e 6b30ede2d13:.github/workflows/telemetry.yml 2>/dev/null; then
  git -C "$ROOT" show 6b30ede2d13:.github/workflows/telemetry.yml > "$T/at-6b30ede2d13.yml"
  if out="$(judge "$T/at-6b30ede2d13.yml" "$MODEL")"; then
    echo "    the judge passed the workflow of 6b30ede2d13:"; echo "$out"; exit 1
  fi
fi

echo "    a cancelled validate run starts no telemetry job, and the workflow says what the model says"
