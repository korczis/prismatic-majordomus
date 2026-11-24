# majordomus-covers: none
# A record cites a brief where it is kept, never where it was staged (ADR 0236, I4714).
#
# A pack in progress lives in the primary checkout's ignored tmp/owner-packs/ (older ones in
# tmp/prompt-packs/). A plan record or an ADR that cites a file there cites bytes only one disk
# holds: a clone, a linked worktree and CI cannot open them. scripts/ci/campaigns-check step 5
# fails each such citation unless .ai/repo/ci/campaigns-staged-citations.txt names it, and fails
# a line of that list whose citation is gone, so the list only shrinks. Each branch is planted
# here: a clean tree, a staged citation in an issue and in an ADR, the legacy directory, a
# sentence-final full stop, an allowed line, a stale allowed line, and tmp/packs/, which is
# pack output and not a staging place.
. "$ROOT/test/lib.sh"

CHK="$ROOT/scripts/ci/campaigns-check"
[ -x "$CHK" ] || { echo "    scripts/ci/campaigns-check is not executable"; exit 1; }

F="$T/tree"
mkdir -p "$F/campaigns/alpha-pack" "$F/.ai/repo/project/issues" "$F/.ai/repo/adrs" "$F/.ai/repo/ci"
run_check() { rc=0; LAST_OUT="$(MJ_ROOT="$F" "$CHK" 2>&1)" || rc=$?; return 0; }
show() { printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; }

printf '# Alpha Pack\n\nWhat was asked for on the day.\n' > "$F/campaigns/alpha-pack/README.md"
{
  printf '# Campaign briefs\n\n| pack | of | files | first heading | taken from |\n|---|---|---|---|---|\n'
  printf '| `alpha-pack` | 2026-11-23 | 1 | Alpha Pack | `tmp/owner-packs/alpha-pack` |\n'
} > "$F/campaigns/README.md"
if command -v sha256sum >/dev/null 2>&1; then
  hash_all() { xargs -0 sha256sum; }
else
  hash_all() { xargs -0 openssl dgst -sha256 -r; }
fi
( cd "$F/campaigns" && find . -type f ! -path ./MANIFEST.sha256 -print0 | LC_ALL=C sort -z | hash_all \
    | sed 's|^\([0-9a-f]*\) [ *]\./|\1  |' > MANIFEST.sha256 )

ISSUE="$F/.ai/repo/project/issues/I1.yaml"
ADR="$F/.ai/repo/adrs/0001-a-decision.md"
ALLOW="$F/.ai/repo/ci/campaigns-staged-citations.txt"
issue() { printf 'id: I1\nwhy: "%s"\n' "$1" > "$ISSUE"; }

# ---------------------------------------------------------------- 1. cited where it is kept
# The index row's "taken from" names the staging place: that is history, in campaigns/, and
# not a citation by a record.
issue 'Planned from campaigns/alpha-pack/README.md.'
printf '# 1. A decision\n\nThe brief is campaigns/alpha-pack.\n' > "$ADR"
run_check
[ "$rc" = 0 ] || { echo "    a record citing the kept pack exited $rc, not 0"; show; exit 1; }

# ---------------------------------------------------------------- 2. an issue cites the staged copy
issue 'Planned from campaigns/alpha-pack, drafted in tmp/owner-packs/alpha-pack/README.md'
run_check
[ "$rc" = 10 ] || { echo "    an issue citing tmp/owner-packs exited $rc, not 10"; show; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -qF 'FAIL campaigns  .ai/repo/project/issues/I1.yaml — cites tmp/owner-packs/alpha-pack, a brief staged on one disk' \
  || { echo "    the finding does not name the record and the staged pack"; show; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -qF "[reproduce: grep -nF 'tmp/owner-packs/alpha-pack' .ai/repo/project/issues/I1.yaml]" \
  || { echo "    the finding carries no reproduce command"; show; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q '1 finding(s)' || { echo "    one citation did not make one finding"; show; exit 1; }

# ---------------------------------------------------------------- 3. the legacy place, at a full stop
issue 'Planned from campaigns/alpha-pack; the draft was tmp/prompt-packs/alpha-pack.'
run_check
[ "$rc" = 10 ] || { echo "    a citation of tmp/prompt-packs exited $rc, not 10"; show; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -qF 'cites tmp/prompt-packs/alpha-pack, a brief' \
  || { echo "    the legacy citation is not named, or its full stop was kept"; show; exit 1; }

# ---------------------------------------------------------------- 4. an ADR cites the staged copy
issue 'Planned from campaigns/alpha-pack.'
printf '# 1. A decision\n\nThe brief is tmp/owner-packs/alpha-pack/01.md.\n' > "$ADR"
run_check
[ "$rc" = 10 ] || { echo "    an ADR citing tmp/owner-packs exited $rc, not 10"; show; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -qF 'FAIL campaigns  .ai/repo/adrs/0001-a-decision.md — cites tmp/owner-packs/alpha-pack' \
  || { echo "    the ADR's citation is not named"; show; exit 1; }

# ---------------------------------------------------------------- 5. the allowed line admits it
printf '# grandfathered\n.ai/repo/adrs/0001-a-decision.md tmp/owner-packs/alpha-pack  # cited before the rule\n' > "$ALLOW"
run_check
[ "$rc" = 0 ] || { echo "    a citation the list names exited $rc, not 0"; show; exit 1; }

# ---------------------------------------------------------------- 6. a stale allowed line fails
printf '# 1. A decision\n\nThe brief is campaigns/alpha-pack.\n' > "$ADR"
run_check
[ "$rc" = 10 ] || { echo "    a list line whose citation is gone exited $rc, not 10"; show; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -qF 'FAIL campaigns  .ai/repo/ci/campaigns-staged-citations.txt — names .ai/repo/adrs/0001-a-decision.md tmp/owner-packs/alpha-pack, and that file no longer cites it' \
  || { echo "    the stale line is not named"; show; exit 1; }
rm -f "$ALLOW"

# ---------------------------------------------------------------- 7. tmp/packs is pack output
# `majordomus pack` writes there, and a record may describe what it wrote.
issue 'Planned from campaigns/alpha-pack. pack build wrote tmp/packs/oscilla-chatgpt-1234/ inside the checkout.'
run_check
[ "$rc" = 0 ] || { echo "    a record describing tmp/packs output exited $rc, not 0"; show; exit 1; }

# ---------------------------------------------------------------- 8. the repository's own tree
# Every staged citation the repository holds is named in its list, and no line is stale.
rc=0; LAST_OUT="$(MJ_ROOT="$ROOT" "$CHK" 2>&1)" || rc=$?
[ "$rc" = 0 ] || { echo "    the repository's own tree exited $rc, not 0"; show; exit 1; }
exit 0
