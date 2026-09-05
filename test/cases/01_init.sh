# majordomus-covers: init
# majordomus-negative: init
# init creates the repository's AI layer and nothing else: no tool installation, no hook,
# no shell file, no .majordomus/. What it seeds under .ai/repo/ belongs to the repository
# from then on; what it seeds under .ai/local/ is this checkout's own and ignored.
. "$ROOT/test/lib.sh"
expect_exit 0 "$MJ" init
expect_grep 'next: majordomus update'

# --- the fresh layout, as the protocol describes it
for f in .ai/README.md .ai/manifest.yaml .ai/repo/README.md .ai/repo/policy.yaml \
         .ai/repo/profiles/debugging.yaml .ai/repo/rules/README.md \
         .ai/repo/rules/vendor/majordomus/manifest.yaml .ai/repo/knowledge/sources.yaml \
         .ai/repo/workflows/task-lifecycle.md .ai/local/state/decisions.md; do
  [ -f "$f" ] || { echo "    init did not create $f"; exit 1; }
done
[ -d .ai/local/state/handovers ] && [ -d .ai/repo/rules/project ] && [ -d .ai/repo/project ]
grep -q '^schema: ai-repository/v1$' .ai/manifest.yaml
[ ! -e .majordomus ] || { echo "    init created .majordomus/, which is only ever a tool installation"; exit 1; }
[ ! -e .envrc ] || { echo "    init touched .envrc"; exit 1; }

# --- the vendored baseline is the distribution's package, byte for byte, with its manifest
diff -r "$ROOT/share/standard/majordomus" .ai/repo/rules/vendor/majordomus >/dev/null \
  || { echo "    the vendored rule package differs from the one the distribution ships"; exit 1; }
[ "$(grep -c '^  - id: majordomus\.' .ai/repo/rules/vendor/majordomus/manifest.yaml)" = "$(ls .ai/repo/rules/vendor/majordomus/rules/*.md | wc -l | tr -d ' ')" ]

# --- the local half is ignored, and only the local half
grep -qx '.ai/local/' .gitignore
git check-ignore -q .ai/local/state/decisions.md
git check-ignore -q .ai/repo/policy.yaml && { echo "    the tracked half is ignored"; exit 1; }
git add -A >/dev/null; git commit -qm install
[ -z "$(git ls-files .ai/local)" ] || { echo "    local state was committed"; exit 1; }
git ls-files .ai/repo/policy.yaml | grep -q . || { echo "    the tracked half was not committed"; exit 1; }

# --- refuses to overwrite; --extend adds what is missing and touches nothing that exists
echo "custom: note" >> .ai/local/state/decisions.md
echo "# edited" >> .ai/repo/policy.yaml
expect_exit 15 "$MJ" init
expect_grep 'already exists'
rm .ai/repo/workflows/plan.md
expect_exit 0 "$MJ" init --extend
[ -f .ai/repo/workflows/plan.md ] || { echo "    --extend did not restore a missing file"; exit 1; }
expect_grep '# edited' .ai/repo/policy.yaml
expect_grep 'custom: note' .ai/local/state/decisions.md
# the ignore line is added once, however often init runs
expect_exit 0 "$MJ" init --extend
[ "$(grep -c '^.ai/local/$' .gitignore)" = 1 ]

# --- the ai-layer doctrine: doctor proves the layer, and goes red when it is not real
"$MJ" update >/dev/null
expect_exit 10 "$MJ" doctor      # hooks are not wired yet, nothing else fails
expect_grep 'OK +layout +\.ai/local/ — ignored'
expect_grep 'OK +layout +\.ai/ — every section the manifest names exists'
mv .ai/repo/workflows .ai/repo/workflows.off
expect_exit 10 "$MJ" doctor
expect_grep "FAIL layout +\.ai/repo/workflows — named by the manifest as section 'workflows' but absent"
mv .ai/repo/workflows.off .ai/repo/workflows
sed -i.bak '/^\.ai\/local\/$/d' .gitignore; rm -f .gitignore.bak
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL layout +\.ai/local/ — is not ignored by git'
printf '.ai/local/\n' >> .gitignore
# a manifest this executable cannot read is refused, with the reason, by every command
sed -i.bak 's/^schema: .*/schema: ai-repository\/v9/' .ai/manifest.yaml; rm -f .ai/manifest.yaml.bak
expect_exit 10 "$MJ" doctor
expect_grep "schema 'ai-repository/v9' is not ai-repository/v1"
sed -i.bak 's/^schema: .*/schema: ai-repository\/v1/' .ai/manifest.yaml; rm -f .ai/manifest.yaml.bak
printf 'extra: key\n' >> .ai/manifest.yaml
expect_exit 10 "$MJ" doctor
expect_grep 'unknown key'
sed -i.bak '/^extra: key$/d' .ai/manifest.yaml; rm -f .ai/manifest.yaml.bak
expect_exit 10 "$MJ" doctor
expect_no_grep 'FAIL +layout'

# --- pre-.ai project data is refused, not silently doubled
mkdir -p .majordomus && printf 'version: 1\n' > .majordomus/policy.yaml
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL layout +\.majordomus/policy.yaml — pre-\.ai project data beside the \.ai layer'
rm -rf .majordomus .ai
mkdir -p .majordomus && printf 'version: 1\n' > .majordomus/policy.yaml
expect_exit 15 "$MJ" init
expect_grep 'run: majordomus migrate'
rm -rf .majordomus

# --- outside a git repo
outside="$(mktemp -d "${TMPDIR:-/tmp}/mj-plain.XXXXXX")"; expect_exit 2 "$MJ" --repo "$outside" init; rm -rf "$outside"
