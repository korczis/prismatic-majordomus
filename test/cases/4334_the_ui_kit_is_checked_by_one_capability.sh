# majordomus-covers: none
# The UI kit is checked by one capability (I4334, ADR 0230): `majordomus design kit` judges a
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
#   a workspace wired, using nothing   exit 10, an S1 finding on its manifest; a manifest
#                                      under examples/ is a notice, its kit never counted
#   a licensed marker, no kit adopted  exit 10, an S1 finding: the design licence is
#                                      inactive, so the marker is unlicensed
#   without --check                    the same findings, exit 0
#   adopt --dry-run                    writes nothing
#   adopt, then adopt again            the rule, the entries and ui.kit in the skeleton's ui:
#                                      block once, no design_kit key, doctor accepts the
#                                      adopted policy; the second run changes nothing
#   ... then check                     clean under its own ui block
#   a licensed marker once adopted     exit 10, an S1 finding: a public tree with a UI and no
#                                      evidence is not judged, so the licence is undecided
#   an exemption with its reason       ui.exempt {path, because}: every finding at the path
#                                      moves to the exempted list, exit 0, doctor accepts it
#   adopt over a stale rule            the rule is brought up to the template, and its
#                                      section 'In this repository' is kept
#   an exemption without a reason      exit 2, nothing judged: no finding, no exemption, no
#                                      surface
#   ... and over a licensed marker     the finding stands, exit 10
#   a published page                   an interface; a stray manifest is a notice; docs/ is
#                                      published only by a publish_dir or Pages step, never by
#                                      a checkout's path; a nested repository is a notice
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

# ---------------------------------------------------------------- surfaces and usage
# A declared workspace is a surface of its own: wired (its dependency, its stylesheet) and
# using the kit in none of its own files, it fails S1, whatever the root uses. A manifest
# under examples/ is no surface: a notice, and its kit is not counted.
printf '{"workspaces": ["apps/*"], "devDependencies": {"tailwindcss": "^4.1.0", "flowbite": "^4.0.1"}}\n' > package.json
mkdir -p apps/web examples/demo
printf '{"dependencies": {"vue": "3", "flowbite": "4", "tailwindcss": "4"}}\n' > apps/web/package.json
cp src/input.css apps/web/app.css
printf "import 'flowbite';\n" > apps/web/main.js
printf '<main class="p-4"></main>\n' > apps/web/index.html
printf '{"dependencies": {"react": "19", "daisyui": "5"}}\n' > examples/demo/package.json
out="$(kit)" || true
printf '%s' "$out" | jq -e '[.surfaces[] | .dir] == ["", "apps/web"] and .surfaces[1].kind == "workspace" and (.surfaces[1].usage | length) == 0' >/dev/null \
  || { echo "    the workspace is not a surface of its own: $out"; exit 1; }
printf '%s' "$out" | jq -e '[.notices[] | select(startswith("examples/demo/package.json"))] | length == 1' >/dev/null \
  || { echo "    the example's manifest is not a notice: $out"; exit 1; }
printf '%s' "$out" | jq -e '[.findings[] | select(.signal == "S4")] | length == 0' >/dev/null \
  || { echo "    the example's kit was counted: $out"; exit 1; }
expect S1 apps/web/package.json "a workspace wired and using nothing"

# A licensed marker (a Flowbite Pro distribution in the tree) in a repository that adopts
# no kit: the design licence is inactive (ui.kit is empty), so the marker is unlicensed.
pro=design/flowbite-pro-admin-dashboard-1.2.0/README.md
mkdir -p "$(dirname "$pro")"
printf 'the distribution\n' > "$pro"
out="$(kit)" || true
printf '%s' "$out" | jq -e '.licence.state == "inactive" and .licence.variant == "unlicensed"' >/dev/null \
  || { echo "    an unadopted kit's licensed marker is not unlicensed: $out"; exit 1; }
expect S1 "$pro" "a licensed marker with no kit adopted"

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
# The kit is the ui: block's (I4336), which the skeleton seeds with no kit: adoption sets
# ui.kit there, writes no other key under it and no design_kit block, and the adopted policy
# is one doctor reads.
ui_kit() { awk '/^ui:/ { u = 1; next } u && /^[^ #]/ { u = 0 } u && /^  kit: / { sub(/^  kit: /, ""); print }' .ai/repo/policy.yaml; }
[ "$(ui_kit)" = flowbite-pro ] || { echo "    adoption did not set ui.kit: $(cat .ai/repo/policy.yaml)"; exit 1; }
! grep -q '^design_kit:' .ai/repo/policy.yaml || { echo "    adoption wrote the retired design_kit block"; exit 1; }
grep -q '^  visibility: public$' .ai/repo/policy.yaml || { echo "    adoption lost the ui block's other keys"; exit 1; }
! grep -q '^  exempt:' .ai/repo/policy.yaml || { echo "    adoption wrote exemptions, which are the repository's"; exit 1; }
"$MJ" doctor >"$S/doctor" 2>&1 || true
grep -q 'parsed, version 1' "$S/doctor" || { echo "    doctor did not read the adopted policy: $(cat "$S/doctor")"; exit 1; }
! grep -q 'unknown keys' "$S/doctor" || { echo "    doctor refuses the adopted policy: $(grep 'unknown keys' "$S/doctor")"; exit 1; }
grep -q 'name: design-kit-on-push' .ai/repo/policy.yaml || { echo "    the pre-push entry is missing"; exit 1; }
grep -q 'args: \[design, kit, --check\]' .ai/repo/policy.yaml || { echo "    the entry does not run design kit --check"; exit 1; }
git add -A >/dev/null && git commit -qm adopt

out="$("$RB" design kit adopt flowbite-pro --format json 2>"$S/err")" \
  || { echo "    a second adoption failed: $(cat "$S/err")"; exit 1; }
printf '%s' "$out" | jq -e '[.writes[] | select(.action != "unchanged")] | length == 0' >/dev/null \
  || { echo "    a second adoption changed something: $out"; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "    a second adoption wrote: $(git status --porcelain)"; exit 1; }

out="$(kit)" && code=0 || code=$?
[ "$code" = 0 ] && [ "$(printf '%s' "$out" | verdict)" = clean ] || {
  echo "    the adopted repository is not clean under its own policy (exit $code): $out"; exit 1; }
printf '%s' "$out" | jq -e '.policy_declared and .profile == "flowbite-pro" and .licence.variant == "not_used"' >/dev/null \
  || { echo "    the adopted kit is not the one judged: $out"; exit 1; }

# Adopted, the tree is public, has a UI and holds no licence evidence: the design licence owes
# a judgement it cannot give, so a licensed marker is undecided, never a pass.
mkdir -p "$(dirname "$pro")"
printf 'the distribution\n' > "$pro"
out="$(kit)" || true
printf '%s' "$out" | jq -e '.licence.state == "not_judged" and .licence.variant == "undecided"' >/dev/null \
  || { echo "    the licence is not undecided: $out"; exit 1; }
expect S1 "$pro" "a licensed marker nobody could judge"

# The rule's last section is the repository's: over a stale rule, adoption brings the rest
# up to the template and keeps that section as the repository wrote it.
grep -q '^# In this repository$' "$rule" || { echo "    the adopted rule has no section of the repository's own"; exit 1; }
printf '\nThe site starts from the Pro admin shell.\n' >> "$rule"
sed -i.bak 's/^version: 1$/version: 0/' "$rule" && rm -f "$rule.bak"
out="$("$RB" design kit adopt flowbite-pro --format json 2>"$S/err")" \
  || { echo "    adoption over a stale rule failed: $(cat "$S/err")"; exit 1; }
printf '%s' "$out" | jq -e '.writes[0].action == "updated"' >/dev/null \
  || { echo "    a stale rule was not brought up to the template: $out"; exit 1; }
grep -q '^version: 1$' "$rule" || { echo "    the rule is still stale"; exit 1; }
grep -q '^The site starts from the Pro admin shell.$' "$rule" \
  || { echo "    adoption overwrote the repository's own section"; exit 1; }
out="$("$RB" design kit adopt flowbite-pro --format json 2>"$S/err")" || true
printf '%s' "$out" | jq -e '[.writes[] | select(.action != "unchanged")] | length == 0' >/dev/null \
  || { echo "    adoption after keeping the section changed something: $out"; exit 1; }
git checkout -q -- .

# ---------------------------------------------------------------- exemptions
# An exemption is the repository's own ui.exempt entry, exactly {path, because}: every finding
# at its path moves to the exempted list, and doctor accepts the entry the schema declares.
edit index.html '{ sub(/ aria-label="Breadcrumb"/, ""); print }'
edit .ai/repo/policy.yaml '{ print } /^  kit: flowbite-pro$/ { print "  exempt:"; print "    - path: index.html"; print "      because: the breadcrumb lives in the pages the shell frames" }'
out="$(kit)" && code=0 || code=$?
[ "$code" = 0 ] || { echo "    an exempted finding still refuses (exit $code): $out"; exit 1; }
printf '%s' "$out" | jq -e '(.exempted | length) == 1 and .exempted[0].finding.signal == "S5" and .exempted[0].because != "" and .exemptions[0].matched == 1' >/dev/null \
  || { echo "    the exemption is not listed with its reason: $out"; exit 1; }
printf '%s' "$out" | jq -e '.exemptions[0] | keys == ["because", "matched", "path"]' >/dev/null \
  || { echo    "    an exemption carries more than its path and reason: $out"; exit 1; }
"$MJ" doctor >"$S/doctor" 2>&1 || true
! grep -q 'unknown keys' "$S/doctor" || { echo "    doctor refuses ui.exempt: $(grep 'unknown keys' "$S/doctor")"; exit 1; }

# an exemption does not move a licence the judge cannot decide
mkdir -p "$(dirname "$pro")"
printf 'the distribution\n' > "$pro"
edit .ai/repo/policy.yaml '{ print } /^      because: the breadcrumb/ { print "    - path: design"; print "      because: kept for reference" }'
out="$(kit)" && code=0 || code=$?
[ "$code" = 10 ] || { echo "    an exemption moved an undecided licence (exit $code): $out"; exit 1; }
printf '%s' "$out" | jq -e '(.exempted | length) == 1 and ([.findings[] | select(.path | startswith("design/flowbite-pro-"))] | length) == 1' >/dev/null \
  || { echo "    the licence finding is not standing beside the one exemption: $out"; exit 1; }
rm -rf design

edit .ai/repo/policy.yaml '!/because: the breadcrumb/'
out="$(kit)" && code=0 || code=$?
[ "$code" = 2 ] || { echo "    an exemption without a because exited $code, not 2: $out"; exit 1; }
[ "$(printf '%s' "$out" | verdict)" = bad_policy ] || { echo "    the verdict is not bad_policy: $out"; exit 1; }
printf '%s' "$out" | jq -e '.policy_errors[0] | test("because")' >/dev/null \
  || { echo "    the reason does not name the missing because: $out"; exit 1; }
printf '%s' "$out" | jq -e '(.findings|length)==0 and (.exempted|length)==0 and (.surfaces|length)==0 and (.ui|not)' >/dev/null \
  || { echo "    a bad policy judged something: $out"; exit 1; }
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

# ---------------------------------------------------------------- published pages and notices
# A page a repository publishes is an interface whatever builds it; a manifest with UI
# packages that is no surface is a notice, and a vacuous pass is never a silent one.
P="$S/published"
mkdir -p "$P/site" "$P/examples/demo"
( cd "$P" && git init -q . )
printf '{"dependencies": {"react": "19"}}\n' > "$P/examples/demo/package.json"
out="$("$RB" design kit --check --repo "$P" --format json 2>"$S/err")" && code=0 || code=$?
[ "$code" = 0 ] || { echo "    a repository with only an example exited $code: $out"; exit 1; }
printf '%s' "$out" | jq -e '.verdict == "no_ui" and ([.notices[] | select(startswith("examples/demo/package.json"))] | length) == 1' >/dev/null \
  || { echo "    the example's manifest is not a notice of a vacuous pass: $out"; exit 1; }
printf '<p>about</p>\n' > "$P/site/about.html"
out="$("$RB" design kit --repo "$P" --format json 2>"$S/err")" || true
printf '%s' "$out" | jq -e '.ui and ([.ui_evidence[] | select(test("published page"))] | length) == 1' >/dev/null \
  || { echo "    a published page is not an interface: $out"; exit 1; }

# docs/ is published by a gh-pages publish_dir step, and not by a checkout whose path is docs
D="$S/docs"
mkdir -p "$D/docs" "$D/.github/workflows"
( cd "$D" && git init -q . )
printf '<p>manual</p>\n' > "$D/docs/index.html"
printf 'jobs:\n  b:\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          path: docs\n' > "$D/.github/workflows/wiki.yml"
out="$("$RB" design kit --repo "$D" --format json 2>"$S/err")" || true
[ "$(printf '%s' "$out" | verdict)" = no_ui ] || { echo "    a checkout's path published docs/: $out"; exit 1; }
printf 'jobs:\n  p:\n    steps:\n      - uses: peaceiris/actions-gh-pages@v4\n        with:\n          publish_dir: ./docs\n' > "$D/.github/workflows/pages.yml"
out="$("$RB" design kit --repo "$D" --format json 2>"$S/err")" || true
printf '%s' "$out" | jq -e '.ui and ([.ui_evidence[] | select(test("docs/index.html"))] | length) == 1' >/dev/null \
  || { echo "    a publish_dir step does not publish docs/: $out"; exit 1; }

# a Git repository nested in the tree is named, and judged in its own repository
mkdir -p "$D/web"
( cd "$D/web" && git init -q . )
printf '<p>web</p>\n' > "$D/web/index.html"
out="$("$RB" design kit --repo "$D" --format json 2>"$S/err")" || true
printf '%s' "$out" | jq -e '[.notices[] | select(startswith("web is") and test("judged in its own repository"))] | length == 1' >/dev/null \
  || { echo "    the nested repository is not named in a notice: $out"; exit 1; }
true
