# The critical regeneration test: change one canonical value; the derived data and content change with it.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
fixture_repo "$T" AGENTS.md docs site/data/marketing.toml site/content-src test/cases
git -C "$T" add -A >/dev/null; git -C "$T" commit -qm fixture
"$T/scripts/generate-site-data" >/dev/null; cp -R "$T/site/data/generated" "$T/before"
# 1. version — whatever it is now, not a version written down here: pinning 0.1.0 meant the
#    sed stopped matching at the first release and the mutation silently did nothing, so the
#    assertion below compared the real version with 9.9.9 and failed on every run since.
sed -i.bak -E 's/^MJ_VERSION="[^"]*"/MJ_VERSION="9.9.9"/' "$T/bin/majordomus"; rm -f "$T/bin/majordomus.bak"
grep -q '^MJ_VERSION="9.9.9"$' "$T/bin/majordomus" || { echo "    the version mutation did not apply"; exit 1; }
# 2. a profile description and effort
sed -i.bak 's/^description: .*/description: CHANGED DESCRIPTION/; s/^effort: low$/effort: max/' "$T/share/skeleton/profiles/routine.yaml"; rm -f "$T/share/skeleton/profiles/routine.yaml.bak"
# 3. a principle label (the title of the rule tagged principle in the standard package).
#    The package is hash-pinned: its manifest carries the hash of every rule file and a hand
#    edit is refused until the package is rewritten. Editing the rule and stopping there made
#    every scenario that reads the rules fail, and the generator refused with eight of them
#    named — which is the package integrity doctrine working, not this mutation failing. So
#    re-pin, the way the maintainer of the package would.
sed -i.bak 's/^title: Sessions are workers, not memory$/title: Sessions are CHANGED PRINCIPLE/' "$T/share/standard/majordomus/rules/principle-01-sessions-are-workers.v1.md"; rm -f "$T/share/standard/majordomus/rules/principle-01-sessions-are-workers.v1.md.bak"
( cd "$T" && ./scripts/rules-package write >/dev/null ) || { echo "    the rule package could not be re-pinned after the edit"; exit 1; }
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
jq -e '.principles | index("Sessions are CHANGED PRINCIPLE")' "$G/lifecycle.json" >/dev/null
[ "$(jq -r .context.always_loaded_budget_lines "$G/policy.json")" = 42 ]
[ "$(jq -r '.claims[] | select(.id=="init-refuses") | .claim' "$G/capabilities.json")" = "CHANGED CLAIM TEXT" ]
expect_grep '^title = "Concepts CHANGED"' "$T/site/content/docs/concepts.md"
# the input hash moved, and the previous data is now reported stale
[ "$(jq -r .source_hash "$G/source.json")" != "$(jq -r .source_hash "$T/before/source.json")" ]
rm -rf "$G"; cp -R "$T/before" "$G"
expect_exit 10 "$T/scripts/generate-site-data" --check
expect_grep 'STALE'
