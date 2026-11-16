# majordomus-covers: none
# The github-projection apply job checks out the trunk's whole history without its blobs, and
# its checkout is bounded at ten minutes or less, below its job (I4592).
#
# .github/workflows/github.yml checked out the default branch with `fetch-depth: 0` and no
# filter, bounded at 5 minutes. That fetch of every blob timed out on master pushes 6f8e104
# (run 43338034433, both attempts), aca34e6 (43626240578, both attempts), c992617
# (43657141312) and 04db703 (43673907137): "The action 'Run actions/checkout@v4' has timed
# out after 5 minutes". Until somebody reran it, apply had not run and every open pull
# request's github-check failed with "record(s) never-created". What the job reads of that
# history is commits and trees (rev-list --min-age, merge-base, log --name-only); HEAD's blobs
# come with the checkout and an older commit's are fetched on demand. A blobless full-history
# checkout took 18 s in validate run 42367964807 against 159-347 s for full ones (I3241).
#
# What is proved, from the workflow as YAML:
#   - the apply job has exactly one actions/checkout, of the default branch, with
#     fetch-depth 0 (the gate reads the age of a landing from the history) and
#     filter blob:none;
#   - that step's timeout-minutes is a whole number of at most 10 and below the job's own;
#   - the judge refuses the same workflow with the filter removed, with the bound raised to
#     11, with the bound removed and with a shallow checkout, so it is not passing vacuously.
. "$ROOT/test/lib.sh"

WF="$ROOT/.github/workflows/github.yml"
[ -f "$WF" ] || { echo "    no .github/workflows/github.yml"; exit 1; }

# judge <workflow> -- exit 0 when the apply checkout is blobless and bounded; prints why not
judge() {
  python3 - "$1" <<'PY'
import sys, yaml
wf = yaml.safe_load(open(sys.argv[1], encoding='utf-8'))
def no(msg):
    print('    ' + msg)
    sys.exit(1)
job = (wf.get('jobs') or {}).get('apply')
if not isinstance(job, dict):
    no('the workflow has no job `apply`')
steps = job.get('steps') or []
co = [s for s in steps if str(s.get('uses', '')).startswith('actions/checkout@')]
if len(co) != 1:
    no('the apply job checks out %d times, not once' % len(co))
step = co[0]
w = step.get('with') or {}
if w.get('ref') != '${{ github.event.repository.default_branch }}':
    no('the checkout takes %r, not the default branch' % w.get('ref'))
if w.get('fetch-depth') != 0:
    no('the checkout is not the whole history (fetch-depth %r): the gate reads the age of a'
       ' landing from it' % w.get('fetch-depth'))
if w.get('filter') != 'blob:none':
    no('the checkout fetches every blob of the history (filter %r), not blob:none: the full'
       ' fetch timed out at 5 minutes on master runs 43338034433 and 43626240578'
       % w.get('filter'))
t = step.get('timeout-minutes')
if not isinstance(t, int) or isinstance(t, bool):
    no('the checkout declares no whole-number timeout-minutes (%r)' % t)
if t > 10:
    no('the checkout is bounded at %d minutes, more than 10' % t)
jb = job.get('timeout-minutes')
if not isinstance(jb, int) or t >= jb:
    no('the checkout bound %r is not below the job bound %r' % (t, jb))
print('    the apply checkout is the whole history, blobless, bounded at %d of the job\'s %d'
      ' minutes' % (t, jb))
PY
}

judge "$WF" || exit 1

# ---------------------------------------------------------------- the judge discriminates
# mutant <name> <python that rewrites the checkout step `s`> -- the judge must refuse it
mutant() {
  python3 - "$WF" "$T/$1.yml" "$2" <<'PY' || { echo "    mutant $1 could not be written"; exit 1; }
import sys, yaml
src, dst, edit = sys.argv[1:4]
wf = yaml.safe_load(open(src, encoding='utf-8'))
s = [x for x in wf['jobs']['apply']['steps']
     if str(x.get('uses', '')).startswith('actions/checkout@')][0]
exec(edit)
open(dst, 'w', encoding='utf-8').write(yaml.safe_dump(wf, sort_keys=False))
PY
  if out="$(judge "$T/$1.yml")"; then
    echo "    the judge passed mutant $1:"; echo "$out"; exit 1
  fi
}
mutant no-filter "del s['with']['filter']"
mutant bound-11 "s['timeout-minutes'] = 11"
mutant no-bound "del s['timeout-minutes']"
mutant shallow "s['with']['fetch-depth'] = 1"

echo "    the github-projection apply checkout is blobless and bounded at 10 minutes or less"
