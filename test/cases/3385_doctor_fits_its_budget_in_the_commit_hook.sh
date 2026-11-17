# majordomus-covers: none
# doctor does no work whose answer cannot change a finding (I2995).
#
# A profile of doctor in the commit hook (2026-10-18, 244 s against a budget of 100 s) found
# two readings that dominated it: the roadmap doctrine looked up the release of every DONE
# milestone, each a pickaxe over the whole history, before it read the README that, here,
# lists no releases to compare them with; and the context builder's board asked every
# checkout's server for the one address it uses, this checkout's. What is decided here:
#
#   1. a README whose roadmap section lists no releases is judged without a release lookup,
#      and its finding is the one it was;
#   2. a README that lists releases is still compared with the records, both ways;
#   3. `context` asks this checkout's server alone for the board's address when it has one.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "curl not installed"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH
pj_init
REAL_GIT="$(command -v git)"

# ---------------------------------------------------------------- the records (as case 49)
milestone() { # ID ORDER
  cat > ".ai/repo/project/milestones/$1.yaml" <<Y
id: $1
title: Milestone $1
slug: $1
order: $2
priority: p1
problem: "A problem worth solving."
outcome: "The outcome once it is solved."
acceptance_criteria:
  - The outcome is reached
validation:
  - true
evidence_required:
  - proof
Y
}
accept() { # ISSUE MILESTONE
  pj_issue "$1" "$2"
  "$MJ" plan start "$1" >/dev/null
  "$MJ" plan verify "$1" >/dev/null
  "$MJ" plan evidence "$1" --covers proof --type test --command true --result ok >/dev/null
  "$MJ" plan 'done' "$1" >/dev/null
  printf 'evidence:\n  - covers: proof\n    type: test\n    command: "true"\n    result: "ok"\n    recorded_at: 2026-10-16\n' \
    >> ".ai/repo/project/milestones/$2.yaml"
  git add -A >/dev/null && git commit -qm "feat: accept $1 ($2)"
}
record() { # TAG
  local c; c="$(git rev-parse HEAD)"; mkdir -p .ai/repo/releases
  [ -f .ai/repo/releases/README.md ] || cp "$ROOT/.ai/repo/releases/README.md" .ai/repo/releases/README.md
  printf 'schema: release/v1\nversion: "%s"\ntag: %s\nchannel: stable\ncommit: %s\npublished_at: "2026-10-16T07:01:16Z"\nartifacts: []\n' \
    "${1#v}" "$1" "$c" > ".ai/repo/releases/$1.yaml"
}
readme() { # the release rows, or none for prose that points at the projection
  { printf '# Fixture\n\n## Roadmap\n\n'
    if [ $# -gt 0 ]; then
      printf '| release | adds |\n|---|---|\n'
      for v in "$@"; do printf '| %s | something |\n' "$v"; done
    else
      printf 'The roadmap is a projection: majordomus plan roadmap.\n'
    fi
    printf '\n## Contributing\n\nRead it, and AGENTS.md.\n'
  } > README.md
}
milestone alpha 10
milestone beta 20
accept A1 alpha; record v0.1.0
accept B1 beta; record v0.2.0
git add -A >/dev/null && git commit -qm records

# ---------------------------------------------------------------- a git that counts pickaxes
mkdir -p "$T/shim"
cat > "$T/shim/git" <<EOF
#!/bin/sh
for a in "\$@"; do case "\$a" in -S?*) echo pickaxe >> "$T/pickaxe.log" ;; esac; done
exec "$REAL_GIT" "\$@"
EOF
chmod +x "$T/shim/git"
doctor() { : > "$T/pickaxe.log"; PATH="$T/shim:$PATH" "$MJ" doctor > doctor.txt 2>&1 || true; }
pickaxes() { wc -l < "$T/pickaxe.log" | tr -d ' '; }

# ---------------------------------------------------------------- 1. nothing to compare
readme
doctor
grep -qE 'OK +roadmap +README.md — the roadmap section lists no releases' doctor.txt \
  || { echo "    the prose roadmap is not judged as it was:"; grep roadmap doctor.txt | sed 's/^/    | /'; exit 1; }
[ "$(pickaxes)" = 0 ] || { echo "    a README that lists no releases cost $(pickaxes) release lookup(s)"; exit 1; }
echo "    a roadmap that lists no releases is judged without one release lookup"

# ---------------------------------------------------------------- 2. a list is still compared
readme v0.1.0 v0.2.0
doctor
grep -qE 'OK +roadmap +README.md — every release it lists carried a milestone' doctor.txt \
  || { echo "    a matching list is not accepted:"; grep roadmap doctor.txt | sed 's/^/    | /'; exit 1; }
[ "$(pickaxes)" -gt 0 ] || { echo "    the shim saw no lookup where one is needed; this case would prove nothing"; exit 1; }
readme v0.1.0
doctor
grep -qE 'FAIL +roadmap +README.md — omits release\(s\) .*v0.2.0' doctor.txt \
  || { echo "    a list that hides a release is not refused:"; grep roadmap doctor.txt | sed 's/^/    | /'; exit 1; }
echo "    a roadmap that lists releases is still compared with the records"

# ---------------------------------------------------------------- 3. the board's address
RB="$(rust_bin)" || rust_bin_exit $?
cat > "$T/bin-log" <<EOF
#!/bin/sh
echo "\$*" >> "$T/bin.log"
exec "$RB" "\$@"
EOF
chmod +x "$T/bin-log"
PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT
mkdir -p "$T/state"
( XDG_STATE_HOME="$T/state" MAJORDOMUS_SHARE="$ROOT/share" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' .ai/local/state/mcp/server.json 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done
: > "$T/bin.log"
MAJORDOMUS_BIN="$T/bin-log" MAJORDOMUS_SHARE="$ROOT/share" "$MJ" context > context.txt 2>&1 \
  || { echo "    context failed:"; tail -5 context.txt | sed 's/^/    | /'; exit 1; }
grep -q 'serve status .*--checkouts this' "$T/bin.log" \
  || { echo "    context did not ask this checkout's server alone:"; grep 'serve status' "$T/bin.log" | sed 's/^/    | /'; exit 1; }
if grep 'serve status' "$T/bin.log" | grep -qv -- '--checkouts this'; then
  echo "    context asked every checkout although this one has a server:"
  grep 'serve status' "$T/bin.log" | sed 's/^/    | /'; exit 1
fi
echo "    context asks this checkout's server alone when it has one"
echo "    doctor does no work whose answer cannot change a finding"
