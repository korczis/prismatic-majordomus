# The invariant the product model exists for: one valid feature file added under the layer
# is answered by every projection, and removing it removes it from all of them.
#
# The point is not that `majordomus product` works. It is that nothing between the file and
# the answers keeps a list. So this case adds ONE file, changes nothing else — no registry,
# no navigation, no template, no Rust, no schema — and then asks the index, the command
# line, the JSON answer, MCP and the site projection whether they know about it. Then it
# deletes the file and asks all of them again.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
#
# Proves claim product-features-discovered (docs/CLAIMS.yaml), whose `test:` names this case.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj97.XXXXXX")"; trap 'rm -rf "$S"' EXIT

# --- a repository with the layer. `init` seeds the features section, its contract and its
# source class, so nothing here has to declare them: that is the point.
"$MJ" init >/dev/null
grep -q "kind: feature" .ai/repo/knowledge/sources.yaml \
  || { echo "    init did not seed the feature source class"; exit 1; }
[ -f .ai/repo/features/README.md ] || { echo "    init did not seed the features section's contract"; exit 1; }
git add -A >/dev/null && git commit -qm install

# an empty product is a valid product, and it is empty
expect_exit 0 "$RB" product validate
expect_grep '^valid:'
expect_exit 0 "$RB" product list
expect_grep '0 feature'

# ---------------------------------------------------------------- add exactly one file
# The references name only things a freshly initialised repository has: a capability module
# of the executable, a kind of the layer and a workflow document `init` seeded. Nothing about
# the feature's surfaces, counts or route is written; every one of those is derived below.
before="$(git ls-files | LC_ALL=C sort | shasum -a 256)"
mkdir -p .ai/repo/features
cat > .ai/repo/features/a-probe-feature.md <<'MD'
---
schema: feature/v1
id: a-probe-feature
kind: feature
title: 'The feature this case adds'
short_title: 'The probe'
headline: 'One file was added and nothing else was, and every interface answered it.'
summary: 'A feature that exists to prove that adding one file is the whole act.'
status: stable
weight: 10
featured: true
modules: [repository]
kinds: [rule]
docs: [.ai/repo/workflows/task-lifecycle.md]
tags: [probe]
---

## What it does

Because the case says so.

## What it does not do

Nothing the case does not say.
MD
git add .ai/repo/features/a-probe-feature.md >/dev/null
# nothing but that one file was added: no registry, no navigation, no template, no code
after="$(git ls-files | LC_ALL=C sort | shasum -a 256)"
[ "$before" != "$after" ] || { echo "    the probe file was not added"; exit 1; }
added="$(git diff --cached --name-only)"
[ "$added" = ".ai/repo/features/a-probe-feature.md" ] \
  || { echo "    more than the one file changed: $added"; exit 1; }
git commit -qm "one file"

# --- the index found it, and it is valid
expect_exit 0 "$RB" product validate
expect_grep '^valid:'

# --- the command line answers it, and the filters it declares work
expect_exit 0 "$RB" product list
expect_grep '^a-probe-feature'
expect_exit 0 "$RB" product list --featured
expect_grep '^a-probe-feature'
expect_exit 0 "$RB" product list --module repository
expect_grep '^a-probe-feature'
expect_exit 0 "$RB" product show a-probe-feature
expect_grep 'The feature this case adds'

# --- the surfaces are derived, not authored: the file declares none, and the answer has them
"$RB" product show a-probe-feature --format json > "$S/one.json" 2>/dev/null
grep -q '"surfaces"' "$S/one.json" || { echo "    the answer carries no derived surfaces"; exit 1; }
grep -q '"route": "/features/a-probe-feature/"' "$S/one.json" \
  || { echo "    the answer carries no derived route"; exit 1; }
grep -q '"cli": true' "$S/one.json" \
  || { echo "    the module it names has a command-line path and the surface was not derived"; exit 1; }
grep -q 'surfaces' .ai/repo/features/a-probe-feature.md \
  && { echo "    the source file declares surfaces; the model derives them"; exit 1; }

# --- the matrix gained its row, with the marks derived from what it names
expect_exit 0 "$RB" product matrix
expect_grep 'a-probe-feature'
"$RB" product matrix --format json > "$S/matrix.json" 2>/dev/null
grep -q '"a-probe-feature"' "$S/matrix.json" || { echo "    the matrix does not carry the feature"; exit 1; }

# --- MCP serves it as a resource, and the module's tools are there once
expect_exit 0 "$RB" mcp --inspect
expect_grep '^resource    majordomus://feature/a-probe-feature$'
expect_grep 'tool        majordomus_features$'

# --- the site projection carries it, written by `generate`
expect_exit 0 "$RB" generate site --out "$S/gen"
[ -f "$S/gen/site/data/registry/product.json" ] || { echo "    generate site wrote no product.json"; exit 1; }
grep -q '"a-probe-feature"' "$S/gen/site/data/registry/product.json" \
  || { echo "    the site dataset does not carry the feature"; exit 1; }
grep -q '"/features/a-probe-feature/"' "$S/gen/site/data/registry/product.json" \
  || { echo "    the site dataset carries no route for the feature"; exit 1; }
grep -q 'feature:a-probe-feature' "$S/gen/site/data/registry/product-graph.json" \
  || { echo "    the derived graph does not carry the feature"; exit 1; }
# the public projection is an allow-list: the feature's prose stays in the file
grep -q 'Because the case says so' "$S/gen/site/data/registry/product.json" \
  && { echo "    the site dataset carries the feature's body; the projection is allow-listed"; exit 1; }

# ---------------------------------------------------------------- remove that one file
# a hidden registry anywhere would keep answering after the source is gone, which is the
# failure this half of the case exists to catch
git rm -q .ai/repo/features/a-probe-feature.md
git commit -qm "one file removed"
expect_exit 0 "$RB" product list
expect_no_grep 'a-probe-feature'
expect_exit 12 "$RB" product show a-probe-feature
expect_grep "no feature 'a-probe-feature'"
expect_exit 0 "$RB" mcp --inspect
expect_no_grep 'majordomus://feature/a-probe-feature'
expect_exit 0 "$RB" generate site --out "$S/gone"
grep -q 'a-probe-feature' "$S/gone/site/data/registry/product.json" \
  && { echo "    the site dataset still carries the removed feature"; exit 1; }

# ---------------------------------------------------------------- the contract is enforced
# an unresolved reference is an error with the nearest candidate, not a link to nothing
cat > .ai/repo/features/a-typo-feature.md <<'MD'
---
schema: feature/v1
id: a-typo-feature
kind: feature
title: 'A feature naming a module that does not exist'
headline: 'It named a module the executable does not have.'
summary: 'A feature whose module is a typo.'
status: draft
modules: [repositor]
docs: [.ai/repo/workflows/task-lifecycle.md]
---

## What it does

Because the case says so.

## What it does not do

Nothing.
MD
git add -A >/dev/null && git commit -qm typo
expect_exit 10 "$RB" product validate
expect_grep 'unknown module reference: "repositor"'
expect_grep 'did you mean: repository'
git rm -q .ai/repo/features/a-typo-feature.md && git commit -qm "typo removed"

# a derived value may not be authored: the schema has no `surfaces` key and no `route`
cat > .ai/repo/features/a-derived-feature.md <<'MD'
---
schema: feature/v1
id: a-derived-feature
kind: feature
title: 'A feature that wrote down what the registry knows'
headline: 'It declared the interfaces it is on.'
summary: 'A feature that claims its own surfaces.'
status: draft
modules: [repository]
docs: [.ai/repo/workflows/task-lifecycle.md]
surfaces: [cli, api, mcp]
---

## What it does

Because the case says so.

## What it does not do

Nothing.
MD
git add -A >/dev/null && git commit -qm "derived key"
expect_exit 10 "$RB" product validate
expect_grep "not in schema 'majordomus.feature/v1': surfaces"
git rm -q .ai/repo/features/a-derived-feature.md && git commit -qm "derived key removed"

# only a stable feature may be a chapter of the homepage
cat > .ai/repo/features/a-draft-chapter.md <<'MD'
---
schema: feature/v1
id: a-draft-chapter
kind: feature
title: 'A draft that asked to be featured'
headline: 'It is a draft and it asked for the homepage.'
summary: 'A draft feature that declares itself featured.'
status: draft
featured: true
modules: [repository]
docs: [.ai/repo/workflows/task-lifecycle.md]
---

## What it does

Because the case says so.

## What it does not do

Nothing.
MD
git add -A >/dev/null && git commit -qm "featured draft"
expect_exit 10 "$RB" product validate
expect_grep 'a-draft-chapter'
git rm -q .ai/repo/features/a-draft-chapter.md && git commit -qm "featured draft removed"
