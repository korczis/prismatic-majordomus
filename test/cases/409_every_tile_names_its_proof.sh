# majordomus-covers: none
# A tile that shows a fact names the test that proves it, and the reader is one click away.
#
# The declaration is site/data/proofs.toml and the renderer is site/templates/partials/proof.html,
# so a new tile inherits the link by naming its key. This case holds both directions: a
# declared path that does not exist fails, and a tile whose subject has a test but renders
# no link fails. A tile that states no fact of its own declares `none` and a reason — that
# is the only accepted way to carry no proof, so "no proof" cannot become the silent default.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }

D="$ROOT/site/data/proofs.toml"
[ -f "$D" ] || { echo "    no site/data/proofs.toml: nothing declares what proves a tile"; exit 1; }

# 1. Every declaration is well formed: exactly one of `tests` or `none`, and every path it
#    names exists. A link to a file that is not there is worse than no link.
tiles="$(sed -n 's/^\[tiles\.\([a-z0-9-]*\)\]$/\1/p' "$D")"
[ -n "$tiles" ] || { echo "    proofs.toml declares no tile"; exit 1; }
n_tests=0
n_none=0
for t in $tiles; do
  block="$(awk -v k="[tiles.$t]" '$0==k{f=1;next} /^\[/{f=0} f' "$D")"
  has_tests=0; has_none=0
  printf '%s\n' "$block" | grep -q '^tests = ' && has_tests=1
  printf '%s\n' "$block" | grep -q '^none = ' && has_none=1
  [ "$has_tests" = 1 ] && [ "$has_none" = 1 ] && { echo "    tile $t declares both tests and none"; exit 1; }
  [ "$has_tests" = 0 ] && [ "$has_none" = 0 ] && { echo "    tile $t declares neither tests nor none"; exit 1; }
  if [ "$has_tests" = 1 ]; then
    n_tests=$((n_tests + 1))
    printf '%s\n' "$block" | grep -q '^runs = "..*"$' || { echo "    tile $t names tests but does not say what they run"; exit 1; }
    for p in $(printf '%s\n' "$block" | sed -n 's/^tests = \[\(.*\)\]$/\1/p' | tr -d '" ' | tr ',' ' '); do
      [ -f "$ROOT/$p" ] || { echo "    tile $t names $p, which is not in the tree"; exit 1; }
    done
  else
    n_none=$((n_none + 1))
    reason="$(printf '%s\n' "$block" | sed -n 's/^none = "\(.*\)"$/\1/p')"
    [ -n "$reason" ] || { echo "    tile $t declares none with no reason"; exit 1; }
    # The only accepted class: the tile states no fact of its own.
    printf '%s\n' "$reason" | grep -q 'states no fact of its own' \
      || { echo "    tile $t: '$reason' is not the accepted reason (a tile with no proof states no fact of its own)"; exit 1; }
  fi
done
# 2. Every template that names a tile key names one that is declared. A key with a typo
#    would render nothing at all, silently.
for k in $(grep -rhoE 'set tile = "[a-z0-9-]+"' "$ROOT/site/templates" | sed 's/.*"\(.*\)"/\1/' | sort -u); do
  printf '%s\n' "$tiles" | grep -qx "$k" || { echo "    a template names tile '$k', which proofs.toml does not declare"; exit 1; }
done

# 3. The rendered site: every surface whose tile declares tests carries the link. Without a
#    build there is nothing to read, and a case that cannot reach its subject says so.
P="$ROOT/site/public"
[ -f "$P/index.html" ] || { echo "    site not built; skipping the rendered half"; exit 0; }

cli_test="apps/majordomus-cli/tests/cli_examples.rs"
api_test="apps/majordomus-cli/tests/http_serve.rs"
cmd_test="test/cases/34_command_fixtures.sh"
uc_test="test/cases/94_use_cases.sh"

# every command-line page that shows an example names the test that runs it
pages=0; linked=0
for f in $(find "$P/docs/cli" -name index.html 2>/dev/null); do
  [ -f "$f" ] || continue
  grep -q 'id="example-' "$f" || continue
  pages=$((pages + 1))
  grep -q "$cli_test" "$f" && linked=$((linked + 1))
done
[ "$pages" -gt 0 ] || { echo "    no command-line page shows an example; the corpus moved"; exit 1; }
[ "$pages" = "$linked" ] || { echo "    $pages command page(s) show examples, $linked name $cli_test"; exit 1; }

# every API operation page names the test that replays its routes
apis=0; apis_linked=0
for f in "$P"/docs/api/*/index.html; do
  [ -f "$f" ] || continue
  grep -q 'Try it' "$f" || continue
  apis=$((apis + 1))
  grep -q "$api_test" "$f" && apis_linked=$((apis_linked + 1))
done
[ "$apis" -gt 0 ] || { echo "    no API page shows an operation; the corpus moved"; exit 1; }
[ "$apis" = "$apis_linked" ] || { echo "    $apis API page(s), $apis_linked name $api_test"; exit 1; }

# every shell command page names the test that replays its fixture
cmds=0; cmds_linked=0
for f in "$P"/commands/*/index.html; do
  [ -f "$f" ] || continue
  grep -q 'Demonstration' "$f" || continue
  cmds=$((cmds + 1))
  grep -q "$cmd_test" "$f" && cmds_linked=$((cmds_linked + 1))
done
[ "$cmds" -gt 0 ] || { echo "    no command page carries a demonstration; the corpus moved"; exit 1; }
[ "$cmds" = "$cmds_linked" ] || { echo "    $cmds command page(s), $cmds_linked name $cmd_test"; exit 1; }

# every use-case page that shows a recorded run names the test that runs it
ucs=0; ucs_linked=0
for f in "$P"/use-cases/*/index.html; do
  [ -f "$f" ] || continue
  grep -q 'Evidence of the run' "$f" || continue
  ucs=$((ucs + 1))
  grep -q "$uc_test" "$f" && ucs_linked=$((ucs_linked + 1))
done
[ "$ucs" -gt 0 ] || { echo "    no use-case page shows evidence; the corpus moved"; exit 1; }
[ "$ucs" = "$ucs_linked" ] || { echo "    $ucs use-case page(s) show evidence, $ucs_linked name $uc_test"; exit 1; }

echo "    $pages command-line, $apis API, $cmds command and $ucs use-case page(s) each name their proof"
