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
# 3. a principle label (the title of the rule tagged principle in the standard package)
# The package records a hash per rule and refuses a rule that does not match it, so editing
# the file alone is not an edit to the package — it is a corrupt package, and every scenario
# that runs `init`, `doctor` or `watch` then fails for that reason instead of the one under
# test. The fixture updates the manifest the way an explicit package update would.
PRINCIPLE_RULE="share/standard/majordomus/rules/principle-01-sessions-are-workers.v1.md"
sed -i.bak 's/^title: Sessions are workers, not memory$/title: Sessions are CHANGED PRINCIPLE/' "$T/$PRINCIPLE_RULE"; rm -f "$T/$PRINCIPLE_RULE.bak"
python3 - "$T/share/standard/majordomus/manifest.yaml" "rules/principle-01-sessions-are-workers.v1.md" \
         "$(shasum -a 256 "$T/$PRINCIPLE_RULE" | cut -d' ' -f1)" <<'PY'
import re, sys
manifest, rule_file, digest = sys.argv[1], sys.argv[2], sys.argv[3]
text = open(manifest).read()
pattern = re.compile(r'(file:\s*' + re.escape(rule_file) + r'\n\s*sha256:\s*)[0-9a-f]{64}')
text, n = pattern.subn(lambda m: m.group(1) + digest, text)
assert n == 1, f'{n} manifest entries for {rule_file}'
open(manifest, 'w').write(text)
PY
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
# Each derived value is named when it disagrees. A bare `[ ... ]` under `set -e` ends the
# case with no output at all, which is how "the version mutation stopped matching" read as
# an unexplained exit for as long as it did.
derived() {  # <what> <want> <got>
  [ "$3" = "$2" ] || { echo "    $1: derived $3, and the canonical value was changed to $2"; exit 1; }
}
derived "project.json .version" "9.9.9" "$(jq -r .version "$G/project.json")"
derived "profiles.json routine.description" "CHANGED DESCRIPTION" \
        "$(jq -r '.profiles[] | select(.slug=="routine") | .description' "$G/profiles.json")"
derived "profiles.json routine.effort" "max" \
        "$(jq -r '.profiles[] | select(.slug=="routine") | .effort' "$G/profiles.json")"
jq -e '.principles | index("Sessions are CHANGED PRINCIPLE")' "$G/lifecycle.json" >/dev/null \
  || { echo "    lifecycle.json principles does not carry the changed principle title"; exit 1; }
derived "policy.json context.always_loaded_budget_lines" "42" \
        "$(jq -r .context.always_loaded_budget_lines "$G/policy.json")"
derived "capabilities.json claim init-refuses" "CHANGED CLAIM TEXT" \
        "$(jq -r '.claims[] | select(.id=="init-refuses") | .claim' "$G/capabilities.json")"
expect_grep '^title = "Concepts CHANGED"' "$T/site/content/docs/concepts.md"
# the input hash moved, and the previous data is now reported stale
[ "$(jq -r .source_hash "$G/source.json")" != "$(jq -r .source_hash "$T/before/source.json")" ]
rm -rf "$G"; cp -R "$T/before" "$G"
expect_exit 10 "$T/scripts/generate-site-data" --check
expect_grep 'STALE'
