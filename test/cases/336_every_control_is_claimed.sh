# majordomus-covers: none
# Every interactive control on the site is claimed by exactly one behaviour spec: scripts/ci/interaction-check,
# run against a built site and a set of specs this case writes, so the answer for every control is known
# before the check runs. Rule: project.every-link-and-control-is-tested.
#
# What is proved: a clean site passes and counts each spec's controls; a control no spec claims fails and is
# named; a control two specs claim fails; a spec that claims nothing on the site fails; a disabled element is
# not a control; a control inside <template> (rendered later by a script) is still counted; an attribute value
# holding ">" (an Alpine arrow function) does not end the tag early and hide the control; and an Alpine expression
# that does not compile fails, naming the page and the directive. And the controls judged are the site's own: a
# surface the topology composes into the build at /ref (ADR 0086) carries controls no spec here claims and the
# clean site still passes; the same page under a mount the topology does not compose is judged and fails; with no
# topology the check refuses to decide (12) rather than guess which pages are the site's.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || { echo "    skip: no node"; exit 0; }
CHECK="$ROOT/scripts/ci/interaction-check"

F="$PWD/site-fixture"; mkdir -p "$F/site/public/tools" "$F/site/public/ref/api" "$F/docs/generated" specs
COMPOSING='{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"},{"id":"ref","kind":"static-directory","mount":"/ref","artifact":"target/web/ref","availability":"both"}]}'
printf '%s\n' "$COMPOSING" > "$F/docs/generated/web.json"
# the composed surface's own page: a control of a kind no spec below claims
printf '<!doctype html><html><body><button type="button" id="ref-search-toggle">search</button></body></html>\n' > "$F/site/public/ref/api/index.html"
cat > "$F/site/public/index.html" <<'HTML'
<!doctype html><html><body>
<button type="button" id="theme-toggle">theme</button>
<button type="button" x-on:click="navigator.clipboard.writeText($refs.src.innerText).then(() => { copied = true })">Copy</button>
<input type="checkbox" disabled aria-label="a task list item nobody can tick">
<details><summary>more</summary><p>text</p></details>
</body></html>
HTML
cat > "$F/site/public/tools/index.html" <<'HTML'
<!doctype html><html><body>
<div x-data="{ q: '' }"><input type="search" x-model="q"><template x-if="q"><button type="button" x-on:click="q = ''">clear</button></template></div>
</body></html>
HTML
spec() { printf 'export default { id: %s, title: %s, claims: (el) => %s, async exercise() { return 0; } };\n' "'$1'" "'$1'" "$2" > "specs/$1.mjs"; }
spec theme "el.attrs.id === 'theme-toggle'"
spec copy "/clipboard/.test(el.attrs['x-on:click'] || '')"
spec disclosure "el.tag === 'summary'"
spec search "el.attrs['x-model'] === 'q'"
spec clear "el.attrs['x-on:click'] === \"q = ''\""
run() { rc=0; MJ_ROOT="$F" MJ_INTERACTION_SPECS="$PWD/specs" "$CHECK" "$@" > out.txt 2>&1 || rc=$?; }

# ---------------------------------------------------------------- clean
run
[ "$rc" = 0 ] || { echo "    a fully claimed site did not pass"; cat out.txt; exit 1; }
expect_grep 'every one of 5 control\(s\) is claimed by exactly one of 5 behaviour spec\(s\), and every Alpine expression compiles' out.txt
expect_grep '1 clear, 1 copy, 1 disclosure, 1 search, 1 theme' out.txt       # the template's button is counted
run --inventory
expect_grep 'x-on:click="navigator.clipboard.writeText' out.txt              # the arrow function did not end the tag
expect_no_grep 'disabled' out.txt                                            # nobody can operate it, so it is no control

# ---------------------------------------------------------------- an unclaimed control
rm specs/copy.mjs; run
[ "$rc" = 10 ] || { echo "    an unclaimed control exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep 'FAIL control .*/: <button type="button" x-on:click=.* is claimed by no behaviour spec' out.txt
spec copy "/clipboard/.test(el.attrs['x-on:click'] || '')"

# ---------------------------------------------------------------- a control two specs claim
spec greedy "el.tag === 'summary'"; run
[ "$rc" = 10 ] || { echo "    a doubly claimed control exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep 'FAIL control .*<summary> is claimed by disclosure and greedy' out.txt
rm specs/greedy.mjs

# ---------------------------------------------------------------- a spec that claims nothing
spec dead "el.tag === 'marquee'"; run
[ "$rc" = 10 ] || { echo "    a dead spec exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep 'FAIL control .*spec dead \(dead.mjs\) claims no control on the site' out.txt
rm specs/dead.mjs

# ---------------------------------------------------------------- an Alpine expression that does not compile
# the defect this found on the real site: text interpolated into a JavaScript string, broken by an apostrophe,
# which Alpine drops without a sound. The second directive holds an arrow function, which must still compile.
cat > "$F/site/public/tools/index.html" <<'HTML'
<!doctype html><html><body>
<div x-data="{ q: '' }"><input type="search" x-model="q"><template x-if="q"><button type="button" x-on:click="q = ''">clear</button></template>
<ul><li x-show="'a record's changed files'.includes(q)" x-on:mouseenter="() => { q = q }">row</li></ul></div>
</body></html>
HTML
run
[ "$rc" = 10 ] || { echo "    a broken Alpine expression exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep "FAIL control .*/tools/: the x-show expression on a <li> does not compile" out.txt
expect_no_grep 'x-on:mouseenter expression' out.txt
cat > "$F/site/public/tools/index.html" <<'HTML'
<!doctype html><html><body>
<div x-data="{ q: '' }"><input type="search" x-model="q"><template x-if="q"><button type="button" x-on:click="q = ''">clear</button></template></div>
</body></html>
HTML

# ---------------------------------------------------------------- the composed surface is not the site
# the clean runs above already carried /ref/api/ and its unclaimed button; the exemption is the topology's alone
printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' > "$F/docs/generated/web.json"
run
[ "$rc" = 10 ] || { echo "    a control under a mount the topology does not compose exited $rc, not 10"; cat out.txt; exit 1; }
expect_grep 'FAIL control .*/ref/api/: <button type="button"> is claimed by no behaviour spec' out.txt
rm "$F/docs/generated/web.json"; run
[ "$rc" = 12 ] || { echo "    no topology exited $rc, not 12"; cat out.txt; exit 1; }
expect_grep 'web.json is missing' out.txt
printf '%s\n' "$COMPOSING" > "$F/docs/generated/web.json"; run
[ "$rc" = 0 ] || { echo "    the composed surface was judged again once the topology was back"; cat out.txt; exit 1; }

# ---------------------------------------------------------------- a malformed spec is refused, not skipped
printf 'export default { id: "broken" };\n' > specs/broken.mjs; run
[ "$rc" != 0 ] || { echo "    a spec without claims() or exercise() was accepted"; cat out.txt; exit 1; }
expect_grep 'a spec exports default \{ id, title, claims\(el\), exercise\(ctx\) \}' out.txt
