# A release follows the verdict: a tag is published only from a commit whose `ci` check passed.
#
# v0.3.1, v0.5.0, v0.6.0 and v0.7.0 were published from commits whose `ci` check-run concluded
# failure, because .github/workflows/release.yml never asked. `scripts/ci/release-verdict` is
# the question, and the release workflow's plan job is where it is asked, before any build.
#
# The gate is driven against a stub `gh` on PATH that answers the one endpoint it reads with a
# check-runs document this case writes. The subject is the decision and the words, not GitHub:
#
#   success   exit 0, and it says so
#   failure   exit 10, naming the conclusion — a red commit is not released
#   pending   exit 12 — a run still in progress is no verdict, and never a pass
#   missing   exit 12 — a commit the validate workflow never ran on has no verdict at all
#   newest    an older success does not outvote a newer failure
#   unread    gh failing is exit 12, not a pass
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/release-verdict"
[ -x "$GATE" ] || { echo "    scripts/ci/release-verdict is missing or not executable"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    skip: jq is required"; exit 0; }

# A repository with a tagged commit, so --tag resolves exactly as it does in the workflow.
R="$T/repo"; mkdir -p "$R"
( cd "$R" && git init -q . && git config user.email t@e && git config user.name t \
  && git config commit.gpgsign false && echo a > a && git add a \
  && git -c core.hooksPath=/dev/null commit -qm "feat: a thing" && git tag -a v9.9.9 -m v9.9.9 )
SHA="$(cd "$R" && git rev-parse HEAD)"

# The stub answers the check-runs endpoint of exactly this commit with $T/runs.json, and
# fails everything else; `gh` itself failing is selected by $T/gh-fails.
mkdir -p "$T/bin"
cat > "$T/bin/gh" <<STUB
#!/usr/bin/env bash
[ -f "$T/gh-fails" ] && { echo "HTTP 403: Resource not accessible by integration" >&2; exit 1; }
case "\$*" in
  "api repos/example/repo/commits/$SHA/check-runs?check_name=ci&per_page=100") cat "$T/runs.json"; exit 0 ;;
esac
echo "stub gh: unexpected call: \$*" >&2
exit 1
STUB
chmod +x "$T/bin/gh"

runs() { printf '{"total_count":%s,"check_runs":[%s]}\n' "$1" "$2" > "$T/runs.json"; }
run() { printf '{"id":%s,"name":"%s","status":"%s","conclusion":%s,"html_url":"https://example.invalid/run/%s"}' "$1" "$2" "$3" "$4" "$1"; }
gate() { ( cd "$R" && PATH="$T/bin:$PATH" GITHUB_REPOSITORY=example/repo "$GATE" "$@" ); }

echo "  a commit whose ci concluded success may be released"
runs 1 "$(run 7 ci completed '"success"')"
expect_exit 0 gate --tag v9.9.9 || exit 1
expect_grep "'ci' check of v9.9.9 at ${SHA:0:12} concluded success" || exit 1
expect_exit 0 gate --commit "$SHA" || exit 1
# HEAD and a short id are the same commit, asked about by its full id
expect_exit 0 gate --commit HEAD || exit 1
expect_exit 0 gate --commit "${SHA:0:9}" || exit 1

echo "  a commit whose ci failed is refused, and the refusal names the conclusion"
runs 1 "$(run 8 ci completed '"failure"')"
expect_exit 10 gate --tag v9.9.9 || exit 1
expect_grep 'concluded failure; a release follows the verdict' || exit 1
expect_grep 'https://example.invalid/run/8' || exit 1

echo "  a cancelled ci (a timeout renders as cancelled) is not a success either"
runs 1 "$(run 9 ci completed '"cancelled"')"
expect_exit 10 gate --tag v9.9.9 || exit 1

echo "  a ci still running is no verdict: 12, never a pass"
runs 1 "$(run 10 ci in_progress null)"
expect_exit 12 gate --tag v9.9.9 || exit 1
expect_grep 'is in_progress after [0-9]+s; no verdict yet' || exit 1

echo "  a commit with no ci check-run has no verdict: 12"
runs 1 "$(run 11 build completed '"success"')"
expect_exit 12 gate --tag v9.9.9 || exit 1
expect_grep "has no 'ci' check-run" || exit 1
runs 0 ""
expect_exit 12 gate --tag v9.9.9 || exit 1

echo "  the newest run is the verdict: an older success does not outvote a newer failure"
runs 2 "$(run 20 ci completed '"success"'),$(run 21 ci completed '"failure"')"
expect_exit 10 gate --tag v9.9.9 || exit 1
runs 2 "$(run 31 ci completed '"success"'),$(run 30 ci completed '"failure"')"
expect_exit 0 gate --tag v9.9.9 || exit 1

echo "  a check-runs answer that could not be read is 12, not a pass"
touch "$T/gh-fails"
expect_exit 12 gate --tag v9.9.9 || exit 1
expect_grep 'could not be read' || exit 1
rm -f "$T/gh-fails"
printf 'not json\n' > "$T/runs.json"
expect_exit 12 gate --tag v9.9.9 || exit 1

echo "  a tag the clone does not have is 12, and usage is 2"
expect_exit 12 gate --tag v0.0.0-absent || exit 1
expect_exit 12 gate --commit no-such-ref || exit 1
expect_exit 2 gate || exit 1

# ---------------------------------------------------------------- the workflow asks it
#
# The script is only the question; the release is safe only if the workflow asks it in the
# plan job, before any build or publication, with the permission to read check-runs.
WF="$ROOT/.github/workflows/release.yml"
echo "  release.yml asks the verdict in plan, before anything is built or published"
awk '/^  plan:/{p=1} /^  build:/{p=0} p && /scripts\/ci\/release-verdict/{found=1} END{exit !found}' "$WF" \
  || { echo "    the plan job of release.yml does not run scripts/ci/release-verdict"; exit 1; }
awk '/^  plan:/{p=1} /^  build:/{p=0} p && /checks: read/{found=1} END{exit !found}' "$WF" \
  || { echo "    the plan job cannot read check-runs: it asks for no checks: read"; exit 1; }
grep -q '^    needs: plan$' "$WF" || { echo "    the build job no longer waits for plan"; exit 1; }
exit 0
