# The four liveness rules, checked as documents against the checkout rather than a fixture.
#
# ADR 0039 landed them as four separate rule objects, all advisory and all documented-only,
# because no scan existed yet. Three of them are no longer documented-only:
# `scripts/liveness-check` ships in the `structure` job and reports one shape for each, and
# this case now holds the converse of that relation. The gate named the three rules it
# enforces and none of the three named the gate or the cases back, which is the one-way
# reference this repository keeps rediscovering — a scan can be renamed, narrowed or deleted
# and the rules would go on reading as though nothing had changed.
#
# What is proved here: all four documents exist with the identities ADR 0039 fixed; each
# carries the front-matter fields the rules README requires; each declares an enforcement
# block the repository can back, in whichever of the three modes it claims — a named
# validator must exist as a function in lib/, and a block without one must name the tests
# that prove it or give the reason nothing executable can; every depends_on reference
# resolves to a rule that really exists at the version named, so the set loads instead of
# erroring as a missing dependency; and the three rules `scripts/liveness-check` says it
# enforces name this case and `test/cases/122_liveness_gate.sh` back.
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
    advisory|blocking)
      # An enforcement block is valid in one of three modes, and the class does not decide
      # which: a rule that names a validator is dispatched and the function must be there;
      # a rule that names tests instead is gated — the cases prove it and nothing calls it;
      # a rule that names neither may give the reason nothing executable can express it,
      # and the reason is the whole declaration. An advisory rule may carry any of them;
      # before the second mode existed it could carry none, and asserting that here would
      # now refuse a gated advisory rule for being proven. What a class no longer decides is
      # whether proof is required at all — since `advisory` stopped being an exemption it is
      # required of every rule — and scripts/ci/rule-proof-check is what asks it, over the
      # whole set rather than these four.
      if grep -q '^x-majordomus:' "$f"; then
        validator="$(sed -n 's/^  validator: //p' "$f" | head -1)"
        if [ -n "$validator" ]; then
          grep -rq "mj_validate_$validator()" "$ROOT/lib" ||
            { echo "    $id: no mj_validate_$validator() in lib/"; exit 1; }
        else
          grep -qE '^  (tests:|reviewed_because: .)' "$f" ||
            { echo "    $id: x-majordomus block names neither a validator, nor a test, nor a reason"; exit 1; }
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
  f="$(grep -l "^id: $id\$" "$RULES"/*.v1.md)"
  grep -q 'project.execution-state-is-authoritative@1' "$f" ||
    { echo "    $id does not depend on project.execution-state-is-authoritative@1"; exit 1; }
done

# The relation between the gate and the rules, read from both ends. scripts/liveness-check
# names the three rules it serves in its own header; each of those three must name the two
# cases that prove the gate. Read from one end only, this is exactly the reference that
# rots in silence: the gate claimed three rules and not one of them claimed it back, so the
# rules went on saying "the mechanical half does not exist yet" while it ran on every push.
GATE="$ROOT/scripts/liveness-check"
[ -f "$GATE" ] || { echo "    scripts/liveness-check is missing; the assertion below would pass vacuously"; exit 1; }
served="project.every-wait-is-bounded
project.commands-run-non-interactively
project.execution-state-is-authoritative"
for id in $served; do
  grep -q "$id" "$GATE" ||
    { echo "    scripts/liveness-check no longer names $id; this case and that header disagree"; exit 1; }
  f="$(grep -l "^id: $id\$" "$RULES"/*.v1.md)"
  for c in test/cases/121_liveness_doctrine.sh test/cases/122_liveness_gate.sh; do
    grep -q "$c" "$f" ||
      { echo "    $id does not name $c, so the gate that enforces it is claimed from one side only"; exit 1; }
  done
  grep -q 'scripts/liveness-check' "$f" ||
    { echo "    $id does not name scripts/liveness-check, the gate that decides it"; exit 1; }
done

echo "    4 liveness rules: identities unique, front matter complete, classes backed, dependencies resolve, 3 name the gate back"
