# The critical regeneration test: change one canonical value; the derived data and content change with it.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$ROOT/scripts" "$ROOT/docs" "$ROOT/README.md" "$ROOT/LICENSE" "$ROOT/AGENTS.md" "$T/"; mkdir -p "$T/site/data" "$T/test"; cp "$ROOT/site/data/marketing.toml" "$T/site/data/"; cp -R "$ROOT/test/cases" "$T/test/"
git -C "$T" add -A >/dev/null; git -C "$T" commit -qm fixture
"$T/scripts/generate-site-data" >/dev/null; cp -R "$T/site/data/generated" "$T/before"
# 1. version
sed -i.bak 's/^MJ_VERSION="0.1.0"/MJ_VERSION="9.9.9"/' "$T/bin/majordomus"; rm -f "$T/bin/majordomus.bak"
# 2. a profile description and effort
sed -i.bak 's/^description: .*/description: CHANGED DESCRIPTION/; s/^effort: low$/effort: max/' "$T/share/skeleton/profiles/routine.yaml"; rm -f "$T/share/skeleton/profiles/routine.yaml.bak"
# 3. a principle label
sed -i.bak 's/Sessions are workers, not memory\./Sessions are CHANGED PRINCIPLE./' "$T/share/skeleton/providers/body.md"; rm -f "$T/share/skeleton/providers/body.md.bak"
# 4. a policy value
sed -i.bak 's/always_loaded_budget_lines: 150/always_loaded_budget_lines: 42/' "$T/share/skeleton/policy.yaml"; rm -f "$T/share/skeleton/policy.yaml.bak"
# 5. a claim status
python3 - "$T/docs/CLAIMS.yaml" <<'PY'
import sys; p=sys.argv[1]; s=open(p).read(); s=s.replace("  - id: init-refuses\n    claim: Installing into a repository that already has an installation is refused","  - id: init-refuses\n    claim: CHANGED CLAIM TEXT",1); open(p,'w').write(s)
PY
# 6. a docs heading
sed -i.bak 's/^# Concepts$/# Concepts CHANGED/' "$T/docs/CONCEPTS.md"; rm -f "$T/docs/CONCEPTS.md.bak"
expect_exit 0 "$T/scripts/generate-site-data"
G="$T/site/data/generated"
[ "$(jq -r .version "$G/project.json")" = "9.9.9" ]
[ "$(jq -r '.profiles[] | select(.slug=="routine") | .description' "$G/profiles.json")" = "CHANGED DESCRIPTION" ]
[ "$(jq -r '.profiles[] | select(.slug=="routine") | .effort' "$G/profiles.json")" = "max" ]
jq -e '.principles | index("Sessions are CHANGED PRINCIPLE.")' "$G/lifecycle.json" >/dev/null
[ "$(jq -r .context.always_loaded_budget_lines "$G/policy.json")" = 42 ]
[ "$(jq -r '.claims[] | select(.id=="init-refuses") | .claim' "$G/capabilities.json")" = "CHANGED CLAIM TEXT" ]
expect_grep '^title = "Concepts CHANGED"' "$T/site/content/docs/concepts.md"
# the input hash moved, and the previous data is now reported stale
[ "$(jq -r .source_hash "$G/source.json")" != "$(jq -r .source_hash "$T/before/source.json")" ]
rm -rf "$G"; cp -R "$T/before" "$G"
expect_exit 10 "$T/scripts/generate-site-data" --check
expect_grep 'STALE'
