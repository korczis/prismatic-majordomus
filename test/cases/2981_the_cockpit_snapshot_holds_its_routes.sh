# majordomus-covers: none
# The Cockpit as a publication carries it (I2495, ADR 0147): `majordomus web report
# cockpit-snapshot` reads the Cockpit once, in-process, with the exporter that reads the UI,
# and writes it as files under target/web/cockpit-snapshot.
#
#   its routes      the overview and the capability listing are files of it, and every relative
#                   link of every page names a file the snapshot holds
#   what it is      every page says the commit it was read at and carries no script: the
#                   Cockpit's scripts open sockets, poll and post to a process
#   not this disk   the one page about this machine, /cockpit/continuity, is in no snapshot,
#                   and the navigation's link to it leads to the page about the live Cockpit
#   a surface       the declaration mounts it at /cockpit for a publication and for no process,
#                   names this commit, and the topology holds it without a conflict
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
S=target/web/cockpit-snapshot

"$RB" web report cockpit-snapshot --limit 80 > out.txt 2> err.txt \
  || { echo "    the snapshot was not written:"; tail -5 err.txt | sed 's/^/    | /'; exit 1; }
grep -q "read at $commit" out.txt || { echo "    the run does not say which commit it read:"; sed 's/^/    | /' out.txt; exit 1; }

# ---------------------------------------------------------------- its routes
for page in index.html capabilities/index.html; do
  [ -f "$S/$page" ] || { echo "    the snapshot does not hold $page"; exit 1; }
done
pages="$(jq -r '.pages | length' "$S/snapshot.json")"
[ "$pages" -le 80 ] && [ "$pages" -ge 10 ] || { echo "    the snapshot records $pages page(s) for a bound of 80"; exit 1; }
[ "$(find "$S" -name index.html | wc -l | tr -d ' ')" = "$(( pages + 1 ))" ] \
  || { echo "    the files are not the pages the snapshot records and its one page about the live Cockpit"; exit 1; }
for f in $(jq -r '.pages[]' "$S/snapshot.json") live/index.html; do
  dir="$(dirname "$S/$f")"
  grep -oE '(href|src)="[^"#]+' "$S/$f" | sed 's/^[a-z]*="//' | LC_ALL=C sort -u > links.txt || true
  while IFS= read -r link; do
    case "$link" in
      /cockpit|/cockpit/*) echo "    $f keeps an absolute link into the Cockpit: $link"; exit 1 ;;
      /*|*://*|mailto:*|data:*) continue ;;
    esac
    path="${link%%\?*}"
    [ -e "$dir/$path" ] || { echo "    $f links $link, which the snapshot does not hold"; exit 1; }
  done < links.txt
done

# ---------------------------------------------------------------- what it is
for f in $(jq -r '.pages[]' "$S/snapshot.json"); do
  grep -q "read at commit $commit" "$S/$f" || { echo "    $f does not say the commit it was read at"; exit 1; }
  grep -q '<body data-snapshot ' "$S/$f" || { echo "    $f is not marked as a snapshot"; exit 1; }
  if grep -qE '<script[^>]*src=' "$S/$f"; then echo "    $f carries a script that would talk to a process"; exit 1; fi
done

# ---------------------------------------------------------------- not this disk
if jq -e '.pages | keys | any(startswith("/cockpit/continuity"))' "$S/snapshot.json" >/dev/null; then
  echo "    the snapshot holds /cockpit/continuity, the one page about this machine"; exit 1
fi
[ ! -e "$S/continuity" ] || { echo "    a continuity directory was written into the snapshot"; exit 1; }
grep -q 'live/index.html#/cockpit/continuity' "$S/index.html" \
  || { echo "    the navigation's link to the withheld page does not lead to the page about the live Cockpit"; exit 1; }

# ---------------------------------------------------------------- a surface
jq -e --arg c "$commit" '.id == "cockpit-snapshot" and .mount == "/cockpit" and .availability == "published-only"
                         and .built_from == $c and .schema == "web-surface/v1"' "$S/surface.json" >/dev/null \
  || { echo "    the declaration is not a published-only surface at /cockpit built from $commit:"; sed 's/^/    | /' "$S/surface.json"; exit 1; }
"$RB" web list --format json > web.json 2>/dev/null || { echo "    the topology does not resolve with the snapshot in it"; exit 1; }
jq -e '[.surfaces[] | select(.mount == "/cockpit")] | map(.id) | sort == ["cockpit", "cockpit-snapshot"]' web.json >/dev/null \
  || { echo "    /cockpit is not held by the Cockpit for a process and by its snapshot for a publication:"; jq -c '[.surfaces[] | select(.mount == "/cockpit") | {id, availability}]' web.json | sed 's/^/    | /'; exit 1; }
"$RB" web validate > validate.txt 2>&1 \
  || { echo "    the topology refuses the snapshot beside the Cockpit:"; sed 's/^/    | /' validate.txt; exit 1; }

echo "    the Cockpit's snapshot holds its routes with no script and no page about this machine, and is a surface of a publication at /cockpit"
