# majordomus-covers: none
# A published snapshot names no machine (I1012). The live Cockpit and UI show the checkout's
# root and paths under it; on 2026-10-21 majordomus.dev/cockpit/ and /ui/ carried the CI
# runner's absolute root on their pages, because the exporter wrote what the process
# answered. `majordomus web report cockpit-snapshot` and `ui-snapshot` now write every
# spelling of the repository's root and of the home directory as `<repo>` and `<home>`,
# escaped for HTML.
#
#   no root     no file of either snapshot holds the fixture's root, as given or resolved,
#               or the home directory
#   still said  the Cockpit's overview still says where the repository is: `<repo>`, escaped,
#               so the fact is kept and the page parses as it did
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

for surface in cockpit ui; do
  "$RB" web report "$surface-snapshot" --limit 40 > out.txt 2> err.txt \
    || { echo "    the $surface snapshot was not written:"; tail -5 err.txt | sed 's/^/    | /'; exit 1; }
done

# ---------------------------------------------------------------- no root
for spelling in "$T" "$(cd "$T" && pwd -P)" "${HOME%/}"; do
  [ -n "$spelling" ] && [ "$spelling" != / ] || continue
  if grep -rlF -- "$spelling" target/web/cockpit-snapshot target/web/ui-snapshot > leaks.txt; then
    echo "    $(wc -l < leaks.txt | tr -d ' ') page(s) name $spelling, first: $(head -n 1 leaks.txt)"
    exit 1
  fi
done
echo "    ok: no page of either snapshot names the root or the home directory"

# ---------------------------------------------------------------- still said
grep -qF '&lt;repo&gt;' target/web/cockpit-snapshot/index.html \
  || { echo "    the Cockpit overview no longer says where the repository is"; exit 1; }
if grep -qF '<repo>' target/web/cockpit-snapshot/index.html; then
  echo "    the overview writes <repo> unescaped, where a browser reads an element"; exit 1
fi
echo "    ok: the overview's root reads <repo>, escaped"
