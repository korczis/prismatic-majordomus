# majordomus-covers: plan
# claims: github-projection
# An issue joins a closed milestone by number (I4529). `gh issue create --milestone <title>`
# and `gh issue edit --milestone <title>` resolve the title among the OPEN milestones only,
# and the milestone of a DONE record is closed on GitHub, by this adapter's own milestone
# pass, before its issues are projected: an issue recorded under a finished milestone could
# never be created ("could not add to milestone"), and --apply failed on every run of a
# repository that had finished one milestone and planned another issue in it (first seen by
# the adopter able-cz/flight-agent-empty-legs at v1.0.0-alpha.8). The adapter now creates
# the issue without a milestone and joins it by number through the REST field, which accepts
# a closed milestone. Moving an existing issue (`gh issue edit --milestone <title>`) is unchanged:
# case 894 pins that call and its failure accounting; its closed-milestone hole is a follow-up.
#
# What is proved, against a stub forge that answers as gh does — a title named with
# --milestone is refused when the listing says that milestone is closed — and records every
# call; a fixture is never applied, so `gh` is a stub on PATH and the trunk a local bare
# remote, as in case 45:
#   1  a new issue of a closed milestone is created without --milestone, then joined to it
#      by number: PATCH repos/<r>/issues/<n> milestone=<number>, and the run passes
#   2  the creation names no milestone by title
. "$ROOT/test/lib.sh"
SYNC="$ROOT/scripts/github-sync"
"$MJ" init >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
pj_init
pj_milestone M000
pj_milestone M001 1
pj_issue I0001 M000
pj_issue I0002 M000
git add -A >/dev/null; git commit -qm "the plan" >/dev/null

STUB="$T/stub"; mkdir -p "$STUB"; CALLS="$T/gh.calls"
FX_I="$T/fx_issues"; FX_M="$T/fx_ms"
cat > "$STUB/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$MJ_STUB_CALLS"
case "$1 $2" in "auth status"|"label list") exit 0 ;; esac
case "$*" in
  *"--paginate repos/"*"/milestones"*) cat "$MJ_STUB_MS"; exit 0 ;;
  *"issue list"*)                      cat "$MJ_STUB_IS"; exit 0 ;;
  *"api graphql"*)                     wc -l < "$MJ_STUB_IS" | tr -d ' '; exit 0 ;;
  *"issue create"*"--milestone "*|*"issue edit"*"--milestone "*)
    # as gh: a title is resolved among the open milestones only
    t="${*#*--milestone }"
    if awk -F'\t' -v t="$t" '$2==t && $3=="closed" {f=1} END {exit !f}' "$MJ_STUB_MS"; then
      echo "could not add to milestone '$t'" >&2; exit 1
    fi
    case "$*" in *"issue create"*) echo "https://github.com/example/fixture/issues/99" ;; esac; exit 0 ;;
  *"issue create"*)                    echo "https://github.com/example/fixture/issues/99"; exit 0 ;;
  *"api -X PATCH repos/"*"/issues/"*)  echo '{}'; exit 0 ;;
esac
exit 0
STUB
chmod +x "$STUB/gh"
printf '/stub/\n/remote.git/\n/gh.calls\n/fx_issues\n/fx_ms\n' >> .git/info/exclude
git init -q --bare "$T/remote.git"
git -C "$T/remote.git" fetch -q --no-prune "$T" HEAD:refs/heads/trunk
git -C "$T/remote.git" symbolic-ref HEAD refs/heads/trunk
git remote add origin "$T/remote.git"; git fetch -q origin; git remote set-head origin trunk >/dev/null
# one remote issue row from a body on stdin: number, title, state, body, milestone, identity, created
row() { # <number> <title> <milestone title> <id>
  b="$(cat | base64 | tr -d '\n')"
  printf '%s\t%s\topen\t%s\t%s\t%s\t2000-01-01T00:00:00Z\n' "$1" "$2" "$b" "$3" "$4"
}
# GitHub as the adopter had it, projected long before these records landed: M000 finished and
# closed there, I0001 on it, I0002 recorded under it and never created
"$SYNC" --render I0001 | row 21 'I0001 — Issue I0001' 'M000 — Milestone M000' I0001 > "$FX_I"
printf '1\tM000 — Milestone M000\tclosed\t2000-01-01T00:00:00Z\n2\tM001 — Milestone M001\topen\t2000-01-01T00:00:00Z\n' > "$FX_M"
apply() {
  : > "$CALLS"
  PATH="$STUB:$PATH" MJ_GH_PACE=0 MJ_STUB_MS="$FX_M" MJ_STUB_IS="$FX_I" MJ_STUB_CALLS="$CALLS" \
    "$SYNC" --apply
}

# ---------------------------------------------------------------- 1  created, then joined by number
expect_exit 0 apply
expect_grep 'OK +created +issue I0002 \(#99\)'
expect_grep 'OK +updated +issue I0002 \(#99\) milestone — joined to M000 \(#1\) by number'
grep -qE '^issue create .*I0002 — ' "$CALLS" || { echo "    I0002 was not created"; cat "$CALLS"; exit 1; }
grep -qE '^api -X PATCH repos/example/fixture/issues/99 -F milestone=1$' "$CALLS" \
  || { echo "    I0002 was not joined to its closed milestone by number"; cat "$CALLS"; exit 1; }
expect_no_grep 'FAIL +created'

# ---------------------------------------------------------------- 2  the creation names no milestone by title
if grep -E '^issue create ' "$CALLS" | grep -qE -- '--milestone '; then
  echo "    the creation names a milestone by title, which gh resolves among open milestones only:"; grep -E '^issue create ' "$CALLS"; exit 1
fi
