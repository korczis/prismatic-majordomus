# majordomus-covers: skills
# A skill's `version` says that its procedure changed, and a worker who read version 2 trusts
# that version 2 still says what it said. So a SKILL.md whose body differs from the one at the
# merge-base with the trunk owes a higher version there (I4337: the evolve skill's step 7 asked
# for it and nothing decided it; cases 95 and 391 check only that the version is an integer).
#
# What is proved: on a fixture repository, a branch that changes a skill's body without raising
# its version is refused naming the skill and both versions; raising it, adding a new skill and
# changing only the front matter pass. Then the same judge reads this repository's skills
# against their merge-base with the trunk.
. "$ROOT/test/lib.sh"

# judge <repository> <base>: every skill whose body differs from <base>'s has a higher version
judge() {
  repo="$1"; base="$2"; bad=0
  while IFS= read -r f; do
    git -C "$repo" cat-file -e "$base:$f" 2>/dev/null || continue # new at this branch
    body_now="$(awk 'n >= 2; /^---$/ { n++ }' "$repo/$f")"
    body_was="$(git -C "$repo" show "$base:$f" | awk 'n >= 2; /^---$/ { n++ }')"
    [ "$body_now" = "$body_was" ] && continue
    v_now="$(sed -n 's/^version: *//p' "$repo/$f" | head -1)"
    v_was="$(git -C "$repo" show "$base:$f" | sed -n 's/^version: *//p' | head -1)"
    if ! [ "${v_now:-0}" -gt "${v_was:-0}" ] 2>/dev/null; then
      echo "$f: the body changed since $base and the version did not rise above $v_was (now $v_now)"; bad=1
    fi
  done <<EOF
$(git -C "$repo" ls-files '.ai/repo/skills/*/SKILL.md')
EOF
  return "$bad"
}

skill() { # <id> <version> <procedure line>
  mkdir -p ".ai/repo/skills/$1"
  printf -- '---\nschema: skill/v1\nid: %s\nversion: %s\ntitle: Skill %s\ndescription: %s\nstatus: active\n---\n# Purpose\n\nWhy.\n\n# Procedure\n\n1. %s\n\n# Output\n\nA report.\n' \
    "$1" "$2" "$1" "${4:-The procedure.}" "$3" > ".ai/repo/skills/$1/SKILL.md"
}
git init -q -b master . 2>/dev/null || git init -q .
skill alpha 1 'Do it.'; skill beta 3 'Read it.'
git add -A && git commit -qm base && git branch -f trunk
git checkout -q -b work

# a body changed with its version kept is refused, naming the skill and both versions
skill alpha 1 'Do it, then check it.'
git add -A && git commit -qm "alpha changed, version kept"
expect_exit 1 judge . "$(git merge-base HEAD trunk)" || exit 1
expect_grep 'alpha/SKILL.md: the body changed since [0-9a-f]* and the version did not rise above 1 \(now 1\)' || exit 1
expect_no_grep 'beta' || exit 1

# a version that went down is refused as well
skill beta 2 'Read it twice.'
git add -A && git commit -qm "beta lowered"
expect_exit 1 judge . "$(git merge-base HEAD trunk)" || exit 1
expect_grep 'beta/SKILL.md: .* did not rise above 3 \(now 2\)' || exit 1

# raising each version, adding a skill and changing only the front matter all pass
skill alpha 2 'Do it, then check it.'; skill beta 4 'Read it twice.'
skill gamma 1 'New.'
git add -A && git commit -qm "versions raised, gamma added"
expect_exit 0 judge . "$(git merge-base HEAD trunk)" || exit 1
skill alpha 2 'Do it, then check it.' 'A better description.'
git add -A && git commit -qm "front matter only"
expect_exit 0 judge . "$(git merge-base HEAD trunk)" || exit 1

# ---------------------------------------------------------------- this repository
trunk=""
for ref in origin/master master; do
  git -C "$ROOT" rev-parse -q --verify "$ref^{commit}" >/dev/null 2>&1 && { trunk="$ref"; break; }
done
[ -n "$trunk" ] || skip_case "the checkout has no trunk ref, so its skills were not judged against one"
base="$(git -C "$ROOT" merge-base HEAD "$trunk" 2>/dev/null)" \
  || skip_case "HEAD shares no history with $trunk here, so its skills were not judged"
judge "$ROOT" "$base" || { echo "    a skill changed without raising its version (base $base)"; exit 1; }
