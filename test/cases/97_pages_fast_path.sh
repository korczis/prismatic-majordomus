# The Pages fast path, held to what makes it fast and to what keeps it honest.
#
# A performance property that nothing checks is a property the next edit removes without
# noticing, so this case asserts the shape of .github/workflows/pages.yml against the model it
# is an adapter over (.ai/repo/ci/pages.yaml), and the two facts the shape rests on: the
# canonical input fingerprint is the value the generator itself writes, and a build that skips
# generation produces the same site as a build that does not.
#
# It reads this repository's own files rather than the disposable fixture, because the subject
# is this repository's publication path.
. "$ROOT/test/lib.sh"
W="$ROOT/.github/workflows/pages.yml"
V="$ROOT/.github/workflows/validate.yml"
MODEL="$ROOT/.ai/repo/ci/pages.yaml"
P="$ROOT/scripts/pages"
[ -f "$W" ] || { echo "    no .github/workflows/pages.yml; nothing publishes the site on a push"; exit 1; }
[ -f "$MODEL" ] || { echo "    no .ai/repo/ci/pages.yaml; the fast path would carry its own policy"; exit 1; }
[ -x "$P" ] || { echo "    scripts/pages is missing or not executable"; exit 1; }

# 1. the model parses and the command reads it rather than carrying its own numbers
expect_exit 0 "$P" budget
expect_grep 'controlled'
"$P" budget --json > budget.json || { echo "    scripts/pages budget --json failed"; exit 1; }
for k in checkout setup build check publish controlled; do
  jq -e --arg k "$k" '.warm[$k] | numbers' budget.json >/dev/null || { echo "    the model declares no warm budget for $k"; exit 1; }
done
jq -e '.end_to_end | numbers' budget.json >/dev/null || { echo "    the model declares no end-to-end target"; exit 1; }
grep -qE 'budget|controlled' "$P" && ! grep -qE '^[^#]*(checkout|controlled)[=:][[:space:]]*[0-9]+' "$P" \
  || { echo "    scripts/pages carries a budget number of its own; the model owns them"; exit 1; }

# 2. the workflow triggers directly on a master push, on the derived paths, and nowhere else.
#    A chain through another workflow would put a second scheduler in front of publication.
awk '/^on:/{f=1} /^permissions:/{f=0} f' "$W" | grep -q 'workflow_run' \
  && { echo "    pages.yml is chained behind another workflow; publication would wait for a second scheduler"; exit 1; }
awk '/^on:/{f=1} /^permissions:/{f=0} f' "$W" | grep -A1 '^  push:' | grep -q "branches: \[$("$ROOT/bin/majordomus" --repo "$ROOT" version >/dev/null 2>&1; sed -n 's/^  branch: //p' "$MODEL")\]" \
  || { echo "    pages.yml does not push-trigger on the branch the model names"; exit 1; }
# the paths block is exactly what the model derives from the gate model; neither is written twice
awk '/^    paths:$/{f=1; next} /^  [a-z_]+:/{f=0} f && /^      - /' "$W" | sed 's/^      - //' | sort > declared.txt
"$P" paths | sort > derived.txt
diff -u derived.txt declared.txt > paths.diff 2>&1 || {
  echo "    pages.yml's paths: differ from 'scripts/pages paths'; regenerate the block"; cat paths.diff; exit 1; }
grep -q 'workflow_dispatch' "$W" || { echo "    pages.yml cannot be dispatched by hand"; exit 1; }

# 3. a stale deployment is cancelled: the site is a projection of the newest commit, so an
#    older run finishing after a newer push would publish the older tree
grep -q 'cancel-in-progress: true' "$W" || { echo "    pages.yml does not cancel superseded runs; an older commit could overwrite a newer one"; exit 1; }
grep -qE '^  group: pages-' "$W" || { echo "    pages.yml has no pages concurrency group of its own"; exit 1; }

# 4. minimal permissions, and only what the deploy needs
awk '/^permissions:/{f=1; next} /^[a-z]/{f=0} f && /^  [a-z]/' "$W" | sed 's/^  //' | sort > perms.txt
[ "$(cat perms.txt)" = "contents: write" ] || { echo "    pages.yml asks for more than the gh-pages push needs:"; cat perms.txt; exit 1; }

# 5. the heavy gates are not on the publication path, and are still somewhere. Only what the
#    jobs execute counts: the trigger paths name scripts/site-probe as an input that can change
#    the site, which is not the same as running it.
awk '/^jobs:/{f=1} f' "$W" | grep -vE '^\s*#' > jobs.yml
for heavy in 'test/run.sh' 'site-probe' 'rust-check' 'llvm-cov' 'cargo test'; do
  grep -qF "$heavy" jobs.yml && { echo "    pages.yml runs $heavy on the deployment critical path"; exit 1; }
  grep -qF "$heavy" "$V" || { echo "    $heavy runs in neither workflow; a gate was dropped rather than moved"; exit 1; }
done
# and what does guard the published bytes is there
grep -q 'scripts/pages build' "$W" || { echo "    pages.yml does not build the site through scripts/pages"; exit 1; }
grep -q 'scripts/pages check' "$W" || { echo "    pages.yml does not check the built site"; exit 1; }
grep -qF "$(sed -n 's/^  publish: //p' "$MODEL")" "$W" || { echo "    pages.yml does not publish with the command the model names"; exit 1; }
# no gating step is allowed to swallow its own failure; the publication measurement may, and
# is the only step that does, because GitHub being slow to serve is not this repository failing
[ "$(grep -c 'continue-on-error: true' "$W")" = 1 ] || { echo "    pages.yml lets more (or fewer) than the publication measurement fail softly"; exit 1; }
awk '/scripts\/pages (build|check)/,/^      - /' "$W" | grep -qE '\|\|[[:space:]]*true' \
  && { echo "    a gating step of pages.yml swallows failure"; exit 1; }

# 6. exactly the two workflows, with the split the architecture states: one decides whether a
#    change may merge, the other what the public site shows
[ "$(ls "$ROOT"/.github/workflows/*.yml | wc -l | tr -d ' ')" = 2 ] \
  || { echo "    there are not exactly two workflows (validate.yml and pages.yml):"; ls "$ROOT"/.github/workflows/; exit 1; }
grep -qE 'scripts/site-deploy' "$V" && { echo "    validate.yml still deploys; publication belongs to pages.yml alone"; exit 1; }

# 7. the fingerprint is the generator's own value, not a second hash of the same files
have="$("$ROOT/scripts/generate-site-data" --fingerprint)"
want="$(jq -r '.source_hash' "$ROOT/site/data/generated/source.json")"
[ "$have" = "$want" ] || { echo "    generate-site-data --fingerprint is $have, source.json carries $want; the fast path would refuse a current tree"; exit 1; }
expect_exit 0 "$P" current
expect_grep 'current for this tree'

# 8. skipping the generation changes nothing about the site. This is the whole premise of the
#    fast path: the committed derived data is the projection, so rendering it is rendering the
#    canonical sources. A build without --no-data and a build with it must agree byte for byte.
if command -v zola >/dev/null 2>&1 && [ -d "$ROOT/node_modules/tailwindcss" ]; then
  a="$(mktemp -d "${TMPDIR:-/tmp}/mj-pages-a.XXXXXX")"; b="$(mktemp -d "${TMPDIR:-/tmp}/mj-pages-b.XXXXXX")"
  ( cd "$ROOT" && scripts/site-build >/dev/null 2>&1 ) || { echo "    scripts/site-build failed"; exit 1; }
  cp -R "$ROOT/site/public/." "$a/"
  ( cd "$ROOT" && scripts/site-build --no-data >/dev/null 2>&1 ) || { echo "    scripts/site-build --no-data failed"; exit 1; }
  cp -R "$ROOT/site/public/." "$b/"
  # build.json carries this build's own generation moment in source.json; the site's bytes do not
  if ! diff -r "$a" "$b" > sitediff.txt 2>&1; then
    echo "    a build that skipped the generation differs from one that did not:"; head -20 sitediff.txt; rm -rf "$a" "$b"; exit 1
  fi
  rm -rf "$a" "$b"
else
  echo "    (zola or node_modules absent: the build equivalence is CI's)"
fi

# 9. the identity the probe reads is served, and names the commit the site was built from
[ -f "$ROOT/site/public/$(sed -n 's/^  identity: //p' "$MODEL")" ] \
  || { echo "    the site does not serve $(sed -n 's/^  identity: //p' "$MODEL"); a deployment could not be verified from outside"; exit 1; }
jq -e '.commit | strings' "$ROOT/site/public/$(sed -n 's/^  identity: //p' "$MODEL")" >/dev/null \
  || { echo "    the served identity carries no commit"; exit 1; }
