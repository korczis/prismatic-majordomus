# majordomus-covers: none
# The version bump has something that performs it, not only something that judges it.
#
# This repository measures the version from its public contract rather than from commit
# subjects (ADR 0051). That machinery is complete and it works:
#
#   - the judge: `release analyze` compares every capability, route, MCP tool, command and
#     both schemas of each against the last release. Run against this repository's own history
#     on 2026-09-15 it answered `none` since v0.6.0, `minor` since v0.5.0 and `minor, breaking`
#     since v0.3.1 — real movement, measured, not a fixture;
#   - the gate: `scripts/ci/version-matches-surface`, in the `rust` job;
#   - the writer: `majordomus release bump`, which raises every site that states a version at
#     once, and which test/cases/112 drives against a real fixture — including the refusal
#     paths, where a refused bump must write nothing.
#
# **Nothing ran the writer.** A repository-wide search found `release bump` only in error
# messages advising a person. So the surface could move, the gate would correctly refuse the
# next pull request that happened to notice, and in between the tree carried a version its own
# contract called too low — for as long as nobody looked. test/cases/112's own header names the
# older form of this defect: "the writer runs when a person raises the version and the gate ran
# afterwards, if at all." The gate was fixed to agree with the writer; nobody then arranged for
# the writer to run.
#
# What is asserted here is the wiring, because the tool's behaviour is 112's to hold:
#
#   1. something runs the judge on its own, on a clock as well as on a push;
#   2. it performs the bump with the tool rather than editing a version by hand;
#   3. it does NOT tag or release — the mutation, and the point. `release.yml` triggers on a
#      `v*` tag; creating that tag is a decision, and an automation that made it would be
#      deleting a decision somebody is entitled to make;
#   4. it does not propose the same version twice.
. "$ROOT/test/lib.sh"

WF="$ROOT/.github/workflows/version.yml"
[ -f "$WF" ] || { echo "    no .github/workflows/version.yml: the version is measured and gated,"
                  echo "    and nothing performs the bump it requires"; exit 1; }

python3 - "$WF" "$ROOT" <<'PY' || exit 1
import re, sys, yaml
wf = yaml.safe_load(open(sys.argv[1], encoding='utf-8'))
root = sys.argv[2]
on = wf.get(True) or wf.get('on') or {}
jobs = wf.get('jobs') or {}
steps = [s for j in jobs.values() for s in (j.get('steps') or [])]
runs = '\n'.join(str(s.get('run', '')) for s in steps)

# --- 1. it asks on its own
if 'schedule' not in on:
    print("    the workflow runs on no schedule: a push whose run was cancelled, or which"
          " failed for an unrelated reason, would skip the question silently and nothing"
          " would ask again")
    sys.exit(1)
if 'push' not in on:
    print("    the workflow does not run on a push, so a surface that moves waits for a clock")
    sys.exit(1)
print("    it asks on a push and on a clock, so a cancelled run does not skip the question")

# --- 2. the tool decides and the tool writes
if not re.search(r'release analyze', runs):
    print("    nothing in the workflow runs `release analyze`; the decision would be its own")
    sys.exit(1)
if not re.search(r'release bump', runs):
    print("    nothing in the workflow runs `release bump`: it can judge and not act, which is"
          " the defect it exists to remove")
    sys.exit(1)
# a workflow writing a version itself would put back the two-writers defect ADR 0051 removed
for bad in (r'sed -i.*version', r'>\s*.*Cargo\.toml', r'MJ_VERSION='):
    if re.search(bad, runs):
        print(f"    the workflow writes a version itself ({bad}); `release bump` exists because"
              " two writers disagreeing is the defect ADR 0051 removed")
        sys.exit(1)
print("    the tool measures and the tool writes; the workflow edits no version by hand")

# --- 3. THE MUTATION: it must not tag or release
for bad in (r'git tag', r'gh release create', r'refs/tags'):
    if re.search(bad, runs):
        print(f"    the workflow tags or releases ({bad}). release.yml triggers on a `v*` tag,"
              " so this would turn a decision into a side effect — and make it automatically,"
              " on a schedule, without anybody choosing it")
        sys.exit(1)
if not re.search(r'gh pr create', runs):
    print("    the workflow neither tags nor opens a pull request, so the bump it performs"
          " reaches nobody")
    sys.exit(1)
print("    it proposes and does not tag: the release stays a decision a person makes")

# --- 4. it does not propose the same version twice
if not re.search(r'gh pr list', runs):
    print("    the workflow does not look for an open proposal before opening one; every push"
          " would raise another, burying the review the first is waiting for")
    sys.exit(1)
print("    an open proposal for the same version is not raised again")
PY

# --- 5. the decision rule is the one the tool states, driven both ways
# The rule is a comparison of two fields the tool publishes. Driven here over both answers,
# because a rule only ever exercised on "nothing is owed" is a rule nobody has seen work.
decide() { [ "$1" = "$2" ] && echo no || echo yes; }
[ "$(decide 0.6.1 0.6.1)" = no ]  || { echo "    the rule proposes a bump when none is owed"; exit 1; }
[ "$(decide 0.6.1 0.7.0)" = yes ] || { echo "    the rule proposes nothing when a bump is owed"; exit 1; }
echo "    the rule proposes exactly when the required version differs from the declared one"

# and the workflow uses that comparison rather than a different one
grep -q 'declared" = "\$required\|declared" = "$required' "$WF" \
  || { echo "    the workflow's own comparison is not the one asserted above"; exit 1; }
echo "    and the workflow compares those two fields"

echo "    the version bump has an actor, and it proposes rather than releases"
