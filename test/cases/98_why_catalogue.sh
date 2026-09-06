# The invariant the Why catalogue exists for: one valid file added under the layer's why
# section is answered by every projection, and removing it removes it from all of them.
#
# The point is not that the catalogue works. It is that nothing between the file and the
# answers has a list in it. So this case adds ONE file, changes nothing else — no registry,
# no navigation, no template, no Rust, no schema — and then asks the domain index, the
# command line, the HTTP API, MCP and the site projection whether they know about it. Then
# it deletes the file and asks all of them again.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj98.XXXXXX")"; trap 'rm -rf "$S"' EXIT

# --- a repository with the layer. `init` seeds the why section and its three source
# classes, so nothing here has to declare them: that is the point.
"$MJ" init >/dev/null
grep -q "kind: moment" .ai/repo/knowledge/sources.yaml \
  || { echo "    init did not seed the moment source class"; exit 1; }
[ -f .ai/repo/why/README.md ] || { echo "    init did not seed the why section's contract"; exit 1; }
mkdir -p .ai/repo/why/moments .ai/repo/why/audiences .ai/repo/why/areas
cat > .ai/repo/why/audiences/probe-team.md <<'MD'
---
schema: audience/v1
id: probe-team
kind: audience
title: The probe's team
summary: 'A team that exists so this case has somebody to belong to.'
status: stable
weight: 10
---

# The probe's team

Because the case says so.
MD
cat > .ai/repo/why/areas/probe-area.md <<'MD'
---
schema: area/v1
id: probe-area
kind: area
title: The probe's area
summary: 'An area that exists so this case has somewhere to fall.'
status: stable
weight: 10
---

# The probe's area

Because the case says so.
MD
git add -A >/dev/null && git commit -qm install

# an empty catalogue is a valid catalogue, and it is empty
expect_exit 0 "$RB" why validate
expect_grep '^0 moment'
expect_exit 0 "$RB" why list
expect_grep '0 of 0 moment'

# ---------------------------------------------------------------- add exactly one file
before="$(git ls-files | sort | shasum -a 256)"
cat > .ai/repo/why/moments/a-probe-moment.md <<'MD'
---
schema: moment/v1
id: a-probe-moment
kind: moment
title: 'The moment this case adds'
hook: 'added one file and changed nothing else'
summary: 'A moment that exists to prove that adding one file is the whole act.'
status: stable
severity: high
frequency: rare
weight: 10
featured: true
audiences: [probe-team]
areas: [probe-area]
tags: [probe]
signals:
  - id: probe-signal
    text: 'This case added one file.'
examples:
  - id: one
    audience: probe-team
    title: 'The first situation'
    before: 'Nothing knew about it.'
    after: 'Everything did.'
  - id: two
    audience: probe-team
    title: 'The second situation'
    before: 'Nothing knew about it.'
    after: 'Everything did.'
  - id: three
    audience: probe-team
    title: 'The third situation'
    before: 'Nothing knew about it.'
    after: 'Everything did.'
---

## The moment

Because the case says so.

## Why it happens

Because the case says so.

## What it does not do

Nothing the case does not say.
MD
git add .ai/repo/why/moments/a-probe-moment.md >/dev/null
# nothing but that one file was added: no registry, no navigation, no template, no code
after="$(git ls-files | sort | shasum -a 256)"
[ "$before" != "$after" ] || { echo "    the probe file was not added"; exit 1; }
added="$(git diff --cached --name-only)"
[ "$added" = ".ai/repo/why/moments/a-probe-moment.md" ] \
  || { echo "    more than the one file changed: $added"; exit 1; }
git commit -qm "one file"

# --- the domain index found it, and it is valid
expect_exit 0 "$RB" why validate
expect_grep '^1 moment'
expect_grep '^valid:'

# --- the command line answers it, and every facet it declares became a filter
expect_exit 0 "$RB" why list
expect_grep '^a-probe-moment'
expect_grep '1 of 1 moment'
expect_exit 0 "$RB" why list --audience probe-team
expect_grep '^a-probe-moment'
expect_exit 0 "$RB" why list --area probe-area
expect_grep '^a-probe-moment'
expect_exit 0 "$RB" why list --severity high
expect_grep '^a-probe-moment'
expect_exit 0 "$RB" why list -q 'changed nothing else'
expect_grep '^a-probe-moment'
expect_exit 0 "$RB" why show a-probe-moment
expect_grep 'The moment this case adds'
expect_grep 'The first situation'

# --- the taxonomies gained it without either file being touched
expect_exit 0 "$RB" why audiences
expect_grep '^probe-team *1'
expect_exit 0 "$RB" why areas
expect_grep '^probe-area *1'

# --- the diagnosis knows its signal, and says which moment produced each recommendation
expect_exit 0 "$RB" why diagnose --signal probe-signal
expect_grep 'matched: a-probe-moment'
expect_grep 'probe-area'
expect_exit 0 "$RB" why diagnose
expect_grep 'This case added one file.'

# --- the HTTP API and the OpenAPI document answer it, from the same value
expect_exit 0 "$RB" why list --format json
"$RB" why list --format json > "$S/list.json" 2>/dev/null
grep -q '"id": "a-probe-moment"' "$S/list.json" || { echo "    the JSON answer does not carry the moment"; exit 1; }
grep -q '"probe-team"' "$S/list.json" || { echo "    the JSON answer does not carry the derived facet"; exit 1; }

# --- MCP serves it as a resource, with no tool registered for it
expect_exit 0 "$RB" mcp --inspect
expect_grep '^resource    majordomus://moment/a-probe-moment$'
expect_grep '^resource    majordomus://audience/probe-team$'
expect_grep 'tool        majordomus_why$'

# --- the site projection carries it, written by `generate`
expect_exit 0 "$RB" generate site --out "$S/gen"
[ -f "$S/gen/site/data/registry/why.json" ] || { echo "    generate site wrote no why.json"; exit 1; }
grep -q '"a-probe-moment"' "$S/gen/site/data/registry/why.json" \
  || { echo "    the site dataset does not carry the moment"; exit 1; }
grep -q '"/why/a-probe-moment/"' "$S/gen/site/data/registry/why.json" \
  || { echo "    the site dataset carries no route for the moment"; exit 1; }
grep -q 'moment:a-probe-moment' "$S/gen/site/data/registry/why-graph.json" \
  || { echo "    the derived graph does not carry the moment"; exit 1; }

# ---------------------------------------------------------------- remove that one file
# a hidden registry anywhere would keep answering after the source is gone, which is the
# failure this half of the case exists to catch
git rm -q .ai/repo/why/moments/a-probe-moment.md
git commit -qm "one file removed"
expect_exit 0 "$RB" why list
expect_no_grep 'a-probe-moment'
expect_grep '0 of 0 moment'
expect_exit 12 "$RB" why show a-probe-moment
expect_grep "no moment 'a-probe-moment'"
expect_exit 0 "$RB" why audiences
expect_grep '^probe-team *0'
expect_exit 0 "$RB" why diagnose --signal probe-signal
expect_grep 'unresolved: probe-signal'
expect_exit 0 "$RB" mcp --inspect
expect_no_grep 'majordomus://moment/a-probe-moment'
expect_exit 0 "$RB" generate site --out "$S/gone"
grep -q 'a-probe-moment' "$S/gone/site/data/registry/why.json" \
  && { echo "    the site dataset still carries the removed moment"; exit 1; }

# ---------------------------------------------------------------- the contract is enforced
# an unresolved reference is an error with the nearest candidate, not a link to nothing
mkdir -p .ai/repo/why/moments
cat > .ai/repo/why/moments/a-typo-moment.md <<'MD'
---
schema: moment/v1
id: a-typo-moment
kind: moment
title: 'A moment naming an audience that does not exist'
hook: 'named an audience that does not exist'
summary: 'A moment whose audience is a typo.'
status: draft
severity: low
frequency: rare
audiences: [probe-tea]
areas: [probe-area]
signals:
  - id: typo-signal
    text: 'A reference was a typo.'
examples:
  - id: one
    audience: probe-team
    title: 'One'
    before: 'x'
    after: 'y'
---

## The moment

Because the case says so.
MD
git add -A >/dev/null && git commit -qm typo
expect_exit 10 "$RB" why validate
expect_grep 'unknown audience reference: "probe-tea"'
expect_grep 'did you mean: probe-team'
git rm -q .ai/repo/why/moments/a-typo-moment.md && git commit -qm "typo removed"

# two files claiming one identity: the index refuses both, and the catalogue says so rather
# than reporting itself valid over the objects that were excluded from under it
cp .ai/repo/why/audiences/probe-team.md .ai/repo/why/audiences/a-copy.md
git add -A >/dev/null && git commit -qm duplicate
expect_exit 10 "$RB" why validate
expect_grep 'is claimed by'
git rm -q .ai/repo/why/audiences/a-copy.md && git commit -qm "duplicate removed"

# the file name is the id: a record whose name disagrees is refused before anything renders
git mv .ai/repo/why/audiences/probe-team.md .ai/repo/why/audiences/renamed.md
git commit -qm renamed
expect_exit 10 "$RB" why validate
expect_grep "the file is named 'renamed.md' and the record's id is 'probe-team'"
expect_grep 'did you mean: probe-team.md'
git mv .ai/repo/why/audiences/renamed.md .ai/repo/why/audiences/probe-team.md
git commit -qm "rename undone"

# a derived value may not be authored: the schema has no `route` key
sed 's|^status: stable$|status: stable\nroute: /why/probe-team/|' .ai/repo/why/audiences/probe-team.md > "$S/bad.md"
cp "$S/bad.md" .ai/repo/why/audiences/probe-team.md
git add -A >/dev/null && git commit -qm "derived key"
expect_exit 10 "$RB" why validate
expect_grep "not in schema 'audience': route"
expect_exit 10 "$RB" mcp --inspect --strict
