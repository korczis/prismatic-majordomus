# The critical regeneration test: change one canonical value; the derived data and content change with it.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
fixture_repo "$T" AGENTS.md docs site/data/marketing.toml site/content-src test/cases
git -C "$T" add -A >/dev/null; git -C "$T" commit -qm fixture
"$T/scripts/generate-site-data" >/dev/null; cp -R "$T/site/data/generated" "$T/before"
# 1. version — matched by shape, not by the value it happens to hold today. Naming the
# current version here meant the substitution silently stopped matching the day the version
# moved, and the assertion below went on passing against a value nothing had changed.
sed -i.bak -E 's/^MJ_VERSION=".*"/MJ_VERSION="9.9.9"/' "$T/bin/majordomus"; rm -f "$T/bin/majordomus.bak"
grep -q '^MJ_VERSION="9.9.9"$' "$T/bin/majordomus" || { echo "    the version mutation matched nothing; this case would prove nothing"; exit 1; }
# 2. a profile description and effort
sed -i.bak 's/^description: .*/description: CHANGED DESCRIPTION/; s/^effort: low$/effort: max/' "$T/share/skeleton/profiles/routine.yaml"; rm -f "$T/share/skeleton/profiles/routine.yaml.bak"
# 3. a principle label (the title of the rule tagged principle in the standard package).
# The package is held to its manifest by a hash, and the vendored copy in a repository is
# compared against it — so editing a rule without re-stamping the package is not "a changed
# input", it is a corrupt distribution. `generate-site-data` now runs the repository's use
# cases, and every scenario that validates the layer refused it: eight of them, for a reason
# none of them named. Re-stamp it the way the package's maintainer would.
sed -i.bak 's/^title: Sessions are workers, not memory$/title: Sessions are CHANGED PRINCIPLE/' "$T/share/standard/majordomus/rules/principle-01-sessions-are-workers.v1.md"; rm -f "$T/share/standard/majordomus/rules/principle-01-sessions-are-workers.v1.md.bak"
grep -q '^title: Sessions are CHANGED PRINCIPLE$' "$T/share/standard/majordomus/rules/principle-01-sessions-are-workers.v1.md" \
  || { echo "    the principle mutation matched nothing"; exit 1; }
expect_exit 0 "$T/scripts/rules-package" write
expect_exit 0 "$T/scripts/rules-package" check
# 4. a policy value
# A different value, and one the bootstraps still fit inside: `generate-site-data` now runs
# the repository's use-case scenarios, and `keep-the-bootstrap-thin-and-within-budget` holds
# the generated AGENTS.md and CLAUDE.md to this budget. A budget below their real length is
# not a changed input — it is a false statement, and the scenario is right to refuse it.
sed -i.bak -E 's/always_loaded_budget_lines: [0-9]+/always_loaded_budget_lines: 120/' "$T/share/skeleton/policy.yaml"; rm -f "$T/share/skeleton/policy.yaml.bak"
grep -q 'always_loaded_budget_lines: 120' "$T/share/skeleton/policy.yaml" || { echo "    the budget mutation matched nothing"; exit 1; }
# 5. a claim status
python3 - "$T/docs/CLAIMS.yaml" <<'PY'
import sys; p=sys.argv[1]; s=open(p).read(); s=s.replace("  - id: init-refuses\n    claim: Installing into a repository that already has an installation is refused","  - id: init-refuses\n    claim: CHANGED CLAIM TEXT",1); open(p,'w').write(s)
PY
# 6. a docs heading
sed -i.bak 's/^# Concepts$/# Concepts CHANGED/' "$T/docs/CONCEPTS.md"; rm -f "$T/docs/CONCEPTS.md.bak"
expect_exit 0 "$T/scripts/generate-site-data"
G="$T/site/data/generated"
[ "$(jq -r .version "$G/project.json")" = "9.9.9" ] \
  || { echo "    the version did not reach the derived data"; exit 1; }
[ "$(jq -r '.profiles[] | select(.slug=="routine") | .description' "$G/profiles.json")" = "CHANGED DESCRIPTION" ]
[ "$(jq -r '.profiles[] | select(.slug=="routine") | .effort' "$G/profiles.json")" = "max" ]
jq -e '.principles | index("Sessions are CHANGED PRINCIPLE")' "$G/lifecycle.json" >/dev/null
[ "$(jq -r .context.always_loaded_budget_lines "$G/policy.json")" = 120 ] \
  || { echo "    the policy budget did not reach the derived data"; exit 1; }
[ "$(jq -r '.claims[] | select(.id=="init-refuses") | .claim' "$G/capabilities.json")" = "CHANGED CLAIM TEXT" ]
expect_grep '^title = "Concepts CHANGED"' "$T/site/content/docs/concepts.md"
# the input hash moved, and the previous data is now reported stale
[ "$(jq -r .source_hash "$G/source.json")" != "$(jq -r .source_hash "$T/before/source.json")" ]
rm -rf "$G"; cp -R "$T/before" "$G"
expect_exit 10 "$T/scripts/generate-site-data" --check
expect_grep 'STALE'
