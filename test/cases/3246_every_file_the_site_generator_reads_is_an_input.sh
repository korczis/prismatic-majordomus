# majordomus-covers: none
# Every file the site generator reads by a literal "$ROOT/..." path is one of its declared
# inputs (I2847). `scripts/pages current` judges site/data/generated current by a fingerprint
# of `scripts/generate-site-data --inputs`; a file the generator reads outside that list can
# change without moving the fingerprint, and the site data then reads as current while it is
# stale. On 2026-10-21 the list missed its own programs (lib/, scripts/lib/, the script itself),
# the hand-written pages under site/content-src, the Pages model, artifacts.json,
# distribution.json and AGENTS.md. A derive that skips generation when the fingerprint
# matches (I2880) would have published stale data after editing any of them.
#
# This reads the script, not a trace: every "$ROOT/<literal>" outside a comment is a read or a
# write. A file named that way must be an input unless it is one of the script's outputs; a
# directory named that way must have every tracked file under it be an input unless it is an
# output or a scratch directory. A path built from a variable ("$ROOT/$p") is not judged here.
#
#   1  on this checkout, every literal read is covered
#   2  dropping a covered program from the list (scripts/lib/*.jq) is refused, naming the file
. "$ROOT/test/lib.sh"

GEN="$ROOT/scripts/generate-site-data"
# outputs and scratch the script writes, never reads as inputs; and docs/generated/artifacts.json,
# which derive's stage A writes before this script and stage C rewrites after it, so hashing it
# makes the fingerprint move on every derive and never read as current. The rows the script
# reads from it (the editors projection's skill files) are stage A's, a function of
# .ai/repo/skills/*/SKILL.md, which is an input (I2864, cancelled on that finding).
EXEMPT=" site/data/generated site/content docs/SITE_CLAIMS.md docs/PLAN_STATUS.md docs/PAGES_STATUS.md bin docs/generated/artifacts.json "

judge() {   # judge <generator> — prints one line per uncovered read, exit 0 when none
  local gen="$1" inputs bad=0 p f
  inputs="$(cd "$ROOT" && bash "$gen" --inputs)" || { echo "--inputs failed"; return 2; }
  # a match ending in "/" is the literal prefix of a path built from a variable ("$ROOT/docs/$p"),
  # which this case does not judge
  for p in $(grep -vE '^[[:space:]]*#' "$gen" | grep -oE '\$ROOT/[A-Za-z0-9_.][A-Za-z0-9_./-]*' \
             | grep -v '/$' | sed 's|^\$ROOT/||; s|[.]*$||' | LC_ALL=C sort -u); do
    case "$p" in .mj-stage*|"") continue ;; esac
    case "$EXEMPT" in *" $p "*) continue ;; esac
    if [ -f "$ROOT/$p" ]; then
      printf '%s\n' "$inputs" | grep -qxF "$p" || { echo "uncovered read: $p"; bad=1; }
    elif [ -d "$ROOT/$p" ]; then
      for f in $(git -C "$ROOT" ls-files -- "$p"); do
        printf '%s\n' "$inputs" | grep -qxF "$f" || { echo "uncovered read: $f (under $p)"; bad=1; }
      done
    fi
  done
  return "$bad"
}

# ---------------------------------------------------------------- 1. this checkout
out="$(judge "$GEN")" || { echo "    the generator reads files its --inputs does not declare:"; printf '      %s\n' "$out"; exit 1; }

# ---------------------------------------------------------------- 2. a dropped program is refused
# the script finds its tree from its own location, so the mutant runs in a tree of links to
# this checkout whose scripts/ holds the mutant in place of the real script
R="$T/repo"; mkdir -p "$R/scripts"
for e in "$ROOT"/* "$ROOT"/.[!.]*; do
  case "${e##*/}" in scripts) ;; *) ln -s "$e" "$R/${e##*/}" ;; esac
done
for e in "$ROOT"/scripts/*; do ln -s "$e" "$R/scripts/${e##*/}"; done
mutant="$R/scripts/generate-site-data"
rm -f "$mutant"
sed 's|"\$ROOT"/scripts/lib/\*\.jq ||' "$GEN" > "$mutant"
cmp -s "$GEN" "$mutant" && { echo "    the mutation did not apply: scripts/lib/*.jq is no longer named that way"; exit 1; }
rc=0; out="$(judge "$mutant")" || rc=$?
[ "$rc" = 1 ] || { echo "    dropping scripts/lib/*.jq from the inputs was not refused (rc $rc)"; exit 1; }
case "$out" in *"uncovered read: scripts/lib/cli-site.jq"*) ;; *) echo "    the refusal does not name scripts/lib/cli-site.jq:"; printf '      %s\n' "$out"; exit 1 ;; esac

echo "    every file the site generator reads by a literal path is a declared input; a dropped one is named"
