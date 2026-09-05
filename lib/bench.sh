#!/usr/bin/env bash
# sourced by the dispatcher; guard against re-sourcing
[ -n "${MJ_LIB_bench:-}" ] && return 0 || MJ_LIB_bench=1
# bench — performance as executable evidence.
#
# Every public command in share/commands.yaml is a benchmark target; nothing here keeps a
# list. A target's scenario is the first scenario of its command fixture under
# test/fixtures/commands/ when the tool's own suite is present, and the bare command in an
# installed repository otherwise, so the demonstration the site renders and the case the
# suite executes are the same thing this command times. A read-only command is run once
# cold, warmed, then sampled warm in one repository; a command that mutates state gets a
# fresh repository per sample, so every sample of it is cold. Samples are wall-clock
# milliseconds of the child process alone; setup is never inside the clock.
#
# Runs are written under the local half of the layer (.ai/local/benchmarks/), which is
# ignored by git and is evidence, never a baseline. The baseline is the tracked file
# .ai/repo/benchmarks/baseline.json, written only by --write-baseline, and --check compares
# a fresh run against it under the policy's benchmark.regression thresholds.
# shellcheck source=commands.sh
. "$MJ_LIB_DIR/commands.sh"

MJ_BENCH_SCHEMA="majordomus/benchmark-result/v1"
MJ_BASELINE_SCHEMA="majordomus/benchmark-baseline/v1"

mj_bench_usage() {
  cat <<'USAGE'
usage: majordomus bench [<command>...] [--samples <n>] [--warmup <n>] [--mode cold|warm|both]
                        [--format text|json] [--no-save] [--list] [--check] [--write-baseline [--force]]
  every public command of the registry is a target; name one or more to benchmark only those
  --list            the targets, their scenario source and their class, and exit
  --samples <n>     warm samples per target (policy benchmark.samples)
  --warmup <n>      unsampled runs before the warm samples (policy benchmark.warmup)
  --mode            which distributions to record (default both)
  --format json     the run as one JSON document (schema majordomus/benchmark-result/v1)
  --no-save         do not write the run under .ai/local/benchmarks/
  --check           compare the run against .ai/repo/benchmarks/baseline.json; exit 10 on a
                    regression over benchmark.regression, 12 with no baseline, 15 when the
                    baseline is not comparable
  --write-baseline  write the run as the baseline; refuses a dirty tree without --force
  exit codes: 0 ok · 2 usage · 10 regression · 12 no baseline · 13 a target failed to run · 15 refused
USAGE
}

# ---------------------------------------------------------------- targets
# the public commands, minus this one: the harness does not time itself
mj_bench_targets() {
  mj_cmdreg_load || mj_die "$MJ_EX_INTERNAL" "bench: no command registry in this installation"
  mj_cmdreg_public | grep -vx bench
}
mj_bench_class() { # command -> read-only | state-mutating | generated-output-mutating
  awk -F= -v want="$1" '/^commands\.[0-9]+\.id=/ { split($1, k, "."); v = $0; sub(/^[^=]*=/, "", v); if (v == want) idx = k[2] }
    /^commands\.[0-9]+\.class=/ { split($1, k, "."); v = $0; sub(/^[^=]*=/, "", v); cls[k[2]] = v }
    END { print cls[idx] }' "$MJ_CMDREG_FLAT"
}
mj_bench_fixture_dir() { printf '%s/test/fixtures/commands' "$MJ_HOME"; }
# the scenario of a target: "fixture <id>" when the command fixture exists, else "default"
mj_bench_scenario() {
  local fx; fx="$(mj_bench_fixture_dir)/$1.json"
  if [ -f "$fx" ] && command -v jq >/dev/null 2>&1; then printf 'fixture %s' "$(jq -r '.scenarios[0].id' "$fx")"
  else printf 'default'; fi
}

# ---------------------------------------------------------------- one repository
# a disposable repository prepared for one target: the fixture's setup script when the
# suite is present, otherwise an installed repository with one commit
mj_bench_prepare() { # target, dir
  local t="$1" w="$2" fx setup
  ( cd "$w" && git init -q . && git config user.email bench@example.com && git config user.name bench ) || return 1
  fx="$(mj_bench_fixture_dir)/$t.json"
  if [ -f "$fx" ] && command -v jq >/dev/null 2>&1; then
    setup="$(jq -r '.scenarios[0].setup' "$fx")"
    # shellcheck disable=SC1090
    ( cd "$w" && FIXTURE_SETUP="$(mj_bench_fixture_dir)/setup" MJ="$MJ_BIN_DIR/majordomus" && export FIXTURE_SETUP MJ \
      && . "$FIXTURE_SETUP/$setup.sh" ) >/dev/null 2>&1 || return 1
  else
    ( cd "$w" && git commit -q --allow-empty -m base && "$MJ_BIN_DIR/majordomus" init >/dev/null 2>&1 \
      && "$MJ_BIN_DIR/majordomus" update >/dev/null 2>&1 && mkdir -p lib && echo a > lib/a && git add -A && git commit -qm base ) >/dev/null 2>&1 || return 1
  fi
  return 0
}
# the argv and the stdin file of a target's scenario, into $MJ_BENCH_ARGV (a file, one
# argument per line) and $MJ_BENCH_STDIN (a path, or empty)
mj_bench_scenario_args() { # target
  local fx; fx="$(mj_bench_fixture_dir)/$1.json"
  : > "$MJ_BENCH_ARGV"; MJ_BENCH_STDIN=""
  if [ -f "$fx" ] && command -v jq >/dev/null 2>&1; then
    jq -r '.scenarios[0].run[]' "$fx" > "$MJ_BENCH_ARGV"
    local s; s="$(jq -r '.scenarios[0].stdin // ""' "$fx")"
    [ -n "$s" ] && MJ_BENCH_STDIN="$(mj_bench_fixture_dir)/stdin/$s"
  else
    printf '%s\n' "$1" > "$MJ_BENCH_ARGV"
  fi
}
# one timed run of the target in a repository: prints milliseconds, or "timeout"
mj_bench_once() { # dir
  local w="$1" t0 t1 rc=0 a
  set --
  while IFS= read -r a; do set -- "$@" "$a"; done < "$MJ_BENCH_ARGV"
  t0="$(mj_ms)"
  if [ -n "$MJ_BENCH_STDIN" ]; then
    ( cd "$w" && MJ_TIMING='' "$MJ_BIN_DIR/majordomus" "$@" < "$MJ_BENCH_STDIN" ) >/dev/null 2>&1 || rc=$?
  else
    ( cd "$w" && MJ_TIMING='' "$MJ_BIN_DIR/majordomus" "$@" < /dev/null ) >/dev/null 2>&1 || rc=$?
  fi
  t1="$(mj_ms)"
  MJ_BENCH_LAST_RC="$rc"
  printf '%s' "$(( t1 - t0 ))"
}

# ---------------------------------------------------------------- statistics
# nearest-rank percentiles over the sample lines on stdin, one line of tab-separated
# fields: count min p50 p90 p95 p99 max mean stddev. One implementation, tested by the case.
mj_bench_stats() {
  sort -n | awk '
    { v[++n] = $1 + 0; s += v[n]; ss += v[n] * v[n] }
    function pct(p,   r) { r = int((p * n + 99) / 100); if (r < 1) r = 1; if (r > n) r = n; return v[r] }
    END {
      if (n == 0) { print "0\t0\t0\t0\t0\t0\t0\t0\t0"; exit }
      mean = s / n; var = ss / n - mean * mean; if (var < 0) var = 0
      printf "%d\t%d\t%d\t%d\t%d\t%d\t%d\t%.1f\t%.1f\n", n, v[1], pct(50), pct(90), pct(95), pct(99), v[n], mean, sqrt(var)
    }'
}

# ---------------------------------------------------------------- the run
# MJ_BENCH_ROWS collects one line per target and mode:
#   target <TAB> mode <TAB> class <TAB> scenario <TAB> status <TAB> count min p50 p90 p95 p99 max mean stddev (tab-separated)
mj_bench_run_target() { # target, samples, warmup, mode
  local t="$1" samples="$2" warmup="$3" mode="$4" cls scen w i ms cold="" warm="" status=ok
  cls="$(mj_bench_class "$t")"; scen="$(mj_bench_scenario "$t")"
  mj_bench_scenario_args "$t"
  local tmp; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.bench.XXXXXX")"
  if [ "$cls" = read-only ]; then
    w="$tmp/repo"; mkdir -p "$w"
    mj_bench_prepare "$t" "$w" || { rm -rf "$tmp"; MJ_BENCH_ROWS="$MJ_BENCH_ROWS$t${MJ_TAB}cold${MJ_TAB}$cls${MJ_TAB}$scen${MJ_TAB}setup-failed${MJ_TAB}$(printf '' | mj_bench_stats)
"; return 1; }
    ms="$(mj_bench_once "$w")"; cold="$ms"
    [ "$MJ_BENCH_LAST_RC" = 0 ] || [ "$MJ_BENCH_LAST_RC" = 10 ] || [ "$MJ_BENCH_LAST_RC" = 11 ] || status="exit-$MJ_BENCH_LAST_RC"
    i=0; while [ "$i" -lt "$warmup" ]; do mj_bench_once "$w" >/dev/null; i=$((i + 1)); done
    i=0; while [ "$i" -lt "$samples" ]; do warm="$warm$(mj_bench_once "$w")
"; i=$((i + 1)); done
  else
    # a mutation changes what the next run would measure: every sample is its own repository,
    # every sample is cold, and warm is not a thing this command has
    i=0; while [ "$i" -lt "$samples" ]; do
      w="$tmp/repo$i"; mkdir -p "$w"
      mj_bench_prepare "$t" "$w" || { status='setup-failed'; break; }
      ms="$(mj_bench_once "$w")"; cold="$cold$ms
"
      [ "$MJ_BENCH_LAST_RC" = 0 ] || [ "$MJ_BENCH_LAST_RC" = 10 ] || [ "$MJ_BENCH_LAST_RC" = 11 ] || [ "$MJ_BENCH_LAST_RC" = 15 ] || status="exit-$MJ_BENCH_LAST_RC"
      rm -rf "$tmp/repo$i"; i=$((i + 1))
    done
  fi
  rm -rf "$tmp"
  case "$mode" in cold|both) MJ_BENCH_ROWS="$MJ_BENCH_ROWS$t${MJ_TAB}cold${MJ_TAB}$cls${MJ_TAB}$scen${MJ_TAB}$status${MJ_TAB}$(printf '%s' "$cold" | mj_bench_stats)
" ;; esac
  case "$mode" in warm|both) [ -n "$warm" ] && MJ_BENCH_ROWS="$MJ_BENCH_ROWS$t${MJ_TAB}warm${MJ_TAB}$cls${MJ_TAB}$scen${MJ_TAB}$status${MJ_TAB}$(printf '%s' "$warm" | mj_bench_stats)
" ;; esac
  [ "$status" = ok ]
}

mj_bench_json() { # samples, warmup, mode -> the run as JSON on stdout
  local samples="$1" warmup="$2" mode="$3" dirty="$MJ_BENCH_DIRTY"
  {
    printf '{"schema":"%s","run_id":"%s","recorded_at":"%s","repository":{"commit":"%s","branch":"%s","dirty":%s},"environment":{"os":"%s","arch":"%s","bash":"%s","clock":"%s"},"profile":{"samples":%s,"warmup":%s,"mode":"%s"},"results":[' \
      "$MJ_BENCH_SCHEMA" "$MJ_BENCH_RUN_ID" "$(mj_now)" "$(mj_git_head)" "$(mj_json_esc "$(mj_git_branch)")" "$dirty" \
      "$(uname -s)" "$(uname -m)" "${BASH_VERSION%%(*}" "$MJ_TIMING_CLOCK" "$samples" "$warmup" "$mode"
    printf '%s' "$MJ_BENCH_ROWS" | awk -F'\t' 'NF { printf "%s{\"command\":\"%s\",\"mode\":\"%s\",\"class\":\"%s\",\"scenario\":\"%s\",\"status\":\"%s\",\"samples\":%s,\"min_ms\":%s,\"p50_ms\":%s,\"p90_ms\":%s,\"p95_ms\":%s,\"p99_ms\":%s,\"max_ms\":%s,\"mean_ms\":%s,\"stddev_ms\":%s}", (n++ ? "," : ""), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14 }'
    printf ']}\n'
  }
}
mj_bench_text() {
  printf '%-12s %-5s %-8s %5s %7s %7s %7s %7s  %s\n' command mode status n p50 p95 p99 max scenario
  printf '%s' "$MJ_BENCH_ROWS" | awk -F'\t' 'NF { printf "%-12s %-5s %-8s %5s %7s %7s %7s %7s  %s\n", $1, $2, $5, $6, $8, $10, $11, $12, $4 }'
  printf '\nslowest by warm p95 (cold where warm does not apply):\n'
  printf '%s' "$MJ_BENCH_ROWS" | awk -F'\t' 'NF { if ($2 == "warm" || !(($1) in seen)) { seen[$1] = 1; p[$1] = $10 } }
    END { for (c in p) printf "%s\t%s\n", p[c], c }' | sort -rn | head -n 5 | awk -F'\t' '{ printf "  %-12s p95 %s ms\n", $2, $1 }'
}

# ---------------------------------------------------------------- persistence
mj_bench_dir() { printf '%s/benchmarks' "$MJ_AI_LOCAL_DIR"; }
mj_bench_baseline() { printf '%s/benchmarks/baseline.json' "$MJ_AI_REPO_DIR"; }
mj_bench_save() { # json on stdin
  local d tmp; d="$(mj_bench_dir)"; mkdir -p "$d/runs"
  tmp="$(mktemp "$d/.run.XXXXXX")"; cat > "$tmp"
  mv "$tmp" "$d/runs/$MJ_BENCH_RUN_ID.json"
  cp "$d/runs/$MJ_BENCH_RUN_ID.json" "$d/.latest.tmp" && mv "$d/.latest.tmp" "$d/latest.json"
  tr -d '\n' < "$d/runs/$MJ_BENCH_RUN_ID.json" >> "$d/history.jsonl"; printf '\n' >> "$d/history.jsonl"
  printf '%s\n' "$d/runs/$MJ_BENCH_RUN_ID.json"
}

# ---------------------------------------------------------------- baseline and check
mj_bench_write_baseline() { # samples warmup mode force
  local force="$4" b tmp
  if [ "$force" != 1 ] && [ "$MJ_BENCH_DIRTY" = true ]; then
    mj_die "$MJ_EX_REFUSED" "bench: the working tree is dirty; a baseline records a commit (use --force to write one anyway)"
  fi
  b="$(mj_bench_baseline)"; mkdir -p "$(dirname "$b")"
  if [ -f "$b" ]; then
    printf 'baseline %s -> new run, by command and mode (p50/p95/p99 ms):\n' "$(mj_rel "$b")"
    mj_bench_compare_rows "$b" | awk -F'\t' '{ printf "  %-12s %-5s %s -> %s\n", $1, $2, $3, $4 }'
  fi
  tmp="$(mktemp "$(dirname "$b")/.baseline.XXXXXX")"
  mj_bench_json "$1" "$2" "$3" | sed "s#\"schema\":\"$MJ_BENCH_SCHEMA\"#\"schema\":\"$MJ_BASELINE_SCHEMA\"#" > "$tmp"
  mv "$tmp" "$b"
  printf 'wrote %s\n' "$(mj_rel "$b")"
}
# rows: command <TAB> mode <TAB> "p50/p95/p99" of the baseline <TAB> the same of this run
mj_bench_compare_rows() { # baseline file
  # the baseline rows go through a file: BSD awk refuses a newline inside a -v value
  local bf; bf="$(mktemp "${TMPDIR:-/tmp}/mj.bbase.XXXXXX")"
  mj_bench_rows_of "$1" > "$bf"
  printf '%s' "$MJ_BENCH_ROWS" | awk -F'\t' -v bf="$bf" '
    FILENAME == bf { if (NF >= 5) b[$1 SUBSEP $2] = $3 "/" $4 "/" $5; next }
    NF { k = $1 SUBSEP $2; printf "%s\t%s\t%s\t%s/%s/%s\n", $1, $2, (k in b ? b[k] : "-"), $8, $10, $11 }' "$bf" -
  rm -f "$bf"
}
# the baseline as rows: command mode p50 p95 p99, read with awk so that no jq is needed
mj_bench_rows_of() {
  tr -d '\n' < "$1" | awk '{
    while (match($0, /\{"command":"[^"]*","mode":"[^"]*"[^}]*\}/)) {
      o = substr($0, RSTART, RLENGTH); $0 = substr($0, RSTART + RLENGTH)
      c = o; sub(/.*"command":"/, "", c); sub(/".*/, "", c)
      m = o; sub(/.*"mode":"/, "", m); sub(/".*/, "", m)
      p50 = o; sub(/.*"p50_ms":/, "", p50); sub(/[,}].*/, "", p50)
      p95 = o; sub(/.*"p95_ms":/, "", p95); sub(/[,}].*/, "", p95)
      p99 = o; sub(/.*"p99_ms":/, "", p99); sub(/[,}].*/, "", p99)
      printf "%s\t%s\t%s\t%s\t%s\n", c, m, p50, p95, p99 } }'
}
mj_bench_check() {
  local b; b="$(mj_bench_baseline)"
  [ -f "$b" ] || { mj_fail bench baseline "no baseline at $(mj_rel "$b")" "majordomus bench --write-baseline"; return "$MJ_EX_MISSING"; }
  grep -q "\"schema\":\"$MJ_BASELINE_SCHEMA\"" "$b" \
    || { mj_fail bench baseline "$(mj_rel "$b") is not comparable: schema is not $MJ_BASELINE_SCHEMA" "head -c 200 $(mj_rel "$b")"; return "$MJ_EX_REFUSED"; }
  local r50 r95 r99 bad=0 line c m base cur
  r50="$(mj_pol_req benchmark.regression.p50)"; r95="$(mj_pol_req benchmark.regression.p95)"; r99="$(mj_pol_req benchmark.regression.p99)"
  while IFS="$MJ_TAB" read -r c m base cur; do
    [ -n "$c" ] || continue
    if [ "$base" = "-" ]; then mj_info bench "$c $m" "not in the baseline; no comparison" "majordomus bench --write-baseline"; continue; fi
    line="$(printf '%s\t%s\n' "$base" "$cur" | awk -F'\t' -v r50="$r50" -v r95="$r95" -v r99="$r99" '
      { split($1, b, "/"); split($2, x, "/"); split("p50 p95 p99", nm, " "); split(r50 " " r95 " " r99, th, " ")
        for (i = 1; i <= 3; i++) { d = x[i] - b[i]; rel = (b[i] > 0) ? d / b[i] : 0
          if (rel > th[i] + 0) printf "%s %s -> %s ms (+%d%%, threshold %d%%)\n", nm[i], b[i], x[i], rel * 100, th[i] * 100 } }')"
    if [ -n "$line" ]; then
      printf '%s\n' "$line" | while IFS= read -r l; do mj_fail bench "$c $m" "regression: $l" "majordomus bench $c"; done
      bad=1
    else mj_ok bench "$c $m" "within thresholds ($base -> $cur ms p50/p95/p99)"; fi
  done < <(mj_bench_compare_rows "$b")
  [ "$bad" = 0 ] || return "$MJ_EX_CONTRACT"
  return 0
}

# ---------------------------------------------------------------- the command
mj_cmd_bench() {
  local samples warmup mode=both format=text save=1 list=0 check=0 write=0 force=0 targets="" a t rc=0 failed=0
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "bench: the policy does not parse"
  samples="$(mj_pol_req benchmark.samples)"; warmup="$(mj_pol_req benchmark.warmup)"
  while [ $# -gt 0 ]; do case "$1" in
    --samples) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--samples needs a number"; samples="$2"; shift 2 ;;
    --samples=*) samples="${1#--samples=}"; shift ;;
    --warmup) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--warmup needs a number"; warmup="$2"; shift 2 ;;
    --warmup=*) warmup="${1#--warmup=}"; shift ;;
    --mode) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--mode needs cold, warm or both"; mode="$2"; shift 2 ;;
    --mode=*) mode="${1#--mode=}"; shift ;;
    --format) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--format needs text or json"; format="$2"; shift 2 ;;
    --format=*) format="${1#--format=}"; shift ;;
    --no-save) save=0; shift ;;
    --list) list=1; shift ;;
    --check) check=1; shift ;;
    --write-baseline) write=1; shift ;;
    --force) force=1; shift ;;
    --help|-h) mj_bench_usage; return 0 ;;
    -*) mj_die "$MJ_EX_USAGE" "bench: unknown option $1" ;;
    *) targets="$targets $1"; shift ;;
  esac; done
  [ "$MJ_JSON" = 1 ] && format=json
  case "$samples" in ''|*[!0-9]*) mj_die "$MJ_EX_USAGE" "bench: --samples must be a number" ;; esac
  case "$warmup" in ''|*[!0-9]*) mj_die "$MJ_EX_USAGE" "bench: --warmup must be a number" ;; esac
  case "$mode" in cold|warm|both) ;; *) mj_die "$MJ_EX_USAGE" "bench: --mode must be cold, warm or both" ;; esac
  case "$format" in text|json) ;; *) mj_die "$MJ_EX_USAGE" "bench: --format must be text or json" ;; esac
  [ "$samples" -ge 1 ] || mj_die "$MJ_EX_USAGE" "bench: --samples must be at least 1"

  # the registry is loaded here, in this shell: a load inside a command substitution would
  # set MJ_CMDREG_FLAT in the subshell only, and every class lookup after it would read nothing
  mj_cmdreg_load || mj_die "$MJ_EX_INTERNAL" "bench: no command registry in this installation"
  local all; all="$(mj_bench_targets)"
  if [ -n "$targets" ]; then
    for t in $targets; do printf '%s\n' "$all" | grep -qx "$t" || mj_die "$MJ_EX_MISSING" "bench: '$t' is not a public command of the registry (see: majordomus bench --list)"; done
  else targets="$(printf '%s\n' "$all" | tr '\n' ' ')"; fi

  if [ "$list" = 1 ]; then
    if [ "$format" = json ]; then
      printf '{"schema":1,"targets":['; local first=1
      for t in $targets; do [ "$first" = 1 ] || printf ','; first=0; printf '{"command":"%s","class":"%s","scenario":"%s"}' "$t" "$(mj_bench_class "$t")" "$(mj_bench_scenario "$t")"; done
      printf ']}\n'
    else
      printf '%-12s %-26s %s\n' command class scenario
      for t in $targets; do printf '%-12s %-26s %s\n' "$t" "$(mj_bench_class "$t")" "$(mj_bench_scenario "$t")"; done
    fi
    return 0
  fi

  MJ_BENCH_ROWS=""; MJ_BENCH_RUN_ID="b-$(mj_now_compact)-$(mj_rand16 | cut -c1-4)"
  # the tree's state is read once, before the run writes anything: a baseline's own
  # temporary file must not make the run it records look dirty
  MJ_BENCH_DIRTY=false; [ -z "$(mj_git status --porcelain 2>/dev/null)" ] || MJ_BENCH_DIRTY=true
  MJ_BENCH_ARGV="$(mktemp "${TMPDIR:-/tmp}/mj.bargv.XXXXXX")"; MJ_BENCH_STDIN=""; MJ_BENCH_LAST_RC=0
  for t in $targets; do
    mj_bench_run_target "$t" "$samples" "$warmup" "$mode" || failed=$((failed + 1))
  done
  rm -f "$MJ_BENCH_ARGV"

  if [ "$write" = 1 ]; then mj_bench_write_baseline "$samples" "$warmup" "$mode" "$force"; fi
  local saved=""
  if [ "$save" = 1 ]; then saved="$(mj_bench_json "$samples" "$warmup" "$mode" | mj_bench_save)"; fi
  if [ "$format" = json ]; then mj_bench_json "$samples" "$warmup" "$mode"
  else
    mj_bench_text
    [ -n "$saved" ] && printf '\nrun %s saved as %s\n' "$MJ_BENCH_RUN_ID" "$(mj_rel "$saved")"
  fi
  if [ "$check" = 1 ]; then mj_bench_check || rc=$?; fi
  [ "$failed" = 0 ] || { mj_err "bench: $failed target(s) did not run cleanly (status column)"; [ "$rc" = 0 ] && rc="$MJ_EX_INTERNAL"; }
  return "$rc"
}
