# majordomus-covers: none
#
# The proof that the command contract is enforced dynamically rather than by a list someone
# maintains: a command that does not exist yet is added to the dispatcher and nothing else,
# and every surface that owes it something must say so without any of them being edited.
#
# This case works on a copy of the repository, because it deliberately breaks the command
# surface and must not do that to the checkout it runs from.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }

C="$T/copy"
mj_copy_repo "$C"

# Each case is run against the copy, in a disposable repository of its own — the same shape
# test/run.sh gives a case, so what runs here is what runs in CI.
i=0
run_case() {
  i=$((i + 1))
  local w="$T/case-$i"
  mkdir -p "$w"
  ( cd "$w" && git init -q . && git config user.email t@example.com && git config user.name t \
    && git commit -q --allow-empty -m init ) >/dev/null
  ( cd "$w" && ROOT="$C" MJ="$C/bin/majordomus" T="$w" bash "$C/test/cases/$1.sh" 2>&1 )
}
( cd "$C" && git init -q . && git config user.email t@example.com && git config user.name t && git add -A && git commit -qm fixture ) >/dev/null

# the copy is healthy to begin with, or nothing below means anything
expect_exit 0 "$C/scripts/generate-site-data"

# ---- add a public command and nothing else -------------------------------------------
# The dispatcher gains an arm and the usage text gains a line. No registry entry, no
# fixture, no test, no page.
# The dispatch arm is matched by its shape, not by its current contents. A probe that
# names today's command list stops mutating the day someone adds a command — which is the
# same defect as a validator that stops running, and it passes silently.
sed -i.bak 's/^\(  [a-z|][a-z|]*\))$/\1|futurecmd)/' "$C/bin/majordomus"
sed -i.bak 's/^\(  version  *\)print the version$/\1print the version\n  futurecmd            a command added without any of its surfaces/' "$C/bin/majordomus"
rm -f "$C/bin/majordomus.bak"
printf 'mj_cmd_futurecmd() { printf "future\\n"; }\n' > "$C/lib/futurecmd.sh"
grep -q 'futurecmd)' "$C/bin/majordomus" || { echo "    the dispatcher probe did not take"; exit 1; }
grep -q '^  futurecmd ' "$C/bin/majordomus" || { echo "    the usage probe did not take"; exit 1; }

# 1. the registry reconciliation fails, and says which side is missing. This is the surface
#    that catches a dispatcher-only command: the site generator describes the registry, so
#    it cannot see a command the registry has never heard of. Each surface answers for what
#    it can actually observe.
rc=0; out="$(run_case 30_command_registry)" || rc=$?
[ "$rc" != 0 ] || { echo "    30_command_registry passed with a command absent from the registry"; exit 1; }
printf '%s\n' "$out" | grep -q 'futurecmd' || {
  echo "    the registry failure does not name futurecmd"; printf '%s\n' "$out" | sed 's/^/      | /'; exit 1; }

# ---- give it a registry entry, and the next surface is named ---------------------------
python3 - "$C/share/commands.yaml" <<'PY' 2>/dev/null || { echo "    python3 absent; cannot complete the staged proof"; exit 0; }
import sys
p = sys.argv[1]
s = open(p).read()
entry = '''  - id: futurecmd
    title: futurecmd
    summary: A command added without any of its surfaces, used to prove they are required.
    category: system
    stage: inspect
    visibility: public
    class: read-only
    requires_repository: false
    requires_installed: false
    requires_task: none
    json: false
    syntax: majordomus futurecmd
    reads: []
    writes: []
    exit_codes: [0]
    demo: futurecmd-demo

'''
anchor = '  - id: help\n'
assert anchor in s
open(p, 'w').write(s.replace(anchor, entry + anchor, 1))
PY
grep -q 'id: futurecmd' "$C/share/commands.yaml" || { echo "    the registry probe did not take"; exit 1; }

# the reconciliation now passes; the missing surface has moved on to coverage and the page
rc=0; out="$(run_case 30_command_registry)" || rc=$?
[ "$rc" = 0 ] || {
  echo "    30_command_registry still fails after the registry entry was added"
  printf '%s\n' "$out" | sed 's/^/      | /'; exit 1; }

# 3. coverage is still missing, and the coverage validator names the layers
rc=0; out="$(run_case 31_command_coverage)" || rc=$?
[ "$rc" != 0 ] || { echo "    31_command_coverage passed with an untested command"; exit 1; }
printf '%s\n' "$out" | grep -q 'futurecmd: no case declares behaviour coverage' || {
  echo "    the coverage failure does not name the missing behavioural case"
  printf '%s\n' "$out" | sed 's/^/      | /'; exit 1; }
printf '%s\n' "$out" | grep -q 'futurecmd: no case declares a negative case' || {
  echo "    the coverage failure does not name the missing negative case"
  printf '%s\n' "$out" | sed 's/^/      | /'; exit 1; }

# 4. and the site still refuses, now for the documentation and the fixture rather than the
#    registry — each surface is reported in turn rather than all being one undifferentiated
#    "incomplete"
rc=0; out="$("$C/scripts/generate-site-data" 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    generate-site-data accepted a registered command with no documentation (exit $rc)"; printf '%s\n' "$out" | sed 's/^/      | /'; exit 1; }
printf '%s\n' "$out" | grep -qE 'futurecmd' || {
  echo "    the refusal does not name futurecmd"; printf '%s\n' "$out" | sed 's/^/      | /'; exit 1; }

printf '    a command added to the dispatcher alone is refused by the site, the registry\n'
printf '    reconciliation and the coverage validator, each naming what is missing\n'
