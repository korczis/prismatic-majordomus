# majordomus-covers: none
# majordomus-timeout: 3600
# No derived page grows past the scope's size limit, so one derive is a fixpoint (I4512).
#
# On 2026-10-30 docs/PLAN_STATUS.md, written by stage B of scripts/derive as one page for every
# milestone and issue, crossed .ai/repo/scope.yaml's out.max_bytes on the branch whose records
# grew the plan. A text file over the limit is out of the index; stage C reads the index, so
# it wrote a registry projection that no longer matched what stage B had fingerprinted, and
# only a second derive converged. The plan status is now an index page and one page per
# milestone, and scripts/ci/derived-size-check refuses a derived file the index judges by size
# once it passes the gate's fraction of the limit. This case holds both:
#
#   1  on a clone of this commit whose limit the plan status crosses as one page (the index
#      and every milestone page together are over it), one run of scripts/derive is followed
#      by a second that changes nothing
#   2  after it, no derived file the index judges by size is above the gate's fraction, the
#      index and the largest milestone page are in the index, and every milestone has its
#      page, linked from the index
#   3  the negative control: a milestone page grown past the fraction is refused, and the
#      refusal names the file, its size and the limit
#
# The fixture is a clone (objects shared with this checkout, which it never writes), and its
# derive runs under a lock of its own, so it neither waits behind nor holds up the machine's
# real derive. Stage B runs without the use-case scenarios: what is under test is the
# derivation's fixpoint, and the scenarios are case 10's.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN

PERCENT="$(sed -n 's/^PERCENT=\([0-9][0-9]*\)$/\1/p' "$ROOT/scripts/ci/derived-size-check")"
[ -n "$PERCENT" ] || { echo "    scripts/ci/derived-size-check states no PERCENT"; exit 1; }

W="$T/clone"
git clone -q --shared "$ROOT" "$W" 2>/dev/null || { echo "    could not clone the checkout"; exit 1; }
cd "$W" || exit 1
git config user.email t@example.com && git config user.name t && git config core.hooksPath /dev/null \
  || { echo "    could not configure the clone"; exit 1; }
[ -f docs/PLAN_STATUS.md ] && [ -d docs/plan-status ] \
  || { echo "    this commit has no plan status index and milestone pages to start from"; exit 1; }

# The plan status as one page would be the index and every milestone page together. The
# fixture's limit is nine tenths of that, so the one page would leave the index, while the
# largest page still has to clear the gate's fraction of it by construction.
pages() { wc -c docs/PLAN_STATUS.md docs/plan-status/*.md | awk '$2 != "total"'; }
one="$(pages | awk '{ s += $1 } END { print s + 0 }')"
max=$(( one * 9 / 10 ))
grep -q '^  max_bytes: [0-9][0-9]*$' .ai/repo/scope.yaml || { echo "    .ai/repo/scope.yaml declares no out.max_bytes"; exit 1; }
sed -i.bak "s/^  max_bytes: [0-9][0-9]*\$/  max_bytes: $max/" .ai/repo/scope.yaml && rm -f .ai/repo/scope.yaml.bak
got="$(env -u MAJORDOMUS_SHARE "$RB" scope --repo "$W" --format json 2>/dev/null | jq -r '.declaration.out.max_bytes')"
[ "$got" = "$max" ] || { echo "    the fixture's limit is $got, not $max"; exit 1; }
[ "$one" -gt "$max" ] || { echo "    the plan status as one page ($one bytes) does not cross the fixture's limit ($max)"; exit 1; }
git commit -qam "fixture: a limit the plan status crosses as one page" || { echo "    could not commit the fixture"; exit 1; }

derive() {
  env -u MAJORDOMUS_SHARE MAJORDOMUS_BIN="$RB" MAJORDOMUS_DERIVE_NO_SCENARIOS=1 \
    MAJORDOMUS_DERIVE_LOCK="$T/derive.lock" MAJORDOMUS_DERIVE_PRIORITY="$T/derive.priority" \
    MAJORDOMUS_DERIVE_QUEUE="$T/derive.queue" MAJORDOMUS_DERIVE_TIMINGS= \
    scripts/derive > "$T/derive.$1.out" 2>&1 \
    || { echo "    derive $1 failed:"; tail -20 "$T/derive.$1.out" | sed 's/^/    | /'; exit 1; }
}

# 1 -------------------------------------------------------------------------------------
derive 1
git add -A && git commit -qm "the first derive" --allow-empty || { echo "    could not commit the first derive"; exit 1; }
derive 2
changed="$(git status --porcelain --untracked-files=all)"
[ -z "$changed" ] || {
  echo "    the second derive changed what the first wrote; one derive is not a fixpoint:"
  printf '%s\n' "$changed" | head -20 | sed 's/^/    | /'; exit 1; }

# 2 -------------------------------------------------------------------------------------
one="$(pages | awk '{ s += $1 } END { print s + 0 }')"
[ "$one" -gt "$max" ] || { echo "    after the derive the plan status as one page ($one bytes) no longer crosses $max; the case proves nothing"; exit 1; }
rc=0; MJ_ROOT="$W" "$W/scripts/ci/derived-size-check" > "$T/size.out" 2>&1 || rc=$?
[ "$rc" = 0 ] || { echo "    a derived file is above $PERCENT% of the fixture's limit (exit $rc):"; sed 's/^/    | /' "$T/size.out"; exit 1; }
largest="$(pages | sort -n | tail -n 1 | awk '{ print $2 }')"
env -u MAJORDOMUS_SHARE "$RB" scope --repo "$W" --format json docs/PLAN_STATUS.md "$largest" 2>/dev/null > "$T/verdicts.json"
jq -e 'length == 2 and all(.[]; .verdict == "in")' "$T/verdicts.json" >/dev/null \
  || { echo "    the index page or the largest milestone page ($largest) is not in the index:"; jq -c '.[]' "$T/verdicts.json" | sed 's/^/    | /'; exit 1; }
for m in $(jq -r '.milestones[].id' site/data/generated/plan.json); do
  [ -f "docs/plan-status/$m.md" ] || { echo "    milestone $m has no page under docs/plan-status/"; exit 1; }
  grep -qF "](plan-status/$m.md)" docs/PLAN_STATUS.md || { echo "    the index does not link docs/plan-status/$m.md"; exit 1; }
done
n_pages="$(find docs/plan-status -type f | wc -l | tr -d ' ')"
n_m="$(jq '.milestones | length' site/data/generated/plan.json)"
[ "$n_pages" = "$n_m" ] || { echo "    $n_pages page(s) under docs/plan-status/ for $n_m milestone(s)"; exit 1; }

# 3 -------------------------------------------------------------------------------------
grow=$(( max * PERCENT / 100 + 1 - $(wc -c < "$largest") ))
head -c "$grow" /dev/zero | tr '\0' 'x' >> "$largest"
size="$(wc -c < "$largest" | tr -d ' ')"
rc=0; MJ_ROOT="$W" "$W/scripts/ci/derived-size-check" > "$T/size.out" 2>&1 || rc=$?
[ "$rc" = 10 ] || { echo "    a milestone page above $PERCENT% of the limit was not refused (exit $rc)"; sed 's/^/    | /' "$T/size.out"; exit 1; }
grep -qE "^FAIL derived-size +$largest is $size bytes, .* of out\.max_bytes $max " "$T/size.out" \
  || { echo "    the refusal does not name the file, its size and the limit:"; sed 's/^/    | /' "$T/size.out"; exit 1; }
git checkout -q -- "$largest"
