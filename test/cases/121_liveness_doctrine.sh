# The four liveness rules, checked as documents against the checkout rather than a fixture.
#
# ADR 0039 landed them as four separate rule objects, all advisory, because none of them has
# a validator yet and `project.rule-is-a-doctrine` reserves an x-majordomus block for a rule
# the tool actually enforces. Both halves of that are load-bearing and both are easy to lose
# later: a worker adding a scan may set `class: blocking` without adding the x-majordomus
# block, or add the block without a validator function behind it.
#
# What is proved here: all four documents exist with the identities ADR 0039 fixed; each
# carries the front-matter fields the rules README requires; each declares a class the
# repository can back, so advisory-without-validator and blocking-with-validator both pass
# and blocking-without-validator fails; and every depends_on reference resolves to a rule
# that really exists at the version named, so the set loads instead of erroring as a
# missing dependency.
. "$ROOT/test/lib.sh"

RULES="$ROOT/.ai/repo/rules/project"
[ -d "$RULES" ] || { echo "    $RULES is missing"; exit 1; }

ids="project.execution-state-is-authoritative
project.every-wait-is-bounded
project.commands-run-non-interactively
project.recovery-is-idempotent"

# every id in the doctrine resolves to exactly one file that claims it
for id in $ids; do
  matches="$(grep -l "^id: $id\$" "$RULES"/*.v1.md 2>/dev/null | wc -l | tr -d ' ')"
  [ "$matches" = 1 ] || { echo "    $id is claimed by $matches files, expected exactly 1"; exit 1; }
done

field() { sed -n "s/^$2: //p" "$1" | head -1; }

for id in $ids; do
  f="$(grep -l "^id: $id\$" "$RULES"/*.v1.md)"

  # the front matter the rules README requires of every rule
  for key in version kind title description statement status class; do
    v="$(field "$f" "$key")"
    [ -n "$v" ] || { echo "    $id: front matter is missing '$key'"; exit 1; }
  done

  [ "$(field "$f" kind)" = rule ] || { echo "    $id: kind is not 'rule'"; exit 1; }
  [ "$(field "$f" status)" = active ] || { echo "    $id: status is not 'active'"; exit 1; }

  # a class the repository can actually back: blocking requires an x-majordomus block,
  # and that block requires a validator function that exists in lib/
  class="$(field "$f" class)"
  case "$class" in
    advisory)
      grep -q '^x-majordomus:' "$f" &&
        { echo "    $id: advisory but carries an x-majordomus block"; exit 1; }
      ;;
    blocking)
      grep -q '^x-majordomus:' "$f" ||
        { echo "    $id: blocking with no x-majordomus block; see project.rule-is-a-doctrine"; exit 1; }
      validator="$(sed -n 's/^  validator: //p' "$f" | head -1)"
      [ -n "$validator" ] ||
        { echo "    $id: x-majordomus block names no validator"; exit 1; }
      grep -rq "mj_validate_$validator()" "$ROOT/lib" ||
        { echo "    $id: no mj_validate_$validator() in lib/"; exit 1; }
      ;;
    *)
      echo "    $id: class '$class' is neither advisory nor blocking"; exit 1
      ;;
  esac

  # every declared dependency resolves to a rule that exists at the version named
  deps="$(sed -n 's/^depends_on: \[\(.*\)\]$/\1/p' "$f" | head -1 | tr ',' ' ')"
  for dep in $deps; do
    dep_id="${dep%@*}"; dep_ver="${dep##*@}"
    dep_id="$(echo "$dep_id" | tr -d ' ')"; dep_ver="$(echo "$dep_ver" | tr -d ' ')"
    [ -n "$dep_id" ] || continue
    target="$(grep -l "^id: $dep_id\$" "$RULES"/*.md "$ROOT/.ai/repo/rules/vendor"/*/*.md 2>/dev/null | head -1)"
    [ -n "$target" ] || { echo "    $id: depends_on $dep_id, which no rule declares"; exit 1; }
    have="$(field "$target" version)"
    [ "$have" = "$dep_ver" ] ||
      { echo "    $id: depends_on $dep_id@$dep_ver but that rule is version $have"; exit 1; }
  done
done

# the doctrine's own dependency spine: the other three rest on the first
for id in project.every-wait-is-bounded project.commands-run-non-interactively; do
  f="$(grep -l "^id: $id\$" "$RULES"/*.v1.md)"
  grep -q 'project.execution-state-is-authoritative@1' "$f" ||
    { echo "    $id does not depend on project.execution-state-is-authoritative@1"; exit 1; }
done

echo "    4 liveness rules: identities unique, front matter complete, classes backed, dependencies resolve"
