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

# restored input regenerates cleanly and --check agrees
sed -i.bak 's/^## The problems$/## The problem/' "$T/README.md"; rm -f "$T/README.md.bak"
expect_exit 0 "$T/scripts/generate-site-data"
expect_exit 0 "$T/scripts/generate-site-data" --check
[ -z "$(find "$T" -maxdepth 1 -name '.mj-stage.*' 2>/dev/null)" ]
