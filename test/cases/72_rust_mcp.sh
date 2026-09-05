# The Rust executable serves the layer the shell tool writes: built from the same tree, it
# is started inside a repository `init` created, spoken to over its real stdin and stdout
# with protocol frames, and asked for what the layer declares. Then the layer is extended
# with a rule and broken with another, and the executable is asked again.
#
# Skips itself when cargo is absent, as the site cases do for zola: a CLI-only change needs
# no Rust toolchain, and CI's rust job runs the crate's own suite on every push.
. "$ROOT/test/lib.sh"
command -v cargo >/dev/null 2>&1 || { echo "    skip: cargo not installed"; exit 0; }
MANIFEST="$ROOT/apps/majordomus-cli/Cargo.toml"
[ -f "$MANIFEST" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
# a machine-wide rustflags profile can make a dev build refuse to link; the crate itself
# sets none, so build with none
# scratch files live outside the repository, so the "nothing changed" check below sees only
# what the server did
S="$(mktemp -d "${TMPDIR:-/tmp}/mj72.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RUSTFLAGS='' cargo build -q --manifest-path "$MANIFEST" 2>"$S/build.log" || { cat "$S/build.log"; echo "    cargo build failed"; exit 1; }
RB="$ROOT/apps/majordomus-cli/target/debug/majordomus"
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
# the executable reads kinds.yaml and the schemas from the tool distribution at run time
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

# --- the executable's public surface, black-box
expect_exit 0 "$RB" --help
expect_grep '^  mcp '
expect_exit 0 "$RB" mcp --help
expect_grep '^Usage: majordomus mcp'
expect_exit 2 "$RB" nonsense
expect_grep "unrecognized subcommand 'nonsense'"

# --- the layer init writes is served, complete and with state ok
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
# without a distribution nothing starts, and the search is named
( unset MAJORDOMUS_SHARE; expect_exit 12 "$RB" mcp --inspect; expect_grep 'no share directory holds kinds.yaml' ) || exit 1
before="$(git status --porcelain; git ls-files -s | shasum -a 256)"
expect_exit 0 "$RB" mcp --inspect
expect_grep '^state       ok$'
expect_grep '^resource    majordomus://policy/\.ai/repo/policy\.yaml$'
expect_grep '^resource    majordomus://profile/implementation$'
expect_grep '^resource    majordomus://prompt/continue$'
expect_grep '^resource    majordomus://rule/majordomus\.scope-integrity@1$'
expect_no_grep '\.ai/local/'
# every rule the shell tool resolves is a resource, and nothing else is: the two readers agree
"$MJ" rules list | awk '{print $1}' | sort > "$S/shell_rules.txt"
[ -s "$S/shell_rules.txt" ] || { echo "    rules list printed no rules"; exit 1; }
"$RB" mcp --inspect 2>/dev/null | sed -n 's|^resource    majordomus://rule/\(.*\)@\([0-9]*\)$|\1|p' | sort > "$S/rust_rules.txt"
cmp -s "$S/shell_rules.txt" "$S/rust_rules.txt" || { echo "    the Rust executable and rules list disagree:"; diff "$S/shell_rules.txt" "$S/rust_rules.txt" | head; exit 1; }

# --- a real session over the real pipes: initialize, list, read a vendored rule, call a tool
req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
{
  req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case72","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 resources/list
  req 3 resources/read '{"uri":"majordomus://rule/majordomus.scope-integrity@1"}'
  req 4 tools/call '{"name":"majordomus_get","arguments":{"uri":"majordomus://prompt/continue"}}'
  req 5 tools/call '{"name":"majordomus_repository","arguments":{}}'
  req 6 tools/call '{"name":"majordomus_get","arguments":{"uri":"majordomus://repository"}}'
  req 7 resources/read '{"uri":"majordomus://repository"}'
} > "$S/session.in"
rc=0; MAJORDOMUS_LOG=debug "$RB" mcp < "$S/session.in" > "$S/session.out" 2> "$S/session.err" || rc=$?
[ "$rc" = 0 ] || { echo "    the server exited $rc at EOF"; cat "$S/session.err"; exit 1; }
# stdout: exactly one JSON frame per request, nothing else
[ "$(wc -l < "$S/session.out" | tr -d ' ')" = 7 ] || { echo "    expected 7 frames on stdout, got:"; cat "$S/session.out"; exit 1; }
n=0
while IFS= read -r line; do
  n=$((n+1))
  printf '%s' "$line" | jq -e '.jsonrpc == "2.0" and (.id | type) == "number"' >/dev/null \
    || { echo "    stdout line $n is not a protocol frame: $line"; exit 1; }
done < "$S/session.out"
# stderr carries the diagnostics, stdout does not
grep -q 'index built' "$S/session.err" || { echo "    no diagnostics on stderr"; cat "$S/session.err"; exit 1; }
grep -q 'index built' "$S/session.out" && { echo "    a diagnostic leaked into stdout"; exit 1; }
# the handshake
sed -n 1p "$S/session.out" | jq -e '.result.protocolVersion == "2025-06-18" and .result.serverInfo.name == "majordomus" and .result.capabilities.resources and .result.capabilities.tools and (.result.capabilities.prompts == null)' >/dev/null \
  || { echo "    initialize result is wrong: $(sed -n 1p "$S/session.out")"; exit 1; }
# the listing carries provenance
sed -n 2p "$S/session.out" | jq -e '.result.resources[] | select(.uri == "majordomus://rule/majordomus.scope-integrity@1") | ._meta.majordomus.provenance.path == ".ai/repo/rules/vendor/majordomus/rules/scope-integrity.v1.md" and ._meta.majordomus.provenance.section == "rules"' >/dev/null \
  || { echo "    the vendored rule is not listed with its provenance"; exit 1; }
# the read returns the file as the repository holds it
sed -n 3p "$S/session.out" | jq -j '.result.contents[0].text' > "$S/read.txt"
cmp -s "$S/read.txt" .ai/repo/rules/vendor/majordomus/rules/scope-integrity.v1.md || { echo "    resources/read did not return the file byte for byte"; exit 1; }
# the tool returns metadata the front matter carries
sed -n 4p "$S/session.out" | jq -e '.result.isError == false and .result.structuredContent.metadata.name == "continue" and .result.structuredContent.provenance.source_class == "prompt"' >/dev/null \
  || { echo "    majordomus_get did not return the prompt's metadata"; exit 1; }
sed -n 5p "$S/session.out" | jq -e '.result.structuredContent.state == "ok" and (.result.structuredContent.diagnostics | length) == 0 and .result.structuredContent.repository.git.state == "available"' >/dev/null \
  || { echo "    majordomus_repository does not report a healthy index"; exit 1; }
# one resolution for the tool, the route and the resource read: the repository URI answers
# repository.info as a JSON document tagged builtin, a file of the layer is tagged declarative,
# and the document's text is byte for byte what resources/read returns for the same URI
sed -n 4p "$S/session.out" | jq -e '.result.structuredContent.source == "declarative"' >/dev/null \
  || { echo "    majordomus_get does not tag a file of the layer as declarative"; exit 1; }
sed -n 6p "$S/session.out" | jq -e '.result.isError == false and .result.structuredContent.source == "builtin" and .result.structuredContent.id == "repository.info" and .result.structuredContent.identity == "repository" and .result.structuredContent.media_type == "application/json" and .result.structuredContent.answer.state == "ok"' >/dev/null \
  || { echo "    majordomus_get did not answer majordomus://repository as repository.info"; exit 1; }
sed -n 5p "$S/session.out" | jq -S '.result.structuredContent' > "$S/report.json"
sed -n 6p "$S/session.out" | jq -S '.result.structuredContent.answer' > "$S/answer.json"
cmp -s "$S/report.json" "$S/answer.json" || { echo "    majordomus_get's answer is not majordomus_repository's report"; exit 1; }
sed -n 6p "$S/session.out" | jq -j '.result.structuredContent.content' > "$S/get.txt"
sed -n 7p "$S/session.out" | jq -j '.result.contents[0].text' > "$S/read2.txt"
cmp -s "$S/get.txt" "$S/read2.txt" || { echo "    majordomus_get and resources/read disagree on majordomus://repository"; exit 1; }
# nothing changed
after="$(git status --porcelain; git ls-files -s | shasum -a 256)"
[ "$before" = "$after" ] || { echo "    serving changed the repository"; git status --porcelain; exit 1; }

# --- data-driven: a rule added to the layer is served, with no change to the executable
cat > .ai/repo/rules/project/example.v1.md <<'R'
---
id: project.example
version: 1
kind: rule
title: Example
description: An example project rule.
statement: Do the example thing.
status: active
class: advisory
depends_on: []
tags: [example]
---

# Rationale

Because.
R
git add -A >/dev/null
expect_exit 0 "$RB" mcp --inspect
expect_grep '^resource    majordomus://rule/project\.example@1$'
"$MJ" rules list | grep -q '^project.example ' || { echo "    the shell tool does not see the same rule"; exit 1; }
rm .ai/repo/rules/project/example.v1.md; git add -A >/dev/null
expect_exit 0 "$RB" mcp --inspect
expect_no_grep 'project\.example'

# --- degraded, never silently smaller: a broken rule is named, the rest still serves
printf -- '---\nid: project.broken\nversion: 1\n' > .ai/repo/rules/project/broken.v1.md
git add -A >/dev/null
expect_exit 10 "$RB" mcp --inspect
expect_grep '^state       degraded$'
expect_grep '^FAIL malformed_front_matter +\.ai/repo/rules/project/broken\.v1\.md'
expect_grep '^resource    majordomus://rule/majordomus\.scope-integrity@1$'
expect_exit 10 "$RB" mcp --strict < /dev/null
expect_grep 'refusing to serve under --strict'
# a duplicate identity excludes both claimants and names both
cp .ai/repo/rules/vendor/majordomus/rules/scope-integrity.v1.md .ai/repo/rules/project/copy.v1.md
git add -A >/dev/null
expect_exit 10 "$RB" mcp --inspect
expect_grep '^FAIL duplicate_identity +\.ai/repo/rules/project/copy\.v1\.md'
expect_grep '^FAIL duplicate_identity +\.ai/repo/rules/vendor/majordomus/rules/scope-integrity\.v1\.md'
expect_no_grep '^resource    majordomus://rule/majordomus\.scope-integrity@1$'
rm .ai/repo/rules/project/broken.v1.md .ai/repo/rules/project/copy.v1.md; git add -A >/dev/null

# --- the shell tool's allow-lists are a projection of the schemas: derived and in sync
expect_exit 0 "$RB" generate allow --check
expect_grep 'generate --check: in sync'
for f in "$ROOT"/share/allow/*.txt; do
  grep -qvE '^\^' "$f" && { echo "    $f carries a line that is not a pattern"; exit 1; }
  true
done

# --- not a Majordomus repository: named, exit 12, nothing served
mkdir -p "$S/plain" && ( cd "$S/plain" && git init -q . )
expect_exit 12 "$RB" mcp --repo "$S/plain" --inspect
expect_grep 'no Majordomus repository found'
