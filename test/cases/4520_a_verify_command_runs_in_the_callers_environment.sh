# majordomus-covers: finish
# majordomus-negative: finish
#
# A verify command runs in the environment of the worker who asked for it, not in the one
# majordomus built for itself (I4520). The launcher exports some forty variables of its own:
# MJ_ROOT, MJ_STATE_DIR, MJ_AI_DIR, MJ_LIB_DIR, MJ_SELF and the other paths and exit codes,
# and the installed launcher adds MAJORDOMUS_LAUNCHER. Scripts that take MJ_ROOT=<dir> as an
# override (scripts/ci/release-check, scripts/case-numbers) then examined the finishing
# checkout instead of their own fixture, and on 2026-10-30 `finish --verify-command
# "scripts/ci/run-plan --before-push"` failed cases 3021, 3029, 276, 850, 861, 911, 912 and
# 925-939 on PR #2683 and PR #2689, every one of which passed standalone and in the hook.
#
# So the verify command below compares the MJ_* variables it sees with the ones the caller
# exported: the same names with the same values, MJ_FINISH_OUTCOME excepted (finish never
# exports its own outcome to the command), and no MAJORDOMUS_LAUNCHER, under a launcher that
# exports it as the installer's does.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
note() { printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null; }

X="$T/../x-4520"; rm -rf "$X"; mkdir -p "$X"
# The installed launcher, as the installer writes it: it names itself and execs the tool.
printf '#!/bin/sh\nMAJORDOMUS_LAUNCHER="%s"; export MAJORDOMUS_LAUNCHER\nexec "%s" "$@"\n' \
  "$X/launcher" "$MJ" > "$X/launcher"
# What the verify command sees, written outside the tree it verifies, and compared.
cat > "$X/verify.sh" <<EOF
#!/bin/sh
env | grep '^MJ_' | LC_ALL=C sort > "$X/seen.env"
env | grep '^MAJORDOMUS_LAUNCHER=' > "$X/launcher.env" || true
cmp -s "$X/caller.env" "$X/seen.env" && [ ! -s "$X/launcher.env" ]
EOF
chmod +x "$X/launcher" "$X/verify.sh"

# An MJ_* variable of the caller's own reaches the command with the caller's value; one the
# caller exported under a name the launcher also exports is the caller's, not the launcher's.
MJ_CASE_4520_KEEP='the caller s value'; export MJ_CASE_4520_KEEP
MJ_JSON=0; export MJ_JSON
env | grep '^MJ_' | LC_ALL=C sort > "$X/caller.env"
MJ_FINISH_OUTCOME=blocked; export MJ_FINISH_OUTCOME

"$MJ" start "t1" --scope lib >/dev/null
echo b >> lib/a
note
expect_exit 0 "$X/launcher" finish --outcome completed --verify-command "sh $X/verify.sh"
expect_grep 'OK +verification .*exit 0'
expect_grep '^MJ_CASE_4520_KEEP=the caller s value$' "$X/seen.env"
expect_grep '^MJ_JSON=0$' "$X/seen.env"
for v in MJ_ROOT MJ_STATE_DIR MJ_AI_DIR MJ_AI_LOCAL_DIR MJ_LIB_DIR MJ_BIN_DIR MJ_SELF \
  MJ_SCOPE_FILE MJ_DOCTRINE_CMD MJ_FINISH_OUTCOME; do
  expect_no_grep "^$v=" "$X/seen.env"
done
[ ! -s "$X/launcher.env" ] || { printf '    the verify command saw %s\n' "$(cat "$X/launcher.env")"; exit 1; }

# The finishing checkout is not where a nested tool looks: a verify command that asks for
# MJ_ROOT finds it unset, as a worker's shell has it.
git add -A && git commit -qm t1
unset MJ_FINISH_OUTCOME
"$MJ" start "t2" --scope lib >/dev/null
echo c >> lib/a
note
expect_exit 0 "$MJ" finish --outcome completed --verify-command 'test -z "${MJ_ROOT+set}${MJ_STATE_DIR+set}${MJ_AI_DIR+set}"'
expect_grep 'OK +verification .*exit 0'

rm -rf "$X"
echo "    a verify command sees the caller's MJ_* variables with the caller's values, none of the launcher's and no MAJORDOMUS_LAUNCHER"
