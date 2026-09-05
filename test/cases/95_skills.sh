# majordomus-covers: skills
# majordomus-negative: skills doctor watch
# Skills are data, not registrations. A skill is one directory under the layer's skills
# section holding SKILL.md; the source class `skill` in the knowledge sources is the whole
# registration, shared by the shell tool, the Rust executable and the site generator. This
# case adds a skill and watches it appear in the catalogue, the command, doctor, the Rust
# index (MCP resource, majordomus_list, majordomus_get) and the site's data and pages; then
# breaks one and watches every surface refuse it the same way; then renames and removes it
# and watches every derived trace go with it. The Rust and site halves skip themselves
# when their toolchain is absent, as the other cases do.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj95.XXXXXX")"; trap 'rm -rf "$S"' EXIT
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add -A >/dev/null && git commit -qm base
# the two enforcements the policy declares, wired, so that doctor's verdict is about skills
mkdir -p .githooks
printf '#!/bin/sh\n%s doctor || exit $?\n' "$MJ" > .githooks/pre-commit
printf '#!/bin/sh\n%s finish --check || exit $?\n' "$MJ" > .githooks/pre-push
chmod +x .githooks/pre-commit .githooks/pre-push; git config core.hooksPath .githooks

skill() { # id [status] [related-csv] — a valid skill with one example, written and tracked
  local id="$1" st="${2:-active}" rel="${3:-}"
  mkdir -p ".ai/repo/skills/$id/examples"
  {
    printf -- '---\nschema: skill/v1\nid: %s\nversion: 1\ntitle: Skill %s\ndescription: The %s procedure.\nstatus: %s\ntags: [fixture]\n' "$id" "$id" "$id" "$st"
    [ -z "$rel" ] || printf 'related: [%s]\n' "$rel"
    printf 'inputs:\n  - a checkout\noutputs:\n  - a report\n---\n# Purpose\n\nWhy %s exists.\n\n# Procedure\n\n1. Do the %s thing.\n\n# Output\n\nWhat %s leaves behind.\n' "$id" "$id" "$id"
  } > ".ai/repo/skills/$id/SKILL.md"
  printf '# Example for %s\n\n```text\nApply the %s skill.\n```\n' "$id" "$id" > ".ai/repo/skills/$id/examples/basic.md"
  git add ".ai/repo/skills/$id" >/dev/null
}

# ---------------------------------------------------------------- usage and the empty case
expect_exit 2 "$MJ" skills
expect_grep 'usage: majordomus skills list'
expect_exit 2 "$MJ" skills nonsense
expect_exit 2 "$MJ" skills show
expect_grep 'a skill id is required'
expect_exit 2 "$MJ" skills show ../../etc/passwd
expect_grep 'is not a skill id'
expect_exit 0 "$MJ" skills list
expect_grep 'no skills under \.ai/repo/skills/'
# nothing examined is not a pass: the empty catalogue is a WARN with the counts at zero
expect_exit 0 "$MJ" skills check
expect_grep '^WARN skill '
expect_no_grep '^OK   skill '
expect_grep 'skills: 0 discovered, 0 valid'

# ---------------------------------------------------------------- one file makes a skill
skill review
# untracked is not a source: discovery reads the index, never the working tree
git rm -rq --cached .ai/repo/skills/review >/dev/null
expect_exit 0 "$MJ" skills list
expect_no_grep 'review'
git add .ai/repo/skills/review >/dev/null
expect_exit 0 "$MJ" skills list
expect_grep '^review +active +v1 +The review procedure\.$'
expect_exit 0 "$MJ" skills check
expect_grep '^OK   skill +1 skill\(s\)'
expect_grep '^OK   skill +1 reference\(s\)'
expect_grep 'skills: 1 discovered, 1 valid; examples: 1; references: 1 checked; failures: 0'
expect_exit 0 "$MJ" skills show review
expect_grep '^\.ai/repo/skills/review/SKILL\.md$'
expect_grep '^# Procedure$'
expect_exit 12 "$MJ" skills show nosuch
expect_grep "no skill 'nosuch'"
expect_grep 'skills list'
if command -v jq >/dev/null; then
  "$MJ" skills list --json > "$S/list.json"
  jq -e '.count == 1 and .skills[0].id == "review" and .skills[0].uri == "majordomus://skill/review" and .skills[0].path == ".ai/repo/skills/review/SKILL.md" and .skills[0].valid == true and (.skills[0].examples | length) == 1 and (.skills[0].inputs | length) == 1 and (.skills[0].sha256 | length) == 64' "$S/list.json" >/dev/null \
    || { echo "    skills list --json does not describe the skill from its file"; cat "$S/list.json"; exit 1; }
  "$MJ" skills show review --json > "$S/show.json"
  jq -e '.id == "review" and (.body | contains("# Procedure"))' "$S/show.json" >/dev/null || { echo "    skills show --json carries no body"; exit 1; }
  "$MJ" skills check --json > "$S/check.json"
  jq -se '.[-1].summary == {skills: 1, valid: 1, examples: 1, references: 1, failures: 0}' "$S/check.json" >/dev/null || { echo "    skills check --json summary is wrong:"; cat "$S/check.json"; exit 1; }
fi
# doctor runs the same examination through the doctrine, wired and dispatched
expect_exit 0 "$MJ" doctor
expect_grep '^OK   skill +1 skill\(s\), 1 example\(s\)'
expect_exit 0 "$MJ" doctrine show majordomus.skill-integrity
expect_grep '^validator +mj_validate_skills$'
expect_grep '^wired +yes'

# ---------------------------------------------------------------- deterministic, and ordered by path not by creation
skill zeta; skill alpha "draft" "review"
"$MJ" skills list > "$S/l1.txt"; "$MJ" skills list > "$S/l2.txt"
cmp -s "$S/l1.txt" "$S/l2.txt" || { echo "    two listings of one tree differ"; exit 1; }
[ "$(awk '{print $1}' "$S/l1.txt" | paste -sd, -)" = "alpha,review,zeta" ] || { echo "    listing is not in path order: $(awk '{print $1}' "$S/l1.txt" | paste -sd, -)"; exit 1; }
expect_exit 0 "$MJ" skills check
expect_grep 'skills: 3 discovered, 3 valid; examples: 3; references: 4 checked'

# ---------------------------------------------------------------- every way a skill can be wrong, each named
mkdir -p .ai/repo/skills/broken
printf -- '---\nschema: skill/v1\nid: other\nversion: 0\ntitle: X\ndescription: y\nstatus: shiny\nrelated: [nosuch]\nowner: me\n---\n# Purpose\n\ntext\n\n# Output\n' > .ai/repo/skills/broken/SKILL.md
git add .ai/repo/skills/broken >/dev/null
expect_exit 10 "$MJ" skills check
expect_grep '^FAIL skill +\.ai/repo/skills/broken/SKILL\.md — .*unknown front-matter key\(s\): owner'
expect_grep 'id "other" does not match the directory name "broken"'
expect_grep 'version must be an integer of at least 1'
expect_grep 'status must be one of draft, active, deprecated'
expect_grep 'body lacks a non-empty section for: Procedure Output'
expect_grep "related skill 'nosuch' does not exist"
expect_grep 'skills: 4 discovered, 3 valid'
expect_grep 'failures: 2'
# the listing still shows it, marked by its reason, rather than hiding it
expect_exit 0 "$MJ" skills list
expect_grep '^other +shiny '
expect_exit 10 "$MJ" doctor
expect_grep '^FAIL skill +\.ai/repo/skills/broken/SKILL\.md'
expect_exit 11 "$MJ" watch
expect_grep '^DRIFT skill +\.ai/repo/skills/broken/SKILL\.md'
# no front matter at all, and a duplicate id, are each their own finding
printf '# Purpose\n\nno front matter\n' > .ai/repo/skills/broken/SKILL.md
expect_exit 10 "$MJ" skills check
expect_grep 'broken/SKILL\.md — no front matter'
git rm -rqf .ai/repo/skills/broken >/dev/null
skill twin; sed -i.bak 's/^id: twin$/id: review/' .ai/repo/skills/twin/SKILL.md && rm -f .ai/repo/skills/twin/SKILL.md.bak && git add .ai/repo/skills/twin >/dev/null
expect_exit 10 "$MJ" skills check
expect_grep "duplicate skill id 'review'"
git rm -rqf .ai/repo/skills/twin >/dev/null
# an example under a directory with no skill, and one without a heading
mkdir -p .ai/repo/skills/orphan/examples && printf 'no heading here\n' > .ai/repo/skills/orphan/examples/x.md && git add .ai/repo/skills/orphan >/dev/null
expect_exit 10 "$MJ" skills check
expect_grep 'orphan/examples/x\.md — example under a directory that holds no valid SKILL\.md'
expect_grep 'orphan/examples/x\.md — example has no level-one heading'
git rm -rqf .ai/repo/skills/orphan >/dev/null
expect_exit 0 "$MJ" skills check

# ---------------------------------------------------------------- the Rust executable reads the same declaration
RB="$(rust_bin)" || rc=$?
if [ -n "${RB:-}" ]; then
  MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
  git commit -qm skills >/dev/null
  # the allow-list the shell validator reads is the schema's projection
  expect_exit 0 "$RB" generate allow --check
  expect_exit 0 "$RB" mcp --inspect
  expect_grep '^resource    majordomus://skill/review$'
  expect_grep '^resource    majordomus://skill/alpha$'
  expect_grep '^resource    majordomus://document/\.ai/repo/skills/review/examples/basic\.md$'
  "$RB" capabilities list --kind resource --format json 2>/dev/null > "$S/caps.json"
  jq -e '[.capabilities[].id] | index("skill.review") != null and index("skill.zeta") != null' "$S/caps.json" >/dev/null \
    || { echo "    the Rust registry does not carry the skills as resources"; exit 1; }
  # a real session: list by kind, read the file, get it through the tool
  req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
  {
    req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case95","version":"0"}}'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    req 2 tools/call '{"name":"majordomus_list","arguments":{"kind":"skill"}}'
    req 3 resources/read '{"uri":"majordomus://skill/review"}'
    req 4 tools/call '{"name":"majordomus_get","arguments":{"uri":"majordomus://skill/alpha"}}'
  } > "$S/session.in"
  "$RB" mcp --standalone < "$S/session.in" > "$S/session.out" 2>"$S/session.err" || { echo "    the MCP session failed"; cat "$S/session.err"; exit 1; }
  sed -n 2p "$S/session.out" | jq -e '.result.structuredContent | .count == 3 and ([.objects[].identity] | sort) == ["alpha","review","zeta"]' >/dev/null \
    || { echo "    majordomus_list kind=skill does not list the three skills"; sed -n 2p "$S/session.out"; exit 1; }
  sed -n 3p "$S/session.out" | jq -e '.result.contents[0].text | contains("# Procedure") and contains("id: review")' >/dev/null \
    || { echo "    resources/read of the skill does not return the file"; sed -n 3p "$S/session.out"; exit 1; }
  sed -n 4p "$S/session.out" | jq -e '.result.structuredContent | .metadata.related == ["review"] and .metadata.status == "draft" and .provenance.path == ".ai/repo/skills/alpha/SKILL.md" and (.content | contains("# Procedure"))' >/dev/null \
    || { echo "    majordomus_get does not carry the metadata and provenance of the skill"; sed -n 4p "$S/session.out"; exit 1; }
  # the executable refuses what the shell refuses: an unknown key is a diagnostic naming the file
  mkdir -p .ai/repo/skills/bad && printf -- '---\nschema: skill/v1\nid: bad\nversion: 1\ntitle: B\ndescription: b\nstatus: active\nowner: me\n---\n# Purpose\n\nx\n\n# Procedure\n\nx\n\n# Output\n\nx\n' > .ai/repo/skills/bad/SKILL.md
  git add .ai/repo/skills/bad >/dev/null
  expect_exit 10 "$RB" mcp --inspect
  expect_grep '^FAIL unknown_key +\.ai/repo/skills/bad/SKILL\.md'
  expect_exit 10 "$MJ" skills check
  expect_grep 'bad/SKILL\.md — unknown front-matter key\(s\): owner'
  git rm -rqf .ai/repo/skills/bad >/dev/null
else
  [ "${rc:-0}" = 3 ] && echo "    (no cargo and no MAJORDOMUS_BIN: the Rust half is skipped)" || exit 1
fi

# ---------------------------------------------------------------- rename and removal leave nothing behind
git mv .ai/repo/skills/zeta .ai/repo/skills/omega
sed -i.bak 's/^id: zeta$/id: omega/' .ai/repo/skills/omega/SKILL.md && rm -f .ai/repo/skills/omega/SKILL.md.bak && git add .ai/repo/skills/omega >/dev/null
expect_exit 0 "$MJ" skills list
expect_grep '^omega '
expect_no_grep '^zeta '
git rm -rqf .ai/repo/skills/omega >/dev/null
expect_exit 0 "$MJ" skills check
expect_grep 'skills: 2 discovered, 2 valid'
expect_exit 12 "$MJ" skills show omega

# ---------------------------------------------------------------- the site is a projection of the same catalogue
command -v jq >/dev/null || { echo "    (jq absent: the site half is skipped)"; exit 0; }
F="$S/site"; fixture_repo "$F" AGENTS.md docs
rm -rf "$F/.ai/repo/skills"; mkdir -p "$F/.ai/repo/skills"
cp -R "$ROOT/.ai/repo/skills/README.md" "$F/.ai/repo/skills/" 2>/dev/null || true
cp -R .ai/repo/skills/review .ai/repo/skills/alpha "$F/.ai/repo/skills/"
mkdir -p "$F/site/data" "$F/test"; cp "$ROOT/site/data/marketing.toml" "$ROOT/site/data/nav.toml" "$F/site/data/"; cp -R "$ROOT/site/content-src" "$ROOT/site/templates" "$ROOT/site/static" "$F/site/"; cp -R "$ROOT/test/cases" "$F/test/"
git -C "$F" init -q; git -C "$F" config user.email fixture@example.invalid; git -C "$F" config user.name fixture
git -C "$F" add -A >/dev/null; git -C "$F" commit -qm fixture
expect_exit 0 "$F/scripts/generate-site-data"
SJ="$F/site/data/generated/skills.json"
jq -e '.count == 2 and ([.skills[].id]) == ["alpha","review"] and .skills[1].route == "/skills/review/" and .skills[1].examples[0].title == "Example for review" and (.skills[1].examples[0].body | contains("Apply the review skill")) and .by_tag.fixture == ["alpha","review"]' "$SJ" >/dev/null \
  || { echo "    skills.json is not the catalogue: "; jq -c . "$SJ"; exit 1; }
[ -f "$F/site/content/skills/_index.md" ] && [ -f "$F/site/content/skills/review.md" ] && [ -f "$F/site/content/skills/alpha.md" ] || { echo "    the skill pages were not generated"; exit 1; }
grep -q '^## Procedure$' "$F/site/content/skills/review.md" || { echo "    the skill page does not carry the skill's body (its sections one level down)"; exit 1; }
grep -q '^source = ".ai/repo/skills/review/SKILL.md"$' "$F/site/content/skills/review.md" || { echo "    the skill page does not name its source"; exit 1; }
# in sync, then stale the moment a skill changes, then in sync again once regenerated
expect_exit 0 "$F/scripts/generate-site-data" --check
sed -i.bak 's/^description: The review procedure\.$/description: The review procedure, revised./' "$F/.ai/repo/skills/review/SKILL.md"; rm -f "$F/.ai/repo/skills/review/SKILL.md.bak"
expect_exit 10 "$F/scripts/generate-site-data" --check
expect_exit 0 "$F/scripts/generate-site-data"
jq -e '.skills[1].description == "The review procedure, revised."' "$SJ" >/dev/null || { echo "    the regenerated catalogue does not carry the edit"; exit 1; }
# a hand edit to the projection is drift
cp "$SJ" "$S/skills.json.good"; jq '.count = 9' "$SJ" > "$SJ.tmp" && mv "$SJ.tmp" "$SJ"
expect_exit 10 "$F/scripts/generate-site-data" --check
cp "$S/skills.json.good" "$SJ"
# a broken skill fails the build before a byte is published
awk '{ print } /^schema: skill\/v1$/ { print "stray: yes" }' "$F/.ai/repo/skills/alpha/SKILL.md" > "$S/alpha.md" && mv "$S/alpha.md" "$F/.ai/repo/skills/alpha/SKILL.md"
expect_exit 10 "$F/scripts/generate-site-data"
expect_grep 'unknown front-matter key\(s\): stray'
git -C "$F" checkout -q -- .ai/repo/skills/alpha/SKILL.md
# removal: the page and the entry go with the file
git -C "$F" rm -rq .ai/repo/skills/alpha >/dev/null
expect_exit 0 "$F/scripts/generate-site-data"
jq -e '.count == 1 and .skills[0].id == "review"' "$SJ" >/dev/null || { echo "    the removed skill is still in the catalogue"; exit 1; }
[ ! -e "$F/site/content/skills/alpha.md" ] || { echo "    the removed skill still has a page"; exit 1; }
if command -v zola >/dev/null; then
  ( cd "$F/site" && zola build --force -o "$S/public" >/dev/null 2>&1 ) || { echo "    zola could not build the fixture site"; exit 1; }
  [ -f "$S/public/skills/index.html" ] && [ -f "$S/public/skills/review/index.html" ] || { echo "    the built site lacks the skills index or the skill page"; exit 1; }
  [ ! -e "$S/public/skills/alpha" ] || { echo "    the built site still carries the removed skill"; exit 1; }
  grep -q 'majordomus://skill/review' "$S/public/skills/review/index.html" || { echo "    the skill page does not name its URI"; exit 1; }
  grep -q 'Example for review' "$S/public/skills/review/index.html" || { echo "    the skill page does not render its example"; exit 1; }
  grep -q 'skills/review/' "$S/public/skills/index.html" || { echo "    the skills index does not link the skill"; exit 1; }
else
  echo "    (zola absent: the built pages are not checked)"
fi
