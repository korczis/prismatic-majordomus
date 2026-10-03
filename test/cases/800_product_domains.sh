# A product domain is one file under the layer, and its members are the features that name
# it. This case adds one domain and one feature naming it — nothing else, no template, no
# navigation, no registry — and asks the command line, the JSON answer, MCP and the site
# projection whether they know both, and that the domain's members, surfaces and route were
# derived. Then it removes the domain and asks again, and holds the contract that keeps the
# map honest: a stable feature filed under no domain, or under one that does not exist, is
# refused before anything renders it.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
# claims: product-domains-derived
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj800.XXXXXX")"; trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null
grep -q "kind: domain" .ai/repo/knowledge/sources.yaml \
  || { echo "    init did not seed the domain source class"; exit 1; }
git add -A >/dev/null && git commit -qm install

# a repository that declares no domain lists none and refuses nothing for it
expect_exit 0 "$RB" product domains
expect_grep '^0 domain'

# ---------------------------------------------------------------- one domain, one member
mkdir -p .ai/repo/features/domains
cat > .ai/repo/features/domains/a-probe-domain.md <<'MD'
---
schema: domain/v1
id: a-probe-domain
kind: domain
title: 'Probing'
headline: 'One domain file and one feature naming it, and every interface answered both.'
problem: 'A map nobody can trust, because somebody keeps its membership by hand.'
status: stable
weight: 10
---

# Probing

Because the case says so.
MD
cat > .ai/repo/features/a-probe-feature.md <<'MD'
---
schema: feature/v1
id: a-probe-feature
kind: feature
title: 'The feature this case files under the probe domain'
headline: 'It names its domain, and the domain names nothing.'
summary: 'A feature that exists to be a member.'
status: stable
weight: 10
domain: a-probe-domain
modules: [repository]
kinds: [rule]
docs: [.ai/repo/workflows/task-lifecycle.md]
---

## What it does

Because the case says so.

## What it does not do

Nothing the case does not say.
MD
git add -A >/dev/null && git commit -qm "one domain, one member"
added="$(git diff --name-only HEAD~1 HEAD | LC_ALL=C sort | tr '\n' ' ')"
[ "$added" = ".ai/repo/features/a-probe-feature.md .ai/repo/features/domains/a-probe-domain.md " ] \
  || { echo "    more than the two files changed: $added"; exit 1; }

expect_exit 0 "$RB" product validate
expect_grep '^valid:'
expect_exit 0 "$RB" product domains
expect_grep '^a-probe-domain  Probing'
expect_grep 'a-probe-feature'
expect_grep '^1 domain'
expect_exit 0 "$RB" product list --domain a-probe-domain
expect_grep '^a-probe-feature'
expect_exit 0 "$RB" product show a-probe-feature
expect_grep 'domain a-probe-domain .*/domains/a-probe-domain/'

"$RB" product domains --format json > "$S/d.json" 2>/dev/null
grep -q '"route": "/domains/a-probe-domain/"' "$S/d.json" || { echo "    no derived route"; exit 1; }
grep -q '"cli": true' "$S/d.json" || { echo "    the member's surfaces were not carried up"; exit 1; }
grep -q 'features:' .ai/repo/features/domains/a-probe-domain.md \
  && { echo "    the domain lists its members; membership is the feature's"; exit 1; }

expect_exit 0 "$RB" mcp --inspect
expect_grep 'tool        majordomus_product_domains$'

expect_exit 0 "$RB" generate site --out "$S/gen"
P="$S/gen/site/data/registry/product.json"
grep -q '"/domains/a-probe-domain/"' "$P" || { echo "    the site dataset carries no domain route"; exit 1; }
grep -q 'Because the case says so' "$P" \
  && { echo "    the site dataset carries a body; the projection is allow-listed"; exit 1; }

# ---------------------------------------------------------------- the contract
# a stable feature filed under no domain, while one is declared, is refused
sed -i.bak '/^domain: /d' .ai/repo/features/a-probe-feature.md && rm -f .ai/repo/features/a-probe-feature.md.bak
git add -A >/dev/null && git commit -qm "unfiled"
expect_exit 10 "$RB" product validate
expect_grep 'names no domain'

# a domain that does not exist is an unknown reference, with the nearest candidate
sed -i.bak 's/^weight: 10$/weight: 10\ndomain: a-probe-domian/' .ai/repo/features/a-probe-feature.md && rm -f .ai/repo/features/a-probe-feature.md.bak
git add -A >/dev/null && git commit -qm "a typo"
expect_exit 10 "$RB" product validate
expect_grep 'unknown domain reference: "a-probe-domian"'
expect_grep 'did you mean: a-probe-domain'

# ---------------------------------------------------------------- remove the domain
git rm -q .ai/repo/features/domains/a-probe-domain.md .ai/repo/features/a-probe-feature.md
git commit -qm "removed"
expect_exit 0 "$RB" product domains
expect_no_grep 'a-probe-domain'
expect_exit 0 "$RB" generate site --out "$S/gone"
grep -q 'a-probe-domain' "$S/gone/site/data/registry/product.json" \
  && { echo "    the site dataset still carries the removed domain"; exit 1; }
exit 0
