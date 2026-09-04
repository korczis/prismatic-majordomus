# majordomus-covers: none
#
# Test coverage is computed, not remembered. This case joins the public command surface in
# share/commands.yaml to the coverage each test case declares about itself, and fails when a
# command is missing a layer it is owed. No command is named here: a command added to the
# registry is required to have coverage from that moment, and a maintainer who adds one
# without tests is told which layer is missing rather than discovering it later.
#
# Coverage is declared rather than inferred. It cannot be read reliably from the source: the
# negative assertions for handover and checkpoint are written as `bash -c "... | '$MJ' cmd"`
# because those commands read stdin, and the surface case asserts a failure for every command
# through a loop variable. Both are invisible to any scan. A filename tells you even less.
#
# Layers:
#   behaviour   the case exercises the command's normal path through the real binary
#   negative    the case asserts a named failure mode of that command, with its exit code
#   lifecycle   the case walks a multi-command workflow to an accepted or refused end
#
# Every case in this suite is end to end by construction: test/run.sh builds a fresh git
# repository in a temporary directory and invokes bin/majordomus in it, so there is no
# separate end-to-end layer to declare and no second runner to keep in step.
. "$ROOT/test/lib.sh"
MJ_BIN_DIR="$ROOT/bin"; MJ_LIB_DIR="$ROOT/lib"; export MJ_BIN_DIR MJ_LIB_DIR
# shellcheck source=../../lib/common.sh
. "$ROOT/lib/common.sh"

REG="$ROOT/share/commands.yaml"
FLAT="$T/commands.flat"
mj_yaml_flatten "$REG" > "$FLAT" || { echo "    share/commands.yaml does not parse"; exit 1; }

# ---- the surface that is owed coverage, read from the registry
public=""
i=0
while [ -n "$(sed -n "s|^commands\.$i\.id=||p" "$FLAT")" ]; do
  if [ "$(sed -n "s|^commands\.$i\.visibility=||p" "$FLAT")" = public ]; then
    public="$public $(sed -n "s|^commands\.$i\.id=||p" "$FLAT")"
  fi
  i=$((i+1))
done
[ -n "$public" ] || { echo "    the registry declares no public command"; exit 1; }

# ---- the coverage the suite declares about itself
# One line per claim: "<command> <layer> <case>". Built by reading the headers, so a case
# that stops declaring a command stops covering it.
CLAIMS="$T/claims.txt"; : > "$CLAIMS"
declared_cases=0
for f in "$ROOT"/test/cases/*.sh; do
  case_name="$(basename "$f" .sh)"
  head_covers="$(sed -n 's/^# majordomus-covers: *//p' "$f" | head -1)"
  [ -n "$head_covers" ] || continue
  declared_cases=$((declared_cases + 1))
  [ "$head_covers" = none ] && continue
  for c in $head_covers; do printf '%s behaviour %s\n' "$c" "$case_name" >> "$CLAIMS"; done
  for c in $(sed -n 's/^# majordomus-negative: *//p' "$f" | head -1); do
    printf '%s negative %s\n' "$c" "$case_name" >> "$CLAIMS"
  done
  life="$(sed -n 's/^# majordomus-lifecycle: *//p' "$f" | head -1)"
  [ -n "$life" ] && printf -- '- lifecycle:%s %s\n' "$life" "$case_name" >> "$CLAIMS"
done
[ "$declared_cases" -ge 15 ] || { echo "    only $declared_cases cases declare coverage; the headers are not being read"; exit 1; }

covers() { grep -qE "^$1 $2 " "$CLAIMS"; }          # command, layer
cases_for() { grep -E "^$1 $2 " "$CLAIMS" | awk '{print $3}' | sort -u | tr '\n' ' '; }

# ---- a declared cover must name a command that exists. A header pointing at a command
#      that was renamed or removed is a broken reference, not harmless documentation.
for c in $(awk '$1 != "-" {print $1}' "$CLAIMS" | sort -u); do
  printf '%s\n' $public | grep -qx "$c" || {
    echo "    a test case declares coverage of '$c', which is not a public command"
    echo "    reproduce: grep -rn 'majordomus-covers\\|majordomus-negative' test/cases/ | grep $c"
    exit 1; }
done

# ---- the requirement: every public command is exercised and refuted
missing=0
for c in $public; do
  covers "$c" behaviour || { echo "    $c: no case declares behaviour coverage"; missing=1; }
  covers "$c" negative  || { echo "    $c: no case declares a negative case"; missing=1; }
done
[ "$missing" = 0 ] || {
  echo "    add the case, then declare it in that case's header:"
  echo "      # majordomus-covers: <command>      the normal path"
  echo "      # majordomus-negative: <command>    a named failure mode"
  exit 1; }

# ---- the workflow layers. One accepted path and one refusal path across command
#      boundaries, because a contract that holds for each command separately can still fail
#      to hold across a sequence of them.
grep -q '^- lifecycle:accepted ' "$CLAIMS" || {
  echo "    no case declares '# majordomus-lifecycle: accepted' — a full workflow to an accepted finish"; exit 1; }
grep -q '^- lifecycle:refused ' "$CLAIMS" || {
  echo "    no case declares '# majordomus-lifecycle: refused' — a workflow that violates the"
  echo "    contract, is refused, is repaired, and is then accepted"; exit 1; }

# ---- the matrix, computed. Printed rather than written down anywhere.
printf '\n    %-12s %-28s %s\n' command behaviour negative
for c in $public; do
  printf '    %-12s %-28s %s\n' "$c" "$(cases_for "$c" behaviour)" "$(cases_for "$c" negative)"
done
printf '    lifecycle accepted: %s\n' "$(grep '^- lifecycle:accepted ' "$CLAIMS" | awk '{print $3}' | tr '\n' ' ')"
printf '    lifecycle refused:  %s\n' "$(grep '^- lifecycle:refused ' "$CLAIMS" | awk '{print $3}' | tr '\n' ' ')"

# ---- the check is not vacuous: a command with no coverage must be detected. The probe adds
#      a command to the surface and asserts that the requirement above rejects it.
probe_public="$public nosuchcommand"
probe_missing=0
for c in $probe_public; do
  covers "$c" behaviour || probe_missing=1
done
[ "$probe_missing" = 1 ] || { echo "    the coverage check is vacuous: an uncovered command passed it"; exit 1; }

printf '    %s public commands, every one exercised and refuted\n' "$(printf '%s\n' $public | wc -w | tr -d ' ')"
