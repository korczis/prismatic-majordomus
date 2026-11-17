# majordomus-covers: none
# A snapshot of the UI (I2483, ADR 0141) is files: a host answers a path with a file or with
# nothing, and ignores a query. So no page of it may keep a link only a process answers.
#
#   no absolute UI link   no href, src or action of any page starts with /ui
#   nothing absent        every relative link names a file the snapshot holds
#   the live UI           a link to the Cockpit, the API or a form leads to the snapshot's one
#                         page about the live UI, carrying the path it meant
#   and it is checked     a page given an absolute /ui link fails this case's own reading
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
S="$PWD/target/web/ui-snapshot"
"$RB" web report ui-snapshot --limit 40 >/dev/null 2> err.txt \
  || { echo "    the snapshot was not written:"; tail -5 err.txt | sed 's/^/    | /'; exit 1; }

# every link of every page, as "file<TAB>link": the attributes a reader or the script follows
links() {
  find "$1" -name '*.html' | LC_ALL=C sort | while IFS= read -r f; do
    grep -oE '(href|src|action|data-graph-lib|data-observatory-lib|data-panel-src)="[^"]*"' "$f" \
      | sed -e 's/^[a-z-]*="//' -e 's/"$//' -e 's/&amp;/\&/g' \
      | while IFS= read -r l; do printf '%s\t%s\n' "$f" "$l"; done
  done
}
# the links of a tree that a file cannot answer: an absolute path under the UI's mount, or a
# relative one whose file is absent
unanswered() {
  links "$1" | while IFS="$(printf '\t')" read -r f l; do
    case "$l" in
      ''|'#'*|*://*|mailto:*) continue ;;
      /ui|/ui/*|/ui'?'*) printf '%s keeps the absolute link %s\n' "${f#"$1"/}" "$l" ;;
      /*) continue ;;
      *) p="${l%%#*}"; p="${p%%\?*}"
         [ -e "$(dirname "$f")/$p" ] || printf '%s links %s, which the snapshot does not hold\n' "${f#"$1"/}" "$l" ;;
    esac
  done
}

n="$(links "$S" | wc -l | tr -d ' ')"
[ "$n" -ge 50 ] || { echo "    only $n link(s) were read from the snapshot; the reading is not looking at the pages"; exit 1; }

# ---------------------------------------------------------------- no absolute UI link, nothing absent
unanswered "$S" > bad.txt
[ -s bad.txt ] && { echo "    $(wc -l < bad.txt | tr -d ' ') link(s) of the snapshot lead to nothing:"; head -8 bad.txt | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- the live UI
[ -f "$S/live/index.html" ] || { echo "    the snapshot has no page about the live UI"; exit 1; }
grep -q 'majordomus serve' "$S/live/index.html" || { echo "    the page about the live UI does not say how to get one"; exit 1; }
links "$S" | cut -f2 | grep -q 'live/index.html#/cockpit' \
  || { echo "    no link to the Cockpit leads to the page about the live UI: either the frame lost it or it still points at a process"; exit 1; }
grep -oE 'action="[^"]*"' "$S/index.html" | grep -vq 'live/index.html#' \
  && { echo "    a form of the dashboard still asks a process:"; grep -oE 'action="[^"]*"' "$S/index.html" | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- and it is checked
mkdir -p planted/x && cp "$S/index.html" planted/index.html
printf '<a href="/ui/roadmap">r</a><a href="x/absent.html">a</a>\n' >> planted/index.html
unanswered "$PWD/planted" > planted.txt
grep -q 'keeps the absolute link /ui/roadmap' planted.txt && grep -q 'x/absent.html' planted.txt \
  || { echo "    a planted absolute link and a planted absent file were not both found; this case reads nothing:"; sed 's/^/    | /' planted.txt | head -5; exit 1; }

echo "    $n link(s) of the snapshot: none absolute under /ui, none to an absent file, and what only a process answers leads to the page about the live UI"
