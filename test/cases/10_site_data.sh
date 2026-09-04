# The site data normaliser: canonical files in, stable JSON out, and --check detects drift.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
fixture_repo "$T" AGENTS.md docs site/data/marketing.toml site/content-src test/cases
git -C "$T" add -A >/dev/null; git -C "$T" commit -qm fixture
expect_exit 0 "$T/scripts/generate-site-data"
for f in project profiles policy capabilities lifecycle docs diagrams source; do [ -f "$T/site/data/generated/$f.json" ]; jq -e '.schema == 1' "$T/site/data/generated/$f.json" >/dev/null; done
# version comes from the CLI, profiles from the skeleton, claims from CLAIMS.yaml
[ "$(jq -r .version "$T/site/data/generated/project.json")" = "$(sed -n 's/^MJ_VERSION="\(.*\)"/\1/p' "$ROOT/bin/majordomus")" ]
[ "$(jq '.profiles | length' "$T/site/data/generated/profiles.json")" = "$(ls "$ROOT"/share/skeleton/profiles/*.yaml | wc -l | tr -d ' ')" ]
# count the claims section only: docs/CLAIMS.yaml also declares its statuses as `- id:` entries
[ "$(jq '.claims | length' "$T/site/data/generated/capabilities.json")" = "$(awk '/^claims:/{c=1;next} c&&/^  - id: /{n++} END{print n+0}' "$ROOT/docs/CLAIMS.yaml")" ]
jq -e '.claims | all(.status == "guaranteed" and .test == null | not)' "$T/site/data/generated/capabilities.json" >/dev/null
jq -e '.principles | length >= 8' "$T/site/data/generated/lifecycle.json" >/dev/null
jq -e '.diagrams.lifecycle.mermaid | contains("no_match")' "$T/site/data/generated/diagrams.json" >/dev/null
# derived content exists, has front matter, and projected GitHub-native syntax
[ -f "$T/site/content/docs/cli.md" ]; expect_grep '^source = "docs/CLI.md"' "$T/site/content/docs/cli.md"
expect_grep '<div class="overflow-x-auto">' "$T/site/content/docs/cli.md"
# --check passes when in sync, fails after a canonical edit
expect_exit 0 "$T/scripts/generate-site-data" --check
sed -i.bak 's/^effort: high$/effort: xhigh/' "$T/share/skeleton/profiles/debugging.yaml"; rm -f "$T/share/skeleton/profiles/debugging.yaml.bak"
expect_exit 10 "$T/scripts/generate-site-data" --check
expect_grep 'STALE site/data/generated/profiles.json'
# a guaranteed claim without a test is refused
git -C "$T" checkout -q -- share
python3 - "$T/docs/CLAIMS.yaml" <<'PY'
import sys,re; p=sys.argv[1]; s=open(p).read()
s=s.replace("    test: test/cases/01_init.sh\n    status: guaranteed","    test: '-'\n    status: guaranteed",1); open(p,'w').write(s)
PY
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep 'guaranteed claim .* has no test'

# --- doctrines.json: the chain is resolved from the source, not copied from the registry
git -C "$T" checkout -q -- docs share
"$T/scripts/generate-site-data" >/dev/null
D="$T/site/data/generated/doctrines.json"
[ "$(jq '.doctrines | length' "$D")" = "$(grep -c '^  - id:' "$ROOT/share/doctrines.yaml")" ]
[ "$(jq '.counts.declared' "$D")" = "$(jq '.doctrines | length' "$D")" ]
[ "$(jq '.counts.blocking + .counts.advisory' "$D")" = "$(jq '.counts.declared' "$D")" ]
# every doctrine names the file that actually defines its validator
jq -r '.doctrines[] | [.validator_function, .defined_in] | @tsv' "$D" | while IFS="$(printf '\t')" read -r fn file; do
  grep -qE "^$fn\(\)" "$T/$file" || { echo "    $file does not define $fn"; exit 1; }
done
# every URL slug is derived from the id, and every class is one of two
jq -e '.doctrines | all(.slug == (.id | gsub("_"; "-")))' "$D" >/dev/null
jq -e '.doctrines | all(.class == "blocking" or .class == "advisory")' "$D" >/dev/null
# a doctrine naming a validator nobody wrote does not reach the site
sed 's/^    validator: scope$/    validator: not_implemented/' "$ROOT/share/doctrines.yaml" > "$T/share/doctrines.yaml"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep "names validator 'not_implemented'"
# nor does one declared for a command that never dispatches
sed 's/^    enforced_by: \[doctor\]$/    enforced_by: [update]/' "$ROOT/share/doctrines.yaml" > "$T/share/doctrines.yaml"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep "declared for 'update', which does not dispatch"
# nor a validator in lib/ that no doctrine declares
git -C "$T" checkout -q -- share
printf '\nmj_validate_unclaimed() { return 0; }\n' >> "$T/lib/check.sh"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep 'mj_validate_unclaimed, which no doctrine declares'
