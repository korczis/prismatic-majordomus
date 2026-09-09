# majordomus-covers: plan
# The plan's derivations, in two engines, answering identically.
#
# `lib/project.awk` has been the only implementation of these semantics since the model
# existed: what READY means, what a wave is, which milestone is active, which issue comes
# next. `apps/majordomus-cli/src/plan.rs` is a second reader of the same semantics, written
# so that the capability registry can answer them — an agent asking the shared MCP server
# what to work on could otherwise be handed every issue and no answer.
#
# Two implementations of one derivation is the failure this repository exists to prevent,
# so this case is the reason the second one is allowed to exist. It runs both engines over
# every record of this repository and over a fixture built to hit the edges the real plan
# does not reach — a cycle, a cancelled issue, a completion without its evidence, a
# milestone gate, a scope overlap — and compares them field by field. Where they disagree
# the awk is the incumbent and is right; plan.rs is the file that changes.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as every Rust case does.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj99.XXXXXX")"; trap 'rm -rf "$S"' EXIT

# One MCP tool call over the real pipes, structured content on stdout. The executable has no
# `plan` subcommand — the command line is declared a second time in clap and this module
# claims none of it — so MCP is how a shell reads a plan capability, which is also how an
# agent reads it.
plan_tool() { # <repository> <tool> <arguments-json>
  local repo="$1" tool="$2" args="${3:-}" out
  [ -n "$args" ] || args='{}'
  out="$S/frames.$$"
  {
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case99","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$tool" "$args"
  } | ( cd "$repo" && "$RB" mcp 2>"$S/mcp.err" ) > "$out" || { printf '    the server failed on %s:\n' "$tool" >&2; sed 's/^/    | /' "$S/mcp.err" >&2; return 1; }
  tail -n 1 "$out" | jq -e '.result.isError == false' >/dev/null \
    || { printf '    %s returned an error frame: %s\n' "$tool" "$(tail -n 1 "$out")" >&2; return 1; }
  tail -n 1 "$out" | jq '.result.structuredContent'
}

# The two engines, over one repository, on one derivation. Both sides are normalised to the
# same document by jq and compared byte for byte, so a difference names the field.
same() { # <label> <shell-file> <rust-file>
  cmp -s "$2" "$3" && return 0
  echo "    the shell and the executable disagree on $1:"
  diff "$2" "$3" | head -30 | sed 's/^/    | /'
  return 1
}

compare_repository() { # <repository> <label>
  local repo="$1" label="$2"

  # --- every issue: status, wave, milestone, priority, title, and both dependency lists
  ( cd "$repo" && "$MJ" plan list --json ) | jq -S -c '.issues[]
      | {id, milestone, status, wave, priority, title, depends_on, blocked_by}' > "$S/sh.issues"
  plan_tool "$repo" majordomus_plan_issues '{}' | jq -S -c '.issues[]
      | {id, milestone, status, wave, priority, title,
         depends_on: (.depends_on | join(",")),
         blocked_by: (.blocked_by | join(","))}' > "$S/rs.issues" || return 1
  [ -s "$S/sh.issues" ] || { echo "    $label: the shell listed no issues"; return 1; }
  same "$label: the issues" "$S/sh.issues" "$S/rs.issues" || return 1

  # --- every milestone: derived status and the counts keyed by the declared vocabulary
  ( cd "$repo" && "$MJ" plan status --json ) \
    | jq -S -c '{active_milestone, next_ready, statuses,
                 milestones: [.milestones[] | {id, status, title, counts}]}' > "$S/sh.status"
  plan_tool "$repo" majordomus_plan_status '{}' \
    | jq -S -c '{active_milestone: .project.active_milestone,
                 next_ready: (.next_ready.id // ""), statuses: .statuses.issue,
                 milestones: [.milestones[] | {id, status, title, counts}]}' > "$S/rs.status" || return 1
  same "$label: the milestones and the active milestone" "$S/sh.status" "$S/rs.status" || return 1

  # --- the roadmap: rank, order and the whole derived sequence
  ( cd "$repo" && "$MJ" plan roadmap --json ) | jq -S -c '.milestones[]
      | {id, version, title, status, rank, order, issues, depends_on, blocked_by, dependents, claims}' > "$S/sh.roadmap"
  plan_tool "$repo" majordomus_plan_roadmap '{}' | jq -S -c '.milestones[]
      | {id, version, title, status, rank, order, issues: .counts, depends_on, blocked_by, dependents, claims}' > "$S/rs.roadmap" || return 1
  same "$label: the roadmap" "$S/sh.roadmap" "$S/rs.roadmap" || return 1

  # --- the one issue a worker should take now
  ( cd "$repo" && "$MJ" plan next --json 2>/dev/null ) | jq -S -c '{id, title, milestone, wave}' > "$S/sh.next" || : > "$S/sh.next"
  plan_tool "$repo" majordomus_plan_next '{}' \
    | jq -S -c 'if .issue then {id: .issue.id, title: .issue.title, milestone: .issue.milestone, wave: .issue.wave} else empty end' > "$S/rs.next" || return 1
  same "$label: the next ready issue" "$S/sh.next" "$S/rs.next" || return 1

  # --- the findings, code for code and message for message, in derivation order, and the
  #     tallies they are counted into. `plan validate` exits 10 on an invalid model, which
  #     is a verdict and not a failure of the run.
  ( cd "$repo" && "$MJ" plan validate --json ) > "$S/sh.validate" 2>/dev/null || true
  plan_tool "$repo" majordomus_plan_validate '{}' > "$S/rs.validate" || return 1
  jq -S -c 'select(.category) | {level, code: .category, subject, message}' < "$S/sh.validate" > "$S/sh.findings"
  jq -S -c '.findings[] | {level, code, subject, message}' < "$S/rs.validate" > "$S/rs.findings"
  same "$label: the findings" "$S/sh.findings" "$S/rs.findings" || return 1
  jq -S -c 'select(.failures != null) | {milestones, issues, failures, warnings}' < "$S/sh.validate" > "$S/sh.counts"
  jq -S -c '{milestones, issues, failures, warnings}' < "$S/rs.validate" > "$S/rs.counts"
  same "$label: the validation tallies" "$S/sh.counts" "$S/rs.counts" || return 1

  # --- the waves: which issues the graph allows to run at the same time
  ( cd "$repo" && "$MJ" plan waves ) \
    | awk '/^serialised by scope overlap:/{ done=1 } done { next }
           /^Wave /{ w=$2; next } /^  [A-Za-z0-9]/{ printf "%s %s\n", w, $1 }' | sort > "$S/sh.waves"
  plan_tool "$repo" majordomus_plan_waves '{}' \
    | jq -r '.waves[] | .wave as $w | .issues[] | "\($w) \(.id)"' | sort > "$S/rs.waves" || return 1
  same "$label: the waves" "$S/sh.waves" "$S/rs.waves" || return 1

  # --- the ready set and the blocked set are the same filter, not a second derivation
  local n_ready n_blocked
  n_ready="$(plan_tool "$repo" majordomus_plan_issues '{"status":"READY"}' | jq '.total')" || return 1
  n_blocked="$(plan_tool "$repo" majordomus_plan_issues '{"status":"BLOCKED"}' | jq '.total')" || return 1
  [ "$n_ready" = "$(grep -c '"status":"READY"' "$S/rs.issues" || true)" ] \
    || { echo "    $label: the READY filter and the whole listing disagree"; return 1; }
  [ "$n_blocked" = "$(grep -c '"status":"BLOCKED"' "$S/rs.issues" || true)" ] \
    || { echo "    $label: the BLOCKED filter and the whole listing disagree"; return 1; }
}

# ---------------------------------------------------------------- the projections exist
# Declared once, in `capability!`, and reaching MCP and HTTP by derivation. If the generator
# had to be told about a plan capability, this is where that would show.
run_quiet "$S/caps.err" sh -c 'cd "$1" && exec "$2" capabilities list --format json' _ "$ROOT" "$RB" > "$S/caps.json"
jq -e '
  [ .capabilities[] | select(.id | startswith("plan.")) ] as $p
  | ($p | length) >= 7
  and ([ $p[] | select(.exposure.mcp.tool != null) ] | length) == ($p | length)
  and ([ $p[] | select(.exposure.http.path != null) ] | length) == ($p | length)
  and ([ $p[] | select(.id == "plan.model" and .exposure.mcp.resource.uri == "majordomus://plan") ] | length) == 1
' < "$S/caps.json" >/dev/null || { echo "    the plan capabilities are not projected to MCP and HTTP"; exit 1; }
# and the OpenAPI document carries every one of their routes, without a route table edited
grep -q '"/api/v1/plan"' "$ROOT/docs/generated/openapi.json" \
  || { echo "    the OpenAPI document has no /api/v1/plan; run: majordomus generate"; exit 1; }
for p in status issues waves next roadmap validate record; do
  grep -q "\"/api/v1/plan/$p\"" "$ROOT/docs/generated/openapi.json" \
    || { echo "    the OpenAPI document has no /api/v1/plan/$p"; exit 1; }
done

# ---------------------------------------------------------------- this repository
# 15-odd milestones and 180-odd issues, every status the vocabulary declares, and a real
# graph. The numbers are measured, never written down: the assertion is that the two engines
# agree, not that they agree on a figure somebody typed here.
compare_repository "$ROOT" "this repository" || exit 1

# ---------------------------------------------------------------- the edges
# A fixture the real plan does not reach: a cancelled issue, a completion whose evidence is
# missing, a milestone gate that blocks work underneath it, two issues of one wave that
# share a path, and a dependency on something that does not exist.
"$MJ" init >/dev/null
pj_init
pj_milestone M000 0
pj_milestone M001 1
printf 'depends_on:\n  - M000\n' >> .ai/repo/project/milestones/M001.yaml
pj_issue I0001 M000
pj_issue I0002 M000 I0001
pj_issue I0003 M000 I0001
pj_issue I0004 M000 I0002 I0003
pj_issue I0005 M001
pj_issue I0006 M001 I0005
# a cancelled issue is out of the denominator and out of every wave's ready set
pj_issue I0007 M000
printf 'cancelled: true\n' >> .ai/repo/project/issues/I0007.yaml
# a completion without its evidence stays in VERIFY and says why
pj_issue I0008 M000
printf 'completed_at: 2026-01-01T00:00:00Z\n' >> .ai/repo/project/issues/I0008.yaml
# and two issues of one wave whose scope overlaps are serialised by it
# two issues of one wave that touch the same paths are serialised. The scope entry is
# replaced, never appended: a second `scope:` key is a duplicate mapping key, which the
# shell flattener resolves to the first and the executable's YAML parser refuses outright —
# the record then vanishes from the index and the two engines are compared on different
# models rather than on one.
rescope() { # <issue id> <path>
  awk -v p="$2" '{ sub(/^  - src\/.*$/, "  - " p) } 1' ".ai/repo/project/issues/$1.yaml" > "$S/rescope"
  cat "$S/rescope" > ".ai/repo/project/issues/$1.yaml"
}
pj_issue I0012 M000
rescope I0001 apps/thing
rescope I0012 apps/thing/src/main.rs
git add -A >/dev/null && git commit -qm fixture
expect_exit 0 "$MJ" plan validate
compare_repository "$PWD" "a fixture with cancellation, a gate and an overlap" || exit 1

# the fixture really does exercise what it claims: a warning of each kind the case names
( "$MJ" plan validate --json 2>/dev/null ) | jq -r '.category // empty' | sort -u > "$S/codes"
for code in evidence_missing scope_conflict; do
  grep -qx "$code" "$S/codes" || { echo "    the fixture no longer produces a $code finding"; exit 1; }
done
[ "$("$MJ" plan list | awk '$1=="I0007"{print $2}')" = CANCELLED ] \
  || { echo "    the fixture's cancelled issue is not CANCELLED"; exit 1; }
[ "$("$MJ" plan list | awk '$1=="I0005"{print $2}')" = BLOCKED ] \
  || { echo "    the milestone gate does not reach the work"; exit 1; }

# --- an unknown dependency: an invalid model, refused by both engines by the same name
pj_issue I0009 M000 I9999
git add -A >/dev/null
expect_exit 10 "$MJ" plan validate
plan_tool "$PWD" majordomus_plan_validate '{}' \
  | jq -e '.valid == false and ([.findings[] | select(.code == "unknown_dependency" and .subject == "I0009")] | length) == 1' >/dev/null \
  || { echo "    the executable does not refuse the unknown dependency the shell refuses"; exit 1; }
rm .ai/repo/project/issues/I0009.yaml
git add -A >/dev/null

# --- a cycle: neither engine gives anybody in it a wave, and both name it
pj_issue I0010 M000 I0011
pj_issue I0011 M000 I0010
git add -A >/dev/null
expect_exit 10 "$MJ" plan validate
expect_grep 'cycle'
plan_tool "$PWD" majordomus_plan_validate '{}' \
  | jq -e '[.findings[] | select(.code == "cycle")] | length == 1' >/dev/null \
  || { echo "    the executable does not report the cycle the shell reports"; exit 1; }
plan_tool "$PWD" majordomus_plan_waves '{}' \
  | jq -e '[.waves[].issues[].id] | (index("I0010") == null) and (index("I0011") == null)' >/dev/null \
  || { echo "    an issue inside a cycle was given a wave"; exit 1; }

# ---------------------------------------------------------------- nothing was written
# Every plan capability is a query. The registry's contract is that none of them writes to
# the repository, and this is the assertion that would fail if one did.
before="$(git status --porcelain; git ls-files -s)"
plan_tool "$PWD" majordomus_plan '{}' >/dev/null
plan_tool "$PWD" majordomus_plan_record '{"id":"I0001"}' >/dev/null
after="$(git status --porcelain; git ls-files -s)"
[ "$before" = "$after" ] || { echo "    a plan capability changed the repository"; exit 1; }
