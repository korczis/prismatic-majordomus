# majordomus-covers: none
# The UI kit is checked by one capability (I4334, ADR 0229): `majordomus design kit` judges a
# repository's interface against the flowbite-pro profile, and every signal is proven here by
# mutating a conformant disposable repository once and expecting the finding it names.
#
#   no user interface                  exit 0, verdict no_ui, under --check too, and the
#                                      same verdict to a tools/call over MCP
#   a conformant Tailwind 4 site       exit 0, verdict clean
#   S1 flowbite removed                exit 10, an S1 finding on package.json
#   S2 @plugin removed                 exit 10, an S2 finding on the entry stylesheet
#   S2 the Play CDN loaded             exit 10, an S2 finding on the page
#   S3 Flowbite's JS not loaded        exit 10, an S3 finding on the page
#   S3 a script that reads data-*      exit 10, an S3 finding on that script
#   S4 daisyui declared                exit 10, an S4 finding; Alpine only reported
#   S5 no fixed navbar, no sidebar,    exit 10, an S5 finding each
#      no breadcrumb
#   without --check                    the same findings, exit 0
#   adopt --dry-run                    writes nothing
#   adopt, then adopt again            the rule, the block and the entries once; the second
#                                      run changes nothing
#   an exemption with its reason       the finding moves to the exempted list, exit 0
#   an exemption without a reason      exit 2, nothing judged
#   a repository nobody supervises     judged through --repo by the same function
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-4334.XXXXXX")"
trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

kit() { "$RB" design kit --check --format json 2>"$S/err"; }
verdict() { jq -r '.verdict'; }

# ---------------------------------------------------------------- no user interface
out="$(kit)" && code=0 || code=$?
[ "$code" = 0 ] || { echo "    a repository without a UI exited $code: $out $(cat "$S/err")"; exit 1; }
[ "$(printf '%s' "$out" | verdict)" = no_ui ] || { echo "    no UI is not no_ui: $out"; exit 1; }
[ "$(printf '%s' "$out" | jq -r '.passes')" = true ] || { echo "    no UI does not pass: $out"; exit 1; }

# The same question over MCP, as a client asks it: one standalone session against this
# fixture, and the typed answer of the tools/call.
mcp_kit() {
  { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4334","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_design_kit","arguments":{}}}\n'
  } | "$RB" mcp --standalone --repo "$PWD" 2>"$S/mcp.err" | sed -n 2p | jq -c '.result.structuredContent // empty'
}
m="$(mcp_kit)"
[ "$(printf '%s' "$m" | verdict)" = no_ui ] || {
  echo "    the MCP tool majordomus_design_kit does not agree with the command: $m $(cat "$S/mcp.err")"; exit 1; }

# ---------------------------------------------------------------- a conformant site
mkdir -p src
cat > package.json <<'J'
{"devDependencies": {"tailwindcss": "^4.1.0", "flowbite": "^4.0.1"}}
J
cat > src/input.css <<'C'
@import "tailwindcss";
@plugin "flowbite/plugin";
@source "../node_modules/flowbite";
C
printf "import 'flowbite';\n" > src/main.js
cat > index.html <<'H'
<!doctype html>
<html><body class="bg-gray-50 dark:bg-gray-900">
<nav class="fixed z-30 w-full bg-white border-b border-gray-200 dark:bg-gray-800 dark:border-gray-700">
  <button data-drawer-target="sidebar" data-drawer-toggle="sidebar" aria-controls="sidebar">Menu</button>
</nav>
<aside id="sidebar" class="fixed top-0 left-0 w-64" aria-label="Sidebar">links</aside>
<main><nav class="flex" aria-label="Breadcrumb"><ol></ol></nav></main>
<script type="module" src="/src/main.js"></script>
</body></html>
H
git add -A >/dev/null && git commit -qm site

out="$(kit)" && code=0 || code=$?
[ "$code" = 0 ] && [ "$(printf '%s' "$out" | verdict)" = clean ] || {
  echo "    the conformant site is not clean (exit $code): $out $(cat "$S/err")"; exit 1; }

# expect SIGNAL PATH WHAT: the check exits 10 with a finding of SIGNAL on PATH, then the tree
# is put back as it was committed
expect() {
  out="$(kit)" && code=0 || code=$?
  [ "$code" = 10 ] || { echo "    $3: exit $code, not 10: $out $(cat "$S/err")"; exit 1; }
  printf '%s' "$out" | jq -e --arg s "$1" --arg p "$2" \
    '[.findings[] | select(.signal == $s and .path == $p and (.evidence | length) > 0 and (.remedy | length) > 0)] | length > 0' >/dev/null \
    || { echo "    $3: no $1 finding on $2 with evidence and remedy: $out"; exit 1; }
  git checkout -q -- . && git clean -qfd
}
# edit FILE AWK-PROGRAM: rewrite FILE through awk
edit() { awk "$2" "$1" > "$S/edit" && mv "$S/edit" "$1"; }

edit package.json '{ sub(/, "flowbite": "\^4.0.1"/, ""); print }'
expect S1 package.json "flowbite removed from the dependencies"

edit src/input.css '!/@plugin/'
expect S2 src/input.css "the @plugin line removed"

edit index.html '{ sub(/<\/body>/, "<script src=\"https://cdn.tailwindcss.com\"></script></body>"); print }'
expect S2 index.html "the Play CDN loaded"

printf "console.log('no kit');\n" > src/main.js
expect S3 index.html "Flowbite's JavaScript not loaded"

printf "document.querySelectorAll('[data-drawer-toggle]').forEach(b => b.onclick = open);\n" > src/nav.js
expect S3 src/nav.js "a script that re-implements the drawer"

edit package.json '{ sub(/"flowbite": "\^4.0.1"/, "\"flowbite\": \"^4.0.1\", \"daisyui\": \"^5\", \"alpinejs\": \"3\""); print }'
out="$(kit)" || true
printf '%s' "$out" | jq -e '[.reported[] | select(test("alpinejs"))] | length == 1' >/dev/null \
  || { echo "    Alpine.js is not reported: $out"; exit 1; }
printf '%s' "$out" | jq -e '[.findings[] | select(.evidence | test("alpinejs"))] | length == 0' >/dev/null \
  || { echo "    Alpine.js is refused, and it is only to be reported: $out"; exit 1; }
expect S4 package.json "daisyui declared"

edit index.html '{ sub(/class="fixed z-30/, "class=\"z-30"); print }'
expect S5 index.html "the navbar is not fixed"

edit index.html '{ sub(/aria-label="Sidebar"/, "aria-label=\"Menu\""); print }'
expect S5 index.html "the sidebar is not labelled"

edit index.html '{ sub(/ aria-label="Breadcrumb"/, ""); print }'
expect S5 index.html "the breadcrumb removed"

# without --check the findings are reported and the exit is 0
edit package.json '{ sub(/, "flowbite": "\^4.0.1"/, ""); print }'
expect_exit 0 "$RB" design kit || exit 1
expect_grep 'FAIL  S1  package.json' || exit 1
git checkout -q -- .

# ---------------------------------------------------------------- adoption
expect_exit 0 "$RB" design kit adopt flowbite-pro --dry-run || exit 1
expect_grep 'dry run: nothing written' || exit 1
[ -z "$(git status --porcelain)" ] || { echo "    a dry run wrote: $(git status --porcelain)"; exit 1; }

expect_exit 0 "$RB" design kit adopt flowbite-pro --format json || exit 1
rule=.ai/repo/rules/project/ui-uses-flowbite-pro.v1.md
expect_file "$rule" || exit 1
cmp -s "$rule" "$ROOT/share/design/kits/flowbite-pro.rule.md" || { echo "    the rule is not the template"; exit 1; }
grep -q '^design_kit:$' .ai/repo/policy.yaml || { echo "    the policy has no design_kit block"; exit 1; }
grep -q 'name: design-kit-on-push' .ai/repo/policy.yaml || { echo "    the pre-push entry is missing"; exit 1; }
grep -q 'args: \[design, kit, --check\]' .ai/repo/policy.yaml || { echo "    the entry does not run design kit --check"; exit 1; }
git add -A >/dev/null && git commit -qm adopt

out="$("$RB" design kit adopt flowbite-pro --format json 2>"$S/err")" \
  || { echo "    a second adoption failed: $(cat "$S/err")"; exit 1; }
printf '%s' "$out" | jq -e '[.writes[] | select(.action != "unchanged")] | length == 0' >/dev/null \
  || { echo "    a second adoption changed something: $out"; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "    a second adoption wrote: $(git status --porcelain)"; exit 1; }

out="$(kit)" && code=0 || code=$?
[ "$code" = 0 ] && [ "$(printf '%s' "$out" | jq -r '.policy_declared')" = true ] || {
  echo "    the adopted repository is not clean under its own policy (exit $code): $out"; exit 1; }

# ---------------------------------------------------------------- exemptions
edit index.html '{ sub(/ aria-label="Breadcrumb"/, ""); print }'
edit .ai/repo/policy.yaml '{ print } /^  profile: flowbite-pro$/ { print "  exempt:"; print "    - path: index.html"; print "      because: the breadcrumb lives in the pages the shell frames"; print "      signals: [S5]" }'
out="$(kit)" && code=0 || code=$?
[ "$code" = 0 ] || { echo "    an exempted finding still refuses (exit $code): $out"; exit 1; }
printf '%s' "$out" | jq -e '(.exempted | length) == 1 and .exempted[0].because != "" and .exemptions[0].matched == 1' >/dev/null \
  || { echo "    the exemption is not listed with its reason: $out"; exit 1; }

edit .ai/repo/policy.yaml '!/because: the breadcrumb/'
out="$(kit)" && code=0 || code=$?
[ "$code" = 2 ] || { echo "    an exemption without a because exited $code, not 2: $out"; exit 1; }
[ "$(printf '%s' "$out" | verdict)" = bad_policy ] || { echo "    the verdict is not bad_policy: $out"; exit 1; }
printf '%s' "$out" | jq -e '.policy_errors[0] | test("because")' >/dev/null \
  || { echo "    the reason does not name the missing because: $out"; exit 1; }
git checkout -q -- .

# ---------------------------------------------------------------- a repository nobody supervises
U="$S/unsupervised"
mkdir -p "$U/src"
( cd "$U" && git init -q . )
cp package.json index.html "$U/"
cp src/input.css src/main.js "$U/src/"
expect_exit 0 "$RB" design kit --check --repo "$U" || exit 1
expect_grep 'design kit: clean' || exit 1
printf '{"devDependencies": {"tailwindcss": "^4.1.0", "flowbite": "^4.0.1", "bootstrap": "5"}}\n' > "$U/package.json"
expect_exit 10 "$RB" design kit --check --repo "$U" || exit 1
expect_grep 'FAIL  S4  package.json' || exit 1
[ ! -e "$U/.ai" ] || { echo "    judging an unsupervised repository wrote its layer"; exit 1; }
true
