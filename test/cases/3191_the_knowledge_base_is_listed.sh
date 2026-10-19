# majordomus-covers: none
# The whole knowledge base through the built executable, over a fixture `majordomus init`
# installs and two records it commits — one curated and verified, one candidate derived from
# an observation (I2792):
#
#   knowledge records          every record of both source classes, newest first, as its file
#                              declares it, with how many each class and each status holds
#   --class/--status/--about   each narrows; a subject matches itself and the paths beneath it,
#                              never a sibling that shares its prefix
#   a word outside the schema  refused as a usage error, never answered with nothing
#   GET /api/v1/knowledge/records answers what the command line printed, to the byte
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
mkdir -p .ai/repo/knowledge/curated .ai/repo/knowledge/candidates
cat > .ai/repo/knowledge/curated/read-first.md <<'EOF'
---
schema: knowledge/v1
id: read-first
kind: knowledge
class: convention
title: The design document is read before the command reference
description: The order a newcomer reads in.
status: verified
epistemics: decided
date: 2026-09-05
tags:
  - onboarding
provenance:
  origin: authored
  derived_from:
    - file:README.md
---

# Read first
EOF
cat > .ai/repo/knowledge/candidates/e1-0123456789ab.md <<'EOF'
---
schema: knowledge/v1
id: e1-0123456789ab
kind: knowledge
class: lesson
title: A loop fed by a pipe loses its counter
description: Observed in episode e1.
status: candidate
epistemics: observed
date: 2026-10-17
about: lib/check.sh
route: project
tags:
  - derived
provenance:
  origin: extracted
  derived_from:
    - session:e1
---

# A loop fed by a pipe loses its counter
EOF
git add -A >/dev/null && git commit -qm 'install and two records'

"$RB" knowledge records --format json > all.json 2> err.txt \
  || { echo "    knowledge records failed: $(cat err.txt)"; exit 1; }
[ "$(jq -r '.total' all.json)" = 2 ] || { echo "    the base does not hold both records: $(cat all.json)"; exit 1; }
[ "$(jq -r '[.records[].id] | join(",")' all.json)" = "e1-0123456789ab,read-first" ] \
  || { echo "    the base is not newest first: $(jq -c '[.records[].id]' all.json)"; exit 1; }
[ "$(jq -r '.classes.lesson, .classes.convention, .statuses.verified, .statuses.candidate' all.json | tr '\n' ' ')" = "1 1 1 1 " ] \
  || { echo "    the counts are wrong: $(jq -c '{classes, statuses}' all.json)"; exit 1; }
[ "$(jq -r '.records[1].source_class, .records[1].origin, .records[0].about' all.json | tr '\n' ' ')" = "curated authored lib/check.sh " ] \
  || { echo "    a record is not carried as declared: $(jq -c '.records' all.json)"; exit 1; }

"$RB" knowledge records --status verified --format json > verified.json
[ "$(jq -r '[.records[].id] | join(",")' verified.json)" = "read-first" ] \
  || { echo "    --status verified did not narrow: $(cat verified.json)"; exit 1; }
"$RB" knowledge records --about lib --format json > about.json
[ "$(jq -r '[.records[].id] | join(",")' about.json)" = "e1-0123456789ab" ] \
  || { echo "    --about lib did not find the record about lib/check.sh: $(cat about.json)"; exit 1; }
"$RB" knowledge records --about lib/che --format json > sibling.json
[ "$(jq -r '.total' sibling.json)" = 0 ] \
  || { echo "    a prefix that is not a path matched: $(cat sibling.json)"; exit 1; }
"$RB" knowledge records --class lesson > text.txt
grep -qF 'records      1 — lesson 1 — candidate 1' text.txt \
  || { echo "    the text rendering lost the counts: $(cat text.txt)"; exit 1; }
grep -qF '2026-10-17  candidate  lesson  e1-0123456789ab  A loop fed by a pipe loses its counter' text.txt \
  || { echo "    the text rendering lost the record: $(cat text.txt)"; exit 1; }

set +e
"$RB" knowledge records --class lessons > /dev/null 2> refused.txt; code=$?
set -e
[ "$code" = 2 ] || { echo "    a class outside the schema exited $code, not 2"; exit 1; }

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
curl -s --max-time 120 -o http.json "$U/api/v1/knowledge/records" \
  || { echo "    GET /api/v1/knowledge/records did not answer"; exit 1; }
[ "$(jq -cS . http.json)" = "$(jq -cS . all.json)" ] \
  || { echo "    HTTP and the command line answer differently: $(cat http.json)"; exit 1; }

echo "    the base lists both classes newest first, every filter narrows, a wrong word is refused, and HTTP agrees"
