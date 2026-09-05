#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_PJ_* are read by plan.sh, doctor.sh and the site generator
# project.sh — the canonical project model: milestones, issues, and the graph between them.
#
# This file loads the canonical data and hands it to lib/project.awk, which derives every
# status, wave and finding. Nothing here decides what READY means; nothing downstream of
# here does either. The CLI (lib/plan.sh), the site generator (scripts/generate-site-data)
# and the GitHub adapter (scripts/github-sync) all call mj_project_load and read the same
# model, so a surface cannot disagree with the tool about what is ready.
#
# Canonical layout, all of it authored by hand and none of it generated:
#   .ai/repo/project/project.yaml            the repository this model belongs to
#   .ai/repo/project/milestones/<ID>.yaml    one executable outcome specification
#   .ai/repo/project/issues/<ID>.yaml        one bounded execution contract
#
# Status is never stored. An issue records what happened to it — started_at, verified_at,
# completed_at, evidence — and the status is computed from those facts and from the state
# of its dependencies. There is no status field to contradict the graph.

# Sourcing is idempotent: lib/plan.sh sources this file, and so does the GitHub adapter,
# which then sources lib/plan.sh. Re-initialising here would discard an already loaded model.
: "${MJ_PJ:=}"          # temp workspace: raw.tsv, model.tsv, flat/
: "${MJ_PJ_LOADED:=0}"

mj_project_dir() { printf '%s' "$MJ_PROJECT_DIR"; }
mj_project_present() { [ -f "$MJ_PROJECT_DIR/project.yaml" ]; }

# Load the canonical model. 0 loaded · 1 no project here · 2 a canonical file does not parse.
# Parse failures are reported on stderr with the file that caused them; nothing is guessed.
mj_project_load() {
  [ "$MJ_PJ_LOADED" = 1 ] && return 0
  mj_project_present || return 1
  local dir raw flat f id rc=0
  dir="$MJ_PROJECT_DIR"
  MJ_PJ="$(mktemp -d "${TMPDIR:-/tmp}/mj.pj.XXXXXX")"
  mkdir -p "$MJ_PJ/flat"
  raw="$MJ_PJ/raw.tsv"; : > "$raw"

  flat="$MJ_PJ/flat/PROJECT"
  mj_yaml_flatten "$dir/project.yaml" > "$flat" 2>/dev/null || { mj_err "project.yaml does not parse"; return 2; }
  awk -F= '{ k=$1; sub(/^[^=]*=/, "", $0); printf "P\t%s\t%s\n", k, $0 }' "$flat" >> "$raw"

  MJ_PJ_MILESTONES=""; MJ_PJ_ISSUES=""
  for f in "$dir"/milestones/*.yaml; do
    [ -f "$f" ] || continue
    id="$(basename "$f" .yaml)"
    flat="$MJ_PJ/flat/$id"
    [ -f "$flat" ] && { mj_err "duplicate id $id ($f)"; rc=2; continue; }
    mj_yaml_flatten "$f" > "$flat" 2>/dev/null || { mj_err "$f does not parse"; rc=2; continue; }
    # the id check rides on the same pass that emits the rows: a record whose id field
    # disagrees with its filename emits nothing and is refused by name
    awk -F= -v i="$id" '$1 == "id" { v = $0; sub(/^[^=]*=/, "", v); seen = v }
      { k=$1; sub(/^[^=]*=/, "", $0); rows[++n] = "M\t" i "\t" k "\t" $0 }
      END { if (seen != i) { printf "%s\n", seen > "/dev/stderr"; exit 3 } for (r = 1; r <= n; r++) print rows[r] }' "$flat" >> "$raw" 2>"$MJ_PJ/id.err" \
      || { mj_err "$f declares id '$(cat "$MJ_PJ/id.err")' but its filename says $id"; rc=2; continue; }
    MJ_PJ_MILESTONES="$MJ_PJ_MILESTONES $id"
  done
  for f in "$dir"/issues/*.yaml; do
    [ -f "$f" ] || continue
    id="$(basename "$f" .yaml)"
    flat="$MJ_PJ/flat/$id"
    [ -f "$flat" ] && { mj_err "duplicate id $id ($f)"; rc=2; continue; }
    mj_yaml_flatten "$f" > "$flat" 2>/dev/null || { mj_err "$f does not parse"; rc=2; continue; }
    # the id check rides on the same pass that emits the rows: a record whose id field
    # disagrees with its filename emits nothing and is refused by name
    awk -F= -v i="$id" '$1 == "id" { v = $0; sub(/^[^=]*=/, "", v); seen = v }
      { k=$1; sub(/^[^=]*=/, "", $0); rows[++n] = "I\t" i "\t" k "\t" $0 }
      END { if (seen != i) { printf "%s\n", seen > "/dev/stderr"; exit 3 } for (r = 1; r <= n; r++) print rows[r] }' "$flat" >> "$raw" 2>"$MJ_PJ/id.err" \
      || { mj_err "$f declares id '$(cat "$MJ_PJ/id.err")' but its filename says $id"; rc=2; continue; }
    MJ_PJ_ISSUES="$MJ_PJ_ISSUES $id"
  done
  [ "$rc" = 0 ] || return 2
  awk -f "$MJ_LIB_DIR/project.awk" "$raw" > "$MJ_PJ/model.tsv" || return 2
  MJ_PJ_LOADED=1
  return 0
}
mj_project_unload() { [ -n "$MJ_PJ" ] && rm -rf "$MJ_PJ"; MJ_PJ=""; MJ_PJ_LOADED=0; }

# ---------------------------------------------------------------- model accessors
# Every one of these reads model.tsv. None recomputes anything.
mj_pj_rows()   { awk -F'\t' -v t="$1" '$1==t' "$MJ_PJ/model.tsv"; }
mj_pj_row()    { awk -F'\t' -v t="$1" -v i="$2" '$1==t && $2==i' "$MJ_PJ/model.tsv"; }
mj_pj_col()    { mj_pj_row "$1" "$2" | cut -f"$3"; }

mj_pj_project_name()   { mj_pj_rows P | cut -f2; }
mj_pj_repository()     { mj_pj_rows P | cut -f3; }
mj_pj_default_branch() { mj_pj_rows P | cut -f4; }
mj_pj_active()         { mj_pj_rows P | cut -f5; }

mj_pj_milestone_ids()  { mj_pj_rows M | cut -f2; }
# Is this id a milestone? Answered from the record the loader wrote, never from the shape
# of the id. A milestone id is a stable slug; the version lives in a field, so nothing may
# infer a record's kind from how its id is spelled.
mj_pj_is_milestone()   { case " $MJ_PJ_MILESTONES " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }
mj_pj_issue_ids()      { mj_pj_rows I | cut -f2; }

# milestone columns: 2 id · 3 status · 4 order · 5 priority · 6 title · 7 slug
#                    8 counts (name=value, comma separated, keyed by the status vocabulary)
#                    9 version · 10 rank · 11 depends · 12 blocked_by · 13 dependents · 14 claims
mj_pj_m_status()  { mj_pj_col M "$1" 3; }
mj_pj_m_title()   { mj_pj_col M "$1" 6; }
mj_pj_m_counts()  { mj_pj_col M "$1" 8; }
mj_pj_m_version() { mj_pj_col M "$1" 9; }
mj_pj_m_rank()    { mj_pj_col M "$1" 10; }
mj_pj_m_deps()    { mj_pj_col M "$1" 11; }
mj_pj_m_blocked() { mj_pj_col M "$1" 12; }
mj_pj_m_claims()  { mj_pj_col M "$1" 14; }
# One count out of a milestone's counts field, by name: total, required, or any status the
# vocabulary declares. Nothing outside project.awk decides which counts exist.
mj_pj_m_count()   { mj_pj_m_counts "$1" | tr ',' '\n' | sed -n "s/^$2=//p"; }
# The declared status vocabulary for issues or for milestones, space separated.
mj_pj_statuses()  { mj_pj_rows S | awk -F'\t' -v k="$1" '$2 == k { print $3 }'; }

# milestone ids in roadmap order: rank, then order, then id. This is the roadmap sequence,
# derived from the graph; no list of versions is maintained anywhere.
mj_pj_roadmap() {
  awk -F'\t' '$1=="M" { printf "%s\t%s\t%s\n", $10+0, $4+0, $2 }' "$MJ_PJ/model.tsv" \
    | sort -k1,1n -k2,2n -k3,3 | cut -f3
}
# milestone ids in one derived state
mj_pj_m_in_state() { awk -F'\t' -v s="$1" '$1=="M" && $3==s { print $2 }' "$MJ_PJ/model.tsv"; }
# milestone dependency edges, one "from to" per line
mj_pj_m_edges()    { awk -F'\t' '$1=="R" { print $2, $3 }' "$MJ_PJ/model.tsv"; }

# issue columns: 2 id · 3 milestone · 4 status · 5 wave · 6 priority · 7 profile
#                8 parallel_safe · 9 title · 10 slug · 11 deps · 12 blocked_by
#                13 dependents · 14 scope · 15 evidence_have · 16 evidence_need
mj_pj_i_status()  { mj_pj_col I "$1" 4; }
mj_pj_i_wave()    { mj_pj_col I "$1" 5; }
mj_pj_i_title()   { mj_pj_col I "$1" 9; }
mj_pj_i_deps()    { mj_pj_col I "$1" 11; }
mj_pj_i_blocked() { mj_pj_col I "$1" 12; }
mj_pj_i_scope()   { mj_pj_col I "$1" 14; }

# issue ids in a state, in id order; optionally restricted to one milestone
mj_pj_in_state() {
  awk -F'\t' -v s="$1" -v m="${2:-}" '$1=="I" && $4==s && (m=="" || $3==m) { print $2 }' "$MJ_PJ/model.tsv"
}
mj_pj_of_milestone() { awk -F'\t' -v m="$1" '$1=="I" && $3==m { print $2 }' "$MJ_PJ/model.tsv"; }

mj_pj_findings()      { mj_pj_rows V; }
mj_pj_fail_count()    { mj_pj_rows V | awk -F'\t' '$2=="FAIL"' | wc -l | tr -d ' '; }
mj_pj_warn_count()    { mj_pj_rows V | awk -F'\t' '$2=="WARN"' | wc -l | tr -d ' '; }

# flattened canonical record of one milestone or issue, for the fields the model does not carry
mj_pj_flat()  { printf '%s' "$MJ_PJ/flat/$1"; }
mj_pj_get()   { mj_yget "$MJ_PJ/flat/$1" "$2"; }
mj_pj_list()  { mj_ylist "$MJ_PJ/flat/$1" "$2"; }

# the next issue a worker should take: the lowest-wave READY issue of the active milestone,
# highest priority first, then id. Derived on every call; never stored.
mj_pj_next_ready() {
  local m="${1:-$(mj_pj_active)}" pick
  pick="$(mj_pj_ready_ranked "$m")"
  # The active milestone can have nothing ready while another one does — a milestone waiting
  # on its own acceptance evidence, for instance. Answering "none" then would send a worker
  # away from work that is genuinely executable, so the search widens rather than stops.
  [ -n "$pick" ] || pick="$(mj_pj_ready_ranked "")"
  printf '%s' "$pick"
}
# lowest wave, then priority, then id, among the READY issues of one milestone ("" = any)
mj_pj_ready_ranked() {
  awk -F'\t' -v m="$1" '$1=="I" && $4=="READY" && ($3==m || m=="") {
      p = $6; rank = (p=="p0"?0:(p=="p1"?1:(p=="p2"?2:3)))
      printf "%d\t%d\t%s\n", $5, rank, $2 }' "$MJ_PJ/model.tsv" \
    | sort -k1,1n -k2,2n -k3,3 | head -n 1 | cut -f3
}

# ---------------------------------------------------------------- unknown keys
# Same contract as the policy and profile files: a key nobody reads is an error, not a
# comment. The allowlists live beside the ones for policy and profiles.
mj_project_unknown_keys() {
  local bad=0 f id allow out
  for f in "$MJ_PJ/flat"/*; do
    [ -f "$f" ] || continue
    id="$(basename "$f")"
    if [ "$id" = PROJECT ]; then allow="project"
    elif mj_pj_is_milestone "$id"; then allow="milestone"
    else allow="issue"; fi
    out="$(mj_yaml_unknown_keys "$f" "$MJ_ALLOW_DIR/$allow.txt" || true)"
    if [ -n "$out" ]; then
      printf '%s %s\n' "$id" "$(printf '%s' "$out" | tr '\n' ' ')"
      bad=1
    fi
  done
  return $bad
}

# ---------------------------------------------------------------- Mermaid
# The one place a dependency diagram is drawn. Every diagram on the site, in the docs and
# in a GitHub body comes from here, so a hand-drawn DAG cannot drift from the real one.
# The roadmap graph, drawn from the same R records the CLI and the site read. The milestone
# graph and the issue graph are separate diagrams on purpose: one says which outcomes are
# reachable, the other says what a worker may execute inside one of them.
mj_project_roadmap_mermaid() {
  local id st ver cls
  printf 'flowchart LR\n'
  mj_pj_roadmap | while read -r id; do
    st="$(mj_pj_m_status "$id")"
    ver="$(mj_pj_m_version "$id")"
    printf '    %s["%s%s<br/>%s"]:::%s\n' \
      "$(printf '%s' "$id" | tr -c 'A-Za-z0-9' '_')" \
      "$([ -n "$ver" ] && printf '%s — ' "$ver")" "$id" \
      "$(mj_pj_m_title "$id" | sed 's/"/\&quot;/g')" \
      "$(printf '%s' "$st" | tr 'A-Z' 'a-z')"
  done
  mj_pj_m_edges | while read -r from to; do
    printf '    %s --> %s\n' \
      "$(printf '%s' "$from" | tr -c 'A-Za-z0-9' '_')" \
      "$(printf '%s' "$to" | tr -c 'A-Za-z0-9' '_')"
  done
  for cls in "done:#16a34a:#052e16" "active:#2563eb:#eff6ff" "verify:#7c3aed:#f5f3ff" \
             "planned:#0891b2:#ecfeff" "blocked:#b45309:#fffbeb" \
             "cancelled:#6b7280:#f9fafb" "superseded:#6b7280:#f9fafb"; do
    printf '    classDef %s stroke:%s,fill:%s,stroke-width:2px\n' "${cls%%:*}" "$(printf '%s' "$cls" | cut -d: -f2)" "$(printf '%s' "$cls" | cut -d: -f3)"
  done
}

mj_project_mermaid() {
  local m="${1:-}" id st cls
  printf 'flowchart LR\n'
  mj_pj_issue_ids | while read -r id; do
    [ -n "$m" ] && [ "$(mj_pj_col I "$id" 3)" != "$m" ] && continue
    st="$(mj_pj_i_status "$id")"
    printf '    %s["%s<br/>%s"]:::%s\n' "$id" "$id" "$(mj_pj_i_title "$id" | sed 's/"/\&quot;/g')" "$(printf '%s' "$st" | tr 'A-Z' 'a-z')"
  done
  mj_pj_rows G | while IFS="$(printf '\t')" read -r _ from to; do
    if [ -n "$m" ]; then
      [ "$(mj_pj_col I "$from" 3)" = "$m" ] || continue
      [ "$(mj_pj_col I "$to" 3)" = "$m" ] || continue
    fi
    printf '    %s --> %s\n' "$from" "$to"
  done
  for cls in "done:#16a34a:#052e16" "active:#2563eb:#eff6ff" "verify:#7c3aed:#f5f3ff" \
             "ready:#0891b2:#ecfeff" "blocked:#b45309:#fffbeb" "cancelled:#6b7280:#f9fafb"; do
    printf '    classDef %s stroke:%s,fill:%s,stroke-width:2px\n' "${cls%%:*}" "$(printf '%s' "$cls" | cut -d: -f2)" "$(printf '%s' "$cls" | cut -d: -f3)"
  done
}

# ---------------------------------------------------------------- issue mutation
# The lifecycle markers are the only writable fields, and each one is written by a named
# transition rather than by hand. Everything else in an issue file is authored prose.
# mj_pj_set_field <issue> <key> <value>   — replaces or appends a top-level scalar
mj_pj_set_field() {
  local id="$1" key="$2" val="$3" f tmp
  f="$MJ_PROJECT_DIR/issues/$id.yaml"
  [ -f "$f" ] || f="$MJ_PROJECT_DIR/milestones/$id.yaml"
  [ -f "$f" ] || return 1
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.set.XXXXXX")"
  if grep -qE "^$key:" "$f"; then
    awk -v k="$key" -v v="$val" '
      $0 ~ "^" k ":" { print k ": " v; next } { print }' "$f" > "$tmp"
  else
    awk -v k="$key" -v v="$val" '
      /^evidence:/ && !done { print k ": " v; done=1 } { print }
      END { if (!done) print k ": " v }' "$f" > "$tmp"
  fi
  cat "$tmp" > "$f"
  rm -f "$tmp"
}
