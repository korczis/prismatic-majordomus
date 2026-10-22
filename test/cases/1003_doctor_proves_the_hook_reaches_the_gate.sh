# majordomus-covers: doctor
# majordomus-negative: doctor
# claims: dispatcher-wiring
. "$ROOT/test/lib.sh"
# A hook that contains the invoking line is not a hook that runs it. The audit's survivor
# (F04): an `exit 0` before the line, and doctor still said "wired". Each mutation below is
# applied to a hook doctor first accepts, so every FAIL is the mutation's and nothing else's,
# and each hook is also run for real against a gate that fails, so the expectation is what
# git would actually do with it, not what this file believes.
"$MJ" init >/dev/null
"$MJ" update >/dev/null
git config core.hooksPath .githooks
mkdir -p .githooks

gate_fails() { printf '#!/bin/sh\nexit 3\n' > "$T/failing-gate"; chmod +x "$T/failing-gate"; }
gate_fails
# What git would see: the hook's status when the gate it names fails. The hook is copied
# with the invocation pointed at a gate that always fails, and run.
hook_status() { # hook
  sed -E "s#$MJ doctor#$T/failing-gate doctor#" "$1" > "$T/probe-hook"; chmod +x "$T/probe-hook"
  ( cd "$T" && "$T/probe-hook" </dev/null >/dev/null 2>&1 ); echo $?
}
pre_push() { printf '#!/bin/sh\n%s finish --check || exit $?\n' "$MJ" > .githooks/pre-push; chmod +x .githooks/pre-push; }
pre_push
fail() { printf '    %s\n' "$*"; exit 1; }
hook() { printf '#!/bin/sh\n%s\n' "$1" > .githooks/pre-commit; chmod +x .githooks/pre-commit; }

# the control: the gate's status reaches git, and doctor says so
hook "$MJ doctor || exit \$?"
[ "$(hook_status .githooks/pre-commit)" = 3 ] || fail "control hook does not propagate the gate's failure"
expect_exit 0 "$MJ" doctor
expect_grep 'OK   wiring +doctor-on-commit — wired via .githooks/pre-commit$'

# an exit before the line: the gate never runs, every commit passes
hook "exit 0
$MJ doctor || exit \$?"
[ "$(hook_status .githooks/pre-commit)" = 0 ] || fail "early-exit hook was expected to pass a failing gate"
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL wiring +doctor-on-commit — .githooks/pre-commit never reaches the invocation: line 2 ends the hook first \(exit 0\)'

# exec of something else replaces the shell: same effect
hook "exec true
$MJ doctor || exit \$?"
expect_exit 10 "$MJ" doctor
expect_grep 'never reaches the invocation: line 2 ends the hook first \(exec true\)'

# a commented-out invocation is text, not a command
hook "# $MJ doctor || exit \$?"
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL wiring +doctor-on-commit — majordomus doctor is not invoked by .githooks/pre-commit'

# swallowing forms beyond `|| true`: each turns the gate's failure into a pass
for clause in '|| :' '|| echo doctor failed' '|| exit 0' '; exit 0' '; true'; do
  hook "$MJ doctor $clause"
  [ "$(hook_status .githooks/pre-commit)" = 0 ] || fail "'$clause' was expected to pass a failing gate"
  expect_exit 10 "$MJ" doctor
  expect_grep 'FAIL wiring +doctor-on-commit — .githooks/pre-commit invokes it but swallows the exit code'
done

# what is NOT a defect: each keeps the failure, and doctor must not cry wolf
# - a conditional early exit (the pre-push shape: "nothing to check, leave")
# - an exit inside a function or a branch, a here-document that mentions exit
# - `|| exit 1`, a braced failure block, the dispatcher's deferred status
hook "[ -n \"\${SKIP_NOTHING:-}\" ] && exit 0
if [ -f /nonexistent ]; then
  exit 0
fi
helper() { exit 0; }
cat >/dev/null <<'EOF'
exit 0
EOF
$MJ doctor || exit 1"
[ "$(hook_status .githooks/pre-commit)" = 1 ] || fail "conditional-exit hook was expected to propagate"
expect_exit 0 "$MJ" doctor
expect_grep 'OK   wiring +doctor-on-commit — wired via .githooks/pre-commit$'

hook "$MJ doctor || { echo 'doctor failed' >&2; exit 1; }"
[ "$(hook_status .githooks/pre-commit)" = 1 ] || fail "braced failure block was expected to propagate"
expect_exit 0 "$MJ" doctor

hook "rc=0
$MJ doctor || rc=1
exit \$rc"
[ "$(hook_status .githooks/pre-commit)" = 1 ] || fail "deferred status was expected to propagate"
expect_exit 0 "$MJ" doctor

# a dispatcher that leaves before it dispatches never runs the subhook that holds the line
mkdir -p .githooks/pre-commit.d
printf '#!/bin/sh\n%s doctor || exit $?\n' "$MJ" > .githooks/pre-commit.d/10-majordomus.sh
chmod +x .githooks/pre-commit.d/10-majordomus.sh
cat > .githooks/pre-commit <<'HOOK'
#!/bin/sh
exit 0
for f in "$(git rev-parse --show-toplevel)/.githooks/pre-commit.d"/*; do "$f" || exit $?; done
HOOK
chmod +x .githooks/pre-commit
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL wiring +doctor-on-commit — .githooks/pre-commit never reaches pre-commit.d/: line 2 ends the hook first \(exit 0\)'

# the same dispatcher without the exit: wired through the subhook
sed -i.bak '2d' .githooks/pre-commit && rm -f .githooks/pre-commit.bak
expect_exit 0 "$MJ" doctor
expect_grep 'OK   wiring +doctor-on-commit — wired via .githooks/pre-commit.d/10-majordomus.sh'
