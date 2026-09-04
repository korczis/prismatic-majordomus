# Cross-surface consistency. "One engine" is a claim; this is what keeps it true.
#
# The command line, the Mermaid diagram and the GitHub projection are asked the same
# questions here — how many issues, which are READY, which edges exist — and their answers
# have to be identical. A surface that re-derives a status of its own fails this case.
. "$ROOT/test/lib.sh"
SYNC="$ROOT/scripts/github-sync"
"$MJ" init >/dev/null
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000 I0001
pj_issue I0003 M000 I0001
pj_issue I0004 M000 I0002 I0003
pj_issue I0005 M000
expect_exit 0 "$MJ" plan validate

canonical_ids() { ls .majordomus/project/issues/*.yaml | sed 's#.*/##; s#\.yaml$##' | sort; }
cli_ids()       { "$MJ" plan list | awk 'NR>1{print $1}' | sort; }
json_ids()      { "$MJ" --json plan list | tr ',' '\n' | sed -n 's/.*"id":"\([^"]*\)".*/\1/p' | sort; }
graph_ids()     { "$MJ" plan graph | sed -n 's/^    \([I0-9]*\)\[.*/\1/p' | sort; }
sync_ids()      { "$SYNC" --plan | awk '$2=="open" || $2=="closed" { print $1 }' | sort; }

cli_ready()   { "$MJ" plan ready | sed -n 's/^\(I[0-9]*\) .*/\1/p' | sort; }
list_ready()  { "$MJ" plan list | awk '$2=="READY"{print $1}' | sort; }
graph_ready() { "$MJ" plan graph | sed -n 's/^    \([I0-9]*\)\[.*:::ready$/\1/p' | sort; }
sync_open()   { "$SYNC" --plan | awk '/^  I[0-9]+ +open +READY/{print $1}' | sort; }

canonical_edges() {
  for f in .majordomus/project/issues/*.yaml; do
    id="$(basename "$f" .yaml)"
    awk '/^depends_on:/{f=1;next} f&&/^  - /{sub(/^  - /,"");print;next} f{exit}' "$f" \
      | while read -r d; do printf '%s --> %s\n' "$d" "$id"; done
  done | sort
}
graph_edges()  { "$MJ" plan graph | sed -n 's/^    \(.* --> .*\)$/\1/p' | sort; }
body_edges()   { for i in $(canonical_ids); do "$MJ" plan body "$i" >/dev/null || exit 1; done; }

same() { # label, a, b
  [ "$2" = "$3" ] || { printf '    %s disagree:\n      A: %s\n      B: %s\n' "$1" "$(printf '%s' "$2" | tr '\n' ' ')" "$(printf '%s' "$3" | tr '\n' ' ')"; exit 1; }
}

# --- the issue set is the same everywhere
C="$(canonical_ids)"
same "canonical and CLI issue sets"        "$C" "$(cli_ids)"
same "canonical and JSON issue sets"       "$C" "$(json_ids)"
same "canonical and Mermaid node sets"     "$C" "$(graph_ids)"
same "canonical and GitHub projection sets" "$C" "$(sync_ids)"

# --- the READY set is the same everywhere
R="$(cli_ready)"
[ -n "$R" ] || { echo "    no READY issue in the fixture; the case proves nothing"; exit 1; }
same "plan ready and plan list"            "$R" "$(list_ready)"
same "plan ready and the Mermaid styling"  "$R" "$(graph_ready)"
same "plan ready and the GitHub projection" "$R" "$(sync_open)"

# --- the edge set is the same everywhere
E="$(canonical_edges)"
same "canonical and Mermaid edges" "$E" "$(graph_edges)"
body_edges || { echo "    plan body failed for an issue in the model"; exit 1; }

# --- a status is reported identically by every surface that reports one
"$MJ" plan start I0001 >/dev/null
"$MJ" plan evidence I0001 --covers proof --type test --command "true" --result ok >/dev/null
"$MJ" plan "done" I0001 >/dev/null
graph_out="$("$MJ" plan graph)"
for id in $(canonical_ids); do
  st="$(pj_status "$id")"
  "$MJ" --json plan list | grep -qF "\"id\":\"$id\",\"milestone\":\"M000\",\"status\":\"$st\"" \
    || { echo "    $id is $st on the table and something else in JSON"; exit 1; }
  case "$st" in
    DONE|CANCELLED) want=closed ;;
    *) want=open ;;
  esac
  sync_out="$("$SYNC" --plan)"
  printf '%s\n' "$sync_out" | grep -qE "^  $id +$want +$st " \
    || { echo "    $id is $st canonically; the GitHub projection disagrees"; printf '%s\n' "$sync_out" | grep "$id"; exit 1; }
  printf '%s\n' "$graph_out" | grep -q "^    $id\[.*:::$(printf '%s' "$st" | tr 'A-Z' 'a-z')$" \
    || { echo "    $id is $st canonically; the diagram styles it differently"; exit 1; }
done

# --- the milestone's counts are the sum of the issue statuses, not an independent tally
tot="$(canonical_ids | wc -l | tr -d ' ')"
done_n="$(cli_ids | while read -r i; do pj_status "$i"; done | grep -c '^DONE$' || true)"
"$MJ" --json plan status | grep -qF "\"total\":$tot,\"done\":$done_n" \
  || { echo "    the milestone counts do not match the issue statuses"; "$MJ" --json plan status; exit 1; }

# --- one engine: no surface implements a status rule of its own
for f in "$ROOT/lib/plan.sh" "$ROOT/scripts/github-sync" "$ROOT/scripts/generate-site-data" "$ROOT/lib/doctor.sh"; do
  grep -nE '(completed_at|started_at|verified_at).*(==|!=|-eq|=)' "$f" | grep -vE '^\s*#|mj_pj_set_field|printf|--arg' \
    && { echo "    ${f##*/} derives a status of its own instead of reading the model"; exit 1; }
done
grep -q 'status\[id\] = st' "$ROOT/lib/project.awk" || { echo "    the one place status is assigned has moved; update this case"; exit 1; }
[ "$(grep -c 'status\[id\] = st' "$ROOT/lib/project.awk")" = 1 ] || { echo "    status is assigned in more than one place"; exit 1; }

# --- and across processes, against the real repository: the site data was generated by one
#     program in one process, the CLI answers in another, and they must agree. The fixture
#     above cannot prove this, because generate-site-data reads the repository it lives in.
# The data is generated fresh into this case's scratch directory, so a repository whose
# committed generation is behind its canonical files fails `generate-site-data --check`,
# which is where that belongs, rather than failing this case for the wrong reason.
if command -v jq >/dev/null 2>&1 && "$ROOT/scripts/generate-site-data" --out "$T/gen" >/dev/null 2>&1; then
PJ="$T/gen/plan.json"
  site_ids="$(jq -r '.issues[].id' "$PJ" | sort)"
  cli_real="$("$MJ" --repo "$ROOT" plan list | awk 'NR>1{print $1}' | sort)"
  same "generated site data and the CLI, same repository" "$site_ids" "$cli_real"
  site_ready="$(jq -r '.issues[] | select(.status=="READY") | .id' "$PJ" | sort)"
  cli_real_ready="$("$MJ" --repo "$ROOT" plan list | awk '$2=="READY"{print $1}' | sort)"
  same "generated site data and the CLI READY sets" "$site_ready" "$cli_real_ready"
  site_edges="$(jq -r '.edges[] | .from + " --> " + .to' "$PJ" | sort)"
  cli_real_edges="$("$MJ" --repo "$ROOT" plan graph | sed -n 's/^    \(.* --> .*\)$/\1/p' | sort)"
  same "generated site data and the CLI edge sets" "$site_edges" "$cli_real_edges"
  site_next="$(jq -r '.next_ready' "$PJ")"
  cli_next="$("$MJ" --repo "$ROOT" --json plan next | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')"
  same "generated site data and the CLI next ready issue" "$site_next" "$cli_next"
fi
