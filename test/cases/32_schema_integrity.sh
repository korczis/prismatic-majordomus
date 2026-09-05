# majordomus-covers: none
# majordomus-negative: doctor
# Schema integrity: the rules resolve in one stated order, every kind that carries metadata
# declares a schema, and every schema is applied by something.
#
# The point of this case is the pair. Checking only that kinds have schemas leaves a schema
# file nothing applies — it describes nothing, is enforced nowhere, and drifts away from the
# shape it was written for with nothing to notice. Checking only that schemas are named
# leaves a kind carrying its front matter through unvalidated, which looks exactly like a
# kind that validates. Both directions are provoked below, one at a time, because a check
# that passes because the other one failed proves nothing.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base

# The repository under test uses the distribution beside the tool, so the fixtures below are
# a copy of it: a case may not edit the tree it is run from.
share="$T/share"
cp -R "$ROOT/share" "$share"
export MAJORDOMUS_SHARE="$share"

# A fresh repository is not green — it has no use cases, no project model and no skills, and
# says so. So every restore below is checked by the finding this rule owns, not by doctor's
# exit code, which would be asserting other rules' business.
schema_ok() {
  "$MJ" doctor > "$T/ok" 2>&1 || true
  grep -qE 'OK +schema +share/kinds.yaml' "$T/ok" && grep -qE 'OK +schema +share/schemas' "$T/ok" && return 0
  echo "    the schema findings are not clean after restoring the tree"
  grep -iE ' schema ' "$T/ok" | sed 's/^/    | /'
  exit 1
}

# ---------------------------------------------------------------- the healthy tree
"$MJ" doctor > "$T/out" 2>&1 || true
grep -qF 'resolve in one order' "$T/out" \
  || { echo "    doctor does not report the order the rules resolve in"; grep -i schema "$T/out" | sed 's/^/    | /'; exit 1; }
grep -qE 'OK +schema +share/kinds.yaml' "$T/out" \
  || { echo "    a tree whose kinds all declare a schema is not reported as one"; grep -i schema "$T/out" | sed 's/^/    | /'; exit 1; }
grep -qE 'OK +schema +share/schemas' "$T/out" \
  || { echo "    a tree whose schemas are all applied is not reported as one"; grep -i schema "$T/out" | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- a kind with no schema
# The front matter of a kind without one is carried through unvalidated, silently. That is
# the failure this half exists for, so it must be loud.
cp "$share/kinds.yaml" "$T/kinds.keep"
awk '/^  prompt:/ { p = 1 } p && /^    schema: prompt$/ { next } { print }' "$T/kinds.keep" > "$share/kinds.yaml"
expect_exit 10 "$MJ" doctor
expect_grep 'declare no schema'
expect_grep 'prompt'
cp "$T/kinds.keep" "$share/kinds.yaml"
schema_ok

# ---------------------------------------------------------------- a schema nothing applies
# The other direction: a file that describes something no kind reads and no allow-list is
# read for. It is not an error in any single object, which is exactly why nothing else
# catches it.
printf '{"$schema":"https://json-schema.org/draft/2020-12/schema","title":"Nothing","type":"object"}\n' \
  > "$share/schemas/nothing.schema.json"
expect_exit 10 "$MJ" doctor
expect_grep 'named by no kind'
expect_grep 'nothing'
rm -f "$share/schemas/nothing.schema.json"
schema_ok

# ...and a schema applied through its allow-list rather than by a kind still counts, which is
# how the local half of the layer — not indexed, so no kind can name it — is covered at all.
schema_ok   # an allow-list-applied schema is not an orphan
for n in current session; do
  [ -f "$share/allow/$n.txt" ] || { echo "    share/allow/$n.txt is absent; the schema reaches nothing"; exit 1; }
  grep -rqE "(ALLOW_DIR|share/allow)/$n\.txt" "$ROOT/lib" \
    || { echo "    nothing reads share/allow/$n.txt, so $n.schema.json applies to nothing"; exit 1; }
done

# ---------------------------------------------------------------- a schema that is not JSON
printf 'not json at all\n' > "$share/schemas/broken.schema.json"
expect_exit 10 "$MJ" doctor
expect_grep 'do not parse as JSON'
rm -f "$share/schemas/broken.schema.json"
schema_ok

# ---------------------------------------------------------------- the order is total
# A set that does not resolve has no order, so nothing can be dispatched over it. The refusal
# comes from the doctrine loader before any validator runs, and it names the dependency that
# is missing rather than reporting a count over a set it could not build. Asserted here
# because it is what makes the order a contract and not a best effort.
mkdir -p .ai/repo/rules/project
cat > .ai/repo/rules/project/dangling.v1.md <<'RULE'
---
id: project.dangling
version: 1
kind: rule
title: A rule that depends on one nobody wrote
description: Exists only to prove that an unresolvable set is reported as one.
statement: This rule cannot resolve, because the rule it depends on does not exist.
status: active
class: advisory
depends_on: [majordomus.no-such-rule@1]
---
# Rationale
Provoked by the schema-integrity case.
RULE
"$MJ" doctor > "$T/unresolved" 2>&1 || true
grep -qF 'the rules do not resolve, so nothing can be enforced' "$T/unresolved" \
  || { echo "    an unresolvable rule set was not refused"; tail -n 3 "$T/unresolved" | sed 's/^/    | /'; exit 1; }
grep -qF 'majordomus.no-such-rule@1' "$T/unresolved" \
  || { echo "    the refusal does not name the dependency that is missing"; tail -n 3 "$T/unresolved" | sed 's/^/    | /'; exit 1; }
expect_no_grep 'OK +schema' "$T/unresolved"   # nothing was dispatched, so nothing reported
rm -f .ai/repo/rules/project/dangling.v1.md
schema_ok
