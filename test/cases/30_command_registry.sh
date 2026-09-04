# majordomus-covers: none
#
# The command registry and the dispatcher must describe the same surface. Neither derives
# from the other — bin/majordomus decides what executes, share/commands.yaml decides what a
# command means — so this case is the reconciliation between them, and it reads both from
# source rather than restating either.
#
# It runs against the repository's own files, because the subject is this repository's
# command surface, not a fixture's.
. "$ROOT/test/lib.sh"
REG="$ROOT/share/commands.yaml"
[ -f "$REG" ] || { echo "    no command registry at share/commands.yaml"; exit 1; }

# The registry must parse with the tool's own parser, not with a permissive one: a registry
# the tool cannot read is a registry nothing can enforce.
MJ_BIN_DIR="$ROOT/bin"; MJ_LIB_DIR="$ROOT/lib"; export MJ_BIN_DIR MJ_LIB_DIR
# shellcheck source=../../lib/common.sh
. "$ROOT/lib/common.sh"

"$MJ" init >/dev/null

FLAT="$T/commands.flat"
mj_yaml_flatten "$REG" > "$FLAT" 2>"$T/flat.err" || {
  echo "    share/commands.yaml does not parse: $(cat "$T/flat.err")"; exit 1; }
grep -qx 'version=1' "$FLAT" || { echo "    registry version must be 1"; exit 1; }

get() { sed -n "s|^commands\.$1\.$2=||p" "$FLAT"; }        # index, field -> value
count=0; while [ -n "$(get "$count" id)" ]; do count=$((count+1)); done
[ "$count" -ge 15 ] || { echo "    registry holds only $count commands; the dispatcher carries more"; exit 1; }

ids=""; public=""; internal=""
i=0; while [ "$i" -lt "$count" ]; do
  id="$(get "$i" id)"; vis="$(get "$i" visibility)"
  ids="$ids $id"
  case "$vis" in
    public)   public="$public $id" ;;
    internal) internal="$internal $id" ;;
    *) echo "    $id declares visibility '$vis' (public | internal)"; exit 1 ;;
  esac
  i=$((i+1))
done

# 1. no duplicate ids. Discovery that silently picks the first or the last entry is worse
#    than no discovery, because it looks like it worked.
dupes="$(printf '%s\n' $ids | sort | uniq -d)"
[ -z "$dupes" ] || { echo "    duplicate command id(s): $dupes"; exit 1; }

# 2. unknown keys are errors here as everywhere else in this repository
unk="$(mj_yaml_unknown_keys "$FLAT" "$ROOT/share/allow/commands.txt" || true)"
[ -z "$unk" ] || { echo "    unknown key(s) in share/commands.yaml: $(printf '%s' "$unk" | tr '\n' ' ')"; exit 1; }

# 3. every dispatched command has a registry entry, and every public entry is dispatched.
#    The dispatch table is read from the binary, so adding a command there adds it here.
dispatched="$(grep -oE '^  [a-z|]+\)$' "$ROOT/bin/majordomus" | tr -d ' )' | tr '|' '\n' | sort -u)"
[ "$(printf '%s\n' "$dispatched" | wc -w | tr -d ' ')" -ge 15 ] || {
  echo "    could not read the dispatch table from bin/majordomus"; exit 1; }
for c in $dispatched; do
  printf '%s\n' $ids | grep -qx "$c" || {
    echo "    dispatched but absent from share/commands.yaml: $c"; exit 1; }
  printf '%s\n' $public | grep -qx "$c" || {
    echo "    dispatched and listed in the help text, but classified internal: $c"; exit 1; }
done
# version is dispatched ahead of the option parser, so it is not in the case list above
printf '%s\n' $public | grep -qx version || { echo "    version is not a public registry entry"; exit 1; }
for c in $public; do
  [ "$c" = version ] && continue
  printf '%s\n' "$dispatched" | grep -qx "$c" || {
    echo "    public in the registry but not dispatched by bin/majordomus: $c"; exit 1; }
done

# 4. a command is public exactly when the usage text lists it. That is the whole definition
#    of internal, and it is checked in both directions so that "internal" cannot become a
#    place to hide a command from the obligations a public one carries.
expect_exit 0 "$MJ" --help
for c in $public; do
  expect_grep "^  $c" || { echo "    public command not listed in --help: $c"; exit 1; }
done
for c in $internal; do
  expect_no_grep "^  $c " || { echo "    internal command is listed in --help: $c"; exit 1; }
done

# 5. closed vocabularies stay closed
i=0; while [ "$i" -lt "$count" ]; do
  id="$(get "$i" id)"
  case "$(get "$i" class)" in
    read-only|state-mutating|generated-output-mutating) ;;
    *) echo "    $id declares class '$(get "$i" class)'"; exit 1 ;;
  esac
  case "$(get "$i" requires_task)" in
    required|optional|none) ;;
    *) echo "    $id declares requires_task '$(get "$i" requires_task)'"; exit 1 ;;
  esac
  st="$(get "$i" stage)"
  grep -qE "^stages\.[0-9]+\.id=$st$" "$FLAT" || {
    echo "    $id names stage '$st', which the registry does not declare"; exit 1; }
  case "$(get "$i" syntax)" in
    "majordomus $id"*|"majordomus --help") ;;
    *) echo "    $id: syntax does not begin with 'majordomus $id'"; exit 1 ;;
  esac
  # a public command names the demonstration that represents it; an internal one names none
  d="$(get "$i" demo)"
  case "$(get "$i" visibility)" in
    public)   [ -n "$d" ] && [ "$d" != none ] || { echo "    public command $id declares no demo"; exit 1; } ;;
    internal) [ -z "$d" ] || [ "$d" = none ] || { echo "    internal command $id declares demo '$d'"; exit 1; } ;;
  esac
  i=$((i+1))
done

# 6. the declared exit codes are the codes the implementation can actually produce. Every
#    MJ_EX_* constant a command's module references must be declared, so a new failure path
#    cannot ship undocumented.
code_of() { case "$1" in OK) echo 0 ;; USAGE) echo 2 ;; CONTRACT) echo 10 ;; DRIFT) echo 11 ;;
  MISSING) echo 12 ;; INTERNAL) echo 13 ;; REFUSED) echo 15 ;; esac; }
i=0; while [ "$i" -lt "$count" ]; do
  id="$(get "$i" id)"
  if [ -f "$ROOT/lib/$id.sh" ]; then
    declared="$(sed -n "s|^commands\.$i\.exit_codes\.[0-9]*=||p" "$FLAT" | sort -n)"
    for name in $(grep -oE 'MJ_EX_(OK|USAGE|CONTRACT|DRIFT|MISSING|INTERNAL|REFUSED)' "$ROOT/lib/$id.sh" | sed 's/MJ_EX_//' | sort -u); do
      c="$(code_of "$name")"
      printf '%s\n' "$declared" | grep -qx "$c" || {
        echo "    $id: lib/$id.sh can exit $c (MJ_EX_$name) but the registry does not declare it"; exit 1; }
    done
  fi
  i=$((i+1))
done

# 7. the checks above are not vacuous. Each mutation must be caught by the check that claims
#    to catch it, or the check is decoration rather than enforcement.
probe="$T/probe.yaml"
sed 's|^    stage: setup$|    stage: nosuchstage|' "$REG" > "$probe"
mj_yaml_flatten "$probe" > "$T/probe.flat" 2>/dev/null
grep -q '^commands\.0\.stage=nosuchstage$' "$T/probe.flat" || { echo "    the stage probe did not take"; exit 1; }
if grep -qE '^stages\.[0-9]+\.id=nosuchstage$' "$T/probe.flat"; then
  echo "    stage check is vacuous: the probe stage resolves"; exit 1; fi

sed 's|^  - id: doctor$|  - id: init|' "$REG" > "$probe"
mj_yaml_flatten "$probe" > "$T/probe.flat" 2>/dev/null
pids="$(sed -n 's|^commands\.[0-9]*\.id=||p' "$T/probe.flat")"
[ -n "$(printf '%s\n' $pids | sort | uniq -d)" ] || {
  echo "    duplicate-id check is vacuous: the probe produced no duplicate"; exit 1; }

printf 'ok   registry: %s commands, %s public, reconciled against the dispatch table\n' \
  "$count" "$(printf '%s\n' $public | wc -w | tr -d ' ')"
