# majordomus-covers: none
# A Cockpit statistic names the capability it was asked of, and the field it read.
#
# The Overview's statistics carried free-text provenance, and one was wrong: "1619
# capabilities" read `repository.info`'s capabilities.total while its label named
# `capabilities.list`, a capability the page never called. The labels are now built from
# the id the page handed to `ask`, plus the field read. So this case holds each statistic
# against the answer it names:
#
#   1. every Overview statistic names repository.info, none names capabilities.list, and
#      each links to the capability's own Cockpit page, which answers;
#   2. the value each statistic shows is the value of the field it names in the
#      repository.info answer, read over HTTP: the label is the provenance, not a caption.
#
# Mutation-proven: the old literal `capabilities.list` label fails step 1.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || skip "no curl"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v python3 >/dev/null 2>&1 || skip "no python3"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj546.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer
serve_up "$S/out.txt" "$S/err.txt" || exit 1
get() { curl -s -m 20 "$U$1"; }

get /cockpit > "$S/overview.html"
get /api/v1/repository > "$S/repository.json"

# value, capability, field — one line per statistic, from the rendered page
python3 - "$S/overview.html" > "$S/stats.tsv" <<'PY'
import re, sys
html = open(sys.argv[1], encoding="utf-8").read()
for m in re.finditer(r'<div class="mj-stat">(.*?)</div>', html, re.S):
    body = m.group(1)
    value = re.search(r'class="mj-stat-value">([^<]*)<', body)
    cap = re.search(r'data-capability="([^"]*)"', body)
    field = re.search(r'data-field="([^"]*)"', body)
    source = re.search(r'class="mj-stat-source[^"]*"[^>]*>([^<]*)<', body)
    print("\t".join([value.group(1) if value else "",
                     cap.group(1) if cap else "",
                     field.group(1) if field else "",
                     source.group(1) if source else ""]))
PY

N="$(wc -l < "$S/stats.tsv" | tr -d ' ')"
[ "$N" -ge 6 ] || { echo "    the Overview shows $N statistic(s), expected the six of repository.info"; cat "$S/stats.tsv"; exit 1; }

# ---------------------------------------------------------------- 1. the capability asked
grep -q 'capabilities.list' "$S/stats.tsv" \
  && { echo "    a statistic still names capabilities.list, which the Overview never calls"; cat "$S/stats.tsv"; exit 1; }
while IFS="$(printf '\t')" read -r value cap field source; do
  [ "$cap" = repository.info ] \
    || { echo "    the statistic '$value' names '$cap', not the capability the page asked (repository.info)"; exit 1; }
  [ "$source" = "$cap · $field" ] \
    || { echo "    the statistic '$value' carries a provenance label of its own: '$source'"; exit 1; }
done < "$S/stats.tsv"
grep -q 'href="/cockpit/capabilities/repository.info"' "$S/overview.html" \
  || { echo "    the statistics do not link to the capability they name"; exit 1; }
CODE="$(curl -s -m 20 -o /dev/null -w '%{http_code}' "$U/cockpit/capabilities/repository.info")"
[ "$CODE" = 200 ] || { echo "    the capability page a statistic links to answers $CODE"; exit 1; }

# ---------------------------------------------------------------- 2. the value is the field's
while IFS="$(printf '\t')" read -r value cap field source; do
  want="$(jq -r --arg f "$field" 'getpath($f | split("."))' "$S/repository.json")"
  [ "$value" = "$want" ] \
    || { echo "    the statistic shows $value while repository.info's $field is $want"; exit 1; }
done < "$S/stats.tsv"
serve_down
echo "    $N statistic(s), each the value of the repository.info field it names"
