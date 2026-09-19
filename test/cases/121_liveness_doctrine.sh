# The four liveness rules, checked as documents against the checkout rather than a fixture.
#
# ADR 0039 landed them as four separate rule objects, all advisory, because none of them has
# a validator yet and `project.rule-is-a-doctrine` reserves an x-majordomus block for a rule
# the tool actually enforces. Both halves of that are load-bearing and both are easy to lose
# later: a worker adding a scan may set `class: blocking` without adding the x-majordomus
# block, or add the block without a validator function behind it.
#
# What is proved here: all four documents exist with the identities ADR 0039 fixed; each
# carries the front-matter fields the rules README requires; each declares an enforcement
# block the repository can back, in whichever of the two modes it claims — a named validator
# must exist as a function in lib/, and a block without one must name the tests that prove
# it instead; and every depends_on reference resolves to a rule that really exists at the
# version named, so the set loads instead of erroring as a missing dependency.
#
# Since the scans ADR 0039 named as follow-ups exist, the doctrine also has a shape: the two
# mechanically decidable rules are blocking and each names the scan that decides it, and the
# two the ADR reserved for review are advisory and say why in reviewed_because. That split is
# asserted below, so neither half can drift — a rule cannot be quietly lowered back to
# advisory once its scan exists, and one cannot be raised to blocking while its only
# declaration is that review decides.
#
# Whether a rule is obliged to carry proof at all is not decided here. That is
# scripts/ci/rule-proof-check, which asks it of every rule in the set rather than of these
# four, and test/cases/125_rule_proof.sh mutation-tests it.
. "$ROOT/test/lib.sh"

RULES="$ROOT/.ai/repo/rules/project"
[ -d "$RULES" ] || { echo "    $RULES is missing"; exit 1; }

ids="project.execution-state-is-authoritative
project.every-wait-is-bounded
project.commands-run-non-interactively
project.recovery-is-idempotent"

# every id in the doctrine resolves to exactly one file that claims it
for id in $ids; do
  matches="$(grep -l "^id: $id\$" "$RULES"/*.md 2>/dev/null | wc -l | tr -d ' ')"
  [ "$matches" = 1 ] || { echo "    $id is claimed by $matches files, expected exactly 1"; exit 1; }
done

field() { sed -n "s/^$2: //p" "$1" | head -1; }

for id in $ids; do
  f="$(grep -l "^id: $id\$" "$RULES"/*.md)"

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
    advisory|blocking)
      # An enforcement block is valid in one of two modes, and the class does not decide
      # which: a rule that names a validator is dispatched and the function must be there,
      # and a rule that names tests instead is gated — the cases prove it and nothing calls
      # it. An advisory rule may carry either; before the second mode existed it could carry
      # neither, and asserting that here would now refuse a gated advisory rule for being
      # proven. What a class still decides is whether proof is required at all, and that is
      # scripts/ci/rule-proof-check, over every rule rather than these four.
      if grep -q '^x-majordomus:' "$f"; then
        validator="$(sed -n 's/^  validator: //p' "$f" | head -1)"
        if [ -n "$validator" ]; then
          grep -rq "mj_validate_$validator()" "$ROOT/lib" ||
            { echo "    $id: no mj_validate_$validator() in lib/"; exit 1; }
        elif ! grep -q '^  tests:' "$f"; then
          # the third mode (ADR 0048): the block declares that review decides, and says
          # why. A reason must be written; an empty one is no declaration.
          grep -qE '^  reviewed_because: *[^ ]' "$f" ||
            { echo "    $id: x-majordomus block names no validator, no test and no reviewed_because"; exit 1; }
        fi
      fi
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
  f="$(grep -l "^id: $id\$" "$RULES"/*.md)"
  grep -q 'project.execution-state-is-authoritative@1' "$f" ||
    { echo "    $id does not depend on project.execution-state-is-authoritative@1"; exit 1; }
done

# the split ADR 0039 drew, now that both scans exist: the mechanically decidable rules are
# decided by a scan, the other two by review, and each says which in its own block.
for id in project.commands-run-non-interactively project.every-wait-is-bounded; do
  f="$(grep -l "^id: $id\$" "$RULES"/*.md)"
  [ "$(field "$f" class)" = blocking ] ||
    { echo "    $id: its scan exists, so it is decided by one — class is '$(field "$f" class)', not blocking"; exit 1; }
  grep -q '^  tests:' "$f" ||
    { echo "    $id: blocking and names no test; the scan that decides it must be named"; exit 1; }
  for t in $(sed -n 's/^  tests: \[\(.*\)\]$/\1/p' "$f" | head -1 | tr ',' ' '); do
    t="$(echo "$t" | tr -d ' ')"
    [ -n "$t" ] || continue
    [ -f "$ROOT/$t" ] || { echo "    $id: names $t, which is not in the tree"; exit 1; }
  done
done

for id in project.execution-state-is-authoritative project.recovery-is-idempotent; do
  f="$(grep -l "^id: $id\$" "$RULES"/*.md)"
  [ "$(field "$f" class)" = advisory ] ||
    { echo "    $id: ADR 0039 reserved it for review, and nothing here decides it mechanically"; exit 1; }
  grep -qE '^  reviewed_because: *[^ ]' "$f" ||
    { echo "    $id: advisory by design and does not say why review decides it"; exit 1; }
done

echo "    4 liveness rules: identities unique, front matter complete, classes backed, dependencies resolve; 2 decided by their scan, 2 by review"
