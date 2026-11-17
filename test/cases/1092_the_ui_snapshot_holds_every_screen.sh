# majordomus-covers: none
# The UI as a publication carries it (I2483, ADR 0141): `majordomus web report ui-snapshot`
# reads the UI once, in-process, and writes it as files under target/web/ui-snapshot.
#
#   every screen    each screen the snapshot's own navigation names is a file of it, or the
#                   page about the live UI for a screen the snapshot withholds
#   what it is      every page says the commit it was read at, and tells the script so
#   a surface       the declaration beside it mounts it at /ui for a publication and for no
#                   process, names this commit, and the topology holds it without a conflict
#   one reading     a second run with a smaller bound leaves no page of the first behind
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
commit="$(git rev-parse --short HEAD)"
S=target/web/ui-snapshot

"$RB" web report ui-snapshot --limit 60 > out.txt 2> err.txt \
  || { echo "    the snapshot was not written:"; tail -5 err.txt | sed 's/^/    | /'; exit 1; }
grep -q "read at $commit" out.txt || { echo "    the run does not say which commit it read:"; sed 's/^/    | /' out.txt; exit 1; }

# ---------------------------------------------------------------- every screen
[ -f "$S/index.html" ] || { echo "    the snapshot has no dashboard at its root"; exit 1; }
grep -oE 'class="mj-ui-nav-item[^"]*"[^>]*href="[^"]*"' "$S/index.html" | sed 's/.*href="//; s/"$//' > nav.txt
[ "$(wc -l < nav.txt | tr -d ' ')" -ge 5 ] || { echo "    the dashboard's navigation names fewer than five screens:"; sed 's/^/    | /' nav.txt; exit 1; }
while IFS= read -r href; do
  case "$href" in /*|*://*) echo "    the navigation keeps a link a file cannot answer: $href"; exit 1 ;; esac
  # a screen the snapshot withholds is linked to the page about the live UI, with the path
  # it meant after `#`: the file is that page
  [ -f "$S/${href%%#*}" ] || { echo "    the navigation names $href, which the snapshot does not hold"; exit 1; }
done < nav.txt
pages="$(jq -r '.pages | length' "$S/snapshot.json")"
[ "$pages" -le 60 ] && [ "$pages" -ge "$(wc -l < nav.txt | tr -d ' ')" ] \
  || { echo "    the snapshot records $pages page(s) for a bound of 60 and $(wc -l < nav.txt | tr -d ' ') screen(s)"; exit 1; }
redirects="$(jq -r '.redirects | length' "$S/snapshot.json")"
[ "$(find "$S" -name index.html | wc -l | tr -d ' ')" = "$(( pages + 1 + redirects ))" ] \
  || { echo "    the files are not the pages the snapshot records, its one page about the live UI and the pages for the names it redirects"; exit 1; }

# ---------------------------------------------------------------- what it is
for f in $(jq -r '.pages[]' "$S/snapshot.json"); do
  grep -q "read at commit $commit" "$S/$f" || { echo "    $f does not say the commit it was read at"; exit 1; }
  grep -q '<body data-snapshot ' "$S/$f" || { echo "    $f does not tell the script it is a snapshot"; exit 1; }
done
grep -q "'snapshot' in document.body.dataset" "$S/assets/ui.js" \
  || { echo "    the script the snapshot carries does not know a snapshot; it would read the page again"; exit 1; }

# ---------------------------------------------------------------- a surface
jq -e --arg c "$commit" '.id == "ui-snapshot" and .mount == "/ui" and .availability == "published-only"
                         and .built_from == $c and .schema == "web-surface/v1"' "$S/surface.json" >/dev/null \
  || { echo "    the declaration is not a published-only surface at /ui built from $commit:"; sed 's/^/    | /' "$S/surface.json"; exit 1; }
"$RB" web list --format json > web.json 2>/dev/null || { echo "    the topology does not resolve with the snapshot in it"; exit 1; }
jq -e '[.surfaces[] | select(.mount == "/ui")] | map(.id) | sort == ["ui", "ui-snapshot"]' web.json >/dev/null \
  || { echo "    /ui is not held by the UI for a process and by its snapshot for a publication:"; jq -c '[.surfaces[] | select(.mount == "/ui") | {id, availability}]' web.json | sed 's/^/    | /'; exit 1; }
"$RB" web validate > validate.txt 2>&1 \
  || { echo "    the topology refuses the snapshot beside the UI:"; sed 's/^/    | /' validate.txt; exit 1; }

# ---------------------------------------------------------------- a face
# The process that reads a snapshot is no server: it holds no board and runs no mesh. The
# figures the dashboard leads with are still decided, every one, and none counts a zero for
# what nobody asked.
if grep -oE 'class="mj-ui-kpi-value">(<span class="mj-badge mj-badge--unknown"|not answered<)' "$S/index.html" | grep -q .; then
  echo "    the snapshot's dashboard leads with a figure nothing decided:"
  grep -oE 'class="mj-ui-kpi-label">[^<]*</span><span class="mj-ui-kpi-value">(<span class="mj-badge mj-badge--unknown"|not answered<)' "$S/index.html" | sed 's/^/    | /'
  exit 1
fi
grep -q 'title="read from health.report /repository"' "$S/index.html" \
  || { echo "    the dashboard's health figure is not the repository's verdict"; exit 1; }
grep -q 'class="mj-ui-kpi-label">Mesh machines<' "$S/index.html" \
  && { echo "    the dashboard counts mesh machines in a process that runs no mesh"; exit 1; }

# ---------------------------------------------------------------- one reading
"$RB" web report ui-snapshot --limit 3 >/dev/null 2>&1 || { echo "    a second run failed"; exit 1; }
[ "$(find "$S" -name index.html | wc -l | tr -d ' ')" = 4 ] \
  || { echo "    a run bounded at 3 pages left $(find "$S" -name index.html | wc -l | tr -d ' ') files: pages of the earlier reading survived"; exit 1; }

echo "    the snapshot holds every screen its navigation names, says the commit it read, and is a surface of a publication at /ui"
