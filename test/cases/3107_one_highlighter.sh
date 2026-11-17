# majordomus-covers: none
# majordomus-timeout: 300
# Every code block on every surface is highlighted by one engine with one palette (I2747,
# ADR 0166 §7). Zola 0.23 highlights the site with giallo; the executable embeds the same
# giallo (crate::highlight) for the Cockpit, the UI and the reports.
#
#   1. the same block, rendered by the executable's server on an object page and by Zola
#      under the site's own [markdown.highlighting] section and generated theme, carries the
#      same markup: the <code> element is byte for byte the same, and it is highlighted
#      (several kinds of token, each a class);
#   2. every class either surface writes is coloured by share/design/code.css, which every
#      compiled stylesheet carries, and the generated theme is the one the executable writes;
#   3. scripts/ci/highlighter-check holds on this tree, and refuses a planted highlight.js
#      reference, naming the file, while the same fixture without it passes.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
command -v curl >/dev/null 2>&1 || skip "curl absent"
command -v zola >/dev/null 2>&1 || skip_case "zola is absent, so the site's half of the comparison was not measured"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share" MAJORDOMUS_BIN="$RB"
trap serve_down EXIT

fail=0
note() { echo "    $1"; fail=1; }
# the inner HTML of the first <code data-lang="yaml"> element of a page
code_of() {
  python3 -c '
import re, sys
m = re.search(r"<code data-lang=\"yaml\">(.*?)</code>", open(sys.argv[1], encoding="utf-8").read(), re.S)
sys.stdout.write(m.group(1) if m else "")' "$1"
}

# ------------------------------------------------------------- the block, in a fixture layer
R="$T/repo"; mkdir -p "$R"; cd "$R" || exit 1
git init -q . && git config user.email t@example.com && git config user.name t
"$MJ" init >/dev/null
# a profile is a YAML object `init` writes: keys, strings, numbers and booleans; a comment is
# added so that one more kind of token is drawn
F_YAML=.ai/repo/profiles/debugging.yaml
[ -f "$F_YAML" ] || { echo "    init wrote no $F_YAML"; exit 1; }
printf '# drawn by one engine\n' >> "$F_YAML"
git add -A >/dev/null && git commit -qm fixture

# ------------------------------------------------------------- 1a. the executable's rendering
uri="$("$RB" entity show majordomus://profile/debugging --repo "$R" --format json 2>"$T/show.err" | jq -r ".route // empty")"
[ -n "$uri" ] || { echo "    entity show names no route for the profile:"; sed "s/^/      /" "$T/show.err" | tail -5; ls .ai/repo/profiles; exit 1; }
unset MAJORDOMUS_HTTP_HOST   # serve_up waits for the loopback address
serve_up "$T/serve.out" "$T/serve.err" || exit 1
curl -fsS -m 30 "$U$uri" -o "$T/ui.html" || { echo "    $U$uri did not answer"; exit 1; }
serve_down
ui="$(code_of "$T/ui.html")"
[ -n "$ui" ] || { echo "    the object page draws no yaml code block:"; grep -o '<pre[^>]*>' "$T/ui.html" | head -3 | sed 's/^/      /'; exit 1; }

# ------------------------------------------------------------- 1b. the site's rendering
S="$T/site"; mkdir -p "$S/content" "$S/templates" "$S/highlighting"
python3 - "$ROOT/site/config.toml" > "$S/config.toml" <<'PY'
import re, sys
c = open(sys.argv[1], encoding="utf-8").read()
m = re.search(r"^\[markdown\.highlighting\]\n(?:(?!\[).*\n)*", c, re.M)
print('base_url = "https://example.invalid"\nbuild_search_index = false\n\n' + (m.group(0) if m else ""))
PY
grep -q '^theme = "majordomus"' "$S/config.toml" || note "the site does not highlight with the generated theme"
cp "$ROOT/site/highlighting/majordomus.json" "$S/highlighting/"
printf '{{ section.content | safe }}' > "$S/templates/index.html"
{ printf '+++\n+++\n\n```yaml\n'; cat "$F_YAML"; printf '```\n'; } > "$S/content/_index.md"
( cd "$S" && zola build --force --output-dir "$S/out" >/dev/null 2>"$T/zola.err" ) \
  || { echo "    zola could not build the fixture site:"; sed 's/^/      /' "$T/zola.err"; exit 1; }
site="$(code_of "$S/out/index.html")"
[ -n "$site" ] || { echo "    the fixture site drew no yaml block"; exit 1; }

# ------------------------------------------------------------- 1c. one markup
if [ "$ui" != "$site" ]; then
  note "the object page and the site highlight the same block differently"
  printf '%s\n' "$ui" > "$T/ui.code"; printf '%s\n' "$site" > "$T/site.code"
  diff "$T/site.code" "$T/ui.code" | head -10 | sed 's/^/      /'
fi
kinds="$(printf '%s' "$site" | grep -oE 'class="z-[0-9]+' | sort -u | wc -l | tr -d ' ')"
[ "$kinds" -ge 3 ] || note "the block is not highlighted: $kinds kind(s) of token"
grep -q '<pre tabindex="0" class="mj-pre giallo z-code"' "$T/ui.html" || note "the object page's block is not the engine's frame"

# ------------------------------------------------------------- 2. one palette
for c in $(printf '%s' "$site" | grep -oE 'z-[0-9a-z]+' | sort -u); do
  grep -q "^\.$c {" "$ROOT/share/design/code.css" || note "class $c is coloured by nothing in share/design/code.css"
done
for sheet in share/ui/ui.css share/cockpit/cockpit.css; do
  grep -q 'var(--mj-code-keyword)' "$ROOT/$sheet" || note "$sheet does not carry the code stylesheet"
done
for sheet in site/tailwind.css site/kit.tailwind.css; do
  grep -q 'share/design/code.css' "$ROOT/$sheet" || note "$sheet does not import the code stylesheet"
done
grep -rqE 'giallo-(light|dark)\.css' "$ROOT/site/templates" && note "a site template links a second palette"

# ------------------------------------------------------------- 3. the gate
GATE="$ROOT/scripts/ci/highlighter-check"
out="$(bash "$GATE" 2>&1)" || { note "highlighter-check refuses this tree:"; printf '%s\n' "$out" | tail -8 | sed 's/^/      /'; }
F="$T/fixture"; mkdir -p "$F/site/templates" "$F/share/ui"
printf '<pre><code>plain</code></pre>\n' > "$F/site/templates/page.html"
printf 'body {}\n' > "$F/share/ui/ui.css"
MJ_ROOT="$F" bash "$GATE" > "$T/gate.clean" 2>&1 || { note "the gate refuses a fixture with no highlighter:"; sed 's/^/      /' "$T/gate.clean"; }
printf '<script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js"></script>\n' >> "$F/site/templates/page.html"
rc=0; MJ_ROOT="$F" bash "$GATE" > "$T/gate.planted" 2>&1 || rc=$?
[ "$rc" = 10 ] || note "a planted highlight.js reference exits $rc, not 10"
grep -q '^site/templates/page.html:2' "$T/gate.planted" || note "the refusal does not name the planted file and line"

[ "$fail" = 0 ] || exit 1
echo "    one engine: the object page and the site draw the same block with $kinds kinds of token; a planted highlight.js is refused"
