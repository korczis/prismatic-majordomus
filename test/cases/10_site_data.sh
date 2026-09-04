# The site data normaliser: canonical files in, stable JSON out, and --check detects drift.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$ROOT/scripts" "$ROOT/docs" "$ROOT/README.md" "$ROOT/LICENSE" "$ROOT/AGENTS.md" "$T/"; mkdir -p "$T/site/data" "$T/test"; cp -R "$ROOT/site/data/marketing.toml" "$T/site/data/"; cp -R "$ROOT/test/cases" "$T/test/"
git -C "$T" add -A >/dev/null; git -C "$T" commit -qm fixture
expect_exit 0 "$T/scripts/generate-site-data"
for f in project profiles policy capabilities lifecycle docs diagrams source; do [ -f "$T/site/data/generated/$f.json" ]; jq -e '.schema == 1' "$T/site/data/generated/$f.json" >/dev/null; done
# version comes from the CLI, profiles from the skeleton, claims from CLAIMS.yaml
[ "$(jq -r .version "$T/site/data/generated/project.json")" = "$(sed -n 's/^MJ_VERSION="\(.*\)"/\1/p' "$ROOT/bin/majordomus")" ]
[ "$(jq '.profiles | length' "$T/site/data/generated/profiles.json")" = "$(ls "$ROOT"/share/skeleton/profiles/*.yaml | wc -l | tr -d ' ')" ]
[ "$(jq '.claims | length' "$T/site/data/generated/capabilities.json")" = "$(grep -c '^  - id:' "$ROOT/docs/CLAIMS.yaml")" ]
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
