# majordomus-covers: none
# The repository scope through the built executable and the shell tool, in a repository
# the shell tool's own `init` wrote: the seeded declaration passes doctor and is the one
# the executable reads; a source outside the scope (a document over the limit, a binary
# with a Markdown name, a secret, an undeclared root file) is discovered and dropped with
# the reason, never listed as a resource, never served; every path is answered in or out
# with the rule that decided, through the command line and over real MCP pipes; a
# malformed declaration is refused by both tools naming the key; and a repository whose
# manifest names no scope is read under the distribution's default, which is said.
#
# Skips itself when cargo is absent, as the other Rust cases do.
. "$ROOT/test/lib.sh"
command -v cargo >/dev/null 2>&1 || { echo "    skip: cargo not installed"; exit 0; }
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }
MANIFEST="$ROOT/apps/majordomus-cli/Cargo.toml"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj93.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RUSTFLAGS='' cargo build -q --manifest-path "$MANIFEST" 2>"$S/build.log" || { cat "$S/build.log"; echo "    cargo build failed"; exit 1; }
RB="$ROOT/apps/majordomus-cli/target/debug/majordomus"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

# --- the distribution seeds the declaration, and the two tools agree it is well-formed
"$MJ" init >/dev/null
# doctor is healthy only with the hooks wired and the budget out of the way, as case 83 does;
# the hooks name this tree's executable and are installed after the commits, so that git
# never runs whatever `majordomus` is on PATH
sed -i.bak 's/^    doctor_ms: 3000$/    doctor_ms: 600000/; s/^    watch_ms: 3000$/    watch_ms: 600000/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
"$MJ" update >/dev/null
[ -f .ai/repo/scope.yaml ] || { echo "    init seeded no .ai/repo/scope.yaml"; exit 1; }
grep -q '^  scope: repo/scope.yaml$' .ai/manifest.yaml || { echo "    the seeded manifest names no scope section"; exit 1; }
cmp -s .ai/repo/scope.yaml "$ROOT/share/skeleton/ai/repo/scope.yaml" || { echo "    the seeded scope is not the distribution's default"; exit 1; }
git add -A >/dev/null && git commit -qm install
hooks_on()  { printf '#!/usr/bin/env bash\n%s doctor\n' "$MJ" > .git/hooks/pre-commit; printf '#!/usr/bin/env bash\n%s finish --check\n' "$MJ" > .git/hooks/pre-push; chmod +x .git/hooks/pre-commit .git/hooks/pre-push; }
hooks_off() { rm -f .git/hooks/pre-commit .git/hooks/pre-push; }
hooks_on
expect_exit 0 "$MJ" doctor
expect_grep '^OK   layout      \.ai/repo/scope\.yaml .* version 1; [0-9]+ in pathspec.s.; every key is one the schema declares'
expect_exit 0 "$RB" scope
expect_grep '^scope        \.ai/repo/scope\.yaml \(repository\)$'
expect_grep '^tracked      [0-9]+ file.s.: [0-9]+ in, [0-9]+ out$'
# the allow-list the shell tool checks keys against is generated from the schema, not written
expect_exit 0 "$RB" generate allow --check

# --- sources outside the scope are discovered, dropped with the reason, and never served
mkdir -p docs test/fixtures
printf '# Big\n\n' > docs/big.md; head -c 1100000 /dev/zero | tr '\0' 'x' >> docs/big.md
printf '# Blob\n\0\1\2\n' > docs/blob.md
printf 'TOKEN=1\n' > docs/.env
printf 'not a document\n' > NOTES.md
printf '{"padding": "%s"}\n' "$(head -c 70000 /dev/zero | tr '\0' 'y')" > test/fixtures/large.json
printf '{}\n' > test/fixtures/small.json
hooks_off; git add -A >/dev/null && git commit -qm "outside the scope"; hooks_on
before="$(git status --porcelain; git ls-files -s | shasum -a 256)"
expect_exit 0 "$RB" mcp --inspect
expect_grep '^resource    majordomus://document/docs/CLI\.md$' || true
expect_no_grep 'majordomus://document/docs/big\.md'
expect_no_grep 'majordomus://document/docs/blob\.md'
expect_no_grep 'majordomus://document/NOTES\.md'
"$RB" mcp --inspect --format json 2>/dev/null > "$S/inspect.json"
jq -e '[.diagnostics[] | select(.code == "out_of_scope") | .path] | (index("docs/big.md") != null) and (index("docs/blob.md") != null) and (index("NOTES.md") != null)' "$S/inspect.json" >/dev/null \
  || { echo "    the dropped sources are not reported as out_of_scope:"; jq '.diagnostics' "$S/inspect.json"; exit 1; }
jq -e '.diagnostics[] | select(.path == "docs/big.md") | .message | test("over_limit") and test("max_bytes 1048576")' "$S/inspect.json" >/dev/null \
  || { echo "    the oversized document does not name its limit"; exit 1; }
jq -e '.diagnostics[] | select(.path == "docs/blob.md") | .message | test("binary")' "$S/inspect.json" >/dev/null \
  || { echo "    the binary with a Markdown name is not reported as binary"; exit 1; }
jq -e '.diagnostics[] | select(.path == "NOTES.md") | .message | test("undeclared")' "$S/inspect.json" >/dev/null \
  || { echo "    the undeclared root file is not reported as undeclared"; exit 1; }
jq -e '.diagnostics[] | select(.code == "tracked_secret") | .path == "docs/.env"' "$S/inspect.json" >/dev/null \
  || { echo "    the tracked secret is not reported"; exit 1; }
jq -e '.state == "ok"' "$S/inspect.json" >/dev/null || { echo "    a scope exclusion degraded the index; it is a warning, not an error"; exit 1; }

# --- every path is answered, in or out, with the rule that decided
expect_exit 0 "$RB" scope docs/CLI.md docs/big.md docs/blob.md docs/.env NOTES.md test/fixtures/large.json test/fixtures/small.json .ai/local/state/current.yaml target/debug/x
expect_grep '^in                        docs/CLI\.md  \(docs/\*\*\)'
expect_grep '^out  over_limit           docs/big\.md  \(max_bytes 1048576\)'
expect_grep '^out  binary               docs/blob\.md  \(binary\)'
expect_grep '^out  secret               docs/\.env  \(\.env\)'
expect_grep '^out  undeclared           NOTES\.md'
expect_grep '^out  fixture_over_limit   test/fixtures/large\.json  \(\*\*/fixtures/ over fixtures\.max_bytes 65536\)'
expect_grep '^in                        test/fixtures/small\.json  \(test/\*\*\)'
expect_grep '^out  path                 \.ai/local/state/current\.yaml  \(\.ai/local/\)'
expect_grep '^out  path                 target/debug/x  \(\*\*/target/\)  \[absent\]'
expect_exit 10 "$RB" scope --check docs/CLI.md docs/.env
expect_exit 0 "$RB" scope --check docs/CLI.md
"$RB" scope --format json docs/.env 2>/dev/null | jq -e '.[0] | .verdict == "out" and .reason == "secret" and .rule == ".env" and .exists == true' >/dev/null \
  || { echo "    the JSON answer is wrong"; exit 1; }
expect_exit 13 "$RB" scope /etc/passwd
expect_grep 'absolute'

# --- the same answers over real MCP pipes: the tool, and the scope as a resource
req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
{
  req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case93","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_scope_classify","arguments":{"path":"docs/.env"}}'
  req 3 resources/read '{"uri":"majordomus://scope"}'
  req 4 tools/call '{"name":"majordomus_get","arguments":{"uri":"majordomus://document/docs/big.md"}}'
  req 5 tools/call '{"name":"majordomus_scope_classify","arguments":{"path":"../etc/passwd"}}'
} > "$S/session.in"
rc=0; "$RB" mcp --standalone < "$S/session.in" > "$S/session.out" 2> "$S/session.err" || rc=$?
[ "$rc" = 0 ] || { echo "    the server exited $rc at EOF"; cat "$S/session.err"; exit 1; }
sed -n 2p "$S/session.out" | jq -e '.result.structuredContent.reason == "secret" and .result.structuredContent.verdict == "out"' >/dev/null \
  || { echo "    majordomus_scope_classify answered wrong: $(sed -n 2p "$S/session.out")"; exit 1; }
sed -n 3p "$S/session.out" | jq -e '.result.contents[0].text | fromjson | .origin == "repository" and .path == ".ai/repo/scope.yaml" and (.tracked.by_reason.secret == 1)' >/dev/null \
  || { echo "    majordomus://scope is not the scope report: $(sed -n 3p "$S/session.out")"; exit 1; }
sed -n 4p "$S/session.out" | jq -e '(.error != null) or (.result.isError == true)' >/dev/null \
  || { echo "    an object outside the scope was served: $(sed -n 4p "$S/session.out")"; exit 1; }
sed -n 5p "$S/session.out" | jq -e '(.error != null) or (.result.isError == true)' >/dev/null \
  || { echo "    a path outside the repository was judged: $(sed -n 5p "$S/session.out")"; exit 1; }
after="$(git status --porcelain; git ls-files -s | shasum -a 256)"
[ "$before" = "$after" ] || { echo "    the executable changed the tracked tree"; git status --porcelain; exit 1; }

# --- a malformed declaration is refused by both tools, naming the key
cp .ai/repo/scope.yaml "$S/scope.good"
printf '  colour: red\n' >> .ai/repo/scope.yaml
expect_exit 10 "$MJ" doctor
expect_grep '^FAIL layout      \.ai/repo/scope\.yaml .* unknown key.s.: out\.colour'
expect_exit 10 "$RB" scope
expect_grep 'scope\.yaml.*colour'
cp "$S/scope.good" .ai/repo/scope.yaml
sed -i.bak 's|^  - docs/\*\*$|  - /docs/**|' .ai/repo/scope.yaml && rm -f .ai/repo/scope.yaml.bak
expect_exit 10 "$RB" scope
expect_grep "/docs/\*\*.*relative"
cp "$S/scope.good" .ai/repo/scope.yaml

# --- a manifest naming no scope: the distribution's default applies, and both tools say so
git rm -q .ai/repo/scope.yaml
sed -i.bak '/^  scope: repo\/scope\.yaml$/d' .ai/manifest.yaml && rm -f .ai/manifest.yaml.bak
hooks_off; git add -A >/dev/null && git commit -qm "no scope"; hooks_on
expect_exit 0 "$MJ" doctor
expect_grep 'INFO layout      \.ai/repo/scope\.yaml .* the manifest names no scope section; the executable reads the distribution.s default'
expect_exit 0 "$RB" scope
expect_grep '^scope        .*share/skeleton/ai/repo/scope\.yaml \(distribution\)$'
# and --extend seeds the file back, saying the manifest must name it
expect_exit 0 "$MJ" init --extend
expect_grep 'names no scope section'
[ -f .ai/repo/scope.yaml ] || { echo "    init --extend did not seed the scope"; exit 1; }
