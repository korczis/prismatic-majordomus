# majordomus-covers: plan
# A milestone's version is inferred from the release that carried it, never typed (I2742,
# ADR 0166 decision 3). A milestone record that carries `version` is refused by
# `plan validate`. `released_in` is the first release tag containing the merge of the
# milestone's last accepted issue, and `unreleased` while the milestone is open; the shell
# engine (lib/project.sh) and the executable (plan.rs, `majordomus_plan_roadmap`) answer it
# identically over a history built here: work merged with --no-ff from a branch, two tags, a
# pre-release tag that is not a release, and a milestone whose earlier issue shipped in an
# earlier release than its last one.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm "init" >/dev/null
pj_init

accept() { # ISSUE MILESTONE — the issue accepted with its evidence on a branch, merged --no-ff
  git checkout -q -b "work/$1"
  pj_issue "$1" "$2"
  "$MJ" plan start "$1" >/dev/null
  "$MJ" plan verify "$1" >/dev/null
  "$MJ" plan evidence "$1" --covers proof --type test --command true --result ok >/dev/null
  "$MJ" plan 'done' "$1" >/dev/null
  grep -q '^evidence:' ".ai/repo/project/milestones/$2.yaml" \
    || printf 'evidence:\n  - covers: proof\n    type: test\n    command: "true"\n    result: "ok"\n    recorded_at: 2026-10-16\n' \
      >> ".ai/repo/project/milestones/$2.yaml"
  git add -A >/dev/null && git commit -qm "feat: accept $1"
  git checkout -q master 2>/dev/null || git checkout -q main
  git merge -q --no-ff -m "Merge work/$1 ($1)" "work/$1"
  sleep 1  # completed_at is to the second; the next acceptance must be later
}
released() { "$MJ" plan roadmap --json | jq -r --arg m "$1" '.milestones[] | select(.id == $m) | .released_in'; }

# --- a typed version is refused, and the refusal names what replaced it
pj_milestone first 10
printf 'version: "0.1"\n' >> .ai/repo/project/milestones/first.yaml
expect_exit 10 "$MJ" plan validate
expect_grep 'unknown keys: version'
expect_grep 'released_in'
pj_milestone first 10
expect_exit 0 "$MJ" plan validate
git add -A >/dev/null && git commit -qm "plan" >/dev/null

# --- an open milestone is unreleased
pj_milestone second 20
git add -A >/dev/null && git commit -qm "second" >/dev/null
[ "$(released first)" = unreleased ] || { echo "    an open milestone reads $(released first), expected unreleased"; exit 1; }

# --- a milestone ships in the first release containing its last accepted issue's merge.
#     F1 alone makes first DONE, and v0.26.0 carries it; F2 reopens it, and once F2 is
#     accepted the milestone is DONE again and carried by the release that carries F2.
accept F1 first
[ "$(released first)" = unreleased ] || { echo "    a DONE milestone no tag contains reads $(released first), expected unreleased"; exit 1; }
git tag v0.26.0
[ "$(released first)" = v0.26.0 ] || { echo "    first reads $(released first), expected v0.26.0"; exit 1; }
accept F2 first
[ "$(released first)" = unreleased ] || { echo "    first's last issue is in no tag, yet it reads $(released first)"; exit 1; }
git tag v0.27.0-rc.1
[ "$(released first)" = unreleased ] || { echo "    a pre-release tag was read as a release: $(released first)"; exit 1; }
git tag v0.27.0
[ "$(released first)" = v0.27.0 ] || { echo "    first reads $(released first), expected v0.27.0"; "$MJ" plan roadmap | sed 's/^/    | /'; exit 1; }
git tag v0.28.0
[ "$(released first)" = v0.27.0 ] || { echo "    a later tag moved the release to $(released first)"; exit 1; }
[ "$(released second)" = unreleased ] || { echo "    an open milestone reads $(released second)"; exit 1; }
"$MJ" plan roadmap | grep -qE '^v0\.27\.0 +DONE +first ' \
  || { echo "    the text roadmap does not show the release:"; "$MJ" plan roadmap | sed 's/^/    | /'; exit 1; }

# --- the executable answers the same, field for field
{
  printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case3102","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_plan_roadmap","arguments":{}}}\n'
} | "$RB" mcp 2>"$T/mcp.err" > "$T/frames" || { echo "    the server failed:"; sed 's/^/    | /' "$T/mcp.err"; exit 1; }
tail -n 1 "$T/frames" | jq -c '[.result.structuredContent.milestones[] | {id, released_in}]' > "$T/rs"
"$MJ" plan roadmap --json | jq -c '[.milestones[] | {id, released_in}]' > "$T/sh"
cmp -s "$T/sh" "$T/rs" || { echo "    the shell and the executable disagree:"; diff "$T/sh" "$T/rs" | sed 's/^/    | /'; exit 1; }
grep -q '"released_in":"v0.27.0"' "$T/rs" || { echo "    the executable did not infer v0.27.0: $(cat "$T/rs")"; exit 1; }
