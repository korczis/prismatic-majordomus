# majordomus-covers: rules
# majordomus-negative: rules
# The rule DAG. The effective rule set is the vendored baseline plus the project's own
# rules, resolved as a dependency graph. Every guarantee below is proved by a mutation:
# the set is green, one fact changes, the resolver goes red naming that fact, the change is
# undone, the set is green again. A mutation the resolver survives is a guarantee it does
# not give.
#
# The resolver is invoked as few times as the proof allows: one load reads every rule file,
# and a case that loads after every line would take minutes for no extra evidence.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
P=.ai/repo/rules/project
V=.ai/repo/rules/vendor/majordomus
[ -f "$V/manifest.yaml" ] || { echo "    init vendored no baseline package"; exit 1; }
[ -d "$P" ] || { echo "    init created no project rules directory"; exit 1; }

# prule FILE ID VERSION [DEP ...]  — a minimal valid project rule, enforced by nobody
prule() {
  local f="$1" id="$2" ver="$3"; shift 3
  { cat <<Y
---
id: $id
version: $ver
kind: rule
title: Rule $id
description: What $id requires, in one sentence.
statement: The normative sentence $id asks a worker to follow.
status: active
class: advisory
Y
    if [ $# -gt 0 ]; then printf 'depends_on: [%s]\n' "$(printf '%s\n' "$@" | paste -sd, -)"; else printf 'depends_on: []\n'; fi
    printf 'tags: [fixture]\n---\n\n# Rationale\n\nA fixture.\n'
  } > "$P/$f"
}
# mutate FILE SED-EXPR — rewrite one rule file in place through sed
mutate() { sed "$2" "$P/$1" > "$T/m.md" && mv "$T/m.md" "$P/$1"; }
# the resolver's view of the set: an exit code and one line per rule
green() { expect_exit 0 "$MJ" rules list || { echo "    the set is not green $1"; exit 1; }; }
# red PATTERN LABEL — the set does not resolve and the reason names the fact
red() {
  expect_exit 10 "$MJ" rules list || { echo "    the set resolved despite $2"; exit 1; }
  expect_grep 'rules do not resolve' || exit 1
  expect_grep "$1" || { echo "    the refusal for $2 does not name the fact"; exit 1; }
}

# ---------------------------------------------------------------- the control
# the vendored baseline alone resolves, and its enforcement status is shown, not hidden
green "after init"
expect_grep '^majordomus\.scope-integrity +v1 +blocking +vendor:majordomus +enforced by '
expect_grep '^majordomus\.sessions-are-workers +v1 +advisory +vendor:majordomus +not machine-enforced'
baseline="$LAST_OUT"
# a project rule joins the set; one without x-majordomus is normative and enforced by nobody,
# one with x-majordomus is enforced by the commands it names, and the order respects the edge
prule base.md project.base 1
cat > "$P/enforced.md" <<'Y'
---
id: project.enforced
version: 1
kind: rule
title: An enforced project rule
description: A project rule the tool enforces.
statement: The tool enforces this.
status: active
class: blocking
depends_on: [project.base@1]
tags: [fixture]
x-majordomus:
  validator: scope
  category: scope
  enforced_by: [check]
  exit_code: 10
  claims: []
  tests: [test/cases/67_rule_dag.sh]
---

# Rationale

A fixture.
Y
green "with two project rules"
expect_grep '^project\.base +v1 +advisory +project +not machine-enforced'
expect_grep '^project\.enforced +v1 +blocking +project +enforced by check$'
first="$LAST_OUT"
# show reads the file the resolver chose, and names it
expect_exit 0 "$MJ" rules show project.enforced
expect_grep "^# $P/enforced.md"
expect_grep '^  validator: scope$'
expect_exit 12 "$MJ" rules show project.nobody
expect_grep "no rule 'project.nobody'"

# ---------------------------------------------------------------- determinism
# a second run agrees with the first, on both surfaces, and every dependency is listed
# before the rule that depends on it
expect_exit 0 "$MJ" rules list
[ "$first" = "$LAST_OUT" ] || { echo "    two runs of rules list disagree"; exit 1; }
order="$(printf '%s\n' "$LAST_OUT" | awk '{print $1"@"substr($2,2)}')"
A="$("$MJ" --json rules list)"; B="$("$MJ" --json rules list)"
[ "$A" = "$B" ] || { echo "    two runs of --json rules list disagree"; exit 1; }
if command -v jq >/dev/null 2>&1; then
  printf '%s\n' "$A" | jq -r '.rules[] | .id + "@" + (.version|tostring) + " " + (.depends_on|join(" "))' \
  | while read -r self deps; do
      for d in $deps; do
        s="$(printf '%s\n' "$order" | grep -nx -- "$self" | cut -d: -f1)"
        p="$(printf '%s\n' "$order" | grep -nx -- "$d" | cut -d: -f1)"
        [ -n "$p" ] && [ "$p" -lt "$s" ] || { echo "    $self is listed before its dependency $d"; exit 1; }
      done
    done || exit 1
fi
b="$(printf '%s\n' "$order" | grep -nx 'project.base@1' | cut -d: -f1)"
e="$(printf '%s\n' "$order" | grep -nx 'project.enforced@1' | cut -d: -f1)"
[ "$b" -lt "$e" ] || { echo "    project.enforced is listed before project.base, which it depends on"; exit 1; }

# ---------------------------------------------------------------- mutation 1: a missing dependency
# and the proof that an unresolved set is applied by nothing: not the JSON surface, not show
prule missing.md project.missing 1 project.nowhere@1
red 'project\.missing@1 depends on project\.nowhere@1, which no rule provides' "a missing dependency"
expect_exit 10 "$MJ" --json rules list || { echo "    the JSON surface resolved a set with a missing dependency"; exit 1; }
expect_exit 10 "$MJ" rules show majordomus.scope-integrity || { echo "    rules show applied a set that does not resolve"; exit 1; }
rm -f "$P/missing.md"

# ---------------------------------------------------------------- mutation 2: a cycle
prule c1.md project.c1 1 project.c2@1
prule c2.md project.c2 1 project.c1@1
red 'dependency cycle among.*project\.c1@1.*project\.c2@1|dependency cycle among.*project\.c2@1.*project\.c1@1' "a dependency cycle"
rm -f "$P/c1.md" "$P/c2.md"
# a rule that depends on itself is the smallest cycle
prule self.md project.self 1 project.self@1
red 'dependency cycle among.*project\.self@1' "a self-dependency"
rm -f "$P/self.md"

# ---------------------------------------------------------------- mutation 3: one identity claimed twice
# identity is id@version, never the file name: two files, one identity
prule twin-a.md project.twin 1
prule twin-b.md project.twin 1
red 'project\.twin@1 is claimed twice' "a duplicate id@version"
# two versions of one id are two identities, and both resolve
prule twin-b.md project.twin 2
green "with two versions of one id"
expect_grep '^project\.twin +v1 '
expect_grep '^project\.twin +v2 '
rm -f "$P/twin-a.md" "$P/twin-b.md"

# ---------------------------------------------------------------- mutation 4: a project rule in the vendor namespace
# there is no override: a project rule may not claim a vendored identity, nor the namespace
prule override.md majordomus.scope-integrity 1
red 'is claimed twice|may not claim the majordomus namespace' "a project rule reusing a vendored identity"
prule override.md majordomus.something-new 1
red 'may not claim the majordomus namespace' "a project rule in the vendor namespace"
rm -f "$P/override.md"

# ---------------------------------------------------------------- mutation 5: malformed front matter
printf 'no front matter at all\n' > "$P/bad.md"
red 'bad\.md: no front matter' "a rule with no front matter"
printf -- '---\nid project.bad\nversion: 1\n---\nbody\n' > "$P/bad.md"
red 'bad\.md: front matter does not parse' "front matter that does not parse"
# an opening fence with no closing fence is not front matter, it is a file that starts with ---
prule bad.md project.bad 1
awk '/^---$/ { n++; if (n == 2) next } { print }' "$P/bad.md" > "$T/m.md" && mv "$T/m.md" "$P/bad.md"
grep -c '^---$' "$P/bad.md" | grep -qx 1 || { echo "    the fixture did not remove the closing fence"; exit 1; }
red 'bad\.md: .*(front matter|fence)' "front matter whose fence never closes"
# a required field is absent
prule bad.md project.bad 1
mutate bad.md '/^statement:/d'
red 'bad\.md: front matter lacks statement' "a rule without a statement"
# depends_on absent is not the same as depends_on empty
prule bad.md project.bad 1
mutate bad.md '/^depends_on:/d'
red 'bad\.md: front matter lacks depends_on' "a rule without depends_on"
# a dependency reference that is not exact
prule bad.md project.bad 1 project.base
red "depends_on 'project\.base' is not an exact id@version reference" "an unversioned dependency"
# the wrong kind, an unknown status, an unknown class, a non-integer version
prule bad.md project.bad 1; mutate bad.md 's/^kind: rule$/kind: policy/'
red 'kind is policy, not rule' "kind: policy"
prule bad.md project.bad 1; mutate bad.md 's/^status: active$/status: draft/'
red "status 'draft' is neither active nor deprecated" "status: draft"
prule bad.md project.bad 1; mutate bad.md 's/^class: advisory$/class: fatal/'
red "class 'fatal' is neither blocking nor advisory" "class: fatal"
prule bad.md project.bad 1; mutate bad.md 's/^version: 1$/version: 1.2/'
red 'version 1\.2 is not an integer' "version: 1.2"
rm -f "$P/bad.md"

# ---------------------------------------------------------------- mutation 6: an unknown front-matter key
# the allowlist is share/allow/rule.txt; a key it does not name is an error, not a silent extra
prule extra.md project.extra 1
mutate extra.md 's/^tags: \[fixture\]$/tags: [fixture]\
severity: high/'
red 'extra\.md: unknown front-matter key\(s\): severity' "an unknown front-matter key"
# an unknown key inside x-majordomus is an error too
prule extra.md project.extra 1
mutate extra.md 's/^tags: \[fixture\]$/tags: [fixture]\
x-majordomus:\
  validator: scope\
  category: scope\
  enforced_by: [check]\
  exit_code: 10\
  tests: [test\/cases\/67_rule_dag.sh]\
  severity: high/'
red 'extra\.md: unknown front-matter key\(s\): x-majordomus\.severity' "an unknown x-majordomus key"
# and the allowlist is read, not copied: a distribution whose allowlist names the key accepts it
DIST="$(mktemp -d "${TMPDIR:-/tmp}/mj-dist.XXXXXX")"
cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$DIST/"
printf '^x-majordomus\\.severity$\n' >> "$DIST/share/allow/rule.txt"
expect_exit 0 "$DIST/bin/majordomus" rules list || { echo "    the allowlist is not read from the distribution"; exit 1; }
rm -rf "$DIST"
rm -f "$P/extra.md"

# ---------------------------------------------------------------- mutation 7: x-majordomus without its binding
prule half.md project.half 1
mutate half.md 's/^tags: \[fixture\]$/tags: [fixture]\
x-majordomus:\
  category: scope\
  enforced_by: [check]\
  exit_code: 10\
  tests: [test\/cases\/67_rule_dag.sh]/'
red 'half\.md: x-majordomus lacks validator' "an x-majordomus block without a validator"
prule half.md project.half 1
mutate half.md 's/^tags: \[fixture\]$/tags: [fixture]\
x-majordomus:\
  validator: scope\
  category: scope\
  exit_code: 10\
  tests: [test\/cases\/67_rule_dag.sh]/'
red 'half\.md: x-majordomus names no enforcing command' "an x-majordomus block without enforced_by"
prule half.md project.half 1
mutate half.md 's/^tags: \[fixture\]$/tags: [fixture]\
x-majordomus:\
  validator: scope\
  category: scope\
  enforced_by: [check]\
  exit_code: 10/'
red 'half\.md: x-majordomus names no test' "an x-majordomus block without tests"
rm -f "$P/half.md"

# ---------------------------------------------------------------- mutation 8: a deprecated dependency
# a deprecated rule is outside the effective set, and nothing active may depend on it
prule old.md project.old 1
mutate old.md 's/^status: active$/status: deprecated/'
prule needs-old.md project.needs-old 1 project.old@1
red 'project\.needs-old@1 depends on project\.old@1, which is deprecated' "an active rule depending on a deprecated one"
rm -f "$P/needs-old.md"
green "with a deprecated rule and nothing depending on it"
expect_no_grep '^project\.old ' || { echo "    a deprecated rule is listed in the effective set"; exit 1; }
rm -f "$P/old.md"

# ---------------------------------------------------------------- mutation 9: a hand edit under vendor/
# the manifest is the evidence: one edited byte in a vendored rule is detected, reported by
# status, and refused by update until --force
expect_exit 0 "$MJ" rules vendor status
expect_grep '^state: +current$'
edited="$V/rules/scope-integrity.v1.md"
printf '\nA sentence added by hand.\n' >> "$edited"
expect_exit 10 "$MJ" rules vendor status
expect_grep 'integrity: +rules/scope-integrity\.v1\.md differs from its manifest hash'
expect_exit 15 "$MJ" rules vendor update
expect_grep 'hand-edited'
expect_grep 'rules/scope-integrity\.v1\.md differs from its manifest hash'
grep -q 'A sentence added by hand' "$edited" || { echo "    a refused update changed the vendor directory"; exit 1; }
# a rule removed from under the manifest, and a stray file beside it, are detected the same way
mv "$edited" "$V/rules/stray.v1.md"
expect_exit 10 "$MJ" rules vendor status
expect_grep 'rules/scope-integrity\.v1\.md is listed but absent'
expect_exit 15 "$MJ" rules vendor update
# --force restores the baseline from the distribution, and the record says so
expect_exit 0 "$MJ" rules vendor update --force
expect_grep '^vendored 1 \(.*\) into \.ai/repo/rules/vendor/majordomus$'
expect_exit 0 "$MJ" rules vendor status
expect_grep '^state: +current$'
[ ! -e "$V/rules/stray.v1.md" ] || { echo "    --force left a stray file in the vendor directory"; exit 1; }
grep -q 'A sentence added by hand' "$edited" && { echo "    --force did not restore the edited rule"; exit 1; }
grep -q '"event":"rules.vendored"' .ai/local/state/ledger.jsonl || { echo "    vendor update wrote no ledger event"; exit 1; }
diff -rq "$V" "$ROOT/share/standard/majordomus" >/dev/null || { echo "    the restored vendor directory differs from the distribution"; exit 1; }

# ---------------------------------------------------------------- mutation 10: a newer distribution
# a newer executable reports a newer baseline and never applies it; the repository's copy is
# authoritative until update is asked for, and update never touches rules/project/
prule keep.md project.keep 1
# a second distribution whose standard package differs in one rule and in its revision
NEWER="$(mktemp -d "${TMPDIR:-/tmp}/mj-newer.XXXXXX")"
cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$NEWER/"
pkg="$NEWER/share/standard/majordomus"
sed 's/^description: .*$/description: The next revision says this differently./' "$pkg/rules/scope-integrity.v1.md" > "$T/r.md" && mv "$T/r.md" "$pkg/rules/scope-integrity.v1.md"
newhash="$(shasum -a 256 "$pkg/rules/scope-integrity.v1.md" | cut -d' ' -f1)"
awk -v h="$newhash" '
  /^    file: rules\/scope-integrity\.v1\.md$/ { print; hit=1; next }
  hit && /^    sha256: / { print "    sha256: " h; hit=0; next }
  /^version: 1$/ { print "version: 2"; next }
  /^source_revision: / { print "source_revision: next"; next }
  { print }' "$pkg/manifest.yaml" > "$T/m.yaml" && mv "$T/m.yaml" "$pkg/manifest.yaml"
before_vendor="$(cd "$V" && find . -type f | LC_ALL=C sort | xargs shasum -a 256)"
before_project="$(cd "$P" && find . -type f | LC_ALL=C sort | xargs shasum -a 256)"
# the newer executable reports the difference
expect_exit 11 "$NEWER/bin/majordomus" rules vendor status
expect_grep '^vendored: +1 \(0\.1\.0\)$'
expect_grep '^distribution: +2 \(next\)$'
expect_grep 'ships a different package; review with: majordomus rules vendor diff'
# diff is the reviewable difference; status carries the exit code, diff prints and exits 0
expect_exit 0 "$NEWER/bin/majordomus" rules vendor diff
expect_grep 'The next revision says this differently'
expect_grep '^-source_revision: 0\.1\.0|^\+source_revision: next'
# and applied nothing: the vendor copy is byte for byte what it was, and the set the newer
# executable reads is the repository's, not its own
[ "$before_vendor" = "$(cd "$V" && find . -type f | LC_ALL=C sort | xargs shasum -a 256)" ] || { echo "    a read-only vendor command changed the vendor directory"; exit 1; }
expect_exit 0 "$NEWER/bin/majordomus" rules show majordomus.scope-integrity
expect_no_grep 'The next revision says this differently' || { echo "    rules show read the distribution, not the repository"; exit 1; }
# every other command of the newer executable also leaves the baseline alone
expect_exit 0 "$NEWER/bin/majordomus" update
"$NEWER/bin/majordomus" doctor >/dev/null 2>&1 || true
[ "$before_vendor" = "$(cd "$V" && find . -type f | LC_ALL=C sort | xargs shasum -a 256)" ] || { echo "    update or doctor changed the vendored baseline"; exit 1; }
# the explicit update applies it, atomically, and only under vendor/
expect_exit 0 "$NEWER/bin/majordomus" rules vendor update
expect_grep '^vendored 2 \(next\) into \.ai/repo/rules/vendor/majordomus$'
expect_exit 0 "$NEWER/bin/majordomus" rules vendor status
expect_grep '^vendored: +2 \(next\)$'
expect_grep '^state: +current$'
diff -rq "$V" "$pkg" >/dev/null || { echo "    the vendored package is not the distribution's package after update"; exit 1; }
[ "$before_project" = "$(cd "$P" && find . -type f | LC_ALL=C sort | xargs shasum -a 256)" ] || { echo "    vendor update touched rules/project/"; exit 1; }
for s in "$V"/../.vendor.*; do [ -e "$s" ] && { echo "    vendor update left its staging directory behind"; exit 1; }; done
expect_exit 0 "$NEWER/bin/majordomus" rules show majordomus.scope-integrity
expect_grep 'The next revision says this differently' || { echo "    the updated rule is not what show reads"; exit 1; }
# the older executable now reports the repository ahead of it, and still applies nothing
expect_exit 11 "$MJ" rules vendor status
expect_grep '^vendored: +2 \(next\)$'
expect_grep '^distribution: +1 \(0\.1\.0\)$'
rm -rf "$NEWER"

# ---------------------------------------------------------------- nothing vendored at all
# status says so, names the next command, and exits 12
rm -rf "$V"
expect_exit 12 "$MJ" rules vendor status
expect_grep '^vendored: none$'
expect_grep '^next: majordomus rules vendor update$'
expect_exit 0 "$MJ" rules vendor update
expect_exit 0 "$MJ" rules vendor status
expect_grep '^state: +current$'
[ -f "$P/keep.md" ] || { echo "    vendor update removed a project rule"; exit 1; }
# after every mutation was undone the set is what init produced
rm -f "$P/keep.md" "$P/base.md" "$P/enforced.md"
green "back at the baseline"
[ "$baseline" = "$LAST_OUT" ] || { echo "    the baseline after every mutation differs from the baseline after init"; exit 1; }
