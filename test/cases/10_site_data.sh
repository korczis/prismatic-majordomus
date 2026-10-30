# The site data normaliser: canonical files in, stable JSON out, and --check detects drift.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || skip "jq absent"
fixture_repo "$T" AGENTS.md docs site/data/marketing.toml site/content-src test/cases
git -C "$T" add -A >/dev/null; git -C "$T" commit -qm fixture

# --- Linux's per-argument cap, measured on every host (I3942)
# Linux refuses an exec whose any one argument, with its terminating NUL, exceeds
# MAX_ARG_STRLEN, 131072 bytes, so the longest argument is 131071 bytes; macOS has no such cap, so a payload that fails every CI run passes on the machine it is written
# on. scripts/ci/argv-limit-check refuses the spellings it knows, and three times a new one
# crossed the cap (the claims matrix, forge_paths, then mj_project_mermaid at 132474 bytes
# on train 4). Here the generator runs over every one of its real inputs with a jq on PATH
# that measures each argument instead: over the cap it refuses as Linux does, exit 126;
# over three quarters of it the argument is named, so a growing payload is seen before it
# breaks. Byte lengths are taken under LC_ALL=C by the shell itself, with no fork per call.
cap="$T/.argv-cap"; mkdir -p "$cap"
real_jq="$(command -v jq)"
cat > "$cap/jq" <<EOF
#!/usr/bin/env bash
LC_ALL=C
i=0
for a in "\$@"; do
  i=\$((i + 1)); n=\${#a}
  if [ "\$n" -gt 131071 ]; then
    echo "argv-cap: jq argument \$i is \$n bytes, over the 131071 Linux allows (MAX_ARG_STRLEN 131072, NUL included); pass it with --slurpfile/--rawfile or stdin" >&2
    exit 126
  fi
  if [ "\$n" -gt 98304 ]; then
    h="\${a:0:60}"; h="\${h//\$'\n'/ }"; h="\${h//\$'\t'/ }"
    printf '%s\t%s\n' "\$n" "\$h" >> "$cap/near"
  fi
done
exec "$real_jq" "\$@"
EOF
chmod +x "$cap/jq"
# the shim is held to what it claims: one byte over is refused, the cap itself is not
big="$(head -c 131072 /dev/zero | tr '\0' a)"
rc=0; PATH="$cap:$PATH" jq -n --arg x "$big" '1' >/dev/null 2>&1 || rc=$?
[ "$rc" = 126 ] || { echo "    the argv-cap shim let a 131072-byte argument through (exit $rc)"; exit 1; }
PATH="$cap:$PATH" jq -n --arg x "${big%a}" '1' >/dev/null \
  || { echo "    the argv-cap shim refused an argument at exactly the cap"; exit 1; }
unset big; : > "$cap/near"
path_before="$PATH"; PATH="$cap:$PATH"
expect_exit 0 "$T/scripts/generate-site-data"
PATH="$path_before"
while IFS="$(printf '\t')" read -r n head; do
  echo "    NEAR  a jq argument of $n bytes is within a quarter of Linux's cap: $head..."
done < "$cap/near"
for f in project profiles policy capabilities lifecycle docs diagrams source context; do [ -f "$T/site/data/generated/$f.json" ]; jq -e '.schema == 1' "$T/site/data/generated/$f.json" >/dev/null; done
# version comes from the one place it is authored (read by its one shell reader, never parsed
# here), profiles from the skeleton, claims from CLAIMS.yaml
[ "$(jq -r .version "$T/site/data/generated/project.json")" = "$("$ROOT/scripts/release-version")" ]
[ "$(jq '.profiles | length' "$T/site/data/generated/profiles.json")" = "$(ls "$ROOT"/share/skeleton/profiles/*.yaml | wc -l | tr -d ' ')" ]
# count the claims section only: docs/CLAIMS.yaml also declares its statuses as `- id:` entries
[ "$(jq '.claims | length' "$T/site/data/generated/capabilities.json")" = "$(awk '/^claims:/{c=1;next} c&&/^  - id: /{n++} END{print n+0}' "$ROOT/docs/CLAIMS.yaml")" ]
jq -e '.claims | all(.status == "guaranteed" and .test == null | not)' "$T/site/data/generated/capabilities.json" >/dev/null
jq -e '.principles | length >= 8' "$T/site/data/generated/lifecycle.json" >/dev/null
# the directory contracts: every directory of the layer is present with the verdict
# validation reached, and none of them owes a contract it does not have (ADR 0011)
jq -e '.counts.directories == (.directories | length) and .counts.documented > 0' "$T/site/data/generated/context.json" >/dev/null
jq -e '[.directories[] | select(.document == null and .requires_contract)] | length == 0' "$T/site/data/generated/context.json" >/dev/null
jq -e '[.directories[] | select(.contract != null and .contract.children_require_contract == false)] | length > 0' "$T/site/data/generated/context.json" >/dev/null
jq -e '.diagrams.lifecycle.mermaid | contains("no_match")' "$T/site/data/generated/diagrams.json" >/dev/null
# derived content exists, has front matter, and projected GitHub-native syntax
# CLI.md is projected under a slug of its own: /docs/cli/ is the per-command tree, so the
# specification cannot also live there (doc_slug, CLI_SPEC_SLUG in the generator)
expect_file "$T/site/content/docs/cli-specification.md"
expect_grep '^source = "docs/CLI.md"' "$T/site/content/docs/cli-specification.md"
# the table wrapper scrolls, so it is in the tab order: a scrolling region only a pointer can
# reach is what project.ui-conformance refuses (WCAG 2.1.1)
expect_grep '<div class="overflow-x-auto" tabindex="0">' "$T/site/content/docs/cli-specification.md"
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
# a claim whose status is not one the file declares under `statuses` is refused, by id and word
git -C "$T" checkout -q -- docs
python3 - "$T/docs/CLAIMS.yaml" <<'PY'
import sys; p=sys.argv[1]; s=open(p).read()
s=s.replace("    test: test/cases/01_init.sh\n    status: guaranteed","    test: test/cases/01_init.sh\n    status: proven",1); open(p,'w').write(s)
PY
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep 'carry a status docs/CLAIMS.yaml does not declare under statuses: .*[(]proven[)]'

# --- doctrines.json: the chain is resolved from the source, not copied from the rule objects
git -C "$T" checkout -q -- docs share
"$T/scripts/generate-site-data" >/dev/null
D="$T/site/data/generated/doctrines.json"
[ "$(jq '.doctrines | length' "$D")" = "$(grep -l '^  validator:' "$ROOT"/share/standard/majordomus/rules/*.md | wc -l | tr -d ' ')" ]
[ "$(jq '.counts.declared' "$D")" = "$(jq '.doctrines | length' "$D")" ]
[ "$(jq '.counts.blocking + .counts.advisory' "$D")" = "$(jq '.counts.declared' "$D")" ]
# every doctrine names the file that actually defines its validator
jq -r '.doctrines[] | [.validator_function, .defined_in] | @tsv' "$D" | while IFS="$(printf '\t')" read -r fn file; do
  grep -qE "^$fn\(\)" "$T/$file" || { echo "    $file does not define $fn"; exit 1; }
done
# every URL slug is derived from the id, and every class is one of two
jq -e '.doctrines | all(.slug == (.id | ltrimstr("majordomus.")))' "$D" >/dev/null
jq -e '.doctrines | all(.class == "blocking" or .class == "advisory")' "$D" >/dev/null
# a doctrine naming a validator nobody wrote does not reach the site
sed 's/^  validator: scope$/  validator: not_implemented/' "$ROOT/share/standard/majordomus/rules/scope-integrity.v1.md" > "$T/share/standard/majordomus/rules/scope-integrity.v1.md"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep "names validator 'not_implemented'"
# nor does one declared for a command that never dispatches
git -C "$T" checkout -q -- share
sed 's/^  enforced_by: \[doctor\]$/  enforced_by: [update]/' "$ROOT/share/standard/majordomus/rules/enforcement-wiring.v1.md" > "$T/share/standard/majordomus/rules/enforcement-wiring.v1.md"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep "declared for 'update', which does not dispatch"
# nor a validator in lib/ that no doctrine declares
git -C "$T" checkout -q -- share
printf '\nmj_validate_unclaimed() { return 0; }\n' >> "$T/lib/check.sh"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep 'mj_validate_unclaimed, which no doctrine declares'

# ---------------------------------------------------------------- nothing reader-relative
# `session list` labels every record by its divergence from *this* worktree and branch, so
# the same record labels itself `diverged` here and `different_context` on a CI checkout.
# A published projection may only carry what the record says about itself: baked in, the
# label makes the dataset a fact about the machine that generated it, and --check then
# fails on every other one. This is the defect, from the artifact the repository ships.
S="$ROOT/site/data/generated/sessions.json"
if [ "$(jq -r '.count' "$S")" -gt 0 ]; then
  "$ROOT/bin/majordomus" --repo "$ROOT" session list --all --json \
    | jq -e '.sessions | length > 0 and all(has("label"))' >/dev/null \
    || { echo "    session list no longer labels its records; this assertion needs rewriting"; exit 1; }
  jq -e '.sessions | all(has("label") | not)' "$S" >/dev/null \
    || { echo "    the published sessions dataset carries the reader-relative label"; exit 1; }
fi
