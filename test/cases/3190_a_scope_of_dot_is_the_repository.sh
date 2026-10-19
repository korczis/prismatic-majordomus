# majordomus-covers: start check
# majordomus-negative: check
# A task scoped `.` claims the whole repository.
#
# `start --scope .` is accepted, and cases start tasks that way, but the path matcher kept `.`
# as a literal and asked whether a changed file began with "./", so `.` contained no path at
# all. Nothing noticed while the task changed nothing, or only merged: the first ordinary
# commit under such a task (the 0.29.0 version bump, 2026-10-17) made `finish --check`, and
# the pre-push hook that runs it, judge all 248 files it touched as outside the claimed scope.
#
# Now `.` contains every path inside the repository and still none that escapes it.
. "$ROOT/test/lib.sh"
. "$ROOT/lib/common.sh"

# the matcher itself
for p in Cargo.toml lib/common.sh ./docs/README.md a/b/c.txt; do
  mj_path_contains . "$p" || { echo "    '.' does not contain $p"; exit 1; }
done
for p in ../outside /etc/passwd lib/../../x; do
  if mj_path_contains . "$p"; then echo "    '.' contains $p, which escapes the repository"; exit 1; fi
done
mj_path_contains lib lib/common.sh || { echo "    lib no longer contains lib/common.sh"; exit 1; }
if mj_path_contains lib library/x; then echo "    lib contains library/x"; exit 1; fi

# a task scoped `.` that commits an ordinary change is within its scope
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib/auth && echo x > lib/auth/a.txt && echo v > VERSION && git add . && git commit -qm base
expect_exit 0 "$MJ" start "the whole repository" --scope .
echo y >> VERSION && echo z >> lib/auth/a.txt && git commit -qam "an ordinary change"
expect_exit 0 "$MJ" check
expect_grep 'OK +scope'
expect_no_grep 'outside claimed scope'

# a narrower scope still refuses what lies outside it
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
expect_exit 0 "$MJ" start "auth only" --scope lib/auth
echo w >> VERSION
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +VERSION — outside claimed scope'
