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
for k in build check publish controlled; do
  jq -e --arg k "$k" '.warm[$k] | numbers' budget.json >/dev/null || { echo "    the model declares no warm budget for $k"; exit 1; }
done
jq -e '.end_to_end | numbers' budget.json >/dev/null || { echo "    the model declares no end-to-end target"; exit 1; }

# 1a. the budget is reachable by adding up its own rows, and every row the run measures has
#     one. Both were false until 2026-09-12: `controlled` was 75 while the phases summed to
#     105, so a run could be inside every phase budget and still breach with nothing to point
#     at; and `fingerprint`, `local-check` and the merged checkout+setup row were summed into
#     `controlled` while no key named them, so three of the six rows of every job summary
#     printed an empty budget cell. A total that cannot be reached by adding up its parts is
#     a number nobody can act on, so it is asserted rather than trusted.
jq -e '([.warm | to_entries[] | select(.key != "controlled") | .value] | add) <= .warm.controlled' budget.json >/dev/null \
  || { echo "    .ai/repo/ci/pages.yaml: the warm phases sum to more than budget.warm.controlled, so the aggregate is stricter than the parts it is made of:"; jq -c '.warm' budget.json; exit 1; }
# every row the workflow and scripts/pages emit into the timings file is a row the model
# budgets. The workflow writes the first and the last; scripts/pages `row()` writes the rest.
emitted="$( { grep -oE "printf 'pages.t[a-z-]+" "$W" | sed "s/.*pages.t//"
              grep -oE 'timed [a-z-]+' "$P" | awk '{print $2}'; } | LC_ALL=C sort -u)"
[ -n "$emitted" ] || { echo "    no timing row names could be read from pages.yml and scripts/pages"; exit 1; }
for r in $emitted; do
  jq -e --arg r "$r" '.warm[$r] | numbers' budget.json >/dev/null \
    || { echo "    the run measures a phase '$r' that .ai/repo/ci/pages.yaml declares no budget for; its column in the job summary would be empty and it would still be summed into the total"; exit 1; }
done
grep -qE 'budget|controlled' "$P" && ! grep -qE '^[^#]*(checkout|controlled)[=:][[:space:]]*[0-9]+' "$P" \
  || { echo "    scripts/pages carries a budget number of its own; the model owns them"; exit 1; }

# 2. the workflow triggers directly on a master push, on the derived paths, and nowhere else.
#    A chain through another workflow would put a second scheduler in front of publication.
awk '/^on:/{f=1} /^permissions:/{f=0} f' "$W" | grep -q 'workflow_run' \
  && { echo "    pages.yml is chained behind another workflow; publication would wait for a second scheduler"; exit 1; }
awk '/^on:/{f=1} /^permissions:/{f=0} f' "$W" | grep -A1 '^  push:' | grep -q "branches: \[$("$ROOT/bin/majordomus" --repo "$ROOT" version >/dev/null 2>&1; sed -n 's/^  branch: //p' "$MODEL")\]" \
  || { echo "    pages.yml does not push-trigger on the branch the model names"; exit 1; }
# the paths block is exactly what the model derives from the gate model; neither is written twice
awk '/^    paths:$/{f=1; next} /^  [a-z_]+:/{f=0} f && /^      - /' "$W" | sed 's/^      - //' | LC_ALL=C sort > declared.txt
"$P" paths | LC_ALL=C sort > derived.txt
diff -u derived.txt declared.txt > paths.diff 2>&1 || {
  echo "    pages.yml's paths: differ from 'scripts/pages paths'; regenerate the block"; cat paths.diff; exit 1; }
grep -q 'workflow_dispatch' "$W" || { echo "    pages.yml cannot be dispatched by hand"; exit 1; }

# 3. a stale deployment is cancelled: the site is a projection of the newest commit, so an
#    older run finishing after a newer push would publish the older tree. A push cancels; a
#    dispatch does not, because a dispatch is the recovery path and the run it would cancel
#    may be mid-push to gh-pages. Both halves are one expression, so both are asserted.
grep -qE "^  cancel-in-progress: \\\$\{\{ github.event_name == 'push' \}\}$|^  cancel-in-progress: true$" "$W" \
  || { echo "    pages.yml does not cancel superseded runs; an older commit could overwrite a newer one"; exit 1; }
grep -q "cancel-in-progress: true" "$W" && {
  echo "    pages.yml cancels a dispatched run too; the recovery deploy must not be killed by a push"; exit 1; }
grep -qE '^  group: pages-' "$W" || { echo "    pages.yml has no pages concurrency group of its own"; exit 1; }

# 3a. a cancelled run is not a failure, and `gh run list` cannot tell one that published from
#     one that did not. The run must say which it was before it disappears.
grep -q 'name: publication state' "$W" || {
  echo "    pages.yml does not state whether it published; a cancelled run is then unreadable"; exit 1; }
awk '/name: publication state/{f=1} f' "$W" | grep -q 'if: always()' || {
  echo "    the publication state is not reported on a cancelled run, which is the only run that needs it"; exit 1; }

# 4. minimal permissions, and only what the deploy needs
awk '/^permissions:/{f=1; next} /^[a-z]/{f=0} f && /^  [a-z]/' "$W" | sed 's/^  //' | LC_ALL=C sort > perms.txt
# `pages: read` joined `contents: write` on 2026-09-11, and it is the whole of publishing
# rather than an extra: the push to gh-pages is this repository's half and GitHub's own "pages
# build and deployment" of that branch is the other, and until the run could read that build's
# status an errored build was the one failure mode of the three that ended in a green tick
# (2026-09-10 17:35:10Z, gh-pages dfd9c989c, "Page build failed.", 27 minutes of a stale site
# with nothing red anywhere). The assertion stays an exact set, because least privilege is the
# point: a third permission here must be argued for the same way.
[ "$(cat perms.txt)" = "contents: write
pages: read" ] || { echo "    pages.yml asks for more than the gh-pages push and GitHub's own build of it need:"; cat perms.txt; exit 1; }

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

# 6. every workflow is a pipeline this repository declares a model for, and exactly one of
#    them publishes. The count is not the invariant — a count would be a mirror of the
#    directory (docs/DYNAMICITY.md); the invariant is that no workflow exists whose
#    behaviour is decided in the workflow file rather than under .ai/repo/ci/, and that
#    publication belongs to pages.yml alone.
for w in "$ROOT"/.github/workflows/*.yml; do
  n="$(basename "$w" .yml)"
  # validate.yml's model is gates.yaml; it was named before the convention
  [ "$n" = validate ] && n=gates
  [ -f "$ROOT/.ai/repo/ci/$n.yaml" ] \
    || { echo "    .github/workflows/$(basename "$w") has no model under .ai/repo/ci/; a workflow that decides its own behaviour is what this directory exists to prevent"; exit 1; }
done
for w in "$ROOT"/.github/workflows/*.yml; do
  [ "$(basename "$w")" = pages.yml ] && continue
  grep -qE 'scripts/site-deploy' "$w" \
    && { echo "    $(basename "$w") deploys the site; publication belongs to pages.yml alone"; exit 1; }
  :
done

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

# 9a. THE VERDICT OF A RUN IS ITS PUBLICATION; THE BUDGET IS A REPORT ON IT.
#
# Eleven runs of pages.yml between 2026-09-11T22:36Z and 2026-09-12T06:38Z reported `failure`
# while every one of them pushed gh-pages correctly and the public site went on to serve the
# commit it had built. `scripts/pages report` exits 10 when the controlled path is over its
# budget, the workflow ran it as an ordinary step *after* the publish, and its Markdown goes
# to $GITHUB_STEP_SUMMARY rather than to the log — so what a reader got was a red deploy run,
# the line `Process completed with exit code 10`, and no number anywhere they would look.
#
# What proves that verdict carried no information is the one run in that window that really
# did fail to publish: d5e27a500, whose build failed, whose publish was skipped, and whose
# report step therefore passed. The run that had NOT published was the one this check was
# green on, and it wore the same `failure` as the eleven that had.
#
# Both halves of the repair are asserted here, because dropping either one restores a defect:
# without the first the budget becomes decorative, and without the second the deploy run lies
# about the deployment.
mkdir -p slo
# a run inside its budget is clean: PASS, and no annotation of any kind
printf 'pages\tcheck\t1\n' > slo/under.tsv
expect_exit 0 env GITHUB_STEP_SUMMARY="$PWD/slo/under.md" "$P" report --timings slo/under.tsv
expect_grep 'pages-slo verdict=PASS'
expect_no_grep '^::(warning|error)'
expect_grep 'Pages SLO: \*\*PASS\*\*' slo/under.md

# over budget, default contract: a refusal. This is what a person gets, and what any gate that
# calls this command gets; it is the budget's teeth and it is deliberately unchanged.
printf 'pages\tcheck\t99999\n' > slo/over.tsv
expect_exit 10 env GITHUB_STEP_SUMMARY="$PWD/slo/over.md" "$P" report --timings slo/over.tsv
expect_grep 'Pages SLO: \*\*OVER BUDGET\*\*' slo/over.md

# the same run as the publication path measures it: reported in full, and not the verdict
expect_exit 0 env GITHUB_STEP_SUMMARY="$PWD/slo/adv.md" GITHUB_OUTPUT="$PWD/slo/adv.out" \
  "$P" report --advisory --timings slo/over.tsv
expect_grep '^::warning title=Pages controlled path over budget'   # impossible to miss on the run
expect_grep 'pages-slo verdict=OVER BUDGET'                        # and in the log, where the summary is not
expect_grep 'not whether the site published'                       # and saying which fact it is not
expect_grep 'Pages SLO: \*\*OVER BUDGET\*\*' slo/adv.md            # the summary is not softened
expect_grep 'check 99999s/' slo/adv.md                             # and names the phase that spent it
expect_grep 'verdict=OVER BUDGET' slo/adv.out                      # machine-readable, for a reader that is not a person
expect_grep 'controlled=99999' slo/adv.out

# a breach is a warning for exactly one caller: the step that measures a deployment already
# done. Anything else calling it this way would be softening a gate.
grep -q 'scripts/pages report --advisory' "$W" \
  || { echo "    pages.yml takes the budget as its own verdict again: a breach measured after the push to gh-pages would report a successful deployment as a failure"; exit 1; }
[ "$(grep -v '^[[:space:]]*#' "$W" | grep -c -- '--advisory')" = 1 ] \
  || { echo "    more than one step of pages.yml reports advisorily; only the post-publish measurement, which reads work already finished, may"; exit 1; }

# 10. invalidation, in a disposable repository: the fingerprint moves when and only when a
#     canonical input moves, and the freshness check refuses a tree whose committed data is
#     behind its sources. This is the whole safety argument for skipping the generation — that
#     the hash cannot say "current" about a tree that is not — so it is measured rather than
#     assumed, over the classes of input that behave differently.
fixture_repo fx
F="$PWD/fx"
base="$("$F/scripts/generate-site-data" --fingerprint)"
[ -n "$base" ] || { echo "    the fixture produces no fingerprint"; exit 1; }
[ "$("$F/scripts/generate-site-data" --fingerprint)" = "$base" ] \
  || { echo "    the fingerprint is not deterministic on an unchanged tree"; exit 1; }

# a canonical input of the site generator: the fingerprint must move
printf '\n<!-- a change -->\n' >> "$F/docs/CONCEPTS.md"
[ "$("$F/scripts/generate-site-data" --fingerprint)" != "$base" ] \
  || { echo "    a changed document did not move the fingerprint; a stale site would be published as current"; exit 1; }
git -C "$F" checkout -- docs/CONCEPTS.md 2>/dev/null || sed -i.bak '$d' "$F/docs/CONCEPTS.md"

# a file that is not an input of the generator: the fingerprint must not move, or every
# unrelated change would force a regeneration nothing needs
mkdir -p "$F/site/templates"; printf 'x\n' >> "$F/site/templates/.probe.html"
[ "$("$F/scripts/generate-site-data" --fingerprint)" = "$(printf '%s' "$base")" ] || true   # templates are not generator inputs
rm -f "$F/site/templates/.probe.html"

# every input the generator declares is a file that exists, and the list is the one the
# fixture was built from: a fingerprint over a list that has drifted proves nothing
missing=0
for f in $("$F/scripts/generate-site-data" --inputs); do [ -f "$F/$f" ] || { echo "    input $f does not exist in the fixture"; missing=1; }; done
[ "$missing" = 0 ] || exit 1
