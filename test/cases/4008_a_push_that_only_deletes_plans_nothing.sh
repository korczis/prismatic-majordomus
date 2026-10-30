# claim: derived-data-current
# A push that only deletes refs plans nothing (I4008, project.a-push-runs-what-ci-would-find-first).
# On 2026-10-22 a push of 16 ref deletions ran the whole pre-push hook and timed out twice on the
# shared build lock. The hook now exits 0 when every pushed ref's local sha is all zeros, as it
# does for refs/majordomus/*. This case runs the real hook in a fixture whose finish, case-numbers
# and run-plan are stubs that leave a sentinel: deletions leave none, a mixed push leaves all.
. "$ROOT/test/lib.sh"

fail() { printf '    %s\n' "$*"; exit 1; }
Z=0000000000000000000000000000000000000000
A=1111111111111111111111111111111111111111

mkdir -p fx/.githooks fx/bin fx/scripts/ci
cp "$ROOT/.githooks/pre-push" fx/.githooks/pre-push
printf '#!/bin/sh\necho finish >> "$SENTINEL"\n' > fx/bin/majordomus
printf '#!/bin/sh\necho case-numbers >> "$SENTINEL"\n' > fx/scripts/case-numbers
printf '#!/bin/sh\necho run-plan >> "$SENTINEL"\n' > fx/scripts/ci/run-plan
chmod +x fx/bin/majordomus fx/scripts/case-numbers fx/scripts/ci/run-plan
export SENTINEL="$PWD/sentinel"

hook() { (cd fx && sh .githooks/pre-push origin url) ; }

: > "$SENTINEL"
printf 'refs/heads/a %s refs/heads/a %s\nrefs/heads/b %s refs/heads/b %s\n' "$Z" "$A" "$Z" "$A" | expect_exit 0 hook
[ ! -s "$SENTINEL" ] || fail "a deletion-only push ran: $(tr '\n' ' ' < "$SENTINEL")"

# a deletion beside a continuity-store update still carries no source
: > "$SENTINEL"
printf 'refs/heads/a %s refs/heads/a %s\nrefs/majordomus/x %s refs/majordomus/x %s\n' "$Z" "$A" "$A" "$Z" | expect_exit 0 hook
[ ! -s "$SENTINEL" ] || fail "deletion plus refs/majordomus ran: $(tr '\n' ' ' < "$SENTINEL")"

# a mixed push (one deletion, one update) still runs the whole hook
: > "$SENTINEL"
printf 'refs/heads/a %s refs/heads/a %s\nrefs/heads/c %s refs/heads/c %s\n' "$Z" "$A" "$A" "$Z" | expect_exit 0 hook
for s in finish case-numbers run-plan; do
  grep -qx "$s" "$SENTINEL" || fail "a mixed push did not run $s: $(tr '\n' ' ' < "$SENTINEL")"
done
