# majordomus-covers: script:scripts/generate-site-data
# claims: product-domains-derived, product-features-discovered
# A feature is explored from its domain, and nothing but the feature's own file puts it
# there. This case adds ONE feature file naming a domain the repository already declares —
# no template, no navigation, no registry, no Rust touched — and runs the site pipeline the
# way the build does: the executable writes the product dataset, the generator writes the
# pages, zola renders them. Then it reads the rendered HTML: the feature is one card on
# /features/, inside its domain's group, linking its own page; its page exists and its
# breadcrumb runs Product, then the domain, then the feature; the domain's page lists it.
# Then it deletes the file, runs the same pipeline, and holds the other direction: no card,
# no page, no link to it left anywhere on the site.
#
# 97_product_features and 800_product_domains prove the dataset follows a feature file;
# this proves the rendered site does. Skips itself when there is neither cargo nor
# MAJORDOMUS_BIN, or no jq or zola.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || skip "jq absent"
command -v zola >/dev/null || skip "zola absent"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj810.XXXXXX")"; trap 'rm -rf "$S"' EXIT

# ---------------------------------------------------------------- a copy of this site
F="$S/repo"; fixture_repo "$F" AGENTS.md docs
# every object of the layer a feature can name, so the copy's product model validates as this
# repository's does: the kinds it names are counted from the objects present, and the
# crate (its manifest and its sources, never its build) is what makes its rustdoc a surface
# (web::discover) and what the module registry is read from
# (the benchmarks' records are left out: their contract tracks fixtures of other cases)
for p in "$ROOT"/.ai/repo/*; do
  n="${p##*/}"; [ "$n" = benchmarks ] && continue
  if [ -d "$p" ]; then mkdir -p "$F/.ai/repo/$n"; cp -R "$p/." "$F/.ai/repo/$n/"; else cp "$p" "$F/.ai/repo/$n"; fi
done
mkdir -p "$F/apps/majordomus-cli/src"
cp "$ROOT/apps/majordomus-cli/Cargo.toml" "$F/apps/majordomus-cli/"; cp -R "$ROOT/apps/majordomus-cli/src/." "$F/apps/majordomus-cli/src/"
mkdir -p "$F/site/data" "$F/test"
cp "$ROOT"/site/data/*.toml "$F/site/data/"
cp -R "$ROOT/site/content-src" "$ROOT/site/templates" "$ROOT/site/static" "$ROOT/site/config.toml" "$F/site/"
cp -R "$ROOT/test/cases" "$F/test/"
# the stylesheets are Tailwind build products, ignored by git; the pages only link them
[ -f "$F/site/static/app.css" ] || printf '/* fixture: no Tailwind build here. */\n' > "$F/site/static/app.css"
[ -f "$F/site/static/kit.css" ] || printf '/* fixture: no Tailwind build here. */\n' > "$F/site/static/kit.css"
git -C "$F" init -q; git -C "$F" config user.email fixture@example.invalid; git -C "$F" config user.name fixture
git -C "$F" add -A >/dev/null; git -C "$F" commit -qm fixture

# The domain is read from the repository's own dataset, never named here: the first one the
# product shows. Its route and title are what the pages must carry.
PD="$ROOT/site/data/registry/product.json"
DOM="$(jq -r '[.domains[] | select(.status == "stable" and (.features | length) > 0)][0].id' "$PD")"
[ -n "$DOM" ] && [ "$DOM" != null ] || { echo "    the product shows no domain to file a feature under"; exit 1; }

build() { # the pipeline scripts/site-build runs, without the stylesheets: dataset, pages, html
  ( cd "$F" && run_quiet "$S/gen.err" "$RB" generate site ) || { echo "    majordomus generate site failed"; cat "$S/gen.err"; exit 1; }
  # the generator refuses a dataset that does not validate; say why, from the executable
  ( cd "$F" && "$RB" product validate > "$S/validate.out" 2>&1 ) \
    || { echo "    the fixture's product model does not validate:"; grep -A1 '^ERROR' "$S/validate.out" | head -16 | sed 's/^/    | /'; exit 1; }
  run_quiet "$S/data.err" "$F/scripts/generate-site-data" --no-scenarios \
    || { echo "    generate-site-data failed"; tail -5 "$S/data.err"; exit 1; }
  # the kit's screens replay recorded runs, and a generation without the scenarios records
  # none: they are another case's pages (790, 791), so this build leaves them out
  rm -rf "$S/public" "$F/site/content/kit"
  ( cd "$F/site" && run_quiet "$S/zola.err" zola build --force -o "$S/public" >/dev/null ) \
    || { echo "    zola could not build the fixture site"; tail -5 "$S/zola.err"; exit 1; }
}
cards_on() { # page -> "<card id>\t<group>" for every feature card, in page order
  awk '{ line = $0
    while (match(line, /data-features-domain="[^"]*"|data-feature-card="[^"]*"/)) {
      tok = substr(line, RSTART, RLENGTH); line = substr(line, RSTART + RLENGTH)
      split(tok, kv, "\""); if (tok ~ /^data-features-domain/) grp = kv[2]; else print kv[2] "\t" grp
    } }' "$1"
}

# ---------------------------------------------------------------- add exactly one file
cat > "$F/.ai/repo/features/a-probe-feature.md" <<MD
---
schema: feature/v1
id: a-probe-feature
kind: feature
title: 'The feature this case files under a domain'
short_title: 'The probe'
headline: 'One file was added and nothing else was, and the site drew it under its domain.'
summary: 'A feature that exists to prove that adding one file is the whole act.'
status: stable
weight: 10000
domain: $DOM
modules: [repository]
kinds: [rule]
docs: [docs/PRODUCT.md]
tags: [probe]
---

## What it does

Because the case says so.

## What it does not do

Nothing the case does not say.
MD
git -C "$F" add -A >/dev/null; git -C "$F" commit -qm "one feature"
added="$(git -C "$F" diff --name-only HEAD~1 HEAD)"
[ "$added" = ".ai/repo/features/a-probe-feature.md" ] || { echo "    more than the feature file changed: $added"; exit 1; }
build

FP="$F/site/data/registry/product.json"
DROUTE="$(jq -r --arg d "$DOM" '.domains[] | select(.id == $d) | .route' "$FP")"
DTITLE="$(jq -r --arg d "$DOM" '.domains[] | select(.id == $d) | .title' "$FP")"
jq -e --arg d "$DOM" '.domains[] | select(.id == $d) | [.features[].id] | index("a-probe-feature")' "$FP" >/dev/null \
  || { echo "    the dataset does not file the probe under $DOM"; exit 1; }

# one card on /features/, in its domain's group, linking its page
FI="$S/public/features/index.html"
[ -f "$FI" ] || { echo "    the build has no /features/"; exit 1; }
rows="$(cards_on "$FI" | awk -F'\t' '$1 == "a-probe-feature"')"
[ "$(printf '%s\n' "$rows" | grep -c .)" = 1 ] || { echo "    /features/ draws the probe $(printf '%s\n' "$rows" | grep -c .) time(s), not once"; exit 1; }
[ "$(printf '%s' "$rows" | cut -f2)" = "$DOM" ] || { echo "    /features/ draws the probe under '$(printf '%s' "$rows" | cut -f2)', not $DOM"; exit 1; }
grep -q '/features/a-probe-feature/"' "$FI" || { echo "    the probe's card does not link its page"; exit 1; }
grep -qF 'One file was added and nothing else was, and the site drew it under its domain.' "$FI" \
  || { echo "    the card does not carry the feature's own promise"; exit 1; }

# its page, placed under its domain
P="$S/public/features/a-probe-feature/index.html"
[ -f "$P" ] || { echo "    the probe has no page"; exit 1; }
crumb="$(awk '/aria-label="Breadcrumb"/ { on = 1 } on { print } on && /<\/nav>/ { exit }' "$P")"
printf '%s' "$crumb" | grep -qF "$DROUTE\"" || { echo "    the probe's breadcrumb does not link its domain $DROUTE"; exit 1; }
printf '%s' "$crumb" | grep -qF ">$DTITLE<" || { echo "    the probe's breadcrumb does not name its domain $DTITLE"; exit 1; }
printf '%s' "$crumb" | grep -qF '>The probe<' || { echo "    the probe's breadcrumb does not end at the feature"; exit 1; }
grep -qF 'Because the case says so.' "$P" || { echo "    the probe's page does not carry its own body"; exit 1; }
# a feature that names no claim and no use case says so, rather than printing a zero
grep -q 'data-feature-no-evidence' "$P" || { echo "    the probe's page does not say that no evidence is linked"; exit 1; }
# its domain's other members are its neighbours, each a card linking its own page
grep -q 'data-feature-card="' "$P" || { echo "    the probe's page shows no related feature"; exit 1; }
grep -q '/features/a-probe-feature/"' "$S/public${DROUTE}index.html" || { echo "    the domain page does not list the probe"; exit 1; }

# ---------------------------------------------------------------- remove it
git -C "$F" rm -q .ai/repo/features/a-probe-feature.md; git -C "$F" commit -qm removed
build
jq -e '[.features[].id] | index("a-probe-feature") | not' "$F/site/data/registry/product.json" >/dev/null \
  || { echo "    the dataset still carries the removed feature"; exit 1; }
[ ! -e "$S/public/features/a-probe-feature" ] || { echo "    the removed feature still has a page"; exit 1; }
left="$(grep -rl 'a-probe-feature' "$S/public" 2>/dev/null || true)"
[ -z "$left" ] || { echo "    the removed feature is still named by: $(printf '%s' "$left" | sed "s#$S/public##" | tr '\n' ' ')"; exit 1; }
# and the rest of the domain is still drawn: removal took one card, not the group
cards_on "$S/public/features/index.html" | awk -F'\t' -v d="$DOM" '$2 == d' | grep -q . \
  || { echo "    removing one feature emptied its domain's group"; exit 1; }
exit 0
