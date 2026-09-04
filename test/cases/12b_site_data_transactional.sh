# Generation is transactional: a failure part way through must leave the previous generated
# artifacts intact, not truncated. This reproduces the class of damage where an emitter's
# validation exits inside a `{ ... } | jq . > file` pipeline: the subshell dies but the
# redirect has already truncated the target.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
fixture_repo "$T" AGENTS.md docs
mkdir -p "$T/site/data" "$T/test"; cp "$ROOT/site/data/marketing.toml" "$ROOT/site/data/nav.toml" "$T/site/data/"; cp -R "$ROOT/site/content-src" "$T/site/"; cp -R "$ROOT/test/cases" "$T/test/"
git -C "$T" add -A >/dev/null; git -C "$T" commit -qm fixture

# a good generation first
expect_exit 0 "$T/scripts/generate-site-data"
before_files="$(ls "$T/site/data/generated" | sort)"
before_project="$(cat "$T/site/data/generated/project.json")"
before_claims="$(cat "$T/docs/SITE_CLAIMS.md")"
before_content="$(ls "$T/site/content/guarantees" | wc -l | tr -d ' ')"
[ "$before_content" -gt 0 ]

# now break a canonical input part way through: a claim without its detail document
rm "$T/docs/claims/typed-outcome.md"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep 'has no docs/claims/typed-outcome.md'

# nothing published: every previous artifact is byte-identical and still complete
[ "$(ls "$T/site/data/generated" | sort)" = "$before_files" ]
[ "$(cat "$T/site/data/generated/project.json")" = "$before_project" ]
[ "$(cat "$T/docs/SITE_CLAIMS.md")" = "$before_claims" ]
[ "$(ls "$T/site/content/guarantees" | wc -l | tr -d ' ')" = "$before_content" ]
# and no leftovers: neither half-published artifacts nor the staging directory itself
[ -z "$(find "$T/site" "$T/docs" -name '*.mj-old' -o -name '*.mj-tmp' 2>/dev/null)" ]
[ -z "$(find "$T" -maxdepth 1 -name '.mj-stage.*' 2>/dev/null)" ]

# a second failure class: a README heading the homepage renders is renamed away
git -C "$T" checkout -q -- docs/claims 2>/dev/null || cp -R "$ROOT/docs/claims" "$T/docs/"
sed -i.bak 's/^## The problem$/## The problems/' "$T/README.md"; rm -f "$T/README.md.bak"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep "no heading '## The problem'"
[ "$(cat "$T/site/data/generated/project.json")" = "$before_project" ]

# a third failure class: the README table and docs/RESPONSIBILITIES.yaml are joined by row
# title, and a join that misses is silent. Each direction is broken separately, because a
# check that only looks one way passes while half the correspondence is already gone.
sed -i.bak 's/^## The problems$/## The problem/' "$T/README.md"; rm -f "$T/README.md.bak"
expect_exit 0 "$T/scripts/generate-site-data"

# an entry pointing at a README row that no longer exists
first_key="$(awk '/^responsibilities:/{c=1} c&&/^    readme_key: /{print $2; exit}' "$T/docs/RESPONSIBILITIES.yaml")"
[ -n "$first_key" ] || { echo "    could not read a readme_key from the fixture"; exit 1; }
sed -i.bak "s/^    readme_key: $first_key\$/    readme_key: Nonexistent/" "$T/docs/RESPONSIBILITIES.yaml"; rm -f "$T/docs/RESPONSIBILITIES.yaml.bak"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep "names README row 'Nonexistent'"
[ "$(cat "$T/site/data/generated/project.json")" = "$before_project" ]
sed -i.bak "s/^    readme_key: Nonexistent\$/    readme_key: $first_key/" "$T/docs/RESPONSIBILITIES.yaml"; rm -f "$T/docs/RESPONSIBILITIES.yaml.bak"

# and the far side: a README row that no entry claims. A row is added rather than renamed,
# because renaming one breaks both directions at once and the entry-side check would refuse
# first, leaving this direction unproven.
awk 'BEGIN{d=0} {print} !d && /^\| \*\*/{print "| **Telemetry** | a row no responsibility claims |"; d=1}' \
  "$T/README.md" > "$T/README.next" && mv "$T/README.next" "$T/README.md"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep "README row 'Telemetry' has no entry"
grep -v '^| \*\*Telemetry\*\* |' "$T/README.md" > "$T/README.next" && mv "$T/README.next" "$T/README.md"

# a path an entry names but the repository does not have. The declaration is repointed rather
# than the file moved: every implementation the entries name is also read by an earlier stage
# of the generator, so moving one would refuse for that reason instead of this one.
impl="$(awk '/^responsibilities:/{c=1} c&&/^    implementation: /{print $2; exit}' "$T/docs/RESPONSIBILITIES.yaml")"
[ -n "$impl" ] || { echo "    could not read an implementation path from the fixture"; exit 1; }
sed -i.bak "s|^    implementation: $impl\$|    implementation: lib/does-not-exist.sh|" "$T/docs/RESPONSIBILITIES.yaml"; rm -f "$T/docs/RESPONSIBILITIES.yaml.bak"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep "names missing path lib/does-not-exist.sh"
sed -i.bak "s|^    implementation: lib/does-not-exist.sh\$|    implementation: $impl|" "$T/docs/RESPONSIBILITIES.yaml"; rm -f "$T/docs/RESPONSIBILITIES.yaml.bak"

# an anchor that is not a heading in the document it names. Again the declaration moves, not
# the heading: renaming a `## majordomus <cmd>` section trips the command-documentation check
# first, which is a different refusal about a different fact.
anchor="$(awk '/^responsibilities:/{c=1} c&&/^    cli_anchor: /{sub(/^    cli_anchor: /,"");print;exit}' "$T/docs/RESPONSIBILITIES.yaml")"
[ -n "$anchor" ] || { echo "    could not read a cli_anchor from the fixture"; exit 1; }
sed -i.bak "s|^    cli_anchor: $anchor\$|    cli_anchor: majordomus nowhere|" "$T/docs/RESPONSIBILITIES.yaml"; rm -f "$T/docs/RESPONSIBILITIES.yaml.bak"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep "cli_anchor 'majordomus nowhere'"
sed -i.bak "s|^    cli_anchor: majordomus nowhere\$|    cli_anchor: $anchor|" "$T/docs/RESPONSIBILITIES.yaml"; rm -f "$T/docs/RESPONSIBILITIES.yaml.bak"

# every input restored: generation succeeds again and --check agrees
expect_exit 0 "$T/scripts/generate-site-data"
expect_exit 0 "$T/scripts/generate-site-data" --check
[ -z "$(find "$T" -maxdepth 1 -name '.mj-stage.*' 2>/dev/null)" ]
